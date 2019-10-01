use std::{cmp::Ordering, fs::File};

extern crate plitki_map_qua;
use plitki_map_qua::{
    from_reader, to_writer, GameMode, HitObject, Qua, SliderVelocity, TimingPoint,
};

use plitki_core::{
    map::{Lane, Map, ScrollSpeedChange, TimeSignature},
    object::Object,
    scroll::ScrollSpeedMultiplier,
    timing::{MapTimestamp, MapTimestampDifference},
};
use pretty_assertions::assert_eq;
use proptest::prelude::*;

#[test]
fn parse_sample() {
    let file = File::open("tests/data/sample.qua").unwrap();
    let qua = from_reader(file).unwrap();

    let gt = Qua {
        mode: GameMode::Keys4,
        artist: Some("Unknown".to_owned()),
        title: Some("Sample Map".to_owned()),
        difficulty_name: Some("Easy".to_owned()),
        creator: Some("YaLTeR".to_owned()),
        audio_file: Some("song.mp3".to_owned()),
        timing_points: vec![
            TimingPoint {
                start_time: 0.,
                bpm: 100.,
                signature: 4,
            },
            TimingPoint {
                start_time: 200.,
                bpm: 200.,
                signature: 3,
            },
            TimingPoint {
                start_time: 400.,
                bpm: 200.,
                signature: 4,
            },
            TimingPoint {
                start_time: 500.,
                bpm: 200.,
                signature: 3,
            },
            TimingPoint {
                start_time: 10_000.,
                bpm: 0.,
                signature: 4,
            },
        ],
        slider_velocities: vec![
            SliderVelocity {
                start_time: 300.,
                multiplier: 2.,
            },
            SliderVelocity {
                start_time: 600.,
                multiplier: 0.,
            },
        ],
        hit_objects: vec![
            HitObject {
                start_time: 601,
                lane: 2,
                end_time: 0,
            },
            HitObject {
                start_time: 601,
                lane: 1,
                end_time: 0,
            },
            HitObject {
                start_time: 601,
                lane: 4,
                end_time: 939,
            },
            HitObject {
                start_time: 601,
                lane: 3,
                end_time: 939,
            },
            HitObject {
                start_time: 939,
                lane: 2,
                end_time: 1278,
            },
            HitObject {
                start_time: 939,
                lane: 1,
                end_time: 0,
            },
            HitObject {
                start_time: 1194,
                lane: 4,
                end_time: 1363,
            },
            HitObject {
                start_time: 1278,
                lane: 3,
                end_time: 0,
            },
        ],
    };

    assert_eq!(qua, gt);
}

