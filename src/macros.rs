//! Macros
//!
//! Note that macros have been designed so that there is some
//! punctuation and structure to the arguments, not merely a flat list
//! of anonymous values.  That makes it easier to remember what each
//! part is.  They have also been designed so that `rustfmt` will
//! accept the code within the macro and format it.  So the code must
//! parse as valid Rust, even though the interpretation is different.
//!
//! Arguments are evaluated early where possible.  This means that
//! many borrowing problems common in Rust can be avoided, for example
//! where argument expressions reference something already borrowed
//! earlier in the arg-list, especially `Cx` references.  So the code
//! can look more natural.
//!
//! Also, argument types are checked early where possible, to give
//! easier to understand error messages.

// TODO Write deep tests for all macros.  Need to check all unique
// paths through macros, in order to detect any errors.

// Generate lists of indices from lists of `tt` AST objects.  This is
// used to convert arguments lists into indices so that a tuple can be
// generated and then indexed using `tup.3`-style syntax.
//
// Lists of `tt` items must be enclosed in `[]` and put at the start
// of the arg-list.  Each list is then converted to a list of indices
// contained in `[]` and placed at the end of the arg-list.  More than
// one `[]` list may be included and will be processed.  Finally the
// first identifier is taken to be the name of a macro and is invoked
// with the processed arg-list.
#[doc(hidden)]
#[macro_export]
macro_rules! indices {
    ( $cb:ident $( $args:tt )* ) =>
    { stakker::$cb!( $($args)* ) };
    ( [ ] $($rest:tt)* ) =>
    { stakker::indices!($($rest)* []) };
    ( [ $x0:tt ] $($rest:tt)* ) =>
    { stakker::indices!($($rest)* [ 0 ]) };
    ( [ $x0:tt $x1:tt ] $($rest:tt)* ) =>
    { stakker::indices!($($rest)* [ 0 1 ]) };
    ( [ $x0:tt $x1:tt $x2:tt ] $($rest:tt)* ) =>
    { stakker::indices!($($rest)* [ 0 1 2 ]) };
    ( [ $x0:tt $x1:tt $x2:tt $x3:tt ] $($rest:tt)* ) =>
    { stakker::indices!($($rest)* [ 0 1 2 3 ]) };
    ( [ $x0:tt $x1:tt $x2:tt $x3:tt $x4:tt ] $($rest:tt)* ) =>
    { stakker::indices!($($rest)* [ 0 1 2 3 4 ]) };
    ( [ $x0:tt $x1:tt $x2:tt $x3:tt $x4:tt $x5:tt ] $($rest:tt)* ) =>
    { stakker::indices!($($rest)* [ 0 1 2 3 4 5 ]) };
    ( [ $x0:tt $x1:tt $x2:tt $x3:tt $x4:tt $x5:tt $x6:tt ] $($rest:tt)* ) =>
    { stakker::indices!($($rest)* [ 0 1 2 3 4 5 6 ]) };
    ( [ $x0:tt $x1:tt $x2:tt $x3:tt $x4:tt $x5:tt $x6:tt $x7:tt ] $($rest:tt)* ) =>
    { stakker::indices!($($rest)* [ 0 1 2 3 4 5 6 7 ]) };
    ( [ $x0:tt $x1:tt $x2:tt $x3:tt $x4:tt $x5:tt $x6:tt $x7:tt $x8:tt ] $($rest:tt)* ) =>
    { stakker::indices!($($rest)* [ 0 1 2 3 4 5 6 7 8 ]) };
    ( [ $x0:tt $x1:tt $x2:tt $x3:tt $x4:tt $x5:tt $x6:tt $x7:tt $x8:tt $x9:tt ] $($rest:tt)* ) =>
    { stakker::indices!($($rest)* [ 0 1 2 3 4 5 6 7 8 9 ]) };
}

/// Create a new actor and initialise it
///
/// ```ignore
/// let actor = actor!(Type::init(core, args...), notify);
/// let actor = actor!(<path::Type>::init(core, args...), notify);
/// ```
///
/// This may be used when creation and initialisation of the actor can
/// be done together.  Otherwise see [`actor_new!`].  The actor is
/// created and then the given initialisation function is called
/// asynchronously.  The `notify` argument is a `Fwd<ActorDied>`
/// instance to call if the actor is terminated.
///
/// Implemented using [`ActorOwn::new`].
///
/// [`ActorOwn::new`]: struct.ActorOwn.html#method.new
/// [`actor_new!`]: macro.actor_new.html
#[macro_export]
macro_rules! actor {
    ($type:ident :: $init:ident($core:expr $(, $x:expr)*), $notify:expr) => {{
        let notify = $notify;
        let actor = stakker::ActorOwn::<$type>::new($core, notify);
        stakker::call!([actor], <$type>::init($core $(, $x)*));
        actor
    }};
    (<$type:ty> :: $init:ident($core:expr $(, $x:expr)*), $notify:expr) => {{
        let notify = $notify;
        let actor = stakker::ActorOwn::<$type>::new($core, notify);
        stakker::call!([actor], <$type>::init($core $(, $x)*));
        actor
    }};
}

