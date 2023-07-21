use crate::{Core, Fwd, Waker};
use std::sync::{Arc, Mutex};

/// Channel for sending messages to an actor
///
/// A [`Channel`] may be used to send messages of type `M` to an actor
/// from any thread.  It is an unbounded queue.
///
/// Messages are delivered directly to an actor method via a [`Fwd`]
/// instance.  Cleanup of the channel is handled via a
/// [`ChannelGuard`] which should be kept in the same actor.  When
/// this is dropped, the channel is closed, and senders are informed
/// via the [`Channel::send`] and [`Channel::is_closed`] methods.  So
/// this handles cleanup automatically when the actor fails or
/// terminates for any reason.
///
/// [`Channel::is_closed`]: ../sync/struct.Channel.html#method.is_closed
/// [`Channel::send`]: ../sync/struct.Channel.html#method.send
/// [`ChannelGuard`]: ../sync/struct.ChannelGuard.html
/// [`Channel`]: ../sync/struct.Channel.html
/// [`Fwd`]: ../struct.Fwd.html
pub struct Channel<M: Send> {
    arc: Arc<Mutex<ChannelBuf<M>>>,
}

struct ChannelBuf<M: Send> {
    queue: Vec<M>,
    waker: Option<Waker>, // None if closed
}

impl<M: Send> Channel<M> {
    /// Create a new channel that directs messages to an actor using
    /// the given `Fwd` instance.  Returns the channel and a
    /// channel-guard.  The [`Channel`] may be cloned as many times as
    /// necessary and sent to other threads.  The [`ChannelGuard`]
    /// should be kept in the actor that receives the messages.
    ///
    /// [`ChannelGuard`]: ../sync/struct.ChannelGuard.html
    /// [`Channel`]: ../sync/struct.Channel.html
    pub fn new(core: &mut Core, fwd: Fwd<M>) -> (Self, ChannelGuard) {
        let arc = Arc::new(Mutex::new(ChannelBuf {
            queue: Vec::new(),
            waker: None,
        }));
        let arc1 = arc.clone();
        let waker = core.waker(move |_, _| {
            let mut guard = arc1.lock().expect("Stakker channel lock poisoned");
            let vec = std::mem::take(&mut guard.queue);
            let is_open = guard.waker.is_some();
            drop(guard);
            if is_open {
                for msg in vec {
                    fwd.fwd(msg);
                }
            }
        });
        arc.lock().unwrap().waker = Some(waker);
        let this = Self { arc };
        let guard = ChannelGuard(Box::new(this.clone()));
        (this, guard)
    }

    /// Send a message to destination actor in the `Stakker` thread if
    /// the channel is open, and return `true`.  If the channel has
    /// been closed, returns `false`.
    pub fn send(&self, msg: M) -> bool {
        let mut guard = self.arc.lock().expect("Stakker channel lock poisoned");
        if let Some(ref waker) = guard.waker {
            if guard.queue.is_empty() {
                waker.wake();
            }
            guard.queue.push(msg);
            true
        } else {
            false
        }
    }

    /// Tests whether the channel has been closed.
    pub fn is_closed(&self) -> bool {
        let guard = self.arc.lock().expect("Stakker channel lock poisoned");
        guard.waker.is_none()
    }
}

impl<M: Send> Clone for Channel<M> {
    /// Get another reference to the same channel
    fn clone(&self) -> Self {
        Self {
            arc: self.arc.clone(),
        }
    }
}

trait Closable {
    fn close(&self);
}

impl<M: 'static + Send> Closable for Channel<M> {
    fn close(&self) {
        let mut guard = self.arc.lock().expect("Stakker channel lock poisoned");
        guard.waker.take();
        guard.queue = Vec::new();
    }
}

/// Guard for a channel
///
/// When this is dropped, the associated [`Channel`] is closed and any
/// pending messages are dropped.  This should be kept in the actor
/// that receives messages so that any failure or other termination of
/// the actor results in correct cleanup.
///
/// [`Channel`]: ../sync/struct.Channel.html
pub struct ChannelGuard(Box<dyn Closable>);

impl Drop for ChannelGuard {
    /// Close the channel and drop any pending messages
    fn drop(&mut self) {
        self.0.close();
    }
}
