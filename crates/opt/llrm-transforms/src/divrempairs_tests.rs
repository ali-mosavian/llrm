//! A division and a remainder of the same operands, on a target with a divide
//! that makes both and on one without.

use super::DivRemPairs;
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-n8:16:32\"\n\n";

/// `text` through the pass, on real-mode x86 (a divide that makes both) or on
/// a target that has none.
fn run(
    text: &str,
    divrem: bool,
) -> String {
    let mut module = parsed(&format!("{HEAD}{text}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(DivRemPairs);
    let target: std::rc::Rc<dyn llrm_mir::target::Machine> = if divrem {
        std::rc::Rc::new(llrm_x86_m16::Dos::default())
    } else {
        std::rc::Rc::new(llrm_mir::target::Neutral)
    };
    manager.run_module(&mut module, target).unwrap();
    printed(&module)
}

fn lines(text: &str) -> Vec<String> {
    text.lines().map(|one| one.trim().to_owned()).filter(|one| !one.is_empty()).collect()
}

/// Where `needle` first stands among the lines of `text`.
fn at(
    text: &str,
    needle: &str,
) -> usize {
    lines(text).iter().position(|one| one.contains(needle)).unwrap_or_else(|| panic!("`{needle}` not in\n{text}"))
}

/// A pair already in one block is left to the backend where the target makes
/// both with one divide.
#[test]
fn test_a_pair_in_one_block_is_left_where_the_target_makes_both() {
    let text = "define i32 @f(i32 noundef %a, i32 noundef %b) {
b0:
  %d = sdiv i32 %a, %b
  %r = srem i32 %a, %b
  %s = add i32 %d, %r
  ret i32 %s
}
";
    assert_eq!(run(text, true), printed(&parsed(&format!("{HEAD}{text}"))));
}

/// The remainder stood in the block before the division: where the target
/// makes both, the division is moved up beside it so the backend sees the
/// pair (the pin's `DivInst->moveAfter(RemInst)`). Left below, it was a second
/// divide of the same operands.
#[test]
fn test_a_division_below_its_remainder_moves_up_beside_it() {
    let after = run(
        "define i32 @f(i32 noundef %a, i32 noundef %b, i1 %c) {
b0:
  %r = srem i32 %a, %b
  br i1 %c, label %t, label %e

t:
  %d = sdiv i32 %a, %b
  %s = add i32 %d, %r
  ret i32 %s

e:
  ret i32 %r
}
",
        true,
    );
    let entry = after.split_once("t:").expect("a second block").0;
    assert!(entry.contains("sdiv i32 %a, %b"), "the division is in b0: {after}");
    assert_eq!(at(entry, "sdiv"), at(entry, "srem") + 1, "right after the remainder: {after}");
}

/// A remainder written `a - (a / b) * b` is the real `srem` where the target
/// has a divide that makes both: the pin's `recomposed`. The product stays.
#[test]
fn test_an_expanded_remainder_is_the_real_one_where_the_target_makes_both() {
    let after = run(
        "define i32 @f(i32 noundef %a, i32 noundef %b) {
b0:
  %d = sdiv i32 %a, %b
  %m = mul i32 %d, %b
  %r = sub i32 %a, %m
  %s = add i32 %d, %r
  ret i32 %s
}
",
        true,
    );
    assert!(after.contains("srem i32 %a, %b"), "{after}");
    assert!(!after.contains("sub i32 %a"), "the expanded remainder is gone: {after}");
}

/// The division and the remainder in two arms of one branch go to the block
/// before the branch, where the target makes both: one divide runs on either
/// path.
#[test]
fn test_a_pair_in_the_two_arms_of_a_branch_is_hoisted_above_the_branch() {
    let after = run(
        "define i32 @f(i32 noundef %a, i32 noundef %b, i1 %c) {
b0:
  br i1 %c, label %t, label %e

t:
  %d = sdiv i32 %a, %b
  ret i32 %d

e:
  %r = srem i32 %a, %b
  ret i32 %r
}
",
        true,
    );
    let entry = after.split_once("t:").expect("arms").0;
    assert!(entry.contains("sdiv") && entry.contains("srem"), "both in b0: {after}");
}

/// Where the target has no divide that makes both, a remainder below a
/// division that dominates it is `a - (a / b) * b` of that division.
#[test]
fn test_a_remainder_below_a_division_is_made_of_it_where_the_target_has_no_divrem() {
    let after = run(
        "define i32 @f(i32 noundef %a, i32 noundef %b, i1 %c) {
b0:
  %d = sdiv i32 %a, %b
  br i1 %c, label %t, label %e

t:
  %r = srem i32 %a, %b
  %s = add i32 %d, %r
  ret i32 %s

e:
  ret i32 %d
}
",
        false,
    );
    assert!(!after.contains("srem"), "{after}");
    assert!(after.contains("mul i32 %d, %b") && after.contains("sub i32 %a,"), "{after}");
    assert_eq!(after.matches("sdiv").count(), 1, "one divide: {after}");
}

/// An operand that may be undef is frozen first: with `y = 1` and `x = undef`
/// the remainder is 0 and the product `undef * 1` is not. The `exact` flag of
/// the division goes too.
#[test]
fn test_an_operand_that_may_be_undef_is_frozen_and_the_division_loses_exact() {
    let after = run(
        "define i32 @f(i32 %a, i32 %b, i1 %c) {
b0:
  %d = sdiv exact i32 %a, %b
  br i1 %c, label %t, label %e

t:
  %r = srem i32 %a, %b
  ret i32 %r

e:
  ret i32 %d
}
",
        false,
    );
    assert!(after.contains("freeze i32 %a") && after.contains("freeze i32 %b"), "{after}");
    assert!(!after.contains("sdiv exact"), "{after}");
}
