//! Behaviour of the whole-module step; the old module had no tests.

use std::collections::BTreeSet;

use llrm_mir::passes::PassManager;

use super::*;
use crate::promote::Promote;
use crate::testing::{managed, parsed, printed, results};

fn ids(module: &Module, names: &[&str]) -> BTreeSet<GlobalId> {
    names.iter().map(|name| module.named(name).unwrap_or_else(|| panic!("no @{name}"))).collect()
}

/// The step over `module` from `roots`, the call priced `call`; each
/// pipeline run as `(procedure, stage)`.
fn step(module: &mut Module, roots: &[&str], call: i64) -> (Proved, Vec<(String, String)>) {
    let roots = ids(module, roots);
    let mut stages = Vec::new();
    let costs = OperationCosts { call, ..OperationCosts::default() };
    let mut analyses = ModuleAnalyses::of(module, std::rc::Rc::new(llrm_mir::target::Neutral));
    let proved = optimized::<String>(
        module,
        &mut analyses,
        &roots,
        &costs,
        &mut |module, _, id, stage| {
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
    let target = crate::testing::Tuned { costs: OperationCosts { call: 4, ..OperationCosts::default() }, ..Default::default() };
    manager.add_module(Interprocedural { pipeline: Box::new(|_, _, _, _| {}), proved: None });
    let stages = manager.run_module(&mut module, std::rc::Rc::new(target)).unwrap();
    assert_eq!(stages.iter().map(|stage| stage.function).collect::<BTreeSet<_>>(), ids(&module, &["f"]));
    assert_eq!(results(&module, INPUTS), results(&parsed(HELPERS), INPUTS));
}

const STAMPED: &str = "@slot = global ptr null
@g = global i16 0

define internal i16 @read(ptr %p) {
b:
  %v = load i16, ptr %p
  ret i16 %v
}

define internal void @write(ptr %p, i16 %x) {
b:
  store i16 %x, ptr %p
  ret void
}

define internal void @keep(ptr %p) {
b:
  store ptr %p, ptr @slot
  ret void
}

define internal void @calls(ptr %p, i16 %x) {
b:
  call void @write(ptr %p, i16 %x)
  %v = load i16, ptr %p
  store i16 %v, ptr @g
  ret void
}

declare void @unknown(ptr)

define internal void @hides(ptr %p) {
b:
  %c = alloca ptr
  store ptr %p, ptr %c
  call void @unknown(ptr %c)
  ret void
}

define internal void @ordered() {
b:
  %slot = alloca i16
  store volatile i16 1, ptr %slot
  ret void
}

define i16 @f(i16 %a) {
b:
  %cell = alloca i16
  call void @calls(ptr %cell, i16 %a)
  call void @keep(ptr %cell)
  %r = call i16 @read(ptr %cell)
  %s = load i16, ptr @g
  %t = add i16 %r, %s
  ret i16 %t
}
";

/// No body stated what it does to memory, so a caller without summaries
/// took every call to read and write everything.
#[test]
fn a_body_is_stamped_with_what_its_summary_says_as_llvm_states_it() {
    let mut module = parsed(STAMPED);
    crate::testing::stamped(&mut module).unwrap();
    let text = printed(&module);
    let defined = text.lines().filter(|line| line.starts_with("define")).collect::<Vec<_>>();
    assert_eq!(
        defined,
        [
            "define internal i16 @read(ptr nocapture readonly %p) memory(argmem: read) willreturn {",
            "define internal void @write(ptr nocapture writeonly initializes((0, 2)) %p, i16 %x) memory(argmem: write) willreturn {",
            "define internal void @keep(ptr %p) memory(write, argmem: none, inaccessiblemem: none) willreturn nounwind {",
            "define internal void @calls(ptr nocapture initializes((0, 2)) %p, i16 %x) memory(write, argmem: readwrite, inaccessiblemem: none) willreturn {",
            "define internal void @hides(ptr %p) {",
            "define internal void @ordered() memory(inaccessiblemem: readwrite) willreturn {",
            "define i16 @f(i16 %a) memory(readwrite, argmem: none, inaccessiblemem: none) willreturn {",
        ],
        "{text}"
    );
    assert_eq!(results(&module, INPUTS), results(&parsed(STAMPED), INPUTS));
}

/// Without summaries a call was taken to write every cell: only what its
/// callee states says otherwise.
#[test]
fn a_cell_is_kept_across_a_call_its_stamped_callee_only_reads() {
    let text = |call: &str| {
        format!(
            "@x = global i16 0

declare void @unknown(ptr)

define internal i16 @read(ptr %p) {{
b:
  %v = load i16, ptr %p
  ret i16 %v
}}

define internal void @write(ptr %p) {{
b:
  store i16 1, ptr %p
  ret void
}}

define i16 @f(i16 %c) {{
b:
  store i16 %c, ptr @x
  {call}
  %v = load i16, ptr @x
  ret i16 %v
}}
"
        )
    };
    let loads = |call: &str, stamp: bool| {
        let mut module = parsed(&text(call));
        if stamp {
            crate::testing::stamped(&mut module).unwrap();
        }
        managed(&mut module, Promote);
        let f = module.function_mut("f").unwrap().1;
        f.walk().filter(|(_, inst)| matches!(f.instruction(*inst).opcode, Opcode::Load { .. })).count()
    };
    let read = "%r = call i16 @read(ptr @x)";
    assert_eq!(loads(read, true), 0);
    assert_eq!(loads(read, false), 1);
    assert_eq!(loads("call void @write(ptr @x)", true), 1);
    assert_eq!(loads("call void @unknown(ptr @x)", true), 1);
}

/// The bodies of `text` its stamp states pure, by name.
fn pure(text: &str) -> BTreeSet<String> {
    let mut module = parsed(text);
    crate::testing::stamped(&mut module).unwrap();
    facts::stated_pure(&module).into_iter().map(|id| module.global(id).name.clone().unwrap()).collect()
}

/// The callees whose unused calls in `@uses` of `text`, stamped, go.
fn dropped(text: &str) -> BTreeSet<String> {
    let mut module = parsed(text);
    crate::testing::stamped(&mut module).unwrap();
    let callees = |module: &Module| {
        let uses = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("uses")).expect("@uses").2;
        uses.walk().filter_map(|(_, inst)| llrm_mir::memory::callee(&module.context, uses, inst)).map(|id| module.global(id).name.clone().unwrap()).collect::<BTreeSet<_>>()
    };
    let before = callees(&module);
    let declarations = module.declarations();
    let (context, uses) = module.function_mut("uses").unwrap();
    facts::remove_dead_pure_calls(context, &declarations, uses);
    before.difference(&callees(&module)).cloned().collect()
}

fn named(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|one| (*one).to_owned()).collect()
}

