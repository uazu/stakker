// TCell-based cells
use super::TCellOwner; // Make sure we get the protected one in testing
use qcell::TCell;

pub(crate) struct ActorMarker;
pub(crate) type ActorCell<T> = TCell<ActorMarker, T>;
pub(crate) struct ActorCellMaker;
pub(crate) type ActorCellOwner = TCellOwner<ActorMarker>;

impl ActorCellMaker {
    #[inline]
    pub fn cell<T>(&self, value: T) -> ActorCell<T> {
        TCell::new(value)
    }
}

pub(crate) fn new_actor_cell_owner() -> (ActorCellOwner, ActorCellMaker) {
    (TCellOwner::new(), ActorCellMaker)
}

pub(crate) struct ShareMarker;
pub(crate) type ShareCell<T> = TCell<ShareMarker, T>;
pub(crate) type ShareCellOwner = TCellOwner<ShareMarker>;

pub(crate) fn new_share_cell_owner() -> ShareCellOwner {
    TCellOwner::new()
}
