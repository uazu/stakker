use crate::{Core, Fwd, Waker};
use std::collections::VecDeque;
use std::mem;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Condvar, Mutex};

// TODO: Reallocate queues occasionally in case they grow huge?
// TODO: Fixed-size queue option to avoid allocations whilst locked?

// Uses a Mutex internally.  We expect contention to be very low,
// since operations are quick and there are only two threads involved.
// So hopefully almost all mutex locks will be handled in userspace,
// not needing to go to the OS.
struct Queues<O: Send + Sync + 'static, I: Send + Sync + 'static> {
    mutex: Mutex<QueuesInner<O, I>>,
    condvar: Condvar,
}

struct QueuesInner<O: Send + Sync + 'static, I: Send + Sync + 'static> {
    cancel: bool,          // Set if PipedThread instance is dropped
    panic: Option<String>, // Panic that occurred in the thread, or None
    send: VecDeque<O>,
    recv: Vec<I>,
}

/// A thread connected to the actor runtime via channels
///
/// This takes care of starting a thread and transferring data to and
/// from it via channels.  Data sent to the thread has type `O`, and
/// data received has type `I`.  These would often be enums to handle
/// different kinds of data (e.g. messages, commands or responses as
/// required).
///
/// This is useful for offloading synchronous or blocking work to
/// another thread.  So the normal pattern of use would be for the
/// thread to block on [`PipedLink::recv`] until it gets something to
/// process.  Processing could involve sending messages on other
/// channels or streams and waiting for replies, or running data in
/// parallel through a thread pool.  Processing the received message
/// might or might not result in a message to send back with
/// [`PipedLink::send`].  Another use could be for blocking input,
/// where the thread waits on a device, and uses [`PipedLink::send`]
/// to pass back received data.
///
/// The only thing that this thread can't do is wait for both
/// [`PipedLink::recv`] and some other input at the same time.  If you
/// need that, for now you'll need to write your own interface code to
/// `crossbeam` or some other channel library, using [`Waker`] to
/// interface back to **Stakker**.
///
/// Cleanup is handled as follows:
///
/// - If the thread terminates normally or panics, then the underlying
/// [`Waker`] notifies the main thread and `fwd_term` is called with
/// the panic error, or `None` if there was no panic.  This handler
/// can discard the [`PipedThread`] instance to complete the cleanup,
/// and start a new thread if necessary.
///
/// - If the [`PipedThread`] instance is dropped in the main thread,
/// then a **cancel** flag is set which the thread will notice next
/// time it tries to send or receive data.  The thread should then
/// terminate.  So if the [`PipedThread`] instance is kept within the
/// same actor that is handling the incoming data, then this takes
/// care of thread cleanup automatically if the actor fails
/// unexpectedly.
///
/// [`PipedLink::recv`]: struct.PipedLink.html#method.recv
/// [`PipedLink::send`]: struct.PipedLink.html#method.send
/// [`PipedThread`]: struct.PipedThread.html
/// [`Waker`]: struct.Waker.html
pub struct PipedThread<O: Send + Sync + 'static, I: Send + Sync + 'static> {
    queues: Arc<Queues<O, I>>,
}

