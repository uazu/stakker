//! [![license:MIT/Apache-2.0][1]](https://github.com/uazu/stakker)&nbsp;
//! [![github:uazu/stakker][2]](https://github.com/uazu/stakker)&nbsp;
//! [![crates.io:stakker][3]](https://crates.io/crates/stakker)&nbsp;
//! [![docs.rs:stakker][4]](https://docs.rs/stakker)&nbsp;
//! [![uazu.github.io:stakker][5]](https://uazu.github.io/stakker/)
//!
//! [1]: https://img.shields.io/badge/license-MIT%2FApache--2.0-blue
//! [2]: https://img.shields.io/badge/github-uazu%2Fstakker-brightgreen
//! [3]: https://img.shields.io/badge/crates.io-stakker-red
//! [4]: https://img.shields.io/badge/docs.rs-stakker-purple
//! [5]: https://img.shields.io/badge/uazu.github.io-stakker-yellow
//!
//! **Stakker** is a lightweight low-level single-threaded actor
//! runtime.  It is designed to be layered on top of whatever event
//! source or main loop the user prefers to use.  Asynchronous calls
//! are addressed to individual methods within an actor, rather like
//! Pony behaviours.  All calls and argument types are known and
//! statically checked at compile-time giving the optimiser a lot of
//! scope.  **Stakker** also provides a timer queue for timeouts or
//! delayed calls, a lazy queue to allow batching recent operations,
//! and an idle queue for running a call when nothing else is
//! outstanding.
//!
//! By default **Stakker** uses unsafe code for better time and memory
//! efficiency.  However if you prefer to avoid unsafe code, then
//! enable the **no-unsafe** feature which compiles the whole crate
//! with `forbid(unsafe_code)`.  Safe alternatives will be used, at
//! some cost in time and memory.  There are other features that
//! provide finer-grained control (see below).
//!
//! - [Overview of types](#overview-of-types)
//! - [Efficiency](#efficiency)
//! - [Cargo features](#cargo-features)
//! - [Testing](#testing)
//! - [Tutorial example](#tutorial-example)
//! - [Main loop examples](#main-loop-examples)
//! - [Why the name **Stakker**?](#why-the-name-stakker)
//!
//! See the [Stakker Guide and Design
//! Notes](https://uazu.github.io/stakker/) for additional
//! documentation.
//!
//!
//! # Overview of types
//!
//! [`Actor`] and [`ActorOwn`] are ref-counting references to an
//! actor.  Create an actor with [`actor!`] and call it with
//! [`call!`].
//!
//! [`Fwd`] and [`Ret`] forward data to another destination
//! asynchronously, typically to a particular entry-point in a
//! particular actor.  So [`Fwd`] and [`Ret`] instances take the role
//! of callback functions.  The difference between them is that
//! [`Fwd`] may be called multiple times, is ref-counted for cheap
//! cloning and is based on a `Fn` with `Copy`, whereas [`Ret`] can be
//! used only once, is based on `FnOnce` and is a "move" value.  Also
//! the [`Ret`] end-point is informed if the [`Ret`] instance is
//! dropped without sending back a message, for example if a zombie
//! actor is called.  See the [`fwd_*!`](#macros) and
//! [`ret_*!`](#macros) macros for creation of instances, and [`fwd!`]
//! and [`ret!`] to make use of them.
//!
//! [`Stakker`] is the external interface to the runtime, i.e. how it
//! is managed from the event loop, or during startup.
//!
//! [`Cx`] is the context passed to all actor methods.  It gives
//! access to methods related to the actor being called.  It also
//! gives access to [`Core`].
//!
//! [`Core`] is the part of [`Stakker`] which is accessible to actors
//! during actor calls via [`Cx`].  Both [`Stakker`] and [`Cx`]
//! references dereference to [`Core`] and can be used wherever a
//! [`Core`] ref is required.
//!
//! [`Share`] allows a mutable structure to be shared safely between
//! actors, a bit like IPC shared-memory but with guaranteed exclusive
//! access.  This may be used for efficiency, like shared-memory
//! buffers are sometimes used between OS processes.
//!
//! [`Deferrer`] allows queuing things to run from `Drop` handlers or
//! from other places in the main thread without access to [`Core`].
//! All actors have a built-in [`Deferrer`] which can be used from
//! outside the actor.
//!
//! For interfacing with other threads, [`PipedThread`] wraps a thread
//! and handles all data transfer to/from it and all cleanup.
//! [`Waker`] is a primitive which allows channels and other data
//! transfer to the main thread to be coordinated.
//!
//!
//! # Efficiency
//!
//! A significant aim in the development of **Stakker** was to be
//! lightweight and to minimize overheads in time and memory, and to
//! scale well.  Another significant aim was to be "as simple as
//! possible but no simpler", to try to find an optimal set of types
//! and operations that provide the required functionality and
//! ergonomics and that fit the Rust model, to make maximum use of the
//! guarantees that Rust provides.
//!
//! By default **Stakker** uses [`TCell`](https://docs.rs/qcell) or
//! [`TLCell`](https://docs.rs/qcell) for zero-cost protected access
//! to actor state, which also guarantees at compile-time that no
//! actor can directly access any other actor.
//!
//! By default a cut-down ref-counting implementation is used instead
//! of `Rc`, which saves around one `usize` per [`Actor`] or [`Fwd`]
//! instance.
//!
//! With default features, only one thread is allowed to run a
//! [`Stakker`] instance, which enables an optimisation which uses a
//! global variable for the [`Deferrer`] defer queue (used for drop
//! handlers).  However if more [`Stakker`] instances need to be run,
//! then the **multi-thread** or **multi-stakker** features cause it
//! to use alternative implementations.
//!
//! All deferred operations, including all async actor calls, are
//! handled as `FnOnce` instances on a queue.  The aim is to make this
//! cheap enough so that deferring something doesn't have to be a big
//! decision.  Thanks to Rust's inlining, these are efficient -- the
//! compiler might even choose to inline the internal code of the
//! actor call into the `FnOnce`, as that is all known at
//! compile-time.
//!
//! By default the `FnOnce` queue is a flat heterogeneous queue,
//! storing the closures directly in a byte `Vec`, which should give
//! best performance and cache locality at the cost of some unsafe
//! code.  However a fully-safe boxed closure queue implementation is
//! also available.
//!
//! Forwarding handlers ([`Fwd`]) are boxed `Fn` instances along with
//! a ref-count.  Return handlers ([`Ret`]) are boxed `FnOnce`
//! instances.  Both typically queue a `FnOnce` operation when
//! provided with arguments.  These are also efficient due to
//! inlining.  In this case two chunks of inlined code are generated
//! for each by the compiler: the first which accepts arguments and
//! pushes the second one onto the queue.
//!
//! If no inter-thread operations are active, then **Stakker** will
//! never do locking or any atomic operations, nor block for any
//! reason.  So the code can execute at full speed without triggering
//! any CPU memory fences or whatever.  Usually the only thing that
//! blocks would be the external I/O poller whilst waiting for I/O or
//! timer expiry.  When other threads have been started and they defer
//! wake-ups to the main thread, this is handled as an I/O event which
//! causes the wake flags to be checked using atomic operations.
//!
//!
//! # Cargo features
//!
//! Cargo features in **Stakker** do not change **Stakker**'s public
//! API.  The API stays the same, but the implementation behind the
//! API changes.
//!
//! Also, cargo features are additive.  This means that if one crate
//! using **Stakker** enables a feature, then it is enabled for all
//! uses of **Stakker** in the build.  So when features switch between
//! alternative implementations, enabling a feature has to result in
//! the more tolerant implementation, because all users of the crate
//! have to be able to work with this configuration.  This usually
//! means that features switch from the most efficient and restrictive
//! implementation, to a less efficient but more flexible one.
//!
//! So using the default features is the best choice unless you have
//! specific requirements.  When a crate that uses **Stakker** doesn't
//! care about whether a feature is enabled or not, it should avoid
//! setting it and leave it up to the application to choose.
//!
//! Features enabled by default:
//!
//! - **anymap**: Brings in the `anymap` crate.  When enabled,
//! [`Stakker`] keeps an `AnyMap` which can be used to store and
//! access Stakker-wide values.  The intended use is for passing
//! things related to the outer context through to actors, such as an
//! I/O polling instance.  The alternative is to pass these through on
//! actor creation.
//!
//! - **inter-thread**: Enables inter-thread operations such as
//! [`Waker`] and [`PipedThread`].
//!
//! Optional features:
//!
//! - **no-unsafe-queue**: Disable the fast FnOnce queue implementation,
//! which uses unsafe code.  Uses a boxed queue instead.
//!
//! - **no-unsafe**: Disable all unsafe code within this crate, at
//! some cost in time and memory.
//!
//! - **multi-thread**: Specifies that more than one **Stakker** will
//! run in the process, at most one **Stakker** per thread.  This
//! disables some optimisations that require process-wide access.
//!
//! - **multi-stakker**: Specifies that more than one **Stakker** may
//! need to run in the same thread.  This disables optimisations that
//! require either process-wide or thread-local access.
//!
//! - **inline-deferrer**: Forces use of the inline [`Deferrer`]
//! implementation instead of using the global or thread-local
//! implementation.  Possibly useful if thread-locals are very slow.
//!
//! - **logger**: Enables **Stakker**'s core logging feature, which
//! logs actor startup and termination, and which allows macros from
//! the `stakker_log` crate to log with actor context information.
//! See [`Stakker::set_logger`].
//!
//! Testing features:
//!
//! - **test-multi-thread**: This is enabled by default, because
//! otherwise a plain `cargo test` or crater run test will fail.  It
//! has the effect of forcing **multi-thread** to be enabled when a
//! test runs.  This is to work around there being no way to force
//! `cargo test` to run tests on a single thread without giving it an
//! extra argument or environment variable.  This feature should be
//! disabled if you want to be sure that you're testing with exactly
//! the same features as the main build of your application.  It will
//! be removed if another solution can be found.
//!
//! These are the implementations that are switched, in order of
//! preference, listing most-preferred first:
//!
//! ### Cell type
//!
//! - `TCell`: Best performance, but only allows a single **Stakker**
//! per process
//!
//! - `TLCell`: Best performance, but uses thread-locals at
//! **Stakker** creation time and only allows a single **Stakker** per
//! thread
//!
//! - `QCell`: Allows many **Stakker** instances per thread at some
//! cost in time and memory
//!
//! ### Deferrer
//!
//! - Global deferrer: Uses a global variable to find the [`Deferrer`]
//!
//! - Thread-local deferrer: Uses a thread-local to find the
//! [`Deferrer`], with safe and unsafe variants
//!
//! - Inline deferrer: Keeps references to the [`Deferrer`] in all
//! places where it is needed, with safe and unsafe variants.  In
//! particular this adds a `usize` to all actors.
//!
//! ### Actor ref-counting
//!
//! - Packed: Uses a little unsafe code to save a `usize` per actor
//!
//! - Standard: Uses `std::rc::Rc`
//!
//! ### Call queues
//!
//! - Fast `FnOnce` queue: Appends `FnOnce` closures directly to a
//! flat memory buffer.  Gives best performance, but uses `unsafe`
//! code.
//!
//! - Boxed queue: Stores closures indirectly by boxing them
//!
//!
//! # Testing
//!
//! **Stakker** has unit and doc tests that give over 90% coverage
//! across all feature combinations.  These tests also run cleanly
//! under valgrind and MIRI.  In addition there are some fuzz tests
//! and stress tests under `extra/` that further exercise particular
//! components to verify that they operate as expected.
//!
//!
//! # Tutorial example
//!
//! ```
//!# use stakker::{actor, after, call, ret_nop, ret_shutdown, fwd_to, ret, ret_some_to};
//!# use stakker::{Actor, CX, Fwd, Stakker, Ret};
//!# use std::time::{Duration, Instant};
//!#
//! // An actor is represented as a struct which holds the actor state
//! struct Light {
//!     start: Instant,
//!     on: bool,
//! }
//!
//! impl Light {
//!     // This is a "Prep" method which is used to create a Self value
//!     // for the actor.  `cx` is the actor context and gives access to
//!     // Stakker `Core`.  (`CX![]` expands to `&mut Cx<'_, Self>`.)
//!     // A "Prep" method doesn't have to return a Self value right away.
//!     // For example it might asynchronously attempt a connection to a
//!     // remote server first before arranging a call to another "Prep"
//!     // function which returns the Self value.  Once a value is returned,
//!     // the actor is "Ready" and any queued-up operations on the actor
//!     // will be executed.
//!     pub fn init(cx: CX![]) -> Option<Self> {
//!         // Use cx.now() instead of Instant::now() to allow execution
//!         // in virtual time if supported by the environment.
//!         let start = cx.now();
//!         Some(Self { start, on: false })
//!     }
//!
//!     // Methods that may be called once the actor is "Ready" have a
//!     // `&mut self` or `&self` first argument.
//!     pub fn set(&mut self, cx: CX![], on: bool) {
//!         self.on = on;
//!         let time = cx.now() - self.start;
//!         println!("{:04}.{:03} Light on: {}", time.as_secs(), time.subsec_millis(), on);
//!     }
//!
//!     // A `Fwd` or `Ret` allows passing data to arbitrary destinations,
//!     // like an async callback.  Here we use it to return a value.
//!     pub fn query(&self, cx: CX![], ret: Ret<bool>) {
//!         ret!([ret], self.on);
//!     }
//! }
//!
//! // This is another actor that holds a reference to a Light actor.
//! struct Flasher {
//!     light: Actor<Light>,
//!     interval: Duration,
//!     count: usize,
//! }
//!
//! impl Flasher {
//!     pub fn init(cx: CX![], light: Actor<Light>,
//!                 interval: Duration, count: usize) -> Option<Self> {
//!         // Defer first switch to the queue
//!         call!([cx], switch(true));
//!         Some(Self { light, interval, count })
//!     }
//!
//!     pub fn switch(&mut self, cx: CX![], on: bool) {
//!         // Change the light state
//!         call!([self.light], set(on));
//!
//!         self.count -= 1;
//!         if self.count != 0 {
//!             // Call switch again after a delay
//!             after!(self.interval, [cx], switch(!on));
//!         } else {
//!             // Terminate the actor successfully, causing StopCause handler to run
//!             cx.stop();
//!         }
//!
//!         // Query the light state, receiving the response in the method
//!         // `recv_state`, which has both fixed and forwarded arguments.
//!         let ret = ret_some_to!([cx], recv_state(self.count) as (bool));
//!         call!([self.light], query(ret));
//!     }
//!
//!     fn recv_state(&self, _: CX![], count: usize, state: bool) {
//!         println!("  (at count {} received: {})", count, state);
//!     }
//! }
//!
//! let mut stakker0 = Stakker::new(Instant::now());
//! let stakker = &mut stakker0;
//!
//! // Create and initialise the Light and Flasher actors.  The
//! // Flasher actor is given a reference to the Light.  Use a
//! // StopCause handler to shutdown when the Flasher terminates.
//! let light = actor!(stakker, Light::init(), ret_nop!());
//! let _flasher = actor!(
//!     stakker,
//!     Flasher::init(light.clone(), Duration::from_secs(1), 6),
//!     ret_shutdown!(stakker)
//! );
//!
//! // Since we're not in virtual time, we use `Instant::now()` in
//! // this loop, which is then passed on to all the actors as
//! // `cx.now()`.  (If you want to run time faster or slower you
//! // could use another source of time.)  So all calls in a batch of
//! // processing get the same `cx.now()` value.  Also note that
//! // `Instant::now()` uses a Mutex on some platforms so it saves
//! // cycles to call it less often.
//! stakker.run(Instant::now(), false);
//!# if false {
//! while stakker.not_shutdown() {
//!     // Wait for next timer to expire.  Here there's no I/O polling
//!     // required to wait for external events, so just `sleep`
//!     let maxdur = stakker.next_wait_max(Instant::now(), Duration::from_secs(60), false);
//!     std::thread::sleep(maxdur);
//!
//!     // Run queue and timers
//!     stakker.run(Instant::now(), false);
//! }
//!# } else {  // Use virtual time version when testing
//!#     let mut now = Instant::now();
//!#     while stakker.not_shutdown() {
//!#         now += stakker.next_wait_max(now, Duration::from_secs(60), false);
//!#         stakker.run(now, false);
//!#     }
//!# }
//! ```
//!
//!
//! # Main loop examples
//!
//! Note that the 60s duration used below just means that the process
//! will wake every 60s if nothing else is going on.  You could make
//! this a larger value.
//!
//! ### Virtual time main loop, no I/O, no idle queue handling
//!
//! ```no_run
//!# use stakker::Stakker;
//!# use std::time::{Duration, Instant};
//!# fn test(stakker: &mut Stakker) {
//! let mut now = Instant::now();
//! stakker.run(now, false);
//! while stakker.not_shutdown() {
//!     now += stakker.next_wait_max(now, Duration::from_secs(60), false);
//!     stakker.run(now, false);
//! }
//!# }
//! ```
//!
//! ### Real time main loop, no I/O, no idle queue handling
//!
//! ```no_run
//!# use stakker::Stakker;
//!# use std::time::{Duration, Instant};
//!# fn test(stakker: &mut Stakker) {
//! stakker.run(Instant::now(), false);
//! while stakker.not_shutdown() {
//!     let maxdur = stakker.next_wait_max(Instant::now(), Duration::from_secs(60), false);
//!     std::thread::sleep(maxdur);
//!     stakker.run(Instant::now(), false);
//! }
//!# }
//! ```
//!
//! ### Real time I/O poller main loop, with idle queue handling
//!
//! This example uses `MioPoll` from the `stakker_mio` crate.
//!
//! ```no_run
//!# use stakker::Stakker;
//!# use std::time::{Duration, Instant};
//!# struct MioPoll;
//!# impl MioPoll { fn poll(&self, s: &mut Stakker, d: Duration) -> std::io::Result<bool> { Ok(false) } }
//!# fn test(stakker: &mut Stakker, miopoll: &mut MioPoll) -> std::io::Result<()> {
//! let mut idle_pending = stakker.run(Instant::now(), false);
//! while stakker.not_shutdown() {
//!     let maxdur = stakker.next_wait_max(Instant::now(), Duration::from_secs(60), idle_pending);
//!     let activity = miopoll.poll(stakker, maxdur)?;
//!     idle_pending = stakker.run(Instant::now(), !activity);
//! }
//!#     Ok(())
//!# }
//! ```
//!
//! The way this works is that if there are idle queue items pending,
//! then `next_wait_max` returns 0s, which means that the `poll` call
//! only checks for new I/O events without blocking.  If there are no
//! new events (`activity` is false), then an item from the idle queue
//! is run.
//!
//!
//! # Why the name **Stakker**?
//!
//! "Single-threaded actor runtime" &rarr; STACR &rarr; **Stakker**.
//! The name is also a small tribute to the 1988 Humanoid track
//! "Stakker Humanoid", which borrows samples from the early video
//! game **Berzerk**, and which rolls along quite economically as I
//! hope the **Stakker** runtime also does.
//!
//! [`ActorOwn`]: struct.ActorOwn.html
//! [`Actor`]: struct.Actor.html
//! [`Core`]: struct.Core.html
//! [`Cx`]: struct.Cx.html
//! [`Deferrer`]: struct.Deferrer.html
//! [`Fwd`]: struct.Fwd.html
//! [`PipedThread`]: struct.PipedThread.html
//! [`Ret`]: struct.Ret.html
//! [`Share`]: struct.Share.html
//! [`Stakker::set_logger`]: struct.Stakker.html#method.set_logger
//! [`Stakker`]: struct.Stakker.html
//! [`Waker`]: struct.Waker.html
//! [`actor!`]: macro.actor.html
//! [`call!`]: macro.call.html
//! [`fwd!`]: macro.fwd.html
//! [`ret!`]: macro.ret.html

