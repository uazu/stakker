// Variant of ActorRc using our own Rc implementation (MinRc).  Saves
// 8 bytes compared to safe version because we can pack the state into
// the count value.
//
// TODO: We could save another 8 bytes by using ptr::read/write
// instead of the Inner enum.  However that is impossible right now
// because `union` only supports Copy types.  Also maybe better make
// that a separate implementation because the unsafe review would be
// harder (i.e. some people would prefer this one).

use crate::actor::{Prep, State};
use crate::cell::cell::{ActorCell, ActorCellOwner};
use crate::queue::FnOnceQueue;
use crate::rc::count::CountAndState;
use crate::rc::minrc::MinRc;
use crate::{Core, Deferrer, Fwd, Ret, Stakker, StopCause};
use static_assertions::{assert_eq_size, const_assert};
use std::cell::Cell;
use std::mem;
use std::mem::size_of;

// Memory overhead on 64-bit is around 40 to 64 + size-of-A.  We still
// have the State enum info duplicated both inside and outside of
// ActorCell (outside it's packed in CountAndState, inside it's an
// 8-byte enum discriminant).
//
// The decision about what goes inside or outside the ActorCell is
// critical.  Anything outside can be accessed without needing a
// Stakker reference, e.g. drop() or clone() or owned() or any other
// calls that don't have Stakker access.
//
// - `strong` is outside because owned/drop need it.  It could be put
// inside but that would rely on slower deferred calls to do inc/dec.
//
// - `deferrer` is outside because drop needs it.  It is also useful
// for user-side drop handlers, through the Actor::defer call.
//
// - `notify` is outside because of separation of concerns.  Putting
// it inside and making it part of the actor interface, and letting
// the actor optimise that might be possible, but this makes testing
// harder since every actor might handle `notify` differently, or even
// have it hardcoded.  So it is best that notify remains outside the
// implementation of the actor.
struct ActorBox<A> {
    // 8, plus ref-counts outside: 8 or 16
    strong: Cell<CountAndState>,
    // 0 or 8
    deferrer: Deferrer,
    // 16
    notify: Cell<Option<Ret<StopCause>>>,
    // CellID(0 or 8) + Enum-determinant(8) + Data(max(24, A))  # 24 is for FnOnceQueue
    inner: ActorCell<Inner<A>>,
}

assert_eq_size!(Cell<Option<Fwd<StopCause>>>, [usize; 2]);
const_assert!(size_of::<Deferrer>() <= size_of::<usize>());

enum Inner<A> {
    Prep(Prep),
    Ready(A),
    Zombie,
}

pub(crate) struct ActorRc<A: 'static>(MinRc<ActorBox<A>>);

impl<A> ActorRc<A> {
    pub fn new(core: &mut Core, notify: Option<Ret<StopCause>>) -> Self {
        Self(MinRc::new(ActorBox {
            strong: Cell::new(CountAndState::new()),
            deferrer: core.deferrer(),
            notify: Cell::new(notify),
            inner: core.actor_maker.cell(Inner::Prep(Prep {
                queue: FnOnceQueue::new(),
            })),
        }))
    }

    // Accessors for convenience
    #[inline]
    fn strong(&self) -> &Cell<CountAndState> {
        &self.0.inner().strong
    }
    #[inline]
    fn inner(&self) -> &ActorCell<Inner<A>> {
        &self.0.inner().inner
    }
    #[inline]
    fn notify(&self) -> &Cell<Option<Ret<StopCause>>> {
        &self.0.inner().notify
    }
    #[inline]
    fn deferrer(&self) -> &Deferrer {
        &self.0.inner().deferrer
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
        let inner = s.actor_owner.rw(self.inner());
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
    pub fn access_deferrer(&self) -> &Deferrer {
        self.deferrer()
    }
}

impl<A> Clone for ActorRc<A> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
