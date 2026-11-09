//! `llvm.abs` of a type narrower than 32 bits, on the DAG path alone.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const LAYOUT: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

/// The type legalizer had no promotion for `ABS` ("Unsupported Abs i16 promote result"), so `__divhi3` and
/// `__modhi3` (runtime/compiler-rt/lib/int/int16.c) fell back to the hand selector at -O1 and above. The pin
/// (`PromoteIntRes_ABS`) takes the absolute value of the sign-extended operand.
#[test]
fn abs_of_i16_and_i8_is_promoted_on_the_dag_path() {
    for bits in [8, 16] {
        let ir = format!(
            "{LAYOUT}declare i{bits} @llvm.abs.i{bits}(i{bits}, i1)\n\
define i{bits} @f(i{bits} %x) {{\n  %a = call i{bits} @llvm.abs.i{bits}(i{bits} %x, i1 false)\n  ret i{bits} %a\n}}\n"
        );
        let asm = driver::assembly(&ir, Platform::Linux, true).unwrap_or_else(|why| panic!("i{bits}: {why}"));
        assert!(asm.contains("sbfm") && asm.contains("csneg"), "i{bits}: {asm}");
    }
}
