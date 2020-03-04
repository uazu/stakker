use crate::queue::FnOnceQueue;
use crate::rc::ActorRc;
use crate::{Core, Deferrer, Ret, Stakker};
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
    /// However all calls to be actor will be delayed until the actor
    /// moves into the **Ready** state.
    ///
    /// `notify` is the [`StopCause`] return, which is called when the
    /// actor terminates.
    ///
    /// See macros [`actor!`] and [`actor_new!`] for help with
    /// creating and initialising actors.
    ///
    /// [`Fwd`]: struct.Fwd.html
    /// [`Ret`]: struct.Ret.html
    /// [`StopCause`]: enum.StopCause.html
    /// [`actor!`]: macro.actor.html
    /// [`actor_new!`]: macro.actor_new.html
    pub fn new(core: &mut Core, notify: Ret<StopCause>) -> ActorOwn<A> {
        Self::construct(Actor {
            rc: ActorRc::new(core, Some(notify)),
        })
    }

    fn construct(actor: Actor<A>) -> Self {
        actor.rc.strong_inc();
        Self { actor }
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

/// A ref-counting reference to an actor
///
/// This may be cloned to get another reference to the same actor.
///
/// # Example implementation of an minimal actor
///
/// ```
///# use stakker::{call, Cx, Ret, ret};
/// struct Light {
///     on: bool,
/// }
/// impl Light {
///     pub fn init(_cx: &mut Cx<'_, Self>, on: bool) -> Option<Self> {
///         Some(Self { on })
///     }
///     pub fn set(&mut self, _cx: &mut Cx<'_, Self>, on: bool) {
///         self.on = on;
///     }
///     pub fn get(&self, cx: &mut Cx<'_, Self>, ret: Ret<bool>) {
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
/// A call should be made to one of the static methods on the actor to
/// start the process of initialising it.  Initialisation may be
/// immediate, or it may start an asynchronous process (for example,
/// making a connection to a remote server).  Each call to a static
/// method may schedule callbacks to other static methods.
/// Eventually, one of these methods should fail the actor, or else
/// return `Some(value)`.
///
/// **"Ready" state**: As soon as a value is returned, this is
/// installed as the actor's `self` value, and the actor moves to the
/// **Ready** state.  Any calls to the actor that were queued up
/// whilst it was in the **Prep** state are flushed and executed at
/// this point.  Whilst in the **Ready** state, the actor can only
/// execute calls to methods with the signature `fn method(&mut self,
/// cx: CX![], ...)`, or the same with `&self`.  Any **Prep**-style
/// calls will be dropped.  Now deferred calls from timers or other
/// actors will execute immediately on reaching the front of the
/// queue.  This is the normal operating mode of the actor.
///
/// **"Zombie" state**: The **Zombie** state can be entered for
/// various reasons.  The first is normal shutdown of the actor
/// through the [`Cx::stop`] method.  The second is failure of the
/// actor through the [`Cx::fail`] or [`Cx::fail_str`] methods.  The
/// third is through being killed externally through the
/// [`Actor::kill`] or [`Actor::kill_str`] methods.  Termination of
/// the actor is notified to the [`StopCause`] handler provided to the
/// [`ActorOwn::new`] method when the actor was created.  (If the last
/// reference to the actor is dropped, the actor will be terminated
/// without entering the **Zombie** state.)
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
/// the termination of the actor.  If the coder ensures that there are
/// no cycles in the [`ActorOwn`] graph (e.g. it is a simple tree),
/// then cleanup is safe and straightforward even in the presence of
/// [`Actor`], [`Fwd`] or [`Ret`] reference cycles.
///
/// This also handles the case where many actors reference a common
/// actor so long as there are no [`ActorOwn`] back-references.  The
/// common actor will only be terminated when the last [`ActorOwn`]
/// reference is dropped.
///
/// However if necessary other termination strategies are possible,
/// since the actor can be terminated externally using the
/// [`Actor::kill`] call.
///
/// [`Actor::is_zombie`]: struct.Actor.html#method.is_zombie
/// [`Actor::kill_str`]: struct.Actor.html#method.kill_str
/// [`Actor::kill`]: struct.Actor.html#method.kill
/// [`ActorOwn::new`]: struct.ActorOwn.html#method.new
/// [`ActorOwn`]: struct.ActorOwn.html
/// [`Actor`]: struct.Actor.html
/// [`Cx::fail_str`]: struct.Cx.html#method.fail_str
/// [`Cx::fail`]: struct.Cx.html#method.fail
/// [`Cx::stop`]: struct.Cx.html#method.stop
/// [`Fwd`]: struct.Fwd.html
/// [`Ret`]: struct.Ret.html
/// [`StopCause`]: enum.StopCause.html
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
    /// Create an additional owning reference to this actor.  When the
    /// last owning reference is dropped, the actor is terminated,
    /// even when there are other references active.
    pub fn owned(&self) -> ActorOwn<A> {
        ActorOwn::construct(self.clone())
    }

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

    /// Kill actor, moving to **Zombie** state and dropping the
    /// contained actor `Self` value.  The actor can never return from
    /// the **Zombie** state.  The provided error is used to generate
    /// an `StopCause::Killed` instance, which is passed to the
    /// [`StopCause`] handler set up when the actor was created.
    ///
    /// [`StopCause`]: enum.StopCause.html
    pub fn kill(&self, s: &mut Stakker, err: Box<dyn Error>) {
        self.terminate(s, StopCause::Killed(err));
    }

    /// Kill actor, moving to **Zombie** state and dropping the
    /// contained actor `Self` value.  The actor can never return from
    /// the **Zombie** state.  The provided error string is used to
    /// generate an `StopCause::Killed` instance, which is passed to
    /// the [`StopCause`] handler set up when the actor was created.
    ///
    /// [`StopCause`]: enum.StopCause.html
    pub fn kill_str(&self, s: &mut Stakker, err: impl Into<String>) {
        self.terminate(s, StopCause::Killed(Box::new(StringError(err.into()))));
    }

    // Terminate actor, moving to **Zombie** state and dropping
    // contained value if any.  The actor can never return from the
    // **Zombie** state.  The `died` value is sent to the
    // [`StopCause`] handler set up when the actor was created.
    fn terminate(&self, s: &mut Stakker, died: StopCause) {
        if let Some(notify) = self.rc.to_zombie(s) {
            notify.ret(died);
        }
    }

    /// Apply a function to the actor if it is in the **Prep** state,
    /// otherwise do nothing.
    #[inline]
    pub fn apply_prep(
        &self,
        s: &mut Stakker,
        f: impl FnOnce(&mut Cx<'_, A>) -> Option<A> + 'static,
    ) {
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

    /// Apply a function to the actor if it is in the **Ready** state.
    /// If the actor is in the **Prep** state, then queues the
    /// operation instead of executing it.
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
}

impl<A> Clone for Actor<A> {
    fn clone(&self) -> Self {
        Self {
            rc: self.rc.clone(),
        }
    }
}

/// Indicates reason for actor termination
pub enum StopCause {
    /// Actor terminated using [`Cx::stop`]
    ///
    /// [`Cx::stop`]: struct.Cx.html#method.stop
    Stopped,

    /// Actor failed using [`Cx::fail`]
    ///
    /// [`Cx::fail`]: struct.Cx.html#method.fail
    Failed(Box<dyn Error>),

    /// Actor was killed through [`Actor::kill`]
    ///
    /// [`Actor::kill`]: struct.Actor.html#method.kill
    Killed(Box<dyn Error>),

    /// Last owning reference to the actor was dropped
    Dropped,
}

impl StopCause {
    /// Test whether this the actor died with an associated error,
    /// i.e. `Failed` or `Killed`.
    pub fn has_error(&self) -> bool {
        match self {
            StopCause::Failed(_) | StopCause::Killed(_) => true,
            _ => false,
        }
    }
}

impl std::fmt::Display for StopCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stopped => write!(f, "Actor stopped"),
            Self::Failed(e) => write!(f, "Actor failed: {}", e),
            Self::Killed(e) => write!(f, "Actor was killed: {}", e),
            Self::Dropped => write!(f, "Actor was dropped"),
        }
    }
}

