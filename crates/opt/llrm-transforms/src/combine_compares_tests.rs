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

/// A signed compare of two values of one known sign is unsigned, with
/// `samesign`.
#[test]
fn a_signed_compare_of_non_negative_values_is_unsigned() {
    check(
        "i16 %x, i16 %y",
        "  %a = and i16 %x, 255\n  %b = and i16 %y, 255\n  %r = icmp slt i16 %a, %b\n  ret i1 %r\n",
        "  %a = and i16 %x, 255\n  %b = and i16 %y, 255\n  %r = icmp samesign ult i16 %a, %b\n  ret i1 %r\n",
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

/// `for (short k = 0; k < n; k++) a[k]` reached indvars with
/// `icmp slt (sext i16 %k.next), (sext i16 %n)` kept at i32 beside an i32
/// counter of the same value: `foldICmpWithZextOrSext`'s extension-against-
/// extension arm was not translated, so the pin's `icmp slt i16` never came
/// and the i16 counter stayed (`movsx` a trip, d_faces 18,142 executed).
#[test]
fn a_signed_compare_of_two_sext_is_the_compare_of_their_sources() {
    check(
        "i16 %x, i16 %y",
        "  %a = sext i16 %x to i32\n  %b = sext i16 %y to i32\n  %r = icmp slt i32 %a, %b\n  ret i1 %r\n",
        "  %0 = icmp slt i16 %x, %y\n  ret i1 %0\n",
    );
}

/// Two `zext`: every predicate is the unsigned one (`icmp slt` of two
/// non-negative values).
#[test]
fn a_compare_of_two_zext_is_unsigned() {
    check(
        "i16 %x, i16 %y",
        "  %a = zext i16 %x to i32\n  %b = zext i16 %y to i32\n  %r = icmp slt i32 %a, %b\n  ret i1 %r\n",
        "  %0 = icmp ult i16 %x, %y\n  ret i1 %0\n",
    );
}

/// A `zext nneg` against a `sext` is two sign extensions; without `nneg` the
/// mismatch is left alone.
#[test]
fn a_zext_nneg_beside_a_sext_reads_as_a_sext() {
    check(
        "i16 %x, i16 %y",
        "  %a = zext nneg i16 %x to i32\n  %b = sext i16 %y to i32\n  %r = icmp slt i32 %a, %b\n  ret i1 %r\n",
        "  %0 = icmp slt i16 %x, %y\n  ret i1 %0\n",
    );
    check(
        "i16 %x, i16 %y",
        "  %a = zext i16 %x to i32\n  %b = sext i16 %y to i32\n  %r = icmp slt i32 %a, %b\n  ret i1 %r\n",
        "  %a = zext i16 %x to i32\n  %b = sext i16 %y to i32\n  %r = icmp slt i32 %a, %b\n  ret i1 %r\n",
    );
}

/// Two `i1`: `(zext X) == (sext Y)` is `(X | Y) == 0`.
#[test]
fn an_equality_of_a_zext_and_a_sext_of_bools_is_an_or() {
    check(
        "i1 %x, i1 %y",
        "  %a = zext i1 %x to i8\n  %b = sext i1 %y to i8\n  %r = icmp eq i8 %a, %b\n  ret i1 %r\n",
        "  %0 = or i1 %x, %y\n  %1 = xor i1 %0, true\n  ret i1 %1\n",
    );
}

/// Sources of different widths: the narrower is extended to the wider's type.
#[test]
fn a_compare_of_extensions_of_different_widths_extends_the_narrower_source() {
    check(
        "i16 %x, i8 %y",
        "  %a = sext i16 %x to i32\n  %b = sext i8 %y to i32\n  %r = icmp slt i32 %a, %b\n  ret i1 %r\n",
        "  %0 = sext i8 %y to i16\n  %1 = icmp slt i16 %x, %0\n  ret i1 %1\n",
    );
}

/// `foldICmpBinOp`, the offset-by-one flips (`opt`: `icmp sge i32 %s, %n`).
/// x_heapsort kept `or disjoint %s, 1` live across its sift loop for the guard.
#[test]
fn an_offset_by_one_compare_steps_its_strictness_instead() {
    check(
        "i32 %x, i32 %n",
        "  %s = shl nuw nsw i32 %x, 1\n  %o = or disjoint i32 %s, 1\n  %r = icmp sgt i32 %o, %n\n  ret i1 %r\n",
        "  %s = shl nuw nsw i32 %x, 1\n  %0 = icmp sge i32 %s, %n\n  ret i1 %0\n",
    );
    check(
        "i32 %x, i32 %n",
        "  %m = add nsw i32 %n, -1\n  %r = icmp sgt i32 %x, %m\n  ret i1 %r\n",
        "  %0 = icmp sge i32 %x, %n\n  ret i1 %0\n",
    );
}

/// `foldICmpBinOp`, the sums and differences that cancel an operand.
#[test]
fn a_sum_or_difference_compared_with_its_operand_drops_it() {
    check(
        "i32 %a, i32 %b",
        "  %s = sub i32 %a, %b\n  %r = icmp ugt i32 %s, %a\n  ret i1 %r\n",
        "  %0 = icmp ugt i32 %b, %a\n  ret i1 %0\n",
    );
    check(
        "i32 %a, i32 %x",
        "  %s = add i32 %a, %x\n  %r = icmp ult i32 %s, %a\n  ret i1 %r\n",
        "  %0 = xor i32 %a, -1\n  %1 = icmp ugt i32 %x, %0\n  ret i1 %1\n",
    );
    check(
        "i32 %a, i32 %x",
        "  %s = urem i32 %a, %x\n  %r = icmp eq i32 %s, %a\n  ret i1 %r\n",
        "  %0 = icmp ult i32 %a, %x\n  ret i1 %0\n",
    );
}

/// `foldICmpBinOp`, two offsets: the larger one stays on its side.
#[test]
fn two_offsets_are_compared_by_their_difference() {
    check(
        "i32 %a, i32 %c",
        "  %p = add i32 %a, 5\n  %q = add i32 %c, 3\n  %r = icmp eq i32 %p, %q\n  ret i1 %r\n",
        "  %0 = add i32 %a, 2\n  %1 = icmp eq i32 %0, %c\n  ret i1 %1\n",
    );
    check(
        "i32 %a, i32 %c",
        "  %p = add nsw i32 %a, 5\n  %q = add nsw i32 %c, 3\n  %r = icmp slt i32 %p, %q\n  ret i1 %r\n",
        "  %0 = add nsw i32 %a, 2\n  %1 = icmp slt i32 %0, %c\n  ret i1 %1\n",
    );
}

/// `foldSignBitTest`: the sign bit shifted down and compared with zero is a
/// compare of the value with zero (a `shr` by the width less one, a `trunc` of
/// it, or two shifts that add up to it).
#[test]
fn test_a_sign_bit_extraction_compared_with_zero_is_a_sign_test() {
    check(
        "i32 %x",
        "  %s = lshr i32 %x, 31\n  %r = icmp eq i32 %s, 0\n  ret i1 %r\n",
        "  %0 = icmp sgt i32 %x, -1\n  ret i1 %0\n",
    );
    check(
        "i32 %x",
        "  %s = ashr i32 %x, 31\n  %r = icmp ne i32 %s, 0\n  ret i1 %r\n",
        "  %0 = icmp slt i32 %x, 0\n  ret i1 %0\n",
    );
    check(
        "i32 %x",
        "  %s = lshr i32 %x, 31\n  %t = trunc i32 %s to i8\n  %r = icmp eq i8 %t, 0\n  ret i1 %r\n",
        "  %0 = icmp sgt i32 %x, -1\n  ret i1 %0\n",
    );
}

/// `foldGEPICmp`: a pointer compared with the GEP off it is the offset
/// compared with zero (`p != end` loops on `p + n`).
#[test]
fn test_a_gep_compared_with_its_base_is_the_offset_against_zero() {
    let text = "define i1 @f(ptr %p, i32 %n) {
b0:
  %g = getelementptr inbounds i32, ptr %p, i32 %n
  %r = icmp ne ptr %g, %p
  ret i1 %r
}
";
    let after = combined(text);
    assert!(!after.contains("getelementptr") && after.contains("icmp ne i32"), "{after}");
    let signed = combined(&text.replace("icmp ne", "icmp slt"));
    assert!(signed.contains("getelementptr"), "a signed compare is left alone: {signed}");
}

/// A compare of a phi of constants against a constant stayed a compare: the
/// phi's block ended in `br (icmp eq (phi [1, 1, 0]), 0)` that SimplifyCFG
/// cannot thread (its fold needs the phi itself as the condition), so queens
/// -O1 kept a materialised flag through its inner loop, +16% where the pin
/// exits straight to the two targets. `foldOpIntoPhi` makes it a phi of i1.
#[test]
fn a_compare_of_a_phi_of_constants_becomes_a_phi_of_the_answers() {
    let text = "define i32 @f(i1 %c, i1 %d) {
b0:
  br i1 %c, label %a, label %m

a:
  br i1 %d, label %m, label %b

b:
  br label %m

m:
  %p = phi i32 [ 1, %b0 ], [ 1, %a ], [ 0, %b ]
  %z = icmp eq i32 %p, 0
  br i1 %z, label %x, label %y

x:
  ret i32 5

y:
  ret i32 6
}
";
    let out = combined(text);
    assert!(out.contains("phi i1") && !out.contains("icmp"), "{out}");
}

