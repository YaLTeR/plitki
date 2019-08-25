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
#[derive(Clone)]
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
    /// Contains states of the objects in lanes.
    pub lane_states: Vec<LaneState>,
}

/// Scrolling speed, measured in <sup>1</sup>⁄<sub>10</sub>ths of vertical square screens per
/// second. That is, on a square 1:1 screen, 10 means a note travels from the very top to the very
/// bottom of the screen in one second; 5 means in two seconds and 20 means in half a second.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ScrollSpeed(pub u8);

/// States of a long note object.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum LongNoteState {
    /// The long note has not been hit.
    NotHit,
    /// The long note is currently held.
    Held,
    /// The long note has been hit, that is, held and released.
    Hit,
    /// The long note has not been hit in time, or has been released early.
    Missed,
}

/// State of an individual object.
#[derive(Debug, Clone, Copy)]
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
#[derive(Clone)]
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
            scroll_speed: ScrollSpeed(12),
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

        for (lane, latest_lane) in self.lane_states.iter_mut().zip(latest.lane_states.iter()) {
            assert!(lane.first_active_object <= latest_lane.first_active_object);

            // The range is inclusive because `first_active_object` can be an LN that's changing
            // states.
            let update_range = lane.first_active_object..=latest_lane.first_active_object;
            lane.object_states[update_range.clone()]
                .copy_from_slice(&latest_lane.object_states[update_range]);
        }
    }

    /// Converts a game timestamp into a map timestamp.
    #[inline]
    pub fn game_to_map(&self, timestamp: GameTimestamp) -> MapTimestamp {
        // Without rates and offsets they are the same.
        MapTimestamp(timestamp.0)
    }

    /// Converts a map timestamp into a game timestamp.
    #[inline]
    pub fn map_to_game(&self, timestamp: MapTimestamp) -> GameTimestamp {
        // Without rates and offsets they are the same.
        GameTimestamp(timestamp.0)
    }

    /// Updates the state.
    ///
    /// Essentially, this is a way to signal "some time has passed". Stuff like missed objects is
    /// handled here. This should be called every so often for all lanes.
    pub fn update(&mut self, lane: usize, timestamp: GameTimestamp) {
        let hit_window = GameTimestamp(Duration::from_millis(76).try_into().unwrap());

        let map_timestamp = self.game_to_map(timestamp);
        let map_hit_window = self.game_to_map(hit_window);

        let lane_state = &mut self.lane_states[lane];
        let objects = &self.map.lanes[lane].objects[lane_state.first_active_object..];
        let object_states = &mut lane_state.object_states[lane_state.first_active_object..];

        for (i, (object, state)) in objects.iter().zip(object_states.iter_mut()).enumerate() {
            if object.end_timestamp() + map_hit_window < map_timestamp {
                // The object can no longer be hit.
                // TODO: mark the object as missed

                if let ObjectState::LongNote { state } = state {
                    if *state == LongNoteState::Held {
                        *state = LongNoteState::Hit;
                    } else if *state == LongNoteState::NotHit {
                        *state = LongNoteState::Missed;
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
                        *state = LongNoteState::Missed;
                        continue;
                    }

                    assert!(*state == LongNoteState::Held);
                } else {
                    // Regular objects would be skipped over in the previous if.
                    unreachable!()
                }
            }

            // Update `first_active_object`.
            lane_state.first_active_object += i;
            break;
        }
    }

    /// Handles a key press.
    pub fn key_press(&mut self, lane: usize, timestamp: GameTimestamp) {
        self.update(lane, timestamp);

        let hit_window = GameTimestamp(Duration::from_millis(76).try_into().unwrap());

        let map_timestamp = self.game_to_map(timestamp);
        let map_hit_window = self.game_to_map(hit_window);

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

        let hit_window = GameTimestamp(Duration::from_millis(76).try_into().unwrap());

        let map_timestamp = self.game_to_map(timestamp);
        let map_hit_window = self.game_to_map(hit_window);

        let lane_state = &mut self.lane_states[lane];
        let object = &self.map.lanes[lane].objects[lane_state.first_active_object];
        let state = &mut lane_state.object_states[lane_state.first_active_object];

        if let ObjectState::LongNote { state } = state {
            if *state == LongNoteState::Held {
                if map_timestamp >= object.end_timestamp() - map_hit_window {
                    *state = LongNoteState::Hit;
                } else {
                    // Released too early.
                    *state = LongNoteState::Missed;
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
