//! What `compute` finds, against `GlobalsModRef.cpp`'s rules.

use super::*;
use crate::testing::m16_ir;
use crate::testing::parsed;

fn found(text: &str) -> (Module, GlobalsModRef) {
    let module = parsed(text);
    let result = compute(&module);
    (module, result)
}

const CHAIN: &str = "@g = internal global i16 0

define internal i16 @f() {
b0:
  %v = load i16, ptr @g
  ret i16 %v
}

define void @w() {
b0:
  %v = call i16 @f()
  store i16 %v, ptr @g
  ret void
}
";

#[test]
fn a_reader_of_a_private_global_has_ref_on_it() {
    let (module, result) = found(CHAIN);
    let (f, g) = (module.named("f").unwrap(), module.named("g").unwrap());
    assert!(result.is_non_address_taken(g));
    assert_eq!(result.function_info(f).unwrap().mod_ref_for_global(g), ModRefInfo::Ref);
    assert_eq!(result.function_info(f).unwrap().mod_ref(), ModRefInfo::Ref);
}

/// A caller carries its callees' effects up the call graph.
#[test]
fn a_caller_adds_its_callees_global_effects_to_its_own() {
    let (module, result) = found(CHAIN);
    let (w, g) = (module.named("w").unwrap(), module.named("g").unwrap());
    assert_eq!(result.function_info(w).unwrap().mod_ref_for_global(g), ModRefInfo::ModRef);
}

/// A result held past the walk does not track `@g`, whose address `@f` passed
/// on, and answers `ModRef` for it; inlined, `@g` is tracked and `@f` writes
/// it. `covers` compared the held `NoModRef` it keeps for an untracked global
/// and failed `LLRM_CHECK_STALE` on pl_trace.c.
#[test]
fn a_held_result_covers_a_global_it_did_not_track() {
    let head = "@g = internal global i16 0

define internal void @h(ptr %p) {
b0:
  store i16 1, ptr %p
  ret void
}
";
    let (_, stale) = found(&format!(
        "{head}
define void @f() {{
b0:
  call void @h(ptr @g)
  ret void
}}
"
    ));
    let (module, fresh) = found(&format!(
        "{head}
define void @f() {{
b0:
  store i16 1, ptr @g
  ret void
}}
"
    ));
    let g = module.named("g").unwrap();
    assert!(!stale.is_non_address_taken(g) && fresh.is_non_address_taken(g), "premise: tracked once inlined");
    assert!(stale.covers(&fresh), "a global the held result does not track was compared");
}

/// LSR's pointer step joins `@g`'s address in a `phi`, which the use scan
/// calls an escape: the held result, made before, still holds, and `covers`
/// said it did not (`LLRM_CHECK_STALE` on pl_trace.c -Os).
#[test]
fn a_held_result_covers_a_global_lost_only_to_a_join() {
    let (_, stale) = found(
        "@g = internal global i16 0

define i16 @f(i1 %c) {
b0:
  %v = load i16, ptr @g
  ret i16 %v
}
",
    );
    let (module, fresh) = found(
        "@g = internal global i16 0

define i16 @f(i1 %c) {
b0:
  %p = select i1 %c, ptr @g, ptr @g
  %v = load i16, ptr %p
  ret i16 %v
}
",
    );
    let g = module.named("g").unwrap();
    assert!(!fresh.is_non_address_taken(g), "premise: the scan calls a join an escape");
    assert!(stale.covers(&fresh), "a global lost only to a join was taken for an escape");
}

/// A declaration is read from its attributes (`AnalyzeCallGraph`'s
/// `isDeclaration()` arm): one that touches no memory leaves the caller its
/// info, one that may call back into the module leaves it none.
#[test]
fn a_declaration_gives_its_caller_info_only_where_it_cannot_call_back() {
    let text = |declaration: &str| {
        format!(
            "@g = internal global i16 0
{declaration}

define void @f() {{
b0:
  call void @out()
  %v = load i16, ptr @g
  ret void
}}
"
        )
    };
    let (module, quiet) = found(&text("declare void @out() nocallback readnone"));
    let f = module.named("f").unwrap();
    assert_eq!(quiet.function_info(f).map(|one| one.mod_ref()), Some(ModRefInfo::Ref));
    let (module, writes) = found(&text("declare void @out() nocallback"));
    assert_eq!(writes.function_info(module.named("f").unwrap()).map(|one| one.mod_ref()), Some(ModRefInfo::ModRef));
    // The declaration is itself a node with the info its attributes give, which
    // is what a call's query reads (`getFunctionInfo(Callee)`).
    let (module, again) = found(&text("declare void @out() nocallback"));
    assert_eq!(again.function_info(module.named("out").unwrap()).map(|one| one.mod_ref()), Some(ModRefInfo::ModRef));
    let (module, calls_back) = found(&text("declare void @out()"));
    assert!(calls_back.function_info(module.named("out").unwrap()).is_none());
    assert!(calls_back.function_info(module.named("f").unwrap()).is_none());
    assert_eq!(calls_back.get_memory_effects(module.named("f").unwrap()), MemoryEffects::unknown());
}

