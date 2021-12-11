use crate::engine::world::{
    block::BlockRegistry,
    chunk::{ChunkType, SIZE},
};
use crossbeam_channel::{Receiver, Sender};
use nalgebra::{point, vector, Point3, Vector3};
use rayon::ThreadPool;
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    sync::{Arc, Mutex},
};

use self::block::BlockId;
pub use self::chunk::Chunk;

use super::components::Transform;

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
        ChunkPos(point!(cx, cy, cz))
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
        BlockPos(point!(
            pos.0.x.floor() as i32,
            pos.0.y.floor() as i32,
            pos.0.z.floor() as i32
        ))
    }
}

impl ChunkPos {
    pub fn xyz(x: i32, y: i32, z: i32) -> Self {
        ChunkPos(point!(x, y, z))
    }

    pub fn offset(self, x: i32, y: i32, z: i32) -> Self {
        ChunkPos(self.0 + vector!(x, y, z))
    }

    pub fn base(self) -> BlockPos {
        BlockPos(SIZE as i32 * self.0)
    }
}

impl BlockPos {
    pub fn offset(self, x: i32, y: i32, z: i32) -> Self {
        BlockPos(self.0 + vector!(x, y, z))
    }

    pub fn base(self) -> WorldPos {
        WorldPos(point!(self.0.x as f32, self.0.y as f32, self.0.z as f32))
    }

    pub fn chunk_pos_offset(self) -> (ChunkPos, Vector3<i32>) {
        let cpos: ChunkPos = self.into();
        let bpos = self.0 - cpos.base().0;

        (cpos, bpos)
    }

    pub fn center(self) -> WorldPos {
        WorldPos(point!(
            self.0.x as f32 + 0.5,
            self.0.y as f32 + 0.5,
            self.0.z as f32 + 0.5
        ))
    }
}

impl WorldPos {
    pub fn xyz(x: f32, y: f32, z: f32) -> Self {
        WorldPos(point!(x, y, z))
    }

    pub fn offset(self, x: f32, y: f32, z: f32) -> Self {
        WorldPos(self.0 + vector!(x, y, z))
    }
}

// TODO: chunk paging:
// - Immediate (same-frame) chunk paging (should be used sparingly and when
//   you're fairly confident those chunks are already loaded)
// - Async chunk queries (with time to live)
// - Persistent chunk mappings (make sure certain chunks neveer get unmapped
//   while locked)

#[derive(Debug)]
struct KeyedThreadedProducer<K, T> {
    generator_pool: ThreadPool,
    in_progress: HashSet<K>,
    back: Arc<Mutex<HashMap<K, T>>>,
    front: HashMap<K, T>,
}

impl<K: Copy + Hash + Eq, T: Send + Sync + 'static> KeyedThreadedProducer<K, T> {
    pub fn new(generator_pool: ThreadPool) -> Self {
        Self {
            generator_pool,
            in_progress: Default::default(),
            back: Default::default(),
            front: Default::default(),
        }
    }

    pub fn queue<F>(&mut self, key: K, producer: F)
    where
        F: FnOnce() -> T + Send + Sync + 'static,
        K: Send + Sync + 'static,
    {
        if self.in_progress.insert(key) {
            let back_ref = Arc::clone(&self.back);
            self.generator_pool.spawn(move || {
                let item = producer();
                back_ref.lock().unwrap().insert(key, item);
            });
        }
    }

    pub fn is_queued(&self, key: K) -> bool {
        self.in_progress.contains(&key)
    }

    pub fn drain<'a>(&'a mut self) -> impl Iterator<Item = (K, T)> + 'a {
        std::mem::swap(&mut *self.back.lock().unwrap(), &mut self.front);
        self.in_progress.retain(|key| !self.front.contains_key(key));
        self.front.drain()
    }
}

#[derive(Debug)]
pub struct VoxelWorld {
    pub registry: Arc<BlockRegistry>,
    noise_generator: Arc<gen::NoiseGenerator>,

    chunks: HashMap<ChunkPos, ChunkType>,
    chunk_producer: KeyedThreadedProducer<ChunkPos, ChunkType>,

