use crate::{
    debug::send_debug_event,
    world::{
        lighting::{propagate_block_light, LightUpdateQueues},
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
pub struct LocalBlockUpdate {
    pub index: ChunkIndex,
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

fn write_chunk_updates_array(
    data: &mut ArrayChunk<BlockId>,
    center: ChunkSectionPos,
    rebuild: &mut HashSet<ChunkSectionPos>,
    updates: &[LocalBlockUpdate],
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

    if c {
        rebuild.insert(center);
    }

    if nx {
        rebuild.insert(center.offset([-1, 0, 0]));
    }
    if px {
        rebuild.insert(center.offset([1, 0, 0]));
    }

    if ny {
        rebuild.insert(center.offset([0, -1, 0]));
    }
    if py {
        rebuild.insert(center.offset([0, 1, 0]));
    }

    if nz {
        rebuild.insert(center.offset([0, 0, -1]));
    }
    if pz {
        rebuild.insert(center.offset([0, 0, 1]));
    }
}

fn write_chunk_updates(
    data: &mut ChunkData<BlockId>,
    center: ChunkSectionPos,
    rebuild: &mut HashSet<ChunkSectionPos>,
    updates: &[LocalBlockUpdate],
) {
    match data {
        &mut ChunkData::Homogeneous(id) => {
            let differing = match updates.iter().position(|update| update.id != id) {
                Some(pos) => pos,
                None => return,
            };

            let mut chunk = ArrayChunk::homogeneous(id);
            write_chunk_updates_array(&mut chunk, center, rebuild, &updates[differing..]);

            *data = ChunkData::Array(chunk);
        }
        ChunkData::Array(data) => write_chunk_updates_array(data, center, rebuild, updates),
    }
}

// the idea here is to queue writes to this chunk, and flush the queue once a
// frame when there are modifications, and importantly, orphaning any current
// readers to prevent race conditions when we go to write our updates to actual
// chunk data.
//
// NOTE: this should not be called concurrently, the only guarantee is
// concurrent calls will not produce UB.
pub(crate) fn flush_chunk_writes(
    chunk: &ChunkSection,
    updates: &[LocalBlockUpdate],
    access: &mut MutableChunkAccess,
    rebuild: &mut HashSet<ChunkSectionPos>,
) {
    assert!(!updates.is_empty());

    chunk.was_ever_modified.store(true, AtomicOrdering::Relaxed);

    let registry = Arc::clone(access.registry());
    let mut light_updates = LightUpdateQueues::default();

    {
        let inner = access.chunk(chunk.pos()).unwrap();
        write_chunk_updates(inner.blocks_mut(), chunk.pos(), rebuild, updates);

        #[cfg(feature = "debug")]
        match inner.was_cloned() {
            true => send_debug_event(super::debug::WorldAccessEvent::Orphaned(chunk.pos())),
            false => send_debug_event(super::debug::WorldAccessEvent::Written(chunk.pos())),
        }
    }

    light_updates.queue_updates(
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

    propagate_block_light(&mut light_updates, access);
}

// TODO: maybe think about splitting this into a read half and a write half, so
// writers can operate in parallel with readers.
/// a cache for multiple unaligned world accesses over a short period of time.
pub struct ChunkAccess {
    pub world: Arc<VoxelWorld>,
    chunks: HashMap<ChunkSectionPos, ChunkSectionSnapshot>,

    free_update_queues: Vec<Vec<LocalBlockUpdate>>,
    update_queues: HashMap<ChunkSectionPos, Vec<LocalBlockUpdate>>,
}

impl ChunkAccess {
    pub fn new(world: &Arc<VoxelWorld>) -> Self {
        Self {
            world: Arc::clone(world),
            chunks: Default::default(),
            free_update_queues: Default::default(),
            update_queues: Default::default(),
        }
    }

    pub fn registry(&self) -> &Arc<BlockRegistry> {
        &self.world.registry
    }

    pub fn chunk(&mut self, pos: ChunkSectionPos) -> Option<&ChunkSectionSnapshot> {
        Some(match self.chunks.entry(pos) {
            Entry::Occupied(entry) => &*entry.into_mut(),
            Entry::Vacant(entry) => &*entry.insert(self.world.section(pos)?.snapshot()),
        })
    }

    #[must_use]
    pub fn block(&mut self, pos: BlockPos) -> Option<BlockId> {
        let (chunk_pos, chunk_index) = pos.chunk_and_offset();
        Some(self.chunk(chunk_pos)?.blocks().get(chunk_index))
    }

    #[must_use]
    pub fn light(&mut self, pos: BlockPos) -> Option<LightValue> {
        let (chunk_pos, chunk_index) = pos.chunk_and_offset();
        Some(self.chunk(chunk_pos)?.light().get(chunk_index))
    }

    // TODO: what do we do about updates of chunks that don't exist in the world??
    pub fn set_block(&mut self, pos: BlockPos, id: BlockId) {
        let (chunk_pos, chunk_index) = pos.chunk_and_offset();
        let queue = self
            .update_queues
            .entry(chunk_pos)
            .or_insert_with(|| self.free_update_queues.pop().unwrap_or_default());
        queue.push(LocalBlockUpdate {
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

    fn chunk(&mut self, pos: ChunkSectionPos) -> Option<&mut ChunkSectionSnapshotMut> {
        Some(match self.writers.entry(pos) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(ChunkSectionSnapshotMut::new(
                self.world.section(pos)?.inner.orphan_readers(),
            )),
        })
    }

    #[must_use]
    pub fn block(&mut self, pos: BlockPos) -> Option<BlockId> {
        let (chunk_pos, chunk_index) = pos.chunk_and_offset();
        Some(self.chunk(chunk_pos)?.blocks().get(chunk_index))
    }

    #[must_use]
    pub fn set_block(&mut self, pos: BlockPos, id: BlockId) -> Option<()> {
        let (chunk_pos, chunk_index) = pos.chunk_and_offset();
        let block_data = self.chunk(chunk_pos)?.blocks_mut();

        let prev = block_data.get(chunk_index);
        if id != prev {
            block_data.set(chunk_index, id);
            self.rebuild.insert(pos.into());
        }
        Some(())
    }

    #[must_use]
    pub fn light(&mut self, pos: BlockPos) -> Option<LightValue> {
        let (chunk_pos, chunk_index) = pos.chunk_and_offset();
        Some(self.chunk(chunk_pos)?.light().get(chunk_index))
    }

    #[must_use]
    pub fn set_block_light(&mut self, pos: BlockPos, light: u16) -> Option<()> {
        let (chunk_pos, chunk_index) = pos.chunk_and_offset();
        let light_data = self.chunk(chunk_pos)?.light_mut();

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
    for &pos in access.chunks.keys() {
        send_debug_event(super::debug::WorldAccessEvent::Read(pos));
    }

    // let go of our snapshots before we flush chunk updates so that we don't force
    // an orphan of every updated chunk because we still have readers here.
    access.chunks.clear();

    let mut mut_access = MutableChunkAccess::new(&access.world);

    for (pos, mut updates) in access.update_queues.drain() {
        match access.world.section(pos) {
            Some(chunk) => flush_chunk_writes(&chunk, &updates, &mut mut_access, rebuild),
            None => todo!("unloaded chunk write"),
        }

        // recycle old queues to amortize alloocation cost
        updates.clear();
        access.free_update_queues.push(updates);
    }

    rebuild.extend(mut_access.rebuild);
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ChunkData<T> {
    Homogeneous(T),
    Array(ArrayChunk<T>),
}

impl<T: Copy + Eq> ChunkData<T> {
    pub fn get(&self, index: ChunkIndex) -> T {
        match self {
            &ChunkData::Homogeneous(value) => value,
            ChunkData::Array(data) => data[index],
        }
    }

    pub fn set(&mut self, index: ChunkIndex, new_value: T) {
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

pub type ChunkIndex = [usize; 3];

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
