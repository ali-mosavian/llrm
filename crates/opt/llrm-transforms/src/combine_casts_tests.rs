use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{CastOp, Opcode};

use crate::combine::{Combiner, Visit, combine_with};
use crate::combine_casts::{visit_sext, visit_trunc, visit_zext};
use crate::testing::{parsed, printed};

/// The casts' `visit*` alone, through the driver.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    match c.function.instruction(inst).opcode {
        Opcode::Cast(CastOp::Trunc) => visit_trunc(c, inst),
        Opcode::Cast(CastOp::ZExt) => visit_zext(c, inst),
        Opcode::Cast(CastOp::SExt) => visit_sext(c, inst),
        _ => Visit::None,
    }
}

/// `@f` after the casts' `visit*` alone, under `layout`.
fn combined_under(
    layout: &str,
    text: &str,
) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let layout = DataLayout::parse(layout).expect("a datalayout");
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &layout, function, visit);
    printed(&module)
}

/// `define RET @f(PARAMS)` with `body` before and `after` after the casts'
/// `visit*`, with i8, i16, i32 and i64 native (as `opt` is run here).
fn check(
    ret: &str,
    params: &str,
    body: &str,
    after: &str,
) {
    let text = format!("define {ret} @f({params}) {{\nb0:\n{body}}}\n");
    let want = format!("define {ret} @f({params}) {{\nb0:\n{after}}}\n");
    assert_eq!(combined_under("n8:16:32:64", &text), want);
}

/// `check` with no native integer, where an extension is never worth widening
/// an expression for.
fn check_plain(
    ret: &str,
    params: &str,
    body: &str,
    after: &str,
) {
    let text = format!("define {ret} @f({params}) {{\nb0:\n{body}}}\n");
    let want = format!("define {ret} @f({params}) {{\nb0:\n{after}}}\n");
    assert_eq!(combined_under("", &text), want);
}

/// `EvaluateInDifferentType`: `trunc (zext x) + 1` is computed in the narrow
/// type, and so is a select of extensions (`opt` agrees on both).
#[test]
fn a_truncated_tree_is_computed_in_the_narrow_type() {
    check(
        "i8",
        "i8 %x",
        "  %a = zext i8 %x to i32\n  %b = add i32 %a, 1\n  %r = trunc i32 %b to i8\n  ret i8 %r\n",
        "  %0 = add i8 %x, 1\n  ret i8 %0\n",
    );
    check(
        "i8",
        "i1 %c, i8 %x, i8 %y",
        "  %a = zext i8 %x to i32\n  %b = zext i8 %y to i32\n  %s = select i1 %c, i32 %a, i32 %b\n  %r = trunc i32 %s to i8\n  ret i8 %r\n",
        "  %0 = select i1 %c, i8 %x, i8 %y\n  ret i8 %0\n",
    );
}

/// A value read twice by the tree is built once: `(z * 3) + (z * 3)` in i8 has
/// one multiply (from the C++; `opt` folds on to `mul 6`).
#[test]
fn a_value_the_tree_reads_twice_is_evaluated_once() {
    check(
        "i8",
        "i16 %x",
        "  %z = zext i16 %x to i32\n  %a = mul i32 %z, 3\n  %b = add i32 %a, %a\n  %r = trunc i32 %b to i8\n  ret i8 %r\n",
        "  %0 = trunc i16 %x to i8\n  %1 = mul i8 %0, 3\n  %2 = add i8 %1, %1\n  ret i8 %2\n",
    );
}

/// `commonCastTransforms`: a trunc of a trunc is one trunc, a zext of a zext
/// one zext (`opt` agrees); a trunc of a phi of numbers is a phi of numbers.
#[test]
fn a_cast_of_a_cast_is_one_cast_and_a_cast_of_a_phi_folds_into_it() {
    check(
        "i8",
        "i64 %x",
        "  %e = trunc i64 %x to i32\n  %r = trunc i32 %e to i8\n  ret i8 %r\n",
        "  %0 = trunc i64 %x to i8\n  ret i8 %0\n",
    );
    check(
        "i32",
        "i8 %x",
        "  %e = zext i8 %x to i16\n  %r = zext i16 %e to i32\n  ret i32 %r\n",
        "  %0 = zext i8 %x to i32\n  ret i32 %0\n",
    );
    let text = "define i8 @f(i1 %c) {
b0:
  br i1 %c, label %a, label %b

a:
  br label %j

b:
  br label %j

j:
  %p = phi i32 [ 1, %a ], [ 258, %b ]
  %r = trunc i32 %p to i8
  ret i8 %r
}
";
    let want = "define i8 @f(i1 %c) {
