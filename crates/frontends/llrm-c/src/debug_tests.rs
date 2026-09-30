//! `-g` on the C path: what CodeView reads of a unit wccq recorded under -d2.

use std::path::Path;
use std::rc::Rc;

use llrm_core::backend::omfwrite;
use llrm_core::objectfile::{cvinfo, omf};

/// tests/fixtures/c/debug.cgs, recorded from debug.c (and its debug.h)
/// with -d2, compiled as the CLI compiles it.
fn object() -> Vec<Rc<omf::Record>> {
    let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/debug.cgs")).expect("reads");
    let machine = llrm_core::abi::machine::Machine { cpu: "386".to_owned(), ..llrm_core::abi::machine::BUILT_IN.clone() };
    let built = super::compile::selected(&text, "debug", None, &llrm_core::driver::Options::of(machine)).expect("compiles");
    omf::parse(&omfwrite::written(&built, "debug.c").expect("writes")).expect("parses")
}

/// Every parameter, local, static and global with its C type; `(void)`
/// is no parameter, and its only one was `void`.
#[test]
fn c_symbols_read_with_their_types() {
    let shape = cvinfo::parse(&object()).shape();
    assert_eq!(
        shape,
        [
            "DATA ga: 20 BYTES OF INTEGER",
            "DATA gfp: BYREF CHAR",
            "DATA ghp: BYREF INTEGER",
            "DATA gp: TYPE pt {x +0 INTEGER, y +2 LONG}",
            "DATA gu: TYPE mix {b +0 UNSIGNED CHAR, w +0 UNSIGNED SHORT}",
            "DATA gul: UNSIGNED LONG",
            "DATA st: INTEGER",
            "LOCAL f.a: INTEGER",
            "LOCAL f.b: BYREF TYPE pt",
            "LOCAL f.c: CHAR",
            "LOCAL f.l: INTEGER",
            "LOCAL twice.x: INTEGER",
            "PROC f flags 4 (INTEGER, BYREF TYPE pt, CHAR) -> LONG",
            // Static: near.
            "PROC twice flags 0 (INTEGER) -> INTEGER",
            "PROC v flags 4 () -> STRING",
        ]
    );
}

/// Only the main file's lines: twice's, debug.h's, are none of debug.c's.
#[test]
fn c_lines_are_the_main_files() {
    let lines: Vec<u16> = object().iter().filter(|one| one.r#type == omf::LINNUM).flat_map(|one| omf::lines(one).1).map(|(line, _)| line).collect();
    assert_eq!(lines, [13, 15, 16, 17]);
}
