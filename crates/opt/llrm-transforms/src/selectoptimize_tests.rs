use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::Declared;

use super::{Env, Target, select_optimize};
use crate::testing::{parsed, printed};

fn x86() -> llrm_x86::tti::X86 {
    llrm_x86::tti::X86 {
        sse1: false,
        sse2: false,
        macro_fusion: false,
        cmov: true,
        registers: None,
        quad: false,
        real_mode: None,
        loop_micro_op_buffer: 0,
        issue_width: 0,
        position_independent: std::cell::Cell::new(true),
        size_bytes: false,
        word_vectors: false,
        tuning: llrm_mir::target::Tuning::PIN,
    }
}

/// @f of `text` after SelectOptimize; whether anything changed.
fn optimized(
    text: &str,
    configure: impl FnOnce(&mut Target),
) -> (bool, String) {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let callees = llrm_mir::memory::callees(&module);
    let mut declared = Declared::of(&module);
    let tti = x86();
    let mut target = Target::base(&tti);
    configure(&mut target);
    let layout = DataLayout::default();
    let changed = {
        let (context, function) = module.function_mut("f").expect("@f");
        let env = Env { context, globals: &globals, metadata: &metadata, layout: &layout, callees: &callees };
        select_optimize(&env, &target, &mut declared, function)
    };
    declared.place(&mut module).expect("places");
    (changed, printed(&module))
}

/// A loop whose select picks the next `%acc`, so the condition waits on the
/// last iteration's select: a loop-carried chain of `mul`, `icmp`, `select`.
const CARRIED: &str = "define i32 @f(i32 %a, i32 %b, i32 %n) {
entry:
  br label %loop

loop:
  %i = phi i32 [ 0, %entry ], [ %i.next, %loop ]
  %acc = phi i32 [ 1, %entry ], [ %s, %loop ]
  %m = mul i32 %acc, 3
  %c = icmp sgt i32 %m, 100
  %s = select i1 %c, i32 %a, i32 %b, !prof !0
  %i.next = add i32 %i, 1
  %done = icmp eq i32 %i.next, %n
  br i1 %done, label %exit, label %loop

exit:
  ret i32 %s
}

!0 = !{!\"branch_weights\", i32 1000, i32 1}
";

/// The select feeds the condition of the next iteration and its branch is
/// predictable: the loop's critical path shrinks past the pin's thresholds,
/// so the select is a branch and a phi (`select.false`, a frozen condition).
#[test]
fn test_a_select_on_a_loop_carried_chain_with_a_predictable_condition_becomes_a_branch() {
    let (changed, text) = optimized(CARRIED, |_| {});
    assert!(changed, "{text}");
    assert!(!text.contains("select i1"), "{text}");
    assert!(text.contains("freeze i1 %c"), "{text}");
    assert!(text.contains("phi i32 [ %a, %loop ], [ %b, %select.false ]"), "{text}");
}

/// A select whose condition is not carried gains under 4 cycles from a
/// branch: it stays a select.
#[test]
fn test_a_cheap_select_in_a_loop_stays() {
    let (changed, text) = optimized(
        &CARRIED.replace("%m = mul i32 %acc, 3", "%m = add i32 %a, 3").replace("[ %s, %loop ]", "[ 1, %loop ]"),
        |_| {},
    );
    assert!(!changed, "{text}");
    assert!(text.contains("select i1 %c"), "{text}");
}

#[test]
fn test_a_target_that_does_not_enable_the_pass_keeps_the_select() {
    let (changed, text) = optimized(CARRIED, |target| target.enabled = false);
    assert!(!changed, "{text}");
}

#[test]
fn test_an_optsize_function_keeps_the_select() {
    let (changed, text) = optimized(&CARRIED.replace("i32 %n) {", "i32 %n) optsize {"), |_| {});
    assert!(!changed, "{text}");
}

/// bintree -O3's `insert`: `at = t[3*at + (key < t[3*at] ? 1 : 2)]` until nil.
/// The select of two constants sat on the load-address chain of every
/// iteration and became `setg; sub` (19.5k clk, gcc's branch 10.8k).
const WALK: &str = "define i32 @f(ptr %t, i32 %key) {
entry:
  br label %loop

loop:
  %at = phi i32 [ 1, %entry ], [ %next, %loop ]
  %base = mul nsw i32 %at, 3
  %b64 = sext i32 %base to i64
  %p = getelementptr inbounds i32, ptr %t, i64 %b64
  %k = load i32, ptr %p
  %c = icmp sgt i32 %k, %key
  %side = select i1 %c, i32 1, i32 2
  %slot = add nsw i32 %base, %side
  %s64 = sext i32 %slot to i64
  %q = getelementptr inbounds i32, ptr %t, i64 %s64
  %next = load i32, ptr %q
  %nil = icmp eq i32 %next, 0
  br i1 %nil, label %exit, label %loop

exit:
  ret i32 %slot
}
";

