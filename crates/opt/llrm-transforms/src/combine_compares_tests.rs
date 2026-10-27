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
