use crate::cell::cell::ShareCell;
use crate::Core;
use std::rc::Rc;

// TODO: Use MinRc for Share?  Or will we need weak references someday?

/// Ref-counted shared mutable data
///
/// This allows synchronous modification of shared state from actor
/// methods.  If you only wish to share immutable data, use
/// `std::rc::Rc` instead which avoids having to add `.ro(cx)` to
/// access the data.
///
/// Note that this breaks the actor model.  Using it to share mutable
/// data between actors effectively causes those actors to be bound
/// together in a group.  It would be impossible to split one of those
/// actors off to another process or over a remote link, or to test
/// them independently.  However the actor model still applies to the
/// group's external interface.  So this must be used with some
/// knowledge of the trade-offs.
///
/// Be careful using this because it can easily lead to dependency on
/// the order of execution of actors.  So treat this as similar in
/// danger level to shared memory between threads or IPC shared memory
/// between processes.  You don't need locking however because if you
/// get a mutable reference to the contents you'll have exclusive
/// access until you give it up thanks to Rust's borrow checker.
///
/// `self` methods on a [`Share`] item can't be passed a [`Core`]
/// reference due to borrowing restrictions.  If you need a [`Core`]
/// reference, then use "`this: &Share<Self>, core: &mut Core`" as
/// arguments instead of "`&mut self`", and do `self` access via
/// `this.rw(core)`.
///
/// By default borrow-checking of access to the contents is handled at
/// compile-time using a `TCell` or `TLCell` with its owner in
/// [`Core`].  There are no access overheads at runtime by default.
/// Cloning has the normal `Rc` overheads.
///
/// [`Core`]: struct.Core.html
/// [`Share`]: struct.Share.html
pub struct Share<T> {
    rc: Rc<ShareCell<T>>,
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
