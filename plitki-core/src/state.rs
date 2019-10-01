//! Functionality related to managing the game state.
use alloc::{sync::Arc, vec::Vec};

use circular_queue::CircularQueue;

use crate::{
    map::Map,
    object::Object,
    scroll::{MapTimestampDifferenceTimesScrollSpeedMultiplier, ScrollSpeed},
    timing::{
        GameTimestamp, GameTimestampDifference, MapTimestamp, MapTimestampDifference,
        TimestampConverter,
    },
};

/// State of the game.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GameState {
    /// The map.
    ///
    /// Invariant: objects in each lane must be sorted by start timestamp and not overlap (which
    /// means they are sorted by both start and end timestamp).
    pub map: Arc<Map>,
    /// If `true`, heavily limit the FPS for testing.
    pub cap_fps: bool,
    /// Note scrolling speed.
    pub scroll_speed: ScrollSpeed,
    /// Converter between game timestamps and map timestamps.
    pub timestamp_converter: TimestampConverter,
    /// Contains states of the objects in lanes.
    pub lane_states: Vec<LaneState>,
    /// Contains immutable pre-computed information about objects.
    pub lane_caches: Vec<LaneCache>,
    /// A cache of positions for each scroll speed change timestamp.
    ///
    /// Indices into the cache are equal to indices into `map.scroll_speed_changes`.
    pub position_cache: Vec<CachedPosition>,
    /// Pre-computed timing lines.
    pub timing_lines: Vec<TimingLine>,
    /// Contains a number of last hits.
    ///
    /// Useful for implementing an error bar.
    pub last_hits: CircularQueue<Hit>,
}

/// States of a regular object.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum RegularObjectState {
    /// The object has not been hit.
    NotHit,
    /// The object has been hit.
    Hit {
        /// Difference between the actual hit timestamp and the object timestamp.
        difference: GameTimestampDifference,
    },
}

/// States of a long note object.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum LongNoteState {
    /// The long note has not been hit.
    NotHit,
    /// The long note is currently held.
    Held {
        /// Difference between the actual hit timestamp and the start timestamp.
        press_difference: GameTimestampDifference,
    },
    /// The long note has been hit, that is, held and released on time or later.
    Hit {
        /// Difference between the actual press timestamp and the start timestamp.
        press_difference: GameTimestampDifference,
        /// Difference between the actual release timestamp and the end timestamp.
        release_difference: GameTimestampDifference,
    },
    /// The long note has not been hit in time, or has been released early.
    Missed {
        // TODO: these two fields can only be either both None or both Some.
        /// The timestamp when the LN was released.
        ///
        /// This may be before the start timestamp if the player presses and releases the LN
        /// quickly during the hit window before the LN start.
        held_until: Option<MapTimestamp>,
        /// Difference between the actual press timestamp and the start timestamp, if the LN had
        /// been held.
        press_difference: Option<GameTimestampDifference>,
    },
}

/// State of an individual object.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum ObjectState {
    /// State of a regular object.
    Regular(RegularObjectState),
    /// State of a long note object.
    LongNote(LongNoteState),
}

/// States of the objects in a lane.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LaneState {
    /// States of the objects in this lane.
    pub object_states: Vec<ObjectState>,
    /// Index into `object_states` of the first object that is active, that is, can still be
    /// interacted with (hit or held). Used for incremental updates of the state: no object below
    /// this index can have its state changed.
    first_active_object: usize,
}

/// Cached information of an individual object.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum ObjectCache {
    /// Cached information of a regular object.
    Regular(RegularObjectCache),
    /// Cached information of a long note object.
    LongNote(LongNoteCache),
}

/// Cached information of a regular object.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct RegularObjectCache {
    /// Object position.
    ///
    /// Zero position corresponds to timestamp zero. The position takes scroll speed changes into
    /// account.
    pub position: MapTimestampDifferenceTimesScrollSpeedMultiplier,
}

/// Cached information of a long note.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct LongNoteCache {
    /// Start position.
    ///
    /// Zero position corresponds to timestamp zero. The position takes scroll speed changes into
    /// account.
    pub start_position: MapTimestampDifferenceTimesScrollSpeedMultiplier,
    /// End position.
    ///
    /// Zero position corresponds to timestamp zero. The position takes scroll speed changes into
    /// account.
    pub end_position: MapTimestampDifferenceTimesScrollSpeedMultiplier,
    // TODO: this needs special fields to account for SV direction changes mid-LN. At the very
    // least there should be fields for lowest and highest positions, and possibly fields for
    // positions at every SV direction change timestamp to render them properly.
}

/// Cached information of the objects in a lane.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LaneCache {
    /// Cached information of the objects in this lane.
    pub object_caches: Vec<ObjectCache>,
}

/// Cached position at a given timestamp.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct CachedPosition {
    /// Timestamp of the position.
    timestamp: MapTimestamp,
    /// Position at the timestamp, taking scroll speed changes into account.
    position: MapTimestampDifferenceTimesScrollSpeedMultiplier,
}

/// Timing line.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct TimingLine {
    /// Timestamp of the timing line.
    pub timestamp: MapTimestamp,
    /// Position at the timestamp, taking scroll speed changes into account.
    pub position: MapTimestampDifferenceTimesScrollSpeedMultiplier,
}

/// Information about a hit.
///
/// A hit is either a press or a release that resulted in a judgement.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Hit {
    /// Timestamp of the hit.
    pub timestamp: GameTimestamp,
    /// Difference between the actual press or release and the perfect timing.
    pub difference: GameTimestampDifference,
}

