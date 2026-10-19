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

/// A call to a declaration leaves the caller without information, as LLVM's
/// SCC walk does (a declaration has no `FunctionInfo` and is not in the SCC).
#[test]
fn a_call_to_a_declaration_leaves_the_caller_without_info() {
    let (module, result) = found(
        "@g = internal global i16 0
declare void @out() nocallback readnone

define void @f() {
b0:
  call void @out()
  %v = load i16, ptr @g
  ret void
}
",
    );
    assert!(result.function_info(module.named("f").unwrap()).is_none());
    assert_eq!(result.get_memory_effects(module.named("f").unwrap()), MemoryEffects::unknown());
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
    assert_eq!(result.get_mod_ref_info_for_global(&module, Some(f), g, || ModRefInfo::NoModRef), ModRefInfo::ModRef);
}

/// The query: a call that only reads `@g` does not modify it.
#[test]
fn a_call_that_only_reads_a_tracked_global_does_not_modify_it() {
    let (module, result) = found(CHAIN);
    let (f, g) = (module.named("f").unwrap(), module.named("g").unwrap());
    let info = result.get_mod_ref_info_for_global(&module, Some(f), g, || ModRefInfo::NoModRef);
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
