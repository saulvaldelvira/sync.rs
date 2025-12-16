use core::cell::UnsafeCell;
use core::mem::MaybeUninit;

use crate::{Condvar, Mutex};

mod sealed {
    pub trait SealedTrait {}
    impl SealedTrait for () {}
    impl SealedTrait for super::Waiteable {}
}

#[repr(transparent)]
#[derive(Default)]
pub struct Waiteable(Condvar);

pub trait MaybeWaiteable: Default + sealed::SealedTrait {
    fn notify(&self);
}

impl MaybeWaiteable for () {
    fn notify(&self) { }
}

impl MaybeWaiteable for Waiteable {
    fn notify(&self) {
        self.0.notify_all();
    }
}

/// A synchronization primitive for a one-time execution
///
/// This is a low-level primitive, used to compose others like [OnceLock]
///
/// # Example
/// ```
/// use syncrs::once::{Once, Waiteable};
///
/// static IS_INIT: Once<Waiteable> = Once::new_waiteable();
///
/// std::thread::spawn(|| {
///     IS_INIT.call_once(|| {
///         // Something ...
///     })
/// });
///
/// // Will block until the spawned thread marks IS_INIT as initialized
/// IS_INIT.wait();
///
/// ```
pub struct Once<W: MaybeWaiteable = ()> {
    is_init: Mutex<bool>,
    cv: W,
}

impl<W: MaybeWaiteable> Once<W> {
    /// Calls the given function if this [Once] is uninitialized
    /// Else, does nothing
    pub fn call_once<F>(&self, f: F)
    where
        F: FnOnce()
    {
        {
            let mut lock = self.is_init.lock();
            if !*lock {
                f();
                *lock = true;
                self.cv.notify();
            }
        }
    }

    /// Returns true if this [OnceLock] is initialized
    ///
    /// # Example
    /// ````
    /// use syncrs::once::Once;
    ///
    /// static CELL: Once= Once::new();
    ///
    /// assert!(!CELL.is_initialized());
    /// CELL.call_once(|| {});
    /// assert!(CELL.is_initialized());
    /// ```
    pub fn is_initialized(&self) -> bool {
        *self.is_init.lock()
    }

    pub fn reset(&self) -> bool {
        let mut lock = self.is_init.lock();
        let was_init = *lock;
        *lock = false;
        was_init
    }

}

impl Once<()> {
    /// Creates a new uninitialized [Once]
    pub const fn new() -> Self {
        Self {
            is_init: Mutex::new(false),
            cv: ()
        }
    }
}


impl Once<Waiteable> {
    /// Creates a new waiteable [Once]
    ///
    /// Waiteable [Once]s have the [wait](Self::wait) method
    pub const fn new_waiteable() -> Self {
        Self {
            is_init: Mutex::new(false),
            cv: Waiteable(Condvar::new())
        }
    }

    /// Waits for `self` to be initialized
    pub fn wait(&self) {
        let lock = self.is_init.lock();
        if !*lock {
            let _ = self.cv.0.wait_while(lock, |n| !*n);
        }
    }
}

impl<W: MaybeWaiteable> Default for Once<W> {
    fn default() -> Self {
        Self {
            is_init: Mutex::new(false),
            cv: W::default(),
        }
    }
}

unsafe impl<W: MaybeWaiteable> Sync for Once<W> {}
unsafe impl<W: MaybeWaiteable> Send for Once<W> {}

/// A synchronization primitive that can only be written to once
///
/// # Example
/// ```
/// use syncrs::OnceLock;
///
/// static CELL: OnceLock<usize> = OnceLock::new();
/// // `OnceLock` has not been written to yet.
/// assert!(CELL.get().is_none());
///
/// // Spawn a thread and write to `OnceLock`.
/// std::thread::spawn(|| {
///     let value = CELL.get_or_init(|| 12345);
///     assert_eq!(value, &12345);
/// })
/// .join()
/// .unwrap();
///
/// // `OnceLock` now contains the value.
/// assert_eq!(
///     CELL.get(),
///     Some(&12345),
/// );
/// ```
pub struct OnceLock<T, W: MaybeWaiteable = ()> {
    elem: UnsafeCell<MaybeUninit<T>>,
    once: Once<W>,
}

impl<T> OnceLock<T> {
    /// Creates a new uninitialized [OnceLock]
    pub const fn new() -> Self {
        Self {
            elem: UnsafeCell::new(MaybeUninit::uninit()),
            once: Once::new(),
        }
    }
}

impl<T> OnceLock<T, Waiteable> {
    /// Creates a new waiteable [OnceLock]
    ///
    /// Waiteable [OnceLock]s have a [wait](Self::wait) method
    pub const fn new_waiteable() -> Self {
        Self {
            elem: UnsafeCell::new(MaybeUninit::uninit()),
            once: Once::new_waiteable(),
        }
    }
}

