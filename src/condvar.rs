//! Conditional Variable
//!
//! [Conditional variables](Condvar) provides a way to block a thread
//! while waiting for an event to occur.
//!
//! They are associated with a [Mutex](crate::Mutex), and allows a
//! thread to [wait](Condvar::wait) until another thread notifies
//! a state change (via [`notify_one`] or [`notify_all`])
//!
//! # notify_one vs notify_all
//! The difference between [`notify_one`] and [`notify_all`] is that
//! the first one will only wake up a thread, while the second will
//! wake up all threads currently waiting on a `Condvar`.
//!
//! You may use one or the other depending on your intentions with
//! this Condvar.
//!
//! ## notify_one
//! Multiple threads can be waiting on a condvar, [`notify_one`] will
//! only wake one of them.
//!
//! Example:
//! ```no_run
//! use std::{thread, sync::Arc, time::Duration};
//! use syncrs::{Mutex, Condvar};
//!
//! let pair = Arc::new((Mutex::new(0), Condvar::new()));
//!
//! let pair1 = Arc::clone(&pair);
//! let reader_1 = thread::spawn(move || {
//!     let (n, cv) = &*pair1;
//!     let mut lock = n.lock();
//!     lock = cv.wait(lock);
//!     println!("Thread 1 woke up. Value = {}", *lock);
//! });
//!
//! let pair2 = Arc::clone(&pair);
//! let reader_2 = thread::spawn(move || {
//!     let (n, cv) = &*pair2;
//!     let mut lock = n.lock();
//!     lock = cv.wait(lock);
//!     println!("Thread 2 woke up. Value = {}", *lock);
//! });
//!
//! // Give some time for the readers to enter `wait`
//! thread::sleep(Duration::from_millis(100));
//!
//! let writer = thread::spawn(move || {
//!     let (n, cv) = &*pair;
//!     {
//!         let mut lock = n.lock();
//!         *lock += 1;
//!         // Wake up one thread
//!         cv.notify_one();
//!         println!("notify_one");
//!     }
//!
//!     {
//!         let mut lock = n.lock();
//!         *lock += 1;
//!         // Wake up the other thread
//!         cv.notify_one();
//!         println!("notify_one");
//!     }
//! });
//!
//! reader_1.join().unwrap();
//! reader_2.join().unwrap();
//! writer.join().unwrap();
//! ```
//!
//! The above program will print.
//! ```text
//! notify_one
//! Thread 1 woke up. Value = 1
//! notify_one
//! Thread 2 woke up. Value = 2
//! ```
//! Each [`notify_one`] wakes up one thread. The other won't wake up
//! until the second call to `notify_one`
//!
//! ## notify_all
//! [`notify_all`] wakes up all threads waiting for the Condvar
//!
//! The example above, but with `notify_all` would be:
//! ```no_run
//! use std::{thread, sync::Arc, time::Duration};
//! use syncrs::{Mutex, Condvar};
//!
//! let pair = Arc::new((Mutex::new(0), Condvar::new()));
//!
//! let pair1 = Arc::clone(&pair);
//! let reader_1 = thread::spawn(move || {
//!     let (n, cv) = &*pair1;
//!     let mut lock = n.lock();
//!     lock = cv.wait(lock);
//!
//!     println!("Thread 1 woke up. Value = {}", *lock);
//! });
//!
//! let pair2 = Arc::clone(&pair);
//! let reader_2 = thread::spawn(move || {
//!     let (n, cv) = &*pair2;
//!     let mut lock = n.lock();
//!     lock = cv.wait(lock);
//!
//!     println!("Thread 2 woke up. Value = {}", *lock);
//! });
//!
//! // Give some time for the readers to enter `wait`
//! thread::sleep(Duration::from_millis(100));
//!
//! let writer = thread::spawn(move || {
//!     let (n, cv) = &*pair;
//!     let mut lock = n.lock();
//!     *lock += 1;
//!     // At this point, 2 threads should be waiting, and this
//!     // call will wake up both of them
//!     cv.notify_all();
//!     println!("notify_all");
//!
//!     // For failsafe, keep notifying.
//!     for _ in 0..100 { cv.notify_all(); }
//! });
//!
//! reader_1.join().unwrap();
//! reader_2.join().unwrap();
//! writer.join().unwrap();
//! ```
//! The output would be
//! ```text
//! notify_all
//! Thread 2 woke up. Value = 1
//! Thread 1 woke up. Value = 1
//! ```
//!
//! The two waiting threads will be woken up by the [`notify_all`] call.
//!
//! [`notify_one`]: Condvar::notify_one
//! [`notify_all`]: Condvar::notify_all

use core::sync::atomic::{AtomicU16, Ordering};

use crate::mutex::{guard_to_mutex, MutexGuard};
use crate::semaphore::Semaphore;