#[test]
fn convert() {
    let file = File::open("tests/data/sample.qua").unwrap();
    let qua = from_reader(file).unwrap();
    let map: Map = qua.into();

    let gt = Map {
        song_artist: Some("Unknown".to_owned()),
        song_title: Some("Sample Map".to_owned()),
        difficulty_name: Some("Easy".to_owned()),
        mapper: Some("YaLTeR".to_owned()),
        audio_file: Some("song.mp3".to_owned()),
        timing_points: vec![
            plitki_core::map::TimingPoint {
                timestamp: MapTimestamp::from_millis(0),
                beat_duration: MapTimestampDifference::from_millis(600),
                signature: TimeSignature {
                    beat_count: 4,
                    beat_unit: 4,
                },
            },
            plitki_core::map::TimingPoint {
                timestamp: MapTimestamp::from_millis(200),
                beat_duration: MapTimestampDifference::from_millis(300),
                signature: TimeSignature {
                    beat_count: 3,
                    beat_unit: 4,
                },
            },
            plitki_core::map::TimingPoint {
                timestamp: MapTimestamp::from_millis(400),
                beat_duration: MapTimestampDifference::from_millis(300),
                signature: TimeSignature {
                    beat_count: 4,
                    beat_unit: 4,
                },
            },
            plitki_core::map::TimingPoint {
                timestamp: MapTimestamp::from_millis(500),
                beat_duration: MapTimestampDifference::from_millis(300),
                signature: TimeSignature {
                    beat_count: 3,
                    beat_unit: 4,
                },
            },
            plitki_core::map::TimingPoint {
                timestamp: MapTimestamp::from_millis(10_000),
                beat_duration: MapTimestampDifference::from_milli_hundredths(i32::max_value()),
                signature: TimeSignature {
                    beat_count: 4,
                    beat_unit: 4,
                },
            },
        ],
        scroll_speed_changes: vec![
            ScrollSpeedChange {
                timestamp: MapTimestamp::from_millis(200),
                multiplier: ScrollSpeedMultiplier::default(),
            },
            ScrollSpeedChange {
                timestamp: MapTimestamp::from_millis(300),
                multiplier: ScrollSpeedMultiplier::new(2000),
            },
            ScrollSpeedChange {
                timestamp: MapTimestamp::from_millis(400),
                multiplier: ScrollSpeedMultiplier::default(),
            },
            ScrollSpeedChange {
                timestamp: MapTimestamp::from_millis(600),
                multiplier: ScrollSpeedMultiplier::new(0),
            },
        ],
        initial_scroll_speed_multiplier: ScrollSpeedMultiplier::new(500),
        lanes: vec![
            Lane {
                objects: vec![
                    Object::Regular {
                        timestamp: MapTimestamp::from_millis(601),
                    },
                    Object::Regular {
                        timestamp: MapTimestamp::from_millis(939),
                    },
                ],
            },
            Lane {
                objects: vec![
                    Object::Regular {
                        timestamp: MapTimestamp::from_millis(601),
                    },
                    Object::LongNote {
                        start: MapTimestamp::from_millis(939),
                        end: MapTimestamp::from_millis(1278),
                    },
                ],
            },
            Lane {
                objects: vec![
                    Object::LongNote {
                        start: MapTimestamp::from_millis(601),
                        end: MapTimestamp::from_millis(939),
                    },
                    Object::Regular {
                        timestamp: MapTimestamp::from_millis(1278),
                    },
                ],
            },
            Lane {
                objects: vec![
                    Object::LongNote {
                        start: MapTimestamp::from_millis(601),
                        end: MapTimestamp::from_millis(939),
                    },
                    Object::LongNote {
                        start: MapTimestamp::from_millis(1194),
                        end: MapTimestamp::from_millis(1363),
                    },
                ],
            },
        ],
    };

    assert_eq!(map, gt);
}

#[test]
fn parse_actual_map() {
    let file = File::open("tests/data/actual_map.qua").unwrap();
    from_reader(file).unwrap();
}

#[test]
fn convert_actual_map() {
    let file = File::open("tests/data/actual_map.qua").unwrap();
    let qua = from_reader(file).unwrap();
    let _map: Map = qua.into();
}

#[test]
fn base_bpm_no_durations() {
    let qua = Qua {
        mode: GameMode::Keys4,
        title: None,
        artist: None,
        creator: None,
        difficulty_name: None,
        audio_file: None,
        timing_points: vec![
            TimingPoint {
                start_time: 100.0,
                bpm: 1.0,
                signature: 0,
            },
            TimingPoint {
                start_time: 200.0,
                bpm: 2.0,
                signature: 0,
            },
        ],
        slider_velocities: vec![],
        hit_objects: vec![HitObject {
            start_time: 0,
            lane: 0,
            end_time: 0,
        }],
    };

    #[allow(clippy::float_cmp)]
    {
        assert_eq!(qua.base_bpm(), 1.);
    }
}