    new_chunks_sender: Sender<ChunkPos>,
    pub new_chunks_notifier: Receiver<ChunkPos>,
    modified_chunks_sender: Sender<ChunkPos>,
    pub modified_chunks_notifier: Receiver<ChunkPos>,
}

fn iter_pos(start: BlockPos, end: BlockPos, mut func: impl FnMut(BlockPos)) {
    for x in start.0.x..=end.0.x {
        for y in start.0.y..=end.0.y {
            for z in start.0.z..=end.0.z {
                func(BlockPos(point!(x, y, z)));
            }
        }
    }
}

impl VoxelWorld {
    pub fn new(registry: BlockRegistry) -> Self {
        let generator_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap();

        let (new_chunk_tx, new_chunk_rx) = crossbeam_channel::unbounded();
        let (modified_chunk_tx, modified_chunk_rx) = crossbeam_channel::unbounded();

        VoxelWorld {
            noise_generator: Arc::new(gen::NoiseGenerator::new_default(&registry)),
            registry: Arc::new(registry),
            chunks: Default::default(),
            chunk_producer: KeyedThreadedProducer::new(generator_pool),

            new_chunks_sender: new_chunk_tx,
            new_chunks_notifier: new_chunk_rx,
            modified_chunks_sender: modified_chunk_tx,
            modified_chunks_notifier: modified_chunk_rx,
        }
    }

    pub fn load_chunk(&mut self, pos: ChunkPos) {
        if !does_chunk_need_loading(self, pos) {
            return;
        }

        log::debug!("loading chunk {:?}", pos);

        let noise = Arc::clone(&self.noise_generator);
        self.chunk_producer
            .queue(pos, move || noise.make_chunk(pos));
    }

    pub fn unload_chunk(&mut self, pos: ChunkPos) {
        self.chunks.remove(&pos);
    }

    pub fn chunk(&self, pos: ChunkPos) -> Option<&ChunkType> {
        self.chunks.get(&pos)
    }