/// `foldICmpXorXX`: `x ^ nonzero` is never `x`, so the non-strict compare of
/// the two is strict (`opt`: `icmp ugt i32 %a, %x`); a negative operand
/// decides the compare by the sign of `x`.
#[test]
fn a_xor_compared_with_its_operand_is_decided_by_the_other_operand() {
    check(
        "i32 %x, i32 %y0",
        "  %y = or i32 %y0, 1\n  %a = xor i32 %x, %y\n  %r = icmp uge i32 %a, %x\n  ret i1 %r\n",
        "  %y = or i32 %y0, 1\n  %a = xor i32 %x, %y\n  %0 = icmp ugt i32 %a, %x\n  ret i1 %0\n",
    );
    check(
        "i32 %x, i32 %y0",
        "  %y = or i32 %y0, 1\n  %a = xor i32 %x, %y\n  %r = icmp sle i32 %x, %a\n  ret i1 %r\n",
        "  %y = or i32 %y0, 1\n  %a = xor i32 %x, %y\n  %0 = icmp sgt i32 %a, %x\n  ret i1 %0\n",
    );
    // The pin goes on to `icmp slt i32 %x, 0` (`foldICmpBinOpWithConstant`,
    // OWED).
    check(
        "i32 %x",
        "  %a = xor i32 %x, -8\n  %r = icmp ult i32 %a, %x\n  ret i1 %r\n",
        "  %a = xor i32 %x, -8\n  %0 = icmp sgt i32 %a, -1\n  ret i1 %0\n",
    );
    check(
        "i32 %x",
        "  %a = xor i32 %x, -8\n  %r = icmp sgt i32 %x, %a\n  ret i1 %r\n",
        "  %a = xor i32 %x, -8\n  %0 = icmp slt i32 %a, 0\n  ret i1 %0\n",
    );
}

