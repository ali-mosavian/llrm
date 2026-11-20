//! `x << 16` and `x & 0xFFFF0000` clear the low word of a register that was
//! swapped or masked in place: `clr.w %d0` is 2 B and 4 clk on the 68000,
//! `and.w #0, %d0` was 4 B and 8 clk.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const LAYOUT: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n";

fn asm(
    body: &str,
    cpu: &str,
    level: Level,
) -> String {
    assembly(&format!("{LAYOUT}define i32 @f(i32 %x) {{\n{body}\n}}"), cpu_of(cpu).unwrap(), level).expect("compiles")
}

#[test]
fn a_shift_by_sixteen_clears_the_low_word_with_clr_w() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O1, Level::Os] {
            let out = asm("  %r = shl i32 %x, 16\n  ret i32 %r", cpu, level);
            assert!(out.contains("swap") && out.contains("clr.w"), "{cpu} {level:?}:\n{out}");
            assert!(!out.contains("and.w\t#0") && !out.contains("andi.w"), "{cpu} {level:?}:\n{out}");
        }
    }
}
