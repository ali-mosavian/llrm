//! `div_constants_short`'s `c_short_3` as one function: its stack traffic at
//! -Os.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// The slot stores and loads of the function's text: `ld N(ix), r` and `ld r,
/// N(ix)`.
fn slot_traffic(text: &str) -> (usize, usize) {
    let (mut stores, mut loads) = (0, 0);
    for line in text.lines().map(str::trim) {
        let Some(operands) = line.strip_prefix("ld ") else { continue };
        let Some((to, from)) = operands.split_once(", ") else { continue };
        stores += usize::from(to.ends_with("(ix)"));
        loads += usize::from(from.ends_with("(ix)"));
    }
    (stores, loads)
}

/// Allocation keeps the long temporaries of 20-odd constant divides in stack
/// slots: 75 slot stores and 90 loads (1072 B), where SDCC has 24 and 50
/// (646 B). Targets: stores <= 40, loads <= 60, and the function <= 800 B
/// (`~/scratch/z80-cause-card/count.py` on the listing). Remove `ignore` when
/// the allocator gets there.
#[test]
#[ignore = "greedy-subreg: the spill cause card"]
fn c_short_3_keeps_its_long_temporaries_out_of_the_frame() {
    let ir = include_str!("kernels/ir/c_short_3.ll");
    let text = text(ir, "c_short_3", 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
    let (stores, loads) = slot_traffic(&text);
    assert!(stores <= 40 && loads <= 60, "{stores} slot stores (<= 40), {loads} slot loads (<= 60)");
}
