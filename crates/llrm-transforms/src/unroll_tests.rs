//! Unroll on loops the interpreter runs, before and after. llrm-core's
//! tests stay behind: FPDEEP and FPCSE are BC fixtures through the whole
//! pipeline, and the dead-store test is DSE's, of x87 checkpoints.

use llrm_analysis::cfg;
use llrm_analysis::peelsize::Limits;
use llrm_graph::loops;
use llrm_mir::module::Module;
use llrm_mir::opcode::{BinaryOp, Opcode};

use super::Unroll;
use crate::testing::{f, managed, parsed, printed, results};

/// `%acc * %n + %i` for `bound` trips, across a bridge to the latch; the
/// exit reads the header's `%i` and `%acc`.
fn summing(bound: &str, body: &str) -> String {
    format!(
        "@count = global i16 0

define void @tick() {{
b0:
  %c = load i16, ptr @count
  %d = add i16 %c, 1
  store i16 %d, ptr @count
  ret void
}}

define i16 @f(i16 %x, i16 %n) {{
b0:
  store i16 0, ptr @count
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b3 ]
  %acc = phi i16 [ %x, %b0 ], [ %s, %b3 ]
  %go = icmp slt i16 %i, {bound}
  br i1 %go, label %b2, label %b4

b2:
  %m = mul i16 %acc, %n
{body}  br label %b3

b3:
  %s = add i16 %m, %i
  %next = add i16 %i, 1
  br label %b1

b4:
  %r = phi i16 [ %acc, %b1 ]
  %k = add i16 %i, %r
  %t = load i16, ptr @count
  %u = add i16 %k, %t
  ret i16 %u
}}
"
    )
}

const INPUTS: &[&[i128]] = &[&[0, 3], &[1, 3], &[7, 0], &[-5, 9], &[3, 5]];

fn loops_of(module: &mut Module) -> usize {
    let function = f(module);
    loops::loops(&cfg::graph(function), function.entry().map(cfg::id)).len()
}

fn multiplies(module: &mut Module) -> usize {
    let function = f(module);
    function.walk().filter(|&(_, inst)| function.instruction(inst).opcode == Opcode::Binary(BinaryOp::Mul)).count()
}

/// `text` through `pass`: whether it changed, and it computes what it did.
fn through(text: &str, pass: Unroll) -> (bool, Module) {
    let mut module = parsed(text);
    let before = (printed(&module), results(&module, INPUTS));
    let after = managed(&mut module, pass);
    assert_eq!(results(&module, INPUTS), before.1, "{after}");
    (after != before.0, module)
}

/// One multiply a trip in a straight line and no loop left, where the
/// count is four or five; none, one and a thousand trips stay rolled.
#[test]
fn every_trip_is_copied_and_the_loop_is_gone() {
    for (trips, unrolled) in [(0, false), (1, false), (4, true), (5, true), (1000, false)] {
        let (changed, mut module) = through(&summing(&trips.to_string(), ""), Unroll::default());
        assert_eq!(changed, unrolled, "{trips}");
        assert_eq!(loops_of(&mut module), usize::from(!unrolled), "{trips}");
        assert_eq!(multiplies(&mut module), if unrolled { trips } else { 1 }, "{trips}");
    }
}

/// Two header phis swapping each trip read each other's value from before
/// the trip, not after.
#[test]
fn header_phis_are_carried_together() {
    let text = "define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %a = phi i16 [ %x, %b0 ], [ %b, %b2 ]
  %b = phi i16 [ %n, %b0 ], [ %a2, %b2 ]
  %go = icmp ult i16 %i, 3
  br i1 %go, label %b2, label %b3

b2:
  %a2 = mul i16 %a, 2
  %next = add i16 %i, 1
  br label %b1

b3:
  %shifted = shl i16 %a, 8
  %r = add i16 %shifted, %b
  ret i16 %r
}
";
    let (changed, mut module) = through(text, Unroll::default());
    assert!(changed);
    assert_eq!(loops_of(&mut module), 0);
}

#[test]
fn unknown_trips_have_no_runtime_remainder_and_stay_rolled() {
    assert!(!through(&summing("%n", ""), Unroll::default()).0);
}

#[test]
fn a_copy_over_budget_stays_rolled() {
    let tight = Unroll { limits: Limits { max_unrolled_operations: 1, ..Limits::default() }, ..Unroll::default() };
    assert!(!through(&summing("8", ""), tight).0);
    assert!(through(&summing("8", ""), Unroll::default()).0);
}

/// @tick counts every trip, and the loop keeps it once a trip.
#[test]
fn a_loop_with_a_call_stays_rolled() {
    assert!(!through(&summing("4", "  call void @tick()\n"), Unroll::default()).0);
}

/// A branch inside the body is peel's to copy, not a straight line's.
#[test]
fn a_body_that_branches_stays_rolled() {
    let text = summing("4", "").replace(
        "  %m = mul i16 %acc, %n\n  br label %b3\n",
        "  %m = mul i16 %acc, %n\n  %z = icmp eq i16 %m, 0\n  br i1 %z, label %b3, label %b5\n\nb5:\n  br label %b3\n",
    );
    assert!(!through(&text, Unroll::default()).0);
}

/// The header's work runs once more than the body, so it may not write.
#[test]
fn a_header_that_stores_stays_rolled() {
    let text = summing("4", "").replace("  %go = icmp", "  store i16 %i, ptr @count\n  %go = icmp");
    assert!(!through(&text, Unroll::default()).0);
}
