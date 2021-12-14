use crate::engine::world::chunk::{ChunkKind, CHUNK_LENGTH};
use crossbeam_channel::{Receiver, Sender};
use legion::{world::SubWorld, Entity, IntoQuery, World};
use nalgebra::Point3;
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLockReadGuard, RwLockWriteGuard,
    },
};

pub use self::chunk::ArrayChunk;
use self::{
    chunk::{Chunk, ChunkPos},
    registry::BlockRegistry,
};

use super::transform::Transform;

pub mod chunk;
pub mod gen;
pub mod registry;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl From<BlockPos> for Point3<i32> {
    fn from(BlockPos { x, y, z }: BlockPos) -> Self {
        nalgebra::point![x, y, z]
    }
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct WorldPos {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl From<WorldPos> for Point3<f32> {
    fn from(WorldPos { x, y, z }: WorldPos) -> Self {
        nalgebra::point![x, y, z]
    }
}

impl From<BlockPos> for ChunkPos {
    fn from(pos: BlockPos) -> Self {
        let x = crate::util::floor_div(pos.x, CHUNK_LENGTH as i32);
        let y = crate::util::floor_div(pos.y, CHUNK_LENGTH as i32);
        let z = crate::util::floor_div(pos.z, CHUNK_LENGTH as i32);
        ChunkPos { x, y, z }
    }
}

impl From<WorldPos> for ChunkPos {
    fn from(pos: WorldPos) -> Self {
        BlockPos::from(pos).into()
    }
}

impl From<WorldPos> for BlockPos {
    fn from(pos: WorldPos) -> Self {
        BlockPos {
            x: pos.x.floor() as i32,
            y: pos.y.floor() as i32,
            z: pos.z.floor() as i32,
        }
    }
}

impl ChunkPos {
    pub fn new<I: Into<[i32; 3]>>(pos: I) -> Self {
        let [x, y, z] = pos.into();
        Self { x, y, z }
    }

    pub fn offset<I: Into<[i32; 3]>>(self, offset: I) -> Self {
        let [dx, dy, dz] = offset.into();
        Self {
            x: dx + self.x,
            y: dy + self.y,
            z: dz + self.z,
        }
    }

    pub fn origin(self) -> BlockPos {
        BlockPos {
            x: CHUNK_LENGTH as i32 * self.x,
            y: CHUNK_LENGTH as i32 * self.y,
            z: CHUNK_LENGTH as i32 * self.z,
        }
    }
}

impl BlockPos {
    pub fn new<I: Into<[i32; 3]>>(pos: I) -> Self {
        let [x, y, z] = pos.into();
        Self { x, y, z }
    }

    pub fn offset<I: Into<[i32; 3]>>(self, offset: I) -> Self {
        let [dx, dy, dz] = offset.into();
        Self {
            x: dx + self.x,
            y: dy + self.y,
            z: dz + self.z,
        }
    }

    pub fn origin(self) -> WorldPos {
        WorldPos {
            x: self.x as f32,
            y: self.y as f32,
            z: self.z as f32,
        }
    }

    // pub fn chunk_pos_offset(self) -> (ChunkPos, ChunkOffset) {
    //     let cpos: ChunkPos = self.into();
    //     let bpos = self.0 - cpos.origin().0;
    //     let bpos = ChunkOffset::new(bpos.x as u16, bpos.y as u16, bpos.z as u16);

    //     (cpos, bpos)
    // }
}

impl WorldPos {
    pub fn new<I: Into<[f32; 3]>>(pos: I) -> Self {
        let [x, y, z] = pos.into();
        Self { x, y, z }
    }

    pub fn offset<I: Into<[f32; 3]>>(self, offset: I) -> Self {
        let [dx, dy, dz] = offset.into();
        Self {
            x: dx + self.x,
            y: dy + self.y,
            z: dz + self.z,
        }
    }
}

// #[derive(Debug)]
// struct KeyedThreadedProducer<K, T> {
//     pool: ThreadPool,

//     // map of active keys to a cancellation key
//     in_progress: flurry::HashMap<K, Arc<AtomicBool>>,
//     cancelled: flurry::HashSet<K>,
//     finished_tx: Sender<K>,

//     destination: Arc<flurry::HashMap<K, T>>,
// }

// impl<K: Copy + Hash + Ord + Eq + Send + Sync + 'static, T: Send + Sync +
// 'static>     KeyedThreadedProducer<K, T>
// {
//     pub fn new(
//         num_threads: Option<usize>,
//         destination: Arc<flurry::HashMap<K, T>>,
//     ) -> (Self, Receiver<K>) {
//         let pool = ThreadPoolBuilder::new()
//             .num_threads(num_threads.unwrap_or(0))
//             .build()
//             .unwrap();

//         let (finished_tx, finished_rx) = crossbeam_channel::unbounded();

//         (
//             Self {
//                 pool,
//                 in_progress: Default::default(),
//                 cancelled: Default::default(),
//                 finished_tx,
//                 destination,
//             },
//             finished_rx,
//         )
//     }

//     pub fn queue<F>(&self, key: K, computation: F)
//     where
//         F: FnOnce() -> T + Send + Sync + 'static,
//         K: Send + Sync + 'static,
//     {
//         let guard = self.in_progress.guard();
//         if !self.in_progress.contains_key(&key, &guard) {
//             let cancelled = Arc::new(AtomicBool::new(false));
//             self.in_progress.insert(key, Arc::clone(&cancelled), &guard);

//             let destination = Arc::clone(&self.destination);
//             self.pool.spawn(move || {
//                 if !cancelled.load(Ordering::SeqCst) {
//                     let result = computation();
//                     destination.pin().insert(key, result);
//                 }
//             });
//         }
//     }

//     pub fn is_queued(&self, key: K) -> bool {
//         self.in_progress.pin().contains_key(&key)
//     }

//     pub fn cancel(&self, key: K) {
//         if let Some(cancelled) = self.in_progress.pin().remove(&key) {
//             cancelled.store(true, Ordering::SeqCst);
//             self.cancelled.pin().insert(key);
//         }
//     }
// }

#[derive(Debug)]
pub struct WorldChunks(Arc<flurry::HashMap<ChunkPos, Arc<Chunk>>>);

impl WorldChunks {
    pub fn guard(&self) -> flurry::epoch::Guard {
        self.0.guard()
    }

    pub fn read<'g>(
        &'g self,
        pos: ChunkPos,
        guard: &'g flurry::epoch::Guard,
    ) -> Option<RwLockReadGuard<'g, ChunkKind>> {
        self.0.get(&pos, guard).and_then(|chunk| chunk.read())
    }

