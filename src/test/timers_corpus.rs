use crate::*;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::time::{Duration, Instant};

// This uses a snapshot of a small fuzzing corpus that gives full
// coverage of fixed timer addition and execution in `cargo fuzz`

// TODO: Re-enable MIRI test when timers have been switched to use
// N-ary heap.  This can't run under MIRI right now.  It runs very
// slowly using a huge amount of memory until eventually it crashes
// trying to allocate even more memory.

#[test]
#[cfg_attr(miri, ignore)]
fn run() {
    let data = std::include_bytes!("timers_corpus.bin");
    let mut p = &data[..];
    while !p.is_empty() {
        let len = ((p[0] as usize) << 8) + (p[1] as usize);
        fuzz_timers(&p[2..2 + len]);
        p = &p[2 + len..];
    }
}

// Add timers and remove items from queue, checking that all of them
// come out correctly, and that timing is as expected

pub fn fuzz_timers(data: &[u8]) {
    if data.is_empty() {
        return;
    }
    let mut now = Instant::now();
    let mut stakker = Stakker::new(now);
    let s = &mut stakker;
    let mut seq = 1;
    let expecting = Rc::new(RefCell::new(HashSet::new()));

    // pull_inc value of 0 means never pull, 255 means pull after
    // every push.  This means that the fuzzer can exercise moving
    // data out of the queue whilst adding as well.
    let pull_inc = data[0] as u32;
    let mut pull_val = 128;

    for b in &data[1..] {
        seq += 1;
        // Range is roughly exponential from 0 to 491520, which with
        // ~0.853s unit is 0 to ~116 hours.  This means we test quite
        // a few cases of over 9 hours, which causes different code to
        // run, since the maximum delay supported within the queue is
        // ~9 hours.
        let dur = ((*b & 15) as u64) << ((*b >> 4) as u32);
        // Use non-exact value to exercise internal time format
        // conversion and manipulation
        let dur = Duration::from_micros(dur * 853439);
        let expiry = now + dur;
        expecting.borrow_mut().insert(seq);
        let exp = expecting.clone();
        let seq2 = seq;
        at!(expiry, [s], |s| {
            let now = s.now();
            // Resolution of internal time format is just less than
            // 17us, so expect the expiry to run within range of
            // expiry to expiry+17us
            assert!(now >= expiry);
            assert!(now < expiry + Duration::from_micros(17));
            assert!(exp.borrow_mut().remove(&seq2));
        });

        pull_val += pull_inc;
        if pull_val >= 255 {
            pull_val -= 255;
            now = s.next_expiry().unwrap();
            s.run(now, false);
        }
    }

    // Run the rest of the timers
    while let Some(next) = s.next_expiry() {
        now = next;
        s.run(now, false);
    }

    // Check that they have all completed
    assert!(expecting.borrow_mut().is_empty());
}
