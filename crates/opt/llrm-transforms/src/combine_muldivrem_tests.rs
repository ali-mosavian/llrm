use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::combine::{Combiner, Visit, combine_with};
use crate::combine_muldivrem::{visit_mul, visit_sdiv, visit_srem, visit_udiv, visit_urem};
use crate::testing::{parsed, printed};

/// The mul, div and rem visitors alone, through the driver.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    match c.function.instruction(inst).opcode {
        Opcode::Binary(BinaryOp::Mul) => visit_mul(c, inst),
        Opcode::Binary(BinaryOp::UDiv) => visit_udiv(c, inst),
        Opcode::Binary(BinaryOp::SDiv) => visit_sdiv(c, inst),
        Opcode::Binary(BinaryOp::URem) => visit_urem(c, inst),
        Opcode::Binary(BinaryOp::SRem) => visit_srem(c, inst),
        _ => Visit::None,
    }
}

/// `@f` after the visitors alone.
fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// `define RET @f(PARAMS)` with `body` before and `after` after the visitors.
fn check(
    ret: &str,
    params: &str,
    body: &str,
    after: &str,
) {
    let text = format!("define {ret} @f({params}) {{\nb0:\n{body}}}\n");
    let want = format!("define {ret} @f({params}) {{\nb0:\n{after}}}\n");
    assert_eq!(combined(&text), want);
}

/// `X * -1` is `0 - X`, `nsw` kept (`opt` agrees).
#[test]
fn a_multiply_by_minus_one_is_a_negation() {
    check("i16", "i16 %x", "  %r = mul nsw i16 %x, -1\n  ret i16 %r\n", "  %0 = sub nsw i16 0, %x\n  ret i16 %0\n");
}

/// `(X << C2) * C1` is `X * (C1 << C2)`, flags kept when both have them
/// (`opt` agrees).
#[test]
fn a_shift_then_a_multiply_is_one_multiply() {
    check(
        "i16",
        "i16 %x",
        "  %s = shl nuw nsw i16 %x, 2\n  %r = mul nuw nsw i16 %s, 3\n  ret i16 %r\n",
        "  %0 = mul nuw nsw i16 %x, 12\n  ret i16 %0\n",
    );
}

/// `X * 2^C` is `X << C`; `nsw` is dropped for `INT_MIN` (`opt` agrees).
#[test]
fn a_multiply_by_a_power_of_two_is_a_shift() {
    check(
        "i16",
        "i16 %x",
        "  %r = mul nuw nsw i16 %x, 8\n  ret i16 %r\n",
        "  %0 = shl nuw nsw i16 %x, 3\n  ret i16 %0\n",
    );
    check("i16", "i16 %x", "  %r = mul nsw i16 %x, -32768\n  ret i16 %r\n", "  %0 = shl i16 %x, 15\n  ret i16 %0\n");
}

/// `(X >>exact N) * (2^N + 1)` is `X + (X >>exact N)`; with `nuw` an `ashr`
/// becomes an `lshr` (`opt` agrees).
#[test]
fn an_exact_shift_times_a_power_of_two_plus_one_is_an_add() {
    check(
        "i16",
        "i16 %x",
        "  %s = lshr exact i16 %x, 2\n  %r = mul nsw i16 %s, 5\n  ret i16 %r\n",
        "  %s = lshr exact i16 %x, 2\n  %0 = add nsw i16 %x, %s\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %s = ashr exact i16 %x, 2\n  %r = mul nuw i16 %s, 5\n  ret i16 %r\n",
        "  %0 = lshr exact i16 %x, 2\n  %1 = add nuw i16 %x, %0\n  ret i16 %1\n",
    );
}

/// `(zext X) * (-1 << C)` is `(zext (-X)) << C` when the shift clears the
/// extended bits (`opt` agrees, and adds `nuw`).
/// Unverified against `opt` where it differs: opt also puts `nuw` on the `shl`
/// (visitShl's setShiftFlags runs after this visitor).
#[test]
fn an_extend_times_a_negated_power_of_two_is_a_negation_and_a_shift() {
    check(
        "i16",
        "i8 %x",
        "  %z = zext i8 %x to i16\n  %r = mul i16 %z, -256\n  ret i16 %r\n",
        "  %0 = sub i8 0, %x\n  %1 = zext i8 %0 to i16\n  %2 = shl i16 %1, 8\n  ret i16 %2\n",
    );
}

/// `mul (select C, 1, -1), X` is `select C, X, -X`, the negation `nsw` when
/// the multiply has a wrap flag (`opt` agrees on both).
#[test]
fn a_multiply_by_a_select_of_one_and_minus_one_is_a_select_of_the_negation() {
    check(
        "i16",
        "i1 %c, i16 %x",
        "  %s = select i1 %c, i16 1, i16 -1\n  %r = mul i16 %x, %s\n  ret i16 %r\n",
        "  %0 = sub i16 0, %x\n  %1 = select i1 %c, i16 %x, i16 %0\n  ret i16 %1\n",
    );
    check(
        "i16",
        "i1 %c, i16 %x",
        "  %s = select i1 %c, i16 -1, i16 1\n  %r = mul nsw i16 %s, %x\n  ret i16 %r\n",
        "  %0 = sub nsw i16 0, %x\n  %1 = select i1 %c, i16 %0, i16 %x\n  ret i16 %1\n",
    );
}