/// `foldICmpOrXX`: `(x | y) u<= x` is an equality (`opt`: `icmp eq i32 %o,
/// %x`).
#[test]
fn an_or_compared_with_its_operand_is_an_equality() {
    check(
        "i32 %x, i32 %y",
        "  %o = or i32 %x, %y\n  %r = icmp ule i32 %o, %x\n  ret i1 %r\n",
        "  %o = or i32 %x, %y\n  %0 = icmp eq i32 %o, %x\n  ret i1 %0\n",
    );
    check(
        "i32 %x, i32 %y",
        "  %o = or i32 %x, %y\n  %r = icmp ult i32 %x, %o\n  ret i1 %r\n",
        "  %o = or i32 %x, %y\n  %0 = icmp ne i32 %o, %x\n  ret i1 %0\n",
    );
}

/// `foldICmpBinOp`, `icmp (0-X) < cst --> x > -cst` (`opt`: `icmp sgt i32 %x,
/// -5`).
#[test]
fn a_negation_compared_with_a_constant_is_the_negated_compare() {
    check(
        "i32 %x",
        "  %n = sub nsw i32 0, %x\n  %r = icmp slt i32 %n, 5\n  ret i1 %r\n",
        "  %0 = icmp sgt i32 %x, -5\n  ret i1 %0\n",
    );
}

/// `foldICmpBinOp`, the shared multiplier: `x * z` against `y * z` is `x`
/// against `y` once `z` is known not to flip or collapse the compare
/// (`opt`: `icmp slt i32 %x, %y`).
#[test]
fn a_shared_multiplier_is_dropped_from_the_compare() {
    check(
        "i32 %x, i32 %y, i32 %z0",
        "  %z = or i32 %z0, 1\n  %zp = and i32 %z, 255\n  %a = mul nsw i32 %x, %zp\n  %b = mul nsw i32 %y, %zp\n  %r = icmp slt i32 %a, %b\n  ret i1 %r\n",
        "  %0 = icmp slt i32 %x, %y\n  ret i1 %0\n",
    );
    check(
        "i32 %x, i32 %y, i32 %z0",
        "  %z = or i32 %z0, 1\n  %a = mul i32 %x, %z\n  %b = mul i32 %y, %z\n  %r = icmp eq i32 %a, %b\n  ret i1 %r\n",
        "  %0 = icmp eq i32 %x, %y\n  ret i1 %0\n",
    );
    check(
        "i32 %x, i32 %y, i32 %z0",
        "  %z = add nuw i32 %z0, 1\n  %a = mul nuw i32 %x, %z\n  %b = mul nuw i32 %y, %z\n  %r = icmp ult i32 %a, %b\n  ret i1 %r\n",
        "  %0 = icmp ult i32 %x, %y\n  ret i1 %0\n",
    );
}

/// `foldICmpBinOp`, `srem`: `x srem y` against `y` is decided by the sign of
/// `y` (`opt`: `ret i1 false`, `icmp slt i32 %y, 0`).
#[test]
fn an_srem_compared_with_its_divisor_is_decided_by_the_divisor() {
    check("i32 %x, i32 %y", "  %s = srem i32 %x, %y\n  %r = icmp eq i32 %s, %y\n  ret i1 %r\n", "  ret i1 false\n");
    check(
        "i32 %x, i32 %y",
        "  %s = srem i32 %x, %y\n  %r = icmp sge i32 %s, %y\n  ret i1 %r\n",
        "  %0 = icmp slt i32 %y, 0\n  ret i1 %0\n",
    );
    check(
        "i32 %x, i32 %y",
        "  %s = srem i32 %x, %y\n  %r = icmp sgt i32 %y, %s\n  ret i1 %r\n",
        "  %0 = icmp sgt i32 %y, -1\n  ret i1 %0\n",
    );
}

