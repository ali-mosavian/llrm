//! Behaviour of the whole-module step; the old module had no tests.

use std::collections::BTreeSet;

use llrm_mir::passes::PassManager;

use super::*;
use crate::inline::Threshold;
use crate::promote::Promote;
use crate::testing::{managed, parsed, printed, results};

fn ids(
    module: &Module,
    names: &[&str],
) -> BTreeSet<GlobalId> {
    names.iter().map(|name| module.named(name).unwrap_or_else(|| panic!("no @{name}"))).collect()
}

/// The step over `module` from `roots`, the call priced `call`; each
/// pipeline run as `(procedure, stage)`.
fn step(
    module: &mut Module,
    roots: &[&str],
    call: i64,
) -> (Proved, Vec<(String, String)>) {
    stepped(module, roots, call, Threshold::default())
}

/// `step` with inlining's `threshold`.
fn stepped(
    module: &mut Module,
    roots: &[&str],
    call: i64,
    threshold: Threshold,
) -> (Proved, Vec<(String, String)>) {
    let roots = ids(module, roots).into_iter().map(|id| (0, id)).collect();
    let mut stages = Vec::new();
    let costs = OperationCosts { call, ..OperationCosts::default() };
    let proved = Program::lend(module, std::rc::Rc::new(llrm_mir::target::Neutral), |program| {
        let mut modules = managers(program, &mut ProgramAnalyses::default());
        optimized::<String>(
            program,
            &mut modules,
            &roots,
            &costs,
            None,
            0,
            costs.call,
            threshold,
            &mut |module, _, id, stage| {
                stages.push((module.global(id).name.clone().unwrap(), stage.to_owned()));
                Ok(())
            },
            &mut |_, _, function| Ok(function),
            &mut |_, _, _| Ok(()),
        )
    })
    .unwrap()
    .unwrap();
    (proved, stages)
}

/// `ids` in the one module a test's program has.
fn defined(
    module: &Module,
    names: &[&str],
) -> BTreeSet<Defined> {
    ids(module, names).into_iter().map(|id| (0, id)).collect()
}

fn staged(
    stages: &[(String, String)],
    name: &str,
    stage: &str,
) -> bool {
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
    // Every site of `f` is taken in one scan, and `f` is built once for them.
    assert_eq!(stages, [("f".to_owned(), "inline0.".to_owned())]);
    // No call reaches the helpers any more.
    assert_eq!(proved.reachable, defined(&module, &["f"]));
    assert_eq!(results(&module, INPUTS), results(&parsed(HELPERS), INPUTS));
}

#[test]
fn test_without_roots_every_procedure_is_reachable() {
    let mut module = parsed(HELPERS);
    let (proved, _) = step(&mut module, &[], 4);
    assert_eq!(proved.reachable, defined(&module, &["scale", "clamp", "f"]));
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
    // With no inlining: a callee that stores to memory is inlined otherwise.
    let (_, stages) = stepped(&mut module, &["f"], 40, Threshold::none());
    let text = printed(&module);
    assert!(text.contains("  store i16 5, ptr @g\n"), "{text}");
    assert!(text.contains("  %s = add i16 7, %a\n"), "{text}");
    // Both calls with effects stay.
    assert_eq!(text.matches("call ").count(), 3, "{text}");
    assert!(staged(&stages, "set", "ipa-args0.") && staged(&stages, "f", "ipa0."), "{stages:?}");
}

