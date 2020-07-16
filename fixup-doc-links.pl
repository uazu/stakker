#!/usr/bin/perl -w

# Version: 2020-07-16
#
# This code assumes that it owns all the `[...]: ...` definitions at
# the end of each doc comment block.  It deletes them and rewrites
# them according to the links found in the comment block.  (Any other
# links must be made inline to avoid being deleted.)
#
# Links in certain fixed formats are recognised and converted into
# links within this same crate or to other crates in the generated
# documentation.  A warning is given if the corresponding
# documentation files are not found, so this requires that
# documentation has already been generated.
#
# Recognises structs, enums, methods and macros, optionally with a
# path.  By checking generated documentation files, guesses when a
# path refers to an enum rather than a struct, or another crate rather
# than this one.  For example:
#
#   [`Actor`]
#   [`Actor::apply`]
#   [`actor!`]
#   [`mio::Poll`]
#   [`mio::Poll::poll`]
#   [`mio::net::TcpStream`]
#   [`stakker::actor!`]

use Cwd qw(cwd);

my $CRATE = cwd();
$CRATE =~ s|^.*/([^/]+)/?|$1|;
$CRATE =~ s/-/_/g;

my $TARGET = $ENV{CARGO_TARGET_DIR};
$TARGET = "./target" unless defined $TARGET;

my $DOCDIR = "$TARGET/doc/$CRATE";

sub readfile {
    my $fn = shift;
    die "Can't open file: $fn" unless open READFILE, "<$fn";
    local $/ = undef;
    my $data = <READFILE>;
    die unless close READFILE;
    return $data;
}

my $inplace = 1;

my @rs = ();
push @rs, glob('src/*.rs');
push @rs, glob('src/*/*.rs');
push @rs, glob('src/*/*/*.rs');

my %unknown = ();
my @curr = ();
my $prefix = '';

sub process_section {
    if ($prefix eq '') {
        print OUT @curr;
        return;
    }
    
    # Drop existing link-defs, and any trailing blank lines
    while (@curr > 0 && $curr[@curr-1] =~ m|^\s*//[/!]\s*\[.*\]:|) {
        pop @curr;
    }
    while (@curr > 0 && $curr[@curr-1] =~ m|^\s*//[/!]\s*$|) {
        pop @curr;
    }
    
    # Make a list of [`...`] links
    my %links = ();
    my $data = join('', @curr);
    $data =~ s/```.*?```//sg;
    while ($data =~ /(\[`[^`]+`\])[^(]/g) {
        $links{$1} = 1;
    }
    my @linkdefs = ();
    
    # Generate defs for them
    for my $link (sort keys %links) {
        my $fn;
        my $id;
        my $href;

        my $path = '';
        my $tmp = $link;
        while ($tmp =~ s/^\[`([a-z][a-z0-9_]*)::/[`/) {
            $path = "$path$1/";
        }

        my @href = ();
        if ($tmp =~ /^\[`([A-Z][a-zA-Z0-9_]+)`\]$/) {
            push @href, "${path}struct.$1.html";
            push @href, "../${path}struct.$1.html" if $path ne '';
            push @href, "${path}enum.$1.html";
            push @href, "../${path}enum.$1.html" if $path ne '';
        } elsif ($tmp =~ /^\[`([A-Z][a-zA-Z0-9_]+)::([a-z][a-z0-9_]+)`\]$/) {
            push @href, "${path}struct.$1.html#method.$2";
            push @href, "../${path}struct.$1.html#method.$2" if $path ne '';
        } elsif ($tmp =~ /^\[`([A-Z][a-zA-Z0-9_]+)::([A-Z][a-zA-Z0-9_]+)`\]$/) {
            push @href, "${path}enum.$1.html#variant.$2";
            push @href, "../${path}enum.$1.html#variant.$2" if $path ne '';
        } elsif ($tmp =~ /^\[`([a-z][a-z0-9_]+)!`\]$/) {
            push @href, "${path}macro.$1.html";
            push @href, "../${path}macro.$1.html" if $path ne '';
        } else {
            $unknown{$link} = 1;
        }

        my $found = 0;
        for my $href (@href) {
            my $fn = $href;
            my $id;
            $id = $1 if $fn =~ s/#([^#]*)$//;
            my $fnpath = "$DOCDIR/$fn";
            if (-f $fnpath) {
                push @linkdefs, "$link: $href";
                if (defined $id) {
                    my $html = readfile($fnpath);
                    print "WARNING: Missing anchor '$id' in file: $fnpath\n" 
                        unless $html =~ /['"]$id['"]/;
                }
                $found = 1;
                last;
            }
        }
        if (!$found) {
            print("WARNING: Doc file not found in any of these locations:\n  " . join("\n  ", @href) . "\n");
        } 
    }
    
    # Append
    if (@linkdefs) {
        my $linepre = $curr[0];
        $linepre =~ s|^(\s*//[/!]).*$|$1|s;
        push @curr, "$linepre\n";
        push @curr, "$linepre $_\n" for (@linkdefs);
    }
    
    print OUT @curr;
}

for my $fnam (@rs) {
    my $ofnam = "$fnam-NEW";
    @curr = ();
    $prefix = '';
    
    die "Can't open file: $fnam" unless open IN, "<$fnam";
    die "Can't create file: $ofnam" unless open OUT, ">$ofnam";
    while (<IN>) {
        my $pre = '';
        $pre = $1 if m|^\s*(//[/!])|s;
        
        if ($pre ne $prefix) {
            process_section();
            @curr = ();
            $prefix = $pre;
        }
        
        push @curr, $_;
    }
    process_section() if @curr;
    die "Failed reading file: $fnam" unless close IN;
    die "Failed writing file: $ofnam" unless close OUT;
    
    if ($inplace) {
        my $f1 = readfile($fnam);
        my $f2 = readfile($ofnam);
        if ($f1 ne $f2) {
            if (rename $ofnam, $fnam) {
                print "Updated $fnam\n";
            } else {
                print "Failed to rename file $ofnam to $fnam\n";
            }
        } else {
            unlink $ofnam;
        }
    }
}

for (sort keys %unknown) {
    print "WARNING: Link format not known: $_\n";
}
