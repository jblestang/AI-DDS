//! # Time — Duration and Timestamp types for DDS
//!
//! DDS uses two time-related types throughout its APIs:
//! - `Duration` — a relative time interval (used in QoS policies, timeouts)
//! - `Timestamp` — an absolute wall-clock time (used in sample metadata)
//!
//! Both are represented as (seconds, fraction) pairs on the wire, matching
//! the RTPS Time_t structure.
//!
//! Reference: DCPS §2.2.1, RTPS §8.2.4.5

use std::fmt;
use std::ops::{Add, Sub};
use std::time;

// ──────────────────────────────────────────────────────────────────────────────
// Duration — Relative time interval (DCPS §2.2.1)
// ──────────────────────────────────────────────────────────────────────────────

/// A relative time interval, used in QoS policies and timeouts.
///
/// Stored as seconds + nanoseconds, where nanoseconds is always < 1_000_000_000.
/// Special sentinel values `INFINITE` and `ZERO` are provided.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Duration {
    /// Whole seconds component.
    pub seconds: i32,
    /// Sub-second component in nanoseconds (0..999_999_999).
    pub nanoseconds: u32,
}

/// Maximum valid nanosecond value (exclusive bound).
const NANOS_PER_SEC: u32 = 1_000_000_000;

impl Duration {
    /// An infinite duration — used as default for many QoS timeouts.
    pub const INFINITE: Self = Self {
        seconds: i32::MAX,
        nanoseconds: u32::MAX,
    };

    /// A zero-length duration.
    pub const ZERO: Self = Self {
        seconds: 0,
        nanoseconds: 0,
    };

    /// Create a duration from seconds and nanoseconds.
    ///
    /// # Panics
    /// Panics if `nanoseconds >= 1_000_000_000` (except for `INFINITE`).
    #[must_use]
    pub fn new(seconds: i32, nanoseconds: u32) -> Self {
        // Allow the special INFINITE sentinel through
        if seconds == i32::MAX && nanoseconds == u32::MAX {
            return Self::INFINITE;
        }
        assert!(
            nanoseconds < NANOS_PER_SEC,
            "nanoseconds must be < 1_000_000_000, got {nanoseconds}"
        );
        Self {
            seconds,
            nanoseconds,
        }
    }

    /// Create a duration from whole seconds.
    #[must_use]
    pub const fn from_secs(seconds: i32) -> Self {
        Self {
            seconds,
            nanoseconds: 0,
        }
    }

    /// Create a duration from milliseconds.
    #[must_use]
    pub const fn from_millis(millis: i64) -> Self {
        let seconds = (millis / 1000) as i32;
        let nanoseconds = ((millis % 1000) * 1_000_000) as u32;
        Self {
            seconds,
            nanoseconds,
        }
    }

    /// Check whether this is the infinite sentinel duration.
    #[must_use]
    pub const fn is_infinite(&self) -> bool {
        self.seconds == i32::MAX && self.nanoseconds == u32::MAX
    }

    /// Check whether this is a zero-length duration.
    #[must_use]
    pub const fn is_zero(&self) -> bool {
        self.seconds == 0 && self.nanoseconds == 0
    }

    /// Convert to a `std::time::Duration`. Returns `None` for negative
    /// durations or the INFINITE sentinel.
    #[must_use]
    pub fn to_std(&self) -> Option<time::Duration> {
        if self.is_infinite() || self.seconds < 0 {
            return None;
        }
        Some(time::Duration::new(self.seconds as u64, self.nanoseconds))
    }

    /// Create from a `std::time::Duration`.
    ///
    /// Returns `INFINITE` if the std duration exceeds i32::MAX seconds.
    #[must_use]
    pub fn from_std(d: time::Duration) -> Self {
        if d.as_secs() > i32::MAX as u64 {
            return Self::INFINITE;
        }
        Self {
            seconds: d.as_secs() as i32,
            nanoseconds: d.subsec_nanos(),
        }
    }
}

impl fmt::Debug for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_infinite() {
            write!(f, "Duration(INFINITE)")
        } else {
            write!(f, "Duration({}.{:09}s)", self.seconds, self.nanoseconds)
        }
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_infinite() {
            write!(f, "∞")
        } else {
            write!(f, "{}.{:09}s", self.seconds, self.nanoseconds)
        }
    }
}

