use crate::cell::cell::ShareCell;
use crate::Core;
use std::rc::Rc;

// TODO: Use MinRc for Share?  Or will we need weak references someday?

/// Ref-counted shared mutable data
///
/// This allows synchronous modification of shared state from actor
/// methods.  Note that if you only wish to share immutable data
/// between actors, it's less verbose to simply use `Rc` from the
/// standard library.  Also you might wish to handle very small
/// amounts of shared mutable data with `Rc<Cell>` instead.
///
/// [`Share`] is provided as a compile-time-checked zero-cost
/// alternative to `Rc<RefCell>`.  If [`Share`] didn't exist, the
/// temptation to use `Rc<RefCell>` occasionally would likely be too
/// great.  However `Rc<RefCell>` brings the risk of unexpected
/// runtime panics.  Modification of one piece of code might
/// unexpectedly cause another distant piece of code to crash, but
/// quite possibly only under certain conditions which makes that
/// failure hard to anticipate in testing.  In short `Rc<RefCell>`
/// should be avoided wherever possible.  With [`Share`] everything is
/// checked at compile time.
///
/// Note that using [`Share`], `Rc<RefCell>` or `Rc<Cell>` breaks the
/// actor model.  Using them to share mutable data between actors
/// effectively causes those actors to be bound together in a group.
/// It would be impossible to split one of those actors off to another
/// process or over a remote link.  However the actor model still
/// applies to the group's external interface.
///
/// So this must be used with some knowledge of the trade-offs.  It
/// could easily lead to dependency on the order of execution of
/// actors.  Treat this as similar in danger level to shared memory
/// between threads or IPC shared memory between processes.  You don't
/// need locking however because if you get a mutable reference to the
/// contents you'll have exclusive access until you give it up thanks
/// to Rust's borrow checker.
///
/// To borrow more than one [`Share`] instance at a time, see
/// [`Core::share_rw2`] and [`Core::share_rw3`].
///
/// It's not possible to pass a [`Core`] reference to methods on a
/// [`Share`] item due to borrowing restrictions.  If you need a
/// [`Core`] reference, then use arguments of "`this: &Share<Self>,
/// core: &mut Core`" instead of "`&mut self`", and do `self` access
/// via `this.rw(core)`.  (However if it's getting this complicated,
/// maybe consider whether the shared data should be made into an
/// actor instead, or whether some other approach would be better.)
///
/// By default borrow-checking of access to the contents of the
/// [`Share`] is handled at compile-time using a `TCell` or `TLCell`
/// with its owner in [`Core`], so it is zero-cost and compiles down
/// to a direct pointer access to the contents of a `struct`, just as
/// if the [`Share`] wasn't there.  However cloning a [`Share`] has
/// the normal `Rc` overheads.
///
/// [`Core::share_rw2`]: struct.Core.html#method.share_rw2
/// [`Core::share_rw3`]: struct.Core.html#method.share_rw3
/// [`Core`]: struct.Core.html
/// [`Share`]: struct.Share.html
pub struct Share<T> {
    pub(crate) rc: Rc<ShareCell<T>>,
}

impl<T> Share<T> {
    /// Create a new Share instance
    pub fn new(core: &Core, val: T) -> Self {
        Self {
            rc: Rc::new(core.sharecell_owner.cell(val)),
        }
    }

    /// Get a read-only (immutable, shared) reference to the contents
    /// of the [`Share`] instance.  By default this is a static check,
    /// which compiles down to a direct access.
    ///
    /// [`Share`]: struct.Share.html
    #[inline]
    pub fn ro<'a>(&'a self, core: &'a Core) -> &'a T {
        core.sharecell_owner.ro(&self.rc)
    }

    /// Get a read-write (mutable, exclusive) reference to the
    /// contents of the [`Share`] instance.  By default this is a
    /// static check, which compiles down to a direct access.
    ///
    /// To access more than one [`Share`] instance at the same time,
    /// see [`Core::share_rw2`] and [`Core::share_rw3`].
    ///
    /// [`Core::share_rw2`]: struct.Core.html#method.share_rw2
    /// [`Core::share_rw3`]: struct.Core.html#method.share_rw3
    /// [`Share`]: struct.Share.html
    #[inline]
    pub fn rw<'a>(&'a self, core: &'a mut Core) -> &'a mut T {
        core.sharecell_owner.rw(&self.rc)
    }
}

impl<T> Clone for Share<T> {
    /// Return another reference to the shared data
    fn clone(&self) -> Self {
        Self {
            rc: self.rc.clone(),
        }
    }
}
