//! `jumpthread` over state machines the interpreter runs before and after.

use llrm_analysis::testing::DOS;

use super::JumpThread;
use crate::testing::{managed, parsed, results};

/// A loop of `n` trips: a switch on the state, each case setting the next from
/// the input, the state joined from constants.
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

const MULTI_EDGE: &str = "define i16 @f(i16 %n, i16 %k) {
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
  switch i16 %k, label %join [ i16 0, label %join i16 1, label %join i16 2, label %join ]

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
  %ns = phi i16 [ 1, %d0 ], [ 1, %d0 ], [ 1, %d0 ], [ 1, %d0 ], [ 2, %c1a ], [ 0, %c1b ], [ 1, %c2 ]
  %w = mul i16 %ns, %m
  %acc1 = add i16 %acc, %w
  %i1 = add nsw i16 %i, 1
  br label %head

done:
  ret i16 %acc
}
";

/// A loop whose dispatch is behind a switch with three edges to one block
/// (`odd`) that reads a phi of its own.
const BEHIND_A_SWITCH: &str = "define i16 @f(i16 %n, i16 %k) {
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %i1, %join ]
  %s = phi i16 [ 0, %b0 ], [ %ns, %join ]
  %acc = phi i16 [ 0, %b0 ], [ %acc1, %join ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %pre, label %done

pre:
  switch i16 %k, label %odd [ i16 0, label %disp i16 1, label %odd i16 2, label %odd ]

odd:
  %o = phi i16 [ %i, %pre ], [ %i, %pre ], [ %i, %pre ]
  br label %join

disp:
  switch i16 %s, label %d0 [ i16 1, label %c1 i16 2, label %c2 ]

d0:
  br label %join

c1:
  br label %join

c2:
  br label %join

join:
  %ns = phi i16 [ 1, %d0 ], [ 2, %c1 ], [ 1, %c2 ], [ 0, %odd ]
  %ob = phi i16 [ 0, %d0 ], [ 0, %c1 ], [ 0, %c2 ], [ %o, %odd ]
  %w = mul i16 %ns, %acc
  %x = add i16 %w, %ob
  %acc1 = add i16 %acc, %x
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
    let pairs: Vec<Vec<i128>> =
        [(0, 0), (1, 5), (7, 3), (10, -4), (13, 9), (30, 2)].iter().map(|&(n, k)| vec![n, k]).collect();
    let inputs: Vec<&[i128]> = pairs.iter().map(Vec::as_slice).collect();
    let before = parsed(&format!("{DOS}{MACHINE}"));
    let mut after = before.clone();
    let printed = managed(&mut after, JumpThread { size: false, correlated: false, states: true });
    assert_eq!(results(&after, &inputs), results(&before, &inputs), "{printed}");
    // Every way into the switch, the first trip's included, has its copy of the
    // blocks down to a case: no dispatch is left.
    assert!(blocks(&printed) > blocks(&llrm_mir::print::module(&before)), "{printed}");
    assert_eq!(printed.matches("switch").count(), 0, "{printed}");
}

#[test]
fn test_tuned_for_size_nothing_is_copied() {
    let before = parsed(&format!("{DOS}{MACHINE}"));
    let mut after = before.clone();
    let printed = managed(&mut after, JumpThread { size: true, correlated: false, states: true });
    // The entry still reaches the loop through its header.
    assert!(printed.contains("head:"), "{printed}");
}

/// A state machine as the loop passes leave it (switch in the header, the next
/// state a phi of constants and of a phi of constants): two joins between the
/// case and the switch, and the loop's own exit test in the latch.
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
    let printed = managed(&mut after, JumpThread { size: false, correlated: false, states: true });
    assert_eq!(results(&after, &inputs), results(&before, &inputs), "{printed}");
    assert_eq!(printed.matches("switch").count(), 0, "{printed}");
}