b0:
  br i1 %c, label %a, label %b

a:
  br label %j

b:
  br label %j

j:
  %0 = phi i8 [ 1, %a ], [ 2, %b ]
  ret i8 %0
}
";
    assert_eq!(combined_under("n8:16:32:64", text), want);
}

/// `narrowBinOp`: a truncated binary operation with a constant or an
/// extension of the narrow type is done in the narrow type (`opt` agrees).
#[test]
fn a_truncated_binop_with_a_constant_or_an_extension_is_narrow() {
    check(
        "i8",
        "i32 %x",
        "  %s = add i32 %x, 7\n  %r = trunc i32 %s to i8\n  ret i8 %r\n",
        "  %0 = trunc i32 %x to i8\n  %1 = add i8 %0, 7\n  ret i8 %1\n",
    );
    check(
        "i8",
        "i32 %x",
        "  %s = sub i32 7, %x\n  %r = trunc i32 %s to i8\n  ret i8 %r\n",
        "  %0 = trunc i32 %x to i8\n  %1 = sub i8 7, %0\n  ret i8 %1\n",
    );
    check(
        "i8",
        "i8 %x, i32 %y",
        "  %a = zext i8 %x to i32\n  %s = add i32 %a, %y\n  %r = trunc i32 %s to i8\n  ret i8 %r\n",
        "  %0 = trunc i32 %y to i8\n  %1 = add i8 %x, %0\n  ret i8 %1\n",
    );
}

/// `narrowBinOp`: `trunc (lshr (trunc A), C)` shifts A itself (`opt` agrees).
#[test]
fn a_truncated_shift_of_a_truncate_shifts_the_source() {
    check(
        "i16",
        "i64 %a",
        "  %t = trunc i64 %a to i32\n  %s = lshr i32 %t, 12\n  %r = trunc i32 %s to i16\n  ret i16 %r\n",
        "  %0 = lshr i64 %a, 12\n  %1 = trunc i64 %0 to i16\n  ret i16 %1\n",
    );
}

/// `trunc (lshr (sext A), C)` is `ashr A, C`, with a cast after when the types
/// differ (`opt` agrees on both).
#[test]
fn a_truncated_shift_of_a_sext_is_an_ashr() {
    check(
        "i16",
        "i16 %a",
        "  %e = sext i16 %a to i32\n  %s = lshr i32 %e, 3\n  %r = trunc i32 %s to i16\n  ret i16 %r\n",
        "  %0 = ashr i16 %a, 3\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i8 %a",
        "  %e = sext i8 %a to i32\n  %s = lshr i32 %e, 3\n  %r = trunc i32 %s to i16\n  ret i16 %r\n",
        "  %0 = ashr i8 %a, 3\n  %1 = sext i8 %0 to i16\n  ret i16 %1\n",
    );
}

/// `trunc (shl X, C)` is `shl (trunc X), C` (`opt` agrees).
#[test]
fn a_truncated_shl_is_a_narrow_shl() {
    check(
        "i8",
        "i32 %x",
        "  %s = shl i32 %x, 3\n  %r = trunc i32 %s to i8\n  ret i8 %r\n",
        "  %0 = trunc i32 %x to i8\n  %1 = shl i8 %0, 3\n  ret i8 %1\n",
    );
}