#[test]
fn timing_points_override_svs() {
    let qua = Qua {
        mode: GameMode::Keys4,
        title: None,
        artist: None,
        creator: None,
        difficulty_name: None,
        audio_file: None,
        timing_points: vec![
            TimingPoint {
                start_time: 0.0,
                bpm: 1.0,
                signature: 4,
            },
            TimingPoint {
                start_time: 10.0,
                bpm: 2.0,
                signature: 4,
            },
        ],
        slider_velocities: vec![SliderVelocity {
            start_time: 5.0,
            multiplier: 10.0,
        }],
        hit_objects: vec![
            HitObject {
                start_time: 0,
                lane: 1,
                end_time: 0,
            },
            HitObject {
                start_time: 11,
                lane: 1,
                end_time: 0,
            },
        ],
    };

    let map: Map = qua.into();

    let gt = Map {
        song_artist: None,
        song_title: None,
        difficulty_name: None,
        mapper: None,
        audio_file: None,
        timing_points: vec![
            plitki_core::map::TimingPoint {
                timestamp: MapTimestamp::from_millis(0),
                beat_duration: MapTimestampDifference::from_millis(60_000),
                signature: TimeSignature {
                    beat_count: 4,
                    beat_unit: 4,
                },
            },
            plitki_core::map::TimingPoint {
                timestamp: MapTimestamp::from_millis(10),
                beat_duration: MapTimestampDifference::from_millis(30_000),
                signature: TimeSignature {
                    beat_count: 4,
                    beat_unit: 4,
                },
            },
        ],
        scroll_speed_changes: vec![
            ScrollSpeedChange {
                timestamp: MapTimestamp::from_millis(5),
                multiplier: ScrollSpeedMultiplier::new(10_000),
            },
            ScrollSpeedChange {
                timestamp: MapTimestamp::from_millis(10),
                multiplier: ScrollSpeedMultiplier::new(2000),
            },
        ],
        initial_scroll_speed_multiplier: ScrollSpeedMultiplier::default(),
        lanes: vec![
            Lane {
                objects: vec![
                    Object::Regular {
                        timestamp: MapTimestamp::from_millis(0),
                    },
                    Object::Regular {
                        timestamp: MapTimestamp::from_millis(11),
                    },
                ],
            },
            Lane { objects: vec![] },
            Lane { objects: vec![] },
            Lane { objects: vec![] },
        ],
    };

    assert_eq!(map, gt);
}

#[test]
fn sv_before_first_timing_point() {
    let qua = Qua {
        mode: GameMode::Keys4,
        title: None,
        artist: None,
        creator: None,
        difficulty_name: None,
        audio_file: None,
        timing_points: vec![TimingPoint {
            start_time: 0.0,
            bpm: 1.0,
            signature: 4,
        }],
        slider_velocities: vec![SliderVelocity {
            start_time: -10.0,
            multiplier: 10.0,
        }],
        hit_objects: vec![],
    };

    let map: Map = qua.into();

    let gt = Map {
        song_artist: None,
        song_title: None,
        difficulty_name: None,
        mapper: None,
        audio_file: None,
        timing_points: vec![plitki_core::map::TimingPoint {
            timestamp: MapTimestamp::from_millis(0),
            beat_duration: MapTimestampDifference::from_millis(60_000),
            signature: TimeSignature {
                beat_count: 4,
                beat_unit: 4,
            },
        }],
        scroll_speed_changes: vec![ScrollSpeedChange {
            timestamp: MapTimestamp::from_millis(0),
            multiplier: ScrollSpeedMultiplier::default(),
        }],
        initial_scroll_speed_multiplier: ScrollSpeedMultiplier::new(10_000),
        lanes: vec![
            Lane { objects: vec![] },
            Lane { objects: vec![] },
            Lane { objects: vec![] },
            Lane { objects: vec![] },
        ],
    };

    assert_eq!(map, gt);
}

#[test]
fn sv_at_first_timing_point() {
    let qua = Qua {
        mode: GameMode::Keys4,
        title: None,
        artist: None,
        creator: None,
        difficulty_name: None,
        audio_file: None,
        timing_points: vec![TimingPoint {
            start_time: 0.0,
            bpm: 1.0,
            signature: 4,
        }],
        slider_velocities: vec![SliderVelocity {
            start_time: 0.0,
            multiplier: 10.0,
        }],
        hit_objects: vec![],
    };

    let map: Map = qua.into();

    let gt = Map {
        song_artist: None,
        song_title: None,
        difficulty_name: None,
        mapper: None,
        audio_file: None,
        timing_points: vec![plitki_core::map::TimingPoint {
            timestamp: MapTimestamp::from_millis(0),
            beat_duration: MapTimestampDifference::from_millis(60_000),
            signature: TimeSignature {
                beat_count: 4,
                beat_unit: 4,
            },
        }],
        scroll_speed_changes: vec![],
        initial_scroll_speed_multiplier: ScrollSpeedMultiplier::new(10_000),
        lanes: vec![
            Lane { objects: vec![] },
            Lane { objects: vec![] },
            Lane { objects: vec![] },
            Lane { objects: vec![] },
        ],
    };

    assert_eq!(map, gt);
}

