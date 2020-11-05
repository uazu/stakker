use crate::*;
use std::str::FromStr;

#[test]
fn loglevels() {
    macro_rules! test_conv {
        ($level:ident, $str1:literal, $str2:literal, $str3:literal) => {
            assert_eq!(Ok(LogLevel::$level), LogLevel::from_str($str1));
            assert_eq!(Ok(LogLevel::$level), LogLevel::from_str($str2));
            assert_eq!(Ok(LogLevel::$level), LogLevel::from_str($str3));
            assert_eq!(format!("{}", LogLevel::$level), $str2);
            assert_eq!(LogLevel::$level.name(), $str2);
        };
    }

    test_conv!(Trace, "trace", "TRACE", "Trace");
    test_conv!(Debug, "debug", "DEBUG", "Debug");
    test_conv!(Info, "info", "INFO", "Info");
    test_conv!(Warn, "warn", "WARN", "Warn");
    test_conv!(Error, "error", "ERROR", "Error");
    test_conv!(Off, "off", "OFF", "Off");
    test_conv!(Audit, "audit", "AUDIT", "Audit");
    test_conv!(Open, "open", "OPEN", "Open");
    test_conv!(Close, "close", "CLOSE", "Close");

    assert_eq!(Err(LogLevelError), LogLevel::from_str("zzzzz"));
    assert_eq!(format!("{}", LogLevelError), "invalid logging level");

    for level in LogLevel::all_levels() {
        assert_eq!(LogLevel::from_str(level.name()), Ok(*level));
    }
}

#[test]
fn logfilter() {
    macro_rules! test_filter {
        ($level:ident, $expect:literal) => {
            assert_eq!(format!("{}", LogFilter::from(LogLevel::$level)), $expect);
        };
    }

    test_filter!(Trace, "LogFilter(TRACE,DEBUG,INFO,WARN,ERROR)");
    test_filter!(Debug, "LogFilter(DEBUG,INFO,WARN,ERROR)");
    test_filter!(Info, "LogFilter(INFO,WARN,ERROR)");
    test_filter!(Warn, "LogFilter(WARN,ERROR)");
    test_filter!(Error, "LogFilter(ERROR)");
    test_filter!(Off, "LogFilter()");
    test_filter!(Audit, "LogFilter(AUDIT)");
    test_filter!(Open, "LogFilter(OPEN,CLOSE)");
    test_filter!(Close, "LogFilter(OPEN,CLOSE)");

    macro_rules! test_fromstr {
        ($in:literal, $expect:literal) => {
            assert_eq!(format!("{}", LogFilter::from_str($in).unwrap()), $expect);
        };
    }

    test_fromstr!("warn", "LogFilter(WARN,ERROR)");
    test_fromstr!("info,audit", "LogFilter(INFO,WARN,ERROR,AUDIT)");
    test_fromstr!("error,open,audit", "LogFilter(ERROR,AUDIT,OPEN,CLOSE)");

    macro_rules! test_all {
        ($all:expr, $expect:literal) => {
            assert_eq!(format!("{}", LogFilter::all(&$all)), $expect);
        };
    }

    test_all!([LogLevel::Warn], "LogFilter(WARN,ERROR)");
    test_all!(
        [LogLevel::Info, LogLevel::Audit],
        "LogFilter(INFO,WARN,ERROR,AUDIT)"
    );
    test_all!(
        [LogLevel::Error, LogLevel::Open, LogLevel::Audit],
        "LogFilter(ERROR,AUDIT,OPEN,CLOSE)"
    );

    assert_eq!(
        LogFilter::new() | LogFilter::from(LogLevel::Audit) | LogFilter::from(LogLevel::Error),
        LogFilter::from_str("audit,error").unwrap()
    );

    assert_eq!(true, LogFilter::new().is_empty());
    assert_eq!(false, LogFilter::from(LogLevel::Audit).is_empty());
}

