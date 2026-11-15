//! gcc folds identical functions at -Os (`-fipa-icf`: `.set shl2,shl`); the pin's pipeline has no `MergeFunctions`.
//! shift_int32_variable -Os kept `shl` and `shl2`, `shr` and `shr2`, `sar` and `sar2` as six bodies: 258 B against
//! gcc's 224. DEVIATE MERGEFUNC-OS.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const TWINS: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare void @report(i32)
define internal i32 @shl(i32 %x, i32 %n) noinline nounwind optsize minsize {
  %r = shl i32 %x, %n
  ret i32 %r
}
define internal i32 @shl2(i32 %x, i32 %n) noinline nounwind optsize minsize {
  %r = shl i32 %x, %n
  ret i32 %r
}
define i32 @main(i32 %a, i32 %b) nounwind optsize minsize {
  %p = call i32 @shl(i32 %a, i32 %b)
  call void @report(i32 %p)
  %q = call i32 @shl2(i32 %b, i32 %a)
  call void @report(i32 %q)
  ret i32 0
}";

#[test]
fn two_functions_of_one_body_are_one_at_os() {
    let asm = assembly(TWINS, cpu_of("68000").unwrap(), Level::Os).expect("compiles");
    let bodies = asm.lines().filter(|line| line.trim_start().starts_with("lsl.l")).count();
    assert_eq!(bodies, 1, "the twins stayed two bodies (+14 B each):\n{asm}");
}