// Insist on 2018 style
#![deny(rust_2018_idioms)]
// No unsafe code is allowed anywhere if no-unsafe is set
#![cfg_attr(feature = "no-unsafe", forbid(unsafe_code))]
// To fix these would break the API
#![allow(clippy::upper_case_acronyms)]

// TODO: Illustrate Fwd in the tutorial example, e.g. make println!
// output go via a Fwd

pub use crate::core::{Core, Stakker};
pub use crate::log::{LogFilter, LogID, LogLevel, LogLevelError, LogRecord, LogVisitor};
pub use actor::{Actor, ActorOwn, ActorOwnAnon, Cx, StopCause};
pub use deferrer::Deferrer;
pub use fwd::Fwd;
pub use ret::Ret;
pub use share::Share;
pub use thread::{PipedLink, PipedThread};
pub use timers::{FixedTimerKey, MaxTimerKey, MinTimerKey};
pub use waker::Waker;

// Trait checks
static_assertions::assert_not_impl_any!(Stakker: Send, Sync, Copy, Clone);
static_assertions::assert_not_impl_any!(Core: Send, Sync, Copy, Clone);
static_assertions::assert_not_impl_any!(Cx<'_, u8>: Send, Sync, Copy, Clone);
static_assertions::assert_not_impl_any!(Ret<u8>: Send, Sync, Copy, Clone);
static_assertions::assert_not_impl_any!(Actor<u8>: Send, Sync, Copy);
static_assertions::assert_not_impl_any!(task::Task: Send, Sync, Copy);
static_assertions::assert_not_impl_any!(Deferrer: Send, Sync, Copy);
static_assertions::assert_not_impl_any!(Share<u8>: Send, Sync, Copy);
static_assertions::assert_not_impl_any!(Fwd<u8>: Send, Sync, Copy);
static_assertions::assert_not_impl_any!(Waker: Copy, Clone);
static_assertions::assert_impl_all!(Actor<u8>: Clone);
static_assertions::assert_impl_all!(Deferrer: Clone);
static_assertions::assert_impl_all!(Share<u8>: Clone);
static_assertions::assert_impl_all!(Fwd<u8>: Clone);
static_assertions::assert_impl_all!(Waker: Send, Sync);
static_assertions::assert_impl_all!(FixedTimerKey: Copy, Clone);
static_assertions::assert_impl_all!(MaxTimerKey: Copy, Clone);
static_assertions::assert_impl_all!(MinTimerKey: Copy, Clone);

mod actor;
mod core;
mod fwd;
mod log;
mod macros;
mod ret;
mod share;
pub mod task;
mod thread;
mod timers;
mod waker;

#[cfg(test)]
mod test;

// Ref-counting selections
#[cfg(not(feature = "no-unsafe"))]
mod rc {
    pub(crate) mod count;
    pub(crate) mod minrc;

    pub(crate) mod actorrc_packed;
    pub(crate) use actorrc_packed::ActorRc;

    pub(crate) mod fwdrc_min;
    pub(crate) use fwdrc_min::FwdRc;
}
#[cfg(feature = "no-unsafe")]
mod rc {
    pub(crate) mod count;

    pub(crate) mod actorrc_std;
    pub(crate) use actorrc_std::ActorRc;

    pub(crate) mod fwdrc_std;
    pub(crate) use fwdrc_std::FwdRc;
}

// Deferrer selection
#[cfg(all(
    not(feature = "inline-deferrer"),
    not(feature = "multi-stakker"),
    not(any(feature = "multi-thread", all(test, feature = "test-multi-thread"))),
    not(feature = "no-unsafe")
))]
mod deferrer {
    mod api;
    pub use api::Deferrer;