/// `(X << C1) * (select C, C2, C3)` is `X * (select C, C2 << C1, C3 << C1)`
/// (`opt` agrees).
#[test]
fn a_shifted_value_times_a_select_of_constants_scales_the_constants() {
    check(
        "i16",
        "i1 %c, i16 %x",
        "  %a = shl i16 %x, 2\n  %s = select i1 %c, i16 3, i16 5\n  %r = mul i16 %a, %s\n  ret i16 %r\n",
        "  %0 = select i1 %c, i16 12, i16 20\n  %1 = mul i16 %x, %0\n  ret i16 %1\n",
    );
}

/// `(X + C1) * C` is `X * C + C1 * C`, `nuw` when both have it; the same for
/// a disjoint `or` (`opt` agrees).
#[test]
fn an_add_times_a_constant_distributes() {
    check(
        "i16",
        "i16 %x",
        "  %a = add i16 %x, 7\n  %r = mul i16 %a, 3\n  ret i16 %r\n",
        "  %0 = mul i16 %x, 3\n  %1 = add i16 %0, 21\n  ret i16 %1\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %a = or disjoint i16 %x, 7\n  %r = mul nuw i16 %a, 3\n  ret i16 %r\n",
        "  %0 = mul nuw i16 %x, 3\n  %1 = add nuw i16 %0, 21\n  ret i16 %1\n",
    );
}

/// `@f` after the visitors alone, not verified: MIR declares no `abs`.
fn combined_unverified(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    llrm_mir::print::module(&module)
}

/// `define RET @f(i16 %x)` calling `abs` first, with `body` after it.
fn with_abs(body: &str) -> String {
    format!(
        "define i16 @f(i16 %x) {{\nb0:\n  %a = call i16 @llvm.abs.i16(i16 %x, i1 true)\n{body}}}\n\ndeclare i16 @llvm.abs.i16(i16, i1 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)\n"
    )
}

/// `abs(X) * abs(X)` is `X * X` (`opt` agrees; the call stays because the
/// driver never deletes one).
#[test]
fn an_abs_times_itself_is_a_square() {
    let got = combined_unverified(&with_abs("  %r = mul i16 %a, %a\n  ret i16 %r\n"));
    assert_eq!(got, with_abs("  %0 = mul i16 %x, %x\n  ret i16 %0\n"));
}

/// `abs(X) / X` is `X > -1 ? 1 : -1` (`opt` agrees).
#[test]
fn an_abs_divided_by_its_argument_is_a_sign_select() {
    let got = combined_unverified(&with_abs("  %r = sdiv i16 %a, %x\n  ret i16 %r\n"));
    assert_eq!(got, with_abs("  %0 = icmp sgt i16 %x, -1\n  %1 = select i1 %0, i16 1, i16 -1\n  ret i16 %1\n"));
}

/// `-X * C` is `X * -C`, `-X * -Y` is `X * Y` (`nsw` if all three have it),
/// `-X * Y` is `-(X * Y)` (`opt` agrees on all three).
#[test]
fn a_negated_factor_moves_out_of_the_multiply() {
    check(
        "i16",
        "i16 %x",
        "  %n = sub i16 0, %x\n  %r = mul i16 %n, 5\n  ret i16 %r\n",
        "  %0 = mul i16 %x, -5\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x, i16 %y",
        "  %n = sub nsw i16 0, %x\n  %m = sub nsw i16 0, %y\n  %r = mul nsw i16 %n, %m\n  ret i16 %r\n",
        "  %0 = mul nsw i16 %x, %y\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x, i16 %y",
        "  %n = sub i16 0, %x\n  %r = mul i16 %n, %y\n  ret i16 %r\n",
        "  %0 = mul i16 %x, %y\n  %1 = sub i16 0, %0\n  ret i16 %1\n",
    );
}

/// `(X /exact C0) * C1` is `X /exact (C0 / C1)` (`opt` goes on to an `lshr`).
#[test]
fn an_exact_division_times_a_divisor_multiple_is_one_division() {
    check(
        "i16",
        "i16 %x",
        "  %d = udiv exact i16 %x, 6\n  %r = mul i16 %d, 3\n  ret i16 %r\n",
        "  %0 = lshr exact i16 %x, 1\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %d = sdiv exact i16 %x, 6\n  %r = mul i16 %d, 3\n  ret i16 %r\n",
        "  %0 = ashr exact i16 %x, 1\n  ret i16 %0\n",
    );
}

/// `(X / Y) * Y` is `X - (X % Y)` on a frozen X; `(X / Y) * -Y` is `(X % Y) -
/// X`; an exact division leaves X (`opt` agrees on the first and the last).
/// Unverified against `opt` where it differs: opt also puts `nuw` on the `sub`
/// (visitSub) and, for the signed form, reassociates `x - (x + y)` (visitSub,
/// visitAdd): neither runs in this test.
#[test]
fn a_quotient_times_its_divisor_is_the_dividend_minus_the_remainder() {
    check(
        "i16",
        "i16 %x, i16 %y",
        "  %d = udiv i16 %x, %y\n  %r = mul i16 %d, %y\n  ret i16 %r\n",
        "  %0 = freeze i16 %x\n  %1 = urem i16 %0, %y\n  %2 = sub i16 %0, %1\n  ret i16 %2\n",
    );
    check(
        "i16",
        "i16 %x, i16 %y",
        "  %n = sub i16 0, %y\n  %d = sdiv i16 %x, %y\n  %m = mul i16 %d, %n\n  %r = add i16 %m, %n\n  ret i16 %r\n",
        "  %n = sub i16 0, %y\n  %0 = freeze i16 %x\n  %1 = srem i16 %0, %y\n  %2 = sub i16 %1, %0\n  %r = add i16 %2, %n\n  ret i16 %r\n",
    );
    check(
        "i16",
        "i16 %x, i16 %y",
        "  %d = udiv exact i16 %x, %y\n  %r = mul i16 %y, %d\n  ret i16 %r\n",
        "  ret i16 %x\n",
    );
}

