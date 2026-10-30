use llrm_mir::datalayout::DataLayout;
use llrm_mir::interpret::Val;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{BinaryOp, Opcode};

use super::{visit_ashr, visit_lshr, visit_shl};
use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed, results};

/// The shift visits alone, through the driver.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    match c.function.instruction(inst).opcode {
        Opcode::Binary(BinaryOp::Shl) => visit_shl(c, inst),
        Opcode::Binary(BinaryOp::LShr) => visit_lshr(c, inst),
        Opcode::Binary(BinaryOp::AShr) => visit_ashr(c, inst),
        _ => Visit::None,
    }
}

/// `@f` after the shift visits alone.
fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// Every pair of small values for `params` (a shift amount stays in range).
fn inputs(params: &str) -> Vec<Vec<i128>> {
    const SMALL: [i128; 8] = [0, 1, 2, 3, 5, 7, 12, 15];
    let count = params.split(',').count();
    let mut all = vec![Vec::new()];
    for _ in 0..count {
        all = all.into_iter().flat_map(|prefix| SMALL.map(|value| [prefix.clone(), vec![value]].concat())).collect();
    }
    all
}

/// `define RET @f(PARAMS)` with `body` before and `after` after the shift
/// visits, and `@f` computing the same (where it did not return poison).
fn check(
    ret: &str,
    params: &str,
    body: &str,
    after: &str,
) {
    check_declared("", "", ret, params, body, after);
}

/// [`check`] of a function that calls the intrinsic `declared` declares,
/// which the printed module shows as `printed_declaration`.
fn check_declared(
    declared: &str,
    printed_declaration: &str,
    ret: &str,
    params: &str,
    body: &str,
    after: &str,
) {
    let text = format!("{declared}define {ret} @f({params}) {{\nb0:\n{body}}}\n");
    let want = format!("{printed_declaration}define {ret} @f({params}) {{\nb0:\n{after}}}\n");
    let got = combined(&text);
    assert_eq!(got, want);
    let all = inputs(params);
    let refs: Vec<&[i128]> = all.iter().map(Vec::as_slice).collect();
    let (before, now) = (results(&parsed(&text), &refs), results(&parsed(&got), &refs));
    for ((was, is), input) in before.iter().zip(&now).zip(&all) {
        assert!(
            *was == Val::Poison || was == is,
            "@f{input:?} was {was:?}, is {is:?}
{got}"
        );
    }
}

/// `commonShiftTransforms`: a one-use `sext` amount is a `zext` (`opt` adds
/// `nneg`).
#[test]
fn a_one_use_sext_shift_amount_becomes_a_zext() {
    check(
        "i32",
        "i32 %x, i8 %y",
        "  %e = sext i8 %y to i32\n  %r = shl i32 %x, %e\n  ret i32 %r\n",
        "  %0 = zext i8 %y to i32\n  %1 = shl i32 %x, %0\n  ret i32 %1\n",
    );
}

/// `commonShiftTransforms`: `x shift (a srem 8)` is `x shift (a and 7)`.
#[test]
fn a_shift_by_a_srem_of_a_power_of_two_masks_the_amount() {
    check(
        "i32",
        "i32 %x, i32 %a",
        "  %m = srem i32 %a, 8\n  %r = shl i32 %x, %m\n  ret i32 %r\n",
        "  %0 = and i32 %a, 7\n  %r = shl i32 %x, %0\n  ret i32 %r\n",
    );
}

/// `commonShiftTransforms`: `x >> (a | 31)` is `x >> 31`.
#[test]
fn an_amount_or_ed_with_the_width_less_one_is_that_constant() {
    check(
        "i32",
        "i32 %x, i32 %a",
        "  %m = or i32 %a, 31\n  %r = lshr i32 %x, %m\n  ret i32 %r\n",
        "  %r = lshr i32 %x, 31\n  ret i32 %r\n",
    );
}

/// `FoldShiftByConstant`, `canEvaluateShifted` and
/// `reassociateShiftAmtsOfTwoSameDirectionShifts`: `(x << 3) << 4` is `x << 7`;
/// a sum past the width is 0 or poison; through a `trunc` the larger survives.
#[test]
fn two_shifts_the_same_way_join() {
    check(
        "i32",
        "i32 %x",
        "  %a = shl i32 %x, 3\n  %r = shl i32 %a, 4\n  ret i32 %r\n",
        "  %a = shl i32 %x, 7\n  ret i32 %a\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %a = lshr i32 %x, 3\n  %r = lshr i32 %a, 4\n  ret i32 %r\n",
        "  %0 = lshr i32 %x, 7\n  ret i32 %0\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %a = lshr i32 %x, 7\n  %r = lshr i32 %a, 9\n  ret i32 %r\n",
        "  %0 = lshr i32 %x, 16\n  ret i32 %0\n",
    );
    check("i32", "i32 %x", "  %a = shl i32 %x, 20\n  %r = shl i32 %a, 20\n  ret i32 %r\n", "  ret i32 0\n");
    check(
        "i16",
        "i32 %x",
        "  %a = lshr i32 %x, 4\n  %t = trunc i32 %a to i16\n  %r = lshr i16 %t, 3\n  ret i16 %r\n",
        "  %0 = lshr i32 %x, 7\n  %1 = trunc i32 %0 to i16\n  %2 = and i16 %1, 8191\n  ret i16 %2\n",
    );
}

