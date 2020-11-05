use crate::cell::cell::{new_actor_cell_owner, new_share_cell_owner};
use crate::cell::cell::{ActorCellMaker, ActorCellOwner, ShareCellOwner};
use crate::queue::FnOnceQueue;
use crate::timers::Timers;
use crate::waker::WakeHandlers;
use crate::{
    Deferrer, FixedTimerKey, LogFilter, LogID, LogLevel, LogRecord, LogVisitor, MaxTimerKey,
    MinTimerKey, Share, StopCause, Waker,
};
use std::collections::VecDeque;
use std::fmt::Arguments;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::time::{Duration, Instant, SystemTime};

/// The external interface to the actor runtime
///
/// It contains all the queues and timers, and controls access to the
/// state of all the actors.  It also provides the interface to
/// control all this from outside, i.e. the calls used by an event
/// loop.  The [`Stakker`] instance itself is not accessible from
/// actors due to borrowing restrictions.  It derefs to `&mut Core`
/// (through auto-deref or `*stakker`).  [`Core`] is also accessible
/// to actors through their [`Cx`] context reference.
///
/// [`Core`]: struct.Core.html
/// [`Cx`]: struct.Cx.html
/// [`Stakker`]: struct.Stakker.html
pub struct Stakker {
    pub(crate) core: Core,
    pub(crate) actor_owner: ActorCellOwner,
    alt_queues: Option<(FnOnceQueue<Stakker>, FnOnceQueue<Stakker>)>,
    recreate_queues_time: Instant,
}

