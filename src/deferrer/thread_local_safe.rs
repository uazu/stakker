use crate::queue::FnOnceQueue;
use crate::{task::Task, Stakker};
use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::mem;

thread_local!(
    static QUEUE: RefCell<FnOnceQueue<Stakker>> = RefCell::new(FnOnceQueue::new());
    static TASK: Cell<Option<Task>> = Cell::new(None);
);

// Use *const to make it !Send and !Sync
#[derive(Clone)]
pub struct DeferrerAux(PhantomData<*const u8>);

impl DeferrerAux {
    pub(crate) fn new() -> Self {
        Self(PhantomData)
    }

    pub(crate) fn swap_queue(&self, queue: &mut FnOnceQueue<Stakker>) {
        // Does nothing if it's being destroyed
        let _ = QUEUE.try_with(move |qref| mem::swap(&mut *qref.borrow_mut(), queue));
    }

    #[inline]
    pub fn defer(&self, f: impl FnOnce(&mut Stakker) + 'static) {
        // Does nothing if it's being destroyed
        let _ = QUEUE.try_with(|qref| qref.borrow_mut().push(f));
    }

    #[inline]
    pub fn task_replace(&self, task: Option<Task>) -> Option<Task> {
        // Return None if it's being destroyed
        TASK.try_with(move |tref| tref.replace(task))
            .unwrap_or(None)
    }
}