#[test]
fn test_disagreeing_actuals_or_a_public_callee_are_not_specialized() {
    for text in [
        STORES.replace("call void @set(i16 5)\n  %r", "call void @set(i16 %a)\n  %r"),
        STORES.replace("define internal void @set", "define void @set"),
    ] {
        let mut module = parsed(&text);
        step(&mut module, &["f"], 40);
        assert!(!printed(&module).contains("constprop"), "{text}");
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
    assert_eq!(proved.noreturn, defined(&module, &["spin"]));
    assert!(printed(&module).contains("  call void @spin()\n  unreachable\n"));
    assert!(staged(&stages, "f", "ipa-noreturn."), "{stages:?}");
}

/// Per module, the step saw only its own bodies: a call to another
/// module's body that never returns kept the dead tail after it.
#[test]
fn test_a_terminal_body_in_another_module_cuts_its_callers_tail() {
    let spin = parsed("define void @spin() {\nb:\n  br label %l\n\nl:\n  br label %l\n}\n");
    let caller = parsed(
        "declare void @spin()\n\ndefine i16 @f(i16 %a) {\nb:\n  call void @spin()\n  %s = add i16 %a, 1\n  ret i16 %s\n}\n",
    );
    let exports = llrm_mir::program::Exports::closed(["f".to_owned()].into());
    let mut program =
        Program::new(vec![spin, caller], std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap().exporting(exports);
    let roots = roots(&program);
    let mut modules = managers(&program, &mut ProgramAnalyses::default());
    let proved = optimized::<String>(
        &mut program,
        &mut modules,
        &roots,
        &OperationCosts::default(),
        None,
        0,
        1,
        Threshold::default(),
        &mut |_, _, _, _| Ok(()),
        &mut |_, _, function| Ok(function),
        &mut |_, _, _| Ok(()),
    )
    .unwrap();
    assert_eq!(proved.noreturn, [(0, program.modules[0].named("spin").unwrap())].into());
    assert!(
        printed(&program.modules[1]).contains("  call void @spin()\n  unreachable\n"),
        "{}",
        printed(&program.modules[1])
    );
}

/// Per module, a declaration of another module's body kept no attributes,
/// so every call to it read and wrote all memory.
#[test]
fn test_a_body_s_attributes_are_stated_on_its_declarations() {
    let double = parsed("define i16 @double(i16 %x) {\nb:\n  %y = add i16 %x, %x\n  ret i16 %y\n}\n");
    let caller = parsed(
        "declare i16 @double(i16)\n\ndefine i16 @f(i16 %a) {\nb:\n  %y = call i16 @double(i16 %a)\n  ret i16 %y\n}\n",
    );
    let mut program = Program::new(vec![double, caller], std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let mut modules = managers(&program, &mut ProgramAnalyses::default());
    stamped_all(&mut program, &mut modules).unwrap();
    assert!(
        printed(&program.modules[1]).contains("declare i16 @double(i16) memory(none) willreturn nounwind norecurse\n"),
        "{}",
        printed(&program.modules[1])
    );
}

/// Per module, a call to another module's body that always returns one
/// constant kept its result unknown.
/// Attributes were copied to another module's declaration with their
/// type ids, which name other types there: `range(i16 ...)` read as
/// another type.
#[test]
fn test_a_published_attribute_names_its_type_in_the_declaring_module() {
    let small = parsed("define range(i16 0, 8) i16 @small(i16 %x) {\nb:\n  %y = and i16 %x, 7\n  ret i16 %y\n}\n");
    let caller = parsed(
        "@d = global double 0.0\n@b = global i8 0\n\ndeclare i16 @small(i16)\n\ndefine i16 @f(i16 %a) {\nb:\n  %y = call i16 @small(i16 %a)\n  ret i16 %y\n}\n",
    );
    let mut program = Program::new(vec![small, caller], std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let mut modules = managers(&program, &mut ProgramAnalyses::default());
    stamped_all(&mut program, &mut modules).unwrap();
    let text = printed(&program.modules[1]);
    let declaration = text.lines().find(|line| line.starts_with("declare")).unwrap();
    assert!(declaration.starts_with("declare range(i16 0, 8) i16 @small(i16)"), "{text}");
}

#[test]
fn test_a_constant_another_module_returns_reaches_its_callers() {
    let seven = parsed("define i16 @seven() {\nb:\n  ret i16 7\n}\n");
    let caller = parsed("declare i16 @seven()\n\ndefine i16 @f() {\nb:\n  %y = call i16 @seven()\n  ret i16 %y\n}\n");
    let mut program = Program::new(vec![seven, caller], std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let roots = roots(&program);
    let mut modules = managers(&program, &mut ProgramAnalyses::default());
    optimized::<String>(
        &mut program,
        &mut modules,
        &roots,
        &OperationCosts::default(),
        None,
        0,
        1,
        Threshold::default(),
        &mut |_, _, _, _| Ok(()),
        &mut |_, _, function| Ok(function),
        &mut |_, _, _| Ok(()),
    )
    .unwrap();
    assert!(printed(&program.modules[1]).contains("  ret i16 7\n"), "{}", printed(&program.modules[1]));
}

/// An internal body the runtime calls, as QB's module body, had every
/// caller in the program: its parameters took the one call's constants.
#[test]
fn test_an_entry_keeps_its_parameters_whatever_its_linkage() {
    let body = (1..12).map(|at| format!("  %y{at} = mul i16 %y{}, %x\n", at - 1)).collect::<String>();
    let text = format!(
        "define internal i16 @entered(i16 %x) {{\nb:\n  %y0 = add i16 %x, 1\n{body}  ret i16 %y11\n}}\n\ndefine i16 @f() {{\nb:\n  %r = call i16 @entered(i16 3)\n  %s = call i16 @entered(i16 3)\n  %t = add i16 %r, %s\n  ret i16 %t\n}}\n"
    );
    let mut program = Program::new(vec![parsed(&text)], std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    program.exports.entries = ["entered".to_owned()].into();
    let roots = roots(&program);
    let mut modules = managers(&program, &mut ProgramAnalyses::default());
    optimized::<String>(
        &mut program,
        &mut modules,
        &roots,
        &OperationCosts::default(),
        None,
        0,
        1,
        Threshold::default(),
        &mut |_, _, _, _| Ok(()),
        &mut |_, _, function| Ok(function),
        &mut |_, _, _| Ok(()),
    )
    .unwrap();
    assert!(printed(&program.modules[0]).contains("  %y0 = add i16 %x, 1\n"), "{}", printed(&program.modules[0]));
}

#[test]
fn test_the_step_runs_as_a_program_pass() {
    let mut module = parsed(HELPERS);
    let mut manager = PassManager::default();
    manager.verify_each = true;
    let target =
        crate::testing::Tuned { costs: OperationCosts { call: 4, ..OperationCosts::default() }, ..Default::default() };
    manager.add_program(Interprocedural {
        pipeline: Box::new(|_, _, _, _| {}),
        specialise: Box::new(|_, _, function| function),
        proved: None,
        inline: Threshold::default(),
        rate: None,
        ranges: true,
    });
    let stages = manager.run_module(&mut module, std::rc::Rc::new(target)).unwrap();
    // The helpers every call of which went lose their bodies: changed, and no
    // pipeline for them.
    let changed = stages.iter().map(|stage| stage.function).collect::<BTreeSet<_>>();
    assert!(changed.contains(&module.named("f").unwrap()));
    for name in ["scale", "clamp"] {
        let helper = module.global(module.named(name).unwrap()).function().unwrap();
        assert_eq!(helper.walk().count(), 1, "@{name} kept its body: {}", printed(&module));
    }
    assert_eq!(results(&module, INPUTS), results(&parsed(HELPERS), INPUTS));
}

#[test]
fn test_a_size_build_weighs_bytes_not_clocks() {
    // A nine-operation body at three sites: cheaper than three calls in clocks,
    // dearer in bytes (the callers come to more with the body copied than
    // with three calls, though the body goes). -Os copied it and grew the
    // code.
    let text = "define i16 @mix(i16 %a, i16 %b) {
b:
  %t0 = xor i16 %a, %b
  %t1 = shl i16 %a, 3
  %t2 = add i16 %t0, %t1
  %t3 = lshr i16 %b, 2
  %t4 = sub i16 %t2, %t3
  %t5 = and i16 %t4, 2047
  %t6 = or i16 %t5, %a
  %t7 = xor i16 %t6, %b
  %t8 = add i16 %t7, 5
  ret i16 %t8
}

define i16 @f(i16 %x, i16 %y) {
b:
  %p = call i16 @mix(i16 %x, i16 %y)
  %q = call i16 @mix(i16 %y, i16 %x)
  %r = call i16 @mix(i16 %p, i16 %q)
  ret i16 %r
}
";
    let calls = |rate: Option<i64>| {
        let mut module = parsed(text);
        let mut manager = PassManager::default();
        let target = crate::testing::Tuned {
            costs: OperationCosts { call: 20, ..OperationCosts::default() },
            sizes: OperationCosts { call: 3, ..OperationCosts::default() },
            ..Default::default()
        };
        manager.add_program(Interprocedural {
            pipeline: Box::new(|_, _, _, _| {}),
            specialise: Box::new(|_, _, function| function),
            proved: None,
            inline: Threshold::default(),
            rate,
            ranges: true,
        });
        manager.run_module(&mut module, std::rc::Rc::new(target)).unwrap();
        printed(&module).matches("call i16 @mix").count()
    };
    // Weighing clocks alone inlines every site; weighing bytes puts them back,
    // at -Os and at -O2 (QCport -O2 ran 11 KB past BCC's code, and out of
    // memory), unless the clocks saved pay for the bytes.
    assert_eq!((calls(None), calls(Some(0)), calls(Some(1))), (0, 3, 0));
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
            "define internal i16 @read(ptr nocapture readonly %p) memory(argmem: read) willreturn norecurse {",
            "define internal void @write(ptr nocapture writeonly initializes((0, 2)) %p, i16 %x) memory(argmem: write) willreturn norecurse {",
            "define internal void @keep(ptr %p) memory(write, argmem: none, inaccessiblemem: none) willreturn nounwind norecurse {",
            "define internal void @calls(ptr nocapture initializes((0, 2)) %p, i16 %x) memory(write, argmem: readwrite, inaccessiblemem: none) willreturn norecurse {",
            "define internal void @hides(ptr %p) {",
            "define internal void @ordered() memory(inaccessiblemem: readwrite) willreturn norecurse {",
            "define i16 @f(i16 %a) memory(readwrite, argmem: none, inaccessiblemem: none) willreturn norecurse {",
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
        uses.walk()
            .filter_map(|(_, inst)| llrm_mir::memory::callee(&module.context, uses, inst))
            .map(|id| module.global(id).name.clone().unwrap())
            .collect::<BTreeSet<_>>()
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
        lines.extend(
            text.lines()
                .filter(|line| line.starts_with("define"))
                .map(|line| format!("{name} {}", line.trim_end_matches(" {"))),
        );
    }
    let found = lines.join("\n") + "\n";
    // `STAMP_WRITE=1` rewrites the file where a rule changes on purpose.
    if std::env::var_os("STAMP_WRITE").is_some() {
        std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/src/interprocedural_stamped.txt"), &found).unwrap();
    }
    let expected = include_str!("interprocedural_stamped.txt");
    let changed = found.lines().zip(expected.lines()).find(|(one, other)| one != other);
    assert!(
        found == expected,
        "first difference: {changed:?}; {} lines, expected {}",
        found.lines().count(),
        expected.lines().count()
    );
}

/// Which defined functions of `text` the stamp gives `fact`.
fn stamped_with(
    text: &str,
    fact: &str,
) -> Vec<String> {
    let mut module = parsed(text);
    crate::testing::stamped(&mut module).unwrap();
    let mut found = printed(&module)
        .lines()
        .filter(|line| {
            line.starts_with("define")
                && line.split(" {").next().unwrap_or_default().split_whitespace().any(|word| word == fact)
        })
        .filter_map(|line| line.split('@').nth(1).and_then(|rest| rest.split('(').next()).map(str::to_owned))
        .collect::<Vec<_>>();
    found.sort();
    found
}

/// A function that nothing can enter while it runs says so: a leaf, one that
/// calls a `nocallback` declaration or an intrinsic. Not one that calls itself,
/// its mutual caller, an unbounded pointer, a declaration that may call back,
/// or anything reaching those. The stamp the compilers run was never given
/// this: the first inference lived where no compile reaches.
#[test]
fn the_stamp_infers_norecurse_where_nothing_can_reenter() {
    let text = "declare void @quiet() nocallback
declare void @loud()
declare i16 @llvm.smax.i16(i16, i16)

define void @leaf() {
b0:
  ret void
}

define void @calls_quiet() {
b0:
  call void @quiet()
  ret void
}

define void @calls_loud() {
b0:
  call void @loud()
  ret void
}

define i16 @calls_intrinsic(i16 %c) {
b0:
  %m = call i16 @llvm.smax.i16(i16 %c, i16 0)
  ret i16 %m
}

define void @self() {
b0:
  call void @self()
  ret void
}

define void @ping() {
b0:
  call void @pong()
  ret void
}

define void @pong() {
b0:
  call void @ping()
  ret void
}

define void @pointer(ptr %p) {
b0:
  call void %p()
  ret void
}

define void @listed(ptr %p) {
b0:
  call void %p(), !callees !0
  ret void
}

define void @through() {
b0:
  call void @pointer(ptr null)
  ret void
}

!0 = !{ptr @leaf}
";
    assert_eq!(stamped_with(text, "norecurse"), ["calls_intrinsic", "calls_quiet", "leaf", "listed"]);
}

fn spin(
    attrs: &str,
    load: &str,
    marks: &str,
) -> String {
    format!(
        "define i16 @spin(ptr %p) {attrs} {{
b0:
  br label %b1

b1:
  %v = {load} i16, ptr %p
  %more = icmp ne i16 %v, 0
  br i1 %more, label %b1, label %b2, !llvm.loop !0

b2:
  ret i16 0
}}

{marks}"
    )
}

/// A loop no counter bounds ended only where the language says it must, of
/// every loop (`mustprogress` on the function) or of the loop (C11 6.8.5p6:
/// clang marks each loop whose controlling expression is not constant). An
/// observable loop (a volatile load) may legally run forever, and so may a loop
/// no language marks.
#[test]
fn the_stamp_takes_the_languages_word_that_a_loop_ends() {
    let marked = "!0 = distinct !{!0, !1}\n!1 = !{!\"llvm.loop.mustprogress\"}\n";
    let unmarked = "!0 = distinct !{!0}\n";
    let ends = |text: String| stamped_with(&text, "willreturn") == ["spin"];
    assert!(ends(spin("mustprogress", "load", unmarked)), "the language says so of every loop");
    assert!(ends(spin("", "load", marked)), "the loop says so");
    assert!(!ends(spin("", "load", unmarked)), "no promise: an uncounted loop may not end");
    assert!(!ends(spin("mustprogress", "load volatile", unmarked)), "an observable loop may run forever");
    assert!(!ends(spin("", "load volatile", marked)));
}

/// `for (;;)` hangs: only a loop whose controlling expression is not constant
/// may be assumed to end, so a function with one is never `willreturn`,
/// whatever else is marked, and a loop with no edge out never ends however it
/// is marked.
#[test]
fn a_loop_with_no_exit_is_never_taken_to_end() {
    let function = |second: &str| {
        format!(
            "define void @f(ptr %p) {{
b0:
  br label %first

first:
  %v = load i16, ptr %p
  %more = icmp ne i16 %v, 0
  br i1 %more, label %first, label %next, !llvm.loop !0

next:
  br label %second

second:
{second}
done:
  ret void
}}

!0 = distinct !{{!0, !1}}
!1 = !{{!\"llvm.loop.mustprogress\"}}
!2 = distinct !{{!2, !1}}
"
        )
    };
    let conditional = "  %w = load i16, ptr %p\n  %again = icmp ne i16 %w, 0\n  br i1 %again, label %second, label %done, !llvm.loop !2\n";
    let forever = "  %w = load i16, ptr %p\n  br label %second, !llvm.loop !2\n";
    assert_eq!(stamped_with(&function(conditional), "willreturn"), ["f"], "both loops marked, both leave");
    assert!(
        stamped_with(&function(forever), "willreturn").is_empty(),
        "an unconditional loop, even one something marked"
    );
}

/// A call the byte price refuses and the clocks admit stays inlined where the
/// callers and the callee that goes come to no more (speaker.nib: `now` at two
/// sites, the loop then in registers, -30 bytes), and is put back where they do
/// (the test above).
#[test]
fn test_what_only_the_clocks_admit_is_kept_where_the_callee_going_pays_for_it() {
    let text = "define internal i16 @triple(i16 %a) {
b:
  %t0 = add i16 %a, %a
  %t1 = add i16 %t0, %a
  %t2 = xor i16 %t1, 7
  ret i16 %t2
}

define i16 @f(i16 %x, i16 %y) {
b:
  %p = call i16 @triple(i16 %x)
  %q = call i16 @triple(i16 %y)
  %r = add i16 %p, %q
  ret i16 %r
}
";
    let calls = |size: bool| {
        let mut module = parsed(text);
        let mut manager = PassManager::default();
        let target = crate::testing::Tuned {
            costs: OperationCosts { call: 20, ..OperationCosts::default() },
            sizes: OperationCosts { call: 3, add: 4, ..OperationCosts::default() },
            ..Default::default()
        };
        manager.add_program(Interprocedural {
            pipeline: Box::new(|_, _, _, _| {}),
            specialise: Box::new(|_, _, function| function),
            proved: None,
            inline: Threshold::default(),
            rate: size.then_some(0),
            ranges: true,
        });
        manager.run_module(&mut module, std::rc::Rc::new(target)).unwrap();
        printed(&module).matches("call i16 @triple").count()
    };
    assert_eq!((calls(false), calls(true)), (0, 0));
}

/// Sites put back are not tried again: they are the same calls in the same body
/// every round, and each try re-ran the caller's pipeline (mdl_ai.c: 59 s
/// against 28 s for the same code).
#[test]
fn test_sites_the_trial_put_back_are_not_tried_again() {
    let text = "define internal i16 @mix(i16 %a, i16 %b) {
b:
  %t0 = xor i16 %a, %b
  %t1 = shl i16 %a, 3
  %t2 = add i16 %t0, %t1
  %t3 = lshr i16 %b, 2
  %t4 = sub i16 %t2, %t3
  %t5 = and i16 %t4, 2047
  %t6 = or i16 %t5, %a
  %t7 = xor i16 %t6, %b
  %t8 = add i16 %t7, 5
  ret i16 %t8
}

define i16 @f(i16 %x, i16 %y) {
b:
  %p = call i16 @mix(i16 %x, i16 %y)
  %q = call i16 @mix(i16 %y, i16 %x)
  %r = add i16 %p, %q
  ret i16 %r
}
";
    let mut module = parsed(text);
    let layout = llrm_mir::datalayout::DataLayout::default();
    let clocks = OperationCosts { call: 20, ..OperationCosts::default() };
    let bytes = OperationCosts { call: 3, add: 6, ..OperationCosts::default() };
    let (mix, f) = (module.named("mix").unwrap(), module.named("f").unwrap());
    let counts = inline::call_counts(&module);
    let candidates = inline::candidates(
        &module,
        &llrm_mir::memory::callees(&module),
        &layout,
        &counts,
        &BTreeSet::from([mix]),
        &clocks,
        20,
        Threshold::default(),
    );
    let calls: Vec<_> = module
        .global(f)
        .function()
        .unwrap()
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| llrm_mir::memory::callee(&module.context, module.global(f).function().unwrap(), inst).is_some())
        .collect();
    let sites: llrm_support::hash::IndexMap<_, _> =
        calls.iter().map(|&call| (call, candidates[&mix].clone())).collect();
    let mut analyses = ModuleAnalyses::of(&module, std::rc::Rc::new(llrm_mir::target::Neutral));
    let runs = std::cell::Cell::new(0);
    let mut refused = BTreeSet::new();
    let again = |module: &mut Module, analyses: &mut ModuleAnalyses, refused: &mut BTreeSet<_>| {
        together_trial::<String>(
            module,
            analyses,
            &layout,
            &BTreeSet::from([mix]),
            &BTreeSet::new(),
            &Default::default(),
            f,
            &sites,
            &counts,
            &bytes,
            (&OperationCosts::default(), 0),
            refused,
            &mut |_, _, _, _| {
                runs.set(runs.get() + 1);
                Ok(())
            },
        )
        .unwrap()
    };
    assert!(!again(&mut module, &mut analyses, &mut refused), "putting it back stays nothing");
    let first = runs.get();
    again(&mut module, &mut analyses, &mut refused);
    assert_eq!((first > 0, runs.get()), (true, first), "the second round runs no pipeline: {refused:?}");
}