fn constant_arms(target: &mut Target) {
    target.predictable_select_expensive = true;
    target.misprediction_penalty = 16;
    target.constant_arm_selects = true;
}

#[test]
fn test_a_select_of_constants_on_a_loop_carried_address_chain_becomes_a_branch() {
    let (changed, text) = optimized(WALK, constant_arms);
    assert!(changed, "{text}");
    assert!(!text.contains("select i1"), "{text}");
    assert!(text.contains("phi i32 [ 1, %loop ], [ 2, %select.false ]"), "{text}");
}

/// The pin's `shouldTreatInstructionLikeSelect`: two constants stay a select.
#[test]
fn test_the_pins_select_of_constants_stays_a_select() {
    let (changed, text) = optimized(WALK, |target| target.predictable_select_expensive = true);
    assert!(!changed, "{text}");
    assert!(text.contains("select i1 %c, i32 1, i32 2"), "{text}");
}

/// @f of `text` after `min_max_to_selects`.
fn min_max_selected(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let callees = llrm_mir::memory::callees(&module);
    let layout = DataLayout::default();
    {
        let (context, function) = module.function_mut("f").expect("@f");
        let env = Env { context, globals: &globals, metadata: &metadata, layout: &layout, callees: &callees };
        super::min_max_to_selects(&env, function);
    }
    printed(&module)
}

/// x_heapsort -O3 on the i486: `child = a[c] < a[c+1] ? c+1 : c; v = smax(a[c],
/// a[c+1])`. The intrinsic is no select, so the two were two branches and the
/// compare twice (39.9k clk; one diamond 35.1k, gcc 36.7k).
const BIGGER_CHILD: &str = "declare i32 @llvm.smax.i32(i32, i32)
declare i32 @llvm.umin.i32(i32, i32)

define i32 @f(i32 %x, i32 %y, i32 %c) {
entry:
  %lt = icmp slt i32 %x, %y
  %next = add i32 %c, 1
  %child = select i1 %lt, i32 %next, i32 %c
  %m = call i32 @llvm.smax.i32(i32 %x, i32 %y)
  %r = add i32 %child, %m
  ret i32 %r
}
";

#[test]
fn test_a_max_beside_a_compare_of_its_operands_is_a_select_of_that_compare() {
    let text = min_max_selected(BIGGER_CHILD);
    assert!(!text.contains("call i32 @llvm.smax"), "{text}");
    assert!(text.contains("select i1 %lt, i32 %y, i32 %x"), "{text}");
}

#[test]
fn test_the_compare_may_have_its_operands_swapped_and_be_non_strict() {
    let text = min_max_selected(&BIGGER_CHILD.replace("icmp slt i32 %x, %y", "icmp sge i32 %y, %x"));
    assert!(text.contains("select i1 %lt, i32 %y, i32 %x"), "{text}");
}

#[test]
fn test_a_max_with_no_compare_of_its_operands_stays_a_call() {
    let text = min_max_selected(&BIGGER_CHILD.replace("icmp slt i32 %x, %y", "icmp slt i32 %x, %c"));
    assert!(text.contains("call i32 @llvm.smax"), "{text}");
}

#[test]
fn test_an_unsigned_compare_is_no_compare_for_a_signed_max() {
    let text = min_max_selected(&BIGGER_CHILD.replace("icmp slt i32 %x, %y", "icmp ult i32 %x, %y"));
    assert!(text.contains("call i32 @llvm.smax"), "{text}");
}

/// x_ld_max -O2/-O3 on the i486: `if (a[i] > m) m = a[i]` over `long double` is
/// a select on a carried chain; as branches it ran 12,818 -> 18,450 clk.
#[test]
fn test_a_select_of_an_x87_value_stays_where_the_target_leaves_them() {
    let fp80 = CARRIED
        .replace("i32 %a, i32 %b, i32 %n", "x86_fp80 %a, x86_fp80 %b, i32 %n")
        .replace("%acc = phi i32 [ 1, %entry ], [ %s, %loop ]", "%acc = phi x86_fp80 [ 1.0, %entry ], [ %s, %loop ]")
        .replace("%m = mul i32 %acc, 3", "%m = fmul x86_fp80 %acc, 3.0")
        .replace("%c = icmp sgt i32 %m, 100", "%c = fcmp ogt x86_fp80 %m, 100.0")
        .replace("select i1 %c, i32 %a, i32 %b", "select i1 %c, x86_fp80 %a, x86_fp80 %b")
        .replace("define i32 @f", "define x86_fp80 @f")
        .replace("ret i32 %s", "ret x86_fp80 %s");
    let (changed, text) = optimized(&fp80, |_| {});
    assert!(changed, "the premise: the pass takes it: {text}");
    let (changed, text) = optimized(&fp80, |target| target.i486(16));
    assert!(!changed && text.contains("select i1"), "{text}");
}
