//! `shared` over loops the interpreter runs before and after, over trip
//! counts 0, 1 and more.

use llrm_analysis::testing::DOS;
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};

use super::shared;
use crate::testing::{managed, parsed, results};

struct Shared;

impl FunctionPass for Shared {
    fn name(&self) -> &'static str {
        "ivshare"
    }

    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        if shared(unit, &outer) { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// Counters `%a` and `%b` from `a` and `b`, stepping by `step_a` and
/// `step_b` while `%a < n`; it returns their sum's total.
fn counters(a: &str, b: &str, step_a: i64, step_b: i64) -> String {
    format!(
        "define i16 @f(i16 %n, i16 %x) {{
b0:
  %xb = add i16 %x, 4
  br label %b1

b1:
  %a = phi i16 [ {a}, %b0 ], [ %a.next, %b2 ]
  %b = phi i16 [ {b}, %b0 ], [ %b.next, %b2 ]
  %s = phi i16 [ 0, %b0 ], [ %s.next, %b2 ]
  %more = icmp slt i16 %a, %n
  br i1 %more, label %b2, label %b3

b2:
  %t = mul i16 %b, 3
  %u = add i16 %a, %t
  %s.next = add i16 %s, %u
  %a.next = add i16 %a, {step_a}
  %b.next = add i16 %b, {step_b}
  br label %b1

b3:
  %r = add i16 %s, %b
  ret i16 %r
}}
"
    )
}

const TRIPS: &[&[i128]] = &[&[0, 5], &[1, 5], &[2, -3], &[13, 100]];

/// `text` through `shared`, printed, where it computes what it did; and
/// how many header phis it keeps.
fn run(text: &str) -> (String, usize) {
    let before = parsed(&format!("{DOS}{text}"));
    let mut after = before.clone();
    let printed = managed(&mut after, Shared);
    assert_eq!(results(&after, TRIPS), results(&before, TRIPS), "{printed}");
    let phis = printed.matches(" = phi ").count();
    (printed, phis)
}

#[test]
fn test_twin_counters_become_one() {
    let (after, phis) = run(&counters("0", "0", 1, 1));
    assert_eq!(phis, 2, "{after}");
    assert!(after.contains("%t = mul i16 %a, 3"), "{after}");
}

#[test]
fn test_a_counter_a_constant_ahead_is_the_other_plus_the_distance() {
    let (after, phis) = run(&counters("0", "4", 2, 2));
    assert_eq!(phis, 2, "{after}");
    assert!(after.contains("b1:\n  %a = phi i16 [ 0, %b0 ], [ %a.next, %b2 ]\n  %s = phi i16 [ 0, %b0 ], [ %s.next, %b2 ]\n  %0 = add i16 %a, 4\n"), "{after}");
}

/// `%x + 4` is 4 past `%x`, whatever `%x` is.
#[test]
fn test_symbolic_starts_a_constant_apart_share() {
    let (after, phis) = run(&counters("%x", "%xb", 1, 1));
    assert_eq!(phis, 2, "{after}");
    assert!(after.contains("%0 = add i16 %a, 4"), "{after}");
}

#[test]
fn test_counters_stepping_differently_stay_apart() {
    let (_, phis) = run(&counters("0", "0", 1, 2));
    assert_eq!(phis, 3);
}

#[test]
fn test_starts_no_constant_apart_stay_apart() {
    let (_, phis) = run(&counters("0", "%x", 1, 1));
    assert_eq!(phis, 3);
}
