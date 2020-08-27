use std::alloc::Layout;
use std::marker::PhantomData;
use std::mem;

#[repr(C)]
struct FatPointer {
    data: *mut (),
    meta: *mut (),
}

/// Queue of `FnOnce(&mut Stakker)` items waiting for execution.
pub struct FnOnceQueue<S: 'static> {
    storage: hvec::HVec,
    phantomdata: PhantomData<S>,
}

impl<S: 'static> FnOnceQueue<S> {
    /// Create an empty queue, with no memory allocated
    pub const fn new() -> Self {
        Self {
            storage: hvec::HVec::new(),
            phantomdata: PhantomData,
        }
    }

    /// Push a `FnOnce` instance onto the queue.  This call will be
    /// inlined and specialised to the particular `FnOnce` being
    /// pushed.
    #[inline]
    pub fn push(&mut self, value: impl FnOnce(&mut S) + 'static) {
        Self::push_aux(&mut self.storage, value);
    }

    #[inline(always)]
    fn push_aux(hv: &mut hvec::HVec, value: impl FnOnce(&mut S) + 'static) {
        let mut item = CallItem::new(value);
        let ctref: &mut dyn CallTrait<S> = &mut item;
        assert_eq!(mem::size_of_val(&ctref), mem::size_of::<FatPointer>());
        let repr = unsafe { mem::transmute_copy::<&mut dyn CallTrait<S>, FatPointer>(&ctref) };
        hv.push(repr.meta, item, Self::expand_storage);
    }

    const INITIAL_ALLOCATION: usize = 1024;

    // Expand the storage for the queue
    //
    // Since this implementation of HVec is intentionally fixed-size,
    // when more space is required, a new bigger HVec is allocated,
    // and a FnOnce added to it which contains the old HVec and
    // executes all the items in that old HVec before continuing with
    // newer items.  So:
    //
    // - No data is copied when the HVec is increased in size (unlike
    //   most Vec-style implementations)
    // - Order of item execution is maintained
    // - When the queue is executed, the old HVec is freed
    //
    // (This code is not on the hot path, so choose to never inline
    // it.  This minimizes code size and encourage inlining of the
    // code that actually *is* on the hot path.)
    #[inline(never)]
    fn expand_storage(hv: &mut hvec::HVec, req: usize) {
        // Mostly this will give double the size of the previous HVec,
        // or INITIAL_ALLOCATION the first time.  However this should
        // adjust for `req` if it is too large to fit like that.
        // (Note that `next_power_of_two` doesn't give next power if
        // it's already a power of two.)
        let push_old = hv.len() != 0;
        let mut req2 = req;
        if push_old {
            req2 += mem::size_of::<(*mut (), FnOnceQueue<()>)>();
        }
        let size = (hv.cap().max(req2) + 1)
            .max(Self::INITIAL_ALLOCATION)
            .next_power_of_two();
        let new = hvec::HVec::with_size(size);
        let old = mem::replace(hv, new);
        if push_old {
            let mut old_queue = Self {
                storage: old,
                phantomdata: PhantomData,
            };
            Self::push_aux(hv, move |s| old_queue.execute(s));
        }
        assert!(hv.cap() - hv.len() >= req);
    }

    /// Push a boxed `FnOnce` instance onto the queue.
    #[inline]
    pub fn push_box(&mut self, value: Box<dyn FnOnce(&mut S) + 'static>) {
        // TODO: Unwrap the Box and copy the contents onto the Vec?
        // Might give some cache advantage (or maybe not).  For now
        // this keeps it boxed and calls into it when it is executed.
        self.push(move |s| value(s));
    }

    /// Test whether the queue is empty
    pub fn is_empty(&self) -> bool {
        self.storage.len() == 0
    }

    // Get storage length, for tests
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.storage.len()
    }

    // Get storage capacity, for tests
    #[cfg(test)]
    pub(crate) fn cap(&self) -> usize {
        self.storage.cap()
    }

    /// Execute all the `FnOnce` instances found on this queue,
    /// passing them the given context object ref.  Leaves the queue
    /// empty, but with the same backing memory still allocated to
    /// aid in cache reuse.
    pub fn execute(&mut self, context: &mut S) {
        self.drain_for_each(|ptr| unsafe { (&mut *ptr).call(context) });
    }

    // This call will 'forget' objects (i.e. not drop them) unless the
    // caller takes care of dropping each one.  That doesn't make the
    // interface unsafe, though
    fn drain_for_each(&mut self, mut apply: impl FnMut(*mut dyn CallTrait<S>)) {
        unsafe {
            let mut it = self.storage.drain();
            while let Some(meta) = it.next_vp() {
                let data = 0x8000_usize as _; // Not null but aligned
                let fake_repr = FatPointer { data, meta };
                let fake_ref = mem::transmute_copy::<FatPointer, &dyn CallTrait<S>>(&fake_repr);
                let data = it.next_unchecked(Layout::for_value(fake_ref));
                let repr = FatPointer { data, meta };
                apply(mem::transmute_copy::<FatPointer, *mut dyn CallTrait<S>>(
                    &repr,
                ));
            }
        }
    }
}

