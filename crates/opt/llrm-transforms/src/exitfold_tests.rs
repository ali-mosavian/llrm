//! Exits their counts decide, over loops the interpreter runs before and after.

use llrm_analysis::testing::DOS;
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};

use crate::testing::{managed, parsed, results};

/// `exitfold::folded` as a pass of its own.
struct Folded;

impl FunctionPass for Folded {
    fn name(&self) -> &'static str {
        "exitfold"
    }

    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        if super::folded(unit.context, unit.layout, unit.function, &outer) { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// `text` folded, its results on `inputs` checked against the original's.
fn folded(text: &str, inputs: &[&[i128]]) -> String {
    let before = parsed(&format!("{DOS}{text}"));
    let mut after = before.clone();
    let printed = managed(&mut after, Folded);
    assert_eq!(results(&after, inputs), results(&before, inputs), "{printed}");
    printed
}

/// `i` below `n`, then below `bound`, a count each: `i` from 0 by one.
fn checked(bound: &str) -> String {
    format!(
        "define i16 @f(i16 %n, i16 %len) {{
entry:
  %fits = icmp ult i16 %n, %len
  br i1 %fits, label %head, label %done
head:
  %i = phi i16 [ 0, %entry ], [ %j, %body ]
  %s = phi i16 [ 0, %entry ], [ %t, %body ]
  %more = icmp ult i16 %i, %n
  br i1 %more, label %check, label %done
check:
  %inside = icmp ult i16 %i, {bound}
  br i1 %inside, label %body, label %done
body:
  %t = add i16 %s, %i
  %j = add nuw i16 %i, 1
  br label %head
done:
  %r = phi i16 [ 0, %entry ], [ %s, %head ], [ 99, %check ]
  ret i16 %r
}}
"
    )
}

const INPUTS: &[&[i128]] = &[&[0, 5], &[3, 5], &[5, 9], &[4, 4], &[9, 3]];

/// The check below `len` never leaves where entry proved `n < len`: the
/// header's test leaves on that trip first.
#[test]
fn test_an_exit_the_guard_proves_later_never_leaves() {
    let printed = folded(&checked("%len"), INPUTS);
    assert!(printed.contains("br i1 true, label %body"), "{printed}");
}

/// A check with the header's own count never leaves: the header leaves
/// on that trip before it runs.
#[test]
fn test_an_exit_sharing_an_earlier_count_never_leaves() {
    let printed = folded(&checked("%n"), INPUTS);
    assert!(printed.contains("br i1 true, label %body"), "{printed}");
}

/// Constants decide it without a guard: `i < 7` after `i < 3` never leaves.
#[test]
fn test_an_exit_whose_constant_count_is_higher_never_leaves() {
    let text = checked("7").replace("icmp ult i16 %i, %n", "icmp ult i16 %i, 3");
    let printed = folded(&text, INPUTS);
    assert!(printed.contains("br i1 true, label %body"), "{printed}");
}

/// A test that fails at once leaves on the first trip.
#[test]
fn test_an_exit_counted_zero_leaves_at_once() {
    let text = checked("0").replace("icmp ult i16 %i, %n", "icmp ult i16 %i, 3");
    let printed = folded(&text, INPUTS);
    assert!(printed.contains("br i1 false, label %body"), "{printed}");
}

/// Nothing proves which of two lengths is shorter: both stay.
#[test]
fn test_exits_nothing_orders_keep_their_tests() {
    let text = checked("%len").replace("  %fits = icmp ult i16 %n, %len\n  br i1 %fits, label %head, label %done", "  %fits = icmp ult i16 %n, 1000\n  br i1 %fits, label %head, label %done");
    let printed = folded(&text, INPUTS);
    assert!(printed.contains("br i1 %inside"), "{printed}");
}

/// `i` below `n`, then at most `len` as signed numbers, where entry proved
/// `0 <= n <= len`. An inclusive signed test counts nothing: `len` may be
/// the largest number.
const UNCOUNTED: &str = "define i16 @f(i16 %n, i16 %len) {
entry:
  %fits = icmp sle i16 %n, %len
  %whole = icmp sge i16 %n, 0
  %both = and i1 %fits, %whole
  br i1 %both, label %head, label %done
head:
  %i = phi i16 [ 0, %entry ], [ %j, %body ]
  %s = phi i16 [ 0, %entry ], [ %t, %body ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %check, label %done
check:
  %inside = icmp sle i16 %i, %len
  br i1 %inside, label %body, label %done
body:
  %t = add i16 %s, %i
  %j = add i16 %i, 1
  br label %head
done:
  %r = phi i16 [ 0, %entry ], [ %s, %head ], [ 99, %check ]
  ret i16 %r
}
";

/// An exit no count decides is tested once, on the counter's start, where
/// the guards prove its test holds up to the loop's most trips: `i <= len`
/// on every trip below `n <= len` once `0 <= len`.
#[test]
fn test_an_uncounted_exit_is_tested_once_on_its_start() {
    let printed = folded(UNCOUNTED, &[&[0, 5], &[3, 5], &[5, 5], &[4, 9], &[-2, 3], &[9, 3]]);
    let check = printed.split("check:").nth(1).expect("the check block").split("\n\n").next().unwrap_or_default().to_owned();
    assert!(!check.contains("%i"), "{printed}");
}
