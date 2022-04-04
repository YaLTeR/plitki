use plitki_core::scroll::ScreenPositionDifference;

const SPD_PER_PX_I64: i64 = 2_000_000;
const SPD_PER_PX_F64: f64 = 2_000_000.;

pub(crate) fn to_pixels(length: ScreenPositionDifference) -> i32 {
    let pixels = length.0.checked_add(SPD_PER_PX_I64 - 1).unwrap() / SPD_PER_PX_I64;
    pixels.try_into().unwrap()
}

pub(crate) fn to_pixels_f64(length: ScreenPositionDifference) -> f64 {
    length.0 as f64 / SPD_PER_PX_F64
}

pub(crate) fn from_pixels_f64(pixels: f64) -> ScreenPositionDifference {
    let length = (pixels * SPD_PER_PX_F64) as i64;
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
        ) {
            let pixels = to_pixels_f64(ScreenPositionDifference(length));
            let result = from_pixels_f64(pixels).0;
            prop_assert!((length - result).abs() <= 1);
        }

        #[test]
        fn from_to_from_pixels_f64_returns_length_within_error_margin(
            prev_pixels in 0..100_000,
        ) {
            let length = from_pixels_f64(prev_pixels as f64);
            let pixels = to_pixels_f64(length);
            let result = from_pixels_f64(pixels);
            prop_assert!((length - result).0.abs() <= 1);
        }
    }
}
