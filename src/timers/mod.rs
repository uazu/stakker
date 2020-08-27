use crate::queue::FnOnceQueue;
use std::cmp::Ordering;
use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::mem;
use std::time::{Duration, Instant};

// TODO: Switch to N-ary Heap for storing (WrapTime, slot-index),
// e.g. Octonary heap, and use slots for normal timers as well as
// min/max.  Octonary heap because that means each level is a cache
// line.  So to pick the next, you have to scan 8 items, but they're
// all in one cache line so that's okay.  So reorganization of the
// tree is much cheaper.  Code for a heap should be a lot cheaper than
// BTreeMap, which generates a huge amount of assembler.

/// Timer key for a fixed timer
///
/// Returned by [`Core::timer_add`] or [`Core::after`].  It can be
/// used to delete a timer with [`Core::timer_del`].  It is plain
/// `Copy` data, 8 bytes long.  Note that the key should only be used
/// on this same **Stakker** instance.  If it is used on another then
/// it might cause a panic.
///
/// [`Core::after`]: struct.Core.html#method.after
/// [`Core::timer_add`]: struct.Core.html#method.timer_add
/// [`Core::timer_del`]: struct.Core.html#method.timer_del
#[derive(Copy, Clone, Eq, PartialEq, Default, Debug)]
pub struct FixedTimerKey {
    slot: u32,
    // Generation for slot < 0x8000_0000, or WrapTime otherwise
    gen_or_time: u32,
}

/// Timer key for a Max timer
///
/// Used by `timer_max_*` methods in [`Core`].  It is used to delete a
/// timer or change its expiry time.  It is plain `Copy` data, 8 bytes
/// long.  Note that the key should only be used on this same
/// **Stakker** instance.  If it is used on another then it might
/// cause a panic.
///
/// This sets a timer at the initial timeout time, then just records
/// the largest of the timeout values provided until that original
/// timer expires, at which point it sets a new timer.  So this
/// naturally absorbs a lot of changes without having to delete any
/// timers.  A typical use might be to take some action in a gap in
/// activity, for example to do an expensive background check in a gap
/// in the user's typing into a UI field.  To implement this, the
/// timer expiry time is set to now+1 (for example) by each keypress.
///
/// [`Core`]: struct.Core.html
#[derive(Copy, Clone, Eq, PartialEq, Default, Debug)]
pub struct MaxTimerKey {
    slot: u32, // Slot number
    gen: u32,  // Generation
}

/// Timer key for a Min timer
///
/// Used by `timer_min_*` methods in [`Core`].  It is used to delete a
/// timer or change its expiry time.  It is plain `Copy` data, 8 bytes
/// long.  Note that the key should only be used on this same
/// **Stakker** instance.  If it is used on another then it might
/// cause a panic.
///
/// This is intended for use where the end-time is an estimate and
/// where that estimate is progressively improved over time.  The
/// end-time is approached asymptotically to allow wiggle-room
/// without having to update the underlying timer too many times,
/// e.g. a 60s timer uses 5 timers in sequence, adjusting as it
/// goes according to the latest estimate: ~15s before, ~4s
/// before, ~1s before, ~0.125s before, end-time itself.  So for
/// example in the first 45s, the timer can accomodate up to 15s
/// of change in the end-time without having to delete a timer.
///
/// [`Core`]: struct.Core.html
#[derive(Copy, Clone, Eq, PartialEq, Default, Debug)]
pub struct MinTimerKey {
    slot: u32, // Slot number
    gen: u32,  // Generation
}

// `Instant` converted cheaply into a `u64` by reducing the accuracy.
// This is `(secs << 16) + (nanos >> 14)`.  Nanos take values 0..61036
// in the low 16 bits, giving resolution of ~0.016ms.  This means
// quick conversion but adding/subtracting is not straightforward.
// Range is 8 million years, but more important is that low 32 bits
// cover 18 hours.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct Time(u64);