    // pub fn write(
    //     &self,
    //     pos: ChunkPos,
    //     guard: &flurry::epoch::Guard,
    // ) -> Option<RwLockWriteGuard<ChunkKind>> {
    //     self.0.pin().get(&pos).and_then(|chunk| chunk.write())
    // }
}

// struct ReadLockedChunks<'w> {
//     chunks: HashMap<ChunkPos, RwLockReadGuard<'w, ChunkKind>>,
// }

// impl<'w> ReadLockedChunks<'w> {
//     pub fn lock<I>(
//         chunks: &'w WorldChunks,
//         guard: &'w flurry::epoch::Guard,
//         iter: I,
//     ) -> Option<Self>
//     where
//         I: IntoIterator<Item = ChunkPos>,
//     {
//         let chunks = iter
//             .into_iter()
//             .map(|pos| chunks.read(pos, guard).map(|guard| (pos, guard)))
//             .collect::<Option<_>>()?;

//         Some(ReadLockedChunks { chunks })
//     }
// }

#[derive(Debug)]
pub struct VoxelWorld {
    pub registry: Arc<BlockRegistry>,
    pub noise_generator: Arc<gen::NoiseGenerator>,
    pub chunk_event_notifier: Receiver<ChunkEvent>,

    chunks: Arc<flurry::HashMap<ChunkPos, Arc<Chunk>>>,
    chunk_event_sender: Sender<ChunkEvent>,

    world_gen_pool: ThreadPool,

    // map of active keys to a cancellation key
    chunks_in_progress: Arc<flurry::HashMap<ChunkPos, Arc<AtomicBool>>>,
}

#[derive(Clone, Debug)]
pub enum ChunkEvent {
    Added(Arc<Chunk>),
    Removed(Arc<Chunk>),
    Modified(Arc<Chunk>),
}

