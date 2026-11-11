//! `t[a < b ? 1 : 2]` over words: the index is `inc hl; inc hl; add hl, hl`.
//! Folding the shift into the add (shl (add x, 2), 1 -> add (shl x, 1), 4) cost
//! `ld bc, #4; add hl, bc` instead of two `inc hl`: bintree -O2 +14 B, -Os +7
//! B.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

#[test]
fn a_shifted_sum_with_a_small_constant_stays_increments_before_the_shift() {
    let ir = include_str!("kernels/ir/side_cell.ll");
    let out = text(ir, "side_cell", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(out.contains("inc hl\n\tinc hl\n\tadd hl, hl") && !out.contains("ld bc, #4"), "{out}");
}
