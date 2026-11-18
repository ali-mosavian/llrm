//! `t[a < b ? 1 : 2]` over words: the index is two `inc`s or a byte `add a, #2`
//! before the shift. Folding the shift into the add (shl (add x, 2), 1 -> add
//! (shl x, 1), 4) cost `ld bc, #4; add hl, bc` instead: bintree -O2 +14 B, -Os
//! +7 B.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

#[test]
fn a_shifted_sum_with_a_small_constant_is_not_folded_into_a_wide_add() {
    let ir = include_str!("kernels/ir/side_cell.ll");
    let out = text(ir, "side_cell", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(!out.contains("ld bc, #4"), "{out}");
}
