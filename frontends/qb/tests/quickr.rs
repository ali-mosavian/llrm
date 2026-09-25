//! QuickrBASIC's use-before-definition warning.

use qbfront::semantic::compile_with_warnings;
use qbfront::{parse, Dialect};

fn warnings(source: &str) -> Vec<String> {
    let module = parse(source, Dialect::Quickr).expect("parses");
    compile_with_warnings(&module, "t", Dialect::Quickr, "vbdos", false, false, false, false, false, false)
        .expect("compiles")
        .1
}

#[test]
fn reading_a_local_before_assigning_it_warns() {
    let found = warnings("SUB s\nDIM a AS INTEGER, b AS INTEGER\nb = a\nEND SUB\n");
    assert_eq!(found, ["line 3: warning: A is read before it is assigned"]);
}

#[test]
fn assignment_on_only_one_branch_still_warns() {
    let source = "SUB s (c AS INTEGER)\nDIM a AS INTEGER\nIF c THEN a = 1\nPRINT a\nEND SUB\n";
    assert_eq!(warnings(source), ["line 4: warning: A is read before it is assigned"]);
}

#[test]
fn assignment_on_every_path_is_quiet() {
    let source = "SUB s (c AS INTEGER)\nDIM a AS INTEGER\nIF c THEN a = 1 ELSE a = 2\nPRINT a\nEND SUB\n";
    assert_eq!(warnings(source), Vec::<String>::new());
}

#[test]
fn loop_carried_assignment_warns_on_the_first_pass() {
    let source = "SUB s\nDIM a AS INTEGER, i AS INTEGER\nFOR i = 1 TO 2\nPRINT a\na = i\nNEXT\nEND SUB\n";
    assert_eq!(warnings(source), ["line 4: warning: A is read before it is assigned"]);
}

#[test]
fn byref_arguments_and_input_assign() {
    let source = "SUB t (x AS INTEGER)\nx = 1\nEND SUB\n\
        SUB s\nDIM a AS INTEGER, b AS INTEGER\nt a\nINPUT b\nPRINT a; b\nEND SUB\n";
    assert_eq!(warnings(source), Vec::<String>::new());
}

#[test]
fn module_code_warns_until_a_procedure_may_assign() {
    let source = "DIM SHARED a AS INTEGER, b AS INTEGER\nPRINT a\nsetup\nPRINT b\n\
        SUB setup\nb = 1\nEND SUB\n";
    assert_eq!(warnings(source), ["line 2: warning: A is read before it is assigned"]);
}

#[test]
fn vbdos_does_not_warn() {
    let module = parse("SUB s\nDIM a AS INTEGER\nPRINT a\nEND SUB\n", Dialect::VbDos).expect("parses");
    let (_, found) =
        compile_with_warnings(&module, "t", Dialect::VbDos, "vbdos", false, false, false, false, false, false)
            .expect("compiles");
    assert!(found.is_empty(), "{found:?}");
}

fn select(arms: &str) -> Vec<String> {
    warnings(&format!(
        "SUB s (c AS INTEGER)\nDIM a AS INTEGER\nSELECT CASE c\n{arms}END SELECT\nPRINT a\nEND SUB\n"
    ))
}

#[test]
fn one_unassigning_path_among_many_warns() {
    let warned = ["line 10: warning: A is read before it is assigned"];
    assert_eq!(select("CASE 1\na = 1\nCASE 2\nCASE ELSE\na = 3\n"), warned);
    // Without CASE ELSE, falling through every CASE is the path with no def.
    assert_eq!(select("CASE 1\na = 1\nCASE 2\na = 2\n\n"), warned);
    assert_eq!(select("CASE 1\na = 1\nCASE 2\na = 2\nCASE ELSE\na = 3\n"), Vec::<String>::new());
}
