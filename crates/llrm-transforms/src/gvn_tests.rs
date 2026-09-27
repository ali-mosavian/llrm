//! `joined`'s PRE and the `Gvn` pass, each body MIR text run by llrm-mir's
//! interpreter before and after.

use llrm_mir::passes::PassManager;

use crate::testing::{f, parsed, printed, results};

use super::{Gvn, joined};

const INPUTS: &[&[i128]] = &[&[0, 0, 0], &[3, 5, 1], &[-7, 2, 0], &[0x7fff, 1, 1]];

/// A diamond on %c whose arms are `left` and `right`, joining at b3 to
/// compute `join` into %r.
fn diamond(left: &str, right: &str, join: &str) -> String {
    format!(
        "define i16 @f(i16 %x, i16 %y, i1 %c) {{
b0:
  br i1 %c, label %b1, label %b2

b1:
{left}  br label %b3

b2:
{right}  br label %b3

b3:
{join}  ret i16 %r
}}
"
    )
}

/// `text` joined: its printed form, and whether anything changed. What
/// `@f` returns is what it returned before.
fn joined_once(text: &str, insert: bool) -> (String, bool) {
    let before = parsed(text);
    let mut module = before.clone();
    let changed = joined(f(&mut module), insert).unwrap();
    let text = printed(&module);
    assert_eq!(results(&module, INPUTS), results(&before, INPUTS), "{text}");
    (text, changed)
}

fn kept(text: &str, insert: bool) {
    let (after, changed) = joined_once(text, insert);
    assert!(!changed);
    assert_eq!(after, printed(&parsed(text)));
}

#[test]
fn test_providers_on_every_edge_become_a_phi() {
    let (text, changed) = joined_once(&diamond("  %a = add i16 %x, %y\n", "  %b = add i16 %y, %x\n", "  %r = add i16 %x, %y\n"), false);
    assert!(changed);
    assert!(text.ends_with("b3:\n  %r.pre-phi = phi i16 [ %a, %b1 ], [ %b, %b2 ]\n  ret i16 %r.pre-phi\n}\n"), "{text}");
}

#[test]
fn test_a_join_expression_translates_through_its_phis() {
    let (text, changed) = joined_once(
        &diamond("  %a = mul i16 %x, 3\n", "  %b = mul i16 %y, 3\n", "  %p = phi i16 [ %x, %b1 ], [ %y, %b2 ]\n  %r = mul i16 %p, 3\n"),
        false,
    );
    assert!(changed);
    assert!(text.contains("  %r.pre-phi = phi i16 [ %a, %b1 ], [ %b, %b2 ]\n  ret i16 %r.pre-phi\n"), "{text}");
}

#[test]
fn test_a_missing_provider_is_inserted_on_an_unconditional_edge() {
    let text = diamond("  %a = xor i16 %x, %y\n", "", "  %r = xor i16 %x, %y\n");
    kept(&text, false);
    let (text, changed) = joined_once(&text, true);
    assert!(changed);
    assert!(
        text.ends_with(
            "b2:
  %r.pre = xor i16 %x, %y
  br label %b3

b3:
  %r.pre-phi = phi i16 [ %a, %b1 ], [ %r.pre, %b2 ]
  ret i16 %r.pre-phi
}
"
        ),
        "{text}"
    );
}

/// A divide by %y on the path that did not divide could trap there.
#[test]
fn test_a_division_is_not_inserted_on_a_path_that_did_not_divide() {
    let text = diamond("  %a = udiv i16 %x, %y\n", "", "  %r = udiv i16 %x, %y\n");
    let mut module = parsed(&text);
    assert!(!joined(f(&mut module), true).unwrap());
    assert_eq!(printed(&module), printed(&parsed(&text)));
}

#[test]
fn test_no_provider_on_any_edge_leaves_the_join_alone() {
    kept(&diamond("  %a = add i16 %x, 1\n", "  %b = add i16 %y, 1\n", "  %r = add i16 %x, %y\n"), true);
}

/// `x - y` on one edge and `y - x` on the other: the second edge has no
/// provider.
#[test]
fn test_an_arm_computing_another_expression_serves_nothing() {
    kept(&diamond("  %a = sub i16 %x, %y\n", "  %b = sub i16 %y, %x\n", "  %r = sub i16 %x, %y\n"), false);
}

/// Inserting on the entry edge would hoist into the preheader what the
/// latch computes; the latch, dominated by the header, serves nothing.
#[test]
fn test_a_loop_header_is_not_served_from_its_own_body() {
    let text = "define i16 @f(i16 %x, i16 %y, i1 %c) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %n, %b2 ]
  %r = add i16 %x, %y
  %more = icmp ult i16 %i, 3
  br i1 %more, label %b2, label %b3

b2:
  %a = add i16 %x, %y
  %n = add i16 %i, 1
  br label %b1

b3:
  ret i16 %r
}
";
    kept(text, true);
}

/// Numbering serves the dominated repeat; the join then takes a phi of
/// the arms' values.
#[test]
fn test_gvn_numbers_then_joins() {
    let text = diamond(
        "  %a = shl i16 %x, 2\n  %a2 = shl i16 %x, 2\n  %s = add i16 %a, %a2\n",
        "  %b = shl i16 %x, 2\n",
        "  %r = shl i16 %x, 2\n",
    );
    let before = parsed(&text);
    let mut module = before.clone();
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.verify_invalidation = true;
    manager.add(Gvn::default());
    manager.run(&mut module).unwrap();
    let after = printed(&module);
    assert!(after.contains("  %s = add i16 %a, %a\n"), "{after}");
    assert!(after.contains("  %r.pre-phi = phi i16 [ %a, %b1 ], [ %b, %b2 ]\n  ret i16 %r.pre-phi\n"), "{after}");
    assert_eq!(results(&module, INPUTS), results(&before, INPUTS));
}

/// Under the pass manager a load through a global is served across a
/// call to a `readonly` callee. The pass once saw no globals, so it took
/// the callee for one that may write and loaded @g again.
#[test]
fn a_load_through_a_global_is_reused_across_a_readonly_call() {
    let mut module = parsed(
        "@g = global i16 0

declare i16 @peek() readonly

define i16 @f() {
b0:
  %a = load i16, ptr @g
  %p = call i16 @peek()
  %b = load i16, ptr @g
  %r = add i16 %a, %b
  ret i16 %r
}
",
    );
    let mut manager = PassManager::default();
    manager.add(Gvn::default());
    manager.run(&mut module).unwrap();
    let after = printed(&module);
    assert!(after.contains("  %r = add i16 %a, %a\n"), "{after}");
}
