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
    let proved = Program::lend(module, std::rc::Rc::new(llrm_mir::target::Neutral), |program| {
        let mut modules = managers(program, &mut ProgramAnalyses::default());
        optimized_with::<String>(
            program,
            &mut modules,
            &roots,
            &costs,
            None,
            0,
            threshold,
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
            "define internal i16 @read(ptr nocapture nofree readonly %p) memory(argmem: read) willreturn nounwind nofree nosync norecurse {",
            "define internal void @write(ptr nocapture nofree writeonly initializes((0, 2)) %p, i16 %x) memory(argmem: write) willreturn nounwind nofree nosync norecurse {",
            "define internal void @keep(ptr %p) memory(write, argmem: none, inaccessiblemem: none) willreturn nounwind nofree nosync norecurse {",
            "define internal void @calls(ptr nocapture nofree initializes((0, 2)) %p, i16 %x) memory(write, argmem: readwrite, inaccessiblemem: none) willreturn nounwind nofree nosync norecurse {",
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
        stepped(&mut module, &["f", "g"], 20, Threshold { cp_clone: clone, ..Threshold::none() });
        assert_eq!(results(&module, inputs), results(&parsed(TWO_CONTEXTS), inputs), "{}", printed(&module));
        (module.named("g.constprop.1").is_some(), module.named("g.constprop.2").is_some())
    };
    assert_eq!(cloned(false), (false, false));
    assert_eq!(cloned(true), (true, true));
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
    stepped(&mut module, &["f", "g"], 20, Threshold { cp_clone: true, ..Threshold::none() });
    assert!(module.named("g.constprop.1").is_none(), "{}", printed(&module));
}
