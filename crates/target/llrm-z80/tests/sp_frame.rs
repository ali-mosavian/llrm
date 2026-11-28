//! A frame touched in a few runs is addressed from SP: `push ix; ld ix,#0; add
//! ix,sp` and `ld sp,ix; pop ix` cost 12 B and 68 T a call for a function that
//! reads one stack argument (variadic `walk`, `value_of_valueless`: 103 B -> 79
//! B).

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const FIRST: &str = include_str!("kernels/ir/variadic_unnamed_on_stack.ll");

#[test]
fn a_function_reading_a_stack_argument_builds_no_ix_frame() {
    for level in [Level::O1, Level::O2, Level::Os] {
        let out = text(FIRST, "first", 0, &Request::new(level)).unwrap_or_else(|why| panic!("{why}"));
        assert!(!out.contains("ix"), "{level:?}:\n{out}");
        // With the switch off the function still builds no IX frame: its one
        // stack read is addressed through `iy` (15 B), against 21 B for
        // `push ix; ld ix, #0; add ix, sp; ...; ld sp, ix; pop ix` (#9814
        // priced it).
        let ix = Request { sp_frame: false, ..Request::new(level) };
        let kept = text(FIRST, "first", 0, &ix).unwrap_or_else(|why| panic!("{why}"));
        assert!(!kept.contains("push ix") && kept.contains("add iy, sp"), "{level:?}, the switch is off:\n{kept}");
    }
}