/// `mul i1` and `(a & 1) * (b & 1)` are an `and` (`opt` agrees on the first;
/// it folds the second one step further).
/// Unverified against `opt` where it differs: for the second case opt merges
/// the two masks (visitAnd), which is not run here.
#[test]
fn a_multiply_of_booleans_is_an_and() {
    check("i1", "i1 %a, i1 %b", "  %r = mul i1 %a, %b\n  ret i1 %r\n", "  %0 = and i1 %a, %b\n  ret i1 %0\n");
    check(
        "i16",
        "i16 %a, i16 %b",
        "  %x = and i16 %a, 1\n  %y = and i16 %b, 1\n  %r = mul i16 %x, %y\n  ret i16 %r\n",
        "  %x = and i16 %a, 1\n  %y = and i16 %b, 1\n  %0 = and i16 %x, %y\n  ret i16 %0\n",
    );
}

/// `foldMulShl1`: `X * (1 << Z)` is `X << Z`; `X * ((1 << Z) + 1)` is `(X <<
/// Z) + X` and `X * ~(-1 << Z)` is `(X << Z) - X`, on a frozen X (`opt`
/// agrees on all three).
#[test]
fn a_multiply_by_a_shifted_one_is_a_shift() {
    check(
        "i16",
        "i16 %x, i16 %z",
        "  %s = shl nuw i16 1, %z\n  %r = mul nuw i16 %x, %s\n  ret i16 %r\n",
        "  %0 = shl nuw i16 %x, %z\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x, i16 %z",
        "  %s = shl nsw i16 1, %z\n  %a = add i16 %s, 1\n  %r = mul nsw i16 %x, %a\n  ret i16 %r\n",
        "  %0 = freeze i16 %x\n  %1 = shl nsw i16 %0, %z\n  %2 = add nsw i16 %1, %0\n  ret i16 %2\n",
    );
    check(
        "i16",
        "i16 %x, i16 %z",
        "  %s = shl i16 -1, %z\n  %a = xor i16 %s, -1\n  %r = mul i16 %x, %a\n  ret i16 %r\n",
        "  %0 = freeze i16 %x\n  %1 = shl i16 %0, %z\n  %2 = sub i16 %1, %0\n  ret i16 %2\n",
    );
}

/// Products of boolean extends are an `and` extended: `zext * zext` and `sext
/// * zext` (`opt` agrees).
#[test]
fn a_product_of_boolean_extends_is_an_extended_and() {
    check(
        "i16",
        "i1 %a, i1 %b",
        "  %x = zext i1 %a to i16\n  %y = zext i1 %b to i16\n  %r = mul i16 %x, %y\n  ret i16 %r\n",
        "  %0 = and i1 %a, %b\n  %1 = zext i1 %0 to i16\n  ret i16 %1\n",
    );
    check(
        "i16",
        "i1 %a, i1 %b",
        "  %x = sext i1 %a to i16\n  %y = zext i1 %b to i16\n  %r = mul i16 %x, %y\n  ret i16 %r\n",
        "  %0 = and i1 %a, %b\n  %1 = sext i1 %0 to i16\n  ret i16 %1\n",
    );
}

/// A multiply by a boolean, a sign splat or a low bit is a `select` (`opt`
/// agrees on all six).
#[test]
fn a_multiply_by_a_boolean_like_value_is_a_select() {
    check(
        "i16",
        "i1 %a, i16 %y",
        "  %x = zext i1 %a to i16\n  %r = mul i16 %y, %x\n  ret i16 %r\n",
        "  %0 = select i1 %a, i16 %y, i16 0\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i1 %a, i16 %y",
        "  %x = sext i1 %a to i16\n  %r = mul nsw i16 %x, %y\n  ret i16 %r\n",
        "  %0 = sub nsw i16 0, %y\n  %1 = select i1 %a, i16 %0, i16 0\n  ret i16 %1\n",
    );
    check(
        "i16",
        "i1 %a",
        "  %x = sext i1 %a to i16\n  %r = mul i16 %x, 6\n  ret i16 %r\n",
        "  %0 = select i1 %a, i16 -6, i16 0\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %a",
        "  %x = ashr i16 %a, 15\n  %r = mul i16 %x, 6\n  ret i16 %r\n",
        "  %0 = icmp slt i16 %a, 0\n  %1 = select i1 %0, i16 -6, i16 0\n  ret i16 %1\n",
    );
    check(
        "i16",
        "i16 %a, i16 %y",
        "  %x = lshr i16 %a, 15\n  %r = mul i16 %x, %y\n  ret i16 %r\n",
        "  %0 = icmp slt i16 %a, 0\n  %1 = select i1 %0, i16 %y, i16 0\n  ret i16 %1\n",
    );
    check(
        "i16",
        "i16 %a, i16 %y",
        "  %x = and i16 %a, 1\n  %r = mul i16 %x, %y\n  ret i16 %r\n",
        "  %0 = trunc i16 %a to i1\n  %1 = select i1 %0, i16 %y, i16 0\n  ret i16 %1\n",
    );
}

