use crate::queue::FnOnceQueue;
use crate::rc::ActorRc;
use crate::{ret, ret_some_do, Core, Deferrer, LogID, Ret, Stakker};
use slab::Slab;
use std::error::Error;
use std::fmt;
use std::ops::{Deref, DerefMut};

/// An owning ref-counting reference to an actor
///
/// This not only keeps a reference to the actor, but it will also
/// automatically terminate the actor when the last owning reference
/// is dropped.  This dereferences to a normal [`Actor`] reference, so
/// when `.clone()` is called on it, a normal non-owning reference
/// results.  To get another owning reference, use `.owned()`.
///
/// [`Actor`]: struct.Actor.html
pub struct ActorOwn<A: 'static> {
    actor: Actor<A>,
}

impl<A: 'static> ActorOwn<A> {
    /// Create a new uninitialised actor of the given type.  This is
    /// in the **Prep** state.  The reference can be cloned and passed
    /// around and used to create [`Fwd`] or [`Ret`] instances.
    /// However all calls to the actor will be delayed until the actor
    /// moves into the **Ready** state.
    ///
    /// `notify` is the [`StopCause`] return, which is called when the
    /// actor terminates.  `parent_id` is the logging-ID of the parent
    /// actor if known, or else 0.
    ///
    /// See macros [`actor!`] and [`actor_new!`] for help with
    /// creating and initialising actors.
    ///
    /// [`Fwd`]: struct.Fwd.html
    /// [`Ret`]: struct.Ret.html
    /// [`StopCause`]: enum.StopCause.html
    /// [`actor!`]: macro.actor.html
    /// [`actor_new!`]: macro.actor_new.html
    #[inline]
    pub fn new(core: &mut Core, notify: Ret<StopCause>, parent_id: LogID) -> ActorOwn<A> {
        Self::construct(Actor {
            rc: ActorRc::new(core, Some(notify), parent_id),
        })
    }

    fn construct(actor: Actor<A>) -> Self {
        actor.rc.strong_inc();
        Self { actor }
    }

    /// Create an additional owning reference to this actor.  When the
    /// last owning reference is dropped, the actor is terminated,
    /// even when there are other references active.
    pub fn owned(&self) -> ActorOwn<A> {
        ActorOwn::construct(self.actor.clone())
    }

    /// Kill actor, moving to **Zombie** state and dropping the
    /// contained actor `Self` value.  The actor can never return from
    /// the **Zombie** state.  The provided error is used to generate
    /// an `StopCause::Killed` instance, which is passed to the
    /// [`StopCause`] handler set up when the actor was created.
    ///
    /// [`StopCause`]: enum.StopCause.html
    pub fn kill(&self, s: &mut Stakker, err: Box<dyn Error>) {
        self.actor.terminate(s, StopCause::Killed(err));
    }

    /// Kill actor, moving to **Zombie** state and dropping the
    /// contained actor `Self` value.  The actor can never return from
    /// the **Zombie** state.  The provided error string is used to
    /// generate an `StopCause::Killed` instance, which is passed to
    /// the [`StopCause`] handler set up when the actor was created.
    ///
    /// [`StopCause`]: enum.StopCause.html
    pub fn kill_str(&self, s: &mut Stakker, err: &'static str) {
        self.actor
            .terminate(s, StopCause::Killed(Box::new(StrError(err))));
    }

    /// Kill actor, moving to **Zombie** state and dropping the
    /// contained actor `Self` value.  The actor can never return from
    /// the **Zombie** state.  The provided error string is used to
    /// generate an `StopCause::Killed` instance, which is passed to
    /// the [`StopCause`] handler set up when the actor was created.
    ///
    /// [`StopCause`]: enum.StopCause.html
    pub fn kill_string(&self, s: &mut Stakker, err: impl Into<String>) {
        self.actor
            .terminate(s, StopCause::Killed(Box::new(StringError(err.into()))));
    }

    /// Convert into an anonymous owning reference.  See
    /// [`ActorOwnAnon`].
    ///
    /// [`ActorOwnAnon`]: struct.ActorOwnAnon.html
    pub fn anon(self) -> ActorOwnAnon {
        ActorOwnAnon::new(self)
    }
}

