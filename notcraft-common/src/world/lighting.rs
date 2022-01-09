use std::collections::HashSet;

use super::{
    chunk::{ChunkAccess, ChunkData, ChunkPos},
    BlockPos,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct LightValue(u16);

pub const SKY_LIGHT_BITS: usize = 4;
pub const BLOCK_LIGHT_BITS: usize = 4;

pub const FULL_SKY_LIGHT: LightValue = LightValue::pack(15, 0);

impl LightValue {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn pack(sky: u16, block: u16) -> Self {
        let sky = sky & (SKY_LIGHT_BITS as u16);
        let block = block & (BLOCK_LIGHT_BITS as u16);
        Self(sky << (BLOCK_LIGHT_BITS as u16) | block)
    }

    pub const fn sky(self) -> u16 {
        self.0 >> (BLOCK_LIGHT_BITS as u16)
    }

    pub const fn block(self) -> u16 {
        self.0 & (BLOCK_LIGHT_BITS as u16)
    }
}

pub(crate) fn propagate_block_light(
    cache: &mut ChunkAccess,
    pos: ChunkPos,
    light_updates: &HashSet<BlockPos>,
    light: &mut ChunkData<LightValue>,
) {
    todo!()
}
