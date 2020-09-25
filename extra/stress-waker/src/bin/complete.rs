//! There are two operations on the waker bitmap tree: setting a bit,
//! and collecting the bits (zeroing them in the process).  These two
//! operations may race against one another.  Collecting works down
//! the tree atomically clearing bits, and setting a bit goes the
//! other way, working **up** the tree atomically setting bits.  It is
//! expected that these two operations will occasionally cross over.
//! When they do, the code is designed such that it should not lose a
//! wake, and such that it should give nothing worse than a spurious
//! wake-up.  This test verifies that that occurs.

use stakker::Stakker;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::thread;
use std::time::Instant;
use stress_waker::notify_channel;

/// Number of wakers to use
const COUNT: i32 = 1000;
/// Number of times to repeat the test
const REPEAT: i32 = 10000;

fn main() {
    let now = Instant::now();
    let mut stakker = Stakker::new(now);
    let s = &mut stakker;
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
        for w in &wakers {
            w.wake();
        }
    });

    for _ in 0..REPEAT {
        // Tell the thread to run all the wakers
        run_tx();

        let mut count = 0;
        while count < COUNT {
            wake_rx();
            let before = woken.borrow().len();
            s.poll_wake();
            s.run(now, false);
            count += (woken.borrow().len() - before) as i32;
            woken.borrow_mut().push_back(-2);
        }

        // Check and report results
        let mut woken = std::mem::replace(&mut *woken.borrow_mut(), VecDeque::new());
        let mut set = vec![1u8; COUNT as usize];
        for id in &woken {
            if *id >= 0 {
                set[*id as usize] = 0;
            }
        }
        for id in 0..COUNT {
            if set[id as usize] != 0 {
                println!("ERROR: Missing wake: {}", id);
                std::process::exit(1);
            }
        }

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