/// A caller of many callees was spliced into and put through the pipeline once
/// a callee, as a "trial" of each (the `callers` axis at N=64: 63 runs of the
/// big body, each O(its size), 3,404 Minstr of the compile's 3,800). The sites
/// the estimates refuse are tried together, once.
#[test]
fn test_a_caller_of_many_callees_is_put_through_the_pipeline_a_bounded_number_of_times() {
    const CALLEES: usize = 12;
    let body = |name: String| {
        format!(
            "define i16 @{name}(i16 %a, i16 %b) {{\nb:\n  %t0 = xor i16 %a, %b\n  %t1 = shl i16 %a, 3\n  %t2 = add i16 %t0, %t1\n  %t3 = lshr i16 %b, 2\n  %t4 = sub i16 %t2, %t3\n  %t5 = and i16 %t4, 2047\n  %t6 = or i16 %t5, %a\n  %t7 = xor i16 %t6, %b\n  %t8 = add i16 %t7, 5\n  ret i16 %t8\n}}\n\n"
        )
    };
    let mut text: String = (0..CALLEES).map(|at| body(format!("g{at}"))).collect();
    text.push_str("define i16 @f(i16 %x, i16 %y) {\nb:\n  %c0 = add i16 %x, %y\n");
    for at in 0..CALLEES {
        text.push_str(&format!("  %c{} = call i16 @g{at}(i16 %c{at}, i16 %y)\n", at + 1));
    }
    text.push_str(&format!("  ret i16 %c{CALLEES}\n}}\n"));
    let mut program = Program::new(vec![parsed(&text)], std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let roots = roots(&program);
    let mut modules = managers(&program, &mut ProgramAnalyses::default());
    let clocks = OperationCosts { call: 20, ..OperationCosts::default() };
    let bytes = OperationCosts { call: 3, add: 6, ..OperationCosts::default() };
    let runs = std::cell::Cell::new(0);
    optimized::<String>(
        &mut program,
        &mut modules,
        &roots,
        &bytes,
        Some(&clocks),
        1,
        20,
        Threshold::default(),
        &mut |module, _, id, _| {
            if module.global(id).name.as_deref() == Some("f") {
                runs.set(runs.get() + 1);
            }
            Ok(())
        },
        &mut |_, _, function| Ok(function),
        &mut |_, _, _| Ok(()),
    )
    .unwrap();
    assert!(runs.get() <= 3, "@f went through the pipeline {} times for {CALLEES} callees", runs.get());
}

/// A callee with the constants a site passes is run through the pipeline once,
/// whatever number of sites pass them, and again only for another body or other
/// constants: the fact that (callee, constants) comes to so many bytes, kept
/// for the sites that ask and for a decision about a copy for those constants.
#[test]
fn test_a_callee_is_specialised_once_for_the_constants_its_sites_pass() {
    let mut module = parsed(
        "define i16 @mix(i16 %a, i16 %b) {\nb:\n  %t0 = xor i16 %a, %b\n  %t1 = shl i16 %a, 3\n  %t2 = add i16 %t0, %t1\n  ret i16 %t2\n}\n",
    );
    let mix = module.named("mix").unwrap();
    let mut modules = ModuleAnalyses::of(&module, std::rc::Rc::new(llrm_mir::target::Neutral));
    let three = {
        let ty = module.context.types.int(16);
        module.context.int(ty, 3)
    };
    let four = {
        let ty = module.context.types.int(16);
        module.context.int(ty, 4)
    };
    let runs = std::cell::Cell::new(0);
    let mut memo = Specialisations::default();
    let costs = OperationCosts { call: 3, add: 6, ..OperationCosts::default() };
    let mut bytes = |constants: &[Option<llrm_mir::context::ConstantId>]| {
        memo.bytes::<String>(&mut module, &mut modules, mix, constants, &costs, &mut |_, _, function| {
            runs.set(runs.get() + 1);
            Ok(function)
        })
        .unwrap()
    };
    let (first, again) = (bytes(&[Some(three), None]), bytes(&[Some(three), None]));
    assert_eq!((first, runs.get()), (again, 1), "the second site made the specialisation again");
    bytes(&[Some(four), None]);
    assert_eq!(runs.get(), 2, "other constants share the first's");
}

/// A pass that rewrites the calls of the function it changes (dead arguments,
/// argument promotion) changed the callers too and reported the one function:
/// their analyses were read as they stood. `reporting` says every body whose
/// history moved.
#[test]
fn test_a_body_a_pass_changed_without_saying_so_is_reported() {
    let mut module = parsed(
        "define internal i16 @g(i16 %a, i16 %unused) {\nb:\n  ret i16 %a\n}\n\ndefine i16 @f(i16 %x) {\nb:\n  %r = call i16 @g(i16 %x, i16 %x)\n  ret i16 %r\n}\n",
    );
    let (g, f) = (module.named("g").unwrap(), module.named("f").unwrap());
    let mut modules = ModuleAnalyses::of(&module, std::rc::Rc::new(llrm_mir::target::Neutral));
    modules.function::<llrm_analysis::cfg::Shape>(&module, f);
    modules.function::<llrm_analysis::cfg::Shape>(&module, g);
    reporting(&mut module, &mut modules, |module| {
        let call = {
            let body = module.global(f).function().unwrap();
            body.walk()
                .map(|(_, inst)| inst)
                .find(|&inst| llrm_mir::memory::callee(&module.context, body, inst) == Some(g))
                .unwrap()
        };
        let operand = module.global(f).function().unwrap().instruction(call).operands[0];
        function_mut(module, f).1.set_operand(call, 1, operand);
    });
    assert!(
        modules.cached_function::<llrm_analysis::cfg::Shape>(f).is_none(),
        "the edited caller's analyses were kept"
    );
    assert!(modules.cached_function::<llrm_analysis::cfg::Shape>(g).is_some(), "an untouched body lost its analyses");
}

/// A body whose address is taken is also called through it, with actuals no
/// site names: its one direct call passed 5, and the call through the
/// pointer stored 5 as well.
#[test]
fn test_a_body_whose_address_is_taken_keeps_its_parameters() {
    let text = STORES
        .replace("@g = global i16 0\n", "@g = global i16 0\n@slot = global ptr @set\n")
        .replace(
            "  call void @set(i16 5)\n  call void @set(i16 5)\n",
            "  call void @set(i16 5)\n  %p = load ptr, ptr @slot\n  call void %p(i16 %a)\n",
        );
    let mut module = parsed(&text);
    stepped(&mut module, &["f"], 40, Threshold::none());
    let after = printed(&module);
    assert!(after.contains("  store i16 %x, ptr @g\n"), "{after}");
}

/// queens' `place(q, row, n)` recurses with `row + 1` and its own `n`: the
/// one outside call passes 7, so every call does, and `n` is 7 inside.
/// The recursive call named `%n` as a second value for it, so none was found.
fn recursive(
    first: &str,
    second: &str,
) -> String {
    format!(
        "define internal i16 @place(i16 %row, i16 %n) {{
b:
  %done = icmp eq i16 %row, %n
  br i1 %done, label %leaf, label %more
more:
  %next = add i16 %row, 1
  %r = call i16 @place({first})
  %s = add i16 %r, %row
  ret i16 %s
leaf:
  ret i16 1
}}

define i16 @f(i16 %a) {{
b:
  %x = call i16 @place(i16 0, i16 7)
{second}  ret i16 %x
}}
"
    )
}

