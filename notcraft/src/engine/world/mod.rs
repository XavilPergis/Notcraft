use crate::engine::world::chunk::CHUNK_LENGTH;
use crossbeam_channel::{Receiver, Sender};
use legion::{world::SubWorld, Entity, IntoQuery, World};
use nalgebra::Point3;
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

pub use self::chunk::ArrayChunk;
use self::{
    chunk::{Chunk, ChunkPos},
    registry::BlockRegistry,
};

use super::transform::Transform;

pub mod chunk;
pub mod generation;
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

    pub fn chunk_and_offset(self) -> (ChunkPos, [usize; 3]) {
        let chunk_pos = ChunkPos::from(self);
        let block_base = chunk_pos.origin();
        let offset = [
            (self.x - block_base.x) as usize,
            (self.y - block_base.y) as usize,
            (self.z - block_base.z) as usize,
        ];

        (chunk_pos, offset)
    }
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ChunkHeightmapPos {
    pub x: i32,
    pub z: i32,
}

impl From<ChunkPos> for ChunkHeightmapPos {
    fn from(pos: ChunkPos) -> Self {
        Self { x: pos.x, z: pos.z }
    }
}

pub struct VoxelWorld {
    pub registry: Arc<BlockRegistry>,
    pub chunk_event_notifier: Receiver<ChunkEvent>,

    chunks: Arc<flurry::HashMap<ChunkPos, Arc<Chunk>>>,
    chunk_event_sender: Sender<ChunkEvent>,
    dirty_chunks_rx: Receiver<ChunkPos>,
    dirty_chunks_tx: Sender<ChunkPos>,

    world_gen_pool: ThreadPool,
    chunk_generator: Arc<generation::ChunkGenerator>,
    surface_cache: Arc<generation::SurfaceHeighmapCache>,

    // map of active keys to a cancellation key
    chunks_in_progress: Arc<flurry::HashMap<ChunkPos, Arc<AtomicBool>>>,
}

#[derive(Clone)]
pub enum ChunkEvent {
    Added(Arc<Chunk>),
    Removed(Arc<Chunk>),
    Modified(Arc<Chunk>),
}

impl VoxelWorld {
    pub fn new(registry: Arc<BlockRegistry>) -> Arc<Self> {
        let (chunk_event_tx, chunk_event_rx) = crossbeam_channel::unbounded();
        let (dirty_chunks_tx, dirty_chunks_rx) = crossbeam_channel::unbounded();
        let world_gen_pool = ThreadPoolBuilder::new().build().unwrap();

        let chunk_generator = Arc::new(generation::ChunkGenerator::new_default(&registry));

        Arc::new(VoxelWorld {
            registry,
            chunks: Default::default(),
            chunks_in_progress: Default::default(),

            world_gen_pool,
            chunk_generator,
            surface_cache: Default::default(),

            chunk_event_sender: chunk_event_tx,
            chunk_event_notifier: chunk_event_rx,
            dirty_chunks_tx,
            dirty_chunks_rx,
        })
    }

    pub fn load_chunk(self: &Arc<Self>, pos: ChunkPos) {
        if self.chunks_in_progress.pin().contains_key(&pos) || self.chunks.pin().contains_key(&pos)
        {
            return;
        }

        let guard = self.chunks_in_progress.guard();
        if !self.chunks_in_progress.contains_key(&pos, &guard) {
            let is_cancelled = Arc::new(AtomicBool::new(false));
            self.chunks_in_progress
                .insert(pos, Arc::clone(&is_cancelled), &guard);

            let world = Arc::clone(self);
            self.world_gen_pool.spawn(move || {
                if !is_cancelled.load(Ordering::SeqCst) {
                    let heights = world.surface_cache.surface_heights(pos.into());
                    let chunk_data = world.chunk_generator.make_chunk(pos, heights);

                    let chunk = Arc::new(Chunk::new(&world.dirty_chunks_tx, pos, chunk_data));

                    // insert before and remove if cancelled to prevent a user from cancelling world
                    // chunk after we check whether the chunk was cancelled
                    let guard = world.chunks_in_progress.guard();
                    world.chunks.insert(pos, Arc::clone(&chunk), &guard);

                    if !is_cancelled.load(Ordering::SeqCst) {
                        world
                            .chunk_event_sender
                            .send(ChunkEvent::Added(chunk))
                            .unwrap();
                        world.chunks_in_progress.pin().remove(&pos);
                    } else {
                        world.chunks.remove(&pos, &guard);
                    }
                }
            });
        }
    }

    pub fn unload_chunk(&self, pos: ChunkPos) {
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

    fn update(&self) {
        self.surface_cache.evict_after(Duration::from_secs(10));
        let guard = self.chunks.guard();
        for chunk in self.dirty_chunks_rx.try_iter() {
            if let Some(chunk) = self.chunks.get(&chunk, &guard) {
                chunk::flush_chunk_writes(chunk);
            }
        }
    }
}

#[legion::system]
pub fn update_world(#[resource] world: &Arc<VoxelWorld>) {
    world.update();
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct DynamicChunkLoader {
    pub load_radius: usize,
    pub unload_radius: usize,
}

#[derive(Debug)]
pub struct ChunkLoaderContext {
    loader_events: Receiver<legion::world::Event>,

    loaders: HashMap<Entity, (DynamicChunkLoader, ChunkPos)>,
    loaded_set: HashSet<ChunkPos>,
}

impl ChunkLoaderContext {
    pub fn new(world: &mut World) -> Self {
        let (sender, loader_events) = crossbeam_channel::unbounded();
        world.subscribe(sender, legion::component::<DynamicChunkLoader>());

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

fn recheck_loaded(ctx: &mut ChunkLoaderContext, voxel_world: &Arc<VoxelWorld>) {
    let mut should_be_loaded = HashSet::new();
    let mut should_keep_loaded = HashSet::new();

    for &(loader, pos) in ctx.loaders.values() {
        neighborhood(pos, loader.load_radius, |pos| {
            should_be_loaded.insert(pos);
        });
    }

    for &(loader, pos) in ctx.loaders.values() {
        neighborhood(pos, loader.unload_radius, |pos| {
            should_keep_loaded.insert(pos);
        });
    }

    let to_unload: Vec<_> = ctx
        .loaded_set
        .difference(&should_keep_loaded)
        .copied()
        .collect();

    let mut to_load: Vec<_> = should_be_loaded
        .difference(&ctx.loaded_set)
        .copied()
        .collect();

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

fn remove_loader(ctx: &mut ChunkLoaderContext, voxel_world: &Arc<VoxelWorld>, entity: Entity) {
    ctx.loaders.remove(&entity);
    recheck_loaded(ctx, voxel_world);
}

fn update_loader(
    ctx: &mut ChunkLoaderContext,
    voxel_world: &Arc<VoxelWorld>,
    entity: Entity,
    loader: &DynamicChunkLoader,
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
#[read_component(DynamicChunkLoader)]
#[read_component(Transform)]
pub fn load_chunks(
    #[state] ctx: &mut ChunkLoaderContext,
    #[resource] voxel_world: &Arc<VoxelWorld>,

    world: &mut SubWorld,
) {
    let mut removed = HashSet::new();
    for event in ctx.loader_events.try_iter() {
        match event {
            legion::world::Event::EntityRemoved(entity, _) => drop(removed.insert(entity)),
            _ => {}
        }
    }

    removed
        .into_iter()
        .for_each(|entity| remove_loader(ctx, voxel_world, entity));

    <(Entity, &DynamicChunkLoader, &Transform)>::query()
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
