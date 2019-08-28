//! Types and utilities related to timing.
#![allow(clippy::inconsistent_digit_grouping)]

use core::{convert::TryFrom, time::Duration};

use derive_more::{Add, AddAssign, Neg, Sub, SubAssign};

/// A point in time.
///
/// Timestamps are represented as `i32`s in <sup>1</sup>⁄<sub>100</sub>ths of a millisecond.
#[derive(
    Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd, Add, AddAssign, Sub, SubAssign, Neg,
)]
pub struct Timestamp(i32);

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

/// The error type returned when a duration to timestamp conversion fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TryFromDurationError(());

/// The error type returned when a timestamp to duration conversion fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TryFromTimestampError(());

impl Timestamp {
    /// Returns the number of seconds contained in this `Timestamp` as `f32`.
    #[inline]
    pub fn as_secs_f32(self) -> f32 {
        self.0 as f32 / 1_000_00.
    }

    /// Creates a new `Timestamp` from the specified number of seconds represented as `f32`.
    ///
    /// # Panics
    ///
    /// Panics if `secs` is not finite or overflows the `Timestamp`.
    #[inline]
    pub fn from_secs_f32(secs: f32) -> Self {
        let value = secs * 1_000_00.;
        assert!(value.is_finite());
        // Both of these happen to remain within the i32 range when converted to f32.
        assert!(value <= i32::max_value() as f32);
        assert!(value >= i32::min_value() as f32);

        Self(value as i32)
    }

    /// Creates a new `Timestamp` from the specified number of milliseconds.
    ///
    /// # Panics
    ///
    /// Panics if `millis` overflows the `Timestamp`.
    #[inline]
    pub fn from_millis(millis: i32) -> Self {
        Self(
            millis
                .checked_mul(100)
                .expect("overflow when converting milliseconds to Timestamp"),
        )
    }
}

impl MapTimestamp {
    /// Creates a new `MapTimestamp` from the specified number of milliseconds.
    ///
    /// # Panics
    ///
    /// Panics if `millis` overflows the `MapTimestamp`.
    #[inline]
    pub fn from_millis(millis: i32) -> Self {
        Self(Timestamp::from_millis(millis))
    }
}

impl GameTimestamp {
    /// Creates a new `GameTimestamp` from the specified number of milliseconds.
    ///
    /// # Panics
    ///
    /// Panics if `millis` overflows the `GameTimestamp`.
    #[inline]
    pub fn from_millis(millis: i32) -> Self {
        Self(Timestamp::from_millis(millis))
    }
}

impl TryFrom<Duration> for Timestamp {
    type Error = TryFromDurationError;

    #[inline]
    fn try_from(d: Duration) -> Result<Self, Self::Error> {
        // TODO: should this return an error instead of silently dropping precision?
        // Maybe use a method instead of TryFrom? Seems to be too much effort to use it while
        // preserving the lossless-ness of the conversion...
        let value = d.as_micros() / 10;
        if value > i32::max_value() as u128 {
            Err(TryFromDurationError(()))
        } else {
            Ok(Self(value as i32))
        }
    }
}

impl TryFrom<Timestamp> for Duration {
    type Error = TryFromTimestampError;

    #[inline]
    fn try_from(t: Timestamp) -> Result<Self, Self::Error> {
        if t.0 < 0 {
            return Err(TryFromTimestampError(()));
        }

        let t = t.0 as u64;
        let seconds = t / 1_000_00;
        let nanos = (t - seconds * 1_000_00) as u32 * 10_000;
        Ok(Self::new(seconds, nanos))
    }
}


impl MapTimestamp {
    /// Converts the game timestamp to a map timestamp.
    #[inline]
    pub fn to_game(self, state: &GameState) -> GameTimestamp {
        state.map_to_game(self)
    }
}

impl GameTimestamp {
    /// Converts the game timestamp to a map timestamp.
    #[inline]
    pub fn to_map(self, state: &GameState) -> MapTimestamp {
        state.game_to_map(self)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use core::convert::TryFrom;

    #[test]
    fn timestamp_from_duration() {
        let timestamp = Timestamp::try_from(Duration::from_secs(1));
        assert_eq!(timestamp, Ok(Timestamp(1_000_00)));
    }

    #[test]
    fn timestamp_from_non_representable_duration() {
        let timestamp = Timestamp::try_from(Duration::from_millis(u64::max_value()));
        assert_eq!(timestamp, Err(TryFromDurationError(())));
    }
}
