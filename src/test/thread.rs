#![cfg(feature = "inter-thread")]

//! Test `PipedThread` functionality

use crate::sync::{Channel, ChannelGuard};
use crate::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use std::time::Instant;

/// Simple channel for sending and waiting for notification events.
/// Returns (send, recv) closures.  This means that we can simulate a
/// `mio` waker (or equivalent) for testing without needing to pull in
/// `stakker_mio`.
fn notify_channel() -> (impl Fn() + Send + Sync, impl FnMut() + Send + Sync) {
    let pair1 = Arc::new((Mutex::new(0_usize), Condvar::new()));
    let pair2 = pair1.clone();
    let mut count = 0;
    (
        move || {
            let mut lock = pair1.0.lock().unwrap();
            *lock = lock.wrapping_add(1);
            pair1.1.notify_one();
        },
        move || {
            let mut lock = pair2.0.lock().unwrap();
            while *lock == count {
                lock = pair2.1.wait(lock).unwrap();
            }
            count = *lock;
        },
    )
}

// Test thread that accepts a value, does some processing, and then
// returns the result.  Also test shutting down the thread from the
// main thread.
test_fn!(
    fn pipedthread_processing() {
        struct Test {
            thread: Option<PipedThread<usize, usize>>,
            expect: usize,
        }
        impl Test {
            fn init(cx: CX![]) -> Option<Self> {
                let mut thread = PipedThread::spawn(
                    fwd_to!([cx], recv() as (usize)),
                    fwd_to!([cx], term() as (Option<String>)),
                    cx,
                    move |link| {
                        while let Some(v) = link.recv() {
                            link.send(v * 5);
                        }
                    },
                );
                thread.send(1);
                Some(Self {
                    thread: Some(thread),
                    expect: 5,
                })
            }
            fn recv(&mut self, _cx: CX![], mut v: usize) {
                if v != self.expect {
                    panic!("Thread returned unexpected value: {} != {}", v, self.expect);
                }
                if v < 1000 {
                    v += 1;
                    self.expect = v * 5;
                    if let Some(ref mut t) = self.thread {
                        t.send(v);
                    }
                } else {
                    // Cause thread shutdown from actor end
                    self.thread = None;
                }
            }
            fn term(&mut self, cx: CX![], panic: Option<String>) {
                if let Some(msg) = panic {
                    panic!("Unexpected thread failure: {}", msg);
                }
                cx.stop();
            }
        }

        let now = Instant::now();
        let mut stakker = Stakker::new(now);
        let s = &mut stakker;
        let (tx, mut rx) = notify_channel();
        s.set_poll_waker(tx);

        let _actor = actor!(s, Test::init(), ret_shutdown!(s));
        s.run(now, false);
        while s.not_shutdown() {
            rx();
            s.poll_wake();
            s.run(now, false);
        }
    }
);

// Test thread that accepts a value, does some processing, and then
// returns the result.  Terminates the thread early to test
// notification from the thread back to the main thread.
test_fn!(
    fn pipedthread_terminate() {
        struct Test {
            thread: PipedThread<usize, usize>,
            expect: usize,
        }
        impl Test {
            fn init(cx: CX![]) -> Option<Self> {
                let mut thread = PipedThread::spawn(
                    fwd_to!([cx], recv() as (usize)),
                    fwd_to!([cx], term() as (Option<String>)),
                    cx,
                    move |link| {
                        while let Some(v) = link.recv() {
                            if v > 1000 {
                                break; // Terminate the thread
                            }
                            link.send(v * 5);
                        }
                    },
                );
                thread.send(1);
                Some(Self { thread, expect: 5 })
            }
            fn recv(&mut self, _cx: CX![], mut v: usize) {
                if v != self.expect {
                    panic!("Thread returned unexpected value: {} != {}", v, self.expect);
                }
                v += 1;
                self.expect = v * 5;
                self.thread.send(v);
            }
            fn term(&mut self, cx: CX![], panic: Option<String>) {
                if let Some(msg) = panic {
                    panic!("Unexpected thread failure: {}", msg);
                }
                cx.stop();
            }
        }

        let now = Instant::now();
        let mut stakker = Stakker::new(now);
        let s = &mut stakker;
        let (tx, mut rx) = notify_channel();
        s.set_poll_waker(tx);

        let _actor = actor!(s, Test::init(), ret_shutdown!(s));
        s.run(now, false);
        while s.not_shutdown() {
            rx();
            s.poll_wake();
            s.run(now, false);
        }
    }
);

// Test thread that generates values at intervals and then terminates.
test_fn!(
    fn pipedthread_generate() {
        struct Test {
            _thread: PipedThread<usize, usize>,
            expect: usize,
        }
        impl Test {
            fn init(cx: CX![]) -> Option<Self> {
                let thread = PipedThread::spawn(
                    fwd_to!([cx], recv() as (usize)),
                    fwd_to!([cx], term() as (Option<String>)),
                    cx,
                    move |link| {
                        for v in 0..10 {
                            std::thread::sleep(Duration::from_millis(10));
                            link.send(v);
                        }
                    },
                );
                Some(Self {
                    _thread: thread,
                    expect: 0,
                })
            }
            fn recv(&mut self, _cx: CX![], v: usize) {
                if v != self.expect {
                    panic!("Thread returned unexpected value: {} != {}", v, self.expect);
                }
                self.expect += 1;
            }
            fn term(&mut self, cx: CX![], panic: Option<String>) {
                if let Some(msg) = panic {
                    panic!("Unexpected thread failure: {}", msg);
                }
                assert_eq!(self.expect, 10);
                cx.stop();
            }
        }

        let now = Instant::now();
        let mut stakker = Stakker::new(now);
        let s = &mut stakker;
        let (tx, mut rx) = notify_channel();
        s.set_poll_waker(tx);

        let _actor = actor!(s, Test::init(), ret_shutdown!(s));
        s.run(now, false);
        while s.not_shutdown() {
            rx();
            s.poll_wake();
            s.run(now, false);
        }
    }
);