/// A threaded block whose switch had several edges to one join gave the join's
/// phi one input for them all: the module failed to verify ("a phi in block 6
/// has 1 inputs from block 5 for 5 edges"), found by the levels work with
/// unrolling off.
#[test]
fn test_a_block_with_several_edges_to_a_join_gives_the_phi_an_input_for_each() {
    let pairs: Vec<Vec<i128>> =
        [(0, 0), (1, 5), (7, 3), (10, -4), (13, 9), (30, 2)].iter().map(|&(n, k)| vec![n, k]).collect();
    let inputs: Vec<&[i128]> = pairs.iter().map(Vec::as_slice).collect();
    let before = parsed(&format!("{DOS}{MULTI_EDGE}"));
    let mut after = before.clone();
    let printed = managed(&mut after, JumpThread { size: false, correlated: false, states: true });
    assert_eq!(results(&after, &inputs), results(&before, &inputs), "{printed}");
}

/// A block copied on the way to a case, whose switch has three edges to one
/// block, gave that block's phi one input: the module failed to verify ("a phi
/// in block 6 has 1 inputs from block 5 for 5 edges").
#[test]
fn test_a_copied_block_with_three_edges_to_a_block_gives_its_phi_three_inputs() {
    let pairs: Vec<Vec<i128>> =
        [(0, 0), (1, 5), (7, 3), (10, 1), (13, 2), (30, 0)].iter().map(|&(n, k)| vec![n, k]).collect();
    let inputs: Vec<&[i128]> = pairs.iter().map(Vec::as_slice).collect();
    let before = parsed(&format!("{DOS}{BEHIND_A_SWITCH}"));
    let mut after = before.clone();
    let printed = managed(&mut after, JumpThread { size: false, correlated: false, states: true });
    assert_eq!(results(&after, &inputs), results(&before, &inputs), "{printed}");
}

/// An inlined `safe()`: its result is a phi of constants, tested by the branch
/// after it, in a loop.
const TESTED_RESULT: &str = "define i16 @f(i16 %n, i16 %k) {
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %i1, %step ]
  %acc = phi i16 [ 0, %b0 ], [ %acc1, %step ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %test, label %done

test:
  %c = icmp sgt i16 %k, %i
  br i1 %c, label %yes, label %no

yes:
  br label %join

no:
  br label %join

join:
  %r = phi i16 [ 1, %yes ], [ 0, %no ]
  %t = icmp ne i16 %r, 0
  br i1 %t, label %hit, label %step

hit:
  %h = add i16 %acc, %i
  br label %step

step:
  %acc1 = phi i16 [ %acc, %join ], [ %h, %hit ]
  %i1 = add nsw i16 %i, 1
  br label %head

done:
  ret i16 %acc
}
";

/// `r = phi(1, 0); if (r != 0)` after a call inlined: the test of a constant
/// each path decides was run on every trip (queens: `xor eax, eax; cmp eax, 0;
/// je`, 13% of its -O2 clocks over gcc).
#[test]
fn test_a_branch_on_a_compare_of_a_phi_of_constants_is_decided_on_each_path() {
    let pairs: Vec<Vec<i128>> =
        [(0, 0), (1, 5), (7, 3), (10, -4), (13, 9), (30, 2)].iter().map(|&(n, k)| vec![n, k]).collect();
    let inputs: Vec<&[i128]> = pairs.iter().map(Vec::as_slice).collect();
    let before = parsed(&format!("{DOS}{TESTED_RESULT}"));
    let mut after = before.clone();
    let printed = managed(&mut after, JumpThread { size: false, correlated: false, states: true });
    assert_eq!(results(&after, &inputs), results(&before, &inputs), "{printed}");
    assert_eq!(printed.matches("icmp ne").count(), 0, "{printed}");
}

