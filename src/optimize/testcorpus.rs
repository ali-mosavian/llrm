//! Port of `tests/corpus.py`'s loaders, shared by every test that raises a fixture.
//!
//! Python memoises each stage; these recompute, since a `Module` holds `Rc`s
//! no test thread can share.

use std::path::PathBuf;
use std::rc::Rc;

use crate::abi::runtime::Contract;
use crate::frontend::blocks::{self, Block};
use crate::model::mir::{self, MirBody, RaisedBodies};
use crate::objectfile::module::{self, Module};
use crate::support::hash::IndexMap;

/// A fixture path, relative to the repository root as Python's tests name it.
pub(crate) fn fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

/// `corpus.loaded`: the object's module, which every fixture here has.
pub(crate) fn loaded(relative: &str) -> Rc<Module> {
    Rc::new(module::load(fixture(relative)).expect(relative).expect("not a BASIC object"))
}

/// `corpus.partitioned`: the module's blocks.
pub(crate) fn partitioned(found: &Module) -> Rc<Vec<Block>> {
    let mapped = blocks::code_map(found).expect("mapped");
    Rc::new(blocks::partition(found, &mapped))
}

/// `mir.bodies(found, blocks, contracts)`, with Python's keyword defaults.
pub(crate) fn raised(
    found: &Module,
    blocks: &[Block],
    contracts: Option<&mut IndexMap<i64, Contract>>,
) -> RaisedBodies {
    mir::bodies(found, blocks, contracts, false, false).expect("raised")
}

/// `mir.bodies(found, partitioned)[0][1]` with no contracts: the main body.
pub(crate) fn main_body(found: &Module, blocks: &[Block]) -> Rc<MirBody> {
    raised(found, blocks, None).values[0].1.clone()
}
