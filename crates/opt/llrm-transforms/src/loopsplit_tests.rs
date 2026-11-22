//! `loopsplit`: a loop whose branch tests a counter against an invariant runs
//! as two loops, one on each side of the border.

use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::Module;
use llrm_mir::opcode::Opcode;

use super::LoopSplit;
use crate::testing::{Tuned, managed_on, parsed, results};

fn on() -> Tuned {
    Tuned { loop_split: true, ..Default::default() }
}

const INPUTS: &[&[i128]] =
    &[&[0], &[1], &[2], &[3], &[4], &[5], &[11], &[12], &[30], &[31], &[32], &[33], &[34], &[50], &[51], &[100]];

/// Loops of @f: (all, outermost).
fn loops(module: &Module) -> (usize, usize) {
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    let tree = DomTree::of(function);
    let info = LoopInfo::analyze(function, || &tree);
    (info.get_loops_in_preorder().len(), info.top_level_loops().count())
}

/// The blocks of the outermost loop of @f that stores nothing.
fn counting_loop_blocks(module: &Module) -> usize {
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    let tree = DomTree::of(function);
    let info = LoopInfo::analyze(function, || &tree);
    info.top_level_loops()
        .filter(|one| {
            !one.get_blocks().iter().any(|&block| {
                function.iter_block(block).any(|inst| matches!(function.instruction(inst).opcode, Opcode::Store { .. }))
            })
        })
        .map(|one| one.get_num_blocks())
        .next()
        .expect("a loop that stores nothing")
}

/// A counter loop, `%i` from `init` by `step`, that takes `yes` or `no` by
/// `guard` of `%i`, and goes round while `exit` of `%i.next` holds; entered
/// only where `entry` holds.
fn counting(
    entry: &str,
    init: &str,
    add: &str,
    step: i32,
    guard: &str,
    exit: &str,
) -> String {
    format!(
        "define i32 @f(i32 %n) {{
entry:
  %g = {entry}
  br i1 %g, label %pre, label %out
pre:
  br label %loop
loop:
  %i = phi i32 [ {init}, %pre ], [ %i.next, %test ]
  %acc = phi i32 [ 0, %pre ], [ %acc.next, %test ]
  %c = {guard}
  br i1 %c, label %yes, label %no
yes:
  %a = add i32 %acc, %i
  br label %test
no:
  %b = mul i32 %acc, 3
  %b1 = add i32 %b, 1
  br label %test
test:
  %acc.next = phi i32 [ %a, %yes ], [ %b1, %no ]
  %i.next = {add} i32 %i, {step}
  %go = {exit}
  br i1 %go, label %loop, label %done
done:
  %r = phi i32 [ %acc.next, %test ]
  br label %out
out:
  %res = phi i32 [ 0, %entry ], [ %r, %done ]
  ret i32 %res
}}
"
    )
}

fn up(
    guard: &str,
    exit: &str,
) -> String {
    counting("icmp slt i32 2, %n", "2", "add nsw", 1, guard, exit)
}

/// `text` split under `machine`: its text after, with @f's results on `INPUTS`
/// checked equal to what they were before.
fn split(
    text: &str,
    machine: Tuned,
) -> (Module, String) {
    let mut module = parsed(text);
    let before = results(&module, INPUTS);
    let after = managed_on(&mut module, LoopSplit { cleanup: false }, machine);
    assert_eq!(results(&module, INPUTS), before, "{after}");
    (module, after)
}

fn assert_split(text: &str) {
    let (module, after) = split(text, on());
    assert_eq!(loops(&parsed(text)), (1, 1));
    assert_eq!(loops(&module), (2, 2), "{after}");
}

fn assert_whole(text: &str) {
    let (module, after) = split(text, on());
    assert_eq!(loops(&module), (1, 1), "{after}");
}

#[test]
fn test_a_counter_below_a_border_splits_into_two_loops() {
    assert_split(&up("icmp sle i32 %i, 31", "icmp slt i32 %i.next, %n"));
}

#[test]
fn test_a_counter_that_starts_beyond_a_strict_border_splits() {
    assert_split(&up("icmp sgt i32 %i, 10", "icmp slt i32 %i.next, %n"));
}

#[test]
fn test_a_strict_border_that_the_counter_starts_below_splits() {
    assert_split(&up("icmp slt i32 %i, 12", "icmp slt i32 %i.next, %n"));
}

#[test]
fn test_a_counter_that_starts_at_or_past_a_border_splits() {
    assert_split(&up("icmp sge i32 %i, 33", "icmp slt i32 %i.next, %n"));
}

#[test]
fn test_a_not_equal_exit_splits() {
    assert_split(&up("icmp sle i32 %i, 31", "icmp ne i32 %i.next, %n"));
}

