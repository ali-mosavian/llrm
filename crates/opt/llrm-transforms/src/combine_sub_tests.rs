use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::combine::{Combiner, Visit, combine_with};
use crate::combine_sub::visit_sub;
use crate::testing::{parsed, printed};

/// `visitSub` alone, through the driver.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if c.function.instruction(inst).opcode == Opcode::Binary(BinaryOp::Sub) { visit_sub(c, inst) } else { Visit::None }
}

/// `define RET @f(PARAMS)` with `body` before and `after` after `visitSub`.
fn check(
    ret: &str,
    params: &str,
    body: &str,
    after: &str,
) {
    let text = format!("define {ret} @f({params}) {{\nb0:\n{body}}}\n");
    let want = format!("define {ret} @f({params}) {{\nb0:\n{after}}}\n");
    let mut module = parsed(&text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    assert_eq!(printed(&module), want);
}

/// `x - -y` is `x + y`, `nsw` kept when both had it (`opt` agrees).
#[test]
fn a_subtracted_negation_is_an_add() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %n = sub nsw i32 0, %y\n  %r = sub nsw i32 %x, %n\n  ret i32 %r\n",
        "  %0 = add nsw i32 %x, %y\n  ret i32 %0\n",
    );
}

/// `x - 5` is `x + -5` (`opt` agrees).
#[test]
fn a_constant_subtrahend_is_a_negated_add() {
    check("i32", "i32 %x", "  %r = sub i32 %x, 5\n  ret i32 %r\n", "  %0 = add i32 %x, -5\n  ret i32 %0\n");
}

/// `10 - (x + 3)` is `7 - x`, `nsw` kept (`opt` agrees).
#[test]
fn a_constant_minus_an_add_of_a_constant() {
    check(
        "i32",
        "i32 %x",
        "  %a = add nsw i32 %x, 3\n  %r = sub nsw i32 10, %a\n  ret i32 %r\n",
        "  %0 = sub nsw i32 7, %x\n  ret i32 %0\n",
    );
}

/// `-1 - x` is `~x` (`opt` agrees).
#[test]
fn minus_one_minus_x_is_not() {
    check("i32", "i32 %x", "  %r = sub i32 -1, %x\n  ret i32 %r\n", "  %0 = xor i32 %x, -1\n  ret i32 %0\n");
}

/// `(x + -1) - y` is `~y + x`.
#[test]
fn an_add_of_minus_one_minus_y_is_not_y_plus_x() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %t = add i32 %x, -1\n  %r = sub i32 %t, %y\n  ret i32 %r\n",
        "  %0 = xor i32 %y, -1\n  %1 = add i32 %0, %x\n  ret i32 %1\n",
    );
}

/// `(x & 12) - (x & 4)` is `x & 8` (`opt` agrees).
#[test]
fn two_masks_of_one_value_differ_by_a_mask() {
    check(
        "i32",
        "i32 %x",
        "  %a = and i32 %x, 12\n  %b = and i32 %x, 4\n  %r = sub i32 %a, %b\n  ret i32 %r\n",
        "  %0 = and i32 %x, 8\n  ret i32 %0\n",
    );
}

/// `((x - y) + z) - w` is `(x + z) - (y + w)` (`opt` agrees).
#[test]
fn a_sub_plus_z_minus_w_balances_the_adds() {
    check(
        "i32",
        "i32 %x, i32 %y, i32 %z, i32 %w",
        "  %s = sub i32 %x, %y\n  %a = add i32 %s, %z\n  %r = sub i32 %a, %w\n  ret i32 %r\n",
        "  %0 = add i32 %x, %z\n  %1 = add i32 %y, %w\n  %2 = sub i32 %0, %1\n  ret i32 %2\n",
    );
}

/// `(x - y) - w` is `x - (y + w)`, flags when both subs had them (`opt`
/// agrees).
#[test]
fn a_sub_minus_w_keeps_the_wrap_flags_it_shares() {
    check(
        "i32",
        "i32 %x, i32 %y, i32 %w",
        "  %s = sub nuw nsw i32 %x, %y\n  %r = sub nuw nsw i32 %s, %w\n  ret i32 %r\n",
        "  %0 = add nuw nsw i32 %y, %w\n  %1 = sub nuw nsw i32 %x, %0\n  ret i32 %1\n",
    );
}

