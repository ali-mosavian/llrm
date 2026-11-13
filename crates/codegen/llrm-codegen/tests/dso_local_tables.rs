//! Which globals `isOffsetFoldingLegal` may fold an offset into: the targets
//! that take every symbol as `dso_local` keep doing so, and the ones that
//! read the IR (wasm) follow `GlobalValue::isImplicitDSOLocal`. A default
//! visibility `external` taken as local made wasm fold `e+8` per access where
//! the pin keeps `e` and the offset apart (+4 B on a 17-line memmove).

use llrm_codegen::backend::selectiondag::function_lowering::module_tables;

const IR: &str = r#"
@internal = internal global i32 0
@hidden_def = hidden global i32 0
@hidden_decl = external hidden global i32
@default_def = global i32 0
@default_decl = external global i32
@weak_hidden = extern_weak hidden global i32
"#;

fn names_and_locality(
    dso_local_symbols: bool,
) -> Vec<(String, bool)> {
    let module = llrm_mir::parse::module(IR).expect("parses");
    let (names, local) = module_tables(&module, dso_local_symbols, false);
    names.iter().cloned().zip(local.iter().copied()).collect()
}

#[test]
fn a_target_taking_every_symbol_as_dso_local_keeps_doing_so() {
    assert!(names_and_locality(true).iter().all(|(_, local)| *local));
}

#[test]
fn a_default_visibility_declaration_is_preemptible_and_a_hidden_one_is_not() {
    let got = names_and_locality(false);
    let local = |name: &str| got.iter().find(|(n, _)| n == name).expect(name).1;
    assert!(local("internal"));
    assert!(local("hidden_def"));
    assert!(local("hidden_decl"));
    assert!(!local("default_def"));
    assert!(!local("default_decl"));
    assert!(!local("weak_hidden"));
}