impl Default for Duration {
    /// Default duration is INFINITE (matches most DDS QoS defaults).
    fn default() -> Self {
        Self::INFINITE
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Timestamp — Absolute wall-clock time (RTPS §8.2.4.5)
// ──────────────────────────────────────────────────────────────────────────────

/// An absolute point in time, typically representing when a sample was written.
///
/// Uses the same (seconds, nanoseconds) representation as `Duration`, but
/// with `u32` seconds representing seconds since the Unix epoch (or a
/// middleware-defined epoch).
///
/// Reference: RTPS §8.2.4.5 (Time_t)
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    /// Seconds since epoch.
    pub seconds: u32,
    /// Sub-second component in nanoseconds (0..999_999_999).
    pub nanoseconds: u32,
}

impl Timestamp {
    /// Invalid/unknown timestamp — sentinel value.
    pub const INVALID: Self = Self {
        seconds: u32::MAX,
        nanoseconds: u32::MAX,
    };

    /// The epoch itself (time zero).
    pub const ZERO: Self = Self {
        seconds: 0,
        nanoseconds: 0,
    };

    /// Create a timestamp from seconds and nanoseconds.
    ///
    /// # Panics
    /// Panics if `nanoseconds >= 1_000_000_000` (except for INVALID).
    #[must_use]
    pub fn new(seconds: u32, nanoseconds: u32) -> Self {
        if seconds == u32::MAX && nanoseconds == u32::MAX {
            return Self::INVALID;
        }
        assert!(
            nanoseconds < NANOS_PER_SEC,
            "nanoseconds must be < 1_000_000_000, got {nanoseconds}"
        );
        Self {
            seconds,
            nanoseconds,
        }
    }

    /// Check whether this is the invalid sentinel timestamp.
    #[must_use]
    pub const fn is_invalid(&self) -> bool {
        self.seconds == u32::MAX && self.nanoseconds == u32::MAX
    }

    /// Get the current wall-clock time as a `Timestamp`.
    ///
    /// Uses `std::time::SystemTime::now()` internally.
    #[must_use]
    pub fn now() -> Self {
        let since_epoch = time::SystemTime::now()
            .duration_since(time::UNIX_EPOCH)
            .unwrap_or_default();
        Self {
            seconds: since_epoch.as_secs() as u32,
            nanoseconds: since_epoch.subsec_nanos(),
        }
    }
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_invalid() {
            write!(f, "Timestamp(INVALID)")
        } else {
            write!(f, "Timestamp({}.{:09})", self.seconds, self.nanoseconds)
        }
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_invalid() {
            write!(f, "INVALID")
        } else {
            write!(f, "{}.{:09}", self.seconds, self.nanoseconds)
        }
    }
}

impl Default for Timestamp {
    /// Default timestamp is INVALID (no timestamp set).
    fn default() -> Self {
        Self::INVALID
    }
}

/// Add a Duration to a Timestamp, producing a new Timestamp.
impl Add<Duration> for Timestamp {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self {
        if self.is_invalid() || rhs.is_infinite() {
            return Self::INVALID;
        }
        let total_nanos = self.nanoseconds + rhs.nanoseconds;
        let carry = total_nanos / NANOS_PER_SEC;
        let nanos = total_nanos % NANOS_PER_SEC;
        let secs = self
            .seconds
            .wrapping_add(rhs.seconds as u32)
            .wrapping_add(carry);
        Self {
            seconds: secs,
            nanoseconds: nanos,
        }
    }
}

/// Subtract two Timestamps to get a Duration.
impl Sub for Timestamp {
    type Output = Duration;

