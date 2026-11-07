//! `(long & 7) == 3` was a word compare with `sub a, #0` for the high byte the mask
//! zeroed: the narrowing of `SimplifySetCC` stopped at the illegal `i3`
//! (div_constants_short: 36 such sites). The compare is one byte `cp`.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

#[test]
fn a_masked_value_compared_with_a_constant_is_one_byte_compare() {
    let ir = include_str!("kernels/ir/masked_compare.ll");
    for level in [Level::O2, Level::Os] {
        let out = text(ir, "masked_compare", 0, &Request::new(level)).unwrap_or_else(|why| panic!("{why}"));
        assert!(out.contains("cp a, #3") && !out.contains("sub a, #0"), "{out}");
    }
}
