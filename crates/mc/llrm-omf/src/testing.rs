//! The corpus helpers of `tests/conftest.py` and `tests/corpus.py` that every
//! object-core test shares.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use llrm_support::hash::IndexMap;

use crate::module::{Group, Module, of};
use crate::omf;

/// conftest's `tests/inputs`.
pub fn fixtures() -> PathBuf {
    Path::new(env!("LLRM_ROOT")).join("tests/inputs/omf")
}

/// conftest's `obj`: every committed OMF object, sorted by name. Python's
/// `mapped_obj` was the same list.
pub fn objects() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(fixtures())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "obj"))
        .collect();
    found.sort();
    assert_eq!(found.len(), 523);
    found
}

/// `corpus.loaded`.
pub fn loaded(path: impl AsRef<Path>) -> Option<Module> {
    of(&omf::read(path).unwrap())
}

/// `Module(records, seg, name, code, start, end)` with every other field
/// defaulted.
pub fn bare(
    found: &Module,
    code: Vec<u8>,
    start: i64,
    end: i64,
) -> Module {
    Module {
        records: found.records.clone(),
        seg: found.seg,
        name: found.name.clone(),
        code,
        start,
        end,
        operands: IndexMap::default(),
        calls: IndexMap::default(),
        targets: BTreeSet::new(),
        publics: BTreeSet::new(),
        lines: BTreeSet::new(),
        chunks: Vec::new(),
        sites: BTreeSet::new(),
        fixup_at: IndexMap::default(),
        dgroup: Group::default(),
        program_data: None,
        refs: IndexMap::default(),
        float_protocols: IndexMap::default(),
        absorbed: IndexMap::default(),
        coverage: IndexMap::default(),
    }
}
