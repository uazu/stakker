use crate::queue::FnOnceQueue;
use crate::{task::Task, Stakker};
use std::cell::{Cell, UnsafeCell};
use std::mem;
use std::rc::Rc;

// Unsafe version which gets rid of RefCell overheads

#[derive(Clone)]
pub struct DeferrerAux(Rc<Inner>);

struct Inner {
    queue: UnsafeCell<FnOnceQueue<Stakker>>,
    task: Cell<Option<Task>>,
}

impl DeferrerAux {
    pub(crate) fn new() -> Self {
        Self(Rc::new(Inner {
            queue: UnsafeCell::new(FnOnceQueue::new()),
            task: Cell::new(None),
        }))
    }

    pub(crate) fn swap_queue(&self, queue: &mut FnOnceQueue<Stakker>) {
        // Safety: Safe because the methods called within the unsafe
        // region do not do any operations which will attempt to get
        // another mutable reference to the queue
        unsafe { mem::swap(&mut *self.0.queue.get(), queue) }
    }

    #[inline]
    pub fn defer(&self, f: impl FnOnce(&mut Stakker) + 'static) {
        // Safety: Safe because the methods called within the unsafe
        // region do not do any operations which will attempt to get
        // another mutable reference to the queue
        unsafe { (&mut *self.0.queue.get()).push(f) };
    }

    #[inline]
    pub fn task_replace(&self, task: Option<Task>) -> Option<Task> {
        self.0.task.replace(task)
    }
}
