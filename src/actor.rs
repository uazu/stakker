use crate::queue::FnOnceQueue;
use crate::rc::ActorRc;
use crate::{Core, Fwd, Stakker};
use std::error::Error;
use std::fmt;
use std::ops::{Deref, DerefMut};

/// An owning ref-counting reference to an actor
///
/// This not only keeps a reference to the actor, but it will also
/// automatically terminate the actor when this reference is dropped.
/// This means that typically actors have a single owner and form a
/// simple ownership tree, like a tree of `Box` references in Rust.
/// This only applies to "liveness", however.  Ref-counting references
/// from elsewhere will cause an actor in the **Zombie** state to stay
/// in memory until the final references are released.  However even
/// if there are reference cycles between actors, they will get
/// cleaned up because when the actor is terminated, all its
/// references to other places are dropped on entering the **Zombie**
/// state, breaking the cycles.
///
/// This dereferences to a normal [`Actor`] reference, so when
/// `clone()` is called on it, a normal non-owning reference results.
/// Use `clone_own()` to get another owning reference.
///
/// [`Actor`]: struct.Actor.html
pub struct ActorOwn<A: 'static> {
    actor: Actor<A>,
}

impl<A: 'static> ActorOwn<A> {
    /// Create a new uninitialised actor of the given type.  This is
    /// in the **Prep** state.  The reference can be cloned and passed
    /// around and used to create `Fwd` instances.  However all calls
    /// to be actor will be delayed until the actor moves into the
    /// **Ready** state.
    ///
    /// The `notify` argument allows a notification of actor
    /// termination to be received by the owning actor.
    ///
    /// See macros [`actor!`] and [`actor_new!`] for help with
    /// creating and initialising actors.
    ///
    /// [`actor!`]: macro.actor.html
    /// [`actor_new!`]: macro.actor_new.html
    #[inline]
    pub fn new(core: &mut Core, notify: Fwd<ActorDied>) -> Self {
        Self {
            actor: Actor {
                rc: ActorRc::new(core, Some(notify)),
            },
        }
    }

    /// Make another owning reference.  Note that the actor will be
    /// terminated if either this or the returned reference is
    /// dropped.  Use plain `clone()` if you want a non-owning
    /// reference.
    pub fn clone_own(&self) -> Self {
        Self {
            actor: self.actor.clone(),
        }
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
        let actor = self.actor.clone();
        self.actor
            .lazy(move |s| actor.terminate(s, ActorDied::Dropped));
    }
}

