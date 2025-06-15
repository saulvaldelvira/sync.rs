//! A spinlock, the core primitive of this crate

use core::sync::atomic::{AtomicBool, Ordering};

/// A spin lock synchronization primitive.
///
/// This object is used to create other syncronized data
/// structures, like [Mutex](crate::mutex::Mutex)
pub struct SpinLock {
    lock: AtomicBool,
}

/// A guard for the [SpinLock]
///
/// This structure represents a borrow of the spinlock.
/// When droped, the [SpinLock] is marked as available again
#[must_use = "if unused, the lock will release automatically"]
pub struct SpinLockGuard<'a> {
    lock: &'a AtomicBool,
}

unsafe impl Send for SpinLockGuard<'_> {}
unsafe impl Sync for SpinLockGuard<'_> {}

impl Drop for SpinLockGuard<'_> {
    fn drop(&mut self) {
        self.lock.store(false, Ordering::Release);
    }
}

impl SpinLock {

    /// Creates a new `SpinLock`
    pub const fn new() -> Self {
        Self { lock: AtomicBool::new(false) }
    }

    /// If the lock is available, returns a [SpinLockGuard] inside a [Some] variant.
    /// If it's borrowed, which whould've caused the thread to wait for the previous
    /// guard to drop, returns inmediately with a [None] variant.
    pub fn try_lock(&self) -> Option<SpinLockGuard<'_>> {
        if self.lock
                  .compare_exchange(
                      false,
                      true,
                      Ordering::Acquire,
                      Ordering::Relaxed
                  ).is_ok()
        {
            Some(SpinLockGuard {
                lock: &self.lock,
            })
        } else {
            None
        }
    }

    /// Waits for the lock to be available, and inmediately
    /// borrows it, returning a [SpinLockGuard]
    ///
    /// This function locks the thread until the lock is available.
    /// For a non-blocking alternative, see [try_lock](Self::try_lock)
    pub fn lock(&self) -> SpinLockGuard<'_> {
        loop {
            if let Some(guard) = self.try_lock() {
                return guard
            }
            core::hint::spin_loop();
        }
    }

    /// Returns true if the lock is currently borrowed
    pub fn is_locked(&self) -> bool {
        self.lock.load(Ordering::Relaxed)
    }
}

impl Default for SpinLock {
    fn default() -> Self {
        Self::new()
    }
}
