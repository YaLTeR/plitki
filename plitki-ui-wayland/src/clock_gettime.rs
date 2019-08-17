use std::time::Duration;

use libc::{self, timespec};

// Is there a crate that does this for me?
pub fn clock_gettime(clk_id: u32) -> Duration {
    assert!(clk_id <= i32::max_value() as u32);
    let clk_id = clk_id as i32;

    let mut out = timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };

    let rv = unsafe { libc::clock_gettime(clk_id, &mut out) };
    assert_eq!(rv, 0);

    Duration::new(out.tv_sec as u64, out.tv_nsec as u32)
}
