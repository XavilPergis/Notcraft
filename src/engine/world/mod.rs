use std::collections::HashMap;
use cgmath::{Point3, Vector3};

pub use self::chunk::Chunk;
use self::block::BlockId;

pub mod block;
pub mod chunk;

pub type ChunkPos = Point3<i32>;
pub type BlockPos = Point3<i32>;

/// Get a chunk position from a world position
pub fn chunk_pos_offset(pos: BlockPos) -> (ChunkPos, Vector3<i32>) {
    const SIZE: i32 = ::engine::world::chunk::SIZE as i32;
    let cx = ::util::floor_div(pos.x, SIZE);
    let cy = ::util::floor_div(pos.y, SIZE);
    let cz = ::util::floor_div(pos.z, SIZE);

    let cpos = Point3::new(cx, cy, cz);
    let bpos = pos - (SIZE*cpos);

    (cpos, bpos)
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct VoxelWorld {
    pub chunks: HashMap<ChunkPos, Chunk<BlockId>>,
}

impl VoxelWorld {
    pub fn set_chunk(&mut self, pos: ChunkPos, chunk: Chunk<BlockId>) {
        self.chunks.insert(pos, chunk);
    }
    
    pub fn chunk(&self, pos: ChunkPos) -> Option<&Chunk<BlockId>> {
        self.chunks.get(&pos)
    }

    pub fn chunk_exists(&self, pos: ChunkPos) -> bool {
        self.chunks.contains_key(&pos)
    }

    pub fn set_block(&mut self, pos: BlockPos, block: BlockId) {
        if let None = self.try_set_block(pos, block) {
            panic!("Block requested at ({}, {}, {}) was not found.", pos.x, pos.y, pos.z);
        }
    }
    
    /// Tries to replace the block at `pos`, returning the block that was replaced if it was found
    pub fn try_set_block(&mut self, pos: BlockPos, block: BlockId) -> Option<BlockId> {
        let (chunk_pos, block_pos) = chunk_pos_offset(pos);
        self.chunks.get_mut(&chunk_pos).map(|chunk| ::std::mem::replace(&mut chunk[block_pos], block))
    }


    pub fn get_block(&self, pos: BlockPos) -> BlockId {
        if let Some(block) = self.try_get_block(pos) { block } else {
            panic!("Block requested at ({}, {}, {}) was not found.", pos.x, pos.y, pos.z);
        }
    }

    pub fn try_get_block(&self, pos: BlockPos) -> Option<BlockId> {
        let (chunk_pos, block_pos) = chunk_pos_offset(pos);
        self.chunks.get(&chunk_pos).map(|chunk| chunk[block_pos])
    }
}