/// A global whose address is stored is not tracked, and what reaches it is
/// `ModRef`.
#[test]
fn a_global_whose_address_is_stored_is_not_tracked() {
    let (module, result) = found(
        "@g = internal global i16 0
@p = internal global ptr null

define void @f() {
b0:
  store ptr @g, ptr @p
  ret void
}
",
    );
    let (f, g) = (module.named("f").unwrap(), module.named("g").unwrap());
    assert!(!result.is_non_address_taken(g));
    assert_eq!(
        result.get_mod_ref_info_for_global(&module.globals, Some(f), g, || ModRefInfo::NoModRef),
        ModRefInfo::ModRef
    );
}

/// The query: a call that only reads `@g` does not modify it.
#[test]
fn a_call_that_only_reads_a_tracked_global_does_not_modify_it() {
    let (module, result) = found(CHAIN);
    let (f, g) = (module.named("f").unwrap(), module.named("g").unwrap());
    let info = result.get_mod_ref_info_for_global(&module.globals, Some(f), g, || ModRefInfo::NoModRef);
    assert_eq!(info, ModRefInfo::Ref);
}

/// A constant global read through a chain of GEPs is read by the function.
#[test]
fn a_constant_global_read_through_geps_has_ref_for_its_reader() {
    let (module, result) = found(
        "@t = internal constant [4 x i16] [i16 1, i16 2, i16 3, i16 4]

define i16 @f(i16 %i) {
b0:
  %p = getelementptr inbounds i8, ptr @t, i16 %i
  %q = getelementptr inbounds i8, ptr %p, i16 2
  %v = load i16, ptr %q
  ret i16 %v
}
",
    );
    let (f, t) = (module.named("f").unwrap(), module.named("t").unwrap());
    assert!(result.is_non_address_taken(t));
    assert_eq!(result.function_info(f).unwrap().mod_ref_for_global(t), ModRefInfo::Ref);
}

/// The argument part is asked only after every check passed (a direct callee,
/// a tracked global, a `FunctionInfo`), as LLVM evaluates it inside the nested
/// ifs.
#[test]
fn the_argument_part_is_not_asked_without_a_callee_or_for_an_address_taken_global() {
    let (module, result) = found(
        "@g = internal global i16 0
@p = internal global ptr null

define void @f() {
b0:
  store ptr @g, ptr @p
  ret void
}
",
    );
    let (f, g) = (module.named("f").unwrap(), module.named("g").unwrap());
    let asked = std::cell::Cell::new(0);
    let ask = || {
        asked.set(asked.get() + 1);
        ModRefInfo::NoModRef
    };
    assert_eq!(result.get_mod_ref_info_for_global(&module.globals, None, g, ask), ModRefInfo::ModRef);
    assert_eq!(result.get_mod_ref_info_for_global(&module.globals, Some(f), g, ask), ModRefInfo::ModRef);
    assert_eq!(asked.get(), 0);
    let (module, result) = found(CHAIN);
    let (f, g) = (module.named("f").unwrap(), module.named("g").unwrap());
    result.get_mod_ref_info_for_global(&module.globals, Some(f), g, ask);
    assert_eq!(asked.get(), 1);
}