impl<O: Send + Sync + 'static, I: Send + Sync + 'static> PipedThread<O, I> {
    /// Spawn a new thread.  `fwd_recv` will be called for each
    /// incoming message.  `fwd_term` will be called when the thread
    /// terminates with the argument of `None` for normal termination,
    /// or `Some(msg)` for a panic.  The `run` argument is the closure
    /// that will be run within the new thread.  The [`PipedLink`]
    /// argument passed to it allows the new thread to send and
    /// receive messages.
    ///
    /// Note: `core` argument is third argument so that `fwd_to!` and
    /// similar macros can be used directly in the call arguments,
    /// without borrow errors.
    ///
    /// [`PipedLink`]: struct.PipedLink.html
    pub fn spawn(
        fwd_recv: Fwd<I>,
        fwd_term: Fwd<Option<String>>,
        core: &mut Core,
        run: impl FnOnce(&mut PipedLink<O, I>) + Send + 'static,
    ) -> Self {
        let queues = Arc::new(Queues {
            mutex: Mutex::new(QueuesInner {
                cancel: false,
                panic: None,
                send: VecDeque::new(),
                recv: Vec::new(),
            }),
            condvar: Condvar::new(),
        });

        let qu = queues.clone();
        let waker = core.waker(move |_, deleted| {
            let mut panic = None;
            let mut lock = qu.mutex.lock().unwrap();
            let recv = mem::replace(&mut lock.recv, Vec::new());
            if deleted {
                panic = mem::replace(&mut lock.panic, None);
            }
            drop(lock);

            for msg in recv {
                fwd_recv.fwd(msg);
            }
            if deleted {
                fwd_term.fwd(panic);
            }
        });

        let mut pipes = PipedLink {
            queues: queues.clone(),
            waker,
        };

        std::thread::spawn(move || {
            if let Err(e) = std::panic::catch_unwind(AssertUnwindSafe(|| run(&mut pipes))) {
                // Pass through panic message if it is a `String` or
                // `&str`, else generate some debugging output
                let msg = match e.downcast::<String>() {
                    Ok(v) => *v,
                    Err(e) => match e.downcast::<&str>() {
                        Ok(v) => v.to_string(),
                        Err(e) => format!("Panic with unknown type: {:?}", e.type_id()),
                    },
                };
                pipes.queues.mutex.lock().unwrap().panic = Some(msg);
            }
            // The Waker is dropped here, which will notify the main
            // thread of termination
        });

        Self { queues }
    }

    /// Send a message to the thread.  If the thread is blocked on
    /// receive, wake it.
    pub fn send(&mut self, msg: O) {
        let mut lock = self.queues.mutex.lock().unwrap();
        let empty = lock.send.is_empty();
        lock.send.push_back(msg);
        drop(lock);

        if empty {
            self.queues.condvar.notify_all();
        }
    }
}

impl<O: Send + Sync + 'static, I: Send + Sync + 'static> Drop for PipedThread<O, I> {
    fn drop(&mut self) {
        self.queues.mutex.lock().unwrap().cancel = true;
        self.queues.condvar.notify_all();
    }
}

/// Link from a [`PipedThread`] thread back to the main thread
///
/// [`PipedThread`]: struct.PipedThread.html
pub struct PipedLink<O: Send + Sync + 'static, I: Send + Sync + 'static> {
    queues: Arc<Queues<O, I>>,
    waker: Waker,
}

impl<O: Send + Sync + 'static, I: Send + Sync + 'static> PipedLink<O, I> {
    /// Send a message back to the main thread.  Returns `true` on
    /// success.  If `false` is returned, then the [`PipedThread`] has
    /// been dropped and this thread should terminate itself.
    ///
    /// [`PipedThread`]: struct.PipedThread.html
    pub fn send(&mut self, msg: I) -> bool {
        let mut lock = self.queues.mutex.lock().unwrap();
        let cancel = lock.cancel;
        let empty = lock.recv.is_empty();
        lock.recv.push(msg);
        drop(lock);

        if empty {
            self.waker.wake();
        }
        !cancel
    }

    /// Receive a message from the main thread.  Blocks if there is no
    /// message already waiting.  Returns `None` if the
    /// [`PipedThread`] has been dropped, in which case this thread
    /// should terminate itself.
    ///
    /// [`PipedThread`]: struct.PipedThread.html
    pub fn recv(&mut self) -> Option<O> {
        let mut lock = self.queues.mutex.lock().unwrap();
        while !lock.cancel && lock.send.is_empty() {
            lock = self.queues.condvar.wait(lock).unwrap();
        }
        if lock.cancel {
            None
        } else {
            Some(lock.send.pop_front().unwrap())
        }
    }

    /// Check whether cancellation has been flagged by the main
    /// thread.  When the [`PipedThread`] is dropped, the cancel flag
    /// is set to tell this thread to terminate.  If the thread is
    /// doing a long-running operation or blocking, it should check
    /// the **cancel** flag from time to time to recognise this
    /// condition and to clean up in good time.
    ///
    /// [`PipedThread`]: struct.PipedThread.html
    pub fn cancel(&mut self) -> bool {
        self.queues.mutex.lock().unwrap().cancel
    }
}