/// Create a new actor
///
/// This may be used when creation and initialisation of the actor
/// must be done separately, for example when two actors need to be
/// initialised with `Fwd` instances pointing to each other.
/// Otherwise see [`actor!`].
///
/// ```ignore
/// let actor = actor_new!(Type, core, notify);
/// call!([actor], Type::init(core, arg1, arg2...));
/// ```
///
/// Implemented using [`ActorOwn::new`].
///
/// [`ActorOwn::new`]: struct.ActorOwn.html#method.new
/// [`actor!`]: macro.actor.html
#[macro_export]
macro_rules! actor_new {
    ($type:ty, $core:expr, $notify:expr) => {{
        let notify = $notify;
        stakker::ActorOwn::<$type>::new($core, notify)
    }};
}

// Common code for `call!` etc
#[doc(hidden)]
#[macro_export]
macro_rules! generic_call {
    ($h1:ident $h2:ident $hargs:tt; [$actor:expr], $method:ident ( $core:expr $(, $x:expr)* )) => {{
        let actor: stakker::Actor<_> = $actor.clone();
        let args = ( $($x,)* );
        stakker::indices!([$(($x))*] generic_call_ready $h1 $hargs $core; actor args $method)
    }};
    ($h1:ident $h2:ident $hargs:tt; [$actor:expr], $type:ident :: $method:ident ( $core:expr $(, $x:expr)* )) => {{
        let actor: stakker::Actor<_> = $actor.clone();
        let args = ( $($x,)* );
        stakker::indices!([$(($x))*] generic_call_prep $h1 $hargs $core; actor args <$type> $method)
    }};
    ($h1:ident $h2:ident $hargs:tt; [$actor:expr], < $type:ty > :: $method:ident ( $core:expr $(, $x:expr)* )) => {{
        let actor: stakker::Actor<_> = $actor.clone();
        let args = ( $($x,)* );
        stakker::indices!([$(($x))*] generic_call_prep $h1 $hargs $core; actor args <$type> $method)
    }};
    ($h1:ident $h2:ident $hargs:tt; $method:ident ( $cx:expr $(, $x:expr)* )) => {{
        let actor = $cx.this().clone();
        let args = ( $($x,)* );
        stakker::indices!([$(($x))*] generic_call_ready $h1 $hargs $cx; actor args $method)
    }};
    ($h1:ident $h2:ident $hargs:tt; $type:ident :: $method:ident ( $cx:expr $(, $x:expr)* )) => {{
        let actor = $cx.this().clone();
        let args = ( $($x,)* );
        stakker::indices!([$(($x))*] generic_call_prep $h1 $hargs $cx; actor args <$type> $method)
    }};
    ($h1:ident $h2:ident $hargs:tt; < $type:ty > :: $method:ident ( $cx:expr $(, $x:expr)* )) => {{
        let actor = $cx.this().clone();
        let args = ( $($x,)* );
        stakker::indices!([$(($x))*] generic_call_prep $h1 $hargs $cx; actor args <$type> $method)
    }};
    ($h1:ident $h2:ident $hargs:tt; $cx:expr, move |$this:ident, $cxid:ident| $body:expr) => {{
        let cb = move |$this, $cxid| $body;
        let this = $cx.this().clone();
        stakker::$h1!($hargs $cx; move |s| this.apply(s, cb));
    }};
    ($h1:ident $h2:ident $hargs:tt; $cx:expr, |$this:ident, $cxid:ident| $body:expr) => {{
        let cb = |$this, $cxid| $body;
        let this = $cx.this().clone();
        stakker::$h1!($hargs $cx; move |s| this.apply(s, cb));
    }};
    ($h1:ident $h2:ident $hargs:tt; $core:expr, move |$stakker:ident| $x:expr) => {{
        let cb = move |$stakker| $x;
        stakker::$h1!($hargs $core; cb);
    }};
    ($h1:ident $h2:ident $hargs:tt; $core:expr, |$stakker:ident| $x:expr) => {{
        let cb = |$stakker| $x;
        stakker::$h1!($hargs $core; cb);
    }};
    // `[fwd], core` might be confused with a method call if core is a
    // call expression, so match this last.  Normally there would be
    // no reason for `core` to be a call expression, so this is a not
    // a big inconvenience.  Note: a single argument isn't passed as a
    // tuple, so has special handling.
    ($h1:ident $h2:ident $hargs:tt; [ $fwd:expr ], $core:expr, $arg:expr) => {{
        let arg = $arg;
        stakker::$h2!($hargs $core; $fwd; arg);
    }};
    ($h1:ident $h2:ident $hargs:tt; [ $fwd:expr ], $core:expr $(, $arg:expr)*) => {{
        let arg = ( $($arg ,)* );
        stakker::$h2!($hargs $core; $fwd; arg);
    }};
}
#[doc(hidden)]
#[macro_export]
macro_rules! generic_call_ready {
    ($handler:ident $hargs:tt $core:expr; $actor:ident $args:ident $method:ident [$($xi:tt)*]) => {
        stakker::$handler!($hargs $core; move |s| $actor.apply(s, move |o, c| o.$method(c $(, $args.$xi)*)))
    }
}
#[doc(hidden)]
#[macro_export]
macro_rules! generic_call_prep {
    ($handler:ident $hargs:tt $core:expr; $actor:ident $args:ident <$atyp:ty> $method:ident [$($xi:tt)*]) => {
        stakker::$handler!($hargs $core; move |s| $actor.apply_prep(s, move |c| <$atyp>::$method(c $(, $args.$xi)*)))
    }
}

