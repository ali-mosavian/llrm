//! `compute_peel_count` through the full unroller, on loops the interpreter
//! runs, before and after.

use llrm_mir::module::Module;
use llrm_x86::tti::X86;

use crate::loopunrollpass::{Options, unroll_function};
use crate::testing::{parsed, printed, results};

/// @f's loops after the unroller; whether it changed any (priced for an i486).
fn unrolled(module: &mut Module) -> bool {
    let function = module.function_mut("f").expect("@f").1;
    crate::loopsimplify::simplified(function);
    crate::lcssa::closed(function).expect("closes");
    let tti = X86::of_cpu("i486").expect("a cpu");
    let options =
        Options { opt_level: 2, opt_size: false, min_size: false, only_when_forced: false, prepare_for_lto: false };
    unroll_function(module, "f", &tti, options).expect("unrolls")
}

const INPUTS: &[&[i128]] = &[&[1, 1], &[1, 2], &[1, 5], &[1, 9], &[3, 7]];

/// `for (i = LOW; i <= n; i++) { total += i; if (i > BOUND) total += 100; }`:
/// the test is false on the first trips and true after.
fn guarded(
    low: u32,
    bound: u32,
) -> String {
    format!(
        "define i32 @f(i32 %n, i32 %seed) {{
b0:
  br label %head

head:
  %i = phi i32 [ {low}, %b0 ], [ %next, %latch ]
  %t = phi i32 [ %seed, %b0 ], [ %u, %latch ]
  %add = add i32 %t, %i
  %big = icmp sgt i32 %i, {bound}
  br i1 %big, label %hot, label %latch

hot:
  %more = add i32 %add, 100
  br label %latch

latch:
  %u = phi i32 [ %add, %head ], [ %more, %hot ]
  %next = add nsw i32 %i, 1
  %go = icmp sle i32 %next, %n
  br i1 %go, label %head, label %out

out:
  %r = phi i32 [ %u, %latch ]
  ret i32 %r
}}
"
    )
}

/// The compares of `%i` against a bound the function still makes.
fn compares(text: &str) -> usize {
    text.matches("icmp sgt i32 %").count()
}

/// bintree's `for (i = 1; ...) { ...; if (i > 1) insert(i); }` tested `i > 1`
/// on every trip, the first of which is the only one it is false on: the pin
/// peels that trip and decides the test in both (LoopPeel). It was kept.
#[test]
fn a_compare_false_on_the_first_trip_only_is_decided_by_peeling_one_trip() {
    let mut module = parsed(&guarded(1, 1));
    let before = results(&module, INPUTS);
    assert!(unrolled(&mut module));
    let text = printed(&module);
    assert_eq!(results(&module, INPUTS), before, "{text}");
    assert_eq!(compares(&text), 0, "the test is still in a loop: {text}");
}

/// The copies share the loop's exits, so the residual loop's exit had
/// predecessors outside it: the loop was not in simplified form, and the
/// unroller refused it on its next visit ("not in loop-simplify form"). The
/// pin's `peelLoop` ends in `simplifyLoop`.
#[test]
fn the_residual_loop_after_a_peel_is_in_simplified_form() {
    let mut module = parsed(&guarded(1, 1));
    assert!(unrolled(&mut module));
    let function = module.function_mut("f").expect("@f").1;
    assert!(!crate::loopsimplify::simplified(function), "{}", printed(&module));
}

/// Peeling stops at `unroll-peel-max-count` (7): ten trips would decide the
/// test and are not peeled.
#[test]
fn a_compare_decided_only_after_more_than_seven_trips_is_not_peeled() {
    let mut module = parsed(&guarded(1, 10));
    let before = results(&module, INPUTS);
    unrolled(&mut module);
    let text = printed(&module);
    assert_eq!(results(&module, INPUTS), before, "{text}");
    assert_eq!(compares(&text), 1, "{text}");
}

/// A phi that takes the same value after the first trip, `a = phi [0, 5]`, is
/// a constant in the loop once one trip is peeled (the pin's `PhiAnalyzer`).
#[test]
fn a_phi_that_becomes_invariant_after_a_trip_is_removed_by_peeling_it() {
    let text = "define i32 @f(i32 %n, i32 %seed) {
b0:
  br label %head

head:
  %i = phi i32 [ 0, %b0 ], [ %next, %head ]
  %a = phi i32 [ 0, %b0 ], [ 5, %head ]
  %t = phi i32 [ %seed, %b0 ], [ %u, %head ]
  %u = add i32 %t, %a
  %next = add i32 %i, 1
  %go = icmp slt i32 %next, %n
  br i1 %go, label %head, label %out

out:
  %r = phi i32 [ %u, %head ]
  ret i32 %r
}
";
    let mut module = parsed(text);
    let before = results(&module, INPUTS);
    assert!(unrolled(&mut module));
    let after = printed(&module);
    assert_eq!(results(&module, INPUTS), before, "{after}");
    assert!(!after.contains("%a = phi i32 [ 0, %b0 ], [ 5"), "{after}");
}

/// An invariant load in a loop that also leaves through an `unreachable`
/// exit (a bounds check) is not known dereferenceable until a trip has run:
/// the pin peels one trip so the rest can hoist it. nib's `Level.leaf` was
/// peeled by `opt` and kept by llrm.
#[test]
fn an_invariant_load_that_feeds_the_exit_is_peeled_one_trip() {
    let text = "define i32 @f(ptr %p, i32 %n) {
b0:
  br label %head

head:
  %i = phi i32 [ 0, %b0 ], [ %next, %body ]
  %in = icmp slt i32 %i, %n
  br i1 %in, label %body, label %bad

body:
  %v = load i32, ptr %p
  %next = add i32 %i, 1
  %same = icmp eq i32 %v, %next
  br i1 %same, label %out, label %head

bad:
  unreachable

out:
  %r = phi i32 [ %v, %body ]
  ret i32 %r
}
";
    let mut module = parsed(text);
    assert!(unrolled(&mut module));
    let after = printed(&module);
    assert_eq!(after.matches("load i32, ptr %p").count(), 2, "{after}");
}

/// `opt` runs `simplifyLoopIVs` after a peel, which proves the steps `nuw nsw`
/// and the exit test `samesign`; the residual loop's test of the peeled phi
/// against the bound then folds. llrm left it (demo-deedlines `RGBLIGHTS`).
#[test]
fn the_residual_loop_after_a_peel_has_its_compare_of_the_peeled_phi_decided() {
    // The bound is constant, so SCEV proves the step cannot wrap and the flags
    // are its to write.
    let text = guarded(0, 5)
        .replace("add nsw i32 %i, 1", "add i32 %i, 1")
        .replace("icmp sle i32 %next, %n", "icmp sle i32 %next, 1792");
    let mut module = parsed(&text);
    let before = results(&module, INPUTS);
    assert!(unrolled(&mut module));
    let after = printed(&module);
    assert_eq!(results(&module, INPUTS), before, "{after}");
    assert_eq!(compares(&after), 0, "the peeled trips fold theirs; the loop's is decided too: {after}");
}
