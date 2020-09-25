#!/bin/bash

TOP=.
[ ! -d $TOP/src/test ] && {
    TOP=..
    [ ! -d $TOP/src/test ] && { echo "Running in wrong directory"; exit 1; }
}

rm -r $TOP/src/test/extra

cat >$TOP/src/test/extra.rs <<EOF
//! Normally this is empty, but when extra tests are built, this is
//! modified.  However those changes should not be checked in.
EOF