/// Queue an actor call, forward call or inline code for execution soon
///
/// The call is deferred to the main defer queue, which will execute
/// as soon as possible.
///
/// Note that in the examples below, in general there can be any
/// number of arguments, including zero.  The number of arguments
/// depends on the signature of the called method or `Fwd` instance.
/// All of these values may be full Rust expressions.  Exceptions are
/// method names, types/paths and argument names for closures which
/// may be any valid identifier.
///
/// Note that the context or core reference is included in the
/// argument lists to make the form of the argument list look similar
/// to the signature of the method.  However this core or context
/// reference is only used to add the call to the queue or to obtain a
/// reference to the current actor.  When the call executes it will be
/// given a fresh context.
///
/// ```ignore
/// // Call a method in this actor or in another actor
/// call!(method(cx, arg1, arg2...));
/// call!([actorxx], method(core, arg1, arg2...));
///
/// // Call a method whilst the actor is in the 'Prep' state, before it
/// // has a `Self` instance
/// call!(Self::method(cx, arg1, arg2...));
/// call!(<path::Type>::method(cx, arg1, arg2...));
/// call!([actoryy], Type::method(core, arg1, arg2...));
/// call!([actorzz], <path::Type>::method(core, arg1, arg2...));
///
/// // Forward data, using a `Fwd` instance
/// call!([fwd2zz], core, arg1, arg2...);
///
/// // Defer a call to inline code
/// call!(cx, |this, cx| { ...code... });  // Inline code which refers to this actor
/// call!(cx, move |this, cx| { ...code... });
/// call!(core, |stakker| { ...code... }); // Generic inline code (`&mut Stakker` arg)
/// call!(core, move |stakker| { ...code... });
/// ```
///
/// Note that `call!` also supports using [`Deferrer`] in place of
/// `core` in some of the forms above.
///
/// ```ignore
/// call!([actorxx], method(deferrer, arg1, arg2...));
/// call!([actoryy], Type::method(deferrer, arg1, arg2...));
/// call!([actorzz], <path::Type>::method(deferrer, arg1, arg2...));
/// ```
///
/// When using [`Deferrer`] to send a message to a `Fwd` instance it's
/// necessary to do it in two stages, because `Fwd` instances need
/// access to the main queue.  So defer the forward call:
///
/// ```ignore
/// deferrer.defer(|s| call!([fwd2zz], s, arg1, arg2...));
/// ```
///
/// Implemented using [`Core::defer`], [`Actor::apply`] and
/// [`Actor::apply_prep`].
///
/// [`Actor::apply_prep`]: struct.Actor.html#method.apply_prep
/// [`Actor::apply`]: struct.Actor.html#method.apply
/// [`Core::defer`]: struct.Core.html#method.defer
/// [`Deferrer`]: struct.Deferrer.html
#[macro_export]
macro_rules! call {
    ( $($x:tt)+ ) => {{
        stakker::generic_call!(call_defer_aux call_fwd_aux (); $($x)+);
    }};
}
#[doc(hidden)]
#[macro_export]
macro_rules! call_defer_aux {
    (() $core:expr; $cb:expr) => {
        $core.defer($cb);
    };
}
#[doc(hidden)]
#[macro_export]
macro_rules! call_fwd_aux {
    (() $core:expr; $fwd:expr; $arg:ident) => {
        $fwd.fwd($core, $arg);
    };
}

