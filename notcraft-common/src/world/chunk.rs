use crate::world::registry::BlockId;
use crossbeam_channel::{Receiver, Sender};
use nalgebra::Point3;
use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    ops::{Index, IndexMut},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use super::{
    lighting::LightValue,
    orphan::{Orphan, OrphanSnapshot},
    BlockPos, VoxelWorld,
};

// The width of the chunk is `2 ^ SIZE_BITS`
pub const CHUNK_LENGTH_BITS: usize = 5;

pub const CHUNK_LENGTH: usize = 1 << CHUNK_LENGTH_BITS;
pub const CHUNK_AREA: usize = CHUNK_LENGTH * CHUNK_LENGTH;
pub const CHUNK_VOLUME: usize = CHUNK_LENGTH * CHUNK_LENGTH * CHUNK_LENGTH;

#[derive(Clone)]
struct ChunkInner {
    pos: ChunkPos,
    block_data: ChunkData<BlockId>,
    block_light_data: ChunkData<LightValue>,
}

impl ChunkInner {
    fn new(
        pos: ChunkPos,
        block_data: ChunkData<BlockId>,
        block_light_data: ChunkData<LightValue>,
    ) -> Self {
        Self {
            pos,
            block_data,
            block_light_data,
        }
    }
}

#[derive(Clone)]
pub struct ChunkSnapshot {
    inner: OrphanSnapshot<ChunkInner>,
}

impl ChunkSnapshot {
    fn new(inner: OrphanSnapshot<ChunkInner>) -> Self {
        Self { inner }
    }

    pub fn pos(&self) -> ChunkPos {
        self.inner.pos
    }

    pub fn blocks(&self) -> &ChunkData<BlockId> {
        &self.inner.block_data
    }

    pub fn block_light(&self) -> &ChunkData<LightValue> {
        &self.inner.block_light_data
    }