/// `foldICmpBinOp`, two operators of one opcode with the same second operand
/// (`opt`: `icmp eq i32 %a, %b`, `icmp slt i32 %a, %b`).
#[test]
fn two_operators_with_a_shared_second_operand_compare_their_first_operands() {
    check(
        "i32 %a, i32 %b, i32 %x",
        "  %p = add i32 %a, %x\n  %q = add i32 %b, %x\n  %r = icmp eq i32 %p, %q\n  ret i1 %r\n",
        "  %0 = icmp eq i32 %a, %b\n  ret i1 %0\n",
    );
    check(
        "i32 %a, i32 %b",
        "  %p = xor i32 %a, -2147483648\n  %q = xor i32 %b, -2147483648\n  %r = icmp ult i32 %p, %q\n  ret i1 %r\n",
        "  %0 = icmp slt i32 %a, %b\n  ret i1 %0\n",
    );
    check(
        "i32 %a, i32 %b",
        "  %p = xor i32 %a, 2147483647\n  %q = xor i32 %b, 2147483647\n  %r = icmp slt i32 %p, %q\n  ret i1 %r\n",
        "  %0 = icmp ugt i32 %a, %b\n  ret i1 %0\n",
    );
    check(
        "i32 %a, i32 %b, i32 %y",
        "  %p = udiv exact i32 %a, %y\n  %q = udiv exact i32 %b, %y\n  %r = icmp ult i32 %p, %q\n  ret i1 %r\n",
        "  %0 = icmp ult i32 %a, %b\n  ret i1 %0\n",
    );
    check(
        "i32 %a, i32 %b, i32 %y",
        "  %p = shl nuw i32 %a, %y\n  %q = shl nuw i32 %b, %y\n  %r = icmp ult i32 %p, %q\n  ret i1 %r\n",
        "  %0 = icmp ult i32 %a, %b\n  ret i1 %0\n",
    );
}

/// `foldICmpBinOp`, `X * C == Y * C` for an even `C` compares the low bits
/// and `foldICmpEquality` then makes it `icmp eq i32 (xor %a, %b) & 1073741823,
/// 0` as `opt` does.
#[test]
fn an_even_shared_multiplier_in_an_equality_compares_the_low_bits() {
    check(
        "i32 %a, i32 %b",
        "  %p = mul i32 %a, 12\n  %q = mul i32 %b, 12\n  %r = icmp eq i32 %p, %q\n  ret i1 %r\n",
        "  %0 = xor i32 %a, %b\n  %1 = and i32 %0, 1073741823\n  %2 = icmp eq i32 %1, 0\n  ret i1 %2\n",
    );
}

/// `foldICmpBinOp`, `x << 1` against `x` is the sign of `x`, and
/// `A & (L - 1) u< L` is `L != 0`.
#[test]
fn a_doubling_and_a_masked_value_are_compared_by_sign_and_zero() {
    check(
        "i32 %x",
        "  %p = shl i32 %x, 1\n  %r = icmp ult i32 %p, %x\n  ret i1 %r\n",
        "  %0 = icmp slt i32 %x, 0\n  ret i1 %0\n",
    );
    check(
        "i32 %a, i32 %l",
        "  %m = add i32 %l, -1\n  %p = and i32 %a, %m\n  %r = icmp ult i32 %p, %l\n  ret i1 %r\n",
        "  %0 = icmp ne i32 %l, 0\n  ret i1 %0\n",
    );
}

/// `foldICmpAndXX`: `x & y` against `x` is an equality, or an unsigned compare
/// when the signs are known (`opt`: `icmp ne i32 %a, %x`, `icmp slt i32 %x,
/// 0`).
#[test]
fn an_and_compared_with_its_operand_is_an_equality_or_a_sign_test() {
    check(
        "i32 %x, i32 %y",
        "  %a = and i32 %x, %y\n  %r = icmp ult i32 %a, %x\n  ret i1 %r\n",
        "  %a = and i32 %x, %y\n  %0 = icmp ne i32 %a, %x\n  ret i1 %0\n",
    );
    check(
        "i32 %x, i32 %y",
        "  %a = and i32 %y, %x\n  %r = icmp ule i32 %x, %a\n  ret i1 %r\n",
        "  %a = and i32 %y, %x\n  %0 = icmp eq i32 %a, %x\n  ret i1 %0\n",
    );
    check(
        "i32 %x, i32 %y0",
        "  %y = or i32 %y0, -2147483648\n  %a = and i32 %x, %y\n  %r = icmp slt i32 %a, %x\n  ret i1 %r\n",
        "  %y = or i32 %y0, -2147483648\n  %a = and i32 %x, %y\n  %0 = icmp ne i32 %a, %x\n  ret i1 %0\n",
    );
    check(
        "i32 %x, i32 %y0",
        "  %y = and i32 %y0, 2147483647\n  %a = and i32 %x, %y\n  %r = icmp sgt i32 %a, %x\n  ret i1 %r\n",
        "  %0 = icmp slt i32 %x, 0\n  ret i1 %0\n",
    );
    check(
        "i32 %x0, i32 %y",
        "  %x = or i32 %x0, -2147483648\n  %a = and i32 %x, %y\n  %r = icmp sle i32 %a, %x\n  ret i1 %r\n",
        "  %0 = icmp slt i32 %y, 0\n  ret i1 %0\n",
    );
}

