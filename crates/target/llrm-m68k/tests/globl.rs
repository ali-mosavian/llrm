//! Linkage of a function in the assembly.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
target triple = \"m68k-unknown-linux-gnu\"
define i32 @pub(i32 %x) { %r = add i32 %x, 1 ret i32 %r }
define internal i32 @priv(i32 %x) { %r = add i32 %x, 2 ret i32 %r }
define weak i32 @wk(i32 %x) { %r = add i32 %x, 3 ret i32 %r }";

/// `llrm-cc -S` left every function label local (no `.globl`), so `nm` listed
/// no export of a softfloat routine and an object assembled from the text
/// could not be linked against by name. The pin emits `.globl` for an
/// external function and `.weak` for a weak one.
#[test]
fn external_functions_are_exported_internal_ones_are_not() {
    let text = assembly(IR, Cpu::M68000, Level::O2).expect("compiles");
    assert!(text.contains(".globl\tpub\n"), "pub is global:\n{text}");
    assert!(text.contains(".weak\twk\n"), "wk is weak:\n{text}");
    assert!(!text.contains(".globl\tpriv"), "priv is local:\n{text}");
}