#[test]
fn sv_too_large() {
    let qua = Qua {
        mode: GameMode::Keys4,
        title: None,
        artist: None,
        creator: None,
        difficulty_name: None,
        audio_file: None,
        timing_points: vec![
            TimingPoint {
                start_time: 0.0,
                bpm: 1.0,
                signature: 4,
            },
            TimingPoint {
                start_time: 1.0,
                bpm: 6_000_000.0,
                signature: 4,
            },
        ],
        slider_velocities: vec![],
        hit_objects: vec![],
    };

    let map: Map = qua.into();

    let gt = Map {
        song_artist: None,
        song_title: None,
        difficulty_name: None,
        mapper: None,
        audio_file: None,
        timing_points: vec![
            plitki_core::map::TimingPoint {
                timestamp: MapTimestamp::from_millis(0),
                beat_duration: MapTimestampDifference::from_millis(60_000),
                signature: TimeSignature {
                    beat_count: 4,
                    beat_unit: 4,
                },
            },
            plitki_core::map::TimingPoint {
                timestamp: MapTimestamp::from_millis(1),
                beat_duration: MapTimestampDifference::from_milli_hundredths(1),
                signature: TimeSignature {
                    beat_count: 4,
                    beat_unit: 4,
                },
            },
        ],
        scroll_speed_changes: vec![ScrollSpeedChange {
            timestamp: MapTimestamp::from_millis(1),
            multiplier: ScrollSpeedMultiplier::new(2i32.pow(24) - 1),
        }],
        initial_scroll_speed_multiplier: ScrollSpeedMultiplier::default(),
        lanes: vec![
            Lane { objects: vec![] },
            Lane { objects: vec![] },
            Lane { objects: vec![] },
            Lane { objects: vec![] },
        ],
    };

    assert_eq!(map, gt);
}

#[test]
fn proptest_regression_1() {
    let mut qua = Qua {
        mode: GameMode::Keys4,
        title: None,
        artist: None,
        creator: None,
        difficulty_name: None,
        audio_file: None,
        timing_points: vec![TimingPoint {
            start_time: 0.0,
            bpm: 1.0,
            signature: 0,
        }],
        slider_velocities: vec![],
        hit_objects: vec![],
    };

    let map: Map = qua.clone().into();
    let mut qua2: Qua = map.into();

    qua.hit_objects.sort_unstable_by(hit_object_compare);
    qua2.hit_objects.sort_unstable_by(hit_object_compare);

    assert_eq!(qua, qua2);
}

#[test]
fn proptest_regression_2() {
    let mut qua = Qua {
        mode: GameMode::Keys4,
        title: None,
        artist: None,
        creator: None,
        difficulty_name: None,
        audio_file: None,
        timing_points: vec![TimingPoint {
            start_time: 0.0,
            bpm: 1.0,
            signature: 0,
        }],
        slider_velocities: vec![SliderVelocity {
            start_time: 0.0,
            multiplier: -8.0,
        }],
        hit_objects: vec![],
    };

    let map: Map = qua.clone().into();
    let mut qua2: Qua = map.into();

    qua.hit_objects.sort_unstable_by(hit_object_compare);
    qua2.hit_objects.sort_unstable_by(hit_object_compare);

    assert_eq!(qua, qua2);
}

#[test]
fn proptest_regression_3() {
    let mut qua = Qua {
        mode: GameMode::Keys4,
        title: None,
        artist: None,
        creator: None,
        difficulty_name: None,
        audio_file: None,
        timing_points: vec![TimingPoint {
            start_time: 0.0,
            bpm: 1.0,
            signature: 0,
        }],
        slider_velocities: vec![SliderVelocity {
            start_time: -12_057_820.0,
            multiplier: -8.0,
        }],
        hit_objects: vec![],
    };

    let map: Map = qua.clone().into();
    let mut qua2: Qua = map.into();

    qua.hit_objects.sort_unstable_by(hit_object_compare);
    qua2.hit_objects.sort_unstable_by(hit_object_compare);
    qua.slider_velocities[0].start_time = qua.timing_points[0].start_time - 1.;

    assert_eq!(qua, qua2);
}

