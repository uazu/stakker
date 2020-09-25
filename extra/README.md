# Here are some extra tests

These long-running or large generated tests don't make sense to run as
part of unit tests, so are kept here instead.  Those that need to run
within the crate patch themselves in at src/tests/extra.  To unpatch
that run ./restore-extra-rs.sh.