/// `canEvaluateShiftedShift`: `(x >> 4) << 4` and `(x << 4) >> 4` are masks; a
/// bigger first shift is a shift where the cleared bits are known zero (`opt`
/// folds the mask further).
#[test]
fn a_shift_of_the_opposite_shift_is_a_mask_or_a_shift() {
    check(
        "i32",
        "i32 %x",
        "  %a = shl i32 %x, 4\n  %r = lshr i32 %a, 4\n  ret i32 %r\n",
        "  %0 = and i32 %x, 268435455\n  ret i32 %0\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %a = lshr i32 %x, 4\n  %r = shl i32 %a, 4\n  ret i32 %r\n",
        "  %0 = and i32 %x, -16\n  ret i32 %0\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %m = and i32 %x, 255\n  %a = shl i32 %m, 4\n  %r = lshr i32 %a, 2\n  ret i32 %r\n",
        "  %0 = shl i32 %x, 2\n  %1 = and i32 %0, 1073741823\n  %2 = and i32 %1, 1020\n  ret i32 %2\n",
    );
}

/// `commonShiftTransforms`: `(C << x) << K` is `(C << K) << x`; `C shift (a add
/// nuw K)` pre-shifts `C`; `C << (K - x)`, `C << (x ^ K)` become a right shift.
#[test]
fn a_constant_shifted_by_a_variable_is_shifted_first() {
    check(
        "i32",
        "i32 %x",
        "  %a = shl i32 3, %x\n  %r = shl i32 %a, 4\n  ret i32 %r\n",
        "  %0 = shl i32 48, %x\n  ret i32 %0\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %a = lshr i32 255, %x\n  %r = lshr i32 %a, 2\n  ret i32 %r\n",
        "  %0 = lshr i32 63, %x\n  ret i32 %0\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %a = add nuw i32 %x, 3\n  %r = shl i32 5, %a\n  ret i32 %r\n",
        "  %0 = shl i32 40, %x\n  ret i32 %0\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %a = add i32 %x, -2\n  %r = shl nuw i32 8, %a\n  ret i32 %r\n",
        "  %0 = shl nuw i32 2, %x\n  ret i32 %0\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %a = sub nuw i32 3, %x\n  %r = shl i32 1, %a\n  ret i32 %r\n",
        "  %0 = lshr exact i32 8, %x\n  ret i32 %0\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %m = and i32 %x, 3\n  %a = xor i32 %m, 3\n  %r = shl i32 5, %a\n  ret i32 %r\n",
        "  %m = and i32 %x, 3\n  %0 = lshr exact i32 40, %m\n  ret i32 %0\n",
    );
}

/// `foldShiftOfShiftedBinOp`: `((x << 2) bop y) << 3` is `(x << 5) bop (y <<
/// 3)`; a `sub` keeps its operand order.
#[test]
fn a_shift_of_a_binop_of_a_shift_distributes() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %a = shl i32 %x, 2\n  %b = and i32 %a, %y\n  %r = shl i32 %b, 3\n  ret i32 %r\n",
        "  %0 = shl i32 %x, 5\n  %1 = shl i32 %y, 3\n  %2 = and i32 %0, %1\n  ret i32 %2\n",
    );
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %a = shl i32 %x, 2\n  %b = add i32 %a, %y\n  %r = shl i32 %b, 3\n  ret i32 %r\n",
        "  %0 = shl i32 %x, 5\n  %1 = shl i32 %y, 3\n  %2 = add i32 %0, %1\n  ret i32 %2\n",
    );
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %a = shl i32 %x, 2\n  %b = sub i32 %y, %a\n  %r = shl i32 %b, 3\n  ret i32 %r\n",
        "  %0 = shl i32 %x, 5\n  %1 = shl i32 %y, 3\n  %2 = sub i32 %1, %0\n  ret i32 %2\n",
    );
}

/// `FoldShiftByConstant`: `(x | 12) << 2` is `(x << 2) | 48`; a select of such
/// a binop and its operand likewise.
#[test]
fn a_binop_with_a_constant_right_side_comes_out_of_the_shift() {
    check(
        "i32",
        "i32 %x",
        "  %a = or i32 %x, 12\n  %r = shl i32 %a, 2\n  ret i32 %r\n",
        "  %0 = shl i32 %x, 2\n  %1 = or i32 %0, 48\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %a = add i32 %x, 12\n  %r = shl i32 %a, 2\n  ret i32 %r\n",
        "  %0 = shl i32 %x, 2\n  %1 = add i32 %0, 48\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i1 %c, i32 %x",
        "  %a = add i32 %x, 5\n  %s = select i1 %c, i32 %a, i32 %x\n  %r = shl i32 %s, 2\n  ret i32 %r\n",
        "  %0 = shl i32 %x, 2\n  %1 = add i32 %0, 20\n  %2 = select i1 %c, i32 %1, i32 %0\n  ret i32 %2\n",
    );
}

/// `FoldShiftByConstant`: `(x sdiv 7) >>u 31` is `zext (x <= -7)`, `(x sdiv -7)
/// >>s 31` is `sext (x >= 7)` (`opt` writes the compare in its canonical form).
#[test]
fn a_signed_division_sign_bit_is_a_compare() {
    check(
        "i32",
        "i32 %x",
        "  %d = sdiv i32 %x, 7\n  %r = lshr i32 %d, 31\n  ret i32 %r\n",
        "  %0 = icmp sle i32 %x, -7\n  %1 = zext i1 %0 to i32\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %d = sdiv i32 %x, -7\n  %r = ashr i32 %d, 31\n  ret i32 %r\n",
        "  %0 = icmp sge i32 %x, 7\n  %1 = sext i1 %0 to i32\n  ret i32 %1\n",
    );
}