impl<S> Drop for FnOnceQueue<S> {
    fn drop(&mut self) {
        self.drain_for_each(|ptr| unsafe { (&mut *ptr).drop() });
    }
}

impl<S> Default for FnOnceQueue<S> {
    fn default() -> Self {
        Self::new()
    }
}

// Heterogeneous vector.  This was originally based on some example
// code from Simon Sapin published alongside a Rust RFC, but it has
// since been refactored/rewritten so many times there is probably
// none of that original code left now.  In particular, this is now
// fixed size instead of variable size, and the backing memory is
// never reallocated, so alignment is handled directly on the memory
// addresses, rather than on the offsets.  This means that arbitrary
// alignments can be handled without overhead.
mod hvec {
    use std::alloc::{self, Layout};
    use std::marker::PhantomData;
    use std::mem;
    use std::ptr;

    type VP = *mut (); // void pointer

    pub struct HVec {
        ptr: *mut u8,
        len: usize,
        cap: usize,
    }

    /// Align a pointer, being careful not to create any temporary
    /// pointer values that may point past the end of the allocation.
    /// The caller must ensure that aligning to this position doesn't
    /// move past the end of the allocation.
    #[inline]
    unsafe fn align(p: *mut u8, pow2: usize) -> *mut u8 {
        let inc = (pow2 - 1) & !((p as usize).wrapping_sub(1));
        p.add(inc)
    }

    #[inline]
    const fn align_off(off: usize, pow2: usize) -> usize {
        let inc = (pow2 - 1) & !(off.wrapping_sub(1));
        off + inc
    }

    impl HVec {
        /// Create an empty vector with no allocation attached
        pub const fn new() -> Self {
            Self {
                ptr: ptr::null_mut(),
                len: 0,
                cap: 0,
            }
        }

        /// Allocate a new vector with the given storage size.
        pub fn with_size(size: usize) -> Self {
            let layout = Layout::from_size_align(size, mem::align_of::<VP>()).unwrap();
            let ptr = unsafe { alloc::alloc(layout) };
            if ptr.is_null() {
                alloc::handle_alloc_error(layout);
            }
            Self {
                ptr,
                len: 0,
                cap: size,
            }
        }

        /// Return length used in bytes within the queue
        pub fn len(&self) -> usize {
            self.len
        }

        /// Return total capacity of the queue in bytes
        pub fn cap(&self) -> usize {
            self.cap
        }

        /// Push a (VP,T) to the buffer.  If there isn't enough space,
        /// calls `expand` to make space (which does so by swapping in
        /// a new bigger allocation over `self`).
        #[inline]
        pub fn push<T>(&mut self, v1: VP, v2: T, expand: impl FnOnce(&mut Self, usize)) {
            // This is worst-case space requirement.  So using this
            // means that we may waste some space at the end of the
            // buffer in cases of large alignments.  It is done this
            // way to avoid having several calculations and extra code
            // in the hot path.  This is a compile-time calculation as
            // it has no dependency on the current pointer.
            let req = mem::size_of::<VP>(); // VP
            let req = align_off(req, mem::align_of::<T>()); // Alignment to T
            let req = req + mem::size_of::<T>(); // T
            let req = align_off(req, mem::align_of::<VP>()); // Alignment to next VP
            if req > self.cap - self.len {
                expand(self, req);
                #[cfg(debug_assertions)]
                if req > self.cap - self.len {
                    mem::forget(v2);
                    panic!("HVec::push: not enough space after expand");
                }
            }

            // Safe because after the previous checks, we're sure that
            // we have enough space.  Assumes that `ptr+len` is
            // aligned for VP.
            #[allow(clippy::cast_ptr_alignment)]
            unsafe {
                let p = self.ptr.add(self.len);
                debug_assert_eq!(0, (p as usize) % mem::align_of::<VP>());
                (p as *mut VP).write(v1);
                let p = p.add(mem::size_of::<VP>());

                // Align for T and write T
                let p = align(p, mem::align_of::<T>());
                debug_assert_eq!(0, (p as usize) % mem::align_of::<T>());
                (p as *mut T).write(v2);
                let p = p.add(mem::size_of::<T>());

                // Leave 'len' aligned for next VP
                let p = align(p, mem::align_of::<VP>());
                self.len = (p as usize) - (self.ptr as usize);
                debug_assert!(self.len <= self.cap);
            }
        }

