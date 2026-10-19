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
    let decls = "declare i16 @llvm.umax.i16(i16, i16)\n";
    let text = format!("{decls}define i1 @f({params}) {{\nb0:\n{body}}}\n");
    let want = format!("{decls}define i1 @f({params}) {{\nb0:\n{after}}}\n");
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

/// `foldICmpWithZero`: `(x | 1) * y == 0` is `y == 0` (`opt`: `icmp eq i16 %y, 0`).
#[test]
fn a_product_with_an_odd_factor_is_zero_when_the_other_is() {
    check(
        "i16 %x, i16 %y",
        "  %o = or i16 %x, 1\n  %m = mul i16 %o, %y\n  %r = icmp eq i16 %m, 0\n  ret i1 %r\n",
        "  %o = or i16 %x, 1\n  %m = mul i16 %o, %y\n  %0 = icmp eq i16 %y, 0\n  ret i1 %0\n",
    );
}

/// `umax(x, 5) u> ~x` is `x s< 0`, and the compares of the sign bit
/// (`opt`: `icmp slt i16 %x, 0`, `icmp sgt i16 %x, -1`).
#[test]
fn compares_that_are_sign_tests() {
    check(
        "i16 %x",
        "  %m = call i16 @llvm.umax.i16(i16 %x, i16 5)\n  %n = xor i16 %x, -1\n  %r = icmp ugt i16 %m, %n\n  ret i1 %r\n",
        "  %m = call i16 @llvm.umax.i16(i16 %x, i16 5)\n  %n = xor i16 %x, -1\n  %0 = icmp slt i16 %x, 0\n  ret i1 %0\n",
    );
    check("i16 %x", "  %r = icmp ugt i16 %x, 32767\n  ret i1 %r\n", "  %0 = icmp slt i16 %x, 0\n  ret i1 %0\n");
    check("i16 %x", "  %r = icmp ult i16 %x, 32768\n  ret i1 %r\n", "  %0 = icmp sgt i16 %x, -1\n  ret i1 %0\n");
}
