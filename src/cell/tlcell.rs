// TLCell-based cells
use qcell::{TLCell, TLCellOwner};

pub(crate) struct ActorMarker;
pub(crate) type ActorCell<T> = TLCell<ActorMarker, T>;
pub(crate) struct ActorCellMaker;
pub(crate) type ActorCellOwner = TLCellOwner<ActorMarker>;

impl ActorCellMaker {
    #[inline]
    pub fn cell<T>(&self, value: T) -> ActorCell<T> {
        TLCell::new(value)
    }
}

pub(crate) fn new_actor_cell_owner() -> (ActorCellOwner, ActorCellMaker) {
    (TLCellOwner::new(), ActorCellMaker)
}

pub(crate) struct ShareMarker;
pub(crate) type ShareCell<T> = TLCell<ShareMarker, T>;
pub(crate) type ShareCellOwner = TLCellOwner<ShareMarker>;

pub(crate) fn new_share_cell_owner() -> ShareCellOwner {
    TLCellOwner::new()
}
