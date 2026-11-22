//! Backward threading on the shape of `queens`' column loop: a guard of the
//! inner loop that every trip through the loop already passed.

use llrm_mir::passes::PassManager;
use llrm_mir::target::Neutral;

use crate::backwardthreading::{BackwardThreading, MAX_CHAIN, MAX_THREADS};
use crate::testing::{parsed, printed, results};

/// `head` tests `row > 0` every trip; `latch` joins the path through `loop`
/// (which passed the test) with the one through `hit`.
const COLUMNS: &str = "define i16 @f(i16 %row, i16 %n) {
entry:
  br label %head

head:
  %col = phi i16 [ 0, %entry ], [ %next, %latch ]
  %found = phi i16 [ 0, %entry ], [ %found2, %latch ]
  %g = icmp sgt i16 %row, 0
  br i1 %g, label %enter, label %hit

enter:
  br label %loop

loop:
  %u = icmp eq i16 %col, 3
  br i1 %u, label %latch, label %hit

hit:
  %bumped = add i16 %found, 1
  br label %latch

latch:
  %found2 = phi i16 [ %found, %loop ], [ %bumped, %hit ]
  %next = add i16 %col, 1
  %done = icmp eq i16 %next, %n
  br i1 %done, label %exit, label %head

exit:
  ret i16 %found2
}
";

const INPUTS: &[&[i128]] = &[&[1, 4], &[0, 3], &[2, 5], &[-1, 6], &[1, 1]];

fn threaded(text: &str) -> (String, bool) {
    let before = parsed(text);
    let mut module = before.clone();
    let mut manager = PassManager::default();
    manager.add(BackwardThreading {
        enabled: true,
        max_threads: MAX_THREADS,
        relations: false,
        early: false,
        chain: Some(MAX_CHAIN),
    });
    manager.run_module(&mut module, std::rc::Rc::new(Neutral)).unwrap();
    let text = printed(&module);
    assert_eq!(results(&module, INPUTS), results(&before, INPUTS), "{text}");
    let changed = printed(&before) != text;
    (text, changed)
}

#[test]
fn a_join_of_a_known_and_an_unknown_path_is_threaded_for_the_known_one() {
    let (text, changed) = threaded(COLUMNS);
    assert!(changed, "the guard is tested again after the path that passed it:\n{text}");
    // The copy of `head` that each path through `loop` takes jumps to `enter`:
    // the one that hit (`latch` from `loop`) and the one that did not (`latch`
    // from `hit`, a chain of two blocks).
    assert_eq!(text.matches("br label %enter").count(), 2, "{text}");
}

/// The path through `loop` is taken several trips running (`col > 1`): each
/// copy of `head` must read the column the copy of `latch` before it made,
/// not the one `head` itself last made. Reading `head`'s own phi left the
/// column where the first threaded trip put it: a loop that never ended
/// (queens at arm64 -O2 ran past 217 M instructions, 118 K before).
#[test]
fn trips_through_the_threaded_path_run_on_each_others_values() {
    let text = COLUMNS.replace("%u = icmp eq i16 %col, 3", "%u = icmp ugt i16 %col, 1");
    let (out, changed) = threaded(&text);
    assert!(changed, "{out}");
}

/// A target that ranks bytes first turns the pass off
/// (`Options::backward_threading`): z80 and 6502 `idioms` grew 188 and 619
/// bytes with it on.
#[test]
fn a_target_that_turns_it_off_keeps_the_path_unthreaded() {
    let before = parsed(COLUMNS);
    let mut module = before.clone();
    let mut manager = PassManager::default();
    manager.add(BackwardThreading::from_env().when(false));
    manager.run_module(&mut module, std::rc::Rc::new(Neutral)).unwrap();
    assert_eq!(printed(&module), printed(&before));
}

