use crate::engine::world::chunk::{ChunkType, SIZE};
use crossbeam_channel::{Receiver, Sender};
use legion::{world::SubWorld, Entity, IntoQuery, World};
use nalgebra::{point, vector, Point3, Vector3};
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    hash::Hash,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex, MutexGuard,
    },
    thread::{JoinHandle, Thread},
};

pub use self::chunk::Chunk;
use self::registry::{BlockId, BlockRegistry, RegistryRef};

use super::transform::Transform;

pub mod chunk;
pub mod gen;
pub mod registry;

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

#[derive(Debug)]
struct KeyedThreadedProducer<K, T> {
    pool: ThreadPool,

    finished_rx: Receiver<(K, T)>,
    finished_tx: Sender<(K, T)>,

    // map of active keys to a cancellation key
    in_progress: HashMap<K, Arc<AtomicBool>>,
    not_cancellable: HashSet<K>,
    cancelled: HashSet<K>,
}

impl<K: Copy + Hash + Eq + Send + Sync + 'static, T: Send + Sync + 'static>
    KeyedThreadedProducer<K, T>
{
    pub fn new(num_threads: Option<usize>) -> Self {
        let (finished_tx, finished_rx) = crossbeam_channel::unbounded();
        let pool = ThreadPoolBuilder::new()
            .num_threads(num_threads.unwrap_or(0))
            .build()
            .unwrap();

        Self {
            pool,
            finished_rx,
            finished_tx,
            in_progress: Default::default(),
            not_cancellable: Default::default(),
            cancelled: Default::default(),
        }
    }

    pub fn queue<F>(&mut self, key: K, computation: F)
    where
        F: FnOnce() -> T + Send + Sync + 'static,
        K: Send + Sync + 'static,
    {
        if !self.in_progress.contains_key(&key) {
            let cancelled = Arc::new(AtomicBool::new(false));
            self.in_progress.insert(key, Arc::clone(&cancelled));

            let finished_tx = self.finished_tx.clone();
            self.pool.spawn(move || {
                if !cancelled.load(Ordering::SeqCst) {
                    finished_tx.send((key, computation())).unwrap();
                }
            });
        }
    }

    pub fn is_queued(&self, key: K) -> bool {
        self.in_progress.contains_key(&key)
    }

    pub fn cancel(&mut self, key: K) {
        if let Some(cancelled) = self.in_progress.remove(&key) {
            cancelled.store(true, Ordering::SeqCst);
            self.cancelled.insert(key);
        }
    }

    pub fn drain<'a>(&'a mut self) -> impl Iterator<Item = (K, T)> + 'a {
        self.finished_rx.try_iter().filter(|(key, _)| {
            // i know, iterator abuse :(
            self.in_progress.remove(key);
            !self.cancelled.remove(key)
        })
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
    removed_chunks_sender: Sender<ChunkPos>,
    pub removed_chunks_notifier: Receiver<ChunkPos>,

    chunk_event_sender: Sender<ChunkEvent>,
    pub chunk_event_notifier: Receiver<ChunkEvent>,
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ChunkEvent {
    Added(ChunkPos),
    Removed(ChunkPos),
    Modified(ChunkPos),
}

impl VoxelWorld {
    pub fn new(registry: Arc<BlockRegistry>) -> Self {
        let (new_chunk_tx, new_chunk_rx) = crossbeam_channel::unbounded();
        let (modified_chunk_tx, modified_chunk_rx) = crossbeam_channel::unbounded();
        let (removed_chunk_tx, removed_chunk_rx) = crossbeam_channel::unbounded();
        let (chunk_event_tx, chunk_event_rx) = crossbeam_channel::unbounded();

        VoxelWorld {
            noise_generator: Arc::new(gen::NoiseGenerator::new_default(&registry)),
            registry,
            chunks: Default::default(),
            chunk_producer: KeyedThreadedProducer::new(None),

            new_chunks_sender: new_chunk_tx,
            new_chunks_notifier: new_chunk_rx,
            modified_chunks_sender: modified_chunk_tx,
            modified_chunks_notifier: modified_chunk_rx,
            removed_chunks_sender: removed_chunk_tx,
            removed_chunks_notifier: removed_chunk_rx,
            chunk_event_sender: chunk_event_tx,
            chunk_event_notifier: chunk_event_rx,
        }
    }

    pub fn load_chunk(&mut self, pos: ChunkPos) {
        if !does_chunk_need_loading(self, pos) {
            return;
        }

        let noise = Arc::clone(&self.noise_generator);
        self.chunk_producer
            .queue(pos, move || noise.make_chunk(pos));
    }

