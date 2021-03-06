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

impl Map {
    /// Sorts and de-duplicates scroll speed changes.
    ///
    /// This method removes all but the last scroll speed changes on every given timestamp.
    pub fn sort_and_dedup_scroll_speed_changes(&mut self) {
        let changes = &mut self.scroll_speed_changes;
        changes.sort_by_key(|a| a.timestamp);

        let initial_multiplier = self.initial_scroll_speed_multiplier;
        let first_meaningful_change_index = if let Some((index, _)) = changes
            .iter()
            .enumerate()
            .find(|(_, x)| x.multiplier != initial_multiplier)
        {
            index
        } else {
            changes.clear();
            return;
        };

        // Vec::dedup_by_key would have been useful, but it removes all but the first occurrence
        // of a value, while want to retain the last occurrence.
        if changes.len() <= 1 {
            return;
        }

        // Might be possible to do in-place, but certainly non-trivial.
        //
        // The new_changes.last().unwrap().multiplier in the loop causes a failure in type
        // inference for some reason.
        let mut new_changes: Vec<ScrollSpeedChange> = Vec::with_capacity(changes.len());
        for i in first_meaningful_change_index..changes.len() {
            // Skip changes which don't change the multiplier from the previous one.
            if i > first_meaningful_change_index
                && changes[i].multiplier == new_changes.last().unwrap().multiplier
            {
                continue;
            }

            // Skip to the last change with this timestamp.
            if i + 1 < changes.len() && changes[i + 1].timestamp == changes[i].timestamp {
                continue;
            }

            new_changes.push(changes[i]);
        }

        *changes = new_changes;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn sort_and_dedup_scroll_speed_changes() {
        let mut map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: vec![
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(-1),
                    multiplier: ScrollSpeedMultiplier::new(0),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(0),
                    multiplier: ScrollSpeedMultiplier::new(1),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(1),
                    multiplier: ScrollSpeedMultiplier::new(2),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(1),
                    multiplier: ScrollSpeedMultiplier::new(3),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(2),
                    multiplier: ScrollSpeedMultiplier::new(3),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(3),
                    multiplier: ScrollSpeedMultiplier::new(4),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(3),
                    multiplier: ScrollSpeedMultiplier::new(5),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(4),
                    multiplier: ScrollSpeedMultiplier::new(5),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(4),
                    multiplier: ScrollSpeedMultiplier::new(6),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(4),
                    multiplier: ScrollSpeedMultiplier::new(7),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(5),
                    multiplier: ScrollSpeedMultiplier::new(8),
                },
            ],
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::new(0),
            lanes: Vec::new(),
        };

        map.sort_and_dedup_scroll_speed_changes();

        assert_eq!(
            &map.scroll_speed_changes[..],
            &[
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(0),
                    multiplier: ScrollSpeedMultiplier::new(1)
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(1),
                    multiplier: ScrollSpeedMultiplier::new(3)
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(3),
                    multiplier: ScrollSpeedMultiplier::new(5)
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(4),
                    multiplier: ScrollSpeedMultiplier::new(7)
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(5),
                    multiplier: ScrollSpeedMultiplier::new(8)
                },
            ][..]
        );
    }

    #[test]
    fn sort_and_dedup_scroll_speed_changes_one() {
        let mut map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: vec![ScrollSpeedChange {
                timestamp: MapTimestamp::from_millis(-1),
                multiplier: ScrollSpeedMultiplier::new(0),
            }],
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::new(0),
            lanes: Vec::new(),
        };

        map.sort_and_dedup_scroll_speed_changes();

        assert_eq!(&map.scroll_speed_changes[..], &[][..]);
    }

    #[test]
    fn sort_and_dedup_scroll_speed_changes_one_meaningful() {
        let mut map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: vec![ScrollSpeedChange {
                timestamp: MapTimestamp::from_millis(-1),
                multiplier: ScrollSpeedMultiplier::new(1),
            }],
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::new(0),
            lanes: Vec::new(),
        };

        map.sort_and_dedup_scroll_speed_changes();

        assert_eq!(
            &map.scroll_speed_changes[..],
            &[ScrollSpeedChange {
                timestamp: MapTimestamp::from_millis(-1),
                multiplier: ScrollSpeedMultiplier::new(1),
            },][..]
        );
    }
}
