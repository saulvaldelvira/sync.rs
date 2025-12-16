use core::cell::UnsafeCell;
use core::mem::MaybeUninit;

use crate::Mutex;

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
pub struct OnceLock<T> {
    elem: UnsafeCell<MaybeUninit<T>>,
    is_init: Mutex<bool>,
}

impl<T> OnceLock<T> {
    /// Creates a new uninitialized [OnceLock]
    pub const fn new() -> Self {
        Self {
            elem: UnsafeCell::new(MaybeUninit::uninit()),
            is_init: Mutex::new(false),
        }
    }

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
        {
            let mut lock = self.is_init.lock();
            if !*lock {
                /* SAFETY: We've locked the mutex, so only one thread
                 * can reach this place. Therefore, the mutable reference
                 * won't alias, and the element will only initialize once */
                unsafe {
                    self.elem.get().as_mut().unwrap().write(init());
                }
                *lock = true;
            }
        }
        /* The lock is dropped. Now we can get as many shared
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
        let lock = self.is_init.lock();
        lock.then(|| {
            /* The access is syncronized
             * If lock is true, a previous call to get_or_init has initialized
             * the element correctly. */
            unsafe { self.elem.get().as_ref().unwrap().assume_init_ref() }
        })
    }

    /// Tries to get a mutable reference to the element, if it is initialized
    ///
    /// Since this borrows self mutably, it is statically inforced that no other thread
    /// is accessing the element.
    pub fn get_mut(&mut self) -> Option<&mut T> {
        let lock = self.is_init.lock();
        lock.then(|| {
            /* The access is syncronized
             * If lock is true, a previous call to get_or_init has initialized
             * the element correctly. */
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
    pub fn is_initialized(&self) -> bool {
        *self.is_init.lock()
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
        {
            let mut lock = self.is_init.lock();
            if !*lock {
                /* SAFETY: We've locked the mutex, so only one thread
                 * can reach this place. Therefore, the mutable reference
                 * won't alias, and the element will only initialize once */
                unsafe {
                    self.elem.get().as_mut().unwrap().write(value);
                }
                *lock = true;
                return Ok(())
            }
            return Err(value)
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
        if self.is_initialized() {
            self.is_init = Mutex::new(false);
            /* SAFETY: elem is initialized. And we've set is_init to false.
                So this is the only place were the value will be read. */
            let elem = unsafe { self.elem.get().read().assume_init() };
            Some(elem)
        } else {
            None
        }
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

impl<T> Default for OnceLock<T> {
    fn default() -> Self {
        Self::new()
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