impl<A: 'static> Deref for ActorOwn<A> {
    type Target = Actor<A>;

    fn deref(&self) -> &Actor<A> {
        &self.actor
    }
}

impl<A: 'static> DerefMut for ActorOwn<A> {
    fn deref_mut(&mut self) -> &mut Actor<A> {
        &mut self.actor
    }
}

impl<A: 'static> Drop for ActorOwn<A> {
    fn drop(&mut self) {
        let went_to_zero = self.actor.rc.strong_dec();
        if went_to_zero {
            let actor = self.actor.clone();
            self.actor
                .defer(move |s| actor.terminate(s, StopCause::Dropped));
        }
    }
}

/// An owning ref-counting reference to an anonymous actor
///
/// The purpose of this is to allow owning any one of a class of other
/// actors without knowing the exact type.  The only operation this
/// supports is dropping an owning reference to an actor when this
/// value is dropped.  It can be used in combination with a [`Fwd`]
/// instance to support plugging a variety of different actors into a
/// standard interface, without needing traits.  As an alternative,
/// see [`actor_of_trait!`].
///
/// Example, using [`ActorOwn::anon`] to create the anonymous
/// reference:
///
/// ```
/// # use stakker::*;
/// # use std::time::Instant;
/// struct Cat;
/// impl Cat {
///     fn init(_: CX![]) -> Option<Self> {
///         Some(Self)
///     }
///     fn sound(&mut self, _: CX![]) {
///         println!("Miaow");
///     }
/// }
///
/// struct Dog;
/// impl Dog {
///     fn init(_: CX![]) -> Option<Self> {
///         Some(Self)
///     }
///     fn sound(&mut self, _: CX![]) {
///         println!("Woof");
///     }
/// }
///
/// // This function doesn't know whether it's getting a cat or a dog,
/// // but it can still call it and drop it when it has finished
/// pub fn call_and_drop(sound: Fwd<()>, own: ActorOwnAnon) {
///     fwd!([sound]);
/// }
///
/// let mut stakker = Stakker::new(Instant::now());
/// let s = &mut stakker;
///
/// let cat = actor!(s, Cat::init(), ret_nop!());
/// call_and_drop(fwd_to!([cat], sound() as ()), cat.anon());
///
/// let dog = actor!(s, Dog::init(), ret_nop!());
/// call_and_drop(fwd_to!([dog], sound() as ()), dog.anon());
///
/// s.run(Instant::now(), false);
/// ```
///
/// [`ActorOwn::anon`]: struct.ActorOwn.html#method.anon
/// [`Fwd`]: struct.Fwd.html
/// [`actor_of_trait!`]: macro.actor_of_trait.html
pub struct ActorOwnAnon(Box<dyn ActorOwnAnonTrait>);
trait ActorOwnAnonTrait {}
impl<T: 'static> ActorOwnAnonTrait for ActorOwn<T> {}
impl ActorOwnAnon {
    pub fn new<T: 'static>(actorown: ActorOwn<T>) -> Self {
        Self(Box::new(actorown))
    }
}

