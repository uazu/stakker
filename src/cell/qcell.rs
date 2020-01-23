// QCell-based cells
use qcell::{QCell, QCellOwner, QCellOwnerID};

pub(crate) type ActorCell<T> = QCell<T>;
pub(crate) type ActorCellMaker = QCellOwnerID;
pub(crate) type ActorCellOwner = QCellOwner;

pub(crate) fn new_actor_cell_owner() -> (ActorCellOwner, ActorCellMaker) {
    let owner = QCellOwner::new();
    let maker = owner.id();
    (owner, maker)
}

pub(crate) type ShareCell<T> = QCell<T>;
pub(crate) type ShareCellOwner = QCellOwner;

pub(crate) fn new_share_cell_owner() -> ShareCellOwner {
    QCellOwner::new()
}
