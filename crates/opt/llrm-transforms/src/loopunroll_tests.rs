//! `unroll_loop` on loops the interpreter runs, before and after: a loop
//! with its test in the header, one with its test in the latch (rotated),
//! one that leaves from two blocks, and one that holds a loop.

use llrm_analysis::cfg;
use llrm_mir::module::{BlockId, Function, Module};

use super::{Exit, Options, Unrolled, unroll_loop};
use crate::testing::{f, parsed, printed, results};

fn named(
    function: &Function,
    name: &str,
) -> BlockId {
    function
        .layout()
        .iter()
        .copied()
        .find(|&one| function.block(one).name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no %{name}"))
}

/// `@f` with the loop headed by `header` unrolled `count` times; each
/// `(block, trips)` of `exits` says how often that block runs when it leaves.
fn unrolled(
    module: &mut Module,
    header: &str,
    count: u32,
    exits: &[(&str, u32)],
) -> Unrolled {
    let function = f(module);
    let shape = cfg::Shape::of(function);
    let at = cfg::id(named(function, header));
    let loop_ = shape.loops.iter().find(|one| one.header == at).expect("the loop").clone();
    let exits = exits.iter().map(|&(name, trip_count)| Exit { block: named(function, name), trip_count }).collect();
    unroll_loop(function, &loop_, &Options { count, max_trip_count: count, max_or_zero: false, exits }).unwrap()
}

fn loops_left(module: &mut Module) -> usize {
    cfg::Shape::of(f(module)).loops.len()
}

/// Test in the header: `i < 3` runs the header four times.
const HEADER_TESTED: &str = "define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %acc = phi i16 [ %x, %b0 ], [ %s, %b2 ]
  %go = icmp slt i16 %i, 3
  br i1 %go, label %b2, label %b3

b2:
  %m = mul i16 %acc, %n
  %s = add i16 %m, %i
  %next = add i16 %i, 1
  br label %b1

b3:
  %r = phi i16 [ %acc, %b1 ]
  %k = phi i16 [ %i, %b1 ]
  %u = add i16 %k, %r
  ret i16 %u
}
";

/// Test in the latch, as `loop-rotate` leaves a loop: `i < 4` after the body.
const ROTATED: &str = "define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %acc = phi i16 [ %x, %b0 ], [ %s, %b2 ]
  %m = mul i16 %acc, %n
  %s = add i16 %m, %i
  br label %b2

b2:
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, 4
  br i1 %go, label %b1, label %b3

b3:
  %r = phi i16 [ %s, %b2 ]
  %k = phi i16 [ %next, %b2 ]
  %u = add i16 %k, %r
  ret i16 %u
}
";

const INPUTS: &[&[i128]] = &[&[1, 2], &[0, 5], &[-3, 7], &[100, -1]];

#[test]
fn test_header_tested_loop_unrolls_and_computes_the_same() {
    let mut module = parsed(HEADER_TESTED);
    let before = results(&module, INPUTS);
    assert_eq!(loops_left(&mut module), 1);
    assert_eq!(unrolled(&mut module, "b1", 4, &[("b1", 4)]), Unrolled::Fully);
    let text = printed(&module);
    assert_eq!(loops_left(&mut module), 0, "{text}");
    assert_eq!(results(&module, INPUTS), before, "{text}");
}

/// A loop whose test is in the latch has no test in its header: the guards of
/// `unroll` and `peel` refused it, and with the rotate slot on they refused
/// 116 loops against 83.
#[test]
fn test_rotated_loop_unrolls_and_computes_the_same() {
    let mut module = parsed(ROTATED);
    let before = results(&module, INPUTS);
    assert_eq!(unrolled(&mut module, "b1", 4, &[("b2", 4)]), Unrolled::Fully);
    let text = printed(&module);
    assert_eq!(loops_left(&mut module), 0, "{text}");
    assert_eq!(results(&module, INPUTS), before, "{text}");
}

/// The count alone, with no exit's trip count known, still unrolls: only the
/// last copy's latch is known to leave.
#[test]
fn test_unknown_exit_trip_count_keeps_each_copys_test() {
    let mut module = parsed(ROTATED);
    let before = results(&module, INPUTS);
    assert_eq!(unrolled(&mut module, "b1", 4, &[]), Unrolled::Fully);
    let text = printed(&module);
    assert_eq!(loops_left(&mut module), 0, "{text}");
    assert_eq!(results(&module, INPUTS), before, "{text}");
}