#[test]
fn test_a_test_for_the_first_iteration_is_left_whole() {
    // Peeling one iteration grew fpbench -O3 by 612 B and 161 instrs.
    assert_whole(&up("icmp eq i32 %i, 2", "icmp slt i32 %i.next, %n"));
}

#[test]
fn test_a_countdown_splits() {
    assert_split(&counting(
        "icmp sgt i32 %n, 0",
        "%n",
        "add nsw",
        -1,
        "icmp sge i32 %i, 50",
        "icmp sgt i32 %i.next, 0",
    ));
}

#[test]
fn test_an_unsigned_counter_splits() {
    assert_split(&counting("icmp ult i32 2, %n", "2", "add nuw", 1, "icmp ule i32 %i, 31", "icmp ult i32 %i.next, %n"));
}

#[test]
fn test_a_stride_of_three_splits_where_the_bound_is_not_a_multiple() {
    assert_split(&counting("icmp slt i32 2, %n", "2", "add nsw", 3, "icmp sle i32 %i, 31", "icmp slt i32 %i.next, %n"));
}

#[test]
fn test_a_guard_that_is_not_a_counter_is_left_whole() {
    assert_whole(&up("icmp sle i32 %acc, 31", "icmp slt i32 %i.next, %n"));
}

#[test]
fn test_a_counter_that_may_wrap_is_left_whole() {
    assert_whole(&counting("icmp slt i32 2, %n", "2", "add", 2, "icmp sle i32 %i, 31", "icmp slt i32 %i.next, %n"));
}

#[test]
fn test_a_counter_of_another_stride_than_the_exits_is_left_whole() {
    let text = up("icmp sle i32 %i, 31", "icmp slt i32 %i.next, %n").replace(
        "%acc = phi i32 [ 0, %pre ], [ %acc.next, %test ]",
        "%acc = phi i32 [ 0, %pre ], [ %acc.next, %test ]\n  %j = phi i32 [ 2, %pre ], [ %j.next, %test ]",
    );
    let text = text
        .replace("%c = icmp sle i32 %i, 31", "%c = icmp sle i32 %j, 31")
        .replace("  %go =", "  %j.next = add nsw i32 %j, 2\n  %go =");
    assert_whole(&text);
}

#[test]
fn test_a_loop_with_work_in_its_latch_is_left_whole() {
    let text = up("icmp sle i32 %i, 31", "icmp slt i32 %i.next, %n")
        .replace("br i1 %go, label %loop, label %done", "br i1 %go, label %latch, label %done")
        .replace("done:", "latch:\n  %w = xor i32 %acc.next, 1\n  br label %loop\ndone:")
        .replace("%i = phi i32 [ 2, %pre ], [ %i.next, %test ]", "%i = phi i32 [ 2, %pre ], [ %i.next, %latch ]")
        .replace("%acc = phi i32 [ 0, %pre ], [ %acc.next, %test ]", "%acc = phi i32 [ 0, %pre ], [ %w, %latch ]");
    assert_whole(&text);
}

#[test]
fn test_a_loop_that_leaves_by_two_exits_is_left_whole() {
    let text = up("icmp sle i32 %i, 31", "icmp slt i32 %i.next, %n").replace(
        "yes:\n  %a = add i32 %acc, %i\n  br label %test",
        "yes:\n  %a = add i32 %acc, %i\n  %big = icmp sgt i32 %a, 100000\n  br i1 %big, label %out, label %test",
    );
    let text = text.replace("[ 0, %entry ], [ %r, %done ]", "[ 0, %entry ], [ %r, %done ], [ 7, %yes ]");
    assert_whole(&text);
}

#[test]
fn test_a_target_that_does_not_split_loops_leaves_the_loop_whole() {
    let text = up("icmp sle i32 %i, 31", "icmp slt i32 %i.next, %n");
    let mut module = parsed(&text);
    let before = llrm_mir::print::module(&module);
    managed_on(&mut module, LoopSplit { cleanup: false }, Tuned::default());
    assert_eq!(llrm_mir::print::module(&module), before);
}

/// The sieve of Eratosthenes: past 31 the inner loop is dead, and the counting
/// loop has no store.
const SIEVE: &str = "@c = global [1024 x i8] zeroinitializer