/// A trunc to `i1` of a shift, a shifted constant, an xor or a nuw shift is a
/// compare (`opt` agrees, except `ashr -8` where it writes `ugt %x, 2`).
#[test]
fn a_trunc_to_bool_is_a_compare() {
    let case = |body: &str, after: &str| {
        check("i1", "i32 %x, i32 %y", &format!("{body}  ret i1 %r\n"), &format!("{after}  ret i1 %0\n"));
    };
    case("  %s = lshr i32 %x, 31\n  %r = trunc i32 %s to i1\n", "  %0 = icmp slt i32 %x, 0\n");
    case("  %s = shl i32 1, %x\n  %r = trunc i32 %s to i1\n", "  %0 = icmp eq i32 %x, 0\n");
    case("  %s = xor i32 %x, %y\n  %r = trunc nuw i32 %s to i1\n", "  %0 = icmp ne i32 %x, %y\n");
    case("  %s = lshr i32 8, %x\n  %r = trunc i32 %s to i1\n", "  %0 = icmp eq i32 %x, 3\n");
    case("  %s = lshr i32 15, %x\n  %r = trunc i32 %s to i1\n", "  %0 = icmp ult i32 %x, 4\n");
    case("  %s = ashr i32 -8, %x\n  %r = trunc i32 %s to i1\n", "  %0 = icmp uge i32 %x, 3\n");
    case("  %s = lshr exact i32 %x, 3\n  %r = trunc nuw i32 %s to i1\n", "  %0 = icmp ne i32 %x, 0\n");
    case("  %a = shl i32 4, %y\n  %s = lshr i32 %a, 5\n  %r = trunc i32 %s to i1\n", "  %0 = icmp eq i32 %y, 3\n");
    check(
        "i1",
        "i32 %x, i32 %y",
        "  %s = lshr i32 %x, 5\n  %r = trunc i32 %s to i1\n  ret i1 %r\n",
        "  %0 = and i32 %x, 32\n  %1 = icmp ne i32 %0, 0\n  ret i1 %1\n",
    );
}

/// A trunc learns `nuw` when the high bits are zero and `nsw` when they are
/// sign copies (`opt` agrees on `nuw`; it rewrites an `ashr` to `lshr`, since
/// SimplifyDemandedInstructionBits sees only the low byte demanded).
#[test]
fn a_trunc_learns_its_wrap_flags() {
    let same = "define i8 @f(i32 %x) {\nb0:\n  %s = lshr i32 %x, 24\n  %r = trunc nuw i32 %s to i8\n  ret i8 %r\n}\n";
    assert_eq!(combined_under("n8:16:32:64", &same.replace("trunc nuw", "trunc")), same);
    let signed = "define i8 @f(i32 %x) {\nb0:\n  %s = ashr i32 %x, 24\n  %r = trunc i32 %s to i8\n  ret i8 %r\n}\n";
    assert_eq!(combined_under("n8:16:32:64", signed), same.replace("%s = lshr", "%0 = lshr").replace("%s to", "%0 to"));
}

/// `canEvaluateZExtd`: `zext (trunc x + 1)` is the wide add under a mask
/// (`opt` agrees); a zext of a trunc is a mask.
#[test]
fn a_zext_of_a_narrow_tree_is_the_wide_tree_masked() {
    check(
        "i32",
        "i32 %x",
        "  %t = trunc i32 %x to i8\n  %a = add i8 %t, 1\n  %r = zext i8 %a to i32\n  ret i32 %r\n",
        "  %0 = add i32 %x, 1\n  %1 = and i32 %0, 255\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i64 %a",
        "  %t = trunc i64 %a to i8\n  %r = zext i8 %t to i32\n  ret i32 %r\n",
        "  %0 = trunc i64 %a to i32\n  %1 = and i32 %0, 255\n  ret i32 %1\n",
    );
}