#[test]
fn test_a_recursive_call_passing_a_parameter_on_leaves_the_others_actuals_agreed() {
    let mut module = parsed(&recursive("i16 %next, i16 %n", ""));
    stepped(&mut module, &["f"], 40, Threshold::none());
    let text = printed(&module);
    assert!(text.contains("icmp eq i16 %row, 7"), "{text}");
}

/// A second outside call with another value, or the recursion passing the
/// parameters swapped, keeps `n` unknown.
#[test]
fn test_another_actual_for_the_parameter_keeps_it_unknown() {
    for text in
        [recursive("i16 %next, i16 %n", "  %y = call i16 @place(i16 0, i16 %a)\n"), recursive("i16 %n, i16 %next", "")]
    {
        let mut module = parsed(&text);
        stepped(&mut module, &["f"], 40, Threshold::none());
        let after = printed(&module);
        assert!(after.contains("icmp eq i16 %row, %n"), "{after}");
    }
}

/// queens' `place`: `row` from 0 and from `row + 1` below `n`, the calls
/// known to be those of the program: `row` is in 0 to 7, which `row <u 12`, the
/// length's check, then leaves nothing to decide (`decide` reads the stamp).
fn bounded_recursion(extra: &str) -> String {
    format!(
        "define internal i16 @place(i16 %row, i16 %n) {{
b:
  %done = icmp eq i16 %row, %n
  br i1 %done, label %leaf, label %more
more:
  %fits = icmp ult i16 %row, 12
  br i1 %fits, label %next, label %crash
next:
  %up = add nsw i16 %row, 1
  %r = call i16 @place(i16 %up, i16 %n)
  ret i16 %r
leaf:
  ret i16 1
crash:
  ret i16 99
}}

