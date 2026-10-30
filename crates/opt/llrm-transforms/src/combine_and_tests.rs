use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::combine::{Combiner, Visit, combine_with};
use crate::combine_and::visit_and;
use crate::testing::{parsed, printed};

/// `visitAnd` alone, through the driver.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if c.function.instruction(inst).opcode == Opcode::Binary(BinaryOp::And) { visit_and(c, inst) } else { Visit::None }
}

/// `define RET @f(PARAMS)` with `body` before and `after` after `visitAnd`.
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

/// `(x | y) & ~(x & y)` is `x ^ y` (`opt` agrees).
#[test]
fn or_and_not_and_is_xor() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %o = or i32 %x, %y\n  %a = and i32 %x, %y\n  %n = xor i32 %a, -1\n  %r = and i32 %o, %n\n  ret i32 %r\n",
        "  %0 = xor i32 %x, %y\n  ret i32 %0\n",
    );
}

/// `(x | ~y) & (~x | y)` is `~(x ^ y)` (`opt` agrees).
#[test]
fn or_with_nots_pair_is_a_not_xor() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %ny = xor i32 %y, -1\n  %nx = xor i32 %x, -1\n  %o1 = or i32 %x, %ny\n  %o2 = or i32 %nx, %y\n  %r = and i32 %o1, %o2\n  ret i32 %r\n",
        "  %0 = xor i32 %x, %y\n  %1 = xor i32 %0, -1\n  ret i32 %1\n",
    );
}

/// `(1 >> x) & 1` loses the mask: `1 >> x` has no other bit, and
/// SimplifyDemandedInstructionBits runs first (the zext/icmp form is
/// visitLShr's).
#[test]
fn one_shifted_right_and_one_drops_the_mask() {
    check(
        "i32",
        "i32 %x",
        "  %s = lshr i32 1, %x\n  %r = and i32 %s, 1\n  ret i32 %r\n",
        "  %s = lshr i32 1, %x\n  ret i32 %s\n",
    );
}

/// `-(x & 1) & y` is `(x & 1) == 0 ? 0 : y` (`opt` agrees).
#[test]
fn a_negated_low_bit_and_y_is_a_select() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %l = and i32 %x, 1\n  %n = sub i32 0, %l\n  %r = and i32 %n, %y\n  ret i32 %r\n",
        "  %l = and i32 %x, 1\n  %n = sub i32 0, %l\n  %0 = icmp eq i32 %n, 0\n  %1 = select i1 %0, i32 0, i32 %y\n  ret i32 %1\n",
    );
}

/// `(x + y) & y` is `~x & y` for a power of two `y` (`opt` agrees).
#[test]
fn a_sum_with_a_power_of_two_masked_by_it_is_and_not() {
    check(
        "i32",
        "i32 %x, i32 %n",
        "  %y = shl i32 1, %n\n  %a = add i32 %x, %y\n  %r = and i32 %a, %y\n  ret i32 %r\n",
        "  %y = shl i32 1, %n\n  %0 = xor i32 %x, -1\n  %1 = and i32 %y, %0\n  ret i32 %1\n",
    );
}

/// `(x ^ 12) & 10` is `(x & 10) ^ 8` (`opt` agrees).
#[test]
fn an_xor_with_a_constant_masked_moves_the_mask_in() {
    check(
        "i32",
        "i32 %x",
        "  %e = xor i32 %x, 12\n  %r = and i32 %e, 10\n  ret i32 %r\n",
        "  %0 = and i32 %x, 10\n  %1 = xor i32 %0, 8\n  ret i32 %1\n",
    );
}

/// `(x | 12) & 10` is `(x & 2) | 8` (`opt` also marks it `disjoint`).
#[test]
fn an_or_with_a_constant_masked_moves_the_mask_in() {
    check(
        "i32",
        "i32 %x",
        "  %e = or i32 %x, 12\n  %r = and i32 %e, 10\n  ret i32 %r\n",
        "  %0 = and i32 %x, 2\n  %1 = or i32 %0, 8\n  ret i32 %1\n",
    );
}

/// `(sext (x >>s 3)) & 0x1fffffff` is `(sext x) >>u 3` (`opt` agrees).
#[test]
fn a_mask_of_the_sext_ashr_smear_is_a_lshr_of_the_sext() {
    check(
        "i32",
        "i8 %x",
        "  %a = ashr i8 %x, 3\n  %s = sext i8 %a to i32\n  %r = and i32 %s, 536870911\n  ret i32 %r\n",
        "  %0 = sext i8 %x to i32\n  %1 = lshr i32 %0, 3\n  ret i32 %1\n",
    );
}

