//! **Stakker** is a lightweight low-level single-threaded actor
//! runtime.  It is designed to be layered on top of whatever event
//! loop the user prefers to use.  Asynchronous calls are addressed to
//! individual methods within an actor, rather like Pony behaviours.
//! All calls and argument types are known and statically checked at
//! compile-time giving the optimiser a lot of scope.  **Stakker**
//! also provides a timer queue for timeouts or delayed calls, a lazy
//! queue to allow batching recent operations, and an idle queue for
//! running a call when nothing else is outstanding.
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
//! - [Tutorial example](#tutorial-example)
//! - [Main loop examples](#main-loop-examples)
//! - [Why the name **Stakker**?](#why-the-name-stakker)
//!
//! # Overview of types
//!
//! [`Actor`] and [`ActorOwn`] are ref-counting references to an
//! actor.  Create an actor with [`actor!`] and call it with
//! [`call!`].
//!
//! [`Fwd`] forwards data to another destination, typically to a
//! particular entry-point in a particular actor.  So `Fwd` instances
//! take the role of callback functions.  They are ref-counted, so can
//! be cloned cheaply.  See the [`fwd_*!`](#macros) macros for
//! creation of `Fwd` instances, and [`call!`] to make use of them.
//!
//! [`Stakker`] is the external interface to the runtime, i.e. how it
//! is managed from the event loop, or during startup.
//!
//! [`Core`] is the part of the `Stakker` API which is also accessible
//! to the actors during actor calls.
//!
//! [`Cx`] is the context passed to an actor entry-point.  It gives
//! access to `Core` and also methods related to the actor being
//! called.
//!
//! [`Share`] allows a mutable structure to be shared safely between
//! actors, a bit like IPC shared-memory but with guaranteed exclusive
//! access.  This may be used for efficiency, like shared-memory
//! buffers are sometimes used between OS processes.
//!
//! [`Deferrer`] allows queuing things to run from `Drop` handlers or
//! from other places in the main thread without access to `Core`.
//!
//! For interfacing with other threads, [`PipedThread`] wraps a thread
//! and handles all data transfer to/from it and all cleanup.
//! [`Waker`] is a primitive which allows channels and other data
//! transfer to the main thread to be coordinated.
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
//! of `Rc`, which saves around two `usize` per actor and one `usize`
//! per `Fwd`.
//!
//! By default only one thread is allowed to run a `Stakker` instance,
//! which allows a global variable to be used for the `Deferrer` lazy
//! defer queue (used for drop handlers).  However if more `Stakker`
//! instances need to be run, then the **multi-thread** or
//! **multi-stakker** features select other implementations instead.
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
//! a ref-count, which typically queue a `FnOnce` operation when
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
//! Cargo features are additive.  This means that if one use of a
//! crate enables a feature, it is enabled for all uses of that crate
//! in the build.  So when features switch between alternative
//! implementations, it's necessary to decide whether to make the
//! feature a negative or positive switch, depending on which of two
//! options is more tolerant when different uses of the crate within
//! the build have different needs.
//!
//! When a crate using this crate doesn't care about whether a feature
//! is enabled or not, it should avoid setting it and leave it up to
//! the application to choose.  Note that features do not change the
//! public API of the crate, although disabling the enabled-by-default
//! features will cause disabled operations to panic.
//!
//! Enabled by default:
//!
//! - **anymap**: Brings in the `anymap` crate.  When enabled,
//! `Stakker` keeps an `AnyMap` which can be used to store and access
//! Stakker-wide values.  The intended use is for passing things
//! related to the outer context through to actors, such as an I/O
//! polling instance.  The alternative is to pass these through on
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
//! - **inline-deferrer**: Forces use of the inline `Deferrer`
//! implementation instead of using the global or thread-local
//! implementation.  Possibly useful if thread-locals are very slow.
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
//! ### Drop deferrer
//!
//! - Global deferrer: Uses a global variable to find the `Deferrer`
//!
//! - Thread-local deferrer: Uses a thread-local to find the `Deferrer`
//!
//! - Inline deferrer: Keeps references to the `Deferrer` in all
//! places where it is needed.  In particular this adds a `usize` to
//! all actors.
//!
//! ### Actor ref-counting
//!
//! - Packed: Uses a little unsafe code to save two `usize` per actor
//!
//! - Standard: Uses `std::rc::Rc`
//!
//! ### Call queues
//!
//! - Fast `FnOnce` queue: Stores `FnOnce` closures directly in a flat
//! `Vec<u8>`.  Gives best performance, but uses `unsafe` code.
//!
//! - Boxed queue: Stores closures indirectly through boxing them
//!
//!
//! # Tutorial example
//!
//! ```
//!# use stakker::{actor, after, call, fwd_nop, fwd_shutdown, fwd_to};
//!# use stakker::{Actor, Cx, Fwd, Stakker};
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
//!     // Stakker `Core`.  A "Prep" method doesn't have to return a Self
//!     // value right away.  For example it might asynchronously attempt
//!     // a connection to a remote server first before arranging a call
//!     // to another "Prep" function which returns the Self value.  Once
//!     // a value is returned, the actor is "Ready" and any queued-up
//!     // operations on the actor will be executed.
//!     pub fn init(cx: &mut Cx<Self>) -> Option<Self> {
//!         // Use cx.now() instead of Instant::now() to allow execution
//!         // in virtual time if supported by the environment.
//!         let start = cx.now();
//!         Some(Self { start, on: false })
//!     }
//!
//!     // Methods that may be called once the actor is "Ready" have a
//!     // `&mut self` or `&self` first argument.
//!     pub fn set(&mut self, cx: &mut Cx<Self>, on: bool) {
//!         self.on = on;
//!         let time = cx.now() - self.start;
//!         println!("{:04}.{:03} Light on: {}", time.as_secs(), time.subsec_millis(), on);
//!     }
//!
//!     // A `Fwd` instance allows forwarding data to arbitrary
//!     // destinations, like an async callback.
//!     pub fn query(&self, cx: &mut Cx<Self>, fwd: Fwd<bool>) {
//!         call!([fwd], cx, self.on);
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
//!     pub fn init(cx: &mut Cx<Self>, light: Actor<Light>,
//!                 interval: Duration, count: usize) -> Option<Self> {
//!         // Defer first switch to the queue
//!         call!(switch(cx, true));
//!         Some(Self { light, interval, count })
//!     }
//!
//!     pub fn switch(&mut self, cx: &mut Cx<Self>, on: bool) {
//!         // Change the light state
//!         call!([self.light], set(cx, on));
//!
//!         self.count -= 1;
//!         if self.count != 0 {
//!             // Call switch again after a delay
//!             after!(self.interval, switch(cx, !on));
//!         } else {
//!             // Terminate the actor successfully, causing notify to run
//!             cx.stop();
//!         }
//!
//!         // Query the light state, receiving the response in the method
//!         // `recv_state`, which has both fixed and forwarded arguments.
//!         let fwd = fwd_to!(recv_state(cx, self.count) as (bool));
//!         call!([self.light], query(cx, fwd));
//!     }
//!     
//!     fn recv_state(&self, _: &mut Cx<Self>, count: usize, state: bool) {
//!         println!("  (at count {} received: {})", count, state);
//!     }
//! }
//!
//! let mut stakker0 = Stakker::new(Instant::now());
//! let stakker = &mut stakker0;
//!
//! // Create and initialise the Light and Flasher actors.  The
//! // Flasher actor is given a reference to the Light.  Use a
//! // notification handler to shutdown when the Flasher terminates.
//! let light = actor!(Light::init(stakker), fwd_nop!());
//! let _flasher = actor!(
//!     Flasher::init(stakker, light.clone(), Duration::from_secs(1), 6),
//!     fwd_shutdown!()
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
//!#         now += stakker.next_wait_max(Instant::now(), Duration::from_secs(60), false);
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
//!     now += stakker.next_wait_max(Instant::now(), Duration::from_secs(60), false);
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
//! only checks for new I/O events without blocking.  If there is no
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
//! [`Share`]: struct.Share.html
//! [`Stakker`]: struct.Stakker.html
//! [`Waker`]: struct.Waker.html
//! [`actor!`]: macro.actor.html
//! [`call!`]: macro.call.html

