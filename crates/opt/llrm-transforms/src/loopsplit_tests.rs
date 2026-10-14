//! Splitting on the interpreter: the answers before and after, and the loops.

use llrm_analysis::cfg;
use llrm_analysis::graph::loops;
use llrm_mir::module::Module;

use super::Split;
use crate::testing::{f, managed, parsed, printed, results};

/// Stores to `%a[i]` 1 below `%k` and 2 from it, up to `%n`.
fn early(test: &str) -> String {
    format!(
        "define i16 @f(i16 %n, i16 %k) {{
b0:
  %a = alloca [64 x i16]
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b4 ]
  %sum = phi i16 [ 0, %b0 ], [ %total, %b4 ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %b2, label %b5

b2:
  %early = icmp {test}
  br i1 %early, label %b3, label %b6

b3:
  br label %b4

b6:
  br label %b4

b4:
  %step = phi i16 [ 1, %b3 ], [ 2, %b6 ]
  %total = add i16 %sum, %step
  %next = add i16 %i, 1
  br label %b1

b5:
  ret i16 %sum
}}
"
    )
}

const INPUTS: &[&[i128]] = &[&[0, 3], &[5, 0], &[5, 3], &[9, 20], &[9, 9], &[7, -2]];

fn loop_count(module: &mut Module) -> usize {
    let function = f(module);
    loops::loops(&cfg::graph(function), function.entry().map(cfg::id)).len()
}

/// `text` through the pass: the module, and whether it split.
fn split(text: &str) -> (Module, bool) {
    let mut module = parsed(text);
    let before = loop_count(&mut module);
    managed(&mut module, Split);
    let split = loop_count(&mut module) == before + 1;
    (module, split)
}

/// A test of the counter against `%k` that holds for the first trips gets a
/// loop of its own: the original loop paid it every trip.
#[test]
fn an_early_test_of_the_counter_splits_the_loop() {
    let text = early("slt i16 %i, %k");
    let (module, split) = split(&text);
    assert!(split, "{}", printed(&module));
    assert_eq!(results(&module, INPUTS), results(&parsed(&text), INPUTS));
    assert!(!printed(&module).contains("%early = icmp slt i16 %i, %k\n  br i1"), "{}", printed(&module));
}

/// Every way of asking: held last, or inclusive with a number, signed or not.
#[test]
fn the_test_may_hold_last_or_include_its_limit() {
    let tests = [
        ("slt", "sge i16 %i, %k"),
        ("slt", "sle i16 %i, 3"),
        ("slt", "sgt i16 %i, 3"),
        ("ult", "ult i16 %i, %k"),
        ("ult", "uge i16 %i, %k"),
        ("ult", "ule i16 %i, 3"),
        ("ult", "ugt i16 %i, 3"),
    ];
    for (loop_test, test) in tests {
        let text = early(test).replace("%more = icmp slt", &format!("%more = icmp {loop_test}"));
        let (module, split) = split(&text);
        assert!(split, "{test}: {}", printed(&module));
        assert_eq!(results(&module, INPUTS), results(&parsed(&text), INPUTS), "{test}");
    }
}

/// The counter on the right of the test is the same test.
#[test]
fn the_counter_may_be_the_right_operand() {
    let text = early("sgt i16 %k, %i");
    let (module, split) = split(&text);
    assert!(split, "{}", printed(&module));
    assert_eq!(results(&module, INPUTS), results(&parsed(&text), INPUTS));
}

/// What does not flip once, or that the loop would not see flip, stays: a
/// value the loop changes, equality, the other signedness, a number whose
/// successor wraps, a counter stepped by two.
#[test]
fn a_test_that_does_not_flip_once_keeps_the_loop() {
    let variants = [
        early("slt i16 %sum, %k"),
        early("eq i16 %i, %k"),
        early("ult i16 %i, %k"),
        early("sle i16 %i, 32767"),
        early("ule i16 %i, 65535"),
        early("slt i16 %i, %k").replace("%next = add i16 %i, 1", "%next = add i16 %i, 2"),
    ];
    for text in variants {
        let (module, split) = split(&text);
        assert!(!split, "{}", printed(&module));
        assert_eq!(results(&module, INPUTS), results(&parsed(&text), INPUTS));
    }
}