/// `zext (trunc A)` when the tree is not widened (no native integer): a mask,
/// before or after a cast (`opt` agrees; its `zext` of the i16 mask is `nneg`).
#[test]
fn a_zext_of_a_trunc_is_a_mask_when_not_widened() {
    check_plain(
        "i32",
        "i32 %a",
        "  %t = trunc i32 %a to i8\n  %r = zext i8 %t to i32\n  ret i32 %r\n",
        "  %0 = and i32 %a, 255\n  ret i32 %0\n",
    );
    check_plain(
        "i32",
        "i16 %a",
        "  %t = trunc i16 %a to i8\n  %r = zext i8 %t to i32\n  ret i32 %r\n",
        "  %0 = and i16 %a, 255\n  %1 = zext nneg i16 %0 to i32\n  ret i32 %1\n",
    );
    check_plain(
        "i32",
        "i64 %a",
        "  %t = trunc i64 %a to i8\n  %r = zext i8 %t to i32\n  ret i32 %r\n",
        "  %0 = trunc i64 %a to i32\n  %1 = and i32 %0, 255\n  ret i32 %1\n",
    );
}

/// The late `zext` folds over a trunc: `(trunc X & C) ^ C`, `0 - trunc X`,
/// `trunc X & C`, and a bitwise operation with a `trunc nuw` (`opt` agrees).
#[test]
fn a_zext_of_a_masked_or_negated_trunc_is_wide() {
    check_plain(
        "i32",
        "i32 %x",
        "  %t = trunc i32 %x to i8\n  %a = and i8 %t, 12\n  %b = xor i8 %a, 12\n  %r = zext i8 %b to i32\n  ret i32 %r\n",
        "  %0 = and i32 %x, 12\n  %1 = xor i32 %0, 12\n  ret i32 %1\n",
    );
    check_plain(
        "i32",
        "i32 %x",
        "  %t = trunc i32 %x to i8\n  %n = sub i8 0, %t\n  %r = zext i8 %n to i32\n  ret i32 %r\n",
        "  %0 = sub i32 0, %x\n  %1 = and i32 %0, 255\n  ret i32 %1\n",
    );
    check_plain(
        "i32",
        "i32 %x",
        "  %t = trunc i32 %x to i8\n  %a = and i8 %t, 12\n  %r = zext i8 %a to i32\n  ret i32 %r\n",
        "  %0 = and i32 %x, 12\n  ret i32 %0\n",
    );
    check(
        "i64",
        "i64 %x, i8 %y",
        "  %t = trunc nuw i64 %x to i8\n  %q = and i8 %t, %y\n  %r = zext i8 %q to i64\n  ret i64 %r\n",
        "  %0 = zext i8 %y to i64\n  %1 = and i64 %x, %0\n  ret i64 %1\n",
    );
}

/// `transformZExtICmp`: the sign test is a shift; a compare of a one-bit value
/// with zero is the bit; a shifted-one mask test is a shift and a mask (`opt`
/// agrees on the sign test and the mask test; it masks the single bit too).
#[test]
fn a_zext_of_a_compare_is_bitwise() {
    check(
        "i32",
        "i32 %x",
        "  %c = icmp slt i32 %x, 0\n  %r = zext i1 %c to i32\n  ret i32 %r\n",
        "  %0 = lshr i32 %x, 31\n  ret i32 %0\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %m = and i32 %x, 4\n  %c = icmp ne i32 %m, 0\n  %r = zext i1 %c to i32\n  ret i32 %r\n",
        "  %m = and i32 %x, 4\n  %0 = lshr i32 %m, 2\n  ret i32 %0\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %m = and i32 %x, 4\n  %c = icmp eq i32 %m, 0\n  %r = zext i1 %c to i32\n  ret i32 %r\n",
        "  %m = and i32 %x, 4\n  %0 = lshr i32 %m, 2\n  %1 = xor i32 %0, 1\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i32 %x, i32 %s",
        "  %sh = shl i32 1, %s\n  %m = and i32 %x, %sh\n  %c = icmp ne i32 %m, 0\n  %r = zext i1 %c to i32\n  ret i32 %r\n",
        "  %0 = lshr i32 %x, %s\n  %1 = and i32 %0, 1\n  ret i32 %1\n",
    );
}

