//! Object positioning on screen.
use core::{
    convert::TryFrom,
    ops::{Add, Div, Mul, Sub},
};

#[cfg(test)]
use proptest_derive::Arbitrary;

use crate::{impl_ops, timing::MapTimestampDifference};

/// Object position, taking scroll speed changes into account.
///
/// Position `0` corresponds to map timestamp `0`. If map timestamp is "time", scroll speed
/// multiplier is "velocity" (of the objects on the given time frame), then this is the "position".
///
/// Note that `Position` does _not_ take into account the actual scroll speed;
/// [`ScreenPositionDifference`] does.
///
/// `Position` ranges from -2<sup>56</sup> to 2<sup>56</sup>-1.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Position(i64);

/// Difference between positions.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct PositionDifference(i64);

impl_ops!(
    Position,
    into_i64,
    try_from,
    saturating_new,
    PositionDifference,
    into_i64
);

/// Difference between screen positions.
///
/// Screen position difference takes the scroll speed into account. In order to convert a
/// `ScreenPositionDifference` into a [`PositionDifference`], divide it by the scroll speed.
///
/// In accordance with the [`ScrollSpeed`] units, a `ScreenPositionDifference` of `2_000_000_000`
/// corresponds to one vertical square screen.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ScreenPositionDifference(pub i64);

/// Scrolling speed.
///
/// Measured in <sup>1</sup>⁄<sub>20</sub>ths of vertical square screens per second. That is, on a
/// square 1:1 screen, 20 means a note travels from the very top to the very bottom of the screen
/// in one second; 10 means in two seconds and 40 means in half a second.
///
/// Rather than an actual "velocity", consider this to be a unit-less multiplier to convert between
/// a [`PositionDifference`] and a [`ScreenPositionDifference`]. You could also think of this as a
/// "zoom-level" (the higher the scroll speed, the more "zoomed-in" the view of the map is). The
/// actual "velocity" is, then, the [`ScrollSpeedMultiplier`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct ScrollSpeed(pub u8);

/// Scrolling speed multiplier, otherwise known as SV (Scroll Velocity, Slider Velocity).
///
/// The scroll speed multiplier ranges from -2<sup>24</sup> to 2<sup>24</sup>-1. The value of 1000
/// is equivalent to a multiplier of 1.
///
/// Rather than a unit-less multiplier, consider this to be the actual "velocity" of the objects on
/// a given time frame, which you can multiply by a [`MapTimestampDifference`] and get the
/// resulting [`PositionDifference`]. [`ScrollSpeed`], then, becomes the unit-less multiplier to
/// convert between a [`PositionDifference`] and a [`ScreenPositionDifference`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct ScrollSpeedMultiplier(
    #[cfg_attr(test, proptest(strategy = "-(2i32.pow(24))..2i32.pow(24)"))] i32,
);

// (MapTimestampDifference / Rate) * ScrollSpeed * ScrollSpeedMultiplier = ScreenPositionDifference
//
// Thanks to the commutativity of the operations, we can do:
// (MapTimestampDifference * ScrollSpeedMultiplier) / Rate * ScrollSpeed
//
// And thus, pre-compute the SVs.
//
// This still works for non-constant SVs like this:
// (MapTimestampDifference1 / Rate) * ScrollSpeed * ScrollSpeedMultiplier1
// + (MapTimestampDifference2 / Rate) * ScrollSpeed * ScrollSpeedMultiplier2

// Judgement line => ScreenPositionDifference for top and bottom of the screen
// ScreenPositionDifference / ScrollSpeed = PositionDifference
//
// Current timestamp => to map => Position
// + top and bottom PositionDifference => top and bottom Position
//
// Objects MapTimestamp times ScrollSpeedMultiplier = Position for each object (precomputed)

impl Position {
    /// The smallest value that can be represented by this type, -2<sup>56</sup>.
    pub const MIN: Position = Position(-(2i64.pow(56)));

    /// The largest value that can be represented by this type, 2<sup>56</sup>-1.
    pub const MAX: Position = Position((2i64.pow(56)) - 1);

    /// Returns the zero position.
    #[inline]
    pub fn zero() -> Self {
        Self(0)
    }

    /// Creates a new `Position` with bounds checking.
    ///
    /// # Panics
    ///
    /// Panics if the `value` is outside of the valid `Position` range.
    #[inline]
    pub fn new(value: i64) -> Self {
        Self::try_from(value).unwrap()
    }

    /// Creates a new `Position`, saturating at the bounds instead of panicking.
    #[inline]
    pub fn saturating_new(value: i64) -> Self {
        Self::new(value.clamp(Position::MIN.0, Position::MAX.0))
    }

    #[inline]
    fn into_i64(self) -> i64 {
        self.0
    }
}

impl PositionDifference {
    #[inline]
    fn into_i64(self) -> i64 {
        self.0
    }
}

impl From<Position> for i64 {
    #[inline]
    fn from(value: Position) -> Self {
        value.0
    }
}

