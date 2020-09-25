//! # Inter-thread waking
//!
//! This converts the single `PollWaker` that we get from the I/O
//! poller into many thousands, arranged in a heirarchical bitmap to
//! minimise lookups.  A bitmap is used so that some code wishing to
//! wake up its handler in the **Stakker** thread doesn't need to
//! remember whether it already has a wake-up request outstanding.
//! The bitmap allows the operation to be ignored quickly if it is
//! already outstanding.
//!
//! ## Scaling
//!
//! Each bitmap has two layers, giving 4096 bits (64*64) on 64-bit
//! platforms.  Above this is an additional summary bitmap of 64 bits.
//! Each bit in the summary maps to a `Vec` of bitmaps.  For the first
//! 262144 wakers (64*4096), there are just 0 or 1 bitmaps in each
//! `Vec`.  If the caller goes beyond that number then 2 or more
//! bitmaps may appear in each `Vec`.  So this scales quite gradually.
//! Realistically since each waker will be associated with a channel
//! or mutex connecting to some code in another thread, it seems
//! unlikely we'd even reach 4096, but who knows.
//!
//! So costs of a single "wake" are minimum 1 atomic operation, and
//! maximum 3 atomic operations and the poll-wake.  Costs of
//! collecting that "wake" are 3 atomic operations up to 262144
//! wakers, then 1 additional atomic operation for each 262144 beyond
//! that, although most of the atomic operations will be shared with
//! collecting any other wakes that also happened recently.
//!
//! Also, the more heavily loaded the main thread becomes, the less
//! often it will collect the wakes, accumulating more each time,
//! which means that as the load goes up, the waking mechanism becomes
//! more efficient.
//!
//! (On 32-bit, the figures are 1024 per bitmap, and 32768 to fill the
//! first element of each `Vec`.)
//!
//! ## Drop handling
//!
//! The drop of a `Waker` is handled by pushing a notification onto a
//! mutex-protected list.  There is one waker slot in each bitmap
//! reserved for waking up the drop handler in the main thread.  This
//! overhead is assumed to be acceptable because drops should be very
//! much less frequent than wakes.  For example a channel/waker pair
//! would be created, handle many messages and then finally be
//! dropped.

// If inter-thread is disabled, we just switch in a dummy
// implementation.  This results in a lot of dead code for the
// compiler to optimise out, so ignore those warnings.
#![cfg_attr(not(feature = "inter-thread"), allow(dead_code))]
#![cfg_attr(not(feature = "inter-thread"), allow(unused_variables))]

#[cfg(feature = "inter-thread")]
use {slab::Slab, std::convert::TryFrom, std::mem};

use crate::Stakker;
use std::ops::{Index, IndexMut};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

type BoxFnMutCB = Box<dyn FnMut(&mut Stakker, bool) + 'static>;

#[cfg(feature = "inter-thread")]
pub(crate) struct WakeHandlers {
    pollwaker: Arc<PollWaker>,
    slab: Slab<Option<BoxFnMutCB>>,
    bitmaps: Array<Vec<Arc<BitMap>>>,
}

#[cfg(feature = "inter-thread")]
impl WakeHandlers {
    pub fn new(waker: Box<dyn Fn() + Send + Sync>) -> Self {
        Self {
            pollwaker: Arc::new(PollWaker::new(waker)),
            slab: Slab::new(),
            bitmaps: Default::default(),
        }
    }

    /// Get a list of all the wake handlers that need to run
    pub fn wake_list(&mut self) -> Vec<u32> {
        let mut rv = Vec::new();
        self.pollwaker.summary.drain(|slot| {
            // If there is nothing in the slab for `bit`, ignore it.
            // Maybe a `wake` and a `del` occurred around the same
            // time.  This also means that maybe we `del` and
            // reallocate the slot and then get a `wake` for it.  But
            // a spurious wake is acceptable.
            for bm in &self.bitmaps[slot] {
                bm.drain(|bit| rv.push(bit));
            }
        });
        rv
    }

    // Get Waker `drop_list`
    pub fn drop_list(&mut self) -> Vec<u32> {
        mem::replace(&mut *self.pollwaker.drop_list.lock().unwrap(), Vec::new())
    }