    /// See [`OrphanSnapshot::is_orphaned`]
    pub fn is_orphaned(&self) -> bool {
        self.inner.is_orphaned()
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ChunkUpdate {
    pub index: ChunkIndex,
    pub id: BlockId,
}

pub struct Chunk {
    pos: ChunkPos,
    inner: Orphan<ChunkInner>,
    was_ever_modified: AtomicBool,
    dirty: AtomicBool,
    dirty_sender: Sender<ChunkPos>,
    write_queue_tx: Sender<ChunkUpdate>,
    write_queue_rx: Receiver<ChunkUpdate>,
}

impl Chunk {
    pub fn new(
        dirty_sender: &Sender<ChunkPos>,
        pos: ChunkPos,
        block_data: ChunkData<BlockId>,
        block_light_data: ChunkData<LightValue>,
    ) -> Self {
        let (write_queue_tx, write_queue_rx) = crossbeam_channel::unbounded();
        let inner = Orphan::new(ChunkInner::new(pos, block_data, block_light_data));

        Self {
            pos,
            inner,
            was_ever_modified: AtomicBool::new(false),
            dirty: AtomicBool::new(false),
            dirty_sender: dirty_sender.clone(),
            write_queue_tx,
            write_queue_rx,
        }
    }

    pub fn pos(&self) -> ChunkPos {
        self.pos
    }

    pub fn snapshot(&self) -> ChunkSnapshot {
        ChunkSnapshot::new(self.inner.snapshot())
    }

    pub fn queue_write(&self, index: ChunkIndex, id: BlockId) {
        self.write_queue_tx.send(ChunkUpdate { index, id }).unwrap();

        if !self.dirty.swap(true, Ordering::Relaxed) {
            self.dirty_sender.send(self.pos).unwrap();
        }
    }

    pub(crate) fn was_ever_modified(&self) -> bool {
        self.was_ever_modified.load(Ordering::Relaxed)
    }
}

fn write_chunk_updates_array<I: Iterator<Item = ChunkUpdate>>(
    data: &mut ArrayChunk<BlockId>,
    center: ChunkPos,
    rebuild: &mut HashSet<ChunkPos>,
    updates: I,
) {
    const MAX_AXIS_INDEX: usize = CHUNK_LENGTH - 1;

    let mut c = false;
    let mut nx = false;
    let mut px = false;
    let mut ny = false;
    let mut py = false;
    let mut nz = false;
    let mut pz = false;

    updates.for_each(|update| {
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
    });

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

fn write_chunk_updates<I: Iterator<Item = ChunkUpdate>>(
    data: &mut ChunkData<BlockId>,
    center: ChunkPos,
    rebuild: &mut HashSet<ChunkPos>,
    mut updates: I,
) {
    match data {
        &mut ChunkData::Homogeneous(id) => {
            let differing = loop {
                match updates.next() {
                    None => return,
                    Some(update) if update.id == id => {}
                    Some(update) => break update,
                }
            };

            let mut chunk = ArrayChunk::homogeneous(id);
            write_chunk_updates_array(&mut chunk, center, rebuild, std::iter::once(differing));
            write_chunk_updates_array(&mut chunk, center, rebuild, updates);

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
//
// returns whether any modifications happened
pub(crate) fn flush_chunk_writes(chunk: &Chunk, rebuild: &mut HashSet<ChunkPos>) {
    // we clear the dirty flag here before processing anything in the queue, which
    // is okay. it just means that if we get updates before the queue is drained and
    // none after it is, then this chunk will be queued to be rechecked when there
    // are no pending updates.
    if !chunk.dirty.swap(false, Ordering::Relaxed) || chunk.write_queue_rx.is_empty() {
        return;
    }

    chunk.was_ever_modified.store(true, Ordering::Relaxed);

    // TODO: light propagation

    let mut inner = chunk.inner.orphan_readers();
    write_chunk_updates(
        &mut inner.block_data,
        chunk.pos(),
        rebuild,
        chunk.write_queue_rx.try_iter(),
    );
}

/// a cache for multiple unaligned world accesses over a short period of time.
pub struct ChunkSnapshotCache {
    pub world: Arc<VoxelWorld>,
    chunks: HashMap<ChunkPos, ChunkSnapshot>,
}

impl ChunkSnapshotCache {
    pub fn new(world: &Arc<VoxelWorld>) -> Self {
        Self {
            world: Arc::clone(world),
            chunks: Default::default(),
        }
    }

    pub fn chunk(&mut self, pos: ChunkPos) -> Option<&ChunkSnapshot> {
        Some(match self.chunks.entry(pos) {
            Entry::Occupied(entry) => &*entry.into_mut(),
            Entry::Vacant(entry) => &*entry.insert(self.world.chunk(pos)?.snapshot()),
        })
    }

    #[must_use]
    pub fn block(&mut self, pos: BlockPos) -> Option<BlockId> {
        let (chunk_pos, chunk_index) = pos.chunk_and_offset();
        Some(self.chunk(chunk_pos)?.blocks().get(chunk_index))
    }

    pub fn set_block(&self, pos: BlockPos, id: BlockId) -> Option<()> {
        let (chunk_pos, chunk_index) = pos.chunk_and_offset();
        self.world.chunk(chunk_pos)?.queue_write(chunk_index, id);
        Some(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ChunkData<T> {
    Homogeneous(T),
    Array(ArrayChunk<T>),
}

impl<T: Copy> ChunkData<T> {
    pub fn get(&self, index: ChunkIndex) -> T {
        match self {
            &ChunkData::Homogeneous(id) => id,
            ChunkData::Array(data) => data[index],
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
pub struct ChunkPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl From<ChunkPos> for Point3<i32> {
    fn from(ChunkPos { x, y, z }: ChunkPos) -> Self {
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
pub struct CompactedChunk {
    runs: Vec<(usize, BlockId)>,
}

impl CompactedChunk {
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
