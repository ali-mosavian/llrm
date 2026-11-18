//! A callee that stays and keeps a frame.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// `main` inlined `bench_queens` (a callee that stays, 37 B, with a 24 B local
/// array): `main` grew a frame of its own, `ld iy, #-24; add iy, sp; ld sp,
/// iy` and its undoing, and a copy of the body, 43 B for a 8 B call
/// (queens -Os: 410 B, SDCC 342 B). The size guard counted the frame as
/// moving, not as added.
#[test]
fn a_callee_that_stays_is_not_inlined_into_a_caller_with_no_frame() {
    for level in [Level::Os, Level::O1] {
        let asm = text(include_str!("data/queens_inline_frame.ll"), "main", 0, &Request::new(level)).expect("compiles");
        assert!(asm.contains("call _bench_queens"), "{level:?}\n{asm}");
    }
}
