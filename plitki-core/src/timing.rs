//! Types and utilities related to timing.
#![allow(clippy::inconsistent_digit_grouping)]

use core::{
    convert::TryFrom,
    ops::{Add, Sub},
    time::Duration,
};

#[cfg(test)]
use proptest_derive::Arbitrary;

use crate::{
    impl_ops,
    scroll::{Position, ScrollSpeedMultiplier},
};

/// A point in time.
///
/// Timestamps are represented as `i32`s in <sup>1</sup>⁄<sub>100</sub>ths of a millisecond.
/// Timestamps range from -2<sup>30</sup> to 2<sup>30</sup>-1
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct Timestamp(#[cfg_attr(test, proptest(strategy = "-(2i32.pow(30))..2i32.pow(30)"))] i32);

/// A difference between [`Timestamp`]s.
///
/// Represented the same way as a [`Timestamp`].
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct TimestampDifference(i32);

/// A point in time, measured in map time.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct MapTimestamp(pub Timestamp);

/// A difference between [`MapTimestamp`]s.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct MapTimestampDifference(pub TimestampDifference);

/// A point in time, measured in game time.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct GameTimestamp(pub Timestamp);

/// A difference between [`GameTimestamp`]s.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct GameTimestampDifference(pub TimestampDifference);

/// Contains data necessary to convert between game and map timestamps.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct TimestampConverter {
    /// Global offset.
    ///
    /// Global offset is used to adjust for the audio playback latency of the underlying audio
    /// device. This latency affects any sound regardless of rate or other parameters. Therefore,
    /// the global offset is not affected by rate.
    pub global_offset: GameTimestampDifference,

    /// Local offset.
    ///
    /// Local offset is used to adjust for the incorrect map timing, be it due to the mapper's
    /// mistake or audio playback differences between different games. The local offset is affected
    /// by rate.
    pub local_offset: MapTimestampDifference,
}

/// The error type returned when a duration to timestamp conversion fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TryFromDurationError(());

/// The error type returned when a timestamp to duration conversion fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TryFromTimestampError(());

impl Timestamp {
    /// The smallest value that can be represented by this type, -2<sup>30</sup>.
    pub const MIN: Timestamp = Timestamp(-(2i32.pow(30)));

    /// The largest value that can be represented by this type, 2<sup>30</sup>-1.
    pub const MAX: Timestamp = Timestamp((2i32.pow(30)) - 1);

    /// Returns the zero timestamp.
    #[inline]
    pub fn zero() -> Self {
        Self(0)
    }

    /// Creates a new `Timestamp` from the specified number of milliseconds.
    ///
    /// # Panics
    ///
    /// Panics if `millis` overflows the `Timestamp`.
    #[inline]
    pub fn from_millis(millis: i32) -> Self {
        Self::from_milli_hundredths(
            millis
                .checked_mul(100)
                .expect("overflow when converting milliseconds to Timestamp"),
        )
    }

    /// Returns the total number of whole milliseconds represented by this timestamp.
    #[inline]
    pub fn as_millis(self) -> i32 {
        self.0 / 100
    }

    /// Creates a new `Timestamp` from the specified number of <sup>1</sup>⁄<sub>100</sub>ths of a
    /// millisecond.
    ///
    /// # Panics
    ///
    /// Panics if `milli_hundredths` overflows the `Timestamp`.
    #[inline]
    pub fn from_milli_hundredths(milli_hundredths: i32) -> Self {
        Self::checked_from_milli_hundredths(milli_hundredths).unwrap()
    }

    /// Creates a new `Timestamp` from the specified number of <sup>1</sup>⁄<sub>100</sub>ths of a
    /// millisecond, returning `None` if overflow occurred.
    #[inline]
    pub fn checked_from_milli_hundredths(milli_hundredths: i32) -> Option<Self> {
        const MIN: i32 = Timestamp::MIN.0;
        const MAX: i32 = Timestamp::MAX.0;

        match milli_hundredths {
            MIN..=MAX => Some(Self(milli_hundredths)),
            _ => None,
        }
    }

    /// Creates a new `Timestamp` from the specified number of <sup>1</sup>⁄<sub>100</sub>ths of a
    /// millisecond, saturating at the bounds instead of panicking.
    #[inline]
    pub fn saturating_from_milli_hundredths(milli_hundredths: i32) -> Self {
        let milli_hundredths = milli_hundredths.clamp(Timestamp::MIN.0, Timestamp::MAX.0);
        Self::from_milli_hundredths(milli_hundredths)
    }

    /// Returns the timestamp as the number of <sup>1</sup>⁄<sub>100</sub>ths of a millisecond.
    #[inline]
    pub fn into_milli_hundredths(self) -> i32 {
        self.0
    }
}

