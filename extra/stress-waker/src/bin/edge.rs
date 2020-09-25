//! This attempts to test the edge case of the final wake crossing
//! over with a collect.  Since this is extremely time-sensitive, the
//! parameters in this file will need to be adjusted for the tested
//! CPU, especially RANGE.
//!
//! What we're aiming for in the output is to see a good number of
//! lines ending with "WAKE 999", which means that the last wake
//! happened at almost the same time as the previous collect.  So if
//! there was a bug, there was a good chance of it being exposed
//! (e.g. by losing the last "wake" and getting stuck in an endless
//! wait).
//!
//! The output can be piped to `sort | uniq -c | sort -rn | less` to
//! see that the "WAIT 999" case is getting exercised enough.

use stakker::Stakker;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::ops::Range;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use stress_waker::notify_channel;

/// Number of wakers to use
const COUNT: i32 = 1000;
/// Number of times to repeat the test
const REPEAT: i32 = 1000000;
/// Waker-count range over which to test.  Will need adjusting for
/// different CPUs, and for release or debug builds
const RANGE: Range<usize> = 180..200;

fn main() {
    let now = Instant::now();
    let mut stakker = Stakker::new(now);
    let s = &mut stakker;

    let estimate = Arc::new(AtomicUsize::new(20));
    let estimate_2 = estimate.clone();

    let (wake_tx, mut wake_rx) = notify_channel();
    s.set_poll_waker(wake_tx);

    let (run_tx, mut run_rx) = notify_channel();

    let mut wakers = Vec::new();
    let woken = Rc::new(RefCell::new(VecDeque::new()));
    for id in 0..COUNT {
        let woken = woken.clone();
        wakers.push(s.waker(move |_, deleted| {
            if !deleted {
                woken.borrow_mut().push_back(id);
            }
        }));
    }

    thread::spawn(move || loop {
        run_rx();
        let mut it = wakers.iter();
        for _ in 0..estimate_2.load(Ordering::SeqCst) {
            if let Some(w) = it.next() {
                w.wake();
            }
        }
        // Always wake #999 at the end
        wakers.last().unwrap().wake();
    });

    let mut est_it = RANGE.clone();
    for _ in 0..REPEAT {
        let est = if let Some(est) = est_it.next() {
            est
        } else {
            est_it = RANGE.clone();
            est_it.next().unwrap()
        };
        estimate.store(est, Ordering::SeqCst);

        // Tell the thread to run the wakers
        run_tx();

        while woken.borrow().back() != Some(&999) {
            woken.borrow_mut().push_back(-2);
            wake_rx();
            s.poll_wake();
            s.run(now, false);
        }

        // Check and report results
        let mut woken = std::mem::replace(&mut *woken.borrow_mut(), VecDeque::new());
        while let Some(fr) = woken.pop_front() {
            if fr < 0 {
                print!("WAIT ");
            } else {
                let mut to = fr;
                while woken.front() == Some(&(to + 1)) {
                    to = woken.pop_front().unwrap();
                }
                if to == fr {
                    print!("{} ", fr);
                } else {
                    print!("{}-{} ", fr, to);
                }
            }
        }
        println!();
    }
    println!("SUCCESS");
}
