use crate::{
    codec::{
        encode::{Encode, Encoder},
        NodeKind,
    },
    debug::send_debug_event,
    prelude::*,
    world::{
        lighting::{propagate_block_light, propagate_sky_light, LightUpdateQueues},
        registry::BlockId,
    },
};

use nalgebra::Point3;
use parking_lot::Mutex;
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
pub const CHUNK_LENGTH_2: usize = CHUNK_LENGTH * CHUNK_LENGTH;
pub const CHUNK_LENGTH_3: usize = CHUNK_LENGTH * CHUNK_LENGTH * CHUNK_LENGTH;

#[derive(Clone)]
pub struct ChunkSectionInner {
    pos: ChunkSectionPos,
    block_data: ChunkData<BlockId>,
    light_data: ChunkData<LightValue>,
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

pub struct Chunk {
    pos: ChunkPos,

    heights: Orphan<SurfaceHeightmap>,
    sky_light: Orphan<SkyLightColumns>,
    needs_persistence: AtomicBool,

    sections: Orphan<HashMap<i32, Arc<ChunkSection>>>,
    compacted_sections: flurry::HashMap<ChunkSectionPos, CompactedChunkSection>,
}

impl Chunk {
    pub fn initialize(pos: ChunkPos, heights: SurfaceHeightmap) -> Self {
        Self {
            pos,
            sky_light: Orphan::new(SkyLightColumns::initialize(&heights)),
            heights: Orphan::new(heights),
            needs_persistence: AtomicBool::new(false),
            sections: Default::default(),
            compacted_sections: Default::default(),
        }
    }

    pub fn new(pos: ChunkPos, heights: SurfaceHeightmap, sky_light: SkyLightColumns) -> Self {
        Self {
            pos,
            sky_light: Orphan::new(sky_light),
            heights: Orphan::new(heights),
            needs_persistence: AtomicBool::new(false),
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
        self.sections.snapshot().get(&y).map(Arc::clone)
    }

    pub fn sections(&self) -> OrphanSnapshot<HashMap<i32, Arc<ChunkSection>>> {
        self.sections.snapshot()
    }

    pub fn sections_mut(&self) -> OrphanWriter<HashMap<i32, Arc<ChunkSection>>> {
        self.sections.orphan_readers()
    }

    pub fn is_empty(&self) -> bool {
        self.sections.snapshot().is_empty()
    }

    pub fn is_loaded(&self, y: i32) -> bool {
        self.sections.snapshot().contains_key(&y)
    }

    pub fn needs_persistence(&self) -> bool {
        self.needs_persistence.load(AtomicOrdering::Relaxed)
    }
}

pub struct ChunkSection {
    pos: ChunkSectionPos,
    inner: Orphan<ChunkSectionInner>,
    needs_persistence: AtomicBool,

    updating: Mutex<()>,
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
    pub fn initialize(
        pos: ChunkSectionPos,
        block_data: ChunkData<BlockId>,
        registry: &BlockRegistry,
    ) -> Self {
        let light_data = match &block_data {
            &ChunkData::Homogeneous(id) => ChunkData::Homogeneous(default_light(registry, id)),
            ChunkData::Array(ids) => {
                let mut light = ArrayChunk::homogeneous(FULL_SKY_LIGHT);
                for (i, &id) in ids.data.iter().enumerate() {
                    light.data[i] = default_light(registry, id);
                }
                ChunkData::Array(light)
            }
        };

        let inner = Orphan::new(ChunkSectionInner {
            pos,
            block_data,
            light_data,
        });

        Self {
            pos,
            inner,
            needs_persistence: AtomicBool::new(false),
            updating: Default::default(),
        }
    }

    pub fn new(
        pos: ChunkSectionPos,
        block_data: ChunkData<BlockId>,
        light_data: ChunkData<LightValue>,
    ) -> Self {
        let inner = Orphan::new(ChunkSectionInner {
            pos,
            block_data,
            light_data,
        });

        Self {
            pos,
            inner,
            needs_persistence: AtomicBool::new(false),
            updating: Default::default(),
        }
    }

