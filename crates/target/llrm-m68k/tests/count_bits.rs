//! `llvm.ctlz` and `llvm.cttz` on i32.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
target triple = \"m68k-unknown-linux-gnu\"
declare i32 @llvm.ctlz.i32(i32, i1)
declare i32 @llvm.cttz.i32(i32, i1)
define i32 @lz(i32 %x) { %r = call i32 @llvm.ctlz.i32(i32 %x, i1 false) ret i32 %r }
define i32 @lz_nz(i32 %x) { %r = call i32 @llvm.ctlz.i32(i32 %x, i1 true) ret i32 %r }
define i32 @tz(i32 %x) { %r = call i32 @llvm.cttz.i32(i32 %x, i1 false) ret i32 %r }
define i32 @tz_nz(i32 %x) { %r = call i32 @llvm.cttz.i32(i32 %x, i1 true) ret i32 %r }";

/// M68k has no count instruction, so the pin's legalizer expands them
/// (`expandCTLZ` L10926, `expandCTTZ` L11041: smear and `ctpop`). They stopped
/// at `Unsupported { op: CtLzZeroPoison, what: "convert node to libcall" }`:
/// compiler-rt's addsf3, mulsf3, divsf3 and floatsisf did not compile.
#[test]
fn leading_and_trailing_zero_counts_expand_without_a_libcall() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        for level in [Level::O2, Level::Os] {
            let text = assembly(IR, cpu, level).unwrap_or_else(|why| panic!("{cpu:?} {level:?}: {why}"));
            assert!(!text.contains("jsr"), "{cpu:?} {level:?}: no call:\n{text}");
        }
    }
}