/// Two inlined `safe()` calls in one loop: each result is a phi of constants
/// tested by the branch after it.
const TWO_TESTED_RESULTS: &str = "define i16 @f(i16 %n, i16 %k) {
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %i1, %step ]
  %acc = phi i16 [ 0, %b0 ], [ %acc2, %step ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %test, label %done

test:
  %c = icmp sgt i16 %k, %i
  br i1 %c, label %yes, label %no

yes:
  br label %join

no:
  br label %join

join:
  %r = phi i16 [ 1, %yes ], [ 0, %no ]
  %t = icmp ne i16 %r, 0
  br i1 %t, label %hit, label %mid

hit:
  %h = add i16 %acc, %i
  br label %mid

mid:
  %acc1 = phi i16 [ %acc, %join ], [ %h, %hit ]
  %d = icmp slt i16 %k, %i
  br i1 %d, label %yes2, label %no2

yes2:
  br label %join2

no2:
  br label %join2

join2:
  %r2 = phi i16 [ 1, %yes2 ], [ 0, %no2 ]
  %t2 = icmp ne i16 %r2, 0
  br i1 %t2, label %hit2, label %step

hit2:
  %h2 = add i16 %acc1, %k
  br label %step

step:
  %acc2 = phi i16 [ %acc1, %join2 ], [ %h2, %hit2 ]
  %i1 = add nsw i16 %i, 1
  br label %head

done:
  ret i16 %acc
}
";

/// Every branch of the function as it came is threaded, not the first alone: a
/// round looked at one branch with paths and later rounds at none, so the
/// peeled place of queens at -O3 kept 5 of its 8 tests of a constant
/// (`xor eax, eax; test eax, eax; je`, 5% of its instructions, 109,038 clocks
/// against gcc's 94,839; threaded 94,416).
#[test]
fn test_every_branch_on_a_phi_of_constants_is_decided_not_the_first_alone() {
    let pairs: Vec<Vec<i128>> =
        [(0, 0), (1, 5), (7, 3), (10, -4), (13, 9), (30, 2)].iter().map(|&(n, k)| vec![n, k]).collect();
    let inputs: Vec<&[i128]> = pairs.iter().map(Vec::as_slice).collect();
    let before = parsed(&format!("{DOS}{TWO_TESTED_RESULTS}"));
    let mut after = before.clone();
    let printed = managed(&mut after, JumpThread { size: false, correlated: false, states: true });
    assert_eq!(results(&after, &inputs), results(&before, &inputs), "{printed}");
    assert_eq!(printed.matches("icmp ne").count(), 0, "{printed}");
}

/// Tuned for size GCC threads a branch whose copies would be empty
/// (`tree-ssa-threadupdate.cc:2077`): the result of an inlined call tested by
/// the next branch costs nothing to decide on each path (queens -Os: 187348 ->
/// 173090 instructions).
#[test]
fn test_tuned_for_size_a_branch_whose_copies_are_empty_is_still_decided() {
    let pairs: Vec<Vec<i128>> =
        [(0, 0), (1, 5), (7, 3), (10, -4), (13, 9), (30, 2)].iter().map(|&(n, k)| vec![n, k]).collect();
    let inputs: Vec<&[i128]> = pairs.iter().map(Vec::as_slice).collect();
    let before = parsed(&format!("{DOS}{TESTED_RESULT}"));
    let mut after = before.clone();
    let printed = managed(&mut after, JumpThread { size: true, correlated: false, states: true });
    assert_eq!(results(&after, &inputs), results(&before, &inputs), "{printed}");
    assert_eq!(printed.matches("icmp ne").count(), 0, "{printed}");
}

/// The first trip of a loop is not peeled by threading its header's test
/// (sieve: +29 B, a second copy of the outer loop's start): entering a loop
/// through its header is a rotation, which `Rotate` makes where it pays.
#[test]
fn test_the_test_of_a_loop_s_first_trip_is_not_threaded() {
    let text = "define i16 @f(i16 %n) {
b0:
  br label %head

head:
  %i = phi i16 [ 2, %b0 ], [ %i1, %body ]
  %acc = phi i16 [ 0, %b0 ], [ %acc1, %body ]
  %go = icmp ult i16 %i, 80
  br i1 %go, label %body, label %done

body:
  %acc1 = add i16 %acc, %i
  %i1 = add nsw i16 %i, 1
  br label %head

done:
  ret i16 %acc
}
";
    let before = parsed(&format!("{DOS}{text}"));
    let mut after = before.clone();
    let printed = managed(&mut after, JumpThread { size: false, correlated: false, states: true });
    // The entry still reaches the loop through its header.
    assert!(printed.contains("head:"), "{printed}");
}

