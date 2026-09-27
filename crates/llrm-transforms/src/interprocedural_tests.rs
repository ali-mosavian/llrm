//! Behaviour of the whole-module step; the old module had no tests.

use std::collections::BTreeSet;

use llrm_mir::passes::PassManager;

use super::*;
use crate::testing::{parsed, printed, results};

fn ids(module: &Module, names: &[&str]) -> BTreeSet<GlobalId> {
    names.iter().map(|name| module.named(name).unwrap_or_else(|| panic!("no @{name}"))).collect()
}

/// The step over `module` from `roots`, the call priced `call`; each
/// pipeline run as `(procedure, stage)`.
fn step(module: &mut Module, roots: &[&str], call: i64) -> (Proved, Vec<(String, String)>) {
    let roots = ids(module, roots);
    let mut stages = Vec::new();
    let costs = OperationCosts { call, ..OperationCosts::default() };
    let proved = optimized::<String>(
        module,
        &roots,
        &costs,
        &mut |module, id, stage| {
            stages.push((module.global(id).name.clone().unwrap(), stage.to_owned()));
            Ok(())
        },
        &mut |_, _, _| Ok(()),
    )
    .unwrap();
    (proved, stages)
}

fn staged(stages: &[(String, String)], name: &str, stage: &str) -> bool {
    stages.iter().any(|(one, at)| one == name && at == stage)
}

const HELPERS: &str = "define internal i16 @scale(i16 %x) {
b1:
  %y = shl i16 %x, 2
  %z = add i16 %y, 1
  ret i16 %z
}

define internal i16 @clamp(i16 %x) {
b1:
  %big = icmp sgt i16 %x, 100
  br i1 %big, label %b2, label %b3

b2:
  ret i16 100

b3:
  ret i16 %x
}

define i16 @f(i16 %a, i16 %b) {
b1:
  %s = call i16 @scale(i16 %a)
  %c = call i16 @clamp(i16 %s)
  %t = call i16 @scale(i16 %b)
  %sum = add i16 %c, %t
  ret i16 %sum
}
";

const INPUTS: &[&[i128]] = &[&[0, 0], &[3, 4], &[30, 7], &[-1, 1]];

#[test]
fn test_private_leaves_are_inlined_and_each_changed_body_reoptimised() {
    let mut module = parsed(HELPERS);
    let (proved, stages) = step(&mut module, &["f"], 4);
    let text = printed(&module);
    assert!(!text[text.find("define i16 @f").unwrap()..].contains("call "), "{text}");
    assert!(staged(&stages, "f", "inline0.") && staged(&stages, "f", "inline2."), "{stages:?}");
    // No call reaches the helpers any more.
    assert_eq!(proved.reachable, ids(&module, &["f"]));
    assert_eq!(results(&module, INPUTS), results(&parsed(HELPERS), INPUTS));
}

#[test]
fn test_without_roots_every_procedure_is_reachable() {
    let mut module = parsed(HELPERS);
    let (proved, _) = step(&mut module, &[], 4);
    assert_eq!(proved.reachable, ids(&module, &["scale", "clamp", "f"]));
}

#[test]
fn test_a_dead_readonly_call_goes_and_a_live_one_stays() {
    let text = HELPERS.replace("%sum = add i16 %c, %t", "%sum = add i16 %c, %b");
    let mut module = parsed(&text);
    // Priced at nothing, only @clamp, called once, is inlined; the unread
    // second @scale call goes, the read first one stays.
    let (_, stages) = step(&mut module, &["f"], 0);
    let text = printed(&module);
    assert_eq!(text.matches("call i16 @scale").count(), 1, "{text}");
    assert!(staged(&stages, "f", "ipa-pure."), "{stages:?}");
}

const STORES: &str = "@g = global i16 0

define internal void @set(i16 %x) {
b:
  store i16 %x, ptr @g
  ret void
}

define internal i16 @seven() {
b:
  store i16 1, ptr @g
  ret i16 7
}

define i16 @f(i16 %a) {
b:
  call void @set(i16 5)
  call void @set(i16 5)
  %r = call i16 @seven()
  %s = add i16 %r, %a
  ret i16 %s
}
";

#[test]
fn test_agreed_actuals_specialize_and_a_constant_return_is_carried() {
    let mut module = parsed(STORES);
    let (_, stages) = step(&mut module, &["f"], 40);
    let text = printed(&module);
    assert!(text.contains("  store i16 5, ptr @g\n"), "{text}");
    assert!(text.contains("  %s = add i16 7, %a\n"), "{text}");
    // Both calls with effects stay.
    assert_eq!(text.matches("call ").count(), 3, "{text}");
    assert!(staged(&stages, "set", "ipa-args0.") && staged(&stages, "f", "ipa0."), "{stages:?}");
}

#[test]
fn test_disagreeing_actuals_or_a_public_callee_are_not_specialized() {
    for text in [STORES.replace("call void @set(i16 5)\n  %r", "call void @set(i16 %a)\n  %r"), STORES.replace("define internal void @set", "define void @set")] {
        let mut module = parsed(&text);
        step(&mut module, &["f"], 40);
        assert!(printed(&module).contains("  store i16 %x, ptr @g\n"), "{text}");
    }
}

#[test]
fn test_a_private_terminal_body_cuts_its_callers_tail() {
    let mut module = parsed(
        "define internal void @spin() {
b:
  br label %l

l:
  br label %l
}

define i16 @f(i16 %a) {
b:
  call void @spin()
  %s = add i16 %a, 1
  ret i16 %s
}
",
    );
    let (proved, stages) = step(&mut module, &["f"], 40);
    assert_eq!(proved.noreturn, ids(&module, &["spin"]));
    assert!(printed(&module).contains("  call void @spin()\n  unreachable\n"));
    assert!(staged(&stages, "f", "ipa-noreturn."), "{stages:?}");
}

#[test]
fn test_the_step_runs_as_a_module_pass() {
    let mut module = parsed(HELPERS);
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.add_module(Interprocedural { costs: OperationCosts { call: 4, ..OperationCosts::default() }, roots: BTreeSet::new(), pipeline: Box::new(|_, _, _| {}), proved: None });
    let stages = manager.run(&mut module).unwrap();
    assert_eq!(stages.iter().map(|stage| stage.function).collect::<BTreeSet<_>>(), ids(&module, &["f"]));
    assert_eq!(results(&module, INPUTS), results(&parsed(HELPERS), INPUTS));
}
