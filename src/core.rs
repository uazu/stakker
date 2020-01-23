use crate::cell::cell::{new_actor_cell_owner, new_share_cell_owner};
use crate::cell::cell::{ActorCellMaker, ActorCellOwner, ShareCellOwner};
use crate::queue::FnOnceQueue;
use crate::timers::Timers;
use crate::waker::WakeHandlers;
use crate::{ActorDied, Deferrer, FixedTimerKey, MaxTimerKey, MinTimerKey, Waker};
use std::collections::VecDeque;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::time::{Duration, Instant};

/// The external interface to the actor runtime
///
/// It contains all the queues and timers, and controls access to the
/// state of all the actors.  It also provides the interface to
/// control all this from outside, i.e. the calls used by an event
/// loop.  The `Stakker` instance itself is not accessible from actors
/// due to borrowing restrictions.  It derefs to `&mut Core` (through
/// auto-deref or `*stakker`).  [`Core`] is also accessible to actors
/// through their [`Cx`] context reference.
///
/// [`Core`]: struct.Core.html
/// [`Cx`]: struct.Cx.html
pub struct Stakker {
    pub(crate) core: Core,
    pub(crate) actor_owner: ActorCellOwner,
    spare_queue: FnOnceQueue<Stakker>,
    recreate_queues_time: Instant,
}

impl Stakker {
    /// Construct a `Stakker` instance.  Whether more than one
    /// instance can be created in each process or thread depends on
    /// the [Cargo features](index.html#cargo-features) enabled.
    pub fn new(now: Instant) -> Self {
        // Do this first to get the uniqueness checks to fail early,
        let (actor_owner, actor_maker) = new_actor_cell_owner();
        Self {
            core: Core::new(now, actor_maker),
            actor_owner,
            spare_queue: FnOnceQueue::new(),
            recreate_queues_time: now + Duration::from_secs(60),
        }
    }

    /// Return the next timer expiry time, or None
    pub fn next_expiry(&mut self) -> Option<Instant> {
        self.core.timers.next_expiry()
    }

    /// Return how long we need to wait for the next timer, or None if
    /// there are no timers to wait for
    pub fn next_wait(&mut self, now: Instant) -> Option<Duration> {
        self.core
            .timers
            .next_expiry()
            .map(|t| t.saturating_duration_since(now))
    }

    /// Return how long to wait for the next I/O poll.  If there are
    /// idle items queued (`idle_pending` is true), return 0 seconds,
    /// which allows the caller to quickly check for more I/O and then
    /// run the idle queue if there is nothing to do.  If there is a
    /// timer active, return the time to wait for that timer, limited
    /// by `maxdur`.  If there is nothing to wait for, just return
    /// `maxdur`.
    pub fn next_wait_max(
        &mut self,
        now: Instant,
        maxdur: Duration,
        idle_pending: bool,
    ) -> Duration {
        if idle_pending {
            Duration::from_secs(0)
        } else {
            self.core
                .timers
                .next_expiry()
                .map(|t| t.saturating_duration_since(now).min(maxdur))
                .unwrap_or(maxdur)
        }
    }

    /// Move time forward, expire any timers onto the defer queue,
    /// then run defer and lazy queues until there is nothing
    /// outstanding.  Returns `true` if there are idle items still to
    /// run.
    ///
    /// If `idle` is true, then runs an item from the idle queue as
    /// well.  This should be set only if we've already run the queues
    /// and just polled I/O (without waiting) and still there's
    /// nothing to do.
    ///
    /// Note: All actors should use `cx.now()` to get the time, which
    /// allows the entire system to be run in virtual time (unrelated
    /// to real time) if necessary.
    pub fn run(&mut self, now: Instant, idle: bool) -> bool {
        if now > self.core.now {
            self.core.now = now;
            self.core.timers.advance(now, &mut self.core.queue);
        }

        if idle {
            if let Some(cb) = self.core.idle_queue.pop_front() {
                cb(self);
            }
        }

        // Re-use queues in an attempt to keep using the same memory,
        // which is more cache-efficient.  However if there has been a
        // big burst of activity, the queues might have grown huge.
        // So periodically force the queues to be recreated.
        let mut spare_queue = mem::replace(&mut self.spare_queue, FnOnceQueue::new());
        loop {
            if !self.core.queue.is_empty() {
                mem::swap(&mut self.core.queue, &mut spare_queue);
                spare_queue.execute(self);
                continue;
            }
            spare_queue = self.core.deferrer.replace_queue(spare_queue);
            if !spare_queue.is_empty() {
                spare_queue.execute(self);
                continue;
            }
            break;
        }

        // Recreate the queues?  They will all be empty at this point.
        if now > self.recreate_queues_time {
            spare_queue = FnOnceQueue::new();
            self.core.queue = FnOnceQueue::new();
            self.core.deferrer.replace_queue(FnOnceQueue::new());
            self.recreate_queues_time = now + Duration::from_secs(60);
        }
        self.spare_queue = spare_queue;

        !self.core.idle_queue.is_empty()
    }

