use std::{
    convert::{TryFrom, TryInto},
    io::{Read, Write},
    time::Duration,
};

use plitki_core::{
    map::{Lane, Map},
    object::Object,
    timing::MapTimestamp,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum GameMode {
    Keys4,
    Keys7,
}

impl GameMode {
    /// Returns the lane count of the game mode.
    #[inline]
    pub fn lane_count(self) -> usize {
        match self {
            GameMode::Keys4 => 4,
            GameMode::Keys7 => 7,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub struct HitObject {
    #[serde(rename = "StartTime")]
    pub start_time: i32,
    #[serde(rename = "Lane")]
    pub lane: i32,
    #[serde(default, rename = "EndTime")]
    pub end_time: i32,
}

impl HitObject {
    /// Returns `true` if the hit object is a long note.
    #[inline]
    pub fn is_long_note(&self) -> bool {
        self.end_time > 0
    }
}

impl From<HitObject> for Object {
    #[inline]
    fn from(hit_object: HitObject) -> Self {
        // TODO: this shouldn't panic and should probably be TryFrom instead.
        if hit_object.is_long_note() {
            Object::LongNote {
                start: MapTimestamp(
                    Duration::from_millis(hit_object.start_time as u64)
                        .try_into()
                        .unwrap(),
                ),
                end: MapTimestamp(
                    Duration::from_millis(hit_object.end_time as u64)
                        .try_into()
                        .unwrap(),
                ),
            }
        } else {
            Object::Regular {
                timestamp: MapTimestamp(
                    Duration::from_millis(hit_object.start_time as u64)
                        .try_into()
                        .unwrap(),
                ),
            }
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Qua {
    #[serde(rename = "Mode")]
    pub mode: GameMode,
    #[serde(rename = "Title")]
    pub title: Option<String>,
    #[serde(rename = "Artist")]
    pub artist: Option<String>,
    #[serde(rename = "Creator")]
    pub creator: Option<String>,
    #[serde(rename = "DifficultyName")]
    pub difficulty_name: Option<String>,
    #[serde(rename = "HitObjects")]
    pub hit_objects: Vec<HitObject>,
}

impl Qua {
    /// Returns the lane count of the map.
    #[inline]
    pub fn lane_count(&self) -> usize {
        self.mode.lane_count()
    }
}

impl From<Qua> for Map {
    #[inline]
    fn from(qua: Qua) -> Self {
        // TODO: this shouldn't panic and should probably be TryFrom instead.
        let mut lanes = vec![Lane::new(); qua.lane_count()];
        for hit_object in qua.hit_objects.into_iter() {
            assert!(hit_object.lane > 0);
            lanes[(hit_object.lane - 1) as usize]
                .objects
                .push(hit_object.into());
        }

        Self {
            song_artist: qua.artist,
            song_title: qua.title,
            difficulty_name: qua.difficulty_name,
            mapper: qua.creator,
            lanes,
        }
    }
}

impl From<Map> for Qua {
    #[inline]
    fn from(map: Map) -> Self {
        // TODO: this shouldn't panic and should probably be TryFrom instead.
        Self {
            mode: match map.lanes.len() {
                4 => GameMode::Keys4,
                7 => GameMode::Keys7,
                _ => panic!("Invalid lane count: {}", map.lanes.len()),
            },
            artist: map.song_artist,
            title: map.song_title,
            difficulty_name: map.difficulty_name,
            creator: map.mapper,
            hit_objects: map
                .lanes
                .into_iter()
                .enumerate()
                .flat_map(|(lane, Lane { objects })| {
                    let lane = lane as i32 + 1;

                    objects.into_iter().map(move |object| match object {
                        Object::Regular { timestamp } => HitObject {
                            start_time: Duration::try_from(timestamp.0).unwrap().as_millis() as i32,
                            lane,
                            end_time: 0,
                        },
                        Object::LongNote { start, end } => HitObject {
                            start_time: Duration::try_from(start.0).unwrap().as_millis() as i32,
                            lane,
                            end_time: Duration::try_from(end.0).unwrap().as_millis() as i32,
                        },
                    })
                })
                .collect(),
        }
    }
}

/// Deserializes a `Qua` from an IO stream of YAML.
pub fn from_reader<R: Read>(reader: R) -> Result<Qua, serde_yaml::Error> {
    serde_yaml::from_reader(reader)
}

/// Serializes a `Qua` as YAML into the IO stream.
pub fn to_writer<W: Write>(writer: W, qua: &Qua) -> Result<(), serde_yaml::Error> {
    serde_yaml::to_writer(writer, qua)
}
