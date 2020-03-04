use super::{BoxedFnOnce, Duration, FnOnceQueue, Instant, MaxTimerKey, MinTimerKey, Timers};
const N_COUNTS: usize = 256;
struct Aux {
    start: Instant,
    now: Instant,
    queue: FnOnceQueue<Aux>,
    seed: u16,
    index: usize,
    counts: [u8; N_COUNTS],
}
impl Aux {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            start: now,
            now,
            queue: FnOnceQueue::new(),
            seed: 0x1234,
            index: 0,
            counts: [0; N_COUNTS],
        }
    }
    fn cb(&mut self) -> BoxedFnOnce<Aux> {
        let index = self.index;
        self.index = (index + 1) % N_COUNTS;
        Box::new(move |aux| aux.counts[index] += 1)
    }
    fn cb_check(&mut self, check: Instant) -> BoxedFnOnce<Aux> {
        let index = self.index;
        self.index = (index + 1) % N_COUNTS;
        Box::new(move |aux| {
            aux.counts[index] += 1;
            assert!(
                check <= aux.now,
                "Expected to run at {:?}, but now is {:?}",
                check,
                aux.now
            );
        })
    }
    fn cb_check_exact(&mut self, check: Instant) -> BoxedFnOnce<Aux> {
        let index = self.index;
        self.index = (index + 1) % N_COUNTS;
        Box::new(move |aux| {
            aux.counts[index] += 1;
            assert!(
                within_17us_after(check, Some(aux.now)),
                "Expected to run at {:?}, but now is {:?}",
                check,
                aux.now
            );
        })
    }
    fn advance(&mut self, t: &mut Timers<Aux>, now: Instant) {
        self.now = now;
        t.advance(now, &mut self.queue);
        let mut queue = std::mem::replace(&mut self.queue, FnOnceQueue::new());
        queue.execute(self);
        self.queue = queue;
    }
    fn rand(&mut self) -> u32 {
        // ZX Spectrum random number generator!
        self.seed = (((self.seed as u32) + 1) * 75 % 65537 - 1) as u16;
        self.seed as u32
    }
}

// We expect the expiry time to be within 17us after the time we
// requested, due to rounding up
fn within_17us_after(target: Instant, actual: Option<Instant>) -> bool {
    if let Some(actual) = actual {
        let e0 = target;
        let e1 = target + Duration::new(0, 16384);
        if e0 <= actual && actual <= e1 {
            return true;
        }
        println!("Out of range:");
        println!("  17us before: {:?}", e0);
        println!("  Actual:      {:?}", actual);
        println!("  Target:      {:?}", target);
    } else {
        println!("Out of range: Expiry was None");
    }
    false
}

#[test]
fn fixed_short_expire() {
    let mut aux = Aux::new();
    let mut t = Timers::new(aux.start);
    assert_eq!(None, t.next_expiry());
    let expire = aux.start + Duration::new(3, 123456789);
    t.add(expire, aux.cb_check(expire));
    assert!(within_17us_after(expire, t.next_expiry()));
    assert_eq!(0, t.slots_used());
    assert_eq!(0, aux.counts[0]);
    aux.advance(&mut t, aux.start + Duration::new(4, 0));
    assert_eq!(1, aux.counts[0]);
    assert_eq!(None, t.next_expiry());
}

#[test]
fn fixed_short_delete() {
    let mut aux = Aux::new();
    let mut t = Timers::new(aux.start);
    assert_eq!(None, t.next_expiry());
    let expire = aux.start + Duration::new(3, 123456789);
    let key = t.add(expire, aux.cb_check(expire));
    assert!(within_17us_after(expire, t.next_expiry()));
    assert_eq!(0, t.slots_used());
    assert_eq!(0, aux.counts[0]);
    assert!(t.del(key));
    assert_eq!(None, t.next_expiry());
    aux.advance(&mut t, aux.start + Duration::new(4, 0));
    assert_eq!(0, aux.counts[0]);
}