impl MapTimestamp {
    /// Returns the zero map timestamp.
    #[inline]
    pub fn zero() -> Self {
        Self(Timestamp::zero())
    }

    /// Creates a new `MapTimestamp` from the specified number of milliseconds.
    ///
    /// # Panics
    ///
    /// Panics if `millis` overflows the `MapTimestamp`.
    #[inline]
    pub fn from_millis(millis: i32) -> Self {
        Self(Timestamp::from_millis(millis))
    }

    /// Returns the total number of whole milliseconds represented by this timestamp.
    #[inline]
    pub fn as_millis(self) -> i32 {
        self.0.as_millis()
    }

    /// Creates a new `MapTimestamp` from the specified number of <sup>1</sup>⁄<sub>100</sub>ths of
    /// a millisecond.
    ///
    /// # Panics
    ///
    /// Panics if `milli_hundredths` overflows the `MapTimestamp`.
    #[inline]
    pub fn from_milli_hundredths(milli_hundredths: i32) -> Self {
        Self(Timestamp::from_milli_hundredths(milli_hundredths))
    }

    /// Creates a new `MapTimestamp` from the specified number of <sup>1</sup>⁄<sub>100</sub>ths of
    /// a millisecond, returning `None` if overflow occurred.
    #[inline]
    pub fn checked_from_milli_hundredths(milli_hundredths: i32) -> Option<Self> {
        Timestamp::checked_from_milli_hundredths(milli_hundredths).map(Self)
    }

    /// Creates a new `MapTimestamp` from the specified number of <sup>1</sup>⁄<sub>100</sub>ths of
    /// a millisecond, saturating at the bounds instead of panicking.
    #[inline]
    pub fn saturating_from_milli_hundredths(milli_hundredths: i32) -> Self {
        Self(Timestamp::saturating_from_milli_hundredths(
            milli_hundredths,
        ))
    }

    /// Returns the timestamp as the number of <sup>1</sup>⁄<sub>100</sub>ths of a millisecond.
    #[inline]
    pub fn into_milli_hundredths(self) -> i32 {
        self.0.into_milli_hundredths()
    }

    /// Converts the timestamp into a `Position`, as if there were no scroll speed changes.
    #[inline]
    pub fn no_scroll_speed_change_position(self) -> Position {
        Position::zero() + (self - MapTimestamp::zero()) * ScrollSpeedMultiplier::default()
    }

    /// Converts the `Position` into a timestamp, as if there were no scroll speed changes.
    #[inline]
    pub fn from_no_scroll_speed_change_position(position: Position) -> Self {
        MapTimestamp::zero() + (position - Position::zero()) / ScrollSpeedMultiplier::default()
    }
}

impl GameTimestamp {
    /// Returns the zero game timestamp.
    #[inline]
    pub fn zero() -> Self {
        Self(Timestamp::zero())
    }

    /// Creates a new `GameTimestamp` from the specified number of milliseconds.
    ///
    /// # Panics
    ///
    /// Panics if `millis` overflows the `GameTimestamp`.
    #[inline]
    pub fn from_millis(millis: i32) -> Self {
        Self(Timestamp::from_millis(millis))
    }

    /// Returns the total number of whole milliseconds represented by this timestamp.
    #[inline]
    pub fn as_millis(self) -> i32 {
        self.0.as_millis()
    }

    /// Creates a new `GameTimestamp` from the specified number of <sup>1</sup>⁄<sub>100</sub>ths
    /// of a millisecond.
    ///
    /// # Panics
    ///
    /// Panics if `milli_hundredths` overflows the `GameTimestamp`.
    #[inline]
    pub fn from_milli_hundredths(milli_hundredths: i32) -> Self {
        Self(Timestamp::from_milli_hundredths(milli_hundredths))
    }

    /// Creates a new `GameTimestamp` from the specified number of <sup>1</sup>⁄<sub>100</sub>ths of
    /// a millisecond, returning `None` if overflow occurred.
    #[inline]
    pub fn checked_from_milli_hundredths(milli_hundredths: i32) -> Option<Self> {
        Timestamp::checked_from_milli_hundredths(milli_hundredths).map(Self)
    }