/// The error type returned when a checked conversion fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TryFromError(());

impl TryFrom<i64> for Position {
    type Error = TryFromError;

    #[inline]
    fn try_from(value: i64) -> Result<Self, Self::Error> {
        const MIN: i64 = Position::MIN.0;
        const MAX: i64 = Position::MAX.0;

        match value {
            MIN..=MAX => Ok(Self(value)),
            _ => Err(TryFromError(())),
        }
    }
}

impl ScrollSpeedMultiplier {
    /// Creates a new `ScrollSpeedMultiplier` with bounds checking.
    ///
    /// # Panics
    ///
    /// Panics if the `value` is outside of the valid `ScrollSpeedMultiplier` range.
    #[inline]
    pub fn new(value: i32) -> Self {
        assert!(value < 2i32.pow(24));
        assert!(value >= -(2i32.pow(24)));

        Self(value)
    }

    /// Converts an `f32` to a `ScrollSpeedMultiplier` with bounds checking.
    ///
    /// The value is in the conventional range (so `1.0` is the multiplier of `1`).
    ///
    /// # Panics
    ///
    /// Panics if the converted `value` is outside of the valid `ScrollSpeedMultiplier` range.
    #[inline]
    pub fn from_f32(value: f32) -> Self {
        Self::new((value * 1000.) as i32)
    }

    /// Performs a saturating conversion from an `f32` to a `ScrollSpeedMultiplier`.
    ///
    /// The value is in the conventional range (so `1.0` is the multiplier of `1`).
    #[inline]
    pub fn saturating_from_f32(value: f32) -> Self {
        let value = value
            .min((2i32.pow(24) - 1) as f32 / 1000.)
            .max(-(2i32.pow(24)) as f32 / 1000.);
        Self::new((value * 1000.) as i32)
    }

    /// Converts `ScrollSpeedMultiplier` to an `f32`.
    ///
    /// The returned value is in the conventional range (so `1.0` is the multiplier of `1`).
    #[inline]
    pub fn as_f32(self) -> f32 {
        (self.0 as f32) / 1000.
    }
}

impl Default for ScrollSpeedMultiplier {
    #[inline]
    fn default() -> Self {
        Self(1000)
    }
}

impl Mul<PositionDifference> for ScrollSpeed {
    type Output = ScreenPositionDifference;

    #[inline]
    fn mul(self, rhs: PositionDifference) -> Self::Output {
        ScreenPositionDifference(i64::from(self.0) * rhs.0)
    }
}

impl Mul<ScrollSpeed> for PositionDifference {
    type Output = ScreenPositionDifference;

    #[inline]
    fn mul(self, rhs: ScrollSpeed) -> Self::Output {
        rhs * self
    }
}

impl Div<ScrollSpeed> for ScreenPositionDifference {
    type Output = PositionDifference;

    #[inline]
    fn div(self, rhs: ScrollSpeed) -> Self::Output {
        PositionDifference(self.0 / i64::from(rhs.0))
    }
}

impl Add<ScreenPositionDifference> for ScreenPositionDifference {
    type Output = ScreenPositionDifference;

    #[inline]
    fn add(self, rhs: ScreenPositionDifference) -> Self::Output {
        ScreenPositionDifference(self.0 + rhs.0)
    }
}

impl Sub<ScreenPositionDifference> for ScreenPositionDifference {
    type Output = ScreenPositionDifference;

    #[inline]
    fn sub(self, rhs: ScreenPositionDifference) -> Self::Output {
        ScreenPositionDifference(self.0 - rhs.0)
    }
}

impl Mul<MapTimestampDifference> for ScrollSpeedMultiplier {
    type Output = PositionDifference;

    #[inline]
    fn mul(self, rhs: MapTimestampDifference) -> Self::Output {
        PositionDifference(i64::from(self.0) * i64::from(rhs.into_milli_hundredths()))
    }
}

impl Mul<ScrollSpeedMultiplier> for MapTimestampDifference {
    type Output = PositionDifference;

    #[inline]
    fn mul(self, rhs: ScrollSpeedMultiplier) -> Self::Output {
        rhs * self
    }
}

impl Div<ScrollSpeedMultiplier> for PositionDifference {
    type Output = MapTimestampDifference;

    #[inline]
    fn div(self, rhs: ScrollSpeedMultiplier) -> Self::Output {
        MapTimestampDifference::from_milli_hundredths((self.0 / i64::from(rhs.0)) as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn timestamp_difference_times_scroll_speed_multiplier_doesnt_panic(
            difference: MapTimestampDifference,
            multiplier: ScrollSpeedMultiplier,
        ) {
            let _ = difference * multiplier;
        }

        #[test]
        fn position_difference_times_scroll_speed_doesnt_panic(
            difference: MapTimestampDifference,
            multiplier: ScrollSpeedMultiplier,
            speed: ScrollSpeed,
        ) {
            let _ = (difference * multiplier) * speed;
        }
    }
}
