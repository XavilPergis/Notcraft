use crate::engine::{
    render::debug::{DebugSection, Shape},
    world::{
        block::BlockRegistry,
        chunk::{ChunkType, SIZE},
    },
};
use cgmath::{Point3, Vector3, Vector4};
use collision::{Aabb, Aabb3};
use std::collections::{HashMap, HashSet};

use self::block::BlockId;
pub use self::chunk::Chunk;

pub mod block;
pub mod chunk;
pub mod gen;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ChunkPos(pub Point3<i32>);
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct BlockPos(pub Point3<i32>);
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct WorldPos(pub Point3<f32>);

// CHUNK POS
impl From<BlockPos> for ChunkPos {
    fn from(pos: BlockPos) -> Self {
        const SIZEI: i32 = SIZE as i32;
        let cx = crate::util::floor_div(pos.0.x, SIZEI);
        let cy = crate::util::floor_div(pos.0.y, SIZEI);
        let cz = crate::util::floor_div(pos.0.z, SIZEI);
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
        BlockPos(SIZE as i32 * self.0)
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
            self.0.x as f32 + 0.5,
            self.0.y as f32 + 0.5,
            self.0.z as f32 + 0.5,
        ))
    }

    pub fn aabb(&self) -> Aabb3<f32> {
        Aabb3::new(Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 1.0, 1.0))
            .add_v(crate::util::to_vector(self.base().0))
    }
}

impl WorldPos {
    pub fn offset<I: Into<Vector3<f32>>>(self, vec: I) -> Self {
        WorldPos(self.0 + vec.into())
    }
}

