use std::{
    cell::UnsafeCell,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use arc_swap::{ArcSwap, Guard};
use parking_lot::{lock_api::RawRwLock as RawRwLockApi, RawRwLock};

struct OrphanInner<T> {
    lock: RawRwLock,
    orphaned: AtomicBool,
    value: UnsafeCell<T>,
}

impl<T> OrphanInner<T> {
    fn new(value: T) -> Self {
        Self {
            lock: RawRwLock::INIT,
            orphaned: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }
}

impl<T: Default> Default for OrphanInner<T> {
    fn default() -> Self {
        Self {
            lock: RawRwLock::INIT,
            orphaned: AtomicBool::new(false),
            value: Default::default(),
        }
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for OrphanInner<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OrphanInner")
            .field("orphaned", &self.orphaned)
            .field("value", &self.value)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct OrphanSnapshot<T> {
    inner: Arc<OrphanInner<T>>,
}

unsafe impl<T: Send> Send for OrphanSnapshot<T> {}
unsafe impl<T: Sync> Sync for OrphanSnapshot<T> {}

impl<T> OrphanSnapshot<T> {
    pub fn acquire(orphan: &Orphan<T>, wait: bool) -> Option<Self> {
        let current_inner = orphan.current_inner.load();
        match wait {
            true => current_inner.lock.lock_shared(),
            false => current_inner.lock.try_lock_shared().then(|| ())?,
        }
        Some(Self {
            inner: Guard::into_inner(current_inner),
        })
    }

    /// Returns whether this snapshot has been fully orphaned.
    ///
    /// It is important to note that it is possible for this to return `false`
    /// and at the same time have [`Orphan::snapshot`] return a snapshot
    /// other than this one. However, if this returns `true`, then it is
    /// certain that this snapshot is not the most recent.
    pub fn is_orphaned(&self) -> bool {
        self.inner.orphaned.load(Ordering::Relaxed)
    }
}

impl<T> Drop for OrphanSnapshot<T> {
    fn drop(&mut self) {
        unsafe { self.inner.lock.unlock_shared() };
    }
}

impl<T> std::ops::Deref for OrphanSnapshot<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.inner.value.get() }
    }
}

pub struct OrphanWriter<T> {
    inner: Arc<OrphanInner<T>>,
    was_cloned: bool,
}

impl<T> OrphanWriter<T> {
    pub fn was_cloned(&self) -> bool {
        self.was_cloned
    }
}

unsafe impl<T: Send> Send for OrphanWriter<T> {}
unsafe impl<T: Sync> Sync for OrphanWriter<T> {}

impl<T: Clone> OrphanWriter<T> {
    pub fn acquire(orphan: &Orphan<T>, wait_shared: bool) -> Option<Self> {
        let current_inner = orphan.current_inner.load();
        if current_inner.lock.try_lock_exclusive() {
            Some(OrphanWriter {
                inner: Guard::into_inner(current_inner),
                was_cloned: false,
            })
        } else {
            match wait_shared {
                true => current_inner.lock.lock_shared(),
                false => current_inner.lock.try_lock_shared().then(|| ())?,
            }

            let value = unsafe { (*current_inner.value.get()).clone() };
            let inner = Arc::new(OrphanInner::new(value));
            inner.lock.lock_exclusive();

            orphan.current_inner.store(Arc::clone(&inner));
            current_inner.orphaned.store(true, Ordering::Relaxed);

            Some(OrphanWriter {
                inner,
                was_cloned: true,
            })
        }
    }
}

impl<T> Drop for OrphanWriter<T> {
    fn drop(&mut self) {
        unsafe { self.inner.lock.unlock_exclusive() };
    }
}

impl<T> std::ops::Deref for OrphanWriter<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.inner.value.get() }
    }
}

impl<T> std::ops::DerefMut for OrphanWriter<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.inner.value.get() }
    }
}

#[derive(Debug, Default)]
pub struct Orphan<T> {
    current_inner: ArcSwap<OrphanInner<T>>,
}

unsafe impl<T: Send> Send for Orphan<T> {}
unsafe impl<T: Sync> Sync for Orphan<T> {}

impl<T: Clone> Orphan<T> {
    pub fn try_orphan_readers(&self) -> Option<OrphanWriter<T>> {
        OrphanWriter::acquire(self, false)
    }

    pub fn orphan_readers(&self) -> OrphanWriter<T> {
        OrphanWriter::acquire(self, true).unwrap()
    }

    pub fn try_snapshot(&self) -> Option<OrphanSnapshot<T>> {
        OrphanSnapshot::acquire(self, false)
    }

    pub fn snapshot(&self) -> OrphanSnapshot<T> {
        OrphanSnapshot::acquire(self, true).unwrap()
    }

    pub fn clone_inner(&self) -> T {
        T::clone(&self.snapshot())
    }
}

impl<T> Orphan<T> {
    pub fn new(value: T) -> Self {
        Self {
            current_inner: ArcSwap::from_pointee(OrphanInner::new(value)),
        }
    }
}