/// A ref-counting reference to an actor
///
/// This may be cloned to get another reference to the same actor.
///
/// # Example implementation of an minimal actor
///
/// ```
///# use stakker::{call, Cx, CX, Ret, ret};
/// struct Light {
///     on: bool,
/// }
/// impl Light {
///     pub fn init(_cx: CX![], on: bool) -> Option<Self> {
///         Some(Self { on })
///     }
///     pub fn set(&mut self, _cx: CX![], on: bool) {
///         self.on = on;
///     }
///     pub fn get(&self, cx: CX![], ret: Ret<bool>) {
///         ret!([ret], self.on);
///     }
/// }
/// ```
///
/// # Internal state of an actor
///
/// An actor may be in one of three states: **Prep**, **Ready** or
/// **Zombie**.  This is independent of whether there are still
/// references to it.  The actor only has a `self` value in the
/// **Ready** state.
///
///
/// # Lifecycle of an actor
///
/// **"Prep" state**: As soon as [`ActorOwn::new`] returns, the actor
/// exists and is in the **Prep** state.  It has an actor reference,
/// but it does not yet have a `self` value.  It is possible to create
/// [`Fwd`] or [`Ret`] instances referring to methods in this actor,
/// and to pass the actor reference to other actors.  However any
/// normal actor calls will be queued up until the actor becomes
/// ready.  The only calls that are permitted on the actor in the
/// **Prep** state are calls to static methods with the signature `fn
/// method(cx: CX![], ...) -> Option<Self>`.
///
/// An actor call should be made to one of the static methods on the
/// actor to start the process of initialising it.  Initialisation may
/// be immediate, or it may start an asynchronous process (for
/// example, making a connection to a remote server).  Each call to a
/// static method may schedule callbacks to other static methods.
/// Eventually, one of these methods should either return
/// `Some(value)` or else fail the actor.
///
/// **"Ready" state**: As soon as a `Self` value is returned from a
/// **Prep** method, this is installed as the actor's `self` value,
/// and the actor moves to the **Ready** state.  Any calls to the
/// actor that were queued up whilst it was in the **Prep** state are
/// flushed and executed at this point.  Whilst in the **Ready**
/// state, the actor can only execute calls to methods with the
/// signatures `fn method(&mut self, cx: CX![], ...)` or `fn
/// method(&self, cx: CX![], ...)`.  Any **Prep**-style calls that
/// occur will be dropped.  Now deferred calls from timers or other
/// actors will execute immediately on reaching the front of the
/// queue.  This is the normal operating mode of the actor.
///
/// **"Zombie" state**: The **Zombie** state can be entered for
/// various reasons.  The first is normal shutdown of the actor
/// through the [`Cx::stop`] method.  The second is failure of the
/// actor through the [`Cx::fail`], [`Cx::fail_str`] or
/// [`Cx::fail_string`] methods.  The third is through being killed
/// externally through the [`ActorOwn::kill`], [`ActorOwn::kill_str`]
/// or [`ActorOwn::kill_string`] methods.  The fourth is through the
/// last owning reference to the actor being dropped.  Termination of
/// the actor is notified to the [`StopCause`] handler provided to the
/// [`ActorOwn::new`] method when the actor was created.
///
/// Once an actor is a **Zombie** it never leaves that state.  The
/// `self` value is dropped and all resources are released.
/// References remain valid until the last reference is dropped, but
/// the [`Actor::is_zombie`] method will return true.  Any calls
/// queued for the actor will be dropped.  (Note that if you need to
/// have a very long-lived actor but you also need to restart the
/// actor on failure, consider having one actor wrap another.)
///
///
/// # Ownership of an actor and automatic termination on drop
///
/// There are two forms of reference to an actor: [`ActorOwn`]
/// instances are strong references, and [`Actor`], [`Fwd`] and
/// [`Ret`] instances are weak references.  When the last [`ActorOwn`]
/// reference is dropped, the actor will be terminated automatically.
/// After termination, the actor `self` state data is dropped, but the
/// actor stays in memory in the **Zombie** state until the final weak
/// references are dropped.
///
/// The normal approach is to use [`ActorOwn`] references to control
/// the termination of the actor.  If the actor relationships are
/// designed such that there can be no cycles in the [`ActorOwn`]
/// graph (e.g. it will always be a simple tree), then cleanup is safe
/// and straightforward even in the presence of [`Actor`], [`Fwd`] or
/// [`Ret`] reference cycles.  Everything will be cleaned up correctly
/// by simply dropping things when they are no longer required.  This
/// means that when an actor fails with [`fail!`], all of its child
/// actors will automatically be terminated too.
///
/// This also handles the case where many actors hold owning
/// references to a common shared actor.  The common actor will only
/// be terminated when the last [`ActorOwn`] reference is dropped.
///
/// However if the situation doesn't fit the "no cyclic owning
/// references" model (i.e. ownership cannot be represented as a
/// directed-acyclic-graph), then other termination strategies are
/// possible, since an actor can be terminated externally using a
/// [`kill!`] operation.  This would normally be a design decision to
/// solve some particularly difficult problem, and in this case the
/// coder must ensure proper cleanup occurs using [`kill!`] instead of
/// simply relying on drop.
///
/// [`Actor::is_zombie`]: struct.Actor.html#method.is_zombie
/// [`ActorOwn::kill_str`]: struct.ActorOwn.html#method.kill_str
/// [`ActorOwn::kill_string`]: struct.ActorOwn.html#method.kill_string
/// [`ActorOwn::kill`]: struct.ActorOwn.html#method.kill
/// [`ActorOwn::new`]: struct.ActorOwn.html#method.new
/// [`ActorOwn`]: struct.ActorOwn.html
/// [`Actor`]: struct.Actor.html
/// [`Cx::fail_str`]: struct.Cx.html#method.fail_str
/// [`Cx::fail_string`]: struct.Cx.html#method.fail_string
/// [`Cx::fail`]: struct.Cx.html#method.fail
/// [`Cx::stop`]: struct.Cx.html#method.stop
/// [`Fwd`]: struct.Fwd.html
/// [`Ret`]: struct.Ret.html
/// [`StopCause`]: enum.StopCause.html
/// [`fail!`]: macro.fail.html
/// [`kill!`]: macro.kill.html
pub struct Actor<A: 'static> {
    rc: ActorRc<A>,
}