impl GameState {
    /// Creates a new `GameState` given a map.
    pub fn new(mut map: Map) -> Self {
        map.sort_and_dedup_scroll_speed_changes();

        // Compute the position cache.
        let zero_timestamp_scroll_speed_change_index = match map
            .scroll_speed_changes
            .binary_search_by_key(&MapTimestamp::from_milli_hundredths(0), |a| a.timestamp)
        {
            Ok(index) => Some(index),
            Err(index) => {
                if index == 0 {
                    None
                } else {
                    Some(index - 1)
                }
            }
        };

        let mut position_cache = Vec::with_capacity(map.scroll_speed_changes.len());

        // Compute positions for scroll speed changes before zero timestamp.
        if let Some(index) = zero_timestamp_scroll_speed_change_index {
            let mut last_timestamp = MapTimestamp::from_milli_hundredths(0);
            let mut last_position = MapTimestampDifferenceTimesScrollSpeedMultiplier(0);
            for i in (0..=index).rev() {
                let change = &map.scroll_speed_changes[i];
                let position =
                    last_position + (change.timestamp - last_timestamp) * change.multiplier;

                // Zero timestamp always corresponds to zero position. But we cache it anyway to
                // ensure the indices correspond to scroll speed change indices.
                if change.timestamp == MapTimestamp::from_milli_hundredths(0) {
                    assert_eq!(
                        position,
                        MapTimestampDifferenceTimesScrollSpeedMultiplier(0)
                    );
                }

                position_cache.push(CachedPosition {
                    timestamp: change.timestamp,
                    position,
                });

                last_timestamp = change.timestamp;
                last_position = position;
            }
        }
        position_cache.reverse();

        // Compute positions for scroll speed changes past zero timestamp.
        let mut last_timestamp = MapTimestamp::from_milli_hundredths(0);
        let mut last_position = MapTimestampDifferenceTimesScrollSpeedMultiplier(0);
        let mut last_multiplier = zero_timestamp_scroll_speed_change_index
            .map(|index| map.scroll_speed_changes[index].multiplier)
            .unwrap_or(map.initial_scroll_speed_multiplier);
        for i in zero_timestamp_scroll_speed_change_index
            .map(|x| x + 1)
            .unwrap_or(0)..map.scroll_speed_changes.len()
        {
            let change = &map.scroll_speed_changes[i];
            let position = last_position + (change.timestamp - last_timestamp) * last_multiplier;
            position_cache.push(CachedPosition {
                timestamp: change.timestamp,
                position,
            });

            last_timestamp = change.timestamp;
            last_position = position;
            last_multiplier = change.multiplier;
        }

        // Compute per-lane and per-object data.
        let mut lane_states = Vec::with_capacity(map.lanes.len());
        for lane in &mut map.lanes {
            // Ensure the objects are sorted by their start timestamp (GameState invariant).
            lane.objects.sort_unstable_by_key(Object::start_timestamp);
            // Ensure the objects don't overlap.
            for window in lane.objects.windows(2) {
                let (a, b) = (window[0], window[1]);
                // This does not permit an object at an LN end timestamp... Which is probably a
                // good thing, especially considering the traditional LN skins with an LN end.
                assert!(a.end_timestamp() < b.start_timestamp());
            }

            // Create states for the objects in this lane.
            let mut object_states = Vec::with_capacity(lane.objects.len());
            for object in &lane.objects {
                let state = match object {
                    Object::Regular { .. } => ObjectState::Regular(RegularObjectState::NotHit),
                    Object::LongNote { .. } => ObjectState::LongNote(LongNoteState::NotHit),
                };
                object_states.push(state);
            }
            lane_states.push(LaneState {
                object_states,
                first_active_object: 0,
            });
        }

        let timestamp_converter = TimestampConverter {
            global_offset: GameTimestampDifference::from_millis(0),
            local_offset: MapTimestampDifference::from_millis(0),
        };

        let mut state = Self {
            map: Arc::new(map),
            cap_fps: false,
            scroll_speed: ScrollSpeed(32),
            timestamp_converter,
            lane_states,
            position_cache,
            lane_caches: Vec::new(),
            timing_lines: Vec::new(),
            last_hits: CircularQueue::with_capacity(32),
        };

        // Now that we can use position_at_time(), fill in the lane caches.
        let mut lane_caches = Vec::with_capacity(state.map.lanes.len());
        for lane in &state.map.lanes {
            let mut object_caches = Vec::with_capacity(lane.objects.len());

            for object in &lane.objects {
                // TODO: this can be optimized to not do a binary search for every single
                // timestamp, based on the fact that we're iterating in ascending timestamp order.
                let cache = match *object {
                    Object::Regular { timestamp } => ObjectCache::Regular(RegularObjectCache {
                        position: state.position_at_time(timestamp),
                    }),
                    Object::LongNote { start, end } => ObjectCache::LongNote(LongNoteCache {
                        start_position: state.position_at_time(start),
                        end_position: state.position_at_time(end),
                    }),
                };
                object_caches.push(cache);
            }

            lane_caches.push(LaneCache { object_caches });
        }
        state.lane_caches = lane_caches;

        let mut timing_lines = Vec::new();
        for (i, timing_point) in state.map.timing_points.iter().enumerate() {
            // +1 and -1 ms like osu! does it. TODO: do we want this here?
            let end = if let Some(next_timing_point) = state.map.timing_points.get(i + 1) {
                next_timing_point.timestamp - MapTimestampDifference::from_millis(1)
            } else {
                state
                    .map
                    .lanes
                    .iter()
                    .filter_map(|lane| lane.objects.last())
                    .map(Object::end_timestamp)
                    .max()
                    .unwrap_or(timing_point.timestamp + MapTimestampDifference::from_millis(1))
            }
            .into_milli_hundredths();

            let step = (i64::from(timing_point.signature.beat_count)
                * i64::from(timing_point.beat_duration.into_milli_hundredths()))
            .max(0)
            .min(i64::from(i32::max_value())) as i32;

            let mut timestamp = timing_point.timestamp.into_milli_hundredths();
            while timestamp < end {
                {
                    let timestamp = MapTimestamp::from_milli_hundredths(timestamp);
                    timing_lines.push(TimingLine {
                        timestamp,
                        position: state.position_at_time(timestamp),
                    });
                }

                if step == 0 {
                    break;
                }

                timestamp = timestamp.saturating_add(step);
            }
        }
        state.timing_lines = timing_lines;

        state
    }

