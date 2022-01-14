use crate::{
    debug::send_debug_event,
    world::{
        lighting::{propagate_block_light, propagate_sky_light, LightUpdateQueues},
        registry::BlockId,
    },
};
use nalgebra::Point3;
use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    ops::{Index, IndexMut},
    sync::{
        atomic::{AtomicBool, Ordering as AtomicOrdering},
        Arc,
    },
};

use super::{
    generation::SurfaceHeightmap,
    lighting::{LightValue, SkyLightColumns, FULL_SKY_LIGHT},
    orphan::{Orphan, OrphanSnapshot, OrphanWriter},
    registry::BlockRegistry,
    BlockPos, ChunkPos, VoxelWorld,
};

// The width of the chunk is `2 ^ SIZE_BITS`
pub const CHUNK_LENGTH_BITS: usize = 5;

pub const CHUNK_LENGTH: usize = 1 << CHUNK_LENGTH_BITS;
pub const CHUNK_AREA: usize = CHUNK_LENGTH * CHUNK_LENGTH;
pub const CHUNK_VOLUME: usize = CHUNK_LENGTH * CHUNK_LENGTH * CHUNK_LENGTH;

#[derive(Clone)]
pub struct ChunkSectionInner {
    pos: ChunkSectionPos,
    block_data: ChunkData<BlockId>,
    light_data: ChunkData<LightValue>,
}

impl ChunkSectionInner {
    fn new(
        pos: ChunkSectionPos,
        block_data: ChunkData<BlockId>,
        block_light_data: ChunkData<LightValue>,
    ) -> Self {
        Self {
            pos,
            block_data,
            light_data: block_light_data,
        }
    }
}

#[derive(Clone)]
pub struct ChunkSectionSnapshot {
    inner: OrphanSnapshot<ChunkSectionInner>,
}

impl ChunkSectionSnapshot {
    fn new(inner: OrphanSnapshot<ChunkSectionInner>) -> Self {
        Self { inner }
    }

    pub fn pos(&self) -> ChunkSectionPos {
        self.inner.pos
    }

    pub fn blocks(&self) -> &ChunkData<BlockId> {
        &self.inner.block_data
    }

    pub fn light(&self) -> &ChunkData<LightValue> {
        &self.inner.light_data
    }

    /// See [`OrphanSnapshot::is_orphaned`]
    pub fn is_orphaned(&self) -> bool {
        self.inner.is_orphaned()
    }
}

pub struct ChunkSectionSnapshotMut {
    inner: OrphanWriter<ChunkSectionInner>,
}

impl ChunkSectionSnapshotMut {
    fn new(inner: OrphanWriter<ChunkSectionInner>) -> Self {
        Self { inner }
    }

    pub fn pos(&self) -> ChunkSectionPos {
        self.inner.pos
    }

    pub fn blocks(&self) -> &ChunkData<BlockId> {
        &self.inner.block_data
    }

    pub fn light(&self) -> &ChunkData<LightValue> {
        &self.inner.light_data
    }

    pub fn blocks_mut(&mut self) -> &mut ChunkData<BlockId> {
        &mut self.inner.block_data
    }

    pub fn light_mut(&mut self) -> &mut ChunkData<LightValue> {
        &mut self.inner.light_data
    }