impl Stakker {
    /// Construct a [`Stakker`] instance.  Whether more than one
    /// instance can be created in each process or thread depends on
    /// the [Cargo features](index.html#cargo-features) enabled.
    ///
    /// [`Stakker`]: struct.Stakker.html
    pub fn new(now: Instant) -> Self {
        FnOnceQueue::<Stakker>::sanity_check();
        // Do this first to get the uniqueness checks to fail early,
        let (actor_owner, actor_maker) = new_actor_cell_owner();
        Self {
            core: Core::new(now, actor_maker),
            actor_owner,
            alt_queues: Some((FnOnceQueue::new(), FnOnceQueue::new())),
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

    /// Move time forward, expire any timers onto the main
    /// [`Deferrer`] queue, then run main and lazy queues until there
    /// is nothing outstanding.  Returns `true` if there are idle
    /// items still to run.
    ///
    /// If `idle` is true, then runs an item from the idle queue as
    /// well.  This should be set only if we've already run the queues
    /// and just polled I/O (without waiting) and still there's
    /// nothing to do.
    ///
    /// Note: All actors should use `cx.now()` to get the time, which
    /// allows the entire system to be run in virtual time (unrelated
    /// to real time) if necessary.
    ///
    /// [`Deferrer`]: struct.Deferrer.html
    pub fn run(&mut self, now: Instant, idle: bool) -> bool {
        if idle {
            if let Some(cb) = self.idle_queue.pop_front() {
                cb(self);
            }
        }

        // Swap between normal and alt queues in an attempt to keep
        // using the same memory, which should be more cache-
        // efficient.  However if there has been a big burst of
        // activity, the queues might have grown huge.  So
        // periodically force the queues to be recreated.
        //
        // It's necessary to swap out queues before executing them,
        // because whilst executing, more items may be added to any of
        // them.  (Also the borrow checker complains.)

        // Run main queue and timers
        let (mut alt_main, mut alt_lazy) =
            (self.alt_queues.take()).expect("Previous run call must have panicked");
        self.core.deferrer.swap_queue(&mut alt_main);
        if now > self.core.now {
            self.core.now = now;
            self.core.timers.advance(now, &mut alt_main);
        }
        alt_main.execute(self);

        // Keep running main and lazy queues until exhaustion
        loop {
            self.core.deferrer.swap_queue(&mut alt_main);
            if !alt_main.is_empty() {
                alt_main.execute(self);
                continue;
            }
            mem::swap(&mut self.core.lazy_queue, &mut alt_lazy);
            if !alt_lazy.is_empty() {
                alt_lazy.execute(self);
                continue;
            }
            break;
        }
        self.alt_queues = Some((alt_main, alt_lazy));

        // Recreate the queues?  They will all be empty at this point.
        if now > self.recreate_queues_time {
            self.alt_queues = Some((FnOnceQueue::new(), FnOnceQueue::new()));
            self.core.lazy_queue = FnOnceQueue::new();
            self.core.deferrer.set_queue(FnOnceQueue::new());
            self.recreate_queues_time = now + Duration::from_secs(60);
        }

        !self.core.idle_queue.is_empty()
    }

    /// Set the current `SystemTime`, for use in a virtual time main
    /// loop.  If `None` is passed, then `core.systime()` just calls
    /// `SystemTime::now()`.  Otherwise `core.systime()` returns the
    /// provided `SystemTime` instead.
    #[inline]
    pub fn set_systime(&mut self, systime: Option<SystemTime>) {
        self.core.systime = systime;
    }

    /// Set the logger and logging level
    ///
    /// The provided logger will be called synchronously every time a
    /// [`Core::log`] call is made if the logging level is enabled.
    /// It is provided with a `Core` reference, so can access a
    /// `Share`, or defer calls to actors as necessary.  It may
    /// alternatively choose to forward the logging to an external log
    /// framework, such as the **log** or **tracing** crates.
    ///
    /// The enabled logging levels are described by `filter`.
    /// Typically you'd set something like
    /// `LogFilter::all(&[LogLevel::Info, LogLevel::Audit,
    /// LogLevel::Open])` or `LogFilter::from_str("info,audit,open")`,
    /// which enable info and above, plus audit and span open/close.
    /// See [`LogFilter`].
    ///
    /// Note that the **logger** feature must be enabled for this call
    /// to succeed.  The **stakker** crate provides only the core
    /// logging functionality.  It adds a 64-bit logging ID to each
    /// actor and logs actor startup and termination.  It provides the
    /// framework for logging formatted-text and key-value pairs along
    /// with an actor's logging-ID for context.  An external crate
    /// like **stakker_log** may be used to provide macros that allow
    /// convenient logging from actor code and to allow interfacing to
    /// external logging systems.
    ///
    /// [`Core::log`]: struct.Core.html#method.log
    /// [`LogFilter`]: struct.LogFilter.html
    #[inline]
    #[allow(unused_variables)]
    pub fn set_logger(
        &mut self,
        filter: LogFilter,
        logger: impl FnMut(&mut Core, &LogRecord<'_>) + 'static,
    ) {
        #[cfg(feature = "logger")]
        {
            self.log_filter = filter;
            self.logger = Some(Box::new(logger));
        }
        #[cfg(not(feature = "logger"))]
        panic!("Enable 'logger' feature before setting a logger");
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

    // For testing
    #[cfg(all(test, feature = "inter-thread"))]
    pub(crate) fn poll_waker_handler_count(&self) -> usize {
        self.wake_handlers.handler_count()
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

impl Drop for Stakker {
    fn drop(&mut self) {
        // The defer queue may contain items that have circular
        // references back to the defer queue via a Deferrer, which
        // means they never get freed.  In addition when an actor
        // referenced from the queue is dropped, it may queue a
        // handler, which in turn holds a circular reference.  So
        // special handling is required to avoid leaking memory on
        // Stakker termination.  Limit this to 99 attempts just to
        // avoid the possibility of an infinite loop.
        for _ in 0..99 {
            let mut alt_main = FnOnceQueue::new();
            self.core.deferrer.swap_queue(&mut alt_main);
            if alt_main.is_empty() {
                break;
            }
            drop(alt_main);
        }
    }
}

/// Core operations available from both [`Stakker`] and [`Cx`] objects
///
/// Both [`Stakker`] and [`Cx`] references auto-dereference to a
/// [`Core`] reference, so typically either of those can be used
/// wherever a [`Core`] reference is required.
///
/// [`Core`]: struct.Core.html
/// [`Cx`]: struct.Cx.html
/// [`Stakker`]: struct.Stakker.html
pub struct Core {
    now: Instant,
    deferrer: Deferrer,
    lazy_queue: FnOnceQueue<Stakker>,
    idle_queue: VecDeque<Box<dyn FnOnce(&mut Stakker) + 'static>>,
    timers: Timers<Stakker>,
    shutdown: Option<StopCause>,
    pub(crate) sharecell_owner: ShareCellOwner,
    pub(crate) actor_maker: ActorCellMaker,
    #[cfg(feature = "anymap")]
    anymap: anymap::Map,
    systime: Option<SystemTime>,
    wake_handlers: WakeHandlers,
    wake_handlers_unset: bool,
    #[cfg(feature = "logger")]
    log_id_seq: u64,
    #[cfg(feature = "logger")]
    log_filter: LogFilter,
    #[cfg(feature = "logger")]
    logger: Option<Box<dyn FnMut(&mut Core, &LogRecord<'_>)>>,
}

impl Core {
    pub(crate) fn new(now: Instant, actor_maker: ActorCellMaker) -> Self {
        let deferrer = Deferrer::new();
        // Intentionally drop any queued items belonging to a previous
        // Stakker.  This is to align the behaviour between the three
        // Deferrer implementations, to avoid dependence on any one
        // behaviour.  (With the 'inline' Deferrer, each Stakker has
        // its own Deferrer queue, but the other two use a global or
        // thread-local which may have data from a previous Stakker.)
        deferrer.set_queue(FnOnceQueue::new());

        Self {
            now,
            deferrer,
            lazy_queue: FnOnceQueue::new(),
            idle_queue: VecDeque::new(),
            timers: Timers::new(now),
            shutdown: None,
            sharecell_owner: new_share_cell_owner(),
            actor_maker,
            #[cfg(feature = "anymap")]
            anymap: anymap::Map::new(),
            systime: None,
            wake_handlers: WakeHandlers::new(Box::new(|| unreachable!())),
            wake_handlers_unset: true,
            #[cfg(feature = "logger")]
            log_id_seq: 0,
            #[cfg(feature = "logger")]
            log_filter: LogFilter::new(),
            #[cfg(feature = "logger")]
            logger: None,
        }
    }

    /// Our view of the current time.  Actors should use this in
    /// preference to `Instant::now()` for speed and in order to work
    /// in virtual time.
    #[inline]
    pub fn now(&self) -> Instant {
        self.now
    }

    /// Get the current `SystemTime`.  Normally this returns the same
    /// as `SystemTime::now()`, but if running in virtual time, it
    /// would return the virtual `SystemTime` instead (as provided to
    /// [`Stakker::set_systime`] by the virtual time main loop).  Note
    /// that this time is not suitable for timing things, as it may go
    /// backwards if the user or a system process adjusts the clock.
    /// It is just useful for showing or recording "human time" for
    /// the user, and for recording times that are meaningful on a
    /// longer scale, e.g. from one run of a process to the next.
    ///
    /// [`Stakker::set_systime`]: struct.Stakker.html#method.set_systime
    #[inline]
    pub fn systime(&self) -> SystemTime {
        if let Some(time) = self.systime {
            time
        } else {
            SystemTime::now()
        }
    }

    /// Defer an operation to be executed later.  It is put on the
    /// main queue, and run as soon all operations preceding it have
    /// been executed.
    #[inline]
    pub fn defer(&mut self, f: impl FnOnce(&mut Stakker) + 'static) {
        self.deferrer.defer(f);
    }

    /// Defer an operation to executed soon, but lazily.  It goes onto
    /// a lower priority queue executed once the normal defer queue
    /// has been completely cleared (including any further deferred
    /// items added whilst clearing that queue).  This can be used for
    /// flushing data generated in this batch of processing, for
    /// example.
    #[inline]
    pub fn lazy(&mut self, f: impl FnOnce(&mut Stakker) + 'static) {
        self.lazy_queue.push(f);
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

    /// Add a "Max" timer, which expires at the greatest (latest)
    /// expiry time provided.  See [`MaxTimerKey`] for the
    /// characteristics of this timer.  Returns a key that can be used
    /// to delete or modify the timer.
    ///
    /// See also the [`timer_max!`] macro, which may be more
    /// convenient as it combines [`Core::timer_max_add`] and
    /// [`Core::timer_max_upd`].
    ///
    /// [`Core::timer_max_add`]: struct.Core.html#method.timer_max_add
    /// [`Core::timer_max_upd`]: struct.Core.html#method.timer_max_upd
    /// [`MaxTimerKey`]: struct.MaxTimerKey.html
    /// [`timer_max!`]: macro.timer_max.html
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
    ///
    /// See also the [`timer_max!`] macro, which may be more
    /// convenient as it combines [`Core::timer_max_add`] and
    /// [`Core::timer_max_upd`].
    ///
    /// [`Core::timer_max_add`]: struct.Core.html#method.timer_max_add
    /// [`Core::timer_max_upd`]: struct.Core.html#method.timer_max_upd
    /// [`timer_max!`]: macro.timer_max.html
    #[inline]
    pub fn timer_max_upd(&mut self, key: MaxTimerKey, expiry: Instant) -> bool {
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

    /// Add a "Min" timer, which expires at the smallest (earliest)
    /// expiry time provided.  See [`MinTimerKey`] for the
    /// characteristics of this timer.  Returns a key that can be used
    /// to delete or modify the timer.
    ///
    /// See also the [`timer_min!`] macro, which may be more
    /// convenient as it combines [`Core::timer_min_add`] and
    /// [`Core::timer_min_upd`].
    ///
    /// [`Core::timer_min_add`]: struct.Core.html#method.timer_min_add
    /// [`Core::timer_min_upd`]: struct.Core.html#method.timer_min_upd
    /// [`MinTimerKey`]: struct.MinTimerKey.html
    /// [`timer_min!`]: macro.timer_min.html
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
    ///
    /// See also the [`timer_min!`] macro, which may be more
    /// convenient as it combines [`Core::timer_min_add`] and
    /// [`Core::timer_min_upd`].
    ///
    /// [`Core::timer_min_add`]: struct.Core.html#method.timer_min_add
    /// [`Core::timer_min_upd`]: struct.Core.html#method.timer_min_upd
    /// [`timer_min!`]: macro.timer_min.html
    #[inline]
    pub fn timer_min_upd(&mut self, key: MinTimerKey, expiry: Instant) -> bool {
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
        self.anymap.insert(val);
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
    /// the loop.  See also the [`ret_shutdown!`] macro which can be
    /// used as the [`StopCause`] handler for an actor, to shut down
    /// the event loop when that actor terminates.  The event loop
    /// code can obtain the [`StopCause`] using
    /// [`Core::shutdown_reason`].
    ///
    /// [`Core::not_shutdown`]: struct.Core.html#method.not_shutdown
    /// [`Core::shutdown_reason`]: struct.Core.html#method.shutdown_reason
    /// [`StopCause`]: enum.StopCause.html
    /// [`ret_shutdown!`]: macro.ret_shutdown.html
    pub fn shutdown(&mut self, cause: StopCause) {
        self.shutdown = Some(cause);
    }

    /// Should the event loop continue running?  Returns `true` if
    /// there is no active shutdown in progress.
    pub fn not_shutdown(&self) -> bool {
        self.shutdown.is_none()
    }

    /// Get the reason for shutdown, if shutdown was requested.  After
    /// calling this, the shutdown flag is cleared,
    /// i.e. [`Core::not_shutdown`] will return `false` and the event
    /// loop could continue to run.
    ///
    /// [`Core::not_shutdown`]: struct.Core.html#method.not_shutdown
    pub fn shutdown_reason(&mut self) -> Option<StopCause> {
        mem::replace(&mut self.shutdown, None)
    }

    /// Get a new [`Deferrer`] instance which can be used to defer
    /// calls to the main queue from contexts in the same thread which
    /// don't have access to [`Core`], for example drop handlers.
    ///
    /// [`Core`]: struct.Core.html
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

    /// Borrow two [`Share`] instances mutably at the same time.  This
    /// will panic if they are the same instance.
    ///
    /// [`Share`]: struct.Share.html
    #[inline]
    pub fn share_rw2<'a, T, U>(
        &'a mut self,
        s1: &'a Share<T>,
        s2: &'a Share<U>,
    ) -> (&'a mut T, &'a mut U) {
        self.sharecell_owner.rw2(&s1.rc, &s2.rc)
    }

    /// Borrow three [`Share`] instances mutably at the same time.
    /// This will panic if any two are the same instance.
    ///
    /// [`Share`]: struct.Share.html
    #[inline]
    pub fn share_rw3<'a, T, U, V>(
        &'a mut self,
        s1: &'a Share<T>,
        s2: &'a Share<U>,
        s3: &'a Share<V>,
    ) -> (&'a mut T, &'a mut U, &'a mut V) {
        self.sharecell_owner.rw3(&s1.rc, &s2.rc, &s3.rc)
    }

    /// Log a log-record to the current logger, if one is active and
    /// if the log-level is enabled.  Otherwise it is ignored.  `id`
    /// should be the logging-ID (obtained from `actor.id()` or
    /// `cx.id()` or `core.log_span_open()` for non-actor spans) or 0
    /// if the log-record doesn't belong to any span.
    ///
    /// Normally you would use a macro provided by an external crate
    /// which wraps this call.
    #[inline]
    #[allow(unused_variables)]
    pub fn log(
        &mut self,
        id: LogID,
        level: LogLevel,
        target: &str,
        fmt: Arguments<'_>,
        kvscan: impl Fn(&mut dyn LogVisitor),
    ) {
        // It might seem like it would be better to build the
        // LogRecord first and then pass its address to this call, but
        // actually that doesn't work because `format_args!` creates
        // some temporaries that only live to the end of the enclosing
        // statement.  So `format_args!` can only be used within the
        // argument list of the function that consumes those
        // arguments.  However making this #[inline] means that the
        // LogRecord will be built in place on the stack and then its
        // address will be passed to the logger.
        #[cfg(feature = "logger")]
        if self.log_check(level) {
            if let Some(mut logger) = mem::replace(&mut self.logger, None) {
                logger(
                    self,
                    &LogRecord {
                        id,
                        level,
                        target,
                        fmt,
                        kvscan: &kvscan,
                    },
                );
                self.logger = Some(logger);
            }
        }
    }

    /// Check whether a log-record with the given [`LogLevel`] should
    /// be logged
    ///
    /// [`LogLevel`]: enum.LogLevel.html
    #[inline]
    #[allow(unused_variables)]
    pub fn log_check(&self, level: LogLevel) -> bool {
        #[cfg(feature = "logger")]
        {
            self.log_filter.allows(level)
        }
        #[cfg(not(feature = "logger"))]
        false
    }

    /// Allocate a new logging-ID and write an `Open` record to the
    /// logger.  `tag` will be included as the record's text, and
    /// should indicate what kind of span it is, e.g. the type name
    /// for an actor.  This should be the tag for the record, and
    /// would not normally contain any dynamic information.  If
    /// `parent_id` is non-zero, then a `parent` key will be added
    /// with that value.  `kvscan` will be called to add any other
    /// key-value pairs as required, which is where the dynamic
    /// information should go.
    ///
    /// This is used by actors on startup to allocate a logging-ID for
    /// the span of the actor's lifetime.  However other code that
    /// needs to do logging within a certain identifiable span can
    /// also make use of this call.  Where possible, relate this new
    /// span to another span using `parent_id`.  Use
    /// [`Core::log_span_close`] when the span is complete.
    ///
    /// In the unlikely event that a program allocates 2^64 logging
    /// IDs, the IDs will wrap around to 1 again.  If this is likely
    /// to cause a problem downstream, the logger implementation
    /// should detect this and warn or terminate as appropriate.
    ///
    /// [`Core::log_span_close`]: struct.Core.html#method.log_span_close
    #[inline]
    #[allow(unused_variables)]
    pub fn log_span_open(
        &mut self,
        tag: &str,
        parent_id: LogID,
        kvscan: impl Fn(&mut dyn LogVisitor),
    ) -> LogID {
        #[cfg(feature = "logger")]
        {
            self.log_id_seq = self.log_id_seq.wrapping_add(1).max(1);
            let id = self.log_id_seq;
            self.log(
                id,
                LogLevel::Open,
                "",
                format_args!("{}", tag),
                move |output| {
                    if parent_id != 0 {
                        output.kv_u64(Some("parent"), parent_id);
                    }
                    kvscan(output);
                },
            );
            id
        }
        #[cfg(not(feature = "logger"))]
        0
    }

    /// Write a `Close` record to the logger
    ///
    /// `fmt` is a message which may give more information, e.g. the
    /// error message in the case of a failure.  `kvscan` will be
    /// called to add key-value pairs to the record.
    #[inline]
    #[allow(unused_variables)]
    pub fn log_span_close(
        &mut self,
        id: LogID,
        fmt: Arguments<'_>,
        kvscan: impl Fn(&mut dyn LogVisitor),
    ) {
        #[cfg(feature = "logger")]
        self.log(id, LogLevel::Close, "", fmt, kvscan);
    }

    /// Used in macros to get a [`Core`] reference
    ///
    /// [`Core`]: struct.Core.html
    #[inline]
    pub fn access_core(&mut self) -> &mut Core {
        self
    }

    /// Used in macros to get a [`Deferrer`] reference
    ///
    /// [`Deferrer`]: struct.Deferrer.html
    #[inline]
    pub fn access_deferrer(&self) -> &Deferrer {
        &self.deferrer
    }

    /// Used in macros to get the LogID in case this is an actor.
    /// Since it isn't, this call returns 0.
    #[inline]
    pub fn access_log_id(&self) -> LogID {
        0
    }
}
