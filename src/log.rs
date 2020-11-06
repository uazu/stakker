use std::error::Error;
use std::fmt::{Arguments, Display};
use std::ops::{BitOr, BitOrAssign};
use std::str::FromStr;

// TODO: Add "call-trace" feature to trace calls across actors,
// allowing a full backtrace to be generated later if necessary.  Uses
// a span for each call.  When call is queued: Open the span, with
// 'parent' set from Core::get_exec_id() if non-zero, and extra
// details such as 'after' to show how this was queued.  When
// execution starts: Log an event, and call Core::set_exec_id.  When
// execution stops: Close span and call Core::set_exec_id(0).  Most of
// this code needs to go in the macros, so add this feature when
// macros are rewritten as proc-macros.  `ret!` and `fwd!` are
// problematic as they don't have access to `cx`, so maybe also store
// the exec_id in the Deferrer when "call-trace" feature is enabled.

/// Trait used when visiting all the log-record's key-value pairs
///
/// This is designed as a superset of both JSON and the OpenTelemetry
/// `KeyValueList`.  Anything that cannot be represented with these
/// types must be logged as a string, a map or an array.
///
/// Notes:
///
/// - The `key` argument is normally `Some(key)`.  However where this
/// value is a member of an array the key must be `None`.
///
/// - All signed integer types from `i8` up to `i64` are passed to
/// `kv_i64`, and all unsigned integer types from `u8` up to `u64` are
/// passed to `kv_u64`.
///
/// - `kv_null` may be used to represent a key with presence but no
/// value, i.e. a JSON `null` or Rust unit `()` value.  It might be
/// converted into an empty-string or `true` value depending on the
/// logger if presence values are not supported downstream.
///
/// - Anything that needs to be formatted as a string, e.g. `Display`
/// or `Debug` types, is passed through to `kv_fmt` using
/// `format_args!`.  This way the logger can format it directly into
/// the output, avoiding the allocations that would occur if you used
/// `format!`.
///
/// - Maps are introduced with a call to `kv_map`, then zero or more
/// `kv_*` calls for the contents of that map, then a call to
/// `kv_mapend` to close the map and continue at the original level.
///
/// - Arrays are introduced with a call to `kv_arr`, then zero or more
/// `kv_*` calls for the contents of the array, then a call to
/// `kv_arrend` to close the array and continue at the original level.
/// The `key` argument on the `kv_*` calls within an array should be
/// `None`.
///
/// For maps and arrays, unlimited levels of nesting may be
/// represented.  The same key must be passed to both `kv_map` and
/// `kv_mapend` (or `kv_arr` and `kv_arrend`), for the convenience of
/// different kinds of logging output methods.
///
/// A logger might not support all these data types due to limitations
/// of the downstream format, so some data might get converted into
/// another form if it cannot be passed on as is.
///
/// ## No early termination
///
/// The visitor interface doesn't support terminating early with an
/// error.  This is intentional.  Other approaches were considered,
/// coded up and discarded: both `std::fmt`'s approach which
/// effectively returns `Result<(), ()>` and insists that the visitor
/// store the full error, and the approach used by `slog` which tries
/// to wrap various different types within its error type.  Given that
/// the non-error case will be most common, there is little advantage
/// in letting the visit terminate early.  So, as for `std::fmt`, if
/// the visitor encounters an error, it needs to store it itself.  It
/// will then generally choose to set things up so that any further
/// calls are ignored.  If that makes the code awkward, it can usually
/// be made manageable with a macro.
pub trait LogVisitor {
    fn kv_u64(&mut self, key: Option<&str>, val: u64);
    fn kv_i64(&mut self, key: Option<&str>, val: i64);
    fn kv_f64(&mut self, key: Option<&str>, val: f64);
    fn kv_bool(&mut self, key: Option<&str>, val: bool);
    fn kv_null(&mut self, key: Option<&str>);
    fn kv_str(&mut self, key: Option<&str>, val: &str);
    fn kv_fmt(&mut self, key: Option<&str>, val: &Arguments<'_>);
    fn kv_map(&mut self, key: Option<&str>);
    fn kv_mapend(&mut self, key: Option<&str>);
    fn kv_arr(&mut self, key: Option<&str>);
    fn kv_arrend(&mut self, key: Option<&str>);
}

/// Log record that is passed to a logger
pub struct LogRecord<'a> {
    /// Logging span identifier, or 0 for outside of a span
    pub id: LogID,
    /// Logging level
    pub level: LogLevel,
    /// Logging target, or ""
    pub target: &'a str,
    /// Freeform formatted text.  This can be output with any macro
    /// that accepts a format-string, e.g. `println!("{}", fmt)`.
    pub fmt: Arguments<'a>,
    /// Key-value pairs.  Call this function with your own
    /// [`LogVisitor`] and all the key-value pairs will be passed to
    /// that visitor in sequence.
    ///
    /// [`LogVisitor`]: trait.LogVisitor.html
    pub kvscan: &'a dyn Fn(&mut dyn LogVisitor),
}

