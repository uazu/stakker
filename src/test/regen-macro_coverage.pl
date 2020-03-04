#!/usr/bin/perl -w

die unless open OUT, ">macro_coverage.rs";
print OUT <<"EOF";
//! These functions are used in macro expansions when #[cfg(test)] is
//! enabled to test coverage of macro invocations in the unit tests.

EOF

die unless open IN, "<../macros.rs";
while (<IN>) {
    next unless /COVERAGE\!\((\S+)\)/;
    print OUT "pub fn $1() {}\n";
}

die unless close OUT;