// Insist on 2018 style
#![deny(rust_2018_idioms)]
// No unsafe code is allowed anywhere if no-unsafe is set
#![cfg_attr(feature = "no-unsafe", forbid(unsafe_code))]

pub use crate::core::{Core, Stakker};
pub use actor::{Actor, ActorDied, ActorOwn, Cx};
pub use deferrer::Deferrer;
pub use fwd::Fwd;
pub use share::Share;
pub use thread::{PipedLink, PipedThread};
pub use timers::{FixedTimerKey, MaxTimerKey, MinTimerKey};
pub use waker::Waker;

// Static assertions
static_assertions::assert_not_impl_any!(Actor<u8>: Send, Sync);
static_assertions::assert_not_impl_any!(Stakker: Send, Sync);
static_assertions::assert_not_impl_any!(Core: Send, Sync);
static_assertions::assert_not_impl_any!(Cx<'_, u8>: Send, Sync);
static_assertions::assert_not_impl_any!(Deferrer: Send, Sync);
static_assertions::assert_not_impl_any!(Share<u8>: Send, Sync);
static_assertions::assert_not_impl_any!(Fwd<u8>: Send, Sync);
static_assertions::assert_not_impl_any!(Waker: Clone);
static_assertions::assert_impl_all!(Share<u8>: Clone);
static_assertions::assert_impl_all!(Fwd<u8>: Clone);
static_assertions::assert_impl_all!(Waker: Send, Sync);
static_assertions::assert_impl_all!(FixedTimerKey: Copy, Clone);
static_assertions::assert_impl_all!(MaxTimerKey: Copy, Clone);
static_assertions::assert_impl_all!(MinTimerKey: Copy, Clone);

mod actor;
mod core;
mod fwd;
mod macros;
mod share;
mod thread;
mod timers;
mod waker;

// Ref-counting selections
#[cfg(not(feature = "no-unsafe"))]
mod rc {
    pub(crate) mod minrc;

    pub(crate) mod actorrc_packed;
    pub(crate) use actorrc_packed::ActorRc;

    pub(crate) mod fwdrc_min;
    pub(crate) use fwdrc_min::FwdRc;
}
#[cfg(feature = "no-unsafe")]
mod rc {
    pub(crate) mod actorrc_std;
    pub(crate) use actorrc_std::ActorRc;

    pub(crate) mod fwdrc_std;
    pub(crate) use fwdrc_std::FwdRc;
}

// Deferrer selection
#[cfg(all(
    not(feature = "inline-deferrer"),
    not(feature = "multi-stakker"),
    not(feature = "multi-thread"),
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
    feature = "multi-thread",
))]
mod deferrer {
    mod api;
    pub use api::Deferrer;

