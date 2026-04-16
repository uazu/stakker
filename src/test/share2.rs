use crate::time::Instant;
use crate::*;

test_fn!(
    fn share2() {
        let now = Instant::now();
        let mut stakker = Stakker::new(now);
        let s = &mut stakker;

        struct Test;
        impl Test {
            fn init(cx: CX![]) -> Option<Self> {
                let s1 = Share2::new(cx, 12345_u32);
                assert_eq!(*s1.rw(cx).0, 12345);

                *s1.rw(cx).0 += 11111;
                assert_eq!(*s1.rw(cx).0, 23456);

                let s2 = Share2::new(cx, 98765_u32);
                assert_eq!(*s2.rw(cx).0, 98765);

                let (p1, p2, _core) = cx.share2_rw2(&s1, &s2);
                std::mem::swap(p1, p2);
                assert_eq!(*s1.rw(cx).0, 98765);
                assert_eq!(*s2.rw(cx).0, 23456);

                // Use a different type to check share2_rw3 doesn't mind
                let s3 = Share2::new(cx, 13579_u64);
                assert_eq!(*s3.rw(cx).0, 13579);

                let (p1, p2, p3, _core) = cx.share2_rw3(&s1, &s2, &s3);
                *p3 += (*p2 * 3 + *p1) as u64;
                assert_eq!(*s3.rw(cx).0, 13579 + 23456 * 3 + 98765);
                stop!(cx);
                None
            }
        }

        let a = actor!(s, Test::init(), ret_shutdown!(s));
        s.run(now, false);
        drop(a);
        assert!(matches!(s.shutdown_reason(), Some(StopCause::Stopped)));
    }
);

test_fn!(
    fn share2_weak() {
        let now = Instant::now();
        let mut stakker = Stakker::new(now);
        let s = &mut stakker;

        struct Test;
        impl Test {
            fn init(cx: CX![]) -> Option<Self> {
                let s1 = Share2::new(cx, 12345_u32);
                assert_eq!(*s1.rw(cx).0, 12345);
                assert_eq!(s1.strong_count(), 1);
                assert_eq!(s1.weak_count(), 0);

                let w1 = s1.downgrade();
                assert_eq!(s1.strong_count(), 1);
                assert_eq!(s1.weak_count(), 1);
                assert_eq!(w1.strong_count(), 1);
                assert_eq!(w1.weak_count(), 1);
                assert_eq!(*w1.upgrade().unwrap().rw(cx).0, 12345);

                drop(s1);
                assert_eq!(w1.strong_count(), 0);
                assert!(w1.upgrade().is_none());

                stop!(cx);
                None
            }
        }

        let a = actor!(s, Test::init(), ret_shutdown!(s));
        s.run(now, false);
        drop(a);
        assert!(matches!(s.shutdown_reason(), Some(StopCause::Stopped)));
    }
);
