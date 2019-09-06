use std::{cmp::Ordering, fs::File};

extern crate plitki_map_qua;
use plitki_map_qua::{from_reader, to_writer, GameMode, HitObject, Qua};

use plitki_core::{
    map::{Lane, Map},
    object::Object,
    timing::MapTimestamp,
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

fn hit_object_compare(a: &HitObject, b: &HitObject) -> Ordering {
    a.start_time
        .cmp(&b.start_time)
        .then(a.lane.cmp(&b.lane))
        .then(a.end_time.cmp(&b.end_time))
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

fn arbitrary_game_mode() -> impl Strategy<Value = GameMode> {
    prop_oneof![Just(GameMode::Keys4), Just(GameMode::Keys7)]
}

prop_compose! {
    fn arbitrary_qua()
                    (mode in arbitrary_game_mode())
                    (mode in Just(mode),
                     title in prop::option::of(any::<String>()),
                     artist in prop::option::of(any::<String>()),
                     creator in prop::option::of(any::<String>()),
                     difficulty_name in prop::option::of(any::<String>()),
                     audio_file in prop::option::of(any::<String>()),
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