    /// Put a value into the `anymap`.  This can be accessed using the
    /// [`Core::anymap_get`] or [`Core::anymap_try_get`] call.  An
    /// anymap can store one value for each type (see crate
    /// [`anymap`](https://docs.rs/anymap)).  The value must implement
    /// `Clone`, i.e. it must act something like an `Rc` or else be
    /// copyable data.
    ///
    /// This is intended to be used for storing certain global
    /// instances which actors may need to get hold of, for example an
    /// access-point for the I/O poll implementation that Stakker is
    /// running under.
    ///
    /// [`Core::anymap_get`]: struct.Core.html#method.anymap_get
    /// [`Core::anymap_try_get`]: struct.Core.html#method.anymap_try_get
    #[cfg_attr(not(feature = "anymap"), allow(unused_variables))]
    #[inline]
    pub fn anymap_set<T: Clone + 'static>(&mut self, val: T) {
        // If "anymap" feature is not configured, ignore this
        // operation, but panic on the `anymap_get`
        #[cfg(feature = "anymap")]
        self.core.anymap.insert(val);
    }

    /// Used to provide **Stakker** with a means to wake the main
    /// thread.  This enables [`Waker`] and associated functionality.
    /// A poll-waker is not required otherwise.
    ///
    /// Normally the main thread will be blocked waiting for I/O
    /// events most of the time, with a timeout to handle the
    /// next-expiring timer.  If another thread wants to defer a call
    /// to the main thread, then it needs a way to interrupt that
    /// blocked call.  This is done via creating an artificial I/O
    /// event.  (For example, `mio` handles this with a `mio::Waker`
    /// instance which wraps various platform-specific ways of
    /// creating an artificial I/O event.)
    ///
    /// Normally the poll-waker will be set up automatically by the
    /// user's chosen I/O polling implementation (for example
    /// `stakker_mio`) on initialisation.
    ///
    /// [`Waker`]: struct.Waker.html
    pub fn set_poll_waker(&mut self, waker: impl Fn() + Send + Sync + 'static) {
        if !self.wake_handlers_unset {
            panic!("Stakker::set_poll_waker called more than once");
        }
        self.wake_handlers = WakeHandlers::new(Box::new(waker));
        self.wake_handlers_unset = false;
    }

    /// Indicate that the main thread has been woken up due to a call
    /// from another thread to the waker configured with `set_poll_waker`.
    /// **Stakker** uses this to do [`Waker`] handling.
    ///
    /// [`Waker`]: struct.Waker.html
    pub fn poll_wake(&mut self) {
        // Due to wake handlers needing a Stakker ref, we can't have
        // any borrow on the `wake_handlers` active.  So we
        // have to pull the handler out to call it and then put it
        // back in again afterwards.  This is only a `usize` being
        // changed in memory, though.
        for bit in self.wake_handlers.wake_list() {
            if let Some(mut cb) = self.wake_handlers.handler_borrow(bit) {
                cb(self, false);
                self.wake_handlers.handler_restore(bit, cb);
            }
        }
    }

    // Process all the outstanding wake handler drops
    #[cfg(feature = "inter-thread")]
    pub(crate) fn process_waker_drops(&mut self) {
        for bit in self.wake_handlers.drop_list() {
            if let Some(mut cb) = self.wake_handlers.del(bit) {
                cb(self, true);
            }
        }
    }
}

impl Deref for Stakker {
    type Target = Core;

    fn deref(&self) -> &Core {
        &self.core
    }
}

impl DerefMut for Stakker {
    fn deref_mut(&mut self) -> &mut Core {
        &mut self.core
    }
}