define i32 @f(i32 %limit) {
entry:
  %g = icmp slt i32 2, %limit
  br i1 %g, label %pre, label %out
pre:
  br label %loop
loop:
  %i = phi i32 [ 2, %pre ], [ %i.next, %latch ]
  %count = phi i32 [ 0, %pre ], [ %count.next, %latch ]
  %sum = phi i32 [ 0, %pre ], [ %sum.next, %latch ]
  %p = getelementptr inbounds i8, ptr @c, i32 %i
  %v = load i8, ptr %p
  %comp = icmp ne i8 %v, 0
  br i1 %comp, label %latch, label %body
body:
  %count.b = add i32 %count, 1
  %sum.b = add i32 %sum, %i
  %small = icmp sle i32 %i, 31
  br i1 %small, label %mark, label %latch
mark:
  %sq = mul nsw i32 %i, %i
  %go = icmp slt i32 %sq, %limit
  br i1 %go, label %inner, label %latch
inner:
  %m = phi i32 [ %sq, %mark ], [ %m.next, %inner ]
  %q = getelementptr inbounds i8, ptr @c, i32 %m
  store i8 1, ptr %q
  %m.next = add nsw i32 %m, %i
  %again = icmp slt i32 %m.next, %limit
  br i1 %again, label %inner, label %latch
latch:
  %count.next = phi i32 [ %count, %loop ], [ %count.b, %body ], [ %count.b, %mark ], [ %count.b, %inner ]
  %sum.next = phi i32 [ %sum, %loop ], [ %sum.b, %body ], [ %sum.b, %mark ], [ %sum.b, %inner ]
  %i.next = add nsw i32 %i, 1
  %cont = icmp slt i32 %i.next, %limit
  br i1 %cont, label %loop, label %exit
exit:
  %rc = phi i32 [ %count.next, %latch ]
  %rs = phi i32 [ %sum.next, %latch ]
  %hi = shl i32 %rc, 16
  %r = xor i32 %hi, %rs
  br label %out
out:
  %res = phi i32 [ 0, %entry ], [ %r, %exit ]
  ret i32 %res
}
";

#[test]
fn test_the_sieve_splits_into_a_marking_loop_and_a_counting_loop() {
    let mut module = parsed(SIEVE);
    let before = results(&module, INPUTS);
    assert_eq!(loops(&module), (2, 1));
    let after = managed_on(&mut module, LoopSplit { cleanup: false }, on());
    assert_eq!(results(&module, INPUTS), before, "{after}");
    assert_eq!(loops(&module), (3, 2), "{after}");
    assert_eq!(counting_loop_blocks(&module), 3, "the latch is not a block of its own\n{after}");
    let counting = after.matches("store i8 1").count();
    assert_eq!(counting, 1, "the marking loop is not copied into the counting loop\n{after}");
    let primes_below_100 = (25 << 16) ^ 1060;
    let at_100 = results(&module, &[&[100]]);
    assert_eq!(at_100, [llrm_mir::interpret::Val::Int { bits: primes_below_100, width: 32 }], "{after}");
}

/// The cleanup after the split folded the counting loop's arm into selects, so
/// the loop was two blocks with a branch the vectorizer refuses: arm64 sieve
/// -O3 ran 17098 instructions, 2126 more than -O2.
#[test]
fn test_the_counting_loop_after_the_cleanup_is_one_block() {
    let mut module = parsed(SIEVE);
    let before = results(&module, INPUTS);
    let after = managed_on(&mut module, LoopSplit { cleanup: true }, on());
    assert_eq!(results(&module, INPUTS), before, "{after}");
    assert_eq!(counting_loop_blocks(&module), 1, "{after}");
}

/// The sieve as the mid-end leaves it: SimplifyCFG folded `i < 32` and the
/// inner loop's guard into one `select`, so the border test was no branch
/// of its own and nothing split (arm64 `bench_sieve` -O3 unchanged).
const SIEVE_FOLDED: &str = "@c = global [1024 x i8] zeroinitializer