/// The join `p` ends in a switch with two cases to the same block `s`. The copy
/// of `p` has the two edges too, so each phi of `s` needs two inputs from it:
/// one was added, and the verifier (then `switchvalue` in SimplifyCFG, queens
/// -O3: "an input from the existing predecessor") refused the function.
#[test]
fn a_join_ending_in_a_switch_gives_each_of_its_edges_an_input() {
    let text = "define i32 @f(i32 %x, i32 %s, i1 %c) {
entry:
  %pos = icmp sgt i32 %x, 0
  br i1 %pos, label %q, label %r

q:
  br label %p

r:
  br label %p

p:
  switch i32 %s, label %s_in [
    i32 0, label %s_in
    i32 1, label %bb
  ]

bb:
  %g = icmp sgt i32 %x, 0
  br i1 %g, label %a, label %s_in

a:
  ret i32 7

s_in:
  %v = phi i32 [ 1, %p ], [ 1, %p ], [ 3, %bb ]
  ret i32 %v
}
";
    let before = parsed(text);
    let mut module = before.clone();
    let mut manager = PassManager::default();
    manager.add(BackwardThreading {
        enabled: true,
        max_threads: MAX_THREADS,
        relations: false,
        early: false,
        chain: Some(MAX_CHAIN),
    });
    manager.run_module(&mut module, std::rc::Rc::new(Neutral)).unwrap();
    let out = printed(&module);
    assert_ne!(out, printed(&before), "the guard was not threaded:\n{out}");
}

/// `head` tests its own phi `%x == 2`; the path through `exact` knows `%x == 4`
/// of the trip that took it, but `head` reads the next trip's `%x` (2). Read as
/// the same value, the copy of `head` for that path skipped its test:
/// x_fib -O3 with depth-8 copies returned 5954 for fib(20), 6765 expected.
#[test]
fn a_fact_about_a_header_phi_is_not_the_next_trips_value() {
    let text = "define i32 @f(i32 %n) {
entry:
  br label %head

head:
  %x = phi i32 [ %n, %entry ], [ %y, %latch ]
  %acc = phi i32 [ 0, %entry ], [ %acc2, %latch ]
  %two = icmp eq i32 %x, 2
  br i1 %two, label %done, label %test

test:
  %four = icmp eq i32 %x, 4
  br i1 %four, label %exact, label %other

exact:
  br label %latch

other:
  %bumped = add i32 %acc, 100
  br label %latch

latch:
  %acc2 = phi i32 [ %acc, %exact ], [ %bumped, %other ]
  %y = add i32 %x, -2
  %go = icmp sgt i32 %y, 0
  br i1 %go, label %head, label %done

done:
  %r = phi i32 [ %acc, %head ], [ %acc2, %latch ]
  ret i32 %r
}
";
    let before = parsed(text);
    let mut module = before.clone();
    let mut manager = PassManager::default();
    manager.add(BackwardThreading {
        enabled: true,
        max_threads: MAX_THREADS,
        relations: false,
        early: false,
        chain: Some(MAX_CHAIN),
    });
    manager.run_module(&mut module, std::rc::Rc::new(Neutral)).unwrap();
    let inputs: &[&[i128]] = &[&[4], &[6], &[5], &[3], &[8], &[2], &[1]];
    assert_eq!(results(&module, inputs), results(&before, inputs), "{}", printed(&module));
}

/// Three tests of one unsigned pair, `a < b`, `a <= b`, `a > b`, each adding to
/// a sum on its own arm. The edge from the arm that passed `a < b` into the
/// join knows `a <= b` too, and gcc's relation oracle (`value-relation.cc`)
/// says so where a range of one value against a constant cannot: before, the
/// join kept all three compares on every path (`word_compare_branches` -O2:
/// 18,618 instructions, gcc 17,184).
const LADDER: &str = "define i32 @f(i32 %a, i32 %b) {
entry:
  %lt = icmp ult i32 %a, %b
  br i1 %lt, label %t1, label %j1

t1:
  br label %j1

