use std::alloc::Layout;
use std::marker::PhantomData;
use std::mem;

// TODO: There's no point in reallocating and copying.  It would be
// better to just save a full queue and allocate a new one with double
// the space, and then when executing throw away all the smaller
// queues and keep just the biggest for next time.

// This code was liberally adapted from some example code provided by
// Simon Sapin in the Rust playground related to an RFC and Reddit
// comments.

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

impl<S> FnOnceQueue<S> {
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
        let mut item = CallItem::new(value);
        let ctref: &mut dyn CallTrait<S> = &mut item;
        assert_eq!(mem::size_of_val(&ctref), mem::size_of::<FatPointer>());
        let repr = unsafe { mem::transmute_copy::<&mut dyn CallTrait<S>, FatPointer>(&ctref) };
        self.storage.push(repr.meta);
        self.storage.push(item);
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
    fn len(&self) -> usize {
        self.storage.len()
    }

    /// Execute all the `FnOnce` instances found on this queue,
    /// passing them the given context object ref.  Leaves the queue
    /// empty, but with the same backing memory still allocated to
    /// aid in cache reuse.
    pub fn execute(&mut self, context: &mut S) {
        self.drain_for_each(|ptr| unsafe { (&mut *ptr).call(context) });
    }

    // This call will leak objects (i.e. not drop them) unless the
    // caller takes care of dropping each one.  That doesn't make the
    // interface unsafe, though
    fn drain_for_each(&mut self, mut apply: impl FnMut(*mut dyn CallTrait<S>)) {
        unsafe {
            let mut it = self.storage.drain();
            while let Some(meta) = it.read_next::<*mut ()>() {
                let data = 0x8000_usize as _; // Not null but aligned
                let fake_repr = FatPointer { data, meta };
                let fake_ref = mem::transmute_copy::<FatPointer, &dyn CallTrait<S>>(&fake_repr);
                let data = it.next(Layout::for_value(fake_ref)).unwrap();
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

// Heterogeneous Vec
mod hvec {
    use std::alloc::{self, Layout};
    use std::mem;
    use std::ptr;

    pub struct HVec {
        ptr: *mut u8,
        len: usize,
        capacity: usize,
        align: usize,
    }

    fn align_to(position: usize, align: usize) -> usize {
        // This will wrap (or panic) if position is almost at end of
        // range, but having a `Vec` that long is impossible because
        // `Vec` capacity can only be powers of two
        (position + align - 1) & !(align - 1)
    }

    impl HVec {
        // If we guess a strict-enough alignment here, we can
        // statically eliminate a check from most calls to `push`
        pub const MIN_ALIGN: usize = 16;

        // This just saves some reallocations at the start
        pub const MIN_CAPACITY: usize = 1024;

        pub const fn new() -> Self {
            Self {
                ptr: ptr::null_mut(),
                len: 0,
                capacity: 0,
                align: Self::MIN_ALIGN,
            }
        }

        /// Return length used in bytes within the queue
        pub fn len(&self) -> usize {
            self.len
        }

        #[inline]
        pub fn push<T>(&mut self, value: T) {
            // `capacity` is a power of two, so maximum is half the
            // total address space, and `len` <= `capacity`.  So only
            // way to exceed range of `usize` would be if `value_size`
            // is roughly more than half the address space.  In which
            // case where is it currently stored?  So exceeding
            // `usize` below appears impossible.
            let value_align = mem::align_of::<T>();
            let value_size = mem::size_of::<T>();
            let curr_pos = align_to(self.len, value_align);
            let next_pos = curr_pos + value_size;

            // Since `value_align` is a constant for given T, second
            // test is eliminated statically, except for the cases
            // where T is more strictly aligned than MIN_ALIGN
            if next_pos >= self.capacity
                || (value_align > Self::MIN_ALIGN && value_align > self.align)
            {
                self.realloc(next_pos, value_align);
            }
            unsafe {
                debug_assert!(self.ptr.add(curr_pos) as usize % value_align == 0);
                (self.ptr.add(curr_pos) as *mut T).write(value);
            }
            self.len = next_pos;
        }

        #[inline(never)]
        fn realloc(&mut self, req_capacity: usize, value_align: usize) {
            let new_align = self.align.max(value_align);
            let new_capacity = self
                .capacity
                .max(req_capacity)
                .next_power_of_two()
                .max(Self::MIN_CAPACITY);
            assert!(new_capacity >= req_capacity, "Capacity exceeded usize");
            let new_layout = Layout::from_size_align(new_capacity, new_align).unwrap();
            let old_layout = Layout::from_size_align(self.capacity, self.align).unwrap();
            self.ptr = unsafe {
                if self.ptr.is_null() {
                    alloc::alloc(new_layout)
                } else if new_layout.align() == old_layout.align() {
                    alloc::realloc(self.ptr, old_layout, new_layout.size())
                } else {
                    let new_ptr = alloc::alloc(new_layout);
                    if !new_ptr.is_null() {
                        let size = old_layout.size().min(new_layout.size());
                        ptr::copy_nonoverlapping(self.ptr, new_ptr, size);
                        alloc::dealloc(self.ptr, old_layout);
                    }
                    new_ptr
                }
            };
            if self.ptr.is_null() {
                alloc::handle_alloc_error(new_layout)
            }
            self.capacity = new_capacity;
            self.align = new_align;
        }

        pub fn drain(&mut self) -> Drain<'_> {
            // If `Drain` is leaked, leak memory also, to be safe
            let len = self.len;
            self.len = 0;
            Drain {
                vec: self,
                position: 0,
                len,
            }
        }
    }

    pub struct Drain<'a> {
        vec: &'a HVec,
        position: usize,
        len: usize,
    }

    impl<'a> Drain<'a> {
        pub unsafe fn read_next<T>(&mut self) -> Option<T> {
            self.next(Layout::new::<T>())
                .map(|ptr| (ptr as *mut T).read())
        }

        pub unsafe fn next(&mut self, layout: Layout) -> Option<*mut ()> {
            if self.position < self.len || layout.size() == 0 {
                let curr_pos = align_to(self.position, layout.align());
                let ptr = self.vec.ptr.add(curr_pos);
                self.position = curr_pos + layout.size();
                debug_assert!(self.position <= self.len);
                Some(ptr as *mut ())
            } else {
                None
            }
        }
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

    fn add_d3(queue: &mut super::FnOnceQueue<()>, v1: i32, v2: i32, v3: i32) {
        // This closure can't specialise, so has to store the 3 values
        queue.push(move |_| d3(v1, v2, v3));
    }

    fn add_empty(queue: &mut super::FnOnceQueue<()>) {
        // This aligns the queue so that the measurements come out
        // right
        queue.push(move |_| {});
    }

    #[test]
    fn check_space_used() {
        let mut queue = super::FnOnceQueue::<()>::new();
        let i32_unit = std::mem::size_of::<i32>();
        let usize_unit = std::mem::size_of::<usize>();

        let u0 = queue.len();
        add_d3(&mut queue, 12345678, 23456781, 34567812);
        assert_eq!(queue.len() - u0, usize_unit + 3 * i32_unit);

        add_empty(&mut queue);
        let u0 = queue.len();
        add_d3(&mut queue, 987654321, 765432198, 543219876);
        assert_eq!(queue.len() - u0, usize_unit + 3 * i32_unit);

        // The closures below get inlined and specialised to the
        // arguments provided, so only take up a `usize` on the queue
        add_empty(&mut queue);
        let u0 = queue.len();
        queue.push(|_| d1(1));
        assert_eq!(queue.len() - u0, usize_unit);

        add_empty(&mut queue);
        let u0 = queue.len();
        queue.push(|_| d2(2, 3));
        assert_eq!(queue.len() - u0, usize_unit);

        add_empty(&mut queue);
        let u0 = queue.len();
        queue.push(|_| d3(4, 5, 6));
        assert_eq!(queue.len() - u0, usize_unit);

        queue.execute(&mut ());
    }

    #[repr(align(32))]
    struct Align32([u32; 8]);

    #[inline(never)]
    fn accept_align32(c: &mut u32, v: Align32) {
        (*c) += 0x100;
        assert_eq!(v.0[0], 123456789);
    }

    #[inline(never)]
    fn accept_bigarr(c: &mut u32, v: [u32; 512]) {
        (*c) += 0x10000;
        assert_eq!(v[0], 123456789);
    }

    #[test]
    fn test_reallocation() {
        let mut confirm = 0;
        let mut queue = super::FnOnceQueue::<u32>::new();

        // Force first allocation
        assert!(queue.is_empty());
        queue.push(move |c| (*c) += 1);
        assert!(queue.len() > 0);
        assert!(!queue.is_empty());

        // Test reallocation with a different alignment
        let v = Align32([123456789; 8]);
        queue.push(move |c| accept_align32(c, v));
        assert!(queue.len() < super::hvec::HVec::MIN_CAPACITY);

        // Test reallocation because exceeded initial capacity
        let v = [123456789; 512];
        queue.push(move |c| accept_bigarr(c, v));
        assert!(queue.len() > super::hvec::HVec::MIN_CAPACITY);

        queue.execute(&mut confirm);
        assert_eq!(confirm, 0x10101);
    }

    struct TestDrop(Rc<RefCell<u32>>);
    impl TestDrop {
        fn run(&self) {
            unreachable!("TestDrop::run should never execute");
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

// Problem that `CallTrait` solves is calling a `FnOnce` from a &mut
// ref.  Caller must ensure that each item is called only once, either
// through `call` or `drop`.
trait CallTrait<S> {
    unsafe fn call(&mut self, c: &mut S);
    unsafe fn drop(&mut self);
}

struct CallItem<S, F: FnOnce(&mut S)> {
    cb: F,
    phantomdata: PhantomData<S>,
}

impl<S, F: FnOnce(&mut S)> CallItem<S, F> {
    fn new(f: F) -> Self {
        Self {
            cb: f,
            phantomdata: PhantomData,
        }
    }
}

impl<S, F: FnOnce(&mut S)> CallTrait<S> for CallItem<S, F> {
    unsafe fn call(&mut self, c: &mut S) {
        let cb = std::ptr::read(&self.cb);
        cb(c);
    }
    unsafe fn drop(&mut self) {
        std::ptr::drop_in_place(&mut self.cb);
    }
}

impl<S, F: FnOnce(&mut S)> Drop for CallItem<S, F> {
    fn drop(&mut self) {
        unreachable!("CallItem must never be dropped");
    }
}
