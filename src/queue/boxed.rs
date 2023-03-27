/// Queue of `FnOnce(&mut Stakker)` items waiting for execution.
pub(crate) struct FnOnceQueue<S: 'static> {
    #[allow(clippy::type_complexity)]
    vec: Vec<Box<dyn FnOnce(&mut S) + 'static>>,
}

impl<S> FnOnceQueue<S> {
    pub fn new() -> Self {
        Self { vec: Vec::new() }
    }

    /// Check that internal implementation assumptions are valid
    pub fn sanity_check() {}

    /// Push a `FnOnce` callback onto the queue.
    #[inline]
    pub fn push(&mut self, value: impl FnOnce(&mut S) + 'static) {
        self.vec.push(Box::new(value));
    }

    /// Push a boxed `FnOnce` onto the queue
    #[inline]
    pub fn push_box(&mut self, value: Box<dyn FnOnce(&mut S) + 'static>) {
        self.vec.push(value);
    }

    /// Execute all the `FnOnce` instances found on this queue,
    /// passing them the given context object ref.  Leaves the queue
    /// empty, but with the same backing memory still allocated to
    /// aid in cache reuse.
    pub fn execute(&mut self, context: &mut S) {
        for f in self.vec.drain(..) {
            f(context);
        }
    }

    /// Test whether this queue is empty
    pub fn is_empty(&self) -> bool {
        self.vec.is_empty()
    }
}

impl<S> Default for FnOnceQueue<S> {
    fn default() -> Self {
        Self::new()
    }
}