    mod global;
    use global::DeferrerAux;
}

#[cfg(all(
    not(feature = "inline-deferrer"),
    not(feature = "multi-stakker"),
    any(feature = "multi-thread", all(test, feature = "test-multi-thread")),
))]
mod deferrer {
    mod api;
    pub use api::Deferrer;

    #[cfg(feature = "no-unsafe")]
    mod thread_local_safe;
    #[cfg(feature = "no-unsafe")]
    use thread_local_safe::DeferrerAux;
    #[cfg(not(feature = "no-unsafe"))]
    mod thread_local;
    #[cfg(not(feature = "no-unsafe"))]
    use thread_local::DeferrerAux;
}

// Inline deferrer used if neither of the other options fits.  Clearer
// to not simplify this boolean expression, because the subexpressions
// should match the expressions above.
#[cfg(all(
    not(all(
        not(feature = "inline-deferrer"),
        not(feature = "multi-stakker"),
        not(any(feature = "multi-thread", all(test, feature = "test-multi-thread"))),
        not(feature = "no-unsafe")
    )),
    not(all(
        not(feature = "inline-deferrer"),
        not(feature = "multi-stakker"),
        any(feature = "multi-thread", all(test, feature = "test-multi-thread")),
    )),
))]
mod deferrer {
    mod api;
    pub use api::Deferrer;

