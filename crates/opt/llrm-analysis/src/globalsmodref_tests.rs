//! What `compute` finds, against `GlobalsModRef.cpp`'s rules.

use super::*;
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
