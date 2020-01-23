// Safe variant of actor: costs ~24 bytes more than minimum possible
use crate::actor::{Prep, State};
use crate::cell::cell::{ActorCell, ActorCellOwner};
use crate::queue::FnOnceQueue;
use crate::{ActorDied, Core, Deferrer, Fwd, Stakker};
use std::cell::Cell;
use std::mem;
use std::rc::Rc;

// Memory overhead on 64-bit including Rc counts is around 56 +
// size-of-A.  Effectively we have to duplicate the State enum
// both inside and outside of ActorCell to make it safe.
struct ActorBox<A> {
    // 16 bytes for Rc, plus ...
    state: Cell<State>,                   // 8
    notify: Cell<Option<Fwd<ActorDied>>>, // 16
    inner: ActorCell<Inner<A>>,           // 8 + A
    deferrer: Deferrer,                   // 0 or 8
}

enum Inner<A> {
    Prep(Prep),
    Ready(A),
    Zombie,
}

pub(crate) struct ActorRc<A: 'static>(Rc<ActorBox<A>>);

impl<A> ActorRc<A> {
    pub fn new(core: &mut Core, notify: Option<Fwd<ActorDied>>) -> Self {
        Self(Rc::new(ActorBox {
            state: Cell::new(State::Prep),
            notify: Cell::new(notify),
            inner: core.actor_maker.cell(Inner::Prep(Prep {
                queue: FnOnceQueue::new(),
            })),
            deferrer: core.deferrer(),
        }))
    }

    #[inline]
    pub fn is_zombie(&self) -> bool {
        self.0.state.get() == State::Zombie
    }
    #[inline]
    pub fn is_prep(&self) -> bool {
        self.0.state.get() == State::Prep
    }

    pub fn to_ready(&self, s: &mut Stakker, val: A) {
        let inner = s.actor_owner.rw(&self.0.inner);
        match mem::replace(inner, Inner::Ready(val)) {
            Inner::Prep(mut prep) => {
                prep.queue.execute(s);
                self.0.state.replace(State::Ready);
            }
            Inner::Ready(_) => panic!("Actor::to_ready() called twice"),
            Inner::Zombie => *inner = Inner::Zombie,
        }
    }
    pub fn to_zombie(&self, s: &mut Stakker) -> Option<Fwd<ActorDied>> {
        self.0.state.replace(State::Zombie);
        *s.actor_owner.rw(&self.0.inner) = Inner::Zombie;
        self.0.notify.replace(None)
    }
    pub fn borrow_ready<'a>(&'a self, o: &'a mut ActorCellOwner) -> Option<&'a mut A> {
        match o.rw(&self.0.inner) {
            Inner::Ready(ref mut val) => Some(val),
            _ => None,
        }
    }
    pub fn borrow_prep<'a>(&'a self, o: &'a mut ActorCellOwner) -> Option<&'a mut Prep> {
        match o.rw(&self.0.inner) {
            Inner::Prep(ref mut prep) => Some(prep),
            _ => None,
        }
    }

    #[inline]
    pub fn defer(&self, f: impl FnOnce(&mut Stakker) + 'static) {
        self.0.deferrer.defer(f);
    }
}

impl<A> Clone for ActorRc<A> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
