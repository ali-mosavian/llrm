//! #78: every QB function keyword is an intrinsic with its runtime routine,
//! or refused; never an implicit variable.

use std::path::Path;

use llrm_core::hir::model::Program;

use super::compile as qb_compile;
use super::driver as qb_driver;
use super::test_hir::written;

/// Each dialect, on its own runtime.
const DIALECTS: [&str; 3] = ["qb45", "pds71", "vbdos"];

fn parsed(source: &str, dialect: &str) -> Result<Program, String> {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "intrinsic.bas", source.as_bytes());
    qb_driver::parsed(&path, &qb_driver::Frontend::new(dialect, dialect), None).map_err(|error| error.0)
}

fn callees(program: &Program) -> Vec<&str> {
    let instructions = program.modules.iter().flat_map(|one| &one.functions).flat_map(|one| &one.blocks).flat_map(|one| &one.instructions);
    instructions.filter_map(|one| one.callee.as_deref()).collect()
}

fn places(program: &Program) -> Vec<&str> {
    program.modules.iter().flat_map(|one| &one.functions).flat_map(|one| &one.places).map(|one| one.name.as_str()).collect()
}

/// CSRLIN was an implicit variable, `CSRLIN%`, always 0: BC printed 23
/// where llrm printed 0.
#[test]
fn csrlin_reads_the_cursor_row() {
    for dialect in DIALECTS {
        let program = parsed("DEFINT A-Z\nLOCATE 23, 35\nr = CSRLIN\nPRINT r\n", dialect).expect("compiles");
        assert!(callees(&program).contains(&"B$CSRL"), "{dialect}");
        assert!(!places(&program).iter().any(|one| one.starts_with("CSRLIN")), "{dialect}: {:?}", places(&program));
    }
}

/// POS(0) was refused as an array POS.
#[test]
fn pos_reads_the_cursor_column() {
    for dialect in DIALECTS {
        let program = parsed("LOCATE 5, 7: PRINT \"ab\";\nc = POS(0)\nPRINT c\n", dialect).expect("compiles");
        assert!(callees(&program).contains(&"B$FPOS"), "{dialect}");
    }
}

/// STR$ of a LONG, SINGLE or DOUBLE had no runtime contract but on VBDOS:
/// "runtime call B$STR4 has no complete stack-cleanup contract".
#[test]
fn str_of_every_numeric_type_compiles() {
    let codegen = llrm_core::driver::Options::of(llrm_core::abi::machine::BASIC.clone());
    for dialect in DIALECTS {
        for (type_name, routine) in [("LONG", "B$STI4"), ("SINGLE", "B$STR4"), ("DOUBLE", "B$STR8")] {
            let program = parsed(&format!("DIM r AS {type_name}\nr = 1.5\nPRINT STR$(r)\n"), dialect).expect("parses");
            assert!(callees(&program).contains(&routine), "{dialect} {type_name}: the premise, STR$ calls {routine}");
            qb_compile::object_bytes(&program, Path::new("str.bas"), None, &codegen)
                .unwrap_or_else(|error| panic!("{dialect} {type_name}: {error}"));
        }
    }
}

/// A function keyword the table lacks is refused, never read as a
/// variable: DATE$ compiled to an empty string. A later dialect's keyword
/// stays a name in an earlier one.
#[test]
fn a_function_keyword_without_an_intrinsic_is_refused() {
    for dialect in DIALECTS {
        for (source, keyword) in [("PRINT DATE$\n", "DATE$"), ("x = ERDEV\n", "ERDEV"), ("x = VARPTR$(a)\n", "VARPTR$")] {
            let error = parsed(source, dialect).expect_err(source);
            assert!(error.contains(&format!("{keyword} is not supported")), "{dialect}: {error}");
        }
    }
    let error = parsed("x = STACK\n", "vbdos").expect_err("STACK is VBDOS's");
    assert!(error.contains("STACK is not supported"), "{error}");
    parsed("STACK = 1\nPRINT STACK\n", "qb45").expect("QB 4.5 has no STACK keyword");
}
