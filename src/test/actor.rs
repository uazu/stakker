use crate::actor::StringError;
use crate::*;
use std::time::{Duration, Instant};

test_fn!(
    fn actor_termination() {
        let now = Instant::now();
        let mut stakker = Stakker::new(now);
        let s = &mut stakker;

        struct Test;
        impl Test {
            fn init_stop_1(cx: CX![]) -> Option<Self> {
                cx.stop();
                None
            }
            fn init_stop_2(cx: CX![]) -> Option<Self> {
                stop!(cx);
                None
            }
            fn init_fail(cx: CX![]) -> Option<Self> {
                cx.fail(Box::new(StringError("TEST fail".into())));
                None
            }
            fn init_fail_str(cx: CX![]) -> Option<Self> {
                cx.fail_str("TEST fail_str");
                None
            }
            fn init_fail_string(cx: CX![]) -> Option<Self> {
                cx.fail_string("TEST fail_string");
                None
            }
            fn init_fail_macro_1(cx: CX![]) -> Option<Self> {
                fail!(cx, Box::new(StringError("TEST fail macro 1".into())));
                None
            }
            fn init_fail_macro_2(cx: CX![]) -> Option<Self> {
                fail!(cx, "TEST fail macro 2");
                None
            }
            fn init_fail_macro_3(cx: CX![]) -> Option<Self> {
                fail!(cx, "{}", "TEST fail macro 3");
                None
            }
            fn init(_cx: CX![]) -> Option<Self> {
                Some(Self)
            }
            fn do_stop_1(&mut self, cx: CX![]) {
                cx.stop();
            }
            fn do_stop_2(&mut self, cx: CX![]) {
                stop!(cx);
            }
            fn do_fail(&mut self, cx: CX![]) {
                cx.fail(Box::new(StringError("TEST fail".into())));
            }
            fn do_fail_str(&mut self, cx: CX![]) {
                cx.fail_str("TEST fail_str");
            }
            fn do_fail_string(&mut self, cx: CX![]) {
                cx.fail_string("TEST fail_string");
            }
            fn do_fail_macro_1(&mut self, cx: CX![]) {
                fail!(cx, Box::new(StringError("TEST fail macro 1".into())));
            }
            fn do_fail_macro_2(&mut self, cx: CX![]) {
                fail!(cx, "TEST fail macro 2");
            }
            fn do_fail_macro_3(&mut self, cx: CX![]) {
                fail!(cx, "{}", "TEST fail macro 3");
            }
            fn query(&mut self, _cx: CX![]) -> u32 {
                12345
            }
            fn query2(&mut self, _cx: CX![], a: u32, b: u32) -> u32 {
                12345 + a * 5 + b
            }
        }

        // Test stop and stop! from init
        macro_rules! test_init_stopped {
        ($($call:tt)+) => {
            let _a = actor!(s, $($call)+, ret_shutdown!(s));
            assert!(matches!(s.shutdown_reason(), None));
            s.run(now, false);
            assert!(matches!(s.shutdown_reason(), Some(StopCause::Stopped)));
        };
    }
        test_init_stopped!(Test::init_stop_1());
        test_init_stopped!(Test::init_stop_2());

        // Test fail/fail_str/fail_string/fail! from init
        macro_rules! test_init_failed {
        // Args backwards due to needing a TT+
        ($expect:expr; $($call:tt)+) => {
            let _a = actor!(s, $($call)+, ret_shutdown!(s));
            assert!(matches!(s.shutdown_reason(), None));
            s.run(now, false);
            match s.shutdown_reason() {
                Some(StopCause::Failed(e)) => assert_eq!($expect, format!("{}", e)),
                cause => panic!("Unexpected shutdown reason: {:?}", cause),
            }
        };
    }
        test_init_failed!("TEST fail"; Test::init_fail());
        test_init_failed!("TEST fail_str"; Test::init_fail_str());
        test_init_failed!("TEST fail_string"; Test::init_fail_string());
        test_init_failed!("TEST fail macro 1"; Test::init_fail_macro_1());
        test_init_failed!("TEST fail macro 2"; Test::init_fail_macro_2());
        test_init_failed!("TEST fail macro 3"; Test::init_fail_macro_3());

        // Test stop from method
        macro_rules! test_stopped {
        ($($call:tt)+) => {
            let a = actor!(s, Test::init(), ret_shutdown!(s));
            call!([a], $($call)+);
            assert!(matches!(s.shutdown_reason(), None));
            s.run(now, false);
            assert!(matches!(s.shutdown_reason(), Some(StopCause::Stopped)));
        };
    }
        test_stopped!(do_stop_1());
        test_stopped!(do_stop_2());

        // Test fail from method
        macro_rules! test_failed {
        // Args backwards due to needing a TT+
        ($expect:expr; $($call:tt)+) => {
            let a = actor!(s, Test::init(), ret_shutdown!(s));
            call!([a], $($call)+);
            assert!(matches!(s.shutdown_reason(), None));
            s.run(now, false);
            match s.shutdown_reason() {
                Some(StopCause::Failed(e)) => assert_eq!($expect, format!("{}", e)),
                cause => panic!("Unexpected shutdown: {:?}", cause),
            }
        };
    }
        test_failed!("TEST fail"; do_fail());
        test_failed!("TEST fail_str"; do_fail_str());
        test_failed!("TEST fail_string"; do_fail_string());
        test_failed!("TEST fail macro 1"; do_fail_macro_1());
        test_failed!("TEST fail macro 2"; do_fail_macro_2());
        test_failed!("TEST fail macro 3"; do_fail_macro_3());

        // Test kill/kill_str/kill_string/kill!
        macro_rules! test_killed {
        ($expect:expr; $a:ident; $($kill:tt)+) => {
            let $a = actor!(s, Test::init(), ret_shutdown!(s));
            $($kill)+;
            assert!(matches!(s.shutdown_reason(), None));
            s.run(now, false);
            match s.shutdown_reason() {
                Some(StopCause::Killed(e)) => assert_eq!($expect, format!("{}", e)),
                cause => panic!("Unexpected shutdown: {:?}", cause),
            }
        };
    }
        test_killed!("TEST kill"; a; a.kill(s, Box::new(StringError("TEST kill".into()))));
        test_killed!("TEST kill_str"; a; a.kill_str(s, "TEST kill_str"));
        test_killed!("TEST kill_string"; a; a.kill_string(s, "TEST kill_string"));
        test_killed!("TEST macro kill 1"; a; kill!(a, Box::new(StringError("TEST macro kill 1".into()))));
        test_killed!("TEST macro kill 2"; a; kill!(a, "TEST macro kill 2"));
        test_killed!("TEST macro kill 3"; a; kill!(a, "{}", "TEST macro kill 3"));

        // Test drop
        let a = actor!(s, Test::init(), ret_shutdown!(s));
        drop(a);
        assert!(matches!(s.shutdown_reason(), None));
        s.run(now, false);
        assert!(matches!(s.shutdown_reason(), Some(StopCause::Dropped)));

        // Test fail in query
        let g = actor!(s, Test::init(), ret_shutdown!(s));
        assert_eq!(g.query(s, |this, cx| this.query(cx)), None); // Not running yet
        assert_eq!(query!([g, s], query2(1, 2)), None); // Not running yet
        s.run(now, false);
        assert!(matches!(s.shutdown_reason(), None));
        assert_eq!(g.query(s, |this, cx| this.query2(cx, 1, 2)), Some(12352)); // Running
        assert_eq!(query!([g, s], query()), Some(12345)); // Running
        s.run(now, false);
        assert!(matches!(s.shutdown_reason(), None));
        assert_eq!(query!([g, s], do_stop_1()), Some(()));
        s.run(now, false);
        assert!(matches!(s.shutdown_reason(), Some(StopCause::Stopped)));
    }
);

