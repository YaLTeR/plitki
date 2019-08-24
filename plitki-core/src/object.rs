//! Functionality related to objects that constitute the maps.

use crate::timing::MapTimestamp;

/// An object that appears on the lanes, such as a regular note or a long note.
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum Object {
    /// A regular object, hit by tapping a key.
    Regular {
        /// The timestamp at which this object should be hit.
        // TODO: I kinda don't like that essentially the same thing is called "timestamp" for
        // regular objects and "start" for LNs... But it's also kinda not exactly the same because
        // you don't need to hold the regular objects, so "start" doesn't make sense for them.
        // Meanwhile, "start" makes much more sense for LNs than "timestamp" because it goes with
        // "end".
        timestamp: MapTimestamp,
    },
    /// A long note, hit by holding down a key and subsequently releasing.
    LongNote {
        /// The timestamp at which this long note should be held down.
        start: MapTimestamp,
        /// The timestamp at which this long note should be released.
        end: MapTimestamp,
    },
}

impl Object {
    /// Returns the start timestamp of the object.
    ///
    /// This is the first timestamp at which this object is visible.
    #[inline]
    pub fn start_timestamp(&self) -> MapTimestamp {
        match *self {
            Object::Regular { timestamp } => timestamp,
            Object::LongNote { start, .. } => start,
        }
    }

    /// Returns the end timestamp of the object.
    ///
    /// This is the last timestamp at which this object is visible.
    #[inline]
    pub fn end_timestamp(&self) -> MapTimestamp {
        match *self {
            Object::Regular { timestamp } => timestamp,
            Object::LongNote { end, .. } => end,
        }
    }
}