/// Lazily perform an actor call, forward call or inline code
///
/// The call syntax accepted is identical to the [`call!`] macro.
/// This queues calls to the lazy queue which is run only after the
/// normal defer queue has been completely exhausted.  This can be
/// used to run something at the end of this batch of processing, for
/// example to minimize flushes.
///
/// Implemented using [`Core::lazy`], [`Actor::apply`] and
/// [`Actor::apply_prep`].
///
/// [`Actor::apply_prep`]: struct.Actor.html#method.apply_prep
/// [`Actor::apply`]: struct.Actor.html#method.apply
/// [`Core::lazy`]: struct.Core.html#method.lazy
/// [`call!`]: macro.call.html
#[macro_export]
macro_rules! lazy {
    ( $($x:tt)+ ) => {{
        stakker::generic_call!(lazy_defer_aux lazy_fwd_aux (); $($x)+);
    }};
}
#[doc(hidden)]
#[macro_export]
macro_rules! lazy_defer_aux {
    (() $core:expr; $cb:expr) => {
        $core.lazy($cb);
    };
}
#[doc(hidden)]
#[macro_export]
macro_rules! lazy_fwd_aux {
    (() $core:expr; $fwd:expr; $arg:ident) => {
        let fwd: stakker::Fwd<_> = $fwd.clone();
        $core.lazy(move |s| fwd.fwd(s, $arg));
    };
}

/// Perform an actor call, forward call or inline code when the
/// process becomes idle
///
/// The call syntax accepted is identical to the [`call!`] macro.
/// This queues calls to the idle queue which is run only when there
/// is nothing left to run in the normal and lazy queues, and there is
/// no I/O pending.  This can be used to create backpressure,
/// i.e. fetch more data only when all current data has been fully
/// processed.
///
/// Implemented using [`Core::idle`], [`Actor::apply`] and
/// [`Actor::apply_prep`].
///
/// [`Actor::apply_prep`]: struct.Actor.html#method.apply_prep
/// [`Actor::apply`]: struct.Actor.html#method.apply
/// [`Core::idle`]: struct.Core.html#method.idle
/// [`call!`]: macro.call.html
#[macro_export]
macro_rules! idle {
    ( $($x:tt)+ ) => {{
        stakker::generic_call!(idle_defer_aux idle_fwd_aux (); $($x)+);
    }};
}
#[doc(hidden)]
#[macro_export]
macro_rules! idle_defer_aux {
    (() $core:expr; $cb:expr) => {
        $core.idle($cb);
    };
}
#[doc(hidden)]
#[macro_export]
macro_rules! idle_fwd_aux {
    (() $core:expr; $fwd:expr; $arg:ident) => {
        let fwd: stakker::Fwd<_> = $fwd.clone();
        $core.idle(move |s| fwd.fwd(s, $arg));
    };
}

/// After a delay, perform an actor call, forward call or inline code
///
/// The syntax of the calls is identical to [`call!`], but with a
/// `Duration` argument first.  Returns a [`FixedTimerKey`] which can
/// be used to delete the timer if necessary.  See also [`at!`].
///
/// ```ignore
/// after!(dur, ...args-as-for-call-macro...);
/// ```
///
/// Implemented using [`Core::after`], [`Actor::apply`] and
/// [`Actor::apply_prep`].
///
/// [`Actor::apply_prep`]: struct.Actor.html#method.apply_prep
/// [`Actor::apply`]: struct.Actor.html#method.apply
/// [`Core::after`]: struct.Core.html#method.after
/// [`FixedTimerKey`]: struct.FixedTimerKey.html
/// [`at!`]: macro.at.html
/// [`call!`]: macro.call.html
#[macro_export]
macro_rules! after {
    ( $dur:expr, $($x:tt)+ ) => {{
        let dur: Duration = $dur;
        stakker::generic_call!(after_defer_aux after_fwd_aux (dur); $($x)+)
    }};
}
#[doc(hidden)]
#[macro_export]
macro_rules! after_defer_aux {
    (($dur:ident) $core:expr; $cb:expr) => {
        $core.after($dur, $cb);
    };
}
#[doc(hidden)]
#[macro_export]
macro_rules! after_fwd_aux {
    (($dur:ident) $core:expr; $fwd:expr; $arg:ident) => {
        let fwd: stakker::Fwd<_> = $fwd.clone();
        $core.after($dur, move |s| fwd.fwd(s, arg));
    };
}

