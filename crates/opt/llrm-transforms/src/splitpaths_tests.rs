//! Tests of `splitpaths`: the join of a loop's latch diamond copied into its
//! arm.

use llrm_analysis::testing::DOS;

use super::SplitPaths;
use crate::testing::{Tuned, managed_on, parsed};

fn run(
    body: &str,
    split_paths: bool,
) -> String {
    managed_on(&mut parsed(&format!("{DOS}{body}")), SplitPaths, Tuned { split_paths, ..Default::default() })
}

/// The descent of phi_slot_unknown_arm: `side = c ? 1 : 2` joins an empty arm,
/// and the join (an address, a load, the loop's test) was one block reached by
/// two arms, `mov side, 1 ... jmp` in the arm that has a block.
const DESCENT: &str = "define i32 @f(ptr %t, i32 %c) {
b0:
  br label %d
d:
  %n = phi i32 [ 1, %b0 ], [ %v, %j ]
  %lt = icmp slt i32 %n, %c
  br i1 %lt, label %j, label %arm
arm:
  br label %j
j:
  %side = phi i32 [ 1, %d ], [ 2, %arm ]
  %i = add i32 %n, %side
  %p = getelementptr i32, ptr %t, i32 %i
  %v = load i32, ptr %p
  %z = icmp eq i32 %v, 0
  br i1 %z, label %out, label %d
out:
  ret i32 %i
}
";

#[test]
fn test_a_join_of_an_empty_arm_is_copied_into_it() {
    let after = run(DESCENT, true);
    assert!(!after.contains("%side = phi"), "{after}");
    assert!(after.contains("add i32 %n, 1") && after.contains("add i32 %n, 2"), "{after}");
    // `%i` is read after the loop from two places: a phi at the exit.
    let exit = after.split("out:").nth(1).expect("the exit block");
    assert!(exit.contains("= phi i32") && exit.contains("[ %i, %j ]") && exit.contains(", %arm ]"), "{after}");
}

/// A target that has not asked keeps the diamond.
#[test]
fn test_a_target_that_does_not_ask_keeps_the_join() {
    let after = run(DESCENT, false);
    assert!(after.contains("%side = phi i32 [ 1, %d ], [ 2, %arm ]"), "{after}");
}

/// Arms of one assignment each that feed the join's phi are an if-conversion
/// candidate: left.
#[test]
fn test_two_arms_of_one_assignment_feeding_the_phi_are_left() {
    let after = run(
        "define i32 @f(ptr %t, i32 %c) {
b0:
  br label %d
d:
  %n = phi i32 [ 1, %b0 ], [ %v, %j ]
  %lt = icmp slt i32 %n, %c
  %a = add i32 %n, 1
  br i1 %lt, label %arm1, label %arm2
arm1:
  %x = add i32 %a, 3
  br label %j
arm2:
  %y = add i32 %a, 5
  br label %j
j:
  %side = phi i32 [ %x, %arm1 ], [ %y, %arm2 ]
  %p = getelementptr i32, ptr %t, i32 %side
  %v = load i32, ptr %p
  %z = icmp eq i32 %v, 0
  br i1 %z, label %out, label %d
out:
  ret i32 %side
}
",
        true,
    );
    assert!(after.contains("%side = phi i32 [ %x, %arm1 ], [ %y, %arm2 ]"), "{after}");
}

/// A join that nothing uses a phi in (a block of its own work that does not
/// read the phi) gains nothing: left.
#[test]
fn test_a_join_that_does_not_read_its_phi_is_left() {
    let after = run(
        "define i32 @f(ptr %t, i32 %c) {
b0:
  br label %d
d:
  %n = phi i32 [ 1, %b0 ], [ %v, %j ]
  %lt = icmp slt i32 %n, %c
  br i1 %lt, label %j, label %arm
arm:
  br label %j
j:
  %side = phi i32 [ 1, %d ], [ 2, %arm ]
  %p = getelementptr i32, ptr %t, i32 %n
  %v = load i32, ptr %p
  %z = icmp eq i32 %v, 0
  br i1 %z, label %out, label %d
out:
  ret i32 %side
}
",
        true,
    );
    assert!(after.contains("%side = phi i32 [ 1, %d ], [ 2, %arm ]"), "{after}");
}
