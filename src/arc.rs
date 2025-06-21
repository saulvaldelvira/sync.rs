//! An atomic reference-counted smart pointer

use core::alloc::Layout;
use core::marker::PhantomData;
use core::ops::Deref;
use core::ptr::{self, NonNull};
use allocator_api2::alloc::{self, Allocator, Global};
use crate::Mutex;

struct ArcInner<T> {
    strong_count: Mutex<usize>,
    weak_count: Mutex<usize>,
    value: T,
}

impl<T> ArcInner<T> {
    unsafe fn destroy<A: Allocator>(this: NonNull<Self>, a: &A) {
        let layout = Layout::new::<ArcInner<T>>();
        let inner = this.cast();
        unsafe { a.deallocate(inner, layout); }
    }
}

/// A weak pointer to an [Arc]
///
/// It is created from [Arc::downgrade]
///
/// A `Weak` pointer doesn't own the value.
/// To use a weak pointer to need to [upgrade](Weak::upgrade)
/// it to an [Arc] first
///
/// Weak pointers are used to avoid reference cycles.
pub struct Weak<T, A: Allocator = Global> {
    inner: NonNull<ArcInner<T>>,
    allocator: A,
}

impl<T, A: Allocator> Drop for Weak<T, A> {
    fn drop(&mut self) {
        unsafe {
            {
                let mut weak = self.inner.as_ref().weak_count.lock();
                *weak -= 1;
                if *weak > 0 {
                    return
                }
            }
            if *self.inner.as_ref().strong_count.lock() == 0 {
                ArcInner::destroy(self.inner, &self.allocator);
            }
        }
    }
}

impl<T, A: Allocator> Weak<T, A> {
    fn new(inner: NonNull<ArcInner<T>>, allocator: A) -> Self {
        unsafe { *inner.as_ref().weak_count.lock() += 1 };
        Self { inner, allocator }
    }

    /// Upgrades `self` to an [Arc].
    ///
    /// This may fail, since the original [Arc] from which
    /// it was created could've been destroyed. In that case
    /// returns [None]
    pub fn upgrade(&self) -> Option<Arc<T, A>>
    where
        A: Clone
    {
        let can_upgrade = unsafe {
            let mut strong = self.inner.as_ref().strong_count.lock();
            if *strong > 0 {
                *strong += 1;
                true
            } else {
                false
            }
        };
        can_upgrade.then(|| Arc {
            inner: self.inner,
            allocator: self.allocator.clone(),
            _marker: PhantomData
        })
    }

    /// Returns the strong count for this pointer
    ///
    /// # Example
    /// ```
    /// use syncrs::Arc;
    ///
    /// let strong = Arc::new(123);
    ///
    /// let weak = strong.downgrade();
    /// assert_eq!(weak.strong_count(), 1);
    ///
    /// {
    ///     let strong = Arc::clone(&strong);
    ///     assert_eq!(weak.strong_count(), 2);
    /// }
    ///
    /// assert_eq!(weak.strong_count(), 1);
    /// drop(strong);
    /// assert_eq!(weak.strong_count(), 0);
    /// ```
    pub fn strong_count(&self) -> usize {
        unsafe { *self.inner.as_ref().strong_count.lock() }
    }

    /// Returns the weak count for this pointer
    ///
    /// # Example
    /// ```
    /// use syncrs::arc::{Arc, Weak};
    ///
    /// let n = Arc::new(123);
    /// let weak = n.downgrade();
    /// assert_eq!(weak.weak_count(), 1);
    /// {
    ///     let weak2 = Weak::clone(&weak);
    ///     assert_eq!(weak.weak_count(), 2);
    /// }
    /// assert_eq!(weak.weak_count(), 1);
    /// ```
    pub fn weak_count(&self) -> usize {
        unsafe { *self.inner.as_ref().weak_count.lock() }
    }
}

impl<T, A: Allocator + Clone> Clone for Weak<T, A> {
    fn clone(&self) -> Self {
        Self::new(self.inner, self.allocator.clone())
    }
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
    allocator: A,
    _marker: PhantomData<ArcInner<T>>,
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
                    weak_count: Mutex::new(0),
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

    /// Returns the weak count for this pointer
    ///
    /// # Example
    /// ```
    /// use syncrs::Arc;
    ///
    /// let n = Arc::new(123);
    /// assert_eq!(n.weak_count(), 0);
    /// {
    ///     let weak = n.downgrade();
    ///     assert_eq!(n.weak_count(), 1);
    /// }
    /// assert_eq!(n.weak_count(), 0);
    /// ```
    pub fn weak_count(&self) -> usize {
        unsafe { *self.inner.as_ref().weak_count.lock() }
    }

    /// Creates a [Weak] pointer from this [Arc].
    #[inline]
    pub fn downgrade(&self) -> Weak<T, A>
    where
        A: Clone
    {
        Weak::new(self.inner, self.allocator.clone())
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
                *count -= 1;
                if *count >= 1 {
                    return
                }
            }
            let inner = self.inner.as_mut();
            ptr::drop_in_place(inner);

            let weak = self.inner.as_ref().weak_count.lock();
            if *weak == 0 {
                ArcInner::destroy(self.inner, &self.allocator);
            }
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
