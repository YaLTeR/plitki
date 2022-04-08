//! Functionality related to managing the game state.
use alloc::{sync::Arc, vec::Vec};
use core::cmp::Ord;

use circular_queue::CircularQueue;

use crate::{
    map::Map,
    object::Object,
    scroll::Position,
    timing::{
        GameTimestamp, GameTimestampDifference, MapTimestamp, MapTimestampDifference,
        TimestampConverter,
    },
};

/// State of the game.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GameState {
    /// The immutable part of the game state.
    ///
    /// Stored in an [`Arc`] so it doesn't have to be cloned.
    pub immutable: Arc<ImmutableGameState>,
    /// Largest timestamp difference that will be considered a hit.
    ///
    /// Notes past the hit window will be considered missed.
    pub hit_window: GameTimestampDifference,
    /// Converter between game timestamps and map timestamps.
    pub timestamp_converter: TimestampConverter,
    /// Contains states of the objects in lanes.
    pub lane_states: Vec<LaneState>,
    /// Contains a number of last hits.
    ///
    /// Useful for implementing an error bar.
    pub last_hits: CircularQueue<Hit>,
}

/// Immutable part of the game state.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ImmutableGameState {
    /// The map.
    ///
    /// Invariant: objects in each lane must be sorted by start timestamp and not overlap (which
    /// means they are sorted by both start and end timestamp).
    pub map: Map,
    /// Contains immutable pre-computed information about objects.
    pub lane_caches: Vec<LaneCache>,
    /// A cache of positions for each scroll speed change timestamp.
    ///
    /// Indices into the cache are equal to indices into [`Map::scroll_speed_changes`].
    pub position_cache: Vec<CachedPosition>,
    /// Pre-computed timing lines.
    pub timing_lines: Vec<TimingLine>,

    /// Regular object which has the minimum position.
    pub min_regular: Option<RegularObjectCache>,
    /// Long note, one of the ends of which has the minimum position.
    pub min_long_note: Option<LongNoteCache>,
    /// Regular object which has the minimum position.
    pub max_regular: Option<RegularObjectCache>,
    /// Long note, one of the ends of which has the maximum position.
    pub max_long_note: Option<LongNoteCache>,

    /// Minimum position across all objects.
    pub min_position: Option<Position>,
    /// Maximum position across all objects.
    pub max_position: Option<Position>,

    /// Timing line with the maximum position.
    pub max_timing_line: Option<TimingLine>,
}

/// States of a regular object.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum RegularObjectState {
    /// The object has not been hit yet.
    NotHit,
    /// The object has been hit.
    Hit {
        /// Difference between the actual hit timestamp and the object timestamp.
        difference: GameTimestampDifference,
    },
    /// The object has been missed, that is, has not been hit on time.
    Missed,
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
    pub position: Position,
}

/// Cached information of a long note.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct LongNoteCache {
    /// Start position.
    ///
    /// Zero position corresponds to timestamp zero. The position takes scroll speed changes into
    /// account.
    pub start_position: Position,
    /// End position.
    ///
    /// Zero position corresponds to timestamp zero. The position takes scroll speed changes into
    /// account.
    pub end_position: Position,
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
    position: Position,
}

/// Timing line.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct TimingLine {
    /// Timestamp of the timing line.
    pub timestamp: MapTimestamp,
    /// Position at the timestamp, taking scroll speed changes into account.
    pub position: Position,
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

/// An error returned from [`GameState::new()`].
#[derive(Clone, PartialEq, Eq)]
pub enum GameStateCreationError {
    /// The map has overlapping objects.
    ///
    /// The tuple contains the map, as well as two overlapping objects.
    MapHasOverlappingObjects(Map, Object, Object),
}

// Manual implementation to avoid printing the whole `Map`.
impl core::fmt::Debug for GameStateCreationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MapHasOverlappingObjects(_map, a, b) => f
                .debug_tuple("MapHasOverlappingObjects")
                .field(a)
                .field(b)
                .finish(),
        }
    }
}

