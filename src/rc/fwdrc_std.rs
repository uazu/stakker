use std::rc::Rc;

pub(crate) struct FwdRc<M>(Rc<dyn Fn(M)>);

impl<M> FwdRc<M> {
    #[inline]
    pub fn new(val: impl Fn(M) + 'static) -> Self {
        Self(Rc::new(val))
    }

    #[inline]
    pub fn inner(&self) -> &dyn Fn(M) {
        &*self.0
    }
}

impl<M> Clone for FwdRc<M> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