j1:
  %r1 = phi i32 [ 0, %entry ], [ 1, %t1 ]
  %le = icmp ule i32 %a, %b
  br i1 %le, label %t2, label %j2

t2:
  %r2a = add i32 %r1, 2
  br label %j2

j2:
  %r2 = phi i32 [ %r1, %j1 ], [ %r2a, %t2 ]
  %gt = icmp ugt i32 %a, %b
  br i1 %gt, label %t3, label %j3

t3:
  %r3a = add i32 %r2, 4
  br label %j3

j3:
  %r3 = phi i32 [ %r2, %j2 ], [ %r3a, %t3 ]
  ret i32 %r3
}
";

fn related(
    text: &str,
    relations: bool,
) -> (String, bool) {
    let before = parsed(text);
    let mut module = before.clone();
    let mut manager = PassManager::default();
    manager.add(BackwardThreading { enabled: true, max_threads: MAX_THREADS, relations, early: false, chain: None });
    manager.run_module(&mut module, std::rc::Rc::new(Neutral)).unwrap();
    let inputs: &[&[i128]] = &[&[1, 2], &[2, 1], &[3, 3], &[0, 0], &[5, 9], &[9, 5], &[0, -1], &[-1, 0]];
    let out = printed(&module);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{out}");
    let changed = out != printed(&before);
    (out, changed)
}

/// Whether the path that takes the first arm of `f`'s entry reaches the return
/// without testing a condition.
fn first_arm_tests_nothing(text: &str) -> bool {
    let module = parsed(text);
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    let entry = *function.layout().first().expect("an entry");
    let mut block = function.successors(entry)[0];
    loop {
        match function.successors(block)[..] {
            [] => return true,
            [next] => block = next,
            _ => return false,
        }
    }
}

#[test]
fn a_path_that_passed_a_compare_decides_the_compares_it_implies() {
    assert!(!first_arm_tests_nothing(LADDER), "the ladder tests again on the arm that passed `a < b`");
    let (text, changed) = related(LADDER, true);
    assert!(changed, "the edge that passed `a < b` decides `a <= b`:\n{text}");
    assert!(first_arm_tests_nothing(&text), "{text}");
}

#[test]
fn a_target_without_relations_keeps_the_ladder() {
    let (text, changed) = related(LADDER, false);
    assert!(!changed, "{text}");
}

/// The join `bb` tests `x == 3`, which the path through `q` knows by a range of
/// `x`, not by a relation of a pair: the join shape threads relations only.
/// Range facts threaded seven latch tests of `known_loads_walks` -O2 (+9.6%
/// executed instructions, 33,114 -> 36,298, a counted cell lost).
#[test]
fn a_join_holding_a_test_a_range_decides_is_not_threaded_by_relation() {
    let text = "define i32 @f(i32 %x, i32 %n) {
entry:
  %g = icmp sgt i32 %n, 0
  br i1 %g, label %a, label %b

a:
  %k = icmp eq i32 %x, 3
  br i1 %k, label %q, label %out

b:
  br label %bb

q:
  br label %bb

bb:
  %again = icmp eq i32 %x, 3
  br i1 %again, label %s, label %out

s:
  ret i32 1

out:
  ret i32 0
}
";
    let (out, changed) = related(text, true);
    assert!(!changed, "a range of `x` decides it, not a relation:\n{out}");
}

/// queens' `place` at -O2 (arm64), the rotated shape: `%g`, the entry guard of
/// the inner loop, is hoisted out of the column loop, and `head` branches on it
/// every trip. The path that hit (`inner` -> `latch`) is a path of one block;
/// the one that ran the whole inner loop (`step` -> `call` -> `latch` ->
/// `head`) is a chain of two, and the facts at `step` cross the join `pre` the
/// first thread made.
const ROTATED: &str = "define i32 @f(i32 %row, i32 %n) {
entry:
  %g = icmp sgt i32 %row, 0
  br label %head