    pub fn was_cloned(&self) -> bool {
        self.inner.was_cloned()
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ChunkSectionUpdate {
    pub index: ChunkSectionIndex,
    pub id: BlockId,
}

pub struct Chunk {
    pos: ChunkPos,

    heights: Orphan<SurfaceHeightmap>,
    sky_light: Orphan<SkyLightColumns>,

    sections: flurry::HashMap<i32, Arc<ChunkSection>>,
    compacted_sections: flurry::HashMap<ChunkSectionPos, CompactedChunkSection>,
}

impl Chunk {
    pub fn new(pos: ChunkPos, heights: SurfaceHeightmap) -> Self {
        Self {
            pos,
            sky_light: Orphan::new(SkyLightColumns::initialize(&heights)),
            heights: Orphan::new(heights),
            sections: Default::default(),
            compacted_sections: Default::default(),
        }
    }

    pub fn heights(&self) -> OrphanSnapshot<SurfaceHeightmap> {
        self.heights.snapshot()
    }

    pub fn sky_light(&self) -> OrphanSnapshot<SkyLightColumns> {
        self.sky_light.snapshot()
    }

    pub fn pos(&self) -> ChunkPos {
        self.pos
    }

    pub fn section(&self, y: i32) -> Option<Arc<ChunkSection>> {
        self.sections.pin().get(&y).map(Arc::clone)
    }

    pub fn sections(&self) -> &flurry::HashMap<i32, Arc<ChunkSection>> {
        &self.sections
    }

    pub fn insert(&self, y: i32, section: Arc<ChunkSection>) -> Option<Arc<ChunkSection>> {
        self.sections.pin().insert(y, section).map(Arc::clone)
    }

    pub fn remove(&self, y: i32) -> Option<Arc<ChunkSection>> {
        self.sections.pin().remove(&y).map(Arc::clone)
    }

    pub fn is_empty(&self) -> bool {
        self.sections.pin().is_empty()
    }

    pub fn is_loaded(&self, y: i32) -> bool {
        self.sections.pin().contains_key(&y)
    }
}

// pub struct CompactedChunk {
//     heights: SurfaceHeightmap,
//     sky_light: SkyLightColumns,

//     compacted_chunks: flurry::HashMap<ChunkPos, CompactedChunk>,
// }

// impl CompactedChunk {
//     pub fn compact(column: &Arc<Chunk>) -> Self {
//         // Self {
//         //     sky_light: column.sky_light.clone_inner(),
//         //     heights: column.heights.clone_inner(),
//         //     compacted_chunks: todo!(),
//         //     // compacted_chunks: std::mem::take(&mut
//         // column.compacted_chunks.write()), }
//         todo!()
//     }

//     pub fn decompact(self) -> Chunk {
//         // Chunk {
//         //     heights: Orphan::new(self.heights),
//         //     sky_light: Orphan::new(self.sky_light),
//         //     sections: Default::default(),
//         //     compacted_chunks: self.compacted_chunks,
//         // }
//         todo!()
//     }
// }

pub struct ChunkSection {
    pos: ChunkSectionPos,
    inner: Orphan<ChunkSectionInner>,
    was_ever_modified: AtomicBool,
}

fn default_light(registry: &BlockRegistry, id: BlockId) -> LightValue {
    let sky_light = match registry.light_transmissible(id) {
        true => 15,
        false => 0,
    };
    let block_light = registry.block_light(id);
    LightValue::pack(sky_light, block_light)
}

impl ChunkSection {
    pub fn new(
        pos: ChunkSectionPos,
        block_data: ChunkData<BlockId>,
        registry: &BlockRegistry,
    ) -> Self {
        let block_light_data = match &block_data {
            &ChunkData::Homogeneous(id) => ChunkData::Homogeneous(default_light(registry, id)),
            ChunkData::Array(ids) => {
                let mut light = ArrayChunk::homogeneous(FULL_SKY_LIGHT);
                for (i, &id) in ids.data.iter().enumerate() {
                    light.data[i] = default_light(registry, id);
                }
                ChunkData::Array(light)
            }
        };

        let inner = Orphan::new(ChunkSectionInner::new(pos, block_data, block_light_data));

        Self {
            pos,
            inner,
            was_ever_modified: AtomicBool::new(false),
        }
    }

    pub fn pos(&self) -> ChunkSectionPos {
        self.pos
    }

    pub fn snapshot(&self) -> ChunkSectionSnapshot {
        ChunkSectionSnapshot::new(self.inner.snapshot())
    }

    pub(crate) fn was_ever_modified(&self) -> bool {
        self.was_ever_modified.load(AtomicOrdering::Relaxed)
    }
}

struct ChunkUpdateContext<'a> {
    pub rebuild: &'a mut HashSet<ChunkSectionPos>,
    pub solid_updates: &'a mut HashMap<ChunkIndex, HashMap<i32, bool>>,
    pub light_queues: &'a mut LightUpdateQueues,
    pub registry: &'a BlockRegistry,
    pub chunk: ChunkPos,
}

fn write_chunk_updates_array(
    data: &mut ArrayChunk<BlockId>,
    ctx: &mut ChunkUpdateContext,
    y: i32,
    updates: &[ChunkSectionUpdate],
) {
    const MAX_AXIS_INDEX: usize = CHUNK_LENGTH - 1;

    let mut c = false;
    let mut nx = false;
    let mut px = false;
    let mut ny = false;
    let mut py = false;
    let mut nz = false;
    let mut pz = false;

    for update in updates.iter() {
        let slot = &mut data[update.index];

        let new_solid = !ctx.registry.light_transmissible(update.id);
        if !ctx.registry.light_transmissible(*slot) != new_solid {
            let [x, yi, z] = update.index;
            let y = CHUNK_LENGTH as i32 * y + yi as i32;
            ctx.solid_updates
                .entry([x, z])
                .or_default()
                .insert(y, new_solid);
        }

        if *slot != update.id {
            c = true;
            nx |= update.index[0] == 0;
            px |= update.index[0] == MAX_AXIS_INDEX;
            ny |= update.index[1] == 0;
            py |= update.index[1] == MAX_AXIS_INDEX;
            nz |= update.index[2] == 0;
            pz |= update.index[2] == MAX_AXIS_INDEX;
            *slot = update.id;
        }
    }

    let pos = ctx.chunk.section(y);
    if c {
        ctx.rebuild.insert(pos);
    }

    if nx {
        ctx.rebuild.insert(pos.offset([-1, 0, 0]));
    }
    if px {
        ctx.rebuild.insert(pos.offset([1, 0, 0]));
    }

    if ny {
        ctx.rebuild.insert(pos.offset([0, -1, 0]));
    }
    if py {
        ctx.rebuild.insert(pos.offset([0, 1, 0]));
    }

    if nz {
        ctx.rebuild.insert(pos.offset([0, 0, -1]));
    }
    if pz {
        ctx.rebuild.insert(pos.offset([0, 0, 1]));
    }
}

fn write_chunk_updates(
    data: &mut ChunkData<BlockId>,
    ctx: &mut ChunkUpdateContext,
    y: i32,
    updates: &[ChunkSectionUpdate],
) {
    match data {
        &mut ChunkData::Homogeneous(id) => {
            let differing = match updates.iter().position(|update| update.id != id) {
                Some(pos) => pos,
                None => return,
            };

            let mut chunk = ArrayChunk::homogeneous(id);
            write_chunk_updates_array(&mut chunk, ctx, y, &updates[differing..]);

            *data = ChunkData::Array(chunk);
        }
        ChunkData::Array(data) => write_chunk_updates_array(data, ctx, y, updates),
    }
}

// the idea here is to queue writes to this chunk, and flush the queue once a
// frame when there are modifications, and importantly, orphaning any current
// readers to prevent race conditions when we go to write our updates to actual
// chunk data.
//
// NOTE: this should not be called concurrently, the only guarantee is
// concurrent calls will not produce UB.
fn flush_chunk_section_writes(
    chunk: &ChunkSection,
    updates: &[ChunkSectionUpdate],
    access: &mut MutableChunkAccess,
    ctx: &mut ChunkUpdateContext,
) {
    assert!(!updates.is_empty());

    chunk.was_ever_modified.store(true, AtomicOrdering::Relaxed);

    let registry = Arc::clone(access.registry());

    {
        let inner = access.section(chunk.pos()).unwrap();
        write_chunk_updates(inner.blocks_mut(), ctx, chunk.pos().y, updates);

        #[cfg(feature = "debug")]
        match inner.was_cloned() {
            true => send_debug_event(super::debug::WorldAccessEvent::Orphaned(chunk.pos())),
            false => send_debug_event(super::debug::WorldAccessEvent::Written(chunk.pos())),
        }
    }

    ctx.light_queues.queue_blocklight_updates(
        access,
        updates.iter().map(|update| {
            let [x, y, z] = update.index;
            let pos = BlockPos {
                x: chunk.pos().x * CHUNK_LENGTH as i32 + x as i32,
                y: chunk.pos().y * CHUNK_LENGTH as i32 + y as i32,
                z: chunk.pos().z * CHUNK_LENGTH as i32 + z as i32,
            };

            let light = registry.block_light(update.id);

            (pos, light)
        }),
    );
}

fn flush_chunk_writes(
    chunk: &Chunk,
    updates: &HashMap<i32, Vec<ChunkSectionUpdate>>,
    access: &mut MutableChunkAccess,
    rebuild: &mut HashSet<ChunkSectionPos>,
) {
    assert!(!updates.is_empty());

    let mut solid_updates = HashMap::new();
    let mut light_queues = LightUpdateQueues::default();

    let registry = Arc::clone(access.registry());
    let mut ctx = ChunkUpdateContext {
        rebuild,
        solid_updates: &mut solid_updates,
        light_queues: &mut light_queues,
        registry: &registry,
        chunk: chunk.pos(),
    };

    for (&y, updates) in updates.iter() {
        match chunk.section(y) {
            Some(section) => flush_chunk_section_writes(&section, updates, access, &mut ctx),
            None => todo!("update of unloaded section"),
        }
    }

    let mut sky_nodes = chunk.sky_light.orphan_readers();
    for (&[x, z], updates) in ctx.solid_updates.iter() {
        let prev_top = sky_nodes.node(x, z).top();
        for (&y, &solid) in updates.iter() {
            sky_nodes.node_mut(x, z).update(y, solid);
        }
        let new_top = sky_nodes.node(x, z).top();

        if prev_top != new_top {
            let x = CHUNK_LENGTH as i32 * ctx.chunk.x + x as i32;
            let z = CHUNK_LENGTH as i32 * ctx.chunk.z + z as i32;
            // log::info!("prev top = {prev_top}, new_top = {new_top}");
            let min_y = i32::min(prev_top, new_top);
            let max_y = i32::max(prev_top, new_top);
            let light = match new_top > prev_top {
                true => 0,
                false => 15,
            };
            ctx.light_queues
                .queue_skylight_updates(access, x, z, min_y, max_y, light);
        }
    }

    propagate_block_light(&mut ctx.light_queues, access);
    propagate_sky_light(&mut ctx.light_queues, access);
}

// TODO: maybe think about splitting this into a read half and a write half, so
// writers can operate in parallel with readers.
/// a cache for multiple unaligned world accesses over a short period of time.
pub struct ChunkAccess {
    pub world: Arc<VoxelWorld>,
    sections: HashMap<ChunkSectionPos, ChunkSectionSnapshot>,

