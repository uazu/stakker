use crate::rc::minrc::MinRc;

pub(crate) struct FwdRc<M>(MinRc<dyn Fn(M)>);

impl<M> FwdRc<M> {
    #[inline]
    pub fn new(val: impl Fn(M) + 'static) -> Self {
        // Original code: Self(MinRc::new(val))

        // TODO: This is a CoerceUnsized workaround
        Self(MinRc::<dyn Fn(M)>::new_with(|| {
            Box::new(super::minrc::MinRcBox {
                count: std::cell::Cell::new(1),
                inner: val, // <= coerce appears to occur here
            })
        }))
    }

    #[inline]
    pub fn inner(&self) -> &dyn Fn(M) {
        self.0.inner()
    }
}

impl<M> Clone for FwdRc<M> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
