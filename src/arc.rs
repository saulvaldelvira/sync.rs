use core::alloc::Layout;
use core::marker::PhantomData;
use core::ops::Deref;
use core::ptr::{self, NonNull};
use allocator_api2::alloc::{self, Allocator, Global};
use crate::Mutex;

struct ArcInner<T> {
    strong_count: Mutex<usize>,
    value: T,
}

/// A thread-safe reference counted smart pointer
///
/// # Example
/// ```
/// use syncrs::Arc;
///
/// let n = Arc::new(123);
/// {
///     let n2 = Arc::clone(&n);
///     assert_eq!(n.strong_count(), 2);
/// } // n2 is dropped here, but the value isn't
///
/// assert_eq!(n.strong_count(), 1);
/// ```
pub struct Arc<T, A: Allocator = Global> {
    inner: NonNull<ArcInner<T>>,
    _marker: PhantomData<ArcInner<T>>,
    allocator: A,
}

impl<T> Arc<T> {
    /// Creates a new [Arc] from `value`, using the [Global] allocator
    pub fn new(value: T) -> Self {
        Arc::new_in(value, Global)
    }
}

impl<T, A: Allocator> Arc<T, A> {

    /// Creates a new [Arc] from `value`, using the given `allocator`
    pub fn new_in(value: T, allocator: A) -> Self {
        let layout = Layout::new::<ArcInner<T>>();
        let inner = allocator
            .allocate(layout)
            .unwrap_or_else(|_| {
                alloc::handle_alloc_error(layout);
            })
            .cast();
        unsafe {
            inner.write(
                ArcInner {
                    value,
                    strong_count: Mutex::new(1),
                }
            );
        }

        Self { inner, allocator, _marker: PhantomData }
    }

    /// Returns a reference to the inner value
    pub const fn as_ref(&self) -> &T {
        unsafe { &self.inner.as_ref().value }
    }

    /// Returns the strong count for this pointer
    ///
    /// # Example
    /// ```
    /// use syncrs::Arc;
    ///
    /// let n = Arc::new(123);
    /// assert_eq!(n.strong_count(), 1);
    /// let n2 = Arc::clone(&n);
    /// assert_eq!(n.strong_count(), 2);
    /// ```
    pub fn strong_count(&self) -> usize {
        unsafe { *self.inner.as_ref().strong_count.lock() }
    }
}

unsafe impl<T: Sync + Send, A: Allocator + Send> Send for Arc<T, A> {}
unsafe impl<T: Sync + Send, A: Allocator + Sync> Sync for Arc<T, A> {}

impl<T, A: Allocator> Deref for Arc<T, A> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<T, A: Allocator> AsRef<T> for Arc<T, A> {
    fn as_ref(&self) -> &T {
        self.as_ref()
    }
}

impl<T, A: Allocator + Clone> Clone for Arc<T, A> {
    fn clone(&self) -> Self {
        unsafe {
            let inner = self.inner.as_ref();
            let mut count = inner.strong_count.lock();
            *count += 1;
            Self {
                inner: self.inner,
                allocator: self.allocator.clone(),
                _marker: PhantomData
            }
        }
    }
}

impl<T, A: Allocator> Drop for Arc<T, A> {
    fn drop(&mut self) {
        unsafe {
            {
                let mut count = self.inner.as_ref().strong_count.lock();
                if *count > 1 {
                    *count -= 1;
                    return
                }
            }
            let inner = self.inner.as_mut();
            ptr::drop_in_place(inner);

            let layout = Layout::new::<ArcInner<T>>();
            let inner = self.inner.cast();
            self.allocator.deallocate(inner, layout);
        }
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use allocator_api2::vec::Vec;

    use super::*;

    #[test]
    fn arc_test() {
        let arc = Arc::new(25);

        let handles = (0..20).map(|_| {
            let ac = Arc::clone(&arc);
            std::thread::spawn(move || {
                assert_eq!(*ac, 25);
            })
        }).collect::<Vec<_>>();

        for h in handles { h.join().unwrap() }
    }
}
