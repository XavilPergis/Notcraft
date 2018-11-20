use cgmath::{Point3, Vector3, Vector4};
use collision::{Aabb, Aabb3};
use engine::systems::debug_render::{DebugSection, Shape};
use engine::world::block::BlockRegistry;
use engine::Side;
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

    pub fn aabb(&self) -> Aabb3<f64> {
        Aabb3::new(Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 1.0, 1.0))
            .add_v(::util::to_vector(self.base().0))
    }
}

impl WorldPos {
    pub fn offset<I: Into<Vector3<f64>>>(self, vec: I) -> Self {
        WorldPos(self.0 + vec.into())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VoxelWorld {
    chunks: HashMap<ChunkPos, Chunk>,
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

    pub fn set_chunk(&mut self, pos: ChunkPos, chunk: Chunk) {
        self.chunks.insert(pos, chunk);
    }

    pub fn chunk(&self, pos: ChunkPos) -> Option<&Chunk> {
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

    pub fn trace_block(
        &self,
        ray: Ray3<f64>,
        radius: f64,
        debug: &mut DebugSection,
    ) -> Option<(BlockPos, Option<Vector3<i32>>)> {
        let mut ret_pos = WorldPos(ray.origin).into();
        let mut ret_norm = None;

        if trace_ray(ray, radius, |pos, norm| {
            ret_pos = pos;
            ret_norm = norm;
            debug.draw(Shape::Block(1.0, pos, Vector4::new(1.0, 0.0, 1.0, 1.0)));
            self.get_block_properties(pos)
                .map(|props| props.opaque)
                .unwrap_or(false)
        }) {
            Some((ret_pos, ret_norm))
        } else {
            None
        }
    }
}

use collision::Ray3;

fn modulo(a: f64, b: f64) -> f64 {
    (a % b + b) % b
}

fn int_bound(ray: Ray3<f64>, axis: usize) -> f64 {
    if ray.direction[axis] < 0.0 {
        let mut new = ray;
        new.origin[axis] *= -1.0;
        new.direction[axis] *= -1.0;
        int_bound(new, axis)
    } else {
        (1.0 - modulo(ray.origin[axis], 1.0)) / ray.direction[axis]
    }
}

fn sign(n: f64) -> f64 {
    if n > 0.0 {
        1.0
    } else if n < 0.0 {
        -1.0
    } else {
        0.0
    }
}

fn trace_ray<F>(ray: Ray3<f64>, radius: f64, mut func: F) -> bool
where
    F: FnMut(BlockPos, Option<Vector3<i32>>) -> bool,
{
    // init phase
    let origin: BlockPos = WorldPos(ray.origin).into();
    let mut current = origin.0;
    let step_x = sign(ray.direction.x);
    let step_y = sign(ray.direction.y);
    let step_z = sign(ray.direction.z);

    let mut t_max_x = int_bound(ray, 0);
    let mut t_max_y = int_bound(ray, 1);
    let mut t_max_z = int_bound(ray, 2);

    let t_delta_x = step_x / ray.direction.x;
    let t_delta_y = step_y / ray.direction.y;
    let t_delta_z = step_z / ray.direction.z;

    let step_x = step_x as i32;
    let step_y = step_y as i32;
    let step_z = step_z as i32;
    let mut normal = None;

    // incremental pahse
    loop {
        if func(BlockPos(current), normal) {
            return true;
        }

        if t_max_x < t_max_y {
            if t_max_x < t_max_z {
                if t_max_x > radius {
                    break;
                }
                current.x += step_x;
                t_max_x += t_delta_x;
                normal = Some(Vector3::new(-step_x, 0, 0));
            } else {
                if t_max_z > radius {
                    break;
                }
                current.z += step_z;
                t_max_z += t_delta_z;
                normal = Some(Vector3::new(0, 0, -step_z));
            }
        } else {
            if t_max_y < t_max_z {
                if t_max_y > radius {
                    break;
                }
                current.y += step_y;
                t_max_y += t_delta_y;
                normal = Some(Vector3::new(0, -step_y, 0));
            } else {
                if t_max_z > radius {
                    break;
                }
                current.z += step_z;
                t_max_z += t_delta_z;
                normal = Some(Vector3::new(0, 0, -step_z));
            }
        }
    }

    false
}
