//! Backward threading on the shape of `queens`' column loop: a guard of the
//! inner loop that every trip through the loop already passed.

use crate::backwardthreading::{BackwardThreading, MAX_THREADS};
use llrm_mir::passes::PassManager;
use llrm_mir::target::Neutral;

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
    threaded_on(text, INPUTS)
}

fn threaded_on(
    text: &str,
    inputs: &[&[i128]],
) -> (String, bool) {
    let before = parsed(text);
    let mut module = before.clone();
    let mut manager = PassManager::default();
    manager.add(BackwardThreading { enabled: true, max_threads: MAX_THREADS });
    manager.run_module(&mut module, std::rc::Rc::new(Neutral)).unwrap();
    let text = printed(&module);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{text}");
    let changed = printed(&before) != text;
    (text, changed)
}

#[test]
fn a_join_of_a_known_and_an_unknown_path_is_threaded_for_the_known_one() {
    let (text, changed) = threaded(COLUMNS);
    assert!(changed, "the guard is tested again after the path that passed it:\n{text}");
    // The copy of `head` that the path through `loop` takes jumps to `enter`.
    assert_eq!(text.matches("br label %enter").count(), 1, "{text}");
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

/// The join `p` ends in a switch with two cases to the same block `s`. The copy of `p` has the two edges too, so each
/// phi of `s` needs two inputs from it: one was added, and the verifier (then `switchvalue` in SimplifyCFG, queens
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
    manager.add(BackwardThreading { enabled: true, max_threads: MAX_THREADS });
    manager.run_module(&mut module, std::rc::Rc::new(Neutral)).unwrap();
    let out = printed(&module);
    assert_ne!(out, printed(&before), "the guard was not threaded:\n{out}");
}

/// The Collatz step: the trip after an odd one starts with `3x + 1`, whose
/// low bit is 0, so `x & 1` is decided on that path. The select is the
/// diamond GCC still has when it threads (`threadfull2`). x_collatz -O2 ran
/// 136,825 clk on x86-m32 where gcc 15 ran 121,682.
const COLLATZ: &str = "define i32 @f(i32 %n) {
entry:
  %skip = icmp eq i32 %n, 1
  br i1 %skip, label %exit, label %loop

loop:
  %x = phi i32 [ %n, %entry ], [ %next, %loop ]
  %steps = phi i32 [ 0, %entry ], [ %steps2, %loop ]
  %low = and i32 %x, 1
  %even = icmp eq i32 %low, 0
  %m = mul i32 %x, 3
  %odd = add i32 %m, 1
  %half = lshr i32 %x, 1
  %next = select i1 %even, i32 %half, i32 %odd
  %steps2 = add nuw nsw i32 %steps, 1
  %done = icmp eq i32 %next, 1
  br i1 %done, label %exit, label %loop

exit:
  %r = phi i32 [ 0, %entry ], [ %steps2, %loop ]
  ret i32 %r
}
";

const COLLATZ_INPUTS: &[&[i128]] = &[&[1], &[2], &[3], &[6], &[7], &[9], &[27]];

#[test]
fn the_trip_after_an_odd_one_skips_the_parity_test() {
    let (text, changed) = threaded_on(COLLATZ, COLLATZ_INPUTS);
    assert!(changed, "3x + 1 of an odd x is even, so the next `x & 1` is known:\n{text}");
}

/// The select's condition says nothing of the value the next trip tests: no
/// bit is known on either arm, so nothing is threaded.
#[test]
fn a_select_whose_condition_is_unrelated_to_the_next_test_stays_a_select() {
    let text = COLLATZ.replace("define i32 @f(i32 %n)", "define i32 @f(i32 %n, i32 %k)").replace("%even = icmp eq i32 %low, 0", "%even = icmp eq i32 %k, 0");
    let (out, changed) = threaded_on(&text, &[&[1, 0], &[7, 0], &[8, 0]]);
    assert!(!changed, "{out}");
}
