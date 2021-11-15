use std::{convert::TryFrom, time::Duration};

extern crate plitki_core;
use plitki_core::timing::Timestamp;

use proptest::prelude::*;

proptest! {
    #[allow(clippy::inconsistent_digit_grouping)]
    #[test]
    fn duration_to_timestamp_and_back(secs in 0..i32::max_value() as u64 / 1_000_00,
                                      rest in 0..1_000_00u32) {
        let duration = Duration::new(secs, rest * 10_000);
        let timestamp = Timestamp::try_from(duration).unwrap();
        let duration2 = Duration::try_from(timestamp).unwrap();

        prop_assert_eq!(duration, duration2);
    }
}