/// At the given `Instant`, perform an actor call, forward call or inline code
///
/// The syntax of the calls is identical to [`call!`], but with an
/// `Instant` argument first.  Returns a [`FixedTimerKey`] which can
/// be used to delete the timer if necessary.  See also [`after!`].
///
/// ```ignore
/// at!(instant, ...args-as-for-call-macro...);
/// ```
///
/// Implemented using [`Core::timer_add`], [`Actor::apply`] and
/// [`Actor::apply_prep`].
///
/// [`Actor::apply_prep`]: struct.Actor.html#method.apply_prep
/// [`Actor::apply`]: struct.Actor.html#method.apply
/// [`Core::timer_add`]: struct.Core.html#method.timer_add
/// [`FixedTimerKey`]: struct.FixedTimerKey.html
/// [`after!`]: macro.after.html
/// [`call!`]: macro.call.html
#[macro_export]
macro_rules! at {
    ( $inst:expr, $($x:tt)+ ) => {{
        let inst: Instant = $inst;
        stakker::generic_call!(at_defer_aux at_fwd_aux (inst); $($x)+)
    }};
}
#[doc(hidden)]
#[macro_export]
macro_rules! at_defer_aux {
    (($inst:ident) $core:expr; $cb:expr) => {
        $core.timer_add($inst, $cb)
    };
}
#[doc(hidden)]
#[macro_export]
macro_rules! at_fwd_aux {
    (($dur:ident) $core:expr; $fwd:expr; $arg:ident) => {
        let fwd: stakker::Fwd<_> = $fwd.clone();
        $core.timer_add($inst, move |s| fwd.fwd(s, arg))
    };
}

/// Create a "Max" timer
///
/// A "Max" timer expires at the latest (greatest) expiry time
/// provided.  Returns a [`MaxTimerKey`] which can be used to update
/// the expiry time or delete the timer using [`Core::timer_max_mod`]
/// or [`Core::timer_max_del`].
///
/// The syntax of the calls is identical to [`call!`], but with an
/// `Instant` argument first.
///
/// ```ignore
/// timer_max!(instant, ...args-as-for-call-macro...);
/// ```
///
/// Implemented using [`Core::timer_max_add`], [`Actor::apply`] and
/// [`Actor::apply_prep`].
///
/// [`Actor::apply_prep`]: struct.Actor.html#method.apply_prep
/// [`Actor::apply`]: struct.Actor.html#method.apply
/// [`Core::timer_max_add`]: struct.Core.html#method.timer_max_add
/// [`Core::timer_max_del`]: struct.Core.html#method.timer_max_del
/// [`Core::timer_max_mod`]: struct.Core.html#method.timer_max_mod
/// [`MaxTimerKey`]: struct.MaxTimerKey.html
/// [`call!`]: macro.call.html
#[macro_export]
macro_rules! timer_max {
    ( $inst:expr, $($x:tt)+ ) => {{
        let inst: Instant = $inst;
        stakker::generic_call!(timer_max_defer_aux timer_max_fwd_aux (inst); $($x)+)
    }};
}
#[doc(hidden)]
#[macro_export]
macro_rules! timer_max_defer_aux {
    (($inst:ident) $core:expr; $cb:expr) => {
        $core.timer_max_add($inst, $cb)
    };
}
#[doc(hidden)]
#[macro_export]
macro_rules! timer_max_fwd_aux {
    (($dur:ident) $core:expr; $fwd:expr; $arg:ident) => {
        let fwd: stakker::Fwd<_> = $fwd.clone();
        $core.timer_max_add($inst, move |s| fwd.fwd(s, arg))
    };
}