impl Time {
    // Rounding up, used for timer expiry times, to make sure they
    // don't expire early
    pub fn new_ceil(inst: Instant, t0: Instant) -> Self {
        let dur = inst.saturating_duration_since(t0);
        Self((dur.as_secs() << 16) | u64::from((dur.subsec_nanos() + (1 << 14) - 1) >> 14))
    }

    // Rounding down, used for current time, to make sure we don't
    // accidentally expire something before its actual time.
    pub fn new_floor(inst: Instant, t0: Instant) -> Self {
        let dur = inst.saturating_duration_since(t0);
        Self((dur.as_secs() << 16) | u64::from(dur.subsec_nanos() >> 14))
    }

    pub fn add_secs(self, secs: u32) -> Time {
        Self(self.0 + (u64::from(secs) << 16))
    }

    pub fn instant(self, t0: Instant) -> Instant {
        t0 + Duration::new(self.0 >> 16, ((self.0 & 0xFFFF) as u32) << 14)
    }

    pub fn wt(self) -> WrapTime {
        WrapTime(self.0 as u32)
    }
}

// Cyclic (wrapping) time representation.  This can represent ~18
// hours cyclic range with resolution of ~0.016ms.  The difference
// between any two times is taken to be the smallest wrapped distance
// between them.  So to maintain a total order, no two timers can be
// more than ~9 hours apart.  So longer timers use a `VarSlot` and set
// the longest possible time there, and the actual time in the
// `VarSlot`.  They will be reinserted into the list about every 9
// hours until they expire.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct WrapTime(u32);

impl WrapTime {
    // Convert back to a Time, given the base value
    fn time(self, base: Time) -> Time {
        let val = (base.0 & !0xFFFF_FFFF) | u64::from(self.0);
        if val < base.0 {
            Time(val + 0x1_0000_0000)
        } else {
            Time(val)
        }
    }
}

impl Ord for WrapTime {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.0.wrapping_sub(other.0) as i32).cmp(&0)
    }
}

impl PartialOrd for WrapTime {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// Internal timer key for queue
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct TimerKey {
    time: WrapTime,
    // Has a `VarSlot` for slot < 0x8000_0000, else this is just a
    // number to make the key unique
    slot: u32,
}

impl TimerKey {
    fn new(time: WrapTime, slot: u32) -> Self {
        assert_eq!(8, std::mem::size_of::<TimerKey>());
        Self { time, slot }
    }
}

impl Ord for TimerKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time
            .cmp(&other.time)
            .then_with(|| self.slot.cmp(&other.slot))
    }
}

impl PartialOrd for TimerKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// Variable-expiry timer: either min or max timer
struct VarTimer {
    // Target expiry time
    expiry: Time,
    // Current timer expiry
    curr: Time,
}

enum VarItem {
    Max(VarTimer),
    Min(VarTimer),
    Free(Option<u32>), // Free slot with link to next
}

struct VarSlot {
    gen: u32, // Generation, incremented on delete
    item: VarItem,
}

pub(crate) type BoxedFnOnce<S> = Box<dyn FnOnce(&mut S) + 'static>;

// Timers
pub(crate) struct Timers<S> {
    // Base time against which all other times are measured
    t0: Instant,
    // The last `now` value we were given, relative to `t0`.  All
    // times in the `queue` will be within ~9 hours of this time.
    now: Time,
    // Stores all timers waiting to run, in order
    queue: BTreeMap<TimerKey, BoxedFnOnce<S>>,
    // Fixed timers don't use a slot, but max/min timers do
    var: Vec<VarSlot>,
    // First free slot in `var`, or None
    var_free: Option<u32>,
    // Sequential number used for `slot` values >= 0x8000_0000
    seq: u32,
}