#[test]
fn fixed_long_expire() {
    let mut aux = Aux::new();
    let mut t = Timers::new(aux.start);
    assert_eq!(None, t.next_expiry());
    let expire = aux.start + Duration::new(36000, 567891234); // 10+ hours
    let expire1 = aux.start + Duration::new(32767, 0); // ~9 hours
    t.add(expire, aux.cb_check(expire));
    assert_eq!(1, t.slots_used());
    assert!(within_17us_after(expire1, t.next_expiry()));
    assert_eq!(0, aux.counts[0]);
    aux.advance(&mut t, aux.start + Duration::new(32768, 0));
    assert_eq!(1, t.slots_used());
    assert!(within_17us_after(expire, t.next_expiry()));
    assert_eq!(0, aux.counts[0]);
    aux.advance(&mut t, aux.start + Duration::new(40000, 0));
    assert_eq!(1, aux.counts[0]);
    assert_eq!(None, t.next_expiry());
    assert_eq!(0, t.slots_used());
}

#[test]
fn fixed_long_delete() {
    let mut aux = Aux::new();
    let mut t = Timers::new(aux.start);
    assert_eq!(None, t.next_expiry());
    let expire = aux.start + Duration::new(36000, 987654321); // 10+ hours
    let expire1 = aux.start + Duration::new(32767, 0); // ~9 hours
    let key = t.add(expire, aux.cb_check(expire));
    assert_eq!(1, t.slots_used());
    assert!(within_17us_after(expire1, t.next_expiry()));
    assert_eq!(0, aux.counts[0]);
    assert!(t.del(key));
    assert_eq!(0, t.slots_used());
    assert_eq!(None, t.next_expiry());
    assert_eq!(0, aux.counts[0]);
    aux.advance(&mut t, aux.start + Duration::new(40000, 0));
    assert_eq!(0, aux.counts[0]);
}

#[test]
fn max_expire() {
    let mut aux = Aux::new();
    let mut t = Timers::new(aux.start);
    let expire1 = aux.start + Duration::new(20000, 567891234);
    let expire2 = aux.start + Duration::new(30000, 123456789);
    let key = MaxTimerKey::default();
    assert!(!t.mod_max(key, expire1));
    assert!(!t.max_is_active(key));
    let key = t.add_max(expire1, aux.cb());
    assert!(t.max_is_active(key));
    assert_eq!(1, t.slots_used());
    assert!(within_17us_after(expire1, t.next_expiry()));
    assert_eq!(0, aux.counts[0]);
    assert!(t.mod_max(key, expire2));
    let key2 = t.add_max(expire2, aux.cb());
    assert!(t.max_is_active(key2));
    assert_eq!(2, t.slots_used());
    aux.advance(&mut t, aux.start + Duration::new(22000, 0));
    assert!(t.max_is_active(key));
    assert_eq!(0, aux.counts[0]);
    assert_eq!(0, aux.counts[1]);
    assert!(within_17us_after(expire2, t.next_expiry()));
    assert!(t.del_max(key2));
    assert!(!t.max_is_active(key2));
    assert_eq!(1, t.slots_used());
    aux.advance(&mut t, aux.start + Duration::new(32000, 0));
    assert!(!t.max_is_active(key));
    assert_eq!(1, aux.counts[0]);
    assert_eq!(0, aux.counts[1]);
    assert_eq!(0, t.slots_used());
    assert_eq!(None, t.next_expiry());
}

