//! The high word of a 64x64 product from four calls (`hh + (lh >> 32) + (hl >>
//! 32) + (mid >> 32)`): the shared `MachineCSE` took an `addx` of two equal
//! operands for the same value as an earlier one, though each reads another
//! `add`'s carry. Three `addx` where five are needed: 125 of 400 products wrong
//! at 68000 -O2.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const IR: &str = include_str!("fixtures/mul64_hi_carries.ll");

#[test]
fn an_addx_of_equal_operands_reading_another_carry_is_not_shared() {
    let module = llrm_mir::parse::module(IR).expect("parses");
    let out = text_with(&module, "f", Cpu::M68000, Options { mid_end: false, ..Options::default() }).expect("compiles");
    let addx = out.lines().filter(|line| line.trim_start().starts_with("addx")).count();
    assert!(addx >= 5, "{addx} addx, five carries to add:\n{out}");
}