    /// Tries to replace the block at `pos`, returning the block that was
    /// replaced if it was found
    // oh god...
    pub fn set_block_id(&mut self, pos: BlockPos, block: BlockId) -> Option<BlockId> {
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

        iter_pos(pos.offset(-1, -1, -1), pos.offset(1, 1, 1), |pos| {
            if self.chunks.contains_key(&pos.into()) {
                self.modified_chunks_sender.send(pos.into()).unwrap();
            }
        });

        self.chunks.get_mut(&chunk_pos).map(|chunk| match chunk {
            ChunkType::Array(chunk) => std::mem::replace(&mut chunk[block_pos], block),
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

    pub fn update(&mut self) {
        for (pos, chunk) in self.chunk_producer.drain() {
            self.chunks.insert(pos, chunk);
            self.new_chunks_sender.send(pos).unwrap();
        }
    }

    // pub fn trace_block(
    //     &self,
    //     ray: Ray3<f32>,
    //     radius: f32,
    //     // debug: &mut DebugSection,
    // ) -> Option<(BlockPos, Option<Vector3<i32>>)> {
    //     let mut ret_pos = WorldPos(ray.origin).into();
    //     let mut ret_norm = None;

    //     if trace_ray(ray, radius, |pos, norm| {
    //         ret_pos = pos;
    //         ret_norm = norm;
    //         // debug.block(pos, 1.0, Vector4::new(1.0, 0.0, 1.0, 1.0));
    //         self.registry(pos)
    //             .map(|props| props.opaque())
    //             .unwrap_or(false)
    //     }) {
    //         Some((ret_pos, ret_norm))
    //     } else {
    //         None
    //     }
    // }
}

fn does_chunk_need_loading(world: &mut VoxelWorld, pos: ChunkPos) -> bool {
    !world.chunks.contains_key(&pos) && !world.chunk_producer.is_queued(pos)
}

pub fn load_neighborhood(world: &mut VoxelWorld, pos: ChunkPos, radius: i32) {
    fn alternating_sequence(radius: i32) -> impl Iterator<Item = i32> {
        use std::iter::once;
        once(0)
            .chain(1..radius)
            .flat_map(|x| once(-x).chain(once(x)))
    }

    for xoff in alternating_sequence(radius) {
        for zoff in alternating_sequence(radius) {
            for yoff in alternating_sequence(radius) {
                let pos = pos.offset(xoff, yoff, zoff);
                if does_chunk_need_loading(world, pos) {
                    world.load_chunk(pos);
                }
            }
        }
    }
}

#[legion::system]
pub fn update_world(#[resource] world: &mut VoxelWorld) {
    world.update();
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct ChunkLoader {
    pub radius: usize,
}

#[derive(Debug)]
pub struct ChunkLoaderContext {
    loader_heatmap: HashMap<ChunkPos, Vector3<usize>>,
}

impl ChunkLoaderContext {
    // fn
}

/*

B -> A
C -> B

A  A  A  A  A
A  A  A  A  A  C
A  A  Aa A  Ac B  B  B
A  A  A  A  A  B  B  B
A  A  A  A  A  Bb B  B
         B  B  B  B  B
         B  B  B  B  B

*/

impl ChunkLoaderContext {
    pub fn new() -> Self {
        Self {
            loader_heatmap: Default::default(),
        }
    }
}

#[legion::system(for_each)]
pub fn load_chunks(
    #[state] ctx: &mut ChunkLoaderContext,
    #[resource] world: &mut VoxelWorld,
    chunk_loader: &ChunkLoader,
    transform: &Transform,
) {
    let pos = WorldPos(transform.translation.vector.into()).into();
    load_neighborhood(world, pos, chunk_loader.radius as i32);
}

// fn int_bound(ray: Ray3<f32>, axis: usize) -> f32 {
//     if ray.direction[axis] < 0.0 {
//         let mut new = ray;
//         new.origin[axis] *= -1.0;
//         new.direction[axis] *= -1.0;
//         int_bound(new, axis)
//     } else {
//         (1.0 - crate::util::modulo(ray.origin[axis], 1.0)) /
// ray.direction[axis]     }
// }

// fn trace_ray<F>(ray: Ray3<f32>, radius: f32, mut func: F) -> bool
// where
//     F: FnMut(BlockPos, Option<Vector3<i32>>) -> bool,
// {
//     // FIXME: actually do something when looking straight up/down! please!
//     if ray.direction.y == 0.0 {
//         return false;
//     }

//     // init phase
//     let origin: BlockPos = WorldPos(ray.origin).into();
//     let mut current = origin.0;
//     let step_x = ray.direction.x.signum();
//     let step_y = ray.direction.y.signum();
//     let step_z = ray.direction.z.signum();

//     let mut t_max_x = int_bound(ray, 0);
//     let mut t_max_y = int_bound(ray, 1);
//     let mut t_max_z = int_bound(ray, 2);

//     let t_delta_x = step_x / ray.direction.x;
//     let t_delta_y = step_y / ray.direction.y;
//     let t_delta_z = step_z / ray.direction.z;

//     let step_x = step_x as i32;
//     let step_y = step_y as i32;
//     let step_z = step_z as i32;
//     let mut normal = None;

//     // incremental pahse
//     for _ in 0..3000 {
//         if func(BlockPos(current), normal) {
//             return true;
//         }

//         if t_max_x < t_max_y {
//             if t_max_x < t_max_z {
//                 if t_max_x > radius {
//                     break;
//                 }
//                 current.x += step_x;
//                 t_max_x += t_delta_x;
//                 normal = Some(vector!(-step_x, 0, 0));
//             } else {
//                 if t_max_z > radius {
//                     break;
//                 }
//                 current.z += step_z;
//                 t_max_z += t_delta_z;
//                 normal = Some(vector!(0, 0, -step_z));
//             }
//         } else {
//             if t_max_y < t_max_z {
//                 if t_max_y > radius {
//                     break;
//                 }
//                 current.y += step_y;
//                 t_max_y += t_delta_y;
//                 normal = Some(vector!(0, -step_y, 0));
//             } else {
//                 if t_max_z > radius {
//                     break;
//                 }
//                 current.z += step_z;
//                 t_max_z += t_delta_z;
//                 normal = Some(vector!(0, 0, -step_z));
//             }
//         }
//     }

//     false
// }
