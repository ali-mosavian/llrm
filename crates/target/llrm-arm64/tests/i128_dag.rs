//! `i128` on the DAG path alone: every operation the type legalizer splits into
//! `i64` halves.
//!
//! Each of these was refused ("custom lowering of
//! Load/Store/ShlParts/SrlParts/UAddOCarry/SetCCCarry", "expand a
//! division without a target") and the function fell back to the hand selector:
//! 20 compiler-rt programs, `int128`, `gen_div64`, `host_int128` and
//! `gen_vectors64_edge` at the levels they build at.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const LAYOUT: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn selects(body: &str) -> String {
    driver::assembly(&format!("{LAYOUT}{body}"), Platform::Linux, true).unwrap_or_else(|why| panic!("{why}"))
}

#[test]
fn an_i128_load_and_store_are_two_i64_accesses() {
    let asm =
        selects("define void @f(ptr %p, ptr %q) {\n  %v = load i128, ptr %p\n  store i128 %v, ptr %q\n  ret void\n}\n");
    assert!(asm.contains("ldp") && asm.contains("stp"), "{asm}");
}

#[test]
fn i128_shifts_are_expanded_into_i64_parts() {
    for op in ["shl", "lshr", "ashr"] {
        selects(&format!("define i128 @f(i128 %x, i128 %n) {{\n  %r = {op} i128 %x, %n\n  ret i128 %r\n}}\n"));
    }
}

/// `tryShiftAmountMod` ran inside the last DAG combine, so the known-bits fold
/// saw `(~zext n) & 63` as `~zext n`, a shift by 2^32 or more, and made the
/// funnel term of the high half 0: `(u128)v << n` for an unsigned `n` lost the
/// low half's top bits (n = 7 of `0x0123456789abcdeffedcba9876543210` gave
/// `...f780`, not `...f7ff`, under qemu). The pin does it when it selects the
/// shift, after every combine.
#[test]
fn an_i128_shift_by_a_zero_extended_amount_keeps_the_bits_carried_from_the_low_half() {
    let asm = selects(
        "define i128 @f(i128 %x, i32 %n) {\n  %z = zext i32 %n to i128\n  %r = shl i128 %x, %z\n  ret i128 %r\n}\n",
    );
    assert!(asm.contains("lsr") || asm.contains("ubfm"), "no low-half bits are shifted into the high half:\n{asm}");
}
#[test]
fn i128_add_sub_and_compare_carry_between_the_halves() {
    let asm = selects(
        "define i128 @add(i128 %a, i128 %b) {\n  %r = add i128 %a, %b\n  ret i128 %r\n}\n\
define i128 @sub(i128 %a, i128 %b) {\n  %r = sub i128 %a, %b\n  ret i128 %r\n}\n\
define i1 @lt(i128 %a, i128 %b) {\n  %r = icmp slt i128 %a, %b\n  ret i1 %r\n}\n",
    );
    assert!(asm.contains("adcs") || asm.contains("adc "), "{asm}");
    assert!(asm.contains("sbcs") || asm.contains("sbc "), "{asm}");
}

#[test]
fn an_i128_division_is_a_call_of_the_runtime() {
    let asm = selects("define i128 @f(i128 %a, i128 %b) {\n  %r = udiv i128 %a, %b\n  ret i128 %r\n}\n");
    assert!(asm.contains("__udivti3"), "{asm}");
}

/// The carry out of an `i128` add was the constant false (see the m64 test of
/// the same name in llrm-codegen): the overflow of
/// `llvm.uadd.with.overflow.i128` compiled to `mov w0, #0`.
#[test]
fn a_128_bit_unsigned_add_keeps_its_carry_out() {
    let asm = selects(
        "declare {i128, i1} @llvm.uadd.with.overflow.i128(i128, i128)\n\
define i1 @f(i128 %a, i128 %b) {\n  %r = call {i128, i1} @llvm.uadd.with.overflow.i128(i128 %a, i128 %b)\n  %o = extractvalue {i128, i1} %r, 1\n  ret i1 %o\n}\n",
    );
    assert!(asm.contains("adcs") && asm.matches("csinc").count() == 2, "{asm}");
}

/// A `select` on an `i128` compare was refused ("Unsupported SelectCC i128:
/// expand operand"): the pin expands the compare to a boolean and selects on it
/// (`ExpandIntOp_SELECT_CC`), so the min and max of two wide values did not
/// build (f128 compare and conversion routines of compiler-rt).
#[test]
fn a_select_on_an_i128_compare_is_expanded_to_a_boolean() {
    for code in ["ult", "slt", "eq", "sge"] {
        selects(&format!(
            "define i64 @f(i128 %a, i128 %b, i64 %x, i64 %y) {{\n  %c = icmp {code} i128 %a, %b\n  %r = select i1 %c, i64 %x, i64 %y\n  ret i64 %r\n}}\n"
        ));
    }
}