/// A loop whose test is branched on twice: at its top, and again in the join
/// the two ways of the first reach, where the way in settles it (the rotated
/// nest of hanoi). Each way is threaded to its arm; the loop computes what it
/// did.
const CORRELATED: &str = "define i16 @f(i16 %n, i16 %k) {
b0:
  %go = icmp ne i16 %n, 0
  br i1 %go, label %pre, label %out

pre:
  br label %head

head:
  %i = phi i16 [ 0, %pre ], [ %i1, %join ]
  %acc = phi i16 [ 0, %pre ], [ %acc1, %join ]
  %i1 = add i16 %i, 1
  %last = icmp eq i16 %i1, %n
  br i1 %last, label %join, label %work

work:
  %w = mul i16 %i, %k
  br label %join

join:
  %x = phi i16 [ 0, %head ], [ %w, %work ]
  %acc1 = add i16 %acc, %x
  br i1 %last, label %done, label %head

done:
  ret i16 %acc1

out:
  ret i16 -1
}
";

#[test]
fn test_a_branch_the_way_in_settles_is_threaded_on_each_way() {
    let inputs: &[&[i128]] = &[&[0, 3], &[1, 3], &[2, 5], &[7, 2], &[9, 1]];
    let before = parsed(&format!("{DOS}{CORRELATED}"));
    let mut after = before.clone();
    let printed = managed(&mut after, JumpThread { size: false, correlated: true, states: true });
    assert_eq!(results(&after, inputs), results(&before, inputs), "{printed}");
    assert!(printed.matches("br i1 %last").count() < 2, "the second branch on %last is decided on each way: {printed}");
}

/// A loop whose header branches on a compare of `%x`, which the loop does not
/// change: the true arm goes straight to the body, the false one adds a
/// constant first, and the body is one block that is the latch.
const INVARIANT: &str = "define i16 @f(i16 %x, i16 %n) {
b0:
  %c = icmp eq i16 %x, 8
  br label %head

head:
  %m = phi i16 [ %n, %b0 ], [ %m1, %join ]
  %acc = phi i16 [ 0, %b0 ], [ %acc1, %join ]
  br i1 %c, label %join, label %other

other:
  %d = add i16 %acc, 5
  br label %join

join:
  %v = phi i16 [ 0, %head ], [ %d, %other ]
  %acc1 = add i16 %acc, %v
  %m1 = sub i16 %m, 1
  %done = icmp sle i16 %m1, 0
  br i1 %done, label %exit, label %head

exit:
  %r = phi i16 [ %acc1, %join ]
  ret i16 %r
}
";

/// `INVARIANT` with a second block in the chain to the latch.
const INVARIANT_CHAIN: &str = "define i16 @f(i16 %x, i16 %n) {
b0:
  %c = icmp eq i16 %x, 8
  br label %head

head:
  %m = phi i16 [ %n, %b0 ], [ %m1, %tail ]
  %acc = phi i16 [ 0, %b0 ], [ %acc2, %tail ]
  br i1 %c, label %join, label %other

other:
  %d = add i16 %acc, 5
  br label %join

join:
  %v = phi i16 [ 0, %head ], [ %d, %other ]
  %acc1 = add i16 %acc, %v
  %big = icmp sgt i16 %acc1, 1000
  br i1 %big, label %exit, label %tail

tail:
  %acc2 = add i16 %acc1, 1
  %m1 = sub i16 %m, 1
  %done = icmp sle i16 %m1, 0
  br i1 %done, label %exit, label %head

exit:
  %r = phi i16 [ %acc1, %join ], [ %acc2, %tail ]
  ret i16 %r
}
";

const INVARIANT_INPUTS: &[&[i128]] = &[&[8, 1], &[8, 5], &[3, 1], &[3, 5], &[8, 400], &[3, 400], &[0, 2]];

