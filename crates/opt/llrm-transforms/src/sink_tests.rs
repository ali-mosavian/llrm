//! A load or operation read only below a branch moves below it; each case's
//! fixture is the pass's input.

use super::Sink;
use crate::testing::{managed, parsed, results};

const HEAD: &str = "target datalayout = \"e-p:32:32-i32:32-n8:16:32\"\n\n@g = global [8 x i32] zeroinitializer\n\n";

/// `text` through the pass, answering as before on `inputs`: the text after.
fn sunk(
    text: &str,
    inputs: &[&[i128]],
) -> String {
    let original = parsed(&format!("{HEAD}{text}"));
    let mut module = original.clone();
    let after = managed(&mut module, Sink { size: false });
    assert_eq!(results(&module, inputs), results(&original, inputs), "{after}");
    after
}

/// The block `name` of `text`, from its label to the next blank line.
fn block<'a>(
    text: &'a str,
    name: &str,
) -> &'a str {
    let from = text.find(&format!("{name}:")).unwrap_or_else(|| panic!("{name} in {text}"));
    text[from..].split("\n\n").next().unwrap()
}

/// d_faces loaded eight array pointers at the top of its loop and stored most
/// of them to the stack, before the test that skips a face: the load is read
/// in one arm only and runs there.
#[test]
fn test_a_load_read_in_one_arm_moves_into_it() {
    let text = "define i32 @f(i32 %c) {
b0:
  %v = load i32, ptr @g
  %cold = icmp slt i32 %c, 0
  br i1 %cold, label %b1, label %b2

b1:
  %r = add i32 %v, 1
  ret i32 %r

b2:
  ret i32 0
}
";
    let after = sunk(text, &[&[-1], &[5]]);
    assert!(!block(&after, "b0").contains("load"), "{after}");
    assert!(block(&after, "b1").contains("load"), "{after}");
}

/// A store on the way may change what the load reads: the load stays.
#[test]
fn test_a_load_does_not_cross_a_store_that_may_write_it() {
    let text = "define i32 @f(i32 %c) {
b0:
  %v = load i32, ptr @g
  store i32 7, ptr @g
  %cold = icmp slt i32 %c, 0
  br i1 %cold, label %b1, label %b2

b1:
  %r = add i32 %v, 1
  ret i32 %r

b2:
  ret i32 0
}
";
    let after = sunk(text, &[&[-1], &[5]]);
    assert!(block(&after, "b0").contains("load"), "{after}");
}

/// Read in both arms, the load has nowhere to go that one arm skips.
#[test]
fn test_a_load_read_in_both_arms_stays() {
    let text = "define i32 @f(i32 %c) {
b0:
  %v = load i32, ptr @g
  %skip = icmp eq i32 %c, 0
  br i1 %skip, label %b2, label %b1

b1:
  %r = add i32 %v, 1
  ret i32 %r

b2:
  %q = add i32 %v, 2
  ret i32 %q
}
";
    let after = sunk(text, &[&[0], &[5]]);
    assert!(block(&after, "b0").contains("load"), "{after}");
}

/// Not into a loop: the load would run every trip.
#[test]
fn test_a_load_does_not_move_into_a_loop() {
    let text = "define i32 @f(i32 %n) {
b0:
  %v = load i32, ptr @g
  %skip = icmp sle i32 %n, 0
  br i1 %skip, label %b3, label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %i2, %b1 ]
  %s = phi i32 [ 0, %b0 ], [ %s2, %b1 ]
  %s2 = add i32 %s, %v
  %i2 = add i32 %i, 1
  %more = icmp slt i32 %i2, %n
  br i1 %more, label %b1, label %b2

b2:
  ret i32 %s2

b3:
  ret i32 0
}
";
    let after = sunk(text, &[&[0], &[3]]);
    assert!(!block(&after, "b1").contains("load"), "{after}");
}

/// A load and its address, both sunk to one block, kept their order only when
/// the anchor was read from the function as it stood after the first move:
/// read from the one before it, the load came out above its `gep` and the
/// verifier refused it (lru at -O2, queens and jpeg at -m16).
#[test]
fn test_a_sunk_load_stays_below_its_sunk_address() {
    let text = "define i32 @f(i32 %c, i32 %i) {
b0:
  %p = getelementptr i32, ptr @g, i32 %i
  %v = load i32, ptr %p
  %cold = icmp slt i32 %c, 0
  br i1 %cold, label %b1, label %b2

b1:
  %r = add i32 %v, 1
  ret i32 %r

b2:
  ret i32 0
}
";
    let after = sunk(text, &[&[-1, 1], &[5, 2]]);
    let one = block(&after, "b1");
    assert!(one.find("getelementptr").unwrap() < one.find("load").unwrap(), "{after}");
}
