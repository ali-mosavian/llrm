use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use super::{should_change_type, simplify_associative_or_commutative};
use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visit` that does only `SimplifyAssociativeOrCommutative` of a binary
/// operator.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if matches!(c.function.instruction(inst).opcode, Opcode::Binary(_)) && simplify_associative_or_commutative(c, inst)
    {
        Visit::Changed
    } else {
        Visit::None
    }
}

/// `@f` after the helper alone, through the driver.
fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

fn check(
    body: &str,
    params: &str,
    expected: &str,
) {
    let text = format!("define i16 @f({params}) {{\nb0:\n{body}}}\n");
    let want = format!("define i16 @f({params}) {{\nb0:\n{expected}}}\n");
    assert_eq!(combined(&text), want);
}

/// A constant goes to the right: `getComplexity` is 1 for it and 3 for an
/// argument. Seen in `opt -passes=instcombine`: `add i16 %x, 1`.
#[test]
fn a_constant_on_the_left_of_a_commutative_operation_moves_right() {
    check("  %a = add i16 1, %x\n  ret i16 %a\n", "i16 %x", "  %a = add i16 %x, 1\n  ret i16 %a\n");
}

/// `(A op B) op C` becomes `A op (B op C)` where `B op C` folds: `opt` gives
/// `add i16 %x, 3` and `and i16 %x, 8`.
#[test]
fn a_left_chain_with_two_constants_folds_them() {
    check(
        "  %a = add i16 %x, 1\n  %b = add i16 %a, 2\n  ret i16 %b\n",
        "i16 %x",
        "  %b = add i16 %x, 3\n  ret i16 %b\n",
    );
    check(
        "  %a = and i16 %x, 12\n  %b = and i16 %a, 10\n  ret i16 %b\n",
        "i16 %x",
        "  %b = and i16 %x, 8\n  ret i16 %b\n",
    );
}

/// `nuw` and `nsw` stay where the constants do not overflow (`opt`:
/// `add nuw nsw i16 %x, 3`).
#[test]
fn wrap_flags_stay_when_the_constants_do_not_overflow() {
    check(
        "  %a = add nuw nsw i16 %x, 1\n  %b = add nuw nsw i16 %a, 2\n  ret i16 %b\n",
        "i16 %x",
        "  %b = add nuw nsw i16 %x, 3\n  ret i16 %b\n",
    );
}

/// `maintainNoSignedWrap`: 32767 + 1 overflows an `i16`, so `nsw` goes and
/// `nuw` stays. (`opt` goes on to `or disjoint`: its `visitAdd`, not this
/// helper.)
#[test]
fn nsw_goes_when_the_constants_overflow() {
    check(
        "  %a = add nuw nsw i16 %x, 32767\n  %b = add nuw nsw i16 %a, 1\n  ret i16 %b\n",
        "i16 %x",
        "  %b = add nuw i16 %x, -32768\n  ret i16 %b\n",
    );
}

/// `(A op C1) op (B op C2)` becomes `(A op B) op (C1 op C2)` when both inner
/// operations have one use. `opt`: `%x + %y` then `+ 3`.
#[test]
fn two_inner_operations_with_a_constant_each_join() {
    check(
        "  %a = add i16 %x, 1\n  %b = add i16 %y, 2\n  %c = add i16 %a, %b\n  ret i16 %c\n",
        "i16 %x, i16 %y",
        "  %0 = add i16 %x, %y\n  %c = add i16 %0, 3\n  ret i16 %c\n",
    );
}

/// Not with a second use: the `add`s are left as they are.
#[test]
fn an_inner_operation_with_two_uses_is_not_joined() {
    let text = "define i16 @f(i16 %x, i16 %y) {\nb0:\n  %a = add i16 %x, 1\n  %b = add i16 %y, 2\n  %c = add i16 %a, %b\n  %d = mul i16 %a, %c\n  ret i16 %d\n}\n";
    assert_eq!(combined(text), text);
}

/// `matchSymmetricPair`: `select c, a, b` and `select c, b, a` multiply as `a
/// * b` (`opt`: `mul i16 %a, %b`).
#[test]
fn a_swapped_select_pair_is_replaced_by_its_two_arms() {
    check(
        "  %s = select i1 %c, i16 %a, i16 %b\n  %t = select i1 %c, i16 %b, i16 %a\n  %r = mul i16 %s, %t\n  ret i16 %r\n",
        "i1 %c, i16 %a, i16 %b",
        "  %r = mul i16 %a, %b\n  ret i16 %r\n",
    );
}

/// `simplifyAssocCastAssoc`: `and (zext (and X, 15)), 255` is `and (zext X),
/// 15`, the constants folded in the wide type.
#[test]
fn constants_either_side_of_a_zext_fold_together() {
    let text = "define i16 @f(i8 %x) {\nb0:\n  %a = and i8 %x, 15\n  %z = zext i8 %a to i16\n  %r = and i16 %z, 255\n  ret i16 %r\n}\n";
    let want = "define i16 @f(i8 %x) {\nb0:\n  %z = zext i8 %x to i16\n  %r = and i16 %z, 15\n  ret i16 %r\n}\n";
    assert_eq!(combined(text), want);
}

/// `shouldChangeType`: shrink to a desirable width, never from a legal or
/// desirable width to an illegal one, never grow when both are illegal.
#[test]
fn should_change_type_follows_the_native_widths() {
    let layout = DataLayout::parse("e-n16:32").expect("parses");
    assert!(should_change_type(&layout, 32, 8), "shrink to i8");
    assert!(should_change_type(&layout, 17, 16), "illegal to legal");
    assert!(!should_change_type(&layout, 16, 17), "legal to illegal");
    assert!(!should_change_type(&layout, 32, 64), "i64 is not native here");
    assert!(!should_change_type(&layout, 17, 33), "both illegal: no growth");
    assert!(should_change_type(&layout, 33, 17), "both illegal: shrink");
}
