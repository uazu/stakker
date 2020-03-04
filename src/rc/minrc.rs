use std::cell::Cell;
use std::marker::PhantomData;
use std::ptr::NonNull;

/// Minimum Rc for our purposes.  Only uses a couple of `unsafe`.  Has
/// a single reference count which takes care of dropping the
/// contained instance when it goes to zero.  This could be regarded
/// as a strong count if it is the only count, or as a weak count if
/// there is another count which deletes the contents earlier.
pub(crate) struct MinRc<T: ?Sized> {
    ptr: NonNull<MinRcBox<T>>,
    // Use *const to make it !Send and !Sync
    phantomdata: PhantomData<*const T>,
}

pub(super) struct MinRcBox<T: ?Sized> {
    pub count: Cell<usize>,
    pub inner: T,
}

// TODO: Handle DSTs like Rc does, but CoerceUnsized is not stable.
// So right now we have to use a work-around via `new_with`.
// https://github.com/rust-lang/rust/issues/27732
//
//impl<T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<MinRc<U>> for MinRc<T> {}

impl<T> MinRc<T> {
    #[inline]
    pub fn new(val: T) -> Self {
        Self {
            ptr: NonNull::new(Box::into_raw(Box::new(MinRcBox {
                count: Cell::new(1),
                inner: val,
            })))
            .unwrap(),
            phantomdata: PhantomData,
        }
    }
}

impl<T: ?Sized> MinRc<T> {
    // TODO: This is a CoerceUnsized work-around
    #[inline]
    pub(super) fn new_with(cb: impl FnOnce() -> Box<MinRcBox<T>>) -> Self {
        Self {
            ptr: NonNull::new(Box::into_raw(cb())).unwrap(),
            phantomdata: PhantomData,
        }
    }

    #[inline]
    fn rcbox(&self) -> &MinRcBox<T> {
        // Safe because MinRcBox is not freed until last ref has gone
        unsafe { self.ptr.as_ref() }
    }

    #[inline]
    pub fn inner(&self) -> &T {
        &self.rcbox().inner
    }
}

impl<T: ?Sized> Clone for MinRc<T> {
    fn clone(&self) -> Self {
        let rcbox = self.rcbox();
        rcbox.count.replace(rcbox.count.get().saturating_add(1));
        Self {
            ptr: self.ptr,
            phantomdata: PhantomData,
        }
    }
}

impl<T: ?Sized> Drop for MinRc<T> {
    fn drop(&mut self) {
        // Locks on max value.  So this leaks the memory, which is
        // safe.  This can only happen if someone has already leaked
        // ~2**64 references to this instance (on 64-bit).
        let rcbox = self.rcbox();
        let (count, went_to_zero) = match rcbox.count.get() {
            1 => (0, true),
            0 => (0, false),
            std::usize::MAX => (std::usize::MAX, false),
            v => (v - 1, false),
        };
        rcbox.count.replace(count);

        if went_to_zero {
            // Safe because we drop this just once, when the final
            // reference is released
            unsafe { drop(Box::from_raw(self.ptr.as_mut())) }
        }
    }
}