/// Core operations available from both [`Stakker`] and [`Cx`] objects
///
/// Both `&mut Stakker` and `&mut Cx` dereference to `&mut Core`, so
/// typically either of those can be used wherever `&mut Core` or
/// `&Core` is required.
///
/// [`Cx`]: struct.Cx.html
/// [`Stakker`]: struct.Stakker.html
pub struct Core {
    now: Instant,
    queue: FnOnceQueue<Stakker>,
    deferrer: Deferrer,
    idle_queue: VecDeque<Box<dyn FnOnce(&mut Stakker) + 'static>>,
    timers: Timers<Stakker>,
    shutdown: Option<ActorDied>,
    pub(crate) sharecell_owner: ShareCellOwner,
    pub(crate) actor_maker: ActorCellMaker,
    #[cfg(feature = "anymap")]
    anymap: anymap::Map,
    wake_handlers: WakeHandlers,
    wake_handlers_unset: bool,
}

impl Core {
    pub(crate) fn new(now: Instant, actor_maker: ActorCellMaker) -> Self {
        let mut deferrer = Deferrer::new();
        // Intentionally drop any queued items belonging to a previous
        // Stakker.  This is to align the behaviour between the three
        // Deferrer implementations, to avoid dependence on any one
        // behaviour.  (With the 'inline' Deferrer, each Stakker has
        // its own Deferrer queue, but the other two share a queue
        // between deferrers.)
        drop(deferrer.replace_queue(FnOnceQueue::new()));

        Self {
            now,
            queue: FnOnceQueue::new(),
            deferrer,
            idle_queue: VecDeque::new(),
            timers: Timers::new(now),
            shutdown: None,
            sharecell_owner: new_share_cell_owner(),
            actor_maker,
            #[cfg(feature = "anymap")]
            anymap: anymap::Map::new(),
            wake_handlers: WakeHandlers::new(Box::new(|| unreachable!())),
            wake_handlers_unset: true,
        }
    }

    /// Our view of the current time.  Actors should use this in
    /// preference to `Instant::now()` for speed and in order to work
    /// in virtual time.
    #[inline]
    pub fn now(&self) -> Instant {
        self.now
    }

    /// Defer an operation to be executed later.  It is put on the
    /// defer queue, and run as soon all operations preceding it have
    /// been executed.
    #[inline]
    pub fn defer(&mut self, f: impl FnOnce(&mut Stakker) + 'static) {
        self.queue.push(f);
    }

    /// Defer an operation to executed soon, but lazily.  It goes onto
    /// a lower priority queue executed once the normal defer queue
    /// has been completely cleared (including any further deferred
    /// items added whilst clearing that queue).  This can be used for
    /// flushing data generated in this batch of processing, for
    /// example.
    #[inline]
    pub fn lazy(&mut self, f: impl FnOnce(&mut Stakker) + 'static) {
        self.deferrer.defer(f);
    }

    /// Defer an operation to be executed when this process next
    /// becomes idle, i.e. when all other queues are empty and there
    /// is no I/O to process.  This can be used to implement
    /// backpressure on incoming streams, i.e. only fetch more data
    /// once there is nothing else left to do.
    #[inline]
    pub fn idle(&mut self, f: impl FnOnce(&mut Stakker) + 'static) {
        self.idle_queue.push_back(Box::new(f));
    }

    /// Delay an operation to be executed after a duration has passed.
    /// This is the same as adding it as a fixed timer.  Returns a key
    /// that can be used to delete the timer.
    #[inline]
    pub fn after(
        &mut self,
        dur: Duration,
        f: impl FnOnce(&mut Stakker) + 'static,
    ) -> FixedTimerKey {
        self.timers.add(self.now + dur, Box::new(f))
    }

    /// Add a fixed timer that expires at the given time.  Returns a
    /// key that can be used to delete the timer.
    #[inline]
    pub fn timer_add(
        &mut self,
        expiry: Instant,
        f: impl FnOnce(&mut Stakker) + 'static,
    ) -> FixedTimerKey {
        self.timers.add(expiry, Box::new(f))
    }

    /// Delete a fixed timer.  Returns `true` on success, `false` if
    /// timer no longer exists (i.e. it expired or was deleted)
    #[inline]
    pub fn timer_del(&mut self, key: FixedTimerKey) -> bool {
        self.timers.del(key)
    }

    /// Add or update a "Max" timer, which expires at the greatest
    /// (latest) expiry time provided.  See [`MaxTimerKey`] for the
    /// characteristics of this timer.  If the value in the
    /// [`MaxTimerKey`] variable provided is no longer valid
    /// (e.g. expired), creates a new timer using the given closure
    /// and writes a [`MaxTimerKey`] value back to the variable.
    /// Otherwise updates the given timer with the new time.  The
    /// variable should be initialised using `MaxTimerKey::default()`.
    /// The timer may be cancelled using [`Core::timer_max_del`].
    ///
    /// [`Core::timer_max_del`]: struct.Core.html#method.timer_max_del
    /// [`MaxTimerKey`]: struct.MaxTimerKey.html
    #[inline]
    pub fn timer_max(
        &mut self,
        key_variable: &mut MaxTimerKey,
        expiry: Instant,
        f: impl FnOnce(&mut Stakker) + 'static,
    ) {
        if !self.timers.mod_max(*key_variable, expiry) {
            *key_variable = self.timers.add_max(expiry, Box::new(f));
        }
    }

