use super::DeferrerAux;
use crate::queue::FnOnceQueue;
use crate::Stakker;

/// Lazy defer queue, which can be accessed without a `Core` ref
///
/// `Deferrer` provides a way to defer calls from contexts which don't
/// have access to a `Core` reference.  The most obvious of these is a
/// `Drop` handler.  So when a `Drop` handler needs to queue cleanup
/// actions, keep a `Deferrer` instance in the structure and then
/// those operations can be deferred without problem.  (The size of a
/// `Deferrer` instance is 0 bytes for the global or thread-local
/// implementations, or a `usize` for inline.)
///
/// Obtain a `Deferrer` instance using [`Core::deferrer`].  To use it,
/// call the [`Deferrer::defer`] method with a closure which performs
/// the operation required.  A `Deferrer` instance can also be used in
/// some forms of the [`call!`] macro.
///
/// Note that in cleanup, deferring an action after the main loop has
/// stopped running the `Stakker` queues or after the `Stakker`
/// instance has been dropped will be accepted but the call will never
/// execute.  So make sure that all actors are terminated before the
/// last run of the `Stakker` queues if you need cleanup actions to
/// complete.
///
/// [`Core::deferrer`]: struct.Core.html#method.deferrer
/// [`Deferrer::defer`]: struct.Deferrer.html#method.defer
/// [`call!`]: macro.call.html
pub struct Deferrer(super::DeferrerAux);

impl Deferrer {
    #[inline]
    pub(crate) fn new() -> Self {
        Self(DeferrerAux::new())
    }

    // Replace the queue with a new one, returning the old one
    #[inline]
    pub(crate) fn replace_queue(&mut self, queue: FnOnceQueue<Stakker>) -> FnOnceQueue<Stakker> {
        self.0.replace_queue(queue)
    }

    /// Defer a call to be executed in the main loop at the next
    /// opportunity.  This goes onto the lazy queue.
    #[inline]
    pub fn defer(&self, f: impl FnOnce(&mut Stakker) + 'static) {
        self.0.defer(f);
    }
}

impl Clone for Deferrer {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
