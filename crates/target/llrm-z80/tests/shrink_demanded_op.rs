//! A value read only in its low bytes is computed in them.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// `val >> (iv + off)` added the shift count's two words (`add hl, bc`, 61 T with
/// the moves) for the one byte the shift reads: `ld a, l; add a, e`
/// (shift_until_idioms `offset_count` -O2).
#[test]
fn a_shift_count_that_is_a_sum_is_added_in_a_byte() {
    let asm = text(include_str!("kernels/ir/shift_count_sum.ll"), "g", 0, &Request::new(Level::O2)).expect("compiles");
    assert!(!asm.contains("add hl, bc"), "{asm}");
}