/// `GlobalsAAResult::getModRefInfo(Call, Loc)`: a call to a function that only
/// writes `@h` leaves `@g` alone and modifies `@h`; both are internal and
/// never have their address taken.
#[test]
fn a_call_is_apart_from_a_tracked_global_its_callee_never_names() {
    use crate::aa::{AAQueryInfo, AAResults, MemoryLocation};
    let module = parsed(
        "@g = internal global i16 0
@h = internal global i16 0

define internal void @seth() {
b0:
  store i16 1, ptr @h
  ret void
}

define i16 @f() {
b0:
  store i16 7, ptr @g
  call void @seth()
  %a = load i16, ptr @g
  %b = load i16, ptr @h
  %r = add i16 %a, %b
  ret i16 %r
}
",
    );
    let info = compute(&module);
    let layout = crate::testing::layout(&module);
    let f = crate::testing::function(&module, "f");
    let unit = crate::memory::Unit::of(&module, &layout, f).with_globals_mr(&info);
    let call =
        f.walk().map(|(_, inst)| inst).find(|&inst| matches!(f.instruction(inst).opcode, Opcode::Call(_))).unwrap();
    let load = |name: &str| {
        let value = crate::testing::value(f, name);
        let inst = f.walk().map(|(_, inst)| inst).find(|&inst| f.instruction(inst).result == Some(value)).unwrap();
        MemoryLocation::get(&unit, inst).expect("a load")
    };
    let mut aa = GlobalsAAResult::new(&unit, &info);
    let mut aaqi = AAQueryInfo::default();
    assert_eq!(aa.get_mod_ref_info(call, Some(&load("a")), &mut aaqi), ModRefInfo::NoModRef, "@g");
    assert_eq!(aa.get_mod_ref_info(call, Some(&load("b")), &mut aaqi), ModRefInfo::Mod, "@h");
}

/// `GlobalsAAResult::alias` (L836): a pointer that is an argument, a call's
/// result or loaded from a global cannot be a non-address-taken global, so the
/// two do not alias. It answered MayAlias for everything: a `memcpy` into a
/// tracked array was a clobber of a field of a parameter in MemorySSA (22 of
/// 1796 call queries in qbq's d_faces -O2 where the footprint said apart).
#[test]
fn a_tracked_global_does_not_alias_an_argument_or_a_second_tracked_global() {
    use crate::aa::{AAQueryInfo, AAResults, AliasResult, LocationSize, MemoryLocation};
    let module = parsed(
        "@g = internal global [4 x i16] zeroinitializer
@h = internal global i16 0
@taken = global i16 0

define i16 @f(ptr %p) {
b0:
  %a = load i16, ptr %p
  %gv = load i16, ptr @g
  %hv = load i16, ptr @h
  %tv = load i16, ptr @taken
  ret i16 %a
}
",
    );
    let info = compute(&module);
    let layout = crate::testing::layout(&module);
    let f = crate::testing::function(&module, "f");
    let unit = crate::memory::Unit::of(&module, &layout, f).with_globals_mr(&info);
    let pointer = |result: &str| {
        let value = crate::testing::value(f, result);
        let load = f.walk().map(|(_, inst)| inst).find(|&inst| f.instruction(inst).result == Some(value)).unwrap();
        f.instruction(load).operands[0]
    };
    let global = |name: &str| match name {
        "g" => pointer("gv"),
        "h" => pointer("hv"),
        _ => pointer("tv"),
    };
    let at = |ptr| MemoryLocation::new(ptr, LocationSize::Precise(2));
    let argument = llrm_mir::module::Operand::Value(crate::testing::value(f, "p"));
    let mut aa = GlobalsAAResult::new(&unit, &info);
    let mut aaqi = AAQueryInfo::default();
    assert_eq!(aa.alias(&at(global("g")), &at(argument), &mut aaqi, None), AliasResult::NoAlias, "@g, an argument");
    assert_eq!(aa.alias(&at(argument), &at(global("g")), &mut aaqi, None), AliasResult::NoAlias, "an argument, @g");
    assert_eq!(aa.alias(&at(global("g")), &at(global("h")), &mut aaqi, None), AliasResult::NoAlias, "@g, @h");
    assert_eq!(aa.alias(&at(global("g")), &at(global("g")), &mut aaqi, None), AliasResult::MayAlias, "@g, @g");
    assert_eq!(aa.alias(&at(global("taken")), &at(argument), &mut aaqi, None), AliasResult::MayAlias, "@taken");
}