/// `foldICmpXNegX`: a value against its negation (`opt`: `icmp sgt i32 %x, 0`,
/// `icmp slt i32 %x, 0`); without `nsw` only an equality folds, to the value
/// without its sign bit.
#[test]
fn a_value_compared_with_its_negation_is_compared_with_zero() {
    check(
        "i32 %x",
        "  %n = sub nsw i32 0, %x\n  %r = icmp eq i32 %n, %x\n  ret i1 %r\n",
        "  %0 = icmp eq i32 %x, 0\n  ret i1 %0\n",
    );
    check(
        "i32 %x",
        "  %n = sub nsw i32 0, %x\n  %r = icmp slt i32 %n, %x\n  ret i1 %r\n",
        "  %0 = icmp sgt i32 %x, 0\n  ret i1 %0\n",
    );
    check(
        "i32 %x",
        "  %n = sub nsw i32 0, %x\n  %r = icmp ult i32 %n, %x\n  ret i1 %r\n",
        "  %0 = icmp slt i32 %x, 0\n  ret i1 %0\n",
    );
    check(
        "i32 %x",
        "  %n = sub nsw i32 0, %x\n  %r = icmp ule i32 %x, %n\n  ret i1 %r\n",
        "  %0 = icmp sgt i32 %x, -1\n  ret i1 %0\n",
    );
    check(
        "i32 %x",
        "  %n = sub i32 0, %x\n  %r = icmp ne i32 %n, %x\n  ret i1 %r\n",
        "  %0 = and i32 %x, 2147483647\n  %1 = icmp ne i32 %0, 0\n  ret i1 %1\n",
    );
}

/// `foldMultiplicationOverflowCheck`: `(-1 /u x) u< y` and `((x * y) / x) != y`
/// were a division the target cannot do cheaply; the pin asks the multiply for
/// its overflow bit (`opt`: `%mul.ov = extractvalue ...
/// @llvm.umul.with.overflow.i32`).
#[test]
fn a_division_overflow_check_is_a_multiply_with_overflow() {
    let wrap = |body: &str| {
        format!(
            "declare {{ i32, i1 }} @llvm.umul.with.overflow.i32(i32, i32)\ndeclare {{ i32, i1 }} @llvm.smul.with.overflow.i32(i32, i32)\ndefine i1 @f(i32 %x, i32 %y, ptr %p) {{\nb0:\n{body}}}\n"
        )
    };
    let out = combined(&wrap("  %d = udiv i32 -1, %x\n  %r = icmp ult i32 %d, %y\n  ret i1 %r\n"));
    assert!(out.contains("%0 = call { i32, i1 } @llvm.umul.with.overflow.i32(i32 %x, i32 %y)"), "{out}");
    assert!(out.contains("%1 = extractvalue { i32, i1 } %0, 1\n  ret i1 %1"), "{out}");
    assert!(!out.contains("udiv"), "{out}");

    // The inverted predicate asks for the opposite answer.
    let out = combined(&wrap("  %d = udiv i32 -1, %x\n  %r = icmp uge i32 %d, %y\n  ret i1 %r\n"));
    assert!(out.contains("%2 = xor i1 %1, true\n  ret i1 %2"), "{out}");

    let out =
        combined(&wrap("  %m = mul i32 %x, %y\n  %d = udiv i32 %m, %x\n  %r = icmp ne i32 %d, %y\n  ret i1 %r\n"));
    assert!(out.contains("@llvm.umul.with.overflow.i32(i32 %x, i32 %y)") && !out.contains("udiv"), "{out}");
    assert!(out.contains("extractvalue { i32, i1 } %0, 1\n  ret i1 %1"), "{out}");

    let out =
        combined(&wrap("  %m = mul i32 %y, %x\n  %d = sdiv i32 %m, %x\n  %r = icmp eq i32 %y, %d\n  ret i1 %r\n"));
    assert!(out.contains("@llvm.smul.with.overflow.i32") && out.contains("xor i1"), "{out}");

    // A multiply read elsewhere is replaced by the intrinsic's result.
    let out = combined(&wrap(
        "  %m = mul i32 %x, %y\n  store i32 %m, ptr %p\n  %d = udiv i32 %m, %x\n  %r = icmp ne i32 %d, %y\n  ret i1 %r\n",
    ));
    assert!(out.contains("extractvalue { i32, i1 } %0, 0"), "{out}");
    assert!(out.contains("store i32 %1, ptr %p") && !out.contains("= mul i32"), "{out}");
}

/// `foldICmpWithTruncSignExtendedVal`: a value that survives a sign-extending
/// truncation is in the kept range (`opt`: `add i32 %x, 128` `u< 256`).
#[test]
fn a_value_compared_with_its_sign_extended_truncation_is_a_range_check() {
    check(
        "i32 %x",
        "  %s = shl i32 %x, 24\n  %a = ashr i32 %s, 24\n  %r = icmp eq i32 %a, %x\n  ret i1 %r\n",
        "  %0 = add i32 %x, 128\n  %1 = icmp ult i32 %0, 256\n  ret i1 %1\n",
    );
    check(
        "i32 %x",
        "  %s = shl i32 %x, 24\n  %a = ashr i32 %s, 24\n  %r = icmp ne i32 %x, %a\n  ret i1 %r\n",
        "  %0 = add i32 %x, -128\n  %1 = icmp ult i32 %0, -256\n  ret i1 %1\n",
    );
}

