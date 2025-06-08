//! A read-write lock

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

use crate::Mutex;

struct State {
    writing: bool,
    read_count: u32,
}

/// A read-write lock
///
/// This lock allows an arbitrary amount of readers, or a single writer
/// at the same time.
///
/// It provides a thread-safe equivalent of rust reference semantics.
/// If you lock for writing ([RwLock::write]), it will lock until no guard
/// is holding a read or write lock on this object.
///
/// If you lock for reading ([RwLock::read]), it will lock until the current
/// write-lock (if existing) drops.
///
/// Unlike [Mutex], this lock allows multiple read locks at the same time. It's
/// more convenient for uses where writing is less common, and you want a way to
/// read from multiple sources without the overhead of constantly locking and unlocking.
pub struct RwLock<T> {
    elem: UnsafeCell<T>,
    state: Mutex<State>,
}

unsafe impl<T> Sync for RwLock<T> {}
unsafe impl<T> Send for RwLock<T> {}

/// A read guard for a [RwLock].
/// This object is created by [RwLock::read]. It provides
/// read-only access to the inner value
pub struct RwLockReadGuard<'a, T> {
    elem: NonNull<T>,
    guard: &'a Mutex<State>,
}

unsafe impl<T> Sync for RwLockReadGuard<'_, T> {}
unsafe impl<T> Send for RwLockReadGuard<'_, T> {}

impl<T> Drop for RwLockReadGuard<'_, T> {
    fn drop(&mut self) {
        self.guard.lock().read_count -= 1;
    }
}

impl<T> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.elem.as_ref() }
    }
}

/// A write guard for a [RwLock].
/// This object is created by [RwLock::write]
///
/// It provides unique (thus mutable) access to the inner object.
pub struct RwLockWriteGuard<'a, T> {
    elem: NonNull<T>,
    guard: &'a Mutex<State>,
}

unsafe impl<T> Sync for RwLockWriteGuard<'_, T> {}
unsafe impl<T> Send for RwLockWriteGuard<'_, T> {}

impl<T> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.elem.as_ref() }
    }
}

impl<T> DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.elem.as_mut() }
    }
}

impl<T> Drop for RwLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        self.guard.lock().writing = false;
    }
}

/// An error for the [RwLock]
pub enum RwLockError {
    /// Attempt to get a write lock, while it was
    /// already borrowed for writes
    WriteWhileWrite,
    /// Attempt to get a write lock while holding a
    /// read access guard
    WriteWhileRead,
    /// Attempt to get a read lock while holding
    /// a write guard
    ReadWhileWrite,
}

pub type RwLockResult<T> = Result<T, RwLockError>;

impl<T> RwLock<T> {

    /// Creates a new `RwLock` from the given element
    pub const fn new(elem: T) -> Self {
        Self {
            elem: UnsafeCell::new(elem),
            state: Mutex::new(State {
                writing: false,
                read_count: 0
            })
        }
    }

    /// Locks `self` for reading.
    ///
    /// The returned [RwLockReadGuard] grants read-only access to the data
    ///
    /// # Errors
    /// If `self` is currently locked for writing
    pub fn read(&self) -> RwLockResult<RwLockReadGuard<'_, T>> {
        let mut state = self.state.lock();
        (!state.writing).then(|| {
            state.read_count += 1;
            RwLockReadGuard {
                guard: &self.state,
                elem: unsafe { NonNull::new_unchecked(self.elem.get() ) }
            }
        }).ok_or(RwLockError::ReadWhileWrite)
    }

    /// Locks `self` for writing.
    ///
    /// The returned [RwLockWriteGuard] grants write access to the data
    ///
    /// # Errors
    /// If `self` is currently locked by any other lock
    pub fn write(&self) -> RwLockResult<RwLockWriteGuard<'_, T>> {
        let mut state = self.state.lock();
        if state.writing {
            Err(RwLockError::WriteWhileWrite)
        } else if state.read_count > 0 {
            Err(RwLockError::WriteWhileRead)
        } else {
            state.writing = true;
            Ok(RwLockWriteGuard {
                guard: &self.state,
                elem: unsafe { NonNull::new_unchecked(self.elem.get() ) }
            })
        }
    }
}