/// An `ashr` by less than the sign bit is not a sign splat: the multiply stays
/// (and gains `nsw`).
#[test]
fn a_multiply_by_a_partial_sign_shift_is_kept() {
    check(
        "i16",
        "i16 %a",
        "  %x = ashr i16 %a, 7\n  %r = mul i16 %x, 6\n  ret i16 %r\n",
        "  %x = ashr i16 %a, 7\n  %r = mul nsw i16 %x, 6\n  ret i16 %r\n",
    );
}

/// `tryGetLog2`: `X * (64 >>exact Y)` is `X << (6 - Y)` (`opt` agrees).
#[test]
fn a_multiply_by_a_power_of_two_whose_log_folds_is_a_shift() {
    check(
        "i16",
        "i16 %x, i16 %y",
        "  %p = lshr exact i16 64, %y\n  %r = mul i16 %x, %p\n  ret i16 %r\n",
        "  %0 = sub i16 6, %y\n  %1 = shl i16 %x, %0\n  ret i16 %1\n",
    );
}

/// A product of two zero-extended bytes cannot wrap unsigned: `nuw` is added
/// (`opt` agrees).
#[test]
fn a_product_known_not_to_wrap_gets_nuw() {
    check(
        "i16",
        "i8 %a, i8 %b",
        "  %x = zext i8 %a to i16\n  %y = zext i8 %b to i16\n  %r = mul i16 %x, %y\n  ret i16 %r\n",
        "  %x = zext i8 %a to i16\n  %y = zext i8 %b to i16\n  %r = mul nuw i16 %x, %y\n  ret i16 %r\n",
    );
}

/// `(X / C1) / C2` is `X / (C1 * C2)`, signed and unsigned (`opt` agrees).
#[test]
fn a_division_chain_by_constants_is_one_division() {
    check(
        "i16",
        "i16 %x",
        "  %a = udiv i16 %x, 3\n  %r = udiv i16 %a, 5\n  ret i16 %r\n",
        "  %0 = udiv i16 %x, 15\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %a = sdiv i16 %x, 3\n  %r = sdiv i16 %a, 5\n  ret i16 %r\n",
        "  %0 = sdiv i16 %x, 15\n  ret i16 %0\n",
    );
}

/// `(X *nuw C1) / C2`: a multiple divisor divides, a multiple multiplier
/// multiplies, else their gcd is divided out (`opt` agrees on the first two
/// and goes on to shifts on the third).
#[test]
fn a_non_wrapping_multiply_then_a_division_by_constants_cancels() {
    check(
        "i16",
        "i16 %x",
        "  %a = mul nuw i16 %x, 6\n  %r = udiv i16 %a, 3\n  ret i16 %r\n",
        "  %0 = shl nuw i16 %x, 1\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %a = mul nuw i16 %x, 3\n  %r = udiv exact i16 %a, 6\n  ret i16 %r\n",
        "  %0 = lshr exact i16 %x, 1\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %a = mul nuw i16 %x, 6\n  %r = udiv i16 %a, 4\n  ret i16 %r\n",
        "  %0 = mul nuw i16 %x, 3\n  %1 = lshr i16 %0, 1\n  ret i16 %1\n",
    );
}

/// `(X <<nuw C1) / C2` is a division by `C2 >> C1`, a multiply by `(1 << C1) /
/// C2`, or a smaller shift and divisor (`opt` agrees on the first two).
#[test]
fn a_non_wrapping_shift_then_a_division_by_a_constant_cancels() {
    check(
        "i16",
        "i16 %x",
        "  %a = shl nuw i16 %x, 2\n  %r = udiv i16 %a, 12\n  ret i16 %r\n",
        "  %0 = udiv i16 %x, 3\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %a = shl nuw i16 %x, 3\n  %r = udiv i16 %a, 2\n  ret i16 %r\n",
        "  %0 = shl nuw i16 %x, 2\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %a = shl nuw i16 %x, 2\n  %r = udiv i16 %a, 6\n  ret i16 %r\n",
        "  %0 = shl nuw i16 %x, 1\n  %1 = udiv i16 %0, 3\n  ret i16 %1\n",
    );
}

/// `((X * C2) + C1) / C2` is `X + C1 / C2` (`opt` agrees on both).
#[test]
fn a_division_distributes_over_a_matching_multiply_and_add() {
    check(
        "i16",
        "i16 %x",
        "  %m = mul nuw i16 %x, 5\n  %a = add nuw i16 %m, 17\n  %r = udiv i16 %a, 5\n  ret i16 %r\n",
        "  %0 = add nuw i16 %x, 3\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %m = mul nsw i16 %x, 5\n  %a = add nsw i16 %m, 15\n  %r = sdiv i16 %a, 5\n  ret i16 %r\n",
        "  %0 = add nsw i16 %x, 3\n  ret i16 %0\n",
    );
}