    pub fn pos(&self) -> ChunkSectionPos {
        self.pos
    }

    pub fn snapshot(&self) -> ChunkSectionSnapshot {
        ChunkSectionSnapshot::new(self.inner.snapshot())
    }

    pub fn needs_persistence(&self) -> bool {
        self.needs_persistence.load(AtomicOrdering::Relaxed)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct BlockUpdate {
    pub old_id: BlockId,
    pub new_id: BlockId,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ChunkSectionUpdate {
    pub index: ChunkSectionIndex,
    pub id: BlockId,
}

struct ChunkUpdateContext<'a> {
    pub rebuild: &'a mut HashSet<ChunkSectionPos>,
    pub block_updates: &'a mut HashMap<BlockPos, BlockUpdate>,
    pub light_queues: &'a mut LightUpdateQueues,
    pub registry: &'a BlockRegistry,
    pub chunk: ChunkPos,
}

fn write_section_updates_array(
    data: &mut ArrayChunk<BlockId>,
    ctx: &mut ChunkUpdateContext,
    y: i32,
    updates: &[ChunkSectionUpdate],
) {
    const MAX_AXIS_INDEX: usize = CHUNK_LENGTH - 1;

    let pos = ctx.chunk.section(y);

    let mut c = false;
    let mut nx = false;
    let mut px = false;
    let mut ny = false;
    let mut py = false;
    let mut nz = false;
    let mut pz = false;

    for update in updates.iter() {
        let slot = &mut data[update.index];

        if *slot != update.id {
            ctx.block_updates
                .insert(index_to_block(pos, update.index), BlockUpdate {
                    old_id: *slot,
                    new_id: update.id,
                });
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

fn write_section_block_updates(
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
            write_section_updates_array(&mut chunk, ctx, y, &updates[differing..]);

            *data = ChunkData::Array(chunk);
        }
        ChunkData::Array(data) => write_section_updates_array(data, ctx, y, updates),
    }
}

// the idea here is to queue writes to this chunk, and flush the queue once a
// frame when there are modifications, and importantly, orphaning any current
// readers to prevent race conditions when we go to write our updates to actual
// chunk data.
fn write_chunk_updates_to_section(
    chunk: &ChunkSection,
    updates: &[ChunkSectionUpdate],
    access: &mut MutableChunkAccess,
    ctx: &mut ChunkUpdateContext,
) {
    assert!(!updates.is_empty());
    let _updating = chunk
        .updating
        .try_lock()
        .expect("chunk section update not exclusive");

    chunk.needs_persistence.store(true, AtomicOrdering::Relaxed);

    let registry = Arc::clone(access.registry());

    write_section_block_updates(
        access.section(chunk.pos()).unwrap().blocks_mut(),
        ctx,
        chunk.pos().y,
        updates,
    );

    #[cfg(feature = "debug")]
    match access.section(chunk.pos()).unwrap().was_cloned() {
        true => send_debug_event(super::debug::WorldAccessEvent::Orphaned(chunk.pos())),
        false => send_debug_event(super::debug::WorldAccessEvent::Written(chunk.pos())),
    }

    ctx.light_queues.queue_blocklight_updates(
        access,
        updates.iter().map(|update| {
            let pos = index_to_block(chunk.pos(), update.index);
            let light = registry.block_light(update.id);
            (pos, light)
        }),
    );
}

fn write_chunk_updates_to_chunk(
    chunk: &Chunk,
    updates: &HashMap<i32, Vec<ChunkSectionUpdate>>,
    access: &mut MutableChunkAccess,
    rebuild: &mut HashSet<ChunkSectionPos>,
    block_updates: &mut HashMap<BlockPos, BlockUpdate>,
) {
    assert!(!updates.is_empty());

    let mut light_queues = LightUpdateQueues::default();

    let registry = Arc::clone(access.registry());
    let mut ctx = ChunkUpdateContext {
        rebuild,
        block_updates,
        light_queues: &mut light_queues,
        registry: &registry,
        chunk: chunk.pos(),
    };

    for (&y, updates) in updates.iter() {
        match chunk.section(y) {
            Some(section) => write_chunk_updates_to_section(&section, updates, access, &mut ctx),
            None => todo!("update of unloaded section"),
        }
    }

    let mut updated_sky_columns: HashMap<[usize; 2], HashMap<i32, bool>> = HashMap::default();
    for (&pos, update) in ctx.block_updates.iter() {
        let old_solid = !ctx.registry.light_transmissible(update.old_id);
        let new_solid = !ctx.registry.light_transmissible(update.new_id);
        if old_solid != new_solid {
            let x = pos_to_index(pos.x);
            let z = pos_to_index(pos.z);
            updated_sky_columns
                .entry([x, z])
                .or_default()
                .insert(pos.y, new_solid);
        }
    }

    let mut sky_nodes = chunk.sky_light.orphan_readers();
    for (&[x, z], updates) in updated_sky_columns.iter() {
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

pub(crate) fn write_all_chunk_updates(
    access: &mut ChunkAccess,
    rebuild: &mut HashSet<ChunkSectionPos>,
    block_updates: &mut HashMap<BlockPos, BlockUpdate>,
) {
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
            Some(chunk) => write_chunk_updates_to_chunk(
                &chunk,
                &updates,
                &mut mut_access,
                rebuild,
                block_updates,
            ),
            None => todo!("unloaded chunk write"),
        }

        // recycle old queues to amortize allocation cost
        updates.values_mut().for_each(Vec::clear);
        access.free_update_queues.extend(updates.into_values());
    }

    rebuild.extend(mut_access.rebuild);
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
            data: vec![id; CHUNK_LENGTH_3].into_boxed_slice(),
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
            return &self.data[CHUNK_LENGTH_2 * x + CHUNK_LENGTH * z + y];
        }

        panic!(
            "chunk index out of bounds: the size is {} but the index is ({}, {}, {})",
            CHUNK_LENGTH, x, y, z
        )
    }
}

pub fn pos_to_index(pos: i32) -> usize {
    pos.rem_euclid(CHUNK_LENGTH as i32) as usize
}

pub fn index_to_pos(chunk_pos: i32, index: usize) -> i32 {
    chunk_pos * CHUNK_LENGTH as i32 + index as i32
}

pub fn block_to_index(pos: BlockPos) -> ChunkSectionIndex {
    [
        pos_to_index(pos.x),
        pos_to_index(pos.y),
        pos_to_index(pos.z),
    ]
}

pub fn index_to_block(section: ChunkSectionPos, index: ChunkSectionIndex) -> BlockPos {
    let [x, y, z] = index;
    BlockPos {
        x: index_to_pos(section.x, x),
        y: index_to_pos(section.y, y),
        z: index_to_pos(section.z, z),
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
            return &mut self.data[CHUNK_LENGTH_2 * x + CHUNK_LENGTH * z + y];
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
        if data.len() != CHUNK_LENGTH_3 {
            return Err(ChunkTryFromError {
                provided_size: data.len(),
                expected_size: CHUNK_LENGTH_3,
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
                let mut res = Vec::with_capacity(CHUNK_LENGTH_3);
                for &(run_len, id) in self.runs.iter() {
                    res.extend(std::iter::repeat(id).take(run_len));
                }
                assert!(res.len() == CHUNK_LENGTH_3);
                ArrayChunk::try_from(res.into_boxed_slice()).unwrap()
            }),
        }
    }
}

impl<T: Encode<W> + PartialEq, W: std::io::Write> Encode<W> for ChunkData<T> {
    const KIND: NodeKind = NodeKind::List;

