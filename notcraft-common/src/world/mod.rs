use nalgebra::{Point3, Scalar, Vector3};
use parking_lot::{Mutex, RwLock};
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::{
    borrow::Borrow,
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    hash::Hash,
    ops::{Index, IndexMut},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

pub use self::chunk::ArrayChunk;
use self::{
    chunk::{Chunk, ChunkAccess, ChunkSection, ChunkSectionPos, CompactedChunkSection},
    registry::{load_registry, BlockRegistry, CollisionType},
};
use crate::{
    aabb::Aabb, debug::send_debug_event, prelude::*, transform::Transform, util::ChannelPair,
    world::chunk::CHUNK_LENGTH, Axis, Side,
};

pub mod chunk;
pub mod generation;
pub mod lighting;
pub mod orphan;
pub mod registry;

pub mod debug {
    use super::{chunk::ChunkSectionPos, ChunkPos};
    use crate::debug_events;

    pub enum WorldLoadEvent {
        Loaded(ChunkPos),
        Unloaded(ChunkPos),
        Modified(ChunkPos),
        LoadedSection(ChunkSectionPos),
        UnloadedSection(ChunkSectionPos),
        ModifiedSection(ChunkSectionPos),
    }
    pub enum WorldAccessEvent {
        Read(ChunkSectionPos),
        Written(ChunkSectionPos),
        Orphaned(ChunkSectionPos),
    }

    debug_events! {
        events,
        WorldLoadEvent => "world-load",
        WorldAccessEvent => "world-access",
    }
}

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

impl From<BlockPos> for ChunkSectionPos {
    fn from(pos: BlockPos) -> Self {
        let x = crate::util::floor_div(pos.x, CHUNK_LENGTH as i32);
        let y = crate::util::floor_div(pos.y, CHUNK_LENGTH as i32);
        let z = crate::util::floor_div(pos.z, CHUNK_LENGTH as i32);
        ChunkSectionPos { x, y, z }
    }
}

impl From<WorldPos> for ChunkSectionPos {
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

impl ChunkSectionPos {
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

    pub fn chunk_and_offset(self) -> (ChunkSectionPos, [usize; 3]) {
        let chunk_pos = ChunkSectionPos::from(self);
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
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

impl ChunkPos {
    pub fn section(&self, y: i32) -> ChunkSectionPos {
        ChunkSectionPos {
            x: self.x,
            y,
            z: self.z,
        }
    }
}

impl From<ChunkSectionPos> for ChunkPos {
    fn from(pos: ChunkSectionPos) -> Self {
        Self { x: pos.x, z: pos.z }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
enum LoadEvent {
    Load(ChunkPos),
    Unload(ChunkPos),
    LoadSection(ChunkSectionPos),
    UnloadSection(ChunkSectionPos),
}

type ConcurrentHashMap<K, V> = flurry::HashMap<K, V>;

pub struct VoxelWorld {
    // TODO: probably a good idea to remove this
    pub registry: Arc<BlockRegistry>,

    updating_mutex: Mutex<()>,

    chunks: ConcurrentHashMap<ChunkPos, Arc<Chunk>>,
    compacted_columns: ConcurrentHashMap<ChunkPos, CompactedChunkSection>,
}

struct WorldGenerator {
    pool: ThreadPool,
    generator: Arc<generation::ChunkGenerator>,
    surface_cache: Arc<generation::SurfaceHeighmapCache>,
    finished_chunks: ChannelPair<Chunk>,
    finished_sections: ChannelPair<ChunkSection>,
}

impl WorldGenerator {
    pub fn new(registry: &BlockRegistry) -> Self {
        // TODO: make configurable
        let pool = ThreadPoolBuilder::new().build().unwrap();
        let generator = Arc::new(generation::ChunkGenerator::new_default(&registry));

        Self {
            pool,
            generator,
            surface_cache: Default::default(),
            finished_chunks: Default::default(),
            finished_sections: Default::default(),
        }
    }
}

#[derive(Clone)]
pub enum WorldEvent {
    Loaded(Arc<Chunk>),
    Unloaded(Arc<Chunk>),

    LoadedSection(Arc<ChunkSection>),
    UnloadedSection(Arc<ChunkSection>),
    ModifiedSection(Arc<ChunkSection>),
}

impl VoxelWorld {
    pub fn new(registry: &Arc<BlockRegistry>) -> Arc<Self> {
        Arc::new(VoxelWorld {
            registry: Arc::clone(registry),
            updating_mutex: Default::default(),
            chunks: Default::default(),
            compacted_columns: Default::default(),
        })
    }

    pub fn is_loaded(&self, pos: ChunkPos) -> bool {
        self.chunks.pin().contains_key(&pos)
    }

    pub fn is_section_loaded(&self, pos: ChunkSectionPos) -> bool {
        self.chunk(pos.column())
            .map_or(false, |chunk| chunk.is_loaded(pos.y))
    }

    pub fn chunk(&self, pos: ChunkPos) -> Option<Arc<Chunk>> {
        self.chunks.pin().get(&pos).map(Arc::clone)
    }

    pub fn section(&self, pos: ChunkSectionPos) -> Option<Arc<ChunkSection>> {
        Some(self.chunk(pos.column())?.section(pos.y)?)
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

        let world = VoxelWorld::new(&registry);
        app.insert_resource(ChunkAccess::new(&world));
        app.insert_resource(world);

        app.insert_resource(Arc::new(WorldGenerator::new(&registry)));
        app.insert_resource(registry);

        app.insert_resource(LoadQueue::default());

        app.add_event::<WorldEvent>();

        app.add_system_to_stage(CoreStage::PostUpdate, apply_chunk_updates.system());
        app.add_system_to_stage(CoreStage::PostUpdate, generate_world.system());
        app.add_system_to_stage(CoreStage::PostUpdate, load_chunks.system());
    }
}

#[derive(Default)]
pub struct LoadQueue {
    inner: Arc<RwLock<MutableLoadQueue>>,
}

impl LoadQueue {
    pub fn load(&self, pos: ChunkPos) {
        let events = &mut self.inner.write().events;
        events.push_back(LoadEvent::Load(pos));
    }

    pub fn unload(&self, pos: ChunkPos) {
        let events = &mut self.inner.write().events;
        events.push_back(LoadEvent::Unload(pos));
    }

    pub fn load_section(&self, pos: ChunkSectionPos) {
        let events = &mut self.inner.write().events;
        events.push_back(LoadEvent::LoadSection(pos));
    }

    pub fn unload_section(&self, pos: ChunkSectionPos) {
        let events = &mut self.inner.write().events;
        events.push_back(LoadEvent::UnloadSection(pos));
    }
}

// `btree.pop_front()` isnt stable yet :(
fn btree_pop_front<K: Ord + Copy, V>(btree: &mut BTreeMap<K, V>) -> Option<V> {
    let &key = btree.keys().next()?;
    btree.remove(&key)
}

// a queue that discards insertions if that element is already in the queue
#[derive(Clone, Debug)]
pub struct DedupQueue<T> {
    head: usize,
    queue: BTreeMap<usize, T>,
    dedup_map: HashMap<T, usize>,
}

impl<T> Default for DedupQueue<T> {
    fn default() -> Self {
        Self {
            head: Default::default(),
            queue: Default::default(),
            dedup_map: Default::default(),
        }
    }
}

impl<T: Hash + Eq> DedupQueue<T> {
    pub fn push_back(&mut self, value: T) -> bool
    where
        T: Copy,
    {
        if self.dedup_map.contains_key(&value) {
            return false;
        }

        self.queue.insert(self.head, value);
        self.dedup_map.insert(value, self.head);
        self.head += 1;
        true
    }

    pub fn pop_front(&mut self) -> Option<T> {
        let value = btree_pop_front(&mut self.queue)?;
        self.dedup_map.remove(&value);
        if self.queue.is_empty() {
            self.head = 0;
            self.dedup_map.clear();
        }
        Some(value)
    }

    pub fn remove<Q>(&mut self, key: &Q) -> Option<T>
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.dedup_map
            .remove(key)
            .map(|id| self.queue.remove(&id).unwrap())
    }

    pub fn pop_iter(&mut self) -> impl Iterator<Item = T> + '_ {
        std::iter::from_fn(|| self.pop_front())
    }
}

#[derive(Default)]
struct MutableLoadQueue {
    load: DedupQueue<ChunkPos>,
    unload: DedupQueue<ChunkPos>,
    load_sections: DedupQueue<ChunkSectionPos>,
    unload_sections: DedupQueue<ChunkSectionPos>,

    events: VecDeque<LoadEvent>,
}

fn process_load_events(world: &VoxelWorld, queues: &mut MutableLoadQueue) {
    assert!(world.updating_mutex.is_locked());

    for event in queues.events.drain(..) {
        match event {
            LoadEvent::Load(pos) => {
                queues.unload.remove(&pos);
                if !world.is_loaded(pos) {
                    queues.load.push_back(pos);
                }
            }
            LoadEvent::Unload(pos) => {
                queues.load.remove(&pos);
                if world.is_loaded(pos) {
                    queues.unload.push_back(pos);
                }
            }
            LoadEvent::LoadSection(pos) => {
                assert!(
                    world.is_loaded(pos.column()),
                    "tried loading section for unloaded chunk"
                );
                queues.unload_sections.remove(&pos);
                if !world.is_section_loaded(pos) {
                    queues.load_sections.push_back(pos);
                }
            }
            LoadEvent::UnloadSection(pos) => {
                assert!(
                    world.is_loaded(pos.column()),
                    "tried unloading section for unloaded chunk"
                );
                queues.load_sections.remove(&pos);
                if world.is_section_loaded(pos) {
                    queues.unload_sections.push_back(pos);
                }
            }
        }
    }
}

fn run_chunk_generation_task(generator: Arc<WorldGenerator>, pos: ChunkPos) {
    // if let Some(compacted) = self.compacted_chunks.pin().get(&pos) {
    //     let chunk_data = compacted.decompact();
    //     let chunk = Arc::new(Chunk::new(pos, chunk_data, &self.registry));

    //     self.insert_chunk(chunk, &is_cancelled);
    //     return;
    // }

    let heights = generator.surface_cache.surface_heights(pos.into());
    let chunk = Chunk::new(pos, heights);

    let _ = generator.finished_chunks.tx.send(chunk);
}

fn run_chunk_section_generation_task(
    chunk: Arc<Chunk>,
    pos: i32,
    generator: Arc<WorldGenerator>,
    registry: Arc<BlockRegistry>,
) {
    // if let Some(compacted) = self.compacted_chunks.pin().get(&pos) {
    //     let chunk_data = compacted.decompact();
    //     let chunk = Arc::new(Chunk::new(pos, chunk_data, &self.registry));

    //     self.insert_chunk(chunk, &is_cancelled);
    //     return;
    // }

    let pos = chunk.pos().section(pos);
    let chunk_data = generator.generator.make_chunk(pos, &chunk.heights());
    let chunk = ChunkSection::new(pos, chunk_data, &registry);

    let _ = generator.finished_sections.tx.send(chunk);
}

fn apply_chunk_updates(
    world: Res<Arc<VoxelWorld>>,
    mut access: ResMut<ChunkAccess>,
    mut chunk_events: EventWriter<WorldEvent>,
) {
    let mut rebuild_set = HashSet::new();

    // TODO: think about what section updates might do to the chunk's data, like
    // updating heightmaps and such
    chunk::flush_chunk_access(&mut access, &mut rebuild_set);

    for &pos in rebuild_set.iter() {
        if let Some(chunk) = world.section(pos) {
            chunk_events.send(WorldEvent::ModifiedSection(chunk));
            send_debug_event(debug::WorldLoadEvent::ModifiedSection(pos));
        }
    }
}

fn generate_world(
    world: Res<Arc<VoxelWorld>>,
    registry: Res<Arc<BlockRegistry>>,
    load_queue: Res<LoadQueue>,
    generator: Res<Arc<WorldGenerator>>,
    mut chunk_events: EventWriter<WorldEvent>,
) {
    generator.surface_cache.evict_after(Duration::from_secs(10));

    // because im paranoid lol.
    let _guard = match world.updating_mutex.try_lock() {
        Some(it) => it,
        None => return,
    };

    let mut queues = load_queue.inner.write();
    process_load_events(&world, &mut queues);

    for pos in queues.load.pop_iter().take(1) {
        // TODO: assert that we arent loading already-loaded chunks

        let generator_ref = Arc::clone(&generator);
        generator.pool.spawn(move || {
            run_chunk_generation_task(generator_ref, pos);
        });
    }

    for pos in queues.load_sections.pop_iter().take(8) {
        // TODO: assert that we arent loading already-loaded chunks

        let chunk = world.chunk(pos.column()).unwrap();
        let generator_ref = Arc::clone(&generator);
        let registry_ref = Arc::clone(&registry);

        generator.pool.spawn(move || {
            run_chunk_section_generation_task(chunk, pos.y, generator_ref, registry_ref);
        });
    }

    for finished in generator.finished_chunks.rx.try_iter() {
        let chunk = Arc::new(finished);

        // FIXME: drop generated chunks that have since been unloaded
        assert!(!world.chunks.pin().contains_key(&chunk.pos()));
        world.chunks.pin().insert(chunk.pos(), Arc::clone(&chunk));

        send_debug_event(debug::WorldLoadEvent::Loaded(chunk.pos()));
        chunk_events.send(WorldEvent::Loaded(chunk));
    }

    for finished in generator.finished_sections.rx.try_iter() {
        if let Some(chunk) = world.chunk(finished.pos().column()) {
            let section = Arc::new(finished);

            // FIXME: drop generated chunks that have since been unloaded
            assert!(!chunk.is_loaded(section.pos().y));
            chunk.insert(section.pos().y, Arc::clone(&section));

            send_debug_event(debug::WorldLoadEvent::LoadedSection(section.pos()));
            chunk_events.send(WorldEvent::LoadedSection(section));
        }
    }

    for pos in queues.unload_sections.pop_iter() {
        if let Some(chunk) = world.chunk(pos.column()) {
            let section = chunk.remove(pos.y).unwrap();

            send_debug_event(debug::WorldLoadEvent::UnloadedSection(pos));
            chunk_events.send(WorldEvent::UnloadedSection(section));
        }
    }

    for pos in queues.unload.pop_iter() {
        let chunk = Arc::clone(world.chunks.pin().remove(&pos).unwrap());

        for section in chunk.sections().pin().values() {
            let section = chunk.remove(section.pos().y).unwrap();

            send_debug_event(debug::WorldLoadEvent::UnloadedSection(section.pos()));
            chunk_events.send(WorldEvent::UnloadedSection(section));
        }

        send_debug_event(debug::WorldLoadEvent::Unloaded(pos));
        chunk_events.send(WorldEvent::Unloaded(chunk));
    }
}

pub fn chunk_section_aabb(pos: ChunkSectionPos) -> Aabb {
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
    prev_loaders: HashMap<Entity, (DynamicChunkLoader, ChunkSectionPos)>,
    loaded_chunk_set: HashSet<ChunkPos>,
    waiting_sections: HashMap<ChunkPos, HashSet<i32>>,
}

fn recheck_loaded_chunks(ctx: &mut ChunkLoaderContext, load_queue: &LoadQueue) {
    let mut should_be_loaded = HashSet::new();
    let mut should_keep_loaded = HashSet::new();

    for &(loader, pos) in ctx.prev_loaders.values() {
        for x in pos.x - loader.load_radius as i32..=pos.x + loader.load_radius as i32 {
            for z in pos.z - loader.load_radius as i32..=pos.z + loader.load_radius as i32 {
                let chunk_pos = ChunkPos { x, z };
                should_be_loaded.insert(chunk_pos);
                for y in pos.y - loader.load_radius as i32..=pos.y + loader.load_radius as i32 {
                    ctx.waiting_sections.entry(chunk_pos).or_default().insert(y);
                }
            }
        }
    }

    for &(loader, pos) in ctx.prev_loaders.values() {
        for x in pos.x - loader.unload_radius as i32..=pos.x + loader.unload_radius as i32 {
            for z in pos.z - loader.unload_radius as i32..=pos.z + loader.unload_radius as i32 {
                let chunk_pos = ChunkPos { x, z };
                should_keep_loaded.insert(chunk_pos);
            }
        }
    }

    let to_unload: Vec<_> = ctx
        .loaded_chunk_set
        .difference(&should_keep_loaded)
        .copied()
        .collect();

    let mut to_load: Vec<_> = should_be_loaded
        .difference(&ctx.loaded_chunk_set)
        .copied()
        .collect();

    // TODO: sort by distance to closest loader
    to_load.sort_unstable_by_key(|pos| (pos.x, pos.z));

    for pos in to_load {
        load_queue.load(pos);
        ctx.loaded_chunk_set.insert(pos);
    }

    for pos in to_unload {
        load_queue.unload(pos);
        ctx.loaded_chunk_set.remove(&pos);
        ctx.waiting_sections.remove(&pos);
    }
}

fn remove_loader(ctx: &mut ChunkLoaderContext, load_queue: &LoadQueue, entity: Entity) {
    ctx.prev_loaders.remove(&entity);
    recheck_loaded_chunks(ctx, load_queue);
}

fn update_loader(
    ctx: &mut ChunkLoaderContext,
    load_queue: &LoadQueue,
    entity: Entity,
    loader: &DynamicChunkLoader,
    pos: ChunkSectionPos,
) {
    if let Some(&(_, previous_pos)) = ctx.prev_loaders.get(&entity) {
        if previous_pos != pos {
            ctx.prev_loaders.get_mut(&entity).unwrap().1 = pos;
            recheck_loaded_chunks(ctx, load_queue);
        }
    } else {
        ctx.prev_loaders.insert(entity, (*loader, pos));
        recheck_loaded_chunks(ctx, load_queue);
    }
}

pub fn load_chunks(
    mut ctx: Local<ChunkLoaderContext>,
    load_queue: Res<LoadQueue>,
    query: Query<(Entity, &DynamicChunkLoader, &Transform), Changed<Transform>>,
    removed: RemovedComponents<DynamicChunkLoader>,
    mut chunk_events: EventReader<WorldEvent>,
) {
    removed
        .iter()
        .for_each(|entity| remove_loader(&mut ctx, &load_queue, entity));

    query.for_each(|(entity, loader, transform)| {
        let pos = WorldPos::new(transform.translation.vector).into();
        update_loader(&mut *ctx, &load_queue, entity, loader, pos);
    });

    for event in chunk_events.iter() {
        match event {
            WorldEvent::Loaded(chunk) => {
                if let Some(waiting) = ctx.waiting_sections.remove(&chunk.pos()) {
                    for &waiting in waiting.iter() {
                        let pos = chunk.pos().section(waiting);
                        load_queue.load_section(pos);
                    }
                }
            }
            _ => {}
        }
    }
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
pub fn trace_ray(cache: &mut ChunkAccess, ray: Ray3<f32>, radius: f32) -> Option<RaycastHit> {
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
        match cache.registry().collision_type(id) {
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
