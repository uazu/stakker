use crate::*;
use std::time::Instant;

#[test]
fn share() {
    let now = Instant::now();
    let mut stakker = Stakker::new(now);
    let s = &mut stakker;

    let s1 = Share::new(s, 12345_u32);
    assert_eq!(*s1.ro(s), 12345);

    *s1.rw(s) += 11111;
    assert_eq!(*s1.ro(s), 23456);

    let s2 = Share::new(s, 98765_u32);
    assert_eq!(*s2.ro(s), 98765);

    let (p1, p2) = s.share_rw2(&s1, &s2);
    std::mem::swap(p1, p2);
    assert_eq!(*s1.ro(s), 98765);
    assert_eq!(*s2.ro(s), 23456);

    // Use a different type to check share_rw3 doesn't mind
    let s3 = Share::new(s, 13579_u64);
    assert_eq!(*s3.ro(s), 13579);

    let (p1, p2, p3) = s.share_rw3(&s1, &s2, &s3);
    *p3 += (*p2 * 3 + *p1) as u64;
    assert_eq!(*s3.ro(s), 13579 + 23456 * 3 + 98765);
}