define i16 @f(i16 %a) {{
b:
  %x = call i16 @place(i16 0, i16 7)
{extra}  ret i16 %x
}}
"
    )
}

#[test]
fn test_what_the_callers_pass_bounds_a_parameter_the_body_checks() {
    let mut module = parsed(&bounded_recursion(""));
    assert!(printed(&module).contains("icmp ult i16 %row, 12"), "premise");
    stepped(&mut module, &["f"], 40, Threshold::none());
    let text = printed(&module);
    assert!(text.contains("range(i16 0, 8) %row"), "{text}");
}

/// A call from elsewhere with a value of its own, or through a pointer, leaves
/// the parameter unbounded.
#[test]
fn test_another_caller_leaves_the_parameter_unbounded() {
    let mut module = parsed(&bounded_recursion("  %y = call i16 @place(i16 %a, i16 7)\n"));
    stepped(&mut module, &["f"], 40, Threshold::none());
    let text = printed(&module);
    assert!(text.contains("icmp ult i16 %row, 12"), "{text}");
}

/// A function called with a constant is copied for it at -O3 (gcc's
/// `-fipa-cp-clone`): `g`'s loop runs `%k` trips, which the copies for 4 and
/// for 5 know. gcc's -O3 queens is eight such copies of its recursive `place`,
/// one a row.
const TWO_CONTEXTS: &str = "define i16 @g(i16 %k, i16 %x) {
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %i1, %body ]
  %acc = phi i16 [ 0, %b0 ], [ %acc1, %body ]
  %go = icmp slt i16 %i, %k
  br i1 %go, label %body, label %done

