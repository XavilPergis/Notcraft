use crossbeam_channel::{Receiver, Sender};
use nalgebra::{Point3, Scalar, Vector3};
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    ops::{Index, IndexMut},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

pub use self::chunk::ArrayChunk;
use self::{
    chunk::{Chunk, ChunkPos, ChunkSnapshotCache, CompactedChunk},
    registry::{load_registry, BlockId, BlockRegistry, CollisionType},
};
use crate::{aabb::Aabb, prelude::*, transform::Transform, world::chunk::CHUNK_LENGTH, Axis, Side};

pub mod chunk;
pub mod generation;
pub mod registry;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Index<Axis> for BlockPos {
    type Output = i32;

    fn index(&self, index: Axis) -> &Self::Output {
        match index {
            Axis::X => &self.x,
            Axis::Y => &self.y,
            Axis::Z => &self.z,
        }
    }
}

impl IndexMut<Axis> for BlockPos {
    fn index_mut(&mut self, index: Axis) -> &mut Self::Output {
        match index {
            Axis::X => &mut self.x,
            Axis::Y => &mut self.y,
            Axis::Z => &mut self.z,
        }
    }
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
    chunk_event_tx: Sender<ChunkEvent>,
    chunk_event_rx: Receiver<ChunkEvent>,

    chunks: Arc<flurry::HashMap<ChunkPos, Arc<Chunk>>>,
    modified_chunks: Arc<flurry::HashMap<ChunkPos, CompactedChunk>>,
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
            modified_chunks: Default::default(),
            chunks_in_progress: Default::default(),

            world_gen_pool,
            chunk_generator,
            surface_cache: Default::default(),

            chunk_event_tx,
            chunk_event_rx,
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

            if let Some(compacted) = self.modified_chunks.pin().get(&pos) {
                let chunk_data = compacted.decompact();
                let chunk = Arc::new(Chunk::new(&self.dirty_chunks_tx, pos, chunk_data));

                // insert before and remove if cancelled to prevent a user from cancelling world
                // chunk after we check whether the chunk was cancelled
                self.chunks.insert(pos, Arc::clone(&chunk), &guard);

                if !is_cancelled.load(Ordering::SeqCst) {
                    self.chunk_event_tx.send(ChunkEvent::Added(chunk)).unwrap();
                    self.chunks_in_progress.pin().remove(&pos);
                    // add_transient_debug_box(Duration::from_secs(1), DebugBox
                    // {     bounds: chunk_aabb(pos),
                    //     rgba: [0.0, 0.0, 1.0, 1.0],
                    //     kind: DebugBoxKind::Solid,
                    // });
                } else {
                    self.chunks.remove(&pos, &guard);
                }

                return;
            }

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
                        world.chunk_event_tx.send(ChunkEvent::Added(chunk)).unwrap();
                        world.chunks_in_progress.pin().remove(&pos);
                        // add_transient_debug_box(Duration::from_secs(1),
                        // DebugBox {     bounds:
                        // chunk_aabb(pos),
                        //     rgba: [0.0, 1.0, 0.0, 1.0],
                        //     kind: DebugBoxKind::Solid,
                        // });
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
            // add_transient_debug_box(Duration::from_secs(1), DebugBox {
            //     bounds: chunk_aabb(chunk.pos()),
            //     rgba: [1.0, 0.0, 0.0, 1.0],
            //     kind: DebugBoxKind::Solid,
            // });

            // save this chunk if it differs from what was originally generated
            if chunk.was_ever_modified() {
                let compacted = CompactedChunk::compact(chunk.snapshot().data());
                self.modified_chunks.pin().insert(pos, compacted);
            }

