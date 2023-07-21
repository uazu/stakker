//! Cross-thread support
//!
//! # `Waker` primitive
//!
//! [`Waker`] is the primitive that allows all the other cross-thread
//! types to integrate with **Stakker**.  It allows a wakeup to be
//! scheduled in the **Stakker** thread.  That wakeup handler (a
//! [`Fwd`]) can then check the channel or mutex or whatever and
//! perform whatever actions are necessary.
//!
//! # Unbounded channels
//!
//! - For receiving messages into a **Stakker** thread via an
//! unbounded queue, use [`Channel`].
//!
//! - For sending messages to another thread via an unbounded channel,
//! use whatever unbounded channel is supported by the runtime of that
//! thread, for example `tokio::sync::mpsc::unbounded_channel()`.
//!
//! # Bounded channels
//!
//! There is no high-level support for bounded channels yet.  Raise an
//! issue if this is required.
//!
//! - For a bounded receiving channel, it would be straightforward to
//! wrap the native bounded channel of the sending runtime with some
//! code to use a [`Waker`] to wake **Stakker** to receive the
//! messages.
//!
//! - For a bounded sending channel, it requires that the other
//! runtime supports some means of calling a [`Waker`] when the
//! channel has free space.  Possibly this could also be done with a
//! wrapper.
//!
//! - Alternatively bounded channels could be layered on top of
//! unbounded channels by doing some form of token exchange (like
//! token ring), i.e. a sender uses up one of its logical tokens to
//! send, and the logical token has to be returned before it can be
//! used to send again.  This just means keeping a count of tokens and
//! having certain messages logically pass a token one way or the
//! other.  A limited number of tokens would be introduced into the
//! system on initialisation.
//!
//! # Worker threads
//!
//! [`PipedThread`] provides a simple and safe way to start a thread
//! to offload CPU-intensive or blocking tasks.  This thread may in
//! turn use `rayon` or whatever to parallelize work if necessary.
//!
//! [`Channel`]: ../sync/struct.Channel.html
//! [`Fwd`]: ../struct.Fwd.html
//! [`PipedThread`]: ../sync/struct.PipedThread.html
//! [`Waker`]: ../sync/struct.Waker.html

mod channel;
pub(crate) mod thread;
pub(crate) mod waker;
pub use channel::{Channel, ChannelGuard};
pub use thread::{PipedLink, PipedThread};
pub use waker::Waker;