    /// Borrows a wake handler from its slot in the slab, leaving a
    /// `None` there.  Panics if a waker has been borrowed twice.
    /// Returns `None` if the slot is now unoccupied, i.e. the handler
    /// was deleted.
    pub fn handler_borrow(&mut self, bit: u32) -> Option<BoxFnMutCB> {
        match self.slab.get_mut(bit as usize) {
            None => None,
            Some(slot) => {
                if let Some(cb) = slot.take() {
                    Some(cb)
                } else {
                    panic!("Wake handler has been borrowed from its slot twice");
                }
            }
        }
    }

    /// Restores a wake handler back into its slot in the slab.  If the
    /// handler slot has been deleted, or is currently occupied (not
    /// None), then panics as it means that something has gone badly
    /// wrong.  It should not be possible for a wake handler to delete
    /// itself.  A wake handler is only deleted when the [`Waker`] is
    /// dropped.  A `drop_list` wake handler won't delete itself.
    ///
    /// [`Waker`]: struct.Waker.html
    pub fn handler_restore(&mut self, bit: u32, cb: BoxFnMutCB) {
        if mem::replace(
            self.slab
                .get_mut(bit as usize)
                .expect("WakeHandlers slot unexpectedly deleted during handler call"),
            Some(cb),
        )
        .is_some()
        {
            panic!(
                "WakeHandlers slot unexpected occupied by another handler during wake handler call"
            );
        }
    }

    /// Add a wake handler and return a Waker to pass to the
    /// thread to use to trigger it.  A wake handler must be able to
    /// handle spurious wakes, since there is a small chance of those
    /// happening from time to time.
    pub fn add(&mut self, cb: impl FnMut(&mut Stakker, bool) + 'static) -> Waker {
        let mut bit = u32::try_from(self.slab.insert(Some(Box::new(cb))))
            .expect("Exceeded 2^32 Waker instances");
        let mut base = bit & !(BitMap::SIZE - 1);
        while base == bit {
            // Bit zero in any bitmap is used to signal a drop, so
            // swap it and add another slab entry
            let somecb = mem::replace(
                self.slab.get_mut(bit as usize).unwrap(),
                Some(Box::new(|s, _| s.process_waker_drops())),
            );
            let bit2 =
                u32::try_from(self.slab.insert(somecb)).expect("Exceeded 2^32 Waker instances");
            bit = bit2;
            base = bit & !(BitMap::SIZE - 1);
        }
        let vec_index = bit >> (USIZE_INDEX_BITS + BitMap::SIZE_BITS);
        let waker_slot = (bit >> BitMap::SIZE_BITS) & (USIZE_BITS - 1);
        let vec = &mut self.bitmaps[waker_slot];
        while vec.len() <= vec_index as usize {
            vec.push(Arc::new(BitMap::new(
                base,
                waker_slot,
                self.pollwaker.clone(),
            )));
        }
        Waker {
            bit,
            bitmap: vec[vec_index as usize].clone(),
        }
    }

    /// Delete a handler, and return it if it was found.  The returned
    /// handler should be called with a deleted argument of 'true'.
    pub fn del(&mut self, bit: u32) -> Option<BoxFnMutCB> {
        if 0 != (bit & (BitMap::SIZE - 1)) && self.slab.contains(bit as usize) {
            return self.slab.remove(bit as usize);
        }
        None
    }

    /// Check the number of stored handlers (for testing)
    #[cfg(test)]
    pub(crate) fn handler_count(&self) -> usize {
        self.slab.len()
    }
}

// Dummy implementation used if feature "inter-thread" is disabled.
// Allows `WakeHandlers` instance to be created, but panics if it is
// actually used to create a Waker.
#[cfg(not(feature = "inter-thread"))]
pub(crate) struct WakeHandlers;

#[cfg(not(feature = "inter-thread"))]
impl WakeHandlers {
    pub fn new(waker: Box<dyn Fn() + Send + Sync>) -> Self {
        Self
    }
    pub fn wake_list(&mut self) -> Vec<u32> {
        Vec::new()
    }
    pub fn drop_list(&mut self) -> Vec<u32> {
        Vec::new()
    }
    pub fn handler_borrow(&mut self, bit: u32) -> Option<BoxFnMutCB> {
        None
    }
    pub fn handler_restore(&mut self, bit: u32, cb: BoxFnMutCB) {}
    pub fn add(&mut self, cb: impl FnMut(&mut Stakker, bool) + 'static) -> Waker {
        panic!("Enable feature 'inter-thread' to create Waker instances");
    }
    pub fn del(&mut self, bit: u32) -> Option<BoxFnMutCB> {
        None
    }
}

