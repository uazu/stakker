//! Since 1.67 `RUST_TEST_THREADS=1` no longer runs all tests in a
//! single thread.  Since the Stakker single-threaded tests require
//! that, we can no longer use `RUST_TEST_THREADS=1`.  See:
//! https://github.com/rust-lang/cargo/issues/11896 and
//! https://github.com/rust-lang/rust/issues/104053.
//!
//! So the solution is to run a worker thread and have the tests send
//! work to it and wait for an okay or panic to come back.  This works
//! well for normal `cargo test`.
//!
//! However MIRI testing adds a further complication.  If using a
//! worker thread, MIRI insists that you wait for your worker thread
//! to finish, or else disable MIRI leak checking.  But if you have a
//! test that waits for the worker thread to finish, that blocks a
//! whole thread, and that seems to cause MIRI to block further tests
//! from running.  So as a workaround, MIRI tests have to be run one
//! at a time from outside of the `cargo miri` invocation.  This code
//! assumes that that approach is being taken, and disables the worker
//! thread for MIRI.
//!
//! To use this: Put `test_fn!( ... )` around the whole test function
//! definition, including any outer attributes such as `#[cfg(...)]`
//! or `#[should_panic]`.  This simply adds a `#[test]` in the normal
//! case, or else redirects the code to run in the worker thread in
//! the single-threaded case.

// Simple case, where tests can be run in the normal way, in parallel
#[cfg(any(miri, feature = "multi-stakker", feature = "multi-thread"))]
macro_rules! test_fn {
    ($(#[$attr:meta])* fn $name:ident() $code:expr) => {
        #[test]
        $(#[$attr])*
        fn $name() {
            $code
        }
    };
}

// Case where everything has to be sent to a single worker thread
#[cfg(not(any(miri, feature = "multi-stakker", feature = "multi-thread")))]
macro_rules! test_fn {
    ($(#[$attr:meta])* fn $name:ident() $code:expr) => {
        #[test]
        $(#[$attr])*
        fn $name() {
            crate::test::test::worker::run_on_worker(|| $code);
        }
    };
}

#[cfg(not(any(miri, feature = "multi-stakker", feature = "multi-thread")))]
pub(crate) mod worker {
    use std::sync::mpsc::{channel, Sender};
    use std::sync::Mutex;

    struct Job {
        cb: Box<dyn FnOnce() + Send + 'static>,
        ret: Sender<JobRet>,
    }

    struct JobRet {
        panic: Option<String>,
    }

    struct Worker {
        send: Sender<Job>,
    }

    static WORKER: Mutex<Option<Worker>> = Mutex::new(None);

    fn start_worker() {
        let mut guard = WORKER.lock().expect("Worker lock poisoned");
        if guard.is_none() {
            let (send, recv) = channel::<Job>();
            std::thread::spawn(move || {
                while let Ok(job) = recv.recv() {
                    let panic =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (job.cb)()))
                            .map_err(|e| {
                                // Pass through panic message if it is a `String` or
                                // `&str`, else generate some debugging output
                                match e.downcast::<String>() {
                                    Ok(v) => *v,
                                    Err(e) => match e.downcast::<&str>() {
                                        Ok(v) => v.to_string(),
                                        Err(e) => {
                                            format!("Panicked with unknown type: {:?}", e.type_id())
                                        }
                                    },
                                }
                            })
                            .err();
                    let ret = JobRet { panic };
                    job.ret.send(ret).expect("Test not waiting for result");
                }
            });
            *guard = Some(Worker { send });
        }
    }

    #[track_caller]
    pub(crate) fn run_on_worker(cb: impl FnOnce() + Send + 'static) {
        start_worker();

        let (ret_send, ret_recv) = channel();
        let job = Job {
            cb: Box::new(cb),
            ret: ret_send,
        };

        let mut guard = WORKER.lock().expect("Worker lock poisoned");
        if let Some(ref mut worker) = *guard {
            worker.send.send(job).expect("Worker thread died?");

            let ret = ret_recv.recv().expect("Worker thread died?");
            if let Some(msg) = ret.panic {
                drop(guard);
                panic!("{msg}");
            }
        } else {
            unreachable!();
        }
    }
}