    /// Returns the map position at the given map timestamp.
    ///
    /// The position takes scroll speed changes into account.
    pub fn position_at_time(
        &self,
        timestamp: MapTimestamp,
    ) -> MapTimestampDifferenceTimesScrollSpeedMultiplier {
        if self.position_cache.is_empty() {
            return MapTimestampDifferenceTimesScrollSpeedMultiplier(0)
                + (timestamp - MapTimestamp::from_millis(0))
                    * self.map.initial_scroll_speed_multiplier;
        }

        match self
            .position_cache
            .binary_search_by_key(&timestamp, |x| x.timestamp)
        {
            Ok(index) => self.position_cache[index].position,
            Err(index) if index == 0 => {
                self.position_cache[0].position
                    + (timestamp - self.position_cache[0].timestamp)
                        * self.map.initial_scroll_speed_multiplier
            }
            Err(index) => {
                let cached_position = self.position_cache[index - 1];
                let multiplier = self.map.scroll_speed_changes[index - 1].multiplier;
                cached_position.position + (timestamp - cached_position.timestamp) * multiplier
            }
        }
    }

    /// Updates the state to match the `latest` state.
    ///
    /// # Panics
    ///
    /// Panics if the `latest` state is older than `self` (as indicated by `first_active_object` in
    /// one of the lane states being bigger than the one in the `latest` state).
    pub fn update_to_latest(&mut self, latest: &GameState) {
        self.cap_fps = latest.cap_fps;
        self.scroll_speed = latest.scroll_speed;
        self.timestamp_converter = latest.timestamp_converter;
        self.last_hits = latest.last_hits.clone();

        for (lane, latest_lane) in self.lane_states.iter_mut().zip(latest.lane_states.iter()) {
            assert!(lane.first_active_object <= latest_lane.first_active_object);

            // The range is inclusive because `first_active_object` can be an LN that's changing
            // states.
            if latest_lane.first_active_object < lane.object_states.len() {
                let update_range = lane.first_active_object..=latest_lane.first_active_object;
                lane.object_states[update_range.clone()]
                    .copy_from_slice(&latest_lane.object_states[update_range]);
            } else {
                // Exclusive range so we don't panic.
                let update_range = lane.first_active_object..latest_lane.first_active_object;
                lane.object_states[update_range.clone()]
                    .copy_from_slice(&latest_lane.object_states[update_range]);
            };

            lane.first_active_object = latest_lane.first_active_object;
        }
    }

    /// Returns `true` if the lane has active objects remaining.
    ///
    /// If this method returns `false`, then there are no more active objects in this lane. For
    /// example, this happens in the end of the map after the last object in the lane has been hit
    /// or missed.
    #[inline]
    pub fn has_active_objects(&self, lane: usize) -> bool {
        let lane_state = &self.lane_states[lane];
        lane_state.first_active_object < lane_state.object_states.len()
    }

    /// Updates the state.
    ///
    /// Essentially, this is a way to signal "some time has passed". Stuff like missed objects is
    /// handled here. This should be called every so often for all lanes.
    pub fn update(&mut self, lane: usize, timestamp: GameTimestamp) {
        if !self.has_active_objects(lane) {
            return;
        }

        let hit_window = GameTimestampDifference::from_millis(76);

        let map_timestamp = timestamp.to_map(&self.timestamp_converter);
        let map_hit_window = hit_window.to_map(&self.timestamp_converter);

        let lane_state = &mut self.lane_states[lane];
        let objects = &self.map.lanes[lane].objects[lane_state.first_active_object..];
        let object_states = &mut lane_state.object_states[lane_state.first_active_object..];

        for (object, state) in objects.iter().zip(object_states.iter_mut()) {
            // We want to increase first_active_object on every continue.
            lane_state.first_active_object += 1;

            if object.end_timestamp() + map_hit_window < map_timestamp {
                // The object can no longer be hit.
                // TODO: mark the object as missed

                if let ObjectState::LongNote(state) = state {
                    if let LongNoteState::Held { press_difference } = *state {
                        *state = LongNoteState::Hit {
                            press_difference,
                            release_difference: hit_window,
                        };

                        self.last_hits.push(Hit {
                            timestamp: (object.end_timestamp() + map_hit_window)
                                .to_game(&self.timestamp_converter),
                            difference: hit_window,
                        });
                    } else if *state == LongNoteState::NotHit {
                        // I kind of dislike how this branch exists both here and in the condition
                        // below...
                        *state = LongNoteState::Missed {
                            held_until: None,
                            press_difference: None,
                        };
                    } else {
                        unreachable!()
                    }
                }

                continue;
            }

            if object.start_timestamp() + map_hit_window < map_timestamp {
                // The object can no longer be hit.
                // TODO: mark the object as missed

                if let ObjectState::LongNote(state) = state {
                    // Mark this long note as missed.
                    if *state == LongNoteState::NotHit {
                        *state = LongNoteState::Missed {
                            held_until: None,
                            press_difference: None,
                        };
                        continue;
                    }

                    if let LongNoteState::Held { .. } = state {
                        // All good.
                    } else {
                        unreachable!()
                    }
                } else {
                    // Regular objects would be skipped over in the previous if.
                    unreachable!()
                }
            }

            // Didn't hit a continue, decrease it back.
            lane_state.first_active_object -= 1;
            break;
        }
    }