/// `(x >>s 4) & 0x0fffffff` is `x >>u 4` (`opt` agrees).
#[test]
fn a_mask_of_the_ashr_smear_is_a_lshr() {
    check(
        "i32",
        "i32 %x",
        "  %a = ashr i32 %x, 4\n  %r = and i32 %a, 268435455\n  ret i32 %r\n",
        "  %0 = lshr i32 %x, 4\n  ret i32 %0\n",
    );
}

/// `(x + 4) & 4` is `(x & 4) ^ 4` (`opt` agrees).
#[test]
fn an_add_that_flips_one_bit_is_a_xor() {
    check(
        "i32",
        "i32 %x",
        "  %a = add i32 %x, 4\n  %r = and i32 %a, 4\n  ret i32 %r\n",
        "  %0 = and i32 %x, 4\n  %1 = xor i32 %0, 4\n  ret i32 %1\n",
    );
}

/// `((zext x) + 5) & 7` is `zext((x + 5) & 7)` (`opt` agrees, adding `nneg`).
#[test]
fn a_binop_of_a_zext_and_a_constant_is_narrowed() {
    check(
        "i32",
        "i8 %x",
        "  %z = zext i8 %x to i32\n  %a = add i32 %z, 5\n  %r = and i32 %a, 7\n  ret i32 %r\n",
        "  %0 = add i8 %x, 5\n  %1 = and i8 %0, 7\n  %2 = zext i8 %1 to i32\n  ret i32 %2\n",
    );
}

/// `((zext x) + y) & 255` is `zext(x + trunc y)` (`opt` agrees).
#[test]
fn a_binop_of_a_zext_masked_to_its_width_is_narrowed() {
    check(
        "i32",
        "i8 %x, i32 %y",
        "  %z = zext i8 %x to i32\n  %a = add i32 %z, %y\n  %r = and i32 %a, 255\n  ret i32 %r\n",
        "  %0 = trunc i32 %y to i8\n  %1 = add i8 %x, %0\n  %2 = zext i8 %1 to i32\n  ret i32 %2\n",
    );
}

/// `((a & 15) ^ y) & 15` drops the inner mask, which the outer one covers
/// (SimplifyDemandedInstructionBits runs before the arm-mask fold, as in the
/// pin).
#[test]
fn the_outer_mask_makes_the_inner_one_redundant() {
    check(
        "i32",
        "i32 %a, i32 %y",
        "  %x = and i32 %a, 15\n  %e = xor i32 %x, %y\n  %r = and i32 %e, 15\n  ret i32 %r\n",
        "  %e = xor i32 %a, %y\n  %r = and i32 %e, 15\n  ret i32 %r\n",
    );
}

/// `(4 << x) & 16` is `x == 2 ? 16 : 0` (`opt` agrees).
#[test]
fn a_shifted_power_of_two_masked_is_a_compare_select() {
    check(
        "i32",
        "i32 %x",
        "  %s = shl i32 4, %x\n  %r = and i32 %s, 16\n  ret i32 %r\n",
        "  %0 = icmp eq i32 %x, 2\n  %1 = select i1 %0, i32 16, i32 0\n  ret i32 %1\n",
    );
}

/// FILL's `(i & 15) + *x` kept the 15 for an `i` that starts at 0 and steps by
/// 2: `visitAnd` runs `SimplifyDemandedInstructionBits` (DEVIATE 2 left it
/// out), whose `ShrinkDemandedConstant` leaves out the bit the recurrence
/// keeps clear, and the pin writes `and %i, 14`.
#[test]
fn an_and_with_a_mask_drops_the_bit_an_induction_variable_keeps_clear() {
    let text = "define i32 @f(i32 %n) {
b0:
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %next, %b1 ]
  %m = and i32 %i, 15
  %next = add nuw nsw i32 %i, 2
  %c = icmp ult i32 %next, %n
  br i1 %c, label %b1, label %b2

b2:
  ret i32 %m
}
";
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    let after = printed(&module);
    assert!(after.contains("and i32 %i, 14"), "{after}");
}

/// A mask of a load with a second user crashed in
/// `simplify_multiple_use_demanded_bits`, which read operand 1 of every opcode:
/// index out of bounds on the load.
#[test]
fn a_mask_of_a_load_with_two_users_is_left_alone() {
    let body = "  %l = load i32, ptr %p\n  %a = and i32 %l, 255\n  %r = add i32 %l, %a\n  ret i32 %r\n";
    check("i32", "ptr %p", body, body);
}
