//! `rewritten` over loops the interpreter runs before and after.

use llrm_mir::domtree::DomTree;

use super::rewritten;
use crate::testing::{parsed, printed, results};

/// A loop whose first block leaves on `%inv` (a parameter) with the header
/// counter `%x` (preheader value 5) read in the exit.
const LOOP: &str = "define i32 @f(i32 %n, i1 %inv) {
entry:
  br label %header

header:
  %x = phi i32 [ 5, %entry ], [ %x.next, %latch ]
  br i1 %inv, label %exit, label %latch

latch:
  %x.next = add i32 %x, 1
  %c = icmp ult i32 %x.next, %n
  br i1 %c, label %header, label %done

done:
  ret i32 0

exit:
  %r = phi i32 [ %x, %header ]
  ret i32 %r
}
";

fn run(text: &str) -> (String, bool) {
    let mut module = parsed(text);
    let before = module.clone();
    let (_, function) = module.function_mut("f").expect("@f");
    let loops = llrm_analysis::cfg::loops_of(function);
    let tree = DomTree::of(function);
    let changed = rewritten(function, &loops, &tree);
    let after = printed(&module);
    assert_eq!(results(&module, &[&[3, 1], &[3, 0]]), results(&before, &[&[3, 1], &[3, 0]]), "{after}");
    (after, changed)
}

/// The pin's rewriteFirstIterationLoopExitValues: the exit is taken on the
/// first trip, where `%x` is its preheader value.
#[test]
fn an_invariant_exit_reads_the_preheader_value_of_a_header_phi() {
    let (after, changed) = run(LOOP);
    assert!(changed, "{after}");
    assert!(after.contains("%r = phi i32 [ 5, %header ]"), "{after}");
}

#[test]
fn a_loop_variant_exit_condition_leaves_the_phi() {
    let text = LOOP.replace("br i1 %inv, label %exit", "%v = icmp eq i32 %x, 7\n  br i1 %v, label %exit");
    let (after, changed) = run(&text);
    assert!(!changed, "{after}");
}
