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
