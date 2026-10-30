use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::combine::{Combiner, Visit, combine_with};
use crate::combine_add_idioms::fold_add_rest;
use crate::combine_addsub::visit_add;
use crate::testing::{parsed, printed};

/// `visitAdd` alone, through the driver.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if c.function.instruction(inst).opcode == Opcode::Binary(BinaryOp::Add) { visit_add(c, inst) } else { Visit::None }
}

/// This module's folds alone, on the adds without a constant operand (so a
/// `x + 1` inside the pattern is left as it is).
fn visit_rest(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    let add = c.function.instruction(inst);
    if add.opcode == Opcode::Binary(BinaryOp::Add) && add.operands.iter().all(|o| matches!(o, Operand::Value(_))) {
        fold_add_rest(c, inst)
    } else {
        Visit::None
    }
}

/// `define RET @f(PARAMS)` with `body` before and `after` after `visitAdd`.
fn check(
    ret: &str,
    params: &str,
    body: &str,
    after: &str,
) {
    check_with(visit, ret, params, body, after);
}

/// `check` with the visitor `visit`.
fn check_with(
    visit: fn(&mut Combiner, InstId) -> Visit,
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

/// `i32 %x, i32 %y`.
const XY: &str = "i32 %x, i32 %y";

/// `-A + -B` is `-(A + B)` (`opt` agrees).
#[test]
fn two_negations_are_the_negated_sum() {
    check(
        "i32",
        XY,
        "  %a = sub i32 0, %x\n  %b = sub i32 0, %y\n  %r = add i32 %a, %b\n  ret i32 %r\n",
        "  %0 = add i32 %x, %y\n  %1 = sub i32 0, %0\n  ret i32 %1\n",
    );
}

/// `-A + B` is `B - A`, `nsw` kept when both had it (`opt` agrees).
#[test]
fn a_negated_left_operand_is_a_sub() {
    check(
        "i32",
        XY,
        "  %a = sub nsw i32 0, %x\n  %r = add nsw i32 %a, %y\n  ret i32 %r\n",
        "  %0 = sub nsw i32 %y, %x\n  ret i32 %0\n",
    );
}

/// `A + -B` is `A - B`; no flag survives without `nsw` on the add.
#[test]
fn a_negated_right_operand_is_a_sub() {
    check(
        "i32",
        XY,
        "  %b = sub i32 0, %y\n  %r = add i32 %x, %b\n  ret i32 %r\n",
        "  %0 = sub i32 %x, %y\n  ret i32 %0\n",
    );
}

/// `(A + 1) + ~B` is `A - B`.
#[test]
fn a_plus_one_and_a_not_make_a_sub() {
    check(
        "i32",
        XY,
        "  %a = add i32 %x, 1\n  %n = xor i32 %y, -1\n  %r = add i32 %a, %n\n  ret i32 %r\n",
        "  %0 = sub i32 %x, %y\n  ret i32 %0\n",
    );
}

/// `(~B + A) + 1` is `A - B`.
#[test]
fn a_not_plus_a_and_one_make_a_sub() {
    check(
        "i32",
        XY,
        "  %n = xor i32 %y, -1\n  %a = add i32 %n, %x\n  %r = add i32 %a, 1\n  ret i32 %r\n",
        "  %0 = sub i32 %x, %y\n  ret i32 %0\n",
    );
}

/// `(A + 5) + ~B` is `(A - B) + 4`.
#[test]
fn a_constant_add_and_a_not_keep_the_constant_less_one() {
    check(
        "i32",
        XY,
        "  %a = add i32 %x, 5\n  %n = xor i32 %y, -1\n  %r = add i32 %a, %n\n  ret i32 %r\n",
        "  %0 = sub i32 %x, %y\n  %1 = add i32 %0, 4\n  ret i32 %1\n",
    );
}

/// `(A + Y) + Y` is `A + (Y << 1)`.
#[test]
fn an_add_of_the_other_operand_is_a_shift() {
    check(
        "i32",
        XY,
        "  %s = add i32 %x, %y\n  %r = add i32 %s, %y\n  ret i32 %r\n",
        "  %0 = shl i32 %y, 1\n  %1 = add i32 %x, %0\n  ret i32 %1\n",
    );
}

/// `(A + 3) + (7 - B)` is `(A - B) + 10`.
#[test]
fn an_add_and_a_sub_of_constants_share_their_sum() {
    check(
        "i32",
        XY,
        "  %a = add i32 %x, 3\n  %s = sub i32 7, %y\n  %r = add i32 %a, %s\n  ret i32 %r\n",
        "  %0 = sub i32 %x, %y\n  %1 = add i32 %0, 10\n  ret i32 %1\n",
    );
}

/// `(5 - A) + B` is `(B - A) + 5`.
#[test]
fn a_constant_sub_becomes_a_constant_add() {
    check(
        "i32",
        XY,
        "  %s = sub i32 5, %x\n  %r = add i32 %s, %y\n  ret i32 %r\n",
        "  %0 = sub i32 %y, %x\n  %1 = add i32 %0, 5\n  ret i32 %1\n",
    );
}

/// `(A & 128) + A` is `A & 127` when bit 7 of A is a sign bit (A is a sext of
/// i8).
#[test]
fn a_masked_sign_bit_added_to_itself_clears_the_bit() {
    check(
        "i32",
        "i8 %p",
        "  %e = sext i8 %p to i32\n  %a = and i32 %e, 128\n  %r = add i32 %a, %e\n  ret i32 %r\n",
        "  %e = sext i8 %p to i32\n  %0 = and i32 %e, 127\n  ret i32 %0\n",
    );
}

/// `zext(B -nuw A) + zext(A)` is `zext B` (`opt` agrees).
#[test]
fn zexts_of_a_nuw_sub_and_its_subtrahend_are_the_minuend() {
    check(
        "i32",
        "i8 %a, i8 %b",
        "  %d = sub nuw i8 %b, %a\n  %zd = zext i8 %d to i32\n  %za = zext i8 %a to i32\n  %r = add i32 %zd, %za\n  ret i32 %r\n",
        "  %0 = zext i8 %b to i32\n  ret i32 %0\n",
    );
}

/// `zext(i1 A) + sext(A)` is 0 (`opt` agrees).
#[test]
fn zext_and_sext_of_a_bool_cancel() {
    check(
        "i32",
        "i1 %a",
        "  %z = zext i1 %a to i32\n  %s = sext i1 %a to i32\n  %r = add i32 %z, %s\n  ret i32 %r\n",
        "  ret i32 0\n",
    );
}

/// `A + B` with no common bits is `or disjoint` (`opt` agrees).
#[test]
fn operands_with_no_common_bits_are_a_disjoint_or() {
    check(
        "i32",
        XY,
        "  %a = and i32 %x, 240\n  %b = and i32 %y, 15\n  %r = add i32 %a, %b\n  ret i32 %r\n",
        "  %a = and i32 %x, 240\n  %b = and i32 %y, 15\n  %0 = or disjoint i32 %a, %b\n  ret i32 %0\n",
    );
}

/// `(A ^ B) + (A & B)` is `A | B` (`opt` agrees).
#[test]
fn xor_plus_and_is_or() {
    check(
        "i32",
        XY,
        "  %a = xor i32 %x, %y\n  %b = and i32 %x, %y\n  %r = add i32 %a, %b\n  ret i32 %r\n",
        "  %0 = or i32 %x, %y\n  ret i32 %0\n",
    );
}

/// `(A | B) + (A & B)` is `A + B` in place, `nuw` kept (`opt` agrees).
#[test]
fn or_plus_and_is_the_add_with_its_flags() {
    check(
        "i32",
        XY,
        "  %a = or i32 %x, %y\n  %b = and i32 %y, %x\n  %r = add nuw i32 %a, %b\n  ret i32 %r\n",
        "  %r = add nuw i32 %x, %y\n  ret i32 %r\n",
    );
}

/// `ADD(XOR(OR(Z, ~C), C), 1) + R` is `R - (Z & C)` (checkForNegativeOperand).
#[test]
fn an_or_xor_one_is_a_negated_and() {
    check(
        "i32",
        "i32 %z, i32 %r0",
        "  %o = or i32 %z, -16\n  %x = xor i32 %o, 15\n  %a = add i32 %x, 1\n  %r = add i32 %a, %r0\n  ret i32 %r\n",
        "  %0 = and i32 %z, 15\n  %1 = sub i32 %r0, %0\n  ret i32 %1\n",
    );
}

/// `ADD(XOR(AND(Z, C), C), 1) + R` is `R - (Z | ~C)`.
#[test]
fn an_and_xor_one_is_a_negated_or() {
    check_with(
        visit_rest,
        "i32",
        "i32 %z, i32 %r0",
        "  %o = and i32 %z, 240\n  %x = xor i32 %o, 240\n  %a = add i32 %x, 1\n  %r = add i32 %a, %r0\n  ret i32 %r\n",
        "  %0 = or i32 %z, -241\n  %1 = sub i32 %r0, %0\n  ret i32 %1\n",
    );
}

/// `XOR(AND(Z, 14), 15) + R` is `R - (Z | ~14)`.
#[test]
fn an_odd_xor_of_an_even_mask_is_a_negated_or() {
    check(
        "i32",
        "i32 %z, i32 %r0",
        "  %o = and i32 %z, 14\n  %x = xor i32 %o, 15\n  %r = add i32 %x, %r0\n  ret i32 %r\n",
        "  %0 = or i32 %z, -15\n  %1 = sub i32 %r0, %0\n  ret i32 %1\n",
    );
}
