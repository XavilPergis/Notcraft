use std::collections::{HashMap, HashSet, VecDeque};

use super::{
    chunk::{ChunkAccess, ChunkData, ChunkPos, MutableChunkAccess},
    BlockPos,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct LightValue(u16);

pub const SKY_LIGHT_BITS: u16 = 4;
pub const BLOCK_LIGHT_BITS: u16 = 4;

pub const SKY_LIGHT_MASK: u16 = (1 << SKY_LIGHT_BITS) - 1;
pub const BLOCK_LIGHT_MASK: u16 = (1 << BLOCK_LIGHT_BITS) - 1;

pub const FULL_SKY_LIGHT: LightValue = LightValue::pack(15, 0);

impl LightValue {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn pack(sky: u16, block: u16) -> Self {
        let sky = sky & SKY_LIGHT_MASK;
        let block = block & BLOCK_LIGHT_MASK;
        Self(sky << BLOCK_LIGHT_BITS | block)
    }

    pub const fn raw(self) -> u16 {
        self.0
    }

    pub const fn sky(self) -> u16 {
        self.0 >> SKY_LIGHT_MASK
    }

    pub const fn block(self) -> u16 {
        self.0 & BLOCK_LIGHT_MASK
    }
}

pub(crate) fn propagate_block_light(
    light_updates: &HashMap<BlockPos, u16>,
    access: &mut MutableChunkAccess,
) {
    let mut queue = VecDeque::new();
    queue.extend(light_updates.iter().map(|(&k, &v)| (k, v)));

    let mut visited = HashSet::new();

    while let Some((pos, value)) = queue.pop_front() {
        access.set_block_light(pos, value).unwrap();

        if !visited.insert(pos) || value == 0 {
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
            let light = access.light(dir).unwrap().block();
            let id = access.block(dir).unwrap();
            let opaque = matches!(
                access.registry().mesh_type(id),
                crate::world::registry::BlockMeshType::FullCube
            );
            if !opaque && light < value {
                queue.push_back((dir, value - 1));
            }
        }
    }
}
