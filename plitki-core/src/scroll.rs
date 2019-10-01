//! Object positioning on screen.
use core::ops::{Add, Div, Mul, Sub};

use crate::{
    impl_ops,
    timing::{MapTimestampDifference, TimestampConverter},
};

/// Object position on screen.
///
/// The object is at position `0` when the current map timestamp matches the object timestamp.
/// `2_000_000_000` corresponds to one vertical square screen.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Position(pub i64);

/// Difference between positions.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct PositionDifference(pub i64);

impl_ops!(Position, PositionDifference);

/// Scrolling speed.
///
/// Measured in <sup>1</sup>⁄<sub>20</sub>ths of vertical square screens per second. That is, on a
/// square 1:1 screen, 20 means a note travels from the very top to the very bottom of the screen
/// in one second; 10 means in two seconds and 40 means in half a second.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ScrollSpeed(pub u8);

/// Scrolling speed multiplier, otherwise known as SV (Scroll Velocity, Slider Velocity).
///
/// The scroll speed multiplier ranges from -2<sup>24</sup> to 2<sup>24</sup>-1. The value of 1000
/// is equivalent to a multiplier of 1.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ScrollSpeedMultiplier(i32);

// (MapTimestampDifference / Rate) * ScrollSpeed * ScrollSpeedMultiplier = PositionDifference
//
// Thanks to the commutativity of the operations, we can do:
// (MapTimestampDifference * ScrollSpeedMultiplier) / Rate * ScrollSpeed
//
// And thus, pre-compute the SVs.
//
// This still works for non-constant SVs like this:
// (MapTimestampDifference1 / Rate) * ScrollSpeed * ScrollSpeedMultiplier1
// + (MapTimestampDifference2 / Rate) * ScrollSpeed * ScrollSpeedMultiplier2

// TODO: naming.
/// Map timestamp pre-multiplied by scroll speed multiplier.
///
/// Used for pre-computing the scroll speed changes.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct MapTimestampDifferenceTimesScrollSpeedMultiplier(pub(crate) i64);

/// Game timestamp pre-multiplied by scroll speed multiplier.
///
/// An intermediate step in conversion of [`MapTimestampDifferenceTimesScrollSpeedMultiplier`] into
/// a [`PositionDifference`].
///
/// [`MapTimestampDifferenceTimesScrollSpeedMultiplier`]:
/// struct.MapTimestampDifferenceTimesScrollSpeedMultiplier.html
/// [`PositionDifference`]: struct.PositionDifference.html
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct GameTimestampDifferenceTimesScrollSpeedMultiplier(pub(crate) i64);

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

impl MapTimestampDifferenceTimesScrollSpeedMultiplier {
    /// Converts the map timestamp difference pre-multiplied by scroll speed multiplier to a game
    /// timestamp difference pre-multiplied by scroll speed multiplier.
    #[inline]
    pub fn to_game(
        self,
        converter: &TimestampConverter,
    ) -> GameTimestampDifferenceTimesScrollSpeedMultiplier {
        converter.map_to_game_difference_times_multiplier(self)
    }
}

impl GameTimestampDifferenceTimesScrollSpeedMultiplier {
    /// Converts the game timestamp difference pre-multiplied by scroll speed multiplier to a map
    /// timestamp difference pre-multiplied by scroll speed multiplier.
    #[inline]
    pub fn to_map(
        self,
        converter: &TimestampConverter,
    ) -> MapTimestampDifferenceTimesScrollSpeedMultiplier {
        converter.game_to_map_difference_times_multiplier(self)
    }
}

impl Mul<GameTimestampDifferenceTimesScrollSpeedMultiplier> for ScrollSpeed {
    type Output = PositionDifference;

    #[inline]
    fn mul(self, rhs: GameTimestampDifferenceTimesScrollSpeedMultiplier) -> Self::Output {
        PositionDifference(i64::from(self.0) * rhs.0)
    }
}

impl Mul<ScrollSpeed> for GameTimestampDifferenceTimesScrollSpeedMultiplier {
    type Output = PositionDifference;

    #[inline]
    fn mul(self, rhs: ScrollSpeed) -> Self::Output {
        rhs * self
    }
}

impl Div<ScrollSpeed> for PositionDifference {
    type Output = GameTimestampDifferenceTimesScrollSpeedMultiplier;

    #[inline]
    fn div(self, rhs: ScrollSpeed) -> Self::Output {
        GameTimestampDifferenceTimesScrollSpeedMultiplier(self.0 / i64::from(rhs.0))
    }
}
// impl Mul<GameTimestampDifference> for ScrollSpeed {
//     type Output = PositionDifference;
//
//     #[inline]
//     fn mul(self, rhs: GameTimestampDifference) -> Self::Output {
//         PositionDifference(
//             i64::from(self.0)
//                 * i64::from(rhs.into_milli_hundredths())
//                 * i64::from(ScrollSpeedMultiplier::default().0),
//         )
//     }
// }
//
// impl Mul<ScrollSpeed> for GameTimestampDifference {
//     type Output = PositionDifference;
//
//     #[inline]
//     fn mul(self, rhs: ScrollSpeed) -> Self::Output {
//         rhs * self
//     }
// }
//
// impl Div<ScrollSpeed> for PositionDifference {
//     type Output = GameTimestampDifference;
//
//     #[inline]
//     fn div(self, rhs: ScrollSpeed) -> Self::Output {
//         let value = self.0 / i64::from(rhs.0) / i64::from(ScrollSpeedMultiplier::default().0);
//         GameTimestampDifference::from_milli_hundredths(value.try_into().unwrap())
//     }
// }

impl Add<MapTimestampDifferenceTimesScrollSpeedMultiplier>
    for MapTimestampDifferenceTimesScrollSpeedMultiplier
{
    type Output = MapTimestampDifferenceTimesScrollSpeedMultiplier;

    #[inline]
    fn add(self, rhs: MapTimestampDifferenceTimesScrollSpeedMultiplier) -> Self::Output {
        MapTimestampDifferenceTimesScrollSpeedMultiplier(self.0 + rhs.0)
    }
}

impl Sub<MapTimestampDifferenceTimesScrollSpeedMultiplier>
    for MapTimestampDifferenceTimesScrollSpeedMultiplier
{
    type Output = MapTimestampDifferenceTimesScrollSpeedMultiplier;

    #[inline]
    fn sub(self, rhs: MapTimestampDifferenceTimesScrollSpeedMultiplier) -> Self::Output {
        MapTimestampDifferenceTimesScrollSpeedMultiplier(self.0 - rhs.0)
    }
}

impl Mul<MapTimestampDifference> for ScrollSpeedMultiplier {
    type Output = MapTimestampDifferenceTimesScrollSpeedMultiplier;

    #[inline]
    fn mul(self, rhs: MapTimestampDifference) -> Self::Output {
        MapTimestampDifferenceTimesScrollSpeedMultiplier(
            i64::from(self.0) * i64::from(rhs.into_milli_hundredths()),
        )
    }
}

impl Mul<ScrollSpeedMultiplier> for MapTimestampDifference {
    type Output = MapTimestampDifferenceTimesScrollSpeedMultiplier;

    #[inline]
    fn mul(self, rhs: ScrollSpeedMultiplier) -> Self::Output {
        rhs * self
    }
}