    free_update_queues: Vec<Vec<ChunkSectionUpdate>>,
    chunk_updates: HashMap<ChunkPos, HashMap<i32, Vec<ChunkSectionUpdate>>>,
}

impl ChunkAccess {
    pub fn new(world: &Arc<VoxelWorld>) -> Self {
        Self {
            world: Arc::clone(world),
            sections: Default::default(),
            free_update_queues: Default::default(),
            chunk_updates: Default::default(),
        }
    }

    pub fn registry(&self) -> &Arc<BlockRegistry> {
        &self.world.registry
    }

    pub fn section(&mut self, pos: ChunkSectionPos) -> Option<&ChunkSectionSnapshot> {
        Some(match self.sections.entry(pos) {
            Entry::Occupied(entry) => &*entry.into_mut(),
            Entry::Vacant(entry) => &*entry.insert(self.world.section(pos)?.snapshot()),
        })
    }

    #[must_use]
    pub fn block(&mut self, pos: BlockPos) -> Option<BlockId> {
        let (section_pos, chunk_index) = pos.section_and_offset();
        Some(self.section(section_pos)?.blocks().get(chunk_index))
    }

    #[must_use]
    pub fn light(&mut self, pos: BlockPos) -> Option<LightValue> {
        let (section_pos, chunk_index) = pos.section_and_offset();
        Some(self.section(section_pos)?.light().get(chunk_index))
    }

