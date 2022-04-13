use plitki_core::scroll::ScreenPositionDifference;

const SPD_PER_PX_I64: i64 = 2_000_000;

pub(crate) fn to_pixels(length: ScreenPositionDifference) -> i32 {
    let pixels = length.0.checked_add(SPD_PER_PX_I64 - 1).unwrap() / SPD_PER_PX_I64;
    pixels.try_into().unwrap()
}
