//! A mutex, providing unique syncronized access to a value

use core::cell::UnsafeCell;
use core::cmp::Ordering;
use core::fmt::{Debug, Display};
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

use crate::spin::{SpinLock, SpinLockGuard};

/// A mutex grants synchronized access to a value
///
/// # Example
/// ```
/// use syncrs::{Mutex, LazyLock};
///
/// static N: Mutex<u32> = Mutex::new(0);
///
/// let threads = (0..10).map(|_|{
///     std::thread::spawn(move || {
///         let mut sync_n = N.lock();
///         for _ in 0..10 {
///             *sync_n += 1;
///         }
///     })
/// }).collect::<Vec<_>>();
///
/// for t in threads {
///     t.join().unwrap();
/// }
///
/// assert_eq!(*N.lock(), 100);
/// ```
pub struct Mutex<T: ?Sized> {
    lock: SpinLock,
    data: UnsafeCell<T>,
}

/// `T` must be Send for `Mutex<T>` to be Send
/// It's possible to unwrap the `Mutex` into it's inner
/// value ([Mutex::into_inner]). So we need to make sure
/// that T is Send.
unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}

unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

/// A RAII structure for a [mutex](Mutex) lock
///
/// This struct is built by [Mutex::lock], and it
/// unlocks the mutex when droped.
#[must_use = "if unused, the lock will release automatically"]
pub struct MutexGuard<'a, T: ?Sized> {
    data: NonNull<T>,
    _lock: SpinLockGuard<'a>,
}

impl<T: Debug> Debug for MutexGuard<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "MutexGuard({:?})", unsafe { &*self.data.as_ptr() })
    }
}

impl<T: Display> Display for MutexGuard<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        unsafe { <T as Display>::fmt(self.data.as_ref(), f) }
    }
}

impl<T: PartialEq> PartialEq for MutexGuard<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        unsafe { self.data.as_ref().eq(other.data.as_ref()) }
    }
}

impl<T: PartialOrd> PartialOrd for MutexGuard<'_, T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        unsafe { self.data.as_ref().partial_cmp(other.data.as_ref()) }
    }
}

/// It's safe to drop a `MutexGuard` on a different thread from
/// the one it created it
unsafe impl<T: ?Sized + Send> Send for MutexGuard<'_, T> {}

/// `T` must be `Sync` for `MutexGuard` to be `Sync`, since
/// `MutexGuard` derefs to `&T`
unsafe impl<T: ?Sized + Sync> Sync for MutexGuard<'_, T> {}

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

    /// Consumes `self` and returns the inner `T` value
    ///
    /// # Safety
    /// Since we take `self` by value, we don't need to
    /// synchronize the access.
    ///
    /// # Example
    /// ```
    /// use syncrs::Mutex;
    ///
    /// let mutex = Mutex::new(120);
    /// assert_eq!(Mutex::into_inner(mutex), 120);
    /// ```
    pub fn into_inner(self) -> T {
        UnsafeCell::into_inner(self.data)
    }
}

impl<T: ?Sized> Mutex<T> {
    /// Returns a [lock guard](MutexGuard) for `self`, if it is
    /// available.
    /// If the operation whould've locked the execution, returns
    /// [None] inmediately
    ///
    /// The lock is held until the `MutexGuard` is dropped.
    ///
    /// # Example
    /// ```
    /// use syncrs::Mutex;
    ///
    /// let mutex = Mutex::new(10);
    /// let guard1 = mutex.lock();
    ///
    /// // guard1 holds the lock
    /// assert_eq!(mutex.try_lock(), None);
    /// drop(guard1); // This frees the mutex
    /// assert!(mutex.try_lock().is_some());
    /// ```
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
    ///
    /// # Example
    /// ```
    /// use syncrs::Mutex;
    ///
    /// let mutex = Mutex::new(10);
    /// let guard1 = mutex.lock();
    /// assert_eq!(*guard1, 10);
    /// ```
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
    #[inline]
    pub const fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }

    /// Returns true if `self` is currently locked
    ///
    /// # Example
    /// ```
    /// use syncrs::Mutex;
    ///
    /// let mutex = Mutex::new(5);
    ///
    /// {
    ///     let guard = mutex.lock();
    ///     assert!(mutex.is_locked());
    /// } // guard is dropped here
    /// assert!(!mutex.is_locked());
    /// ```
    #[inline]
    pub fn is_locked(&self) -> bool {
        self.lock.is_locked()
    }
}

impl<T: Default> Default for Mutex<T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}
