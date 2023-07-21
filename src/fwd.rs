use crate::rc::FwdRc;
use crate::{Actor, Cx};
use static_assertions::assert_eq_size;

/// Forwarder for messages of type `M`
///
/// Typically this would be created using one of the `fwd_*!` macros.
/// This may be called many times to forward different messages to the
/// same end-point.  For situations where a callback can be used at
/// most one time, prefer [`Ret`], because it is less restrictive on
/// the types of data that may be captured by the closure.
///
/// This is a fat pointer to a ref-counted dynamic `Fn`, so it
/// consumes two `usize` locally, and the size of the `Fn` closure
/// plus the ref-count (a `usize`) on the heap.  It may be cloned
/// cheaply if another identical [`Fwd`] is required.
///
/// For zero arguments, use `Fwd<()>`.  For one argument, use
/// `Fwd<type>`, where `type` is the type of the argument.  For two or
/// more use a tuple: `Fwd<(type1, type2...)>`.  Call the [`Fwd::fwd`]
/// method to send a message or use the [`fwd!`] macro.  Sending a
/// message typically results in the asynchronous execution of an
/// actor call, but may have other effects depending on the type of
/// forwarder.
///
/// [`Fwd::fwd`]: struct.Fwd.html#method.fwd
/// [`Fwd`]: struct.Fwd.html
/// [`Ret`]: struct.Ret.html
/// [`fwd!`]: macro.fwd.html
pub struct Fwd<M: 'static>(FwdRc<M>);

assert_eq_size!(Fwd<()>, [usize; 2]);

// Design note: Adding an `is_valid` method was considered, which
// could allow stale `Fwd` instances to be dropped if the target goes
// away.  However this adds overhead to all `Fwd` types, and in the
// cases where this might have been useful, it would require scanning
// potentially long lists at intervals, i.e. O(N).  It is better to
// make sure that proper cleanup occurs instead.  For actors, that
// means when the actor dies, cleaning up the whole group of related
// actors.

impl<M> Fwd<M> {
    /// Forward a message through the [`Fwd`] instance
    ///
    /// [`Fwd`]: struct.Fwd.html
    pub fn fwd(&self, msg: M) {
        self.0.inner()(msg);
    }

    /// Create a [`Fwd`] instance that performs an arbitrary action
    /// with the message on being called.  The call is made
    /// synchronously at the point that the message is forwarded.
    ///
    /// [`Fwd`]: struct.Fwd.html
    #[inline]
    pub fn new(f: impl Fn(M) + 'static) -> Self {
        Self(FwdRc::new(f))
    }

    /// Create a [`Fwd`] instance that queues calls to an actor.  The
    /// `Fn` provided must be `Copy` because on each invocation a new
    /// copy is made and put on the queue.  Use [`Ret`] instead if
    /// this is too restrictive.
    ///
    /// [`Fwd`]: struct.Fwd.html
    /// [`Ret`]: struct.Ret.html
    #[allow(clippy::wrong_self_convention)]
    #[inline]
    pub fn to_actor<A: 'static>(
        actor: Actor<A>,
        f: impl Fn(&mut A, &mut Cx<'_, A>, M) + Copy + 'static,
    ) -> Self {
        // This relies on closures being inlined.  We expect this to
        // result in just two fully-inlined closures: one boxed and
        // returned as the Fwd value, the other appended to the defer
        // queue when that is called.
        Self::new(move |m| {
            let actor2 = actor.clone();
            actor.defer(move |s| actor2.apply(s, move |a, cx| f(a, cx, m)));
        })
    }

    /// Create a [`Fwd`] instance that queues calls to an actor whilst
    /// in the **Prep** phase.  Once the actor is **Ready**, any
    /// queued prep calls are dropped.
    ///
    /// [`Fwd`]: struct.Fwd.html
    #[allow(clippy::wrong_self_convention)]
    #[inline]
    pub fn to_actor_prep<A: 'static>(
        actor: Actor<A>,
        f: impl Fn(&mut Cx<'_, A>, M) -> Option<A> + Copy + 'static,
    ) -> Self {
        Self::new(move |m| {
            let actor2 = actor.clone();
            actor.defer(move |s| actor2.apply_prep(s, move |cx| f(cx, m)));
        })
    }

    /// Create a [`Fwd`] instance that panics with the given message
    /// when called
    ///
    /// [`Fwd`]: struct.Fwd.html
    #[inline]
    pub fn panic(msg: impl Into<String>) -> Self {
        let msg: String = msg.into();
        Self::new(move |_| panic!("{}", msg))
    }
}

impl<M> Clone for Fwd<M> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