#[test]
fn proptest_regression_4() {
    let mut qua = Qua {
        mode: GameMode::Keys4,
        title: None,
        artist: None,
        creator: None,
        difficulty_name: None,
        audio_file: None,
        timing_points: vec![TimingPoint {
            start_time: 0.0,
            bpm: 1.0,
            signature: 0,
        }],
        slider_velocities: vec![
            SliderVelocity {
                start_time: 0.0,
                multiplier: -8.0,
            },
            SliderVelocity {
                start_time: -15_698_166.0,
                multiplier: -8.0,
            },
            SliderVelocity {
                start_time: -12_057_820.0,
                multiplier: -4.0,
            },
        ],
        hit_objects: vec![],
    };

    let map: Map = qua.clone().into();
    let mut qua2: Qua = map.into();

    qua.hit_objects.sort_unstable_by(hit_object_compare);
    qua2.hit_objects.sort_unstable_by(hit_object_compare);
    normalize_svs(&mut qua);
    normalize_svs(&mut qua2);
    qua.slider_velocities[0].start_time = qua.slider_velocities[1].start_time - 1.;

    assert_eq!(qua, qua2);
}

#[test]
fn proptest_regression_5() {
    let mut qua = Qua {
        mode: GameMode::Keys4,
        title: None,
        artist: None,
        creator: None,
        difficulty_name: None,
        audio_file: None,
        timing_points: vec![
            TimingPoint {
                start_time: 0.0,
                bpm: 1.0,
                signature: 1,
            },
            TimingPoint {
                start_time: 0.0,
                bpm: 2.0,
                signature: 1,
            },
        ],
        slider_velocities: vec![SliderVelocity {
            start_time: -100.0,
            multiplier: -0.25,
        }],
        hit_objects: vec![HitObject {
            start_time: 0,
            lane: 1,
            end_time: 0,
        }],
    };

    let map: Map = qua.clone().into();
    let mut qua2: Qua = map.into();

    qua.hit_objects.sort_unstable_by(hit_object_compare);
    qua2.hit_objects.sort_unstable_by(hit_object_compare);
    qua.timing_points
        .sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap());
    qua2.timing_points
        .sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap());
    normalize_svs(&mut qua);
    normalize_svs(&mut qua2);
    qua.slider_velocities[0].start_time = qua.timing_points[0].start_time - 1.;

    assert_eq!(qua, qua2);
}

#[test]
fn proptest_regression_6() {
    let mut qua = Qua {
        mode: GameMode::Keys4,
        title: None,
        artist: None,
        creator: None,
        difficulty_name: None,
        audio_file: None,
        timing_points: vec![
            TimingPoint {
                start_time: -37700.0,
                bpm: 64.0,
                signature: 1,
            },
            TimingPoint {
                start_time: 0.0,
                bpm: 128.0,
                signature: 1,
            },
        ],
        slider_velocities: vec![SliderVelocity {
            start_time: -37800.0,
            multiplier: 2.0,
        }],
        hit_objects: vec![HitObject {
            start_time: 37920,
            lane: 1,
            end_time: 0,
        }],
    };

    let map: Map = qua.clone().into();
    let mut qua2: Qua = map.into();

    qua.hit_objects.sort_unstable_by(hit_object_compare);
    qua2.hit_objects.sort_unstable_by(hit_object_compare);
    qua.timing_points
        .sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap());
    qua2.timing_points
        .sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap());
    normalize_svs(&mut qua);
    normalize_svs(&mut qua2);
    qua.slider_velocities[0].start_time = qua.timing_points[0].start_time - 1.;

    assert_eq!(qua, qua2);
}

#[test]
fn proptest_regression_7() {
    let mut qua = Qua {
        mode: GameMode::Keys4,
        title: None,
        artist: None,
        creator: None,
        difficulty_name: None,
        audio_file: None,
        timing_points: vec![
            TimingPoint {
                start_time: -83000.0,
                bpm: 64.0,
                signature: 1,
            },
            TimingPoint {
                start_time: 0.0,
                bpm: 128.0,
                signature: 1,
            },
        ],
        slider_velocities: vec![],
        hit_objects: vec![HitObject {
            start_time: 0,
            lane: 1,
            end_time: 201_025,
        }],
    };

    let map: Map = qua.clone().into();
    let mut qua2: Qua = map.into();

    qua.hit_objects.sort_unstable_by(hit_object_compare);
    qua2.hit_objects.sort_unstable_by(hit_object_compare);
    qua.timing_points
        .sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap());
    qua2.timing_points
        .sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap());
    normalize_svs(&mut qua);
    normalize_svs(&mut qua2);

    assert_eq!(qua, qua2);
}

