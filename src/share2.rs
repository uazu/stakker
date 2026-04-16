use crate::cell::cell::Share2Cell;
use crate::{Core, Cx};
use std::rc::{Rc, Weak};

/// Ref-counted shared mutable data, with `Core` access
///
/// Look at [`Share`] first.  This does the same as `Share`, but the
/// borrowing owner is outside [`Core`].  This means that it possible
/// to get a reference to a [`Share2`] value and a reference to
/// [`Core`] at the same time.  The cost is that borrowing can only
/// occur in the actor context, i.e. with access to [`Cx`].
///
/// To borrow more than one [`Share2`] instance at a time, see
/// [`Cx::share2_rw2`] and [`Cx::share2_rw3`].
///
/// By default borrow-checking of access to the contents of the
/// [`Share2`] is handled at compile-time using a `TCell` or `TLCell`,
/// so it is zero-cost and compiles down to a direct pointer access to
/// the contents of a `struct`, just as if the [`Share2`] wasn't
/// there.  However cloning a [`Share2`] has the normal `Rc`
/// overheads.
///
/// [`Core`]: struct.Core.html
/// [`Cx::share2_rw2`]: struct.Cx.html#method.share2_rw2
/// [`Cx::share2_rw3`]: struct.Cx.html#method.share2_rw3
/// [`Cx`]: struct.Cx.html
/// [`Share2`]: struct.Share2.html
/// [`Share`]: struct.Share.html
pub struct Share2<T> {
    pub(crate) rc: Rc<Share2Cell<T>>,
}

impl<T> Share2<T> {
    /// Create a new Share instance
    pub fn new<A>(cx: &Cx<A>, val: T) -> Self {
        Self {
            rc: Rc::new(cx.nexus.share2_owner.cell(val)),
        }
    }

    /// Get a read-write (mutable, exclusive) reference to the
    /// contents of the [`Share2`] instance.  By default this is a
    /// static check, which compiles down to a direct access.
    ///
    /// To access more than one [`Share2`] instance at the same time,
    /// see [`Cx::share2_rw2`] and [`Cx::share2_rw3`].
    ///
    /// [`Cx::share2_rw2`]: struct.Cx.html#method.share2_rw2
    /// [`Cx::share2_rw3`]: struct.Cx.html#method.share2_rw3
    /// [`Share2`]: struct.Share2.html
    #[inline]
    pub fn rw<'a, A>(&'a self, cx: &'a mut Cx<A>) -> (&'a mut T, &'a mut Core) {
        let borrow = cx.nexus.share2_owner.rw(&self.rc);
        (borrow, &mut cx.nexus.core)
    }

    /// Create a [`Share2Weak`] reference to the contained object
    ///
    /// [`Share2Weak`]: struct.Share2Weak.html
    #[inline]
    pub fn downgrade(&self) -> Share2Weak<T> {
        Share2Weak {
            weak: Rc::downgrade(&self.rc),
        }
    }

    /// Return the number of strong references to the shared data
    #[inline]
    pub fn strong_count(&self) -> usize {
        Rc::strong_count(&self.rc)
    }

    /// Return the number of weak references to the shared data
    #[inline]
    pub fn weak_count(&self) -> usize {
        Rc::weak_count(&self.rc)
    }
}

impl<T> Clone for Share2<T> {
    /// Return another reference to the shared data
    fn clone(&self) -> Self {
        Self {
            rc: self.rc.clone(),
        }
    }
}

/// Weak reference to a [`Share2`]
///
/// This reference does not stop the [`Share2`] from being dropped, but
/// can be used to recover a [`Share2`] reference if it is still alive
/// due to a strong reference held elsewhere.
///
/// [`Share2`]: struct.Share2.html
pub struct Share2Weak<T> {
    weak: Weak<Share2Cell<T>>,
}

impl<T> Share2Weak<T> {
    /// If there are still strong references to the shared data,
    /// returns a new [`Share`].  Otherwise the shared data has been
    /// dropped, and so returns `None`.
    ///
    /// [`Share`]: struct.Share.html
    pub fn upgrade(&self) -> Option<Share2<T>> {
        self.weak.upgrade().map(|rc| Share2 { rc })
    }

    /// Return the number of strong references to the shared data
    pub fn strong_count(&self) -> usize {
        self.weak.strong_count()
    }

    /// Return the number of weak references to the shared data,
    /// including this one, or 0 if there are no strong pointers
    /// remaining.
    pub fn weak_count(&self) -> usize {
        self.weak.weak_count()
    }
}

impl<T> Clone for Share2Weak<T> {
    /// Return another weak reference to the shared data
    fn clone(&self) -> Self {
        Self {
            weak: self.weak.clone(),
        }
    }
}