impl VoxelWorld {
    pub fn new(registry: Arc<BlockRegistry>) -> Self {
        let (chunk_event_tx, chunk_event_rx) = crossbeam_channel::unbounded();
        let world_gen_pool = ThreadPoolBuilder::new().build().unwrap();

        VoxelWorld {
            noise_generator: Arc::new(gen::NoiseGenerator::new_default(&registry)),
            registry,
            chunks: Default::default(),
            chunks_in_progress: Default::default(),

            world_gen_pool,

            chunk_event_sender: chunk_event_tx,
            chunk_event_notifier: chunk_event_rx,
        }
    }

    pub fn queue_world_gen_task<F>(&self, pos: ChunkPos, task: F)
    where
        F: FnOnce() -> ChunkKind + Send + Sync + 'static,
    {
        let guard = self.chunks_in_progress.guard();
        if !self.chunks_in_progress.contains_key(&pos, &guard) {
            let is_cancelled = Arc::new(AtomicBool::new(false));
            self.chunks_in_progress
                .insert(pos, Arc::clone(&is_cancelled), &guard);

            let chunks = Arc::clone(&self.chunks);
            let chunk_event_sender = Sender::clone(&self.chunk_event_sender);
            let chunks_in_progress = Arc::clone(&self.chunks_in_progress);

            self.world_gen_pool.spawn(move || {
                if !is_cancelled.load(Ordering::SeqCst) {
                    let chunk = Arc::new(Chunk::new(pos, task()));
                    if !is_cancelled.load(Ordering::SeqCst) {
                        chunks.pin().insert(pos, Arc::clone(&chunk));
                        chunk_event_sender.send(ChunkEvent::Added(chunk)).unwrap();
                        chunks_in_progress.pin().remove(&pos);
                    }
                }
            });
        }
    }

    pub fn load_chunk(&mut self, pos: ChunkPos) {
        if self.chunks_in_progress.pin().contains_key(&pos) || self.chunks.pin().contains_key(&pos)
        {
            return;
        }

        let noise = Arc::clone(&self.noise_generator);
        self.queue_world_gen_task(pos, move || noise.make_chunk(pos));
    }

    pub fn unload_chunk(&mut self, pos: ChunkPos) {
        if let Some(cancelled) = self.chunks_in_progress.pin().remove(&pos) {
            cancelled.store(true, Ordering::SeqCst);
        } else if let Some(chunk) = self.chunks.pin().remove(&pos) {
            self.chunk_event_sender
                .send(ChunkEvent::Removed(Arc::clone(chunk)))
                .unwrap();
        }
    }

    pub fn chunk(&self, pos: ChunkPos) -> Option<Arc<Chunk>> {
        self.chunks.pin().get(&pos).map(Arc::clone)
    }

    pub fn chunks(&self) -> WorldChunks {
        WorldChunks(Arc::clone(&self.chunks))
    }

    pub fn guard(&self) -> flurry::epoch::Guard {
        self.chunks.guard()
    }

    pub fn read<'w>(
        &'w self,
        pos: ChunkPos,
        guard: &'w flurry::epoch::Guard,
    ) -> Option<RwLockReadGuard<'w, ChunkKind>> {
        self.chunks.get(&pos, guard).and_then(|chunk| chunk.read())
    }

    pub fn write<'w>(
        &'w self,
        pos: ChunkPos,
        guard: &'w flurry::epoch::Guard,
    ) -> Option<RwLockWriteGuard<'w, ChunkKind>> {
        self.chunks.get(&pos, guard).and_then(|chunk| chunk.write())
    }

    // pub fn registry(&self, pos: BlockPos) -> Option<RegistryRef> {
    //     self.get_block_id(pos).map(|id| self.registry.get_ref(id))
    // }

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

#[legion::system]
pub fn update_world(#[resource] _world: &mut VoxelWorld) {
    // world.update();
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
    for x in center.x - radius..=center.x + radius {
        for y in center.y - radius..=center.y + radius {
            for z in center.z - radius..=center.z + radius {
                func(ChunkPos { x, y, z });
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

    let to_unload: Vec<_> = ctx.loaded_set.difference(&keep_loaded).copied().collect();
    let mut to_load: Vec<_> = keep_loaded.difference(&ctx.loaded_set).copied().collect();

    // TODO: sort by distance to closest loader
    // group all positions that only differ in their vertical position, so that
    // world gen tasks are ordered in a way that should hit the generator's surface
    // height cache more often.
    to_load.sort_unstable_by_key(|pos| (pos.x, pos.z));

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
            let pos = WorldPos::new(transform.translation.vector).into();
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
