use crate::queue::FnOnceQueue;
use crate::Stakker;
use std::cell::UnsafeCell;
use std::mem;
use std::rc::Rc;

// Unsafe version which gets rid of RefCell overheads

#[derive(Clone)]
pub struct DeferrerAux {
    queue: Rc<UnsafeCell<FnOnceQueue<Stakker>>>,
}

impl DeferrerAux {
    pub(crate) fn new() -> Self {
        Self {
            queue: Rc::new(UnsafeCell::new(FnOnceQueue::new())),
        }
    }

    pub(crate) fn swap_queue(&self, queue: &mut FnOnceQueue<Stakker>) {
        // Safety: Safe because the methods called within the unsafe
        // region do not do any operations which will attempt to get
        // another mutable reference to the queue
        unsafe { mem::swap(&mut *self.queue.get(), queue) }
    }

    #[inline]
    pub fn defer(&self, f: impl FnOnce(&mut Stakker) + 'static) {
        // Safety: Safe because the methods called within the unsafe
        // region do not do any operations which will attempt to get
        // another mutable reference to the queue
        unsafe { (&mut *self.queue.get()).push(f) };
    }
}
