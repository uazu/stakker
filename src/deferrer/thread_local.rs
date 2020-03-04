use crate::queue::FnOnceQueue;
use crate::Stakker;
use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::mem;

thread_local!(
    static QUEUE: UnsafeCell<FnOnceQueue<Stakker>> = UnsafeCell::new(FnOnceQueue::new());
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
        unsafe { QUEUE.with(move |qref| mem::swap(&mut *qref.get(), queue)) }
    }

    #[inline]
    pub fn defer(&self, f: impl FnOnce(&mut Stakker) + 'static) {
        // Safety: Safe because the methods called within the unsafe
        // region do not do any operations which will attempt to get
        // another mutable reference to the queue
        unsafe {
            QUEUE.with(|qref| (&mut *qref.get()).push(f));
        }
    }
}
