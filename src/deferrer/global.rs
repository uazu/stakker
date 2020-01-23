use crate::queue::FnOnceQueue;
use crate::Stakker;
use std::marker::PhantomData;
use std::mem;
use std::sync::Once;
use std::thread::ThreadId;

// ONCE protects initialisation of LOCKED_TO_THREAD and QUEUE
static ONCE: Once = Once::new();

// Thread onto which this QUEUE is locked.  Once initialised it
// remains constant.
static mut LOCKED_TO_THREAD: Option<ThreadId> = None;

// QUEUE is only ever accessed by the thread registered in
// LOCKED_TO_THREAD, which is guaranteed by the checks below.
//
// TODO: Remove the Option when FnOnceQueue::new() can be made const.
// Right now Vec::new() isn't const due to some trait bound problem.
static mut QUEUE: Option<FnOnceQueue<Stakker>> = None;

// Use *const to make it !Send and !Sync
#[derive(Clone)]
pub struct DeferrerAux(PhantomData<*const u8>);

impl DeferrerAux {
    pub(crate) fn new() -> Self {
        // Safety: Once LOCKED_TO_THREAD is set to a ThreadId, the
        // QUEUE will only be accessed from that thread.  This is
        // guaranteed by this check and the fact that DeferrerAux is
        // !Send and !Sync.  So there are no data races on QUEUE.
        let tid = std::thread::current().id();
        unsafe {
            ONCE.call_once(|| {
                LOCKED_TO_THREAD = Some(tid);
                QUEUE = Some(FnOnceQueue::new());
            });
            assert_eq!(
                LOCKED_TO_THREAD,
                Some(tid),
                "Attempted to create another Stakker instance on a different thread.  {}",
                "Enable crate feature `multi-thread` to allow this."
            );
        }
        Self(PhantomData)
    }

    pub(crate) fn replace_queue(&mut self, queue: FnOnceQueue<Stakker>) -> FnOnceQueue<Stakker> {
        // Safety: The running thread has exclusive access to QUEUE;
        // see above
        unsafe {
            mem::replace(&mut QUEUE, Some(queue)).expect("Deferrer queue unexpectedly disappeared")
        }
    }

    #[inline]
    pub fn defer(&self, f: impl FnOnce(&mut Stakker) + 'static) {
        // Safety: The running thread has exclusive access to QUEUE;
        // see above
        unsafe {
            if let Some(ref mut queue) = QUEUE {
                queue.push(f)
            }
        };
    }
}
