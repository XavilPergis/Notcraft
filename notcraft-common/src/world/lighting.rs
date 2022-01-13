use std::{
    cmp::Ordering,
    collections::{HashSet, VecDeque},
};

use super::{
    chunk::{MutableChunkAccess, CHUNK_LENGTH},
    generation::SurfaceHeightmap,
    BlockPos,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
#[repr(transparent)]
pub struct LightValue(pub u16);

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
    visited: HashSet<BlockPos>,
}

impl LightUpdateQueues {
    pub fn queue_updates<I>(&mut self, access: &mut MutableChunkAccess, iter: I)
    where
        I: Iterator<Item = (BlockPos, u16)>,
    {
        for (pos, new_light) in iter {
            if !self.visited.insert(pos) {
                continue;
            }

            let prev_light = access.light(pos).unwrap().block();

            let id = access.block(pos).unwrap();
            if access.registry().light_transmissible(id) {
                self.block_removal.push_back((pos, prev_light));
                access.set_block_light(pos, 0).unwrap();
            }

            match new_light.cmp(&prev_light) {
                Ordering::Equal => {}
                Ordering::Less => self.block_removal.push_back((pos, prev_light)),
                Ordering::Greater => self.block_update.push_back((pos, new_light)),
            }
        }

        self.visited.clear();
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct SkyLightNode {
    min_y: i32,
}

#[derive(Clone, Debug)]
pub struct SkyLightColumns {
    nodes: Box<[Vec<SkyLightNode>]>,
}

impl SkyLightColumns {
    pub fn initialize(heightmap: &SurfaceHeightmap) -> Self {
        let mut nodes = Vec::with_capacity(CHUNK_LENGTH * CHUNK_LENGTH);

        for i in 0..nodes.capacity() {
            nodes.push(vec![SkyLightNode {
                min_y: heightmap.data()[i],
            }]);
        }

        Self {
            nodes: nodes.into_boxed_slice(),
        }
    }
}
