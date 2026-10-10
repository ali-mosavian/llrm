//! llrm-core's `rotate_tests.rs` (tests/test_countdown.py and
//! tests/test_rotate.py), each body now MIR text run by the interpreter
//! before and after.
//!
//! Skipped, reading BC fixtures through the old emission:
//! `test_entering_mains_first_loop_at_its_body_keeps_the_code_after_it`
//! (qbdemo-fil2, ignored there too) and
//! `test_a_back_edge_keeps_its_copies_in_the_latch` (harr, a layout question
//! for the backend).

use llrm_analysis::cfg;
use llrm_analysis::graph::loops::{self, Loop};
use llrm_mir::module::{Function, Module, Operand};

use super::Rotate;
use crate::peel::Peel;
use crate::testing::{f, managed, parsed, printed, results};
use crate::unroll::Unroll;

const INPUTS: &[&[i128]] = &[&[3, 0], &[3, 1], &[2, 7], &[-4, 300], &[5, 9]];

/// `%acc` summed with `%i` for `bound` trips; the exit reads both.
fn summing(bound: &str) -> String {
    format!(
        "define i16 @f(i16 %x, i16 %n) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %acc = phi i16 [ %x, %b0 ], [ %sum, %b2 ]
  %go = icmp slt i16 %i, {bound}
  br i1 %go, label %b2, label %b3

b2:
  %sum = add i16 %acc, %i
  %next = add i16 %i, 1
  br label %b1

b3:
  %r = add i16 %acc, %i
  ret i16 %r
}}
"
    )
}

fn only_loop(function: &Function) -> Loop {
    let found = loops::loops(&cfg::graph(function), function.entry().map(cfg::id));
    let [one] = &found[..] else { panic!("one loop, not {found:?}") };
    one.clone()
}

/// `text` through `pass`: whether it changed, and it computes what it did.
fn through(
    text: &str,
    pass: impl llrm_mir::passes::FunctionPass + 'static,
) -> (bool, Module) {
    let mut module = parsed(text);
    let before = (printed(&module), results(&module, INPUTS));
    let after = managed(&mut module, pass);
    assert_eq!(results(&module, INPUTS), before.1, "{after}");
    (after != before.0, module)
}

/// Four trips, or one, are entered at the body: the loop is one block
/// testing at its bottom, entered by a plain jump. None and `%n` trips
/// keep their first test.
#[test]
fn a_proven_loop_is_entered_at_its_body() {
    for (bound, entered) in [("4", true), ("1", true), ("0", false), ("%n", false)] {
        let (changed, mut module) = through(&summing(bound), Rotate { proven: true, copy: false });
        assert_eq!(changed, entered, "{bound}");
        let function = f(&mut module);
        let loop_ = only_loop(function);
        assert_eq!(loop_.body.len(), if entered { 1 } else { 2 }, "{bound}");
        if entered {
            let entry = function.entry().unwrap();
            let jump = function.instruction(function.terminator(entry).unwrap());
            assert_eq!(jump.operands, [Operand::Block(cfg::block(loop_.header))], "{bound}");
        }
    }
}

/// A header phi reading another's value, as `a, b = b, a + b`, reads the
/// one in the body.
#[test]
fn a_header_phi_may_read_another() {
    let text = "define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %a = phi i16 [ %x, %b0 ], [ %b, %b2 ]
  %b = phi i16 [ %n, %b0 ], [ %sum, %b2 ]
  %go = icmp ult i16 %i, 5
  br i1 %go, label %b2, label %b3

b2:
  %sum = add i16 %a, %b
  %next = add i16 %i, 1
  br label %b1

b3:
  %shifted = shl i16 %a, 4
  %r = xor i16 %shifted, %b
  ret i16 %r
}
";
    assert!(through(text, Rotate { proven: true, copy: false }).0);
}