/// Logging span identifier
///
/// A span is a period of time.  It is marked by a [`LogLevel::Open`]
/// logging event, any number of normal logging events and then a
/// [`LogLevel::Close`] event, all associated with the same [`LogID`].
/// It corresponds to the lifetime of an actor, or a call, or whatever
/// other process is being described.
///
/// The [`LogID`] numbers are allocated sequentially from one, with
/// zero reserved for "none" or "missing".  If 2^64 values are
/// allocated, they wrap around to one again.  If this would cause
/// problems downstream, then the logger should detect the situation
/// and warn or terminate the process.
///
/// [`LogID`]: type.LogID.html
/// [`LogLevel::Close`]: enum.LogLevel.html#variant.Close
/// [`LogLevel::Open`]: enum.LogLevel.html#variant.Open
pub type LogID = u64;

/// Levels for logging
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
#[non_exhaustive]
pub enum LogLevel {
    /// Tracing (very low priority or verbose logging)
    Trace = 0,

    /// Debugging (low priority logging)
    Debug = 1,

    /// Informational logging
    Info = 2,

    /// Warnings
    Warn = 3,

    /// Errors
    Error = 4,

    /// Logging level used to disable logging of severity-based
    /// logging levels.  Anything logged at this level will be ignored.
    Off = 8,

    /// Audit-level log-records are sets of key-value pairs that are
    /// intended for machine processing.  The formatted log-message
    /// should be a simple record tag, with all the variable data in
    /// key-value pairs.  This corresponds to trace events in
    /// OpenTelemetry, or what are called 'metrics' in some other
    /// systems.
    Audit = 5,

    /// Span open.  For an actor, this means actor startup.  The
    /// formatted text contains the name of the object, e.g. the actor
    /// name. If the parent logging-ID is known, it is passed as a
    /// `parent` key-value pair.
    Open = 6,

    /// Span close.  For an actor, this means actor termination.  The
    /// formatted text may give a description of why the span closed
    /// if there was a problem.  In case of actor failure, one of
    /// these presence key-values will be added: `failed`, `dropped`
    /// or `killed`.
    Close = 7,
}

impl LogLevel {
    /// Return the name of the [`LogLevel`] as a static string.
    ///
    /// [`LogLevel`]: enum.LogLevel.html
    pub fn name(self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
            Self::Off => "OFF",
            Self::Audit => "AUDIT",
            Self::Open => "OPEN",
            Self::Close => "CLOSE",
        }
    }

    /// Return a slice containing all defined logging levels
    pub fn all_levels() -> &'static [LogLevel] {
        &[
            Self::Trace,
            Self::Debug,
            Self::Info,
            Self::Warn,
            Self::Error,
            Self::Off,
            Self::Audit,
            Self::Open,
            Self::Close,
        ]
    }
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.name().fmt(f)
    }
}

impl FromStr for LogLevel {
    type Err = LogLevelError;

    /// This does a case-insensitive match to the level names as
    /// returned by [`LogLevel::name`]
    ///
    /// [`LogLevel::name`]: enum.LogLevel.html#method.name
    fn from_str(s: &str) -> Result<LogLevel, LogLevelError> {
        let s = s.trim();
        macro_rules! ret_if_matches {
            ($name:literal, $val:expr) => {
                if s.eq_ignore_ascii_case($name) {
                    return Ok($val);
                }
            };
        }
        if let Some(c) = s.as_bytes().first() {
            match c {
                b'T' | b't' => ret_if_matches!("TRACE", Self::Trace),
                b'D' | b'd' => ret_if_matches!("DEBUG", Self::Debug),
                b'I' | b'i' => ret_if_matches!("INFO", Self::Info),
                b'W' | b'w' => ret_if_matches!("WARN", Self::Warn),
                b'E' | b'e' => ret_if_matches!("ERROR", Self::Error),
                b'O' | b'o' => {
                    ret_if_matches!("OFF", Self::Off);
                    ret_if_matches!("OPEN", Self::Open);
                }
                b'A' | b'a' => ret_if_matches!("AUDIT", Self::Audit),
                b'C' | b'c' => ret_if_matches!("CLOSE", Self::Close),
                _ => (),
            }
        }
        Err(LogLevelError)
    }
}