/// Create a "Min" timer
///
/// A "Min" timer expires at the smallest (earliest) expiry time
/// provided.  Returns a [`MinTimerKey`] which can be used to update
/// the expiry time or delete the timer using [`Core::timer_min_mod`]
/// or [`Core::timer_min_del`].
///
/// The syntax of the calls is identical to [`call!`], but with an
/// `Instant` argument first.
///
/// ```ignore
/// timer_min!(instant, ...args-as-for-call-macro...);
/// ```
///
/// Implemented using [`Core::timer_min_add`], [`Actor::apply`] and
/// [`Actor::apply_prep`].
///
/// [`Actor::apply_prep`]: struct.Actor.html#method.apply_prep
/// [`Actor::apply`]: struct.Actor.html#method.apply
/// [`Core::timer_min_add`]: struct.Core.html#method.timer_min_add
/// [`Core::timer_min_del`]: struct.Core.html#method.timer_min_del
/// [`Core::timer_min_mod`]: struct.Core.html#method.timer_min_mod
/// [`MinTimerKey`]: struct.MinTimerKey.html
/// [`call!`]: macro.call.html
#[macro_export]
macro_rules! timer_min {
    ( $inst:expr, $($x:tt)+ ) => {{
        let inst: Instant = $inst;
        stakker::generic_call!(timer_min_defer_aux timer_min_fwd_aux (inst); $($x)+)
    }};
}
#[doc(hidden)]
#[macro_export]
macro_rules! timer_min_defer_aux {
    (($inst:ident) $core:expr; $cb:expr) => {
        $core.timer_min_add($inst, $cb)
    };
}
#[doc(hidden)]
#[macro_export]
macro_rules! timer_min_fwd_aux {
    (($dur:ident) $core:expr; $fwd:expr; $arg:ident) => {
        let fwd: stakker::Fwd<_> = $fwd.clone();
        $core.timer_min_add($inst, move |s| fwd.fwd(s, arg))
    };
}

// Common code for `fwd_*!`
#[doc(hidden)]
#[macro_export]
macro_rules! generic_fwd {
    // Calling this actor
    ($handler:ident; $method:ident ( $cx:expr $(, $x:expr)* ) as ( $($t:ty),* )) => {{
        let args = ( $($x,)* );
        let actor = $cx.this().clone();
        stakker::indices!([$(($x))*] [$(($t))*] generic_fwd_ready $handler actor args ($($t,)*) $method)
    }};
    ($handler:ident; $type:ident::$method:ident ( $cx:expr $(, $x:expr)* ) as ( $($t:ty),* )) => {{
        let args = ( $($x,)* );
        let actor = $cx.this().clone();
        stakker::indices!([$(($x))*] [$(($t))*] generic_fwd_prep $handler actor args ($($t,)*) <$type> $method)
    }};
    ($handler:ident; <$type:ty>::$method:ident ( $cx:expr $(, $x:expr)* ) as ( $($t:ty),* )) => {{
        let args = ( $($x,)* );
        let actor = $cx.this().clone();
        stakker::indices!([$(($x))*] [$(($t))*] generic_fwd_prep $handler actor args ($($t,)*) <$type> $method)
    }};

    // Calling other actors
    ($handler:ident; [$actor:expr], $method:ident ( __ $(, $x:expr)* ) as ( $($t:ty),* )) => {{
        let actor: stakker::Actor<_> = $actor.clone();
        let args = ( $($x,)* );
        stakker::indices!([$(($x))*] [$(($t))*] generic_fwd_ready $handler actor args ($($t,)*) $method)
    }};
    ($handler:ident; [$actor:expr], $type:ident::$method:ident ( __ $(, $x:expr)* ) as ( $($t:ty),* )) => {{
        let actor: stakker::Actor<_> = $actor.clone();
        let args = ( $($x,)* );
        stakker::indices!([$(($x))*] [$(($t))*] generic_fwd_prep $handler actor args ($($t,)*) <$type> $method)
    }};
    ($handler:ident; [$actor:expr], <$type:ty>::$method:ident ( __ $(, $x:expr)* ) as ( $($t:ty),* )) => {{
        let actor: stakker::Actor<_> = $actor.clone();
        let args = ( $($x,)* );
        stakker::indices!([$(($x))*] [$(($t))*] generic_fwd_prep $handler actor args ($($t,)*) <$type> $method)
    }};

    // Closures
    ($handler:ident; $cx:expr, |$this:ident, $cxid:ident, $arg:ident : $t:ty| $($body:tt)+) => {{
        let actor = $cx.this().clone();
        stakker::$handler!(ready actor |$this, $cxid, $arg: $t| $($body)*)
    }};
    ($handler:ident; $cx:expr, |$this:ident, $cxid:ident $(, $arg:ident : $t:ty)*| $($body:tt)+) => {{
        let actor = $cx.this().clone();
        stakker::$handler!(ready actor |$this, $cxid, ($($arg),*): ($($t),*)| $($body)*)
    }};
    ($handler:ident; $cx:expr, move |$this:ident, $cxid:ident, $arg:ident : $t:ty| $($body:tt)+) => {{
        let actor = $cx.this().clone();
        stakker::$handler!(ready actor move |$this, $cxid, $arg: $t| $($body)*)
    }};
    ($handler:ident; $cx:expr, move |$this:ident, $cxid:ident $(, $arg:ident : $t:ty)*| $($body:tt)+) => {{
        let actor = $cx.this().clone();
        stakker::$handler!(ready actor move |$this, $cxid, ($($arg),*): ($($t),*)| $($body)*)
    }};

    // Help for some invalid cases
    ($handler:ident; [$actor:expr], $method:ident ( $z:expr $(, $x:expr)* ) as ( $($t:ty),* )) => {{
        std::compile_error!("Expecting `__` as first argument of method");
    }};
    ($handler:ident; [$actor:expr], $type:ident::$method:ident ( $z:expr $(, $x:expr)* ) as ( $($t:ty),* )) => {{
        std::compile_error!("Expecting `__` as first argument of method");
    }};
    ($handler:ident; [$actor:expr], <$type:ty>::$method:ident ( $z:expr $(, $x:expr)* ) as ( $($t:ty),* )) => {{
        std::compile_error!("Expecting `__` as first argument of method");
    }};
}
#[doc(hidden)]
#[macro_export]
macro_rules! generic_fwd_ready {
    ($handler:ident $actor:ident $args:ident ($t:ty,) $method:ident [$($xi:tt)*] [$($ti:tt)*]) => {
        stakker::$handler!(ready $actor move |a, cx, m: $t| a.$method(cx $(, $args.$xi)* , m))
    };
    ($handler:ident $actor:ident $args:ident ($($t:ty,)*) $method:ident [$($xi:tt)*] [$($ti:tt)*]) => {
        stakker::$handler!(ready $actor move |a, cx, m: ($($t,)*)| a.$method(cx $(, $args.$xi)* $(, m.$ti)*))
    };
}
#[doc(hidden)]
#[macro_export]
macro_rules! generic_fwd_prep {
    ($handler:ident $actor:ident $args:ident ($t:ty,) <$atyp:ty> $method:ident [$($xi:tt)*] [$($ti:tt)*]) => {
        stakker::$handler!(prep $actor move |cx, m: $t| <$atyp>::$method(cx $(, $args.$xi)* , m))
    };
    ($handler:ident $actor:ident $args:ident ($($t:ty,)*) <$atyp:ty> $method:ident [$($xi:tt)*] [$($ti:tt)*]) => {
        stakker::$handler!(prep $actor move |cx, m: ($($t,)*)| <$atyp>::$method(cx $(, $args.$xi)* $(, m.$ti)*))
    };
}

