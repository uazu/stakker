// Variant of ActorRc using our own Rc implementation (MinRc).  Saves
// 16 bytes compared to safe version because we don't need a weak
// count, and we can pack the state into the count value.
//
// TODO: We could save another 8 bytes by using ptr::read/write
// instead of the Inner enum, but make that a separate implementation
// because the unsafe review would be harder (i.e. some people would
// prefer this one).
use crate::actor::{Prep, State};
use crate::cell::cell::{ActorCell, ActorCellOwner};
use crate::queue::FnOnceQueue;
use crate::rc::minrc::{MinRc, StrongCount};
use crate::{ActorDied, Core, Deferrer, Fwd, Stakker};
use std::cell::Cell;
use std::mem;

// If the count reaches the maximum value, it locks there and never
// decreases, leaking the ActorBox memory (which is safe).  However
// the actor can still be terminated which frees up any referenced
// data.
#[derive(Copy, Clone)]
struct Count(usize);
const COUNT_SHIFT: u32 = 2;
const COUNT_INC: usize = 1 << COUNT_SHIFT;
const COUNT_MASK: usize = !(COUNT_INC - 1);
impl StrongCount for Count {
    fn new() -> Self {
        // Start at count of 1
        Self(COUNT_INC + (State::Prep as usize))
    }
    fn inc(self) -> Self {
        if self.0 >= COUNT_MASK {
            self
        } else {
            Self(self.0 + COUNT_INC)
        }
    }
    fn dec(self) -> (Self, bool) {
        if self.0 >= COUNT_MASK || self.0 < COUNT_INC {
            (self, false)
        } else {
            let val = self.0 - COUNT_INC;
            (Self(val), val < COUNT_INC)
        }
    }
    fn get(self) -> usize {
        self.0 >> COUNT_SHIFT
    }
}
impl Count {
    fn set_state(self, state: State) -> Self {
        Self((self.0 & COUNT_MASK) | (state as usize))
    }
    fn is_prep(self) -> bool {
        (self.0 & !COUNT_MASK) == (State::Prep as usize)
    }
    fn is_zombie(self) -> bool {
        (self.0 & !COUNT_MASK) == (State::Zombie as usize)
    }
}

// Memory overhead on 64-bit is around 40 + size-of-A.  We still have
// the State enum info duplicated both inside and outside of ActorCell
// (outside it's packed in Count, inside it's an enum discriminant)
struct ActorBox<A> {
    notify: Cell<Option<Fwd<ActorDied>>>, // 16
    inner: ActorCell<Inner<A>>,           // (0 or 8) + 8 + max(24, A)  # 24 is for FnOnceQueue
    deferrer: Deferrer,                   // 0 or 8
}

enum Inner<A> {
    Prep(Prep),
    Ready(A),
    Zombie,
}

pub(crate) struct ActorRc<A: 'static>(MinRc<Count, ActorBox<A>>);

impl<A> ActorRc<A> {
    pub fn new(core: &mut Core, notify: Option<Fwd<ActorDied>>) -> Self {
        Self(MinRc::new(ActorBox {
            notify: Cell::new(notify),
            inner: core.actor_maker.cell(Inner::Prep(Prep {
                queue: FnOnceQueue::new(),
            })),
            deferrer: core.deferrer(),
        }))
    }

    // Accessors for convenience
    #[inline]
    fn count<R>(&self, cb: impl FnOnce(&Cell<Count>) -> R) -> R {
        self.0.count(cb)
    }
    #[inline]
    fn inner(&self) -> &ActorCell<Inner<A>> {
        &self.0.inner().inner
    }
    #[inline]
    fn notify(&self) -> &Cell<Option<Fwd<ActorDied>>> {
        &self.0.inner().notify
    }
    #[inline]
    fn deferrer(&self) -> &Deferrer {
        &self.0.inner().deferrer
    }

    #[inline]
    pub fn is_zombie(&self) -> bool {
        self.count(|c| c.get().is_zombie())
    }

    #[inline]
    pub fn is_prep(&self) -> bool {
        self.count(|c| c.get().is_prep())
    }

    pub fn to_ready(&self, s: &mut Stakker, val: A) {
        let inner = s.actor_owner.rw(self.inner());
        match mem::replace(inner, Inner::Ready(val)) {
            Inner::Prep(mut prep) => {
                prep.queue.execute(s);
                self.count(|c| c.replace(c.get().set_state(State::Ready)));
            }
            Inner::Ready(_) => panic!("Actor::to_ready() called twice"),
            Inner::Zombie => *inner = Inner::Zombie,
        }
    }

    pub fn to_zombie(&self, s: &mut Stakker) -> Option<Fwd<ActorDied>> {
        self.count(|c| c.replace(c.get().set_state(State::Zombie)));
        *s.actor_owner.rw(self.inner()) = Inner::Zombie;
        self.notify().replace(None)
    }

    pub fn borrow_ready<'a>(&'a self, o: &'a mut ActorCellOwner) -> Option<&'a mut A> {
        match o.rw(self.inner()) {
            Inner::Ready(ref mut val) => Some(val),
            _ => None,
        }
    }

    pub fn borrow_prep<'a>(&'a self, o: &'a mut ActorCellOwner) -> Option<&'a mut Prep> {
        match o.rw(self.inner()) {
            Inner::Prep(ref mut prep) => Some(prep),
            _ => None,
        }
    }

    #[inline]
    pub fn defer(&self, f: impl FnOnce(&mut Stakker) + 'static) {
        self.deferrer().defer(f);
    }
}

impl<A> Clone for ActorRc<A> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