define i64 @f(i16 %limit) {
entry:
  %none = icmp eq i16 %limit, 0
  br i1 %none, label %out, label %check
check:
  %big = icmp ugt i16 %limit, 2
  br i1 %big, label %pre, label %out
pre:
  %end = zext i16 %limit to i64
  br label %loop
loop:
  %i = phi i64 [ 2, %pre ], [ %i.next, %latch ]
  %sum = phi i64 [ 0, %pre ], [ %sum.next, %latch ]
  %count = phi i16 [ 0, %pre ], [ %count.next, %latch ]
  %p = getelementptr inbounds nuw i8, ptr @c, i64 %i
  %v = load i8, ptr %p
  %prime = icmp eq i8 %v, 0
  br i1 %prime, label %body, label %latch
body:
  %count.b = add i16 %count, 1
  %sum.b = add i64 %sum, %i
  %small = icmp ult i64 %i, 32
  %sq = mul nuw nsw i64 %i, %i
  %inside = icmp ult i64 %sq, %end
  %both = select i1 %small, i1 %inside, i1 false
  br i1 %both, label %mark, label %latch
mark:
  %first = trunc i64 %sq to i16
  %stride = trunc i64 %i to i16
  br label %inner
inner:
  %m = phi i16 [ %m.next, %inner ], [ %first, %mark ]
  %mz = zext i16 %m to i64
  %q = getelementptr inbounds nuw i8, ptr @c, i64 %mz
  store i8 1, ptr %q
  %m.next = add i16 %m, %stride
  %again = icmp ult i16 %m.next, %limit
  br i1 %again, label %inner, label %after
after:
  br label %latch
latch:
  %count.next = phi i16 [ %count, %loop ], [ %count.b, %body ], [ %count.b, %after ]
  %sum.next = phi i64 [ %sum, %loop ], [ %sum.b, %body ], [ %sum.b, %after ]
  %i.next = add nuw i64 %i, 1
  %done = icmp eq i64 %i.next, %end
  br i1 %done, label %exit, label %loop
exit:
  %wide = zext i16 %count.next to i64
  %hi = shl nuw nsw i64 %wide, 16
  %r = xor i64 %hi, %sum.next
  br label %out
out:
  %res = phi i64 [ %r, %exit ], [ 0, %entry ], [ 0, %check ]
  ret i64 %res
}
";

#[test]
fn test_a_border_test_folded_into_a_select_splits_the_sieve() {
    let mut module = parsed(SIEVE_FOLDED);
    let before = results(&module, INPUTS);
    let after = managed_on(&mut module, LoopSplit { cleanup: false }, on());
    assert_eq!(results(&module, INPUTS), before, "{after}");
    assert_eq!(loops(&module), (3, 2), "{after}");
    assert_eq!(counting_loop_blocks(&module), 3, "{after}");
    let at_100 = results(&module, &[&[100]]);
    assert_eq!(at_100, [llrm_mir::interpret::Val::Int { bits: (25 << 16) ^ 1060, width: 64 }], "{after}");
}

/// `sum += i < k ? i * 3 : i + 7`, as the mid-end leaves it: one block, the
/// guard a `select` of values (`split_loops` 68000 -O3 ran 6 instructions a
/// trip where gcc's two loops run 4: +13.6% clocks).
const SELECT_OF_VALUES: &str = "define i32 @f(i32 %n) {
entry:
  %g = icmp sgt i32 %n, 0
  br i1 %g, label %loop, label %out
loop:
  %i = phi i32 [ %i.next, %loop ], [ 0, %entry ]
  %sum = phi i32 [ %sum.next, %loop ], [ 0, %entry ]
  %c = icmp slt i32 %i, 11
  %a = mul nuw nsw i32 %i, 3
  %b = add nuw nsw i32 %i, 7
  %v = select i1 %c, i32 %a, i32 %b
  %sum.next = add nuw nsw i32 %v, %sum
  %i.next = add nuw nsw i32 %i, 1
  %go = icmp eq i32 %i.next, %n
  br i1 %go, label %done, label %loop
done:
  br label %out
out:
  %res = phi i32 [ 0, %entry ], [ %sum.next, %done ]
  ret i32 %res
}
";

#[test]
fn test_a_select_of_values_on_the_counter_splits_into_two_loops() {
    let (module, after) = split(SELECT_OF_VALUES, on());
    assert_eq!(loops(&parsed(SELECT_OF_VALUES)), (1, 1));
    assert_eq!(loops(&module), (2, 2), "{after}");
    // each loop decides the select; the next combine folds it
    assert!(
        after.contains("select i1 true") && after.contains("select i1 false"),
        "the guard is not decided:\n{after}"
    );
}

/// A loop of `body` adds beside the guard: `SELECT_OF_VALUES` grown.
fn heavy(body: usize) -> String {
    let mut text = SELECT_OF_VALUES.replace("  %sum.next = add nuw nsw i32 %v, %sum\n", "");
    let mut last = "%v".to_string();
    let mut adds = String::new();
    for k in 0..body {
        adds.push_str(&format!("  %w{k} = add i32 {last}, {}\n", k + 1));
        last = format!("%w{k}");
    }
    adds.push_str(&format!("  %sum.next = add nuw nsw i32 {last}, %sum\n"));
    text = text.replace("  %i.next = add nuw nsw i32 %i, 1\n", &format!("{adds}  %i.next = add nuw nsw i32 %i, 1\n"));
    text
}

#[test]
fn test_a_loop_of_more_than_the_peel_bound_is_left_whole() {
    // idioms -O3 split an outer loop of inlined bodies and doubled the program
    assert_split(&heavy(60));
    assert_whole(&heavy(120));
}
