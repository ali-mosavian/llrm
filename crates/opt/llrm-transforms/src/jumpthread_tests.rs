//! `jumpthread` over state machines the interpreter runs before and after.

use llrm_analysis::testing::DOS;

use super::JumpThread;
use crate::testing::{managed, parsed, results};

/// A loop of `n` trips: a switch on the state, each case setting the next from the input, the state joined from constants.
const MACHINE: &str = "define i16 @f(i16 %n, i16 %k) {
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %i1, %join ]
  %s = phi i16 [ 0, %b0 ], [ %ns, %join ]
  %acc = phi i16 [ 0, %b0 ], [ %acc1, %join ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %disp, label %done

disp:
  %m = add i16 %acc, %i
  switch i16 %s, label %d0 [ i16 1, label %c1 i16 2, label %c2 ]

d0:
  br label %join

c1:
  %t = icmp sgt i16 %k, %i
  br i1 %t, label %c1a, label %c1b

c1a:
  br label %join

c1b:
  br label %join

c2:
  br label %join

join:
  %ns = phi i16 [ 1, %d0 ], [ 2, %c1a ], [ 0, %c1b ], [ 1, %c2 ]
  %w = mul i16 %ns, %m
  %acc1 = add i16 %acc, %w
  %i1 = add nsw i16 %i, 1
  br label %head

done:
  ret i16 %acc
}
";

fn blocks(text: &str) -> usize {
    text.lines().filter(|line| line.ends_with(':')).count()
}

#[test]
fn test_a_switch_on_a_state_the_join_makes_constant_is_entered_at_its_case() {
    let pairs: Vec<Vec<i128>> = [(0, 0), (1, 5), (7, 3), (10, -4), (13, 9), (30, 2)].iter().map(|&(n, k)| vec![n, k]).collect();
    let inputs: Vec<&[i128]> = pairs.iter().map(Vec::as_slice).collect();
    let before = parsed(&format!("{DOS}{MACHINE}"));
    let mut after = before.clone();
    let printed = managed(&mut after, JumpThread { size: false });
    assert_eq!(results(&after, &inputs), results(&before, &inputs), "{printed}");
    // Every way into the switch, the first trip's included, has its copy of the blocks down to a case: no dispatch is left.
    assert!(blocks(&printed) > blocks(&llrm_mir::print::module(&before)), "{printed}");
    assert_eq!(printed.matches("switch").count(), 0, "{printed}");
}

#[test]
fn test_tuned_for_size_nothing_is_copied() {
    let before = parsed(&format!("{DOS}{MACHINE}"));
    let mut after = before.clone();
    let printed = managed(&mut after, JumpThread { size: true });
    assert_eq!(blocks(&printed), blocks(&llrm_mir::print::module(&before)), "{printed}");
}

/// A state machine as the loop passes leave it (switch in the header, the next state a phi of constants and of a phi of
/// constants): two joins between the case and the switch, and the loop's own exit test in the latch.
const ROTATED: &str = r#"define i32 @f(i32 %0) {
b1:
  %1 = sub i32 0, %0
  %2 = icmp sle i32 %0, 0
  br i1 %2, label %b3, label %15

b3:
  %3 = phi i32 [ 0, %b1 ], [ %12, %16 ]
  ret i32 %3

b4:
  %4 = phi i32 [ %13, %b12 ], [ 0, %15 ]
  %5 = phi i32 [ %12, %b12 ], [ 0, %15 ]
  %lsr.iv1 = phi i32 [ %lsr.iv.next, %b12 ], [ %1, %15 ]
  switch i32 %4, label %b5 [
    i32 0, label %b6
    i32 1, label %b7
    i32 2, label %b8
  ]

b5:
  br label %b12

b6:
  %6 = add i32 %lsr.iv1, %0
  %7 = and i32 %6, 1
  %8 = icmp ne i32 %7, 0
  br i1 %8, label %b11, label %b9

b7:
  %9 = add nsw i32 %5, 3
  br label %b12

b8:
  %10 = add nsw i32 %5, 5
  br label %b12

b9:
  br label %b10

b10:
  %11 = phi i32 [ 2, %b9 ], [ 1, %b11 ]
  br label %b12

b11:
  br label %b10

b12:
  %12 = phi i32 [ %10, %b8 ], [ %9, %b7 ], [ %5, %b10 ], [ %5, %b5 ]
  %13 = phi i32 [ 0, %b8 ], [ 2, %b7 ], [ %11, %b10 ], [ 0, %b5 ]
  %lsr.iv.next = add i32 %lsr.iv1, 1
  %14 = icmp ne i32 %lsr.iv.next, 0
  br i1 %14, label %b4, label %16

15:
  br label %b4

16:
  br label %b3
}
"#;

#[test]
fn test_a_state_made_by_two_joins_is_threaded_and_computes_the_same() {
    let pairs: Vec<Vec<i128>> = [-2, 0, 1, 2, 3, 7, 10, 25].iter().map(|&n| vec![n]).collect();
    let inputs: Vec<&[i128]> = pairs.iter().map(Vec::as_slice).collect();
    let before = parsed(&format!("target datalayout = \"e-p:32:32-n8:16:32\"\n\n{ROTATED}"));
    let mut after = before.clone();
    let printed = managed(&mut after, JumpThread { size: false });
    assert_eq!(results(&after, &inputs), results(&before, &inputs), "{printed}");
    assert_eq!(printed.matches("switch").count(), 0, "{printed}");
}
