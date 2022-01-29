use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashSet, VecDeque},
    ops::Bound,
};

use super::{
    chunk::{MutableChunkAccess, CHUNK_LENGTH, CHUNK_LENGTH_2},
    generation::SurfaceHeightmap,
    BlockPos,
};
use crate::{
    codec::{
        encode::{Encode, Encoder},
        NodeKind,
    },
    prelude::*,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
#[repr(transparent)]
pub struct LightValue(pub u16);

impl<W: std::io::Write> Encode<W> for LightValue {
    const KIND: NodeKind = NodeKind::UnsignedVarInt;

    fn encode(&self, encoder: Encoder<W>) -> Result<()> {
        encoder.encode(&self.0)
    }
}

pub const SKY_LIGHT_BITS: u16 = 4;
pub const BLOCK_LIGHT_BITS: u16 = 4;

pub const SKY_LIGHT_MASK: u16 = ((1 << SKY_LIGHT_BITS) - 1) << BLOCK_LIGHT_BITS;
pub const BLOCK_LIGHT_MASK: u16 = (1 << BLOCK_LIGHT_BITS) - 1;

pub const FULL_SKY_LIGHT: LightValue = LightValue::pack(15, 0);

impl LightValue {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn pack(sky: u16, block: u16) -> Self {
        let mut val = 0;
        val |= block & BLOCK_LIGHT_MASK;
        val |= (sky << BLOCK_LIGHT_BITS) & SKY_LIGHT_MASK;
        Self(val)
    }

    pub const fn raw(self) -> u16 {
        self.0
    }

    pub const fn sky(self) -> u16 {
        self.0 >> BLOCK_LIGHT_BITS
    }

    pub const fn block(self) -> u16 {
        self.0 & BLOCK_LIGHT_MASK
    }

    pub const fn intensity(self) -> u16 {
        if self.sky() > self.block() {
            self.sky()
        } else {
            self.block()
        }
    }

    pub fn combine_max(self, other: LightValue) -> LightValue {
        let block = u16::max(self.0 & BLOCK_LIGHT_MASK, other.0 & BLOCK_LIGHT_MASK);
        let sky = u16::max(self.0 & SKY_LIGHT_MASK, other.0 & SKY_LIGHT_MASK);
        LightValue(sky | block)
    }
}

// the basic idea for this comes from the Seed of Andromeda light update code.
// there used to be a technical blog post about it on their site, but that has
// since gone defunct
//
// it should not matter what order we tackle propagation in, since any low
// values we set will get overwritten with a higher value later if need be.
//
// SoA lighting code: https://github.com/RegrowthStudios/SoACode-Public/blob/develop/SoA/VoxelLightEngine.cpp
#[derive(Debug, Default)]
pub struct LightUpdateQueues {
    block_removal: VecDeque<(BlockPos, u16)>,
    block_update: VecDeque<(BlockPos, u16)>,
    sky_removal: VecDeque<(BlockPos, u16)>,
    sky_update: VecDeque<(BlockPos, u16)>,
    visited: HashSet<BlockPos>,
}

impl LightUpdateQueues {
    pub fn queue_blocklight_updates<I>(&mut self, access: &mut MutableChunkAccess, iter: I)
    where
        I: Iterator<Item = (BlockPos, u16)>,
    {
        for (pos, new_light) in iter {
            if !self.visited.insert(pos) {
                continue;
            }

            let prev_light = access.light(pos).unwrap();

            let id = access.block(pos).unwrap();
            if access.registry().light_transmissible(id) {
                self.sky_removal.push_back((pos, prev_light.sky()));
                self.block_removal.push_back((pos, prev_light.block()));
            }

            match new_light.cmp(&prev_light.block()) {
                Ordering::Equal => {}
                Ordering::Less => self.block_removal.push_back((pos, prev_light.block())),
                Ordering::Greater => self.block_update.push_back((pos, new_light)),
            }
        }

        self.visited.clear();
    }

    pub fn queue_skylight_updates(
        &mut self,
        access: &mut MutableChunkAccess,
        x: i32,
        z: i32,
        min_y: i32,
        max_y: i32, // exclusive bound
        light: u16,
    ) {
        // log::debug!(
        //     "queued skylight updates: ({x},{z}) -> min={min_y}, max={max_y},
        // light={light}" );
        for y in min_y..max_y {
            let pos = BlockPos { x, y, z };

            let prev_light = access.light(pos).unwrap().sky();

            match light.cmp(&prev_light) {
                Ordering::Equal => {}
                Ordering::Less => self.sky_removal.push_back((pos, prev_light)),
                Ordering::Greater => self.sky_update.push_back((pos, light)),
            }
        }
    }
}

pub(crate) fn propagate_block_light(
    queues: &mut LightUpdateQueues,
    access: &mut MutableChunkAccess,
) {
    for &(pos, _) in queues.block_removal.iter() {
        access.set_block_light(pos, 0).unwrap();
    }

    while let Some((pos, light)) = queues.block_removal.pop_front() {
        let dirs = [
            pos.offset([1, 0, 0]),
            pos.offset([-1, 0, 0]),
            pos.offset([0, 1, 0]),
            pos.offset([0, -1, 0]),
            pos.offset([0, 0, 1]),
            pos.offset([0, 0, -1]),
        ];

        for dir in dirs.into_iter() {
            let neighbor_light = access.light(dir).unwrap().block();

            if neighbor_light > 0 && neighbor_light < light {
                access.set_block_light(dir, 0).unwrap();
                queues.block_removal.push_back((dir, light - 1));
            } else if neighbor_light > 0 {
                queues.block_update.push_back((dir, 0));
            }
        }
    }

    while let Some((pos, queue_light)) = queues.block_update.pop_front() {
        let current_light = access.light(pos).unwrap().block();
        let queue_light = u16::max(queue_light, current_light);
        if queue_light != current_light {
            access.set_block_light(pos, queue_light).unwrap();
        }

        if queue_light == 0 {
            continue;
        }

        let dirs = [
            pos.offset([1, 0, 0]),
            pos.offset([-1, 0, 0]),
            pos.offset([0, 1, 0]),
            pos.offset([0, -1, 0]),
            pos.offset([0, 0, 1]),
            pos.offset([0, 0, -1]),
        ];

        for dir in dirs.into_iter() {
            let neighbor_light = access.light(dir).unwrap().block();
            let new_light = u16::max(queue_light - 1, neighbor_light);

            let id = access.block(dir).unwrap();
            let neighbor_transmissible = access.registry().light_transmissible(id);

            if new_light != neighbor_light && neighbor_transmissible {
                access.set_block_light(dir, new_light).unwrap();
                queues.block_update.push_back((dir, new_light));
            }
        }
    }
}

pub(crate) fn propagate_sky_light(queues: &mut LightUpdateQueues, access: &mut MutableChunkAccess) {
    for &(pos, _) in queues.sky_removal.iter() {
        access.set_sky_light(pos, 0).unwrap();
    }

    while let Some((pos, light)) = queues.sky_removal.pop_front() {
        let dirs = [
            pos.offset([1, 0, 0]),
            pos.offset([-1, 0, 0]),
            pos.offset([0, 1, 0]),
            pos.offset([0, -1, 0]),
            pos.offset([0, 0, 1]),
            pos.offset([0, 0, -1]),
        ];

        for dir in dirs.into_iter() {
            let neighbor_light = access.light(dir).unwrap().sky();

            if neighbor_light > 0 && neighbor_light < light {
                access.set_sky_light(dir, 0).unwrap();
                queues.sky_removal.push_back((dir, light - 1));
            } else if neighbor_light > 0 {
                queues.sky_update.push_back((dir, 0));
            }
        }
    }

    while let Some((pos, queue_light)) = queues.sky_update.pop_front() {
        let current_light = access.light(pos).unwrap().sky();
        let queue_light = u16::max(queue_light, current_light);
        if queue_light != current_light {
            access.set_sky_light(pos, queue_light).unwrap();
        }

        if queue_light == 0 {
            continue;
        }

        let dirs = [
            pos.offset([1, 0, 0]),
            pos.offset([-1, 0, 0]),
            pos.offset([0, 1, 0]),
            pos.offset([0, -1, 0]),
            pos.offset([0, 0, 1]),
            pos.offset([0, 0, -1]),
        ];

        for dir in dirs.into_iter() {
            let neighbor_light = access.light(dir).unwrap().sky();
            let new_light = u16::max(queue_light - 1, neighbor_light);

            let id = access.block(dir).unwrap();
            let neighbor_transmissible = access.registry().light_transmissible(id);

            if new_light != neighbor_light && neighbor_transmissible {
                access.set_sky_light(dir, new_light).unwrap();
                queues.sky_update.push_back((dir, new_light));
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct SkyLightNode {
    // each entry in this map represents a range that starts at a height specified by the key, and
    // ends at the next entry's key (exclusive)
    //
    // INVARIANT: for a node N, len(N) must be odd.
    // INVARIANT: for a node N, solid(N[a]) must not equal solid(N[a + 1])
    intervals: BTreeMap<i32, bool>,
}

impl SkyLightNode {
    fn init(y: i32, solid: bool) -> Self {
        let mut intervals = BTreeMap::new();
        intervals.insert(y, solid);
        Self { intervals }
    }

    fn lookup(&self, y: i32) -> (i32, i32, bool) {
        let (min, solid) = match self
            .intervals
            .range((Bound::Unbounded, Bound::Included(y)))
            .rev()
            .next()
        {
            Some((&min, &solid)) => (min, solid),
            // TODO: what if the bottom-most node is not solid?
            None => (i32::MIN, true),
        };

        let max = match self
            .intervals
            .range((Bound::Excluded(y), Bound::Unbounded))
            .next()
        {
            Some((&max, _)) => max,
            None => i32::MAX,
        };

        (min, max, solid)
    }

    pub fn update(&mut self, y: i32, solid: bool) {
        let (min, max, interval_solid) = self.lookup(y);

        if solid != interval_solid {
            match y == min {
                true => drop(self.intervals.remove(&min)),
                false => drop(self.intervals.insert(y, solid)),
            }

            match y + 1 == max {
                true => drop(self.intervals.remove(&max)),
                false => drop(self.intervals.insert(y + 1, !solid)),
            }
        }
    }

    pub fn top(&self) -> i32 {
        self.intervals
            .keys()
            .rev()
            .next()
            .copied()
            .unwrap_or(i32::MIN)
    }
}

#[derive(Clone, Debug)]
pub struct SkyLightColumns {
    nodes: Box<[SkyLightNode]>,
}

impl SkyLightColumns {
    pub fn initialize(heightmap: &SurfaceHeightmap) -> Self {
        let mut nodes = Vec::with_capacity(CHUNK_LENGTH_2);

        for i in 0..nodes.capacity() {
            nodes.push(SkyLightNode::init(heightmap.data()[i], false));
        }

        Self {
            nodes: nodes.into_boxed_slice(),
        }
    }

    pub fn node(&self, x: usize, z: usize) -> &SkyLightNode {
        &self.nodes[CHUNK_LENGTH * x + z]
    }

    pub fn node_mut(&mut self, x: usize, z: usize) -> &mut SkyLightNode {
        &mut self.nodes[CHUNK_LENGTH * x + z]
    }
}

impl<W: std::io::Write> Encode<W> for SkyLightColumns {
    const KIND: NodeKind = NodeKind::List;

    fn encode(&self, mut encoder: Encoder<W>) -> Result<()> {
        // encoder.encode_rle_list(self.nodes)
        todo!()
    }
}

// TODO: compress! could likely both palletize and run-length encode here
// impl Codec for SkyLightColumns {
//     fn encode<W: std::io::Write>(&self, writer: &mut W) -> Result<()> {
//         for node in self.nodes.iter() {
//             node.intervals.len().encode(writer)?;
//             for height in node.intervals.keys() {
//                 height.encode(writer)?;
//             }
//         }
//         Ok(())
//     }

//     fn decode<R: std::io::Read>(reader: &mut R) -> Result<Self> {
//         let mut nodes = Vec::with_capacity(CHUNK_LENGTH_2);
//         for _ in 0..nodes.capacity() {
//             let intervals_size = usize::decode(reader)?;
//             let mut intervals = BTreeMap::new();
//             for i in 0..intervals_size {
//                 intervals.insert(i32::decode(reader)?, i % 2 == 1);
//             }
//             nodes.push(SkyLightNode { intervals });
//         }

//         Ok(SkyLightColumns {
//             nodes: nodes.into_boxed_slice(),
//         })
//     }
// }
