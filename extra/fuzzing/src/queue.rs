use stakker::*;
use std::time::Instant;

// The idea is to push random sequences of calls of different lengths
// onto the queue, interspersed with calls to run the queue.

pub fn fuzz_queue(data: &[u8]) {
    let now = Instant::now();
    let mut stakker = Stakker::new(now);
    let s = &mut stakker;

    let acc = actor!(s, Acc::init(), ret_nop![]);
    s.run(now, false);

    let mut expected = Vec::new();
    let mut seq = 1;
    for b in data {
        seq += 1;
        if *b < 4 {
            // If pure-random, would flush queue 1 in 64 bytes, which
            // biases queue-length to about 64*(8+(8+16+24+32)/4) ==
            // 1792.  So this helps the fuzzer start to explore around
            // the 1024-2048 range.
            s.run(now, false);
        } else {
            let n = *b & 15;
            expected.push(n);
            match n {
                0 => Acc::push_c0(&acc, seq),
                1 => Acc::push_c1(&acc, seq),
                2 => Acc::push_c2(&acc, seq),
                3 => Acc::push_c3(&acc, seq),
                4 => Acc::push_c4(&acc, seq),
                5 => Acc::push_c5(&acc, seq),
                6 => Acc::push_c6(&acc, seq),
                7 => Acc::push_c7(&acc, seq),
                8 => Acc::push_c8(&acc, seq),
                9 => Acc::push_c9(&acc, seq),
                10 => Acc::push_c10(&acc, seq),
                11 => Acc::push_c11(&acc, seq),
                12 => Acc::push_c12(&acc, seq),
                13 => Acc::push_c13(&acc, seq),
                14 => Acc::push_c14(&acc, seq),
                15 => Acc::push_c15(&acc, seq),
                _ => (),
            }
        }
    }
    s.run(now, false);

    // Check that all the calls executed, and in the correct order
    let done = acc
        .query(s, |this, _| std::mem::replace(&mut this.done, Vec::new()))
        .unwrap();
    assert_eq!(done, expected);
}

struct Acc {
    done: Vec<u8>,
}

macro_rules! def {
    (0, $name:ident, $push:ident) => {
        fn $name(&mut self, _: CX![]) {
            self.done.push(0);
        }
        fn $push(this: &Actor<Self>, _: u16) {
            call!([this], $name());
        }
    };
    ($len:expr, $name:ident, $push:ident) => {
        // Never inline either call, in order to force `n` to be
        // treated as a variable and actually store the array on the
        // queue (rather than risk inlining and specialising the whole
        // call)
        #[inline(never)]
        fn $name(&mut self, _: CX![], val: [u16; $len]) {
            self.done.push($len);
            if $len >= 2 {
                assert_eq!(val[0] + val[$len - 1], 44444);
            }
        }
        #[inline(never)]
        fn $push(this: &Actor<Self>, n: u16) {
            let mut v = [0; $len];
            if $len >= 2 {
                v[0] = n;
                v[$len - 1] = 44444 - n;
            } else {
                v[0] = 22222;
            }
            call!([this], $name(v));
        }
    };
}

impl Acc {
    fn init(_: CX![]) -> Option<Self> {
        Some(Self { done: Vec::new() })
    }

    def!(0, c0, push_c0);
    def!(1, c1, push_c1);
    def!(2, c2, push_c2);
    def!(3, c3, push_c3);
    def!(4, c4, push_c4);
    def!(5, c5, push_c5);
    def!(6, c6, push_c6);
    def!(7, c7, push_c7);
    def!(8, c8, push_c8);
    def!(9, c9, push_c9);
    def!(10, c10, push_c10);
    def!(11, c11, push_c11);
    def!(12, c12, push_c12);
    def!(13, c13, push_c13);
    def!(14, c14, push_c14);
    def!(15, c15, push_c15);
}
