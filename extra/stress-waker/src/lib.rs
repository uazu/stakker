use std::sync::{Arc, Condvar, Mutex};

/// Simple channel for sending and waiting for notification events.
/// Returns (send, recv) closures.
#[allow(clippy::mutex_atomic)]
pub fn notify_channel() -> (impl Fn() + Send + Sync, impl FnMut() + Send + Sync) {
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
