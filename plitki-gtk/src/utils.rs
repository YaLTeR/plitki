use plitki_core::scroll::ScreenPositionDifference;

pub(crate) fn to_pixels(length: ScreenPositionDifference, lane_width: i32, lane_count: i32) -> i32 {
    let playfield_width = lane_width * lane_count;
    let pixels = length
        .0
        .checked_mul(playfield_width.into())
        .unwrap()
        .checked_add(2_000_000_000 - 1)
        .unwrap()
        / 2_000_000_000;
    pixels.try_into().unwrap()
}

pub(crate) fn to_pixels_f64(
    length: ScreenPositionDifference,
    lane_width: i32,
    lane_count: i32,
) -> f64 {
    let playfield_width = lane_width * lane_count;
    length.0 as f64 / 2_000_000_000. * playfield_width as f64
}

pub(crate) fn from_pixels_f64(
    pixels: f64,
    lane_width: i32,
    lane_count: i32,
) -> ScreenPositionDifference {
    let playfield_width = lane_width * lane_count;
    let length = (pixels / playfield_width as f64 * 2_000_000_000.) as i64;
    ScreenPositionDifference(length)
}

#[cfg(test)]
mod tests {
    use super::*;

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn from_to_pixels_f64_returns_result_within_error_margin(
            length in 0..10_000_000_000i64,
            lane_count in 1..10,
            lane_width in 1..10_000,
        ) {
            let pixels = to_pixels_f64(ScreenPositionDifference(length), lane_width, lane_count);
            let result = from_pixels_f64(pixels, lane_width, lane_count).0;
            prop_assert!((length - result).abs() <= 1);
        }

        #[test]
        fn from_to_from_pixels_f64_returns_length_within_error_margin(
            lane_count in 1..10,
            prev_pixels in 0..100_000,
            prev_lane_width in 1..10_000,
            lane_width in 1..10_000,
        ) {
            let length = from_pixels_f64(prev_pixels as f64, prev_lane_width, lane_count);
            let pixels = to_pixels_f64(length, lane_width, lane_count);
            let result = from_pixels_f64(pixels, lane_width, lane_count);
            prop_assert!((length - result).0.abs() <= 1);
        }
    }
}
