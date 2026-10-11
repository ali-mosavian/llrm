//! Unroll-and-jam on nests the interpreter runs, before and after. The cases
//! are gcc's: bench/particle's repeated sweeps (jammed), `x[j-1] = x[j]`
//! (`gimple-loop-jam.cc` 62-79: not), an inner count that follows the outer
//! counter, and an outer count the factor does not divide.

use llrm_analysis::cfg;
use llrm_analysis::graph::loops;
use llrm_mir::module::Module;
use llrm_mir::opcode::Opcode;

use super::UnrollJam;
use crate::testing::{f, managed, parsed, printed, results};

const INPUTS: &[&[i128]] = &[&[0, 0], &[3, 1], &[-7, 5]];

/// `outer` sweeps of `p[j] = body` over `cells` cells (`bound` the inner
/// loop's test), then a checksum of three cells.
fn nest(
    outer: i16,
    bound: &str,
    body: &str,
) -> String {
    let cells = |base: i32| (0..8).map(|at| format!("i16 {}", base + at)).collect::<Vec<_>>().join(", ");
    format!(
        "@p = global [8 x i16] [{}]
@v = global [8 x i16] [{}]

define i16 @f(i16 %x, i16 %bias) {{
b0:
  br label %b1

b1:
  %t = phi i16 [ 0, %b0 ], [ %t.next, %b4 ]
  %run = icmp slt i16 %t, {outer}
  br i1 %run, label %b2, label %b5

b2:
  br label %b3

b3:
  %j = phi i16 [ 0, %b2 ], [ %j.next, %b3.body ]
  %more = icmp slt i16 %j, {bound}
  br i1 %more, label %b3.body, label %b4

b3.body:
  %o = mul i16 %j, 2
  %pp = getelementptr i8, ptr @p, i16 %o
  %pv = getelementptr i8, ptr @v, i16 %o
{body}  %j.next = add i16 %j, 1
  br label %b3

b4:
  %t.next = add i16 %t, 1
  br label %b1

b5:
  %q = getelementptr i8, ptr @p, i16 4
  %w = load i16, ptr %q
  %q2 = getelementptr i8, ptr @p, i16 8
  %w2 = load i16, ptr %q2
  %u = add i16 %w, %w2
  %z = add i16 %u, %x
  ret i16 %z
}}
",
        cells(1),
        cells(10)
    )
}

/// `p[j] += v[j] + bias`: the sweeps bench/particle makes.
const ACCUMULATE: &str = "  %a = load i16, ptr %pp
  %b = load i16, ptr %pv
  %s = add i16 %a, %b
  %s2 = add i16 %s, %bias
  store i16 %s2, ptr %pp
";

/// gcc's `x[j-1] = x[j]`: each sweep moves the cells down by one. A second
/// array is swept again too, so a jam would pay and only the shift forbids it.
const SHIFT: &str = "  %o1 = add i16 %o, 2
  %pn = getelementptr i8, ptr @p, i16 %o1
  %a = load i16, ptr %pn
  store i16 %a, ptr %pp
  %c = load i16, ptr %pv
  %c1 = add i16 %c, 1
  store i16 %c1, ptr %pv
";

fn loads(module: &mut Module) -> usize {
    let function = f(module);
    function.walk().filter(|&(_, inst)| matches!(function.instruction(inst).opcode, Opcode::Load { .. })).count()
}

fn loop_count(module: &mut Module) -> usize {
    let function = f(module);
    loops::loops(&cfg::graph(function), function.entry().map(cfg::id)).len()
}

/// `text` through the pass: whether it changed, and it computes what it did.
fn through(text: &str) -> (bool, Module) {
    let mut module = parsed(text);
    let before = (printed(&module), results(&module, INPUTS));
    let after = managed(&mut module, UnrollJam::default());
    assert_eq!(results(&module, INPUTS), before.1, "{after}");
    (after != before.0, module)
}

/// Six sweeps of `p[j] += v[j] + bias` are three, each making two passes over
/// the cells in one: the loads of both passes are in the one inner loop, and
/// the outer loop is still a loop around it.
#[test]
fn sweeps_of_the_same_cells_are_made_two_at_a_time() {
    let text = nest(6, "8", ACCUMULATE);
    let (changed, mut module) = through(&text);
    assert!(changed);
    assert_eq!(loop_count(&mut module), 2);
    assert_eq!(loads(&mut module), 4 + 2, "{}", printed(&module));
}

/// A nest the pass could jam is left alone below -O3 by being left out of the
/// pipeline, not by the pass; this is the pass refusing what gcc's own comment
/// says would be wrong. Each sweep of `x[j] = x[j+1]` moves the cells by one,
/// and a jam by two would move them by one in all: the checksum would differ,
/// which `through` asserts.
#[test]
fn a_sweep_that_shifts_the_cells_is_not_jammed() {
    let text = nest(6, "7", SHIFT);
    let (changed, mut module) = through(&text);
    assert!(!changed, "{}", printed(&module));
    assert_eq!(loads(&mut module), 2 + 2);
}

/// An inner loop whose count follows the outer counter is a different loop in
/// each outer iteration: its copies cannot be fused.
#[test]
fn an_inner_count_that_follows_the_outer_counter_is_not_jammed() {
    let text = nest(6, "%t", ACCUMULATE)
        .replace("%more = icmp slt i16 %j, %t", "%tc = add i16 %t, 2\n  %more = icmp slt i16 %j, %tc");
    let (changed, _) = through(&text);
    assert!(!changed);
}

/// Five sweeps in groups of two leave one over, which this does not run.
#[test]
fn an_outer_count_the_factor_does_not_divide_is_not_jammed() {
    let (changed, _) = through(&nest(5, "8", ACCUMULATE));
    assert!(!changed);
}

/// Sweeps over distinct arrays: nothing is read again, so no reference goes
/// away and the nest is left as it is.
#[test]
fn a_nest_that_reads_nothing_twice_is_not_jammed() {
    let text = nest(6, "8", "  %b = load i16, ptr %pv\n  store i16 %b, ptr %pp\n");
    let (changed, _) = through(&text);
    assert!(!changed);
}