/// `canEvaluateShifted` and `getShiftedValue`: `((x << 8) | (y << 8)) >>u 8` is
/// evaluated shifted (`opt` folds the masks together); `(mul x, -16) >>u 4` is
/// `(neg x) & mask`.
#[test]
fn a_shift_through_a_bitwise_tree_is_evaluated_shifted() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %a = shl i32 %x, 8\n  %b = shl i32 %y, 8\n  %o = or i32 %a, %b\n  %r = lshr i32 %o, 8\n  ret i32 %r\n",
        "  %0 = and i32 %x, 16777215\n  %1 = and i32 %y, 16777215\n  %o = or i32 %0, %1\n  ret i32 %o\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %a = mul i32 %x, -16\n  %r = lshr i32 %a, 4\n  ret i32 %r\n",
        "  %0 = sub i32 0, %x\n  %1 = and i32 %0, 268435455\n  ret i32 %1\n",
    );
}

/// `visitShl`: `shl (zext X), C` is `zext (shl X, C)` where X has the high bits
/// to shift out zero.
#[test]
fn shl_of_a_zext_with_zeros_to_shift_out_is_a_zext_of_a_shl() {
    check(
        "i32",
        "i8 %x",
        "  %m = and i8 %x, 15\n  %z = zext i8 %m to i32\n  %r = shl i32 %z, 4\n  ret i32 %r\n",
        "  %0 = shl i8 %x, 4\n  %1 = zext i8 %0 to i32\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i8 %x",
        "  %z = zext i8 %x to i32\n  %r = shl i32 %z, 3\n  ret i32 %r\n",
        "  %z = zext i8 %x to i32\n  %r = shl nuw nsw i32 %z, 3\n  ret i32 %r\n",
    );
}

/// `visitShl`: `(x >>exact C1) << C` is `x << (C - C1)` or `x >>exact (C1 -
/// C)`.
#[test]
fn shl_of_an_exact_shr_by_constants_is_one_shift() {
    check(
        "i32",
        "i32 %x",
        "  %a = lshr exact i32 %x, 2\n  %r = shl i32 %a, 5\n  ret i32 %r\n",
        "  %0 = shl i32 %x, 3\n  ret i32 %0\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %a = ashr exact i32 %x, 5\n  %r = shl i32 %a, 2\n  ret i32 %r\n",
        "  %0 = ashr exact i32 %x, 3\n  ret i32 %0\n",
    );
}

/// `visitShl`: `(x >> C1) << C` is a shift and `and (-1 << C)`.
#[test]
fn shl_of_a_shr_by_constants_is_a_shift_and_a_mask() {
    check(
        "i32",
        "i32 %x",
        "  %a = lshr i32 %x, 2\n  %r = shl i32 %a, 5\n  ret i32 %r\n",
        "  %0 = shl i32 %x, 3\n  %1 = and i32 %0, -32\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %a = lshr i32 %x, 5\n  %r = shl i32 %a, 2\n  ret i32 %r\n",
        "  %0 = lshr i32 %x, 3\n  %1 = and i32 %0, -4\n  ret i32 %1\n",
    );
    check(
        "i16",
        "i32 %x",
        "  %a = lshr i32 %x, 9\n  %t = trunc i32 %a to i16\n  %r = shl i16 %t, 3\n  ret i16 %r\n",
        "  %0 = lshr i32 %x, 6\n  %1 = trunc i32 %0 to i16\n  %2 = and i16 %1, -8\n  ret i16 %2\n",
    );
}

/// `visitShl`: `((x >> C) bop y) << C` is `(x bop (y << C)) & (-1 << C)`, with
/// a mask in the shr `(x & (CC << C)) bop (y << C)`; `(C1 - x) << C` is `(C1 <<
/// C) - (x << C)`.
#[test]
fn shl_of_a_binop_of_a_shr_moves_the_shift_into_the_binop() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %s = lshr i32 %x, 4\n  %b = or i32 %y, %s\n  %r = shl i32 %b, 4\n  ret i32 %r\n",
        "  %0 = shl i32 %y, 4\n  %1 = or i32 %x, %0\n  %2 = and i32 %1, -16\n  ret i32 %2\n",
    );
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %s = lshr i32 %x, 4\n  %m = and i32 %s, 255\n  %b = xor i32 %m, %y\n  %r = shl i32 %b, 4\n  ret i32 %r\n",
        "  %0 = shl i32 %y, 4\n  %1 = and i32 %x, 4080\n  %2 = xor i32 %1, %0\n  ret i32 %2\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %a = sub i32 10, %x\n  %r = shl i32 %a, 3\n  ret i32 %r\n",
        "  %0 = shl i32 %x, 3\n  %1 = sub i32 80, %0\n  ret i32 %1\n",
    );
}

/// `visitShl`: `(x >> y) << y` is `x & (-1 << y)`; `(-1 >>u y) << y` is `-1 <<
/// y`.
#[test]
fn shl_of_a_shr_by_the_same_variable_is_a_mask() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %a = lshr i32 %x, %y\n  %r = shl i32 %a, %y\n  ret i32 %r\n",
        "  %0 = shl nsw i32 -1, %y\n  %1 = and i32 %0, %x\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i32 %y",
        "  %a = lshr i32 -1, %y\n  %r = shl i32 %a, %y\n  ret i32 %r\n",
        "  %0 = shl nsw i32 -1, %y\n  %1 = and i32 %0, -1\n  ret i32 %1\n",
    );
}

