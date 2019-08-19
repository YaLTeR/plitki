//! Functionality related to mapsets and maps.
use alloc::{string::String, vec::Vec};

use crate::object::Object;

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

    /// Lanes constituting the map.
    pub lanes: Vec<Lane>,
}

impl Lane {
    /// Constructs an empty lane.
    #[inline]
    pub fn new() -> Self {
        Default::default()
    }
}