#[test]
fn min_expire() {
    let mut aux = Aux::new();
    let mut t = Timers::new(aux.start);
    let expire1 = aux.start + Duration::new(30000, 567891234);
    let expire2 = aux.start + Duration::new(20000, 123456789);
    let key = MinTimerKey::default();
    assert!(!t.min_is_active(key));
    assert!(!t.mod_min(key, expire1));
    let key = t.add_min(expire1, aux.cb());
    assert!(t.min_is_active(key));
    assert_eq!(1, t.slots_used());
    assert_eq!(0, aux.counts[0]);
    assert!(t.mod_min(key, expire2));
    let key2 = t.add_min(expire1, aux.cb());
    assert!(t.min_is_active(key2));
    assert_eq!(2, t.slots_used());
    aux.advance(&mut t, aux.start + Duration::new(22000, 0));
    assert!(!t.min_is_active(key));
    assert!(t.min_is_active(key2));
    assert_eq!(1, aux.counts[0]);
    assert_eq!(0, aux.counts[1]);
    assert_eq!(1, t.slots_used());
    assert!(t.del_min(key2));
    assert_eq!(0, t.slots_used());
    assert!(!t.min_is_active(key2));
    assert_eq!(None, t.next_expiry());
    aux.advance(&mut t, aux.start + Duration::new(32000, 0));
    assert_eq!(1, aux.counts[0]);
    assert_eq!(0, aux.counts[1]);
}

#[test]
fn min_asymptote() {
    let mut aux = Aux::new();
    let mut t = Timers::new(aux.start);
    let expiry = aux.start + Duration::from_secs(60);
    t.add_min(expiry, aux.cb());
    // Expect this to go roughly (T-15, T-4, T-1, T-0.125, T)
    for _ in 1..=5 {
        assert_eq!(0, aux.counts[0]);
        let exp = t.next_expiry().unwrap();
        //println!("{:?}", expiry - exp);
        aux.advance(&mut t, exp);
    }
    assert_eq!(None, t.next_expiry());
    assert_eq!(1, aux.counts[0]);
}

#[test]
fn min_rand() {
    // Set up a large number of min timers, and check that they
    // all expire correctly.
    let mut aux = Aux::new();
    let mut t = Timers::new(aux.start);
    let unit = Duration::from_secs_f64(60.0 / 65535.0);
    for _ in 0..N_COUNTS {
        let expire = aux.now + aux.rand() * unit;
        t.add_min(expire, aux.cb_check_exact(expire));
    }
    while let Some(exp) = t.next_expiry() {
        aux.advance(&mut t, exp);
    }
    for i in 0..N_COUNTS {
        assert_eq!(aux.counts[i], 1);
    }
}

#[test]
fn rand_queue() {
    // Pseudo-random addition and expiry of min/max/fixed timers
    // to exercise internals.  Aims to keep timer queue level at
    // around 50.
    let level = 50.0;
    let mut aux = Aux::new();
    let mut t = Timers::new(aux.start);
    let unit = Duration::from_secs_f64(54321.987654321 / 65535.0);
    let unit_adv = Duration::from_secs_f64(unit.as_secs_f64() / level);
    for _ in 0..N_COUNTS {
        let expire = aux.now + aux.rand() * unit;
        // Mostly fixed, with some min/max mixed in
        match aux.rand() % 7 {
            0 => {
                t.add_min(expire, aux.cb_check(expire));
            }
            1 => {
                t.add_max(expire, aux.cb_check(expire));
            }
            _ => {
                t.add(expire, aux.cb_check(expire));
            }
        }
        let adv = aux.rand() * unit_adv;
        aux.advance(&mut t, aux.now + adv);
        //println!("Queue level: {}", t.queue.len());
    }
    while let Some(exp) = t.next_expiry() {
        aux.advance(&mut t, exp);
    }
    for i in 0..N_COUNTS {
        assert_eq!(aux.counts[i], 1);
    }
}

#[test]
fn rand_queue_exact() {
    // Pseudo-random addition and expiry of fixed timers, checking
    // exact end-time.
    let mut aux = Aux::new();
    let mut t = Timers::new(aux.start);
    let unit = Duration::from_secs_f64(54321.987654321 / 65535.0);
    for _ in 0..N_COUNTS {
        let rand = aux.rand();
        let expire = aux.now + rand * unit;
        t.add(expire, aux.cb_check_exact(expire));
        if 0 != (rand & 1) {
            if let Some(exp) = t.next_expiry() {
                aux.advance(&mut t, exp);
            }
        }
    }
    while let Some(exp) = t.next_expiry() {
        aux.advance(&mut t, exp);
    }
    for i in 0..N_COUNTS {
        assert_eq!(aux.counts[i], 1);
    }
}