/// `visitShl`: `(x * C2) << C1` is `x * (C2 << C1)`; `shl (zext i1 x), C` is a
/// select; `1 << (31 - x)` is `signmask >>u x`.
#[test]
fn shl_of_a_mul_or_a_bool_or_a_one_is_simpler() {
    check(
        "i32",
        "i32 %x",
        "  %a = mul i32 %x, 5\n  %r = shl i32 %a, 3\n  ret i32 %r\n",
        "  %0 = mul i32 %x, 40\n  ret i32 %0\n",
    );
    check(
        "i32",
        "i1 %x",
        "  %z = zext i1 %x to i32\n  %r = shl i32 %z, 4\n  ret i32 %r\n",
        "  %0 = select i1 %x, i32 16, i32 0\n  ret i32 %0\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %a = sub i32 31, %x\n  %r = shl i32 1, %a\n  ret i32 %r\n",
        "  %0 = lshr exact i32 -2147483648, %x\n  ret i32 %0\n",
    );
}

/// `dropRedundantMaskingOfLeftShiftInput`: a mask no shift amount makes
/// redundant stays (`opt`'s `shl` of `and 255` is `SimplifyDemandedBits`'s).
#[test]
fn shl_of_a_masked_value_keeps_the_mask_it_cannot_drop() {
    check(
        "i32",
        "i32 %x, i32 %n",
        "  %one = shl i32 1, %n\n  %m = add i32 %one, -1\n  %a = and i32 %x, %m\n  %r = shl i32 %a, 24\n  ret i32 %r\n",
        "  %one = shl nuw i32 1, %n\n  %m = add i32 %one, 255\n  %a = and i32 %x, %m\n  %r = shl i32 %a, 24\n  ret i32 %r\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %a = and i32 %x, 255\n  %r = shl i32 %a, 24\n  ret i32 %r\n",
        "  %r = shl i32 %x, 24\n  ret i32 %r\n",
    );
    check(
        "i32",
        "i32 %x, i32 %n",
        "  %a = shl i32 %x, %n\n  %b = lshr i32 %a, %n\n  %r = shl i32 %b, %n\n  ret i32 %r\n",
        "  %0 = shl i32 %x, %n\n  ret i32 %0\n",
    );
}

/// `visitLShr`: `lshr 1, x` is `zext (x == 0)`; `(~x) >>u 31` is `zext (x >
/// -1)`.
#[test]
fn lshr_of_one_is_a_compare_and_a_zext() {
    check(
        "i32",
        "i32 %x",
        "  %r = lshr i32 1, %x\n  ret i32 %r\n",
        "  %0 = icmp eq i32 %x, 0\n  %1 = zext i1 %0 to i32\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %n = xor i32 %x, -1\n  %r = lshr i32 %n, 31\n  ret i32 %r\n",
        "  %0 = icmp sgt i32 %x, -1\n  %1 = zext i1 %0 to i32\n  ret i32 %1\n",
    );
}

/// `visitLShr`: `((x <<nuw z) -nuw y) >>u exact z` is `x -nuw (y >>u exact z)`,
/// and the same with the shl on the right.
#[test]
fn lshr_exact_of_a_nuw_sub_of_a_shl_distributes() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %s = shl nuw i32 %x, 3\n  %d = sub nuw i32 %s, %y\n  %r = lshr exact i32 %d, 3\n  ret i32 %r\n",
        "  %0 = lshr exact i32 %y, 3\n  %1 = sub nuw i32 %x, %0\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %s = shl nuw i32 %y, 3\n  %d = sub nuw i32 %x, %s\n  %r = lshr exact i32 %d, 3\n  ret i32 %r\n",
        "  %0 = lshr exact i32 %x, 3\n  %1 = sub nuw i32 %0, %y\n  ret i32 %1\n",
    );
}

/// `visitLShr`: `(x + y) >>u 1` is `x & y` when both are at most 1; of two
/// zexts of bools, an `and` (`opt` ands the bools).
#[test]
fn lshr_of_the_sum_of_two_bits_is_an_and() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %a = and i32 %x, 1\n  %b = and i32 %y, 1\n  %s = add i32 %a, %b\n  %r = lshr i32 %s, 1\n  ret i32 %r\n",
        "  %a = and i32 %x, 1\n  %b = and i32 %y, 1\n  %0 = and i32 %a, %b\n  ret i32 %0\n",
    );
    check(
        "i32",
        "i1 %a, i1 %b",
        "  %x = zext i1 %a to i32\n  %y = zext i1 %b to i32\n  %s = add i32 %x, %y\n  %r = lshr i32 %s, 1\n  ret i32 %r\n",
        "  %x = zext i1 %a to i32\n  %y = zext i1 %b to i32\n  %0 = and i32 %x, %y\n  ret i32 %0\n",
    );
}

/// `visitLShr`: `((x <<nuw z) binop nuw y) >>u z` is `x binop nuw (y >>u z)`.
#[test]
fn lshr_of_a_nuw_shl_binop_moves_the_shift_to_the_other_operand() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %s = shl nuw i32 %x, 3\n  %a = add nuw i32 %s, %y\n  %r = lshr i32 %a, 3\n  ret i32 %r\n",
        "  %0 = lshr i32 %y, 3\n  %1 = add nuw i32 %0, %x\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %s = shl nuw i32 %x, 3\n  %a = or i32 %y, %s\n  %r = lshr i32 %a, 3\n  ret i32 %r\n",
        "  %0 = lshr i32 %y, 3\n  %1 = or i32 %0, %x\n  ret i32 %1\n",
    );
}