/// Context for an actor call
///
/// Gives access to [`Core`] through auto-deref or `*cx`.  Also allows
/// stopping the actor with [`Cx::stop`] (successful termination) and
/// aborting the actor with [`Cx::fail`] or [`Cx::fail_str`] (failure
/// with an error).  A reference to the current actor is available
/// through [`Cx::this`].
///
/// [`Core`]: struct.Core.html
/// [`Cx::fail_str`]: struct.Cx.html#method.fail_str
/// [`Cx::fail`]: struct.Cx.html#method.fail
/// [`Cx::stop`]: struct.Cx.html#method.stop
/// [`Cx::this`]: struct.Cx.html#method.this
pub struct Cx<'a, A: 'static> {
    pub(crate) core: &'a mut Core,
    pub(crate) this: &'a Actor<A>,
    pub(crate) die: Option<StopCause>,
}

impl<'a, A> Cx<'a, A> {
    pub(super) fn new(core: &'a mut Core, this: &'a Actor<A>) -> Self {
        Self {
            this,
            core,
            die: None,
        }
    }

    /// Borrow the current actor reference temporarily.  If you need a
    /// longer-lived reference to the actor, then clone the result of
    /// this call.
    pub fn this(&self) -> &Actor<A> {
        self.this
    }

    /// Indicate successful termination of the actor.  As soon as the
    /// currently-running actor call finishes, the actor will be
    /// terminated.  Actor state will be dropped, and any further
    /// calls to this actor will be discarded.  The termination status
    /// is passed back to the [`StopCause`] handler provided when the
    /// actor was created.
    ///
    /// [`StopCause`]: enum.StopCause.html
    pub fn stop(&mut self) {
        self.die = Some(StopCause::Stopped);
    }

    /// Indicate failure of the actor.  As soon as the
    /// currently-running actor call finishes, the actor will be
    /// terminated.  Actor state will be dropped, and any further
    /// calls to this actor will be discarded.  The termination status
    /// is passed back to the [`StopCause`] handler provided when the
    /// actor was created.
    ///
    /// [`StopCause`]: enum.StopCause.html
    pub fn fail(&mut self, e: impl Error + 'static) {
        self.die = Some(StopCause::Failed(Box::new(e)));
    }

    /// Indicate failure of the actor.  As soon as the
    /// currently-running actor call finishes, the actor will be
    /// terminated.  Actor state will be dropped, and any further
    /// calls to this actor will be discarded.  The termination status
    /// is passed back to the [`StopCause`] handler provided when the
    /// actor was created.
    ///
    /// [`StopCause`]: enum.StopCause.html
    pub fn fail_str(&mut self, e: impl Into<String>) {
        self.die = Some(StopCause::Failed(Box::new(StringError(e.into()))));
    }

    /// Used in macros to get an [`Actor`] reference
    ///
    /// [`Actor`]: struct.Actor.html
    #[inline]
    pub fn access_actor(&self) -> &Actor<A> {
        self.this
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
pub struct StringError(pub String);
impl Error for StringError {}
impl fmt::Display for StringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
