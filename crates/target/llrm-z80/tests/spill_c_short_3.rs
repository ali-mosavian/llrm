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

/// PARKED. c_short_3 at Os is 665 B against SDCC's 646 B (release/next
/// c98e231ab1, `~/scratch/z80-cause-card/count.py` on the sdasz80 listing):
/// 45 slot stores and 57 loads (308 B). The allocator is not the lever: of 31
/// spills 23 cross a call and 2 are pressure; `stack_slots` refuses most slots
/// the push/pop form (`LLRM_Z80_STACK_WHY`: LoadNotOnTop). The bar is bytes
/// <= 646, which needs the assembler; the bounds below are a unit-test proxy
/// (stores <= 40 fails today, loads <= 60 holds). Remove `ignore` when
/// target-z80's slot form lands.
#[test]
#[ignore = "parked: z80-spill-analysis-card-2026-11-10, slot form (stack_slots)"]
fn c_short_3_keeps_its_long_temporaries_out_of_the_frame() {
    let ir = include_str!("kernels/ir/c_short_3.ll");
    // premise: the module is the Z80's, so a failure below is the count and not
    // a refused module
    let layout = include_str!("../src/machines/datalayout.toml");
    let declared = ir
        .lines()
        .find_map(|line| line.strip_prefix("target datalayout = \"")?.strip_suffix('"'))
        .expect("a datalayout line");
    assert!(
        layout.contains(declared),
        "the module's datalayout `{declared}` is not the Z80's (machines/datalayout.toml)"
    );
    let text = text(ir, "c_short_3", 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
    let (stores, loads) = slot_traffic(&text);
    assert!(stores <= 40 && loads <= 60, "{stores} slot stores (<= 40), {loads} slot loads (<= 60)");
}
