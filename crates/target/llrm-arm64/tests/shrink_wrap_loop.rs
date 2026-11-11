//! Shrink wrapping weighs a block by the loops it is in, not by where it sits.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// A join laid out after a frame-free loop that jumps to it counted as inside
/// the loop (blocks between a back edge's ends, in layout order), so the frame
/// was made at the top and the loop ran under it; the pin makes it after the
/// loop (recpow: 77742 instrs against 44336).
#[test]
fn the_frame_is_made_after_a_loop_that_needs_none() {
    let asm = driver::assembly(include_str!("data/shrink_wrap_loop.ll"), Platform::Linux, Level::O2).expect("lowers");
    let start = asm.find("power:").expect("the function is there");
    let body = &asm[start..];
    let end = body.find("entry_point:").unwrap_or(body.len());
    let body = &body[..end];
    let frame = body.find("stp x29, x30").expect("the frame");
    let spin = body.find("msub").expect("the loop");
    assert!(spin < frame, "{body}");
}
