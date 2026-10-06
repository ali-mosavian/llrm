//! Ports of the `tests/test_hir.py` cases that exercise `qbopt/frontend/qb`
//! and `tools/qbstages.py`, plus `tests/test_qbstages.py` and the portable
//! `tests/test_qb_frontend_command.py` cases. The `qbopt/hir`-only cases are
//! in `crates/backend/llrm-core/src/hir/test_hir.rs`.
//!
//! Sources are compiled by qbfront, linked in, exactly as the driver does.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use super::compile::{self as qb_compile, CompileError};
use super::driver as qb_driver;
use llrm_core::backend::masm;
use llrm_core::hir::model::Program;
use llrm_core::objectfile::omf;

pub(super) fn root() -> PathBuf {
    PathBuf::from(env!("LLRM_ROOT"))
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

/// `parsed` with every procedure on the runtime's frame, as BC frames them.
pub(super) fn parsed_runtime_frames(source: &Path) -> Program {
    let frontend = qb_driver::Frontend { runtime_frames: true, ..qb_driver::Frontend::new("vbdos", "vbdos") };
    qb_driver::parsed(source, &frontend, None).unwrap_or_else(|error| panic!("{}: {error}", source.display()))
}

pub(super) fn fixture(name: &str) -> PathBuf {
    root().join("crates/frontends/qbfront/fixtures").join(name)
}

fn codegen() -> llrm_core::driver::Options {
    llrm_driver::m16_options(llrm_core::abi::machine::BASIC.clone())
}

/// `qb_compile.assembled(program)`.
pub(super) fn assembled(program: &Program) -> Result<masm::Module, CompileError> {
    qb_compile::assembled(program, None, &codegen())
}

/// `masm.text(qb_compile.assembled(program))`.
pub(super) fn listing(program: &Program) -> String {
    masm::text(&assembled(program).expect("assembles")).expect("prints")
}

/// The module's MIR as the front end emits it, as text.
pub(super) fn emitted_mir(program: &Program) -> String {
    let options = llrm_driver::m16_options(llrm_core::abi::machine::BASIC.clone());
    let (mir, _) = llrm_core::driver::emitted(program, &options).expect("emits");
    llrm_mir::print::module(&mir.modules[0])
}

/// The module's MIR after the pipeline, as text.
pub(super) fn optimized_mir(program: &Program) -> String {
    let options = llrm_driver::m16_options(llrm_core::abi::machine::BASIC.clone());
    let (mut mir, _) = llrm_core::driver::emitted(program, &options).expect("emits");
    llrm_core::driver::optimized(&mut mir, &options).expect("optimizes");
    llrm_mir::print::module(&mir.modules[0])
}

/// `qb_compile.object_bytes(program, name)`.
pub(super) fn object_bytes(program: &Program, name: &str) -> Result<Vec<u8>, CompileError> {
    qb_compile::object_bytes(program, Path::new(name), None, &codegen())
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
