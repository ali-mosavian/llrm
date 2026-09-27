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
use llrm_mir::opcode::{BinaryOp, IntPredicate, Opcode};

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

/// `%acc` tripled `%n` times, the counter read by nothing else, and the
/// exit reading it through `leave` (`""` or an LCSSA phi).
fn dead_counter(test: &str, leave: &str, result: &str) -> String {
    format!(
        "define i16 @f(i16 %x, i16 %n) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %acc = phi i16 [ %x, %b0 ], [ %sum, %b2 ]
  %go = icmp {test} i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  %sum = mul i16 %acc, 3
  %next = add i16 %i, 1
  br label %b1

b3:
{leave}  ret i16 {result}
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
fn through(text: &str, pass: impl llrm_mir::passes::FunctionPass + 'static) -> (bool, Module) {
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
        let (changed, mut module) = through(&summing(bound), Rotate);
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
    assert!(through(text, Rotate).0);
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
        let (changed, mut module) = through(&text.replace("TRIPS", trips), Rotate);
        assert!(changed, "{trips}");
        assert_eq!(only_loop(f(&mut module)).body.len(), 1, "{trips}");
    }
}

/// The header's work runs once fewer: a store there is not skipped.
#[test]
fn a_header_that_stores_keeps_its_first_test() {
    let text = format!("@g = global i16 0\n\n{}", summing("4").replace("  %go = icmp", "  store i16 %i, ptr @g\n  %go = icmp"));
    assert!(!through(&text, Rotate).0);
}

/// C floats retained `add/cmp/jb` in its ten-trip hot path.
///
/// Clang tests the dynamic count once before the loop, then ends every
/// trip with `dec/jne`. The entry test is essential: `bench_floats(0)` must
/// still run the body no times, so the guard is asserted, not only the
/// decrement.
#[test]
fn test_dead_dynamic_counter_counts_down_after_a_zero_trip_guard() {
    for test in ["slt", "ult", "ne"] {
        let (changed, mut module) = through(&dead_counter(test, "", "%acc"), Rotate);
        assert!(changed, "{test}");
        let function = f(&mut module);
        let loop_ = only_loop(function);
        assert_eq!(loop_.body.len(), 1, "{test}");
        let block = cfg::block(loop_.header);
        let ops = function.block(block).instructions().iter().map(|&inst| function.instruction(inst)).collect::<Vec<_>>();
        let decrement = ops.iter().find(|op| op.opcode == Opcode::Binary(BinaryOp::Sub)).expect("a decrement");
        let tested = ops.iter().find(|op| op.opcode == Opcode::ICmp(IntPredicate::Ne)).expect("a test for zero");
        assert_eq!(tested.operands[0], Operand::Value(decrement.result.unwrap()), "{test}");
        let entry = function.entry().unwrap();
        let guard = function.instruction(function.terminator(entry).unwrap());
        assert_eq!(guard.operands.len(), 3, "{test}: a guard");
        assert!(guard.operands[1..].iter().any(|one| *one != Operand::Block(block)), "{test}");
    }
}

/// The counter leaving through an LCSSA phi leaves as its exit value, or
/// its start where the guard skipped the loop.
#[test]
fn a_counted_down_counter_still_leaves_with_its_value() {
    assert!(through(&dead_counter("slt", "  %e = phi i16 [ %i, %b1 ]\n  %r = add i16 %e, %acc\n", "%r"), Rotate).0);
}

/// Replacing an index stored by the body with trips remaining changes the program.
#[test]
fn test_countdown_refuses_an_observed_source_counter() {
    let text = format!("@g = global i16 0\n\n{}", dead_counter("slt", "", "%acc").replace("  %next = add", "  store i16 %i, ptr @g\n  %next = add"));
    assert!(!through(&text, Rotate).0);
}

/// A rotated loop is one block testing at its bottom, which unroll and
/// peel do not take: they run before rotation.
#[test]
fn a_rotated_loop_is_not_unrolled_or_peeled() {
    let mut module = parsed(&summing("4"));
    managed(&mut module, Rotate);
    let rotated = printed(&module);
    assert!(!through(&rotated, Unroll::default()).0);
    assert!(!through(&rotated, Peel::default()).0);
    assert!(through(&summing("4"), Unroll::default()).0);
}
