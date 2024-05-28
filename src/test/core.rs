use crate::*;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

test_fn!(
    fn test_times() {
        let t0 = Instant::now();
        let t1 = t0 + Duration::from_secs(1);
        let s0 = SystemTime::now();
        let s1 = s0 + Duration::from_secs(2);

        let mut stakker = Stakker::new(t0);
        let s = &mut stakker;
        s.set_systime(Some(s0));

        assert_eq!(t0, s.start_instant());
        assert_eq!(t0, s.now());
        assert_eq!(s0, s.systime());

        s.set_systime(Some(s1));
        s.run(t1, false);
        assert_eq!(t0, s.start_instant());
        assert_eq!(t1, s.now());
        assert_eq!(s1, s.systime());
    }
);

test_fn!(
    fn anymap() {
        let mut stakker = Stakker::new(Instant::now());
        let s = &mut stakker;

        s.anymap_set(Rc::new(42u8));
        s.anymap_set(Rc::new(-17i8));
        assert_eq!(42, *s.anymap_get::<Rc<u8>>());
        assert_eq!(-17, *s.anymap_get::<Rc<i8>>());

        s.anymap_set(Rc::new(24u8));
        assert_eq!(24, *s.anymap_get::<Rc<u8>>());
        assert_eq!(-17, *s.anymap_get::<Rc<i8>>());

        s.anymap_unset::<Rc<u8>>();
        assert_eq!(None, s.anymap_try_get::<Rc<u8>>());
        assert_eq!(-17, *s.anymap_get::<Rc<i8>>());

        assert_eq!(None, s.anymap_try_get::<Rc<u16>>());
        assert_eq!(None, s.anymap_try_get::<Rc<i16>>());
    }
);
