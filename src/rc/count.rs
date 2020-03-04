use crate::actor::State;

// If the count reaches the maximum value, it locks there and never
// decreases, possibly leaking the ActorBox memory if there are any
// weak reference cycles.  However the only way to get a count that
// high is by the coder leaking actor references, which means there
// are much bigger problems.  Leaking memory is safe in any case.
#[derive(Copy, Clone)]
pub(crate) struct CountAndState(usize);

const COUNT_SHIFT: u32 = 2;
const COUNT_INC: usize = 1 << COUNT_SHIFT;
const COUNT_MASK: usize = !(COUNT_INC - 1);

impl CountAndState {
    pub fn new() -> Self {
        // Start at count of 0
        Self(State::Prep as usize)
    }

    pub fn inc(self) -> Self {
        if self.0 >= COUNT_MASK {
            self
        } else {
            Self(self.0 + COUNT_INC)
        }
    }

    pub fn dec(self) -> (Self, bool) {
        if self.0 < COUNT_INC || self.0 >= COUNT_MASK {
            (self, false)
        } else {
            let val = self.0 - COUNT_INC;
            (Self(val), val < COUNT_INC)
        }
    }

    pub fn set_state(self, state: State) -> Self {
        Self((self.0 & COUNT_MASK) | (state as usize))
    }

    pub fn is_prep(self) -> bool {
        (self.0 & !COUNT_MASK) == (State::Prep as usize)
    }

    pub fn is_zombie(self) -> bool {
        (self.0 & !COUNT_MASK) == (State::Zombie as usize)
    }
}