        #[inline]
        pub fn drain(&mut self) -> Drain<'_> {
            // If `Drain` is leaked, 'forget' contents of memory also,
            // which is safe.  Drain instance shares lifetime with
            // `&mut self`, which stops operations on `self` whilst
            // drain is in progress.
            let end = unsafe { self.ptr.add(self.len) };
            self.len = 0;
            Drain {
                pos: self.ptr,
                end,
                phantomdata: PhantomData,
            }
        }
    }

    impl Drop for HVec {
        fn drop(&mut self) {
            // We just 'forget' the contents of the queue, if there is
            // any, which is safe.  It's the caller's responsibility
            // to drop the queue contents in its own Drop handler.
            if !self.ptr.is_null() {
                let layout = Layout::from_size_align(self.cap, mem::align_of::<VP>()).unwrap();
                unsafe { alloc::dealloc(self.ptr, layout) };
            }
        }
    }

    pub struct Drain<'a> {
        pos: *mut u8,
        end: *mut u8,
        phantomdata: PhantomData<&'a ()>,
    }

    impl<'a> Drain<'a> {
        #[inline]
        pub unsafe fn next_vp(&mut self) -> Option<VP> {
            if self.pos < self.end {
                let p = self.pos;
                self.pos = p.add(mem::size_of::<VP>());
                debug_assert!(self.pos <= self.end);
                debug_assert_eq!(0, (p as usize) % mem::align_of::<VP>());
                #[allow(clippy::cast_ptr_alignment)]
                Some(*(p as *mut VP))
            } else {
                None
            }
        }

        // This assumes that the item definitely exists on the queue
        #[inline]
        pub unsafe fn next_unchecked(&mut self, layout: Layout) -> *mut () {
            let p = align(self.pos, layout.align());
            self.pos = align(p.add(layout.size()), mem::align_of::<VP>());
            debug_assert!(self.pos <= self.end);
            debug_assert_eq!(0, (p as usize) % layout.align());
            p as *mut ()
        }
    }
}

// Problem that `CallTrait` solves is calling a `FnOnce` from a &mut
// ref.
//
// Safety: Caller must ensure that each item is called only once,
// either through `call` or `drop`.
trait CallTrait<S> {
    unsafe fn call(&mut self, c: &mut S);
    unsafe fn drop(&mut self);
}

struct CallItem<S, F>
where
    F: FnOnce(&mut S),
{
    cb: F,
    phantomdata: PhantomData<S>,
}

impl<S, F> CallItem<S, F>
where
    F: FnOnce(&mut S),
{
    fn new(f: F) -> Self {
        Self {
            cb: f,
            phantomdata: PhantomData,
        }
    }
}

impl<S, F> CallTrait<S> for CallItem<S, F>
where
    F: FnOnce(&mut S),
{
    unsafe fn call(&mut self, c: &mut S) {
        let cb = std::ptr::read(&self.cb);
        cb(c);
    }
    unsafe fn drop(&mut self) {
        std::ptr::drop_in_place(&mut self.cb);
    }
}

