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

/// Shorthand for context argument type
///
/// Usually (for Rust 2018 edition) the context argument must be
/// written `cx: &mut Cx<'_, Self>`.  Using this macro it can instead
/// be written `cx: CX![]`.  This reduces the boilerplate, but keeps
/// everything else as plain Rust.  (The alternative would be to try
/// to wrap the whole method in a macro or use procedural macros.)
///
/// Note that sometimes you'll need a context with a different type
/// than `Self`, in which case `cx: CX![OtherType]` may be used,
/// equivalent to `cx: &mut Cx<'_, OtherType>`.
#[macro_export]
macro_rules! CX {
    () => { &mut $crate::Cx<'_, Self> };
    ($other:ty) => { &mut $crate::Cx<'_, $other> };
}

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
    { $crate::$cb!( $($args)* ) };
    ( [ ] $($rest:tt)* ) =>
    { $crate::indices!($($rest)* []) };
    ( [ $a:tt ] $($rest:tt)* ) =>
    { $crate::indices!($($rest)* [ 0 ]) };
    ( [ $a:tt $b:tt ] $($rest:tt)* ) =>
    { $crate::indices!($($rest)* [ 0 1 ]) };
    ( [ $a:tt $b:tt $c:tt ] $($rest:tt)* ) =>
    { $crate::indices!($($rest)* [ 0 1 2 ]) };
    ( [ $a:tt $b:tt $c:tt $d:tt ] $($rest:tt)* ) =>
    { $crate::indices!($($rest)* [ 0 1 2 3 ]) };
    ( [ $a:tt $b:tt $c:tt $d:tt $e:tt ] $($rest:tt)* ) =>
    { $crate::indices!($($rest)* [ 0 1 2 3 4 ]) };
    ( [ $a:tt $b:tt $c:tt $d:tt $e:tt $f:tt ] $($rest:tt)* ) =>
    { $crate::indices!($($rest)* [ 0 1 2 3 4 5 ]) };
    ( [ $a:tt $b:tt $c:tt $d:tt $e:tt $f:tt $g:tt ] $($rest:tt)* ) =>
    { $crate::indices!($($rest)* [ 0 1 2 3 4 5 6 ]) };
    ( [ $a:tt $b:tt $c:tt $d:tt $e:tt $f:tt $g:tt $h:tt ] $($rest:tt)* ) =>
    { $crate::indices!($($rest)* [ 0 1 2 3 4 5 6 7 ]) };
    ( [ $a:tt $b:tt $c:tt $d:tt $e:tt $f:tt $g:tt $h:tt $i:tt ] $($rest:tt)* ) =>
    { $crate::indices!($($rest)* [ 0 1 2 3 4 5 6 7 8 ]) };
    ( [ $a:tt $b:tt $c:tt $d:tt $e:tt $f:tt $g:tt $h:tt $i:tt $j:tt ] $($rest:tt)* ) =>
    { $crate::indices!($($rest)* [ 0 1 2 3 4 5 6 7 8 9 ]) };
    ( [ $a:tt $b:tt $c:tt $d:tt $e:tt $f:tt $g:tt $h:tt $i:tt $j:tt $k:tt  ] $($rest:tt)* ) =>
    { $crate::indices!($($rest)* [ 0 1 2 3 4 5 6 7 8 9 10 ]) };
    ( [ $a:tt $b:tt $c:tt $d:tt $e:tt $f:tt $g:tt $h:tt $i:tt $j:tt $k:tt $l:tt  ] $($rest:tt)* ) =>
    { $crate::indices!($($rest)* [ 0 1 2 3 4 5 6 7 8 9 10 11 ]) };
    ( [ $a:tt $b:tt $c:tt $d:tt $e:tt $f:tt $g:tt $h:tt $i:tt $j:tt $k:tt $l:tt $m:tt  ] $($rest:tt)* ) =>
    { $crate::indices!($($rest)* [ 0 1 2 3 4 5 6 7 8 9 10 11 12 ]) };
    ( [ $a:tt $b:tt $c:tt $d:tt $e:tt $f:tt $g:tt $h:tt $i:tt $j:tt $k:tt $l:tt $m:tt $n:tt ] $($rest:tt)* ) =>
    { $crate::indices!($($rest)* [ 0 1 2 3 4 5 6 7 8 9 10 11 12 13 ]) };
    ( [ $a:tt $b:tt $c:tt $d:tt $e:tt $f:tt $g:tt $h:tt $i:tt $j:tt $k:tt $l:tt $m:tt $n:tt $o:tt ] $($rest:tt)* ) =>
    { $crate::indices!($($rest)* [ 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 ]) };
    ( [ $a:tt $b:tt $c:tt $d:tt $e:tt $f:tt $g:tt $h:tt $i:tt $j:tt $k:tt $l:tt $m:tt $n:tt $o:tt $p:tt ] $($rest:tt)* ) =>
    { $crate::indices!($($rest)* [ 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 ]) };
    ( $($rest:tt)* ) =>
    { std::compile_error!("Too many arguments in call"); }
}

// Used to insert empty function calls in test mode which let us test
// coverage of the macro branches
#[cfg(test)]
#[doc(hidden)]
#[macro_export]
macro_rules! COVERAGE {
    ($name:ident) => {
        $crate::test::macro_coverage::$name();
    };
}
#[cfg(not(test))]
#[doc(hidden)]
#[macro_export]
macro_rules! COVERAGE {
    ($name:ident) => {};
}

/// Create a new actor and initialise it
///
/// ```ignore
/// let actor = actor!(core, Type::init(args...), notify);
/// let actor = actor!(core, <path::Type>::init(args...), notify);
/// ```
///
/// This may be used when creation and initialisation of the actor can
/// be done together.  Otherwise see [`actor_new!`].  The actor is
/// created and then the given initialisation function is called
/// asynchronously.  The `notify` argument is a `Ret<StopCause>`
/// instance to call if the actor is terminated.  An [`ActorOwn`]
/// reference is returned.
///
/// Implemented using [`ActorOwn::new`].
///
/// [`ActorOwn::new`]: struct.ActorOwn.html#method.new
/// [`ActorOwn`]: struct.ActorOwn.html
/// [`actor_new!`]: macro.actor_new.html
#[macro_export]
macro_rules! actor {
    ($core:expr, $type:ident :: $init:ident($($x:expr),* $(,)? ), $notify:expr) => {{
        $crate::COVERAGE!(actor_0);
        let notify = $notify;
        let core = $core.access_core();
        let actor = $crate::ActorOwn::<$type>::new(core, notify);
        $crate::call!([actor], <$type>::$init($($x),*));
        actor
    }};
    ($core:expr, <$type:ty> :: $init:ident($($x:expr),* $(,)? ), $notify:expr) => {{
        $crate::COVERAGE!(actor_1);
        let notify = $notify;
        let core = $core.access_core();
        let actor = $crate::ActorOwn::<$type>::new(core, notify);
        $crate::call!([actor], <$type>::$init($($x),*));
        actor
    }};
}

/// Create a new actor
///
/// This may be used when creation and initialisation of the actor
/// must be done separately, for example when two actors need to be
/// initialised with [`Fwd`] instances pointing to each other.
/// Otherwise see [`actor!`].
///
/// ```ignore
/// let actor = actor_new!(core, Type, notify);
/// call!([actor, core], Type::init(arg1, arg2...));
/// ```
///
/// An [`ActorOwn`] reference is returned.  Implemented using
/// [`ActorOwn::new`].
///
/// [`ActorOwn::new`]: struct.ActorOwn.html#method.new
/// [`ActorOwn`]: struct.ActorOwn.html
/// [`Fwd`]: struct.Fwd.html
/// [`actor!`]: macro.actor.html
#[macro_export]
macro_rules! actor_new {
    ($core:expr, $type:ty, $notify:expr) => {{
        $crate::COVERAGE!(actor_new);
        let notify = $notify;
        $crate::ActorOwn::<$type>::new($core, notify) // Expecting Cx, Core or Stakker ref
    }};
}

/// Create a new actor that implements a trait and initialise it
///
/// ```ignore
/// let actor = actor_of_trait!(core, BoxedTrait, Type::init(args...), notify);
/// let actor = actor_of_trait!(core, BoxedTrait, <path::Type>::init(args...), notify);
/// ```
///
/// This allows treating a set of actors that all implement a trait
/// equally in the calling code.  The actors have to be defined
/// slightly differently to make this work.  Here's a short example:
///
/// ```
/// # use stakker::*;
/// # use std::time::Instant;
/// // Trait definition
/// type Animal = Box<dyn AnimalTrait>;
/// trait AnimalTrait {
///     fn sound(&mut self, cx: CX![Animal]);
/// }
///
/// struct Cat;
/// impl Cat {
///     fn init(_: CX![Animal]) -> Option<Animal> {
///         Some(Box::new(Self))
///     }
/// }
/// impl AnimalTrait for Cat {
///     fn sound(&mut self, _: CX![Animal]) {
///         println!("Miaow");
///     }
/// }
///
/// struct Dog;
/// impl Dog {
///     fn init(_: CX![Animal]) -> Option<Animal> {
///         Some(Box::new(Self))
///     }
/// }
/// impl AnimalTrait for Dog {
///     fn sound(&mut self, _: CX![Animal]) {
///         println!("Woof");
///     }
/// }
///
/// let mut stakker = Stakker::new(Instant::now());
/// let s = &mut stakker;
///
/// // This variable can hold any kind of animal
/// let mut animal: ActorOwn<Animal>;
/// animal = actor_of_trait!(s, Animal, Cat::init(), ret_nop!());
/// call!([animal], sound());
/// animal = actor_of_trait!(s, Animal, Dog::init(), ret_nop!());
/// call!([animal], sound());
///
/// // To separate creation and initialisation, do it this way:
/// animal = actor_new!(s, Animal, ret_nop!());
/// call!([animal], Cat::init());
/// call!([animal], sound());
///
/// s.run(Instant::now(), false);
/// ```
///
/// Implemented using [`ActorOwn::new`].
///
/// [`ActorOwn::new`]: struct.ActorOwn.html#method.new
#[macro_export]
macro_rules! actor_of_trait {
    ($core:expr, $trait:ident, $type:ident :: $init:ident($($x:expr),* $(,)? ), $notify:expr) => {{
        $crate::COVERAGE!(actor_2);
        let notify = $notify;
        let core = $core.access_core();
        let actor = $crate::ActorOwn::<$trait>::new(core, notify);
        $crate::call!([actor], <$type>::$init($($x),*));
        actor
    }};
    ($core:expr, $trait:ident, <$type:ty> :: $init:ident($($x:expr),* $(,)? ), $notify:expr) => {{
        $crate::COVERAGE!(actor_3);
        let notify = $notify;
        let core = $core.access_core();
        let actor = $crate::ActorOwn::<$trait>::new(core, notify);
        $crate::call!([actor], <$type>::$init($($x),*));
        actor
    }};
}

// Common code for `call!` etc
#[doc(hidden)]
#[macro_export]
macro_rules! generic_call {
    // Closures
    ($handler:ident $hargs:tt $access:ident;
     [$cx:expr], |$this:ident, $cxid:ident| $body:expr) => {{
         $crate::COVERAGE!(generic_call_0);
         let cb = move |$this: &mut Self, $cxid: &mut $crate::Cx<'_, Self>| $body;
         let cx: &mut $crate::Cx<'_, Self> = $cx;  // Expecting Cx<Self> ref
         let this = cx.this().clone();
         let core = $cx.access_core();
         $crate::$handler!($hargs core; move |s| this.apply(s, cb));
     }};
    ($handler:ident $hargs:tt $access:ident;
     [$core:expr], |$stakker:ident| $body:expr) => {{
         $crate::COVERAGE!(generic_call_1);
         let core = $core.access_core();  // Expecting Core, Cx or Stakker ref
         let cb = move |$stakker : &mut $crate::Stakker| $body;
         $crate::$handler!($hargs core; cb);
     }};
    ($handler:ident $hargs:tt $access:ident;
     [$cx:expr], |_ $($x:tt)*) => {{
         std::compile_error!("Do not use '_' as a closure argument in this macro; use a '_' prefix instead");
     }};
    ($handler:ident $hargs:tt $access:ident;
     [$cx:expr], |$t:ident, _ $($x:tt)*) => {{
         std::compile_error!("Do not use '_' as a closure argument in this macro; use a '_' prefix instead");
     }};
    ($handler:ident $hargs:tt $access:ident;
     [$cx:expr], move | $($x:tt)*) => {{
         std::compile_error!("Do not add `move` to closures as they get an implicit `move` anyway");
     }};
    // All remaining [actor] turned to [actor, actor]
    ($handler:ident $hargs:tt $access:ident;
     [$actor_or_cx:expr], $($x:tt)+) => {{
         // Can't do `let` for actor_or_cx here because that would move it and drop it
         $crate::generic_call!($handler $hargs $access; [$actor_or_cx, $actor_or_cx], $($x)+)
     }};
    ($handler:ident $hargs:tt $access:ident;
     [$actor:expr, $core:expr], $method:ident ( $($x:expr),* $(,)? )) => {{
         $crate::COVERAGE!(generic_call_2);
         let actor = $actor.access_actor().clone();  // Expecting Actor or Cx ref
         let _args = ( $($x,)* );  // This must be before access borrow
         let access = $core.$access();
         $crate::indices!([$(($x))*] generic_call_ready $handler $hargs access; actor _args $method)
     }};
    ($handler:ident $hargs:tt $access:ident;
     [$actor:expr, $core:expr], $type:ident :: $method:ident ( $($x:expr),* $(,)? )) => {{
         $crate::COVERAGE!(generic_call_3);
         let actor = $actor.access_actor().clone();  // Expecting Actor or Cx ref
         let _args = ( $($x,)* );  // This must be before access borrow
         let access = $core.$access();
         $crate::indices!([$(($x))*] generic_call_prep $handler $hargs access; actor _args <$type> $method)
     }};
    ($handler:ident $hargs:tt $access:ident;
     [$actor:expr, $core:expr], < $type:ty > :: $method:ident ( $($x:expr),* $(,)? )) => {{
         $crate::COVERAGE!(generic_call_4);
         let actor = $actor.access_actor().clone();  // Expecting Actor or Cx ref
         let _args = ( $($x,)* );  // This must be before access borrow
         let access = $core.$access();
         $crate::indices!([$(($x))*] generic_call_prep $handler $hargs access; actor _args <$type> $method)
     }};
}
#[doc(hidden)]
#[macro_export]
macro_rules! generic_call_ready {
    ($handler:ident $hargs:tt $core:ident; $actor:ident $args:ident $method:ident [$($xi:tt)*]) => {
        $crate::$handler!($hargs $core; move |s| $actor.apply(s, move |o, c| o.$method(c $(, $args.$xi)*)))
    }
}
#[doc(hidden)]
#[macro_export]
macro_rules! generic_call_prep {
    ($handler:ident $hargs:tt $core:ident; $actor:ident $args:ident <$atyp:ty> $method:ident [$($xi:tt)*]) => {
        $crate::$handler!($hargs $core; move |s| $actor.apply_prep(s, move |c| <$atyp>::$method(c $(, $args.$xi)*)))
    }
}

/// Queue an actor call or inline code for execution soon
///
/// The call is deferred to the main defer queue, which will execute
/// as soon as possible.
///
/// Note that in the examples below, in general there can be any
/// number of arguments, including zero.  The number of arguments
/// depends on the signature of the called method.  All of these
/// values may be full Rust expressions, which are evaluated at the
/// call-site before queuing the call.  The exceptions are method
/// names, types/paths and argument names for closures which may be
/// any valid identifier.
///
/// Note that the part in square brackets gives the context of the
/// call.  For calls to the same actor, this is normally just `cx`.
/// For calls to another actor, normally the actor and a [`Core`]
/// reference should be given (e.g. `cx`), but if the [`Core`]
/// reference is omitted, the actor's built-in [`Deferrer`] is used
/// instead.  There is no difference in functionality (apart from
/// borrowing issues), just that [`Core`] is more likely to be in
/// cache.
///
/// ```ignore
/// // Call a method in this actor or in another actor
/// call!([cx], method(arg1, arg2...));
/// call!([actorxx, core], method(arg1, arg2...));
///
/// // Call a method whilst the actor is in the 'Prep' state, before it
/// // has a `Self` instance.  `Type` here in the first line may be `Self`.
/// call!([cx], Type::method(arg1, arg2...));
/// call!([cx], <path::Type>::method(arg1, arg2...));
/// call!([actoryy, core], Type::method(arg1, arg2...));
/// call!([actorzz, core], <path::Type>::method(arg1, arg2...));
///
/// // Use the actor's built-in Deferrer; doesn't require Core
/// call!([actorxx], method(arg1, arg2...));
/// call!([actoryy], Type::method(arg1, arg2...));
/// call!([actorzz], <path::Type>::method(arg1, arg2...));
///
/// // Defer a call to inline code.  Closure is always treated as a `move` closure
/// call!([cx], |this, cx| ...code...);   // Inline code which refers to this actor
/// call!([core], |stakker| ...code...);  // Generic inline code (`&mut Stakker` arg)
/// ```
///
/// Implemented using [`Core::defer`], [`Actor::defer`],
/// [`Actor::apply`] and [`Actor::apply_prep`].
///
/// [`Actor::apply_prep`]: struct.Actor.html#method.apply_prep
/// [`Actor::apply`]: struct.Actor.html#method.apply
/// [`Actor::defer`]: struct.Actor.html#method.defer
/// [`Core::defer`]: struct.Core.html#method.defer
/// [`Core`]: struct.Core.html
/// [`Deferrer`]: struct.Deferrer.html
#[macro_export]
macro_rules! call {
    ( $($x:tt)+ ) => {{
        $crate::COVERAGE!(call_0);
        $crate::generic_call!(call_aux () access_deferrer; $($x)+);
    }};
}
#[doc(hidden)]
#[macro_export]
macro_rules! call_aux {
    (() $defer:ident; $cb:expr) => {{
        $crate::COVERAGE!(call_1);
        $defer.defer($cb);
    }};
}

/// Lazily perform an actor call or inline code
///
/// The call syntax accepted is identical to the [`call!`] macro.
/// However the plain `[actor], ...` form is not accepted because a
/// [`Core`] reference is always needed.  This queues calls to the
/// lazy queue which is run only after the normal defer queue has been
/// completely exhausted.  This can be used to run something at the
/// end of this batch of processing, for example to flush buffers
/// after accumulating data.
///
/// Implemented using [`Core::lazy`], [`Actor::apply`] and
/// [`Actor::apply_prep`].
///
/// [`Actor::apply_prep`]: struct.Actor.html#method.apply_prep
/// [`Actor::apply`]: struct.Actor.html#method.apply
/// [`Core::lazy`]: struct.Core.html#method.lazy
/// [`Core`]: struct.Core.html
/// [`call!`]: macro.call.html
#[macro_export]
macro_rules! lazy {
    ( $($x:tt)+ ) => {{
        $crate::COVERAGE!(lazy_0);
        $crate::generic_call!(lazy_aux () access_core; $($x)+); // Error? Try [actor, core] form
    }};
}
#[doc(hidden)]
#[macro_export]
macro_rules! lazy_aux {
    (() $core:ident; $cb:expr) => {{
        $crate::COVERAGE!(lazy_1);
        $core.lazy($cb);
    }};
}

/// Perform an actor call or inline code when the process becomes idle
///
/// The call syntax accepted is identical to the [`call!`] macro.
/// However the `[actor], ...` form is not accepted because a [`Core`]
/// reference is always needed.  This queues calls to the idle queue
/// which is run only when there is nothing left to run in the normal
/// and lazy queues, and there is no I/O pending.  This can be used to
/// create backpressure in the case of processing overload, i.e. fetch
/// more data only when all current data has been fully processed.
///
/// Implemented using [`Core::idle`], [`Actor::apply`] and
/// [`Actor::apply_prep`].
///
/// [`Actor::apply_prep`]: struct.Actor.html#method.apply_prep
/// [`Actor::apply`]: struct.Actor.html#method.apply
/// [`Core::idle`]: struct.Core.html#method.idle
/// [`Core`]: struct.Core.html
/// [`call!`]: macro.call.html
#[macro_export]
macro_rules! idle {
    ( $($x:tt)+ ) => {{
        $crate::COVERAGE!(idle_0);
        $crate::generic_call!(idle_aux () access_core; $($x)+); // Error? Try [actor, core] form
    }};
}
#[doc(hidden)]
#[macro_export]
macro_rules! idle_aux {
    (() $core:ident; $cb:expr) => {{
        $crate::COVERAGE!(idle_1);
        $core.idle($cb);
    }};
}

/// After a delay, perform an actor call or inline code
///
/// The syntax of the calls is identical to [`call!`], but with a
/// `Duration` argument first.  However the `[actor], ...` form is not
/// accepted because a [`Core`] reference is always needed.  Returns a
/// [`FixedTimerKey`] which can be used to delete the timer if
/// necessary.  See also [`at!`].
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
/// [`Core`]: struct.Core.html
/// [`FixedTimerKey`]: struct.FixedTimerKey.html
/// [`at!`]: macro.at.html
/// [`call!`]: macro.call.html
#[macro_export]
macro_rules! after {
    ( $dur:expr, $($x:tt)+ ) => {{
        $crate::COVERAGE!(after_0);
        let dur: Duration = $dur;
        $crate::generic_call!(after_aux (dur) access_core; $($x)+) // Error? Try [actor, core] form
    }};
}
#[doc(hidden)]
#[macro_export]
macro_rules! after_aux {
    (($dur:ident) $core:ident; $cb:expr) => {{
        $crate::COVERAGE!(after_1);
        $core.after($dur, $cb);
    }};
}

/// At the given `Instant`, perform an actor call or inline code
///
/// The syntax of the calls is identical to [`call!`], but with an
/// `Instant` argument first.  However the `[actor], ...` form is not
/// accepted because a [`Core`] reference is always needed.  Returns a
/// [`FixedTimerKey`] which can be used to delete the timer if
/// necessary.  See also [`after!`].
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
/// [`Core`]: struct.Core.html
/// [`FixedTimerKey`]: struct.FixedTimerKey.html
/// [`after!`]: macro.after.html
/// [`call!`]: macro.call.html
#[macro_export]
macro_rules! at {
    ( $inst:expr, $($x:tt)+ ) => {{
        $crate::COVERAGE!(at_0);
        let inst: std::time::Instant = $inst;
        $crate::generic_call!(at_aux (inst) access_core; $($x)+) // Error? Try [actor, core] form
    }};
}
#[doc(hidden)]
#[macro_export]
macro_rules! at_aux {
    (($inst:ident) $core:ident; $cb:expr) => {{
        $crate::COVERAGE!(at_1);
        $core.timer_add($inst, $cb)
    }};
}

/// Create or update a "Max" timer
///
/// A "Max" timer expires at the latest (greatest) expiry time
/// provided.  Modifies a [`MaxTimerKey`] variable or structure member
/// provided by the caller, which should be initialised with
/// `Default::default()` (which is an invalid timer key).  If the
/// timer key currently in the variable is invalid or expired, then a
/// new timer is created using the call-args following, and the key
/// stored in the variable.  Otherwise the timer expiry time is
/// updated with the maximum of the current and provided expiry times,
/// and the call-args are ignored.  The timer may be deleted using
/// [`Core::timer_max_del`].
///
/// The syntax of the calls is identical to [`call!`], but with a
/// variable reference and `Instant` argument first.  However the
/// plain `[actor], ...` form is not accepted because a [`Core`]
/// reference is always needed.
///
/// ```ignore
/// let mut var = Default::default();
///   :::
/// timer_max!(&mut var, instant, ...args-as-for-call-macro...);
/// ```
///
/// Implemented using [`Core::timer_max_upd`],
/// [`Core::timer_max_add`], [`Actor::apply`] and
/// [`Actor::apply_prep`].
///
/// [`Actor::apply_prep`]: struct.Actor.html#method.apply_prep
/// [`Actor::apply`]: struct.Actor.html#method.apply
/// [`Core::timer_max_add`]: struct.Core.html#method.timer_max_add
/// [`Core::timer_max_del`]: struct.Core.html#method.timer_max_del
/// [`Core::timer_max_upd`]: struct.Core.html#method.timer_max_upd
/// [`Core`]: struct.Core.html
/// [`MaxTimerKey`]: struct.MaxTimerKey.html
/// [`call!`]: macro.call.html
#[macro_export]
macro_rules! timer_max {
    ( $var:expr, $inst:expr, $($x:tt)+ ) => {{
        $crate::COVERAGE!(timer_max_0);
        let var: &mut $crate::MaxTimerKey = $var;
        let inst: std::time::Instant = $inst;
        $crate::generic_call!(timer_max_aux (var, inst) access_core; $($x)+) // Error? Try [actor, core] form
    }};
}
#[doc(hidden)]
#[macro_export]
macro_rules! timer_max_aux {
    (($var:ident, $inst:ident) $core:ident; $cb:expr) => {{
        $crate::COVERAGE!(timer_max_1);
        if !$core.timer_max_upd(*$var, $inst) {
            *$var = $core.timer_max_add($inst, $cb);
        }
    }};
}

/// Create or update a "Min" timer
///
/// A "Min" timer expires at the smallest (earliest) expiry time
/// provided.  Modifies a [`MinTimerKey`] variable or structure member
/// provided by the caller, which should be initialised with
/// `Default::default()` (which is an invalid timer key).  If the
/// timer key currently in the variable is invalid or expired, then a
/// new timer is created using the call-args following, and the key
/// stored in the variable.  Otherwise the timer expiry time is
/// updated with the minimum of the current and provided expiry times,
/// and the call-args are ignored.  The timer may be deleted using
/// [`Core::timer_min_del`].
///
/// The syntax of the calls is identical to [`call!`], but with a
/// variable reference and `Instant` argument first.  However the
/// plain `[actor], ...` form is not accepted because a [`Core`]
/// reference is always needed.
///
/// ```ignore
/// let mut var = Default::default();
///   :::
/// timer_min!(&mut var, instant, ...args-as-for-call-macro...);
/// ```
///
/// Implemented using [`Core::timer_min_upd`],
/// [`Core::timer_min_add`], [`Actor::apply`] and
/// [`Actor::apply_prep`].
///
/// [`Actor::apply_prep`]: struct.Actor.html#method.apply_prep
/// [`Actor::apply`]: struct.Actor.html#method.apply
/// [`Core::timer_min_add`]: struct.Core.html#method.timer_min_add
/// [`Core::timer_min_del`]: struct.Core.html#method.timer_min_del
/// [`Core::timer_min_upd`]: struct.Core.html#method.timer_min_upd
/// [`Core`]: struct.Core.html
/// [`MinTimerKey`]: struct.MinTimerKey.html
/// [`call!`]: macro.call.html
#[macro_export]
macro_rules! timer_min {
    ( $var:expr, $inst:expr, $($x:tt)+ ) => {{
        $crate::COVERAGE!(timer_min_0);
        let var: &mut $crate::MinTimerKey = $var;
        let inst: std::time::Instant = $inst;
        $crate::generic_call!(timer_min_aux (var, inst) access_core; $($x)+) // Error? Try [actor, core] form
    }};
}
#[doc(hidden)]
#[macro_export]
macro_rules! timer_min_aux {
    (($var:ident, $inst:ident) $core:ident; $cb:expr) => {{
        $crate::COVERAGE!(timer_min_1);
        if !$core.timer_min_upd(*$var, $inst) {
            *$var = $core.timer_min_add($inst, $cb);
        }
    }};
}

/// Forward data via a [`Fwd`] instance
///
/// ```ignore
/// fwd!([fwd2zz], arg1, arg2...);
/// ```
///
/// There may be zero or more arguments, and they must match the
/// message type.  Implemented using [`Fwd::fwd`]
///
/// [`Fwd::fwd`]: struct.Fwd.html#method.fwd
/// [`Fwd`]: struct.Fwd.html
#[macro_export]
macro_rules! fwd {
    // A single argument isn't passed as a tuple, so has special
    // handling.
    ([ $fwd:expr ], $arg:expr) => {{
        $crate::COVERAGE!(fwd_0);
        $fwd.fwd($arg);
    }};
    ([ $fwd:expr ] $(, $arg:expr)*) => {{
        $crate::COVERAGE!(fwd_1);
        $fwd.fwd(( $($arg ,)* ));
    }};
}

/// Return data via a [`Ret`] instance
///
/// ```ignore
/// ret!([ret2zz], arg1, arg2...);
/// ```
///
/// This consumes the [`Ret`] instance, which means that it cannot be
/// used again.  There may be zero or more arguments, and they must
/// match the message type.  Implemented using [`Ret::ret`].
///
/// [`Ret::ret`]: struct.Ret.html#method.ret
/// [`Ret`]: struct.Ret.html
#[macro_export]
macro_rules! ret {
    // A single argument isn't passed as a tuple, so has special
    // handling.
    ([ $ret:expr ], $arg:expr) => {{
        $crate::COVERAGE!(ret_0);
        $ret.ret($arg);
    }};
    ([ $ret:expr ] $(, $arg:expr)*) => {{
        $crate::COVERAGE!(ret_1);
        $ret.ret(( $($arg ,)* ));
    }};
}

// Common code for `fwd_*!`
#[doc(hidden)]
#[macro_export]
macro_rules! generic_fwd {
    // Calling actors
    ($handler:ident; [$actor:expr], $method:ident ( $($x:expr),* ) as ( $($t:ty),* )) => {{
        $crate::COVERAGE!(generic_fwd_0);
        let actor = $actor.access_actor().clone();  // Expecting Actor or Cx ref
        let _args = ( $($x,)* );
        $crate::indices!([$(($x))*] [$(($t))*] generic_fwd_ready $handler actor _args ($($t,)*) $method)
    }};
    ($handler:ident; [$actor:expr], $type:ident::$method:ident ( $($x:expr),* ) as ( $($t:ty),* )) => {{
        $crate::COVERAGE!(generic_fwd_1);
        let actor = $actor.access_actor().clone();  // Expecting Actor or Cx ref
        let _args = ( $($x,)* );
        $crate::indices!([$(($x))*] [$(($t))*] generic_fwd_prep $handler actor _args ($($t,)*) <$type> $method)
    }};
    ($handler:ident; [$actor:expr], <$type:ty>::$method:ident ( $($x:expr),* ) as ( $($t:ty),* )) => {{
        $crate::COVERAGE!(generic_fwd_2);
        let actor = $actor.access_actor().clone();  // Expecting Actor or Cx ref
        let _args = ( $($x,)* );
        $crate::indices!([$(($x))*] [$(($t))*] generic_fwd_prep $handler actor _args ($($t,)*) <$type> $method)
    }};
    // Calling closures
    ($handler:ident; [$cx:expr], |$this:ident, $cxid:ident, $arg:ident : $t:ty| $($body:tt)+) => {{
        $crate::COVERAGE!(generic_fwd_3);
        let cx: &mut $crate::Cx<'_, _> = $cx;  // Expecting Cx ref
        let actor = cx.this().clone();
        $crate::$handler!(ready actor;
                           move |$this, $cxid, $arg: $t| $($body)*;
                           std::compile_error!("`ret_to!` with a closure requires a single Option argument"))
    }};
    ($handler:ident; [$cx:expr], |$this:ident, $cxid:ident $(, $arg:ident : $t:ty)*| $($body:tt)+) => {{
        $crate::COVERAGE!(generic_fwd_4);
        let cx: &mut $crate::Cx<'_, _> = $cx;  // Expecting Cx ref
        let actor = cx.this().clone();
        $crate::$handler!(ready actor;
                           move |$this, $cxid, ($($arg),*): ($($t),*)| $($body)*;
                           std::compile_error!("`ret_to!` with a closure requires a single Option argument"))
    }};
    ($handler:ident; [$cx:expr], |_ $($x:tt)+) => {{
        std::compile_error!("Do not use '_' as a closure argument in this macro; use a '_' prefix instead");
    }};
    ($handler:ident; [$cx:expr], |$t:ident, _ $($x:tt)+) => {{
        std::compile_error!("Do not use '_' as a closure argument in this macro; use a '_' prefix instead");
    }};
    ($handler:ident; [$cx:expr], move | $($x:tt)*) => {{
        std::compile_error!("Do not add `move` to closures as they get an implicit `move` anyway");
    }};
}
#[doc(hidden)]
#[macro_export]
macro_rules! generic_fwd_ready {
    ($handler:ident $actor:ident $args:ident ($t:ty,) $method:ident [$($xi:tt)*] [$($ti:tt)*]) => {{
        $crate::COVERAGE!(generic_fwd_5);
        $crate::$handler!(ready $actor;
                           move |a, cx, m: $t| a.$method(cx $(, $args.$xi)* , m);
                           move |a, cx, m: Option<$t>| a.$method(cx $(, $args.$xi)* , m))
    }};
    ($handler:ident $actor:ident $args:ident ($($t:ty,)*) $method:ident [$($xi:tt)*] [$($ti:tt)*]) => {{
        $crate::COVERAGE!(generic_fwd_6);
        $crate::$handler!(ready $actor;
                           move |a, cx, _m: ($($t,)*)| a.$method(cx $(, $args.$xi)* $(, _m.$ti)*);
                           move |a, cx, m: Option<($($t,)*)>| a.$method(cx $(, $args.$xi)*, m))
    }};
}
#[doc(hidden)]
#[macro_export]
macro_rules! generic_fwd_prep {
    ($handler:ident $actor:ident $args:ident ($t:ty,) <$atyp:ty> $method:ident [$($xi:tt)*] [$($ti:tt)*]) => {{
        $crate::COVERAGE!(generic_fwd_7);
        $crate::$handler!(prep $actor;
                           move |cx, m: $t| <$atyp>::$method(cx $(, $args.$xi)* , m);
                           move |cx, m: Option<$t>| <$atyp>::$method(cx $(, $args.$xi)* , m))
    }};
    ($handler:ident $actor:ident $args:ident ($($t:ty,)*) <$atyp:ty> $method:ident [$($xi:tt)*] [$($ti:tt)*]) => {{
        $crate::COVERAGE!(generic_fwd_8);
        $crate::$handler!(prep $actor;
                           move |cx, _m: ($($t,)*)| <$atyp>::$method(cx $(, $args.$xi)* $(, _m.$ti)*);
                           move |cx, m: Option<($($t,)*)>| <$atyp>::$method(cx $(, $args.$xi)*, m))
    }};
}

/// Create a [`Fwd`] instance for actor calls
///
/// The syntax is similar to that used for [`call!`], except that the
/// call is followed by `as` and a tuple of argument types (which may
/// be empty).  These types are the types of the arguments accepted by
/// the [`Fwd`] instance when it is called, and which are appended to
/// the argument list of the method call.  So each call to a method is
/// made up of first the fixed arguments (if any) provided at the time
/// the [`Fwd`] instance was created, followed by the variable arguments
/// (if any) provided when the [`Fwd`] instance was called.  This must
/// match the signature of the method itself.
///
/// `as` is used here because this is a standard Rust token that can
/// introduce a tuple and so `rustfmt` can format the code, although
/// something like `with` would make more sense.
///
/// ```ignore
/// // Forward to a method in this actor or in another actor
/// fwd_to!([cx], method(arg1, arg2...) as (type1, type2...));
/// fwd_to!([actorxx], method(arg1, arg2...) as (type1, type2...));
///
/// // Forward to a method whilst in the 'Prep' state
/// fwd_to!([cx], Self::method(arg1, arg2...) as (type1, type2...));
/// fwd_to!([cx], <path::Type>::method(arg1, arg2...) as (type1, type2...));
/// fwd_to!([actoryy], Type::method(arg1, arg2...) as (type1, type2...));
/// fwd_to!([actorzz], <path::Type>::method(arg1, arg2...) as (type1, type2...));
///
/// // Forward a call to inline code which refers to this actor.  In
/// // this case the `Fwd` argument list is extracted from the closure
/// // argument list and no `as` section is required.  Closure is
/// // always treated as a `move` closure.
/// fwd_to!([cx], |this, cx, arg1: type1, arg2: type2...| ...code...);
/// ```
///
/// Implemented using [`Fwd::to_actor`] or [`Fwd::to_actor_prep`].
///
/// [`Fwd::to_actor_prep`]: struct.Fwd.html#method.to_actor_prep
/// [`Fwd::to_actor`]: struct.Fwd.html#method.to_actor
/// [`Fwd`]: struct.Fwd.html
/// [`call!`]: macro.call.html
#[macro_export]
macro_rules! fwd_to {
    ($($x:tt)*) => {{
        $crate::COVERAGE!(fwd_to_0);
        $crate::generic_fwd!(fwd_to_aux; $($x)*)
    }}
}
#[doc(hidden)]
#[macro_export]
macro_rules! fwd_to_aux {
    (ready $actor:ident; $cb:expr; $cb2:expr) => {{
        $crate::COVERAGE!(fwd_to_1);
        $crate::Fwd::to_actor($actor, $cb)
    }};
    (prep $actor:ident; $cb:expr; $cb2:expr) => {{
        $crate::COVERAGE!(fwd_to_2);
        $crate::Fwd::to_actor_prep($actor, $cb)
    }};
}

/// Create a [`Fwd`] instance which panics when called
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
/// [`Fwd`]: struct.Fwd.html
#[macro_export]
macro_rules! fwd_panic {
    ($arg:expr) => {{
        $crate::COVERAGE!(fwd_panic_0);
        $crate::Fwd::panic($arg)
    }};
}

/// Create a [`Fwd`] instance which performs an arbitrary action
///
/// The action is performed immediately at the point in the code where
/// the message is forwarded.  So this is executed synchronously
/// rather than asynchronously.  However it will normally be used to
/// defer a call, since it doesn't have access to any actor, just the
/// message data.  If it doesn't have an actor reference available, it
/// will probably need to capture a [`Deferrer`] in the closure.
///
/// ```ignore
/// fwd_do!(|msg| ...);
/// ```
///
/// Implemented using [`Fwd::new`].
///
/// [`Deferrer`]: struct.Deferrer.html
/// [`Fwd::new`]: struct.Fwd.html#method.new
/// [`Fwd`]: struct.Fwd.html
#[macro_export]
macro_rules! fwd_do {
    ($cb:expr) => {{
        $crate::COVERAGE!(fwd_do_0);
        $crate::Fwd::new($cb)
    }};
}

/// Create a [`Fwd`] instance which does nothing at all
///
/// ```ignore
/// fwd_nop!();
/// ```
///
/// NOP means "no operation".  Implemented using [`Fwd::new`].
///
/// [`Fwd::new`]: struct.Fwd.html#method.new
/// [`Fwd`]: struct.Fwd.html
#[macro_export]
macro_rules! fwd_nop {
    () => {{
        $crate::COVERAGE!(fwd_nop_0);
        $crate::Fwd::new(|_| {})
    }};
}

/// Create a [`Ret`] instance for actor calls
///
/// This is guaranteed to be called **exactly once**, even if dropped.
/// The message is passed as `Some(msg)` if called normally, or as
/// `None` if the [`Ret`] instance was dropped (e.g. if it couldn't be
/// delivered somewhere).  The underlying closure is a `FnOnce`, so
/// non-Copy types can be passed.  The syntax is the same as for
/// [`fwd_to!`], and the message types are specified as normal.
/// However the message is received in a single argument on the
/// receiving method, either `Option<type>` for a single type, or else
/// `Option<(type1, type2...)>`.
///
/// ```ignore
/// ret_to!(...arguments-as-for-fwd_to-macro...);
/// ```
///
/// The closure form must use a single `Option` as above as the
/// argument type, containing all the types passed from the [`Ret`].
///
/// Implemented using [`Ret::to_actor`] or [`Ret::to_actor_prep`].
///
/// [`Ret::to_actor_prep`]: struct.Ret.html#method.to_actor_prep
/// [`Ret::to_actor`]: struct.Ret.html#method.to_actor
/// [`Ret`]: struct.Ret.html
/// [`fwd_to!`]: macro.fwd_to.html
#[macro_export]
macro_rules! ret_to {
    ([$cx:expr], |$this:ident, $cxid:ident, $arg:ident : Option<$t:ty>| $($body:tt)+) => {{
        $crate::COVERAGE!(ret_to_0);
        let cx: &mut $crate::Cx<'_, _> = $cx;  // Expecting Cx ref
        let actor = cx.this().clone();
        $crate::Ret::to_actor(actor, move |$this, $cxid, $arg: Option<$t>| $($body)*)
    }};
    ([$cx:expr], move | $($x:tt)*) => {{
        std::compile_error!("Do not add `move` to closures as they get an implicit `move` anyway");
    }};
    // Closures not matching above will get caught below, giving a
    // compilation error
    ($($x:tt)*) => {{
        $crate::COVERAGE!(ret_to_1);
        $crate::generic_fwd!(ret_to_aux; $($x)*)
    }}
}
#[doc(hidden)]
#[macro_export]
macro_rules! ret_to_aux {
    (ready $actor:ident; $cb:expr; $cb2:expr) => {{
        $crate::COVERAGE!(ret_to_2);
        $crate::Ret::to_actor($actor, $cb2)
    }};
    (prep $actor:ident; $cb:expr; $cb2:expr) => {{
        $crate::COVERAGE!(ret_to_3);
        $crate::Ret::to_actor_prep($actor, $cb2)
    }};
}

/// Create a [`Ret`] instance for actor calls, ignoring drops
///
/// This is guaranteed to be called **at most once**.  Dropping the
/// [`Ret`] instance is ignored, unlike [`ret_to!`], so the message is
/// passed through without an `Option` wrapper, just like [`fwd_to!`].
/// The underlying closure is a `FnOnce`, so non-Copy types can be
/// passed.  The syntax is the same as for [`fwd_to!`], and messages
/// are received in exactly the same way in the target actor method.
///
/// ```ignore
/// ret_some_to!(...arguments-as-for-fwd_to-macro...);
/// ```
///
/// Implemented using [`Ret::some_to_actor`] or [`Ret::some_to_actor_prep`].
///
/// [`Ret::some_to_actor_prep`]: struct.Ret.html#method.some_to_actor_prep
/// [`Ret::some_to_actor`]: struct.Ret.html#method.some_to_actor
/// [`Ret`]: struct.Ret.html
/// [`fwd_to!`]: macro.fwd_to.html
/// [`ret_to!`]: macro.ret_to.html
#[macro_export]
macro_rules! ret_some_to {
    ($($x:tt)*) => {{
        $crate::COVERAGE!(ret_some_to_0);
        $crate::generic_fwd!(ret_some_to_aux; $($x)*)
    }}
}
#[doc(hidden)]
#[macro_export]
macro_rules! ret_some_to_aux {
    (ready $actor:ident; $cb:expr; $cb2:expr) => {{
        $crate::COVERAGE!(ret_some_to_1);
        $crate::Ret::some_to_actor($actor, $cb)
    }};
    (prep $actor:ident; $cb:expr; $cb2:expr) => {{
        $crate::COVERAGE!(ret_some_to_2);
        $crate::Ret::some_to_actor_prep($actor, $cb)
    }};
}

/// Create a [`Ret`] instance which performs an arbitrary action
///
/// The action is performed immediately at the point in the code where
/// the message is returned.  So this is executed synchronously rather
/// than asynchronously.  However it will normally be used to defer a
/// call, since it doesn't have access to any actor, just the message
/// data.  If it doesn't have an actor reference available, it will
/// probably need to capture a [`Deferrer`] in the closure.
///
/// ```ignore
/// ret_do!(|msg| ...);
/// ```
///
/// Implemented using [`Ret::new`].
///
/// [`Deferrer`]: struct.Deferrer.html
/// [`Ret::new`]: struct.Ret.html#method.new
/// [`Ret`]: struct.Ret.html
#[macro_export]
macro_rules! ret_do {
    ($cb:expr) => {{
        $crate::COVERAGE!(ret_do_0);
        $crate::Ret::new($cb)
    }};
}

/// Create a [`Ret`] instance which performs an arbitrary action, ignoring drops
///
/// Like [`ret_some_to!`], this ignores the case of the [`Ret`] instance
/// being dropped, so the message is received without the wrapping
/// `Option`.  The action is performed immediately at the point in the
/// code where the message is returned.  So this is executed
/// synchronously rather than asynchronously.  However it will
/// normally be used to defer a call, since it doesn't have access to
/// any actor, just the message data.  If it doesn't have an actor
/// reference available, it will probably need to capture a
/// [`Deferrer`] in the closure.
///
/// ```ignore
/// ret_some_do!(|msg| ...);
/// ```
///
/// Implemented using [`Ret::new`].
///
/// [`Deferrer`]: struct.Deferrer.html
/// [`Ret::new`]: struct.Ret.html#method.new
/// [`Ret`]: struct.Ret.html
/// [`ret_some_to!`]: macro.ret_some_to.html
#[macro_export]
macro_rules! ret_some_do {
    ($cb:expr) => {{
        $crate::COVERAGE!(ret_some_do_0);
        $crate::Ret::new(move |m| {
            if let Some(m) = m {
                ($cb)(m);
            }
        })
    }};
}

/// Create a [`Ret`] instance which panics when called
///
/// ```ignore
/// ret_panic!(panic_msg)
/// ```
///
/// Ignores the case where the [`Ret`] instance is dropped.  Argument
/// will typically be a `String` or `&str`.  Note that this will
/// receive and ignore any message type.  Implemented using
/// [`Ret::panic`].
///
/// [`Ret::panic`]: struct.Ret.html#method.panic
/// [`Ret`]: struct.Ret.html
#[macro_export]
macro_rules! ret_panic {
    ($arg:expr) => {{
        $crate::COVERAGE!(ret_panic_0);
        $crate::Ret::panic($arg)
    }};
}

/// Create a [`Ret`] instance which does nothing at all
///
/// ```ignore
/// ret_nop!();
/// ```
///
/// NOP means "no operation".  Implemented using [`Ret::new`].
///
/// [`Ret::new`]: struct.Ret.html#method.new
/// [`Ret`]: struct.Ret.html
#[macro_export]
macro_rules! ret_nop {
    () => {{
        $crate::COVERAGE!(ret_nop_0);
        $crate::Ret::new(|_| {})
    }};
}

/// Create a [`Ret`] instance which shuts down the event loop
///
/// ```ignore
/// ret_shutdown!(core);
/// ```
///
/// This can be used as the notify handler on an actor to shut down
/// the event loop once that actor terminates.  The reason for the
/// actor's failure is passed through, and can be recovered after loop
/// termination using [`Core::shutdown_reason`].  See also
/// [`Ret::new`] and [`Core::shutdown`].
///
/// [`Core::shutdown_reason`]: struct.Core.html#method.shutdown_reason
/// [`Core::shutdown`]: struct.Core.html#method.shutdown
/// [`Ret::new`]: struct.Ret.html#method.new
/// [`Ret`]: struct.Ret.html
#[macro_export]
macro_rules! ret_shutdown {
    ($core:expr) => {{
        $crate::COVERAGE!(ret_shutdown_0);
        let core = $core.access_core();
        let deferrer = core.deferrer();
        $crate::Ret::new(move |m| {
            if let Some(cause) = m {
                deferrer.defer(|s| s.shutdown(cause));
            } else {
                deferrer.defer(|s| s.shutdown($crate::StopCause::Dropped));
            }
        })
    }};
}
