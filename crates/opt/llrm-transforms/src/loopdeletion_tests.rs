//! Each body is MIR text in loop-simplify and LCSSA form, run by llrm-mir's
//! interpreter before and after, so a wrong deletion shows as a wrong answer.

use super::LoopDeletion;
use crate::testing::{managed, parsed, results};

/// `text` through the pass: its printed form, run as before on `inputs`.
fn deleted(
    text: &str,
    inputs: &[&[i128]],
) -> String {
    let before = parsed(text);
    let mut module = before.clone();
    let after = managed(&mut module, LoopDeletion);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    after
}

const INPUTS: &[&[i128]] = &[&[0, 5], &[1, 0], &[3, 9]];

/// A loop that only counts, tested in its header: `x` is the answer, the
/// counter is read by nobody.
const COUNTER: &str = "define i16 @f(i16 %x, i16 %n) mustprogress {
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %next, %body ]
  %go = icmp slt i16 %i, 10
  br i1 %go, label %body, label %out

body:
  %next = add i16 %i, 1
  br label %head

out:
  ret i16 %x
}
";

#[test]
fn a_loop_that_only_counts_is_deleted() {
    let after = deleted(COUNTER, INPUTS);
    assert!(!after.contains("phi"), "{after}");
    assert!(after.contains("br label %out"), "{after}");
}

/// The same loop rotated: one block, tested at the bottom. The old pass took
/// only the header-tested form (`loopexit.rs` took two blocks with the test
/// before the step), so a rotated counter loop stayed.
#[test]
fn a_rotated_loop_that_only_counts_is_deleted() {
    let text = "define i16 @f(i16 %x, i16 %n) {
b0:
  br label %loop

loop:
  %i = phi i16 [ 0, %b0 ], [ %next, %loop ]
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, 10
  br i1 %go, label %loop, label %out

out:
  ret i16 %x
}
";
    let after = deleted(text, INPUTS);
    assert!(!after.contains("phi"), "{after}");
}

/// A value the exit phi takes that the loop computes is not invariant.
#[test]
fn a_loop_whose_result_is_read_stays() {
    let text = COUNTER.replace("out:\n  ret i16 %x", "out:\n  %r = phi i16 [ %i, %head ]\n  ret i16 %r");
    let after = deleted(&text, INPUTS);
    assert!(after.contains("phi i16 [ 0"), "{after}");
}

#[test]
fn a_loop_that_stores_stays() {
    let text = "@g = global i16 0

define i16 @f(i16 %x, i16 %n) mustprogress {
b0:
  br label %loop

loop:
  %i = phi i16 [ 0, %b0 ], [ %next, %loop ]
  store i16 %i, ptr @g
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, 10
  br i1 %go, label %loop, label %out

out:
  ret i16 %x
}
";
    let after = deleted(text, INPUTS);
    assert!(after.contains("store i16 %i"), "{after}");
}

/// A loop nothing may end (it waits on memory) is kept unless it is told to
/// make progress. (`i < n` has a bound ScalarEvolution finds, so it is not
/// such a loop.)
#[test]
fn a_loop_of_unknown_length_stays_unless_it_must_make_progress() {
    let waiting = "@g = global i16 7

define i16 @f(i16 %x, i16 %n) {
b0:
  br label %loop

loop:
  %v = load i16, ptr @g
  %go = icmp ne i16 %v, 7
  br i1 %go, label %loop, label %out

out:
  ret i16 %x
}
";
    let after = deleted(waiting, &[&[1, 0]]);
    assert!(after.contains("load"), "{after}");
    let told = waiting.replace("@f(i16 %x, i16 %n) {", "@f(i16 %x, i16 %n) mustprogress {");
    let after = deleted(&told, &[&[1, 0]]);
    assert!(!after.contains("load"), "{after}");
}

/// A loop its preheader's only predecessor never enters is deleted, whatever
/// it holds.
#[test]
fn a_loop_that_never_executes_is_deleted() {
    let text = "@g = global i16 0

define i16 @f(i16 %x, i16 %n) {
b0:
  br i1 false, label %pre, label %out

pre:
  br label %loop

loop:
  %i = phi i16 [ 0, %pre ], [ %next, %loop ]
  store i16 %i, ptr @g
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, %n
  br i1 %go, label %loop, label %exit

exit:
  br label %out

out:
  %r = phi i16 [ %x, %b0 ], [ %x, %exit ]
  ret i16 %r
}
";
    let after = deleted(text, INPUTS);
    assert!(!after.contains("store"), "{after}");
}

