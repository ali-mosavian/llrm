//! `-g` on the Nib path: what CodeView reads of a compiled module.

use std::path::Path;
use std::rc::Rc;

use llrm_core::backend::objbuild::CodeLayout;
use llrm_core::objectfile::{cvinfo, omf};

const SOURCE: &str = "var counter: i16 = 5
var total: u32 = 70000

struct point:
    x: i16
    y: i32

fn scale(p: point, factor: i16) -> i32:
    let doubled: i32 = p.y * 2
    return doubled + factor

fn main() -> i16:
    let mut values: i16[4] = [1, 2, 3, 4]
    let origin = point(3, 40000)
    let small: u8 = 200
    let ratio: f32 = 1.5
    counter += values[2]
    let result = scale(origin, counter)
    if result > 0:
        print(\"ok\")
    return counter
";

fn object() -> Vec<Rc<omf::Record>> {
    compiled(false)
}

fn compiled(inlined: bool) -> Vec<Rc<omf::Record>> {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = directory.path().join("probe.nib");
    std::fs::write(&path, SOURCE).expect("writes");
    let frontend = crate::Frontend { debug: true, ..crate::Frontend::default() };
    let program = crate::driver::parsed(&path, &frontend, None).expect("parses");
    // Unless asked, not inlined: `scale` is a symbol and its lines are statements to read.
    let threshold = if inlined { llrm_transforms::inline::Threshold::default() } else { llrm_transforms::inline::Threshold::new(0) };
    let pipeline = llrm_transforms::pipeline::Options { inline: threshold, ..Default::default() };
    let options = llrm_core::driver::Options { pipeline, ..llrm_driver::code16_options(crate::compile::machine(&llrm_x86_code16::Code16, &crate::Frontend::default().os)) };
    let module = crate::compile::assembled(&program, "main", &options, &crate::Frontend::default().os).expect("compiles");
    omf::parse(&crate::compile::object(&module, Path::new("probe.nib"), CodeLayout::OneSegment).expect("writes")).expect("parses")
}

/// Each source parameter, local and module variable with its Nib type; no
/// compiler temporary.
#[test]
fn nib_symbols_read_with_their_types() {
    assert_eq!(
        cvinfo::parse(&object()).shape(),
        [
            "DATA counter: INTEGER",
            "DATA total: UNSIGNED LONG",
            "LOCAL main.origin: TYPE point {x +0 INTEGER, y +2 LONG}",
            "LOCAL main.ratio: SINGLE",
            "LOCAL main.result: LONG",
            "LOCAL main.small: UNSIGNED CHAR",
            "LOCAL main.values: 8 BYTES OF INTEGER",
            "LOCAL scale.doubled: LONG",
            "PARAM scale.factor: INTEGER",
            "PARAM scale.p: BYREF TYPE point",
            "PROC main flags 4 () -> INTEGER",
            "PROC scale flags 0 (BYREF TYPE point, INTEGER) -> LONG",
        ]
    );
}

/// Each statement's line.
#[test]
fn nib_lines_are_its_statements() {
    let lines: Vec<u16> = object().iter().filter(|one| one.r#type == omf::LINNUM).flat_map(|one| omf::lines(one).1).map(|(line, _)| line).collect();
    assert_eq!(lines, [9, 10, 13, 14, 15, 16, 17, 18, 19, 20, 21]);
}

/// `scale` inlined into `main` has no symbols of its own; its statements,
/// lines 9 and 10, are numbered where its code now is, between `main`'s.
#[test]
fn nib_inlined_code_keeps_its_lines_and_loses_its_symbols() {
    let object = compiled(true);
    let lines: Vec<u16> = object.iter().filter(|one| one.r#type == omf::LINNUM).flat_map(|one| omf::lines(one).1).map(|(line, _)| line).collect();
    assert_eq!(lines, [13, 14, 15, 16, 17, 9, 10, 18, 19, 20, 21]);
    let shape = cvinfo::parse(&object).shape();
    assert!(shape.iter().all(|one| !one.contains("scale")), "{shape:?}");
    assert!(shape.contains(&"PROC main flags 4 () -> INTEGER".to_owned()), "{shape:?}");
}
