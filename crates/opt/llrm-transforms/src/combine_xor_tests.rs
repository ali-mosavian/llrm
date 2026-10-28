use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visit` that does only `visitXor`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if c.function.instruction(inst).opcode == Opcode::Binary(BinaryOp::Xor) {
        super::visit_xor(c, inst)
    } else {
        Visit::None
    }
}

/// `@f(params)` returning i32 after `visitXor` alone.
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

/// `body` must come out with each of `wanted` and none of `unwanted`, the
/// shape pinned `opt -passes=instcombine` gives.
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

const X: &str = "i32 %x";
const XY: &str = "i32 %x, i32 %y";
const ABC: &str = "i32 %a, i32 %b, i32 %c";
const AB: &str = "i32 %a, i32 %b";

/// `(X | 12) ^ 5` is `(X & -13) ^ 9`: the `or` was the xor's operand.
#[test]
fn an_or_with_a_constant_under_a_xor_constant_becomes_a_mask() {
    check(X, "  %o = or i32 %x, 12\n  %r = xor i32 %o, 5\n", &["and i32 %x, -13", "xor i32 %", ", 9"], &[" or i32"]);
}

#[test]
fn a_not_under_an_or_or_and_constant_loses_the_not() {
    check(
        X,
        "  %n = xor i32 %x, -1\n  %o = or i32 %n, 12\n  %r = xor i32 %o, 5\n",
        &["and i32 %x, -13", ", -6"],
        &[" or i32"],
    );
    check(
        X,
        "  %n = xor i32 %x, -1\n  %o = and i32 %n, 12\n  %r = xor i32 %o, 5\n",
        &["and i32 %x, 12", ", 9"],
        &[" or i32"],
    );
}

/// `xor (ashr X, 31), 5` is `select (X >s -1), 5, -6`.
#[test]
fn a_xor_of_the_sign_splat_selects_between_the_constant_and_its_not() {
    check(X, "  %s = ashr i32 %x, 31\n  %r = xor i32 %s, 5\n", &["icmp sgt i32 %x, -1", "i32 5, i32 -6"], &["ashr"]);
}

#[test]
fn a_sign_mask_xor_folds_into_the_add_or_sub_constant() {
    check(X, "  %s = sub i32 7, %x\n  %r = xor i32 %s, -2147483648\n", &["sub i32 -2147483641, %x"], &["xor"]);
    check(X, "  %s = add i32 %x, 7\n  %r = xor i32 %s, -2147483648\n", &["add i32 %x, -2147483641"], &["xor"]);
}

/// The xor inverts exactly the bits a shift left alone: `~X << 4`.
#[test]
fn a_xor_of_the_bits_a_shift_leaves_is_a_not_before_the_shift() {
    check(X, "  %s = shl i32 %x, 4\n  %r = xor i32 %s, -16\n", &["xor i32 %x, -1", "shl i32 %"], &[", -16"]);
    check(
        X,
        "  %s = lshr i32 %x, 4\n  %r = xor i32 %s, 268435455\n",
        &["xor i32 %x, -1", "lshr i32 %"],
        &["268435455"],
    );
}

#[test]
fn a_xor_constant_under_a_lshr_moves_past_it() {
    check(
        X,
        "  %e = xor i32 %x, 255\n  %s = lshr i32 %e, 4\n  %r = xor i32 %s, 3\n",
        &["lshr i32 %x, 4", ", 12"],
        &["255"],
    );
}

#[test]
fn a_xor_with_an_or_or_and_that_shares_an_operand_is_an_and_with_a_not() {
    check(XY, "  %o = or i32 %x, %y\n  %r = xor i32 %y, %o\n", &["xor i32 %y, -1", "and i32 %x, %0"], &[" or i32"]);
    check(XY, "  %o = and i32 %x, %y\n  %r = xor i32 %o, %y\n", &["xor i32 %x, -1", "and i32 %"], &[" xor i32 %o"]);
}

#[test]
fn a_xor_of_a_xor_and_an_or_sharing_an_operand_is_a_masked_xor() {
    check(
        ABC,
        "  %x = xor i32 %a, %b\n  %o = or i32 %a, %c\n  %r = xor i32 %x, %o\n",
        &["xor i32 %a, -1", "xor i32 %", ", %b"],
        &[" or i32"],
    );
}

#[test]
fn an_and_xored_with_the_xor_of_its_operands_is_their_or() {
    check(
        AB,
        "  %x = xor i32 %a, %b\n  %o = and i32 %a, %b\n  %r = xor i32 %o, %x\n",
        &["or i32 %a, %b"],
        &["and", "xor"],
    );
}

#[test]
fn two_ors_with_a_common_operand_xor_to_an_and_with_its_not() {
    check(
        ABC,
        "  %o1 = or i32 %a, %b\n  %o2 = or i32 %a, %c\n  %r = xor i32 %o1, %o2\n",
        &["xor i32 %a, -1", "xor i32 %b, %c", "and i32 %"],
        &[" or i32"],
    );
}

#[test]
fn a_not_and_xored_with_its_operand_is_an_or_and_a_not_or_a_nand() {
    check(
        AB,
        "  %n = xor i32 %a, -1\n  %o = and i32 %n, %b\n  %r = xor i32 %o, %a\n",
        &["or i32 %a, %b"],
        &["and", "xor"],
    );
    check(
        AB,
        "  %n = xor i32 %a, -1\n  %o = or i32 %n, %b\n  %r = xor i32 %o, %a\n",
        &["and i32 %a, %b", "xor i32 %", ", -1"],
        &[" or i32"],
    );
}

/// `(X ^ 77) ^ Y` is `(X ^ Y) ^ 77`.
#[test]
fn a_xor_constant_moves_out_of_the_inner_xor() {
    check(XY, "  %e = xor i32 %x, 77\n  %r = xor i32 %e, %y\n", &["xor i32 %x, %y", ", 77"], &[]);
}

/// `foldNot`: `not (cmp A, B)` is the inverse compare. Left as `xor (icmp eq (and x, 1), 0), true`, the
/// `zext` of it never became `and x, 1`, and QB `rounded()` was 28 instructions against the pin's 14.
#[test]
fn a_not_of_a_one_use_compare_is_the_inverse_compare() {
    let mut module = parsed(
        "define i32 @f(i32 %x) {
b0:
  %a = and i32 %x, 1
  %c = icmp eq i32 %a, 0
  %n = xor i1 %c, true
  %z = zext i1 %n to i32
  ret i32 %z
}
",
    );
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    let out = printed(&module);
    assert!(out.contains("icmp ne i32 %a, 0") && !out.contains("xor i1"), "{out}");
}

/// `foldNot`'s DeMorgan arms: `~(~X & Y)` is `X | ~Y`, `~(~X | Y)` is `X & ~Y`.
#[test]
fn a_not_of_a_logic_op_over_a_not_is_demorgan() {
    check(
        XY,
        "  %n = xor i32 %x, -1\n  %a = and i32 %n, %y\n  %r = xor i32 %a, -1\n",
        &[" or i32 %x,"],
        &[" and i32"],
    );
    check(
        XY,
        "  %n = xor i32 %x, -1\n  %a = or i32 %n, %y\n  %r = xor i32 %a, -1\n",
        &[" and i32 %x,"],
        &[" or i32"],
    );
}

/// `~((-X) | Y)` is `(X - 1) & ~Y`.
#[test]
fn a_not_of_a_negated_or_is_a_decrement_and_mask() {
    check(
        XY,
        "  %g = sub i32 0, %x\n  %o = or i32 %g, %y\n  %r = xor i32 %o, -1\n",
        &["add i32 %x, -1", " and i32"],
        &[" or i32", "sub i32 0"],
    );
}

/// The shift arms: `~(~X >>s Y)` is `X >>s Y`; `~(X >>s 31)` is `sext (X > -1)`; `~(C >>s Y)` for
/// a negative `C` is `~C >>u Y`.
#[test]
fn a_not_of_a_shift_inverts_the_shift_or_becomes_a_sign_test() {
    check(XY, "  %n = xor i32 %x, -1\n  %s = ashr i32 %n, %y\n  %r = xor i32 %s, -1\n", &["ashr i32 %x, %y"], &["xor"]);
    check(X, "  %s = ashr i32 %x, 31\n  %r = xor i32 %s, -1\n", &["icmp sgt i32 %x, -1", "sext i1"], &["xor", "ashr"]);
    check("i32 %y", "  %s = ashr i32 -8, %y\n  %r = xor i32 %s, -1\n", &["lshr i32 7, %y"], &["xor", "ashr"]);
}

/// `~(X + C)` is `~C - X`; `~(X - Y)` is `~X + Y`; `~(~X + Y)` is `X - Y`.
#[test]
fn a_not_of_an_add_or_sub_moves_into_the_operands() {
    check(X, "  %a = add i32 %x, 5\n  %r = xor i32 %a, -1\n", &["sub i32 -6, %x"], &["xor", "add"]);
    check(XY, "  %a = sub i32 %x, %y\n  %r = xor i32 %a, -1\n", &["add i32", ", %y"], &["sub"]);
    check(
        XY,
        "  %n = xor i32 %x, -1\n  %a = add i32 %n, %y\n  %r = xor i32 %a, -1\n",
        &["sub i32 %x, %y"],
        &["xor", "add"],
    );
}

/// `~select(c, cmp1, cmp2)` of one-use compares inverts both.
#[test]
fn a_not_of_a_select_of_compares_inverts_the_compares() {
    let mut module = parsed(
        "define i1 @f(i1 %c, i32 %x, i32 %y) {
b0:
  %p = icmp slt i32 %x, %y
  %q = icmp eq i32 %x, 0
  %s = select i1 %c, i1 %p, i1 %q
  %r = xor i1 %s, true
  ret i1 %r
}
",
    );
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    let out = printed(&module);
    assert!(out.contains("icmp sge i32 %x, %y") && out.contains("icmp ne i32 %x, 0") && !out.contains("xor"), "{out}");
}
