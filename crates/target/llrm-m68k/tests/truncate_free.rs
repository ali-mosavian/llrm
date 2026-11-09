//! crc16 -O1 tested the polynomial bit with `move.l %d0,%d1; and.l #32768,%d1; beq` (22 clk, 8 B) where
//! gcc's `tst.w %d0; jpl` is 4 clk, 2 B: 4972 clk against 4200 at 68000. The pin's `foldSetCCWithAnd` turns
//! `(x & 32768) != 0` into `(trunc x to i16) < 0` only where the truncate is free, and m68k says nothing:
//! the low word of a data register is the word operand, so the truncate is free.

use llrm_codegen::backend::selectiondag::dag_combiner::CombineHooks;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::lowering::M68kLowering;
use llrm_m68k::dagisel::hooks::Cpu;

const SIGN_BIT_OF_A_WORD: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @k(i32 %x) {
entry:
  %m = and i32 %x, 32768
  %c = icmp ne i32 %m, 0
  br i1 %c, label %a, label %b
a:
  ret i32 1
b:
  ret i32 2
}";

fn asm(
    options: Options,
    cpu: Cpu,
) -> String {
    let module = llrm_mir::parse::module(SIGN_BIT_OF_A_WORD).expect("parses");
    text_with(&module, "k", cpu, options).expect("compiles")
}

#[test]
fn the_pin_keeps_the_mask() {
    let out = asm(Options::PINNED, Cpu::M68000);
    assert!(out.contains("and.l\t#32768") || out.contains("btst"), "premise: the pin masks:\n{out}");
}

#[test]
fn a_word_sign_test_is_tst_w() {
    for cpu in [Cpu::M68000, Cpu::M68020, Cpu::M68040, Cpu::M68060] {
        let out = asm(Options::default(), cpu);
        assert!(out.contains("tst.w"), "{cpu:?}:\n{out}");
        assert!(!out.contains("32768"), "{cpu:?}:\n{out}");
    }
}

/// Only a narrowing to a byte or a word has an operand: an `i1` or a long is not free.
#[test]
fn only_a_narrowing_to_a_byte_or_a_word_is_free() {
    let hooks = M68kLowering { short: true, ..M68kLowering::default() };
    for (from, to, free) in [
        (EVT::I32, EVT::I16, true),
        (EVT::I32, EVT::I8, true),
        (EVT::I64, EVT::I16, true),
        (EVT::I32, EVT::I1, false),
        (EVT::I64, EVT::I32, false),
        (EVT::I16, EVT::I16, false),
    ] {
        assert_eq!(hooks.is_truncate_free(from, to), free, "{from:?} -> {to:?}");
    }
    assert!(!M68kLowering::default().is_truncate_free(EVT::I32, EVT::I16), "the pin leaves it false");
}