    fn encode(&self, encoder: Encoder<W>) -> Result<()> {
        match self {
            ChunkData::Homogeneous(element) => {
                encoder.encode_rle_list_runs(std::iter::once((CHUNK_LENGTH_3, element)))
            }
            ChunkData::Array(ArrayChunk { data }) => encoder.encode_rle_list(data.iter()),
        }
    }
}

// run-length encoded. format is `(usize ~ T ~ !0usize) ~ 0usize`. that is, it's
// a sequence of (length, item) where length > 0, terminated by a 0.
// impl<T: Codec + Clone + PartialEq> Codec for ChunkData<T> {
//     fn decode<R: std::io::Read>(reader: &mut R) -> Result<Self> {
//         Ok(match usize::decode(reader)? {
//             // chunk data always has a length greater than 0, so the first
// length we encounter             // should never be ther terminator sentinel.
//             0 => bail!("chunk data had 0 runs"),

//             CHUNK_LENGTH_3 => {
//                 let element = T::decode(reader)?;
//                 match usize::decode(reader)? {
//                     0 => ChunkData::Homogeneous(element),
//                     _ => bail!("homogenous chunk data did not terminate after
// element"),                 }
//             }

//             len => ChunkData::Array({
//                 let mut res = Vec::with_capacity(CHUNK_LENGTH_3);

//                 let mut current_run_len = len;
//                 let mut current_run_element = T::decode(reader)?;

//                 while current_run_len != 0 {
//                     if current_run_len > res.capacity() - res.len() {
//                         bail!("chunk data had more than the maximum amount of
// elements");                     }
//
// res.extend(std::iter::repeat(current_run_element).take(current_run_len));
//                     current_run_len = usize::decode(reader)?;
//                     current_run_element = T::decode(reader)?;
//                 }

//                 if res.len() != CHUNK_LENGTH_3 {
//                     bail!(
//                         "chunk data did not decompress to enough elements:
// {}",                         res.len()
//                     );
//                 }

//                 ArrayChunk {
//                     data: res.into_boxed_slice(),
//                 }
//             }),
//         })
//     }
// }

impl<W: std::io::Write> Encode<W> for Chunk {
    const KIND: NodeKind = NodeKind::Map;

