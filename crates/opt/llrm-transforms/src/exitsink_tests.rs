//! `sunk` over loops the interpreter runs before and after, over trip
//! counts 0, 1 and more.

use super::sunk;
use crate::testing::{f, parsed, printed, results};

/// A post-tested loop whose `%last = LAST` the exit reads.
fn looped(
    last: &str,
    more: &str,
) -> String {
    format!(
        "define i16 @f(i16 %n, i16 %x) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b1 ]
  %i.next = add i16 %i, 1
  %last = {last}
  %more = icmp ult i16 {more}, %n
  br i1 %more, label %b1, label %b2

b2:
  %e = phi i16 [ %last, %b1 ]
  ret i16 %e
}}
"
    )
}

/// `text` sunk, printed, where it computes what it did.
fn run(text: &str) -> (String, bool) {
    let before = parsed(text);
    let mut after = before.clone();
    let changed = sunk(f(&mut after));
    let text = printed(&after);
    let inputs: &[&[i128]] = &[&[0, 3], &[1, 3], &[2, -7], &[9, 1000]];
    assert_eq!(results(&after, inputs), results(&before, inputs), "{text}");
    (text, changed)
}

#[test]
fn test_final_update_moves_to_the_exit() {
    let (after, changed) = run(&looped("add i16 %i, 40", "%i.next"));
    assert!(changed);
    assert!(
        after.ends_with("b2:\n  %i.lcssa = phi i16 [ %i, %b1 ]\n  %0 = add i16 %i.lcssa, 40\n  ret i16 %0\n}\n"),
        "{after}"
    );
}

/// An invariant operand is read where it is; only the loop's own goes
/// through an exit phi.
#[test]
fn test_an_invariant_operand_needs_no_exit_phi() {
    let (after, changed) = run(&looped("sub i16 %i, %x", "%i.next"));
    assert!(changed);
    assert!(after.contains("  %0 = sub i16 %i.lcssa, %x\n"), "{after}");
}

#[test]
fn test_an_update_the_loop_reads_stays() {
    let (after, changed) = run(&looped("add i16 %i, 40", "%last"));
    assert!(!changed);
    assert_eq!(after, printed(&parsed(&looped("add i16 %i, 40", "%last"))));
}

#[test]
fn test_only_adds_and_subtracts_move() {
    let (_, changed) = run(&looped("mul i16 %i, 40", "%i.next"));
    assert!(!changed);
}

/// bench/mandel: a `short` counter's `sext`, read after the loop by two exits (the break and the
/// bound), was computed every trip. Each exit takes a copy.
#[test]
fn test_an_extension_two_exits_read_moves_to_both() {
    let text = "define i32 @f(i16 %n, i16 %x) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b3 ]
  %w = sext i16 %i to i32
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b4

b2:
  %d = icmp sgt i16 %i, %x
  br i1 %d, label %b5, label %b3

b3:
  %next = add i16 %i, 1
  br label %b1

b4:
  %e = phi i32 [ %w, %b1 ]
  ret i32 %e

b5:
  %g = phi i32 [ %w, %b2 ]
  ret i32 %g
}
";
    let before = parsed(text);
    let mut after = before.clone();
    assert!(sunk(f(&mut after)));
    let shown = printed(&after);
    let inputs: &[&[i128]] = &[&[0, 3], &[5, 3], &[5, 100], &[9, -1]];
    assert_eq!(results(&after, inputs), results(&before, inputs), "{shown}");
    let (body, rest) = shown.split_once("b2:").expect("b2");
    assert!(!body.contains("sext"), "{shown}");
    assert_eq!(rest.matches("sext").count(), 2, "{shown}");
}
