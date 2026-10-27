use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use super::visit_icmp;
use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visitICmpInst` alone, through the driver.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if matches!(c.function.instruction(inst).opcode, Opcode::ICmp(_)) { visit_icmp(c, inst) } else { Visit::None }
}

/// `@f` after `visitICmpInst` alone.
fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// `define i1 @f(PARAMS)` with `body` before and `after` after `visitICmpInst`.
fn check(
    params: &str,
    body: &str,
    after: &str,
) {
    let decl = "declare i16 @llvm.umax.i16(i16, i16)\n";
    let printed_decl = "declare i16 @llvm.umax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)\n\n";
    let text = format!("{decl}define i1 @f({params}) {{\nb0:\n{body}}}\n");
    let want = format!("{printed_decl}define i1 @f({params}) {{\nb0:\n{after}}}\n");
    assert_eq!(combined(&text), want);
}

/// `canonicalizeCmpWithConstant`: the non-strict predicates of a constant
/// become strict ones (`opt`: `icmp ult i16 %x, 6`).
#[test]
fn a_non_strict_compare_with_a_constant_is_strict() {
    check("i16 %x", "  %r = icmp ule i16 %x, 5\n  ret i1 %r\n", "  %0 = icmp ult i16 %x, 6\n  ret i1 %0\n");
    check("i16 %x", "  %r = icmp sge i16 %x, 3\n  ret i1 %r\n", "  %0 = icmp sgt i16 %x, 2\n  ret i1 %0\n");
}

/// The operands are ordered, the constant last (`opt`: `icmp ugt i16 %x, 5`).
#[test]
fn a_constant_goes_to_the_right() {
    check("i16 %x", "  %r = icmp ult i16 5, %x\n  ret i1 %r\n", "  %r = icmp ugt i16 %x, 5\n  ret i1 %r\n");
}

/// `foldICmpUsingKnownBits`: `A u> C` where max(A) - 1 == C is `A == C + 1`
/// (`opt`: `icmp eq i16 %a, 12`).
#[test]
fn a_compare_one_short_of_the_known_maximum_is_an_equality() {
    check(
        "i16 %x",
        "  %a = and i16 %x, 12\n  %r = icmp ugt i16 %a, 11\n  ret i1 %r\n",
        "  %a = and i16 %x, 12\n  %0 = icmp eq i16 %a, 12\n  ret i1 %0\n",
    );
    check(
        "i16 %x, i16 %y",
        "  %a = and i16 %x, 4\n  %b = or i16 %y, 4\n  %r = icmp ult i16 %a, %b\n  ret i1 %r\n",
        "  %a = and i16 %x, 4\n  %b = or i16 %y, 4\n  %0 = icmp ne i16 %a, %b\n  ret i1 %0\n",
    );
    // The single bit that may be set: `(x & 4) == 4` is `(x & 4) != 0`.
    check(
        "i16 %x",
        "  %a = and i16 %x, 4\n  %r = icmp eq i16 %a, 4\n  ret i1 %r\n",
        "  %a = and i16 %x, 4\n  %0 = icmp ne i16 %a, 0\n  ret i1 %0\n",
    );
    check(
        "i16 %x",
        "  %a = and i16 %x, 4\n  %r = icmp ne i16 %a, 4\n  ret i1 %r\n",
        "  %a = and i16 %x, 4\n  %0 = icmp eq i16 %a, 0\n  ret i1 %0\n",
    );
}

/// A signed compare of two values of one known sign is unsigned (`opt` adds
/// `samesign`, which MIR's `icmp` has no flag for).
#[test]
fn a_signed_compare_of_non_negative_values_is_unsigned() {
    check(
        "i16 %x, i16 %y",
        "  %a = and i16 %x, 255\n  %b = and i16 %y, 255\n  %r = icmp slt i16 %a, %b\n  ret i1 %r\n",
        "  %a = and i16 %x, 255\n  %b = and i16 %y, 255\n  %r = icmp ult i16 %a, %b\n  ret i1 %r\n",
    );
}

/// `foldICmpWithZero`: `(x | 1) * y == 0` is `y == 0` (`opt`: `icmp eq i16 %y,
/// 0`).
#[test]
fn a_product_with_an_odd_factor_is_zero_when_the_other_is() {
    check(
        "i16 %x, i16 %y",
        "  %o = or i16 %x, 1\n  %m = mul i16 %o, %y\n  %r = icmp eq i16 %m, 0\n  ret i1 %r\n",
        "  %0 = icmp eq i16 %y, 0\n  ret i1 %0\n",
    );
}