test_fn!(
    fn stopcause() {
        let s0 = StopCause::Stopped;
        let s1 = StopCause::Failed(Box::new(StringError("TEST".to_string())));
        let s2 = StopCause::Killed(Box::new(StringError("TEST".to_string())));
        let s3 = StopCause::Dropped;
        assert_eq!(false, s0.has_error());
        assert_eq!(true, s1.has_error());
        assert_eq!(true, s2.has_error());
        assert_eq!(false, s3.has_error());
        // Exercise the Debug and Display code
        assert_eq!(
            format!("{:?} / {:?} / {:?} / {:?}", s0, s1, s2, s3),
            "Actor stopped / Actor failed: TEST / Actor was killed: TEST / Actor was dropped"
        );
    }
);

test_fn!(
    fn cascade_failure() {
        let now = Instant::now();
        let mut stakker = Stakker::new(now);
        let s = &mut stakker;

        struct A(Option<ActorOwn<B>>);
        impl A {
            fn init0(cx: CX![]) -> Option<Self> {
                Some(Self(Some(actor!(
                    cx,
                    B::init(),
                    ret_fail!(cx, "Test fail 0")
                ))))
            }
            fn init1(cx: CX![]) -> Option<Self> {
                Some(Self(Some(actor!(
                    cx,
                    B::init(),
                    ret_fail!(cx, "{}", "Test fail 1")
                ))))
            }
            fn init2(cx: CX![]) -> Option<Self> {
                Some(Self(Some(actor!(
                    cx,
                    B::init(),
                    ret_fail!(cx, Box::new(StringError("Test fail 2".into())))
                ))))
            }
            fn init3(cx: CX![]) -> Option<Self> {
                Some(Self(Some(actor!(
                    cx,
                    B::init(),
                    ret_failthru!(cx, "Test fail 3")
                ))))
            }
            fn init4(cx: CX![]) -> Option<Self> {
                Some(Self(Some(actor!(
                    cx,
                    B::init(),
                    ret_failthru!(cx, "{}", "Test fail 4")
                ))))
            }
            fn init5(cx: CX![]) -> Option<Self> {
                Some(Self(Some(actor!(
                    cx,
                    B::init(),
                    ret_failthru!(cx, Box::new(StringError("Test fail 5".into())))
                ))))
            }
            fn stop_b(&mut self, _: CX![]) {
                if let Some(ref b) = self.0 {
                    call!([b], stop());
                }
            }
            fn fail_b(&mut self, _: CX![]) {
                if let Some(ref b) = self.0 {
                    call!([b], fail());
                }
            }
            fn drop_b(&mut self, _: CX![]) {
                self.0.take();
            }
            fn kill_b(&mut self, _: CX![]) {
                if let Some(ref b) = self.0 {
                    kill!(b, "KILLED");
                }
            }
        }

        struct B;
        impl B {
            fn init(_: CX![]) -> Option<Self> {
                Some(Self)
            }
            fn stop(&self, cx: CX![]) {
                stop!(cx);
            }
            fn fail(&self, cx: CX![]) {
                fail!(cx, "FAILED");
            }
        }

        macro_rules! expect_fail {
        ([$($init:tt)+]; $call:ident(); $expect:expr) => {
            let a = actor!(s, $($init)+, ret_shutdown!(s));
            call!([a], $call());
            assert!(matches!(s.shutdown_reason(), None));
            s.run(now, false);
            match s.shutdown_reason() {
                Some(StopCause::Failed(e))  =>  assert_eq!($expect, format!("{}", e)),
                cause => panic!("Unexpected shutdown: {:?}", cause),
            }
        };
    }
        macro_rules! expect_okay {
        ([$($init:tt)+]; $call:ident()) => {
            let a = actor!(s, $($init)+, ret_shutdown!(s));
            call!([a], $call());
            assert!(matches!(s.shutdown_reason(), None));
            s.run(now, false);
            assert!(matches!(s.shutdown_reason(), None));
            drop(a);
            s.run(now, false);
            assert!(matches!(s.shutdown_reason(), Some(StopCause::Dropped)));
        };
    }

        // ret_fail! should fail A however B terminates
        expect_fail!([A::init0()]; stop_b(); "Test fail 0");
        expect_fail!([A::init0()]; fail_b(); "Test fail 0");
        expect_fail!([A::init0()]; drop_b(); "Test fail 0");
        expect_fail!([A::init0()]; kill_b(); "Test fail 0");

        expect_fail!([A::init1()]; stop_b(); "Test fail 1");
        expect_fail!([A::init1()]; fail_b(); "Test fail 1");
        expect_fail!([A::init1()]; drop_b(); "Test fail 1");
        expect_fail!([A::init1()]; kill_b(); "Test fail 1");

        expect_fail!([A::init2()]; stop_b(); "Test fail 2");
        expect_fail!([A::init2()]; fail_b(); "Test fail 2");
        expect_fail!([A::init2()]; drop_b(); "Test fail 2");
        expect_fail!([A::init2()]; kill_b(); "Test fail 2");

        // ret_failthru! should fail A only if B fails
        // TODO: Test ret_failthru! with StopCause::Lost
        expect_okay!([A::init3()]; stop_b());
        expect_fail!([A::init3()]; fail_b(); "Test fail 3");
        expect_okay!([A::init3()]; drop_b());
        expect_okay!([A::init3()]; kill_b());

        expect_okay!([A::init4()]; stop_b());
        expect_fail!([A::init4()]; fail_b(); "Test fail 4");
        expect_okay!([A::init4()]; drop_b());
        expect_okay!([A::init4()]; kill_b());

        expect_okay!([A::init5()]; stop_b());
        expect_fail!([A::init5()]; fail_b(); "Test fail 5");
        expect_okay!([A::init5()]; drop_b());
        expect_okay!([A::init5()]; kill_b());
    }
);

