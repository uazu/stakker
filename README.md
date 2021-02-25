# A lightweight low-level single-threaded actor runtime

[![license:MIT/Apache-2.0][1]](https://github.com/uazu/stakker)
[![github:uazu/stakker][2]](https://github.com/uazu/stakker)
[![crates.io:stakker][3]](https://crates.io/crates/stakker)
[![docs.rs:stakker][4]](https://docs.rs/stakker)
[![uazu.github.io:stakker][5]](https://uazu.github.io/stakker/)

[1]: https://img.shields.io/badge/license-MIT%2FApache--2.0-blue
[2]: https://img.shields.io/badge/github-uazu%2Fstakker-brightgreen
[3]: https://img.shields.io/badge/crates.io-stakker-red
[4]: https://img.shields.io/badge/docs.rs-stakker-purple
[5]: https://img.shields.io/badge/uazu.github.io-stakker-yellow

**Stakker** is designed to be layered on top of whatever event loop
the user prefers to use.  It aims to take maximum advantage of Rust's
compile-time checks and optimisations.

### Documentation

See the [crate documentation](http://docs.rs/stakker) and the [Stakker
Guide and Design Notes](https://uazu.github.io/stakker/)

# License

This project is licensed under either the Apache License version 2 or
the MIT license, at your option.  (See
[LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT)).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in this crate by you, as defined in the
Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.

### Maintenance approach

You're very welcome to try to break this code!  I intend to conform to
Rust safety conventions, including on internal interfaces.  Any
unsound behaviour that can be shown to exist will be treated as a
serious bug, and I will endeavour to find a solution as soon as
reasonably possible.

I reserve the right to (metaphorically) go off to a mountain-top cave
to consider issues in depth, to make the right decision without being
rushed.

Most of the design decisions in this software have had a lot of
consideration, with many different approaches tried and discarded
before arriving at the current solution.  The current implementations
have been rewritten and refactored and minimised to get to the current
state.  So I'd ask that any requests for changes to how things are
done be accompanied by some reasonably in-depth justification, such as
example use-cases that require the change, or some other discussion of
why that change would be a good one.  I prefer to keep the code tight,
so I might need to refactor PRs, or reimplement them a different way.