#[derive(Eq, PartialEq, Copy, Clone)]
pub(crate) enum State {
    // In 'prep' state, all it does is accumulate calls made to the
    // actor ready to be executed when it enters 'ready'
    Prep = 0,
    // In 'ready' state, actor has stored state `A` and is working,
    // receiving calls
    Ready = 1,
    // In 'zombie' state, contents have been dropped, and the only the
    // empty ActorBox remains allocated waiting for references to go
    // away.
    Zombie = 2,
}

pub(crate) struct Prep {
    pub(crate) queue: FnOnceQueue<Stakker>,
}

impl<A> Actor<A> {
    /// Check whether the actor is a zombie.  Note that this call is
    /// less useful than it appears, since the actor may become a
    /// zombie between the time you make this call and whatever
    /// asynchronous operation follows.  It is better to make a call
    /// with a [`ret_to!`] callback which will send back a `None` if
    /// the actor has died or if the actor drops the [`Ret`] for any
    /// other reason.
    ///
    /// [`Ret`]: struct.Ret.html
    /// [`ret_to!`]: macro.ret_to.html
    pub fn is_zombie(&self) -> bool {
        self.rc.is_zombie()
    }

    // Initialise actor, taking it from **Prep** to **Ready**.  Panics
    // if called twice on the same actor.  Runs the queue of calls
    // waiting for this actor to be initialised.  Has no effect if the
    // actor is already a **Zombie**.
    fn to_ready(&self, s: &mut Stakker, val: A) {
        self.rc.to_ready(s, val);
    }

    // Terminate actor, moving to **Zombie** state and dropping
    // contained value if any.  The actor can never return from the
    // **Zombie** state.  The `died` value is sent to the
    // [`StopCause`] handler set up when the actor was created.
    fn terminate(&self, s: &mut Stakker, died: StopCause) {
        if let Some(notify) = self.rc.to_zombie(s) {
            self.log_termination(s, &died);
            notify.ret(died);
        }
    }

