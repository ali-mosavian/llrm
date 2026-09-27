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

#[test]
fn void_declared_and_partly_dynamic_returns_have_no_constant() {
    let module = parsed(
        "declare i16 @external()

define void @nothing() {
b:
  ret void
}

define i16 @mixed(i1 %c, i16 %x) {
b1:
  br i1 %c, label %b2, label %b3

b2:
  ret i16 5

b3:
  ret i16 %x
}
",
    );
    assert!(constant_returns(&module).is_empty());
}

#[test]
fn a_returned_constant_replaces_only_uses_of_its_callers_result() {
    let mut module = parsed(
        "define internal i16 @five() {
b:
  ret i16 5
}

declare i16 @other()

define i16 @caller() {
b:
  %unused = call i16 @five()
  %five = call i16 @five()
  %other = call i16 @other()
  %sum = add i16 %five, %other
  ret i16 %sum
}
",
    );
    let returns = constant_returns(&module);
    let (context, caller) = module.function_mut("caller").unwrap();
    assert!(propagate_returns(context, caller, &returns));
    assert_eq!(calls(caller).len(), 3);
    assert!(llrm_mir::print::module(&module).contains("%sum = add i16 5, %other"));
}

#[test]
fn an_unused_call_result_is_not_a_change() {
    let mut module = parsed(
        "define internal i16 @five() {
b:
  ret i16 5
}

define void @caller() {
b:
  %unused = call i16 @five()
  ret void
}
",
    );
    let returns = constant_returns(&module);
    let (context, caller) = module.function_mut("caller").unwrap();
    assert!(!propagate_returns(context, caller, &returns));
}

#[test]
fn a_constant_that_is_not_an_integer_is_no_parameter_fact() {
    let module = parsed(
        "define internal void @take(ptr %p, i16 %n) {
b:
  ret void
}

define void @caller() {
b:
  call void @take(ptr null, i16 3)
  ret void
}
",
    );
    assert_eq!(named(&module, &constant_parameters(&module, &names(&module, &["take"]))), [("take".to_owned(), vec![None, Some(3)])]);
}

#[test]
fn a_body_nobody_calls_has_no_parameter_facts() {
    let module = parsed(
        "define internal void @take(i16 %n) {
b:
  ret void
}
",
    );
    assert!(constant_parameters(&module, &names(&module, &["take"])).is_empty());
}

#[test]
fn only_eligible_callees_get_parameter_facts() {
    let module = parsed(
        "define void @exported(i16 %n) {
b:
  ret void
}

define void @caller() {
b:
  call void @exported(i16 3)
  ret void
}
",
    );
    assert!(constant_parameters(&module, &BTreeSet::new()).is_empty());
}

#[test]
fn an_indirect_call_has_no_per_call_constants() {
    let module = parsed(
        "define void @caller(ptr %target) {
b:
  call void %target(i16 3)
  ret void
}
",
    );
    assert!(current_call_constants(&module.context, function(&module, "caller")).is_empty());
}

#[test]
fn specializing_a_parameter_nothing_reads_changes_nothing() {
    let mut module = parsed(
        "define internal i16 @leaf(i16 %x, i16 %y) {
b:
  ret i16 %y
}
",
    );
    let i16 = module.context.types.int(16);
    let seven = module.context.int(i16, 7);
    let (context, leaf) = module.function_mut("leaf").unwrap();
    assert!(!specialize_parameters(context, leaf, &[Some(seven), None]));
}

#[test]
fn purity_reaches_a_caller_of_a_pure_body_but_not_a_self_recursive_one() {
    let module = parsed(
        "define internal i16 @leaf(i16 %x) {
b:
  %y = add i16 %x, 1
  ret i16 %y
}

define i16 @middle(i16 %x) {
b:
  %y = call i16 @leaf(i16 %x)
  ret i16 %y
}

define i16 @recursive(i16 %x) {
b:
  %y = call i16 @recursive(i16 %x)
  ret i16 %y
}
",
    );
    assert_eq!(pure_procedures(&module), names(&module, &["leaf", "middle"]));
}

#[test]
fn purity_refuses_reads_of_globals_parameters_and_volatile_frame_accesses() {
    let module = parsed(
        "@g = global i16 0

define i16 @global() {
b:
  %v = load i16, ptr @g
  ret i16 %v
}

define i16 @parameter(ptr %p) {
b:
  %v = load i16, ptr %p
  ret i16 %v
}

define i16 @volatile() {
b:
  %slot = alloca i16
  %v = load volatile i16, ptr %slot
  ret i16 %v
}
",
    );
    assert_eq!(pure_procedures(&module), BTreeSet::new());
}

