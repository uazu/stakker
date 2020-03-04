use crate::queue::FnOnceQueue;
use crate::Stakker;
use std::cell::RefCell;
use std::mem;
use std::rc::Rc;

// TODO: Try using Cell instead of RefCell, swapping out queue and
// swapping it back in.  Check assembly generated to compare.

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

    pub(crate) fn swap_queue(&self, queue: &mut FnOnceQueue<Stakker>) {
        mem::swap(&mut *self.queue.borrow_mut(), queue)
    }

    #[inline]
    pub fn defer(&self, f: impl FnOnce(&mut Stakker) + 'static) {
        self.queue.borrow_mut().push(f);
    }
}