    /// Add a "Max" timer, which expires at the greatest (latest)
    /// expiry time provided.  See [`MaxTimerKey`] for the
    /// characteristics of this timer.  Returns a key that can be used
    /// to delete or modify the timer.
    ///
    /// [`MaxTimerKey`]: struct.MaxTimerKey.html
    #[inline]
    pub fn timer_max_add(
        &mut self,
        expiry: Instant,
        f: impl FnOnce(&mut Stakker) + 'static,
    ) -> MaxTimerKey {
        self.timers.add_max(expiry, Box::new(f))
    }

    /// Update a "Max" timer with a new expiry time.  It will be used
    /// as the new expiry time only if it is greater than the current
    /// expiry time.  This call is designed to be very cheap to call
    /// frequently.
    ///
    /// Returns `true` on success, `false` if timer no longer exists
    /// (i.e. it expired or was deleted)
    #[inline]
    pub fn timer_max_mod(&mut self, key: MaxTimerKey, expiry: Instant) -> bool {
        self.timers.mod_max(key, expiry)
    }

    /// Delete a "Max" timer.  Returns `true` on success, `false` if
    /// timer no longer exists (i.e. it expired or was deleted)
    #[inline]
    pub fn timer_max_del(&mut self, key: MaxTimerKey) -> bool {
        self.timers.del_max(key)
    }

    /// Check whether a "Max" timer is active.  Returns `true` if it
    /// exists and is active, `false` if it expired or was deleted or
    /// never existed
    #[inline]
    pub fn timer_max_active(&mut self, key: MaxTimerKey) -> bool {
        self.timers.max_is_active(key)
    }

    /// Add or update a "Min" timer, which expires at the smallest
    /// (earliest) expiry time provided.  See [`MinTimerKey`] for the
    /// characteristics of this timer.  If the value in the
    /// [`MinTimerKey`] variable provided is no longer valid
    /// (e.g. expired), creates a new timer using the given closure
    /// and writes a [`MinTimerKey`] value back to the variable.
    /// Otherwise updates the given timer with the new time.  The
    /// variable should be initialised using `MinTimerKey::default()`.
    /// The timer may be cancelled using [`Core::timer_min_del`].
    ///
    /// [`Core::timer_min_del`]: struct.Core.html#method.timer_min_del
    /// [`MinTimerKey`]: struct.MinTimerKey.html
    #[inline]
    pub fn timer_min(
        &mut self,
        key_variable: &mut MinTimerKey,
        expiry: Instant,
        f: impl FnOnce(&mut Stakker) + 'static,
    ) {
        if !self.timers.mod_min(*key_variable, expiry) {
            *key_variable = self.timers.add_min(expiry, Box::new(f));
        }
    }

    /// Add a "Min" timer, which expires at the smallest (earliest)
    /// expiry time provided.  See [`MinTimerKey`] for the
    /// characteristics of this timer.  Returns a key that can be used
    /// to delete or modify the timer.
    ///
    /// [`MinTimerKey`]: struct.MinTimerKey.html
    #[inline]
    pub fn timer_min_add(
        &mut self,
        expiry: Instant,
        f: impl FnOnce(&mut Stakker) + 'static,
    ) -> MinTimerKey {
        self.timers.add_min(expiry, Box::new(f))
    }

    /// Update a "Min" timer with a new expiry time.  It will be used
    /// as the new expiry time only if it is earlier than the current
    /// expiry time.  This call is designed to be very cheap to call
    /// frequently, so long as the change is within the wiggle-room
    /// allowed.  Otherwise it causes the working timer to be deleted
    /// and added again, readjusting the wiggle-room accordingly.
    ///
    /// Returns `true` on success, `false` if timer no longer exists
    /// (i.e. it expired or was deleted)
    #[inline]
    pub fn timer_min_mod(&mut self, key: MinTimerKey, expiry: Instant) -> bool {
        self.timers.mod_min(key, expiry)
    }

    /// Delete a "Min" timer.  Returns `true` on success, `false` if
    /// timer no longer exists (i.e. it expired or was deleted)
    #[inline]
    pub fn timer_min_del(&mut self, key: MinTimerKey) -> bool {
        self.timers.del_min(key)
    }