    // TODO: what do we do about updates of chunk sections that don't exist in the
    // world??
    pub fn set_block(&mut self, pos: BlockPos, id: BlockId) {
        let (section_pos, chunk_index) = pos.section_and_offset();
        let queue = self
            .chunk_updates
            .entry(section_pos.column())
            .or_default()
            .entry(section_pos.y)
            .or_insert_with(|| self.free_update_queues.pop().unwrap_or_default());
        queue.push(ChunkSectionUpdate {
            index: chunk_index,
            id,
        });
    }
}

pub struct MutableChunkAccess {
    rebuild: HashSet<ChunkSectionPos>,
    world: Arc<VoxelWorld>,
    writers: HashMap<ChunkSectionPos, ChunkSectionSnapshotMut>,
}

impl MutableChunkAccess {
    fn new(world: &Arc<VoxelWorld>) -> Self {
        Self {
            world: Arc::clone(world),
            writers: Default::default(),
            rebuild: Default::default(),
        }
    }

    pub fn registry(&self) -> &Arc<BlockRegistry> {
        &self.world.registry
    }

    fn section(&mut self, pos: ChunkSectionPos) -> Option<&mut ChunkSectionSnapshotMut> {
        Some(match self.writers.entry(pos) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(ChunkSectionSnapshotMut::new(
                self.world.section(pos)?.inner.orphan_readers(),
            )),
        })
    }