#[test]
fn a_call_that_states_only_memory_none_is_not_pure_without_willreturn_and_nounwind() {
    let module = parsed(
        "declare i16 @quiet(i16) memory(none)
declare i16 @returns(i16) memory(none) willreturn nounwind

define i16 @f(i16 %x) {
b:
  %y = call i16 @quiet(i16 %x)
  ret i16 %y
}

define i16 @g(i16 %x) {
b:
  %y = call i16 @returns(i16 %x)
  ret i16 %y
}
",
    );
    assert_eq!(pure_procedures(&module), names(&module, &["g"]));
}

#[test]
fn dead_call_removal_keeps_used_results_impure_callees_and_invokes() {
    let mut module = parsed(
        "declare void @effect()
declare i32 @__gxx_personality_v0(...)

define internal i16 @leaf(i16 %x) {
b:
  ret i16 %x
}

define i16 @caller() personality ptr @__gxx_personality_v0 {
b:
  %used = call i16 @leaf(i16 1)
  call void @effect()
  %dead = invoke i16 @leaf(i16 2) to label %ok unwind label %pad

ok:
  ret i16 %used

pad:
  %lp = landingpad { ptr, i32 } cleanup
  resume { ptr, i32 } %lp
}
",
    );
    let pure = pure_procedures(&module);
    let declarations = effects::declarations(&module);
    let before = llrm_mir::print::module(&module);
    let (context, caller) = module.function_mut("caller").unwrap();
    assert!(!remove_dead_pure_calls(context, &declarations, caller, &pure));
    assert_eq!(llrm_mir::print::module(&module), before);
}

#[test]
fn readonly_reaches_callers_and_refuses_far_external_and_pointer_reads() {
    let module = parsed(
        "@near = global i16 0
@far_data = addrspace(1) global i16 0
@external_data = external global i16

define i16 @read() {
b:
  %v = load i16, ptr @near
  ret i16 %v
}

define i16 @caller() {
b:
  %v = call i16 @read()
  ret i16 %v
}

define i16 @far() {
b:
  %v = load i16, ptr addrspace(1) @far_data
  ret i16 %v
}

define i16 @external() {
b:
  %v = load i16, ptr @external_data
  ret i16 %v
}

define i16 @pointer(ptr %p) {
b:
  %v = load i16, ptr %p
  ret i16 %v
}
",
    );
    assert_eq!(readonly_procedures(&module, &layout(&module)), names(&module, &["read", "caller"]));
}

#[test]
fn a_private_body_that_returns_on_one_path_is_not_noreturn() {
    let module = parsed(
        "declare void @exit(i16) noreturn

define internal void @sometimes(i1 %c) {
b1:
  br i1 %c, label %b2, label %b3

b2:
  call void @exit(i16 1)
  unreachable

b3:
  ret void
}
",
    );
    assert_eq!(noreturn_procedures(&module, &names(&module, &["sometimes"])), BTreeSet::new());
}

#[test]
fn an_invoke_of_a_noreturn_callee_is_not_a_terminal_site() {
    let module = parsed(
        "declare void @exit(i16) noreturn
declare i32 @__gxx_personality_v0(...)

define void @f() personality ptr @__gxx_personality_v0 {
b:
  invoke void @exit(i16 1) to label %ok unwind label %pad

ok:
  unreachable

pad:
  %lp = landingpad { ptr, i32 } cleanup
  ret void
}
",
    );
    let f = function(&module, "f");
    assert!(terminal_sites(&module.context, &effects::declarations(&module), f, &BTreeSet::new()).is_empty());
}

/// Each linkage the linker may swap for another body.
const REPLACEABLE: [&str; 5] = ["weak", "weak_odr", "linkonce", "linkonce_odr", "available_externally"];

/// A replaceable body's constant return and purity were taken as facts of
/// whichever body links in: its calls lost their results and effects.
#[test]
fn a_body_the_linker_may_replace_proves_no_return_constant_purity_or_readonly() {
    for linkage in REPLACEABLE {
        let module = parsed(&format!(
            "@g = global i16 0

define {linkage} i16 @five() {{
b:
  ret i16 5
}}

define {linkage} i16 @read() {{
b:
  %v = load i16, ptr @g
  ret i16 %v
}}

define i16 @exact() {{
b:
  ret i16 5
}}
"
        ));
        let exact = names(&module, &["exact"]);
        assert_eq!(constant_returns(&module).keys().copied().collect::<BTreeSet<_>>(), exact, "{linkage}");
        assert_eq!(pure_procedures(&module), exact, "{linkage}");
        assert_eq!(readonly_procedures(&module, &layout(&module)), exact, "{linkage}");
    }
}