impl<S, F> Drop for CallItem<S, F>
where
    F: FnOnce(&mut S),
{
    fn drop(&mut self) {
        panic!("CallItem must never be dropped");
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    fn d1(v1: i32) {
        println!("d1: {}", v1);
    }
    fn d2(v1: i32, v2: i32) {
        println!("d2: {} {}", v1, v2);
    }
    fn d3(v1: i32, v2: i32, v3: i32) {
        println!("d3: {} {} {}", v1, v2, v3);
    }

    struct Confirm(u64);
    impl Confirm {
        fn push(&mut self, nyb: u64) {
            self.0 = (self.0 << 4) + nyb;
        }
    }

    #[inline(never)]
    fn add_d3(queue: &mut super::FnOnceQueue<Confirm>, v1: i32, v2: i32, v3: i32) {
        // This closure can't specialise, so has to store the 3 values
        queue.push(move |c| {
            c.push(0xD);
            d3(v1, v2, v3);
        });
    }

    const fn round_up(val: usize, pow2: usize) -> usize {
        val.wrapping_add(pow2 - 1) & !(pow2 - 1)
    }

    #[repr(align(64))]
    struct Align64([u64; 8]);

    fn accept_align64(c: &mut Confirm, v: Align64) {
        c.push(0x6);
        assert_eq!(v.0[0], 123456789);
    }

    #[inline(never)]
    fn push_call_to_accept_align64(queue: &mut super::FnOnceQueue<Confirm>, v: Align64) {
        queue.push(move |c| accept_align64(c, v));
    }

    // 2048 bytes
    fn accept_bigarr(c: &mut Confirm, v: [u32; 512]) {
        c.push(0xB);
        assert_eq!(v[0], 123456789);
    }

    #[inline(never)]
    fn push_call_to_accept_bigarr(queue: &mut super::FnOnceQueue<Confirm>, v: [u32; 512]) {
        queue.push(move |c| accept_bigarr(c, v));
    }

    #[test]
    fn check_space_used() {
        let mut confirm = Confirm(0xF);
        let mut queue = super::FnOnceQueue::<Confirm>::new();
        let i32_unit = std::mem::size_of::<i32>();
        let usize_unit = std::mem::size_of::<usize>();

        // The three values are stored in the closure
        assert_eq!(queue.cap(), 0);
        let u0 = queue.len();
        add_d3(&mut queue, 12345678, 23456781, 34567812);
        assert_eq!(
            queue.len() - u0,
            round_up(usize_unit + 3 * i32_unit, usize_unit)
        );
        assert_eq!(queue.cap(), 1024);

        let u0 = queue.len();
        add_d3(&mut queue, 987654321, 765432198, 543219876);
        assert_eq!(
            queue.len() - u0,
            round_up(usize_unit + 3 * i32_unit, usize_unit)
        );

        // The closures below get inlined and specialised to the
        // arguments provided, so only take up a `usize` on the queue
        let u0 = queue.len();
        queue.push(|c| {
            c.push(1);
            d1(1);
        });
        assert_eq!(queue.len() - u0, usize_unit);

        let u0 = queue.len();
        queue.push(|c| {
            c.push(2);
            d2(2, 3);
        });
        assert_eq!(queue.len() - u0, usize_unit);

        let u0 = queue.len();
        queue.push(|c| {
            c.push(3);
            d3(4, 5, 6);
        });
        assert_eq!(queue.len() - u0, usize_unit);

        // Add align64 once to force the alignment to be predictable,
        // then a second time to check the space consumed.  Expect VP
        // (usize) plus padding out to 64 alignment, then 64-byte
        // value == 128.
        push_call_to_accept_align64(&mut queue, Align64([123456789; 8]));
        let u0 = queue.len();
        push_call_to_accept_align64(&mut queue, Align64([123456789; 8]));
        assert_eq!(queue.len() - u0, 128);

        // Now add something that will force allocation of a new
        // chunk, and chaining the old chunk.  Expect on the new list:
        // Call to old chunk `(*mut (), FnOnceQueue)`, then call to
        // bigarr `(*mut (), [u32; 512])`.
        assert_eq!(queue.cap(), 1024);
        push_call_to_accept_bigarr(&mut queue, [123456789; 512]);
        assert_eq!(
            queue.len(),
            usize_unit + std::mem::size_of::<super::FnOnceQueue<()>>() + usize_unit + 2048
        );
        assert_eq!(queue.cap(), 4096);

        // Pushing same value again should cause another allocation
        push_call_to_accept_bigarr(&mut queue, [123456789; 512]);
        assert_eq!(queue.cap(), 8192);

        queue.execute(&mut confirm);

        // Confirm that all the calls ran, in the correct order
        assert_eq!(confirm.0, 0xFDD12366BB);
    }

    struct TestDrop(Rc<RefCell<u32>>);
    impl TestDrop {
        fn run(&self) {
            panic!("TestDrop::run should never execute");
        }
    }
    impl Drop for TestDrop {
        fn drop(&mut self) {
            *self.0.borrow_mut() += 1;
        }
    }

    #[test]
    fn test_drop() {
        let confirm = Rc::new(RefCell::new(0));
        let mut queue = super::FnOnceQueue::<()>::new();
        let test = TestDrop(confirm.clone());
        queue.push(move |_| test.run());
        assert!(queue.len() > 0);
        assert_eq!(0, *confirm.borrow());
        drop(queue);
        assert_eq!(1, *confirm.borrow());
    }
}
