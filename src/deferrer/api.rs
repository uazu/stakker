use super::DeferrerAux;
use crate::queue::FnOnceQueue;
use crate::{task::Task, Stakker};

/// Defer queue which can be accessed without a [`Core`] ref
///
/// [`Deferrer`] provides a way to defer calls from contexts which
/// don't have access to a [`Core`] reference.  The most obvious of
/// these is a `Drop` handler.  So when a `Drop` handler needs to
/// queue cleanup actions, keep a [`Deferrer`] instance in the
/// structure and then those operations can be deferred without
/// problem.  (The size of a [`Deferrer`] instance is 0 bytes for the
/// global or thread-local implementations, or a `usize` for inline.)
///
/// Obtain a [`Deferrer`] instance using [`Core::deferrer`].  To use
/// it, call the [`Deferrer::defer`] method with a closure which
/// performs the operation required.  Note that all [`Actor`]
/// instances have a [`Deferrer`] built in which can be used from
/// outside the actor as [`Actor::defer`].
///
/// Note that in final shutdown of a **Stakker** system, deferring an
/// action after the main loop has stopped running the [`Stakker`]
/// queues or after the [`Stakker`] instance has been dropped will be
/// accepted but the call will never execute.  So make sure that all
/// actors are terminated before the last run of the [`Stakker`]
/// queues if you need cleanup actions to complete.
///
/// [`Actor::defer`]: struct.Actor.html#method.defer
/// [`Actor`]: struct.Actor.html
/// [`Core::deferrer`]: struct.Core.html#method.deferrer
/// [`Core`]: struct.Core.html
/// [`Deferrer::defer`]: struct.Deferrer.html#method.defer
/// [`Deferrer`]: struct.Deferrer.html
/// [`Stakker`]: struct.Stakker.html
pub struct Deferrer(super::DeferrerAux);

impl Deferrer {
    #[inline]
    pub(crate) fn new() -> Self {
        Self(DeferrerAux::new())
    }

    // Swap out the queue
    #[inline]
    pub(crate) fn swap_queue(&self, queue: &mut FnOnceQueue<Stakker>) {
        self.0.swap_queue(queue);
    }

    // Set the queue and discard the old value
    #[inline]
    pub(crate) fn set_queue(&self, mut queue: FnOnceQueue<Stakker>) {
        self.0.swap_queue(&mut queue);
    }

    /// Defer a call to be executed in the main loop at the next
    /// opportunity.
    #[inline]
    pub fn defer(&self, f: impl FnOnce(&mut Stakker) + 'static) {
        self.0.defer(f);
    }

    /// Replace the current task, returning the old value.
    #[inline]
    pub(crate) fn task_replace(&self, task: Option<Task>) -> Option<Task> {
        self.0.task_replace(task)
    }
}

impl Clone for Deferrer {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
