//! Functionality related to managing the game state.
use alloc::{sync::Arc, vec::Vec};

use crate::{
    map::Map,
    object::Object,
    timing::{GameTimestamp, MapTimestamp, Timestamp},
};

const HIT_WINDOW: GameTimestamp = GameTimestamp(Timestamp(76_00));

/// State of the game.
#[derive(Clone)]
pub struct GameState {
    /// The map.
    ///
    /// Invariant: objects in each lane must be sorted by timestamp.
    pub map: Arc<Map>,
    /// If `true`, heavily limit the FPS for testing.
    pub cap_fps: bool,
    /// The scroll speed, in vertical square screens per second, multiplied by 10. That is, on a
    /// square 1:1 screen, 10 means a note travels from the very top to the very bottom of the
    /// screen in one second; 5 means in two seconds and 20 means in half a second.
    pub scroll_speed: u8,
    /// Contains states of the objects in lanes.
    pub lane_states: Vec<LaneState>,
}

/// States of a long note object.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum LongNoteState {
    /// The long note has not been hit.
    NotHit,
    /// The long note is currently held.
    Held,
    /// The long note has been hit, that is, held and released.
    Hit,
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

impl ObjectState {
    /// Returns `true` if the object has been hit, that is, can no longer be interacted with.
    pub fn is_hit(&self) -> bool {
        match self {
            Self::Regular { hit } => *hit,
            Self::LongNote { state } => *state == LongNoteState::Hit,
        }
    }
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
            // Ensure the objects are sorted by their timestamp (GameState invariant).
            lane.objects.sort_unstable_by_key(Object::timestamp);

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
            scroll_speed: 12,
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

    /// Handles a key press.
    pub fn key_press(&mut self, lane: usize, timestamp: GameTimestamp) {
        let map_timestamp = self.game_to_map(timestamp);
        let map_hit_window = self.game_to_map(HIT_WINDOW);

        let lane_state = &mut self.lane_states[lane];
        let objects = &self.map.lanes[lane].objects[lane_state.first_active_object..];
        let object_states = &mut lane_state.object_states[lane_state.first_active_object..];

        for (i, (object, state)) in objects.iter().zip(object_states.iter_mut()).enumerate() {
            if object.timestamp() + map_hit_window < map_timestamp {
                // The object can no longer be hit.
                // TODO: LNs
                // TODO: mark the object as missed
                continue;
            }

            // Update `first_active_object`.
            lane_state.first_active_object += i;

            if map_timestamp >= object.timestamp() - map_hit_window {
                // The object can be hit.
                match state {
                    ObjectState::Regular { ref mut hit } => *hit = true,
                    ObjectState::LongNote { ref mut state } => *state = LongNoteState::Hit, // TODO
                }

                // This object is no longer active.
                lane_state.first_active_object += 1;
            }

            break;
        }
    }
}
