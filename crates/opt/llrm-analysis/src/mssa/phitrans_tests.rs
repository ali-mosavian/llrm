//! `PHITransAddr`: the address of a load at a join, as it is in a predecessor.

use llrm_mir::domtree::DomTree;
use llrm_mir::module::{Function, Operand};

use super::{Env, PhiTransAddr};
use crate::testing::{DOS, block, function, layout, parsed, value};

/// A diamond: b0 branches to b1 and b2, b1 to b2; @f's body is `b0`'s
/// instructions, `b1`'s and `b2`'s, whose first instruction is the phi `%p`
/// (or `%v`) the cases translate.
fn diamond(
    b0: &str,
    b1: &str,
    b2: &str,
) -> String {
    format!(
        "{DOS}@g = global [256 x i8] zeroinitializer\n\ndefine void @f(i1 %c, ptr %x, ptr %y, i16 %n) {{\nb0:\n{b0}\n  br i1 %c, label %b1, label %b2\n\nb1:\n{b1}\n  br label %b2\n\nb2:\n{b2}\n  ret void\n}}\n"
    )
}

fn name(
    function: &Function,
    got: Option<Operand>,
) -> String {
    match got {
        None => "none".into(),
        Some(Operand::Value(id)) => format!("%{}", function.value(id).name.as_deref().unwrap_or("?")),
        Some(Operand::Constant(_)) => "constant".into(),
        Some(Operand::Block(_)) => unreachable!("an address"),
    }
}

/// `start` translated from b2 to `pred`.
fn translated(
    text: &str,
    start: &str,
    pred: &str,
    must_dominate: bool,
) -> String {
    let module = parsed(text);
    let (layout, function) = (layout(&module), function(&module, "f"));
    let (tree, mut context) = (DomTree::of(function), module.context.clone());
    let mut one = PhiTransAddr::new(function, Operand::Value(value(function, start)));
    let mut env = Env { context: &mut context, layout: &layout, function, dt: Some(&tree) };
    let got = one.translate_value(&mut env, block(function, "b2"), block(function, pred), must_dominate);
    name(function, got)
}

#[test]
fn test_a_phi_translates_to_its_incoming_value() {
    let text = diamond("", "", "  %p = phi ptr [ %x, %b0 ], [ %y, %b1 ]");
    assert_eq!(translated(&text, "p", "b0", true), "%x");
    assert_eq!(translated(&text, "p", "b1", true), "%y");
}

#[test]
fn test_a_gep_of_a_phi_is_found_among_the_users_of_its_translated_base() {
    let text = diamond(
        "  %a4 = getelementptr i8, ptr %x, i16 4",
        "",
        "  %p = phi ptr [ %x, %b0 ], [ %y, %b1 ]\n  %q = getelementptr i8, ptr %p, i16 4",
    );
    assert_eq!(translated(&text, "q", "b0", true), "%a4");
    assert_eq!(translated(&text, "q", "b1", true), "none", "no GEP off %y is available");
}

#[test]
fn test_a_gep_translated_to_a_zero_offset_is_its_base() {
    let text = diamond("", "", "  %p = phi i16 [ 0, %b0 ], [ %n, %b1 ]\n  %q = getelementptr i8, ptr %x, i16 %p");
    assert_eq!(translated(&text, "q", "b0", true), "%x");
}

#[test]
fn test_a_cast_of_a_phi_of_a_constant_folds() {
    let text = diamond("", "", "  %p = phi i16 [ 3, %b0 ], [ %n, %b1 ]\n  %q = zext i16 %p to i32");
    assert_eq!(translated(&text, "q", "b0", true), "constant");
    assert_eq!(translated(&text, "q", "b1", true), "none", "no zext of %n is available");
}

#[test]
fn test_the_newest_of_two_equal_casts_is_returned() {
    let text = diamond(
        "  %c1 = zext i16 %n to i32\n  %c2 = zext i16 %n to i32",
        "",
        "  %p = phi i16 [ %n, %b0 ], [ 1, %b1 ]\n  %q = zext i16 %p to i32",
    );
    // LLVM walks a newest-first use list and finds %c2.
    assert_eq!(translated(&text, "q", "b0", true), "%c2");
}

#[test]
fn test_an_add_of_a_constant_folds_with_an_add_on_the_translated_left() {
    let text = diamond(
        "  %t = add i16 %n, 3\n  %a5 = add i16 %n, 5",
        "",
        "  %p = phi i16 [ %t, %b0 ], [ %n, %b1 ]\n  %q = add i16 %p, 2",
    );
    assert_eq!(translated(&text, "q", "b0", true), "%a5");
    assert_eq!(translated(&text, "q", "b1", true), "none", "no add of %n and 2 is available");
}

#[test]
fn test_a_gep_off_a_global_base_folds_to_a_constant_expression() {
    let text = diamond(
        "  %a4 = getelementptr i8, ptr @g, i16 4",
        "",
        "  %p = phi ptr [ @g, %b0 ], [ %y, %b1 ]\n  %q = getelementptr i8, ptr %p, i16 4",
    );
    // `simplifyGEPInst` makes `gep @g, 4` a constant expression, as in LLVM.
    assert_eq!(translated(&text, "q", "b0", true), "constant");
}