impl<S: 'static> Timers<S> {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            t0: now,
            now: Time(0),
            queue: BTreeMap::new(),
            var: Vec::new(),
            var_free: None,
            seq: 0,
        }
    }

    /// Get the time that the next timer will expire, if there is one
    pub(crate) fn next_expiry(&self) -> Option<Instant> {
        self.queue
            .iter()
            .next()
            .map(|(tk, _)| tk.time.time(self.now).instant(self.t0))
    }

    /// Advance 'now' to a new value, expiring timers to the provided
    /// queue
    pub(crate) fn advance(&mut self, now: Instant, queue: &mut FnOnceQueue<S>) {
        let target_now = Time::new_floor(now, self.t0);
        while self.now < target_now {
            // Advance in steps of max 0x7FFF seconds to avoid
            // skipping timers in the queue
            let now = self.now.add_secs(0x7FFF).min(target_now);
            let key = TimerKey::new(WrapTime(now.wt().0 + 1), 0);
            let rest = self.queue.split_off(&key);
            let head = mem::replace(&mut self.queue, rest);
            self.now = now;

            for (key, bfn) in head {
                if key.slot >= 0x8000_0000 {
                    // Fixed timer
                    queue.push_box(bfn);
                    continue;
                }
                let slot = &mut self.var[key.slot as usize];
                match slot.item {
                    VarItem::Max(ref mut vt) => {
                        if vt.expiry <= target_now {
                            queue.push_box(bfn);
                            self.free_slot(key.slot);
                        } else {
                            // Set time to current expiry time
                            vt.curr = vt.expiry.min(self.now.add_secs(0x7FFF));
                            self.queue
                                .insert(TimerKey::new(vt.curr.wt(), key.slot), bfn);
                        }
                    }
                    VarItem::Min(ref mut vt) => {
                        if vt.expiry <= target_now {
                            queue.push_box(bfn);
                            self.free_slot(key.slot);
                        } else {
                            // Set timer to 75% current expiry time
                            vt.curr =
                                rounded_75point(self.now, vt.expiry.min(self.now.add_secs(0x7FFF)));
                            self.queue
                                .insert(TimerKey::new(vt.curr.wt(), key.slot), bfn);
                        }
                    }
                    VarItem::Free(_) => panic!("TimerKey points to a free slot"),
                }
            }
        }
    }

    fn alloc_slot(&mut self, item: VarItem) -> (u32, u32) {
        if let Some(i) = self.var_free {
            let slot = &mut self.var[i as usize];
            match slot.item {
                VarItem::Free(next) => self.var_free = next,
                _ => panic!("Timers: var_free pointed to slot that wasn't free"),
            };
            slot.item = item;
            (i, slot.gen)
        } else {
            let i = self.var.len();
            if i >= 0x8000_0000 {
                panic!("Exceeded 2^31 variable timers at the same time");
            }
            // Start at gen==1 so that `Default` on keys doesn't match
            // anything
            self.var.push(VarSlot { gen: 1, item });
            (i as u32, 1)
        }
    }

    fn free_slot(&mut self, i: u32) {
        let slot = &mut self.var[i as usize];
        slot.gen = slot.gen.wrapping_add(1).max(1); // Avoid gen==0, reserved for Default
        if let VarItem::Free(_) = slot.item {
            panic!("Timers: deleting slot that was already free");
        }
        slot.item = VarItem::Free(self.var_free);
        self.var_free = Some(i);
    }

    // Check slots in use, for tests
    #[cfg(test)]
    fn slots_used(&self) -> usize {
        let mut rv = self.var.len();
        let mut free = self.var_free;
        while let Some(curr) = free {
            rv -= 1;
            match self.var[curr as usize].item {
                VarItem::Free(next) => free = next,
                _ => panic!("Free slot is not free"),
            }
        }
        rv
    }

    // Add a fixed timer.  A fixed timer can only expire or be
    // deleted.
    pub(crate) fn add(&mut self, expiry_time: Instant, bfn: BoxedFnOnce<S>) -> FixedTimerKey {
        let expiry = Time::new_ceil(expiry_time, self.t0).max(self.now);
        if expiry >= self.now.add_secs(0x7FFF) {
            // Add it as a var-timer, so it will keep rescheduling
            // itself until it reaches the target time
            let mk = self.add_max(expiry_time, bfn);
            return FixedTimerKey {
                slot: mk.slot,
                gen_or_time: mk.gen,
            };
        }

        // Collision should be very unlikely in actual use, since we
        // have 2^31 unique values per 15us instant, and we cycle
        // constantly, so the next caller will get a different unique
        // value.  Almost always the first attempt will succeed.
        // Otherwise we try all the 2^31 slots for this instant, then
        // try the next instant and so on.  This must succeed, because
        // the address space is not big enough to contain 2^63 active
        // timers, so there will always be a slot free somewhere.
        let mut wt = expiry.wt();
        loop {
            for _ in 0..0x8000_0000u32 {
                self.seq = self.seq.wrapping_add(1);
                let slot = self.seq | 0x8000_0000;
                if let Entry::Vacant(ent) = self.queue.entry(TimerKey::new(wt, slot)) {
                    ent.insert(bfn);
                    return FixedTimerKey {
                        slot,
                        gen_or_time: wt.0,
                    };
                }
            }
            wt.0 = wt.0.wrapping_add(1);
        }
    }

    // Delete a fixed timer.  Returns: true: success, false: timer no
    // longer exists (i.e. it expired or was deleted)
    pub(crate) fn del(&mut self, fk: FixedTimerKey) -> bool {
        if fk.slot < 0x8000_0000 {
            self.del_max(MaxTimerKey {
                slot: fk.slot,
                gen: fk.gen_or_time,
            })
        } else {
            self.queue
                .remove(&TimerKey::new(WrapTime(fk.gen_or_time), fk.slot))
                .is_some()
        }
    }

    // Add a Max timer, which expires at the greatest (latest) expiry
    // time it has been given.  A Max timer allows its expiry time to
    // be modified efficiently after creation, without having to
    // insert or delete timers from the queue, so the `mod_max` method
    // can be called frequently.
    //
    // When the expiry time is changed to be further in the future,
    // the new value is stored but the old timer remains active in the
    // queue.  A new timer is set only when the old timer expires.
    pub(crate) fn add_max(&mut self, expiry_time: Instant, bfn: BoxedFnOnce<S>) -> MaxTimerKey {
        let expiry = Time::new_ceil(expiry_time, self.t0);
        let curr = expiry.max(self.now).min(self.now.add_secs(0x7FFF));
        let (slot, gen) = self.alloc_slot(VarItem::Max(VarTimer { expiry, curr }));
        self.queue.insert(TimerKey::new(curr.wt(), slot), bfn);
        MaxTimerKey { slot, gen }
    }

    /// Modify a Max timer.  This is a quick operation.  Returns:
    /// true: success, false: timer no longer exists (i.e. it expired
    /// or was deleted or never existed)
    pub(crate) fn mod_max(&mut self, mk: MaxTimerKey, expiry_time: Instant) -> bool {
        if let Some(slot) = self.var.get_mut(mk.slot as usize) {
            if slot.gen == mk.gen {
                if let VarItem::Max(ref mut vt) = slot.item {
                    let expiry = Time::new_ceil(expiry_time, self.t0);
                    vt.expiry = vt.expiry.max(expiry);
                    return true;
                }
            }
        }
        false
    }

    /// Delete a Max timer.  Returns: true: success, false: timer no
    /// longer exists (i.e. it expired or was deleted)
    pub(crate) fn del_max(&mut self, mk: MaxTimerKey) -> bool {
        if let Some(slot) = self.var.get_mut(mk.slot as usize) {
            if slot.gen == mk.gen {
                if let VarItem::Max(ref mut vt) = slot.item {
                    self.queue.remove(&TimerKey::new(vt.curr.wt(), mk.slot));
                    self.free_slot(mk.slot);
                    return true;
                }
            }
        }
        false
    }

    // Check whether a max timer is still active
    pub(crate) fn max_is_active(&self, mk: MaxTimerKey) -> bool {
        if let Some(slot) = self.var.get(mk.slot as usize) {
            slot.gen == mk.gen
        } else {
            false
        }
    }

    // Add a Min timer, which expires at the smallest (earliest)
    // expiry time it has been given.  A Min timer allows its expiry
    // time to be modified efficiently after creation, without having
    // to insert or delete timers from the queue most of the time, so
    // the `mod_min` method can be called frequently.
    //
    // This timer attempts to approach the end-time asymptotically, so
    // that if the expiry time is changed to an earlier time, that can
    // be absorbed without having to delete a timer.  For very large
    // changes, a timer will have to be deleted, though, but small
    // changes after that will be absorbed.
    pub(crate) fn add_min(&mut self, expiry_time: Instant, bfn: BoxedFnOnce<S>) -> MinTimerKey {
        let expiry = Time::new_ceil(expiry_time, self.t0);
        let curr = rounded_75point(
            self.now,
            expiry.max(self.now).min(self.now.add_secs(0x7FFF)),
        );
        let (slot, gen) = self.alloc_slot(VarItem::Min(VarTimer { expiry, curr }));
        self.queue.insert(TimerKey::new(curr.wt(), slot), bfn);
        MinTimerKey { slot, gen }
    }

    /// Modify a Min timer.  This is a quick operation.  Returns:
    /// true: success, false: timer no longer exists (i.e. it expired
    /// or was deleted)
    pub(crate) fn mod_min(&mut self, mk: MinTimerKey, expiry_time: Instant) -> bool {
        let expiry = Time::new_ceil(expiry_time, self.t0);
        if let Some(slot) = self.var.get_mut(mk.slot as usize) {
            if slot.gen == mk.gen {
                if let VarItem::Min(ref mut vt) = slot.item {
                    if expiry < vt.expiry {
                        vt.expiry = expiry;
                        if expiry < vt.curr {
                            // Delete old timer, set a new one
                            let bfn = self
                                .queue
                                .remove(&TimerKey::new(vt.curr.wt(), mk.slot))
                                .unwrap();
                            vt.curr =
                                rounded_75point(self.now, expiry.min(self.now.add_secs(0x7FFF)));
                            self.queue.insert(TimerKey::new(vt.curr.wt(), mk.slot), bfn);
                        }
                    }
                    return true;
                }
            }
        }
        false
    }

    /// Delete a Min timer.  Returns: true: success, false: timer no
    /// longer exists (i.e. it expired or was deleted)
    pub(crate) fn del_min(&mut self, mk: MinTimerKey) -> bool {
        if let Some(slot) = self.var.get_mut(mk.slot as usize) {
            if slot.gen == mk.gen {
                if let VarItem::Min(ref vt) = slot.item {
                    self.queue.remove(&TimerKey::new(vt.curr.wt(), mk.slot));
                    self.free_slot(mk.slot);
                    return true;
                }
            }
        }
        false
    }

    // Check whether a min timer is still active
    pub(crate) fn min_is_active(&self, mk: MinTimerKey) -> bool {
        if let Some(slot) = self.var.get(mk.slot as usize) {
            slot.gen == mk.gen
        } else {
            false
        }
    }
}

