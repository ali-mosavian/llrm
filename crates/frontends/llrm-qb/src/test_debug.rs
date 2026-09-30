//! `-g`: CodeView line numbers.

use llrm_core::objectfile::omf;

use super::driver as qb_driver;
use super::compile as qb_compile;
use llrm_core::model::passes::O2;

use super::test_hir::written;

/// The object's LINNUM pairs and whether it has the CodeView marker, with
/// its other records, for `source` including `ONE.BI`.
fn compiled(debug: bool) -> (Vec<(u16, u16)>, bool, Vec<std::rc::Rc<omf::Record>>) {
    let directory = tempfile::tempdir().expect("creates a directory");
    written(&directory, "ONE.BI", b"PRINT 98\nPRINT 99\n");
    let source = "DECLARE SUB Square (n AS INTEGER)\n'$INCLUDE: 'ONE.BI'\nPRINT 1\nSquare 2\nEND\nSUB Square (n AS INTEGER)\nPRINT n * n\nEND SUB\n";
    let path = written(&directory, "debug.bas", source.as_bytes());
    let frontend = qb_driver::Frontend { debug, includes: vec![directory.path().to_path_buf()], ..qb_driver::Frontend::new("vbdos", "vbdos") };
    let program = qb_driver::parsed(&path, &frontend, None).expect("parses");
    let codegen = llrm_core::driver::Options::of(llrm_core::abi::machine::BASIC.clone());
    let object = qb_compile::object_bytes_by(&program, &path, None, &O2(), qb_compile::Route::Selected, &codegen).expect("compiles");
    let (lines, rest): (Vec<_>, Vec<_>) = omf::parse(&object).expect("parses").into_iter().partition(|one| one.r#type == omf::LINNUM);
    let marker = rest.iter().any(|one| one.r#type == omf::COMENT && one.body.get(1) == Some(&0xA1));
    let rest = rest.into_iter().filter(|one| !(one.r#type == omf::COMENT && one.body.get(1) == Some(&0xA1))).collect();
    (lines.iter().flat_map(|one| omf::lines(one).1).collect(), marker, rest)
}

/// Each statement's main-file line at its code, an included file's that of
/// its $INCLUDE; the switch changes no code.
#[test]
fn debug_lines_are_the_main_files_statements() {
    let (lines, marker, code) = compiled(true);
    assert!(marker, "the CodeView marker");
    let numbers: Vec<u16> = lines.iter().map(|&(line, _)| line).collect();
    assert_eq!(numbers, [2, 3, 4, 5, 7], "{lines:?}");
    // The module body starts after BASIC's 30h module header.
    assert_eq!(lines[0].1, 0x30);
    assert!(lines.windows(2).all(|two| two[0].1 < two[1].1), "{lines:?}");
    let (none, no_marker, plain) = compiled(false);
    assert!(none.is_empty() && !no_marker);
    assert_eq!(code, plain);
}
