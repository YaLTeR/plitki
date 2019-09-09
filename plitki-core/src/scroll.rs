//! Object positioning on screen.
use core::{
    convert::TryInto,
    ops::{Div, Mul},
};

use crate::timing::GameTimestampDifference;

/// Object position on screen.
///
/// The object is at position `0` when the current map timestamp matches the object timestamp.
/// `2_000_000_000` corresponds to one vertical square screen.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Position(pub i64);

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

impl Mul<GameTimestampDifference> for ScrollSpeed {
    type Output = Position;

    #[inline]
    fn mul(self, rhs: GameTimestampDifference) -> Self::Output {
        Position(
            i64::from(self.0)
                * i64::from(rhs.into_milli_hundredths())
                * i64::from(ScrollSpeedMultiplier::default().0),
        )
    }
}

impl Mul<ScrollSpeed> for GameTimestampDifference {
    type Output = Position;

    #[inline]
    fn mul(self, rhs: ScrollSpeed) -> Self::Output {
        rhs * self
    }
}

impl Div<ScrollSpeed> for Position {
    type Output = GameTimestampDifference;

    #[inline]
    fn div(self, rhs: ScrollSpeed) -> Self::Output {
        let value = self.0 / i64::from(rhs.0) / i64::from(ScrollSpeedMultiplier::default().0);
        GameTimestampDifference::from_milli_hundredths(value.try_into().unwrap())
    }
}