// Take a 75% point and round up to fixed units to try and get a lot
// of Min timers all expiring and updating at the same time, to be
// more cache-friendly when there are lot of timers running.  Go
// straight to target when we get within 500ms of it.
fn rounded_75point(t0: Time, t1: Time) -> Time {
    // Calc approx 75%-point and gap; worst-case error is
    // (65536-61036)/61036 seconds (== 74ms).
    let p75 = (t0.0 + 3 * t1.0) >> 2;
    let gap = t1.0 - t0.0;
    if gap < 0x8000 {
        //println!("p75 {:08X} gap {:08X}", p75, gap);
        return t1; // Below 500ms approx, go straight to target
    }

    // Round up 75%-point by about 1/8 to 1/16 of gap.  For example
    // gap of 1s (0x10000) gives max rounding of 0x1FFF, which is max
    // 125ms.  Or gap of 0.999s (0xFF??) gives rounding of 0x0FFF,
    // which is max 62ms (this is the smallest rounding value).
    let round = u64::max_value() >> gap.leading_zeros() >> 4;
    let mut rv = ((p75 - 1) | round) + 1;
    if (rv & 0xFFFF) >= 61036 {
        rv = (rv & !0xFFFF) + 0x10000;
    }
    //println!("p75 {:08X} gap {:08X} round {:08X} out {:08X}", p75, gap, round, rv);
    Time(rv)
}

#[cfg(test)]
mod tests;
