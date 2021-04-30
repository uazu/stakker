//! The ActorOwnAnon example from:
//! https://docs.rs/stakker/*/stakker/struct.ActorOwnAnon.html

use stakker::*;
use std::time::Instant;

struct Cat;
impl Cat {
    fn init(_: CX![]) -> Option<Self> {
        Some(Self)
    }
    fn sound(&mut self, _: CX![]) {
        println!("Miaow");
    }
}

struct Dog;
impl Dog {
    fn init(_: CX![]) -> Option<Self> {
        Some(Self)
    }
    fn sound(&mut self, _: CX![]) {
        println!("Woof");
    }
}

// This function doesn't know whether it's getting a cat or a dog,
// but it can still call it and drop it when it has finished
pub fn call_and_drop(sound: Fwd<()>, own: ActorOwnAnon) {
    fwd!([sound]);
}

fn main() {
    let mut stakker = Stakker::new(Instant::now());
    let s = &mut stakker;

    let cat = actor!(s, Cat::init(), ret_nop!());
    call_and_drop(fwd_to!([cat], sound() as ()), cat.anon());

    let dog = actor!(s, Dog::init(), ret_nop!());
    call_and_drop(fwd_to!([dog], sound() as ()), dog.anon());

    s.run(Instant::now(), false);
}