fn the_branch_on_the_invariant_is_in_no_loop(text: &str) -> bool {
    let mut module = parsed(text);
    let function = crate::testing::f(&mut module);
    let graph = llrm_analysis::cfg::graph(function);
    let loops = llrm_analysis::graph::loops::loops(&graph, function.entry().map(llrm_analysis::cfg::id));
    function
        .walk()
        .all(
            |(block, inst)| {
                let op = function.instruction(inst);
                let tests = op.opcode == llrm_mir::opcode::Opcode::Br
                    && op.operands.len() == 3
                    && matches!(
                        op.operands[0],
                        llrm_mir::module::Operand::Value(c) if function.value(c).name.as_deref() == Some("c")
                    );
                !tests || !loops.iter().any(|one| one.body.contains(&llrm_analysis::cfg::id(block)))
            },
        )
}

fn split(text: &str) -> String {
    let mut module = parsed(&format!("{DOS}{text}"));
    let expected = results(&module, INVARIANT_INPUTS);
    let after = crate::testing::managed(&mut module, JumpThread { size: false, correlated: true, states: true });
    assert_eq!(results(&module, INVARIANT_INPUTS), expected, "{after}");
    after
}

/// rectwo -O2 ran `xor ebp,ebp; cmp [esp+14h],8; je` on every trip of its
/// hottest loop: the header's branch on the invariant `n == 8`, the arms
/// rejoining in the body. A loop for each arm leaves the test out of both.
#[test]
fn test_a_loop_branching_on_an_invariant_has_a_loop_for_each_arm() {
    let after = split(INVARIANT);
    assert!(the_branch_on_the_invariant_is_in_no_loop(&after), "{after}");
    // The original join is deleted, not left unreachable.
    assert!(!after.contains("join:"), "{after}");
}

#[test]
fn test_the_arms_chain_may_run_through_two_blocks_with_an_exit_between() {
    let after = split(INVARIANT_CHAIN);
    assert!(the_branch_on_the_invariant_is_in_no_loop(&after), "{after}");
    assert!(!after.contains("join:") && !after.contains("tail:"), "{after}");
}

#[test]
fn test_a_branch_the_loop_changes_is_left() {
    let text = INVARIANT
        .replace("%c = icmp eq i16 %x, 8\n  br label %head", "br label %head")
        .replace(
            "  br i1 %c, label %join, label %other",
            "  %c = icmp eq i16 %m, 8\n  br i1 %c, label %join, label %other",
        );
    let after = split(&text);
    assert!(after.contains("join:"), "{after}");
}

#[test]
fn test_the_size_of_the_copies_is_gccs_limit() {
    let mut module = parsed(&format!("{DOS}{INVARIANT}"));
    let after = crate::testing::managed(&mut module, JumpThread { size: true, correlated: true, states: true });
    assert!(after.contains("join:"), "a size build copies nothing: {after}");
}

/// The four passes that clean what the copies know of their state were each
/// run after an invalidation of everything, whatever the one before preserved:
/// the analyses a pass left true were worked out again for the next.
#[test]
fn the_cleaning_after_a_thread_keeps_what_each_pass_preserved() {
    let mut module = parsed(&format!("{DOS}{MACHINE}"));
    llrm_mir::passes::trace_recomputes(true);
    managed(&mut module, JumpThread { size: false, correlated: false, states: true });
    let runs: usize = llrm_mir::passes::recomputes().iter().map(|(.., count)| count).sum();
    llrm_mir::passes::trace_recomputes(false);
    assert!(runs <= BOUND, "{runs} analyses worked out");
}
/// 53 with everything invalidated before each pass, 38 with what each
/// preserved.
const BOUND: usize = 45;

/// QCport `ent_trig_shot` as it reaches the pass: a state machine whose two
/// switches find every way in twice. A path found twice was copied twice and
/// its second copy panicked ("an input from the path") at -O2 once
/// JumpThreading ran before it.
const FOUND_TWICE: &str = r#"target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32"

declare cc 1005 i16 @"_snd_start@3"(ptr, ptr, i16, i16, i16, ptr, i16) addrspace(1)
declare cc 1005 i16 @"_ent_box_touched@3"(ptr, ptr addrspace(1), ptr addrspace(1), double) addrspace(1)

