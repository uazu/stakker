//! Test macros

use crate::{
    actor, actor_new, after, at, call, fwd, fwd_do, fwd_nop, fwd_panic, fwd_to, idle, lazy, ret,
    ret_do, ret_nop, ret_panic, ret_shutdown, ret_some_do, ret_some_to, ret_to, timer_max,
    timer_min, Actor, Ret, Stakker, StopCause, CX,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

#[derive(Clone)]
struct Scores(Rc<RefCell<[u8; 100]>>);
impl Scores {
    fn new() -> Self {
        Self(Rc::new(RefCell::new([0; 100])))
    }
    fn get(&self, i: usize) -> u8 {
        self.0.borrow()[i]
    }
}

struct ScoreInc {
    scores: Scores,
    index: usize,
}
impl ScoreInc {
    fn inc(&self) {
        let mut arr = self.scores.0.borrow_mut();
        arr[self.index] = arr[self.index].saturating_add(1);
    }
}

struct Aux {
    now: Instant,
    test_index: usize,
    scores: Scores,
}
impl Aux {
    fn new(s: &mut Stakker) -> Self {
        let now = s.now();
        let scores = Scores::new();
        let test_index = 0;
        Self {
            now,
            test_index,
            scores,
        }
    }
    fn si(&mut self) -> ScoreInc {
        self.test_index += 1;
        ScoreInc {
            scores: self.scores.clone(),
            index: self.test_index - 1,
        }
    }
    fn run(&mut self, s: &mut Stakker) {
        const MINUTE: Duration = Duration::from_secs(60);
        let stop = self.now + MINUTE;
        s.run(self.now, false);
        while self.now < stop {
            let delay = s.next_wait_max(self.now, MINUTE, false);
            assert_ne!(delay, Duration::from_secs(0));
            self.now += delay;
            s.run(self.now, true);
        }
    }
    fn check_scores(&mut self) {
        assert!(self.test_index > 0, "Expecting at least one score to check");
        let mut fail = false;
        for i in 0..self.test_index {
            let score = self.scores.get(i);
            if score != 1 {
                fail = true;
                println!("score[{}] == {} (instead of 1)", i, score);
            }
        }
        assert!(!fail);
    }
}

// Register a score increment
macro_rules! SCORE {
    ($si:expr) => {
        $si.inc()
    };
}

struct A;
impl A {
    fn init(_: CX![]) -> Option<Self> {
        Some(Self)
    }
    fn get(&self, _: CX![], resp: Ret<()>) {
        ret!([resp]);
    }
    fn check(&self, _: CX![], si: ScoreInc) {
        SCORE!(si);
    }
    fn fwd_test(&mut self, cx: CX![], si1: ScoreInc, si2: ScoreInc, si3: ScoreInc) {
        let fwd = fwd_to!([cx], check() as (ScoreInc));
        fwd!([fwd], si1);
        let fwd = fwd_to!([cx], |_this, _cx, si: ScoreInc| SCORE!(si));
        fwd!([fwd], si2);
        let fwd = fwd_to!([cx], |_this, _cx, si: ScoreInc, v: u8| {
            assert_eq!(v, 99);
            SCORE!(si);
        });
        fwd!([fwd], si3, 99);
    }
    fn fwd_init(cx: CX![]) -> Option<Self> {
        let fwd = fwd_to!([cx], Self::init() as ());
        fwd!([fwd]);
        None
    }
    fn ret_test(cx: CX![], b: Actor<B>, si: ScoreInc) -> Option<Self> {
        // Test that returned value is passed through as Some
        call!([b, cx], get(ret_to!([cx], Self::ret_test_1(b, si) as (u8))));
        None
    }
    fn ret_test_1(cx: CX![], b: Actor<B>, si: ScoreInc, rv: Option<u8>) -> Option<Self> {
        assert_eq!(rv, Some(123));
        // Test drop gives None result
        drop(ret_to!([cx], Self::ret_test_2(b, si) as (u8)));
        None
    }
    fn ret_test_2(cx: CX![], b: Actor<B>, si: ScoreInc, rv: Option<u8>) -> Option<Self> {
        assert_eq!(rv, None);
        call!([b, cx], get(ret_to!([cx], ret_test_3(b, si) as (u8))));
        Some(Self) // Continue on to Ready part of test
    }
    fn ret_test_3(&mut self, cx: CX![], b: Actor<B>, si: ScoreInc, rv: Option<u8>) {
        assert_eq!(rv, Some(123));
        call!(
            [b],
            get(ret_to!([cx], |_this, _cx, rv: Option<u8>| {
                assert_eq!(rv, Some(123));
                SCORE!(si);
            }))
        );
    }
    fn ret_some_test(cx: CX![], b: Actor<B>, si: ScoreInc) -> Option<Self> {
        call!(
            [b, cx],
            get(ret_some_to!([cx], Self::ret_some_test_1(b, si) as (u8)))
        );
        None
    }
    fn ret_some_test_1(cx: CX![], b: Actor<B>, si: ScoreInc, rv: u8) -> Option<Self> {
        assert_eq!(rv, 123);
        call!(
            [b, cx],
            get(ret_some_to!([cx], ret_some_test_2(b, si) as (u8)))
        );
        Some(Self) // Continue on to Ready part of test
    }
    fn ret_some_test_2(&mut self, cx: CX![], b: Actor<B>, si: ScoreInc, rv: u8) {
        assert_eq!(rv, 123);
        call!(
            [b],
            get(ret_some_to!([cx], |_this, _cx, rv: u8| {
                assert_eq!(rv, 123);
                SCORE!(si);
            }))
        );
    }
}

struct B(u8);
impl B {
    fn init(cx: CX![], v: u8) -> Option<Self> {
        // Test Prep call to same actor
        call!([cx], Self::init2(v));
        None
    }
    fn init2(_: CX![], v: u8) -> Option<Self> {
        Some(Self(v))
    }
    fn get(&self, _: CX![], resp: Ret<u8>) {
        ret!([resp], self.0);
    }
    fn test(&mut self, cx: CX![], si: ScoreInc) {
        // Test inline closure
        call!([cx], |this, cx| {
            this.test2(cx, si);
        });
    }
    fn test2(&mut self, _: CX![], si: ScoreInc) {
        SCORE!(si);
    }
    fn check(&self, _: CX![], v: u8, si: ScoreInc) {
        assert_eq!(v, self.0);
        SCORE!(si);
    }
    fn fwd_init(cx: CX![], v: u8) -> Option<Self> {
        let fwd = fwd_to!([cx], <crate::test::macros::B>::init() as (u8));
        fwd!([fwd], v);
        None
    }
}

struct C(u8, u16);
impl C {
    // Test calling something other than `init` from actor!
    fn init1(cx: CX![], v1: u8, v2: u16) -> Option<Self> {
        // Test path form of Prep call to same actor
        call!([cx], <crate::test::macros::C>::init2(v2, v1));
        None
    }
    fn init2(_: CX![], v2: u16, v1: u8) -> Option<Self> {
        Some(Self(v1, v2))
    }
    fn get(&self, cx: CX![], resp: Ret<(u8, u16)>) {
        // Test Ready call to same actor
        call!([cx], get2(resp));
    }
    fn get2(&self, _: CX![], resp: Ret<(u8, u16)>) {
        ret!([resp], self.0, self.1);
    }
    fn test(&mut self, cx: CX![], si: ScoreInc) {
        call!([cx], test2(si, self.0, self.1));
    }
    fn test2(&mut self, _: CX![], si: ScoreInc, v1: u8, v2: u16) {
        assert_eq!(v1, self.0);
        assert_eq!(v2, self.1);
        SCORE!(si);
    }
    fn check(&self, _: CX![], v1: u8, v2: u16, si: ScoreInc) {
        assert_eq!(v1, self.0);
        assert_eq!(v2, self.1);
        SCORE!(si);
    }
    fn fwd_init(cx: CX![], v1: u8, v2: u16) -> Option<Self> {
        let fwd = fwd_to!([cx], <C>::init1() as (u8, u16));
        fwd!([fwd], v1, v2);
        None
    }
}

#[test]
fn creation_call_and_response() {
    let mut stakker = Stakker::new(Instant::now());
    let s = &mut stakker;
    let mut aux = Aux::new(s);

    // 3 forms of actor creation with `actor!` and `actor_new!`
    let a = actor_new!(s, A, ret_panic!("Actor A died"));
    call!([a], A::init());
    let b = actor!(s, B::init(123), ret_panic!("Actor B died"));
    let c = actor!(
        s,
        <crate::test::macros::C>::init1(123, 23456),
        ret_panic!("Actor C died")
    );

    // Check result of get
    let si = aux.si();
    call!(
        [a],
        get(ret_some_do!(move |()| {
            SCORE!(si);
        }))
    );
    let si = aux.si();
    call!(
        [b],
        get(ret_some_do!(move |v| {
            assert_eq!(v, 123);
            SCORE!(si);
        }))
    );
    let si = aux.si();
    call!(
        [c],
        get(ret_some_do!(move |(v1, v2)| {
            assert_eq!(v1, 123);
            assert_eq!(v2, 23456);
            SCORE!(si);
        }))
    );

    aux.run(s);
    aux.check_scores();
}

#[test]
fn actor_calls() {
    let mut stakker = Stakker::new(Instant::now());
    let s = &mut stakker;
    let mut aux = Aux::new(s);

    let a = actor_new!(s, A, ret_panic!("Actor A died"));
    let b = actor_new!(s, B, ret_panic!("Actor B died"));
    let c = actor_new!(s, C, ret_panic!("Actor C died"));

    // Test that Ready calls are queued until after Prep state is
    // established
    call!([a], check(aux.si()));
    call!([b], check(159, aux.si()));
    call!([c], check(159, 54321, aux.si()));

    // Test all forms of Prep call to another actor
    call!([a], A::init());
    call!([b], <B>::init(159));
    call!([c], <crate::test::macros::C>::init1(159, 54321));

    // Test some chained calls
    call!([b], test(aux.si()));
    call!([c], test(aux.si()));

    // Test Stakker closure call
    let si = aux.si();
    call!([s], |_s| SCORE!(si));

    aux.run(s);
    aux.check_scores();
}

#[test]
fn lazy_prep_calls() {
    let mut stakker = Stakker::new(Instant::now());
    let s = &mut stakker;
    let mut aux = Aux::new(s);

    let a = actor_new!(s, A, ret_panic!("Actor A died"));
    let b = actor_new!(s, B, ret_panic!("Actor B died"));
    let c = actor_new!(s, C, ret_panic!("Actor C died"));

    lazy!([a, s], A::init());
    lazy!([b, s], <B>::init(159));
    lazy!([c, s], <crate::test::macros::C>::init1(159, 54321));

    // These will also get delayed and queued on the actor until it
    // goes to Ready (which executes on the lazy queue)
    call!([a], check(aux.si()));
    call!([b], check(159, aux.si()));
    call!([c], check(159, 54321, aux.si()));

    aux.run(s);
    aux.check_scores();
}

#[test]
fn timers_and_queues() {
    let mut stakker = Stakker::new(Instant::now());
    let s = &mut stakker;
    let mut aux = Aux::new(s);

    let a = actor!(s, A::init(), ret_panic!("Actor A died"));
    let fwd = fwd_to!([a], check() as (ScoreInc));

    call!([a], check(aux.si()));
    fwd!([fwd], aux.si());
    idle!([a, s], check(aux.si()));
    lazy!([a, s], check(aux.si()));
    after!(Duration::from_secs(1), [a, s], check(aux.si()));
    at!(s.now() + Duration::from_secs(3), [a, s], check(aux.si()));

    let mut key = Default::default();
    timer_max!(
        &mut key,
        s.now() + Duration::from_secs(5),
        [a, s],
        check(aux.si())
    );
    let mut key = Default::default();
    timer_min!(
        &mut key,
        s.now() + Duration::from_secs(7),
        [a, s],
        check(aux.si())
    );

    aux.run(s);
    aux.check_scores();
}

#[test]
fn fwds() {
    let mut stakker = Stakker::new(Instant::now());
    let s = &mut stakker;
    let mut aux = Aux::new(s);

    let a = actor_new!(s, A, ret_panic!("Actor A died"));
    let b = actor_new!(s, B, ret_panic!("Actor B died"));
    let c = actor_new!(s, C, ret_panic!("Actor C died"));

    let fwd = fwd_to!([a], check() as (ScoreInc));
    fwd!([fwd], aux.si());
    let fwd = fwd_to!([b], check() as (u8, ScoreInc));
    fwd!([fwd], 159, aux.si());
    let fwd = fwd_to!([c], check(159, 54321) as (ScoreInc));
    fwd!([fwd], aux.si());

    let fwd = fwd_to!([a], A::init() as ());
    fwd!([fwd]);
    let fwd = fwd_to!([b], <B>::init() as (u8));
    fwd!([fwd], 159);
    let fwd = fwd_to!([c], <crate::test::macros::C>::init1(159) as (u16));
    fwd!([fwd], 54321);

    // Self-based fwd tested in A
    call!([a], fwd_test(aux.si(), aux.si(), aux.si()));

    aux.run(s);
    aux.check_scores();
}

#[test]
fn fwds_2() {
    let mut stakker = Stakker::new(Instant::now());
    let s = &mut stakker;
    let mut aux = Aux::new(s);

    let a = actor_new!(s, A, ret_panic!("Actor A died"));
    let b = actor_new!(s, B, ret_panic!("Actor B died"));
    let c = actor_new!(s, C, ret_panic!("Actor C died"));

    call!([a], check(aux.si()));
    call!([b], check(159, aux.si()));
    call!([c], check(159, 54321, aux.si()));

    call!([a], A::fwd_init());
    call!([b], B::fwd_init(159));
    call!([c], C::fwd_init(159, 54321));

    aux.run(s);
    aux.check_scores();
}

#[test]
fn ret() {
    let mut stakker = Stakker::new(Instant::now());
    let s = &mut stakker;
    let mut aux = Aux::new(s);

    let a = actor_new!(s, A, ret_panic!("Actor A died"));
    let b = actor!(s, B::init(123), ret_panic!("Actor B died"));
    call!([a], A::ret_test(b.clone(), aux.si()));

    aux.run(s);
    aux.check_scores();
}

#[test]
fn ret_some() {
    let mut stakker = Stakker::new(Instant::now());
    let s = &mut stakker;
    let mut aux = Aux::new(s);

    let a = actor_new!(s, A, ret_panic!("Actor A died"));
    let b = actor!(s, B::init(123), ret_panic!("Actor B died"));
    call!([a], A::ret_some_test(b.clone(), aux.si()));

    aux.run(s);
    aux.check_scores();
}

#[test]
fn fwd_misc() {
    let mut stakker = Stakker::new(Instant::now());
    let s = &mut stakker;

    let fwd = fwd_nop!();
    fwd!([fwd]);
    let ret = ret_nop!();
    ret!([ret]);

    let ret = ret_shutdown!(s);
    assert!(s.not_shutdown());
    ret!([ret], StopCause::Stopped);
    s.run(s.now(), false);
    assert!(!s.not_shutdown());
}

#[test]
#[should_panic]
fn panic_0() {
    let fwd = fwd_panic!("Test");
    fwd!([fwd]);
}

#[test]
#[should_panic]
fn panic_1() {
    let ret = ret_panic!("Test");
    ret!([ret]);
}

#[test]
#[should_panic]
fn panic_2() {
    let fwd = fwd_do!(|()| panic!("Test"));
    fwd!([fwd]);
}

#[test]
fn panic_2_drop() {
    // This one doesn't panic
    let fwd = fwd_do!(|()| panic!("Test"));
    drop(fwd);
}

#[test]
#[should_panic]
fn panic_3() {
    let ret = ret_do!(|_: Option<()>| panic!("Test"));
    ret!([ret]);
}

#[test]
#[should_panic]
fn panic_3_drop() {
    // This one panics
    let ret = ret_do!(|_: Option<()>| panic!("Test"));
    drop(ret);
}