/// `foldICmpOrXX` and `foldICmpAndXX`, the equality forms: `(x | y) == y` and
/// `(x & y) == y` with `y` or `x` freely invertible (`getFreelyInverted`).
#[test]
fn an_or_or_and_against_its_operand_absorbs_a_freely_inverted_side() {
    check(
        "i32 %x, i32 %z",
        "  %y = xor i32 %z, -1\n  %o = or i32 %x, %y\n  %r = icmp eq i32 %o, %y\n  ret i1 %r\n",
        "  %0 = and i32 %x, %z\n  %1 = icmp eq i32 %0, 0\n  ret i1 %1\n",
    );
    check(
        "i32 %xx, i32 %y",
        "  %x = xor i32 %xx, -1\n  %o = or i32 %x, %y\n  %r = icmp ne i32 %o, %y\n  ret i1 %r\n",
        "  %0 = or i32 %y, %xx\n  %1 = icmp ne i32 %0, -1\n  ret i1 %1\n",
    );
    check(
        "i32 %x, i32 %z",
        "  %y = xor i32 %z, -1\n  %o = and i32 %x, %y\n  %r = icmp eq i32 %o, %y\n  ret i1 %r\n",
        "  %0 = or i32 %x, %z\n  %1 = icmp eq i32 %0, -1\n  ret i1 %1\n",
    );
    check(
        "i32 %xx, i32 %y",
        "  %x = xor i32 %xx, -1\n  %o = and i32 %x, %y\n  %r = icmp ne i32 %o, %y\n  ret i1 %r\n",
        "  %0 = and i32 %y, %xx\n  %1 = icmp ne i32 %0, 0\n  ret i1 %1\n",
    );
    check(
        "i32 %p, i32 %y",
        "  %x = add i32 %p, 5\n  %o = and i32 %x, %y\n  %r = icmp eq i32 %o, %y\n  ret i1 %r\n",
        "  %0 = sub i32 -6, %p\n  %1 = and i32 %y, %0\n  %2 = icmp eq i32 %1, 0\n  ret i1 %2\n",
    );
}

/// `foldICmpBinOp`: `(x & -P2) == INT_MIN` is `x s< INT_MIN + P2`, `!=` is
/// `s>=` (`opt`: `icmp slt i16 %x, -32764`, `icmp sgt i16 %x, -32765`). Left
/// alone, the masked compare reached the selector as an and plus a compare.
#[test]
fn test_a_masked_compare_with_the_sign_mask_is_a_signed_bound() {
    let m = "  %m = and i16 %x, -4\n";
    check(
        "i16 %x",
        &format!("{m}  %r = icmp eq i16 %m, -32768\n  ret i1 %r\n"),
        &format!("  %0 = icmp slt i16 %x, -32764\n  ret i1 %0\n"),
    );
    check(
        "i16 %x",
        &format!("{m}  %r = icmp ne i16 %m, -32768\n  ret i1 %r\n"),
        &format!("  %0 = icmp sgt i16 %x, -32765\n  ret i1 %0\n"),
    );
    check(
        "i16 %x",
        &format!("{m}  %r = icmp eq i16 %m, 8\n  ret i1 %r\n"),
        &format!("{m}  %r = icmp eq i16 %m, 8\n  ret i1 %r\n"),
    );
}

/// `foldICmpBinOp`: `((b + M) & M) u< b` is `b != 0` for a low-bit mask `M`
/// (`opt`: `icmp ne i16 %b, 0`); a different addend is not the idiom.
#[test]
fn test_an_offset_and_mask_overflow_check_is_a_zero_test() {
    let m = "  %s = add i16 %b, 3\n  %m = and i16 %s, 3\n";
    check(
        "i16 %b",
        &format!("{m}  %r = icmp ult i16 %m, %b\n  ret i1 %r\n"),
        &format!("  %0 = icmp ne i16 %b, 0\n  ret i1 %0\n"),
    );
    check(
        "i16 %b",
        &format!("{m}  %r = icmp uge i16 %m, %b\n  ret i1 %r\n"),
        &format!("  %0 = icmp eq i16 %b, 0\n  ret i1 %0\n"),
    );
    check(
        "i16 %b",
        &format!("{m}  %r = icmp ugt i16 %b, %m\n  ret i1 %r\n"),
        &format!("  %0 = icmp ne i16 %b, 0\n  ret i1 %0\n"),
    );
    check(
        "i16 %b",
        &format!("{m}  %r = icmp ule i16 %b, %m\n  ret i1 %r\n"),
        &format!("  %0 = icmp eq i16 %b, 0\n  ret i1 %0\n"),
    );
    let other = "  %s = add i16 %b, 5\n  %m = and i16 %s, 3\n  %r = icmp ult i16 %m, %b\n  ret i1 %r\n";
    check("i16 %b", other, other);
}

/// `foldICmpMulConstant`; every shape pinned by `opt -passes=instcombine`
/// (i16). A square against a constant is a compare of roots: the sieve's
/// `i * i < n` kept its multiply, 150 B over the pin on the QCport family.
#[test]
fn test_a_square_against_a_constant_is_a_compare_of_roots() {
    let sq = |flags: &str, pred: &str, c: i32| {
        format!("  %m = mul {flags} i16 %x, %x\n  %r = icmp {pred} i16 %m, {c}\n  ret i1 %r\n")
    };
    let cases = [
        ("nuw nsw", "ult", 1024, "  %0 = icmp ult i16 %x, 32\n  ret i1 %0\n"),
        ("nuw", "ult", 1000, "  %0 = icmp ult i16 %x, 32\n  ret i1 %0\n"),
        ("nuw", "ugt", 1000, "  %0 = icmp ugt i16 %x, 31\n  ret i1 %0\n"),
        ("nuw", "eq", 1000, "  ret i1 false\n"),
        ("nuw", "ne", 900, "  %0 = icmp ne i16 %x, 30\n  ret i1 %0\n"),
    ];
    for (flags, pred, c, after) in cases {
        check("i16 %x", &sq(flags, pred, c), after);
    }
}