/// Used to schedule a wake handler to be called in the main thread
///
/// Obtain an instance using [`Core::waker`], and pass it to the
/// thread that needs to wake the main thread.  This primitive would
/// normally be used in conjunction with a channel or some other
/// shared list or shared state, to alert a wake handler in the main
/// thread that there is a new message that needs attention, or that
/// some other change has occurred.
///
/// When this is dropped, either due to being manually dropped or due
/// to a panic, a final call to the wake handler in the main thread is
/// scheduled with the `deleted` argument set to true, and then the
/// wake handler is removed.
///
/// Note that there is no mechanism for back-pressure or cancellation
/// here, i.e. no way to inform a [`Waker`] that whatever the wake
/// handler notifies has gone away or needs to pause.  This should all
/// be handled via whatever channel or shared state is used to
/// communicate data from the other thread to the main thread.
/// Usually cancellation would need to be flagged in a drop handler in
/// the main thread, e.g. in an actor's drop handler.  That way the
/// other thread can recognise the situation and terminate, ensuring
/// that things clean up nicely in case of failure of the actor.
///
/// [`Core::waker`]: struct.Core.html#method.waker
/// [`Waker`]: struct.Waker.html
pub struct Waker {
    bit: u32,
    bitmap: Arc<BitMap>,
}

impl Waker {
    /// Schedule a call to the corresponding wake handler in the main
    /// thread, if it is not already scheduled to be called.  If it is
    /// already scheduled (or a nearby handler is already scheduled),
    /// this requires just one `SeqCst` atomic operation.  In the
    /// worst case it requires 3 atomic operations, and a wake-up call
    /// to the I/O poller.
    ///
    /// This is handled using a heirarchical tree of `usize` bitmaps
    /// containing wake bits, one leaf wake bit for each [`Waker`].
    /// At the top of the tree is the poll-waker.  The wake-up only
    /// needs to ascend until it reaches a level which has already
    /// been woken.  In the main thread, in response to the poll-wake
    /// the heirarchy is descended only on those branches where there
    /// are wake bits set.  The wake bits are cleared and the
    /// corresponding wake handlers are called.  The longer the
    /// poll-wake process takes, the more wakes will be accumulated in
    /// the bitmap, making the next wake more efficient, so this
    /// scales well.
    ///
    /// Note that this operation has to be as cheap as possible
    /// because with a channel for example, you'll need to call it
    /// every time you add an item.  Trying to detect when you add the
    /// first item to an empty queue is unreliable due to races,
    /// unless the channel has specific support for that, some kind of
    /// `send_and_was_empty()` call.
    ///
    /// [`Waker`]: struct.Waker.html
    pub fn wake(&self) {
        self.bitmap.set(self.bit);
    }
}

impl Drop for Waker {
    fn drop(&mut self) {
        // Ignore poisoning here, to not panic in panic handler
        if let Ok(mut guard) = self.bitmap.pollwaker.drop_list.lock() {
            guard.push(self.bit);
            self.bitmap.set(self.bitmap.base_index);
        }
    }
}

const LOG2_TABLE: [u32; 9] = [0, 0, 1, 0, 2, 0, 0, 0, 3];
const USIZE_BYTES: u32 = std::mem::size_of::<usize>() as u32; // 4 or 8
const USIZE_BITS: u32 = 8 * USIZE_BYTES; // 32 or 64
const USIZE_INDEX_BITS: u32 = 3 + LOG2_TABLE[USIZE_BYTES as usize]; // 5 or 6

// Regarding Ordering::SeqCst, there are two BitMap operations: `set`
// sets the bit at the bottom of the heirarchy and works up, and
// `drain` clears the bits at the top of the heirarchy and works down.
// If these operations occur at the same time, they must cross over in
// an ordered way, hence SeqCst.  If they do occur at the same time we
// get a spurious wake, but that is harmless.  If they crossed over in
// an unordered way we could get a situation where a bit is set lower
// down but not in the higher level summaries, which means that that
// wakeup is stuck indefinitely.
const ORDERING: Ordering = Ordering::SeqCst;

