//! Types and utilities related to timing.
#![allow(clippy::inconsistent_digit_grouping)]

use core::{
    convert::TryFrom,
    ops::{Add, Sub},
    time::Duration,
};

/// A point in time.
///
/// Timestamps are represented as `i32`s in <sup>1</sup>⁄<sub>100</sub>ths of a millisecond.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Timestamp(i32);

/// A difference between [`Timestamp`]s.
///
/// Represented the same way as a [`Timestamp`].
///
/// [`Timestamp`]: struct.Timestamp.html
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct TimestampDifference(i32);

/// A point in time, measured in map time.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct MapTimestamp(pub Timestamp);

/// A difference between [`MapTimestamp`]s.
///
/// [`MapTimestamp`]: struct.MapTimestamp.html
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct MapTimestampDifference(pub TimestampDifference);

/// A point in time, measured in game time.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct GameTimestamp(pub Timestamp);

/// A difference between [`GameTimestamp`]s.
///
/// [`GameTimestamp`]: struct.GameTimestamp.html
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct GameTimestampDifference(pub TimestampDifference);

/// Contains data necessary to convert between game and map timestamps.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct TimestampConverter {
    /// Global offset.
    ///
    /// Global offset is used to adjust for the audio playback latency of the underlying audio
    /// device. This latency affects any sound regardless of rate or other parameters. Therefore,
    /// the global offset is not affected by rate.
    pub global_offset: GameTimestampDifference,
}

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

    /// Creates a new `Timestamp` from the specified number of <sup>1</sup>⁄<sub>100</sub>ths of a
    /// millisecond.
    #[inline]
    pub fn from_milli_hundredths(milli_hundredths: i32) -> Self {
        Self(milli_hundredths)
    }

    /// Returns the timestamp as the number of <sup>1</sup>⁄<sub>100</sub>ths of a millisecond.
    #[inline]
    pub fn into_milli_hundredths(self) -> i32 {
        self.0
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

    /// Creates a new `MapTimestamp` from the specified number of <sup>1</sup>⁄<sub>100</sub>ths of
    /// a millisecond.
    #[inline]
    pub fn from_milli_hundredths(milli_hundredths: i32) -> Self {
        Self(Timestamp::from_milli_hundredths(milli_hundredths))
    }

    /// Returns the timestamp as the number of <sup>1</sup>⁄<sub>100</sub>ths of a millisecond.
    #[inline]
    pub fn into_milli_hundredths(self) -> i32 {
        self.0.into_milli_hundredths()
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

    /// Creates a new `GameTimestamp` from the specified number of <sup>1</sup>⁄<sub>100</sub>ths
    /// of a millisecond.
    #[inline]
    pub fn from_milli_hundredths(milli_hundredths: i32) -> Self {
        Self(Timestamp::from_milli_hundredths(milli_hundredths))
    }

    /// Returns the timestamp as the number of <sup>1</sup>⁄<sub>100</sub>ths of a millisecond.
    #[inline]
    pub fn into_milli_hundredths(self) -> i32 {
        self.0.into_milli_hundredths()
    }
}

impl TimestampDifference {
    /// Creates a new `TimestampDifference` from the specified number of milliseconds.
    ///
    /// # Panics
    ///
    /// Panics if `millis` overflows the `TimestampDifference`.
    #[inline]
    pub fn from_millis(millis: i32) -> Self {
        Self(
            millis
                .checked_mul(100)
                .expect("overflow when converting milliseconds to TimestampDifference"),
        )
    }

    /// Creates a new `TimestampDifference` from the specified number of
    /// <sup>1</sup>⁄<sub>100</sub>ths of a millisecond.
    #[inline]
    pub fn from_milli_hundredths(milli_hundredths: i32) -> Self {
        Self(milli_hundredths)
    }

    /// Returns the timestamp difference as the number of <sup>1</sup>⁄<sub>100</sub>ths of a
    /// millisecond.
    #[inline]
    pub fn into_milli_hundredths(self) -> i32 {
        self.0
    }
}

impl MapTimestampDifference {
    /// Creates a new `MapTimestampDifference` from the specified number of milliseconds.
    ///
    /// # Panics
    ///
    /// Panics if `millis` overflows the `MapTimestampDifference`.
    #[inline]
    pub fn from_millis(millis: i32) -> Self {
        Self(TimestampDifference::from_millis(millis))
    }

    /// Creates a new `MapTimestampDifference` from the specified number of
    /// <sup>1</sup>⁄<sub>100</sub>ths of a millisecond.
    #[inline]
    pub fn from_milli_hundredths(milli_hundredths: i32) -> Self {
        Self(TimestampDifference::from_milli_hundredths(milli_hundredths))
    }

    /// Returns the timestamp difference as the number of <sup>1</sup>⁄<sub>100</sub>ths of a
    /// millisecond.
    #[inline]
    pub fn into_milli_hundredths(self) -> i32 {
        self.0.into_milli_hundredths()
    }
}

impl GameTimestampDifference {
    /// Creates a new `GameTimestampDifference` from the specified number of milliseconds.
    ///
    /// # Panics
    ///
    /// Panics if `millis` overflows the `GameTimestampDifference`.
    #[inline]
    pub fn from_millis(millis: i32) -> Self {
        Self(TimestampDifference::from_millis(millis))
    }

    /// Creates a new `GameTimestampDifference` from the specified number of
    /// <sup>1</sup>⁄<sub>100</sub>ths of a millisecond.
    #[inline]
    pub fn from_milli_hundredths(milli_hundredths: i32) -> Self {
        Self(TimestampDifference::from_milli_hundredths(milli_hundredths))
    }

    /// Returns the timestamp difference as the number of <sup>1</sup>⁄<sub>100</sub>ths of a
    /// millisecond.
    #[inline]
    pub fn into_milli_hundredths(self) -> i32 {
        self.0.into_milli_hundredths()
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

macro_rules! impl_ops {
    ($timestamp:ty, $timestamp_difference:ty) => {
        impl Sub<$timestamp> for $timestamp {
            type Output = $timestamp_difference;

            #[inline]
            fn sub(self, other: $timestamp) -> Self::Output {
                Self::Output {
                    0: self.0 - other.0,
                }
            }
        }

        impl Add<$timestamp_difference> for $timestamp {
            type Output = $timestamp;

            #[inline]
            fn add(self, other: $timestamp_difference) -> Self::Output {
                Self::Output {
                    0: self.0 + other.0,
                }
            }
        }

        impl Sub<$timestamp_difference> for $timestamp {
            type Output = $timestamp;

            #[inline]
            fn sub(self, other: $timestamp_difference) -> Self::Output {
                Self::Output {
                    0: self.0 - other.0,
                }
            }
        }

        impl Add<$timestamp_difference> for $timestamp_difference {
            type Output = $timestamp_difference;

            #[inline]
            fn add(self, other: $timestamp_difference) -> Self::Output {
                Self::Output {
                    0: self.0 + other.0,
                }
            }
        }

        impl Sub<$timestamp_difference> for $timestamp_difference {
            type Output = $timestamp_difference;

            #[inline]
            fn sub(self, other: $timestamp_difference) -> Self::Output {
                Self::Output {
                    0: self.0 - other.0,
                }
            }
        }
    };
}

impl_ops!(Timestamp, TimestampDifference);
impl_ops!(MapTimestamp, MapTimestampDifference);
impl_ops!(GameTimestamp, GameTimestampDifference);

impl MapTimestamp {
    /// Converts the game timestamp to a map timestamp.
    #[inline]
    pub fn to_game(self, converter: &TimestampConverter) -> GameTimestamp {
        converter.map_to_game(self)
    }
}

impl MapTimestampDifference {
    /// Converts the game timestamp difference to a map timestamp difference.
    #[inline]
    pub fn to_game(self, converter: &TimestampConverter) -> GameTimestampDifference {
        converter.map_to_game_difference(self)
    }
}

impl GameTimestamp {
    /// Converts the game timestamp to a map timestamp.
    #[inline]
    pub fn to_map(self, converter: &TimestampConverter) -> MapTimestamp {
        converter.game_to_map(self)
    }
}

impl GameTimestampDifference {
    /// Converts the game timestamp difference to a map timestamp difference.
    #[inline]
    pub fn to_map(self, converter: &TimestampConverter) -> MapTimestampDifference {
        converter.game_to_map_difference(self)
    }
}

impl TimestampConverter {
    /// Converts a game timestamp into a map timestamp.
    ///
    /// Takes global offset into account. For differences (which do _not_ need to consider global
    /// offset) use [`game_to_map_difference`].
    ///
    /// [`game_to_map_difference`]: #method.game_to_map_difference
    #[inline]
    pub fn game_to_map(&self, timestamp: GameTimestamp) -> MapTimestamp {
        MapTimestamp((timestamp + self.global_offset).0)
    }

    /// Converts a map timestamp into a game timestamp.
    ///
    /// Takes global offset into account. For differences (which do _not_ need to consider global
    /// offset) use [`map_to_game_difference`].
    ///
    /// [`map_to_game_difference`]: #method.map_to_game_difference
    #[inline]
    pub fn map_to_game(&self, timestamp: MapTimestamp) -> GameTimestamp {
        GameTimestamp(timestamp.0) - self.global_offset
    }

    /// Converts a game difference into a map difference.
    ///
    /// Difference conversion does _not_ consider global offset. For timestamps (which need to
    /// consider global offset) use [`game_to_map`].
    ///
    /// [`game_to_map`]: #method.game_to_map
    #[inline]
    pub fn game_to_map_difference(
        &self,
        difference: GameTimestampDifference,
    ) -> MapTimestampDifference {
        MapTimestampDifference(difference.0)
    }

    /// Converts a map difference into a game difference.
    ///
    /// Difference conversion does _not_ consider global offset. For timestamps (which need to
    /// consider global offset) use [`map_to_game`].
    ///
    /// [`map_to_game`]: #method.map_to_game
    #[inline]
    pub fn map_to_game_difference(
        &self,
        difference: MapTimestampDifference,
    ) -> GameTimestampDifference {
        GameTimestampDifference(difference.0)
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