body:
  %m = mul i16 %x, %i
  %acc1 = add i16 %acc, %m
  %i1 = add nsw i16 %i, 1
  br label %head

done:
  ret i16 %acc
}

define i16 @f(i16 %x) {
b0:
  %a = call i16 @g(i16 4, i16 %x)
  %b = call i16 @g(i16 5, i16 %x)
  %s = add i16 %a, %b
  ret i16 %s
}
";

#[test]
fn test_a_function_called_with_two_constants_is_cloned_for_each_at_o3() {
    let inputs: &[&[i128]] = &[&[0], &[1], &[7], &[-3]];
    let cloned = |clone: bool| {
        let mut module = parsed(TWO_CONTEXTS);
        stepped(&mut module, &["f", "g"], 20, Threshold { cp_clone: clone, ..Threshold::none() });
        assert_eq!(results(&module, inputs), results(&parsed(TWO_CONTEXTS), inputs), "{}", printed(&module));
        (module.named("g.constprop.1").is_some(), module.named("g.constprop.2").is_some())
    };
    assert_eq!(cloned(false), (false, false));
    assert_eq!(cloned(true), (true, true));
}

/// A small function that calls itself is given copies of itself (gcc's
/// `recursive_inlining`, `max-inline-recursive-depth-auto` 8 and
/// `-insns-recursive-auto` 450): `hanoi` at -O2 was one call per move, gcc's is
/// eight levels in one body.
const COUNT: &str = "define i16 @f(i16 %n) {
b0:
  %z = icmp eq i16 %n, 0
  br i1 %z, label %done, label %rec