/// A loop that stores but leaves on its first iteration has no backedge left.
#[test]
fn a_loop_that_exits_on_its_first_iteration_loses_its_backedge() {
    let text = "@g = global i16 0

define i16 @f(i16 %x, i16 %n) {
b0:
  br label %loop

loop:
  %i = phi i16 [ 0, %b0 ], [ %next, %latch ]
  store i16 %x, ptr @g
  %c = icmp eq i16 %i, 0
  br i1 %c, label %out, label %latch

latch:
  %next = add i16 %i, 1
  br label %loop

out:
  ret i16 %x
}
";
    let after = deleted(text, INPUTS);
    assert!(after.contains("unreachable"), "the latch no longer jumps back: {after}");
}

/// `breakLoopBackedge` splits a switch latch's backedge with `SplitEdge`,
/// which stands the block right after the latch (LoopUtils.cpp:800), not at
/// the end of the function.
#[test]
fn the_block_that_replaces_the_backedge_stands_after_the_latch() {
    let text = "@g = global i16 0

define i16 @f(i16 %x, i16 %n) {
b0:
  br label %loop

loop:
  %i = phi i16 [ 0, %b0 ], [ %next, %latch ]
  store i16 %x, ptr @g
  %c = icmp eq i16 %i, 0
  br i1 %c, label %out, label %latch

latch:
  %next = add i16 %i, 1
  switch i16 %x, label %loop []

out:
  ret i16 %x
}
";
    let after = deleted(text, INPUTS);
    let (bridge, out) = (after.find("unreachable").expect("the bridge"), after.find("out:").expect("out"));
    assert!(bridge < out, "the bridge stands before out: {after}");
}

/// `n` dead loops in a row cost entries in proportion to `n`.
fn sequence(n: usize) -> String {
    let mut text = String::from("define i16 @f(i16 %x) mustprogress {\nb0:\n  br label %l0\n\n");
    for k in 0..n {
        let next = if k + 1 == n { "end".to_owned() } else { format!("l{}", k + 1) };
        let pre = if k == 0 { "b0".to_owned() } else { format!("x{}", k - 1) };
        text += &format!(
            "l{k}:\n  %i{k} = phi i16 [ 0, %{pre} ], [ %n{k}, %l{k} ]\n  %n{k} = add i16 %i{k}, 1\n  %g{k} = icmp slt i16 %n{k}, 10\n  br i1 %g{k}, label %l{k}, label %x{k}\n\nx{k}:\n  br label %{next}\n\n"
        );
    }
    text += "end:\n  ret i16 %x\n}\n";
    text
}

#[test]
fn deleting_loops_in_sequence_scans_in_proportion_to_their_number() {
    use llrm_mir::edit::layout_scanned;
    let scanned = |n: usize| {
        let mut module = parsed(&sequence(n));
        let before = layout_scanned();
        let after = managed(&mut module, LoopDeletion);
        assert!(!after.contains("phi"), "{after}");
        layout_scanned() - before
    };
    let (a, b, c) = (scanned(64), scanned(128), scanned(256));
    assert!(b * 100 <= a * 250 && c * 100 <= b * 250, "64: {a}, 128: {b}, 256: {c}");
}

/// Every loop pass built its own `ScalarEvolution`: d_faces -O2 made 1774 SCEV
/// expressions for 468 distinct values (the pin keeps one). `loopdeletion`
/// asks the trip count `indvars` has just worked out and made it again.
#[test]
fn test_loopdeletion_reuses_the_expressions_indvars_made() {
    use llrm_mir::counts::{Event, get};
    let text = "define i16 @f(i16 %seed, i16 %n) {
b0:
  br label %b1

b1:
  %carried = phi i16 [ %seed, %b0 ], [ %stepped, %b2 ]
  %more = icmp ult i16 %carried, %n
  br i1 %more, label %b2, label %b3

b2:
  %stepped = add i16 %carried, 3
  br label %b1

b3:
  ret i16 %carried
}
";
    let nodes = |loopdeletion: bool| {
        let mut module = crate::testing::parsed(text);
        let mut manager = llrm_mir::passes::PassManager::default();
        manager.add(crate::indvars::IndVars { lftr: true, size: false });
        if loopdeletion {
            manager.add(super::LoopDeletion);
        }
        let before = get(Event::ScevNodes);
        manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
        get(Event::ScevNodes) - before
    };
    let (alone, both) = (nodes(false), nodes(true));
    assert!(alone > 0);
    assert_eq!(both, alone, "loopdeletion made {} expressions again", both - alone);
}