/// Create a `Fwd` instance for actor calls
///
/// The syntax is similar to that used for [`call!`], except that the
/// call is followed by `as` and a tuple of argument types (which may
/// be empty).  These types are the types of the arguments accepted by
/// the `Fwd` instance when it is called, and which are appended to
/// the argument list of the method call.  So each call to a method is
/// made up of first the fixed arguments (if any) provided at the time
/// the `Fwd` instance was created, followed by the variable arguments
/// (if any) provided when the `Fwd` instance was called.  This must
/// match the signature of the method itself.
///
/// `as` is used here because this is a standard Rust token and so
/// `rustfmt` can format the code, although something like `with`
/// would make more sense.
///
/// Note that a core reference is not required when creating a `Fwd`
/// instance to call another actor.  However for consistency it is
/// included as `__`, and a compilation error is generated if an
/// actual argument is provided.
///
/// ```ignore
/// // Forward to a method in this actor or in another actor
/// fwd_to!(method(cx, arg1, arg2...) as (type1, type2...));
/// fwd_to!([actorxx], method(__, arg1, arg2...) as (type1, type2...));
///
/// // Forward to a method whilst in the 'Prep' state
/// fwd_to!(Self::method(cx, arg1, arg2...) as (type1, type2...));
/// fwd_to!(<path::Type>::method(cx, arg1, arg2...) as (type1, type2...));
/// fwd_to!([actoryy], Type::method(__, arg1, arg2...) as (type1, type2...));
/// fwd_to!([actorzz], <path::Type>::method(__, arg1, arg2...) as (type1, type2...));
///
/// // Forward a call to inline code which refers to this actor.  In
/// // this case the `Fwd` argument list is extracted from the closure
/// // argument list and no `as` section is required.
/// fwd_to!(cx, |this, cx, arg1: type1, arg2: type2...| { ...code... });
/// fwd_to!(cx, move |this, cx, arg1: type1, arg2: type2...| { ...code... });
/// ```
///
/// Implemented using [`Fwd::to_actor`] or [`Fwd::to_actor_prep`].
///
/// [`Fwd::to_actor_prep`]: struct.Fwd.html#method.to_actor_prep
/// [`Fwd::to_actor`]: struct.Fwd.html#method.to_actor
/// [`call!`]: macro.call.html
#[macro_export]
macro_rules! fwd_to {
    ($($x:tt)*) => {{
        stakker::generic_fwd!(fwd_to_aux; $($x)*)
    }}
}
#[doc(hidden)]
#[macro_export]
macro_rules! fwd_to_aux {
    (ready $actor:ident $cb:expr) => {
        stakker::Fwd::to_actor($actor, $cb)
    };
    (prep $actor:ident $cb:expr) => {
        stakker::Fwd::to_actor_prep($actor, $cb)
    };
}

