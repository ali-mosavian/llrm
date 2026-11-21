//! LSR ranked a solution with fewer base adds before one with fewer registers
//! (Z80L3). histo's counting loop kept three induction variables, two of them
//! in the frame: the module priced 349 B / 4.20M clocks (ucsim 348 B / 3.54M T
//! against SDCC's 332 B / 3.89M T); the pin's order, registers first, 294 B /
//! 3.78M. The same order takes `sum4`'s `djnz` loop apart (11 B / 135 clocks
//! against 20 B / 291) and lz4's second `ldir`, with the same cost deltas, so
//! the driver prices both.

use llrm_z80::compile::{effective, module_priced, text};
use llrm_z80::driver::{Level, Request};

#[test]
fn histo_at_o1_takes_the_alternate_order_that_leaves_one_induction_variable() {
    let ir = include_str!("data/histo_o1.ll");
    let request = effective(ir, &Request::new(Level::O1)).expect("chooses");
    let (bytes, clocks) = module_priced(ir, &request).expect("prices");
    assert!(request.lsr_alternate, "-O1 {bytes} B / {clocks} clocks: the loop keeps three induction variables");
    assert!(bytes <= 310 && clocks <= 4_000_000, "-O1 {bytes} B / {clocks} clocks");
}

#[test]
fn sum4_at_o2_keeps_the_order_that_leaves_its_djnz() {
    let ir = include_str!("kernels/ir/sum4.ll");
    let request = effective(ir, &Request::new(Level::O2)).expect("chooses");
    let asm = text(ir, "sum4", 0, &request).expect("compiles");
    assert!(!request.lsr_alternate && asm.contains("djnz"), "{asm}");
}