/// `(x + 7) - (y + 3)` is `(x - y) + 4` (`opt` agrees).
#[test]
fn adds_of_constants_share_their_difference() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %a = add i32 %x, 7\n  %b = add i32 %y, 3\n  %r = sub i32 %a, %b\n  ret i32 %r\n",
        "  %0 = sub i32 %x, %y\n  %1 = add i32 %0, 4\n  ret i32 %1\n",
    );
}

/// `(x + z) - (z + y)` is `x - y`, `nsw` kept (`opt` agrees).
#[test]
fn adds_sharing_an_operand_cancel_it() {
    check(
        "i32",
        "i32 %x, i32 %y, i32 %z",
        "  %a = add nsw i32 %x, %z\n  %b = add nsw i32 %z, %y\n  %r = sub nsw i32 %a, %b\n  ret i32 %r\n",
        "  %0 = sub nsw i32 %x, %y\n  ret i32 %0\n",
    );
}

/// `15 - (y & 15)` is `(y & 15) ^ 15` (`opt` agrees).
#[test]
fn a_mask_minus_a_value_within_it_is_a_xor() {
    check(
        "i32",
        "i32 %y",
        "  %a = and i32 %y, 15\n  %r = sub i32 15, %a\n  ret i32 %r\n",
        "  %a = and i32 %y, 15\n  %0 = xor i32 %a, 15\n  ret i32 %0\n",
    );
}

/// `x - (x + y)` is `-y` (`opt` agrees).
#[test]
fn x_minus_x_plus_y_is_minus_y() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %a = add i32 %x, %y\n  %r = sub i32 %x, %a\n  ret i32 %r\n",
        "  %0 = sub i32 0, %y\n  ret i32 %0\n",
    );
}

/// `(x - y) - x` is `-y` (`opt` agrees).
#[test]
fn x_minus_y_minus_x_is_minus_y() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %s = sub i32 %x, %y\n  %r = sub i32 %s, %x\n  ret i32 %r\n",
        "  %0 = sub i32 0, %y\n  ret i32 %0\n",
    );
}

/// `(x | y) - (x & y)` is `x ^ y` (`opt` agrees).
#[test]
fn or_minus_and_is_xor() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %o = or i32 %x, %y\n  %a = and i32 %x, %y\n  %r = sub i32 %o, %a\n  ret i32 %r\n",
        "  %0 = xor i32 %x, %y\n  ret i32 %0\n",
    );
}

/// `(x + y) - (x | y)` is `x & y` (`opt` agrees).
#[test]
fn add_minus_or_is_and() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %s = add i32 %x, %y\n  %o = or i32 %x, %y\n  %r = sub i32 %s, %o\n  ret i32 %r\n",
        "  %0 = and i32 %x, %y\n  ret i32 %0\n",
    );
}

/// `(x + y) - (x & y)` is `x | y` (`opt` agrees).
#[test]
fn add_minus_and_is_or() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %s = add i32 %x, %y\n  %a = and i32 %x, %y\n  %r = sub i32 %s, %a\n  ret i32 %r\n",
        "  %0 = or i32 %x, %y\n  ret i32 %0\n",
    );
}

/// `(x & y) - (x | y)` is `-(x ^ y)` (`opt` agrees).
#[test]
fn and_minus_or_is_minus_xor() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %a = and i32 %x, %y\n  %o = or i32 %x, %y\n  %r = sub i32 %a, %o\n  ret i32 %r\n",
        "  %0 = xor i32 %x, %y\n  %1 = sub i32 0, %0\n  ret i32 %1\n",
    );
}

/// `(x | y) - (x ^ y)` is `x & y` (`opt` agrees).
#[test]
fn or_minus_xor_is_and() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %o = or i32 %x, %y\n  %e = xor i32 %x, %y\n  %r = sub i32 %o, %e\n  ret i32 %r\n",
        "  %0 = and i32 %x, %y\n  ret i32 %0\n",
    );
}

