//! Behaviour of the whole-module step; the old module had no tests.

use std::collections::BTreeSet;

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
    // The second IPSCCP the promotion tests below rest on is a target's choice
    // (SD443): a target that asks for it.
    let target = crate::testing::Tuned { second_ipsccp: true, ..Default::default() };
    let proved = Program::lend(module, std::rc::Rc::new(target), |program| {
        let mut modules = managers(program, &mut ProgramAnalyses::default());
        optimized_with::<String>(
            program,
            &mut modules,
            &roots,
            &costs,
            None,
            0,
            threshold,
            &mut crate::ipacp::Cloning::default(),
            &mut |module, _, id, stage| {
                stages.push((module.global(id).name.owned().unwrap(), stage.to_owned()));
                Ok(())
            },
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
fn test_without_roots_every_procedure_is_reachable() {
    let mut module = parsed(HELPERS);
    let (proved, _) = step(&mut module, &[], 4);
    assert_eq!(proved.reachable, defined(&module, &["scale", "clamp", "f"]));
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
fn test_a_constant_return_is_carried_to_the_caller() {
    let mut module = parsed(STORES);
    // With no inlining: a callee that stores to memory is inlined otherwise.
    let (_, stages) = stepped(&mut module, &["f"], 40, Threshold::none());
    let text = printed(&module);
    assert!(text.contains("  %s = add i16 7, %a\n"), "{text}");
    // Both calls with effects stay.
    assert_eq!(text.matches("call ").count(), 3, "{text}");
    assert!(staged(&stages, "f", "ipa0."), "{stages:?}");
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
        printed(&program.modules[1])
            .contains("declare i16 @double(i16) memory(none) willreturn nounwind nofree nosync norecurse\n"),
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
            "define internal i16 @read(ptr nocapture nofree readonly \"llrm.reads\"=\"0:2\" \"llrm.writes\"=\"\" %p) memory(argmem: read) willreturn nounwind nofree nosync norecurse {",
            "define internal void @write(ptr nocapture nofree writeonly initializes((0, 2)) \"llrm.reads\"=\"\" \"llrm.writes\"=\"0:2\" %p, i16 %x) memory(argmem: write) willreturn nounwind nofree nosync norecurse {",
            "define internal void @keep(ptr %p) memory(write, argmem: none, inaccessiblemem: none) willreturn nounwind nofree nosync norecurse {",
            "define internal void @calls(ptr nocapture nofree initializes((0, 2)) \"llrm.reads\"=\"0:2\" \"llrm.writes\"=\"0:2\" %p, i16 %x) memory(write, argmem: readwrite, inaccessiblemem: none) willreturn nounwind nofree nosync norecurse {",
            "define internal void @hides(ptr %p) {",
            "define internal void @ordered() memory(inaccessiblemem: readwrite) willreturn nounwind nofree nosync norecurse {",
            "define i16 @f(i16 %a) memory(readwrite, argmem: write, inaccessiblemem: none) willreturn nounwind nofree nosync norecurse {",
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

/// A caller reloaded a record field after a call whose callee wrote two other
/// fields: the call was taken to write the whole record (qmove -Os, +8 clocks).
#[test]
fn a_field_is_kept_across_a_call_that_writes_other_fields_of_the_record() {
    let loads = |body: &str, frame: bool, which: u8| {
        let (decl, record) = if frame {
            ("", "%r = alloca [3 x i16]")
        } else {
            ("@r = global [3 x i16] zeroinitializer\n", "%r = getelementptr i8, ptr @r, i16 0")
        };
        let mut module = parsed(&format!(
            "{decl}

define internal void @fill(ptr %p, i16 %i) {{
b:
{body}
  ret void
}}

define i16 @f(i16 %c, i16 %i) {{
b:
  {record}
  %z = getelementptr i8, ptr %r, i16 4
  store i16 %c, ptr %z
  call void @fill(ptr %r, i16 %i)
  %v = load i16, ptr %z
  ret i16 %v
}}
"
        ));
        crate::testing::stamped(&mut module).unwrap();
        match which {
            0 => drop(managed(&mut module, Promote)),
            1 => drop(managed(&mut module, crate::gvn::Gvn::default())),
            _ => drop(managed(&mut module, crate::earlycse::EarlyCse::default())),
        }
        let f = module.function_mut("f").unwrap().1;
        f.walk().filter(|(_, inst)| matches!(f.instruction(*inst).opcode, Opcode::Load { .. })).count()
    };
    let x_and_y = "  store i16 1, ptr %p\n  %y = getelementptr i8, ptr %p, i16 2\n  store i16 2, ptr %y";
    for (frame, which) in [(false, 0), (true, 0), (true, 1), (true, 2)] {
        assert_eq!(loads(x_and_y, frame, which), 0, "{frame} {which}");
        let and_z = format!("{x_and_y}\n  %z = getelementptr i8, ptr %p, i16 4\n  store i16 3, ptr %z");
        assert_eq!(loads(&and_z, frame, which), 1);
        assert_eq!(loads("  %q = getelementptr i8, ptr %p, i16 %i\n  store i16 1, ptr %q", frame, which), 1);
    }
}

/// The bodies of `text` its stamp states pure, by name.
fn pure(text: &str) -> BTreeSet<String> {
    let mut module = parsed(text);
    crate::testing::stamped(&mut module).unwrap();
    facts::stated_pure(&module).into_iter().map(|id| module.global(id).name.owned().unwrap()).collect()
}

/// The callees whose unused calls in `@uses` of `text`, stamped, go.
fn dropped(text: &str) -> BTreeSet<String> {
    let mut module = parsed(text);
    crate::testing::stamped(&mut module).unwrap();
    let callees = |module: &Module| {
        let uses = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("uses")).expect("@uses").2;
        uses.walk()
            .filter_map(|(_, inst)| llrm_mir::memory::callee(&module.context, uses, inst))
            .map(|id| module.global(id).name.owned().unwrap())
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

/// A call that only reads, comes back and cannot unwind goes unnoticed, as in
/// LLVM (`wouldInstructionBeTriviallyDead`): a far, an external or a pointer's
/// read too, which llrm kept while `nounwind` also meant "cannot fault"
/// (`cannot_fault`, F1). A volatile one is ordered.
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
    assert_eq!(dropped(text), named(&["read", "caller", "far", "external", "pointer"]));
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
        stepped(&mut module, &["f", "g"], 20, Threshold { cp_clone: Some(clone), ..Threshold::none() });
        assert_eq!(results(&module, inputs), results(&parsed(TWO_CONTEXTS), inputs), "{}", printed(&module));
        (module.named("g.constprop.1").is_some(), module.named("g.constprop.2").is_some())
    };
    assert_eq!(cloned(false), (false, false));
    assert_eq!(cloned(true), (true, true));
}

/// A copy whose only gain is a known loop bound is gcc's -O3 (`-fipa-cp-clone`
/// is on there only); at -O2 on a target that copies there, the copy for
/// `pack_ref`'s four agreed constants kept the caller's mode loop from
/// unrolling: m32 x_spill_store_order 53,652 -> 76,462 clk.
#[test]
fn test_a_copy_that_only_makes_a_loop_bound_known_waits_for_o3() {
    let cloned = |hints: bool| {
        let mut module = parsed(TWO_CONTEXTS);
        let threshold = Threshold { cp_clone: Some(true), cp_hint_clones: hints, ..Threshold::none() };
        stepped(&mut module, &["f", "g"], 20, threshold);
        module.named("g.constprop.1").is_some()
    };
    assert_eq!((cloned(false), cloned(true)), (false, true));
}

/// A known bound the unroller cannot take whole is no reason to copy: gcc's
/// hint counts it, but m64 nbody -O2 went 655 -> 1189 B and recpow 1207 ->
/// 3060 B for a loop of 400 trips, so a target's default counts the hint up to
/// `max-completely-peel-times` (16) trips; a flag given, or -Omax, is gcc's own
/// (m16 recchop -O3 clones `chop(0, 64)`).
#[test]
fn test_a_loop_bound_past_the_peel_limit_is_no_reason_to_clone() {
    let cloned = |trips: &str| {
        let text = TWO_CONTEXTS.replace("i16 4,", &format!("i16 {trips},")).replace("i16 5,", &format!("i16 {trips},"));
        let mut module = parsed(&text);
        stepped(&mut module, &["f", "g"], 20, Threshold { cp_clone: Some(true), cp_peel_limited: true, ..Threshold::none() });
        module.named("g.constprop.1").is_some()
    };
    assert_eq!((cloned("4"), cloned("400")), (true, false));
}

/// `fib(20)` credited clones for 19 and 18 but counted only the first's size:
/// m64 fib -O2 grew 424 -> 595 B and 12.7 k -> 19.1 k clocks. A chain pays for
/// every clone it credits (gcc's `prop_size_cost`).
const BRANCHING: &str = "define i32 @fib(i32 %n) {
b0:
  %small = icmp slt i32 %n, 2
  br i1 %small, label %done, label %rec

rec:
  %a1 = sub i32 %n, 1
  %a = call i32 @fib(i32 %a1)
  %b1 = sub i32 %n, 2
  %b = call i32 @fib(i32 %b1)
  %m1 = mul i32 %n, %n
  %m2 = mul i32 %m1, %n
  %m3 = mul i32 %m2, %n
  %m4 = mul i32 %m3, %n
  %m5 = mul i32 %m4, %n
  %m6 = mul i32 %m5, %n
  %m7 = mul i32 %m6, %n
  %m8 = mul i32 %m7, %n
  %m9 = mul i32 %m8, %n
  %m10 = mul i32 %m9, %n
  %m11 = mul i32 %m10, %n
  %m12 = mul i32 %m11, %n
  %m13 = mul i32 %m12, %n
  %m14 = mul i32 %m13, %n
  %m15 = mul i32 %m14, %n
  %m16 = mul i32 %m15, %n
  %m17 = mul i32 %m16, %n
  %m18 = mul i32 %m17, %n
  %m19 = mul i32 %m18, %n
  %m20 = mul i32 %m19, %n
  %m21 = mul i32 %m20, %n
  %m22 = mul i32 %m21, %n
  %m23 = mul i32 %m22, %n
  %m24 = mul i32 %m23, %n
  %m25 = mul i32 %m24, %n
  %m26 = mul i32 %m25, %n
  %m27 = mul i32 %m26, %n
  %m28 = mul i32 %m27, %n
  %m29 = mul i32 %m28, %n
  %m30 = mul i32 %m29, %n
  %m31 = mul i32 %m30, %n
  %m32 = mul i32 %m31, %n
  %m33 = mul i32 %m32, %n
  %m34 = mul i32 %m33, %n
  %m35 = mul i32 %m34, %n
  %m36 = mul i32 %m35, %n
  %m37 = mul i32 %m36, %n
  %m38 = mul i32 %m37, %n
  %m39 = mul i32 %m38, %n
  %m40 = mul i32 %m39, %n
  %m41 = mul i32 %m40, %n
  %m42 = mul i32 %m41, %n
  %m43 = mul i32 %m42, %n
  %m44 = mul i32 %m43, %n
  %m45 = mul i32 %m44, %n
  %m46 = mul i32 %m45, %n
  %m47 = mul i32 %m46, %n
  %m48 = mul i32 %m47, %n
  %m49 = mul i32 %m48, %n
  %m50 = mul i32 %m49, %n
  %m51 = mul i32 %m50, %n
  %m52 = mul i32 %m51, %n
  %m53 = mul i32 %m52, %n
  %m54 = mul i32 %m53, %n
  %m55 = mul i32 %m54, %n
  %m56 = mul i32 %m55, %n
  %m57 = mul i32 %m56, %n
  %m58 = mul i32 %m57, %n
  %m59 = mul i32 %m58, %n
  %m60 = mul i32 %m59, %n
  %s0 = add i32 %a, %b
  %s = add i32 %s0, %m60
  ret i32 %s

done:
  ret i32 %n
}

define i32 @f(i32 %x) {
b0:
  %r = call i32 @fib(i32 20)
  ret i32 %r
}
";

#[test]
fn test_a_branching_recursion_called_once_is_not_cloned_down_a_chain() {
    let mut module = parsed(BRANCHING);
    stepped(&mut module, &["f", "fib"], 20, Threshold { cp_clone: Some(true), ..Threshold::none() });
    assert!(module.named("fib.constprop.1").is_none(), "{}", printed(&module));
}

/// A copy for a context two call sites pass has two callers, so nothing inlines
/// it back and the calls to one pure callee end up in different functions:
/// m64 -O2 fib(20) ran fib(16) three times (+50% clk), frames +2%.
#[test]
fn test_a_recursion_is_not_copied_for_a_context_several_sites_pass() {
    let text = BRANCHING.replace(
        "  %r = call i32 @fib(i32 20)\n  ret i32 %r",
        "  %r = call i32 @fib(i32 20)\n  %q = call i32 @fib(i32 20)\n  %p = call i32 @fib(i32 20)\n  %u = add i32 %r, %q\n  %t = add i32 %u, %p\n  ret i32 %t",
    );
    assert_ne!(text, BRANCHING);
    let mut module = parsed(&text);
    stepped(&mut module, &["f", "fib"], 20, Threshold { cp_clone: Some(true), ..Threshold::none() });
    assert!(module.named("fib.constprop.1").is_none(), "{}", printed(&module));
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

/// A function that calls itself, or a cycle of functions that call one another,
/// does what its other calls and accesses do: LLVM's PostOrderFunctionAttrs
/// stamps an SCC together. The call to itself was taken to touch every
/// memory, so `hanoi` was `memory(inaccessiblemem: readwrite)`, its two calls
/// with equal arguments could not be one, and it ran 143,747 clocks against
/// gcc's 39,880.
#[test]
fn a_cycle_of_calls_does_what_its_members_do_besides_calling_one_another() {
    let cycle = |sink: &str| {
        format!(
            "@sink = global i16 0

define internal i16 @even(i16 %n) {{
b1:
  %z = icmp eq i16 %n, 0
  br i1 %z, label %b2, label %b3
b2:
  ret i16 1
b3:
  %m = sub i16 %n, 1
  %r = call i16 @odd(i16 %m)
  ret i16 %r
}}

define internal i16 @odd(i16 %n) {{
b1:
  %z = icmp eq i16 %n, 0
  br i1 %z, label %b2, label %b3
b2:
{sink}  ret i16 0
b3:
  %m = sub i16 %n, 1
  %r = call i16 @even(i16 %m)
  ret i16 %r
}}

define internal i16 @alone(i16 %n) {{
b1:
  %z = icmp eq i16 %n, 0
  br i1 %z, label %b2, label %b3
b2:
  ret i16 0
b3:
  %m = sub i16 %n, 1
  %r = call i16 @alone(i16 %m)
  %s = add i16 %r, 1
  ret i16 %s
}}

define i16 @f(i16 %a) {{
b1:
  %x = call i16 @even(i16 %a)
  %y = call i16 @alone(i16 %a)
  %t = add i16 %x, %y
  ret i16 %t
}}
"
        )
    };
    let memory_of = |text: &str, name: &str| {
        let mut module = parsed(text);
        crate::testing::stamped(&mut module).unwrap();
        printed(&module)
            .lines()
            .find(|line| line.starts_with("define") && line.contains(&format!("@{name}(")))
            .map(|line| line.contains("memory(none)"))
            .expect("the function")
    };
    let quiet = cycle("");
    assert!(memory_of(&quiet, "alone"), "a function that only calls itself touches nothing");
    assert!(memory_of(&quiet, "even") && memory_of(&quiet, "odd"), "nor does a cycle of two");
    let loud = cycle("  store volatile i16 1, ptr @sink\n");
    assert!(!memory_of(&loud, "odd") && !memory_of(&loud, "even"), "a member's access is the cycle's");
    assert!(memory_of(&loud, "alone"));
}

/// A `noipa` body is stamped with nothing, so its callers keep its call and
/// the cell it might write: the same body without the attribute is pure and
/// its unused call goes, and a read-only callee lets a stored cell stay in a
/// register.
#[test]
fn a_noipa_body_is_stamped_with_nothing_and_read_as_a_declaration() {
    let leaf = |attribute: &str| {
        format!(
            "define internal i16 @leaf(i16 %x) {attribute} {{\nb:\n  ret i16 %x\n}}\n\ndefine i16 @uses() {{\nb:\n  %r = call i16 @leaf(i16 9)\n  ret i16 42\n}}\n"
        )
    };
    assert_eq!(dropped(&leaf("")), named(&["leaf"]), "premise");
    assert_eq!(dropped(&leaf("noipa")), BTreeSet::new());
    assert_eq!(pure(&leaf("noipa")), BTreeSet::new());
    let loads = |attribute: &str| {
        let text = format!(
            "@x = global i16 0\n\ndefine internal i16 @read(ptr %p) {attribute} {{\nb:\n  %v = load i16, ptr %p\n  ret i16 %v\n}}\n\ndefine i16 @f(i16 %c) {{\nb:\n  store i16 %c, ptr @x\n  %r = call i16 @read(ptr @x)\n  %v = load i16, ptr @x\n  ret i16 %v\n}}\n"
        );
        let mut module = parsed(&text);
        crate::testing::stamped(&mut module).unwrap();
        managed(&mut module, Promote);
        let f = module.function_mut("f").unwrap().1;
        f.walk().filter(|(_, inst)| matches!(f.instruction(*inst).opcode, Opcode::Load { .. })).count()
    };
    assert_eq!(loads(""), 0, "premise: the stamped read-only callee leaves the cell alone");
    assert_eq!(loads("noipa"), 1);
}

/// erlnum.bas m16 -O3: the runtime's `B$FCHR` is `available_externally` (a body
/// to read, not the definition that runs), and a copy of it for a constant
/// became an internal definition that can raise an error, which `ehprepare`
/// refuses ("@llrm.qb.B$FCHR.constprop.1 may raise an error").
#[test]
fn test_an_available_externally_function_is_not_cloned_for_a_constant() {
    let text = TWO_CONTEXTS.replacen("define i16 @g", "define available_externally i16 @g", 1);
    let mut module = parsed(&text);
    stepped(&mut module, &["f", "g"], 20, Threshold { cp_clone: Some(true), ..Threshold::none() });
    assert!(module.named("g.constprop.1").is_none(), "{}", printed(&module));
}

/// A slice descriptor read for its length, passed on by the recursion; the one
/// outside call fills it with 1024. Promotion makes the length a value that
/// crosses the call, so the recursion's own pass-through must meet the outside
/// 1024 and fold.
const SLICE_RECURSION: &str = "@slice = internal addrspace(1) constant { i16, i16, i16 } { i16 0, i16 0, i16 1024 }

define internal i16 @sum(ptr addrspace(1) noalias readonly dereferenceable(6) nocapture %d, i16 %n) {
b0:
  %at = getelementptr i8, ptr addrspace(1) %d, i16 4
  %len = load i16, ptr addrspace(1) %at
  %done = icmp sle i16 %n, 0
  br i1 %done, label %out, label %again
again:
  %m = sub i16 %n, 1
  %r = call i16 @sum(ptr addrspace(1) %d, i16 %m)
  %s = add i16 %r, %n
  %c = icmp ult i16 %s, %len
  %t = select i1 %c, i16 %s, i16 0
  ret i16 %t
out:
  ret i16 %len
}

define i16 @top(i16 %k) {
b0:
  %r = call i16 @sum(ptr addrspace(1) @slice, i16 %k)
  ret i16 %r
}
";

/// The length the descriptor holds is 1024 at every call, the recursion's
/// included; `@sum` kept it as a parameter, compared against a live value
/// (quicksort's `cmp di, [bp-4]`) where the old pipeline had the immediate.
#[test]
fn test_a_length_promoted_from_a_descriptor_folds_through_the_recursion() {
    let mut module = parsed(SLICE_RECURSION);
    step_no_inline(&mut module);
    let text = printed(&module);
    assert!(text.contains("icmp ult i16 %s, 1024") || text.contains("1024"), "{text}");
    assert!(!text.contains("define internal i16 @sum(i16 %0, i16 %n)"), "{text}");
}

fn step_no_inline(module: &mut Module) {
    stepped(module, &["top"], 40, Threshold::none());
}

/// Queens' `place(q, row, n)`: a bounds check against the descriptor's length
/// before the store, the recursion passing the descriptor on. `slice_len` is
/// the length each outside call's descriptor holds.
fn queens_shaped(outside: &str) -> String {
    format!(
        "@twelve = internal addrspace(1) constant {{ i16, i16, i16 }} {{ i16 0, i16 0, i16 12 }}
@five = internal addrspace(1) constant {{ i16, i16, i16 }} {{ i16 0, i16 0, i16 5 }}
declare void @trap() noreturn

define internal i16 @place(ptr addrspace(1) noalias readonly dereferenceable(6) nocapture %q, i16 %row) {{
b0:
  %at = getelementptr i8, ptr addrspace(1) %q, i16 4
  %len = load i16, ptr addrspace(1) %at
  %done = icmp eq i16 %row, 7
  br i1 %done, label %out, label %go
go:
  %in = icmp ult i16 %row, %len
  br i1 %in, label %ok, label %bad
bad:
  call void @trap()
  unreachable
ok:
  %next = add i16 %row, 1
  %r = call i16 @place(ptr addrspace(1) %q, i16 %next)
  ret i16 %r
out:
  ret i16 1
}}

define i16 @top() {{
b0:
{outside}
}}
"
    )
}

/// The length the one outside call's descriptor holds is 12 on every call, the
/// recursion's included: the bounds check compares against 12 and `@place`
/// takes no length, and the bounds-check call is gone. It carried the length as
/// a parameter and the check stayed a comparison with a live value (queens:
/// 225467 instructions, main's 167694).
#[test]
fn test_a_queens_shaped_recursion_folds_the_descriptor_length() {
    let mut module = parsed(&queens_shaped("  %r = call i16 @place(ptr addrspace(1) @twelve, i16 0)\n  ret i16 %r"));
    stepped(&mut module, &["top"], 40, Threshold::none());
    let text = printed(&module);
    // Row 0..7 against a length of 12: the check, and the call it guards, go.
    assert!(!text.contains("@trap()\n") && !text.contains("%0"), "{text}");
    assert!(text.contains("@place(i16 range(i16 0, 8) %row)"), "{text}");
}

/// Two outside calls with different lengths agree on nothing: the length stays
/// a parameter (the control that the extra IPSCCP folds only a constant).
#[test]
fn test_a_recursion_called_with_two_lengths_keeps_the_length_a_parameter() {
    let mut module = parsed(&queens_shaped(
        "  %a = call i16 @place(ptr addrspace(1) @twelve, i16 0)\n  %b = call i16 @place(ptr addrspace(1) @five, i16 0)\n  %r = add i16 %a, %b\n  ret i16 %r",
    ));
    stepped(&mut module, &["top"], 40, Threshold::none());
    let text = printed(&module);
    assert!(text.contains("icmp ult i16 %row, %0") && text.contains("call void @trap()"), "{text}");
    assert!(!text.contains("icmp ult i16 %row, 12") && !text.contains("icmp ult i16 %row, 5"), "{text}");
}

/// `willreturn` for a loop nothing proves finite summarized every global again
/// for each such function (8.3G of 28G instructions at 1200 functions): the
/// summaries are made once for the walk over the components.
#[test]
fn stamping_does_not_summarize_every_global_for_each_function() {
    let scans = |functions: usize| {
        let text: String = (0..functions)
            .map(|n| {
                format!(
                    "define i32 @f{n}(ptr %p) {{\nb0:\n  br label %b1\nb1:\n  %v = load i32, ptr %p\n  %c = icmp ne i32 %v, 0\n  br i1 %c, label %b1, label %b2\nb2:\n  ret i32 %v\n}}\n"
                )
            })
            .collect();
        let mut module = parsed(&text);
        let before = llrm_mir::memory::callee_map_scans();
        crate::testing::stamped(&mut module).unwrap();
        llrm_mir::memory::callee_map_scans() - before
    };
    let (few, many) = (scans(4), scans(32));
    assert!(many <= few + 2, "4 functions: {few} scans, 32: {many}");
}

/// `row` indexes a global: with it known, the address of its cell is an
/// immediate, and a loop of stores through it saves the address arithmetic.
/// `time_saved` priced that fold at nothing, so a call with only that
/// constant (hanoi2's `mv(n, 0, 2, 1)`: 28% of its clocks to gcc, which cloned
/// it) was never cloned.
const CONSTANT_ROW: &str = "@cells = global [4 x [8 x i16]] zeroinitializer

define i16 @fill(i16 %row, i16 %n) {
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %i1, %body ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %body, label %done

body:
  %p = getelementptr [8 x i16], ptr @cells, i16 %row
  %q = getelementptr i16, ptr %p, i16 %i
  store i16 %i, ptr %q
  %i1 = add nsw i16 %i, 1
  br label %head

done:
  ret i16 0
}

define i16 @f(i16 %n) {
b0:
  %a = call i16 @fill(i16 2, i16 %n)
  %b = call i16 @fill(i16 2, i16 %n)
  ret i16 %a
}
";

#[test]
fn test_a_constant_index_into_a_global_is_priced_as_a_saving() {
    let cloned = |clone: bool| {
        let mut module = parsed(CONSTANT_ROW);
        stepped(&mut module, &["f"], 20, Threshold { cp_clone: Some(clone), ..Threshold::none() });
        module.named("fill.constprop.1").is_some()
    };
    assert!(!cloned(false));
    assert!(cloned(true));
}

/// `CONSTANT_ROW` with a second caller, `h`, that the first round does not see.
const TWO_CALLERS: &str = "@cells = global [4 x [8 x i16]] zeroinitializer

define i16 @fill(i16 %row, i16 %n) {
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %i1, %body ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %body, label %done

body:
  %p = getelementptr [8 x i16], ptr @cells, i16 %row
  %q = getelementptr i16, ptr %p, i16 %i
  store i16 %i, ptr %q
  %i1 = add nsw i16 %i, 1
  br label %head

done:
  ret i16 0
}

define i16 @f(i16 %n) {
b0:
  %a = call i16 @fill(i16 2, i16 %n)
  %b = call i16 @fill(i16 2, i16 %n)
  ret i16 %a
}

define i16 @h(i16 %n) {
b0:
  %a = call i16 @fill(i16 2, i16 %n)
  ret i16 %a
}
";

/// The clone for a context is remembered across the walk, which empties the
/// bodies it inlined (`is_deleted`: one `unreachable`). A call that met the
/// same constants afterwards was sent to that empty body: m32 x_hanoi2 -O3 with
/// the clone bound at 4, 5 or 6 lost the tail of a function (no second recursive
/// call, no `ret`) and ran off its end, a `UC_ERR_EXCEPTION` in the emulator.
#[test]
fn test_a_call_is_not_sent_to_a_clone_the_walk_emptied() {
    let mut module = parsed(TWO_CALLERS);
    Program::lend(&mut module, std::rc::Rc::new(crate::testing::Tuned::default()), |program| {
        let costs = program.target.costs();
        let (f, h) = (program.modules[0].named("f").unwrap(), program.modules[0].named("h").unwrap());
        let private = private(program, 0);
        let mut state = crate::ipacp::Cloning::default();
        let first = crate::ipacp::cloned(&mut program.modules[0], &program.layout, &[f], &private, &costs, true, crate::ipacp::Tuning { hint_clones: true, peel_limited: false, max_clones: crate::ipacp::DEFAULT_MAX_CLONES }, &mut state);
        assert_eq!(first.added.len(), 1, "the first round clones fill for row 2");
        let mut analyses = managers(program, &mut ProgramAnalyses::default());
        // What `bare` leaves, without its check that no call stands: the callers
        // of the clone were inlined away in the walk this stands for.
        let module = &mut program.modules[0];
        let mut stub = module.global(first.added[0]).function().map(llrm_mir::module::Function::declaration).unwrap();
        let void = module.context.types.void();
        let entry = stub.create_block(None);
        stub.insert_block(entry, None).unwrap();
        let end = stub.create_instruction(Opcode::Unreachable, void, Vec::new(), llrm_mir::opcode::Flags::default(), None);
        stub.insert(end, llrm_mir::edit::Position::End(entry)).unwrap();
        module.globals[first.added[0].0 as usize].kind = GlobalKind::Function(Box::new(stub));
        analyses[0].changed(first.added[0]);
        crate::ipacp::cloned(&mut program.modules[0], &program.layout, &[f, h], &private, &costs, true, crate::ipacp::Tuning { hint_clones: true, peel_limited: false, max_clones: crate::ipacp::DEFAULT_MAX_CLONES }, &mut state);
        let module = &program.modules[0];
        let caller = module.global(h).function().unwrap();
        let target = caller
            .walk()
            .find_map(|(_, inst)| llrm_mir::memory::callee(&module.context, caller, inst))
            .expect("h still calls something");
        assert!(!module.global(target).function().unwrap().is_deleted(), "h calls an emptied clone:\n{}", printed(module));
    })
    .unwrap();
}

/// Each level of `down` saves a few clocks, too little to pay for a copy; the
/// four levels, each called four times, do, and GCC's `propagate_effects` credits the copy for
/// `row = 0` with the ones after it. Priced alone, the copy was refused: queens
/// got four clones of eight, hanoi2 none.
const DOWN: &str = "@cells = global [8 x [8 x i16]] zeroinitializer

define void @down(i16 %row, i16 %x) {
b0:
  %done = icmp eq i16 %row, 4
  br i1 %done, label %out, label %go

go:
  %p = getelementptr [8 x i16], ptr @cells, i16 %row
  %a = add i16 %x, 1
  %b = mul i16 %a, %x
  %c = add i16 %b, %a
  %d = mul i16 %c, %b
  %e = add i16 %d, %c
  %f = mul i16 %e, %d
  %r1 = add nsw i16 %row, 1
  %q1 = mul i16 %r1, %r1
  %q2 = mul i16 %q1, %r1
  %q3 = mul i16 %q2, %r1
  %q4 = mul i16 %q3, %r1
  %q5 = mul i16 %q4, %r1
  %q6 = mul i16 %q5, %r1
  %q7 = mul i16 %q6, %r1
  %q8 = mul i16 %q7, %r1
  %q9 = mul i16 %q8, %r1
  %q10 = mul i16 %q9, %r1
  %q11 = mul i16 %q10, %r1
  %q12 = mul i16 %q11, %r1
  %g = add i16 %f, %q12
  store i16 %g, ptr %p
  br label %head

head:
  %i = phi i16 [ 0, %go ], [ %i1, %next ]
  %more = icmp slt i16 %i, 4
  br i1 %more, label %next, label %out

next:
  call void @down(i16 %r1, i16 %e)
  %i1 = add nsw i16 %i, 1
  br label %head

out:
  ret void
}

define void @f(i16 %x) {
b0:
  call void @down(i16 0, i16 %x)
  call void @down(i16 0, i16 3)
  ret void
}
";

#[test]
fn test_a_clone_is_credited_with_the_clones_its_recursion_makes() {
    let mut module = parsed(DOWN);
    stepped(&mut module, &["f"], 20, Threshold { cp_clone: Some(true), ..Threshold::none() });
    assert!(module.named("down.constprop.1").is_some(), "{}", printed(&module));
}

/// `down` halves the range it passes on: `(row + hi) / 2` is no pass-through
/// of one parameter, so GCC follows no chain of contexts from it. Credited
/// with the contexts anyway, recchop got nine clones (+3485 B, 650 clk) at
/// -O3.
const HALVING: &str = "@cells = global [8 x [8 x i16]] zeroinitializer

define void @down(i16 %row, i16 %hi, i16 %x) {
b0:
  %done = icmp eq i16 %row, %hi
  br i1 %done, label %out, label %go

go:
  %p = getelementptr [8 x i16], ptr @cells, i16 %row
  %a = add i16 %x, 1
  %b = mul i16 %a, %x
  %c = add i16 %b, %a
  %d = mul i16 %c, %b
  %e = add i16 %d, %c
  %f = mul i16 %e, %d
  store i16 %f, ptr %p
  %s = add i16 %row, %hi
  %m = udiv i16 %s, 2
  call void @down(i16 %m, i16 %hi, i16 %e)
  br label %out

out:
  ret void
}

define void @f(i16 %x) {
b0:
  call void @down(i16 0, i16 7, i16 %x)
  call void @down(i16 0, i16 7, i16 3)
  ret void
}
";

#[test]
fn test_a_recursion_that_halves_a_range_makes_no_chain_of_contexts() {
    let mut module = parsed(HALVING);
    stepped(&mut module, &["f"], 20, Threshold { cp_clone: Some(true), ..Threshold::none() });
    assert!(module.named("down.constprop.1").is_none(), "{}", printed(&module));
}
