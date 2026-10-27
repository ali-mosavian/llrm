//! clang's `-g` types and globals were carried (PR 1) but nothing read them:
//! `llrm.dbg.types` and `llrm.dbg.globals` stayed empty, so an object built
//! from `-g` IR described no type and no variable.

use llrm_mir::debuginfo::{self, Kind, Reach, Scalar};
use llrm_mir::{dbgimport, parse};

fn imported(text: &str) -> llrm_mir::module::Module {
    let mut module = parse::module(text).expect("parses");
    dbgimport::types_and_globals(&mut module);
    module
}

#[test]
fn test_a_struct_a_pointer_and_an_int_become_the_models_types() {
    let module = imported(include_str!("clang/dbg_struct.ll"));
    let types: Vec<_> =
        debuginfo::types(&module).into_iter().filter_map(|id| debuginfo::read_type(&module, id)).collect();
    let int = types.iter().find(|one| one.kind == Kind::Scalar && one.spelling.as_deref() == Some("int")).expect("int");
    assert_eq!((int.name.as_str(), int.size), (Scalar::Int32.value(), 4));
    let point = types.iter().find(|one| one.kind == Kind::Struct && one.name == "P").expect("struct P");
    assert_eq!((point.size, point.members.len()), (8, 2));
    assert_eq!(
        point.members.iter().map(|one| (one.name.as_str(), one.offset)).collect::<Vec<_>>(),
        [("x", 0), ("y", 4)]
    );
    let pointer = types.iter().find(|one| one.kind == Kind::Pointer).expect("a pointer");
    assert_eq!((pointer.size, pointer.reach), (4, Reach::Near));
}

#[test]
fn test_file_scope_variables_become_globals_with_their_types() {
    let module = imported(include_str!("clang/dbg_types.ll"));
    let globals = debuginfo::globals(&module);
    let names: Vec<_> = globals.iter().map(|one| (one.global.as_str(), one.name.as_str())).collect();
    assert_eq!(names, [("g", "g"), ("arr", "arr")]);
    let array = debuginfo::read_type(&module, globals[1].r#type).expect("arr's type");
    assert_eq!((array.kind, array.size), (Kind::Sized, 16));
}

#[test]
fn test_a_module_without_debug_nodes_gains_none() {
    let module = imported("define i32 @f() {\n  ret i32 0\n}\n");
    assert!(module.named_metadata.is_empty());
}

#[test]
fn test_a_defined_function_becomes_a_procedure_and_its_declares_name_variables() {
    let module = imported(include_str!("clang/dbg.ll"));
    let functions = debuginfo::functions(&module);
    let dist = functions.iter().find(|one| one.function == "dist").expect("dist");
    assert_eq!(dist.name, "dist");
    assert_eq!(
        dist.parameters.iter().map(|(at, name, _)| (*at, name.as_str())).collect::<Vec<_>>(),
        [(0, "p"), (1, "k")]
    );
    let (_, function) = module
        .functions()
        .find(|(_, global, _)| global.name.as_deref() == Some("dist"))
        .map(|(i, _, f)| (i, f))
        .unwrap();
    let variables: Vec<_> =
        function.debug_records().iter().filter_map(|one| debuginfo::read_variable(&module, one.variable)).collect();
    assert_eq!(variables.len(), 4, "{variables:?}");
    assert!(variables.iter().all(|one| one.scope == "dist"));
    assert_eq!(variables.iter().filter(|one| one.parameter).count(), 2);
}

/// The unit's `!DIFile` was dropped: an object named its source after the
/// `.ll` file it was built from (`dbgfix.ll`), with no checksum.
#[test]
fn test_the_units_file_name_and_md5_are_kept() {
    let module = imported(include_str!("clang/dbg.ll"));
    assert_eq!(
        debuginfo::file(&module),
        Some(("dbg.c".to_owned(), Some("edf9135163a14d7b5141ba381daf995b".to_owned())))
    );
}