            self.chunk_event_tx
                .send(ChunkEvent::Removed(Arc::clone(chunk)))
                .unwrap();
        }
    }

    pub fn chunk(&self, pos: ChunkPos) -> Option<Arc<Chunk>> {
        self.chunks.pin().get(&pos).map(Arc::clone)
    }

    fn update(&self, mut chunk_events: EventWriter<ChunkEvent>) {
        self.surface_cache.evict_after(Duration::from_secs(10));
        let guard = self.chunks.guard();
        for chunk in self.dirty_chunks_rx.try_iter() {
            if let Some(chunk) = self.chunks.get(&chunk, &guard) {
                let mut rebuild_set = HashSet::new();
                chunk::flush_chunk_writes(chunk, &mut rebuild_set);
                for &pos in rebuild_set.iter() {
                    if let Some(chunk) = self.chunk(pos) {
                        self.chunk_event_tx
                            .send(ChunkEvent::Modified(chunk))
                            .unwrap();
                    }
                }
            }
        }

        for event in self.chunk_event_rx.try_iter() {
            chunk_events.send(event);
        }
    }

    pub fn set_block(&self, pos: BlockPos, id: BlockId) -> Option<()> {
        let (chunk_pos, chunk_index) = pos.chunk_and_offset();
        self.chunk(chunk_pos)?.queue_write(chunk_index, id);
        Some(())
    }
}

#[derive(Debug, Default)]
pub struct WorldPlugin {
    registry_path: Option<PathBuf>,
}

impl WorldPlugin {
    pub fn with_registry_path<P: AsRef<Path>>(mut self, path: &P) -> Self {
        self.registry_path = Some(path.as_ref().into());
        self
    }
}

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut AppBuilder) {
        let registry = load_registry(
            self.registry_path
                .clone()
                .unwrap_or_else(|| "resources/blocks.json".into()),
        )
        .unwrap();

        app.insert_resource(VoxelWorld::new(Arc::clone(&registry)));
        app.insert_resource(registry);

        app.add_event::<ChunkEvent>();

        app.add_system_to_stage(CoreStage::PostUpdate, update_world.system());
        app.add_system_to_stage(CoreStage::PostUpdate, load_chunks.system());
    }
}

pub fn update_world(world: Res<Arc<VoxelWorld>>, chunk_events: EventWriter<ChunkEvent>) {
    world.update(chunk_events);
}

