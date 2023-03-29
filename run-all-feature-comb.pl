#!/usr/bin/perl -w

# Unrelated features may go on the same line (and end up being
# switched on/off together).  Related features must go on different
# lines (and are switched independently).  This way we can minimise
# the number of tests required.
#
# These interrelated features must not change together:
# - multi-thread multi-stakker inline-deferrer no-unsafe no-unsafe-queue
my $FEATURE_SETS = <<"EOF";
inline-deferrer inter-thread
no-unsafe anymap
no-unsafe-queue
multi-stakker logger
multi-thread
EOF
my @FEATURES = split(/\s*\n\s*/, $FEATURE_SETS);

my $MAX = 0;
if ($ARGV[0] eq '--max') {
    # Maximize combinations tested
    @FEATURES = split(/\s+/, $FEATURE_SETS);
    $MAX = 1;
    shift @ARGV;
}

sub bitcount {
    return unpack('%32b*', pack('N', $_[0]));
}
sub bitcount_then_value {
    my $cmp = bitcount($a) <=> bitcount($b);
    return $cmp unless $cmp == 0;
    return $a <=> $b;
}

my @COMMAND = @ARGV;
die "Usage: run-all-feature-comb.pl [--max] <command...>" unless @COMMAND;

# Create an order which tests no-features, then features one by one,
# then combinations of two, then increasing numbers of features.  This
# is so that we can hopefully fail earlier.
#
# It is necessary to test all combinations because if there is an
# error in one of the cfg boolean expressions it could cause a
# situation that's impossible to compile.
my $N_COMB = 2**@FEATURES;
my @order = sort bitcount_then_value (0..$N_COMB-1);
my @comb = ();
for my $bitmap (@order) {
    my @tmp = ();
    for my $i (0..@FEATURES-1) {
        push @tmp, $FEATURES[$i] if 0 != (1 & ($bitmap >> (@FEATURES-1-$i)));
    }
    my $tmp = ' ' . join(' ', @tmp) . ' ';
    # We don't need to test some combinations, since one overrides the other
    next if !$MAX && $tmp =~ / multi-stakker / && $tmp =~ / multi-thread /;
    next if !$MAX && $tmp =~ / no-unsafe / && $tmp =~ / no-unsafe-queue /;
    push @comb, $tmp;
}

for my $c (@comb) {
    my $st = '';
    if ($c =~ /multi-stakker|multi-thread/) {
        delete $ENV{RUST_TEST_THREADS};
    } else {
        # If we're going to run with the 'global' deferrer option or
        # the 'tcell' cell option, test will fail if it is run on
        # multiple threads
        $ENV{RUST_TEST_THREADS} = "1";
        $st = "(single-threaded)";
    }
    print("=== " . join(' ', @COMMAND) . "$c$st\n");
    die "FAILED\n" unless 0 == system(@COMMAND, $c);
}