#[test]
fn proptest_regression_8() {
    let mut qua = Qua {
        mode: GameMode::Keys4,
        title: None,
        artist: None,
        creator: None,
        difficulty_name: None,
        audio_file: None,
        timing_points: vec![
            TimingPoint {
                start_time: -98800.0,
                bpm: 128.0,
                signature: 227,
            },
            TimingPoint {
                start_time: -91100.0,
                bpm: 128.0,
                signature: 179,
            },
            TimingPoint {
                start_time: -88500.0,
                bpm: 128.0,
                signature: 198,
            },
            TimingPoint {
                start_time: -79000.0,
                bpm: 64.0,
                signature: 70,
            },
            TimingPoint {
                start_time: -70700.0,
                bpm: 128.0,
                signature: 68,
            },
            TimingPoint {
                start_time: -62600.0,
                bpm: 128.0,
                signature: 75,
            },
            TimingPoint {
                start_time: -60000.0,
                bpm: 128.0,
                signature: 82,
            },
            TimingPoint {
                start_time: -58300.0,
                bpm: 64.0,
                signature: 180,
            },
            TimingPoint {
                start_time: -50400.0,
                bpm: 64.0,
                signature: 137,
            },
            TimingPoint {
                start_time: -45400.0,
                bpm: 64.0,
                signature: 99,
            },
            TimingPoint {
                start_time: -45300.0,
                bpm: 64.0,
                signature: 89,
            },
            TimingPoint {
                start_time: -43500.0,
                bpm: 64.0,
                signature: 1,
            },
            TimingPoint {
                start_time: -40900.0,
                bpm: 64.0,
                signature: 33,
            },
            TimingPoint {
                start_time: -39000.0,
                bpm: 64.0,
                signature: 32,
            },
            TimingPoint {
                start_time: -38800.0,
                bpm: 128.0,
                signature: 104,
            },
            TimingPoint {
                start_time: -36300.0,
                bpm: 64.0,
                signature: 173,
            },
            TimingPoint {
                start_time: -32800.0,
                bpm: 64.0,
                signature: 71,
            },
            TimingPoint {
                start_time: -29800.0,
                bpm: 64.0,
                signature: 202,
            },
            TimingPoint {
                start_time: -28400.0,
                bpm: 64.0,
                signature: 240,
            },
            TimingPoint {
                start_time: -18700.0,
                bpm: 64.0,
                signature: 115,
            },
            TimingPoint {
                start_time: -17700.0,
                bpm: 64.0,
                signature: 36,
            },
            TimingPoint {
                start_time: 4700.0,
                bpm: 64.0,
                signature: 199,
            },
            TimingPoint {
                start_time: 6300.0,
                bpm: 128.0,
                signature: 231,
            },
            TimingPoint {
                start_time: 6900.0,
                bpm: 128.0,
                signature: 106,
            },
            TimingPoint {
                start_time: 14400.0,
                bpm: 128.0,
                signature: 131,
            },
            TimingPoint {
                start_time: 16200.0,
                bpm: 64.0,
                signature: 3,
            },
            TimingPoint {
                start_time: 16700.0,
                bpm: 128.0,
                signature: 29,
            },
            TimingPoint {
                start_time: 39300.0,
                bpm: 64.0,
                signature: 157,
            },
            TimingPoint {
                start_time: 39400.0,
                bpm: 128.0,
                signature: 18,
            },
            TimingPoint {
                start_time: 46000.0,
                bpm: 64.0,
                signature: 112,
            },
            TimingPoint {
                start_time: 55900.0,
                bpm: 64.0,
                signature: 168,
            },
            TimingPoint {
                start_time: 62600.0,
                bpm: 64.0,
                signature: 53,
            },
            TimingPoint {
                start_time: 66900.0,
                bpm: 64.0,
                signature: 206,
            },
            TimingPoint {
                start_time: 84800.0,
                bpm: 64.0,
                signature: 167,
            },
            TimingPoint {
                start_time: 89300.0,
                bpm: 128.0,
                signature: 129,
            },
            TimingPoint {
                start_time: 92500.0,
                bpm: 128.0,
                signature: 16,
            },
            TimingPoint {
                start_time: 94500.0,
                bpm: 64.0,
                signature: 92,
            },
            TimingPoint {
                start_time: 98400.0,
                bpm: 64.0,
                signature: 211,
            },
        ],
        slider_velocities: vec![
            SliderVelocity {
                start_time: 46000.0,
                multiplier: -8.0,
            },
            SliderVelocity {
                start_time: 55900.0,
                multiplier: 1.0,
            },
        ],
        hit_objects: vec![
            HitObject {
                start_time: 202462,
                lane: 1,
                end_time: 0,
            },
            HitObject {
                start_time: 970163,
                lane: 1,
                end_time: 15680283,
            },
            HitObject {
                start_time: 2506236,
                lane: 3,
                end_time: 19481258,
            },
            HitObject {
                start_time: 2852464,
                lane: 1,
                end_time: 20351517,
            },
            HitObject {
                start_time: 3544150,
                lane: 3,
                end_time: 20464567,
            },
            HitObject {
                start_time: 7072036,
                lane: 1,
                end_time: 0,
            },
            HitObject {
                start_time: 8092560,
                lane: 2,
                end_time: 0,
            },
            HitObject {
                start_time: 8228738,
                lane: 4,
                end_time: 12197545,
            },
            HitObject {
                start_time: 8636771,
                lane: 3,
                end_time: 9902122,
            },
            HitObject {
                start_time: 9678597,
                lane: 4,
                end_time: 12003914,
            },
            HitObject {
                start_time: 11177954,
                lane: 4,
                end_time: 0,
            },
            HitObject {
                start_time: 17004952,
                lane: 2,
                end_time: 19753175,
            },
            HitObject {
                start_time: 19669830,
                lane: 1,
                end_time: 0,
            },
            HitObject {
                start_time: 20457648,
                lane: 1,
                end_time: 0,
            },
            HitObject {
                start_time: 21287804,
                lane: 3,
                end_time: 21458135,
            },
        ],
    };

    let map: Map = qua.clone().into();
    let mut qua2: Qua = map.into();

    qua.hit_objects.sort_unstable_by(hit_object_compare);
    qua2.hit_objects.sort_unstable_by(hit_object_compare);
    qua.timing_points
        .sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap());
    qua2.timing_points
        .sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap());
    normalize_svs(&mut qua);
    normalize_svs(&mut qua2);

    assert_eq!(qua, qua2);
}