/// `1 / X`: `zext (X == 1)` unsigned, on a frozen X for signed (`opt` agrees).
#[test]
fn one_divided_by_x_is_a_compare() {
    check(
        "i16",
        "i16 %x",
        "  %r = udiv i16 1, %x\n  ret i16 %r\n",
        "  %0 = icmp eq i16 %x, 1\n  %1 = zext i1 %0 to i16\n  ret i16 %1\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %r = sdiv i16 1, %x\n  ret i16 %r\n",
        "  %0 = freeze i16 %x\n  %1 = add i16 %0, 1\n  %2 = icmp ult i16 %1, 3\n  %3 = select i1 %2, i16 %0, i16 0\n  ret i16 %3\n",
    );
}

/// `(X - X % Y) / Y` is `X / Y`; `(X << Y) / X` is `1 << Y`; `X / (X * Y)` is
/// `1 / Y` (`opt` agrees on all three).
#[test]
fn a_division_cancels_a_remainder_a_shift_or_a_multiply_of_the_dividend() {
    check(
        "i16",
        "i16 %x, i16 %y",
        "  %m = urem i16 %x, %y\n  %s = sub i16 %x, %m\n  %r = udiv i16 %s, %y\n  ret i16 %r\n",
        "  %0 = udiv i16 %x, %y\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x, i16 %y",
        "  %s = shl nuw i16 %x, %y\n  %r = udiv i16 %s, %x\n  ret i16 %r\n",
        "  %0 = shl nuw i16 1, %y\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x, i16 %y",
        "  %m = mul nuw i16 %x, %y\n  %r = udiv i16 %x, %m\n  ret i16 %r\n",
        "  %0 = icmp eq i16 %y, 1\n  %1 = zext i1 %0 to i16\n  ret i16 %1\n",
    );
}

/// `foldIDivShl`: a factor shared as a multiplier and a shifted value, or as a
/// shift amount (`opt` agrees on all three).
#[test]
fn a_division_cancels_a_factor_disguised_as_a_shift() {
    check(
        "i16",
        "i16 %x, i16 %y, i16 %z",
        "  %m = mul nuw i16 %x, %y\n  %s = shl nuw i16 %x, %z\n  %r = udiv exact i16 %m, %s\n  ret i16 %r\n",
        "  %0 = lshr exact i16 %y, %z\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x, i16 %y, i16 %z",
        "  %a = shl nuw i16 %x, %z\n  %b = shl nuw i16 %y, %z\n  %r = udiv i16 %a, %b\n  ret i16 %r\n",
        "  %0 = udiv i16 %x, %y\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x, i16 %y, i16 %z",
        "  %a = shl nuw i16 %x, %y\n  %b = shl nuw i16 %x, %z\n  %r = udiv i16 %a, %b\n  ret i16 %r\n",
        "  %0 = shl nuw i16 1, %y\n  %1 = lshr i16 %0, %z\n  ret i16 %1\n",
    );
}

/// A multiply by the divisor is cancelled through another division, a right
/// shift or a second multiply (`opt` agrees on all three).
#[test]
fn a_division_cancels_a_multiply_by_its_divisor_through_another_operation() {
    check(
        "i16",
        "i16 %x, i16 %y, i16 %z",
        "  %m = mul nuw i16 %z, %x\n  %d = udiv i16 %m, %y\n  %r = udiv i16 %d, %z\n  ret i16 %r\n",
        "  %0 = udiv i16 %x, %y\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x, i16 %y, i16 %b",
        "  %m = mul nuw i16 %y, %x\n  %s = lshr exact i16 %m, %b\n  %r = udiv exact i16 %s, %y\n  ret i16 %r\n",
        "  %0 = lshr exact i16 %x, %b\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x, i16 %y, i16 %z",
        "  %a = mul nuw i16 %x, %y\n  %b = mul nuw i16 %x, %z\n  %r = udiv i16 %a, %b\n  ret i16 %r\n",
        "  %0 = udiv i16 %y, %z\n  ret i16 %0\n",
    );
}

/// `X / (select C, 1, Y)` is `select C, X, X / Y` when Y is a safe divisor
/// (`opt` agrees); the signed form needs a constant.
#[test]
fn a_division_by_a_select_with_one_sinks_into_the_other_arm() {
    check(
        "i16",
        "i1 %c, i16 %x, i16 %y",
        "  %f = freeze i16 %y\n  %n = or i16 %f, 1\n  %s = select i1 %c, i16 1, i16 %n\n  %r = udiv i16 %x, %s\n  ret i16 %r\n",
        "  %f = freeze i16 %y\n  %n = or i16 %f, 1\n  %0 = udiv i16 %x, %n\n  %1 = select i1 %c, i16 %x, i16 %0\n  ret i16 %1\n",
    );
    check(
        "i16",
        "i1 %c, i16 %x",
        "  %s = select i1 %c, i16 7, i16 1\n  %r = sdiv i16 %x, %s\n  ret i16 %r\n",
        "  %0 = sdiv i16 %x, 7\n  %1 = select i1 %c, i16 %0, i16 %x\n  ret i16 %1\n",
    );
}