/// `foldICmpMulConstant`'s constant-factor arms (pinned, i16): equality
/// divides the compare constant, an `nuw`/`nsw` multiply divides it rounding
/// the way the predicate needs, and a sign test drops the multiply.
#[test]
fn test_a_product_with_a_constant_against_a_constant_divides_the_constant() {
    let run = |body: &str, after: &str| check("i16 %x", body, after);
    run(
        "  %m = mul nuw i16 %x, 6\n  %r = icmp ult i16 %m, 20\n  ret i1 %r\n",
        "  %0 = icmp ult i16 %x, 4\n  ret i1 %0\n",
    );
    run(
        "  %m = mul nuw i16 %x, 6\n  %r = icmp ugt i16 %m, 20\n  ret i1 %r\n",
        "  %0 = icmp ugt i16 %x, 3\n  ret i1 %0\n",
    );
    run(
        "  %m = mul nsw i16 %x, -4\n  %r = icmp slt i16 %m, 10\n  ret i1 %r\n",
        "  %0 = icmp sgt i16 %x, -3\n  ret i1 %0\n",
    );
    run(
        "  %m = mul nsw i16 %x, 4\n  %r = icmp sgt i16 %m, -10\n  ret i1 %r\n",
        "  %0 = icmp sgt i16 %x, -3\n  ret i1 %0\n",
    );
    run("  %m = mul i16 %x, 5\n  %r = icmp eq i16 %m, 100\n  ret i1 %r\n", "  %0 = icmp eq i16 %x, 20\n  ret i1 %0\n");
    run(
        "  %m = mul nsw i16 %x, 6\n  %r = icmp ne i16 %m, -18\n  ret i1 %r\n",
        "  %0 = icmp ne i16 %x, -3\n  ret i1 %0\n",
    );
    run(
        "  %m = mul nsw i16 %x, -3\n  %r = icmp slt i16 %m, 0\n  ret i1 %r\n",
        "  %0 = icmp sgt i16 %x, 0\n  ret i1 %0\n",
    );
}

/// `foldGEPICmp`, `CommonPointerBase`'s two-GEP arm: `zero()` of the QB heap
/// compared `p + 24` with `p + x - 8` as pointers, so loop-idiom's memset
/// length was three instructions where the pin's is eight and `lh_alloc`
/// inlined `first_fit` (311 instructions against the pin's 90).
#[test]
fn test_two_geps_off_one_base_compare_their_offsets() {
    let after = combined(
        "define i1 @f(ptr %p, i64 %x) {
b0:
  %a = getelementptr inbounds nuw i8, ptr %p, i64 24
  %b = getelementptr inbounds nuw i8, ptr %p, i64 %x
  %c = getelementptr inbounds i8, ptr %b, i64 -8
  %r = icmp ult ptr %a, %c
  ret i1 %r
}
",
    );
    assert!(after.contains("icmp sgt i64 %x, 32") && !after.contains("getelementptr"), "{after}");
}

/// Two GEPs of one base that differ in one index compare that index.
#[test]
fn test_two_geps_that_differ_in_one_index_compare_it() {
    let after = combined(
        "define i1 @f(ptr %p, i64 %a, i64 %b) {
b0:
  %x = getelementptr inbounds nuw i32, ptr %p, i64 %a
  %y = getelementptr inbounds nuw i32, ptr %p, i64 %b
  %r = icmp ult ptr %x, %y
  ret i1 %r
}
",
    );
    assert!(after.contains("icmp samesign ult i64 %a, %b") && !after.contains("getelementptr"), "{after}");
}

/// A GEP compared with itself needs no compare.
#[test]
fn test_a_gep_compared_with_an_equal_gep_is_decided() {
    let after = combined(
        "define i1 @f(ptr %p, i64 %a) {
b0:
  %x = getelementptr inbounds nuw i32, ptr %p, i64 %a
  %y = getelementptr inbounds nuw i32, ptr %p, i64 %a
  %r = icmp ule ptr %x, %y
  ret i1 %r
}
",
    );
    assert!(after.contains("ret i1 true") && !after.contains("icmp"), "{after}");
}

/// GEPs of different bases and the same indices compare the bases.
#[test]
fn test_geps_of_two_bases_with_the_same_indices_compare_the_bases() {
    let after = combined(
        "define i1 @f(ptr %p, ptr %q, i64 %a) {
b0:
  %x = getelementptr inbounds nuw i32, ptr %p, i64 %a
  %y = getelementptr inbounds nuw i32, ptr %q, i64 %a
  %r = icmp ult ptr %x, %y
  ret i1 %r
}
",
    );
    assert!(after.contains("icmp ult ptr %p, %q") && !after.contains("getelementptr"), "{after}");
}