/// observed.c's `mix` at -O2 -g lost the debug record of its parameter `n`,
/// which stands before the preheader's branch, when its loop was deleted: the
/// branch was replaced and its records dropped, and gdb read `n` as optimised
/// out where it is in a register throughout.
#[test]
fn the_debug_records_before_the_preheaders_branch_survive_a_deletion() {
    let text = COUNTER.replace("b0:\n  br label %head", "b0:\n  #dbg_value(i16 %n, !1)\n  br label %head")
        + "\n!1 = !{!\"f\", !\"n\", !2, i64 0, i64 1, i64 0}\n!2 = !{!\"int\"}\n";
    let after = deleted(&text, INPUTS);
    assert!(!after.contains("phi") && after.contains("#dbg_value(i16 %n"), "{after}");
}

/// A pass that only reads the dominator tree, as the pass before this one in
/// the pipeline does.
struct ReadsTree;

impl llrm_mir::passes::FunctionPass for ReadsTree {
    fn name(&self) -> &'static str {
        "reads-tree"
    }

    fn run(
        &mut self,
        unit: &mut llrm_mir::passes::Unit,
        analyses: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        analyses.get::<llrm_mir::passes::Dominators>(unit.context, unit.layout, unit.function);
        llrm_mir::passes::PreservedAnalyses::all()
    }
}

/// The pass took a tree of its own (a build or a copy of the memoized one)
/// where the manager held the tree already: two trees for a run that needs one.
#[test]
fn test_the_managers_dominator_tree_is_used_not_a_second_one() {
    use llrm_mir::counts::{Event, get};
    let mut module = parsed(COUNTER);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(ReadsTree);
    manager.add(LoopDeletion);
    let trees = || get(Event::Dominators) + get(Event::DominatorsReused);
    let before = trees();
    manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).unwrap();
    assert_eq!(trees() - before, 1);
}

/// A loop that only counts, then a counted loop `n` times: the first is
/// deleted, the second is not touched.
const DEAD_THEN_LIVE: &str = "define i32 @f(i32 %n) mustprogress {
b0:
  br label %p0

p0:
  br label %h0

h0:
  %i0 = phi i32 [ 0, %p0 ], [ %n0, %l0 ]
  %g0 = icmp ult i32 %i0, 10
  br i1 %g0, label %l0, label %x0

l0:
  %n0 = add nuw nsw i32 %i0, 1
  br label %h0

x0:
  br label %p1

p1:
  br label %h1

h1:
  %i1 = phi i32 [ 0, %p1 ], [ %n1, %l1 ]
  %g1 = icmp ult i32 %i1, %n
  br i1 %g1, label %l1, label %x1

l1:
  %n1 = add nuw nsw i32 %i1, 1
  br label %h1

x1:
  %r = phi i32 [ %i1, %h1 ]
  ret i32 %r
}
";

/// The trip counts `steps` work out: 0 is indvars, 1 loopdeletion.
fn counts_of(
    text: &str,
    steps: &[u8],
) -> usize {
    let mut module = parsed(text);
    let before = llrm_analysis::scev::trip::backedge_taken_computations();
    let mut manager = llrm_mir::passes::PassManager::default();
    for step in steps {
        if *step == 0 {
            manager.add(crate::indvars::IndVars { lftr: true, size: false });
        } else {
            manager.add(LoopDeletion);
        }
    }
    manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).unwrap();
    llrm_analysis::scev::trip::backedge_taken_computations() - before
}

/// A run that deleted a loop dropped every expression, so the next run (and
/// the passes after) counted the loop it left again: 2 counts for the 1 loop
/// that stays. `forgetLoop` drops only what the deletion reached.
#[test]
fn test_a_deleted_loop_leaves_the_other_loops_trip_counts() {
    assert_eq!(counts_of(DEAD_THEN_LIVE, &[1, 1]), 1);
}