/// `visitLShr`: `(x <<nuw C1) >>u C` is one shift; without nuw a shift and a
/// mask; `((x << C) + y) >>u C` is `(x + (y >>u C)) & mask`.
#[test]
fn lshr_of_a_shl_by_constants_is_a_shift_or_a_mask() {
    check(
        "i32",
        "i32 %x",
        "  %a = shl nuw i32 %x, 2\n  %r = lshr i32 %a, 5\n  ret i32 %r\n",
        "  %0 = lshr i32 %x, 3\n  ret i32 %0\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %a = shl i32 %x, 2\n  %r = lshr i32 %a, 5\n  ret i32 %r\n",
        "  %0 = lshr i32 %x, 3\n  %1 = and i32 %0, 134217727\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %a = shl nuw i32 %x, 5\n  %r = lshr i32 %a, 2\n  ret i32 %r\n",
        "  %0 = shl nuw nsw i32 %x, 3\n  ret i32 %0\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %a = shl i32 %x, 5\n  %r = lshr i32 %a, 2\n  ret i32 %r\n",
        "  %0 = shl i32 %x, 3\n  %1 = and i32 %0, 1073741823\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %s = shl i32 %x, 4\n  %a = add i32 %y, %s\n  %r = lshr i32 %a, 4\n  ret i32 %r\n",
        "  %0 = lshr i32 %y, 4\n  %1 = add i32 %0, %x\n  %2 = and i32 %1, 268435455\n  ret i32 %2\n",
    );
}

/// `visitLShr`: `lshr (zext X), C` is `zext (lshr X, C)`; of a `sext i1` a
/// select; of a `sext` by the sign bit a `zext` of a shift (`opt` adds `nneg`).
#[test]
fn lshr_of_an_extension_is_an_extension_of_a_shift() {
    check(
        "i32",
        "i8 %x",
        "  %z = zext i8 %x to i32\n  %r = lshr i32 %z, 3\n  ret i32 %r\n",
        "  %0 = lshr i8 %x, 3\n  %1 = zext i8 %0 to i32\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i1 %x",
        "  %z = sext i1 %x to i32\n  %r = lshr i32 %z, 3\n  ret i32 %r\n",
        "  %0 = select i1 %x, i32 536870911, i32 0\n  ret i32 %0\n",
    );
    check(
        "i32",
        "i8 %x",
        "  %z = sext i8 %x to i32\n  %r = lshr i32 %z, 31\n  ret i32 %r\n",
        "  %0 = lshr i8 %x, 7\n  %1 = zext i8 %0 to i32\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i8 %x",
        "  %z = sext i8 %x to i32\n  %r = lshr i32 %z, 24\n  ret i32 %r\n",
        "  %0 = ashr i8 %x, 7\n  %1 = zext i8 %0 to i32\n  ret i32 %1\n",
    );
}

/// `visitLShr`: `or(x, -x) >>u 31` is `zext (x != 0)`; `(x -nsw y) >>u 31` is
/// `zext (x < y)`; `(x srem 2) >>u 31` is `(x >>u 31) & x`; `((x - 1) & ~x) >>u
/// 31` is `zext (x == 0)`.
#[test]
fn lshr_by_width_less_one_of_a_sign_test_is_a_compare() {
    check(
        "i32",
        "i32 %x",
        "  %n = sub i32 0, %x\n  %o = or i32 %n, %x\n  %r = lshr i32 %o, 31\n  ret i32 %r\n",
        "  %0 = icmp ne i32 %x, 0\n  %1 = zext i1 %0 to i32\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %d = sub nsw i32 %x, %y\n  %r = lshr i32 %d, 31\n  ret i32 %r\n",
        "  %0 = icmp slt i32 %x, %y\n  %1 = zext i1 %0 to i32\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %d = srem i32 %x, 2\n  %r = lshr i32 %d, 31\n  ret i32 %r\n",
        "  %0 = lshr i32 %x, 31\n  %1 = and i32 %0, %x\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %a = add i32 %x, -1\n  %n = xor i32 %x, -1\n  %m = and i32 %a, %n\n  %r = lshr i32 %m, 31\n  ret i32 %r\n",
        "  %0 = icmp eq i32 %x, 0\n  %1 = zext i1 %0 to i32\n  ret i32 %1\n",
    );
}

/// `visitLShr`: `(trunc (x >>u C1)) >>u C` is `trunc (x >>u (C1 + C))` and a
/// mask unless the first shift covers the truncated bits (`opt` then drops the
/// mask).
#[test]
fn lshr_of_a_trunc_of_a_lshr_is_one_shift() {
    check(
        "i16",
        "i32 %x",
        "  %a = lshr i32 %x, 4\n  %t = trunc i32 %a to i16\n  %r = lshr i16 %t, 3\n  ret i16 %r\n",
        "  %0 = lshr i32 %x, 7\n  %1 = trunc i32 %0 to i16\n  %2 = and i16 %1, 8191\n  ret i16 %2\n",
    );
    check(
        "i16",
        "i32 %x",
        "  %a = lshr i32 %x, 20\n  %t = trunc i32 %a to i16\n  %r = lshr i16 %t, 3\n  ret i16 %r\n",
        "  %0 = lshr i32 %x, 23\n  %1 = trunc i32 %0 to i16\n  %2 = and i16 %1, 8191\n  ret i16 %2\n",
    );
}