/// Header phis read in parallel: `a, b = b, a` rotates, each moved phi
/// reading the other's moved one, over odd and even trips. It was refused.
#[test]
fn header_phis_that_swap_rotate() {
    let text = "define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %a = phi i16 [ %x, %b0 ], [ %b, %b2 ]
  %b = phi i16 [ %n, %b0 ], [ %a, %b2 ]
  %go = icmp ult i16 %i, TRIPS
  br i1 %go, label %b2, label %b3

b2:
  %next = add i16 %i, 1
  br label %b1

b3:
  %shifted = shl i16 %a, 4
  %r = xor i16 %shifted, %b
  ret i16 %r
}
";
    for trips in ["1", "2", "5"] {
        let (changed, mut module) = through(&text.replace("TRIPS", trips), Rotate { proven: true, copy: false });
        assert!(changed, "{trips}");
        assert_eq!(only_loop(f(&mut module)).body.len(), 1, "{trips}");
    }
}

/// The header's work runs once fewer: a store there is not skipped.
#[test]
fn a_header_that_stores_keeps_its_first_test() {
    let text = format!(
        "@g = global i16 0\n\n{}",
        summing("4").replace("  %go = icmp", "  store i16 %i, ptr @g\n  %go = icmp")
    );
    assert!(!through(&text, Rotate { proven: true, copy: false }).0);
}

/// A rotated loop is one block testing at its bottom, which unroll and
/// peel do not take: they run before rotation.
#[test]
fn a_rotated_loop_is_not_unrolled_or_peeled() {
    let mut module = parsed(&summing("4"));
    managed(&mut module, Rotate { proven: true, copy: false });
    let rotated = printed(&module);
    assert!(!through(&rotated, Unroll::default()).0);
    assert!(!through(&rotated, Peel::default()).0);
    assert!(through(&summing("4"), Unroll::default()).0);
}

/// A step made in the header that a header phi takes round the back edge is not
/// skipped on entry: rotation read it as test-only and the counter's next value
/// was its own (`sub %x, 1` of itself, #811).
#[test]
fn a_step_in_the_header_that_a_phi_reads_keeps_the_header() {
    let text = "define i16 @f(i16 %x) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 4, %b0 ], [ %next, %b2 ]
  %next = sub i16 %i, 1
  %go = icmp ne i16 %i, 0
  br i1 %go, label %b2, label %b3

b2:
  br label %b1

b3:
  ret i16 %i
}
";
    assert!(!through(text, Rotate { proven: true, copy: false }).0);
}

/// The guard a header copy leaves before the loop was an unweighted branch like
/// any other, so the way into the loop was laid out as the unlikely one (the
/// copy of a `while (at >= gap && ...)` in shellsort -O1: 17,720 clocks against
/// 15,658 without the copy). It takes the loop test's weights as gcc's header
/// copy takes its probabilities and LLVM's `LoopRotate` its weights.
#[test]
fn test_a_copied_header_guard_enters_the_loop_as_the_loop_stays_in() {
    use std::collections::BTreeMap;

    use llrm_analysis::branchprob::{self, Heuristic};
    use llrm_analysis::cfg::Shape;
    let (changed, module) = through(&summing("%n"), Rotate { proven: false, copy: true });
    assert!(changed);
    let function = llrm_analysis::testing::function(&module, "f");
    let entry = cfg::id(function.entry().unwrap());
    let odds = branchprob::estimated(
        &module.context,
        &module.metadata,
        &module.globals,
        function,
        &Shape::of(function),
        &BTreeMap::new(),
    );
    assert_eq!(odds.by.get(&entry), Some(&Heuristic::Declared));
    let into = function
        .successors(function.entry().unwrap())
        .into_iter()
        .map(cfg::id)
        .find(
            |at| {
                // The way in is the successor that is in the loop.
                only_loop(function).body.contains(at)
            },
        );
    let into = into.expect("a way into the loop");
    let got = odds.probability(entry, into).unwrap();
    assert!((got - 124.0 / 128.0).abs() < 1e-9, "{got}");
}
