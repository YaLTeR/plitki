#![allow(clippy::inconsistent_digit_grouping)]

use std::{
    collections::HashMap,
    fmt,
    io::{Read, Write},
    mem,
};

use plitki_core::{
    map::{Lane, Map, ScrollSpeedChange, TimeSignature},
    object::Object,
    scroll::ScrollSpeedMultiplier,
    timing::{MapTimestamp, MapTimestampDifference},
};
use serde::{de, Deserialize, Deserializer, Serialize};

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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TimingPoint {
    #[serde(default, rename = "StartTime")]
    pub start_time: f32,
    #[serde(default, rename = "Bpm")]
    pub bpm: f32,
    #[serde(
        default = "default_signature",
        rename = "Signature",
        deserialize_with = "deserialize_signature"
    )]
    pub signature: i32,
}

fn default_signature() -> i32 {
    4
}

fn deserialize_signature<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: Deserializer<'de>,
{
    struct TripleOrNumber;

    impl<'de> de::Visitor<'de> for TripleOrNumber {
        type Value = i32;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("\"Triple\" or i32")
        }

        fn visit_str<E>(self, value: &str) -> Result<i32, E>
        where
            E: de::Error,
        {
            if value != "Triple" {
                return Err(de::Error::invalid_value(
                    de::Unexpected::Str(value),
                    &"\"Triple\"",
                ));
            }

            Ok(3)
        }

        fn visit_u64<E>(self, value: u64) -> Result<i32, E>
        where
            E: de::Error,
        {
            if value > i32::max_value() as u64 {
                return Err(de::Error::invalid_value(
                    de::Unexpected::Unsigned(value),
                    &"i32",
                ));
            }

            Ok(value as i32)
        }
    }

    let value = deserializer.deserialize_any(TripleOrNumber)?;
    Ok(if value == 0 { 4 } else { value })
}

impl From<TimingPoint> for plitki_core::map::TimingPoint {
    #[inline]
    fn from(timing_point: TimingPoint) -> Self {
        Self {
            timestamp: MapTimestamp::from_milli_hundredths((timing_point.start_time * 100.) as i32),
            beat_duration: MapTimestampDifference::from_milli_hundredths(
                if timing_point.bpm == 0. {
                    i32::max_value()
                } else {
                    (60_000_00. / timing_point.bpm) as i32
                },
            ),
            signature: TimeSignature {
                beat_count: timing_point.signature as u8,
                beat_unit: 4,
            },
        }
    }
}

impl From<plitki_core::map::TimingPoint> for TimingPoint {
    #[inline]
    fn from(timing_point: plitki_core::map::TimingPoint) -> Self {
        assert_eq!(timing_point.signature.beat_unit, 4);

        Self {
            start_time: (timing_point.timestamp.into_milli_hundredths() as f32) / 100.,
            bpm: 60_000_00. / (timing_point.beat_duration.into_milli_hundredths() as f32),
            signature: i32::from(timing_point.signature.beat_count),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SliderVelocity {
    #[serde(default, rename = "StartTime")]
    pub start_time: f32,
    #[serde(default, rename = "Multiplier")]
    pub multiplier: f32,
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
                start: MapTimestamp::from_millis(hit_object.start_time),
                end: MapTimestamp::from_millis(hit_object.end_time),
            }
        } else {
            Object::Regular {
                timestamp: MapTimestamp::from_millis(hit_object.start_time),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    #[serde(rename = "AudioFile")]
    pub audio_file: Option<String>,
    #[serde(rename = "TimingPoints")]
    pub timing_points: Vec<TimingPoint>,
    #[serde(rename = "SliderVelocities")]
    pub slider_velocities: Vec<SliderVelocity>,
    #[serde(rename = "HitObjects")]
    pub hit_objects: Vec<HitObject>,
}

impl Qua {
    /// Returns the lane count of the map.
    #[inline]
    pub fn lane_count(&self) -> usize {
        self.mode.lane_count()
    }

    /// Computes the base BPM for the scroll speed multiplier.
    ///
    /// The base BPM corresponds to the multiplier of `1.0`.
    ///
    /// # Panics
    ///
    /// Panics if there are no objects or if the timing points are not sorted by the start time.
    pub fn base_bpm(&self) -> f32 {
        let last_object_end_time = self
            .hit_objects
            .iter()
            .map(|x| {
                if x.is_long_note() {
                    x.end_time
                } else {
                    x.start_time
                }
            })
            .max()
            .unwrap() as f32;

        let mut durations = HashMap::new();
        self.timing_points
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, x)| x.start_time <= last_object_end_time)
            .fold(last_object_end_time, |last_time, (i, timing_point)| {
                let start_time = if i == 0 { 0. } else { timing_point.start_time };
                let duration = last_time - timing_point.start_time;
                assert!(duration >= 0.);

                *durations.entry(timing_point.bpm.to_bits()).or_insert(0.) += duration;

                start_time
            });

        if durations.is_empty() {
            self.timing_points[0].bpm
        } else {
            let bits = *durations
                .iter()
                .max_by(|(bits_1, duration_1), (bits_2, duration_2)| {
                    duration_1
                        .partial_cmp(duration_2)
                        .unwrap()
                        // TODO: this is here so that in case multiple timing points have the same
                        // duration the same one is returned every time (since the HashMap
                        // iteration order is unstable). Quaver seems to have the same issue. Need
                        // to see which one osu! picks in this case.
                        .then(
                            f32::from_bits(**bits_1)
                                .partial_cmp(&f32::from_bits(**bits_2))
                                .unwrap(),
                        )
                })
                .unwrap()
                .0;
            f32::from_bits(bits)
        }
    }
}

impl From<Qua> for Map {
    #[inline]
    fn from(mut qua: Qua) -> Self {
        // TODO: this shouldn't panic and should probably be TryFrom instead.
        qua.timing_points
            .sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap());
        qua.slider_velocities
            .sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap());