/// Invalid [`LogLevel`] passed to [`LogLevel::from_str`]
///
/// [`LogLevel::from_str`]: enum.LogLevel.html#method.from_str
/// [`LogLevel`]: enum.LogLevel.html
#[derive(Debug, Eq, PartialEq)]
pub struct LogLevelError;
impl Error for LogLevelError {}
impl Display for LogLevelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        "invalid logging level".fmt(f)
    }
}

/// Filter for logging levels
///
/// This is a "copy" value which represents a set of enabled logging
/// levels.  Filters can be combined using the `|` and `|=` bit-or
/// operators.  A filter can be generated from a [`LogLevel`] using
/// [`LogFilter::from`].  Note that converting from a [`LogLevel`]
/// will also enable any related levels -- see [`LogFilter::from`]
/// documentation.
///
/// [`LogFilter::from`]: struct.LogFilter.html#method.from
/// [`LogLevel`]: enum.LogLevel.html
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct LogFilter(u32);

impl LogFilter {
    /// Return a [`LogFilter`] with no levels enabled
    ///
    /// [`LogFilter`]: struct.LogFilter.html
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Return a [`LogFilter`] with all the listed levels enabled, as
    /// converted by [`LogFilter::from`].
    ///
    /// [`LogFilter::from`]: struct.LogFilter.html#method.from
    /// [`LogFilter`]: struct.LogFilter.html
    #[inline]
    pub fn all(levels: &[LogLevel]) -> Self {
        let mut rv = Self::new();
        for level in levels {
            rv |= Self::from(*level);
        }
        rv
    }

    /// Test whether the given [`LogLevel`] is enabled
    ///
    /// [`LogLevel`]: enum.LogLevel.html
    #[inline]
    pub fn allows(&self, level: LogLevel) -> bool {
        0 != (self.0 & (1 << (level as u32)))
    }

    /// Test whether the set of enabled levels is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }
}

impl From<LogLevel> for LogFilter {
    /// Convert a [`LogLevel`] into a [`LogFilter`].  Where a severity
    /// level ([`LogLevel::Trace`] to [`LogLevel::Error`]) is passed,
    /// all higher severity levels are also enabled.  Where
    /// [`LogLevel::Open`] or [`LogLevel::Close`] is passed, the other
    /// is also enabled.  [`LogLevel::Audit`] only enables itself.
    /// [`LogLevel::Off`] gives no levels enabled.
    ///
    /// [`LogFilter`]: struct.LogFilter.html
    /// [`LogLevel::Audit`]: enum.LogLevel.html#variant.Audit
    /// [`LogLevel::Close`]: enum.LogLevel.html#variant.Close
    /// [`LogLevel::Error`]: enum.LogLevel.html#variant.Error
    /// [`LogLevel::Off`]: enum.LogLevel.html#variant.Off
    /// [`LogLevel::Open`]: enum.LogLevel.html#variant.Open
    /// [`LogLevel::Trace`]: enum.LogLevel.html#variant.Trace
    /// [`LogLevel`]: enum.LogLevel.html
    #[inline]
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace
            | LogLevel::Debug
            | LogLevel::Info
            | LogLevel::Warn
            | LogLevel::Error
            | LogLevel::Off => Self(0x1F & (0x1F << level as u32)),
            LogLevel::Audit => Self(1 << LogLevel::Audit as u32),
            LogLevel::Open | LogLevel::Close => {
                Self((1 << LogLevel::Open as u32) | (1 << LogLevel::Close as u32))
            }
        }
    }
}

impl BitOr for LogFilter {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for LogFilter {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl Display for LogFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        "LogFilter(".fmt(f)?;
        let mut first = true;
        for level in LogLevel::all_levels() {
            if 0 != (self.0 & (1 << *level as u32)) {
                if first {
                    first = false;
                } else {
                    ",".fmt(f)?;
                }
                level.name().fmt(f)?;
            }
        }
        ")".fmt(f)
    }
}

impl FromStr for LogFilter {
    type Err = LogLevelError;

    fn from_str(s: &str) -> Result<LogFilter, LogLevelError> {
        let mut rv = LogFilter::new();
        for level in s.split(',') {
            let level = LogLevel::from_str(level)?;
            rv |= LogFilter::from(level);
        }
        Ok(rv)
    }
}
