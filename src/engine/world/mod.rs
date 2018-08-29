use std::collections::HashMap;
use cgmath::Point3;

pub use self::chunk::Chunk;
use self::block::BlockId;

pub mod block;
pub mod chunk;

pub type ChunkPos = Point3<i32>;

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
}