/// `visitLShr`: `lshr (mul nuw x, 2^N+1), N` is `x` for the splat, else `add
/// nuw x, (x >>u N)`; a mul whose constant is divisible by `1 << C` is a
/// smaller mul; the `nsw` form is `add nsw`.
#[test]
fn lshr_of_a_mul_by_a_power_of_two_plus_one_is_an_add() {
    check("i16", "i16 %x", "  %m = mul nuw i16 %x, 257\n  %r = lshr i16 %m, 8\n  ret i16 %r\n", "  ret i16 %x\n");
    check(
        "i32",
        "i32 %x",
        "  %m = mul nuw i32 %x, 17\n  %r = lshr i32 %m, 4\n  ret i32 %r\n",
        "  %0 = lshr i32 %x, 4\n  %1 = add nuw i32 %x, %0\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %m = mul nuw i32 %x, 24\n  %r = lshr i32 %m, 3\n  ret i32 %r\n",
        "  %0 = mul nuw nsw i32 %x, 3\n  ret i32 %0\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %m = mul nsw i32 %x, 9\n  %r = lshr i32 %m, 3\n  ret i32 %r\n",
        "  %0 = lshr i32 %x, 3\n  %1 = add nsw i32 %x, %0\n  ret i32 %1\n",
    );
}

/// `visitLShr`: `(x << y) >>u y` is `x & (-1 >>u y)`; `(-1 << y) >>u y` is `-1
/// >>u y`.
#[test]
fn lshr_of_a_shl_by_the_same_variable_is_a_mask() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %a = shl i32 %x, %y\n  %r = lshr i32 %a, %y\n  ret i32 %r\n",
        "  %0 = lshr i32 -1, %y\n  %1 = and i32 %0, %x\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i32 %y",
        "  %a = shl i32 -1, %y\n  %r = lshr i32 %a, %y\n  ret i32 %r\n",
        "  %0 = lshr i32 -1, %y\n  %1 = and i32 %0, -1\n  ret i32 %1\n",
    );
}

/// `foldLShrOverflowBit`: `lshr (add (zext x), (zext y)), 16` is `zext (icmp
/// ult (add x, y), x)` (`opt` writes the compare as `y u> ~x`).
#[test]
fn lshr_of_a_widened_add_is_its_carry() {
    check(
        "i32",
        "i16 %x, i16 %y",
        "  %a = zext i16 %x to i32\n  %b = zext i16 %y to i32\n  %s = add i32 %a, %b\n  %r = lshr i32 %s, 16\n  ret i32 %r\n",
        "  %0 = add i16 %x, %y\n  %1 = icmp ult i16 %0, %x\n  %2 = zext i1 %1 to i32\n  ret i32 %2\n",
    );
}

/// `visitAShr`: `ashr (shl (zext X), C), C` is `sext X` when C is the width
/// difference.
#[test]
fn ashr_of_a_shl_of_a_zext_is_a_sext() {
    check(
        "i32",
        "i8 %x",
        "  %z = zext i8 %x to i32\n  %a = shl i32 %z, 24\n  %r = ashr i32 %a, 24\n  ret i32 %r\n",
        "  %0 = sext i8 %x to i32\n  ret i32 %0\n",
    );
}

/// `visitAShr`: `(x <<nsw C1) >>s C2` is `x >>s (C2 - C1)` or `x <<nsw (C1 -
/// C2)`; `(x >>s C1) >>s C2` is `x >>s (C1 + C2)`.
#[test]
fn ashr_of_an_nsw_shl_is_one_shift() {
    check(
        "i32",
        "i32 %x",
        "  %a = shl nsw i32 %x, 3\n  %r = ashr i32 %a, 5\n  ret i32 %r\n",
        "  %0 = ashr i32 %x, 2\n  ret i32 %0\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %a = shl nsw i32 %x, 5\n  %r = ashr i32 %a, 3\n  ret i32 %r\n",
        "  %0 = shl nsw i32 %x, 2\n  ret i32 %0\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %a = ashr i32 %x, 20\n  %r = ashr i32 %a, 20\n  ret i32 %r\n",
        "  %0 = ashr i32 %x, 31\n  ret i32 %0\n",
    );
}

/// `visitAShr`: `ashr (sext X), C` is `sext (ashr X, C')`.
#[test]
fn ashr_of_a_sext_is_a_sext_of_an_ashr() {
    check(
        "i32",
        "i8 %x",
        "  %z = sext i8 %x to i32\n  %r = ashr i32 %z, 3\n  ret i32 %r\n",
        "  %0 = ashr i8 %x, 3\n  %1 = sext i8 %0 to i32\n  ret i32 %1\n",
    );
}

