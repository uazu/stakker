use crate::rc::FwdRc;
use crate::{Actor, Core, Cx};
use std::cell::Cell;

/// Forwarder for messages of type `M`
///
/// This is a pointer to a ref-counted `Fn`, so it consumes one
/// `usize` locally, and the size of the `Fn` closure plus the
/// ref-count (a `usize`) on the heap.  It may be cloned cheaply if
/// another identical `Fwd` is required.
///
/// For zero arguments, use `Fwd<()>`.  For one argument, use
/// `Fwd<type>`, where `type` is the type of the argument.  For two or
/// more use a tuple: `Fwd<(type1, type2...)>`.  Call the `fwd` method
/// to send a message.  This requires a `Core` reference rather than a
/// `Stakker` reference, which means that it can be called from
/// actors.  Sending a message typically results in the asynchronous
/// execution of an actor call, but may have other effects depending
/// on the type of forwarder.
pub struct Fwd<M: 'static>(FwdRc<M>);

// Design note: Adding an `is_valid` method was considered, which
// could allow stale `Fwd` instances to be dropped if the target goes
// away.  However this adds overhead to all `Fwd` types, and in the
// cases where this might have been useful, it would require scanning
// potentially long lists at intervals, i.e. O(N).  It is better to
// make sure that proper cleanup occurs instead.  For actors, that
// means when the actor dies, cleaning up the whole group of related
// actors.

impl<M> Fwd<M> {
    /// Forward a message through the `Fwd` instance
    pub fn fwd(&self, core: &mut Core, msg: M) {
        self.0.inner()(core, msg);
    }

    /// Create a `Fwd` instance that performs an arbitrary action with
    /// the message on being called.  The call is made synchronously
    /// at the point that the message is forwarded.
    #[inline]
    pub fn new(f: impl Fn(&mut Core, M) + 'static) -> Self {
        Self(FwdRc::new(f))
    }

    /// Create a `Fwd` instance that queues calls to an actor
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
        Self::new(move |c, m| {
            let actor = actor.clone();
            c.defer(move |s| actor.apply(s, move |a, cx| f(a, cx, m)));
        })
    }

    /// Create a single-use `Fwd` instance that queues calls to an
    /// actor.  If it is used more than once, it panics.  The call is
    /// a `FnOnce`, so may contain non-Copy data, i.e. data being
    /// moved from one place to another.
    #[allow(clippy::wrong_self_convention)]
    #[inline]
    pub fn to_actor_once<A: 'static>(
        actor: Actor<A>,
        f: impl FnOnce(&mut A, &mut Cx<'_, A>, M) + 'static,
    ) -> Self {
        let data = Cell::new(Some((actor, f)));
        Self::new(move |c, m| {
            if let Some((actor, f)) = data.replace(None) {
                c.defer(move |s| actor.apply(s, move |a, cx| f(a, cx, m)));
            } else {
                panic!("Fwd::to_actor_once() callback called twice");
            }
        })
    }

    /// Create a `Fwd` instance that queues calls to an actor whilst
    /// in the `Prep` phase.  Once the actor is `Ready`, any queued
    /// prep calls are dropped.
    #[allow(clippy::wrong_self_convention)]
    #[inline]
    pub fn to_actor_prep<A: 'static>(
        actor: Actor<A>,
        f: impl Fn(&mut Cx<'_, A>, M) -> Option<A> + Copy + 'static,
    ) -> Self {
        Self::new(move |c, m| {
            let actor = actor.clone();
            c.defer(move |s| actor.apply_prep(s, move |cx| f(cx, m)));
        })
    }

    /// Create a single-use `Fwd` instance that queues calls to an
    /// actor whilst in the `Prep` phase.  If it is used more than
    /// once, it panics.  The call is a `FnOnce`, so may contain
    /// non-Copy data, i.e. data being moved from one place to
    /// another.  Once the actor is `Ready`, any queued prep calls are
    /// dropped.
    #[allow(clippy::wrong_self_convention)]
    #[inline]
    pub fn to_actor_prep_once<A: 'static>(
        actor: Actor<A>,
        f: impl FnOnce(&mut Cx<'_, A>, M) -> Option<A> + Copy + 'static,
    ) -> Self {
        let data = Cell::new(Some((actor, f)));
        Self::new(move |c, m| {
            if let Some((actor, f)) = data.replace(None) {
                c.defer(move |s| actor.apply_prep(s, move |cx| f(cx, m)));
            } else {
                panic!("Fwd::to_actor_once() callback called twice");
            }
        })
    }

    /// Create a `Fwd` instance that panics with the given message
    /// when called
    #[inline]
    pub fn panic(msg: impl Into<String>) -> Self {
        let msg: String = msg.into();
        Self::new(move |_, _| panic!("{}", msg))
    }
}

impl<M> Clone for Fwd<M> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
