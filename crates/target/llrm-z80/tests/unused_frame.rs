//! A function whose argument home nothing uses makes no frame.

use llrm_z80::driver::{Level, Request};

/// `by_bytes` (-O1) took a 12 B `push ix` ... `pop ix` frame around a body
/// that never touches IX: the argument's home was a fixed object, and a fixed
/// object alone switched the frame on.
#[test]
fn an_argument_home_nothing_uses_makes_no_frame() {
    let ir = include_str!("data/unused_argument_home.ll");
    let lines =
        llrm_z80::compile::text(ir, "by_bytes", 0, &Request::new(Level::O1)).unwrap_or_else(|why| panic!("{why}"));
    assert!(!lines.contains("(ix)") && !lines.contains("push ix"), "by_bytes made a frame:\n{lines}");
}