    /// Handles a key press.
    pub fn key_press(&mut self, lane: usize, timestamp: GameTimestamp) {
        self.update(lane, timestamp);
        if !self.has_active_objects(lane) {
            return;
        }

        let hit_window = GameTimestampDifference::from_millis(76);

        let map_timestamp = timestamp.to_map(&self.timestamp_converter);
        let map_hit_window = hit_window.to_map(&self.timestamp_converter);

        let lane_state = &mut self.lane_states[lane];
        let object = &self.map.lanes[lane].objects[lane_state.first_active_object];
        let state = &mut lane_state.object_states[lane_state.first_active_object];

        if map_timestamp >= object.start_timestamp() - map_hit_window {
            // The object can be hit.
            let difference =
                (map_timestamp - object.start_timestamp()).to_game(&self.timestamp_converter);

            match state {
                ObjectState::Regular(ref mut state) => {
                    *state = RegularObjectState::Hit { difference };

                    // This object is no longer active.
                    lane_state.first_active_object += 1;
                }
                ObjectState::LongNote(ref mut state) => {
                    *state = LongNoteState::Held {
                        press_difference: difference,
                    }
                }
            }

            self.last_hits.push(Hit {
                timestamp,
                difference,
            });
        }
    }

    /// Handles a key release.
    pub fn key_release(&mut self, lane: usize, timestamp: GameTimestamp) {
        self.update(lane, timestamp);
        if !self.has_active_objects(lane) {
            return;
        }

        let hit_window = GameTimestampDifference::from_millis(76);

        let map_timestamp = timestamp.to_map(&self.timestamp_converter);
        let map_hit_window = hit_window.to_map(&self.timestamp_converter);

        let lane_state = &mut self.lane_states[lane];
        let object = &self.map.lanes[lane].objects[lane_state.first_active_object];
        let state = &mut lane_state.object_states[lane_state.first_active_object];

        if let ObjectState::LongNote(state) = state {
            if let LongNoteState::Held { press_difference } = *state {
                if map_timestamp >= object.end_timestamp() - map_hit_window {
                    let difference =
                        (map_timestamp - object.end_timestamp()).to_game(&self.timestamp_converter);

                    *state = LongNoteState::Hit {
                        press_difference,
                        release_difference: difference,
                    };

                    self.last_hits.push(Hit {
                        timestamp,
                        difference,
                    });
                } else {
                    // Released too early.
                    *state = LongNoteState::Missed {
                        held_until: Some(map_timestamp),
                        press_difference: Some(press_difference),
                    };
                }

                // This object is no longer active.
                lane_state.first_active_object += 1;
            }
        }
    }
}

impl ObjectState {
    /// Returns `true` if the object has been hit.
    pub fn is_hit(&self) -> bool {
        match self {
            Self::Regular(RegularObjectState::Hit { .. })
            | Self::LongNote(LongNoteState::Hit { .. }) => true,
            _ => false,
        }
    }
}

impl ObjectCache {
    /// Returns the cached start position of the object.
    ///
    /// This is the first position at which this object is visible.
    #[inline]
    pub fn start_position(&self) -> MapTimestampDifferenceTimesScrollSpeedMultiplier {
        match *self {
            ObjectCache::Regular(RegularObjectCache { position }) => position,
            ObjectCache::LongNote(LongNoteCache { start_position, .. }) => start_position,
        }
    }

    /// Returns the cached end position of the object.
    ///
    /// This is the last position at which this object is visible.
    #[inline]
    pub fn end_position(&self) -> MapTimestampDifferenceTimesScrollSpeedMultiplier {
        match *self {
            ObjectCache::Regular(RegularObjectCache { position }) => position,
            ObjectCache::LongNote(LongNoteCache { end_position, .. }) => end_position,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        map::{Lane, ScrollSpeedChange, TimeSignature, TimingPoint},
        scroll::ScrollSpeedMultiplier,
    };
    use alloc::vec;

    #[test]
    fn game_state_objects_sorted() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: Vec::new(),
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::default(),
            lanes: vec![
                Lane {
                    objects: vec![
                        Object::Regular {
                            timestamp: MapTimestamp::from_millis(10),
                        },
                        Object::Regular {
                            timestamp: MapTimestamp::from_millis(0),
                        },
                    ],
                },
                Lane {
                    objects: vec![
                        Object::Regular {
                            timestamp: MapTimestamp::from_millis(10),
                        },
                        Object::LongNote {
                            start: MapTimestamp::from_millis(0),
                            end: MapTimestamp::from_millis(9),
                        },
                    ],
                },
                Lane {
                    objects: vec![
                        Object::LongNote {
                            start: MapTimestamp::from_millis(7),
                            end: MapTimestamp::from_millis(10),
                        },
                        Object::Regular {
                            timestamp: MapTimestamp::from_millis(0),
                        },
                    ],
                },
                Lane {
                    objects: vec![
                        Object::LongNote {
                            start: MapTimestamp::from_millis(10),
                            end: MapTimestamp::from_millis(7),
                        },
                        Object::LongNote {
                            start: MapTimestamp::from_millis(0),
                            end: MapTimestamp::from_millis(6),
                        },
                    ],
                },
            ],
        };

