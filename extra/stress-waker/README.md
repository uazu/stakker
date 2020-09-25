These two tests stress the `Waker` internal implementation across
threads.  See the comments in each file.  One checks that all wakes
come through correctly when collection occurs in parallel, and the
other checks that a final wake that clashes with a collection also
comes through okay.

These will probably need adjusting to exercise things properly on
another CPU since timing is so sensitive.  So if you want to try
these, be ready to tweak things until you're comfortable that things
have been tested sufficiently on your CPU.
