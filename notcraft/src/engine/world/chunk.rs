use cgmath::{Point3, Vector3};
use crate::engine::world::{
    block::{self, BlockId},
    ChunkPos, VoxelWorld,
};

// The width of the chunk is `2 ^ SIZE_BITS`
pub const SIZE_BITS: usize = 5;
pub const SIZE_BITS_2: usize = SIZE_BITS * 2;
pub const SIZE: usize = 1 << SIZE_BITS;

pub const AREA: usize = SIZE * SIZE;
pub const VOLUME: usize = SIZE * SIZE * SIZE;

pub fn in_chunk_bounds(pos: Point3<i32>) -> bool {
    const SIZEI: i32 = SIZE as i32;
    pos.x < SIZEI && pos.y < SIZEI && pos.z < SIZEI && pos.x >= 0 && pos.y >= 0 && pos.z >= 0
}

const fn index_for_coord(x: usize, y: usize, z: usize) -> usize {
    (x << SIZE_BITS_2) + (y << SIZE_BITS) + z
}

const fn index_for_coord_size(size: usize, x: usize, y: usize, z: usize) -> usize {
    x * size * size + y * size + z
}

pub struct PaddedChunk {
    data: Box<[BlockId]>,
}

pub fn make_padded(world: &VoxelWorld, pos: ChunkPos) -> Option<PaddedChunk> {
    let padded_size = SIZE + 2;

    let mut data = Vec::with_capacity(padded_size * padded_size * padded_size);

    let base = pos.base();

    for x in 0..padded_size {
        for y in 0..padded_size {
            for z in 0..padded_size {
                let block =
                    world.get_block_id(base.offset((x as i32 - 1, y as i32 - 1, z as i32 - 1)))?;
                data.push(block);
            }
        }
    }

    // // back
    // for x in 0..SIZE {
    //     let dest_base_x = padded_size * padded_size * (x + 1);
    //     let src_base_x = SIZE * SIZE * x;
    //     for y in 0..SIZE {
    //         let dest_base_y = padded_size * (y + 1);
    //         let src_base_y = SIZE * y;
    //         data[dest_base_x + dest_base_y + dest_base_z] =
    //             chunk.data[src_base_x + src_base_y + src_base_z];
    //     }
    // }
    // data[]

    Some(PaddedChunk { data: data.into() })
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ChunkType {
    Homogeneous(BlockId),
    Array(Chunk),
}

impl ChunkType {
    pub fn is_homogeneous(&self) -> bool {
        match self {
            ChunkType::Homogeneous(_) => true,
            _ => false,
        }
    }
}

impl From<Chunk> for ChunkType {
    fn from(chunk: Chunk) -> ChunkType {
        if chunk.data.iter().all(|&item| item == chunk[0]) {
            ChunkType::Homogeneous(chunk[0])
        } else {
            ChunkType::Array(chunk)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Chunk {
    data: Box<[BlockId]>,
}

impl Chunk {
    pub fn new(voxels: Vec<BlockId>) -> Self {
        Chunk {
            data: voxels.into(),
        }
    }

    /// An UNCOMPRESSED empty chunk. This is probably not what you want.
    pub fn empty() -> Self {
        Chunk {
            data: vec![block::AIR; VOLUME].into(),
        }
    }

    pub fn get(&self, pos: Point3<i32>) -> Option<&BlockId> {
        if in_chunk_bounds(pos) {
            let pos: Point3<usize> = pos.cast().unwrap();
            Some(&self[pos])
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, pos: Point3<i32>) -> Option<&mut BlockId> {
        if in_chunk_bounds(pos) {
            let pos: Point3<usize> = pos.cast().unwrap();
            Some(&mut self[pos])
        } else {
            None
        }
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }
}

use std::ops::{Index, IndexMut};

macro_rules! gen_index {
    ($name:ident : $type:ty => $x:expr, $y:expr, $z:expr) => {
        impl Index<$type> for Chunk {
            type Output = BlockId;

            fn index(&self, $name: $type) -> &BlockId {
                debug_assert!(in_chunk_bounds(Point3::new(
                    $x as i32, $y as i32, $z as i32
                )));
                &self.data[index_for_coord($x, $y, $z)]
            }
        }

        impl IndexMut<$type> for Chunk {
            fn index_mut(&mut self, $name: $type) -> &mut BlockId {
                debug_assert!(in_chunk_bounds(Point3::new(
                    $x as i32, $y as i32, $z as i32
                )));
                &mut self.data[index_for_coord($x, $y, $z)]
            }
        }
        impl Index<$type> for PaddedChunk {
            type Output = BlockId;

            fn index(&self, $name: $type) -> &BlockId {
                debug_assert!(in_chunk_bounds(Point3::new(
                    $x as i32, $y as i32, $z as i32
                )));
                &self.data[index_for_coord_size(34, $x, $y, $z)]
            }
        }

        impl IndexMut<$type> for PaddedChunk {
            fn index_mut(&mut self, $name: $type) -> &mut BlockId {
                debug_assert!(in_chunk_bounds(Point3::new(
                    $x as i32, $y as i32, $z as i32
                )));
                &mut self.data[index_for_coord_size(34, $x, $y, $z)]
            }
        }
    };
}

impl Index<usize> for Chunk {
    type Output = BlockId;

    fn index(&self, idx: usize) -> &BlockId {
        &self.data[idx]
    }
}

impl IndexMut<usize> for Chunk {
    fn index_mut(&mut self, idx: usize) -> &mut BlockId {
        &mut self.data[idx]
    }
}

gen_index!(point: Point3<usize> => point.x, point.y, point.z);
gen_index!(point: Point3<i32> => point.x as usize, point.y as usize, point.z as usize);
gen_index!(point: Point3<isize> => point.x as usize, point.y as usize, point.z as usize);
gen_index!(point: Vector3<usize> => point.x, point.y, point.z);
gen_index!(point: Vector3<i32> => point.x as usize, point.y as usize, point.z as usize);
gen_index!(point: Vector3<isize> => point.x as usize, point.y as usize, point.z as usize);
gen_index!(point: (usize, usize, usize) => point.0, point.1, point.2);