        // TODO: this fallback isn't really justified...
        let base_bpm = if qua.hit_objects.is_empty() {
            qua.timing_points[0].bpm
        } else {
            qua.base_bpm()
        };

        let mut timing_points = Vec::with_capacity(qua.timing_points.len());

        let mut scroll_speed_changes = Vec::new();
        let mut current_bpm = qua.timing_points.first().unwrap().bpm;
        let mut current_sv_index = 0;
        let mut current_sv_start_time = None;
        let mut current_sv_multiplier = 1.;
        let mut current_adjusted_sv_multiplier = None;
        let mut initial_scroll_speed_multiplier = None;

        for timing_point in qua.timing_points.drain(..) {
            loop {
                if current_sv_index >= qua.slider_velocities.len() {
                    break;
                }

                let sv = qua.slider_velocities[current_sv_index];
                if sv.start_time > timing_point.start_time {
                    break;
                }

                if sv.start_time < timing_point.start_time {
                    let multiplier =
                        ScrollSpeedMultiplier::from_f32(sv.multiplier * (current_bpm / base_bpm));

                    if current_adjusted_sv_multiplier.is_none() {
                        current_adjusted_sv_multiplier = Some(multiplier);
                        initial_scroll_speed_multiplier = Some(multiplier);
                    }

                    if multiplier != current_adjusted_sv_multiplier.unwrap() {
                        scroll_speed_changes.push(ScrollSpeedChange {
                            timestamp: MapTimestamp::from_milli_hundredths(
                                (sv.start_time * 100.) as i32,
                            ),
                            multiplier,
                        });
                        current_adjusted_sv_multiplier = Some(multiplier);
                    }
                }

                current_sv_start_time = Some(sv.start_time);
                current_sv_multiplier = sv.multiplier;
                current_sv_index += 1;
            }

            // Timing points reset the previous SV multiplier.
            if current_sv_start_time
                .map(|x| x < timing_point.start_time)
                .unwrap_or(true)
            {
                current_sv_multiplier = 1.;
            }

            current_bpm = timing_point.bpm;

            let multiplier = ScrollSpeedMultiplier::saturating_from_f32(
                current_sv_multiplier * (current_bpm / base_bpm),
            );

            if current_adjusted_sv_multiplier.is_none() {
                current_adjusted_sv_multiplier = Some(multiplier);
                initial_scroll_speed_multiplier = Some(multiplier);
            }

            if multiplier != current_adjusted_sv_multiplier.unwrap() {
                scroll_speed_changes.push(ScrollSpeedChange {
                    timestamp: MapTimestamp::from_milli_hundredths(
                        (timing_point.start_time * 100.) as i32,
                    ),
                    multiplier,
                });
                current_adjusted_sv_multiplier = Some(multiplier);
            }

            timing_points.push(timing_point.into());
        }

        for sv in &qua.slider_velocities[current_sv_index..] {
            let multiplier = ScrollSpeedMultiplier::saturating_from_f32(
                sv.multiplier * (current_bpm / base_bpm),
            );
            if multiplier != current_adjusted_sv_multiplier.unwrap() {
                scroll_speed_changes.push(ScrollSpeedChange {
                    timestamp: MapTimestamp::from_milli_hundredths((sv.start_time * 100.) as i32),
                    multiplier,
                });
                current_adjusted_sv_multiplier = Some(multiplier);
            }
        }

        let mut lanes = vec![Lane::new(); qua.lane_count()];
        for hit_object in qua.hit_objects.drain(..) {
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
            audio_file: qua.audio_file,
            timing_points,
            scroll_speed_changes,
            initial_scroll_speed_multiplier: initial_scroll_speed_multiplier.unwrap_or_default(),
            lanes,
        }
    }
}

