//! Test support: `tests/corpus.py`'s loaders, without its caches, and the
//! Python test idioms the Rust API spells differently.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::analysis::regions::{self, RegionLayout};
use crate::frontend::blocks::{self, Block, CodeMap};
use crate::model::ir::decode::{self, BodyIR};
use crate::model::mir::{self, Arg, MemRef, MirBody, Op, RaisedBodies};
use crate::abi::runtime::Contract;
use crate::objectfile::module::{self, Group, Module};
use crate::support::hash::IndexMap;
use crate::objectfile::omf;
use crate::backend::cpu::ProfileOrName;
use crate::model::passes::{O2, Options};
use crate::optimize::transform;
use crate::wholeseg::{self, Emission, Emitted, Watch, Watched};

/// A path from the repo root, where Python's tests run.
pub fn path(relative: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

pub fn data(relative: impl AsRef<Path>) -> Vec<u8> {
    std::fs::read(path(relative)).unwrap()
}

/// `corpus.loaded`.
pub fn loaded(relative: impl AsRef<Path>) -> Option<Module> {
    loaded_bytes(&data(relative))
}

/// Frontend blocks as the `MirBlock`s `analysis::loops` walks; the walks read
/// only `at` and `succ`.
pub fn graph(blocks: &[Block]) -> Vec<mir::MirBlock> {
    let edges = |one: &Block| one.succ.iter().map(|&at| at as i64).collect();
    blocks.iter().map(|one| mir::MirBlock::new(one.at as i64, vec![], vec![], edges(one))).collect()
}

/// `corpus.loaded`, of an object's bytes.
pub fn loaded_bytes(data: &[u8]) -> Option<Module> {
    module::of(&omf::parse(data).unwrap())
}

fn _module(relative: impl AsRef<Path>) -> Module {
    loaded(relative).expect("not a BASIC object this pass can read")
}

/// `corpus.mapped`.
pub fn mapped(relative: impl AsRef<Path>) -> Result<CodeMap, String> {
    blocks::code_map(&_module(relative))
}

/// `corpus.partitioned`.
pub fn partitioned(relative: impl AsRef<Path>) -> Vec<Block> {
    partitioned_bytes(&data(relative))
}

/// `corpus.partitioned`, of an object's bytes.
pub fn partitioned_bytes(data: &[u8]) -> Vec<Block> {
    let found = loaded_bytes(data).expect("not a BASIC object this pass can read");
    let found_map = blocks::code_map(&found).unwrap();
    blocks::partition(&found, &found_map)
}

/// `corpus.bodies`: the decoded IR.
pub fn bodies(relative: impl AsRef<Path>) -> Result<Vec<BodyIR>, String> {
    decode::decode_module(&_module(relative))
}

/// `mir.bodies(corpus.loaded(path), corpus.partitioned(path))`.
pub fn raised(relative: impl AsRef<Path>) -> RaisedBodies {
    raised_with(relative, false, false)
}

/// `raised`, with `basic_semantics` and `bounds_checks`.
pub fn raised_with(relative: impl AsRef<Path>, basic_semantics: bool, bounds_checks: bool) -> RaisedBodies {
    let found = _module(&relative);
    mir::bodies(&found, &partitioned(&relative), None, basic_semantics, bounds_checks).unwrap()
}

/// `[op for block in body.blocks for op in block.ops]`.
pub fn ops(body: &MirBody) -> Vec<Op> {
    body.blocks.iter().flat_map(|block| block.ops.iter().cloned()).collect()
}

/// `[op for _, body in bodies for block in body.blocks for op in block.ops]`.
pub fn all_ops(raised: &RaisedBodies) -> Vec<Op> {
    raised.values.iter().flat_map(|(_, body)| ops(body)).collect()
}

/// The raised body Python finds as `bodies[index][1]`.
pub fn nth(raised: &RaisedBodies, index: usize) -> MirBody {
    MirBody::clone(&raised.values[index].1)
}

/// Python's `arg.width`, which a `Cell` or an `Opaque` does not have.
pub fn width(arg: &Arg) -> u32 {
    match arg {
        Arg::Held(one) => one.width,
        Arg::Const(one) => one.width,
        Arg::Symbol(one) => one.width,
        Arg::FrameAddress(one) => one.width,
        Arg::FrameSelector(one) => one.width,
        Arg::Cell(_) | Arg::Opaque(_) => panic!("{arg:?} has no width"),
    }
}

/// `mir.overlapping(one, other, dgroup)`; `None` is Python's `frozenset()`.
pub fn overlapping(one: &MemRef, other: &MemRef, dgroup: Option<&Group>) -> bool {
    let layout = dgroup.map(|group| RegionLayout { shared_segments: Some(group.shared.clone()), landmarks: Default::default() });
    regions::overlapping(one, other, None, None, layout.as_ref()).unwrap()
}

/// `corpus.loaded`, shared the way the optimizer's `Where.found` holds it.
pub fn module(relative: &str) -> Rc<Module> {
    Rc::new(_module(relative))
}

/// `corpus.partitioned`, of a module already loaded.
pub fn blocks_of(found: &Module) -> Rc<Vec<Block>> {
    Rc::new(blocks::partition(found, &blocks::code_map(found).expect("mapped")))
}

/// `mir.bodies(found, blocks, contracts)`, with Python's keyword defaults.
pub fn raised_from(found: &Module, blocks: &[Block], contracts: Option<&mut IndexMap<i64, Contract>>) -> RaisedBodies {
    mir::bodies(found, blocks, contracts, false, false).expect("raised")
}

/// `mir.bodies(found, blocks)[0][1]`: the main body.
pub fn main_body(found: &Module, blocks: &[Block]) -> Rc<MirBody> {
    raised_from(found, blocks, None).values[0].1.clone()
}

/// `corpus.runtime_library`: the BC runtime a fixture links against, or
/// None where Python's test skips because it is not installed.
pub fn runtime_library(obj: &str) -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var("HOME").ok()?);
    let name = Path::new(obj).file_name()?.to_str()?;
    let compiler = ["-p-", "-q-", "-v-"].into_iter().find(|one| name.contains(one))?;
    let library = match compiler {
        "-p-" => home.join("work/other/d32x/toolchains/pds71/LIB/BCL71ENR.LIB"),
        "-q-" => home.join("work/42-labs/mini-qb/dosbox/qb45/LIB/BCOM45.LIB"),
        _ => home.join("work/other/d32x/toolchains/vbdos/LIB/VBDCL10E.LIB"),
    };
    library.exists().then_some(library)
}

