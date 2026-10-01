//! `-g`: CodeView line numbers, symbols and types.

use std::rc::Rc;

use llrm_core::model::passes::O2;
use llrm_core::objectfile::{cvinfo, omf};

use super::compile as qb_compile;
use super::driver as qb_driver;
use super::test_hir::{image, root, written};

/// The rich route's object of `source`, `includes` beside it.
fn object(source: &str, includes: &[(&str, &str)], dialect: &str, runtime: &str, debug: bool) -> Vec<Rc<omf::Record>> {
    let directory = tempfile::tempdir().expect("creates a directory");
    for (name, text) in includes {
        written(&directory, name, text.as_bytes());
    }
    let path = written(&directory, "debug.bas", source.as_bytes());
    let frontend = qb_driver::Frontend { debug, includes: vec![directory.path().to_path_buf()], ..qb_driver::Frontend::new(dialect, runtime) };
    let program = qb_driver::parsed(&path, &frontend, None).unwrap_or_else(|error| panic!("{dialect}: {error}"));
    let codegen = llrm_core::driver::Options::of(llrm_core::abi::machine::BASIC.clone());
    let bytes = qb_compile::object_bytes_by(&program, &path, None, &O2(), qb_compile::Route::Selected, &codegen).expect("compiles");
    omf::parse(&bytes).expect("parses")
}

fn lines(records: &[Rc<omf::Record>]) -> Vec<(u16, u16)> {
    records.iter().filter(|one| one.r#type == omf::LINNUM).flat_map(|one| omf::lines(one).1).collect()
}

fn marked(records: &[Rc<omf::Record>]) -> bool {
    records.iter().any(|one| one.r#type == omf::COMENT && one.body.get(1) == Some(&0xA1))
}

/// Each statement's main-file line at its code, an included file's that of
/// its $INCLUDE; with no variable to keep, the switch changes no code.
#[test]
fn debug_lines_are_the_main_files_statements() {
    let source = "DECLARE SUB Square (n AS INTEGER)\n'$INCLUDE: 'ONE.BI'\nPRINT 1\nSquare 2\nEND\nSUB Square (n AS INTEGER)\nPRINT n * n\nEND SUB\n";
    let includes = [("ONE.BI", "PRINT 98\nPRINT 99\n")];
    let debugged = object(source, &includes, "vbdos", "vbdos", true);
    assert!(marked(&debugged), "the CodeView marker");
    let found = lines(&debugged);
    let numbers: Vec<u16> = found.iter().map(|&(line, _)| line).collect();
    assert_eq!(numbers, [2, 3, 4, 5, 7], "{found:?}");
    // The module body starts after BASIC's 30h module header.
    assert_eq!(found[0].1, 0x30);
    assert!(found.windows(2).all(|two| two[0].1 < two[1].1), "{found:?}");
    let plain = object(source, &includes, "vbdos", "vbdos", false);
    assert!(lines(&plain).is_empty() && !marked(&plain));
    assert_eq!(image(&debugged, "DEBUG_CODE"), image(&plain, "DEBUG_CODE"));
}

/// What a debugger reads of `records`, BC's own DEF SEG variable aside:
/// QB 4.5 declares `__bseg%` in every module.
fn shape(records: &[Rc<omf::Record>]) -> Vec<String> {
    cvinfo::parse(records).shape().into_iter().filter(|one| !one.contains("__bseg%")).collect()
}

/// Each suite program's procedures, parameters and variables read as BC's
/// /Zi object of it does, in each dialect.
#[test]
fn debug_symbols_read_as_bc_writes_them() {
    for (program, fixture, dialect) in [
        ("byref2", "omf/byref2-q-o-zi", "qb45"),
        ("byref2", "cv/byref2-p-g2-zi", "pds71"),
        ("byref2", "cv/byref2-v-g3-zi", "vbdos"),
        ("cvonly/byval", "cv/byval-p-g2-zi", "pds71"),
        ("cvonly/byval", "cv/byval-v-g3-zi", "vbdos"),
        ("udt", "cv/udt-q-o-zi", "qb45"),
        ("udt", "cv/udt-p-g2-zi", "pds71"),
        ("udt", "cv/udt-v-g3-zi", "vbdos"),
        ("arrays", "cv/arrays-q-o-zi", "qb45"),
        ("arrays", "cv/arrays-p-g2-zi", "pds71"),
        ("arrays", "cv/arrays-v-g3-zi", "vbdos"),
        ("nestud", "omf/nestud-q-o-zi", "qb45"),
    ] {
        let source = std::fs::read_to_string(root().join(format!("tests/suite/{program}.bas"))).expect("reads");
        let bc = shape(&omf::read(root().join(format!("tests/fixtures/{fixture}.obj"))).expect("reads"));
        assert!(!bc.is_empty(), "{fixture} carries no symbols");
        assert_eq!(shape(&object(&source, &[], dialect, dialect, true)), bc, "{fixture}");
    }
}

/// A local's recorded offset is the frame slot its code writes, under
/// B$ENRA's frame, whose header moves every local, and the procedure's own.
#[test]
fn a_local_is_where_its_code_keeps_it() {
    let source = "SUB s\nDIM k AS INTEGER\nk = 12345\nPRINT k\nEND SUB\n";
    for own_frames in [false, true] {
        let directory = tempfile::tempdir().expect("creates a directory");
        let path = written(&directory, "local.bas", source.as_bytes());
        let frontend = qb_driver::Frontend { debug: true, own_frames, ..qb_driver::Frontend::new("vbdos", "vbdos") };
        let program = qb_driver::parsed(&path, &frontend, None).expect("parses");
        let codegen = llrm_core::driver::Options::of(llrm_core::abi::machine::BASIC.clone());
        let bytes = qb_compile::object_bytes_by(&program, &path, None, &O2(), qb_compile::Route::Selected, &codegen).expect("compiles");
        let info = cvinfo::parse(&omf::parse(&bytes).expect("parses"));
        let local = info.procedures.iter().flat_map(|one| &one.locals).find(|one| one.name == "k").expect("k is described");
        let listing = super::test_hir::rich_listing(&program);
        let store = format!("mov word ptr [bp{:+}], 12345", local.bp_offset);
        assert!(listing.contains(&store), "own frames {own_frames}: no {store:?} in\n{listing}");
    }
}

/// A module variable a debugger reads keeps its memory: its store dead
/// once the program ENDs, `m` was optimized away and missing from the
/// symbols, and CodeView could not evaluate it.
#[test]
fn a_folded_module_variable_is_still_described() {
    let source = "DECLARE SUB s (BYVAL v AS DOUBLE)\nDIM m AS DOUBLE\nm = 2\ns m\nEND\nSUB s (BYVAL v AS DOUBLE)\nPRINT v\nEND SUB\n";
    let records = object(source, &[], "vbdos", "vbdos", true);
    let names: Vec<String> = cvinfo::parse(&records).variables.into_iter().map(|one| one.name).collect();
    assert!(names.iter().any(|one| one == "m"), "{names:?}");
}

/// VBDOS names a procedure as its source spells it, the whole word: `s`
/// read as the `S` of `SUB`.
#[test]
fn a_name_is_spelled_as_its_whole_word() {
    let records = object("DECLARE SUB s ()\ns\nSUB s\nPRINT 1\nEND SUB\n", &[], "vbdos", "vbdos", true);
    let names: Vec<String> = cvinfo::parse(&records).procedures.into_iter().map(|one| one.name).collect();
    assert_eq!(names, ["s"]);
}