fn hit_object_compare(a: &HitObject, b: &HitObject) -> Ordering {
    a.start_time
        .cmp(&b.start_time)
        .then(a.lane.cmp(&b.lane))
        .then(a.end_time.cmp(&b.end_time))
}

/// Removes SVs which don't affect the map.
fn normalize_svs(qua: &mut Qua) {
    let mut slider_velocities = Vec::with_capacity(qua.slider_velocities.len());

    qua.slider_velocities
        .sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap());
    let mut current_sv_multiplier = 1.;

    let mut next_timing_point_index = 0;

    #[allow(clippy::float_cmp)]
    for mut i in 0..qua.slider_velocities.len() {
        let mut sv = &qua.slider_velocities[i];
        loop {
            // Take the last SV at this timestamp.
            if i == qua.slider_velocities.len() - 1
                || qua.slider_velocities[i + 1].start_time > sv.start_time
            {
                break;
            }

            i += 1;
            sv = &qua.slider_velocities[i];
        }

        // Timing points reset the SV multiplier.
        while next_timing_point_index < qua.timing_points.len()
            && qua.timing_points[next_timing_point_index].start_time <= sv.start_time
        {
            next_timing_point_index += 1;
            current_sv_multiplier = 1.;
        }

        // Skip SVs which don't change the multiplier.
        if sv.multiplier != current_sv_multiplier {
            slider_velocities.push(*sv);
            current_sv_multiplier = sv.multiplier;
        }
    }

    qua.slider_velocities = slider_velocities;
}

#[test]
fn normalize_svs_test() {
    let mut qua = Qua {
        mode: GameMode::Keys4,
        title: None,
        artist: None,
        creator: None,
        difficulty_name: None,
        audio_file: None,
        timing_points: vec![TimingPoint {
            start_time: 0.0,
            bpm: 1.0,
            signature: 0,
        }],
        slider_velocities: vec![
            SliderVelocity {
                start_time: 0.,
                multiplier: 1.0,
            },
            SliderVelocity {
                start_time: 1.,
                multiplier: 2.0,
            },
            SliderVelocity {
                start_time: 2.,
                multiplier: 2.0,
            },
            SliderVelocity {
                start_time: 3.,
                multiplier: 1.0,
            },
        ],
        hit_objects: vec![],
    };

    normalize_svs(&mut qua);

    assert_eq!(
        &qua.slider_velocities[..],
        &[
            SliderVelocity {
                start_time: 1.,
                multiplier: 2.0,
            },
            SliderVelocity {
                start_time: 3.,
                multiplier: 1.0,
            },
        ][..]
    );
}