rec:
  %m = sub i16 %n, 1
  %a = call i16 @f(i16 %m)
  %b = call i16 @f(i16 %m)
  %s = add i16 %a, %b
  %t = add i16 %s, 1
  ret i16 %t

done:
  ret i16 0
}
";

#[test]
fn test_a_small_recursive_function_is_inlined_into_itself_to_a_depth() {
    let inputs: &[&[i128]] = &[&[0], &[1], &[3], &[6]];
    let calls = |threshold: Threshold| {
        let mut module = parsed(COUNT);
        stepped(&mut module, &["f"], 20, threshold);
        assert_eq!(results(&module, inputs), results(&parsed(COUNT), inputs), "{}", printed(&module));
        printed(&module).matches("call i16 @f").count()
    };
    assert_eq!(calls(Threshold::none()), 2, "no inlining at all: the two calls it began with");
    assert!(calls(Threshold::default()) > 2, "the body grew by copies of itself");
    assert_eq!(calls(Threshold::default().for_size()), 2, "not for size: the recursive call is cold there");
}

/// A trial of several sites splices them all and runs the caller's
/// pipeline once, the way gcc and LLVM inline: it ran the pipeline after each
/// site (host.c -6.6%, QCport -2.2%, the code the same).
#[test]
fn test_a_trial_of_several_sites_runs_the_callers_pipeline_once() {
    let text = "define internal i16 @mix(i16 %a, i16 %b) {
b:
  %t0 = xor i16 %a, %b
  %t1 = shl i16 %a, 3
  %t2 = add i16 %t0, %t1
  %t3 = lshr i16 %b, 2
  %t4 = sub i16 %t2, %t3
  %t5 = and i16 %t4, 2047
  %t6 = or i16 %t5, %a
  %t7 = xor i16 %t6, %b
  %t8 = add i16 %t7, 5
  ret i16 %t8
}

define i16 @f(i16 %x, i16 %y) {
b:
  %p = call i16 @mix(i16 %x, i16 %y)
  %q = call i16 @mix(i16 %y, i16 %x)
  %r = add i16 %p, %q
  ret i16 %r
}
";
    let mut module = parsed(text);
    let layout = llrm_mir::datalayout::DataLayout::default();
    let clocks = OperationCosts { call: 20, ..OperationCosts::default() };
    let bytes = OperationCosts { call: 3, add: 6, ..OperationCosts::default() };
    let mix = module.named("mix").unwrap();
    let counts = inline::call_counts(&module);
    let candidates = inline::candidates(
        &module,
        &llrm_mir::memory::callees(&module),
        &layout,
        &counts,
        &BTreeSet::from([mix]),
        &clocks,
        20,
        Threshold::default(),
    );
    let f = module.named("f").unwrap();
    let calls: Vec<_> = module
        .global(f)
        .function()
        .unwrap()
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| {
            llrm_mir::memory::callee(&module.context, module.global(f).function().unwrap(), inst) == Some(mix)
        })
        .collect();
    let sites: llrm_support::hash::IndexMap<_, _> =
        calls.iter().map(|&call| (call, candidates[&mix].clone())).collect();
    let mut analyses = ModuleAnalyses::of(&module, std::rc::Rc::new(llrm_mir::target::Neutral));
    let runs = std::cell::Cell::new(0);
    let mut refused = BTreeSet::new();
    together_trial::<String>(
        &mut module,
        &mut analyses,
        &layout,
        &BTreeSet::from([mix]),
        &BTreeSet::new(),
        &Default::default(),
        f,
        &sites,
        &counts,
        &bytes,
        (&OperationCosts::default(), 0),
        &mut refused,
        &mut |_, _, _, _| {
            runs.set(runs.get() + 1);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(runs.get(), 1, "the pipeline ran once for each of the callee's two sites");
}

/// A round assumes a parameter's range before it proves it (`place`'s `row` is
/// 0 at first), and under it a loop `r < row` that runs from 0 is never
/// entered: the facts of the loop around it and of its own contradict each
/// other there, and a call in it passes nothing. Passing what the contradiction
/// left (`r` below 0) made the callee's parameter range wide for good.
#[test]
fn test_a_call_in_a_block_the_assumed_range_makes_unreachable_passes_nothing() {
    let mut module = parsed(
        "define internal i16 @sink(i16 %x) {
b:
  %y = add i16 %x, 1
  ret i16 %y
}

define internal i16 @place(i16 %row, i16 %n) {
b:
  %done = icmp eq i16 %row, %n
  br i1 %done, label %leaf, label %scan
scan:
  br label %outer
outer:
  %c = phi i16 [ 0, %scan ], [ %c1, %latch ]
  %more = icmp slt i16 %c, %n
  br i1 %more, label %pre, label %next
pre:
  br label %head
head:
  %r = phi i16 [ 0, %pre ], [ %r1, %body ]
  %go = icmp slt i16 %r, %row
  br i1 %go, label %body, label %latch
body:
  %s = call i16 @sink(i16 %r)
  %r1 = add nsw i16 %r, 1
  br label %head
latch:
  %c1 = add nsw i16 %c, 1
  br label %outer
next:
  %up = add nsw i16 %row, 1
  %p = call i16 @place(i16 %up, i16 %n)
  ret i16 %p
leaf:
  ret i16 1
}

define i16 @f(i16 %a) {
b:
  %x = call i16 @place(i16 0, i16 7)
  ret i16 %x
}
",
    );
    stepped(&mut module, &["f"], 40, Threshold::none());
    let text = printed(&module);
    assert!(text.contains("range(i16 0, ") && text.contains("%x)"), "{text}");
    let sink = text.lines().find(|line| line.contains("@sink(")).unwrap_or_default();
    assert!(!sink.contains("range(i16 -"), "{sink}\n{text}");
}

