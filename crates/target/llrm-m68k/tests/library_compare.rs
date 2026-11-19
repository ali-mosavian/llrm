//! `memcmp(p, q, n) == 0` stayed a `memcmp` call on m68k: the target's C
//! library did not state `bcmp`, so `SimplifyLibCalls` never formed it.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{self, Level};

#[test]
fn a_memcmp_tested_for_equality_calls_bcmp() {
    let text = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare i32 @memcmp(ptr, ptr, i32)
define i1 @same(ptr %p, ptr %q, i32 %n) {
  %r = call i32 @memcmp(ptr %p, ptr %q, i32 %n)
  %c = icmp eq i32 %r, 0
  ret i1 %c
}
";
    let asm = driver::assembly(text, Cpu::M68000, Level::O2).expect("lowers");
    assert!(asm.contains("bcmp") && !asm.contains("memcmp"), "{asm}");
}
