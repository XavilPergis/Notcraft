use crate::engine::world::registry::BlockId;
use nalgebra::Point3;
use std::{
    ops::{Index, IndexMut},
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError},
};

// The width of the chunk is `2 ^ SIZE_BITS`
pub const CHUNK_LENGTH_BITS: usize = 5;

pub const CHUNK_LENGTH: usize = 1 << CHUNK_LENGTH_BITS;
pub const CHUNK_AREA: usize = CHUNK_LENGTH * CHUNK_LENGTH;
pub const CHUNK_VOLUME: usize = CHUNK_LENGTH * CHUNK_LENGTH * CHUNK_LENGTH;

#[derive(Debug)]
pub struct Chunk {
    pos: ChunkPos,
    kind: RwLock<ChunkKind>,
}

impl Chunk {
    pub fn new(pos: ChunkPos, kind: ChunkKind) -> Self {
        Self {
            pos,
            kind: RwLock::new(kind),
        }
    }

    pub fn pos(&self) -> ChunkPos {
        self.pos
    }

    pub fn read(&self) -> Option<RwLockReadGuard<ChunkKind>> {
        match self.kind.try_read() {
            Ok(guard) => Some(guard),
            Err(TryLockError::WouldBlock) => None,
            Err(TryLockError::Poisoned(_)) => panic!("chunk was poisoned"),
        }
    }

    pub fn write(&self) -> Option<RwLockWriteGuard<ChunkKind>> {
        match self.kind.try_write() {
            Ok(guard) => Some(guard),
            Err(TryLockError::WouldBlock) => None,
            Err(TryLockError::Poisoned(_)) => panic!("chunk was poisoned"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ChunkKind {
    Homogeneous(BlockId),
    Array(ArrayChunk),
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ArrayChunk {
    // data order is XZY
    data: Box<[BlockId]>,
}

pub fn is_in_chunk_bounds(x: usize, y: usize, z: usize) -> bool {
    x < CHUNK_LENGTH && y < CHUNK_LENGTH && z < CHUNK_LENGTH
}

impl<I: Into<[usize; 3]>> Index<I> for ArrayChunk {
    type Output = BlockId;

    fn index(&self, index: I) -> &Self::Output {
        let [x, y, z] = index.into();
        if is_in_chunk_bounds(x, y, z) {
            return &self.data[CHUNK_LENGTH * CHUNK_LENGTH * x + CHUNK_LENGTH * z + y];
        }

        panic!(
            "chunk index out of bounds: the size is {} but the index is ({}, {}, {})",
            CHUNK_LENGTH, x, y, z
        )
    }
}

impl<I: Into<[usize; 3]>> IndexMut<I> for ArrayChunk {
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        let [x, y, z] = index.into();
        if is_in_chunk_bounds(x, y, z) {
            return &mut self.data[CHUNK_LENGTH * CHUNK_LENGTH * x + CHUNK_LENGTH * z + y];
        }

        panic!(
            "chunk index out of bounds: the size is {} but the index is ({}, {}, {})",
            CHUNK_LENGTH, x, y, z
        )
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ChunkPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl From<ChunkPos> for Point3<i32> {
    fn from(ChunkPos { x, y, z }: ChunkPos) -> Self {
        nalgebra::point![x, y, z]
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ChunkOffset {
    x: u16,
    y: u16,
    z: u16,
}

impl ChunkOffset {
    pub fn new(x: u16, y: u16, z: u16) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug)]
pub struct ChunkTryFromError {
    provided_size: usize,
    expected_size: usize,
}

impl std::error::Error for ChunkTryFromError {}
impl std::fmt::Display for ChunkTryFromError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "could not convert slice to array chunk: mismatched sizes: expected {}, got {}",
            self.expected_size, self.provided_size
        )
    }
}

impl TryFrom<Box<[BlockId]>> for ArrayChunk {
    type Error = ChunkTryFromError;

    fn try_from(data: Box<[BlockId]>) -> Result<Self, Self::Error> {
        if data.len() != CHUNK_VOLUME {
            return Err(ChunkTryFromError {
                provided_size: data.len(),
                expected_size: CHUNK_VOLUME,
            });
        }

        Ok(ArrayChunk { data })
    }
}
