use core::sync::atomic::{AtomicU16, Ordering};

/// A semaphore
///
/// This object provides a lock that can be "locked" multiple times.
pub struct Semaphore {
    lock: AtomicU16,
    max: Option<u16>,
}

#[must_use = "if unused, the lock will release automatically"]
pub struct SemaphoreGuard<'a> {
    lock: &'a AtomicU16,
}

impl Drop for SemaphoreGuard<'_> {
    fn drop(&mut self) {
        self.lock.fetch_sub(1, Ordering::Release);
    }
}

impl Semaphore {
    pub const fn new() -> Self {
        Self { lock: AtomicU16::new(0), max: None }
    }

    pub const fn new_with_max(max: u16) -> Self {
        Self { lock: AtomicU16::new(0), max: Some(max) }
    }

    pub fn try_lock(&self) -> Option<SemaphoreGuard<'_>> {
        while self.lock.fetch_update(Ordering::Acquire, Ordering::Relaxed, |n| {
            if self.max.is_some_and(|max| n >= max) {
                None
            } else {
                Some(n + 1)
            }
        })
        .is_err() { }
        Some(SemaphoreGuard {
            lock: &self.lock,
        })
    }

    pub fn lock(&self) -> SemaphoreGuard<'_> {
        loop {
            if let Some(guard) = self.try_lock() {
                return guard
            }
            core::hint::spin_loop();
        }
    }
}

impl Default for Semaphore {
    fn default() -> Self {
        Self::new()
    }
}
