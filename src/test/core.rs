use crate::*;
use std::time::{Duration, Instant, SystemTime};

#[test]
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