/// Without flags the offsets still compare when both cross the base's
/// alignment boundary the same number of times: `p + 1 < p + 3` on a base
/// aligned to 16.
#[test]
fn test_constant_offsets_inside_one_alignment_unit_compare_without_flags() {
    let after = combined(
        "define i1 @f(ptr align 16 %p) {
b0:
  %x = getelementptr i8, ptr %p, i64 1
  %y = getelementptr i8, ptr %p, i64 3
  %r = icmp ult ptr %x, %y
  ret i1 %r
}
",
    );
    assert!(!after.contains("icmp ult ptr"), "{after}");
    let across = combined(
        "define i1 @f(ptr align 16 %p) {
b0:
  %x = getelementptr i8, ptr %p, i64 15
  %y = getelementptr i8, ptr %p, i64 17
  %r = icmp ult ptr %x, %y
  ret i1 %r
}
",
    );
    assert!(across.contains("icmp ult ptr"), "offsets on two sides of the boundary are left: {across}");
}

/// `foldSelectICmp`: `(c ? 0 : y) == 0` with `c = y u< 32` is `c`: the compare
/// folds into the arms (`true`, and `y == 0` which `c` implies), and the select
/// into a logical or. Left alone, QB `carve` kept a priced `icmp` the pin's
/// does not (first_fit 240 against 230, so it was not inlined as the pin's is).
#[test]
fn test_a_compare_of_a_select_folds_into_its_arms() {
    let after = combined(
        "define i1 @f(i64 %y) {
b0:
  %c = icmp ult i64 %y, 32
  %s = select i1 %c, i64 0, i64 %y
  %r = icmp eq i64 %s, 0
  ret i1 %r
}
",
    );
    // `c` implies `y != 0` when false: both arms fold to constants (the select
    // itself is `visitSelectInst`'s).
    assert!(after.contains("select i1 %c, i1 true, i1 false") && !after.contains("select i1 %c, i64"), "{after}");
}

/// With a single use the compare is moved into both arms: `(c ? a : b) < 5` is
/// `c ? a < 5 : b < 5`.
#[test]
fn test_a_compare_of_a_one_use_select_with_a_folding_arm_becomes_a_select_of_compares() {
    let after = combined(
        "define i1 @f(i1 %c, i64 %y) {
b0:
  %s = select i1 %c, i64 0, i64 %y
  %r = icmp ult i64 %s, 5
  ret i1 %r
}
",
    );
    assert!(after.contains("select i1 %c, i1 true, i1 %") && after.contains("icmp ult i64 %y, 5"), "{after}");
}

/// An unsigned compare of two operands with a known-clear sign bit stayed
/// plain `ult`, so ConstraintElimination could not use it and quicksort's
/// `a[j]` bounds check stayed in the loop (+13% clocks at -Os).
#[test]
fn test_an_unsigned_compare_of_two_non_negative_values_is_samesign() {
    let after = combined(
        "define i1 @f(i16 %x, i16 %y) {
b0:
  %a = and i16 %x, 255
  %b = and i16 %y, 255
  %c = icmp ult i16 %a, %b
  ret i1 %c
}
",
    );
    assert!(after.contains("icmp samesign ult"), "{after}");
}

/// InstCombine's queries read no metadata, so a load's `!range` told it
/// nothing: `icmp ult %i, %len` never got `samesign` and quicksort's `a[j]`
/// check stayed in the loop.
#[test]
fn test_a_load_range_makes_an_unsigned_compare_samesign() {
    let after = combined(
        "define i1 @f(ptr %p, ptr %q) {
b0:
  %a = load i16, ptr %p, !range !0
  %b = load i16, ptr %q, !range !0
  %c = icmp ult i16 %a, %b
  ret i1 %c
}
!0 = !{i16 0, i16 -32768}
",
    );
    assert!(after.contains("icmp samesign ult"), "{after}");
}

/// `foldICmpWithCastOp`'s pointer arms (pinned by `opt -passes=instcombine`):
/// `icmp eq (inttoptr %x), null` stays a pointer compare here, so QCport's
/// `sprite_load` carried a far-pointer null test (`%p = inttoptr i32; icmp eq
/// ptr addrspace(1) %p, null`) where the pin tests the integer: +10
/// instructions.
#[test]
fn test_a_compare_of_a_same_width_inttoptr_is_the_integer_compare() {
    let out = combined(
        "define i1 @f(i64 %x, i64 %y) {\nb0:\n  %p = inttoptr i64 %x to ptr\n  %q = inttoptr i64 %y to ptr\n  %a = icmp eq ptr %p, null\n  %b = icmp ult ptr %p, %q\n  %r = and i1 %a, %b\n  ret i1 %r\n}\n",
    );
    assert!(out.contains("icmp eq i64 %x, 0") && out.contains("icmp ult i64 %x, %y"), "{out}");
    let out = combined(
        "define i1 @f(ptr %p) {\nb0:\n  %i = ptrtoint ptr %p to i64\n  %r = icmp eq i64 %i, 0\n  ret i1 %r\n}\n",
    );
    assert!(out.contains("icmp eq ptr %p, null"), "{out}");
}

/// A narrower integer than the pointer is not the same compare.
#[test]
fn test_a_compare_of_a_narrower_inttoptr_is_kept() {
    let out = combined(
        "define i1 @f(i32 %x) {\nb0:\n  %p = inttoptr i32 %x to ptr\n  %r = icmp eq ptr %p, null\n  ret i1 %r\n}\n",
    );
    assert!(out.contains("icmp eq ptr"), "{out}");
}
