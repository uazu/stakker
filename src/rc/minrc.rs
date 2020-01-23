// Minimum Rc for our purposes, with only a strong count (no weak
// count).  Also the count is type parameter, so there's the option of
// packing some other data in there as well.  Only uses a couple of
// `unsafe`.

use std::cell::Cell;
use std::marker::PhantomData;
use std::ptr::NonNull;

pub(super) struct MinRcBox<C: StrongCount, T: ?Sized> {
    pub count: Cell<C>,
    pub inner: T,
}

pub(crate) struct MinRc<C: StrongCount, T: ?Sized> {
    ptr: NonNull<MinRcBox<C, T>>,
    // Use *const to make it !Send and !Sync
    phantomdata: PhantomData<*const T>,
}

// TODO: Handle DSTs like Rc does, but CoerceUnsized is not stable, so
// right now we have to use a work-around via `new_with`.
// https://github.com/rust-lang/rust/issues/27732
//
//impl<T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<MinRc<U>> for MinRc<T> {}

impl<C: StrongCount, T> MinRc<C, T> {
    #[inline]
    pub fn new(val: T) -> Self {
        Self {
            ptr: NonNull::new(Box::into_raw(Box::new(MinRcBox {
                count: Cell::new(C::new()),
                inner: val,
            })))
            .unwrap(),
            phantomdata: PhantomData,
        }
    }
}

impl<C: StrongCount, T: ?Sized> MinRc<C, T> {
    // TODO: This is a CoerceUnsized work-around
    #[inline]
    pub(super) fn new_with(cb: impl FnOnce() -> Box<MinRcBox<C, T>>) -> Self {
        Self {
            ptr: NonNull::new(Box::into_raw(cb())).unwrap(),
            phantomdata: PhantomData,
        }
    }

    #[inline]
    fn rcbox(&self) -> &MinRcBox<C, T> {
        // Safe because MinRcBox is not freed until last ref has gone
        unsafe { self.ptr.as_ref() }
    }

    #[inline]
    pub fn inner(&self) -> &T {
        &self.rcbox().inner
    }

    /// The purpose of this call is to allow associated data stashed
    /// along with the count to be changed.  However the caller is not
    /// allowed to change the count itself, or otherwise they could
    /// cause unsoundness.  So maintain invariants with an assertion,
    /// which should hopefully be eliminated by the compiler.
    #[inline]
    pub fn count<R>(&self, cb: impl FnOnce(&Cell<C>) -> R) -> R {
        let cell = &self.rcbox().count;
        let was = cell.get().get();
        let rv = cb(cell);
        assert_eq!(
            was,
            cell.get().get(),
            "Count changed in MinRc::count() call"
        );
        rv
    }
}

impl<C: StrongCount, T: ?Sized> Clone for MinRc<C, T> {
    fn clone(&self) -> Self {
        let rcbox = self.rcbox();
        rcbox.count.replace(rcbox.count.get().inc());
        Self {
            ptr: self.ptr,
            phantomdata: PhantomData,
        }
    }
}

impl<C: StrongCount, T: ?Sized> Drop for MinRc<C, T> {
    fn drop(&mut self) {
        let rcbox = self.rcbox();
        let (count, went_to_zero) = rcbox.count.get().dec();
        rcbox.count.replace(count);
        if went_to_zero {
            // Safe because we drop this just once, when the final
            // reference is released
            unsafe { drop(Box::from_raw(self.ptr.as_mut())) }
        }
    }
}

/// Interface that a count must implement
pub(crate) trait StrongCount: Copy + Clone {
    // Create a new instance with count of 1
    fn new() -> Self;

    // Increase count
    fn inc(self) -> Self;

    // Decrease count and check if went to zero (i.e. 1 -> 0
    // transition)
    fn dec(self) -> (Self, bool);

    // Get the current ref-count
    fn get(self) -> usize;
}

/// Simple usize-based count implementation.  Locks on max value
/// rather than wrap or panic, so leaks in that case (which is safe).
#[derive(Copy, Clone)]
pub(crate) struct Count(usize);

impl StrongCount for Count {
    fn new() -> Self {
        Self(1)
    }
    fn inc(self) -> Self {
        Self(self.0.saturating_add(1))
    }
    fn dec(self) -> (Self, bool) {
        match self.0 {
            1 => (Self(0), true),
            0 | std::usize::MAX => (self, false), // Locks on max
            v => (Self(v - 1), false),
        }
    }
    fn get(self) -> usize {
        self.0
    }
}
