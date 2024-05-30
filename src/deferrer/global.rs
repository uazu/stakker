use crate::queue::FnOnceQueue;
use crate::{task::Task, Stakker};
use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::mem;
use std::mem::MaybeUninit;
use std::sync::Once;
use std::thread::ThreadId;

/// ONCE protects initialisation of THREAD_ID and GLOBALS.  This is
/// called rarely, so no need to look for a more efficient way.
static ONCE: Once = Once::new();

/// Thread-ID of the only thread that is allowed to access GLOBALS.
/// This is accessed mutably only when protected by ONCE.  `Once`
/// guarantees that only one thread will make that access.  After that
/// it is only accessed immutably.
static THREAD_ID: SyncUnsafeCell<Option<ThreadId>> = SyncUnsafeCell {
    cell: UnsafeCell::new(None),
};

/// Global variables protected by THREAD_ID.  These are only accessed
/// on the "chosen" thread.
static GLOBALS: GlobalsCell = GlobalsCell {
    cell: UnsafeCell::new(Globals {
        queue: MaybeUninit::uninit(),
        task: None,
    }),
};

/// Similar to the one in std nightly
struct SyncUnsafeCell<T> {
    cell: UnsafeCell<T>,
}
unsafe impl<T: Sync> Sync for SyncUnsafeCell<T> {}

/// Global variables
struct Globals {
    /// Active queue.  Only accessed when thread-ID matches, so
    /// doesn't have to be `Sync`.
    ///
    /// We can't static-initialise this due to problems making
    /// `FnOnceQueue::new()` const in queue/boxed.rs (due to compiler
    /// complaining about `&mut` in the type created by `Vec::new()`).
    /// `MaybeUninit` is used because all the code that accesses it
    /// can only run after this structure is initialised, so at that
    /// point we can assume that it is initialised without doing any
    /// checks, for efficiency.
    queue: MaybeUninit<FnOnceQueue<Stakker>>,

    /// Current `Task`.  Only accessed when thread-ID matches, so
    /// doesn't have to be `Sync`.
    task: Option<Task>,
}

struct GlobalsCell {
    cell: UnsafeCell<Globals>,
}

/// Safety: We have to claim that this is `Sync` even though it isn't
/// or else `static` won't accept it.  However the thread-ID check
/// means that the members are not accessed except on the original
/// thread.
unsafe impl Sync for GlobalsCell {}

/// Safety: Caller must not call this unless the thread-ID check has
/// passed.  Caller must make sure that the returned reference does
/// not coexist in time with any other `&mut Globals` reference.  This
/// includes calling anything that might get another reference further
/// down the stack.
#[inline(always)]
unsafe fn globals() -> &'static mut Globals {
    &mut *GLOBALS.cell.get()
}

// Use *const to make it !Send and !Sync
#[derive(Clone)]
pub struct DeferrerAux(PhantomData<*const u8>);

impl DeferrerAux {
    pub(crate) fn new() -> Self {
        // Safety: GLOBALS must only ever be accessed from a single
        // "chosen" thread.  The first thread that calls this method
        // gets its `ThreadId` written to `THREAD_ID` and becomes the
        // "chosen" thread.  After that, any other thread calling this
        // method will panic.  Races in initialisation are handled by
        // `Once`.  The returned instance of this type is !Send and
        // !Sync, and so is not accessible from other threads.  So
        // access to this instance, or a clone of it, guarantees that
        // we are running on the thread that has permission to use
        // GLOBALS.
        //
        // `queue` can't be initialised statically, so it is
        // initialised here when the `THREAD_ID` is set, which
        // guarantees that it is initialised only once, and is always
        // present for the other method calls.
        let tid = std::thread::current().id();
        unsafe {
            ONCE.call_once(|| {
                *THREAD_ID.cell.get() = Some(tid);
                globals().queue.write(FnOnceQueue::new());
            });
            assert_eq!(
                *THREAD_ID.cell.get(),
                Some(tid),
                "Attempted to create another Stakker instance on a different thread.  {}",
                "Enable crate feature `multi-thread` to allow this."
            );
        }
        Self(PhantomData)
    }

    pub(crate) fn swap_queue(&self, queue: &mut FnOnceQueue<Stakker>) {
        // Safety: Access to `self` means that the running thread has
        // exclusive access to GLOBALS.  The operation doesn't call
        // into any method which might also attempt to access the same
        // global.  This code cannot be called before `queue` is
        // initialised.
        unsafe {
            mem::swap(queue, globals().queue.assume_init_mut());
        }
    }

    #[inline]
    pub fn defer(&self, f: impl FnOnce(&mut Stakker) + 'static) {
        // Safety: Access to `self` means that the running thread has
        // exclusive access to GLOBALS.  The operation doesn't call
        // into any method which might also attempt to access the same
        // global.  This code cannot be called before `queue` is
        // initialised.
        unsafe {
            globals().queue.assume_init_mut().push(f);
        };
    }

    #[inline]
    pub fn task_replace(&self, task: Option<Task>) -> Option<Task> {
        // Safety: Access to `self` means that the running thread has
        // exclusive access to GLOBALS.  The operation doesn't call
        // into any method which might also attempt to access the same
        // global.
        unsafe { mem::replace(&mut globals().task, task) }
    }
}