    fn log_termination(&self, core: &mut Core, died: &StopCause) {
        if cfg!(feature = "logger") {
            match died {
                StopCause::Stopped => core.log_span_close(self.id(), format_args!(""), |_| {}),
                StopCause::Failed(ref e) => {
                    core.log_span_close(self.id(), format_args!("{}", e), |out| {
                        out.kv_null(Some("failed"))
                    })
                }
                StopCause::Killed(ref e) => {
                    core.log_span_close(self.id(), format_args!("{}", e), |out| {
                        out.kv_null(Some("killed"))
                    })
                }
                StopCause::Dropped => core.log_span_close(self.id(), format_args!(""), |out| {
                    out.kv_null(Some("dropped"))
                }),
                StopCause::Lost => core
                    .log_span_close(self.id(), format_args!(""), |out| out.kv_null(Some("lost"))),
            };
        }
    }

    /// Apply a closure to the actor if it is in the **Prep** state,
    /// otherwise do nothing.  This is used to implement deferred prep
    /// calls.
    #[inline]
    pub fn apply_prep(&self, s: &mut Stakker, f: impl FnOnce(&mut Cx<'_, A>) -> Option<A>) {
        if self.rc.is_prep() {
            let mut cx = Cx::new(&mut s.core, self);
            let val = f(&mut cx);
            if let Some(die) = cx.die {
                self.terminate(s, die);
            } else if let Some(val) = val {
                self.to_ready(s, val);
            }
        }
    }

    /// Apply a closure to the actor when it reaches the **Ready**
    /// state.  If the actor is already in the **Ready** state, the
    /// closure is executed immediately.  If the actor is in the
    /// **Prep** state, then it queues the operation instead of
    /// executing it.  This is used to implement deferred ready calls.
    #[inline]
    pub fn apply(&self, s: &mut Stakker, f: impl FnOnce(&mut A, &mut Cx<'_, A>) + 'static) {
        if let Some(val) = self.rc.borrow_ready(&mut s.actor_owner) {
            let mut cx = Cx::new(&mut s.core, self);
            f(val, &mut cx);
            if let Some(die) = cx.die {
                self.terminate(s, die);
            }
        } else if let Some(prep) = self.rc.borrow_prep(&mut s.actor_owner) {
            let actor = self.clone();
            prep.queue.push(move |s| actor.apply(s, f));
        }
    }

    /// Query an actor from outside the runtime.  This is a
    /// synchronous call intended for use when interfacing to external
    /// code.  Executes the closure on the actor immediately if the
    /// actor has a `Self` value (i.e. is in the **Ready** state), and
    /// returns the result.  Otherwise returns `None`.
    #[inline]
    pub fn query<R>(
        &self,
        s: &mut Stakker,
        f: impl FnOnce(&mut A, &mut Cx<'_, A>) -> R,
    ) -> Option<R> {
        if let Some(val) = self.rc.borrow_ready(&mut s.actor_owner) {
            let mut cx = Cx::new(&mut s.core, self);
            let rv = f(val, &mut cx);
            if let Some(die) = cx.die {
                self.terminate(s, die);
            }
            Some(rv)
        } else {
            None
        }
    }

    /// Get the logging-ID of this actor.  If the **logger** feature
    /// isn't enabled, returns 0.
    #[inline]
    pub fn id(&self) -> LogID {
        self.rc.id()
    }

    /// This may be used to submit items to the [`Deferrer`] main
    /// queue from a drop handler, without needing a [`Core`]
    /// reference.
    ///
    /// [`Core`]: struct.Core.html
    /// [`Deferrer`]: struct.Deferrer.html
    #[inline]
    pub fn defer(&self, f: impl FnOnce(&mut Stakker) + 'static) {
        self.rc.access_deferrer().defer(f);
    }

    /// Used in macros to get a [`Deferrer`] reference
    ///
    /// [`Deferrer`]: struct.Deferrer.html
    #[inline]
    pub fn access_deferrer(&self) -> &Deferrer {
        self.rc.access_deferrer()
    }

    /// Used in macros to get an [`Actor`] reference
    ///
    /// [`Actor`]: struct.Actor.html
    #[inline]
    pub fn access_actor(&self) -> &Self {
        self
    }

    /// Used in macros to get the actor's logging-ID.  If the
    /// **logger** feature isn't enabled, returns 0.
    #[inline]
    pub fn access_log_id(&self) -> LogID {
        self.rc.id()
    }
}

impl<A> Clone for Actor<A> {
    fn clone(&self) -> Self {
        Self {
            rc: self.rc.clone(),
        }
    }
}

/// Indicates reason for actor termination
///
/// In case of failure, this is not intended to provide a full
/// backtrace of actor failures leading up to this failure.  It only
/// provides information on the immediate failure that occurred, to
/// allow the actor receiving this indication to make a decision on
/// what to do next.
///
/// To trace back exactly what happened, enable the **logger** feature
/// and record the `Open` and `Close` events.
pub enum StopCause {
    /// Actor terminated using [`Cx::stop`]
    ///
    /// [`Cx::stop`]: struct.Cx.html#method.stop
    Stopped,

    /// Actor failed using [`Cx::fail`]
    ///
    /// [`Cx::fail`]: struct.Cx.html#method.fail
    Failed(Box<dyn Error>),

    /// Actor was killed through [`ActorOwn::kill`]
    ///
    /// [`ActorOwn::kill`]: struct.ActorOwn.html#method.kill
    Killed(Box<dyn Error>),

    /// Last owning reference to the actor was dropped
    Dropped,

    /// Lost the connection to a remote actor's host.  (This will be
    /// used when remote actors are implemented.)
    Lost,
}

impl StopCause {
    /// Test whether this the actor died with an associated error,
    /// i.e. `Failed` or `Killed`.
    pub fn has_error(&self) -> bool {
        matches!(self, StopCause::Failed(_) | StopCause::Killed(_))
    }
}

impl std::fmt::Display for StopCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stopped => write!(f, "Actor stopped"),
            Self::Failed(e) => write!(f, "Actor failed: {}", e),
            Self::Killed(e) => write!(f, "Actor was killed: {}", e),
            Self::Dropped => write!(f, "Actor was dropped"),
            Self::Lost => write!(f, "Lost connection to actor"),
        }
    }
}

