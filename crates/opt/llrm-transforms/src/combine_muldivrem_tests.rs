use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::combine::{Combiner, Visit, combine_with};
use crate::combine_muldivrem::visit_mul;
use crate::testing::{parsed, printed};

/// `visitMul` alone, through the driver.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    match c.function.instruction(inst).opcode {
        Opcode::Binary(BinaryOp::Mul) => visit_mul(c, inst),
        _ => Visit::None,
    }
}

/// `@f` after `visitMul` alone.
fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// `define RET @f(PARAMS)` with `body` before and `after` after `visitMul`.
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

/// `abs(X) * abs(X)` is `X * X` (`opt` agrees). MIR declares no `abs`, so the
/// text is not verified; the call stays because the driver never deletes one.
#[test]
fn an_abs_times_itself_is_a_square() {
    let text = |r: &str| {
        format!(
            "define i16 @f(i16 %x) {{\nb0:\n  %a = call i16 @llvm.abs.i16(i16 %x, i1 false)\n{r}}}\n\ndeclare i16 @llvm.abs.i16(i16, i1 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)\n"
        )
    };
    let mut module = parsed(&text("  %r = mul i16 %a, %a\n  ret i16 %r\n"));
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    assert_eq!(llrm_mir::print::module(&module), text("  %0 = mul i16 %x, %x\n  ret i16 %0\n"));
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
        "  %0 = udiv exact i16 %x, 2\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %d = sdiv exact i16 %x, 6\n  %r = mul i16 %d, 3\n  ret i16 %r\n",
        "  %0 = sdiv exact i16 %x, 2\n  ret i16 %0\n",
    );
}

/// `(X / Y) * Y` is `X - (X % Y)` on a frozen X; `(X / Y) * -Y` is `(X % Y) -
/// X`; an exact division leaves X (`opt` agrees on the first and the last).
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
