//! Check sizes of objects
//!
//! This test intentionally mem::forgets some allocations in order to
//! have the allocation size show up in valgrind.

use crate::{actor_new, ret_nop, ret_panic, ret_to, ActorOwn, Stakker, StopCause, CX};
use std::time::Instant;

#[allow(dead_code)]
struct Actor0([u8; 0]);
#[allow(dead_code)]
struct Actor1000([u8; 1000]);
#[allow(dead_code)]
struct Actor2000([u8; 2000]);

impl Actor0 {
    #[inline(never)]
    fn test(&self, _cx: CX![], _cause: Option<StopCause>) {}
}

struct Actors {
    _a0: ActorOwn<Actor0>,
    _a1: ActorOwn<Actor1000>,
    _a2: ActorOwn<Actor2000>,
}

test_fn!(
    fn actor_size() {
        let mut stakker = Stakker::new(Instant::now());
        let s = &mut stakker;
        let _a0 = actor_new!(s, Actor0, ret_nop!());
        let _a1 = actor_new!(s, Actor1000, ret_to!([_a0], test() as (StopCause)));
        let _a2 = actor_new!(s, Actor2000, ret_panic!("Shouldn't have died"));

        if std::env::var("STAKKER_ENABLE_TEST_MEMSIZES").is_ok() {
            // Forget a boxed struct holding the references.  This means that
            // they show up as indirect lost blocks in valgrind
            std::mem::forget(Box::new(Actors { _a0, _a1, _a2 }));
        }
    }
);