#[inline(always)]
fn chunk_id(pos: ChunkPos) -> u64 {
    let mut id = 0;
    id |= pos.0.x as u64 & 0xFFFF;
    id |= (pos.0.y as u64 & 0xFFFF) << 16;
    id |= (pos.0.z as u64 & 0xFFFF) << 32;
    id
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct QueryId(usize);

#[derive(Debug, Default)]
struct ChunkPager {
    queries: HashMap<ChunkPos, usize>,

    /// Map of chunk positions to (chunks, time to live)
    paged: HashMap<ChunkPos, (ChunkType, usize)>,
}

impl ChunkPager {
    pub fn update(&mut self) {}

    pub fn query(&self) -> QueryId {
        unimplemented!()
    }
}

// TODO: chunk paging:
// - Immediate (same-frame) chunk paging (should be used sparingly and when
//   you're fairly confident those chunks are already loaded)
// - Async chunk queries (with time to live)
// - Persistent chunk mappings (make sure certain chunks neveer get unmapped
//   while locked)

#[derive(Debug)]
pub struct VoxelWorld {
    chunks: HashMap<ChunkPos, ChunkType>,
    dirty_mesh: HashSet<ChunkPos>,
    registry: BlockRegistry,
}

// use std::ops::{Index, IndexMut};
// impl Index<BlockPos> for VoxelWorld {
//     type Output = BlockId;

//     fn index(&self, idx: BlockPos) -> &Self::Output {
//         let (chunk_pos, block_pos) = idx.chunk_pos_offset();
//         match self.chunks.get(&chunk_pos).map(|chunk| &chunk[block_pos]) {
//             Some(v) => v,
//             _ => panic!("Block requested at {:?} was not found.", idx),
//         }
//     }
// }
// impl IndexMut<BlockPos> for VoxelWorld {
//     fn index_mut(&mut self, idx: BlockPos) -> &mut Self::Output {
//         let (chunk_pos, block_pos) = idx.chunk_pos_offset();
//         match self
//             .chunks
//             .get_mut(&chunk_pos)
//             .map(|chunk| &mut chunk[block_pos])
//         {
//             Some(v) => v,
//             _ => panic!("Block requested at {:?} was not found.", idx),
//         }
//     }
// }

fn iter_pos(start: BlockPos, end: BlockPos, mut func: impl FnMut(BlockPos)) {
    for x in start.0.x..=end.0.x {
        for y in start.0.y..=end.0.y {
            for z in start.0.z..=end.0.z {
                func(BlockPos(Point3::new(x, y, z)));
            }
        }
    }
}

impl VoxelWorld {
    pub fn new(registry: BlockRegistry) -> Self {
        VoxelWorld {
            chunks: Default::default(),
            dirty_mesh: Default::default(),
            registry,
        }
    }

    pub fn get_registry(&self) -> &BlockRegistry {
        &self.registry
    }

    pub fn unload_chunk(&mut self, pos: ChunkPos) {
        self.chunks.remove(&pos);
    }

    pub fn set_chunk(&mut self, pos: ChunkPos, chunk: Chunk) {
        self.dirty_mesh.insert(pos);
        self.chunks.insert(pos, chunk.into());
    }

    pub fn chunk(&self, pos: ChunkPos) -> Option<&ChunkType> {
        self.chunks.get(&pos)
    }

    pub fn chunk_exists(&self, pos: ChunkPos) -> bool {
        self.chunks.contains_key(&pos)
    }

    fn mark_neighborhood_dirty(&mut self, pos: BlockPos) {
        iter_pos(pos.offset((-1, -1, -1)), pos.offset((1, 1, 1)), |pos| {
            self.dirty_mesh.insert(pos.into());
        });
    }

    /// Tries to replace the block at `pos`, returning the block that was
    /// replaced if it was found
    // oh god...
    pub fn set_block_id(&mut self, pos: BlockPos, block: BlockId) -> Option<BlockId> {
        use std::mem;
        let (chunk_pos, block_pos) = pos.chunk_pos_offset();

        let homogeneous = self.chunks.get(&chunk_pos).and_then(|chunk| match chunk {
            ChunkType::Homogeneous(id) => Some(*id),
            _ => None,
        });

        if let Some(id) = homogeneous {
            // Setting the same block as the homogeneous chunk already contains means that
            // we shouldn't update the chunk!
            if id == block {
                return homogeneous;
            }

            match self.chunks.get_mut(&chunk_pos) {
                // There is a chunk here, expand it
                Some(chunk) => *chunk = ChunkType::Array(Chunk::empty()),
                // No chunk, return saying we found no block
                None => return None,
            }
        }

        self.mark_neighborhood_dirty(pos);
        self.chunks.get_mut(&chunk_pos).map(|chunk| match chunk {
            ChunkType::Array(chunk) => mem::replace(&mut chunk[block_pos], block),
            // We always expand the chunk or exit early by this point
            _ => unreachable!(),
        })
    }

    pub fn get_block_id(&self, pos: BlockPos) -> Option<BlockId> {
        let (chunk_pos, block_pos) = pos.chunk_pos_offset();
        self.chunks.get(&chunk_pos).map(|chunk| match chunk {
            ChunkType::Homogeneous(id) => *id,
            ChunkType::Array(chunk) => chunk[block_pos],
        })
    }

    pub fn registry(&self, pos: BlockPos) -> Option<block::RegistryRef> {
        self.get_block_id(pos).map(|id| self.registry.get_ref(id))
    }

    pub fn get_dirty_chunk(&mut self) -> Option<ChunkPos> {
        self.dirty_mesh
            .iter()
            .filter(|pos| {
                let mut surrounded = true;
                for x in -1..=1 {
                    for y in -1..=1 {
                        for z in -1..=1 {
                            surrounded &= self.chunk_exists(pos.offset((x, y, z)));
                        }
                    }
                }
                surrounded
            })
            .next()
            .cloned()
    }

    pub fn clean_chunk(&mut self, pos: ChunkPos) {
        self.dirty_mesh.remove(&pos);
    }

    pub fn trace_block(
        &self,
        ray: Ray3<f32>,
        radius: f32,
        debug: &mut DebugSection,
    ) -> Option<(BlockPos, Option<Vector3<i32>>)> {
        let mut ret_pos = WorldPos(ray.origin).into();
        let mut ret_norm = None;

        if trace_ray(ray, radius, |pos, norm| {
            ret_pos = pos;
            ret_norm = norm;
            debug.draw(Shape::Block(1.0, pos, Vector4::new(1.0, 0.0, 1.0, 1.0)));
            self.registry(pos)
                .map(|props| props.opaque())
                .unwrap_or(false)
        }) {
            Some((ret_pos, ret_norm))
        } else {
            None
        }
    }
}

// mod benches {
//     use super::*;
//     use test::Bencher;

//     #[bench]
//     fn bench_world_get(b: &mut Bencher) {
//         let mut world = VoxelWorld::new(
//             BlockRegistry::load_from_file("resources/blocks.json")
//                 .unwrap()
//                 .0,
//         );
//         world.set_chunk(ChunkPos(Point3::new(0, 0, 0)),
// super::gen::get_test_chunk());         b.iter(|| {
//             for x in -2..34 {
//                 for y in -2..34 {
//                     for z in -2..34 {
//
// test::black_box(world.get_block_id(BlockPos(Point3::new(x, y, z))));
//                     }
//                 }
//             }
//         });
//     }
// }

use collision::Ray3;

fn int_bound(ray: Ray3<f32>, axis: usize) -> f32 {
    if ray.direction[axis] < 0.0 {
        let mut new = ray;
        new.origin[axis] *= -1.0;
        new.direction[axis] *= -1.0;
        int_bound(new, axis)
    } else {
        (1.0 - crate::util::modulo(ray.origin[axis], 1.0)) / ray.direction[axis]
    }
}

fn trace_ray<F>(ray: Ray3<f32>, radius: f32, mut func: F) -> bool
where
    F: FnMut(BlockPos, Option<Vector3<i32>>) -> bool,
{
    // init phase
    let origin: BlockPos = WorldPos(ray.origin).into();
    let mut current = origin.0;
    let step_x = ray.direction.x.signum();
    let step_y = ray.direction.y.signum();
    let step_z = ray.direction.z.signum();

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
