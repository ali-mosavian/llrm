//! Each case is a function pinned `opt -passes=instcombine` rewrites by a
//! distributive law; `visit` runs the six `visit*` that call
//! `foldUsingDistributiveLaws` and nothing else. Before it was wired in, each
//! left both operations of the input in place.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    match c.function.instruction(inst).opcode {
        Opcode::Binary(BinaryOp::Add) => crate::combine_addsub::visit_add(c, inst),
        Opcode::Binary(BinaryOp::Sub) => crate::combine_sub::visit_sub(c, inst),
        Opcode::Binary(BinaryOp::Mul) => crate::combine_muldivrem::visit_mul(c, inst),
        Opcode::Binary(BinaryOp::And) => crate::combine_and::visit_and(c, inst),
        Opcode::Binary(BinaryOp::Or) => crate::combine_or::visit_or(c, inst),
        Opcode::Binary(BinaryOp::Xor) => crate::combine_xor::visit_xor(c, inst),
        _ => Visit::None,
    }
}

/// `@f(params)` returning i32 after the six `visit*`.
fn combined(
    params: &str,
    body: &str,
) -> String {
    let mut module = parsed(&format!("define i32 @f({params}) {{\nb0:\n{body}  ret i32 %r\n}}\n"));
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// `body` comes out with each of `wanted` and none of `unwanted`.
fn check(
    params: &str,
    body: &str,
    wanted: &[&str],
    unwanted: &[&str],
) {
    let out = combined(params, body);
    for one in wanted {
        assert!(out.contains(one), "`{one}` missing\n{out}");
    }
    for one in unwanted {
        assert!(!out.contains(one), "`{one}` present\n{out}");
    }
}

const XYZ: &str = "i32 %x, i32 %y, i32 %z";

/// `x*y + x*z` was left as two `mul`s and an `add`; it is `x * (y + z)`.
#[test]
fn a_sum_of_products_with_a_common_factor_is_one_product() {
    check(
        XYZ,
        "  %a = mul i32 %x, %y\n  %b = mul i32 %x, %z\n  %r = add i32 %a, %b\n",
        &["add i32 %y, %z", "mul i32 %x, %"],
        &["mul i32 %x, %y"],
    );
}

/// The common factor on the other side of the commutative `mul`, and `sub`.
#[test]
fn a_difference_of_products_with_a_common_factor_is_one_product() {
    check(
        XYZ,
        "  %a = mul i32 %x, %y\n  %b = mul i32 %z, %x\n  %r = sub i32 %a, %b\n",
        &["sub i32 %y, %z", "mul i32 %x, %"],
        &["mul i32 %x, %y"],
    );
}

/// `(a|b) & (a|c)` was left as three operations; it is `a | (b & c)`.
#[test]
fn an_and_of_ors_sharing_an_operand_is_an_or_of_the_rest() {
    check(
        XYZ,
        "  %p = or i32 %x, %y\n  %q = or i32 %x, %z\n  %r = and i32 %p, %q\n",
        &["and i32 %y, %z", "or i32 %x, %"],
        &["or i32 %x, %y"],
    );
}

/// `(x|y) & ~x`: `x & ~x` is the identity of `or`, so the `or` is dropped.
#[test]
fn an_and_with_the_not_of_an_or_operand_drops_the_or() {
    check(
        "i32 %x, i32 %y",
        "  %n = xor i32 %x, -1\n  %o = or i32 %x, %y\n  %r = and i32 %o, %n\n",
        &["and i32 %y, %n"],
        &[" or i32"],
    );
}

/// `(x << 2) + x` was left as a `shl` and an `add`; it is `x * 5`.
#[test]
fn a_shift_plus_its_operand_is_a_product() {
    check("i32 %x", "  %s = shl i32 %x, 2\n  %r = add i32 %s, %x\n", &["mul i32 %x, 5"], &["shl", "add"]);
}

/// A shift of both operands by one amount: `(x >> s) ^ (y >> s)` is `(x ^ y) >>
/// s`.
#[test]
fn a_xor_of_shifts_by_one_amount_is_a_shift_of_the_xor() {
    check(
        "i32 %x, i32 %y, i32 %s",
        "  %a = lshr i32 %x, %s\n  %b = lshr i32 %y, %s\n  %r = xor i32 %a, %b\n",
        &["xor i32 %x, %y", "lshr i32 %"],
        &["lshr i32 %x, %s"],
    );
}

/// `3x + x` with `nuw nsw` on both is `4x` with both flags: the factored
/// product keeps what holds of the operands.
#[test]
fn a_factored_sum_keeps_nuw_and_nsw_of_the_operands() {
    check(
        "i32 %x",
        "  %m = mul nuw nsw i32 %x, 3\n  %r = add nuw nsw i32 %m, %x\n",
        &["nuw nsw i32 %x, 2", "shl nuw nsw"],
        &["add"],
    );
}
