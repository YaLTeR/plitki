//! Functionality related to managing the game state.
use alloc::{sync::Arc, vec::Vec};
use core::ops::{Div, Mul};

use crate::{
    map::Map,
    object::Object,
    timing::{
        GameTimestamp, GameTimestampDifference, MapTimestamp, MapTimestampDifference, Timestamp,
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
    /// Global offset.
    pub offset: GameTimestampDifference,
    /// Contains states of the objects in lanes.
    pub lane_states: Vec<LaneState>,
}

/// Scrolling speed.
///
/// Measured in <sup>1</sup>⁄<sub>10</sub>ths of vertical square screens per second. That is, on a
/// square 1:1 screen, 10 means a note travels from the very top to the very bottom of the screen
/// in one second; 5 means in two seconds and 20 means in half a second.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ScrollSpeed(pub u8);

/// States of a regular object.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum RegularObjectState {
    /// The object has not been hit.
    NotHit,
    /// The object has been hit.
    Hit,
}

/// States of a long note object.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum LongNoteState {
    /// The long note has not been hit.
    NotHit,
    /// The long note is currently held.
    Held,
    /// The long note has been hit, that is, held and released on time or later.
    Hit,
    /// The long note has not been hit in time, or has been released early.
    Missed {
        /// The timestamp when the LN was released.
        ///
        /// This may be before the start timestamp if the player presses and releases the LN
        /// quickly during the hit window before the LN start.
        held_until: Option<MapTimestamp>,
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

impl GameState {
    /// Creates a new `GameState` given a map.
    pub fn new(mut map: Map) -> Self {
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

        Self {
            map: Arc::new(map),
            cap_fps: false,
            scroll_speed: ScrollSpeed(16),
            offset: GameTimestampDifference::from_millis(0),
            lane_states,
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
        self.offset = latest.offset;

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

    /// Converts a game timestamp into a map timestamp.
    ///
    /// Takes global offset into account. For differences (which do _not_ need to consider global
    /// offset) use [`game_to_map_difference`].
    ///
    /// [`game_to_map_difference`]: #method.game_to_map_difference
    #[inline]
    pub fn game_to_map(&self, timestamp: GameTimestamp) -> MapTimestamp {
        MapTimestamp((timestamp + self.offset).0)
    }

    /// Converts a map timestamp into a game timestamp.
    ///
    /// Takes global offset into account. For differences (which do _not_ need to consider global
    /// offset) use [`map_to_game_difference`].
    ///
    /// [`map_to_game_difference`]: #method.map_to_game_difference
    #[inline]
    pub fn map_to_game(&self, timestamp: MapTimestamp) -> GameTimestamp {
        GameTimestamp(timestamp.0) - self.offset
    }

    /// Converts a game difference into a map difference.
    ///
    /// Difference conversion does _not_ consider global offset. For timestamps (which need to
    /// consider global offset) use [`game_to_map`].
    ///
    /// [`game_to_map`]: #method.game_to_map
    #[inline]
    pub fn game_to_map_difference(
        &self,
        difference: GameTimestampDifference,
    ) -> MapTimestampDifference {
        MapTimestampDifference(difference.0)
    }

    /// Converts a map difference into a game difference.
    ///
    /// Difference conversion does _not_ consider global offset. For timestamps (which need to
    /// consider global offset) use [`map_to_game`].
    ///
    /// [`map_to_game`]: #method.map_to_game
    #[inline]
    pub fn map_to_game_difference(
        &self,
        difference: MapTimestampDifference,
    ) -> GameTimestampDifference {
        GameTimestampDifference(difference.0)
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

        let map_timestamp = self.game_to_map(timestamp);
        let map_hit_window = self.game_to_map_difference(hit_window);

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
                    if *state == LongNoteState::Held {
                        *state = LongNoteState::Hit;
                    } else if *state == LongNoteState::NotHit {
                        *state = LongNoteState::Missed { held_until: None };
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
                        *state = LongNoteState::Missed { held_until: None };
                        continue;
                    }

                    assert!(*state == LongNoteState::Held);
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

        let map_timestamp = self.game_to_map(timestamp);
        let map_hit_window = self.game_to_map_difference(hit_window);

        let lane_state = &mut self.lane_states[lane];
        let object = &self.map.lanes[lane].objects[lane_state.first_active_object];
        let state = &mut lane_state.object_states[lane_state.first_active_object];

        if map_timestamp >= object.start_timestamp() - map_hit_window {
            // The object can be hit.
            match state {
                ObjectState::Regular(ref mut state) => {
                    *state = RegularObjectState::Hit;

                    // This object is no longer active.
                    lane_state.first_active_object += 1;
                }
                ObjectState::LongNote(ref mut state) => *state = LongNoteState::Held,
            }
        }
    }

    /// Handles a key release.
    pub fn key_release(&mut self, lane: usize, timestamp: GameTimestamp) {
        self.update(lane, timestamp);
        if !self.has_active_objects(lane) {
            return;
        }

        let hit_window = GameTimestampDifference::from_millis(76);

        let map_timestamp = self.game_to_map(timestamp);
        let map_hit_window = self.game_to_map_difference(hit_window);

        let lane_state = &mut self.lane_states[lane];
        let object = &self.map.lanes[lane].objects[lane_state.first_active_object];
        let state = &mut lane_state.object_states[lane_state.first_active_object];

        if let ObjectState::LongNote(state) = state {
            if *state == LongNoteState::Held {
                if map_timestamp >= object.end_timestamp() - map_hit_window {
                    *state = LongNoteState::Hit;
                } else {
                    // Released too early.
                    *state = LongNoteState::Missed {
                        held_until: Some(map_timestamp),
                    };
                }

                // This object is no longer active.
                lane_state.first_active_object += 1;
            }
        }
    }
}

impl Mul<GameTimestampDifference> for ScrollSpeed {
    type Output = f32;

    #[inline]
    fn mul(self, rhs: GameTimestampDifference) -> Self::Output {
        f32::from(self.0) * (GameTimestamp::from_millis(0) + rhs).0.as_secs_f32()
    }
}

impl Mul<ScrollSpeed> for GameTimestampDifference {
    type Output = f32;

    #[inline]
    fn mul(self, rhs: ScrollSpeed) -> Self::Output {
        rhs * self
    }
}

impl Div<ScrollSpeed> for f32 {
    type Output = GameTimestampDifference;

    #[inline]
    fn div(self, rhs: ScrollSpeed) -> Self::Output {
        GameTimestamp(Timestamp::from_secs_f32(self / f32::from(rhs.0)))
            - GameTimestamp::from_millis(0)
    }
}

impl ObjectState {
    /// Returns `true` if the object has been hit.
    pub fn is_hit(&self) -> bool {
        match self {
            Self::Regular(state) => *state == RegularObjectState::Hit,
            Self::LongNote(state) => *state == LongNoteState::Hit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Lane;
    use alloc::vec;

    #[test]
    fn game_state_objects_sorted() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
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
                ObjectState::Regular(RegularObjectState::Hit),
            ][..]
        );
    }

    #[test]
    fn game_state_regular_hit_note_lock() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
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
                ObjectState::Regular(RegularObjectState::Hit),
                ObjectState::Regular(RegularObjectState::NotHit),
            ][..]
        );
    }

    #[test]
    fn game_state_long_note_hit() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
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
            &[ObjectState::LongNote(LongNoteState::Hit)][..]
        );
    }

    #[test]
    fn game_state_long_note_released_early() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
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
                held_until: Some(state.game_to_map(GameTimestamp::from_millis(7_000)))
            })][..]
        );
    }

    #[test]
    fn game_state_long_note_pressed_late() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
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
                held_until: None
            })][..]
        );
    }

    #[test]
    fn game_state_key_press_long_note_held() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
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
            &[ObjectState::LongNote(LongNoteState::Held)][..]
        );
    }

    #[test]
    fn game_state_global_offset() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
            lanes: vec![Lane {
                objects: vec![Object::Regular {
                    timestamp: MapTimestamp::from_millis(20_000),
                }],
            }],
        };

        let mut state = GameState::new(map);
        state.offset = GameTimestampDifference::from_millis(10_000);

        state.key_press(0, GameTimestamp::from_millis(0));
        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[ObjectState::Regular(RegularObjectState::NotHit)][..]
        );

        state.key_press(0, GameTimestamp::from_millis(10_000));
        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[ObjectState::Regular(RegularObjectState::Hit)][..]
        );
    }

    #[test]
    fn game_state_update_to_latest() {
        let map = Map {
            song_artist: None,
            song_title: None,
            difficulty_name: None,
            mapper: None,
            audio_file: None,
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
        state2.offset = GameTimestampDifference::from_millis(10_000);
        state2.scroll_speed = ScrollSpeed(5);
        state2.lane_states[0].first_active_object = 1;
        state2.lane_states[0].object_states[0] = ObjectState::Regular(RegularObjectState::Hit);
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
            lanes: vec![Lane { objects: vec![] }],
        };

        let mut state = GameState::new(map);
        let state2 = state.clone();

        state.update_to_latest(&state2);
        assert_eq!(state, state2);
    }
}