    #[must_use]
    pub fn block(&mut self, pos: BlockPos) -> Option<BlockId> {
        let (section_pos, chunk_index) = pos.section_and_offset();
        Some(self.section(section_pos)?.blocks().get(chunk_index))
    }

    #[must_use]
    pub fn set_block(&mut self, pos: BlockPos, id: BlockId) -> Option<()> {
        let (section_pos, chunk_index) = pos.section_and_offset();
        let block_data = self.section(section_pos)?.blocks_mut();

        let prev = block_data.get(chunk_index);
        if id != prev {
            block_data.set(chunk_index, id);
            self.rebuild.insert(pos.into());
        }
        Some(())
    }

    #[must_use]
    pub fn light(&mut self, pos: BlockPos) -> Option<LightValue> {
        let (section_pos, chunk_index) = pos.section_and_offset();
        Some(self.section(section_pos)?.light().get(chunk_index))
    }

    #[must_use]
    pub fn set_sky_light(&mut self, pos: BlockPos, light: u16) -> Option<()> {
        let (section_pos, chunk_index) = pos.section_and_offset();
        let light_data = self.section(section_pos)?.light_mut();

        let prev = light_data.get(chunk_index);
        if light != prev.sky() {
            light_data.set(chunk_index, LightValue::pack(light, prev.block()));
            self.rebuild.insert(pos.into());
        }

        Some(())
    }

    #[must_use]
    pub fn set_block_light(&mut self, pos: BlockPos, light: u16) -> Option<()> {
        let (section_pos, chunk_index) = pos.section_and_offset();
        let light_data = self.section(section_pos)?.light_mut();

        let prev = light_data.get(chunk_index);
        if light != prev.block() {
            light_data.set(chunk_index, LightValue::pack(prev.sky(), light));
            self.rebuild.insert(pos.into());
        }

        Some(())
    }
}

