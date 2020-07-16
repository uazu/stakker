# Significant feature changes and additions

This project follows Rust semantic versioning, so any patch release
before 0.1.0 is allowed to make breaking changes.

<!-- see keepachangelog.com for format ideas -->

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

- Coverage testing with kcov, and many more tests.  Includes coverage
  testing of macros.

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

## 0.0.1 (2020-01-23)

First public release

<!-- Local Variables: -->
<!-- mode: markdown -->
<!-- End: -->
