use crate::queue::FnOnceQueue;
use crate::Stakker;
use std::cell::RefCell;
use std::marker::PhantomData;

thread_local!(
    static QUEUE: RefCell<FnOnceQueue<Stakker>> = RefCell::new(FnOnceQueue::new());
);

// Use *const to make it !Send and !Sync
#[derive(Clone)]
pub struct DeferrerAux(PhantomData<*const u8>);

impl DeferrerAux {
    pub(crate) fn new() -> Self {
        Self(PhantomData)
    }

    pub(crate) fn replace_queue(&mut self, queue: FnOnceQueue<Stakker>) -> FnOnceQueue<Stakker> {
        QUEUE.with(move |qref| qref.replace(queue))
    }

    #[inline]
    pub fn defer(&self, f: impl FnOnce(&mut Stakker) + 'static) {
        QUEUE.with(|qref| qref.borrow_mut().push(f));
    }
}
