//! `sunk` over loops the interpreter runs before and after, over trip counts 0,
//! 1 and more.

use std::collections::HashMap;

use super::sunk as sunk_with;
use crate::testing::{parsed, printed, results};

/// A loop whose preheader computes `%k = KIND` and whose body reads `USE`.
fn looped(
    kind: &str,
    use_in_loop: &str,
) -> String {
    format!(
        "@g = global i32 0
define i32 @f(i32 %n, i32 %a) {{
entry:
  %k = {kind}
  br label %loop

loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %s = add i32 %i, {use_in_loop}
  %next = add i32 %i, 1
  %c = icmp ult i32 %next, %n
  br i1 %c, label %loop, label %exit

exit:
  %r = add i32 %s, %k
  ret i32 %r
}}
"
    )
}

fn run(text: &str) -> (String, bool) {
    let mut module = parsed(text);
    let before = module.clone();
    let loops;
    let changed = {
        let (context, function) = module.function_mut("f").expect("@f");
        loops = llrm_analysis::cfg::loops_of(function);
        sunk_with(context, &HashMap::default(), function, &loops)
    };
    (printed(&module), changed)
        .tap(|(text, _)| assert_eq!(results(&module, &[&[3, 5]]), results(&before, &[&[3, 5]]), "{text}"))
}

trait Tap: Sized {
    fn tap(
        self,
        check: impl FnOnce(&Self),
    ) -> Self {
        check(&self);
        self
    }
}

impl<T> Tap for T {}

/// The pin's sinkUnusedInvariants: a preheader value only the exit reads left
/// its register held through the loop.
#[test]
fn a_value_only_the_exit_reads_moves_to_the_exit() {
    let (after, changed) = run(&looped("mul i32 %a, 7", "1"));
    assert!(changed, "{after}");
    let (entry, exit) = after.split_once("exit:").expect("an exit");
    assert!(!entry.contains("mul i32 %a, 7"), "{after}");
    assert!(exit.trim_start().starts_with("%k = mul i32 %a, 7"), "{after}");
}

#[test]
fn a_value_the_loop_reads_stays() {
    let (after, changed) = run(&looped("mul i32 %a, 7", "%k"));
    assert!(!changed, "{after}");
}

#[test]
fn a_load_stays_because_the_loop_may_write_what_it_reads() {
    let (after, changed) = run(&looped("load i32, ptr @g", "1"));
    assert!(!changed, "{after}");
}
