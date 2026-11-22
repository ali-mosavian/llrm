//! Values a loop computes on some trips only.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// queens' `place` hoisted `&q[row]` and `row + 1`, each used on the trips that
/// find a safe column, out of the column loop (LICM hoists what it may
/// speculate): two more values held across the loop, so two more frame slots
/// (-Os 379 B against 360 B, 56 frame accesses against 52).
#[test]
fn a_value_the_loop_computes_on_some_trips_is_not_held_across_it() {
    for level in [Level::Os, Level::O1] {
        let asm =
            text(include_str!("data/queens_inline_frame.ll"), "place", 0, &Request::new(level)).expect("compiles");
        let frame = asm.lines().filter(|line| line.contains("(ix)")).count();
        assert!(frame < 54, "{level:?}: {frame} frame accesses\n{asm}");
    }
}
