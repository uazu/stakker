use crate::queue::FnOnceQueue;
use crate::{task::Task, Stakker};
use std::cell::{Cell, UnsafeCell};
use std::marker::PhantomData;
use std::mem;

thread_local!(
    static QUEUE: UnsafeCell<FnOnceQueue<Stakker>> = UnsafeCell::new(FnOnceQueue::new());
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
        // Safety: Safe because the methods called within the unsafe
        // region do not do any operations which will attempt to get
        // another mutable reference to the queue

        // Clippy false positive: They don't overlap, guaranteed by
        // `&mut` param outside of `unsafe`
        #[allow(clippy::swap_ptr_to_ref)]
        unsafe {
            // Does nothing if it's being destroyed
            let _ = QUEUE.try_with(move |qref| mem::swap(&mut *qref.get(), queue));
        }
    }

    #[inline]
    pub fn defer(&self, f: impl FnOnce(&mut Stakker) + 'static) {
        // Safety: Safe because the methods called within the unsafe
        // region do not do any operations which will attempt to get
        // another mutable reference to the queue
        unsafe {
            // Does nothing if it's being destroyed
            let _ = QUEUE.try_with(|qref| (*qref.get()).push(f));
        }
    }

    #[inline]
    pub fn task_replace(&self, task: Option<Task>) -> Option<Task> {
        // Return None if it's being destroyed
        TASK.try_with(move |tref| tref.replace(task))
            .unwrap_or(None)
    }
}