define cc 1005 void @"_ent_trig_shot@3"(ptr %0, ptr %1, ptr nocapture readonly %2, ptr nocapture readonly %3, float %4, i16 %5, i16 %6, i16 %7) addrspace(1) {
b1:
  %8 = alloca [12 x i8]
  %9 = getelementptr inbounds nuw i8, ptr %0, i16 224
  %10 = getelementptr inbounds nuw i8, ptr %0, i16 226
  %11 = fcmp ogt float undef, 0.000000e+00
  %12 = getelementptr inbounds nuw i8, ptr %0, i16 7026
  %13 = icmp ne i16 %7, 0
  br label %b2

b2:
  %lsr.iv = phi i16 [ 0, %b1 ], [ %lsr.iv.next, %b22 ]
  %14 = phi i16 [ poison, %b1 ], [ %31, %b22 ]
  %15 = phi i16 [ 0, %b1 ], [ %32, %b22 ]
  %16 = load i16, ptr %9
  %17 = icmp slt i16 %15, %16
  br i1 %17, label %b4, label %b3

b3:
  ret void

b4:
  %18 = load ptr addrspace(1), ptr %10
  %19 = getelementptr inbounds i8, ptr addrspace(1) %18, i16 %lsr.iv
  %20 = getelementptr inbounds nuw i8, ptr addrspace(1) %19, i16 2
  %21 = load i16, ptr addrspace(1) %20
  %22 = icmp eq i16 %21, 9
  br i1 %22, label %b6, label %b16

b6:
  %23 = getelementptr inbounds nuw i8, ptr addrspace(1) %19, i16 10
  %24 = load i16, ptr addrspace(1) %23
  %25 = icmp eq i16 %24, 4
  br i1 %25, label %b18, label %b22

b16:
  %26 = phi i16 [ %14, %b4 ], [ 4, %b19 ]
  %27 = phi i16 [ %21, %b4 ], [ %30, %b19 ]
  switch i16 %27, label %b22 [
    i16 1, label %38
    i16 5, label %b23
    i16 6, label %b23
  ]

b18:
  br i1 %11, label %b19, label %b22

b19:
  %28 = add i16 %15, 2048
  %29 = call cc 1005 addrspace(1) i16 @"_snd_start@3"(ptr nonnull %12, ptr %1, i16 %28, i16 4, i16 163, ptr nonnull %8, i16 1)
  %30 = load i16, ptr addrspace(1) %20
  br label %b16

b22:
  %31 = phi i16 [ 0, %b30 ], [ 0, %b29 ], [ 0, %b27 ], [ %33, %b23 ], [ %26, %b16 ], [ 4, %b18 ], [ %24, %b6 ]
  %32 = add nuw nsw i16 %15, 1
  %lsr.iv.next = add i16 %lsr.iv, 78
  br label %b2

b23:
  %33 = phi i16 [ %39, %38 ], [ %26, %b16 ], [ %26, %b16 ]
  switch i16 %33, label %b22 [
    i16 0, label %b27
  ]

b27:
  br i1 %13, label %b29, label %b22

b29:
  %34 = icmp ne i16 %27, 5
  br i1 %34, label %b30, label %b22

b30:
  %35 = getelementptr inbounds nuw i8, ptr addrspace(1) %19, i16 54
  %36 = getelementptr inbounds nuw i8, ptr addrspace(1) %19, i16 42
  %37 = call cc 1005 addrspace(1) i16 @"_ent_box_touched@3"(ptr %1, ptr addrspace(1) nonnull %36, ptr addrspace(1) nonnull %35, double 0.000000e+00)
  br label %b22

38:
  %.phi.trans.insert = getelementptr inbounds nuw i8, ptr addrspace(1) %19, i16 10
  %39 = load i16, ptr addrspace(1) %.phi.trans.insert
  br label %b23
}
"#;

#[test]
fn test_a_way_in_found_twice_is_copied_once() {
    let mut module = parsed(FOUND_TWICE);
    managed(&mut module, JumpThread { size: false, correlated: true, states: true });
}