    mod thread_local;
    use thread_local::DeferrerAux;
}

// Inline deferrer used if neither of the other options fits.  Clearer
// to not simplify this boolean expression, because the subexpressions
// should match the expressions above.
#[cfg(all(
    not(all(
        not(feature = "inline-deferrer"),
        not(feature = "multi-stakker"),
        not(feature = "multi-thread"),
        not(feature = "no-unsafe")
    )),
    not(all(
        not(feature = "inline-deferrer"),
        not(feature = "multi-stakker"),
        feature = "multi-thread",
    ))
))]
mod deferrer {
    mod api;
    pub use api::Deferrer;

    mod inline;
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
#[cfg(all(not(feature = "multi-stakker"), not(feature = "multi-thread")))]
mod cell {
    // For testing we have to protect the TCellOwner from use in parallel
    #[cfg(test)]
    pub(crate) mod protected_tcellowner;
    #[cfg(test)]
    pub(crate) use protected_tcellowner::ProtectedTCellOwner as TCellOwner;
    #[cfg(not(test))]
    pub(crate) use qcell::TCellOwner;

    pub(crate) mod tcell;
    pub(crate) use tcell as cell;
}

#[cfg(all(not(feature = "multi-stakker"), feature = "multi-thread"))]
mod cell {
    pub(crate) mod tlcell;
    pub(crate) use tlcell as cell;
}

#[cfg(all(feature = "multi-stakker"))]
mod cell {
    pub(crate) mod qcell;
    pub(crate) use self::qcell as cell;
}