/// `getModRefInfo(Call, Loc)` of the aggregate asks the aggregate's `alias`
/// for each argument (`AAQI.AAR.alias`, BasicAliasAnalysis.cpp L1030): a
/// `memcpy` within a tracked array leaves a parameter's field alone. BasicAA
/// asked its own `alias`, which says "may" for a global against an argument,
/// so the call stayed a clobber in MemorySSA (22 of 1796 call queries in
/// qbq's d_faces -O2).
#[test]
fn a_call_into_a_tracked_global_leaves_an_argument_through_the_aggregate() {
    use crate::aa::{AAQueryInfo, AAResults, MemoryLocation};
    let module = parsed(
        "@g = internal global [4 x i16] zeroinitializer

declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)

define i16 @f(ptr %p) {
b0:
  %x = load i16, ptr %p
  call void @llvm.memcpy.p0.p0.i16(ptr @g, ptr @g, i16 4, i1 false)
  ret i16 %x
}
",
    );
    let info = compute(&module);
    let layout = crate::testing::layout(&module);
    let f = crate::testing::function(&module, "f");
    let unit = crate::memory::Unit::of(&module, &layout, f).with_globals_mr(&info);
    let call =
        f.walk().map(|(_, inst)| inst).find(|&inst| matches!(f.instruction(inst).opcode, Opcode::Call(_))).unwrap();
    let value = crate::testing::value(f, "x");
    let load = f.walk().map(|(_, inst)| inst).find(|&inst| f.instruction(inst).result == Some(value)).unwrap();
    let location = MemoryLocation::get(&unit, load).expect("a load");
    let mut alone = crate::basicaa::BasicAA::new(&unit);
    assert_eq!(alone.get_mod_ref_info(call, Some(&location), &mut AAQueryInfo::default()), ModRefInfo::ModRef);
    let mut aggregate = crate::aggregate::AggregateAA::new();
    aggregate.add_aa_result(crate::basicaa::BasicAA::new(&unit));
    aggregate.add_aa_result(GlobalsAAResult::new(&unit, &info));
    assert_eq!(aggregate.get_mod_ref_info(call, Some(&location), &mut AAQueryInfo::default()), ModRefInfo::NoModRef);
}

/// The pin's answers (`opt -aa-pipeline=basic-aa -passes=aa-eval
/// -print-all-alias-modref-info`, d1106deb71cc) for a call against the slots of
/// the frame: a slot the call is not given and nothing captured before it is
/// apart from it, a slot passed to it (or to an earlier call that captured it)
/// is not; `lifetime.end` of one slot is apart from every other. The walker's
/// call queries go to this stack, so a "no" here is a "no clobber" in codegen.
#[test]
fn a_call_against_the_frames_slots_answers_as_the_pins_basic_aa_does() {
    use crate::aa::{AAQueryInfo, AAResults, MemoryLocation};
    let ask = |text: &str, call_of: &str, slots: &[(&str, ModRefInfo)]| {
        let module = parsed(text);
        let info = compute(&module);
        let layout = crate::testing::layout(&module);
        let f = crate::testing::function(&module, "f");
        let unit = crate::memory::Unit::of(&module, &layout, f).with_globals_mr(&info);
        let call = f
            .walk()
            .map(|(_, inst)| inst)
            .find(|&inst| {
                matches!(f.instruction(inst).opcode, Opcode::Call(_))
                    && llrm_mir::memory::callee(unit.context, f, inst)
                        .and_then(|one| module.globals.get(one.0 as usize))
                        .is_some_and(|global| global.name.owned().is_some_and(|text| text.contains(call_of)))
            })
            .expect("the call");
        let mut aggregate = crate::aggregate::AggregateAA::new();
        aggregate.add_aa_result(crate::basicaa::BasicAA::new(&unit));
        aggregate.add_aa_result(GlobalsAAResult::new(&unit, &info));
        for (loaded, expected) in slots {
            let value = crate::testing::value(f, loaded);
            let load = f.walk().map(|(_, inst)| inst).find(|&inst| f.instruction(inst).result == Some(value)).unwrap();
            let location = MemoryLocation::get(&unit, load).expect("a load");
            let found = aggregate.get_mod_ref_info(call, Some(&location), &mut AAQueryInfo::default());
            assert_eq!(found, *expected, "%{loaded} against @{call_of}");
        }
    };
    let prelude = "declare void @pak_seek(ptr, ptr)
declare void @load_stream(ptr, ptr, ptr)
declare void @llvm.lifetime.end.p0(i64, ptr)

";
    let body = |captures_bytes: &str| {
        format!(
            "{prelude}define i32 @f(ptr %w) {{
b0:
  %path = alloca [24 x i8]
  %bytes = alloca i32
  %im = alloca [72 x i8]
  %other = alloca i32
  {captures_bytes}
  call void @load_stream(ptr %w, ptr %im, ptr %path)
  %v = load i32, ptr %bytes
  %u = load i32, ptr %im
  %p = load i8, ptr %path
  call void @llvm.lifetime.end.p0(i64 4, ptr %other)
  ret i32 %v
}}
"
        )
    };
    let apart = ModRefInfo::NoModRef;
    let both = ModRefInfo::ModRef;
    ask(&body("call void @pak_seek(ptr %w, ptr %path)"), "load_stream", &[("v", apart), ("u", both), ("p", both)]);
    ask(&body("call void @pak_seek(ptr %bytes, ptr %path)"), "load_stream", &[("v", both), ("u", both), ("p", both)]);
    ask(&body("call void @pak_seek(ptr %w, ptr %path)"), "lifetime.end", &[("v", apart), ("u", apart), ("p", apart)]);
    // Far pointers: arguments cannot point at a slot made after them, one a
    // call captured included; `memset` writes its destination alone.
    let far = |call: &str| {
        m16_ir(&format!(
            "target datalayout = \"@m16-datalayout@\"
declare void @cap(ptr)
declare void @llvm.memcpy.p1.p0.i16(ptr addrspace(1), ptr, i16, i1)
declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)

define i16 @f(ptr addrspace(1) %far, ptr %near) {{
b0:
  %slot = alloca [8 x i8]
  %slot2 = alloca [8 x i8]
  call void @cap(ptr %slot2)
  {call}
  %a = load i16, ptr %slot
  %b = load i16, ptr %slot2
  %c = load i16, ptr %near
  %d = load i16, ptr addrspace(1) %far
  ret i16 %a
}}
"
        ))
    };
    ask(
        &far("call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %far, ptr %near, i16 8, i1 false)"),
        "memcpy",
        &[("a", apart), ("b", apart), ("c", both), ("d", both)],
    );
    ask(
        &far("call void @llvm.memset.p0.i16(ptr %slot, i8 0, i16 8, i1 false)"),
        "memset",
        &[("a", ModRefInfo::Mod), ("b", apart), ("c", apart), ("d", apart)],
    );
}