/// `(X u>> C1) / C2` is `X / (C2 << C1)`; `(X / Y) / Z` is `X / (Y * Z)` when
/// the product cannot wrap (`opt` agrees on both).
#[test]
fn nested_unsigned_divisions_combine() {
    check(
        "i16",
        "i16 %x",
        "  %a = lshr i16 %x, 2\n  %r = udiv i16 %a, 3\n  ret i16 %r\n",
        "  %0 = udiv i16 %x, 12\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x, i8 %a, i8 %b",
        "  %za = zext i8 %a to i16\n  %zb = zext i8 %b to i16\n  %d = udiv i16 %x, %za\n  %r = udiv i16 %d, %zb\n  ret i16 %r\n",
        "  %za = zext i8 %a to i16\n  %zb = zext i8 %b to i16\n  %0 = mul nuw i16 %za, %zb\n  %1 = udiv i16 %x, %0\n  ret i16 %1\n",
    );
}

/// An unsigned division by a negative divisor or by `sext i1` is a compare
/// (`opt` agrees).
#[test]
fn an_unsigned_division_by_a_large_divisor_is_a_compare() {
    check(
        "i16",
        "i16 %x, i16 %y",
        "  %n = or i16 %y, -32768\n  %r = udiv i16 %x, %n\n  ret i16 %r\n",
        "  %n = or i16 %y, -32768\n  %0 = icmp uge i16 %x, %n\n  %1 = zext i1 %0 to i16\n  ret i16 %1\n",
    );
    check(
        "i16",
        "i16 %x, i1 %b",
        "  %s = sext i1 %b to i16\n  %r = udiv i16 %x, %s\n  ret i16 %r\n",
        "  %0 = icmp eq i16 %x, -1\n  %1 = zext i1 %0 to i16\n  ret i16 %1\n",
    );
}

/// `udiv`/`urem` of zero-extended operands, or of one and a constant that
/// fits, is narrowed (`opt` agrees, and adds `nneg` to the extend).
/// Unverified against `opt` where it differs: opt also puts `nneg` on the
/// `zext` (visitZExt).
#[test]
fn an_unsigned_division_of_extends_is_narrowed() {
    check(
        "i16",
        "i8 %a, i8 %b",
        "  %za = zext i8 %a to i16\n  %zb = zext i8 %b to i16\n  %r = udiv i16 %za, %zb\n  ret i16 %r\n",
        "  %0 = udiv i8 %a, %b\n  %1 = zext i8 %0 to i16\n  ret i16 %1\n",
    );
    check(
        "i16",
        "i8 %a",
        "  %za = zext i8 %a to i16\n  %r = udiv i16 %za, 100\n  ret i16 %r\n",
        "  %0 = udiv i8 %a, 100\n  %1 = zext i8 %0 to i16\n  ret i16 %1\n",
    );
}

/// `X / 2^k` is `X >> k`, also when the power of two is `1 << Y` (`opt`
/// agrees).
#[test]
fn an_unsigned_division_by_a_power_of_two_is_a_shift() {
    check(
        "i16",
        "i16 %x, i16 %y",
        "  %p = shl nuw i16 1, %y\n  %r = udiv i16 %x, %p\n  ret i16 %r\n",
        "  %0 = lshr i16 %x, %y\n  ret i16 %0\n",
    );
    check("i16", "i16 %x", "  %r = udiv i16 %x, 8\n  ret i16 %r\n", "  %0 = lshr i16 %x, 3\n  ret i16 %0\n");
}

/// `sdiv X, -1`, `sdiv X, (sext i1)` and `sdiv X, INT_MIN` (`opt` agrees).
#[test]
fn a_signed_division_by_minus_one_or_int_min_is_cheap() {
    check(
        "i16",
        "i16 %x, i1 %b",
        "  %s = sext i1 %b to i16\n  %r = sdiv i16 %x, %s\n  ret i16 %r\n",
        "  %0 = sub nsw i16 0, %x\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %r = sdiv i16 %x, -32768\n  ret i16 %r\n",
        "  %0 = icmp eq i16 %x, -32768\n  %1 = zext i1 %0 to i16\n  ret i16 %1\n",
    );
}

/// `sdiv exact` by a power of two, `1 << Y` or a negated power of two is an
/// `ashr exact` (`opt` agrees on all three).
#[test]
fn an_exact_signed_division_by_a_power_of_two_is_an_arithmetic_shift() {
    check(
        "i16",
        "i16 %x",
        "  %r = sdiv exact i16 %x, 8\n  ret i16 %r\n",
        "  %0 = ashr exact i16 %x, 3\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x, i16 %y",
        "  %s = shl nsw i16 1, %y\n  %r = sdiv exact i16 %x, %s\n  ret i16 %r\n",
        "  %0 = ashr exact i16 %x, %y\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %r = sdiv exact i16 %x, -8\n  ret i16 %r\n",
        "  %0 = ashr exact i16 %x, 3\n  %1 = sub nsw i16 0, %0\n  ret i16 %1\n",
    );
}