pub fn chunk_aabb(pos: ChunkPos) -> Aabb {
    let len = chunk::CHUNK_LENGTH as f32;
    let pos = len * nalgebra::point![pos.x as f32, pos.y as f32, pos.z as f32];
    Aabb {
        min: pos,
        max: pos + len * nalgebra::vector![1.0, 1.0, 1.0],
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct DynamicChunkLoader {
    pub load_radius: usize,
    pub unload_radius: usize,
}

#[derive(Debug, Default)]
pub struct ChunkLoaderContext {
    loaders: HashMap<Entity, (DynamicChunkLoader, ChunkPos)>,
    loaded_set: HashSet<ChunkPos>,
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

pub fn load_chunks(
    mut ctx: Local<ChunkLoaderContext>,
    voxel_world: Res<Arc<VoxelWorld>>,
    query: Query<(Entity, &DynamicChunkLoader, &Transform), Changed<Transform>>,
    removed: RemovedComponents<DynamicChunkLoader>,
) {
    removed
        .iter()
        .for_each(|entity| remove_loader(&mut ctx, &voxel_world, entity));

    query.for_each(|(entity, loader, transform)| {
        let pos = WorldPos::new(transform.translation.vector).into();
        update_loader(&mut *ctx, &voxel_world, entity, loader, pos);
    });
}

fn block_distance_sq(a: BlockPos, b: BlockPos) -> f32 {
    let x = f32::abs(a.x as f32 - b.x as f32);
    let y = f32::abs(a.y as f32 - b.y as f32);
    let z = f32::abs(a.z as f32 - b.z as f32);
    x * x + y * y + z * z
}

#[derive(Copy, Clone, Debug)]
pub struct Ray3<T: Scalar> {
    pub direction: Vector3<T>,
    pub origin: Point3<T>,
}

#[must_use]
pub fn trace_ray(
    cache: &mut ChunkSnapshotCache,
    ray: Ray3<f32>,
    radius: f32,
) -> Option<RaycastHit> {
    let start_block = BlockPos {
        x: ray.origin.x.floor() as i32,
        y: ray.origin.y.floor() as i32,
        z: ray.origin.z.floor() as i32,
    };
    trace_ray_generic(ray, |pos| {
        if block_distance_sq(start_block, pos) > radius * radius {
            return RaycastStep::Exit;
        }
        let id = match cache.block(pos) {
            None => return RaycastStep::Exit,
            Some(id) => id,
        };
        match cache.world.registry.collision_type(id) {
            CollisionType::Solid => RaycastStep::Hit,
            _ => RaycastStep::Continue,
        }
    })
}

#[derive(Copy, Clone, Debug)]
pub struct RaycastHit {
    pub pos: BlockPos,
    // a `None` side means the block we started in was an immediate hit
    pub side: Option<Side>,
}

#[derive(Copy, Clone, Debug)]
pub enum RaycastStep {
    Continue,
    Exit,
    Hit,
}

fn f32_checked_div(num: f32, denom: f32) -> Option<f32> {
    if denom == 0.0 {
        None
    } else {
        Some(num / denom)
    }
}

fn trace_ray_generic<F>(ray: Ray3<f32>, mut func: F) -> Option<RaycastHit>
where
    F: FnMut(BlockPos) -> RaycastStep,
{
    // init phase
    let origin = BlockPos {
        x: ray.origin.x.floor() as i32,
        y: ray.origin.y.floor() as i32,
        z: ray.origin.z.floor() as i32,
    };

    let mut current = origin;
    let step_x = ray.direction.x.signum();
    let step_y = ray.direction.y.signum();
    let step_z = ray.direction.z.signum();

    let next_x = origin.x as f32 + if step_x < 0.0 { 0.0 } else { 1.0 };
    let next_y = origin.y as f32 + if step_y < 0.0 { 0.0 } else { 1.0 };
    let next_z = origin.z as f32 + if step_z < 0.0 { 0.0 } else { 1.0 };

    // the distance along the ray from `current` where each axis meets the next
    // voxel. if the ray is parallel with an axis, then the ray will never
    // intersect, and we should never try to step in that axis, so we use f32::MAX
    // in that case, so everything will compare smaller.
    let mut t_max_x = f32_checked_div(next_x - ray.origin.x, ray.direction.x).unwrap_or(f32::MAX);
    let mut t_max_y = f32_checked_div(next_y - ray.origin.y, ray.direction.y).unwrap_or(f32::MAX);
    let mut t_max_z = f32_checked_div(next_z - ray.origin.z, ray.direction.z).unwrap_or(f32::MAX);

    // if the ray direction is 0 on a particular axis, then we don't ever step along
    // that axis, and this delta value is kind of meaningless, so we just stuff it
    // with a dummy value.
    let t_delta_x = f32_checked_div(step_x, ray.direction.x).unwrap_or(f32::MAX);
    let t_delta_y = f32_checked_div(step_y, ray.direction.y).unwrap_or(f32::MAX);
    let t_delta_z = f32_checked_div(step_z, ray.direction.z).unwrap_or(f32::MAX);

    let step_x = step_x as i32;
    let step_y = step_y as i32;
    let step_z = step_z as i32;
    let mut hit_axis = None;

    // incremental pahse
    loop {
        match func(current) {
            RaycastStep::Continue => {}
            RaycastStep::Exit => break None,
            RaycastStep::Hit => {
                let side = hit_axis.map(|axis| match axis {
                    Axis::X if step_x > 0 => Side::Left,
                    Axis::X => Side::Right,
                    Axis::Y if step_y > 0 => Side::Bottom,
                    Axis::Y => Side::Top,
                    Axis::Z if step_z > 0 => Side::Back,
                    Axis::Z => Side::Front,
                });
                break Some(RaycastHit { pos: current, side });
            }
        }

        // find smallest step along the ray that we can take and still remain inside the
        // current voxel, which will put us on the boundary of the next.
        if t_max_x < t_max_y && t_max_x < t_max_z {
            current.x += step_x;
            t_max_x += t_delta_x;
            hit_axis = Some(Axis::X);
        } else if t_max_y < t_max_z {
            current.y += step_y;
            t_max_y += t_delta_y;
            hit_axis = Some(Axis::Y);
        } else {
            current.z += step_z;
            t_max_z += t_delta_z;
            hit_axis = Some(Axis::Z);
        }
    }
}
