//! Types and utilities related to timing.
use core::time::Duration;

use derive_more::{Add, AddAssign, Neg, Sub, SubAssign};

/// A point in time.
///
/// Timestamps are represented as `i32`s in <sup>1</sup>⁄<sub>100</sub>ths of a millisecond.
#[derive(
    Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd, Add, AddAssign, Sub, SubAssign, Neg,
)]
pub struct Timestamp(pub i32);

/// A point in time, measured in map time.
#[derive(
    Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd, Add, AddAssign, Sub, SubAssign, Neg,
)]
pub struct MapTimestamp(pub Timestamp);

/// A point in time, measured in game time.
#[derive(
    Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd, Add, AddAssign, Sub, SubAssign, Neg,
)]
pub struct GameTimestamp(pub Timestamp);

impl Timestamp {
    /// Creates a `Timestamp` from a `Duration`.
    #[inline]
    pub const fn from_duration(duration: Duration) -> Self {
        #[allow(clippy::inconsistent_digit_grouping)]
        Self(duration.as_secs() as i32 * 1_000_00 + duration.subsec_micros() as i32 / 10)
    }
}

impl From<Duration> for Timestamp {
    #[inline]
    fn from(d: Duration) -> Self {
        Self::from_duration(d)
    }
}