/// `(sext X) sdiv C` is narrowed; `-X / C` is `X / -C`; `-X / Y` is `-(X / Y)`
/// (`opt` agrees on all three).
#[test]
fn a_signed_division_moves_a_sext_or_a_negation_out() {
    check(
        "i32",
        "i8 %x",
        "  %s = sext i8 %x to i32\n  %r = sdiv i32 %s, 100\n  ret i32 %r\n",
        "  %0 = sdiv i8 %x, 100\n  %1 = sext i8 %0 to i32\n  ret i32 %1\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %n = sub nsw i16 0, %x\n  %r = sdiv i16 %n, 7\n  ret i16 %r\n",
        "  %0 = sdiv i16 %x, -7\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x, i16 %y",
        "  %n = sub nsw i16 0, %x\n  %r = sdiv i16 %n, %y\n  ret i16 %r\n",
        "  %0 = sdiv i16 %x, %y\n  %1 = sub nsw i16 0, %0\n  ret i16 %1\n",
    );
}

/// A divisor of a power of two the dividend's low bits are zero for is exact
/// (`opt` goes on to the shift).
#[test]
fn a_signed_division_known_to_be_exact_gets_exact() {
    check(
        "i16",
        "i16 %x",
        "  %m = shl i16 %x, 3\n  %r = sdiv i16 %m, 8\n  ret i16 %r\n",
        "  %m = shl i16 %x, 3\n  %0 = ashr exact i16 %m, 3\n  ret i16 %0\n",
    );
}

/// A signed division of a non-negative dividend is unsigned: by a non-negative
/// divisor, by a negated power of two (negated) or by `1 << Y` (`opt`
/// agrees).
/// Unverified against `opt` where it differs: opt narrows the mask through the
/// shift (visitAnd, visitLShr) and puts `nsw` on the `sub`.
#[test]
fn a_signed_division_of_a_non_negative_dividend_is_unsigned() {
    check(
        "i16",
        "i16 %x, i16 %y",
        "  %a = and i16 %x, 255\n  %b = and i16 %y, 255\n  %r = sdiv i16 %a, %b\n  ret i16 %r\n",
        "  %a = and i16 %x, 255\n  %b = and i16 %y, 255\n  %0 = udiv i16 %a, %b\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %a = and i16 %x, 255\n  %r = sdiv i16 %a, -8\n  ret i16 %r\n",
        "  %a = and i16 %x, 255\n  %0 = lshr i16 %a, 3\n  %1 = sub i16 0, %0\n  ret i16 %1\n",
    );
    check(
        "i16",
        "i16 %x, i16 %y",
        "  %a = and i16 %x, 255\n  %s = shl i16 1, %y\n  %r = sdiv i16 %a, %s\n  ret i16 %r\n",
        "  %a = and i16 %x, 255\n  %0 = lshr i16 %a, %y\n  ret i16 %0\n",
    );
}

/// `-X / X` is `-X == INT_MIN ? 1 : -1` (`opt` folds the compare to `X ==
/// INT_MIN`).
/// Unverified against `opt` where it differs: opt also folds the compare of the
/// negation to `icmp eq x, INT_MIN` (visitICmp).
#[test]
fn a_negation_divided_by_its_operand_is_a_select() {
    check(
        "i16",
        "i16 %x",
        "  %n = sub i16 0, %x\n  %r = sdiv i16 %n, %x\n  ret i16 %r\n",
        "  %n = sub i16 0, %x\n  %0 = icmp eq i16 %n, -32768\n  %1 = select i1 %0, i16 1, i16 -1\n  ret i16 %1\n",
    );
}

/// `urem X, 1 << Y` is a mask; `urem 1, X` is `X != 1` (`opt` goes on to
/// `shl -1`, and agrees on the second).
/// Unverified against `opt` where it differs: opt rewrites `(1 << y) + -1` as
/// `~(-1 << y)` (visitAdd).
#[test]
fn an_unsigned_remainder_by_a_power_of_two_is_a_mask() {
    check(
        "i16",
        "i16 %x, i16 %y",
        "  %s = shl nuw i16 1, %y\n  %r = urem i16 %x, %s\n  ret i16 %r\n",
        "  %s = shl nuw i16 1, %y\n  %0 = add i16 %s, -1\n  %1 = and i16 %x, %0\n  ret i16 %1\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %r = urem i16 1, %x\n  ret i16 %r\n",
        "  %0 = icmp ne i16 %x, 1\n  %1 = zext i1 %0 to i16\n  ret i16 %1\n",
    );
}

/// `urem X, C` with `C` negative and `urem X, (sext i1)` are selects on a
/// frozen X (`opt` agrees up to the `sub` spelling).
/// Unverified against `opt` where it differs: opt writes `sub x, C` as `add x,
/// -C` (visitSub).
#[test]
fn an_unsigned_remainder_by_a_large_divisor_is_a_select() {
    check(
        "i16",
        "i16 %x",
        "  %r = urem i16 %x, 40000\n  ret i16 %r\n",
        "  %0 = freeze i16 %x\n  %1 = icmp ult i16 %0, -25536\n  %2 = sub i16 %0, -25536\n  %3 = select i1 %1, i16 %0, i16 %2\n  ret i16 %3\n",
    );
    check(
        "i16",
        "i16 %x, i1 %b",
        "  %s = sext i1 %b to i16\n  %r = urem i16 %x, %s\n  ret i16 %r\n",
        "  %0 = freeze i16 %x\n  %1 = icmp eq i16 %0, -1\n  %2 = select i1 %1, i16 0, i16 %0\n  ret i16 %2\n",
    );
}