/// A zext of a non-negative value gets `nneg` (`opt` agrees); a sext of one is
/// that zext.
#[test]
fn a_zext_or_sext_of_a_non_negative_is_zext_nneg() {
    let body = |ext: &str| format!("  %a = lshr i8 %x, 1\n  %r = {ext} i8 %a to i32\n  ret i32 %r\n");
    check("i32", "i8 %x", &body("zext"), "  %a = lshr i8 %x, 1\n  %r = zext nneg i8 %a to i32\n  ret i32 %r\n");
    check("i32", "i8 %x", &body("sext"), "  %a = lshr i8 %x, 1\n  %0 = zext nneg i8 %a to i32\n  ret i32 %0\n");
}

/// `canEvaluateSExtd`: `sext (trunc x + 1)` is the wide add, shifted up and
/// back down (from the C++; `opt` goes on to fold the shifts into an add).
#[test]
fn a_sext_of_a_narrow_tree_is_the_wide_tree_sign_extended_in_register() {
    check(
        "i32",
        "i32 %x",
        "  %t = trunc i32 %x to i8\n  %a = add i8 %t, 1\n  %r = sext i8 %a to i32\n  ret i32 %r\n",
        "  %0 = add i32 %x, 1\n  %1 = shl i32 %0, 24\n  %2 = ashr i32 %1, 24\n  ret i32 %2\n",
    );
}

/// `transformSExtICmp`: the sign test is an `ashr`; a one-bit test is a shift
/// pair or an add (from the C++; `opt` simplifies further).
#[test]
fn a_sext_of_a_compare_is_bitwise() {
    check(
        "i64",
        "i32 %x",
        "  %c = icmp slt i32 %x, 0\n  %r = sext i1 %c to i64\n  ret i64 %r\n",
        "  %0 = ashr i32 %x, 31\n  %1 = sext i32 %0 to i64\n  ret i64 %1\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %m = and i32 %x, 4\n  %c = icmp ne i32 %m, 0\n  %r = sext i1 %c to i32\n  ret i32 %r\n",
        "  %m = and i32 %x, 4\n  %0 = shl i32 %m, 29\n  %1 = ashr i32 %0, 31\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %m = and i32 %x, 4\n  %c = icmp eq i32 %m, 0\n  %r = sext i1 %c to i32\n  ret i32 %r\n",
        "  %m = and i32 %x, 4\n  %0 = lshr i32 %m, 2\n  %1 = add i32 %0, -1\n  ret i32 %1\n",
    );
}

/// `sext (trunc nsw x)` is `sext x`; a shl/ashr pair under a trunc is a shift
/// pair on x (`opt` agrees, and marks the `ashr` `exact`); a splat of the sign
/// bit is a shift pair (`opt` agrees).
#[test]
fn a_sext_of_a_trunc_is_a_sext_or_a_shift_pair() {
    check_plain(
        "i64",
        "i32 %x",
        "  %t = trunc nsw i32 %x to i8\n  %r = sext i8 %t to i64\n  ret i64 %r\n",
        "  %0 = sext i32 %x to i64\n  ret i64 %0\n",
    );
    check_plain(
        "i32",
        "i32 %x",
        "  %t = trunc i32 %x to i8\n  %s = shl i8 %t, 3\n  %a = ashr i8 %s, 3\n  %r = sext i8 %a to i32\n  ret i32 %r\n",
        "  %0 = shl i32 %x, 27\n  %1 = ashr i32 %0, 27\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %t = trunc i32 %x to i8\n  %a = ashr i8 %t, 7\n  %r = sext i8 %a to i32\n  ret i32 %r\n",
        "  %0 = shl i32 %x, 24\n  %1 = ashr i32 %0, 31\n  ret i32 %1\n",
    );
}

/// `commonCastTransforms`: a cast of a select whose condition is not a compare
/// goes into the arms (`FoldOpIntoSelect`): the constant arm folds, the other
/// gets the cast. Stayed a select of narrow values before.
#[test]
fn a_zext_of_a_select_with_a_constant_arm_goes_into_the_arms() {
    check(
        "i32",
        "i1 %c, i8 %y",
        "  %s = select i1 %c, i8 1, i8 %y\n  %r = zext i8 %s to i32\n  ret i32 %r\n",
        "  %0 = zext i8 %y to i32\n  %1 = select i1 %c, i32 1, i32 %0\n  ret i32 %1\n",
    );
}

