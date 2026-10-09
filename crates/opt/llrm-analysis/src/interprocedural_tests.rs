//! Adapted from llrm-core's `analysis/interprocedural_tests.rs`.

use std::collections::BTreeSet;

use llrm_mir::module::Module;

use super::*;
use crate::testing::{function, parsed};

fn names(
    module: &Module,
    items: &[&str],
) -> BTreeSet<GlobalId> {
    items.iter().map(|one| module.named(one).unwrap_or_else(|| panic!("no @{one}"))).collect()
}

fn number(
    module: &Module,
    constant: ConstantId,
) -> u128 {
    match module.context.get(constant).kind {
        ConstantKind::Int(bits) => bits,
        ref other => panic!("{other:?} is not an integer"),
    }
}

/// Each map entry by its function's name, its constants as numbers.
fn named(
    module: &Module,
    facts: &Parameters,
) -> Vec<(String, Vec<Option<u128>>)> {
    facts
        .iter()
        .map(|(&id, values)| {
            (
                module.global(id).name.clone().unwrap(),
                values.iter().map(|value| value.map(|one| number(module, one))).collect(),
            )
        })
        .collect()
}

fn calls(function: &Function) -> Vec<InstId> {
    function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::Call(_)))
        .collect()
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
    assert_eq!(noreturn_procedures(&module, &module.declarations(), &both), both);
    let spin = names(&module, &["spin"]);
    let caller = function(&module, "caller");
    let sites = terminal_sites(&module.context, &module.declarations(), caller, &spin);
    assert_eq!(sites.into_iter().collect::<Vec<_>>(), calls(caller));

    let mut module = module;
    let declarations = module.declarations();
    let (context, caller) = module.function_mut("caller").unwrap();
    assert!(terminal_calls(context, &declarations, caller, &spin));
    let left: Vec<&str> = caller.walk().map(|(_, inst)| caller.instruction(inst).opcode.mnemonic()).collect();
    assert_eq!(left, ["call", "unreachable"]);
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
    assert_eq!(noreturn_procedures(&module, &module.declarations(), &fails), fails);
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
    assert_eq!(noreturn_procedures(&module, &module.declarations(), &BTreeSet::new()), BTreeSet::new());
}

#[test]
fn test_mutually_recursive_private_terminal_bodies_are_noreturn() {
    let module = parsed(
        "define internal void @first() {
b:
  call void @second()
  ret void
}

define internal void @second() {
b:
  call void @first()
  ret void
}
",
    );
    let both = names(&module, &["first", "second"]);
    assert_eq!(noreturn_procedures(&module, &module.declarations(), &both), both);
}

#[test]
fn test_noreturn_scc_rejects_a_member_with_a_normal_return() {
    let module = parsed(
        "define internal void @first() {
b:
  call void @second()
  ret void
}

define internal void @second() {
b:
  ret void
}
",
    );
    assert_eq!(
        noreturn_procedures(&module, &module.declarations(), &names(&module, &["first", "second"])),
        BTreeSet::new()
    );
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
    assert_eq!(
        named(&module, &constant_parameters(&module, &names(&module, &["take"]))),
        [("take".to_owned(), vec![None, Some(3)])]
    );
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
    assert_eq!(noreturn_procedures(&module, &module.declarations(), &names(&module, &["sometimes"])), BTreeSet::new());
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
    assert!(terminal_sites(&module.context, &module.declarations(), f, &BTreeSet::new()).is_empty());
}

/// Each linkage the linker may swap for another body.
const REPLACEABLE: [&str; 5] = ["weak", "weak_odr", "linkonce", "linkonce_odr", "available_externally"];

/// A replaceable body's constant return and purity were taken as facts of
/// whichever body links in: its calls lost their results and effects.
#[test]
fn a_body_the_linker_may_replace_proves_no_return_constant() {
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
    }
}