#[cfg(feature = "logger")]
#[test]
fn logger() {
    use std::fmt::Arguments;
    use std::time::Instant;

    let now = Instant::now();
    let mut stakker = Stakker::new(now);
    let s = &mut stakker;

    struct A;
    impl A {
        fn init(_: CX![]) -> Option<Self> {
            Some(Self)
        }
        fn warn(&self, cx: CX![]) {
            let id = cx.id();
            cx.log(
                id,
                LogLevel::Warn,
                "target",
                format_args!("Warning"),
                |out| out.kv_i64(Some("num"), 1234),
            );
        }
        fn fail(&self, cx: CX![]) {
            fail!(cx, "Called A::fail");
        }
    }

    // Don't test the LogVisitor interface fully here.  Leave that for
    // a logging crate.
    #[derive(Default)]
    struct CheckVisitor {
        found_num_i64: bool,
        found_failed_null: bool,
    }
    impl LogVisitor for CheckVisitor {
        fn kv_u64(&mut self, key: Option<&str>, _val: u64) {
            panic!("unexpected kv_u64: {:?}", key);
        }
        fn kv_i64(&mut self, key: Option<&str>, val: i64) {
            assert_eq!(key, Some("num"));
            assert_eq!(val, 1234);
            self.found_num_i64 = true;
        }
        fn kv_f64(&mut self, key: Option<&str>, _val: f64) {
            panic!("unexpected kv_f64: {:?}", key);
        }
        fn kv_bool(&mut self, key: Option<&str>, _val: bool) {
            panic!("unexpected kv_bool: {:?}", key);
        }
        fn kv_null(&mut self, key: Option<&str>) {
            assert_eq!(key, Some("failed"));
            self.found_failed_null = true;
        }
        fn kv_str(&mut self, key: Option<&str>, _val: &str) {
            panic!("unexpected kv_str: {:?}", key);
        }
        fn kv_fmt(&mut self, key: Option<&str>, _val: &Arguments<'_>) {
            panic!("unexpected kv_fmt: {:?}", key);
        }
        fn kv_map(&mut self, key: Option<&str>) {
            panic!("unexpected kv_map: {:?}", key);
        }
        fn kv_mapend(&mut self, key: Option<&str>) {
            panic!("unexpected kv_mapend: {:?}", key);
        }
        fn kv_arr(&mut self, key: Option<&str>) {
            panic!("unexpected kv_arr: {:?}", key);
        }
        fn kv_arrend(&mut self, key: Option<&str>) {
            panic!("unexpected kv_arrend: {:?}", key);
        }
    }

    let mut expect = 0;
    s.set_logger(
        LogFilter::all(&[LogLevel::Warn, LogLevel::Audit, LogLevel::Open]),
        move |core, r| {
            assert_eq!(r.id, 1);
            expect += 1;
            match expect {
                1 => {
                    assert_eq!(r.level, LogLevel::Open);
                    assert_eq!(r.target, "");
                    assert_eq!(format!("{}", r.fmt), "stakker::test::log::logger::A");
                }
                2 => {
                    assert_eq!(r.level, LogLevel::Warn);
                    assert_eq!(r.target, "target");
                    assert_eq!(format!("{}", r.fmt), "Warning");
                    let mut visitor = CheckVisitor::default();
                    (r.kvscan)(&mut visitor);
                    assert_eq!(true, visitor.found_num_i64);
                    assert_eq!(false, visitor.found_failed_null);
                }
                _ => {
                    assert_eq!(r.level, LogLevel::Close);
                    assert_eq!(r.target, "");
                    assert_eq!(format!("{}", r.fmt), "Called A::fail");
                    let mut visitor = CheckVisitor::default();
                    (r.kvscan)(&mut visitor);
                    assert_eq!(false, visitor.found_num_i64);
                    assert_eq!(true, visitor.found_failed_null);
                    core.shutdown(StopCause::Stopped);
                }
            }
        },
    );

    let a = actor!(s, A::init(), ret_nop!());
    call!([a], warn());
    call!([a], fail());
    s.run(now, false);
    assert!(matches!(s.shutdown_reason(), Some(StopCause::Stopped)));
}