/// `commonCastTransforms` leaves a cast of a select alone when its condition is a `trunc` of the select's own
/// type, as for a compare: `select_ext_const` folds the cast back out, so folding it in looped. A seed of the
/// 6502 difftest ran `combine` to 80 GB.
#[test]
fn a_zext_of_a_select_on_a_trunc_of_its_own_type_stays() {
    let body = "  %c = trunc i8 %a to i1\n  %s = select i1 %c, i8 %y, i8 5\n  %r = zext i8 %s to i16\n  ret i16 %r\n";
    check("i16", "i8 %a, i8 %y", body, body);
}

/// `trunc(smax(zext(a) - zext(b), 0))` is `usub.sat(a, b)`. Stayed a wide
/// subtract, a max and a truncate before.
#[test]
fn a_truncated_clamped_difference_of_extensions_is_a_saturating_subtract() {
    let text = "declare i32 @llvm.smax.i32(i32, i32)

define i16 @f(i16 %a, i16 %b) {
b0:
  %x = zext i16 %a to i32
  %y = zext i16 %b to i32
  %d = sub i32 %x, %y
  %m = call i32 @llvm.smax.i32(i32 %d, i32 0)
  %r = trunc i32 %m to i16
  ret i16 %r
}
";
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let layout = DataLayout::parse("e-i8:8-i16:16-i32:32-i64:64").expect("a datalayout");
    let (context, function) = module.function_mut("f").expect("@f");
    crate::combine::combine_with_declared(context, &globals, &metadata, &layout, function, visit, &mut declared);
    declared.place(&mut module).unwrap();
    let out = printed(&module);
    assert!(out.contains("call i16 @llvm.usub.sat.i16(i16 %a, i16 %b)"), "{out}");
}

/// `trunc (ctlz_i32 (zext A), B)` is `ctlz_i16 (A, B) + 16`. Stayed a 32-bit
/// count and a truncate before.
#[test]
fn a_truncated_ctlz_of_an_extension_is_a_narrow_ctlz_plus_the_width_difference() {
    let text = "declare i32 @llvm.ctlz.i32(i32, i1)

define i16 @f(i16 %a) {
b0:
  %z = zext i16 %a to i32
  %c = call i32 @llvm.ctlz.i32(i32 %z, i1 false)
  %r = trunc i32 %c to i16
  ret i16 %r
}
";
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let layout = DataLayout::parse("e-i8:8-i16:16-i32:32-i64:64").expect("a datalayout");
    let (context, function) = module.function_mut("f").expect("@f");
    crate::combine::combine_with_declared(context, &globals, &metadata, &layout, function, visit, &mut declared);
    declared.place(&mut module).unwrap();
    let out = printed(&module);
    assert!(out.contains("call i16 @llvm.ctlz.i16(i16 %a, i1 false)"), "{out}");
    assert!(out.contains("add i16 %0, 16"), "{out}");
}

/// `canEvaluateTruncated` of a min, max or abs call: the call is evaluated in
/// the narrow type. Stayed a wide call and a truncate before.
#[test]
fn a_truncated_umax_and_abs_of_extensions_are_narrow_calls() {
    let text = "declare i32 @llvm.umax.i32(i32, i32)
declare i32 @llvm.abs.i32(i32, i1)

define i16 @f(i16 %a, i16 %b) {
b0:
  %x = zext i16 %a to i32
  %y = zext i16 %b to i32
  %m = call i32 @llvm.umax.i32(i32 %x, i32 %y)
  %r = trunc i32 %m to i16
  ret i16 %r
}

define i16 @g(i16 %a) {
b0:
  %x = sext i16 %a to i32
  %m = call i32 @llvm.abs.i32(i32 %x, i1 false)
  %r = trunc i32 %m to i16
  ret i16 %r
}
";
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let layout = DataLayout::parse("e-i8:8-i16:16-i32:32-i64:64").expect("a datalayout");
    for name in ["f", "g"] {
        let (context, function) = module.function_mut(name).expect("a function");
        crate::combine::combine_with_declared(context, &globals, &metadata, &layout, function, visit, &mut declared);
    }
    declared.place(&mut module).unwrap();
    let out = printed(&module);
    assert!(out.contains("call i16 @llvm.umax.i16(i16 %a, i16 %b)"), "{out}");
    assert!(out.contains("call i16 @llvm.abs.i16(i16 %a, i1 false)"), "{out}");
}