    #[cfg(feature = "no-unsafe")]
    mod inline_safe;
    #[cfg(feature = "no-unsafe")]
    use inline_safe::DeferrerAux;
    #[cfg(not(feature = "no-unsafe"))]
    mod inline;
    #[cfg(not(feature = "no-unsafe"))]
    use inline::DeferrerAux;
}

// FnOnceQueue selection
#[cfg(not(any(feature = "no-unsafe", feature = "no-unsafe-queue")))]
mod queue {
    mod flat;
    pub(crate) use flat::FnOnceQueue;
}

#[cfg(any(feature = "no-unsafe", feature = "no-unsafe-queue"))]
mod queue {
    mod boxed;
    pub(crate) use boxed::FnOnceQueue;
}

// Cell selection
#[cfg(all(
    not(feature = "multi-stakker"),
    not(any(feature = "multi-thread", all(test, feature = "test-multi-thread")))
))]
mod cell {
    pub(crate) mod tcell;
    pub(crate) use tcell as cell;
}

#[cfg(all(
    not(feature = "multi-stakker"),
    any(feature = "multi-thread", all(test, feature = "test-multi-thread"))
))]
mod cell {
    pub(crate) mod tlcell;
    pub(crate) use tlcell as cell;
}

#[cfg(feature = "multi-stakker")]
mod cell {
    pub(crate) mod qcell;
    pub(crate) use self::qcell as cell;
}
