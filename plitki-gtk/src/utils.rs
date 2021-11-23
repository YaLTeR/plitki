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