/// A GEP off `@g` that is already in the predecessor was not found: a constant
/// has no use list, so PRE of every load of a global array element through a
/// phi or select index (x_heapsort's sift loop) found nothing available.
#[test]
fn test_a_gep_off_a_global_base_is_found_in_a_dominating_block() {
    let text = diamond(
        "  %a = getelementptr i32, ptr @g, i16 %n",
        "",
        "  %p = phi i16 [ %n, %b0 ], [ 1, %b1 ]\n  %q = getelementptr i32, ptr @g, i16 %p",
    );
    assert_eq!(translated(&text, "q", "b0", true), "%a");
    assert_eq!(translated(&text, "q", "b1", true), "constant", "gep @g, 1 folds to a constant expression");
}

#[test]
fn test_a_gep_off_a_global_base_is_not_invented_when_no_block_has_it() {
    let text = diamond(
        "  %a = getelementptr i32, ptr @g, i16 3",
        "",
        "  %p = phi i16 [ %n, %b0 ], [ 1, %b1 ]\n  %q = getelementptr i32, ptr @g, i16 %p",
    );
    assert_eq!(translated(&text, "q", "b0", true), "none", "no gep of @g by %n is available");
}

#[test]
fn test_a_gep_off_a_global_base_in_a_sibling_block_is_not_available() {
    // %a is in b1, which does not dominate b0: a scan of the whole function
    // would find it.
    let text = diamond(
        "",
        "  %a = getelementptr i32, ptr @g, i16 %n",
        "  %p = phi i16 [ %n, %b0 ], [ 1, %b1 ]\n  %q = getelementptr i32, ptr @g, i16 %p",
    );
    assert_eq!(translated(&text, "q", "b0", true), "none");
}

#[test]
fn test_a_select_on_the_condition_resolves_to_each_side() {
    let module = parsed(&diamond("  %s = select i1 %c, ptr %x, ptr %y", "", "  %p = phi ptr [ %s, %b0 ], [ %y, %b1 ]"));
    let (layout, function) = (layout(&module), function(&module, "f"));
    let (tree, mut context) = (DomTree::of(function), module.context.clone());
    let one = PhiTransAddr::new(function, Operand::Value(value(function, "p")));
    let mut env = Env { context: &mut context, layout: &layout, function, dt: Some(&tree) };
    let cond = Operand::Value(value(function, "c"));
    let (yes, no) = one.translate_value_select(&mut env, block(function, "b2"), block(function, "b0"), cond);
    assert_eq!((name(function, yes), name(function, no)), ("%x".into(), "%y".into()));
}

#[test]
fn test_the_inputs_say_where_translation_is_needed() {
    let module =
        parsed(&diamond("", "", "  %p = phi ptr [ %x, %b0 ], [ %y, %b1 ]\n  %q = getelementptr i8, ptr %p, i16 4"));
    let function = function(&module, "f");
    let one = PhiTransAddr::new(function, Operand::Value(value(function, "q")));
    assert!(one.is_potentially_phi_translatable(&module.context, function));
    assert!(one.verify(&module.context, function));
    assert!(one.needs_phi_translation_from_block(function, block(function, "b2")));
    assert!(!one.needs_phi_translation_from_block(function, block(function, "b0")));
}

#[test]
fn test_an_unreachable_predecessor_has_no_translation() {
    let text = format!(
        "{DOS}define void @f(ptr %x, ptr %y) {{\nb0:\n  br label %b2\n\ndead:\n  br label %b2\n\nb2:\n  %p = phi ptr [ %x, %b0 ], [ %y, %dead ]\n  ret void\n}}\n"
    );
    assert_eq!(translated(&text, "p", "dead", false), "none");
    assert_eq!(translated(&text, "p", "b0", false), "%x");
}

#[test]
fn test_insertion_makes_the_missing_gep_at_the_end_of_the_predecessor_or_nothing() {
    let text = diamond("", "", "  %p = phi ptr [ %x, %b0 ], [ %y, %b1 ]\n  %q = getelementptr i8, ptr %p, i16 4");
    let module = parsed(&text);
    let layout = layout(&module);
    let mut context = module.context.clone();
    let mut function = function(&module, "f").clone();
    let tree = DomTree::of(&function);
    let (b0, b2) = (block(&function, "b0"), block(&function, "b2"));
    let q = Operand::Value(value(&function, "q"));
    let mut made = Vec::new();
    let got = PhiTransAddr::new(&function, q).translate_with_insertion(
        &mut context,
        &layout,
        &mut function,
        &tree,
        b2,
        b0,
        &mut made,
    );
    assert_eq!(made.len(), 1);
    assert_eq!(got, Some(Operand::Value(function.instruction(made[0]).result.expect("a value"))));
    assert_eq!(function.parent(made[0]), Some(b0));
    assert!(function.terminator(b0).is_some_and(|t| function.instruction(t).opcode.is_terminator()));
    assert_eq!(function.block(b0).len(), 2, "the GEP and the branch");
}
