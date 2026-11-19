//! `LoopTermFold`: the exit test of a loop moves to another induction variable
//! and the counter it read dies.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::passes::Outer;

use crate::testing::{parsed, printed, results};

/// `text`'s @f with its exit tests folded, printed.
fn folded(text: &str) -> (bool, String) {
    let mut module = parsed(&format!("{DOS}{text}"));
    let before = module.clone();
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let (changed, _) = super::folded_held(context, &layout, function, &mut declared, &outer, None);
    let after = printed(&module);
    let inputs: &[&[i128]] = &[&[0], &[1], &[7], &[100]];
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    (changed, after)
}

/// Ten trips: `%i` only counts (its test is the exit), `%j` steps by four and
/// addresses a store (poison in it is UB, which the pass requires of the
/// phi it tests), so the test can read `%j.next == 40`.
fn counted(exit: &str) -> String {
    format!(
        "@g = global [100 x i16] zeroinitializer
define i16 @f(i16 %n) {{
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %inc, %loop ]
  %j = phi i16 [ 0, %entry ], [ %j.next, %loop ]
  %s = phi i16 [ 0, %entry ], [ %s2, %loop ]
  %s2 = add i16 %s, %j
  %a = getelementptr i8, ptr @g, i16 %j
  store i16 %s2, ptr %a
  %j.next = add nuw i16 %j, 4
  %inc = add nuw i16 %i, 1
  {exit}
exit:
  ret i16 %s2
}}
"
    )
}

const LOOP_ON_NE: &str = "%c = icmp ne i16 %inc, 10
  br i1 %c, label %loop, label %exit";

#[test]
fn test_a_counter_that_only_ends_the_loop_gives_the_test_to_the_other_induction_variable() {
    let (changed, after) = folded(&counted(LOOP_ON_NE));
    assert!(changed, "{after}");
    assert!(after.contains("%lsr_fold_term_cond.replaced_term_cond = icmp eq i16 %j.next, 40"), "{after}");
    assert!(!after.contains("%i = phi"), "the dead counter stays:\n{after}");
}

/// The branch left the loop on false; it now leaves on true.
#[test]
fn test_the_successors_swap_so_equality_exits_the_loop() {
    let (changed, after) = folded(&counted(LOOP_ON_NE));
    assert!(changed, "{after}");
    assert!(after.contains("br i1 %lsr_fold_term_cond.replaced_term_cond, label %exit, label %loop"), "{after}");
}

/// An exit that already leaves on true (`icmp eq`) keeps its arms.
#[test]
fn test_an_exit_on_true_keeps_its_successors() {
    let (changed, after) = folded(&counted(
        "%c = icmp eq i16 %inc, 10
  br i1 %c, label %exit, label %loop",
    ));
    assert!(changed, "{after}");
    assert!(after.contains("br i1 %lsr_fold_term_cond.replaced_term_cond, label %exit, label %loop"), "{after}");
}

/// The counter is read by the sum as well: it stays alive, so replacing the
/// test saves nothing.
#[test]
fn test_a_counter_that_is_read_elsewhere_is_left() {
    let (changed, after) =
        folded(&counted(LOOP_ON_NE).replace("%s2 = add i16 %s, %j", "%t = add i16 %s, %j\n  %s2 = add i16 %t, %i"));
    assert!(!changed, "{after}");
}
