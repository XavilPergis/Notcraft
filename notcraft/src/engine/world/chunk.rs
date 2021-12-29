use crate::{engine::world::registry::BlockId, util};
use arc_swap::ArcSwap;
use crossbeam_channel::{Receiver, Sender};
use nalgebra::Point3;
use parking_lot::{lock_api::RawRwLock as RawRwLockApi, RawRwLock};
use std::{
    cell::UnsafeCell,
    collections::{hash_map::Entry, HashMap, HashSet},
    ops::{Index, IndexMut},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use super::{BlockPos, VoxelWorld};

// The width of the chunk is `2 ^ SIZE_BITS`
pub const CHUNK_LENGTH_BITS: usize = 5;

pub const CHUNK_LENGTH: usize = 1 << CHUNK_LENGTH_BITS;
pub const CHUNK_AREA: usize = CHUNK_LENGTH * CHUNK_LENGTH;
pub const CHUNK_VOLUME: usize = CHUNK_LENGTH * CHUNK_LENGTH * CHUNK_LENGTH;

#[derive(Clone)]
pub struct ChunkSnapshot {
    inner: Arc<ChunkInner>,
}

impl ChunkSnapshot {
    fn acquire(inner: Arc<ChunkInner>) -> Self {
        inner.lock.lock_shared();
        Self { inner }
    }

    pub fn pos(&self) -> ChunkPos {
        self.inner.pos
    }

    pub fn data(&self) -> &ChunkData {
        unsafe { &*self.inner.data.get() }
    }

    /// returns true if this chunk reader is known to be orphaned, and therefore
    /// not the most up-to-date version of the chunk data in this location. it
    /// is important to note that this _may_ return false even if the reader has
    /// been orphaned, and is meant more for coarse-grained optimizations.
    pub fn is_orphaned(&self) -> bool {
        self.inner.orphaned.load(Ordering::Relaxed)
    }
}

impl Drop for ChunkSnapshot {
    fn drop(&mut self) {
        unsafe { self.inner.lock.unlock_shared() };
    }
}

struct ChunkInner {
    pos: ChunkPos,
    lock: RawRwLock,
    data: UnsafeCell<ChunkData>,
    orphaned: AtomicBool,
}

// impl Drop for ChunkInner {
//     fn drop(&mut self) {
//         log::debug!(
//             "inner dropped! {} {} {}",
//             self.pos.x,
//             self.pos.y,
//             self.pos.z
//         );
//     }
// }

impl ChunkInner {
    pub fn new(pos: ChunkPos, data: ChunkData) -> Self {
        Self {
            pos,
            lock: RawRwLock::INIT,
            data: UnsafeCell::new(data),
            orphaned: AtomicBool::new(false),
        }
    }
}

unsafe impl Send for ChunkInner {}
unsafe impl Sync for ChunkInner {}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ChunkUpdate {
    pub index: ChunkIndex,
    pub id: BlockId,
}

pub struct Chunk {
    pos: ChunkPos,
    inner: ArcSwap<ChunkInner>,
    dirty: AtomicBool,
    dirty_sender: Sender<ChunkPos>,
    write_queue_tx: Sender<ChunkUpdate>,
    write_queue_rx: Receiver<ChunkUpdate>,
}

impl Chunk {
    pub fn new(dirty_sender: &Sender<ChunkPos>, pos: ChunkPos, kind: ChunkData) -> Self {
        let (write_queue_tx, write_queue_rx) = crossbeam_channel::unbounded();
        let inner = ArcSwap::from_pointee(ChunkInner::new(pos, kind));

        Self {
            pos,
            inner,
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
        ChunkSnapshot::acquire(self.inner.load_full())
    }

    pub fn queue_write(&self, index: ChunkIndex, id: BlockId) {
        self.write_queue_tx.send(ChunkUpdate { index, id }).unwrap();

        if !self.dirty.swap(true, Ordering::SeqCst) {
            self.dirty_sender.send(self.pos).unwrap();
        }
    }
}

fn write_chunk_updates_array<I: Iterator<Item = ChunkUpdate>>(
    data: &mut ArrayChunk,
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
    data: &mut ChunkData,
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
    if !chunk.dirty.swap(false, Ordering::SeqCst) || chunk.write_queue_rx.is_empty() {
        return;
    }

    let old_inner = chunk.inner.load();
    if old_inner.lock.try_lock_exclusive() {
        util::defer!(unsafe { old_inner.lock.unlock_exclusive() });

        let data = unsafe { &mut *old_inner.data.get() };
        write_chunk_updates(data, chunk.pos(), rebuild, chunk.write_queue_rx.try_iter())
    } else {
        log::debug!("flush failed, orphaning");
        old_inner.lock.lock_shared();
        util::defer!(unsafe { old_inner.lock.unlock_shared() });

        let mut chunk_data_copy = unsafe { (*old_inner.data.get()).clone() };
        write_chunk_updates(
            &mut chunk_data_copy,
            chunk.pos(),
            rebuild,
            chunk.write_queue_rx.try_iter(),
        );
        if rebuild.contains(&chunk.pos()) {
            chunk
                .inner
                .store(Arc::new(ChunkInner::new(chunk.pos, chunk_data_copy)));
            old_inner.orphaned.store(true, Ordering::SeqCst);
        }
    }
}

/// a cache for multiple unaligned world accesses over a short period of time.
pub struct ChunkSnapshotCache {
    world: Arc<VoxelWorld>,
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
        Some(self.chunk(chunk_pos)?.data().get(chunk_index))
    }

    pub fn set_block(&self, pos: BlockPos, id: BlockId) -> Option<()> {
        let (chunk_pos, chunk_index) = pos.chunk_and_offset();
        self.world.chunk(chunk_pos)?.queue_write(chunk_index, id);
        Some(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ChunkData {
    Homogeneous(BlockId),
    Array(ArrayChunk),
}

impl ChunkData {
    pub fn get(&self, index: ChunkIndex) -> BlockId {
        match self {
            &ChunkData::Homogeneous(id) => id,
            ChunkData::Array(data) => data[index],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ArrayChunk {
    // data order is XZY
    data: Box<[BlockId]>,
}

impl ArrayChunk {
    pub fn homogeneous(id: BlockId) -> Self {
        Self {
            data: vec![id; CHUNK_VOLUME].into_boxed_slice(),
        }
    }
}

pub fn is_in_chunk_bounds(x: usize, y: usize, z: usize) -> bool {
    x < CHUNK_LENGTH && y < CHUNK_LENGTH && z < CHUNK_LENGTH
}

impl<I: Into<[usize; 3]>> Index<I> for ArrayChunk {
    type Output = BlockId;

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

impl<I: Into<[usize; 3]>> IndexMut<I> for ArrayChunk {
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

impl TryFrom<Box<[BlockId]>> for ArrayChunk {
    type Error = ChunkTryFromError;

    fn try_from(data: Box<[BlockId]>) -> Result<Self, Self::Error> {
        if data.len() != CHUNK_VOLUME {
            return Err(ChunkTryFromError {
                provided_size: data.len(),
                expected_size: CHUNK_VOLUME,
            });
        }

        Ok(ArrayChunk { data })
    }
}