/// `(x ^ y) - (x | y)` is `-(x & y)` (`opt` agrees).
#[test]
fn xor_minus_or_is_minus_and() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %e = xor i32 %x, %y\n  %o = or i32 %x, %y\n  %r = sub i32 %e, %o\n  ret i32 %r\n",
        "  %0 = and i32 %x, %y\n  %1 = sub i32 0, %0\n  ret i32 %1\n",
    );
}

/// `(y | x) - x` is `y & ~x` (`opt` agrees).
#[test]
fn or_minus_an_operand_is_and_not() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %o = or i32 %y, %x\n  %r = sub i32 %o, %x\n  ret i32 %r\n",
        "  %0 = xor i32 %x, -1\n  %1 = and i32 %y, %0\n  ret i32 %1\n",
    );
}

/// `(x & -y) - x` is `-(x & (y - 1))` (`opt` agrees).
#[test]
fn an_and_with_a_negation_minus_its_operand() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %n = sub i32 0, %y\n  %a = and i32 %x, %n\n  %r = sub i32 %a, %x\n  ret i32 %r\n",
        "  %0 = add i32 %y, -1\n  %1 = and i32 %x, %0\n  %2 = sub i32 0, %1\n  ret i32 %2\n",
    );
}

/// `(x & 12) - x` is `-(x & ~12)` (`opt` agrees).
#[test]
fn an_and_with_a_constant_minus_its_operand() {
    check(
        "i32",
        "i32 %x",
        "  %a = and i32 %x, 12\n  %r = sub i32 %a, %x\n  ret i32 %r\n",
        "  %0 = and i32 %x, -13\n  %1 = sub i32 0, %0\n  ret i32 %1\n",
    );
}

/// `(x ^ sext c) - sext c` is `c ? -x : x` (`opt` agrees).
#[test]
fn a_xor_with_a_sext_bool_minus_it_is_a_select() {
    check(
        "i32",
        "i32 %x, i1 %c",
        "  %s = sext i1 %c to i32\n  %e = xor i32 %x, %s\n  %r = sub i32 %e, %s\n  ret i32 %r\n",
        "  %0 = sub i32 0, %x\n  %1 = select i1 %c, i32 %0, i32 %x\n  ret i32 %1\n",
    );
}

/// `sext c - (x ^ sext c)` is `c ? x : -x` (`opt` agrees).
#[test]
fn a_sext_bool_minus_the_xor_is_the_other_select() {
    check(
        "i32",
        "i32 %x, i1 %c",
        "  %s = sext i1 %c to i32\n  %e = xor i32 %x, %s\n  %r = sub i32 %s, %e\n  ret i32 %r\n",
        "  %0 = sub i32 0, %x\n  %1 = select i1 %c, i32 %x, i32 %0\n  ret i32 %1\n",
    );
}

/// `0 - zext(i8 x)` cannot overflow signed: `nsw` is inferred (`opt` agrees).
#[test]
fn a_negation_of_a_wide_value_gets_nsw() {
    check(
        "i32",
        "i8 %x",
        "  %z = zext i8 %x to i32\n  %r = sub i32 0, %z\n  ret i32 %r\n",
        "  %z = zext i8 %x to i32\n  %r = sub nsw i32 0, %z\n  ret i32 %r\n",
    );
}

/// `C - ((C3 - x) & C2)` with C2 a negated power of two is `(C - (C2 & C3))
/// + (x & C2)` (`opt` agrees).
#[test]
fn a_constant_minus_a_masked_constant_sub_is_a_masked_add() {
    check(
        "i32",
        "i32 %x",
        "  %s = sub i32 63, %x\n  %a = and i32 %s, -64\n  %r = sub i32 100, %a\n  ret i32 %r\n",
        "  %0 = and i32 %x, -64\n  %1 = add i32 %0, 100\n  ret i32 %1\n",
    );
}