/// `n` guarded counted loops in a row, each storing, so none is deleted.
fn live_sequence(n: usize) -> String {
    let mut text = String::from(
        "@a = global i32 0\n@arr = global [4096 x i32] zeroinitializer\n\ndefine void @f(i32 %n) {\nb0:\n  %g = icmp sgt i32 %n, 0\n  br i1 %g, label %p0, label %end\n\n",
    );
    for k in 0..n {
        let next = if k + 1 == n { "end".to_owned() } else { format!("p{}", k + 1) };
        text += &format!(
            "p{k}:\n  br label %l{k}\n\nl{k}:\n  %i{k} = phi i32 [ 0, %p{k} ], [ %n{k}, %l{k} ]\n  %m{k} = mul nuw nsw i32 %i{k}, 3\n  %q{k} = and i32 %m{k}, 4095\n  %p{k} = getelementptr inbounds i32, ptr @arr, i32 %q{k}\n  %v{k} = load i32, ptr %p{k}\n  store i32 %v{k}, ptr @a\n  %n{k} = add nuw nsw i32 %i{k}, 1\n  %c{k} = icmp eq i32 %n{k}, %n\n  br i1 %c{k}, label %x{k}, label %l{k}\n\nx{k}:\n  br label %{next}\n\n"
        );
    }
    text += "end:\n  ret void\n}\n";
    text
}

/// The guards of a loop climb every block above it, and the blocks seen were a
/// list searched whole at each step: 128 loops in a row cost `loopdeletion`
/// 32 M instructions in `collect_from_block` where the pin's set costs 9 M, and
/// the work grew with the cube of the loops. A membership test costs one
/// probe, whatever the climb has seen.
#[test]
fn collecting_loop_guards_probes_each_visited_block_once() {
    let probes = |n: usize| {
        let mut module = parsed(&live_sequence(n));
        let before = llrm_analysis::scev::guards::visits();
        managed(&mut module, LoopDeletion);
        let after = llrm_analysis::scev::guards::visits();
        (after.0 - before.0, after.1 - before.1)
    };
    let (inserts, examined) = probes(128);
    assert!(inserts > 128 * 64, "the climb reaches the earlier loops: {inserts}");
    assert!(examined <= 2 * inserts, "{examined} elements examined for {inserts} inserts");
}

/// `n` counted loops in a row, each storing, to the same bound `%n`.
fn stored_loops(n: usize) -> String {
    let mut text = String::from(
        "@a = global i32 0\n\ndefine void @f(i32 %n) {\nb0:\n  %g = icmp sgt i32 %n, 0\n  br i1 %g, label %p0, label %end\n\n",
    );
    for k in 0..n {
        let next = if k + 1 == n { "end".to_owned() } else { format!("p{}", k + 1) };
        text += &format!(
            "p{k}:\n  br label %l{k}\n\nl{k}:\n  %i{k} = phi i32 [ 0, %p{k} ], [ %n{k}, %l{k} ]\n  store i32 %i{k}, ptr @a\n  %n{k} = add nuw nsw i32 %i{k}, 1\n  %c{k} = icmp eq i32 %n{k}, %n\n  br i1 %c{k}, label %x{k}, label %l{k}\n\nx{k}:\n  br label %{next}\n\n"
        );
    }
    text + "end:\n  ret void\n}\n"
}

/// The disposition of an expression was held in a list searched whole, and an
/// expression that every loop of a function asks about (the bound `%n`) had a
/// list as long as the function's loops: 7 M entries looked at for 81 K
/// lookups over 128 nests, 20 M of the 233 M instructions of
/// `loop-load-elim`. A lookup costs the same whatever the loops before it.
#[test]
fn a_disposition_lookup_costs_the_same_whatever_the_loops_before_it() {
    let looked_at = |n: usize| {
        let mut module = parsed(&stored_loops(n));
        let before = llrm_analysis::scev::disposition::examined();
        managed(&mut module, LoopDeletion);
        llrm_analysis::scev::disposition::examined() - before
    };
    let (a, b) = (looked_at(32), looked_at(128));
    assert!(b * 100 <= a * 450, "32 loops: {a}, 128 loops: {b}");
}
