use crate::queue::FnOnceQueue;
use crate::{task::Task, Stakker};
use std::cell::{Cell, RefCell};
use std::mem;
use std::rc::Rc;

// Tried using Cell instead of RefCell, swapping out queue and
// swapping it back in, but it works out way more expensive, from
// looking at the assembly.

#[derive(Clone)]
pub struct DeferrerAux(Rc<Inner>);

struct Inner {
    queue: RefCell<FnOnceQueue<Stakker>>,
    task: Cell<Option<Task>>,
}

impl DeferrerAux {
    pub(crate) fn new() -> Self {
        Self(Rc::new(Inner {
            queue: RefCell::new(FnOnceQueue::new()),
            task: Cell::new(None),
        }))
    }

    pub(crate) fn swap_queue(&self, queue: &mut FnOnceQueue<Stakker>) {
        mem::swap(&mut *self.0.queue.borrow_mut(), queue)
    }

    #[inline]
    pub fn defer(&self, f: impl FnOnce(&mut Stakker) + 'static) {
        self.0.queue.borrow_mut().push(f);
    }

    #[inline]
    pub fn task_replace(&self, task: Option<Task>) -> Option<Task> {
        self.0.task.replace(task)
    }
}
