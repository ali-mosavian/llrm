//! Adapted from llrm-core's `analysis/interprocedural_tests.rs`.

use std::collections::BTreeSet;

use llrm_mir::module::Module;

use super::*;
use crate::testing::{function, parsed};

fn names(module: &Module, items: &[&str]) -> BTreeSet<GlobalId> {
    items.iter().map(|one| module.named(one).unwrap_or_else(|| panic!("no @{one}"))).collect()
}

fn number(module: &Module, constant: ConstantId) -> u128 {
    match module.context.get(constant).kind {
        ConstantKind::Int(bits) => bits,
        ref other => panic!("{other:?} is not an integer"),
    }
}

/// Each map entry by its function's name, its constants as numbers.
fn named(module: &Module, facts: &Parameters) -> Vec<(String, Vec<Option<u128>>)> {
    facts
        .iter()
        .map(|(&id, values)| {
            (module.global(id).name.clone().unwrap(), values.iter().map(|value| value.map(|one| number(module, one))).collect())
        })
        .collect()
}

fn calls(function: &Function) -> Vec<InstId> {
    function.walk().map(|(_, inst)| inst).filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::Call(_))).collect()
}

fn layout(module: &Module) -> DataLayout {
    DataLayout::parse(module.datalayout.as_deref().unwrap_or("")).unwrap()
}

#[test]
fn test_module_constant_returns_require_every_exit_to_agree() {
    let module = parsed(
        "define internal i16 @yes() {
b1:
  ret i16 37
}

define internal i16 @no(i1 %c) {
b1:
  br i1 %c, label %b2, label %b3

b2:
  ret i16 37

b3:
  ret i16 38
}
",
    );
    let returns = constant_returns(&module);
    let found: Vec<(GlobalId, u128)> = returns.iter().map(|(&id, &constant)| (id, number(&module, constant))).collect();
    assert_eq!(found, [(module.named("yes").unwrap(), 37)]);
}

#[test]
fn test_parameter_specialization_requires_every_call_to_agree() {
    let text = |second: u32| {
        format!(
            "define internal i16 @leaf(i16 %x) {{
b:
  ret i16 %x
}}

define void @a() {{
b:
  %r = call i16 @leaf(i16 7)
  ret void
}}

define void @b() {{
b:
  %r = call i16 @leaf(i16 {second})
  ret void
}}
"
        )
    };
    let module = parsed(&text(9));
    assert!(constant_parameters(&module, &names(&module, &["leaf"])).is_empty());

    let mut module = parsed(&text(7));
    let agreed = constant_parameters(&module, &names(&module, &["leaf"]));
    assert_eq!(named(&module, &agreed), [("leaf".to_owned(), vec![Some(7)])]);
    let constants = agreed[&module.named("leaf").unwrap()].clone();
    let (context, leaf) = module.function_mut("leaf").unwrap();
    assert!(specialize_parameters(context, leaf, &constants));
    assert_eq!(constant_returns(&module).values().map(|&one| number(&module, one)).collect::<Vec<_>>(), [7]);
}

#[test]
fn test_constant_parameters_reads_a_propagated_return() {
    let mut module = parsed(
        "define internal i16 @four() {
b:
  ret i16 4
}

define internal void @choose(i16 %x) {
b:
  ret void
}

define void @caller() {
b:
  %x = call i16 @four()
  call void @choose(i16 %x)
  ret void
}
",
    );
    let eligible = names(&module, &["choose"]);
    assert!(constant_parameters(&module, &eligible).is_empty());
    let returns = constant_returns(&module);
    let (context, caller) = module.function_mut("caller").unwrap();
    assert!(propagate_returns(context, caller, &returns));
    assert_eq!(calls(caller).len(), 2, "the call stays");
    assert_eq!(named(&module, &constant_parameters(&module, &eligible)), [("choose".to_owned(), vec![Some(4)])]);
}

#[test]
fn test_current_call_constants_keeps_a_per_call_fact_when_another_call_is_dynamic() {
    let module = parsed(
        "define internal void @choose(i16 %x) {
b:
  ret void
}

define void @caller(i16 %dynamic) {
b:
  call void @choose(i16 4)
  call void @choose(i16 %dynamic)
  ret void
}
",
    );
    let caller = function(&module, "caller");
    let found = current_call_constants(&module.context, caller);
    let sites = calls(caller);
    assert_eq!(found.keys().copied().collect::<Vec<_>>(), sites);
    assert_eq!(found[&sites[0]].iter().map(|one| one.map(|one| number(&module, one))).collect::<Vec<_>>(), [Some(4)]);
    assert_eq!(found[&sites[1]], vec![None]);
    assert!(constant_parameters(&module, &names(&module, &["choose"])).is_empty());
}

