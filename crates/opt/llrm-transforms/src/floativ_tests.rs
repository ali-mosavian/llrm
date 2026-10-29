//! `rewritten` over loops the interpreter runs before and after.

use std::collections::HashMap;

use super::rewritten;
use crate::testing::{parsed, printed, results};

/// A loop counting `%x` from `START` by `STEP` while `x.next PRED LIMIT`,
/// summing `READ` (`0` or `%t`, the old counter as an `i32`) into `%s`.
fn looped(
    start: &str,
    step: &str,
    predicate: &str,
    limit: &str,
    read: &str,
) -> String {
    format!(
        "define i32 @f(i32 %n) {{
entry:
  br label %loop

loop:
  %x = phi double [ {start}, %entry ], [ %x.next, %loop ]
  %s = phi i32 [ 0, %entry ], [ %s.next, %loop ]
  %t = fptosi double %x to i32
  %r = add i32 %t, {read}
  %s.next = add i32 %s, %r
  %x.next = fadd double %x, {step}
  %c = fcmp {predicate} double %x.next, {limit}
  br i1 %c, label %loop, label %exit

exit:
  ret i32 %s.next
}}
"
    )
}

fn run(text: &str) -> (String, bool) {
    let mut module = parsed(text);
    let before = module.clone();
    let changed = {
        let (context, function) = module.function_mut("f").expect("@f");
        let loops = llrm_analysis::cfg::loops_of(function);
        rewritten(context, &HashMap::default(), function, &loops)
    };
    let after = printed(&module);
    // An unchanged loop may not end (the equality case runs on), so only a
    // rewrite is run against the original.
    if changed {
        assert_eq!(results(&module, &[&[0]]), results(&before, &[&[0]]), "{after}");
    }
    (after, changed)
}

/// The pin's rewriteNonIntegerIVs: a `double` counter with small whole start,
/// step and limit is an `i32` one, and what read the old one reads a `sitofp`.
#[test]
fn a_double_counter_with_whole_bounds_becomes_an_i32_one() {
    let (after, changed) = run(&looped("0.0", "1.0", "olt", "10.0", "0"));
    assert!(changed, "{after}");
    assert!(after.contains("%x.int = phi i32 [ 0,") && after.contains("icmp slt i32 %x.next.int, 10"), "{after}");
    assert!(!after.contains("fadd") && !after.contains("fcmp"), "{after}");
    assert!(after.contains("%indvar.conv = sitofp i32 %x.int to double"), "{after}");
}

#[test]
fn a_fractional_step_stays() {
    let (after, changed) = run(&looped("0.0", "0.5", "olt", "10.0", "0"));
    assert!(!changed && after.contains("fadd"), "{after}");
}

/// `i != 10` from 0 by 3 skips 10: the integer loop would wrap, the float one
/// would run on.
#[test]
fn an_equality_test_the_step_misses_stays() {
    let (after, changed) = run(&looped("0.0", "3.0", "one", "10.0", "0"));
    assert!(!changed && after.contains("fadd"), "{after}");
}