    /// Check whether a "Min" timer is active.  Returns `true` if it
    /// exists and is active, `false` if it expired or was deleted or
    /// never existed
    #[inline]
    pub fn timer_min_active(&mut self, key: MinTimerKey) -> bool {
        self.timers.min_is_active(key)
    }

    /// Gets a clone of a value from the Stakker `anymap`.  This is
    /// intended to be used to access certain global instances, for
    /// example the I/O poll implementation that this Stakker is
    /// running inside.  Panics if the value is not found.
    pub fn anymap_get<T: Clone + 'static>(&mut self) -> T {
        self.anymap_try_get()
            .unwrap_or_else(|| panic!("No anymap entry found for {}", std::any::type_name::<T>()))
    }

    /// Tries to get a clone of a value from the Stakker `anymap`.
    /// This is intended to be used to access certain global
    /// instances, for example the I/O poll implementation that this
    /// Stakker is running inside.  Returns `None` if the value is
    /// missing.
    pub fn anymap_try_get<T: Clone + 'static>(&mut self) -> Option<T> {
        #[cfg(feature = "anymap")]
        return self.anymap.get::<T>().cloned();
        #[cfg(not(feature = "anymap"))]
        panic!("Enable feature 'anymap' to use anymap_get() or anymap_try_get()");
    }

    /// Request that the event loop terminate.  For this to work, the
    /// event loop must check [`Core::not_shutdown`] each time through
    /// the loop.  See also the [`fwd_shutdown!`] macro which can be
    /// used as the notify handler for an actor, to shut down the
    /// event loop when that actor terminates.  The event loop code
    /// can obtain the `ActorDied` instance using `shutdown_reason`.
    ///
    /// [`Core::not_shutdown`]: struct.Core.html#method.not_shutdown
    /// [`fwd_shutdown!`]: macro.fwd_shutdown.html
    pub fn shutdown(&mut self, died: ActorDied) {
        self.shutdown = Some(died);
    }

    /// Should the event loop continue running?  Returns `true` if
    /// there is no active shutdown in progress.
    pub fn not_shutdown(&self) -> bool {
        self.shutdown.is_none()
    }

    /// Get the reason for shutdown, if shutdown was requested.  After
    /// calling this, the shutdown flag is cleared,
    /// i.e. `not_shutdown` will return `false` and the event loop
    /// could continue to run.
    pub fn shutdown_reason(&mut self) -> Option<ActorDied> {
        mem::replace(&mut self.shutdown, None)
    }

    /// Get a [`Deferrer`] instance which can be used to defer calls
    /// from contexts in the same thread which don't have access to
    /// `Core`, for example drop handlers.  Calls submitted via a
    /// `Deferrer` go onto the lazy queue.
    ///
    /// [`Deferrer`]: struct.Deferrer.html
    pub fn deferrer(&self) -> Deferrer {
        assert!(mem::size_of::<usize>() >= mem::size_of::<Deferrer>());
        self.deferrer.clone()
    }

    /// Register a wake handler callback, and obtain a [`Waker`]
    /// instance which can be passed to another thread.  The wake
    /// handler will always be executed in the main thread.  When
    /// [`Waker::wake`] is called in another thread, a wake-up
    /// is scheduled to occur in the main thread, using the wake-up
    /// mechanism provided by the I/O poller.  Then when that wake-up
    /// is received, the corresponding wake handler is executed.  Note
    /// that this is efficient -- if many wake handlers are scheduled
    /// around the same time, they share the same main thread wake-up.
    ///
    /// The wake handler is called in the main thread with arguments
    /// of `(stakker, deleted)`.  Note that there is a small chance of
    /// a spurious wake call happening occasionally, so the wake
    /// handler code must be ready for that.  If `deleted` is true
    /// then the [`Waker`] was dropped, and this wake handler is
    /// also just about to be dropped.
    ///
    /// This call panics if no I/O poller has yet set up a waker using
    /// [`Stakker::set_poll_waker`].
    ///
    /// [`Stakker::set_poll_waker`]: struct.Stakker.html#method.set_poll_waker
    /// [`Waker::wake`]: struct.Waker.html#method.wake
    /// [`Waker`]: struct.Waker.html
    pub fn waker(&mut self, cb: impl FnMut(&mut Stakker, bool) + 'static) -> Waker {
        if self.wake_handlers_unset {
            panic!("Core::waker() called with no waker set up");
        }
        self.wake_handlers.add(cb)
    }
}