#[test]
fn test_pure_call_removal_drops_the_unused_call() {
    let mut module = parsed(
        "define internal i16 @leaf(i16 %x) {
b:
  ret i16 %x
}

define i16 @caller() {
b:
  %r = call i16 @leaf(i16 9)
  ret i16 42
}
",
    );
    let pure = pure_procedures(&module);
    let declarations = effects::declarations(&module);
    let (context, caller) = module.function_mut("caller").unwrap();
    assert!(remove_dead_pure_calls(context, &declarations, caller, &pure));
    let left: Vec<&str> = caller.walk().map(|(_, inst)| caller.instruction(inst).opcode.mnemonic()).collect();
    assert_eq!(left, ["ret"]);
}

#[test]
fn test_purity_refuses_nontermination_and_nonlocal_stores() {
    let module = parsed(
        "@g = global i16 0

define void @loop() {
b:
  br label %b
}

define void @write() {
b:
  store i16 1, ptr @g
  ret void
}
",
    );
    assert_eq!(pure_procedures(&module), BTreeSet::new());
}

#[test]
fn test_a_stated_pure_callee_and_a_frame_store_keep_a_body_pure() {
    let module = parsed(
        "declare i16 @llvm.smax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i16 @unknown(i16)

define i16 @local(i16 %x) {
b:
  %slot = alloca [2 x i16]
  %high = getelementptr inbounds i16, ptr %slot, i16 1
  store i16 %x, ptr %high
  %y = load i16, ptr %high
  %z = call i16 @llvm.smax.i16(i16 %y, i16 0)
  ret i16 %z
}

define i16 @opaque(i16 %x) {
b:
  %z = call i16 @unknown(i16 %x)
  ret i16 %z
}
",
    );
    assert_eq!(pure_procedures(&module), names(&module, &["local"]));
}

#[test]
fn test_readonly_procedure_allows_only_direct_nonvolatile_static_reads() {
    let module = parsed(
        "@g = global i16 0

define i16 @read() {
b:
  %v = load i16, ptr @g
  ret i16 %v
}

define i16 @volatile() {
b:
  %v = load volatile i16, ptr @g
  ret i16 %v
}

define void @write() {
b:
  store i16 1, ptr @g
  ret void
}
",
    );
    assert_eq!(readonly_procedures(&module, &layout(&module)), names(&module, &["read"]));
}

#[test]
fn test_direct_noreturn_summary_names_the_callers_terminal_call() {
    let module = parsed(
        "define internal void @spin() {
b:
  br label %b
}

define internal void @caller() {
b:
  call void @spin()
  ret void
}
",
    );
    let both = names(&module, &["spin", "caller"]);
    assert_eq!(noreturn_procedures(&module, &both), both);
    let caller = function(&module, "caller");
    let sites = terminal_sites(&module.context, &effects::declarations(&module), caller, &names(&module, &["spin"]));
    assert_eq!(sites.into_iter().collect::<Vec<_>>(), calls(caller));
}

#[test]
fn test_a_stated_noreturn_callee_ends_the_path() {
    let module = parsed(
        "declare void @llrm.qb.error(i16) noreturn

define internal void @fails() {
b:
  call void @llrm.qb.error(i16 5)
  ret void
}
",
    );
    let fails = names(&module, &["fails"]);
    assert_eq!(noreturn_procedures(&module, &fails), fails);
}

#[test]
fn test_noreturn_summary_does_not_make_an_exported_body_a_private_fact() {
    let module = parsed(
        "define void @exported() {
b:
  br label %b
}
",
    );
    assert_eq!(noreturn_procedures(&module, &BTreeSet::new()), BTreeSet::new());
}

#[test]
fn test_mutually_recursive_private_terminal_bodies_are_noreturn() {
    let module = parsed(
        "define internal void @first() {
b:
  call void @second()
  unreachable
}

define internal void @second() {
b:
  call void @first()
  unreachable
}
",
    );
    let both = names(&module, &["first", "second"]);
    assert_eq!(noreturn_procedures(&module, &both), both);
}

#[test]
fn test_noreturn_scc_rejects_a_member_with_a_normal_return() {
    let module = parsed(
        "define internal void @first() {
b:
  call void @second()
  unreachable
}

define internal void @second() {
b:
  ret void
}
",
    );
    assert_eq!(noreturn_procedures(&module, &names(&module, &["first", "second"])), BTreeSet::new());
}
