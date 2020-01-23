use crate::cell::cell::ShareCell;
use crate::Core;
use std::rc::Rc;

// TODO: Option to use MinRc for Share?

/// Ref-counted shared mutable data
///
/// By default borrow-checking of access to the contents is handled at
/// compile-time using a `TCell` or `TLCell` with its owner in `Core`.
/// There are no access overheads at runtime.  (With some feature
/// settings, these would be replaced with `QCell` which does have
/// some overhead.)  Cloning has the normal `Rc` overheads.
///
/// Be careful using this because it can easily lead to dependency on
/// the order of execution of actors.  So treat this as similar in
/// danger level to shared memory between threads or IPC shared memory
/// between processes.  You don't need locking however because if you
/// get a mutable reference to the contents you'll have exclusive
/// access until you give it up thanks to Rust's borrow checker.
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
    /// of the `Share` instance
    #[inline]
    pub fn ro<'a>(&'a self, core: &'a Core) -> &'a T {
        core.sharecell_owner.ro(&self.rc)
    }

    /// Get a read-write (mutable, exclusive) reference to the
    /// contents of the `Share` instance
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
