#!/usr/bin/perl -w

use bytes;

# This packs the corpus into a single binary file.  Each item is
# preceded by its length as a 16-bit big-endian number.

my $target = shift;
die "Usage: ./pack-corpus.pl <queue|timers>\n" unless $target =~ /^(queue|timers)$/;

die unless open OUT, ">${target}_corpus.bin";

$/ = undef;
for my $fnam (<fuzz/corpus/$target/*>) {
    die unless open IN, "<$fnam";
    my $data = <IN>;
    die unless close IN;
    print OUT chr(length($data)>>8), chr(length($data)&255), $data;
}

die unless close OUT;