head:
  %col = phi i32 [ 0, %entry ], [ %next, %latch ]
  %found = phi i32 [ 0, %entry ], [ %found2, %latch ]
  br i1 %g, label %pre, label %call

pre:
  br label %inner

inner:
  %i = phi i32 [ 0, %pre ], [ %i1, %step ]
  %hit = icmp eq i32 %i, %col
  br i1 %hit, label %latch, label %step

step:
  %i1 = add i32 %i, 1
  %end = icmp eq i32 %i1, %row
  br i1 %end, label %call, label %inner

call:
  %x = add i32 %found, 7
  br label %latch

latch:
  %found2 = phi i32 [ %x, %call ], [ %found, %inner ]
  %next = add i32 %col, 1
  %done = icmp eq i32 %next, %n
  br i1 %done, label %exit, label %head

exit:
  ret i32 %found2
}
";

/// Both ways round the column loop skip the guard: the path that ran the whole
/// inner loop (`step`) no longer goes through `call` and `latch` to `head`.
/// queens at arm64 -O2 kept the guard on the path that ran the whole
/// inner loop (`cmp row, #1; b.lt` each column trip: +4.2% executed
/// instructions against gcc).
#[test]
fn a_loop_guard_every_path_passed_is_skipped_on_each_back_edge() {
    let (text, changed) = threaded(ROTATED);
    assert!(changed, "{text}");
    assert!(!text.contains("br i1 %end, label %call"), "`step` still reaches the guard:\n{text}");
    assert_eq!(text.matches("br i1 %g,").count(), 1, "{text}");
}

/// The guard reads a value the latch makes each trip (`%next > 2`): what the
/// edge into `pre` knew about the last trip's value says nothing of this one.
/// A back edge may be left out of what every way in knows only for a value
/// defined above the loop; read as such, `head`'s test of the new column
/// would be skipped, and a run that ends at column 2 would end at 4.
#[test]
fn a_guard_on_a_value_the_loop_remakes_is_not_assumed_on_the_back_edge() {
    let text = "define i32 @f(i32 %row, i32 %n) {
entry:
  br label %head

head:
  %col = phi i32 [ 0, %entry ], [ %next, %latch ]
  %found = phi i32 [ 0, %entry ], [ %found2, %latch ]
  %g = icmp sgt i32 %col, 2
  br i1 %g, label %pre, label %call

pre:
  br label %inner

inner:
  %i = phi i32 [ 0, %pre ], [ %i1, %step ]
  %hit = icmp eq i32 %i, %col
  br i1 %hit, label %latch, label %step

step:
  %i1 = add i32 %i, 1
  %end = icmp eq i32 %i1, %row
  br i1 %end, label %call, label %inner

call:
  %x = add i32 %found, 7
  br label %latch

latch:
  %found2 = phi i32 [ %x, %call ], [ %found, %inner ]
  %next = add i32 %col, 1
  %done = icmp eq i32 %next, %n
  br i1 %done, label %exit, label %head

exit:
  ret i32 %found2
}
";
    let (out, _) = threaded(text);
    assert_eq!(results(&parsed(&out), INPUTS), results(&parsed(text), INPUTS), "{out}");
}

/// A target that asks for one block between `q` and `bb` (the default) keeps
/// the path that ran the whole inner loop through the guard: a longer path
/// moved x86_64 `known_loads_walks` -O2 from 155,300 to 160,530 executed
/// instructions.
#[test]
fn a_target_that_asks_for_one_block_keeps_the_long_path() {
    let before = parsed(ROTATED);
    let mut module = before.clone();
    let mut manager = PassManager::default();
    manager.add(BackwardThreading {
        enabled: true,
        max_threads: MAX_THREADS,
        relations: false,
        early: false,
        chain: Some(1),
    });
    manager.run_module(&mut module, std::rc::Rc::new(Neutral)).unwrap();
    let text = printed(&module);
    assert!(text.contains("br i1 %end, label %call"), "{text}");
}
