//! `rewriteLoopExitValues`: what a loop computes, as read after it.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::passes::Outer;

use crate::testing::{parsed, printed, results};

/// `text`'s @f in LCSSA form with its exit values rewritten, printed.
fn rewritten(text: &str) -> (bool, String) {
    let mut module = parsed(&format!("{DOS}{text}"));
    let before = module.clone();
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    crate::lcssa::closed(function).expect("closes");
    let changed = super::rewritten(context, &layout, function, &mut declared, &outer);
    let after = printed(&module);
    let inputs: &[&[i128]] = &[&[0], &[1], &[7], &[100]];
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    (changed, after)
}

/// Ten trips of `s += 2`, read after the loop.
const TEN_TRIPS: &str = "define i16 @f(i16 %n) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %inc, %loop ]
  %s = phi i16 [ 0, %entry ], [ %s2, %loop ]
  %s2 = add i16 %s, 2
  %inc = add i16 %i, 1
  %c = icmp ult i16 %inc, 10
  br i1 %c, label %loop, label %exit
exit:
  ret i16 %s2
}
";

#[test]
fn test_a_sum_over_a_constant_count_is_a_constant_after_the_loop() {
    let (changed, after) = rewritten(TEN_TRIPS);
    assert!(changed, "{after}");
    assert!(after.contains("ret i16 20"), "{after}");
}

/// `n` trips of `s += 2`, the loop entered only for `n != 0`: the value after
/// the loop is `2 * n`, and the loop's sum is dead.
const N_TRIPS: &str = "define i16 @f(i16 %n) {
entry:
  %any = icmp ne i16 %n, 0
  br i1 %any, label %pre, label %done
pre:
  br label %loop
loop:
  %i = phi i16 [ 0, %pre ], [ %inc, %loop ]
  %s = phi i16 [ 0, %pre ], [ %s2, %loop ]
  %s2 = add i16 %s, 2
  %inc = add i16 %i, 1
  %c = icmp ne i16 %inc, %n
  br i1 %c, label %loop, label %exit
exit:
  br label %done
done:
  %r = phi i16 [ 0, %entry ], [ %s2, %exit ]
  ret i16 %r
}
";

#[test]
fn test_a_sum_over_n_trips_is_written_from_n_after_the_loop() {
    let (changed, after) = rewritten(N_TRIPS);
    assert!(changed, "{after}");
    assert!(after.contains("shl i16 %n, 1") || after.contains("mul i16"), "{after}");
}

/// The loop stores `s` on every trip: a hard user, so a value that is not a
/// number stays computed by the loop.
#[test]
fn test_a_value_the_loop_stores_is_left_computed_by_the_loop() {
    let text = N_TRIPS
        .replace("define i16 @f(i16 %n) {", "@g = global i16 0\n\ndefine i16 @f(i16 %n) {")
        .replace("  %inc = add i16 %i, 1\n", "  store i16 %s2, ptr @g\n  %inc = add i16 %i, 1\n");
    let (changed, after) = rewritten(&text);
    assert!(!changed, "{after}");
}

/// A sum over `n` trips of the counter, `n(n-1)/2`, is computed in `i33` (the
/// product of two `zext i32` operands): the back end cannot select it (#2497,
/// `-O3` of a float counted loop failed `a i33 value`). The sum stays computed
/// by the loop.
#[test]
fn test_no_exit_value_is_written_in_an_odd_width() {
    let text = "define i32 @f(i32 %n) {
entry:
  %any = icmp ne i32 %n, 0
  br i1 %any, label %pre, label %done
pre:
  br label %loop
loop:
  %i = phi i32 [ 0, %pre ], [ %inc, %loop ]
  %s = phi i32 [ 0, %pre ], [ %s2, %loop ]
  %s2 = add i32 %s, %i
  %inc = add i32 %i, 1
  %c = icmp ne i32 %inc, %n
  br i1 %c, label %loop, label %exit
exit:
  %out = phi i32 [ %s2, %loop ]
  br label %done
done:
  %r = phi i32 [ 0, %entry ], [ %out, %exit ]
  ret i32 %r
}
";
    let (changed, after) = rewritten(text);
    assert!(!after.contains("i33") && !after.contains("i34"), "{after}");
    assert!(!changed, "{after}");
}

/// The expansion is priced by the target's cost model, not one unit each: an
/// `i32` divide costs more than an `i32` add on the x87 targets (the unit
/// prices made `isHighCostExpansion` count both the same).
#[test]
fn test_the_expansion_is_priced_by_the_targets_cost_model() {
    use llrm_analysis::scev::expand_cost::{ExpansionTti, MachineCosts, Opc};
    use llrm_analysis::scev::expr::Ty;
    let tti =
        llrm_x86::tti::X86::of_cpu("i486").map(|one| std::rc::Rc::new(one) as std::rc::Rc<dyn llrm_mir::tti::Tti>);
    let costs = MachineCosts { tti, minimum_size: false };
    let (divide, add) = (
        costs.get_arithmetic_instr_cost(Opc::UDiv, Ty::Int(32)),
        costs.get_arithmetic_instr_cost(Opc::Add, Ty::Int(32)),
    );
    assert!(divide > add, "divide {divide}, add {add}");
    let unit = MachineCosts { tti: None, minimum_size: false };
    assert_eq!(unit.get_arithmetic_instr_cost(Opc::UDiv, Ty::Int(32)), 1);
}