impl From<Map> for Qua {
    #[inline]
    fn from(mut map: Map) -> Self {
        // TODO: this shouldn't panic and should probably be TryFrom instead.
        let mut timing_points = map.timing_points.clone(); // TODO: get rid of the clone.
        let mut scroll_speed_changes = mem::replace(&mut map.scroll_speed_changes, Vec::new());
        let initial_scroll_speed_multiplier = map.initial_scroll_speed_multiplier;

        let mut qua = Self {
            mode: match map.lanes.len() {
                4 => GameMode::Keys4,
                7 => GameMode::Keys7,
                _ => panic!("Invalid lane count: {}", map.lanes.len()),
            },
            artist: map.song_artist,
            title: map.song_title,
            difficulty_name: map.difficulty_name,
            creator: map.mapper,
            audio_file: map.audio_file,
            timing_points: map.timing_points.into_iter().map(Into::into).collect(),
            slider_velocities: Vec::new(),
            hit_objects: map
                .lanes
                .into_iter()
                .enumerate()
                .flat_map(|(lane, Lane { objects })| {
                    let lane = lane as i32 + 1;

                    objects.into_iter().map(move |object| match object {
                        Object::Regular { timestamp } => HitObject {
                            start_time: timestamp.as_millis(),
                            lane,
                            end_time: 0,
                        },
                        Object::LongNote { start, end } => HitObject {
                            start_time: start.as_millis(),
                            lane,
                            end_time: end.as_millis(),
                        },
                    })
                })
                .collect(),
        };

        qua.timing_points
            .sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap());
        timing_points.sort_by_key(|a| a.timestamp);
        scroll_speed_changes.sort_by_key(|a| a.timestamp);

        // TODO: this fallback isn't really justified...
        let base_bpm = if qua.hit_objects.is_empty() {
            qua.timing_points[0].bpm
        } else {
            qua.base_bpm()
        };

        let mut slider_velocities = Vec::new();
        let mut current_bpm = qua.timing_points.first().map(|x| x.bpm).unwrap_or(base_bpm);
        let mut current_sv_index = 0;
        let mut current_sv_multiplier = initial_scroll_speed_multiplier;
        let mut current_adjusted_sv_multiplier = None;

        #[allow(clippy::float_cmp)]
        for (i, timing_point) in timing_points.iter().enumerate() {
            loop {
                if current_sv_index >= scroll_speed_changes.len() {
                    break;
                }

                let sv = &scroll_speed_changes[current_sv_index];
                if sv.timestamp > timing_point.timestamp {
                    break;
                }

                if sv.timestamp < timing_point.timestamp {
                    let multiplier = sv.multiplier.as_f32() / (current_bpm / base_bpm);

                    if current_adjusted_sv_multiplier.is_none()
                        || multiplier != current_adjusted_sv_multiplier.unwrap()
                    {
                        if current_adjusted_sv_multiplier.is_none()
                            && sv.multiplier != initial_scroll_speed_multiplier
                        {
                            // Insert an SV 1 ms earlier to simulate the initial scroll speed
                            // multiplier.
                            slider_velocities.push(SliderVelocity {
                                start_time: (sv.timestamp.into_milli_hundredths() as f32) / 100.
                                    - 1.,
                                multiplier: initial_scroll_speed_multiplier.as_f32()
                                    / (current_bpm / base_bpm),
                            });
                        }

                        slider_velocities.push(SliderVelocity {
                            start_time: (sv.timestamp.into_milli_hundredths() as f32) / 100.,
                            multiplier,
                        });
                        current_adjusted_sv_multiplier = Some(multiplier);
                    }
                }

                current_sv_multiplier = sv.multiplier;
                current_sv_index += 1;
            }

            current_bpm = 60_000_00. / timing_point.beat_duration.into_milli_hundredths() as f32;

            if current_adjusted_sv_multiplier.is_none()
                && current_sv_multiplier != initial_scroll_speed_multiplier
            {
                // Insert an SV 1 ms earlier to simulate the initial scroll speed multiplier.
                slider_velocities.push(SliderVelocity {
                    start_time: (timing_point.timestamp.into_milli_hundredths() as f32) / 100. - 1.,
                    multiplier: initial_scroll_speed_multiplier.as_f32() / (current_bpm / base_bpm),
                });
            }

            // Timing points reset the SV multiplier.
            current_adjusted_sv_multiplier = Some(1.);

            // Skip over multiple timing points at the same timestamp.
            if i + 1 < timing_points.len()
                && timing_points[i + 1].timestamp == timing_point.timestamp
            {
                continue;
            }

            let multiplier = current_sv_multiplier.as_f32() / (current_bpm / base_bpm);
            if multiplier != current_adjusted_sv_multiplier.unwrap() {
                slider_velocities.push(SliderVelocity {
                    start_time: timing_point.timestamp.into_milli_hundredths() as f32 / 100.,
                    multiplier,
                });
                current_adjusted_sv_multiplier = Some(multiplier);
            }
        }

        #[allow(clippy::float_cmp)]
        for sv in &scroll_speed_changes[current_sv_index..] {
            let multiplier = sv.multiplier.as_f32() / (current_bpm / base_bpm);
            if multiplier != current_adjusted_sv_multiplier.unwrap() {
                slider_velocities.push(SliderVelocity {
                    start_time: (sv.timestamp.into_milli_hundredths() as f32) / 100.,
                    multiplier,
                });
                current_adjusted_sv_multiplier = Some(multiplier);
            }
        }

        qua.slider_velocities = slider_velocities;
        qua
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
