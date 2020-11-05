// Safe variant of actor: costs ~16 bytes more than minimum possible
use crate::actor::{Prep, State};
use crate::cell::cell::{ActorCell, ActorCellOwner};
use crate::queue::FnOnceQueue;
use crate::rc::count::CountAndState;
use crate::{Core, Deferrer, LogID, Ret, Stakker, StopCause};
use std::cell::Cell;
use std::mem;
use std::rc::Rc;

// Memory overhead on 64-bit including Rc counts is around 56 +
// size-of-A.  Effectively we have to duplicate the State enum
// both inside and outside of ActorCell to make it safe.
struct ActorBox<A> {
    // 16 bytes for Rc, plus ...
    strong: Cell<CountAndState>,          // 8
    notify: Cell<Option<Ret<StopCause>>>, // 16
    deferrer: Deferrer,                   // 0 or 8
    #[cfg(feature = "logger")]
    id: LogID,
    inner: ActorCell<Inner<A>>, // 8 + A
}

enum Inner<A> {
    Prep(Prep),
    Ready(A),
    Zombie,
}

pub(crate) struct ActorRc<A: 'static>(Rc<ActorBox<A>>);

impl<A> ActorRc<A> {
    pub fn new(core: &mut Core, notify: Option<Ret<StopCause>>, _parent_id: LogID) -> Self {
        Self(Rc::new(ActorBox {
            strong: Cell::new(CountAndState::new()),
            notify: Cell::new(notify),
            #[cfg(feature = "logger")]
            id: core.log_span_open(std::any::type_name::<A>(), _parent_id, |_| {}),
            inner: core.actor_maker.cell(Inner::Prep(Prep {
                queue: FnOnceQueue::new(),
            })),
            deferrer: core.deferrer(),
        }))
    }

    #[inline]
    pub fn id(&self) -> LogID {
        #[cfg(feature = "logger")]
        {
            self.0.id
        }
        #[cfg(not(feature = "logger"))]
        0
    }

    #[inline]
    fn strong(&self) -> &Cell<CountAndState> {
        &self.0.strong
    }

    pub fn strong_inc(&self) {
        self.strong().replace(self.strong().get().inc());
    }
    pub fn strong_dec(&self) -> bool {
        let (count, went_to_zero) = self.strong().get().dec();
        self.strong().replace(count);
        went_to_zero
    }

    #[inline]
    pub fn is_zombie(&self) -> bool {
        self.strong().get().is_zombie()
    }
    #[inline]
    pub fn is_prep(&self) -> bool {
        self.strong().get().is_prep()
    }

    pub fn to_ready(&self, s: &mut Stakker, val: A) {
        let inner = s.actor_owner.rw(&self.0.inner);
        match mem::replace(inner, Inner::Ready(val)) {
            Inner::Prep(mut prep) => {
                self.strong()
                    .replace(self.strong().get().set_state(State::Ready));
                prep.queue.execute(s);
            }
            Inner::Ready(_) => panic!("Actor::to_ready() called twice"),
            Inner::Zombie => *inner = Inner::Zombie,
        }
    }
    pub fn to_zombie(&self, s: &mut Stakker) -> Option<Ret<StopCause>> {
        self.strong()
            .replace(self.strong().get().set_state(State::Zombie));
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
    pub fn access_deferrer(&self) -> &Deferrer {
        &self.0.deferrer
    }
}

impl<A> Clone for ActorRc<A> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