/// `trunc` and `sext` of an `scmp` result (-1, 0 or 1) change the intrinsic's
/// result type. Stayed an `i8` compare and a cast before.
#[test]
fn a_trunc_or_sext_of_scmp_changes_its_result_type() {
    let text = "declare i8 @llvm.scmp.i8.i32(i32, i32)

define i4 @f(i32 %x, i32 %y) {
b0:
  %c = call i8 @llvm.scmp.i8.i32(i32 %x, i32 %y)
  %r = trunc i8 %c to i4
  ret i4 %r
}

define i32 @g(i32 %x, i32 %y) {
b0:
  %c = call i8 @llvm.scmp.i8.i32(i32 %x, i32 %y)
  %r = sext i8 %c to i32
  ret i32 %r
}
";
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let layout = DataLayout::parse("e-i8:8-i16:16-i32:32-i64:64").expect("a datalayout");
    for name in ["f", "g"] {
        let (context, function) = module.function_mut(name).expect("a function");
        crate::combine::combine_with_declared(context, &globals, &metadata, &layout, function, visit, &mut declared);
    }
    declared.place(&mut module).unwrap();
    let out = printed(&module);
    assert!(out.contains("call i4 @llvm.scmp.i4.i32(i32 %x, i32 %y)"), "{out}");
    assert!(out.contains("call i32 @llvm.scmp.i32.i32(i32 %x, i32 %y)"), "{out}");
}

/// `trunc (select (icmp ult A, 256), A, sext (icmp sgt A, 0))` is `trunc
/// (smin (smax (0, A), 255))`. Stayed a compare, a select and a truncate.
#[test]
fn a_truncated_saturation_select_is_a_smax_and_a_smin() {
    let text = "define i8 @f(i32 %a) {
b0:
  %lt = icmp ult i32 %a, 256
  %gt = icmp sgt i32 %a, 0
  %s = sext i1 %gt to i32
  %sel = select i1 %lt, i32 %a, i32 %s
  %r = trunc i32 %sel to i8
  ret i8 %r
}
";
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let layout = DataLayout::parse("e-i8:8-i16:16-i32:32-i64:64").expect("a datalayout");
    let (context, function) = module.function_mut("f").expect("@f");
    crate::combine::combine_with_declared(context, &globals, &metadata, &layout, function, visit, &mut declared);
    declared.place(&mut module).unwrap();
    let out = printed(&module);
    assert!(out.contains("call i32 @llvm.smax.i32(i32 0, i32 %a)"), "{out}");
    assert!(out.contains("call i32 @llvm.smin.i32(i32 %0, i32 255)"), "{out}");
}

/// `narrowFunnelShift`: a rotate done in 32 bits and truncated to 8 is
/// `fshl.i8` of the narrow value. Stayed two wide shifts, an or and a truncate.
#[test]
fn a_truncated_wide_rotate_is_a_narrow_funnel_shift() {
    let text = "define i8 @f(i8 %x, i32 %amt) {
b0:
  %zx = zext i8 %x to i32
  %n = and i32 %amt, 7
  %sub = sub i32 8, %n
  %shl = shl i32 %zx, %n
  %shr = lshr i32 %zx, %sub
  %or = or i32 %shl, %shr
  %r = trunc i32 %or to i8
  ret i8 %r
}
";
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let layout = DataLayout::parse("e-i8:8-i16:16-i32:32-i64:64").expect("a datalayout");
    let (context, function) = module.function_mut("f").expect("@f");
    crate::combine::combine_with_declared(context, &globals, &metadata, &layout, function, visit, &mut declared);
    declared.place(&mut module).unwrap();
    let out = printed(&module);
    assert!(out.contains("call i8 @llvm.fshl.i8(i8 %"), "{out}");
}
