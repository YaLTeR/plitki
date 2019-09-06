//! Functionality related to managing the game state.
use alloc::{sync::Arc, vec::Vec};

use circular_queue::CircularQueue;

use crate::{
    map::Map,
    object::Object,
    scroll::ScrollSpeed,
    timing::{GameTimestamp, GameTimestampDifference, MapTimestamp, TimestampConverter},
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
        };

        Self {
            map: Arc::new(map),
            cap_fps: false,
            scroll_speed: ScrollSpeed(32),
            timestamp_converter,
            lane_states,
            last_hits: CircularQueue::with_capacity(32),
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
        state2.timestamp_converter.global_offset = GameTimestampDifference::from_millis(10_000);
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