pub(crate) fn flush_chunk_access(access: &mut ChunkAccess, rebuild: &mut HashSet<ChunkSectionPos>) {
    #[cfg(feature = "debug")]
    for &pos in access.sections.keys() {
        send_debug_event(super::debug::WorldAccessEvent::Read(pos));
    }

    // let go of our snapshots before we flush chunk updates so that we don't force
    // an orphan of every updated chunk because we still have readers here.
    access.sections.clear();

    let mut mut_access = MutableChunkAccess::new(&access.world);

    for (pos, mut updates) in access.chunk_updates.drain() {
        match access.world.chunk(pos) {
            Some(chunk) => flush_chunk_writes(&chunk, &updates, &mut mut_access, rebuild),
            None => todo!("unloaded chunk write"),
        }

        // recycle old queues to amortize allocation cost
        updates.values_mut().for_each(Vec::clear);
        access.free_update_queues.extend(updates.into_values());
    }

    rebuild.extend(mut_access.rebuild);
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ChunkData<T> {
    Homogeneous(T),
    Array(ArrayChunk<T>),
}

impl<T: Copy + Eq> ChunkData<T> {
    pub fn get(&self, index: ChunkSectionIndex) -> T {
        match self {
            &ChunkData::Homogeneous(value) => value,
            ChunkData::Array(data) => data[index],
        }
    }

    pub fn set(&mut self, index: ChunkSectionIndex, new_value: T) {
        match self {
            &mut ChunkData::Homogeneous(value) if value == new_value => {}
            &mut ChunkData::Homogeneous(value) => {
                let mut array_chunk = ArrayChunk::homogeneous(value);
                array_chunk[index] = new_value;
                *self = ChunkData::Array(array_chunk);
            }
            ChunkData::Array(data) => data[index] = new_value,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ArrayChunk<T> {
    // data order is XZY
    data: Box<[T]>,
}

impl<T: Copy> ArrayChunk<T> {
    pub fn homogeneous(id: T) -> Self {
        Self {
            data: vec![id; CHUNK_VOLUME].into_boxed_slice(),
        }
    }
}

pub fn is_in_chunk_bounds(x: usize, y: usize, z: usize) -> bool {
    x < CHUNK_LENGTH && y < CHUNK_LENGTH && z < CHUNK_LENGTH
}

impl<T, I: Into<[usize; 3]>> Index<I> for ArrayChunk<T> {
    type Output = T;

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

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct LocalChunkPos {
    pub xz: ChunkIndex,
    pub y: i32,
}

pub type ChunkIndex = [usize; 2];
pub type ChunkSectionIndex = [usize; 3];

impl<T, I: Into<[usize; 3]>> IndexMut<I> for ArrayChunk<T> {
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
pub struct ChunkSectionPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl ChunkSectionPos {
    pub fn column(&self) -> ChunkPos {
        ChunkPos {
            x: self.x,
            z: self.z,
        }
    }
}

impl From<ChunkSectionPos> for Point3<i32> {
    fn from(ChunkSectionPos { x, y, z }: ChunkSectionPos) -> Self {
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

impl<T> TryFrom<Box<[T]>> for ArrayChunk<T> {
    type Error = ChunkTryFromError;

    fn try_from(data: Box<[T]>) -> Result<Self, Self::Error> {
        if data.len() != CHUNK_VOLUME {
            return Err(ChunkTryFromError {
                provided_size: data.len(),
                expected_size: CHUNK_VOLUME,
            });
        }

        Ok(ArrayChunk { data })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct CompactedChunkSection {
    runs: Vec<(usize, BlockId)>,
}

impl CompactedChunkSection {
    pub fn compact(data: &ChunkData<BlockId>) -> Self {
        match data {
            &ChunkData::Homogeneous(id) => Self {
                runs: vec![(1, id)],
            },
            ChunkData::Array(ArrayChunk { data }) => {
                let mut current_run = 1;
                let mut current_id = data[0];

                let mut runs = vec![];
                for id in data.iter().skip(1).copied() {
                    if current_id != id {
                        runs.push((current_run, current_id));
                        current_run = 1;
                        current_id = id;
                    } else {
                        current_run += 1;
                    }
                }
                runs.push((current_run, current_id));

                Self { runs }
            }
        }
    }

    pub fn decompact(&self) -> ChunkData<BlockId> {
        match self.runs.len() {
            1 => ChunkData::Homogeneous(self.runs[0].1),
            _ => ChunkData::Array({
                let mut res = Vec::with_capacity(CHUNK_VOLUME);
                for &(run_len, id) in self.runs.iter() {
                    res.extend(std::iter::repeat(id).take(run_len));
                }
                assert!(res.len() == CHUNK_VOLUME);
                ArrayChunk::try_from(res.into_boxed_slice()).unwrap()
            }),
        }
    }
}
