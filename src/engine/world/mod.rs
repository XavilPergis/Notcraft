use cgmath::{Point3, Vector3};
use engine::world::block::BlockRegistry;
use std::collections::HashMap;

use self::block::BlockId;
pub use self::chunk::Chunk;

pub mod block;
pub mod chunk;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ChunkPos(pub Point3<i32>);
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct BlockPos(pub Point3<i32>);
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct WorldPos(pub Point3<f64>);

// CHUNK POS
impl From<BlockPos> for ChunkPos {
    fn from(pos: BlockPos) -> Self {
        const SIZE: i32 = ::engine::world::chunk::SIZE as i32;
        let cx = ::util::floor_div(pos.0.x, SIZE);
        let cy = ::util::floor_div(pos.0.y, SIZE);
        let cz = ::util::floor_div(pos.0.z, SIZE);
        ChunkPos(Point3::new(cx, cy, cz))
    }
}

impl From<WorldPos> for ChunkPos {
    fn from(pos: WorldPos) -> Self {
        BlockPos::from(pos).into()
    }
}

// BLOCK POS
impl From<WorldPos> for BlockPos {
    fn from(pos: WorldPos) -> Self {
        BlockPos(Point3::new(
            pos.0.x.floor() as i32,
            pos.0.y.floor() as i32,
            pos.0.z.floor() as i32,
        ))
    }
}

impl ChunkPos {
    pub fn offset<I: Into<Vector3<i32>>>(self, vec: I) -> Self {
        ChunkPos(self.0 + vec.into())
    }

    pub fn base(self) -> BlockPos {
        BlockPos(::engine::world::chunk::SIZE as i32 * self.0)
    }
}

impl BlockPos {
    pub fn offset<I: Into<Vector3<i32>>>(self, vec: I) -> Self {
        BlockPos(self.0 + vec.into())
    }

    pub fn base(self) -> WorldPos {
        WorldPos(self.0.cast().unwrap())
    }

    pub fn chunk_pos_offset(self) -> (ChunkPos, Vector3<i32>) {
        let cpos: ChunkPos = self.into();
        let bpos = self.0 - cpos.base().0;

        (cpos, bpos)
    }

    pub fn center(self) -> WorldPos {
        WorldPos(Point3::new(
            self.0.x as f64 + 0.5,
            self.0.y as f64 + 0.5,
            self.0.z as f64 + 0.5,
        ))
    }
}

impl WorldPos {
    pub fn offset<I: Into<Vector3<f64>>>(self, vec: I) -> Self {
        WorldPos(self.0 + vec.into())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VoxelWorld {
    chunks: HashMap<ChunkPos, Chunk<BlockId>>,
    registry: BlockRegistry,
}

use std::ops::{Index, IndexMut};
impl Index<BlockPos> for VoxelWorld {
    type Output = BlockId;

    fn index(&self, idx: BlockPos) -> &Self::Output {
        let (chunk_pos, block_pos) = idx.chunk_pos_offset();
        match self.chunks.get(&chunk_pos).map(|chunk| &chunk[block_pos]) {
            Some(v) => v,
            _ => panic!("Block requested at {:?} was not found.", idx),
        }
    }
}
impl IndexMut<BlockPos> for VoxelWorld {
    fn index_mut(&mut self, idx: BlockPos) -> &mut Self::Output {
        let (chunk_pos, block_pos) = idx.chunk_pos_offset();
        match self
            .chunks
            .get_mut(&chunk_pos)
            .map(|chunk| &mut chunk[block_pos])
        {
            Some(v) => v,
            _ => panic!("Block requested at {:?} was not found.", idx),
        }
    }
}

impl VoxelWorld {
    pub fn new(registry: BlockRegistry) -> Self {
        VoxelWorld {
            chunks: Default::default(),
            registry,
        }
    }

    pub fn unload_chunk(&mut self, pos: ChunkPos) {
        self.chunks.remove(&pos);
    }

    pub fn set_chunk(&mut self, pos: ChunkPos, chunk: Chunk<BlockId>) {
        self.chunks.insert(pos, chunk);
    }

    pub fn chunk(&self, pos: ChunkPos) -> Option<&Chunk<BlockId>> {
        self.chunks.get(&pos)
    }

    pub fn chunk_exists(&self, pos: ChunkPos) -> bool {
        self.chunks.contains_key(&pos)
    }

    /// Tries to replace the block at `pos`, returning the block that was replaced if it was found
    pub fn set_block_id(&mut self, pos: BlockPos, block: BlockId) -> Option<BlockId> {
        let (chunk_pos, block_pos) = pos.chunk_pos_offset();
        self.chunks
            .get_mut(&chunk_pos)
            .map(|chunk| ::std::mem::replace(&mut chunk[block_pos], block))
    }

    pub fn get_block_id(&self, pos: BlockPos) -> Option<BlockId> {
        let (chunk_pos, block_pos) = pos.chunk_pos_offset();
        self.chunks.get(&chunk_pos).map(|chunk| chunk[block_pos])
    }

    pub fn get_block_properties(&self, pos: BlockPos) -> Option<&block::BlockProperties> {
        let (chunk_pos, block_pos) = pos.chunk_pos_offset();
        self.chunks
            .get(&chunk_pos)
            .map(|chunk| &self.registry[chunk[block_pos]])
    }
}
