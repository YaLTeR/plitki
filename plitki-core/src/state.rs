//! Functionality related to managing the game state.
use alloc::{sync::Arc, vec::Vec};
use core::{
    convert::TryInto,
    ops::{Div, Mul},
    time::Duration,
};

use crate::{
    map::Map,
    object::Object,
    timing::{GameTimestamp, MapTimestamp, Timestamp},
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
    pub offset: GameTimestamp,
    /// Contains states of the objects in lanes.
    pub lane_states: Vec<LaneState>,
}

/// Scrolling speed, measured in <sup>1</sup>⁄<sub>10</sub>ths of vertical square screens per
/// second. That is, on a square 1:1 screen, 10 means a note travels from the very top to the very
/// bottom of the screen in one second; 5 means in two seconds and 20 means in half a second.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ScrollSpeed(pub u8);

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
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ObjectState {
    /// State of a regular object.
    Regular {
        /// If `true`, this object has been hit.
        hit: bool,
    },
    /// State of a long note object.
    LongNote {
        /// The state.
        state: LongNoteState,
    },
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
                    Object::Regular { .. } => ObjectState::Regular { hit: false },
                    Object::LongNote { .. } => ObjectState::LongNote {
                        state: LongNoteState::NotHit,
                    },
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
            offset: GameTimestamp(Timestamp::from_secs_f32(0.)),
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
    /// Takes global offset into account. For durations (which do _not_ need to consider global
    /// offset) use [`game_to_map_duration`].
    ///
    /// [`game_to_map_duration`]: #method.game_to_map_duration
    #[inline]
    pub fn game_to_map(&self, timestamp: GameTimestamp) -> MapTimestamp {
        MapTimestamp((timestamp + self.offset).0)
    }

    /// Converts a map timestamp into a game timestamp.
    ///
    /// Takes global offset into account. For durations (which do _not_ need to consider global
    /// offset) use [`map_to_game_duration`].
    ///
    /// [`map_to_game_duration`]: #method.map_to_game_duration
    #[inline]
    pub fn map_to_game(&self, timestamp: MapTimestamp) -> GameTimestamp {
        GameTimestamp(timestamp.0) - self.offset
    }

    /// Converts a game duration into a map duration.
    ///
    /// Duration conversion does _not_ consider global offset. For timestamps (which need to
    /// consider global offset) use [`game_to_map`].
    ///
    /// [`game_to_map`]: #method.game_to_map
    #[inline]
    pub fn game_to_map_duration(&self, duration: GameTimestamp) -> MapTimestamp {
        MapTimestamp(duration.0)
    }

    /// Converts a map duration into a game duration.
    ///
    /// Duration conversion does _not_ consider global offset. For timestamps (which need to
    /// consider global offset) use [`map_to_game`].
    ///
    /// [`map_to_game`]: #method.map_to_game
    #[inline]
    pub fn map_to_game_duration(&self, duration: MapTimestamp) -> GameTimestamp {
        GameTimestamp(duration.0)
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

        let hit_window = GameTimestamp(Duration::from_millis(76).try_into().unwrap());

        let map_timestamp = self.game_to_map(timestamp);
        let map_hit_window = self.game_to_map_duration(hit_window);

        let lane_state = &mut self.lane_states[lane];
        let objects = &self.map.lanes[lane].objects[lane_state.first_active_object..];
        let object_states = &mut lane_state.object_states[lane_state.first_active_object..];

        for (object, state) in objects.iter().zip(object_states.iter_mut()) {
            // We want to increase first_active_object on every continue.
            lane_state.first_active_object += 1;

            if object.end_timestamp() + map_hit_window < map_timestamp {
                // The object can no longer be hit.
                // TODO: mark the object as missed

                if let ObjectState::LongNote { state } = state {
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

                if let ObjectState::LongNote { state } = state {
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

        let hit_window = GameTimestamp(Duration::from_millis(76).try_into().unwrap());

        let map_timestamp = self.game_to_map(timestamp);
        let map_hit_window = self.game_to_map_duration(hit_window);

        let lane_state = &mut self.lane_states[lane];
        let object = &self.map.lanes[lane].objects[lane_state.first_active_object];
        let state = &mut lane_state.object_states[lane_state.first_active_object];

        if map_timestamp >= object.start_timestamp() - map_hit_window {
            // The object can be hit.
            match state {
                ObjectState::Regular { ref mut hit } => {
                    *hit = true;

                    // This object is no longer active.
                    lane_state.first_active_object += 1;
                }
                ObjectState::LongNote { ref mut state } => *state = LongNoteState::Held,
            }
        }
    }

    /// Handles a key release.
    pub fn key_release(&mut self, lane: usize, timestamp: GameTimestamp) {
        self.update(lane, timestamp);
        if !self.has_active_objects(lane) {
            return;
        }

        let hit_window = GameTimestamp(Duration::from_millis(76).try_into().unwrap());

        let map_timestamp = self.game_to_map(timestamp);
        let map_hit_window = self.game_to_map_duration(hit_window);

        let lane_state = &mut self.lane_states[lane];
        let object = &self.map.lanes[lane].objects[lane_state.first_active_object];
        let state = &mut lane_state.object_states[lane_state.first_active_object];

        if let ObjectState::LongNote { state } = state {
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

impl Mul<GameTimestamp> for ScrollSpeed {
    type Output = f32;

    #[inline]
    fn mul(self, rhs: GameTimestamp) -> Self::Output {
        f32::from(self.0) * rhs.0.as_secs_f32()
    }
}

impl Mul<ScrollSpeed> for GameTimestamp {
    type Output = f32;

    #[inline]
    fn mul(self, rhs: ScrollSpeed) -> Self::Output {
        rhs * self
    }
}

impl Div<ScrollSpeed> for f32 {
    type Output = GameTimestamp;

    #[inline]
    fn div(self, rhs: ScrollSpeed) -> Self::Output {
        GameTimestamp(Timestamp::from_secs_f32(self / f32::from(rhs.0)))
    }
}

impl ObjectState {
    /// Returns `true` if the object has been hit.
    pub fn is_hit(&self) -> bool {
        match self {
            Self::Regular { hit } => *hit,
            Self::LongNote { state } => *state == LongNoteState::Hit,
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
                            timestamp: MapTimestamp(Duration::from_millis(10).try_into().unwrap()),
                        },
                        Object::Regular {
                            timestamp: MapTimestamp(Duration::from_millis(0).try_into().unwrap()),
                        },
                    ],
                },
                Lane {
                    objects: vec![
                        Object::Regular {
                            timestamp: MapTimestamp(Duration::from_millis(10).try_into().unwrap()),
                        },
                        Object::LongNote {
                            start: MapTimestamp(Duration::from_millis(0).try_into().unwrap()),
                            end: MapTimestamp(Duration::from_millis(9).try_into().unwrap()),
                        },
                    ],
                },
                Lane {
                    objects: vec![
                        Object::LongNote {
                            start: MapTimestamp(Duration::from_millis(7).try_into().unwrap()),
                            end: MapTimestamp(Duration::from_millis(10).try_into().unwrap()),
                        },
                        Object::Regular {
                            timestamp: MapTimestamp(Duration::from_millis(0).try_into().unwrap()),
                        },
                    ],
                },
                Lane {
                    objects: vec![
                        Object::LongNote {
                            start: MapTimestamp(Duration::from_millis(10).try_into().unwrap()),
                            end: MapTimestamp(Duration::from_millis(7).try_into().unwrap()),
                        },
                        Object::LongNote {
                            start: MapTimestamp(Duration::from_millis(0).try_into().unwrap()),
                            end: MapTimestamp(Duration::from_millis(6).try_into().unwrap()),
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
                        timestamp: MapTimestamp(Duration::from_secs(0).try_into().unwrap()),
                    },
                    Object::Regular {
                        timestamp: MapTimestamp(Duration::from_secs(10).try_into().unwrap()),
                    },
                ],
            }],
        };

        let mut state = GameState::new(map);
        state.key_press(
            0,
            GameTimestamp(Duration::from_secs(10).try_into().unwrap()),
        );

        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[
                ObjectState::Regular { hit: false },
                ObjectState::Regular { hit: true },
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
                        timestamp: MapTimestamp(Duration::from_millis(0).try_into().unwrap()),
                    },
                    Object::Regular {
                        timestamp: MapTimestamp(Duration::from_millis(10).try_into().unwrap()),
                    },
                ],
            }],
        };

        let mut state = GameState::new(map);
        state.key_press(
            0,
            GameTimestamp(Duration::from_millis(10).try_into().unwrap()),
        );

        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[
                ObjectState::Regular { hit: true },
                ObjectState::Regular { hit: false },
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
                    start: MapTimestamp(Duration::from_secs(5).try_into().unwrap()),
                    end: MapTimestamp(Duration::from_secs(10).try_into().unwrap()),
                }],
            }],
        };

        let mut state = GameState::new(map);
        state.key_press(0, GameTimestamp(Duration::from_secs(5).try_into().unwrap()));
        state.key_release(
            0,
            GameTimestamp(Duration::from_secs(10).try_into().unwrap()),
        );

        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[ObjectState::LongNote {
                state: LongNoteState::Hit,
            }][..]
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
                    start: MapTimestamp(Duration::from_secs(5).try_into().unwrap()),
                    end: MapTimestamp(Duration::from_secs(10).try_into().unwrap()),
                }],
            }],
        };

        let mut state = GameState::new(map);
        state.key_press(0, GameTimestamp(Duration::from_secs(5).try_into().unwrap()));
        state.key_release(0, GameTimestamp(Duration::from_secs(7).try_into().unwrap()));

        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[ObjectState::LongNote {
                state: LongNoteState::Missed {
                    held_until: Some(
                        state
                            .game_to_map(GameTimestamp(Duration::from_secs(7).try_into().unwrap()))
                    )
                },
            }][..]
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
                    start: MapTimestamp(Duration::from_secs(5).try_into().unwrap()),
                    end: MapTimestamp(Duration::from_secs(10).try_into().unwrap()),
                }],
            }],
        };

        let mut state = GameState::new(map);
        state.key_press(0, GameTimestamp(Duration::from_secs(7).try_into().unwrap()));
        state.key_release(
            0,
            GameTimestamp(Duration::from_secs(10).try_into().unwrap()),
        );

        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[ObjectState::LongNote {
                state: LongNoteState::Missed { held_until: None },
            }][..]
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
                    start: MapTimestamp(Duration::from_secs(5).try_into().unwrap()),
                    end: MapTimestamp(Duration::from_secs(10).try_into().unwrap()),
                }],
            }],
        };

        let mut state = GameState::new(map);
        state.key_press(0, GameTimestamp(Duration::from_secs(5).try_into().unwrap()));

        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[ObjectState::LongNote {
                state: LongNoteState::Held,
            }][..]
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
                    timestamp: MapTimestamp(Duration::from_secs(20).try_into().unwrap()),
                }],
            }],
        };

        let mut state = GameState::new(map);
        state.offset = GameTimestamp(Duration::from_secs(10).try_into().unwrap());

        state.key_press(0, GameTimestamp(Duration::from_secs(0).try_into().unwrap()));
        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[ObjectState::Regular { hit: false }][..]
        );

        state.key_press(
            0,
            GameTimestamp(Duration::from_secs(10).try_into().unwrap()),
        );
        assert_eq!(
            &state.lane_states[0].object_states[..],
            &[ObjectState::Regular { hit: true }][..]
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
                        timestamp: MapTimestamp(Duration::from_secs(20).try_into().unwrap()),
                    },
                    Object::Regular {
                        timestamp: MapTimestamp(Duration::from_secs(30).try_into().unwrap()),
                    },
                ],
            }],
        };

        let mut state = GameState::new(map);

        let mut state2 = state.clone();
        state2.cap_fps = true;
        state2.offset = GameTimestamp(Duration::from_secs(10).try_into().unwrap());
        state2.scroll_speed = ScrollSpeed(5);
        state2.lane_states[0].first_active_object = 1;
        state2.lane_states[0].object_states[0] = ObjectState::Regular { hit: true };
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
                    timestamp: MapTimestamp(Duration::from_secs(20).try_into().unwrap()),
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