    fn sub(self, rhs: Self) -> Duration {
        if self.is_invalid() || rhs.is_invalid() {
            return Duration::INFINITE;
        }
        let secs_diff = self.seconds as i64 - rhs.seconds as i64;
        let nanos_diff = self.nanoseconds as i64 - rhs.nanoseconds as i64;
        let total_nanos = secs_diff * NANOS_PER_SEC as i64 + nanos_diff;
        let seconds = (total_nanos / NANOS_PER_SEC as i64) as i32;
        let nanoseconds = (total_nanos.rem_euclid(NANOS_PER_SEC as i64)) as u32;
        Duration {
            seconds,
            nanoseconds,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Duration tests ──

    #[test]
    fn duration_zero() {
        let d = Duration::ZERO;
        assert!(d.is_zero());
        assert!(!d.is_infinite());
        assert_eq!(d.seconds, 0);
        assert_eq!(d.nanoseconds, 0);
    }

    #[test]
    fn duration_infinite() {
        let d = Duration::INFINITE;
        assert!(d.is_infinite());
        assert!(!d.is_zero());
    }

    #[test]
    fn duration_from_secs() {
        let d = Duration::from_secs(5);
        assert_eq!(d.seconds, 5);
        assert_eq!(d.nanoseconds, 0);
        assert!(!d.is_zero());
        assert!(!d.is_infinite());
    }

    #[test]
    fn duration_from_millis() {
        let d = Duration::from_millis(1500);
        assert_eq!(d.seconds, 1);
        assert_eq!(d.nanoseconds, 500_000_000);
    }

    #[test]
    fn duration_to_std_normal() {
        let d = Duration::new(3, 500_000_000);
        let std_d = d.to_std().expect("should convert");
        assert_eq!(std_d, time::Duration::new(3, 500_000_000));
    }

    #[test]
    fn duration_to_std_infinite_returns_none() {
        assert!(Duration::INFINITE.to_std().is_none());
    }

    #[test]
    fn duration_to_std_negative_returns_none() {
        let d = Duration::from_secs(-1);
        assert!(d.to_std().is_none());
    }

    #[test]
    fn duration_from_std_normal() {
        let std_d = time::Duration::new(10, 123_456_789);
        let d = Duration::from_std(std_d);
        assert_eq!(d.seconds, 10);
        assert_eq!(d.nanoseconds, 123_456_789);
    }

    #[test]
    fn duration_from_std_overflow_becomes_infinite() {
        let std_d = time::Duration::from_secs(u64::MAX);
        let d = Duration::from_std(std_d);
        assert!(d.is_infinite());
    }

    #[test]
    fn duration_default_is_infinite() {
        assert!(Duration::default().is_infinite());
    }

    #[test]
    fn duration_ordering() {
        let a = Duration::from_secs(1);
        let b = Duration::from_secs(2);
        let c = Duration::INFINITE;
        assert!(a < b);
        assert!(b < c);
    }

    #[test]
    #[should_panic(expected = "nanoseconds must be")]
    fn duration_invalid_nanos_panics() {
        let _ = Duration::new(0, 1_000_000_000);
    }

    #[test]
    fn duration_debug_format() {
        assert_eq!(format!("{:?}", Duration::INFINITE), "Duration(INFINITE)");
        assert_eq!(
            format!("{:?}", Duration::from_secs(5)),
            "Duration(5.000000000s)"
        );
    }

    // ── Timestamp tests ──

    #[test]
    fn timestamp_invalid() {
        let ts = Timestamp::INVALID;
        assert!(ts.is_invalid());
    }

    #[test]
    fn timestamp_zero() {
        let ts = Timestamp::ZERO;
        assert!(!ts.is_invalid());
        assert_eq!(ts.seconds, 0);
        assert_eq!(ts.nanoseconds, 0);
    }

    #[test]
    fn timestamp_now_is_not_invalid() {
        let ts = Timestamp::now();
        assert!(!ts.is_invalid());
        // Sanity: seconds should be a recent Unix timestamp
        assert!(ts.seconds > 1_700_000_000);
    }

    #[test]
    fn timestamp_default_is_invalid() {
        assert!(Timestamp::default().is_invalid());
    }

    #[test]
    #[should_panic(expected = "nanoseconds must be")]
    fn timestamp_invalid_nanos_panics() {
        let _ = Timestamp::new(0, 1_000_000_000);
    }

    #[test]
    fn timestamp_add_duration() {
        let ts = Timestamp::new(10, 500_000_000);
        let d = Duration::new(2, 700_000_000);
        let result = ts + d;
        // 500M + 700M = 1.2B nanos = 1 sec + 200M nanos
        assert_eq!(result.seconds, 13);
        assert_eq!(result.nanoseconds, 200_000_000);
    }

    #[test]
    fn timestamp_add_infinite_duration() {
        let ts = Timestamp::new(10, 0);
        let result = ts + Duration::INFINITE;
        assert!(result.is_invalid());
    }

    #[test]
    fn timestamp_add_to_invalid() {
        let result = Timestamp::INVALID + Duration::from_secs(1);
        assert!(result.is_invalid());
    }

    #[test]
    fn timestamp_sub_normal() {
        let a = Timestamp::new(10, 0);
        let b = Timestamp::new(7, 500_000_000);
        let d = a - b;
        assert_eq!(d.seconds, 2);
        assert_eq!(d.nanoseconds, 500_000_000);
    }

    #[test]
    fn timestamp_sub_invalid_returns_infinite() {
        let d = Timestamp::INVALID - Timestamp::ZERO;
        assert!(d.is_infinite());
    }

    #[test]
    fn timestamp_ordering() {
        let a = Timestamp::new(1, 0);
        let b = Timestamp::new(2, 0);
        assert!(a < b);
    }

    #[test]
    fn timestamp_debug_format() {
        assert_eq!(format!("{:?}", Timestamp::INVALID), "Timestamp(INVALID)");
        assert_eq!(
            format!("{:?}", Timestamp::new(100, 0)),
            "Timestamp(100.000000000)"
        );
    }
}
