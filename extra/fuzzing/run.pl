#!/usr/bin/perl -w

my $target = shift;
die "Usage: ./run.pl <queue|timers>\n" unless $target =~ /^(queue|timers)$/;

die unless open LOG, ">tmp-log-$target";
LOG->autoflush(1);

die unless open IN, "cargo +nightly fuzz run $target -- -len_control=10000 -use_value_profile=1 2>&1 |";
while (<IN>) {
    chomp;
    s/MS:.*//;
    print "$_\n";
    if (/pulse/) {
        print LOG "$_\n";
    }
}
close IN;
