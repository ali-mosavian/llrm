//! A soft-float operation is a call to the runtime, and the inliner prices it as one. x87rotate -O2: `rot` (external,
//! ten `fmuladd` in its loop and epilogue, each a `__mulsf3` and an `__addsf3`) priced 21 units, was inlined into its
//! one caller and the body stood twice: 3764 B against SDCC's 3500.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const IR: &str = include_str!("data/soft_float_inline_o2.ll");

#[test]
fn an_external_callee_of_soft_float_work_is_called_not_inlined() {
    let main = text(IR, "main", 1, &Request::new(Level::O2)).expect("compiles");
    assert!(main.contains("call _rot"), "rot was inlined into main:\n{main}");
}
