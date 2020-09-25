#!/bin/bash

# The intention is to test all sizes and alignments crossing the
# break-point where the code decides to allocate a new queue block.

TOP=.
[ ! -d $TOP/src/test ] && {
    TOP=..
    [ ! -d $TOP/src/test ] && { echo "Running in wrong directory"; exit 1; }
}

mkdir $TOP/src/test/extra
FILE=$TOP/src/test/extra/tmp_stress_queue.rs

echo "mod tmp_stress_queue; // DON'T CHECK THIS CHANGE IN" >$TOP/src/test/extra.rs

echo "Writing test to $FILE"
(
cat <<EOF
use crate::queue::FnOnceQueue;

struct Confirm(u64);
impl Confirm {
    fn push(&mut self, nyb: u64) {
        self.0 = (self.0 << 4) + nyb;
    }
}

// Check that there is a transition from false to true over the vector
fn check_vec(v: Vec<bool>) {
    match (v.first(), v.last()) {
        (Some(false), Some(true)) => (),
        _ => panic!("Didn't cover break-point"),
    }
}
EOF

CALL1=""
CALL2=""
for A in 1 2 4 8 16 32 64; do
    CALL1="$CALL1 check_vec(vec!["
    CALL2="$CALL2 check_vec(vec!["
    for N in {112..128} #{2016..2032}
    do
        let L=N*8
        AL="${A}_${L}"
        CALL1="$CALL1 check_empty_a$AL(),"
        CALL2="$CALL2 check_single_a$AL(),"
        cat <<EOF
#[repr(align(${A}))]
struct A$AL([u8; $L]);

fn accept_a$AL(c: &mut Confirm, v: A$AL) {
    c.push(0x2);
    assert_eq!(v.0[0], 123);
    assert_eq!(v.0[$L-1], 234);
}

#[inline(never)]
fn push_a$AL(queue: &mut FnOnceQueue<Confirm>, v: A$AL) {
    queue.push(move |c| accept_a$AL(c, v));
}

#[inline(never)]
fn check_empty_a$AL() -> bool {
    // First push directly from 0
    let mut confirm = Confirm(0xF);
    let mut queue = FnOnceQueue::<Confirm>::new();
    assert_eq!(queue.cap(), 0);
    let mut v = A$AL([0; $L]);
    v.0[0] = 123;
    v.0[$L-1] = 234;
    push_a$AL(&mut queue, v);
    let rv = queue.cap() == 2048;
    println!("A$AL on empty -> {}", queue.cap());
    queue.execute(&mut confirm);
    assert_eq!(confirm.0, 0xF2);
    rv
}

#[inline(never)]
fn check_single_a$AL() -> bool {
    // Then push with a small item already on the queue
    let mut confirm = Confirm(0xF);
    let mut queue = FnOnceQueue::<Confirm>::new();
    queue.push(|c| c.push(0x1));
    assert_eq!(queue.cap(), 1024);
    let mut v = A$AL([0; $L]);
    v.0[0] = 123;
    v.0[$L-1] = 234;
    push_a$AL(&mut queue, v);
    let rv = queue.cap() == 2048;
    println!("A$AL on non-empty -> {}", queue.cap());
    queue.execute(&mut confirm);
    assert_eq!(confirm.0, 0xF12);
    rv
}
EOF
    done
    CALL1="$CALL1 ]);"
    CALL2="$CALL2 ]);"
done

cat <<EOF
#[test]
fn check() {
    $CALL1
    $CALL2
}
EOF

) >$FILE

# This builds and tests
cargo test --lib tmp_stress_queue -- --nocapture

# Retest with valgrind (after build has been done) for any other issues
SUPP=$PWD/tmp-valgrind-suppressions.txt
cat >$SUPP <<EOF
{
    uninteresting statx problem
    Memcheck:Param
    statx(file_name)
    fun:syscall
    fun:statx
}
{
    uninteresting statx problem
    Memcheck:Param
    statx(buf)
    fun:syscall
    fun:statx
}
EOF

valgrind \
    --tool=memcheck \
    --trace-children=yes \
    --leak-check=full \
    --suppressions=$SUPP \
    --show-leak-kinds=definite \
    --undef-value-errors=no \
    cargo test --lib tmp_stress_queue -- --nocapture 2>&1

rm $SUPP