impl std::fmt::Debug for StopCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

/// Context for an actor call
///
/// Gives access to [`Core`] through auto-deref or `*cx`.  Also allows
/// stopping the actor with [`stop!`] (successful termination) and
/// aborting the actor with [`fail!`] (failure with an error).  A
/// reference to the current actor is available through [`Cx::this`].
///
/// [`Core`]: struct.Core.html
/// [`Cx::this`]: struct.Cx.html#method.this
/// [`fail!`]: macro.fail.html
/// [`stop!`]: macro.stop.html
pub struct Cx<'a, A: 'static> {
    pub(crate) core: &'a mut Core,
    pub(crate) this: &'a Actor<A>,
    pub(crate) die: Option<StopCause>,
}

impl<'a, A> Cx<'a, A> {
    #[inline]
    pub(super) fn new(core: &'a mut Core, this: &'a Actor<A>) -> Self {
        Self {
            this,
            core,
            die: None,
        }
    }

    /// Borrow the current actor reference temporarily.  If you need a
    /// longer-lived reference to the actor, then use
    /// `cx.this().clone()`.
    #[inline]
    pub fn this(&self) -> &Actor<A> {
        self.this
    }

    /// Get the logging-ID of the current actor.  If the **logger** feature
    /// isn't enabled, returns 0.
    #[inline]
    pub fn id(&self) -> LogID {
        self.this.id()
    }