/// `wholeseg.emitted(data)`, with Python's keyword defaults.
pub fn emitted(data: &[u8]) -> Emitted {
    emitted_watching(data, None)
}

/// `wholeseg.emitted(data, watch=watch)`.
pub fn emitted_watching(data: &[u8], watch: Option<Watch<'_>>) -> Emitted {
    wholeseg::emitted(data, true, true, None, watch, ProfileOrName::Name("386"), false, false, None, &O2()).unwrap()
}

/// `wholeseg.emitted(data, basic_semantics=..., bounds_checks=...)`.
pub fn emitted_with(data: &[u8], basic_semantics: bool, bounds_checks: bool) -> Emitted {
    wholeseg::emitted(data, true, true, None, None, ProfileOrName::Name("386"), basic_semantics, bounds_checks, None, &O2())
        .unwrap()
}

/// `wholeseg.emitted(data, watch=...)`, keeping the MIR of each body named
/// `name...` at `stage`.
pub fn emitted_mir(data: &[u8], stage: &str, name: &str) -> (Emitted, Vec<MirBody>) {
    let mut states = vec![];
    let mut watch = |seen: &str, named: Option<&str>, low: Watched<'_>| {
        if let (true, Some(named), Watched::Mir(state)) = (seen == stage, named, low) {
            if named.starts_with(name) {
                states.push(state.clone());
            }
        }
    };
    let result = emitted_watching(data, Some(&mut watch));
    (result, states)
}

/// `wholeseg.emitted` of a fixture, asserting the LIR emitter wrote it.
pub fn emitted_lir(relative: impl AsRef<Path>) -> Emitted {
    let result = emitted(&data(relative));
    assert_eq!(result.outcome, Emission::Lir, "{}", result.reason);
    result
}

/// `[one.insn for block in corpus.partitioned(data) for one in block.insns]`.
pub fn instructions(data: &[u8]) -> Vec<iced_x86::Instruction> {
    partitioned_bytes(data).iter().flat_map(|block| block.insns.iter().map(|one| one.insn)).collect()
}

/// `transform.applied(body, dgroup, calls, found=found, blocks=blocks, options=options)`.
pub fn applied(found: &Rc<Module>, blocks: Option<&Rc<Vec<Block>>>, body: &Rc<MirBody>, options: Options) -> Rc<MirBody> {
    transform::applied(
        body,
        &found.dgroup.members,
        &found.calls,
        transform::Applied { blocks: blocks.cloned(), found: Some(found.clone()), options, ..Default::default() },
    )
    .unwrap()
}
