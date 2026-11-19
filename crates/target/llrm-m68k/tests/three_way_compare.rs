//! `llvm.scmp` and `llvm.ucmp` (`(a > b) - (a < b)`, which clang makes of
//! `(a > b) - (a < b)` in C) reached selection and were refused by name:
//! `CannotSelect("SCmp")` in the softfloat `__cmpsf2`.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

const SOURCE: &str = "
declare i32 @llvm.scmp.i32.i32(i32, i32)
declare i32 @llvm.ucmp.i32.i32(i32, i32)
define i32 @s(i32 %a, i32 %b) {
  %r = call i32 @llvm.scmp.i32.i32(i32 %a, i32 %b)
  ret i32 %r
}
define i32 @u(i32 %a, i32 %b) {
  %r = call i32 @llvm.ucmp.i32.i32(i32 %a, i32 %b)
  ret i32 %r
}";

#[test]
fn a_three_way_compare_compiles() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let text = assembly(&format!("target datalayout = \"{DATALAYOUT}\"\n{SOURCE}"), cpu, Level::O2)
            .unwrap_or_else(|why| panic!("{cpu:?}: {why}"));
        assert!(text.contains("s:") && text.contains("u:"), "{cpu:?}:\n{text}");
    }
}
