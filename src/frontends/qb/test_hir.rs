//! Ports of the `tests/test_hir.py` cases that exercise `qbopt/frontend/qb`
//! and `tools/qbstages.py`, plus `tests/test_qbstages.py` and the portable
//! `tests/test_qb_frontend_command.py` cases. The `qbopt/hir`-only cases are
//! in `src/hir/test_hir.rs`.
//!
//! Sources are parsed by `QBOPT_QBFRONT` when set, else by `cargo run` on
//! `crates/qbfront`, exactly as the driver does.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use super::compile::{self as qb_compile, CompileError};
use super::driver as qb_driver;
use crate::backend::masm;
use crate::hir::model::Program;
use crate::model::passes::O2;
use crate::objectfile::omf;

pub(super) fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// `tmp_path / name` holding `bytes`, in a directory that lives as long as the test.
pub(super) fn written(directory: &tempfile::TempDir, name: &str, bytes: &[u8]) -> PathBuf {
    let path = directory.path().join(name);
    std::fs::write(&path, bytes).expect("writes the source");
    path
}

/// `qb_driver.parsed(source, dialect=..., runtime=...)` with every other default.
pub(super) fn parsed_as(source: &Path, dialect: &str, runtime: &str) -> Program {
    qb_driver::parsed(source, &qb_driver::Frontend::new(dialect, runtime), None)
        .unwrap_or_else(|error| panic!("{}: {error}", source.display()))
}

/// `qb_driver.parsed(source)`.
pub(super) fn parsed(source: &Path) -> Program {
    parsed_as(source, "vbdos", "vbdos")
}

pub(super) fn fixture(name: &str) -> PathBuf {
    root().join("crates/qbfront/fixtures").join(name)
}

/// `qb_compile.assembled(program)`.
pub(super) fn assembled(program: &Program) -> Result<masm::Module, CompileError> {
    qb_compile::assembled(program, None, &O2())
}

/// `masm.text(qb_compile.assembled(program))`.
pub(super) fn listing(program: &Program) -> String {
    masm::text(&assembled(program).expect("assembles")).expect("prints")
}

/// `qb_compile.object_bytes(program, name)`.
pub(super) fn object_bytes(program: &Program, name: &str) -> Result<Vec<u8>, CompileError> {
    qb_compile::object_bytes(program, Path::new(name), None, &O2())
}

/// `omf.parse(qb_compile.object_bytes(program, name))`.
pub(super) fn records(program: &Program, name: &str) -> Vec<std::rc::Rc<omf::Record>> {
    omf::parse(&object_bytes(program, name).expect("emits")).expect("parses")
}

/// `text.split(start, 1)[1].split(end, 1)[0]`.
pub(super) fn between<'t>(text: &'t str, start: &str, end: &str) -> &'t str {
    let after = text.split_once(start).unwrap_or_else(|| panic!("{start:?} not in listing")).1;
    after.split_once(end).map_or(after, |(inside, _)| inside)
}

/// `(index, size)` of the named segment: `next(... if name == ...)`.
pub(super) fn segment(records: &[std::rc::Rc<omf::Record>], wanted: &str) -> (i64, i64) {
    omf::segments(records)
        .iter()
        .enumerate()
        .find_map(|(index, item)| match item {
            Some((name, size)) if name == wanted => Some((index as i64, *size)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no {wanted} segment"))
}

/// `omf.segment_image(records, index, size)` of the named segment.
pub(super) fn image(records: &[std::rc::Rc<omf::Record>], wanted: &str) -> Vec<u8> {
    let (index, size) = segment(records, wanted);
    omf::segment_image(records, index, size)
}
