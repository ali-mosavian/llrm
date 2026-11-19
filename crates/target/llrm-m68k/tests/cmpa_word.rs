//! `cmpa.l #2, %a1` is 6 B and 14 clk on the 68000; `cmpa.w #2, %a1` compares
//! the same 32 bits (the word is sign-extended) in 4 B and 10 clk, with no
//! data register to hold the constant. A compare of a value held in an
//! address register against a small constant (sibling_call_callees -O1: +4 B)
//! costs that.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare void @a(i32)
define void @f(ptr %p) {
  %v = load i32, ptr %p
  call void @a(i32 %v)
  %is2 = icmp eq ptr %p, inttoptr (i32 2 to ptr)
  br i1 %is2, label %two, label %rest
two:
  call void @a(i32 2)
  ret void
rest:
  %is1 = icmp eq ptr %p, inttoptr (i32 1 to ptr)
  br i1 %is1, label %one, label %out
one:
  call void @a(i32 1)
  ret void
out:
  ret void
}
";

#[test]
fn a_pointer_is_compared_with_a_small_constant_by_a_word_immediate() {
    let module = llrm_mir::parse::module(IR).expect("parses");
    let options = Options { size: true, ..Options::default() };
    let text = text_with(&module, "f", Cpu::M68000, options).expect("compiles");
    assert!(!text.contains("cmpa.l\t#"), "{text}");
    assert!(text.contains("cmpa.w\t#2"), "{text}");
    assert!(text.contains("cmpa.w\t#1"), "{text}");
}

#[test]
fn a_word_immediate_compare_is_4_bytes_and_10_clocks_on_the_68000() {
    let cost = llrm_m68k::timing::cost(Cpu::M68000, "cmpa.w #2, %a1", 4, false).expect("priced");
    assert_eq!(cost.clocks, 10);
}