    pub fn unload_chunk(&mut self, pos: ChunkPos) {
        if !self.chunks.contains_key(&pos) {
            return;
        }

        self.chunk_producer.cancel(pos);
        self.chunks.remove(&pos);
        self.removed_chunks_sender.send(pos).unwrap();
        self.chunk_event_sender
            .send(ChunkEvent::Removed(pos))
            .unwrap();
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

    pub fn registry(&self, pos: BlockPos) -> Option<RegistryRef> {
        self.get_block_id(pos).map(|id| self.registry.get_ref(id))
    }

    pub fn update(&mut self) {
        for (pos, chunk) in self.chunk_producer.drain() {
            self.chunks.insert(pos, chunk);
            self.new_chunks_sender.send(pos).unwrap();
            self.chunk_event_sender
                .send(ChunkEvent::Added(pos))
                .unwrap();
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
    loader_events: Receiver<legion::world::Event>,

    loaders: HashMap<Entity, (ChunkLoader, ChunkPos)>,
    loaded_set: HashSet<ChunkPos>,
}

impl ChunkLoaderContext {
    pub fn new(world: &mut World) -> Self {
        let (sender, loader_events) = crossbeam_channel::unbounded();
        world.subscribe(sender, legion::component::<ChunkLoader>());

        Self {
            loader_events,
            loaders: Default::default(),
            loaded_set: Default::default(),
        }
    }
}

fn neighborhood(center: ChunkPos, radius: usize, mut func: impl FnMut(ChunkPos)) {
    let radius = radius as i32;
    for x in center.0.x - radius..=center.0.x + radius {
        for y in center.0.y - radius..=center.0.y + radius {
            for z in center.0.z - radius..=center.0.z + radius {
                func(ChunkPos(point!(x, y, z)));
            }
        }
    }
}

fn recheck_loaded(ctx: &mut ChunkLoaderContext, voxel_world: &mut VoxelWorld) {
    let mut keep_loaded = HashSet::new();

    for &(loader, pos) in ctx.loaders.values() {
        neighborhood(pos, loader.radius, |pos| {
            keep_loaded.insert(pos);
        });
    }

    let to_unload: HashSet<_> = ctx.loaded_set.difference(&keep_loaded).copied().collect();
    let to_load: HashSet<_> = keep_loaded.difference(&ctx.loaded_set).copied().collect();

    for pos in to_load {
        voxel_world.load_chunk(pos);
        ctx.loaded_set.insert(pos);
    }

    for pos in to_unload {
        voxel_world.unload_chunk(pos);
        ctx.loaded_set.remove(&pos);
    }
}

fn remove_loader(ctx: &mut ChunkLoaderContext, voxel_world: &mut VoxelWorld, entity: Entity) {
    ctx.loaders.remove(&entity);
    recheck_loaded(ctx, voxel_world);
}

fn update_loader(
    ctx: &mut ChunkLoaderContext,
    voxel_world: &mut VoxelWorld,
    entity: Entity,
    loader: &ChunkLoader,
    pos: ChunkPos,
) {
    if let Some(&(_, previous_pos)) = ctx.loaders.get(&entity) {
        if previous_pos != pos {
            ctx.loaders.get_mut(&entity).unwrap().1 = pos;
            recheck_loaded(ctx, voxel_world);
        }
    } else {
        ctx.loaders.insert(entity, (*loader, pos));
        recheck_loaded(ctx, voxel_world);
    }
}

#[legion::system]
#[read_component(ChunkLoader)]
#[read_component(Transform)]
pub fn load_chunks(
    #[state] ctx: &mut ChunkLoaderContext,
    #[resource] voxel_world: &mut VoxelWorld,

    world: &mut SubWorld,
) {
    // let mut removed = HashSet::new();
    // for event in ctx.loader_events.try_iter() {
    //     match event {
    //         legion::world::Event::EntityRemoved(entity, _) =>
    // drop(removed.insert(entity)),         _ => {}
    //     }
    // }

    // removed
    //     .into_iter()
    //     .for_each(|entity| remove_loader(ctx, voxel_world, entity));

    <(Entity, &ChunkLoader, &Transform)>::query()
        .filter(legion::maybe_changed::<Transform>())
        .for_each(world, |(&entity, loader, transform)| {
            let pos = WorldPos(transform.translation.vector.into()).into();
            update_loader(ctx, voxel_world, entity, loader, pos);
        });
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
