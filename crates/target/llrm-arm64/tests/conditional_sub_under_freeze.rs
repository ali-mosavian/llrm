//! `x - select(x < q, 0, q)` under a frozen shift: arm64 keeps `subs; csel`,
//! the pin's `performConditionalSubCombine`.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const IR: &str =
    "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"
define i32 @f(i32 %a, i32 %q) {
  %c = icmp ult i32 %a, %q
  %s = select i1 %c, i32 0, i32 %q
  %d = sub nuw i32 %a, %s
  %sh = shl i32 %d, 8
  %fr = freeze i32 %sh
  %fq = freeze i32 %q
  %r = udiv i32 %fr, %fq
  ret i32 %r
}
";

/// With the freeze pushed through `shl` (as the pin does) the freeze landed on
/// the `select` (ours did not know a select of guaranteed operands is
/// guaranteed), so `sub x, freeze(csel ..)` missed the combine: `subs wzr;
/// csel; sub` (3 instructions, +4 B per use in `__divsf3`, 8 sfm16_float
/// programs).
#[test]
fn a_conditional_subtraction_keeps_its_subs_and_csel_under_a_frozen_shift() {
    let asm = driver::assembly(IR, Platform::Linux, true).expect("compiles");
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    assert!(lines.iter().any(|line| line.starts_with("subs w") && !line.starts_with("subs wzr")), "{asm}");
    assert!(!lines.iter().any(|line| line.starts_with("sub w")), "{asm}");
}