/// A Conditional Variable
///
/// Condvar provides a way to block a thread
/// while waiting for an event to occur.
pub struct Condvar {
    waiters: Semaphore,
    n_waiters: AtomicU16,
}

impl Condvar {
    /// Creates a new [Condvar]
    pub const fn new() -> Self {
        Self { waiters: Semaphore::new(), n_waiters: AtomicU16::new(0) }
    }

    /// Waits for this [Condvar] to be woken up by another thread.
    ///
    /// This method will block until another thread calls
    /// [notify_one](Self::notify_one) or [notify_all](Self::notify_all)
    ///
    /// # Example
    /// ```no_run
    /// use syncrs::{Mutex, Condvar};
    /// use std::sync::Arc;
    /// use std::thread;
    ///
    /// let pair = Arc::new((Mutex::new(false), Condvar::new()));
    /// let pair2 = Arc::clone(&pair);
    ///
    /// // Inside of our lock, spawn a new thread, and then wait for it to start.
    /// thread::spawn(move || {
    ///     let (lock, cvar) = &*pair2;
    ///     let mut started = lock.lock();
    ///     *started = true;
    ///     // We notify the condvar that the value has changed.
    ///     cvar.notify_one();
    /// });
    ///
    /// // Wait for the thread to start up.
    /// let (lock, cvar) = &*pair;
    /// let mut started = lock.lock();
    /// while !*started {
    ///     started = cvar.wait(started);
    /// }
    /// ```
    pub fn wait<'mtx, T>(&self, guard: MutexGuard<'mtx, T>) -> MutexGuard<'mtx, T> {
        let mutex = guard_to_mutex(guard);
        self.n_waiters.fetch_add(1, Ordering::Relaxed);
        self.waiters.dec(1);
        mutex.lock()
    }

    /// Like [wait](Self::wait), but waits while the given `predicate` returns true
    /// If the predicate returns true, the `guard` is released and the thread continues
    /// waiting.
    ///
    /// This is the same as doing the following
    /// ```no_run
    /// use syncrs::{Mutex, Condvar};
    /// let mutex = Mutex::new(10_i32);
    /// let cvar = Condvar::new();
    /// let mut guard = mutex.lock();
    /// let mut pred = |n: &mut i32| *n > 0;
    /// let guard =
    ///   loop {
    ///       guard = cvar.wait(guard);
    ///       if !pred(&mut *guard) {
    ///           break guard
    ///       }
    ///   };
    /// ```
    ///
    /// # Example
    /// ```
    /// use syncrs::{Mutex, Condvar};
    /// use std::sync::Arc;
    /// use std::thread;
    ///
    /// let pair = Arc::new((Mutex::new(10_i32), Condvar::new()));
    /// let pair2 = Arc::clone(&pair);
    ///
    /// thread::spawn(move || {
    ///     loop {
    ///         let (lock, cvar) = &*pair2;
    ///         let mut lock = lock.lock();
    ///         *lock -= 1;
    ///         cvar.notify_one();
    ///     }
    /// });
    ///
    /// let (lock, cvar) = &*pair;
    /// let mut n = lock.lock();
    /// /* Wait until the other thread decrements the counter bellow 1.
    ///    When this condvar returns, the value must be <= 0. It doesn't
    ///    necessarily needs to be 0. Then condvar only guarantees that it will
    ///    wait while the predicate matches. */
    /// n = cvar.wait_while(n, |n| *n > 0);
    /// assert!(*n <= 0);
    /// ```
    pub fn wait_while<'mtx, T, F>(&self, mut guard: MutexGuard<'mtx, T>, mut predicate: F) -> MutexGuard<'mtx, T>
    where
        F: FnMut(&mut T) -> bool
    {
        loop {
            guard = self.wait(guard);
            if !predicate(&mut *guard) {
                return guard
            }
        }
    }

    /// Informs a possible [waiting](Self::wait) thread of a change.
    /// This will only wake up **ONE** single thread. If you want to wake up
    /// all threads wating on this `Condvar`, use [notify_all](Self::notify_all)
    ///
    /// See also [notify_one vs notify_all](self#notify_one-vs-notify_all)
    pub fn notify_one(&self) {
        let n = self.n_waiters.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
            Some(if n > 0 { n - 1} else { 0 })
        }).unwrap();
        self.waiters.inc(u16::min(n, 1));
    }

    /// Informs all threads [waiting](Self::wait) for this `Condvar` of a change.
    /// Unline [notify_one](Self::notify_one) this function will wake up **ALL**
    /// threads waiting.
    ///
    /// See also [notify_one vs notify_all](self#notify_one-vs-notify_all)
    pub fn notify_all(&self) {
        let n = self.n_waiters.fetch_min(0, Ordering::Relaxed);
        self.waiters.inc(n);
    }
}

impl Default for Condvar {
    fn default() -> Self {
        Self::new()
    }
}
