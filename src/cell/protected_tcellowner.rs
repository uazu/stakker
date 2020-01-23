// When doing testing, `TCellOwner::new()` panics immediately, because
// cargo runs the tests on many threads.  So it is necessary to use
// locks to avoid collisions.
use once_cell::sync::Lazy;
use qcell::{TCell, TCellOwner};
use std::sync::{Condvar, Mutex};
use std::thread::ThreadId;

struct Locked {
    thread: ThreadId,
    count: usize,
}

static LOCK: Lazy<Mutex<Option<Locked>>> = Lazy::new(|| Mutex::new(None));
static WAIT: Lazy<Condvar> = Lazy::new(|| Condvar::new());

pub struct ProtectedTCellOwner<Q: 'static> {
    owner: TCellOwner<Q>,
}

impl<Q> ProtectedTCellOwner<Q> {
    pub fn new() -> Self {
        let mut lock = LOCK.lock().unwrap();
        let thread = std::thread::current().id();
        loop {
            match *lock {
                None => {
                    *lock = Some(Locked { thread, count: 1 });
                }
                Some(ref mut locked) if locked.thread == thread => {
                    locked.count += 1;
                }
                Some(_) => {
                    lock = WAIT.wait(lock).unwrap();
                    // Loop around and see if we can get the lock now
                    continue;
                }
            }
            break;
        }

        Self {
            owner: TCellOwner::new(),
        }
    }

    pub fn ro<'a, T>(&'a self, tc: &'a TCell<Q, T>) -> &'a T {
        self.owner.ro(tc)
    }

    pub fn rw<'a, T>(&'a mut self, tc: &'a TCell<Q, T>) -> &'a mut T {
        self.owner.rw(tc)
    }

    pub fn cell<T>(&self, value: T) -> TCell<Q, T> {
        self.owner.cell(value)
    }
}

impl<Q> Drop for ProtectedTCellOwner<Q> {
    fn drop(&mut self) {
        let mut lock = LOCK.lock().unwrap();
        let thread = std::thread::current().id();
        match *lock {
            Some(ref mut locked) if thread == locked.thread => {
                locked.count -= 1;
                if locked.count == 0 {
                    *lock = None;
                    drop(lock);
                    WAIT.notify_one();
                }
            }
            _ => panic!("ProtectedTCellOwner locking found in unexpected state"),
        }
    }
}
