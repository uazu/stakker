//! Support code for asynchronous tasks

use crate::cell::cell::ActorCell;
use crate::{Core, Deferrer, Stakker};
use std::pin::Pin;
use std::rc::Rc;

/// An asynchronous task reference
///
/// **Stakker** provides only a very thin wrapping to enable tasks.
/// Full support is provided in a separate crate
/// (**stakker_async_await**).  A task is represented as an
/// implementation of the [`TaskTrait`](trait.TaskTrait.html) trait
/// which is called when the task needs to be resumed.  It is expected
/// that even if the task cannot advance, it will accept a spurious
/// call gracefully.  Spurious calls may be generated for example if
/// the task is woken from another thread.
///
/// It is guaranteed that the task is resumed directly from the main
/// loop, not within any actor call nor any deeper down the stack.
/// This may be important in order to avoid `RefCell` panics, or to
/// allow the use of `UnsafeCell`.  Internally this is statically
/// guaranteed by using an `ActorCell` to wrap the closure.  This code
/// also handles the pinning required for running futures.
///
/// Deeper down the stack whilst the task is running, it is possible
/// to get a reference to the running task by using
/// [`Task::from_context`](#method.from_context).  This may be
/// used to implement efficient same-thread wakers.
#[derive(Clone)]
pub struct Task {
    rc: Rc<ActorCell<Pin<Box<dyn TaskTrait>>>>,
}

impl Task {
    /// Create a new task from an implementation of the
    /// [`TaskTrait`](trait.TaskTrait.html), and return a reference to
    /// it.  (`Task` is like an `Rc` reference to the task.)  The
    /// `resume` method will be called each time the task needs to be
    /// resumed.
    pub fn new(core: &mut Core, inner: impl TaskTrait + 'static) -> Self {
        Self {
            rc: Rc::new(core.actor_maker.cell(Box::pin(inner))),
        }
    }

    /// Resume execution of the task.  The task will advance its state
    /// as much as possible, and then return.  It's intended that
    /// whatever poll-handler caused the task to yield will have saved
    /// a reference to this task to keep it alive until it can be
    /// woken again.
    pub fn resume(&mut self, s: &mut Stakker) {
        s.deferrer.task_replace(Some(self.clone()));
        s.actor_owner.rw(&self.rc).as_mut().resume(&mut s.core);
        s.deferrer.task_replace(None);
    }

    /// Obtain the [`Task`](struct.Task.html) reference of the
    /// currently-running task from the provided
    /// [`Deferrer`](../struct.Deferrer.html), if a task is currently
    /// running.
    pub fn from_context(deferrer: &Deferrer) -> Option<Self> {
        let v0 = deferrer.task_replace(None);
        let v1 = v0.clone();
        deferrer.task_replace(v0);
        v1
    }
}

/// Trait that tasks must implement
pub trait TaskTrait {
    /// Resume execution of the task.  This must handle spurious calls
    /// gracefully, even if nothing has changed.
    fn resume(self: Pin<&mut Self>, core: &mut Core);
}

// Not useful yet, as there's no way to pass an owner-borrow through
// `Future::poll`
//
// /// Statically-checked cell type for use within task-related code
// ///
// /// This requires access to `&mut Stakker` in order to borrow the cell
// /// contents, so cannot be used within actor code or anywhere else
// /// where a Stakker reference is not available.
// pub struct TaskCell<T: 'static>(ActorCell<T>);
//
// impl<T: 'static> TaskCell<T> {
//     /// Create a new TaskCell
//     pub fn new(core: &mut Core, value: T) -> Self {
//         Self(core.actor_maker.cell(value))
//     }
//
//     // Get a mutable reference to the cell contents
//     pub fn rw<'a>(&'a self, s: &'a mut Stakker) -> &'a mut T {
//         s.actor_owner.rw(&self.0)
//     }
// }
