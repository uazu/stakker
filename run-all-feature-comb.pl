#!/usr/bin/perl -w

# Unrelated features may go on the same line (and end up being
# switched on/off together).  Related features must go on different
# lines (and are switched independently).  This way we can minimise
# the number of tests required.
my $FEATURE_SETS = <<"EOF";
no-unsafe anymap inter-thread
no-unsafe-queue inline-deferrer 
multi-stakker 
multi-thread
EOF

my @FEATURES = split(/\s*\n\s*/, $FEATURE_SETS);

sub bitcount {
    return unpack('%32b*', pack('N', $_[0]));
}
sub bitcount_then_value {
    my $cmp = bitcount($a) <=> bitcount($b);
    return $cmp unless $cmp == 0;
    return $a <=> $b;
}

my @COMMAND = @ARGV;
die "Usage: run-all-feature-comb.pl <command...>" unless @COMMAND;

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
    push @comb, join(' ', @tmp);
}

for my $c (@comb) {
    print("=== " . join(' ', @COMMAND) . " $c\n");
    die "FAILED\n" unless 0 == system(@COMMAND, $c);
}
