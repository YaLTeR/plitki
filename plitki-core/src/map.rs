//! Functionality related to mapsets and maps.
use alloc::{string::String, vec::Vec};

use crate::{
    object::Object,
    scroll::ScrollSpeedMultiplier,
    timing::{MapTimestamp, MapTimestampDifference},
};

/// One lane in a map.
#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct Lane {
    /// Objects in this lane.
    pub objects: Vec<Object>,
}

/// A map (beatmap, chart, file).
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Map {
    // TODO: separate these out? Leave only the actual object info here?
    // Idea for separation: Mapset contains Difficulties, which have this info plus a Map which has
    // just lanes, timing, etc.
    /// Artist of the song.
    pub song_artist: Option<String>,
    /// Title of the song.
    pub song_title: Option<String>,
    /// Difficulty name.
    pub difficulty_name: Option<String>,
    /// Mapper's name.
    pub mapper: Option<String>,

    /// Filename of the audio track.
    pub audio_file: Option<String>,
    /// BPM and time signature changes.
    pub timing_points: Vec<TimingPoint>,
    /// Scroll speed changes (SVs).
    pub scroll_speed_changes: Vec<ScrollSpeedChange>,
    /// The scroll speed multiplier in effect at the map start, before any scroll speed changes.
    pub initial_scroll_speed_multiplier: ScrollSpeedMultiplier,
    /// Lanes constituting the map.
    pub lanes: Vec<Lane>,
}

/// A scroll speed change (an SV).
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct ScrollSpeedChange {
    /// Timestamp when this change takes an effect.
    pub timestamp: MapTimestamp,
    /// The scroll speed multiplier.
    pub multiplier: ScrollSpeedMultiplier,
}

/// A change of the BPM or time signature.
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct TimingPoint {
    /// Timestamp when this change takes an effect.
    pub timestamp: MapTimestamp,
    /// Duration of one beat.
    pub beat_duration: MapTimestampDifference,
    /// The time signature.
    pub signature: TimeSignature,
}

/// A time signature.
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct TimeSignature {
    /// How many beats to count.
    ///
    /// This is the top number in a time signature.
    pub beat_count: u8,
    /// What kind of note corresponds to one beat.
    ///
    /// This is the bottom number in a time signature.
    pub beat_unit: u8,
}

impl Lane {
    /// Constructs an empty lane.
    #[inline]
    pub fn new() -> Self {
        Default::default()
    }
}
