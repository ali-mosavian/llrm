//! `fneg`, `fabs` and `copysign` of `fp128` on the DAG path alone.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const LAYOUT: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn selects(body: &str) -> String {
    driver::assembly(&format!("{LAYOUT}{body}"), Platform::Linux, true).unwrap_or_else(|why| panic!("{why}"))
}

/// `FNEG` of `fp128` had no expansion (`CannotSelect("FNeg")`), so
/// `tests/fixtures/c/longdouble` fell back to the hand selector at O0. The pin
/// stores the value, flips the sign byte and loads it back (`ExpandFNEG`).
#[test]
fn an_fp128_negation_flips_the_sign_byte_in_memory() {
    let asm = selects("define fp128 @f(fp128 %x) {\n  %r = fneg fp128 %x\n  ret fp128 %r\n}\n");
    assert!(asm.contains("ldrb") && asm.contains("strb") && asm.contains("eor"), "{asm}");
}

#[test]
fn an_fp128_absolute_value_clears_the_sign_byte_in_memory() {
    let asm = selects(
        "declare fp128 @llvm.fabs.f128(fp128)\ndefine fp128 @f(fp128 %x) {\n  %r = call fp128 @llvm.fabs.f128(fp128 %x)\n  ret fp128 %r\n}\n",
    );
    assert!(asm.contains("ldrb") && asm.contains("strb"), "{asm}");
}

#[test]
fn an_fp128_copysign_moves_the_sign_bit_of_the_second_operand() {
    let asm = selects(
        "declare fp128 @llvm.copysign.f128(fp128, fp128)\ndefine fp128 @f(fp128 %x, fp128 %y) {\n  %r = call fp128 @llvm.copysign.f128(fp128 %x, fp128 %y)\n  ret fp128 %r\n}\n",
    );
    assert!(asm.contains("ldrb") && asm.contains("strb"), "{asm}");
}