/// DEVIATE G8 takes the globals the program does not export as local. QB's
/// `VARPTR`, `VARSEG` and `@` still make one reachable by a far pointer: the
/// address passed to a call, turned into an integer or stored is taken, so
/// such a global is not tracked and a call may write it.
#[test]
fn an_unexported_global_whose_address_a_program_takes_is_still_not_tracked() {
    for taken in ["call void @use(ptr @g)", "%i = ptrtoint ptr @g to i16", "store ptr @g, ptr @slot"] {
        let module = parsed(&format!(
            "@g = global i16 0
@slot = internal global ptr null

declare void @use(ptr)
declare void @callee() nocallback

define void @f() {{
b0:
  {taken}
  call void @callee()
  ret void
}}
"
        ));
        let (g, callee) = (module.named("g").unwrap(), module.named("callee").unwrap());
        let result = compute_with(&module, BTreeSet::from([g]));
        assert!(!result.is_non_address_taken(g), "{taken}");
        assert_eq!(
            result.get_mod_ref_info_for_global(&module.globals, Some(callee), g, || ModRefInfo::NoModRef),
            ModRefInfo::ModRef,
            "{taken}"
        );
    }
    let quiet = parsed(
        "@g = global i16 0\n\ndeclare void @callee() nocallback\n\ndefine void @f() {\nb0:\n  call void @callee()\n  ret void\n}\n",
    );
    let (g, callee) = (quiet.named("g").unwrap(), quiet.named("callee").unwrap());
    let result = compute_with(&quiet, BTreeSet::from([g]));
    let info = result.get_mod_ref_info_for_global(&quiet.globals, Some(callee), g, || ModRefInfo::NoModRef);
    assert!(!info.is_mod_set(), "premise: without a taken address the unexported global is tracked");
}

/// A global lost to a join was followed with a second scan of the whole module
/// for its use sites (`Sites::of`, 112 K a scan on `d_faces`, 17 runs): the
/// sites are the module's, not an analysis's.
#[test]
fn a_global_followed_through_a_join_costs_no_second_scan_of_the_module() {
    let module = parsed(
        "@g = internal global i16 0

define i16 @f(i1 %c) {
b0:
  %p = select i1 %c, ptr @g, ptr @g
  %v = load i16, ptr %p
  ret i16 %v
}
",
    );
    let before = built();
    let result = compute(&module);
    assert!(!result.is_non_address_taken(module.named("g").unwrap()), "premise: a join is followed");
    assert_eq!(built() - before, 1, "the module was scanned for sites more than once");
}
