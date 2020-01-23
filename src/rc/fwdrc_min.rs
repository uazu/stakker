use crate::core::Core;
use crate::rc::minrc::{Count, MinRc};

pub(crate) struct FwdRc<M>(MinRc<Count, dyn Fn(&mut Core, M)>);

impl<M> FwdRc<M> {
    #[inline]
    pub fn new(val: impl Fn(&mut Core, M) + 'static) -> Self {
        // Original code: Self(MinRc::new(val))

        // TODO: This is a CoerceUnsized workaround
        use crate::rc::minrc::StrongCount;
        Self(MinRc::<Count, dyn Fn(&mut Core, M)>::new_with(|| {
            Box::new(super::minrc::MinRcBox {
                count: std::cell::Cell::new(Count::new()),
                inner: val, // <= coerce appears to occur here
            })
        }))
    }

    #[inline]
    pub fn inner(&self) -> &dyn Fn(&mut Core, M) {
        self.0.inner()
    }
}

impl<M> Clone for FwdRc<M> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