        let state = GameState::new(map);

        for lane in &state.map.lanes {
            for xs in lane.objects.windows(2) {
                let (a, b) = (xs[0], xs[1]);
                assert!(a.start_timestamp() < b.start_timestamp());
                assert!(a.end_timestamp() < b.end_timestamp());
                assert!(a.end_timestamp() < b.start_timestamp());
            }
        }
    }

    #[test]
    fn game_state_regular_hit() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: Vec::new(),
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::default(),
            lanes: vec![Lane {
                objects: vec![
                    Object::Regular {
                        timestamp: MapTimestamp::from_millis(0),
                    },
                    Object::Regular {
                        timestamp: MapTimestamp::from_millis(10_000),
                    },
                ],
            }],
        };

        let mut state = GameState::new(map);
        state.key_press(0, GameTimestamp::from_millis(10_000));

        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[
                ObjectState::Regular(RegularObjectState::NotHit),
                ObjectState::Regular(RegularObjectState::Hit {
                    difference: GameTimestampDifference::from_millis(0)
                }),
            ][..]
        );

        let mut hits = CircularQueue::with_capacity(1);
        hits.push(Hit {
            timestamp: GameTimestamp::from_millis(10_000),
            difference: GameTimestampDifference::from_millis(0),
        });
        assert_eq!(state.last_hits, hits);
    }

    #[test]
    fn game_state_regular_hit_note_lock() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: Vec::new(),
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::default(),
            lanes: vec![Lane {
                objects: vec![
                    Object::Regular {
                        timestamp: MapTimestamp::from_millis(0),
                    },
                    Object::Regular {
                        timestamp: MapTimestamp::from_millis(10),
                    },
                ],
            }],
        };

        let mut state = GameState::new(map);
        state.key_press(0, GameTimestamp::from_millis(10));

        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[
                ObjectState::Regular(RegularObjectState::Hit {
                    difference: GameTimestampDifference::from_millis(10)
                }),
                ObjectState::Regular(RegularObjectState::NotHit),
            ][..]
        );

        let mut hits = CircularQueue::with_capacity(1);
        hits.push(Hit {
            timestamp: GameTimestamp::from_millis(10),
            difference: GameTimestampDifference::from_millis(10),
        });
        assert_eq!(state.last_hits, hits);
    }

    #[test]
    fn game_state_long_note_hit() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: Vec::new(),
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::default(),
            lanes: vec![Lane {
                objects: vec![Object::LongNote {
                    start: MapTimestamp::from_millis(5_000),
                    end: MapTimestamp::from_millis(10_000),
                }],
            }],
        };

        let mut state = GameState::new(map);
        state.key_press(0, GameTimestamp::from_millis(5_000));
        state.key_release(0, GameTimestamp::from_millis(10_000));

        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[ObjectState::LongNote(LongNoteState::Hit {
                press_difference: GameTimestampDifference::from_millis(0),
                release_difference: GameTimestampDifference::from_millis(0)
            })][..]
        );

        let mut hits = CircularQueue::with_capacity(2);
        hits.push(Hit {
            timestamp: GameTimestamp::from_millis(5_000),
            difference: GameTimestampDifference::from_millis(0),
        });
        hits.push(Hit {
            timestamp: GameTimestamp::from_millis(10_000),
            difference: GameTimestampDifference::from_millis(0),
        });
        assert_eq!(state.last_hits, hits);
    }

    #[test]
    fn game_state_long_note_released_early() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: Vec::new(),
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::default(),
            lanes: vec![Lane {
                objects: vec![Object::LongNote {
                    start: MapTimestamp::from_millis(5_000),
                    end: MapTimestamp::from_millis(10_000),
                }],
            }],
        };

        let mut state = GameState::new(map);
        state.key_press(0, GameTimestamp::from_millis(5_000));
        state.key_release(0, GameTimestamp::from_millis(7_000));

        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[ObjectState::LongNote(LongNoteState::Missed {
                held_until: Some(
                    GameTimestamp::from_millis(7_000).to_map(&state.timestamp_converter)
                ),
                press_difference: Some(GameTimestampDifference::from_millis(0)),
            })][..]
        );

        let mut hits = CircularQueue::with_capacity(1);
        hits.push(Hit {
            timestamp: GameTimestamp::from_millis(5_000),
            difference: GameTimestampDifference::from_millis(0),
        });
        assert_eq!(state.last_hits, hits);
    }

    #[test]
    fn game_state_long_note_released_late() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: Vec::new(),
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::default(),
            lanes: vec![Lane {
                objects: vec![Object::LongNote {
                    start: MapTimestamp::from_millis(5_000),
                    end: MapTimestamp::from_millis(10_000),
                }],
            }],
        };

        let mut state = GameState::new(map);
        state.key_press(0, GameTimestamp::from_millis(5_000));
        state.key_release(0, GameTimestamp::from_millis(15_000));

        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[ObjectState::LongNote(LongNoteState::Hit {
                press_difference: GameTimestampDifference::from_millis(0),
                release_difference: GameTimestampDifference::from_millis(76), // TODO: un-hardcode
            })][..]
        );

        let mut hits = CircularQueue::with_capacity(2);
        hits.push(Hit {
            timestamp: GameTimestamp::from_millis(5_000),
            difference: GameTimestampDifference::from_millis(0),
        });
        hits.push(Hit {
            timestamp: GameTimestamp::from_millis(10_076),
            difference: GameTimestampDifference::from_millis(76),
        });
        assert_eq!(state.last_hits, hits);
    }

    #[test]
    fn game_state_long_note_pressed_late() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: Vec::new(),
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::default(),
            lanes: vec![Lane {
                objects: vec![Object::LongNote {
                    start: MapTimestamp::from_millis(5_000),
                    end: MapTimestamp::from_millis(10_000),
                }],
            }],
        };

        let mut state = GameState::new(map);
        state.key_press(0, GameTimestamp::from_millis(7_000));
        state.key_release(0, GameTimestamp::from_millis(10_000));

        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[ObjectState::LongNote(LongNoteState::Missed {
                held_until: None,
                press_difference: None,
            })][..]
        );

        let hits = CircularQueue::with_capacity(1);
        assert_eq!(state.last_hits, hits);
    }

    #[test]
    fn game_state_key_press_long_note_held() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: Vec::new(),
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::default(),
            lanes: vec![Lane {
                objects: vec![Object::LongNote {
                    start: MapTimestamp::from_millis(5_000),
                    end: MapTimestamp::from_millis(10_000),
                }],
            }],
        };

        let mut state = GameState::new(map);
        state.key_press(0, GameTimestamp::from_millis(5_000));

        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[ObjectState::LongNote(LongNoteState::Held {
                press_difference: GameTimestampDifference::from_millis(0)
            })][..]
        );

        let mut hits = CircularQueue::with_capacity(1);
        hits.push(Hit {
            timestamp: GameTimestamp::from_millis(5_000),
            difference: GameTimestampDifference::from_millis(0),
        });
        assert_eq!(state.last_hits, hits);
    }

    #[test]
    fn game_state_long_note_missed_update_after_end_time() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: Vec::new(),
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::default(),
            lanes: vec![Lane {
                objects: vec![Object::LongNote {
                    start: MapTimestamp::from_millis(5_000),
                    end: MapTimestamp::from_millis(10_000),
                }],
            }],
        };

        let mut state = GameState::new(map);
        state.update(0, GameTimestamp::from_millis(15_000));

        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[ObjectState::LongNote(LongNoteState::Missed {
                held_until: None,
                press_difference: None,
            })][..]
        );

        let hits = CircularQueue::with_capacity(1);
        assert_eq!(state.last_hits, hits);
    }

    #[test]
    fn game_state_global_offset() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: Vec::new(),
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::default(),
            lanes: vec![Lane {
                objects: vec![Object::Regular {
                    timestamp: MapTimestamp::from_millis(20_000),
                }],
            }],
        };

        let mut state = GameState::new(map);
        state.timestamp_converter.global_offset = GameTimestampDifference::from_millis(10_000);

        state.key_press(0, GameTimestamp::from_millis(0));
        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[ObjectState::Regular(RegularObjectState::NotHit)][..]
        );

        state.key_press(0, GameTimestamp::from_millis(10_000));
        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[ObjectState::Regular(RegularObjectState::Hit {
                difference: GameTimestampDifference::from_millis(0)
            })][..]
        );

        let mut hits = CircularQueue::with_capacity(1);
        hits.push(Hit {
            timestamp: GameTimestamp::from_millis(10_000),
            difference: GameTimestampDifference::from_millis(0),
        });
        assert_eq!(state.last_hits, hits);
    }

    #[test]
    fn game_state_local_offset() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: Vec::new(),
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::default(),
            lanes: vec![Lane {
                objects: vec![Object::Regular {
                    timestamp: MapTimestamp::from_millis(20_000),
                }],
            }],
        };

        let mut state = GameState::new(map);
        state.timestamp_converter.local_offset = MapTimestampDifference::from_millis(-10_000);

        state.key_press(0, GameTimestamp::from_millis(0));
        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[ObjectState::Regular(RegularObjectState::NotHit)][..]
        );

        state.key_press(0, GameTimestamp::from_millis(10_000));
        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[ObjectState::Regular(RegularObjectState::Hit {
                difference: GameTimestampDifference::from_millis(0)
            })][..]
        );

        let mut hits = CircularQueue::with_capacity(1);
        hits.push(Hit {
            timestamp: GameTimestamp::from_millis(10_000),
            difference: GameTimestampDifference::from_millis(0),
        });
        assert_eq!(state.last_hits, hits);
    }

    #[test]
    fn game_state_update_to_latest() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: Vec::new(),
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::default(),
            lanes: vec![Lane {
                objects: vec![
                    Object::Regular {
                        timestamp: MapTimestamp::from_millis(20_000),
                    },
                    Object::Regular {
                        timestamp: MapTimestamp::from_millis(30_000),
                    },
                ],
            }],
        };

        let mut state = GameState::new(map);

        let mut state2 = state.clone();
        state2.cap_fps = true;
        state2.timestamp_converter.global_offset = GameTimestampDifference::from_millis(10_000);
        state2.timestamp_converter.local_offset = MapTimestampDifference::from_millis(20_000);
        state2.scroll_speed = ScrollSpeed(5);
        state2.lane_states[0].first_active_object = 1;
        state2.lane_states[0].object_states[0] = ObjectState::Regular(RegularObjectState::Hit {
            difference: GameTimestampDifference::from_millis(0),
        });
        state2.last_hits.push(Hit {
            timestamp: GameTimestamp::from_millis(10_000),
            difference: GameTimestampDifference::from_millis(0),
        });
        assert_ne!(state, state2);

        state.update_to_latest(&state2);
        assert_eq!(state, state2);
    }

    #[test]
    fn game_state_update_to_latest_last_first_active_object() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: Vec::new(),
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::default(),
            lanes: vec![Lane {
                objects: vec![Object::Regular {
                    timestamp: MapTimestamp::from_millis(20_000),
                }],
            }],
        };

        let mut state = GameState::new(map);

        let mut state2 = state.clone();
        state2.lane_states[0].first_active_object = 1;
        assert_ne!(state, state2);

        state.update_to_latest(&state2);
        assert_eq!(state, state2);
    }

    #[test]
    fn game_state_update_to_latest_zero_objects() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: Vec::new(),
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::default(),
            lanes: vec![Lane { objects: vec![] }],
        };

        let mut state = GameState::new(map);
        let state2 = state.clone();

        state.update_to_latest(&state2);
        assert_eq!(state, state2);
    }

    #[test]
    fn game_state_position_cache() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: vec![
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(-2),
                    multiplier: ScrollSpeedMultiplier::new(4),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(-1),
                    multiplier: ScrollSpeedMultiplier::new(5),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(0),
                    multiplier: ScrollSpeedMultiplier::new(1),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(1),
                    multiplier: ScrollSpeedMultiplier::new(3),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(3),
                    multiplier: ScrollSpeedMultiplier::new(5),
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
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::default(),
            lanes: vec![Lane { objects: vec![] }],
        };

        let state = GameState::new(map);

        assert_eq!(
            state.position_cache.len(),
            state.map.scroll_speed_changes.len()
        );
        assert_eq!(
            &state.position_cache[..],
            &[
                CachedPosition {
                    timestamp: MapTimestamp::from_millis(-2),
                    position: MapTimestampDifferenceTimesScrollSpeedMultiplier(-900)
                },
                CachedPosition {
                    timestamp: MapTimestamp::from_millis(-1),
                    position: MapTimestampDifferenceTimesScrollSpeedMultiplier(-500)
                },
                CachedPosition {
                    timestamp: MapTimestamp::from_millis(0),
                    position: MapTimestampDifferenceTimesScrollSpeedMultiplier(0)
                },
                CachedPosition {
                    timestamp: MapTimestamp::from_millis(1),
                    position: MapTimestampDifferenceTimesScrollSpeedMultiplier(100)
                },
                CachedPosition {
                    timestamp: MapTimestamp::from_millis(3),
                    position: MapTimestampDifferenceTimesScrollSpeedMultiplier(700)
                },
                CachedPosition {
                    timestamp: MapTimestamp::from_millis(4),
                    position: MapTimestampDifferenceTimesScrollSpeedMultiplier(1200)
                },
                CachedPosition {
                    timestamp: MapTimestamp::from_millis(5),
                    position: MapTimestampDifferenceTimesScrollSpeedMultiplier(1900)
                },
            ][..]
        );
    }

    #[test]
    fn game_state_position_at_time() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: vec![
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(-2),
                    multiplier: ScrollSpeedMultiplier::new(4),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(-1),
                    multiplier: ScrollSpeedMultiplier::new(5),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(0),
                    multiplier: ScrollSpeedMultiplier::new(1),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(1),
                    multiplier: ScrollSpeedMultiplier::new(3),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(3),
                    multiplier: ScrollSpeedMultiplier::new(5),
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
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::new(1),
            lanes: vec![Lane { objects: vec![] }],
        };

        let state = GameState::new(map);

        assert_eq!(
            state.position_at_time(MapTimestamp::from_milli_hundredths(-250)),
            MapTimestampDifferenceTimesScrollSpeedMultiplier(-950)
        );
        assert_eq!(
            state.position_at_time(MapTimestamp::from_milli_hundredths(-50)),
            MapTimestampDifferenceTimesScrollSpeedMultiplier(-250)
        );
        assert_eq!(
            state.position_at_time(MapTimestamp::from_milli_hundredths(300)),
            MapTimestampDifferenceTimesScrollSpeedMultiplier(700)
        );
        assert_eq!(
            state.position_at_time(MapTimestamp::from_milli_hundredths(350)),
            MapTimestampDifferenceTimesScrollSpeedMultiplier(950)
        );
        assert_eq!(
            state.position_at_time(MapTimestamp::from_milli_hundredths(600)),
            MapTimestampDifferenceTimesScrollSpeedMultiplier(2700)
        );
    }

    #[test]
    fn game_state_lane_caches() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: vec![
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(-2),
                    multiplier: ScrollSpeedMultiplier::new(4),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(-1),
                    multiplier: ScrollSpeedMultiplier::new(5),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(0),
                    multiplier: ScrollSpeedMultiplier::new(1),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(1),
                    multiplier: ScrollSpeedMultiplier::new(3),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(3),
                    multiplier: ScrollSpeedMultiplier::new(5),
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
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::new(1),
            lanes: vec![
                Lane {
                    objects: vec![
                        Object::Regular {
                            timestamp: MapTimestamp::from_milli_hundredths(-250),
                        },
                        Object::LongNote {
                            start: MapTimestamp::from_milli_hundredths(300),
                            end: MapTimestamp::from_milli_hundredths(600),
                        },
                    ],
                },
                Lane {
                    objects: vec![Object::LongNote {
                        start: MapTimestamp::from_milli_hundredths(-50),
                        end: MapTimestamp::from_milli_hundredths(350),
                    }],
                },
            ],
        };

        let state = GameState::new(map);

        assert_eq!(
            state.lane_caches[0].object_caches[0],
            ObjectCache::Regular(RegularObjectCache {
                position: MapTimestampDifferenceTimesScrollSpeedMultiplier(-950)
            })
        );
        assert_eq!(
            state.lane_caches[0].object_caches[1],
            ObjectCache::LongNote(LongNoteCache {
                start_position: MapTimestampDifferenceTimesScrollSpeedMultiplier(700),
                end_position: MapTimestampDifferenceTimesScrollSpeedMultiplier(2700)
            })
        );
        assert_eq!(
            state.lane_caches[1].object_caches[0],
            ObjectCache::LongNote(LongNoteCache {
                start_position: MapTimestampDifferenceTimesScrollSpeedMultiplier(-250),
                end_position: MapTimestampDifferenceTimesScrollSpeedMultiplier(950)
            })
        );
    }

    #[test]
    fn game_state_timing_lines() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: vec![
                TimingPoint {
                    timestamp: MapTimestamp::from_millis(-1),
                    beat_duration: MapTimestampDifference::from_millis(10),
                    signature: TimeSignature {
                        beat_count: 4,
                        beat_unit: 4,
                    },
                },
                TimingPoint {
                    timestamp: MapTimestamp::from_millis(0),
                    beat_duration: MapTimestampDifference::from_millis(10),
                    signature: TimeSignature {
                        beat_count: 4,
                        beat_unit: 4,
                    },
                },
                TimingPoint {
                    timestamp: MapTimestamp::from_millis(200),
                    beat_duration: MapTimestampDifference::from_millis(5),
                    signature: TimeSignature {
                        beat_count: 3,
                        beat_unit: 4,
                    },
                },
                TimingPoint {
                    timestamp: MapTimestamp::from_millis(220),
                    beat_duration: MapTimestampDifference::from_millis(0),
                    signature: TimeSignature {
                        beat_count: 4,
                        beat_unit: 4,
                    },
                },
                TimingPoint {
                    timestamp: MapTimestamp::from_millis(240),
                    beat_duration: MapTimestampDifference::from_millis(-10),
                    signature: TimeSignature {
                        beat_count: 4,
                        beat_unit: 4,
                    },
                },
                TimingPoint {
                    timestamp: MapTimestamp::from_millis(260),
                    beat_duration: MapTimestampDifference::from_millis(10),
                    signature: TimeSignature {
                        beat_count: 4,
                        beat_unit: 4,
                    },
                },
            ],
            scroll_speed_changes: vec![],
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::new(1),
            lanes: vec![Lane { objects: vec![] }],
        };

        let state = GameState::new(map);

        assert_eq!(
            &state.timing_lines[..],
            &[
                TimingLine {
                    timestamp: MapTimestamp::from_millis(0),
                    position: MapTimestampDifferenceTimesScrollSpeedMultiplier(0),
                },
                TimingLine {
                    timestamp: MapTimestamp::from_millis(40),
                    position: MapTimestampDifferenceTimesScrollSpeedMultiplier(4000),
                },
                TimingLine {
                    timestamp: MapTimestamp::from_millis(80),
                    position: MapTimestampDifferenceTimesScrollSpeedMultiplier(8000),
                },
                TimingLine {
                    timestamp: MapTimestamp::from_millis(120),
                    position: MapTimestampDifferenceTimesScrollSpeedMultiplier(120_00),
                },
                TimingLine {
                    timestamp: MapTimestamp::from_millis(160),
                    position: MapTimestampDifferenceTimesScrollSpeedMultiplier(160_00),
                },
                TimingLine {
                    timestamp: MapTimestamp::from_millis(200),
                    position: MapTimestampDifferenceTimesScrollSpeedMultiplier(200_00),
                },
                TimingLine {
                    timestamp: MapTimestamp::from_millis(215),
                    position: MapTimestampDifferenceTimesScrollSpeedMultiplier(215_00),
                },
                TimingLine {
                    timestamp: MapTimestamp::from_millis(220),
                    position: MapTimestampDifferenceTimesScrollSpeedMultiplier(220_00),
                },
                TimingLine {
                    timestamp: MapTimestamp::from_millis(240),
                    position: MapTimestampDifferenceTimesScrollSpeedMultiplier(240_00),
                },
                TimingLine {
                    timestamp: MapTimestamp::from_millis(260),
                    position: MapTimestampDifferenceTimesScrollSpeedMultiplier(260_00),
                },
            ][..],
        );
    }

    #[test]
    fn object_cache_methods() {
        let regular = ObjectCache::Regular(RegularObjectCache {
            position: MapTimestampDifferenceTimesScrollSpeedMultiplier(10),
        });
        assert_eq!(
            regular.start_position(),
            MapTimestampDifferenceTimesScrollSpeedMultiplier(10)
        );
        assert_eq!(
            regular.end_position(),
            MapTimestampDifferenceTimesScrollSpeedMultiplier(10)
        );

        let ln = ObjectCache::LongNote(LongNoteCache {
            start_position: MapTimestampDifferenceTimesScrollSpeedMultiplier(20),
            end_position: MapTimestampDifferenceTimesScrollSpeedMultiplier(30),
        });
        assert_eq!(
            ln.start_position(),
            MapTimestampDifferenceTimesScrollSpeedMultiplier(20)
        );
        assert_eq!(
            ln.end_position(),
            MapTimestampDifferenceTimesScrollSpeedMultiplier(30)
        );
    }
}
