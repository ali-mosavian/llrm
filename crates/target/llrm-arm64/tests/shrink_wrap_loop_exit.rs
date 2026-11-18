//! A function whose frame is needed only after an early exit makes the frame
//! there, even when the exit's return is shared with the loop that follows.
//! quicksort -O2's `sort` made its frame (3 stores) and took it down (4
//! instructions) on all 353 of its 682 calls that find an empty range: 119,494
//! instructions against clang's 119,201.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn the_frame_is_made_after_the_empty_range_test() {
    let asm =
        driver::assembly(include_str!("data/shrink_wrap_loop_exit.ll"), Platform::Linux, Level::O2).expect("lowers");
    let first: Vec<&str> =
        asm.lines().map(str::trim).filter(|line| !line.starts_with('.') && !line.ends_with(':')).take(2).collect();
    assert!(first[0].starts_with("subs") && first[1].starts_with("b.ge"), "{asm}");
}