/// Ported from llrm-analysis, where the pure and readonly sets were their
/// own fixed points beside the stamp.
#[test]
fn an_unused_call_to_a_pure_body_goes() {
    let text = "define internal i16 @leaf(i16 %x) {
b:
  ret i16 %x
}

define i16 @uses() {
b:
  %r = call i16 @leaf(i16 9)
  ret i16 42
}
";
    assert_eq!(dropped(text), named(&["leaf"]));
}

#[test]
fn purity_refuses_nontermination_nonlocal_accesses_and_what_callees_do_not_state() {
    let text = "@g = global i16 0

declare i16 @unknown(i16)
declare i16 @quiet(i16) memory(none)
declare i16 @returns(i16) memory(none) willreturn nounwind
declare i16 @llvm.smax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

define void @loop() {
b:
  br label %b
}

define void @write() {
b:
  store i16 1, ptr @g
  ret void
}

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

define i16 @f(i16 %x) {
b:
  %y = call i16 @quiet(i16 %x)
  ret i16 %y
}

define i16 @h(i16 %x) {
b:
  %y = call i16 @returns(i16 %x)
  ret i16 %y
}

define internal i16 @leaf(i16 %x) {
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
";
    assert_eq!(pure(text), named(&["local", "h", "leaf", "middle"]));
}

/// A read of this module's near static data goes unnoticed; a far, an
/// external or a pointer's may fault, a volatile one is ordered.
#[test]
fn an_unused_call_that_only_reads_static_data_goes() {
    let text = "@g = global i16 0
@far_data = addrspace(1) global i16 0
@external_data = external global i16

define i16 @read() {
b:
  %v = load i16, ptr @g
  ret i16 %v
}

define i16 @caller() {
b:
  %v = call i16 @read()
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

define void @uses(ptr %p) {
b:
  %a = call i16 @read()
  %b = call i16 @caller()
  %c = call i16 @volatile()
  call void @write()
  %d = call i16 @far()
  %e = call i16 @external()
  %f = call i16 @pointer(ptr %p)
  ret void
}
";
    assert_eq!(dropped(text), named(&["read", "caller"]));
}

#[test]
fn dead_call_removal_keeps_used_results_impure_callees_and_invokes() {
    let text = "declare void @effect()
declare i32 @__gxx_personality_v0(...)

define internal i16 @leaf(i16 %x) {
b:
  ret i16 %x
}

define i16 @uses() personality ptr @__gxx_personality_v0 {
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
";
    assert_eq!(dropped(text), BTreeSet::new());
}

/// A body the linker may swap for another states nothing.
#[test]
fn a_body_the_linker_may_replace_is_stamped_with_nothing() {
    for linkage in ["weak", "weak_odr", "linkonce", "linkonce_odr", "available_externally"] {
        let text = format!(
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

define void @uses() {{
b:
  %a = call i16 @five()
  %b = call i16 @read()
  %c = call i16 @exact()
  ret void
}}
"
        );
        assert_eq!(pure(&text), named(&["exact"]), "{linkage}");
        assert_eq!(dropped(&text), named(&["exact"]), "{linkage}");
    }
}

/// What the stamp stated of each corpus body, as `interprocedural_stamped.txt`
/// holds it. The old pure and readonly sets proved a subset: a callee's
/// frame-only `llvm.memset` and a loop the frontend states `willreturn`
/// now count.
#[test]
fn the_corpus_is_stamped_as_it_was() {
    let mut lines = Vec::new();
    for (name, mut module) in llrm_analysis::testing::corpus() {
        crate::testing::stamped(&mut module).unwrap();
        let text = printed(&module);
        lines.extend(text.lines().filter(|line| line.starts_with("define")).map(|line| format!("{name} {}", line.trim_end_matches(" {"))));
    }
    let found = lines.join("\n") + "\n";
    // `STAMP_WRITE=1` rewrites the file where a rule changes on purpose.
    if std::env::var_os("STAMP_WRITE").is_some() {
        std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/src/interprocedural_stamped.txt"), &found).unwrap();
    }
    let expected = include_str!("interprocedural_stamped.txt");
    let changed = found.lines().zip(expected.lines()).find(|(one, other)| one != other);
    assert!(found == expected, "first difference: {changed:?}; {} lines, expected {}", found.lines().count(), expected.lines().count());
}
