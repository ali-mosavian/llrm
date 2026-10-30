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

/// `(zext x << 24) & signmask` is `sext x & signmask` (`opt` agrees).
#[test]
fn a_shifted_zext_sign_mask_is_a_sext_mask() {
    check(
        "i32",
        "i8 %x",
        "  %z = zext i8 %x to i32\n  %s = shl i32 %z, 24\n  %r = and i32 %s, -2147483648\n  ret i32 %r\n",
        "  %0 = sext i8 %x to i32\n  %1 = and i32 %0, -2147483648\n  ret i32 %1\n",
    );
}

/// `x & ~(x ^ y)` is `x & y` (`opt` agrees).
#[test]
fn an_operand_and_the_not_of_its_xor_is_an_and() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %e = xor i32 %x, %y\n  %n = xor i32 %e, -1\n  %r = and i32 %x, %n\n  ret i32 %r\n",
        "  %0 = and i32 %x, %y\n  ret i32 %0\n",
    );
}

/// `(x | y) & (~x ^ y)` is `x & y` (`opt` prints `y & x`).
#[test]
fn an_or_and_a_xor_with_a_not_is_an_and() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %nx = xor i32 %x, -1\n  %o = or i32 %x, %y\n  %e = xor i32 %nx, %y\n  %r = and i32 %o, %e\n  ret i32 %r\n",
        "  %0 = and i32 %x, %y\n  ret i32 %0\n",
    );
}

/// `(~x | y) & (x ^ y)` is `~x & y` (`opt` agrees).
#[test]
fn a_not_or_and_a_xor_is_a_not_and() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %nx = xor i32 %x, -1\n  %o = or i32 %nx, %y\n  %e = xor i32 %x, %y\n  %r = and i32 %o, %e\n  ret i32 %r\n",
        "  %0 = xor i32 %x, -1\n  %1 = and i32 %y, %0\n  ret i32 %1\n",
    );
}

/// `sext(a) & b` is `a ? b : 0` (`opt` agrees).
#[test]
fn a_sext_bool_and_b_is_a_select() {
    check(
        "i32",
        "i1 %a, i32 %b",
        "  %s = sext i1 %a to i32\n  %r = and i32 %s, %b\n  ret i32 %r\n",
        "  %0 = select i1 %a, i32 %b, i32 0\n  ret i32 %0\n",
    );
}

/// `~sext(a) & b` is `a ? 0 : b` (`opt` agrees).
#[test]
fn a_not_sext_bool_and_b_is_the_swapped_select() {
    check(
        "i32",
        "i1 %a, i32 %b",
        "  %s = sext i1 %a to i32\n  %n = xor i32 %s, -1\n  %r = and i32 %n, %b\n  ret i32 %r\n",
        "  %0 = select i1 %a, i32 0, i32 %b\n  ret i32 %0\n",
    );
}

/// `zext(a) & b` is `a ? (b & 1) : 0` (`opt` agrees).
#[test]
fn a_zext_bool_and_b_masks_b_to_one_bit_in_a_select() {
    check(
        "i32",
        "i1 %a, i32 %b",
        "  %s = zext i1 %a to i32\n  %r = and i32 %s, %b\n  ret i32 %r\n",
        "  %0 = and i32 %b, 1\n  %1 = select i1 %a, i32 %0, i32 0\n  ret i32 %1\n",
    );
}

/// `(zext a - 1) & b` is `a ? 0 : b` (`opt` agrees).
#[test]
fn a_decremented_flag_and_b_is_a_select() {
    check(
        "i32",
        "i1 %a, i32 %b",
        "  %z = zext i1 %a to i32\n  %d = add i32 %z, -1\n  %r = and i32 %d, %b\n  ret i32 %r\n",
        "  %0 = select i1 %a, i32 0, i32 %b\n  ret i32 %0\n",
    );
}

/// `(x >>s 31) & y` is `x < 0 ? y : 0` (`opt` agrees).
#[test]
fn a_sign_splat_and_y_is_a_select_on_the_sign() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %s = ashr i32 %x, 31\n  %r = and i32 %s, %y\n  ret i32 %r\n",
        "  %0 = icmp slt i32 %x, 0\n  %1 = select i1 %0, i32 %y, i32 0\n  ret i32 %1\n",
    );
}

/// `~(x >>s 31) & y` is `x < 0 ? 0 : y` (`opt` agrees).
#[test]
fn a_not_sign_splat_and_y_is_the_swapped_select() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %s = ashr i32 %x, 31\n  %n = xor i32 %s, -1\n  %r = and i32 %n, %y\n  ret i32 %r\n",
        "  %0 = icmp slt i32 %x, 0\n  %1 = select i1 %0, i32 0, i32 %y\n  ret i32 %1\n",
    );
}