/// gcc builds only the bodies that survive: a private function whose last call
/// was inlined is not inlined into or run through the pipeline again (a chain
/// of N: every body held the ones below it, N squared).
#[test]
fn test_a_private_function_whose_last_call_went_is_not_built() {
    let mut text = String::from("define internal i16 @h0(i16 %x) {\nb1:\n  %y = mul i16 %x, 3\n  ret i16 %y\n}\n");
    for at in 1..12 {
        text.push_str(&format!(
            "\ndefine internal i16 @h{at}(i16 %x) {{\nb1:\n  %a = add i16 %x, {at}\n  %c = call i16 @h{}(i16 %a)\n  %y = xor i16 %c, %x\n  ret i16 %y\n}}\n",
            at - 1
        ));
    }
    text.push_str("\ndefine i16 @f(i16 %x) {\nb1:\n  %r = call i16 @h11(i16 %x)\n  ret i16 %r\n}\n");
    let mut module = parsed(&text);
    let (proved, stages) = step(&mut module, &["f"], 4);
    assert_eq!(proved.reachable, defined(&module, &["f"]));
    let built: BTreeSet<&str> = stages.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(built, BTreeSet::from(["f"]), "{stages:?}");
    assert_eq!(results(&module, INPUTS), results(&parsed(&text), INPUTS));
}

/// A function only an invoke or a `!callees` list names is still called.
#[test]
fn test_a_function_only_an_invoke_calls_is_not_dead() {
    let text = "declare i32 @personality(...)

define internal void @handler() {
b1:
  call void @sink(i16 1)
  ret void
}

declare void @sink(i16)

define void @top() personality ptr @personality {
b1:
  invoke void @handler() to label %b2 unwind label %b3

b2:
  ret void

b3:
  %pad = landingpad { ptr, i32 } cleanup
  resume { ptr, i32 } %pad
}
";
    let mut module = parsed(text);
    step(&mut module, &["top"], 4);
    let handler = module.named("handler").expect("kept");
    let body = module.global(handler).function().expect("a function");
    assert!(
        body.walk().any(|(_, inst)| matches!(body.instruction(inst).opcode, Opcode::Call(_))),
        "the body an invoke reaches was emptied: {}",
        printed(&module)
    );
}

/// A body held only to inline from (`available_externally`) is inlined in the
/// plain round at any size it is a candidate at: it is emitted nowhere, so no
/// copy adds to what the program had. (The plain round used to leave it to the
/// trial, which the round without clocks never runs.)
#[test]
fn test_a_held_body_is_inlined_in_the_plain_round() {
    let text = "define available_externally i16 @held(i16 %x) {
b1:
  %a = shl i16 %x, 2
  %b = add i16 %a, 1
  %c = xor i16 %b, %x
  %d = add i16 %c, 7
  %e = mul i16 %d, 3
  %f = sub i16 %e, %x
  ret i16 %f
}

define i16 @f(i16 %p, i16 %q) {
b1:
  %r = call i16 @held(i16 %p)
  %s = call i16 @held(i16 %q)
  %t = call i16 @held(i16 %r)
  %u = add i16 %s, %t
  ret i16 %u
}
";
    let mut module = parsed(text);
    step(&mut module, &["f"], 4);
    let printed = printed(&module);
    assert!(!printed[printed.find("define i16 @f").unwrap()..].contains("call "), "{printed}");
}

/// A chain longer than a caller may grow to is taken in pieces, and only the
/// function that starts a piece is built: the ones it took are called by none,
/// so none is built (a chain of N: each was built with the next ones in it).
#[test]
fn test_a_chain_longer_than_a_caller_may_take_is_built_only_where_each_piece_starts() {
    let mut text = String::from("define internal i16 @h0(i16 %x) {\nb1:\n  %y = mul i16 %x, 3\n  ret i16 %y\n}\n");
    for at in 1..90 {
        text.push_str(&format!(
            "\ndefine internal i16 @h{at}(i16 %x) {{\nb1:\n  %a = add i16 %x, {at}\n  %c = call i16 @h{}(i16 %a)\n  %y = xor i16 %c, %x\n  ret i16 %y\n}}\n",
            at - 1
        ));
    }
    text.push_str("\ndefine i16 @f(i16 %x) {\nb1:\n  %r = call i16 @h89(i16 %x)\n  ret i16 %r\n}\n");
    let mut module = parsed(&text);
    let (proved, stages) = step(&mut module, &["f"], 4);
    let alive: BTreeSet<&str> =
        proved.reachable.iter().map(|&(_, id)| module.global(id).name.as_deref().unwrap()).collect();
    let built: BTreeSet<&str> = stages.iter().map(|(name, _)| name.as_str()).collect();
    assert!(built.is_subset(&alive), "built and then taken: {:?}", built.difference(&alive).collect::<Vec<_>>());
    assert!(alive.len() > 1 && alive.len() <= 6, "{alive:?}");
    assert_eq!(results(&module, INPUTS), results(&parsed(&text), INPUTS));
}