impl<T, W: MaybeWaiteable> OnceLock<T, W> {
    /// Gets the element, initialzing it with `init` if necessary
    ///
    /// # Panics
    /// If the given function panics, the panic is propagated to the caller
    ///
    /// # Example
    /// ```
    /// use syncrs::OnceLock;
    ///
    /// static CELL: OnceLock<usize> = OnceLock::new();
    ///
    /// assert_eq!(
    ///     // CELL is empty, it will be initialized with the closure
    ///     CELL.get_or_init(|| 123),
    ///     &123,
    /// );
    ///
    /// assert_eq!(
    ///     // CELL is already initialized, so the closure won't be called
    ///     CELL.get_or_init(|| 999),
    ///     &123
    /// );
    /// ```
    pub fn get_or_init<F>(&self, init: F) -> &T
    where
        F: FnOnce() -> T
    {
        self.once.call_once(|| {
            unsafe {
                /* SAFETY: This will execute only once, and synchronously
                 * Therefore, the mutable reference won't alias, and the
                 * element will only initialize once */
                self.elem.get().as_mut().unwrap().write(init());
            }
        });
        /* The element is initialized. Now we can get as many shared
         * references as we want*/
        unsafe { self.elem.get().as_ref().unwrap().assume_init_ref() }
    }

    /// Tries to get the element, if it is initialized
    ///
    /// # Example
    /// ```
    /// use syncrs::OnceLock;
    ///
    /// static CELL: OnceLock<usize> = OnceLock::new();
    ///
    /// assert!(CELL.get().is_none());
    /// CELL.get_or_init(|| 123);
    ///
    /// assert_eq!(
    ///     CELL.get(),
    ///     Some(&123),
    /// );
    /// ```
    pub fn get(&self) -> Option<&T> {
        self.is_initialized().then(|| {
            /* The element is initialized. */
            unsafe { self.elem.get().as_ref().unwrap().assume_init_ref() }
        })
    }

    /// Tries to get a mutable reference to the element, if it is initialized
    ///
    /// Since this borrows self mutably, it is statically inforced that no other thread
    /// is accessing the element.
    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.is_initialized().then(|| {
            /* The element is initialized. */
            unsafe { self.elem.get().as_mut().unwrap().assume_init_mut() }
        })
    }

    /// Returns true if this [OnceLock] is initialized
    ///
    /// # Example
    /// ````
    /// use syncrs::OnceLock;
    ///
    /// static CELL: OnceLock<i32> = OnceLock::new();
    ///
    /// assert!(!CELL.is_initialized());
    /// CELL.set(11).unwrap();
    /// assert!(CELL.is_initialized());
    /// ```
    #[inline]
    pub fn is_initialized(&self) -> bool {
        self.once.is_initialized()
    }

    /// Sets the value of this [OnceLock].
    /// If it was already initialized, returns an Err variant with the given value.
    ///
    /// # Example
    /// ```
    /// use syncrs::OnceLock;
    ///
    /// static CELL: OnceLock<i32> = OnceLock::new();
    ///
    /// assert!(CELL.get().is_none());
    /// assert_eq!(CELL.set(92), Ok(()));
    /// assert_eq!(CELL.set(62), Err(62));
    /// assert_eq!(CELL.get(), Some(&92));
    /// ```
    pub fn set(&self, value: T) -> Result<(), T> {
        let mut result = Some(value);
        self.once.call_once(|| {
            unsafe {
                /* Safety: result is ALWAYS Some. We only need an option in
                 * the case we actually consume the value. */
                let value = result.take().unwrap_unchecked();
                /* SAFETY: This will execute only once, and synchronously
                 * Therefore, the mutable reference won't alias, and the
                 * element will only initialize once */
                self.elem.get().as_mut().unwrap().write(value);
            }
        });
        match result {
            Some(val) => Err(val),
            None => Ok(())
        }
    }

    /// Resets this [OnceLock], returning the contained element if it was initialized.
    ///
    /// # Example
    /// ````
    /// use syncrs::OnceLock;
    ///
    /// let mut cell = OnceLock::new();
    /// cell.set(12).unwrap();
    /// assert_eq!(cell.get(), Some(&12));
    /// assert_eq!(cell.take(), Some(12));
    /// assert_eq!(cell.get(), None);
    /// ```
    pub fn take(&mut self) -> Option<T> {
        self.once.reset().then(|| {
            /* SAFETY: elem is initialized. And we've set is_init to false.
             * Only one thread -and only once- can reach here. */
            unsafe { self.elem.get().read().assume_init() }
        })
    }

    /// Consumes this [OnceLock], returning the contained element if it was initialized.
    ///
    /// # Example
    /// ````
    /// use syncrs::OnceLock;
    ///
    /// let cell = OnceLock::new();
    /// cell.set(12).unwrap();
    /// assert_eq!(cell.get(), Some(&12));
    /// assert_eq!(cell.into_inner(), Some(12));
    /// ```
    #[inline]
    pub fn into_inner(mut self) -> Option<T> {
        self.take()
    }
}

impl<T> OnceLock<T, Waiteable> {
    /// Waits for `self` to be initialized.
    /// Returns the element once it's done.
    pub fn wait(&self) -> &T {
        self.once.wait();
        unsafe { self.elem.get().as_ref().unwrap().assume_init_ref() }
    }
}

impl<T, W: MaybeWaiteable> Default for OnceLock<T, W> {
    fn default() -> Self {
        Self {
            elem: UnsafeCell::new(MaybeUninit::uninit()),
            once: Once::<W>::default(),
        }
    }
}

/* Taken from rust stdlib:  */
// Why do we need `T: Send`?
// Thread A creates a `OnceLock` and shares it with
// scoped thread B, which fills the cell, which is
// then destroyed by A. That is, destructor observes
// a sent value.
unsafe impl<T: Sync + Send> Sync for OnceLock<T> {}
unsafe impl<T: Send> Send for OnceLock<T> {}
