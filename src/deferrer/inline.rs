use crate::queue::FnOnceQueue;
use crate::Stakker;
use std::cell::RefCell;
use std::mem;
use std::rc::Rc;

#[derive(Clone)]
pub struct DeferrerAux {
    queue: Rc<RefCell<FnOnceQueue<Stakker>>>,
}

impl DeferrerAux {
    pub(crate) fn new() -> Self {
        Self {
            queue: Rc::new(RefCell::new(FnOnceQueue::new())),
        }
    }

    pub(crate) fn replace_queue(&mut self, queue: FnOnceQueue<Stakker>) -> FnOnceQueue<Stakker> {
        mem::replace(&mut *self.queue.borrow_mut(), queue)
    }

    #[inline]
    pub fn defer(&self, f: impl FnOnce(&mut Stakker) + 'static) {
        self.queue.borrow_mut().push(f);
    }
}