/// Create a single-use `Fwd` instance for actor calls
///
/// The syntax is the same as for [`fwd_to!`].  The underlying closure
/// is a `FnOnce`, and there is a runtime check that the instance is
/// not called more than once.
///
/// ```ignore
/// fwd_once_to!(...arguments-as-for-fwd-macro...);
/// ```
///
/// Implemented using [`Fwd::to_actor_once`] or [`Fwd::to_actor_prep_once`].
///
/// [`Fwd::to_actor_once`]: struct.Fwd.html#method.to_actor_once
/// [`Fwd::to_actor_prep_once`]: struct.Fwd.html#method.to_actor_prep_once
/// [`fwd_to!`]: macro.fwd_to.html
#[macro_export]
macro_rules! fwd_once_to {
    ($($x:tt)*) => {{
        stakker::generic_fwd!(fwd_once_to_aux; $($x)*)
    }}
}
#[doc(hidden)]
#[macro_export]
macro_rules! fwd_once_to_aux {
    (ready $actor:ident $cb:expr) => {
        stakker::Fwd::to_actor_once($actor, $cb)
    };
    (prep $actor:ident $cb:expr) => {
        stakker::Fwd::to_actor_prep_once($actor, $cb)
    };
}

/// Create a `Fwd` instance which panics when called
///
/// ```ignore
/// fwd_panic!(panic_msg)
/// ```
///
/// Argument will typically be a `String` or `&str`.  Note that this
/// will receive and ignore any message type.  Implemented using
/// [`Fwd::panic`].
///
/// [`Fwd::panic`]: struct.Fwd.html#method.panic
#[macro_export]
macro_rules! fwd_panic {
    ($arg:expr) => {
        stakker::Fwd::panic($arg)
    };
}

/// Create a `Fwd` instance which performs an arbitrary action
///
/// The action is performed immediately at the point in the code where
/// the message is forwarded.  So this is executed synchronously
/// rather than asynchronously.  However it will normally be used to
/// defer a call, since it doesn't have access to any actor, just
/// `Core` and the message data.
///
/// ```ignore
/// fwd_do!(|core, msg| ...);
/// ```
///
/// Implemented using [`Fwd::new`].
///
/// [`Fwd::new`]: struct.Fwd.html#method.new
#[macro_export]
macro_rules! fwd_do {
    ($cb:expr) => {
        stakker::Fwd::new($cb)
    };
}

/// Create a `Fwd` instance which does nothing at all
///
/// ```ignore
/// fwd_nop!();
/// ```
///
/// NOP means "no operation".  Implemented using [`Fwd::new`].
///
/// [`Fwd::new`]: struct.Fwd.html#method.new
#[macro_export]
macro_rules! fwd_nop {
    () => {
        stakker::Fwd::new(|_, _| {})
    };
}

/// Create a `Fwd` instance which shuts down the event loop
///
/// ```ignore
/// fwd_shutdown!();
/// ```
///
/// This can be used as the notify handler on an actor to shut down
/// the event loop once that actor terminates.  The reason for the
/// actor's failure is passed through, and can be recovered after loop
/// termination using [`Core::shutdown_reason`].  See also
/// [`Fwd::new`] and [`Core::shutdown`].
///
/// [`Core::shutdown_reason`]: struct.Core.html#method.shutdown_reason
/// [`Core::shutdown`]: struct.Core.html#method.shutdown
/// [`Fwd::new`]: struct.Fwd.html#method.new
#[macro_export]
macro_rules! fwd_shutdown {
    () => {
        stakker::Fwd::new(|c, m| c.shutdown(m))
    };
}