impl GameState {
    /// Creates a new `GameState` given a map and a hit window.
    pub fn new(
        mut map: Map,
        hit_window: GameTimestampDifference,
    ) -> Result<Self, GameStateCreationError> {
        map.sort_and_dedup_scroll_speed_changes();
        map.sort_and_dedup_timing_points();

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
            let mut last_position = Position::zero();
            for i in (0..=index).rev() {
                let change = &map.scroll_speed_changes[i];
                let position =
                    last_position + (change.timestamp - last_timestamp) * change.multiplier;

                // Zero timestamp always corresponds to zero position. But we cache it anyway to
                // ensure the indices correspond to scroll speed change indices.
                if change.timestamp == MapTimestamp::from_milli_hundredths(0) {
                    assert_eq!(position, Position::zero());
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
        let mut last_position = Position::zero();
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
        let mut lane_states = Vec::with_capacity(map.lane_count());
        for lane in &mut map.lanes {
            // Ensure the objects are sorted by their start timestamp (GameState invariant).
            lane.objects.sort_unstable_by_key(Object::start_timestamp);
            // Ensure the objects don't overlap.
            for window in lane.objects.windows(2) {
                let (a, b) = (window[0], window[1]);
                // This does not permit an object at an LN end timestamp... Which is probably a
                // good thing, especially considering the traditional LN skins with an LN end.
                if a.end_timestamp() >= b.start_timestamp() {
                    return Err(GameStateCreationError::MapHasOverlappingObjects(map, a, b));
                }
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

        let mut immutable = ImmutableGameState {
            map,
            position_cache,
            lane_caches: Vec::new(),
            timing_lines: Vec::new(),
            min_regular: None,
            min_long_note: None,
            max_regular: None,
            max_long_note: None,
            min_position: None,
            max_position: None,
            max_timing_line: None,
        };

        let mut min_regular_position = None;
        let mut min_long_note_position = None;
        let mut max_regular_position = None;
        let mut max_long_note_position = None;

        // Now that we can use position_at_time(), fill in the lane caches.
        let mut lane_caches = Vec::with_capacity(immutable.lane_count());
        for lane in &immutable.map.lanes {
            let mut object_caches = Vec::with_capacity(lane.objects.len());

            for object in &lane.objects {
                // TODO: this can be optimized to not do a binary search for every single
                // timestamp, based on the fact that we're iterating in ascending timestamp order.
                let cache = match *object {
                    Object::Regular { timestamp } => ObjectCache::Regular(RegularObjectCache {
                        position: immutable.position_at_time(timestamp),
                    }),
                    Object::LongNote { start, end } => ObjectCache::LongNote(LongNoteCache {
                        start_position: immutable.position_at_time(start),
                        end_position: immutable.position_at_time(end),
                    }),
                };

                // Update minimum and maximum position.
                let min = cache.start_position().min(cache.end_position());
                match cache {
                    ObjectCache::Regular(cache) => {
                        if let Some(min_position) = min_regular_position {
                            if min < min_position {
                                min_regular_position = Some(min);
                                immutable.min_regular = Some(cache);
                            }
                        } else {
                            min_regular_position = Some(min);
                            immutable.min_regular = Some(cache);
                        }
                    }
                    ObjectCache::LongNote(cache) => {
                        if let Some(min_position) = min_long_note_position {
                            if min < min_position {
                                min_long_note_position = Some(min);
                                immutable.min_long_note = Some(cache);
                            }
                        } else {
                            min_long_note_position = Some(min);
                            immutable.min_long_note = Some(cache);
                        }
                    }
                }
                if let Some(ref mut min_position) = immutable.min_position {
                    *min_position = (*min_position).min(min);
                } else {
                    immutable.min_position = Some(min);
                }

                let max = cache.start_position().max(cache.end_position());
                match cache {
                    ObjectCache::Regular(cache) => {
                        if let Some(max_position) = max_regular_position {
                            if max > max_position {
                                max_regular_position = Some(max);
                                immutable.max_regular = Some(cache);
                            }
                        } else {
                            max_regular_position = Some(max);
                            immutable.max_regular = Some(cache);
                        }
                    }
                    ObjectCache::LongNote(cache) => {
                        if let Some(max_position) = max_long_note_position {
                            if max > max_position {
                                max_long_note_position = Some(max);
                                immutable.max_long_note = Some(cache);
                            }
                        } else {
                            max_long_note_position = Some(max);
                            immutable.max_long_note = Some(cache);
                        }
                    }
                }
                if let Some(ref mut max_position) = immutable.max_position {
                    *max_position = (*max_position).max(max);
                } else {
                    immutable.max_position = Some(max);
                }

                object_caches.push(cache);
            }

            lane_caches.push(LaneCache { object_caches });
        }
        immutable.lane_caches = lane_caches;

        let mut timing_lines = Vec::new();
        for (i, timing_point) in immutable.map.timing_points.iter().enumerate() {
            // +1 and -1 ms like osu! does it. TODO: do we want this here?
            let end = if let Some(next_timing_point) = immutable.map.timing_points.get(i + 1) {
                next_timing_point.timestamp - MapTimestampDifference::from_millis(1)
            } else {
                immutable.last_timestamp().unwrap_or_else(|| {
                    timing_point
                        .timestamp
                        .saturating_add(MapTimestampDifference::from_millis(1))
                })
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
                    let line = TimingLine {
                        timestamp,
                        position: immutable.position_at_time(timestamp),
                    };
                    timing_lines.push(line);

                    if let Some(ref mut max_timing_line) = immutable.max_timing_line {
                        if max_timing_line.position < line.position {
                            *max_timing_line = line;
                        }
                    } else {
                        immutable.max_timing_line = Some(line);
                    }
                }

                if step == 0 {
                    break;
                }

                timestamp = timestamp.saturating_add(step);
            }
        }
        immutable.timing_lines = timing_lines;

        Ok(Self {
            immutable: Arc::new(immutable),
            hit_window,
            timestamp_converter,
            lane_states,
            last_hits: CircularQueue::with_capacity(32),
        })
    }

    /// Returns the map position at the given map timestamp.
    ///
    /// The position takes scroll speed changes into account.
    #[inline]
    pub fn position_at_time(&self, timestamp: MapTimestamp) -> Position {
        self.immutable.position_at_time(timestamp)
    }

    /// Returns the start timestamp of the first object.
    #[inline]
    pub fn first_timestamp(&self) -> Option<MapTimestamp> {
        self.immutable.first_timestamp()
    }

    /// Returns the end timestamp of the last object.
    #[inline]
    pub fn last_timestamp(&self) -> Option<MapTimestamp> {
        self.immutable.last_timestamp()
    }

    /// Returns the regular object which has the minimum position.
    #[inline]
    pub fn min_regular(&self) -> Option<RegularObjectCache> {
        self.immutable.min_regular
    }

    /// Returns the long note, one of the ends of which has the minimum position.
    #[inline]
    pub fn min_long_note(&self) -> Option<LongNoteCache> {
        self.immutable.min_long_note
    }

    /// Returns the regular object which has the maximum position.
    #[inline]
    pub fn max_regular(&self) -> Option<RegularObjectCache> {
        self.immutable.max_regular
    }

    /// Returns the long note, one of the ends of which has the maximum position.
    #[inline]
    pub fn max_long_note(&self) -> Option<LongNoteCache> {
        self.immutable.max_long_note
    }

    /// Returns the timing line with the maximum position.
    #[inline]
    pub fn max_timing_line(&self) -> Option<TimingLine> {
        self.immutable.max_timing_line
    }

    /// Returns the minimum position across all objects.
    #[inline]
    pub fn min_position(&self) -> Option<Position> {
        self.immutable.min_position
    }

    /// Returns the maximum position across all objects.
    #[inline]
    pub fn max_position(&self) -> Option<Position> {
        self.immutable.max_position
    }

    /// Returns the number of lanes in the map.
    #[inline]
    pub fn lane_count(&self) -> usize {
        self.immutable.lane_count()
    }

    /// Updates the state to match the `latest` state.
    ///
    /// # Panics
    ///
    /// Panics if the `latest` state is older than `self` (as indicated by `first_active_object` in
    /// one of the lane states being bigger than the one in the `latest` state).
    pub fn update_to_latest(&mut self, latest: &GameState) {
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
    #[inline]
    pub fn update(&mut self, timestamp: GameTimestamp) {
        for lane in 0..self.lane_count() {
            self.update_lane(lane, timestamp);
        }
    }

    /// Updates the state for `lane`.
    fn update_lane(&mut self, lane: usize, timestamp: GameTimestamp) {
        if !self.has_active_objects(lane) {
            return;
        }

        let map_timestamp = timestamp.to_map(&self.timestamp_converter);
        let map_hit_window = self.hit_window.to_map(&self.timestamp_converter);

        let lane_state = &mut self.lane_states[lane];
        let objects = &self.immutable.map.lanes[lane].objects[lane_state.first_active_object..];
        let object_states = &mut lane_state.object_states[lane_state.first_active_object..];

        for (object, state) in objects.iter().zip(object_states.iter_mut()) {
            // We want to increase first_active_object on every continue.
            lane_state.first_active_object += 1;

            if object.end_timestamp().saturating_add(map_hit_window) < map_timestamp {
                // The object can no longer be hit.
                match state {
                    ObjectState::Regular(state) => {
                        if let RegularObjectState::NotHit = state {
                            *state = RegularObjectState::Missed;
                        } else {
                            unreachable!()
                        }
                    }
                    ObjectState::LongNote(state) => {
                        if let LongNoteState::Held { press_difference } = *state {
                            *state = LongNoteState::Hit {
                                press_difference,
                                release_difference: self.hit_window,
                            };

                            self.last_hits.push(Hit {
                                timestamp: (object.end_timestamp() + map_hit_window)
                                    .to_game(&self.timestamp_converter),
                                difference: self.hit_window,
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
                }

                continue;
            }

            if object.start_timestamp().saturating_add(map_hit_window) < map_timestamp {
                // The object can no longer be hit.
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
        self.update_lane(lane, timestamp);
        if !self.has_active_objects(lane) {
            return;
        }

        let map_timestamp = timestamp.to_map(&self.timestamp_converter);
        let map_hit_window = self.hit_window.to_map(&self.timestamp_converter);

        let lane_state = &mut self.lane_states[lane];
        let object = &self.immutable.map.lanes[lane].objects[lane_state.first_active_object];
        let state = &mut lane_state.object_states[lane_state.first_active_object];

        if map_timestamp >= object.start_timestamp().saturating_sub(map_hit_window) {
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
        self.update_lane(lane, timestamp);
        if !self.has_active_objects(lane) {
            return;
        }

        let map_timestamp = timestamp.to_map(&self.timestamp_converter);
        let map_hit_window = self.hit_window.to_map(&self.timestamp_converter);

        let lane_state = &mut self.lane_states[lane];
        let object = &self.immutable.map.lanes[lane].objects[lane_state.first_active_object];
        let state = &mut lane_state.object_states[lane_state.first_active_object];

        if let ObjectState::LongNote(state) = state {
            if let LongNoteState::Held { press_difference } = *state {
                if map_timestamp >= object.end_timestamp().saturating_sub(map_hit_window) {
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

impl ImmutableGameState {
    /// Returns the map position at the given map timestamp.
    ///
    /// The position takes scroll speed changes into account.
    fn position_at_time(&self, timestamp: MapTimestamp) -> Position {
        if self.position_cache.is_empty() {
            return Position::zero()
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

    /// Returns the start timestamp of the first object.
    #[inline]
    fn first_timestamp(&self) -> Option<MapTimestamp> {
        self.map
            .lanes
            .iter()
            .filter_map(|lane| lane.objects.first())
            .map(Object::start_timestamp)
            .min()
    }

    /// Returns the end timestamp of the last object.
    #[inline]
    fn last_timestamp(&self) -> Option<MapTimestamp> {
        self.map
            .lanes
            .iter()
            .filter_map(|lane| lane.objects.last())
            .map(Object::end_timestamp)
            .max()
    }

    /// Returns the number of lanes in the map.
    #[inline]
    pub fn lane_count(&self) -> usize {
        self.map.lane_count()
    }
}

impl ObjectState {
    /// Returns `true` if the object should be hidden (i.e. it's been hit or missed).
    pub fn is_hidden(&self) -> bool {
        matches!(
            self,
            Self::Regular(RegularObjectState::Hit { .. } | RegularObjectState::Missed)
                | Self::LongNote(LongNoteState::Hit { .. })
        )
    }
}

impl ObjectCache {
    /// Returns the cached start position of the object.
    ///
    /// This is the first position at which this object is visible.
    #[inline]
    pub fn start_position(&self) -> Position {
        match *self {
            ObjectCache::Regular(RegularObjectCache { position }) => position,
            ObjectCache::LongNote(LongNoteCache { start_position, .. }) => start_position,
        }
    }

    /// Returns the cached end position of the object.
    ///
    /// This is the last position at which this object is visible.
    #[inline]
    pub fn end_position(&self) -> Position {
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
        map::{ArbitraryMapType, Lane, ScrollSpeedChange, TimeSignature, TimingPoint},
        scroll::ScrollSpeedMultiplier,
    };
    use alloc::vec;
    use proptest::{collection::size_range, prelude::*};

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

        let state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();

        for lane in &state.immutable.map.lanes {
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

        let mut state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();
        state.key_press(0, GameTimestamp::from_millis(10_000));

        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[
                ObjectState::Regular(RegularObjectState::Missed),
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

        let mut state = GameState::new(map, GameTimestampDifference::from_millis(20)).unwrap();
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

        let mut state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();
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

        let mut state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();
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

        let mut state = GameState::new(map, GameTimestampDifference::from_millis(123)).unwrap();
        state.key_press(0, GameTimestamp::from_millis(5_000));
        state.key_release(0, GameTimestamp::from_millis(15_000));

        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[ObjectState::LongNote(LongNoteState::Hit {
                press_difference: GameTimestampDifference::from_millis(0),
                release_difference: state.hit_window,
            })][..]
        );

        let mut hits = CircularQueue::with_capacity(2);
        hits.push(Hit {
            timestamp: GameTimestamp::from_millis(5_000),
            difference: GameTimestampDifference::from_millis(0),
        });
        hits.push(Hit {
            timestamp: GameTimestamp::from_millis(10_000) + state.hit_window,
            difference: state.hit_window,
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

        let mut state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();
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

        let mut state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();
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

        let mut state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();
        state.update(GameTimestamp::from_millis(15_000));

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

        let mut state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();
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

        let mut state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();
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

        let mut state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();

        let mut state2 = state.clone();
        state2.timestamp_converter.global_offset = GameTimestampDifference::from_millis(10_000);
        state2.timestamp_converter.local_offset = MapTimestampDifference::from_millis(20_000);
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

        let mut state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();

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

        let mut state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();
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

        let state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();

        assert_eq!(
            state.immutable.position_cache.len(),
            state.immutable.map.scroll_speed_changes.len()
        );
        assert_eq!(
            &state.immutable.position_cache[..],
            &[
                CachedPosition {
                    timestamp: MapTimestamp::from_millis(-2),
                    position: Position::new(-900)
                },
                CachedPosition {
                    timestamp: MapTimestamp::from_millis(-1),
                    position: Position::new(-500)
                },
                CachedPosition {
                    timestamp: MapTimestamp::from_millis(0),
                    position: Position::new(0)
                },
                CachedPosition {
                    timestamp: MapTimestamp::from_millis(1),
                    position: Position::new(100)
                },
                CachedPosition {
                    timestamp: MapTimestamp::from_millis(3),
                    position: Position::new(700)
                },
                CachedPosition {
                    timestamp: MapTimestamp::from_millis(4),
                    position: Position::new(1200)
                },
                CachedPosition {
                    timestamp: MapTimestamp::from_millis(5),
                    position: Position::new(1900)
                },
            ][..]
        );
    }

    #[test]
    fn game_state_position_cache_no_zero_scroll_speed_change() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: vec![
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(-1),
                    multiplier: ScrollSpeedMultiplier::new(5),
                },
                ScrollSpeedChange {
                    timestamp: MapTimestamp::from_millis(1),
                    multiplier: ScrollSpeedMultiplier::new(3),
                },
            ],
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::default(),
            lanes: vec![Lane { objects: vec![] }],
        };

        let state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();

        assert_eq!(
            &state.immutable.position_cache[..],
            &[
                CachedPosition {
                    timestamp: MapTimestamp::from_millis(-1),
                    position: Position::new(-500)
                },
                CachedPosition {
                    timestamp: MapTimestamp::from_millis(1),
                    position: Position::new(500)
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

        let state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();

        assert_eq!(
            state.position_at_time(MapTimestamp::from_milli_hundredths(-250)),
            Position::new(-950)
        );
        assert_eq!(
            state.position_at_time(MapTimestamp::from_milli_hundredths(-50)),
            Position::new(-250)
        );
        assert_eq!(
            state.position_at_time(MapTimestamp::from_milli_hundredths(300)),
            Position::new(700)
        );
        assert_eq!(
            state.position_at_time(MapTimestamp::from_milli_hundredths(350)),
            Position::new(950)
        );
        assert_eq!(
            state.position_at_time(MapTimestamp::from_milli_hundredths(600)),
            Position::new(2700)
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

        let state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();

        assert_eq!(
            state.immutable.lane_caches[0].object_caches[0],
            ObjectCache::Regular(RegularObjectCache {
                position: Position::new(-950)
            })
        );
        assert_eq!(
            state.immutable.lane_caches[0].object_caches[1],
            ObjectCache::LongNote(LongNoteCache {
                start_position: Position::new(700),
                end_position: Position::new(2700)
            })
        );
        assert_eq!(
            state.immutable.lane_caches[1].object_caches[0],
            ObjectCache::LongNote(LongNoteCache {
                start_position: Position::new(-250),
                end_position: Position::new(950)
            })
        );
    }

    #[allow(clippy::inconsistent_digit_grouping)]
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

        let state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();

        assert_eq!(
            &state.immutable.timing_lines[..],
            &[
                TimingLine {
                    timestamp: MapTimestamp::from_millis(0),
                    position: Position::new(0),
                },
                TimingLine {
                    timestamp: MapTimestamp::from_millis(40),
                    position: Position::new(40_00),
                },
                TimingLine {
                    timestamp: MapTimestamp::from_millis(80),
                    position: Position::new(80_00),
                },
                TimingLine {
                    timestamp: MapTimestamp::from_millis(120),
                    position: Position::new(120_00),
                },
                TimingLine {
                    timestamp: MapTimestamp::from_millis(160),
                    position: Position::new(160_00),
                },
                TimingLine {
                    timestamp: MapTimestamp::from_millis(200),
                    position: Position::new(200_00),
                },
                TimingLine {
                    timestamp: MapTimestamp::from_millis(215),
                    position: Position::new(215_00),
                },
                TimingLine {
                    timestamp: MapTimestamp::from_millis(220),
                    position: Position::new(220_00),
                },
                TimingLine {
                    timestamp: MapTimestamp::from_millis(240),
                    position: Position::new(240_00),
                },
                TimingLine {
                    timestamp: MapTimestamp::from_millis(260),
                    position: Position::new(260_00),
                },
            ][..],
        );
    }

    #[test]
    fn game_state_first_last_timestamp() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: vec![],
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::default(),
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
                        start: MapTimestamp::from_milli_hundredths(-300),
                        end: MapTimestamp::from_milli_hundredths(350),
                    }],
                },
            ],
        };

        let state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();

        assert_eq!(
            state.first_timestamp(),
            Some(MapTimestamp::from_milli_hundredths(-300))
        );
        assert_eq!(
            state.last_timestamp(),
            Some(MapTimestamp::from_milli_hundredths(600))
        );
    }

    #[test]
    fn game_state_first_last_timestamp_empty() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: Vec::new(),
            scroll_speed_changes: vec![],
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::default(),
            lanes: vec![],
        };

        let state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();

        assert_eq!(state.first_timestamp(), None);
        assert_eq!(state.last_timestamp(), None);
    }

    #[test]
    fn object_cache_methods() {
        let regular = ObjectCache::Regular(RegularObjectCache {
            position: Position::new(10),
        });
        assert_eq!(regular.start_position(), Position::new(10));
        assert_eq!(regular.end_position(), Position::new(10));

        let ln = ObjectCache::LongNote(LongNoteCache {
            start_position: Position::new(20),
            end_position: Position::new(30),
        });
        assert_eq!(ln.start_position(), Position::new(20));
        assert_eq!(ln.end_position(), Position::new(30));
    }

    #[test]
    fn game_state_new_with_overlapping_objects() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: vec![],
            scroll_speed_changes: vec![],
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::default(),
            lanes: vec![Lane {
                objects: vec![
                    Object::LongNote {
                        start: MapTimestamp::zero(),
                        end: MapTimestamp::from_milli_hundredths(100),
                    },
                    Object::Regular {
                        timestamp: MapTimestamp::zero(),
                    },
                ],
            }],
        };

        assert!(matches!(
            GameState::new(map, GameTimestampDifference::from_millis(0)),
            Err(GameStateCreationError::MapHasOverlappingObjects(_, _, _))
        ));
    }

    #[test]
    fn min_regular_proptest_regression_1() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: vec![],
            scroll_speed_changes: vec![],
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::new(-1),
            lanes: vec![Lane {
                objects: vec![
                    Object::Regular {
                        timestamp: MapTimestamp::from_milli_hundredths(-1),
                    },
                    Object::Regular {
                        timestamp: MapTimestamp::from_milli_hundredths(0),
                    },
                ],
            }],
        };

        let state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();
        assert_eq!(
            state.min_regular(),
            Some(RegularObjectCache {
                position: Position::new(0)
            })
        );
    }

    #[test]
    fn game_state_new_doesnt_panic_proptest_regression_1() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            timing_points: vec![
                TimingPoint {
                    timestamp: MapTimestamp::from_millis(0),
                    beat_duration: MapTimestampDifference::from_millis(0),
                    signature: TimeSignature {
                        beat_count: 0,
                        beat_unit: 0,
                    },
                },
                TimingPoint {
                    timestamp: MapTimestamp::from_milli_hundredths(-1073741725),
                    beat_duration: MapTimestampDifference::from_millis(0),
                    signature: TimeSignature {
                        beat_count: 0,
                        beat_unit: 0,
                    },
                },
            ],
            scroll_speed_changes: vec![],
            initial_scroll_speed_multiplier: ScrollSpeedMultiplier::new(0),
            lanes: vec![],
        };
        let _ = GameState::new(map, GameTimestampDifference::from_millis(0));
    }

    proptest! {
        #[test]
        fn game_state_new_doesnt_panic(map: Map) {
            let _ = GameState::new(map, GameTimestampDifference::from_millis(0));
        }

        #[test]
        fn game_state_new_with_valid_map_succeeds(map in any_with::<Map>(ArbitraryMapType::Valid)) {
            let _ = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();
        }

        #[test]
        fn min_regular(map in any_with::<Map>(ArbitraryMapType::Valid)) {
            let state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();

            let result = state.min_regular();
            let correct = state
                .immutable
                .lane_caches
                .iter()
                .flat_map(|lane| lane.object_caches.iter())
                .filter(|object| matches!(object, ObjectCache::Regular(_)))
                .min_by_key(|object| object.start_position().min(object.end_position()))
                .map(|object| if let ObjectCache::Regular(cache) = object {
                    *cache
                } else {
                    unreachable!()
                });
            prop_assert_eq!(result, correct);
        }

        #[test]
        fn max_regular(map in any_with::<Map>(ArbitraryMapType::Valid)) {
            let state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();

            let result = state.max_regular();
            let correct = state
                .immutable
                .lane_caches
                .iter()
                .flat_map(|lane| lane.object_caches.iter())
                .filter(|object| matches!(object, ObjectCache::Regular(_)))
                .max_by_key(|object| object.start_position().max(object.end_position()))
                .map(|object| if let ObjectCache::Regular(cache) = object {
                    *cache
                } else {
                    unreachable!()
                });
            prop_assert_eq!(result, correct);
        }

        #[test]
        fn min_long_note(map in any_with::<Map>(ArbitraryMapType::Valid)) {
            let state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();

            let result = state.min_long_note();
            let correct = state
                .immutable
                .lane_caches
                .iter()
                .flat_map(|lane| lane.object_caches.iter())
                .filter(|object| matches!(object, ObjectCache::LongNote(_)))
                .min_by_key(|object| object.start_position().min(object.end_position()))
                .map(|object| if let ObjectCache::LongNote(cache) = object {
                    *cache
                } else {
                    unreachable!()
                });
            prop_assert_eq!(result, correct);
        }

        #[test]
        fn max_long_note(map in any_with::<Map>(ArbitraryMapType::Valid)) {
            let state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();

            let result = state.max_long_note();
            let correct = state
                .immutable
                .lane_caches
                .iter()
                .flat_map(|lane| lane.object_caches.iter())
                .filter(|object| matches!(object, ObjectCache::LongNote(_)))
                .max_by_key(|object| object.start_position().max(object.end_position()))
                .map(|object| if let ObjectCache::LongNote(cache) = object {
                    *cache
                } else {
                    unreachable!()
                });
            prop_assert_eq!(result, correct);
        }

        #[test]
        fn min_position(map in any_with::<Map>(ArbitraryMapType::Valid)) {
            let state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();

            let result = state.min_position();
            let correct = state
                .immutable
                .lane_caches
                .iter()
                .flat_map(|lane| lane.object_caches.iter())
                .map(|object| object.start_position().min(object.end_position()))
                .min();
            prop_assert_eq!(result, correct);
        }

        #[test]
        fn max_position(map in any_with::<Map>(ArbitraryMapType::Valid)) {
            let state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();

            let result = state.max_position();
            let correct = state
                .immutable
                .lane_caches
                .iter()
                .flat_map(|lane| lane.object_caches.iter())
                .map(|object| object.start_position().max(object.end_position()))
                .max();
            prop_assert_eq!(result, correct);
        }

        #[test]
        fn max_timing_line(map in any_with::<Map>(ArbitraryMapType::Valid)) {
            let state = GameState::new(map, GameTimestampDifference::from_millis(0)).unwrap();

            let result = state.max_timing_line();
            let correct = state
                .immutable
                .timing_lines
                .iter()
                .max_by_key(|line| line.position)
                .copied();
            prop_assert_eq!(result, correct);
        }

        #[test]
        fn update_doesnt_panic(
            map in any_with::<Map>(ArbitraryMapType::Valid),
            hit_window: GameTimestampDifference,
            timestamps: Vec<GameTimestamp>,
        ) {
            let mut state = GameState::new(map, hit_window).unwrap();
            for timestamp in timestamps {
                state.update(timestamp);
            }
        }

        #[test]
        fn intermediate_updates_are_unnecessary(
            map in any_with::<Map>(ArbitraryMapType::Valid),
            hit_window: GameTimestampDifference,
            timestamps in any_with::<Vec<GameTimestamp>>(size_range(1..100).lift()),
        ) {
            let mut state = GameState::new(map, hit_window).unwrap();
            let mut state2 = state.clone();

            for &timestamp in &timestamps {
                state.update(timestamp);
            }

            state2.update(*timestamps.iter().max().unwrap());

            prop_assert_eq!(state, state2);
        }

        #[test]
        fn gameplay_doesnt_panic(
            (map, events) in valid_map_with_events(),
            hit_window: GameTimestampDifference,
        ) {
            let mut state = GameState::new(map, hit_window).unwrap();

            // If additional validation is added to key_press and key_release, this might need
            // further filtering to e.g. exclude pressing keys that are already pressed or releasing
            // keys that were not pressed.
            for (press, lane, timestamp) in events {
                if press {
                    state.key_press(lane, timestamp);
                } else {
                    state.key_release(lane, timestamp);
                }
            }
        }

        #[test]
        fn all_objects_are_eventually_missed(map in any_with::<Map>(ArbitraryMapType::ValidWithObjects)) {
            let hit_window = GameTimestampDifference::from_millis(123);
            let mut state = GameState::new(map, hit_window).unwrap();

            let last_timestamp = state.last_timestamp().unwrap();
            prop_assume!(
                last_timestamp.into_milli_hundredths() < 2i32.pow(30) - 12301,
                "last object too close to timestamp range boundary",
            );

            let one_past_hit_window_end = (last_timestamp
                + hit_window.to_map(&state.timestamp_converter)
                + MapTimestampDifference::from_milli_hundredths(1))
            .to_game(&state.timestamp_converter);
            state.update(one_past_hit_window_end);

            for state in state.lane_states.into_iter().flat_map(|x| x.object_states) {
                match state {
                    ObjectState::Regular(state) => {
                        prop_assert_eq!(state, RegularObjectState::Missed);
                    }
                    ObjectState::LongNote(state) => {
                        let is_missed = matches!(state, LongNoteState::Missed { .. });
                        prop_assert!(is_missed);
                    }
                }
            }
        }
    }

    fn valid_map_with_events(
    ) -> impl proptest::strategy::Strategy<Value = (Map, Vec<(bool, usize, GameTimestamp)>)> {
        any_with::<Map>(ArbitraryMapType::ValidWithLanes).prop_flat_map(|map| {
            let events = prop::collection::vec(
                (any::<bool>(), 0..map.lane_count(), any::<GameTimestamp>()),
                0..100,
            );
            (Just(map), events)
        })
    }
}