/// Leaves from the header and from the body, into one block that merges them.
#[test]
fn test_two_exits_unroll_and_compute_the_same() {
    let mut module = parsed(
        "define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b3 ]
  %acc = phi i16 [ %x, %b0 ], [ %s, %b3 ]
  %go = icmp slt i16 %i, 5
  br i1 %go, label %b2, label %b4

b2:
  %s = add i16 %acc, %n
  %big = icmp sgt i16 %s, 12
  br i1 %big, label %b4, label %b3

b3:
  %next = add i16 %i, 1
  br label %b1

b4:
  %r = phi i16 [ %acc, %b1 ], [ %s, %b2 ]
  %k = phi i16 [ %i, %b1 ], [ %i, %b2 ]
  %u = add i16 %k, %r
  ret i16 %u
}
",
    );
    let inputs: &[&[i128]] = &[&[0, 1], &[0, 5], &[3, 4], &[-9, 2], &[20, 1]];
    let before = results(&module, inputs);
    assert_eq!(unrolled(&mut module, "b1", 6, &[("b1", 6)]), Unrolled::Fully);
    let text = printed(&module);
    assert_eq!(loops_left(&mut module), 0, "{text}");
    assert_eq!(results(&module, inputs), before, "{text}");
}

/// The outer loop of two trips holds a loop of its own; its copies each get
/// the inner loop.
#[test]
fn test_loop_holding_a_loop_unrolls_the_outer_one() {
    let mut module = parsed(
        "define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %o = phi i16 [ 0, %b0 ], [ %o1, %b4 ]
  %t = phi i16 [ %x, %b0 ], [ %t2, %b4 ]
  br label %b2

b2:
  %j = phi i16 [ 0, %b1 ], [ %j1, %b2 ]
  %u = phi i16 [ %t, %b1 ], [ %u1, %b2 ]
  %u1 = add i16 %u, %n
  %j1 = add i16 %j, 1
  %more = icmp slt i16 %j1, %n
  br i1 %more, label %b2, label %b3

b3:
  %w = phi i16 [ %u1, %b2 ]
  br label %b4

b4:
  %t2 = add i16 %w, %o
  %o1 = add i16 %o, 1
  %again = icmp slt i16 %o1, 2
  br i1 %again, label %b1, label %b5

b5:
  %r = phi i16 [ %t2, %b4 ]
  ret i16 %r
}
",
    );
    let inputs: &[&[i128]] = &[&[1, 1], &[2, 3], &[0, 4]];
    let before = results(&module, inputs);
    assert_eq!(loops_left(&mut module), 2);
    assert_eq!(unrolled(&mut module, "b1", 2, &[("b4", 2)]), Unrolled::Fully);
    let text = printed(&module);
    assert_eq!(loops_left(&mut module), 2, "the copies each hold the inner loop\n{text}");
    assert_eq!(results(&module, inputs), before, "{text}");
}

#[test]
fn test_a_count_below_the_maximum_trip_count_is_refused() {
    let mut module = parsed(ROTATED);
    let function = f(&mut module);
    let shape = cfg::Shape::of(function);
    let loop_ = shape.loops[0].clone();
    let partial = Options { count: 2, max_trip_count: 4, max_or_zero: false, exits: Vec::new() };
    assert!(unroll_loop(function, &loop_, &partial).is_err());
}

fn rotated_with(trips: u32) -> String {
    ROTATED.replace("i16 %next, 4", &format!("i16 %next, {trips}"))
}

/// Layout, block and use-list entries `unroll_loop` looks over at `trips`.
fn unroll_scanned(trips: u32) -> usize {
    use llrm_mir::edit::layout_scanned;
    let mut module = parsed(&rotated_with(trips));
    let before = layout_scanned();
    assert_eq!(unrolled(&mut module, "b1", trips, &[("b2", trips)]), Unrolled::Fully);
    layout_scanned() - before
}

/// Unrolling scanned the layout, the exit's use list and its phi once per copy,
/// so doubling the count quadrupled the entries looked over (seen: 3.3 to 4x
/// by time before the fix; counted, not timed, because time failed under load).
#[test]
fn test_unroll_scans_double_with_the_count() {
    let (a, b, c) = (unroll_scanned(256), unroll_scanned(512), unroll_scanned(1024));
    assert!(b * 100 <= a * 250 && c * 100 <= b * 250, "256: {a}, 512: {b}, 1024: {c}");
}

/// The copies of an unrolled loop stand in order right after its latch, before
/// the exit (LoopUnroll.cpp:1214). They stood at the end of the function.
#[test]
fn test_the_copies_stand_after_the_latch_before_the_exit() {
    let mut module = parsed(ROTATED);
    assert_eq!(unrolled(&mut module, "b1", 2, &[]), Unrolled::Fully);
    let function = f(&mut module);
    let order: Vec<String> =
        function.layout().iter().map(|&one| function.block(one).name.clone().unwrap_or("new".into())).collect();
    assert_eq!(order, ["b0", "b1", "b2", "new", "new", "b3"], "{}", printed(&module));
}