/// A ref-counting reference to an actor
///
/// This may be cloned to get another reference to the same actor.
/// See [`ActorOwn`] to create an actor.
///
/// # Example implementation of an minimal actor
///
/// ```
///# use stakker::{call, Cx, Fwd};
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
///     pub fn get(&self, cx: &mut Cx<'_, Self>, mut fwd: Fwd<bool>) {
///         call!([fwd], cx, self.on);
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
/// `Fwd` instances referring to methods in this actor, and to pass
/// the actor reference to other actors.  However any normal actor
/// calls will be queued up until the actor becomes ready.  The only
/// calls that are permitted on the actor in the **Prep** state are
/// calls to static methods with the signature `fn method(cx: &mut
/// Cx<'_, Self>, ...) -> Option<Self>`.
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
/// cx: &mut Cx<'_, Self>, ...)`, or the same with `&self`.  Any
/// **Prep**-style calls will be dropped.  Now deferred calls from
/// timers or other actors will execute immediately on reaching the
/// front of the queue.  This is the normal operating mode of the
/// actor.
///
/// **"Zombie" state**: The **Zombie** state can be entered for
/// various reasons.  The first is normal shutdown of the actor
/// through the [`Cx::stop`] method.  The second is failure of the
/// actor through the [`Cx::fail`] or [`Cx::fail_str`] methods.  The
/// third is through being killed externally through the
/// [`Actor::kill`] or [`Actor::kill_str`] methods.  Termination of
/// the actor will be notified to the ancestor actor if a notification
/// handler was passed to the [`ActorOwn::new`] method.  (Note that if
/// the last reference to the actor is dropped, the actor will be
/// dropped without entering the **Zombie** state.)
///
/// Once an actor is a **Zombie** it never leaves that state.  The
/// `self` value is dropped and all resources are released.
/// References remain valid until the last reference is dropped, but
/// the `Actor::is_zombie` method will return true.  Any calls queued
/// for the actor will be dropped.  (Note that if you need to have a
/// very long-lived actor but you also need to restart the actor on
/// failure, consider having one actor wrap another.)
///
/// [`Actor::kill_str`]: struct.Actor.html#method.kill_str
/// [`Actor::kill`]: struct.Actor.html#method.kill
/// [`ActorOwn::new`]: struct.ActorOwn.html#method.new
/// [`ActorOwn`]: struct.ActorOwn.html
/// [`Cx::fail_str`]: struct.Cx.html#method.fail_str
/// [`Cx::fail`]: struct.Cx.html#method.fail
/// [`Cx::stop`]: struct.Cx.html#method.stop
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
    /// Check whether the pointed-to actor is a zombie
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

    /// Kill actor, moving to **Zombie** state and dropping contained
    /// value if any.  The actor can never return from the **Zombie**
    /// state.  The provided error is used to generate an
    /// `ActorDied::Killed` instance, which is passed to the notify
    /// `Fwd`.
    pub fn kill(&self, s: &mut Stakker, err: Box<dyn Error>) {
        self.terminate(s, ActorDied::Killed(err));
    }

    /// Kill actor, moving to **Zombie** state and dropping contained
    /// value if any.  The actor can never return from the **Zombie**
    /// state.  The provided error string is used to generate an
    /// `ActorDied::Killed` instance, which is passed to the notify
    /// `Fwd`.
    pub fn kill_str(&self, s: &mut Stakker, err: impl Into<String>) {
        self.terminate(s, ActorDied::Killed(Box::new(StringError(err.into()))));
    }

    // Terminate actor, moving to **Zombie** state and dropping
    // contained value if any.  The actor can never return from the
    // **Zombie** state.  If `died` is not `ActorDied::Dropped`, sends
    // it to the notify `Fwd` set up when the actor was created.
    fn terminate(&self, s: &mut Stakker, died: ActorDied) {
        if let Some(notify) = self.rc.to_zombie(s) {
            match died {
                ActorDied::Dropped => (),
                _ => notify.fwd(s, died),
            }
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

    // This is used when terminating an actor from a drop handler.  We
    // could expose it as `pub`, but there doesn't seem to be another
    // use for it just now.
    #[inline]
    fn lazy(&self, f: impl FnOnce(&mut Stakker) + 'static) {
        self.rc.defer(f);
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
pub enum ActorDied {
    /// Actor terminated using [`Cx::stop`]
    ///
    /// [`Cx::stop`]: struct.Cx.html#method.stop
    Ended,

    /// Actor failed using [`Cx::fail`]
    ///
    /// [`Cx::fail`]: struct.Cx.html#method.fail
    Failed(Box<dyn Error>),

    /// Actor was killed through [`Actor::kill`]
    ///
    /// [`Actor::kill`]: struct.Actor.html#method.kill
    Killed(Box<dyn Error>),

    /// ActorOwn instance was dropped
    Dropped,
}

impl std::fmt::Display for ActorDied {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ended => write!(f, "Actor terminated normally"),
            Self::Failed(e) => write!(f, "Actor failed: {}", e),
            Self::Killed(e) => write!(f, "Actor was killed: {}", e),
            Self::Dropped => write!(f, "Actor was dropped"),
        }
    }
}

/// Context for an actor call
///
/// Gives access to `Core` through auto-deref or `*cx`.  Also allows
/// stopping the actor with `stop` (successful termination) and
/// aborting the actor with `fail` or `fail_str` (failure with an
/// error).  A reference to the current actor is available through
/// `this()`.
pub struct Cx<'a, A: 'static> {
    pub(crate) core: &'a mut Core,
    pub(crate) this: &'a Actor<A>,
    pub(crate) die: Option<ActorDied>,
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
    /// is passed back to the notify handler provided when the actor
    /// was created.
    pub fn stop(&mut self) {
        self.die = Some(ActorDied::Ended);
    }

    /// Indicate failure of the actor.  As soon as the
    /// currently-running actor call finishes, the actor will be
    /// terminated.  Actor state will be dropped, and any further
    /// calls to this actor will be discarded.  The termination status
    /// is passed back to the notify handler provided when the actor
    /// was created.
    pub fn fail(&mut self, e: impl Error + 'static) {
        self.die = Some(ActorDied::Failed(Box::new(e)));
    }

    /// Indicate failure of the actor.  As soon as the
    /// currently-running actor call finishes, the actor will be
    /// terminated.  Actor state will be dropped, and any further
    /// calls to this actor will be discarded.  The termination status
    /// is passed back to the notify handler provided when the actor
    /// was created.
    pub fn fail_str(&mut self, e: impl Into<String>) {
        self.die = Some(ActorDied::Failed(Box::new(StringError(e.into()))));
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
