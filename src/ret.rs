use crate::{Actor, Cx};
use static_assertions::assert_eq_size;

// TODO: Consider `Ret` with built-in timeout that returns None if a
// response is not generated within a certain time.  This would be
// used for connections to unreliable components, e.g. ones over TCP
// links etc.

/// Returner for messages of type `M`
///
/// This can be called only once, and if dropped, it will return a
/// message of `None`.
///
/// This is a fat pointer to a boxed dynamic `FnOnce`, so it consumes
/// two `usize` locally, and the size of the `FnOnce` closure on the
/// heap.  It is a "move" type, so does not support `Copy` or `Clone`.
/// It must be passed around by moving, and pulled out of composite
/// types by destructuring (if the compiler doesn't automatically
/// destructure for you).  The [`Ret::ret`] call takes `self` which
/// gives a compile-time guarantee that it is used only once.
///
/// For zero arguments, use `Ret<()>`.  For one argument, use
/// `Ret<type>`, where `type` is the type of the argument.  For two or
/// more use a tuple: `Ret<(type1, type2...)>`.  Call the [`Ret::ret`]
/// method to send a message or use the [`ret!`] macro.  Sending a
/// message typically results in the asynchronous execution of an
/// actor call, but may have other effects depending on the type of
/// returner.
///
/// [`Ret::ret`]: struct.Ret.html#method.ret
/// [`ret!`]: macro.ret.html
pub struct Ret<M: 'static>(Option<Box<dyn FnOnce(Option<M>)>>);

assert_eq_size!(Ret<()>, [usize; 2]);

impl<M> Ret<M> {
    /// Return a message through the [`Ret`] instance.  This uses
    /// `self` which guarantees that it will be called only once.  So
    /// this will not work on a reference to the [`Ret`] instance.
    /// The tuple or struct containing the [`Ret`] instance needs to
    /// be destructured to move the [`Ret`] instance out first.
    ///
    /// [`Ret`]: struct.Ret.html
    pub fn ret(mut self, msg: M) {
        if let Some(f) = self.0.take() {
            f(Some(msg));
        }
    }

    /// Create a [`Ret`] instance that performs an arbitrary action
    /// with the message on being called.  The call is made
    /// synchronously at the point that the message is forwarded.
    /// `None` is passed if the instance is dropped.
    ///
    /// [`Ret`]: struct.Ret.html
    #[inline]
    pub fn new(f: impl FnOnce(Option<M>) + 'static) -> Self {
        Self(Some(Box::new(f)))
    }

    /// Create a [`Ret`] instance that queues a call to an actor.
    ///
    /// [`Ret`]: struct.Ret.html
    #[allow(clippy::wrong_self_convention)]
    #[inline]
    pub fn to_actor<A: 'static>(
        actor: Actor<A>,
        f: impl FnOnce(&mut A, &mut Cx<'_, A>, Option<M>) + 'static,
    ) -> Self {
        // This relies on closures being inlined.  We expect this to
        // result in just two fully-inlined closures: one boxed and
        // returned as the [`Ret`] value, the other appended to the
        // defer queue when that is called.
        Self::new(move |m| {
            let actor2 = actor.clone();
            actor.defer(move |s| actor2.apply(s, move |a, cx| f(a, cx, m)));
        })
    }

    /// Create a [`Ret`] instance that queues calls to an actor whilst
    /// in the **Prep** phase.  Once the actor is **Ready**, any
    /// queued prep calls are dropped.
    ///
    /// [`Ret`]: struct.Ret.html
    #[allow(clippy::wrong_self_convention)]
    #[inline]
    pub fn to_actor_prep<A: 'static>(
        actor: Actor<A>,
        f: impl FnOnce(&mut Cx<'_, A>, Option<M>) -> Option<A> + 'static,
    ) -> Self {
        Self::new(move |m| {
            let actor2 = actor.clone();
            actor.defer(move |s| actor2.apply_prep(s, move |cx| f(cx, m)));
        })
    }

    /// Create a [`Ret`] instance that queues a call to an actor if a
    /// value is returned, but not if the [`Ret`] instance is dropped.
    ///
    /// [`Ret`]: struct.Ret.html
    #[inline]
    pub fn some_to_actor<A: 'static>(
        actor: Actor<A>,
        f: impl FnOnce(&mut A, &mut Cx<'_, A>, M) + 'static,
    ) -> Self {
        Self::new(move |m| {
            if let Some(m) = m {
                let actor2 = actor.clone();
                actor.defer(move |s| actor2.apply(s, move |a, cx| f(a, cx, m)));
            }
        })
    }

    /// Create a [`Ret`] instance that queues calls for returned
    /// values (but not if the [`Ret`] instance is dropped) to an
    /// actor whilst in the **Prep** phase.  Once the actor is
    /// **Ready**, any queued prep calls are dropped.
    ///
    /// [`Ret`]: struct.Ret.html
    #[inline]
    pub fn some_to_actor_prep<A: 'static>(
        actor: Actor<A>,
        f: impl FnOnce(&mut Cx<'_, A>, M) -> Option<A> + 'static,
    ) -> Self {
        Self::new(move |m| {
            if let Some(m) = m {
                let actor2 = actor.clone();
                actor.defer(move |s| actor2.apply_prep(s, move |cx| f(cx, m)));
            }
        })
    }

    /// Create a [`Ret`] instance that panics with the given message
    /// when called
    ///
    /// [`Ret`]: struct.Ret.html
    #[inline]
    pub fn panic(msg: impl Into<String>) -> Self {
        let msg: String = msg.into();
        Self::new(move |m| {
            if m.is_some() {
                panic!("{}", msg);
            }
        })
    }
}

impl<M> Drop for Ret<M> {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f(None);
        }
    }
}