test_fn!(
    fn actor_in_slab() {
        struct Child;
        impl Child {
            fn init_to_stop(cx: CX![], wait: Duration) -> Option<Self> {
                after!(wait, [cx], |_, cx| stop!(cx));
                Some(Child)
            }
            fn init_to_fail(cx: CX![], wait: Duration) -> Option<Self> {
                after!(wait, [cx], |_, cx| fail!(cx, "Timeout"));
                Some(Child)
            }
        }
        struct Parent {
            children: ActorOwnSlab<Child>,
            fail_count: usize,
            stop_count: usize,
            counts: Vec<usize>,
        }
        impl Parent {
            fn init(_: CX![]) -> Option<Self> {
                Some(Self {
                    children: ActorOwnSlab::new(),
                    fail_count: 0,
                    stop_count: 0,
                    counts: Vec::new(),
                })
            }
            fn add_child_1(&mut self, cx: CX![], dur: Duration) {
                actor_in_slab!(self.children, cx, Child::init_to_stop(dur));
                self.counts.push(self.children.len());
            }
            fn add_child_2(&mut self, cx: CX![], dur: Duration) {
                actor_in_slab!(self.children, cx, <Child>::init_to_fail(dur));
                self.counts.push(self.children.len());
            }
            fn add_child_3(&mut self, cx: CX![], dur: Duration) {
                actor_in_slab!(
                    self.children,
                    cx,
                    Child::init_to_stop(dur),
                    ret_some_to!([cx], handle_cause() as (StopCause))
                );
                self.counts.push(self.children.len());
            }
            fn add_child_4(&mut self, cx: CX![], dur: Duration) {
                actor_in_slab!(
                    self.children,
                    cx,
                    <Child>::init_to_fail(dur),
                    ret_some_to!([cx], handle_cause() as (StopCause))
                );
                self.counts.push(self.children.len());
            }
            fn handle_cause(&mut self, _: CX![], cause: StopCause) {
                // This is only called for two of the tests
                self.counts.push(self.children.len());
                match cause {
                    StopCause::Stopped => self.stop_count += 1,
                    StopCause::Failed(_) => self.fail_count += 1,
                    _ => (),
                }
            }
            fn checks(&mut self, cx: CX![]) {
                assert_eq!(self.counts, vec![1, 2, 2, 3, 1, 0]);
                assert_eq!(self.stop_count, 1);
                assert_eq!(self.fail_count, 1);
                assert!(self.children.is_empty());
                stop!(cx);
            }
        }

        let mut now = Instant::now();
        let mut stakker = Stakker::new(now);
        let s = &mut stakker;

        let parent = actor!(s, Parent::init(), ret_shutdown!(s));
        const SEC: Duration = Duration::from_secs(1);
        call!([parent], add_child_1(1 * SEC)); // Spans 0-1s
        call!([parent], add_child_2(4 * SEC)); // Spans 0-4s
        after!(2 * SEC, [parent, s], add_child_3(5 * SEC)); // Spans 2-7s
        after!(3 * SEC, [parent, s], add_child_4(2 * SEC)); // Spans 3-5s
        after!(60 * SEC, [parent, s], checks());

        s.run(now, false);
        while s.not_shutdown() {
            now += s.next_wait_max(now, 60 * SEC, false);
            s.run(now, false);
        }
    }
);
