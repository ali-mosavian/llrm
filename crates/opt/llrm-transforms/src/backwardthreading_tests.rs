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
    let before = parsed(text);
    let mut module = before.clone();
    let mut manager = PassManager::default();
    manager.add(BackwardThreading { enabled: true, max_threads: MAX_THREADS });
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
    manager.add(BackwardThreading { enabled: true, max_threads: MAX_THREADS });
    manager.run_module(&mut module, std::rc::Rc::new(Neutral)).unwrap();
    let inputs: &[&[i128]] = &[&[4], &[6], &[5], &[3], &[8], &[2], &[1]];
    assert_eq!(results(&module, inputs), results(&before, inputs), "{}", printed(&module));
}
