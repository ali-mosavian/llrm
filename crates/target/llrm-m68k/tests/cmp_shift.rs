//! hex -Os ended its digit loop with `cmpi.l #15,%d1; bhi` (6 B) beside the
//! `lsr.l #4,%d2` that made the next value; the shift's own codes say the same
//! (`v > 15` is `v >> 4 != 0`), as gcc's `lsr.l #4,%d0; jne` does.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const HEX: &str = include_str!("../bench/library/hex.ll");

/// The compare of a value with 15 whose shift by 4 is not formed in the block:
/// nothing to reuse, the compare stays.
const NO_SHIFT: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @k(i32 %v) {
entry:
  %c = icmp ugt i32 %v, 15
  br i1 %c, label %a, label %b
a:
  ret i32 1
b:
  ret i32 2
}";

fn asm(
    ir: &str,
    cpu: Cpu,
    reuse: bool,
) -> String {
    let module = llrm_mir::parse::module(ir).expect("parses");
    text_with(&module, "k", cpu, Options { reuse_shift_flags: reuse, size: true, ..Options::default() }).expect("compiles")
}

#[test]
fn the_shift_codes_end_the_digit_loop() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let base = asm(HEX, cpu, false);
        assert!(base.contains("cmpi.l\t#15,"), "premise: hex keeps the compare without the pass:\n{base}");
        let out = asm(HEX, cpu, true);
        assert!(!out.contains("cmpi.l\t#15,"), "{cpu:?}:\n{out}");
    }
}

#[test]
fn a_compare_without_the_shift_stays() {
    let out = asm(NO_SHIFT, Cpu::M68000, true);
    // the mid-end writes `v > 15` as `v < 16`
    assert!(out.contains("cmpi.l\t#16,"), "{out}");
}
