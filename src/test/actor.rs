use crate::actor::StringError;
use crate::*;
use std::time::Instant;

#[test]
fn actor_termination() {
    let now = Instant::now();
    let mut stakker = Stakker::new(now);
    let s = &mut stakker;

    struct Test;
    impl Test {
        fn init_stop(cx: CX![]) -> Option<Self> {
            cx.stop();
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
        fn init(_cx: CX![]) -> Option<Self> {
            Some(Self)
        }
        fn do_stop(&mut self, cx: CX![]) {
            cx.stop();
        }
        fn do_fail(&mut self, cx: CX![]) {
            cx.fail(Box::new(StringError("TEST fail".into())));
        }
        fn do_fail_str(&mut self, cx: CX![]) {
            cx.fail_str("TEST fail_str");
        }
        fn query(&mut self, _cx: CX![]) -> u32 {
            12345
        }
    }

    // Test stop from init
    let _a = actor!(s, Test::init_stop(), ret_shutdown!(s));
    s.run(now, false);
    assert!(matches!(s.shutdown_reason(), Some(StopCause::Stopped)));

    // Test fail from init
    let _a = actor!(s, Test::init_fail(), ret_shutdown!(s));
    s.run(now, false);
    match s.shutdown_reason() {
        Some(StopCause::Failed(e)) => {
            assert_eq!("TEST fail", format!("{}", e));
        }
        cause => panic!("Unexpected shutdown reason: {:?}", cause),
    }

    // Test fail_str from init
    let _a = actor!(s, Test::init_fail_str(), ret_shutdown!(s));
    assert!(matches!(s.shutdown_reason(), None));
    s.run(now, false);
    match s.shutdown_reason() {
        Some(StopCause::Failed(e)) => {
            assert_eq!("TEST fail_str", format!("{}", e));
        }
        cause => panic!("Unexpected shutdown reason: {:?}", cause),
    }

    // Test stop from method
    let a = actor!(s, Test::init(), ret_shutdown!(s));
    call!([a], do_stop());
    assert!(matches!(s.shutdown_reason(), None));
    s.run(now, false);
    assert!(matches!(s.shutdown_reason(), Some(StopCause::Stopped)));

    // Test fail from method
    let a = actor!(s, Test::init(), ret_shutdown!(s));
    call!([a], do_fail());
    assert!(matches!(s.shutdown_reason(), None));
    s.run(now, false);
    match s.shutdown_reason() {
        Some(StopCause::Failed(e)) => {
            assert_eq!("TEST fail", format!("{}", e));
        }
        cause => panic!("Unexpected shutdown: {:?}", cause),
    }

    // Test fail_str from method
    let a = actor!(s, Test::init(), ret_shutdown!(s));
    call!([a], do_fail_str());
    assert!(matches!(s.shutdown_reason(), None));
    s.run(now, false);
    match s.shutdown_reason() {
        Some(StopCause::Failed(e)) => {
            assert_eq!("TEST fail_str", format!("{}", e));
        }
        cause => panic!("Unexpected shutdown: {:?}", cause),
    }

    // Test kill_str
    let a = actor!(s, Test::init(), ret_shutdown!(s));
    a.kill_str(s, "TEST kill_str");
    assert!(matches!(s.shutdown_reason(), None));
    s.run(now, false);
    match s.shutdown_reason() {
        Some(StopCause::Killed(e)) => {
            assert_eq!("TEST kill_str", format!("{}", e));
        }
        cause => panic!("Unexpected shutdown: {:?}", cause),
    }

    // Test kill
    let a = actor!(s, Test::init(), ret_shutdown!(s));
    a.kill(s, Box::new(StringError("TEST kill".into())));
    assert!(matches!(s.shutdown_reason(), None));
    s.run(now, false);
    match s.shutdown_reason() {
        Some(StopCause::Killed(e)) => {
            assert_eq!("TEST kill", format!("{}", e));
        }
        cause => panic!("Unexpected shutdown: {:?}", cause),
    }

    // Test drop
    let a = actor!(s, Test::init(), ret_shutdown!(s));
    drop(a);
    assert!(matches!(s.shutdown_reason(), None));
    s.run(now, false);
    assert!(matches!(s.shutdown_reason(), Some(StopCause::Dropped)));

    // Test fail in query
    let g = actor!(s, Test::init(), ret_shutdown!(s));
    assert_eq!(g.query(s, |this, cx| this.query(cx)), None);
    s.run(now, false);
    assert!(matches!(s.shutdown_reason(), None));
    assert_eq!(g.query(s, |this, cx| this.query(cx)), Some(12345));
    s.run(now, false);
    assert!(matches!(s.shutdown_reason(), None));
    assert_eq!(g.query(s, |this, cx| this.do_stop(cx)), Some(()));
    s.run(now, false);
    assert!(matches!(s.shutdown_reason(), Some(StopCause::Stopped)));
}

#[test]
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