    /// Indicate successful termination of the actor.  As soon as the
    /// currently-running actor call finishes, the actor will be
    /// terminated.  Actor state will be dropped, and any further
    /// calls to this actor will be discarded.  The termination status
    /// is passed back to the [`StopCause`] handler provided when the
    /// actor was created.  See also the [`stop!`] macro.
    ///
    /// [`StopCause`]: enum.StopCause.html
    /// [`stop!`]: macro.stop.html
    #[inline]
    pub fn stop(&mut self) {
        if self.die.is_none() {
            self.die = Some(StopCause::Stopped);
        }
    }

    /// Indicate failure of the actor.  As soon as the
    /// currently-running actor call finishes, the actor will be
    /// terminated.  Actor state will be dropped, and any further
    /// calls to this actor will be discarded.  The termination status
    /// is passed back to the [`StopCause`] handler provided when the
    /// actor was created.  See also the [`fail!`] macro.
    ///
    /// [`StopCause`]: enum.StopCause.html
    /// [`fail!`]: macro.fail.html
    #[inline]
    pub fn fail(&mut self, e: impl Error + 'static) {
        if self.die.is_none() {
            self.die = Some(StopCause::Failed(Box::new(e)));
        }
    }

    /// Indicate failure of the actor.  As soon as the
    /// currently-running actor call finishes, the actor will be
    /// terminated.  Actor state will be dropped, and any further
    /// calls to this actor will be discarded.  The termination status
    /// is passed back to the [`StopCause`] handler provided when the
    /// actor was created.  See also the [`fail!`] macro.
    ///
    /// [`StopCause`]: enum.StopCause.html
    /// [`fail!`]: macro.fail.html
    #[inline]
    pub fn fail_str(&mut self, e: &'static str) {
        if self.die.is_none() {
            self.die = Some(StopCause::Failed(Box::new(StrError(e))));
        }
    }

    /// Indicate failure of the actor.  As soon as the
    /// currently-running actor call finishes, the actor will be
    /// terminated.  Actor state will be dropped, and any further
    /// calls to this actor will be discarded.  The termination status
    /// is passed back to the [`StopCause`] handler provided when the
    /// actor was created.  See also the [`fail!`] macro.
    ///
    /// [`StopCause`]: enum.StopCause.html
    /// [`fail!`]: macro.fail.html
    #[inline]
    pub fn fail_string(&mut self, e: impl Into<String>) {
        if self.die.is_none() {
            self.die = Some(StopCause::Failed(Box::new(StringError(e.into()))));
        }
    }

    /// Used in macros to get an [`Actor`] reference
    ///
    /// [`Actor`]: struct.Actor.html
    #[inline]
    pub fn access_actor(&self) -> &Actor<A> {
        self.this
    }

    /// Used in macros to get the actor's logging-ID.  If the
    /// **logger** feature isn't enabled, returns 0.
    #[inline]
    pub fn access_log_id(&self) -> LogID {
        self.this.id()
    }
}

impl<'a, A> Deref for Cx<'a, A> {
    type Target = Core;

    fn deref(&self) -> &Core {
        self.core
    }
}

impl<'a, A> DerefMut for Cx<'a, A> {
    fn deref_mut(&mut self) -> &mut Core {
        self.core
    }
}