/// `umax(x, 5) u> ~x` is `x s< 0`, and the compares of the sign bit
/// (`opt`: `icmp slt i16 %x, 0`, `icmp sgt i16 %x, -1`).
#[test]
fn compares_that_are_sign_tests() {
    check(
        "i16 %x",
        "  %m = call i16 @llvm.umax.i16(i16 %x, i16 5)\n  %n = xor i16 %x, -1\n  %r = icmp ugt i16 %m, %n\n  ret i1 %r\n",
        "  %m = call i16 @llvm.umax.i16(i16 %x, i16 5)\n  %0 = icmp slt i16 %x, 0\n  ret i1 %0\n",
    );
    check("i16 %x", "  %r = icmp ugt i16 %x, 32767\n  ret i1 %r\n", "  %0 = icmp slt i16 %x, 0\n  ret i1 %0\n");
    check("i16 %x", "  %r = icmp ult i16 %x, 32768\n  ret i1 %r\n", "  %0 = icmp sgt i16 %x, -1\n  ret i1 %0\n");
}

/// `foldICmpWithCastOp`'s constant arm; each shape pinned by
/// `opt -passes=instcombine`: a compare of an extension against a constant the
/// narrow type holds is the compare in the narrow type (the strlen loop's
/// `icmp ne (sext i8 c), 0` was not, and the idiom pass never saw a byte test).
#[test]
fn test_a_compare_of_an_extension_against_a_constant_is_the_narrow_compare() {
    let narrowed = |extension: &str, pred: &str, constant: i32, wanted: &str| {
        let text = format!(
            "define i1 @f(i8 %x) {{\nb0:\n  %e = {extension} i8 %x to i32\n  %r = icmp {pred} i32 %e, {constant}\n  ret i1 %r\n}}\n"
        );
        let out = combined(&text);
        assert!(out.contains(wanted) && !out.contains("i32"), "{wanted}\n{out}");
    };
    narrowed("zext", "ne", 0, "icmp ne i8 %x, 0");
    narrowed("sext", "eq", 0, "icmp eq i8 %x, 0");
    narrowed("sext", "slt", 5, "icmp slt i8 %x, 5");
    narrowed("zext", "ult", 5, "icmp ult i8 %x, 5");
    // The zext is non-negative: a signed compare is the unsigned one.
    narrowed("zext", "slt", 5, "icmp ult i8 %x, 5");
    // A negative sext is huge when read unsigned.
    narrowed("sext", "ult", 5, "icmp ult i8 %x, 5");
}

/// A constant the narrow type does not hold stays a wide compare here (the pin
/// decides it by range).
#[test]
fn test_a_constant_outside_the_narrow_type_is_not_narrowed() {
    let out =
        combined("define i1 @f(i8 %x) {\nb0:\n  %e = zext i8 %x to i32\n  %r = icmp ult i32 %e, 300\n  ret i1 %r\n}\n");
    assert!(!out.contains("icmp ult i8"), "{out}");
}

/// `foldICmpSubConstant` and `foldICmpBinOpEqualityWithConstant` were not translated, so the
/// crc step `(0 - (x & 1)) == 0` stayed a `sub` and a compare: bench/crc came to 288 bytes against
/// the pin's 188.
#[test]
fn test_an_equality_of_a_binary_operator_with_a_constant_is_a_compare_of_its_operand() {
    let folds = |op: &str, pred: &str, constant: u32, wanted: &str| {
        let text = format!(
            "define i1 @f(i16 %x, i16 %y) {{\nb0:\n  %s = {op}\n  %r = icmp {pred} i16 %s, {constant}\n  ret i1 %r\n}}\n"
        );
        let out = combined(&text);
        assert!(out.contains(wanted) && !out.contains("%s ="), "{wanted}\n{out}");
    };
    folds("sub nsw i16 0, %x", "eq", 0, "icmp eq i16 %x, 0");
    folds("sub i16 10, %x", "eq", 3, "icmp eq i16 %x, 7");
    folds("add i16 %x, 5", "eq", 8, "icmp eq i16 %x, 3");
    folds("xor i16 %x, 5", "ne", 3, "icmp ne i16 %x, 6");
    folds("sub i16 %x, %y", "ne", 0, "icmp ne i16 %x, %y");
    folds("udiv i16 %x, %y", "eq", 0, "icmp ugt i16 %y, %x");
}