// Test thread that accepts values at intervals and then terminates.
test_fn!(
    fn pipedthread_sink() {
        struct Test {
            _thread: PipedThread<usize, usize>,
        }
        impl Test {
            fn init(cx: CX![]) -> Option<Self> {
                let mut thread = PipedThread::spawn(
                    fwd_panic!("Not expecting thread to send data"),
                    fwd_to!([cx], term() as (Option<String>)),
                    cx,
                    move |link| {
                        let mut expect = 0;
                        while let Some(v) = link.recv() {
                            assert_eq!(expect, v);
                            expect += 1;
                            std::thread::sleep(Duration::from_millis(10));
                            if expect == 10 {
                                break;
                            }
                        }
                    },
                );
                for v in 0..10 {
                    thread.send(v);
                }
                Some(Self { _thread: thread })
            }
            fn term(&mut self, cx: CX![], panic: Option<String>) {
                if let Some(msg) = panic {
                    panic!("Unexpected thread failure: {}", msg);
                }
                cx.stop();
            }
        }

        let now = Instant::now();
        let mut stakker = Stakker::new(now);
        let s = &mut stakker;
        let (tx, mut rx) = notify_channel();
        s.set_poll_waker(tx);

        let _actor = actor!(s, Test::init(), ret_shutdown!(s));
        s.run(now, false);
        while s.not_shutdown() {
            rx();
            s.poll_wake();
            s.run(now, false);
        }
    }
);

// Test that the panic is notified back correctly when the thread
// panics
test_fn!(
    fn pipedthread_panic() {
        struct Test {
            _thread: PipedThread<usize, usize>,
        }
        impl Test {
            fn init(cx: CX![]) -> Option<Self> {
                let thread = PipedThread::spawn(
                    fwd_panic!("Not expecting thread to send data"),
                    fwd_to!([cx], term() as (Option<String>)),
                    cx,
                    move |_| {
                        std::thread::sleep(Duration::from_millis(10));
                        panic!("TEST PANIC");
                    },
                );
                Some(Self { _thread: thread })
            }
            fn term(&mut self, cx: CX![], panic: Option<String>) {
                if let Some(msg) = panic {
                    if msg != "TEST PANIC" {
                        panic!("Unexpected thread failure: {}", msg);
                    }
                } else {
                    panic!("Unexpected successful completion of thread");
                }
                cx.stop();
            }
        }

        let now = Instant::now();
        let mut stakker = Stakker::new(now);
        let s = &mut stakker;
        let (tx, mut rx) = notify_channel();
        s.set_poll_waker(tx);

        let _actor = actor!(s, Test::init(), ret_shutdown!(s));
        s.run(now, false);
        while s.not_shutdown() {
            rx();
            s.poll_wake();
            s.run(now, false);
        }
    }
);

// Test sending data with a `Channel`, and test "close" behaviour.
test_fn!(
    fn channel() {
        struct Test {
            expect: usize,
            guard: Option<ChannelGuard>,
            done: Arc<AtomicBool>,
        }
        impl Test {
            fn init(cx: CX![]) -> Option<Self> {
                let fwd = fwd_to!([cx], recv() as (usize));
                let (channel, guard) = Channel::new(cx, fwd);
                let done = Arc::new(AtomicBool::new(false));
                let done_clone = done.clone();
                std::thread::spawn(move || {
                    for value in 0..20 {
                        std::thread::sleep(Duration::from_millis(10));
                        if !channel.send(value) {
                            // Main thread closes channel after 9
                            // received.  Give it a generous 50ms for the
                            // close to go through.
                            assert!(value >= 10 && value < 15, "Test system overloaded?");
                            done_clone.store(true, Ordering::SeqCst);
                            return;
                        }
                    }
                    panic!("Test system overloaded?");
                });
                Some(Self {
                    expect: 0,
                    guard: Some(guard),
                    done,
                })
            }
            fn recv(&mut self, cx: CX![], value: usize) {
                assert_eq!(value, self.expect);
                self.expect += 1;
                if self.expect == 10 {
                    // Drop guard (closing channel)
                    self.guard = None;
                    // Wait for thread to finish.  This is blocking code
                    // but it's just for testing
                    for _ in 0..50 {
                        if self.done.load(Ordering::SeqCst) {
                            return cx.stop();
                        }
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    panic!("Thread did not stop within 50ms");
                }
            }
        }

        let now = Instant::now();
        let mut stakker = Stakker::new(now);
        let s = &mut stakker;
        let (tx, mut rx) = notify_channel();
        s.set_poll_waker(tx);

        let _actor = actor!(s, Test::init(), ret_shutdown!(s));
        s.run(now, false);
        while s.not_shutdown() {
            rx();
            s.poll_wake();
            s.run(now, false);
        }
    }
);