prop_compose! {
    fn arbitrary_hit_object(mode: GameMode)
                           (start_time in 0..i32::max_value() / 100, // TODO
                            is_long_note in any::<bool>())
                           (start_time in Just(start_time),
                            lane in 1..=mode.lane_count(),
                            end_time in if is_long_note {
                                (start_time..i32::max_value() / 100).boxed()
                            } else {
                                Just(0).boxed()
                            })
                           -> HitObject {
        HitObject {
            start_time,
            lane: lane as i32,
            end_time,
        }
    }
}

prop_compose! {
    fn arbitrary_timing_point()
                             (start_time in -1000..1000, // TODO
                              // Use BPMs which give exactly-representable floats.
                              // Only use 2 orders of magnitude so the SVs are always
                              // representable.
                              bpm_log2 in 6..8u32,
                              signature in 1..u8::max_value() as i32) // TODO
                             -> TimingPoint {
        TimingPoint {
            start_time: start_time as f32 * 100.,
            bpm: f32::from(2u16.pow(bpm_log2)),
            signature,
        }
    }
}

prop_compose! {
    fn arbitrary_slider_velocity(first_sv_start_time: i32)
                                (start_time in first_sv_start_time / 100..first_sv_start_time / 100 + 1000, // TODO
                                 // Use exactly-representable floats.
                                 multiplier in prop::sample::select(
                                     &[-8., -4., -2., -1., -0.5, 0., 0.5, 1., 2., 4., 8.][..]
                                 ))
                                -> SliderVelocity {
        SliderVelocity {
            start_time: start_time as f32 * 100.,
            multiplier,
        }
    }
}

fn arbitrary_game_mode() -> impl Strategy<Value = GameMode> {
    prop_oneof![Just(GameMode::Keys4), Just(GameMode::Keys7)]
}

prop_compose! {
    fn arbitrary_qua()
                    (mode in arbitrary_game_mode(),
                     timing_points in prop::collection::vec(arbitrary_timing_point(), 1..64))
                    (mode in Just(mode),
                     title in prop::option::of(any::<String>()),
                     artist in prop::option::of(any::<String>()),
                     creator in prop::option::of(any::<String>()),
                     difficulty_name in prop::option::of(any::<String>()),
                     audio_file in prop::option::of(any::<String>()),
                     slider_velocities in prop::collection::vec(
                         arbitrary_slider_velocity(timing_points[0].start_time as i32),
                         0..64,
                     ),
                     timing_points in Just(timing_points),
                     hit_objects in prop::collection::vec(arbitrary_hit_object(mode), 0..64))
                    -> Qua {
        Qua {
            mode,
            title,
            artist,
            creator,
            difficulty_name,
            audio_file,
            hit_objects,
            timing_points,
            slider_velocities,
        }
    }
}

proptest! {
    #[test]
    fn qua_to_map_and_back(mut qua in arbitrary_qua()) {
        let map: Map = qua.clone().into();
        let mut qua2: Qua = map.into();

        qua.hit_objects.sort_unstable_by(hit_object_compare);
        qua2.hit_objects.sort_unstable_by(hit_object_compare);
        qua.timing_points.sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap());
        qua2.timing_points.sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap());
        normalize_svs(&mut qua);
        normalize_svs(&mut qua2);

        prop_assert_eq!(qua, qua2);
    }

    #[test]
    fn qua_serialize_deserialize(mut qua in arbitrary_qua()) {
        let mut buf = Vec::new();
        to_writer(&mut buf, &qua).unwrap();
        let mut qua2 = from_reader(&buf[..]).unwrap();

        qua.hit_objects.sort_unstable_by(hit_object_compare);
        qua2.hit_objects.sort_unstable_by(hit_object_compare);

        prop_assert_eq!(qua, qua2);
    }
}
