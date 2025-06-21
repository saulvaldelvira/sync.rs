use core::sync::atomic::{AtomicU16, Ordering};

/// A semaphore
///
/// # Example
/// ```no_run
/// use syncrs::Semaphore;
/// use std::{sync::Arc, thread, time::Duration};
///
/// let semaphore = Arc::new(Semaphore::new());
///
/// let sem1 = Arc::clone(&semaphore);
/// let wait_t = thread::spawn(move || {
///     for _ in 0..5 {
///         sem1.dec(1);
///         println!("Thread 1 decremented");
///     }
/// });
///
/// let sem2 = Arc::clone(&semaphore);
/// let writer_t = thread::spawn(move || {
///     println!("Thread 2 increments by 1");
///     sem2.inc(1);
///     thread::sleep(Duration::from_millis(100));
///
///     println!("Thread 2 increments by 2");
///     sem2.inc(2);
///     thread::sleep(Duration::from_millis(100));
///
///     println!("Thread 2 increments by 1");
///     sem2.inc(1);
///     thread::sleep(Duration::from_millis(100));
///
///     println!("Thread 2 increments by 2");
///     sem2.inc(2);
///     thread::sleep(Duration::from_millis(100));
/// });
///
/// wait_t.join().unwrap();
/// writer_t.join().unwrap();
///
/// assert_eq!(semaphore.get_value(), 1);
/// ```
pub struct Semaphore {
    lock: AtomicU16,
}

impl Semaphore {

    /// Creates a new semaphore. It's default value is 0
    pub const fn new() -> Self {
        Self { lock: AtomicU16::new(0) }
    }

    /// Creates a new semaphore with an initial `value`
    pub const fn with_initial_value(value: u16) -> Self {
        Self { lock: AtomicU16::new(value) }
    }

    /// Attempts to decrement the semaphore.
    /// If the value is 0, returns [None]
    pub fn try_dec(&self, n: u16) -> Option<()> {
        while self.lock.fetch_update(Ordering::Acquire, Ordering::Relaxed, |old| {
            if old >= n {
                Some(old - n)
            } else {
                None
            }
        })
        .is_err() { }
        Some(())
    }

    /// Decrements the semaphore.
    /// If the value is 0, locks until the value is > 0
    pub fn dec(&self, n: u16) {
        loop {
            if let Some(guard) = self.try_dec(n) {
                return guard
            }
            core::hint::spin_loop();
        }
    }

    /// Increments the semaphore
    pub fn inc(&self, n: u16) {
        self.lock.fetch_add(n, Ordering::Release);
    }

    /// Returns the state of the counter
    pub fn get_value(&self) -> u16 {
        self.lock.load(Ordering::Relaxed)
    }
}

impl Default for Semaphore {
    fn default() -> Self {
        Self::new()
    }
}