// An array of 32/64 `I` items.  Unfortunately until const generics
// appear in stable, it's necessary to use this workaround to get a
// Default implementation for a 64-entry array.  It should hopefully
// all compile down to the natural implementation.
//
// TODO: When const generics land, get rid of this workaround
#[derive(Default)]
struct Array<I>([[I; 8]; USIZE_BYTES as usize]);
impl<I> Index<u32> for Array<I> {
    type Output = I;
    fn index(&self, ii: u32) -> &Self::Output {
        &self.0[(ii >> 3) as usize][(ii & 7) as usize]
    }
}
impl<I> IndexMut<u32> for Array<I> {
    fn index_mut(&mut self, ii: u32) -> &mut Self::Output {
        &mut self.0[(ii >> 3) as usize][(ii & 7) as usize]
    }
}

// A leaf in the heirarchy, providing 32/64 bits.  Uses `usize` as
// that is probably the memory word size.  Also `AtomicUsize` is
// reported to have best platform support.
#[derive(Default)]
struct Leaf {
    bitmap: AtomicUsize,
}

impl Leaf {
    #[inline]
    fn set(&self, bit: u32) -> bool {
        0 == self.bitmap.fetch_or(1 << bit, ORDERING)
    }
    #[inline]
    fn drain(&self, mut cb: impl FnMut(u32)) {
        let mut bits = self.bitmap.swap(0, ORDERING);
        while bits != 0 {
            let bit = bits.trailing_zeros();
            bits &= !(1 << bit);
            cb(bit);
        }
    }
}

#[derive(Default)]
struct Layer<S: Default> {
    summary: Leaf, // A bit is set here if the corresponding child is non-zero
    child: Array<S>,
}

// Code to produce a heirarchy of three levels is a trivial adaption
// of this code (i.e. Layer<Layer<Leaf>>), but I don't think it's
// needed.

// This bitmap is a heirarchy of two levels.  This holds 2^10 or 2^12
// bits (1024 or 4096), taking up 132 or 520 bytes of memory (plus
// overheads).
struct BitMap {
    tree: Layer<Leaf>,
    base_index: u32,
    wake_index: u32,
    pollwaker: Arc<PollWaker>,
}

impl BitMap {
    const SIZE_BITS: u32 = USIZE_INDEX_BITS * 2;
    const SIZE: u32 = 1 << Self::SIZE_BITS;

    pub fn new(base_index: u32, wake_index: u32, pollwaker: Arc<PollWaker>) -> Self {
        assert_eq!(1 << USIZE_INDEX_BITS, USIZE_BITS);
        Self {
            tree: Default::default(),
            base_index,
            wake_index,
            pollwaker,
        }
    }

    #[inline]
    fn set(&self, bit: u32) {
        let bit = bit - self.base_index;
        let a = bit >> USIZE_INDEX_BITS;
        let b = bit & (USIZE_BITS - 1);
        if self.tree.child[a].set(b)
            && self.tree.summary.set(a)
            && self.pollwaker.summary.set(self.wake_index)
        {
            (self.pollwaker.waker)();
        }
    }

    /// Read out all the set bits, clearing them in the process.
    #[inline]
    fn drain(&self, mut cb: impl FnMut(u32)) {
        self.tree.summary.drain(|a| {
            self.tree.child[a].drain(|b| {
                cb((a << USIZE_INDEX_BITS) + b + self.base_index);
            });
        });
    }
}

/// Interface to the I/O poller-provided waker of the main thread.
/// Provides 32/64 slots for waking.  These slots might get used by
/// multiple BitMap instances if we have very many of them.
struct PollWaker {
    // Bitmap showing which slots need processing
    summary: Leaf,

    // Waker provided by I/O poller
    waker: Box<dyn Fn() + Send + Sync + 'static>,

    // List of handlers that need to be dropped
    drop_list: Mutex<Vec<u32>>,
}

impl PollWaker {
    pub fn new(waker: Box<dyn Fn() + Send + Sync>) -> Self {
        Self {
            summary: Default::default(),
            waker,
            drop_list: Mutex::new(Vec::new()),
        }
    }
}
