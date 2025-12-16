use core::mem::ManuallyDrop;
use core::ops::Deref;
use core::ptr;

use crate::once::OnceLock;

/// A synchronization primitive that is lazily initialized on its first access
///
/// # Example
/// ```
/// use syncrs::LazyLock;
/// use std::collections::HashMap;
///
/// static MAP: LazyLock<HashMap<i32, String>> = LazyLock::new(|| {
///     let mut map = HashMap::new();
///     for n in 0..10 {
///         map.insert(n, format!("{n}"));
///     }
///     map
/// });
///
/// let map = MAP.get(); /* MAP is initialized at this point */
/// assert_eq!(
///     map.get(&4).map(|s| s.as_str()),
///     Some("4"),
/// );
/// ```
pub struct LazyLock<T, F = fn() -> T> {
    once: OnceLock<T>,
    init: ManuallyDrop<F>,
}

impl<T, F> LazyLock<T, F>
where
    F: FnOnce() -> T,
{
    /// Builds a new [LazyLock] with the given `init` function
    #[inline]
    pub const fn new(init: F) -> Self {
        Self {
            once: OnceLock::new(),
            init: ManuallyDrop::new(init),
        }
    }

    /// Gets the element inside `self`. If it's the first access,
    /// initializes it with the function passed to [Self::new]
    ///
    /// # Panics
    /// If the initialization function panics, the panic is propagated to the caller
    ///
    /// # Example
    /// ```
    /// use syncrs::LazyLock;
    ///
    /// static MAP: LazyLock<u64> = LazyLock::new(|| 1234);
    ///
    /// let map = MAP.get(); /* MAP is initialized here */
    /// assert_eq!(map, &1234);
    /// ```
    pub fn get(&self) -> &T {
        let Self { once, init } = self;
        once.get_or_init(|| {
            /* SAFETY: OnceLock guarantees that this function will only be
             * called once. So reading this FnOnce from init is fine.
             * The first time call self.get, we'll read the FnOnce from
             * `self.init` and we will never read it again. */
            unsafe {
                let init: F = ptr::read(init.deref());
                init()
            }
        })
    }

    /// Tries to get the element. Returns [None] if the
    /// element is not initialized
    ///
    /// # Example
    /// ```
    /// use syncrs::LazyLock;
    ///
    /// static MAP: LazyLock<u64> = LazyLock::new(|| 1234);
    ///
    /// assert_eq!(MAP.try_get(), None);
    /// MAP.get(); /* MAP is initialized here */
    /// assert_eq!(MAP.try_get(), Some(&1234));
    /// ```
    #[inline]
    pub fn try_get(&self) -> Option<&T> {
        self.once.get()
    }
}

impl<T> Deref for LazyLock<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

// We never create a `&F` from a `&LazyLock<T, F>` so it is fine
// to not impl `Sync` for `F`.
unsafe impl<T: Sync + Send, F: Send> Sync for LazyLock<T, F> {}

// auto-derived `Send` impl is OK.

#[cfg(test)]
mod test {
    extern crate std;
    use std::collections::HashMap;
    use std::vec::Vec;

    use super::*;

    static MAP: LazyLock<HashMap<i32, i32>> = LazyLock::new(|| {
        let mut map = HashMap::new();
        map.insert(12, 12);
        map.insert(13, 13);
        map.insert(14, 14);
        map
    });

    #[test]
    fn test() {

        let handles = (0..10).map(|_| {
            std::thread::spawn(|| {
                for (k, v) in MAP.get() {
                    assert_eq!(k, v);
                }
            })
        }).collect::<Vec<_>>();

        for h in handles { h.join().unwrap() }
    }
}