/// `visitAShr`: `or(x, -x) >>s 31` is `sext (x != 0)`; `(x -nsw y) >>s 31` is
/// `sext (x < y)`; `((x - 1) & ~x) >>s 31` is `sext (x == 0)`.
#[test]
fn ashr_by_width_less_one_of_a_sign_test_is_a_sext_of_a_compare() {
    check(
        "i32",
        "i32 %x",
        "  %n = sub i32 0, %x\n  %o = or i32 %n, %x\n  %r = ashr i32 %o, 31\n  ret i32 %r\n",
        "  %0 = icmp ne i32 %x, 0\n  %1 = sext i1 %0 to i32\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %d = sub nsw i32 %x, %y\n  %r = ashr i32 %d, 31\n  ret i32 %r\n",
        "  %0 = icmp slt i32 %x, %y\n  %1 = sext i1 %0 to i32\n  ret i32 %1\n",
    );
    check(
        "i32",
        "i32 %x",
        "  %a = add i32 %x, -1\n  %n = xor i32 %x, -1\n  %m = and i32 %a, %n\n  %r = ashr i32 %m, 31\n  ret i32 %r\n",
        "  %0 = icmp eq i32 %x, 0\n  %1 = sext i1 %0 to i32\n  ret i32 %1\n",
    );
}

/// `visitAShr`: `ashr (mul nsw x, 2^N+1), N` is `add nsw x, (x >>s N)`.
#[test]
fn ashr_of_an_nsw_mul_is_an_add() {
    check(
        "i32",
        "i32 %x",
        "  %m = mul nsw i32 %x, 9\n  %r = ashr i32 %m, 3\n  ret i32 %r\n",
        "  %0 = ashr i32 %x, 3\n  %1 = add nsw i32 %x, %0\n  ret i32 %1\n",
    );
}

/// `visitAShr`: `(x << 31) >>s 31` is `-(x & 1)`.
#[test]
fn ashr_splat_of_the_low_bit_is_a_negated_and() {
    check(
        "i32",
        "i32 %x",
        "  %a = shl i32 %x, 31\n  %r = ashr i32 %a, 31\n  ret i32 %r\n",
        "  %0 = and i32 %x, 1\n  %1 = sub i32 0, %0\n  ret i32 %1\n",
    );
}

/// `visitAShr`: a signed shift of a value whose sign bit is zero is unsigned.
#[test]
fn ashr_of_a_value_with_a_clear_sign_is_a_lshr() {
    check(
        "i32",
        "i32 %x",
        "  %a = and i32 %x, 2147483647\n  %r = ashr i32 %a, %x\n  ret i32 %r\n",
        "  %a = and i32 %x, 2147483647\n  %0 = lshr i32 %a, %x\n  ret i32 %0\n",
    );
}

/// `visitAShr`: `ashr (~x), y` is `~(ashr x, y)`.
#[test]
fn ashr_of_a_not_is_a_not_of_an_ashr() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %n = xor i32 %x, -1\n  %r = ashr i32 %n, %y\n  ret i32 %r\n",
        "  %0 = ashr i32 %x, %y\n  %1 = xor i32 %0, -1\n  ret i32 %1\n",
    );
}

/// `foldVariableSignZeroExtensionOfVariableHighBitExtract`: the sign extension
/// of `Nbits` of `v` by shifts is one shift where the shift amounts agree.
#[test]
fn ashr_of_a_variable_sign_extension_of_a_high_bit_extract() {
    check(
        "i8",
        "i32 %v, i32 %n",
        "  %s = sub i32 32, %n\n  %a = shl i32 %v, %s\n  %r0 = ashr i32 %a, %s\n  %r = trunc i32 %r0 to i8\n  ret i8 %r\n",
        "  %s = sub i32 32, %n\n  %a = shl i32 %v, %s\n  %r0 = ashr exact i32 %a, %s\n  %r = trunc i32 %r0 to i8\n  ret i8 %r\n",
    );
}
/// `FoldShiftByConstant`: a `not` under a logical shift stays a `not` (the
/// shift of the xor would be a plain `xor`).
#[test]
fn a_not_under_a_logical_shift_is_left_alone() {
    let text = "  %a = xor i32 %x, -1
  %r = lshr i32 %a, 2
  ret i32 %r
";
    check("i32", "i32 %x", text, text);
}

const CTLZ: &str = "declare i32 @llvm.ctlz.i32(i32, i1)\n";
const CTTZ: &str = "declare i32 @llvm.cttz.i32(i32, i1)\n";
const CTPOP: &str = "declare i32 @llvm.ctpop.i32(i32)\n";
const PURE: &str = "nocallback nofree nosync nounwind speculatable willreturn memory(none)";

/// The declaration as the printed module shows it, with `PURE`.
fn printed_declaration(declared: &str) -> String {
    let (head, rest) = declared.trim_end().split_once('(').expect("a declaration");
    let rest = rest.replace("i1)", "i1 immarg)");
    format!("{head}({rest} {PURE}\n\n")
}

/// [`check`] of a function with the call it makes of one intrinsic, which the
/// combine leaves (a call is never trivially dead here).
fn check_calling(
    declared: &str,
    params: &str,
    body: &str,
    after: &str,
) {
    check_declared(declared, &printed_declaration(declared), "i32", params, body, after);
}

/// `visitLShr`: a count shifted right by the log of the width is a compare:
/// `ctlz.i32(x) >> 5` and `cttz` are `zext (x == 0)`, `ctpop` is `zext (x ==
/// -1)`.
#[test]
fn lshr_of_a_bit_count_by_the_log_of_the_width_is_a_compare() {
    for (declared, name, flag, constant) in
        [(CTLZ, "ctlz", ", i1 false", 0), (CTTZ, "cttz", ", i1 false", 0), (CTPOP, "ctpop", "", -1)]
    {
        let call = format!("  %c = call i32 @llvm.{name}.i32(i32 %x{flag})\n");
        check_calling(
            declared,
            "i32 %x",
            &format!("{call}  %r = lshr i32 %c, 5\n  ret i32 %r\n"),
            &format!("{call}  %0 = icmp eq i32 %x, {constant}\n  %1 = zext i1 %0 to i32\n  ret i32 %1\n"),
        );
    }
}

