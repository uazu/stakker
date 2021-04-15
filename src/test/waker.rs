#![cfg(feature = "inter-thread")]

//! Test `Waker` functionality
//!
//! This just checks that wakes come through as expected, and that the
//! drop handler runs okay, with a large combination of orderings.  A
//! longer test to stress the code multi-threaded for races can be
//! found under `extra/`.

use crate::*;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

// Test it all in one thread so that we have total control over the
// order of operations, to exercise many combinations but reliably and
// repeatably.
//
// Rather than try to specifically test particular cases, instead use
// a pseudo-random generator with a fixed seed to drive the test, and
// then check the output to see that this exercises things
// sufficiently.
//
// MIRI takes 4GB+ and much too long to run this test, so skip.
#[test]
#[cfg_attr(miri, ignore)]
fn waker_test() {
    let now = Instant::now();
    let mut stakker = Stakker::new(now);
    let s = &mut stakker;

    let notified1 = Arc::new(AtomicUsize::new(0));
    let notified2 = notified1.clone();
    s.set_poll_waker(move || notified2.store(1, Ordering::SeqCst));
    let notified = move || 0 != notified1.swap(0, Ordering::SeqCst);

    #[derive(Default)]
    struct State {
        awaiting: HashSet<u32>,
        running: HashSet<u32>,
    }
    let state = Rc::new(RefCell::new(State::default()));

    let check_awaiting = || {
        if !state.borrow_mut().awaiting.is_empty() {
            panic!("Still awaiting call for: {:?}", state.borrow_mut().awaiting);
        }
    };
    let check_running = || {
        if !state.borrow_mut().running.is_empty() {
            panic!("Still awaiting drop for: {:?}", state.borrow_mut().running);
        }
    };

    let handle_notify = |s: &mut Stakker| {
        if notified() {
            s.poll_wake();
            s.run(now, false);
        }
    };

    // ZX Spectrum 16-bit pseudo-random number generator
    let mut seed: usize = 12345;
    let mut rand = |n: usize| {
        seed = ((seed + 1) * 75) % 65537 - 1;
        ((seed * n) >> 16) as usize
    };

    struct IdAndWaker {
        id: u32,
        waker: Waker,
    }
    impl std::fmt::Debug for IdAndWaker {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.id)
        }
    }

    let mut id = 0;
    let mut new_waker = |s: &mut Stakker| {
        id += 1;
        let state = state.clone();
        let id = id;
        state.borrow_mut().running.insert(id);
        IdAndWaker {
            id,
            waker: s.waker(move |_, deleted| {
                let mut st = state.borrow_mut();
                if deleted {
                    // Should only be deleted once
                    println!("handle drop {}", id);
                    assert!(st.running.remove(&id));
                } else {
                    // Spurious wakeups are okay, so long as there is
                    // at least one
                    println!("handle wake {}", id);
                    st.awaiting.remove(&id);
                }
            }),
        }
    };

    // Start with 320 wakers.  We need at least 64 to use more than
    // one Leaf in the bitmap.
    let mut wakers = Vec::new();
    for _ in 0..320 {
        wakers.push(new_waker(s));
    }

    while !wakers.is_empty() {
        match rand(4) {
            0 => {
                // If there has been a notify, run handlers.  This
                // should leave the `awaiting` set empty.
                println!("RUN");
                handle_notify(s);
                check_awaiting();
            }
            1 => {
                // Wake a random waker
                let w = &wakers[rand(wakers.len())];
                println!("WAKE {}", w.id);
                state.borrow_mut().awaiting.insert(w.id);
                w.waker.wake();
            }
            2 => {
                // Drop a random waker.  If we drop a waker and it
                // has a call outstanding, that call is lost.
                // (Well it is not lost but the drop gets executed
                // first.)  So remove it from `awaiting`.
                let w = wakers.remove(rand(wakers.len()));
                state.borrow_mut().awaiting.remove(&w.id);
                println!("DROP {}", w.id);
                drop(w);
            }
            _ => {
                if rand(100) < 80 {
                    // Add another waker.  This tests re-use of slots.
                    // Runs 80% of the time, vs 100% for drop, so this
                    // is biased towards reducing the number of
                    // wakers, until there are none (to end the test).
                    let w = new_waker(s);
                    println!("ADD {}", w.id);
                    wakers.push(w);
                }
            }
        }
    }
    handle_notify(s);

    // Expect all waker handlers and all drop handlers to have run
    check_awaiting();
    check_running();

    // Check the number of slots in use in the Slab.  The drop handler
    // will still be there, but all others should have been cleaned up
    assert!(s.poll_waker_handler_count() <= 1);
}
