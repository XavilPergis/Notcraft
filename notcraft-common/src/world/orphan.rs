use std::{
    cell::UnsafeCell,
    mem::ManuallyDrop,
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

#[derive(Debug)]
enum OrphanWriteGuardCow<'a, T> {
    Borrowed(&'a mut T),
    Cloned(ManuallyDrop<T>),
}

pub struct OrphanWriteGuard<'a, T> {
    orphan: &'a Orphan<T>,
    inner: Arc<OrphanInner<T>>,
    value: OrphanWriteGuardCow<'a, T>,
}

impl<'a, T> OrphanWriteGuard<'a, T> {
    pub fn was_cloned(&self) -> bool {
        matches!(&self.value, OrphanWriteGuardCow::Cloned(_))
    }
}

unsafe impl<'a, T: Send> Send for OrphanWriteGuard<'a, T> {}
unsafe impl<'a, T: Sync> Sync for OrphanWriteGuard<'a, T> {}

impl<'a, T: Clone> OrphanWriteGuard<'a, T> {
    pub fn acquire(orphan: &'a Orphan<T>, wait_shared: bool) -> Option<Self> {
        let current_inner = orphan.current_inner.load();
        if current_inner.lock.try_lock_exclusive() {
            let value = unsafe { &mut *current_inner.value.get() };
            Some(OrphanWriteGuard {
                orphan,
                inner: Guard::into_inner(current_inner),
                value: OrphanWriteGuardCow::Borrowed(value),
            })
        } else {
            match wait_shared {
                true => current_inner.lock.lock_shared(),
                false => current_inner.lock.try_lock_shared().then(|| ())?,
            }

            let value = unsafe { (*current_inner.value.get()).clone() };
            current_inner.orphaned.store(true, Ordering::Relaxed);
            Some(OrphanWriteGuard {
                orphan,
                inner: Guard::into_inner(current_inner),
                value: OrphanWriteGuardCow::Cloned(ManuallyDrop::new(value)),
            })
        }
    }
}

impl<'a, T> Drop for OrphanWriteGuard<'a, T> {
    fn drop(&mut self) {
        match &mut self.value {
            OrphanWriteGuardCow::Borrowed(_) => unsafe { self.inner.lock.unlock_exclusive() },
            OrphanWriteGuardCow::Cloned(value) => {
                unsafe { self.inner.lock.unlock_shared() };
                let value = unsafe { ManuallyDrop::take(value) };
                // we store the new inner here and not on guard acquisition because it allows
                // reads to not block while we are still writing.
                self.orphan
                    .current_inner
                    .store(Arc::new(OrphanInner::new(value)));
            }
        }
    }
}

impl<'a, T> std::ops::Deref for OrphanWriteGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match &self.value {
            OrphanWriteGuardCow::Borrowed(borrow) => borrow,
            OrphanWriteGuardCow::Cloned(owned) => owned,
        }
    }
}

impl<'a, T> std::ops::DerefMut for OrphanWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.value {
            OrphanWriteGuardCow::Borrowed(borrow) => borrow,
            OrphanWriteGuardCow::Cloned(owned) => owned,
        }
    }
}

pub struct Orphan<T> {
    current_inner: ArcSwap<OrphanInner<T>>,
}

unsafe impl<T: Send> Send for Orphan<T> {}
unsafe impl<T: Sync> Sync for Orphan<T> {}

impl<T: Clone> Orphan<T> {
    pub fn try_orphan_readers(&self) -> Option<OrphanWriteGuard<'_, T>> {
        OrphanWriteGuard::acquire(self, false)
    }

    pub fn orphan_readers(&self) -> OrphanWriteGuard<'_, T> {
        OrphanWriteGuard::acquire(self, true).unwrap()
    }

    pub fn try_snapshot(&self) -> Option<OrphanSnapshot<T>> {
        OrphanSnapshot::acquire(self, false)
    }

    pub fn snapshot(&self) -> OrphanSnapshot<T> {
        OrphanSnapshot::acquire(self, true).unwrap()
    }
}

impl<T> Orphan<T> {
    pub fn new(value: T) -> Self {
        Self {
            current_inner: ArcSwap::from_pointee(OrphanInner::new(value)),
        }
    }
}