/// `f` as `visitSub` leaves it, its declarations aside: `body` after `visitSub`
/// is `after`.
fn check_calls(
    body: &str,
    after: &str,
) {
    let declares = "declare i32 @llvm.umax.i32(i32, i32)\ndeclare i32 @llvm.umin.i32(i32, i32)\ndeclare i32 @llvm.usub.sat.i32(i32, i32)\n";
    let mut module = parsed(&format!("{declares}define i32 @f(i32 %x, i32 %y) {{\nb0:\n{body}}}\n"));
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    let printed = printed(&module);
    let found = &printed[printed.find("define").expect("a definition")..];
    assert_eq!(found, format!("define i32 @f(i32 %x, i32 %y) {{\nb0:\n{after}}}\n"));
}

/// `umax(x, y) - y` stayed a `umax` and a `sub` where `opt` makes one
/// `usub.sat`: next_byte cost 5 more than the pin's and jpeg_single.nib kept
/// `get_bit` inlined.
#[test]
fn a_umax_minus_its_operand_is_a_saturating_sub() {
    check_calls(
        "  %m = call i32 @llvm.umax.i32(i32 %x, i32 %y)\n  %r = sub i32 %m, %y\n  ret i32 %r\n",
        "  %m = call i32 @llvm.umax.i32(i32 %x, i32 %y)\n  %0 = call i32 @llvm.usub.sat.i32(i32 %x, i32 %y)\n  ret i32 %0\n",
    );
}

/// `x - umin(x, y)` is `usub.sat(x, y)` (`opt` agrees).
#[test]
fn a_value_minus_its_umin_is_a_saturating_sub() {
    check_calls(
        "  %m = call i32 @llvm.umin.i32(i32 %x, i32 %y)\n  %r = sub i32 %x, %m\n  ret i32 %r\n",
        "  %m = call i32 @llvm.umin.i32(i32 %x, i32 %y)\n  %0 = call i32 @llvm.usub.sat.i32(i32 %x, i32 %y)\n  ret i32 %0\n",
    );
}

/// `y - umax(x, y)` is `0 - usub.sat(x, y)`, and `umin(x, y) - y` is `0 -
/// usub.sat(y, x)` (`opt` agrees).
#[test]
fn a_difference_with_the_other_extreme_is_a_negated_saturating_sub() {
    check_calls(
        "  %m = call i32 @llvm.umax.i32(i32 %x, i32 %y)\n  %r = sub i32 %y, %m\n  ret i32 %r\n",
        "  %m = call i32 @llvm.umax.i32(i32 %x, i32 %y)\n  %0 = call i32 @llvm.usub.sat.i32(i32 %x, i32 %y)\n  %1 = sub i32 0, %0\n  ret i32 %1\n",
    );
    check_calls(
        "  %m = call i32 @llvm.umin.i32(i32 %x, i32 %y)\n  %r = sub i32 %m, %y\n  ret i32 %r\n",
        "  %m = call i32 @llvm.umin.i32(i32 %x, i32 %y)\n  %0 = call i32 @llvm.usub.sat.i32(i32 %y, i32 %x)\n  %1 = sub i32 0, %0\n  ret i32 %1\n",
    );
}

/// `x - usub.sat(x, y)` is `umin(x, y)` (`opt` agrees).
#[test]
fn a_value_minus_its_saturating_sub_is_a_umin() {
    check_calls(
        "  %m = call i32 @llvm.usub.sat.i32(i32 %x, i32 %y)\n  %r = sub i32 %x, %m\n  ret i32 %r\n",
        "  %m = call i32 @llvm.usub.sat.i32(i32 %x, i32 %y)\n  %0 = call i32 @llvm.umin.i32(i32 %x, i32 %y)\n  ret i32 %0\n",
    );
}

/// `narrowMathIfNoOverflow` (`TryToNarrowDeduceFlags`, the end of `visitSub`):
/// `20 - zext (and X, 15)` cannot wrap in 8 bits. Stayed a 32-bit sub before.
#[test]
fn a_constant_minus_an_extended_value_that_cannot_wrap_is_subtracted_narrow() {
    check(
        "i32",
        "i8 %x",
        "  %m = and i8 %x, 15\n  %a = zext i8 %m to i32\n  %r = sub i32 20, %a\n  ret i32 %r\n",
        "  %m = and i8 %x, 15\n  %0 = sub nuw nsw i8 20, %m\n  %1 = zext i8 %0 to i32\n  ret i32 %1\n",
    );
}
