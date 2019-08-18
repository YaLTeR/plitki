//! Types and utilities related to timing.
use derive_more::{Add, AddAssign, Sub, SubAssign};

/// A point in time.
///
/// Timestamps are represented as `i32`s in <sup>1</sup>⁄<sub>100</sub>ths of a millisecond.
#[derive(
    Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd, Add, AddAssign, Sub, SubAssign,
)]
pub struct Timestamp(pub i32);

/// A point in time, measured in map time.
#[derive(
    Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd, Add, AddAssign, Sub, SubAssign,
)]
pub struct MapTimestamp(pub Timestamp);

/// A point in time, measured in game time.
#[derive(
    Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd, Add, AddAssign, Sub, SubAssign,
)]
pub struct GameTimestamp(pub Timestamp);