    /// Creates a new `MapTimestamp` from the specified number of <sup>1</sup>⁄<sub>100</sub>ths of
    /// a millisecond, saturating at the bounds instead of panicking.
    #[inline]
    pub fn saturating_from_milli_hundredths(milli_hundredths: i32) -> Self {
        Self(Timestamp::saturating_from_milli_hundredths(
            milli_hundredths,
        ))
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

    /// Returns the total number of whole milliseconds represented by this timestamp.
    #[inline]
    pub fn as_millis(self) -> i32 {
        self.0 / 100
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

    /// Returns the total number of whole milliseconds represented by this timestamp.
    #[inline]
    pub fn as_millis(self) -> i32 {
        self.0.as_millis()
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

    /// Returns the total number of whole milliseconds represented by this timestamp.
    #[inline]
    pub fn as_millis(self) -> i32 {
        self.0.as_millis()
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
        if value >= 2u128.pow(30) {
            Err(TryFromDurationError(()))
        } else {
            Ok(Self::from_milli_hundredths(value as i32))
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
    /// Takes global and local offsets into account. For differences (which do _not_ need to
    /// consider global and local offsets) use [`Self::game_to_map_difference()`].
    ///
    /// The conversion uses saturating arithmetic in case the timestamp or the offsets are too
    /// large.
    #[inline]
    pub fn game_to_map(&self, timestamp: GameTimestamp) -> MapTimestamp {
        // Sacrificing a bit of type safety here for saturating arithmetic.
        let global = self.global_offset.into_milli_hundredths();
        let local = self.local_offset.into_milli_hundredths();
        let timestamp = timestamp.into_milli_hundredths();

        // MapTimestamp((timestamp + global).0) - local
        MapTimestamp::saturating_from_milli_hundredths(
            timestamp.saturating_add(global.saturating_sub(local)),
        )
    }

    /// Converts a map timestamp into a game timestamp.
    ///
    /// Takes global and local offsets into account. For differences (which do _not_ need to
    /// consider global offset) use [`Self::map_to_game_difference()`].
    ///
    /// The conversion uses saturating arithmetic in case the timestamp or the offsets are too
    /// large.
    #[inline]
    pub fn map_to_game(&self, timestamp: MapTimestamp) -> GameTimestamp {
        // Sacrificing a bit of type safety here for saturating arithmetic.
        let global = self.global_offset.into_milli_hundredths();
        let local = self.local_offset.into_milli_hundredths();
        let timestamp = timestamp.into_milli_hundredths();

        // GameTimestamp((timestamp + local).0) - global
        GameTimestamp::saturating_from_milli_hundredths(
            timestamp.saturating_add(local.saturating_sub(global)),
        )
    }

    /// Converts a game difference into a map difference.
    ///
    /// Difference conversion does _not_ consider global and local offsets. For timestamps (which
    /// need to consider global and local offsets) use [`Self::game_to_map()`].
    #[inline]
    pub fn game_to_map_difference(
        &self,
        difference: GameTimestampDifference,
    ) -> MapTimestampDifference {
        MapTimestampDifference(difference.0)
    }

    /// Converts a map difference into a game difference.
    ///
    /// Difference conversion does _not_ consider global and local offsets. For timestamps (which
    /// need to consider global and local offsets) use [`Self::map_to_game()`].
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
    use proptest::prelude::*;

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

    #[test]
    fn map_to_game_and_back_matches() {
        for global in -10..10 {
            for local in -10..10 {
                for timestamp in -10..10 {
                    let converter = TimestampConverter {
                        global_offset: GameTimestampDifference::from_milli_hundredths(global),
                        local_offset: MapTimestampDifference::from_milli_hundredths(local),
                    };
                    let timestamp = MapTimestamp::from_milli_hundredths(timestamp);
                    let result = timestamp.to_game(&converter).to_map(&converter);
                    assert_eq!(result, timestamp);
                }
            }
        }
    }

    proptest! {
        #[allow(clippy::inconsistent_digit_grouping)]
        #[test]
        fn duration_to_timestamp_and_back(
            secs in 0..2u64.pow(30) / 1_000_00,
            rest in 0..1_000_00u32,
        ) {
            let duration = Duration::new(secs, rest * 10_000);
            let timestamp = Timestamp::try_from(duration).unwrap();
            let duration2 = Duration::try_from(timestamp).unwrap();

            prop_assert_eq!(duration, duration2);
        }

        #[test]
        fn saturating_from_milli_hundredths_doesnt_panic(value: i32) {
            let _ = Timestamp::saturating_from_milli_hundredths(value);
        }

        #[test]
        fn subtracting_timestamps_doesnt_panic(a: Timestamp, b: Timestamp) {
            let _ = a - b;
        }

        #[test]
        fn converting_map_to_game_doesnt_panic(timestamp: MapTimestamp, converter: TimestampConverter) {
            let _ = timestamp.to_game(&converter);
        }

        #[test]
        fn converting_game_to_map_doesnt_panic(timestamp: GameTimestamp, converter: TimestampConverter) {
            let _ = timestamp.to_map(&converter);
        }
    }
}
