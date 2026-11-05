//! `x & 1` of a copy (bench/kernels/popcount) was `move.l %d1, %d2; and.l #1,
//! %d2`, 8 B and 20 clocks at the 68000, against gcc's `moveq #1, %d2; and.l
//! %d1, %d2`, 4 B and 12.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const POPCOUNT: &str = include_str!("../bench/kernels/popcount.ll");

fn text(options: Options) -> String {
    let module = llrm_mir::parse::module(POPCOUNT).expect("parses");
    text_with(&module, "k", Cpu::M68000, options).expect("compiles")
}

#[test]
fn a_small_mask_of_a_copy_is_built_in_the_destination() {
    let out = text(Options { size: true, ..Options::default() });
    assert!(!out.contains("and.l\t#1,"), "the immediate form:\n{out}");
    assert!(out.contains("moveq\t#1,") && out.contains("and.l\t%d"), "no moveq and register and:\n{out}");
}

#[test]
fn the_pinned_build_keeps_the_immediate_form() {
    assert!(text(Options::PINNED).contains("and.l\t#1,"));
}