/// `(X + 1) urem Y` with `X u< Y` is `(X + 1) == Y ? 0 : X + 1` (`opt`
/// freezes `X` instead of the sum).
/// Unverified against `opt` where it differs: opt also moves the `freeze` above
/// the `and` and puts `nuw nsw` on the `add` (visitAdd, freeze).
#[test]
fn an_incremented_value_below_the_divisor_wraps_to_zero() {
    check(
        "i16",
        "i16 %x",
        "  %m = and i16 %x, 127\n  %p = add i16 %m, 1\n  %r = urem i16 %p, 200\n  ret i16 %r\n",
        "  %m = and i16 %x, 127\n  %p = add i16 %m, 1\n  %0 = freeze i16 %p\n  %1 = icmp eq i16 %0, 200\n  %2 = select i1 %1, i16 0, i16 %0\n  ret i16 %2\n",
    );
}

/// `srem X, -C` is `srem X, C`; `-X srem Y` is `-(X srem Y)`; non-negative
/// operands make a `urem` (`opt` agrees).
#[test]
fn a_signed_remainder_drops_a_negative_divisor_or_a_negation() {
    check("i16", "i16 %x", "  %r = srem i16 %x, -7\n  ret i16 %r\n", "  %r = srem i16 %x, 7\n  ret i16 %r\n");
    check(
        "i16",
        "i16 %x, i16 %y",
        "  %n = sub nsw i16 0, %x\n  %r = srem i16 %n, %y\n  ret i16 %r\n",
        "  %0 = srem i16 %x, %y\n  %1 = sub nsw i16 0, %0\n  ret i16 %1\n",
    );
    check(
        "i16",
        "i16 %x, i16 %y",
        "  %a = and i16 %x, 255\n  %b = and i16 %y, 255\n  %r = srem i16 %a, %b\n  ret i16 %r\n",
        "  %a = and i16 %x, 255\n  %b = and i16 %y, 255\n  %0 = urem i16 %a, %b\n  ret i16 %0\n",
    );
}

/// `simplifyIRemMulShl`: a remainder of two multiples of one value is zero, the
/// dividend multiple, or a smaller multiple (`opt` agrees on all four).
#[test]
fn a_remainder_of_two_multiples_of_one_value_is_simpler() {
    check(
        "i16",
        "i16 %x",
        "  %a = mul nuw i16 %x, 10\n  %b = mul i16 %x, 5\n  %r = urem i16 %a, %b\n  ret i16 %r\n",
        "  ret i16 0\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %a = mul i16 %x, 3\n  %b = mul nuw i16 %x, 5\n  %r = urem i16 %a, %b\n  ret i16 %r\n",
        "  %0 = mul nuw i16 %x, 3\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %a = mul nuw i16 %x, 13\n  %b = mul i16 %x, 5\n  %r = urem i16 %a, %b\n  ret i16 %r\n",
        "  %0 = mul nuw nsw i16 %x, 3\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %a = shl nuw i16 %x, 4\n  %b = shl i16 %x, 2\n  %r = urem i16 %a, %b\n  ret i16 %r\n",
        "  ret i16 0\n",
    );
}

/// A divisor `(1 << A) >> B` is known non-zero, so `A >= B`: it becomes `1 <<
/// (A - B)` (`opt` agrees on the result).
#[test]
fn a_divisor_known_non_zero_is_simplified() {
    check(
        "i16",
        "i16 %x, i16 %a, i16 %b",
        "  %s = shl i16 1, %a\n  %d = lshr i16 %s, %b\n  %r = udiv i16 %x, %d\n  ret i16 %r\n",
        "  %0 = sub i16 %a, %b\n  %1 = lshr i16 %x, %0\n  ret i16 %1\n",
    );
}

/// `div X, (select C, 0, Y)` is `div X, Y`: the zero arm would be undefined
/// (`opt` agrees).
#[test]
fn a_division_by_a_select_with_a_zero_arm_uses_the_other_arm() {
    check(
        "i16",
        "i1 %c, i16 %x, i16 %y",
        "  %s = select i1 %c, i16 0, i16 %y\n  %r = udiv i16 %x, %s\n  ret i16 %r\n",
        "  %r = udiv i16 %x, %y\n  ret i16 %r\n",
    );
}

/// `(A - B) * -4` sank the negation into the subtraction: `(B - A) * 4`, then
/// the shift. Without `Negator` the product stayed a multiply by `-4`.
#[test]
fn a_difference_times_a_negated_power_of_two_swaps_the_difference() {
    check(
        "i16",
        "i16 %a, i16 %b",
        "  %s = sub i16 %a, %b\n  %r = mul i16 %s, -4\n  ret i16 %r\n",
        "  %0 = sub i16 %b, %a\n  %1 = shl i16 %0, 2\n  ret i16 %1\n",
    );
}

/// `(-X * Y) * -X` is `(X * Y) * X` when `-X` has another use. Without
/// `Negator` the first operand stayed negated.
#[test]
fn a_negated_product_times_the_same_negation_cancels() {
    check(
        "i16",
        "i16 %x, i16 %y",
        "  %n = sub i16 0, %x\n  %p = mul i16 %n, %y\n  %r = mul i16 %p, %n\n  %t = add i16 %r, %n\n  ret i16 %t\n",
        "  %n = sub i16 0, %x\n  %0 = mul i16 %x, %y\n  %1 = mul i16 %0, %x\n  %t = add i16 %1, %n\n  ret i16 %t\n",
    );
}
