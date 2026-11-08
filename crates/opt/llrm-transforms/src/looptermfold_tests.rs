//! `LoopTermFold`: the exit test of a loop moves to another induction variable
//! and the counter it read dies.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::passes::Outer;

use crate::testing::{parsed, printed, results};

/// `text`'s @f with its exit tests folded, printed.
fn folded(text: &str) -> (bool, String) {
    folded_on(text, None)
}

/// [`folded`] on a target of the given tuning.
fn folded_on(text: &str, tuned: Option<crate::testing::Tuned>) -> (bool, String) {
    let mut module = parsed(&format!("{DOS}{text}"));
    let before = module.clone();
    let machine = tuned.map(|one| std::rc::Rc::new(one) as std::rc::Rc<dyn llrm_mir::target::Machine>);
    let (layout, outer) = (layout(&module), Outer::of(&module, machine));
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

/// A pointer walks to `&a[n]`, which the function already computes before the
/// loop. The fold's exit value rebuilt it (`shl; add; gep`: three instructions)
/// instead of reading the existing one, which cost m68k memset 34 clocks.
const END_ADDRESS: &str = "define i16 @f(i16 %n) {
entry:
  %end = getelementptr i16, ptr @g, i16 %n
  %c0 = icmp sgt i16 %n, 0
  br i1 %c0, label %pre, label %exit
pre:
  br label %loop
loop:
  %i = phi i16 [ 0, %pre ], [ %inc, %loop ]
  %p = phi ptr [ @g, %pre ], [ %p.next, %loop ]
  store i16 0, ptr %p
  %p.next = getelementptr inbounds i16, ptr %p, i16 1
  %inc = add nuw nsw i16 %i, 1
  %c = icmp ne i16 %inc, %n
  br i1 %c, label %loop, label %done
done:
  br label %exit
exit:
  %r = ptrtoint ptr %end to i16
  ret i16 %r
}
";

#[test]
fn test_an_exit_value_the_function_already_computes_is_reused_where_the_target_asks() {
    let text = format!("@g = global [100 x i16] zeroinitializer\n{END_ADDRESS}");
    let (_, off) = folded_on(&text, Some(Default::default()));
    let (changed, on) = folded_on(&text, Some(crate::testing::Tuned { ltf_reuse: true, ..Default::default() }));
    assert!(changed, "{on}");
    let reads_end = "icmp eq ptr %p.next, %end";
    assert!(!off.contains(reads_end), "the pin's pass reused it:\n{off}");
    assert!(on.contains(reads_end), "the exit value was rebuilt:\n{on}");
}
