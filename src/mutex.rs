//! A mutex, providing unique syncronized access to a value

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

use crate::spin::{SpinLock, SpinLockGuard};

/// A mutex.
pub struct Mutex<T> {
    lock: SpinLock,
    data: UnsafeCell<T>,
}

unsafe impl<T> Sync for Mutex<T> {}
unsafe impl<T> Send for Mutex<T> {}

pub struct MutexGuard<'a, T> {
    _lock: SpinLockGuard<'a>,
    data: NonNull<T>,
}

unsafe impl<T> Sync for MutexGuard<'_, T> {}
unsafe impl<T> Send for MutexGuard<'_, T> {}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.data.as_ptr() }
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.data.as_ptr() }
    }
}

impl<T> Mutex<T> {

    /// Creates a new `Mutex` from the given value
    pub const fn new(val: T) -> Self {
        Self {
            lock: SpinLock::new(),
            data: UnsafeCell::new(val),
        }
    }

    /// Returns a [lock guard](MutexGuard) for `self`, if it is
    /// available.
    /// If the operation whould've locked the execution, returns
    /// [None] inmediately
    ///
    /// The lock is held until the `MutexGuard` is dropped.
    pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        self.lock.try_lock().map(|guard| {
            unsafe {
                let data = NonNull::new_unchecked( self.data.get() );
                MutexGuard { _lock: guard, data }
            }
        })
    }

    /// Locks the mutex until it's available, and returns
    /// a [guard](MutexGuard) to the object.
    ///
    /// The lock is held until the `MutexGuard` is dropped.
    pub fn lock(&self) -> MutexGuard<'_, T> {
        let guard = self.lock.lock();
        unsafe {
            let data = NonNull::new_unchecked( self.data.get() );
            MutexGuard { _lock: guard, data }
        }
    }

    /// Gets a mutable reference to the object.
    ///
    /// # Safety
    /// Since this function gets a mutable `self` reference,
    /// we know the mutex is available.
    /// The reference is statically guaranteed to be unique, so
    /// we need not to worry about synchronization.
    ///
    /// This is faster that spinlocking, and is 100% safe.
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }
}

impl<T: Default> Default for Mutex<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}