/// `visitShl`: `1 << cttz x` is the lowest set bit, `-x & x`; `y << cttz x`
/// is `(x & -x) * y`, `nuw` kept (`opt` agrees).
#[test]
fn shl_by_a_cttz_is_a_multiply_by_the_lowest_set_bit() {
    let call = "  %c = call i32 @llvm.cttz.i32(i32 %x, i1 false)\n";
    check_calling(
        CTTZ,
        "i32 %x",
        &format!("{call}  %r = shl i32 1, %c\n  ret i32 %r\n"),
        &format!("{call}  %0 = sub i32 0, %x\n  %1 = and i32 %0, %x\n  ret i32 %1\n"),
    );
    check_calling(
        CTTZ,
        "i32 %x, i32 %y",
        &format!("{call}  %r = shl nuw i32 %y, %c\n  ret i32 %r\n"),
        &format!("{call}  %0 = sub i32 0, %x\n  %1 = and i32 %0, %x\n  %2 = mul nuw i32 %1, %y\n  ret i32 %2\n"),
    );
}

/// `visitLShr`: `(pow2 << x) >>u cttz (pow2 << y)` is `(1 << x) >>u y`.
#[test]
fn lshr_by_the_cttz_of_a_shifted_power_of_two_is_a_shift_by_the_amount() {
    check_calling(
        CTTZ,
        "i32 %x, i32 %y",
        "  %a = shl nuw i32 1, %x\n  %b = shl nuw i32 1, %y\n  %c = call i32 @llvm.cttz.i32(i32 %b, i1 false)\n  %r = lshr i32 %a, %c\n  ret i32 %r\n",
        "  %b = shl nuw i32 1, %y\n  %c = call i32 @llvm.cttz.i32(i32 %b, i1 false)\n  %0 = shl nuw i32 1, %x\n  %1 = lshr i32 %0, %y\n  ret i32 %1\n",
    );
}

/// A shift of a select by a constant folds into both arms
/// (`foldBinOpIntoSelectOrPhi`).
#[test]
fn a_select_shifted_by_a_constant_is_a_select_of_shifts() {
    let out = combined(
        "define i16 @f(i1 %c) {\nb0:\n  %s = select i1 %c, i16 3, i16 5\n  %r = shl i16 %s, 2\n  ret i16 %r\n}\n",
    );
    assert!(out.contains("select i1 %c, i16 12, i16 20"), "{out}");
}

/// A constant shifted by a select of amounts folds into both arms
/// (`FoldOpIntoSelect`).
#[test]
fn a_constant_shifted_by_a_select_is_a_select_of_shifts() {
    let out = combined(
        "define i16 @f(i1 %c) {\nb0:\n  %s = select i1 %c, i16 1, i16 3\n  %r = shl i16 1, %s\n  ret i16 %r\n}\n",
    );
    assert!(out.contains("select i1 %c, i16 2, i16 8"), "{out}");
}

/// Two phis shifted by each other: a constant pair for one predecessor folds
/// and the shift moves to the other (`foldBinopWithPhiOperands`).
#[test]
fn a_shift_of_two_phis_with_a_constant_pair_moves_to_the_other_predecessor() {
    let out = combined(
        "define i16 @f(i1 %c, i16 %i, i16 %j) {
b0:
  br i1 %c, label %a, label %b

a:
  br label %m

b:
  br label %m

m:
  %p = phi i16 [ 3, %a ], [ %i, %b ]
  %q = phi i16 [ 1, %a ], [ %j, %b ]
  %r = shl i16 %p, %q
  ret i16 %r
}
",
    );
    assert!(out.contains("shl i16 %i, %j") && out.contains("[ 6, %a ]"), "{out}");
}

/// easing.nib: `lshr (zext i8 X), 15` became a poison `lshr i8 X, 15`; the
/// pin makes it 0 (a shift past the source width leaves only zeros).
#[test]
fn a_shift_of_a_zext_past_the_source_width_is_zero() {
    let out = combined(
        "define i16 @f(i8 %x) {
b0:
  %z = zext i8 %x to i16
  %r = lshr i16 %z, 15
  ret i16 %r
}
",
    );
    assert!(out.contains("ret i16 0"), "{out}");
}

/// `@f` after the visits alone, the intrinsics the fold calls declared.
fn combined_declaring(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    crate::combine::combine_with_declared(
        context,
        &globals,
        &metadata,
        &DataLayout::default(),
        function,
        visit,
        &mut declared,
    );
    declared.place(&mut module).unwrap();
    printed(&module)
}

/// `(bswap (zext X)) >> C` with a shift smaller than the extension is `(zext
/// (bswap X)) << C'`. Stayed a 32-bit `bswap` before.
#[test]
fn a_shifted_bswap_of_a_zext_is_a_narrow_bswap() {
    let after = combined_declaring(
        "declare i32 @llvm.bswap.i32(i32)

define i32 @f(i16 %x) {
b0:
  %z = zext i16 %x to i32
  %s = call i32 @llvm.bswap.i32(i32 %z)
  %r = lshr i32 %s, 8
  ret i32 %r
}
",
    );
    assert!(after.contains("call i16 @llvm.bswap.i16(i16 %x)"), "{after}");
    assert!(after.contains("shl nuw nsw i32 %1, 8"), "{after}");
}
