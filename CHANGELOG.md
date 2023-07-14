# Significant feature changes and additions

This project follows Rust semantic versioning.

<!-- see keepachangelog.com for format ideas -->

## 0.2.8 (2023-07-14)

### Added

- Test MSRV of 1.60
- Use Rust 2021 edition

### Fixed

- Replaced unmaintained `anymap` crate with simple safe inline
  implementation


## 0.2.7 (2023-05-09)

### Added

- `LogLevel::try_from(u8)` plus derives: Hash, Ord, PartialOrd


## 0.2.6 (2023-03-29)

### Fixed

- Fixed build failure when "no-unsafe-queue" feature was enabled alone
- Testing has been improved to catch any other similar issues


## 0.2.5 (2022-06-11)

(maintenance)


## 0.2.4 (2021-05-14)

### Added

- `ActorOwnSlab` / `actor_in_slab!` for basic child actor housekeeping
- `query!` macro for convenient access to `Actor::query`
- `Core::start_instant()` to get runtime startup time
- `ShareWeak` for weak references to `Share` items

### Documentation

- `examples/` folder


## 0.2.3 (2021-04-22)

### Fixed

- On actor `stop!` or `fail!`, if further failures are reported before
  the actor method returns, the first stop/fail is now kept and later
  ones are discarded, since the first failure is closer to the origin
  of the problem.


## 0.2.2 (2021-04-06)

### Added

- Minimum support for asynchronous tasks, in `task` module


## 0.2.1 (2021-02-25)

(only documentation changes)


## 0.2.0 (2020-11-08)

### Breaking changes

- `PipedThread::new` argument order changed, to avoid need for
  temporaries when called
- `Actor::kill`, `Actor::kill_str` and `Actor::owned` moved to
  `ActorOwn`.  This constrains the power to kill an actor and create
  new owners to just owners.  Also `kill_str` now only handles
  `&'static str`, with `kill_string` added for `String`.  If you need
  to kill an actor using just an `Actor` reference, then the actor
  needs a method added that calls `fail!`
- `Cx::fail_str` now only handles `&'static str`.  `Cx::fail_string`
  can be used for `String`.  `fail!` macro now abstracts this.
- `ActorOwn::new` now requires a parent-ID argument.  `actor!` and
  `actor_new!` macros are unchanged.
- `StopCause` adds `StopCause::Lost` to allow for future
  implementation of remote actors

### Added

- "logger" feature for optional logging support.  If enabled, every
  actor gets an span-ID.  Actor start and stop are logged, with
  failure reasons.  All logging from an actor uses the span-ID and
  supports formatted text and key-value pairs.  Logging is
  synchronization-free and is directed to a per-Stakker logging
  handler, which can output directly, or forward to a logging crate,
  or forward to the logging system of the host event loop as required
- `fail!`, `stop!` and `kill!` to express actor termination more
  conveniently, including with formatted strings
- `ret_fail!` and `ret_failthru!` to easily cascade actor failure

### Changed

- `Stakker::anymap_set` moved to `Core::anymap_set` so that actors can
  set anymap values more conveniently.

## 0.1.4 (2020-09-27)

### Added

- `Core::share_rw2` and `Core::share_rw3` to allow borrowing 2 or 3
  `Share` instances at once
- Allow using `_` in initial closure arguments within macros.  For
  example `|_this, _cx|` can now be written `|_, _|`

### Fixed

- `PipedThread` now notifies thread as documented if the actor holding
  the `PipedThread` dies.
- `PipedThread` now reports back panic string correctly if thread
  panics and the panic was made with a `String` or `&str`.
- A timer set at `cx.now()` in virtual time now executes correctly.
- The flat `FnOnce` queue has been fixed to run cleanly under MIRI

### Testing

- Stress tests of `Waker` across threads (to detect races)
- Stress test of flat `FnOnce` queue expansion
- Fuzz-testing of timer queue
- Fuzz-testing of flat `FnOnce` queue
- Unit tests of queue and timers using small corpora derived from fuzz tests
- Unit tests for `Actor`, `StopCause`, `Share`, `Waker` and `PipedThread`
- Coverage now at 91%
- Tests pass when running under MIRI

## 0.1.3 (2020-08-27)

### Added

- Virtual SystemTime: `cx.systime()` and `stakker.set_systime()`
- Synchronous actor queries from outside runtime: `actor.query()`

### Changed

- 'flat' FnOnce queue simplifications and improvements
- Global-based Deferrer access is now branch-free
- General optimisations on hot paths

### Fixed

- Fixed memory leak in 'flat' FnOnce queue cleanup
- Fixed memory leak in Stakker cleanup with inline Deferrer
- Fixed issue with TLS-based Deferrer on cleanup

### Testing

- Valgrind testing script

## 0.1.2 (2020-07-16)

### Added

- `actor_of_trait!`
- `ActorOwnAnon`

## 0.1.1 (2020-06-20)

### Changed

- Use `$crate::` instead of `stakker::` for macros
- Accept trailing comma in more places in macro args
- `'static` on `Actor::apply_prep` is not necessary

## 0.1.0 (2020-04-04)

### Changed

- For `call!`-family and `fwd_to!`-family macros using closures, no
  longer require `move` keyword and make all closures implicitly
  `move` so that they are `'static`

## 0.0.2 (2020-03-04)

### Added

- `Ret` type for compile-time checking of single use, instead of old
  `fwd_once_to!` implementation (now dropped).

- `cx: CX![]` to save on actor method boiler-plate

### Changed

- Big change to notation within macros, using `[]` to contain the
  context (`Cx` / `Actor` / `Core`) that the call is targetted
  towards, which eliminates need for `__` arguments.  This is still
  valid Rust syntax, so is formatted automatically by rustfmt.  `Fwd`
  handling split out of `call!` into `fwd!`.  Error reporting of
  errors in macro args improved.

- Multiple `ActorOwn` refs to an actor are now permitted, and the
  ref'd actor is terminated only when the last one goes away

- `Deferrer` is now the main queue, not the lazy queue.  This is to
  avoid the lazy queue ballooning whilst there's a lot still going on
  on the main queue.

- `Core::timer_max` dropped in favour of `timer_max!` macro which is
  more efficient.  Similarly for `timer_min`.

### Testing

- Coverage testing with kcov, and many more tests.  Includes coverage
  testing of macros.

## 0.0.1 (2020-01-23)

First public release

<!-- Local Variables: -->
<!-- mode: markdown -->
<!-- End: -->