    fn encode(&self, encoder: Encoder<W>) -> Result<()> {
        encoder.encode_map(|mut encoder| {
            encoder.entry("pos").encode(&self.pos())?;
            encoder
                .entry("sky-light")
                .encode(&*self.sky_light.snapshot())?;
            // encoder
            //     .entry("sections")
            //     .encode_verbatim_list(self.sections().pin().iter())?;
            todo!()
        })
    }
}

impl<W: std::io::Write> Encode<W> for ChunkSection {
    const KIND: NodeKind = NodeKind::Map;

    fn encode(&self, encoder: Encoder<W>) -> Result<()> {
        let snapshot = self.snapshot();
        encoder.encode_map(|mut encoder| {
            encoder.entry("pos").encode(&snapshot.pos())?;
            encoder.entry("blocks").encode(&snapshot.blocks())?;
            encoder.entry("light").encode(&snapshot.light())?;
            todo!()
        })
    }
}

impl<W: std::io::Write> Encode<W> for ChunkPos {
    const KIND: NodeKind = NodeKind::Map;

    fn encode(&self, encoder: Encoder<W>) -> Result<()> {
        encoder.encode_map(|mut encoder| {
            encoder.entry("x").encode(&self.x)?;
            encoder.entry("z").encode(&self.z)?;
            Ok(())
        })
    }
}

impl<W: std::io::Write> Encode<W> for ChunkSectionPos {
    const KIND: NodeKind = NodeKind::Map;

    fn encode(&self, encoder: Encoder<W>) -> Result<()> {
        encoder.encode_map(|mut encoder| {
            encoder.entry("x").encode(&self.x)?;
            encoder.entry("y").encode(&self.y)?;
            encoder.entry("z").encode(&self.z)?;
            Ok(())
        })
    }
}