/// A miscellaneous error with a string description
#[derive(Debug)]
pub(crate) struct StrError(&'static str);
impl Error for StrError {}
impl fmt::Display for StrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A miscellaneous error with a string description
#[derive(Debug)]
pub(crate) struct StringError(pub String);
impl Error for StringError {}
impl fmt::Display for StringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A set of owning actor references
///
/// This type may be convenient when an actor will have many children
/// of the same type and the parent doesn't need to differentiate
/// between them, nor access them with a key.  This type keeps track of
/// them and automatically takes care of removing each child actor
/// from the set when it terminates.  It also allows iterating through
/// them in case the parent needs to make a call to all child actors.
///
/// The [`actor_in_slab!`] macro provides a wrapper around this to
/// make its use more readable.
///
/// However for more complicated cases, you might want to do all this
/// in your own code instead of using this type.  For example for the
/// case where you already have a key that you want to associate with
/// the child actor, and you want to use that key to get hold of the
/// actor reference, in that case you need a `HashMap`, not a slab.
///
/// Note that this doesn't expose the `usize` slab key.  This is
/// intentional.  Cases where the slab key would be useful are better
/// handled in user code, i.e. they would probably need a `HashMap`,
/// and in that case the [`ActorOwn`] would be better kept in that
/// `HashMap` instead of in this slab.
///
/// [`ActorOwn`]: struct.ActorOwn.html
/// [`actor_in_slab!`]: macro.actor_in_slab.html
pub struct ActorOwnSlab<T: 'static> {
    slab: Slab<ActorOwn<T>>,
}

impl<T: 'static> ActorOwnSlab<T> {
    /// Create a new [`ActorOwnSlab`]
    ///
    /// [`ActorOwnSlab`]: struct.ActorOwnSlab.html
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an actor whose [`ActorOwn`] is stored in the slab, with
    /// a termination notification handler which automatically removes
    /// it from this slab when it fails or terminates.  `get_slab`
    /// would typically be `|this| this.children`, assuming `children`
    /// is what the [`ActorOwnSlab`] is called in the actor's state.
    /// The `notify` handler is called as normal with the
    /// [`StopCause`].
    ///
    /// This call does the same as [`actor_new!`], i.e. it creates the
    /// actor but does not initialise it.  It returns an [`Actor`]
    /// reference which can be used to initialise the actor.
    ///
    /// [`ActorOwnSlab`]: struct.ActorOwnSlab.html
    /// [`ActorOwn`]: struct.ActorOwn.html
    /// [`Actor`]: struct.Actor.html
    /// [`actor_new!`]: macro.actor_new.html
    #[inline]
    pub fn add<P>(
        &mut self,
        core: &mut Core,
        parent: Actor<P>,
        get_slab: impl for<'a> FnOnce(&'a mut P) -> &'a mut Self + 'static,
        notify: Ret<StopCause>,
    ) -> Actor<T> {
        let vacant = self.slab.vacant_entry();
        let key = vacant.key();
        let parid = parent.id();
        let actorown = ActorOwn::new(
            core,
            ret_some_do!(move |cause| {
                let parent2 = parent.clone();
                parent.defer(move |s| {
                    parent2.apply(s, move |this, _| {
                        get_slab(this).slab.remove(key);
                    });
                });
                ret!([notify], cause);
            }),
            parid,
        );
        let actor = actorown.clone();
        vacant.insert(actorown);
        actor
    }

    /// Returns the number of actors held in the slab
    #[inline]
    pub fn len(&self) -> usize {
        self.slab.len()
    }

    /// Returns `true` if there are no values left in the slab
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.slab.is_empty()
    }
}

impl<T> Default for ActorOwnSlab<T> {
    fn default() -> Self {
        Self { slab: Slab::new() }
    }
}

impl<'a, T> IntoIterator for &'a ActorOwnSlab<T> {
    type Item = &'a ActorOwn<T>;
    type IntoIter = ActorOwnSlabIter<'a, T>;

    fn into_iter(self) -> ActorOwnSlabIter<'a, T> {
        ActorOwnSlabIter(self.slab.iter())
    }
}

/// Iterator over actors in an [`ActorOwnSlab`]
///
/// [`ActorOwnSlab`]: struct.ActorOwnSlab.html
pub struct ActorOwnSlabIter<'a, T: 'static>(slab::Iter<'a, ActorOwn<T>>);

impl<'a, T> Iterator for ActorOwnSlabIter<'a, T> {
    type Item = &'a ActorOwn<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|item| item.1)
    }
}
