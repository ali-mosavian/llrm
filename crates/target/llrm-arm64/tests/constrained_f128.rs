//! Constrained `fp128` operations on arm64: each is a routine called on the
//! node's own chain, so it stays between the calls that change the rounding
//! mode. Before, `llrm-arm64-llc` refused every constrained operation
//! (QuickrBasic's `pfpu.c`, `FENV_ACCESS ON`, fell back to the pin's clang).

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const LAYOUT: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

/// `fesetround(0)`, the operation, `fesetround(1)`: the calls of `asm`, in
/// order.
fn calls(
    ret: &str,
    params: &str,
    body: &str,
    result: &str,
) -> Vec<String> {
    let ir = format!(
        "{LAYOUT}declare i32 @fesetround(i32)\n\
         declare fp128 @llvm.experimental.constrained.fadd.f128(fp128, fp128, metadata, metadata)\n\
         declare fp128 @llvm.experimental.constrained.rint.f128(fp128, metadata, metadata)\n\
         declare fp128 @llvm.experimental.constrained.fpext.f128.f64(double, metadata)\n\
         declare double @llvm.experimental.constrained.fptrunc.f64.f128(fp128, metadata, metadata)\n\
         declare fp128 @llvm.experimental.constrained.sitofp.f128.i32(i32, metadata, metadata)\n\
         declare i32 @llvm.experimental.constrained.fptosi.i32.f128(fp128, metadata)\n\
         declare i1 @llvm.experimental.constrained.fcmps.f128(fp128, fp128, metadata, metadata)\n\
         define {ret} @f({params}) strictfp {{\n  %a = call i32 @fesetround(i32 0) strictfp\n{body}\n  %b = call i32 @fesetround(i32 1) strictfp\n  ret {ret} {result}\n}}\n"
    );
    let asm = driver::assembly(&ir, Platform::Linux, true).unwrap_or_else(|why| panic!("{why}"));
    asm.lines().filter_map(|line| line.trim().strip_prefix("bl ").map(|callee| callee.trim().to_owned())).collect()
}

fn between(
    ret: &str,
    params: &str,
    body: &str,
    result: &str,
    routines: &[&str],
) {
    let mut want = vec!["fesetround"];
    want.extend(routines);
    want.push("fesetround");
    assert_eq!(calls(ret, params, body, result), want);
}

const ROUND: &str = "metadata !\"round.dynamic\", metadata !\"fpexcept.strict\"";

#[test]
fn a_constrained_fp128_add_is_called_between_the_rounding_mode_calls() {
    between(
        "fp128",
        "fp128 %x, fp128 %y",
        &format!("  %r = call fp128 @llvm.experimental.constrained.fadd.f128(fp128 %x, fp128 %y, {ROUND}) strictfp"),
        "%r",
        &["__addtf3"],
    );
}

#[test]
fn a_constrained_fp128_rint_calls_rintl() {
    between(
        "fp128",
        "fp128 %x",
        &format!("  %r = call fp128 @llvm.experimental.constrained.rint.f128(fp128 %x, {ROUND}) strictfp"),
        "%r",
        &["rintl"],
    );
}

#[test]
fn constrained_conversions_to_and_from_fp128_are_their_routines_on_the_chain() {
    between(
        "fp128",
        "double %x",
        "  %r = call fp128 @llvm.experimental.constrained.fpext.f128.f64(double %x, metadata !\"fpexcept.strict\") strictfp",
        "%r",
        &["__extenddftf2"],
    );
    between(
        "double",
        "fp128 %x",
        &format!("  %r = call double @llvm.experimental.constrained.fptrunc.f64.f128(fp128 %x, {ROUND}) strictfp"),
        "%r",
        &["__trunctfdf2"],
    );
    between(
        "fp128",
        "i32 %x",
        &format!("  %r = call fp128 @llvm.experimental.constrained.sitofp.f128.i32(i32 %x, {ROUND}) strictfp"),
        "%r",
        &["__floatsitf"],
    );
    between(
        "i32",
        "fp128 %x",
        "  %r = call i32 @llvm.experimental.constrained.fptosi.i32.f128(fp128 %x, metadata !\"fpexcept.strict\") strictfp",
        "%r",
        &["__fixtfsi"],
    );
}

#[test]
fn a_constrained_fp128_compare_is_called_between_the_rounding_mode_calls() {
    between(
        "i1",
        "fp128 %x, fp128 %y",
        "  %r = call i1 @llvm.experimental.constrained.fcmps.f128(fp128 %x, fp128 %y, metadata !\"olt\", metadata !\"fpexcept.strict\") strictfp",
        "%r",
        &["__lttf2"],
    );
}

/// `FRINT` of `fp128` was `CannotSelect`; the pin expands it to `rintl`.
#[test]
fn an_fp128_rint_calls_rintl() {
    let ir = format!(
        "{LAYOUT}declare fp128 @llvm.rint.f128(fp128)\ndefine fp128 @f(fp128 %x) {{\n  %r = call fp128 @llvm.rint.f128(fp128 %x)\n  ret fp128 %r\n}}\n"
    );
    let asm = driver::assembly(&ir, Platform::Linux, true).unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("bl rintl"), "{asm}");
}

/// A strict routine's out chain, not the entry node, orders the next strict
/// operation: `add`, a mode change, `mul`, a mode change keep their order.
#[test]
fn a_strict_routines_out_chain_orders_the_next_strict_operation() {
    let ir = format!(
        "{LAYOUT}declare i32 @fesetround(i32)\n\
         declare fp128 @llvm.experimental.constrained.fadd.f128(fp128, fp128, metadata, metadata)\n\
         declare fp128 @llvm.experimental.constrained.fmul.f128(fp128, fp128, metadata, metadata)\n\
         define fp128 @f(fp128 %x, fp128 %y) strictfp {{\n\
           %s = call fp128 @llvm.experimental.constrained.fadd.f128(fp128 %x, fp128 %y, {ROUND}) strictfp\n\
           %a = call i32 @fesetround(i32 1) strictfp\n\
           %p = call fp128 @llvm.experimental.constrained.fmul.f128(fp128 %s, fp128 %y, {ROUND}) strictfp\n\
           %b = call i32 @fesetround(i32 0) strictfp\n\
           ret fp128 %p\n}}\n"
    );
    let asm = driver::assembly(&ir, Platform::Linux, true).unwrap_or_else(|why| panic!("{why}"));
    let order: Vec<&str> = asm.lines().filter_map(|line| line.trim().strip_prefix("bl ")).collect();
    assert_eq!(order, ["__addtf3", "fesetround", "__multf3", "fesetround"]);
}
