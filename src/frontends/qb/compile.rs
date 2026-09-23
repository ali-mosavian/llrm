//! Port of `qbopt/frontend/qb/compile.py`: QB HIR through the existing
//! machine pipeline to a fresh OMF module.
//!
//! This emission boundary is a procedure module: it emits far Pascal
//! SUB/FUNCTION bodies and their data inside the BASIC module envelope.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, LazyLock};

use iced_x86::Register;
use crate::support::hash::{IndexMap, IndexSet};

use super::abi::{physicalize, AbiError};
use super::inline_x87::finalized;
use crate::backend::cpu::{self as targets, ProfileOrName};
use crate::backend::{addressvalues, frame, lower, masm, omfwrite};
use crate::flow;
use crate::hir::lower::Lowered;
use crate::hir::{self, callmemory, model};
use crate::model::ir::{self, Loc, Operation, Semantics};
use crate::model::lir;
use crate::model::mir::{self, Arg, Kind, OpCode};
use crate::model::passes::Options;
use crate::objectfile::module::{Addr, Space};
use crate::objectfile::omf;
use crate::optimize::rotate;
use crate::support::pyrepr::{self, Repr};

/// HIR is valid but does not yet have a truthful BASIC object spelling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmissionError(pub String);

impl fmt::Display for EmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for EmissionError {}

/// Every exception the Python pipeline lets escape, by its message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileError {
    Emission(EmissionError),
    Abi(AbiError),
    /// Any other `ValueError` (and its subclasses) with Python's text.
    Value(String),
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Emission(one) => one.fmt(formatter),
            Self::Abi(one) => one.fmt(formatter),
            Self::Value(one) => formatter.write_str(one),
        }
    }
}

impl std::error::Error for CompileError {}

impl From<EmissionError> for CompileError {
    fn from(one: EmissionError) -> Self {
        Self::Emission(one)
    }
}

impl From<AbiError> for CompileError {
    fn from(one: AbiError) -> Self {
        Self::Abi(one)
    }
}

impl From<String> for CompileError {
    fn from(one: String) -> Self {
        Self::Value(one)
    }
}

fn emission<T>(message: impl Into<String>) -> Result<T, CompileError> {
    Err(CompileError::Emission(EmissionError(message.into())))
}

/// What a `Stage` carries: the state the compiler will use next.
#[derive(Clone, Copy, Debug)]
pub enum StageValue<'a> {
    Program(&'a model::Program),
    Lowered(&'a Lowered),
    Lir(&'a lir::LirBody),
    Module(&'a masm::Module),
}

/// One diagnostic view of the exact object-emission pipeline state.
///
/// `value` is the state the compiler will use next, not a reconstruction.
/// `callees` accompanies the final LIR because inline x87 selection and the
/// BASIC ABI are represented beside the body in the assembly model.
#[derive(Clone, Debug)]
pub struct Stage<'a> {
    pub name: String,
    pub function: Option<&'a model::Function>,
    pub value: StageValue<'a>,
    pub callees: Option<IndexMap<i64, masm::Callee>>,
}

pub type StageObserver<'o> = dyn FnMut(&Stage) -> Result<(), String> + 'o;

/// Report a compiler-owned stage without changing its production path.
fn _observe(
    observer: &mut Option<&mut StageObserver<'_>>,
    name: &str,
    value: StageValue<'_>,
    function: Option<&model::Function>,
    callees: Option<IndexMap<i64, masm::Callee>>,
) -> Result<(), CompileError> {
    if let Some(observer) = observer {
        observer(&Stage { name: name.to_owned(), function, value, callees })?;
    }
    Ok(())
}

const _READ_DATA_OBJECT: &str = "$qb$readData";
const _STATEMENT_TABLE_OBJECT: &str = "$qb$statementTable";

/// The linker spelling BC uses: uppercase and without a type suffix.
pub fn _object_name(name: &str) -> String {
    name.trim_end_matches(['%', '&', '!', '#', '$']).to_uppercase()
}

/// BC keeps source globals typed; compiler-owned data keeps `$D<n>`.
fn _data_name(module: &model::Module, object_: &model::DataObject) -> String {
    if object_.linkage == model::DataLinkage::Internal
        && !object_.name.starts_with('$')
        && !object_.name.ends_with("$static")
        && !object_.name.ends_with("$descriptor")
    {
        return object_.name.to_uppercase();
    }
    format!("{}$D{}", _object_name(&module.name), object_.id)
}

pub fn _empty_main(function: &model::Function) -> bool {
    function.blocks.iter().all(|block| {
        block.instructions.is_empty()
            && block.terminator.kind == model::TerminatorKind::Return
            && block.terminator.operands.is_empty()
    })
}

fn _bytes_of(values: &[i64]) -> Vec<u8> {
    values.iter().map(|one| *one as u8).collect()
}

type Names = IndexMap<(Space, i64), String>;

#[allow(clippy::type_complexity)]
fn _data(module: &model::Module) -> Result<(Names, IndexMap<String, Vec<masm::Datum>>), CompileError> {
    let mut names: Names = IndexMap::from_iter([((Space::Group, 0), "DGROUP".to_owned())]);
    let reserved = [_READ_DATA_OBJECT, _STATEMENT_TABLE_OBJECT];
    let internal: IndexMap<i64, &model::DataObject> = module
        .data
        .iter()
        .filter(|one| one.linkage == model::DataLinkage::Internal && !reserved.contains(&one.name.as_str()))
        .map(|one| (one.id, one))
        .collect();
    for object_ in &module.data {
        if reserved.contains(&object_.name.as_str()) {
            continue;
        }
        if object_.linkage == model::DataLinkage::External {
            names.insert((Space::External, object_.id), object_.name.clone());
        } else {
            names.insert((Space::Segment, object_.id), _data_name(module, object_));
        }
    }

    let mut grouped: IndexMap<String, Vec<masm::Datum>> =
        ["BC_DATA", "BC_CN", "FSL_CONST"].into_iter().map(|name| (name.to_owned(), Vec::new())).collect();
    for object_ in internal.values() {
        let segment = if matches!(object_.address, model::AddressKind::Far | model::AddressKind::Huge) {
            "FSL_CONST"
        } else if object_.readonly {
            "BC_CN"
        } else {
            "BC_DATA"
        };
        let items = &mut grouped[segment];
        let label = names[&(Space::Segment, object_.id)].clone();
        items.push(masm::Datum::Object(masm::Label { name: label }));
        let mut cursor = 0;
        let mut relocations: Vec<&model::DataRelocation> = object_.relocations.iter().collect();
        relocations.sort_by_key(|one| one.at);
        for relocation in relocations {
            let far = matches!(relocation.address, model::AddressKind::Far | model::AddressKind::Huge);
            let width = if far { 4 } else { 2 };
            if relocation.at < cursor || relocation.at + width > object_.bytes.len() as i64 {
                return emission(format!("{}: overlapping or out-of-range data relocation", object_.name));
            }
            if relocation.at != cursor {
                items.push(masm::Datum::Bytes(_bytes_of(&object_.bytes[cursor as usize..relocation.at as usize])));
            }
            let Some(target) = names.get(&(Space::Segment, relocation.target)).cloned() else {
                return emission(format!(
                    "{}: relocation names unknown data object {}",
                    object_.name, relocation.target
                ));
            };
            if relocation.address == model::AddressKind::Segment {
                if relocation.addend != 0 {
                    return emission(format!("{}: a segment selector cannot carry an offset", object_.name));
                }
                items.push(masm::Datum::SegmentWord(target));
            } else if far && internal[&relocation.target].address == model::AddressKind::Near {
                // A BASIC array descriptor's AD_fhd pointer to DGROUP data is
                // group-relative in both halves. BC emits an OFFSET fixup with
                // a DGROUP frame followed by the DGROUP selector. A single OMF
                // POINTER fixup selects the target segment instead; pairing
                // that selector with AD_oAdjusted's group-relative offset
                // shifted every formal-array access by BC_DATA's group offset.
                items.push(masm::Datum::Pointer(masm::Pointer { name: target, offset: relocation.addend, far: false }));
                items.push(masm::Datum::SegmentWord("DGROUP".to_owned()));
            } else {
                items.push(masm::Datum::Pointer(masm::Pointer { name: target, offset: relocation.addend, far }));
            }
            cursor = relocation.at + width;
        }
        if cursor != object_.bytes.len() as i64 {
            items.push(masm::Datum::Bytes(_bytes_of(&object_.bytes[cursor as usize..])));
        }
    }
    Ok((names, grouped))
}

/// Decode the frontend-owned DATA statement stream.
///
/// QB45 `rt/read.asm` starts at MODULE_CODE.OF_DS (`BC_DS+2`).  Each
/// NUL-terminated source-text line is followed by a two-byte code address;
/// `B$ReadVal` skips that word at end of line.  The semantic frontend keeps
/// only the text lines in this reserved object.  The OMF adapter supplies a
/// relocated, valid code address before every line and the measured
/// `FFFF 01` out-of-DATA sentinel after the last one.
fn _read_data_lines(module: &model::Module) -> Result<Vec<Vec<u8>>, CompileError> {
    let objects: Vec<&model::DataObject> = module.data.iter().filter(|one| one.name == _READ_DATA_OBJECT).collect();
    if objects.is_empty() {
        return Ok(Vec::new());
    }
    if objects.len() != 1 {
        return emission("a module may contain only one READ/DATA stream");
    }
    let object_ = objects[0];
    if object_.linkage != model::DataLinkage::Internal
        || !object_.readonly
        || !object_.relocations.is_empty()
        || object_.address != model::AddressKind::Near
    {
        return emission("the READ/DATA stream has an invalid storage contract");
    }
    let payload = _bytes_of(&object_.bytes);
    if payload.last() != Some(&0) {
        return emission("the READ/DATA stream must contain NUL-terminated lines");
    }
    let lines: Vec<Vec<u8>> = payload[..payload.len() - 1].split(|byte| *byte == 0).map(<[u8]>::to_vec).collect();
    if lines.iter().any(|line| line.is_empty() || !line.is_ascii()) {
        return emission("READ/DATA lines must be nonempty ASCII source text");
    }
    Ok(lines)
}

/// Serialize the ordered keys consumed by QB45's B$RSTB search.
///
/// BC places a final code address before every DATA row and passes the
/// matching address to B$RSTB. The frontend carries symbolic row labels until
/// code layout; object emission then writes their literal offsets, as BC does.
fn _read_data_items(module: &model::Module, labels: &IndexMap<i64, String>) -> Result<Vec<masm::Datum>, CompileError> {
    let lines = _read_data_lines(module)?;
    let have: BTreeSet<i64> = labels.keys().copied().collect();
    if have != (0..lines.len() as i64).collect() {
        return emission("DATA marker labels do not match the serialized DATA rows");
    }
    let mut items = Vec::new();
    for (row, line) in lines.into_iter().enumerate() {
        items.push(masm::Datum::Pointer(masm::Pointer { name: labels[&(row as i64)].clone(), offset: 0, far: false }));
        items.push(masm::Datum::Bytes([line, vec![0]].concat()));
    }
    Ok(items)
}

/// `struct.pack_into("<H", buffer, at, value)`.
fn pack_into(buffer: &mut [u8], at: usize, value: i64) {
    buffer[at..at + 2].copy_from_slice(&(value as u16).to_le_bytes());
}

/// Encode QB data, including the real-mode selector-only relocation.
fn _object_data(
    segment: &mut omfwrite::Segment,
    index: usize,
    items: &[masm::Datum],
    symbols: &mut IndexMap<String, (usize, usize)>,
) {
    for item in items {
        match item {
            masm::Datum::Label(masm::Label { name }) | masm::Datum::Object(masm::Label { name }) => {
                symbols.insert(name.clone(), (index, segment.image.len()));
            }
            masm::Datum::Fill(masm::Fill { size, byte: None }) => segment.skip(*size as usize),
            masm::Datum::Fill(masm::Fill { size, byte: Some(byte) }) => segment.put(&vec![*byte; *size as usize], &[]),
            masm::Datum::Pointer(masm::Pointer { name, offset, far }) => {
                let loc = if *far { omfwrite::POINTER } else { omfwrite::OFFSET };
                let wide = omfwrite::WIDE[&loc];
                segment.put(&vec![0; wide], &[omfwrite::Fixup::new(0, loc, name.clone())]);
                let at = segment.image.len() - wide;
                pack_into(&mut segment.image, at, offset & 0xFFFF);
            }
            masm::Datum::SegmentWord(name) => {
                segment.put(&[0, 0], &[omfwrite::Fixup::new(0, omfwrite::BASE, name.clone())]);
            }
            masm::Datum::Align(masm::Align { to }) => {
                segment.put(&vec![0; (-(segment.image.len() as i64)).rem_euclid(*to) as usize], &[]);
            }
            masm::Datum::Bytes(item) => segment.put(item, &[]),
        }
    }
}

pub fn _empty_procedure(name: &str) -> masm::Procedure {
    masm::Procedure {
        name: name.to_owned(),
        public: false,
        far: false,
        body: lir::LirBody::new(name, 1, vec![], IndexMap::default(), IndexMap::default()),
        reserve: 0,
        callees: IndexMap::default(),
    }
}

/// Decode frontend-private (function, block, first instruction, line) rows.
fn _statement_metadata(module: &model::Module) -> Result<Vec<(i64, i64, i64, i64)>, CompileError> {
    let objects: Vec<&model::DataObject> =
        module.data.iter().filter(|one| one.name == _STATEMENT_TABLE_OBJECT).collect();
    if objects.len() != 1 {
        return emission("a QB module must carry exactly one statement-table metadata object");
    }
    let object_ = objects[0];
    if object_.linkage != model::DataLinkage::Internal
        || !object_.readonly
        || !object_.relocations.is_empty()
        || object_.address != model::AddressKind::Near
        || object_.bytes.len() % 14 != 0
    {
        return emission("the statement-table metadata object has an invalid storage contract");
    }
    let payload = _bytes_of(&object_.bytes);
    let word = |at: usize, size: usize| -> i64 {
        payload[at..at + size].iter().rev().fold(0i64, |sum, byte| (sum << 8) | i64::from(*byte))
    };
    Ok((0..payload.len())
        .step_by(14)
        .map(|at| (word(at, 4), word(at + 4, 4), word(at + 8, 4), word(at + 12, 2)))
        .collect())
}

/// Statement rows excluding code reachable only inside the handler.
fn _statement_table_blocks(function: &model::Function) -> BTreeSet<i64> {
    let blocks: IndexMap<i64, &model::Block> = function.blocks.iter().map(|one| (one.id, one)).collect();
    let mut handler_only: BTreeSet<i64> = BTreeSet::new();
    let mut pending: Vec<i64> = function.error_handler.into_iter().collect();
    while let Some(block) = pending.pop() {
        if handler_only.contains(&block) || !blocks.contains_key(&block) {
            continue;
        }
        handler_only.insert(block);
        // RESUME NEXT's semantic successors are the possible runtime
        // destinations, not handler-owned statements. Stop the handler walk
        // at that transfer so post-error continuation rows remain eligible.
        if !blocks[&block].instructions.iter().any(|one| one.callee.as_deref() == Some("B$RESN")) {
            pending.extend(blocks[&block].terminator.targets.iter().copied());
        }
    }
    blocks.keys().copied().filter(|one| !handler_only.contains(one)).collect()
}

/// Remove semantic RESUME dispatch edges after they served allocation.
fn _drop_resume_successors(body: &lir::LirBody, calls: &IndexMap<i64, String>) -> lir::LirBody {
    body.with_blocks(body
            .blocks
            .iter()
            .map(|block| {
                if block.insns.iter().any(|instruction| calls.get(&instruction.at).map(String::as_str) == Some("B$RESN")) {
                    lir::LirBlock { succ: vec![], ..block.clone() }
                } else {
                    block.clone()
                }
            })
            .collect())
}

fn _insn(at: i64, what: Semantics) -> Arc<lir::Insn> {
    Arc::new(lir::Insn::new(at, Some((at, at)), Some(what), vec![], vec![]))
}

fn _semantics(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
    Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
}

fn _reg(register: Register) -> Loc {
    Loc::Reg(ir::Reg { register, width: 2 })
}

fn _label_imm(key: i64) -> Loc {
    Loc::Imm(ir::Imm { value: 0, width: 2, address: Some(Addr { index: key, ..Addr::new(Space::Segment, 0) }) })
}

/// Emit MODULE_CODE.OF_STA's relocated (offset,line) rows and zero end.
fn _statement_procedure(entries: &[(i64, i64, String, i64)]) -> masm::Procedure {
    let mut code: Vec<masm::InlinePart> = Vec::new();
    for (_procedure, _order, label, line) in entries {
        code.push(masm::InlinePart::Fixup("offset".into(), label.clone(), 0));
        code.push(masm::InlinePart::Bytes((*line as u16).to_le_bytes().to_vec()));
    }
    code.push(masm::InlinePart::Bytes(vec![0, 0]));
    let instruction = lir::Insn::new(1, None, Some(_semantics(Operation::Call, "statement-table", vec![], vec![])), vec![], vec![]);
    let body = lir::LirBody::new(
        "$QB$STAT",
        1,
        vec![lir::LirBlock::new(1, vec![Arc::new(instruction)])],
        IndexMap::default(),
        IndexMap::default(),
    );
    masm::Procedure {
        name: "$QB$STAT".into(),
        public: false,
        far: false,
        body,
        reserve: 0,
        callees: IndexMap::from_iter([(1, masm::Callee { name: "$statement-table".into(), far: false, code })]),
    }
}

/// Restore source-statement labels after MIR legitimately merged blocks.
///
/// The optimizer may merge straight-line source blocks, but RESUME's runtime
/// table needs an address at the first retained machine operation of each
/// statement. Split allocated LIR only at those operation identities; no
/// operation is copied, deleted, or re-ordered.
fn _split_statement_blocks(body: &lir::LirBody, markers: &BTreeSet<i64>) -> (lir::LirBody, IndexMap<i64, i64>) {
    let mut next_block = body.blocks.iter().map(|block| block.at).max().unwrap_or(0) + 1;
    let mut made: Vec<lir::LirBlock> = Vec::new();
    let mut labels: IndexMap<i64, i64> = IndexMap::default();
    let mut located: BTreeSet<i64> = BTreeSet::new();
    for block in &body.blocks {
        let mut positions: IndexMap<usize, Vec<i64>> = IndexMap::default();
        for (index, instruction) in block.insns.iter().enumerate() {
            // MIR addresses are unique within a body. MIR operation ids are
            // not a source-statement key: source instruction 22 and an
            // independently generated jump may both carry id 22.
            let source = instruction.op.as_ref().map(|op| op.at);
            if let Some(source) = source {
                if markers.contains(&source) && !located.contains(&source) {
                    positions.entry(index).or_default().push(source);
                    located.insert(source);
                }
            }
        }
        if positions.is_empty() {
            made.push(block.clone());
            continue;
        }
        let boundaries: Vec<usize> =
            std::iter::once(0).chain(positions.keys().copied()).collect::<BTreeSet<_>>().into_iter().collect();
        let mut ids = vec![block.at];
        ids.extend(next_block..next_block + boundaries.len() as i64 - 1);
        next_block += boundaries.len() as i64 - 1;
        for (segment, start) in boundaries.iter().enumerate() {
            let end = if segment + 1 < boundaries.len() { boundaries[segment + 1] } else { block.insns.len() };
            let at = ids[segment];
            let successors = if segment + 1 < ids.len() { vec![ids[segment + 1]] } else { block.succ.clone() };
            made.push(lir::LirBlock { at, succ: successors, ..block.with_insns(block.insns[*start..end].to_vec()) });
            for marker in positions.get(start).into_iter().flatten() {
                labels.insert(*marker, at);
            }
        }
    }
    (body.with_blocks(made), labels)
}

fn _sorted_repr(values: &BTreeSet<i64>) -> String {
    pyrepr::list(&values.iter().copied().collect::<Vec<_>>())
}

/// Insert B$RESA's measured AX target after final statement layout.
fn _resume_label_transfers(
    body: &lir::LirBody,
    resume_blocks: &IndexMap<i64, i64>,
    statement_instructions: &IndexMap<i64, i64>,
    statement_labels: &IndexMap<i64, i64>,
    procedure: usize,
    code_names: &mut IndexMap<i64, String>,
) -> Result<lir::LirBody, CompileError> {
    if resume_blocks.is_empty() {
        return Ok(body.clone());
    }
    let mut serial = body.blocks.iter().flat_map(|block| &block.insns).map(|one| one.at).max().unwrap_or(0) + 1;
    let mut blocks = Vec::new();
    let mut found: BTreeSet<i64> = BTreeSet::new();
    for block in &body.blocks {
        let mut instructions = Vec::new();
        for instruction in &block.insns {
            if let Some(target_block) = resume_blocks.get(&instruction.at) {
                let source_instruction = statement_instructions.get(target_block);
                let target = source_instruction.and_then(|one| statement_labels.get(one));
                let Some(target) = target else {
                    return emission(format!("RESUME target block {target_block} has no retained source statement"));
                };
                let key = -(code_names.len() as i64 + 1);
                code_names.insert(key, masm::label(procedure, *target));
                instructions.push(_insn(serial, _semantics(Operation::Move, "mov", vec![_reg(Register::AX)], vec![_label_imm(key)])));
                serial += 1;
                found.insert(instruction.at);
            }
            instructions.push(Arc::clone(instruction));
        }
        blocks.push(block.with_insns(instructions));
    }
    let missing: BTreeSet<i64> = resume_blocks.keys().copied().filter(|one| !found.contains(one)).collect();
    if !missing.is_empty() {
        return emission(format!("RESUME call sites vanished before final layout: {}", _sorted_repr(&missing)));
    }
    Ok(body.with_blocks(blocks))
}

/// Replace a typed placeholder push with RESTORE's relocated DATA label.
fn _restore_label_arguments(
    body: &lir::LirBody,
    restores: &IndexMap<i64, i64>,
    data_keys: &IndexMap<i64, i64>,
) -> Result<lir::LirBody, CompileError> {
    if restores.is_empty() {
        return Ok(body.clone());
    }
    let mut found: BTreeSet<i64> = BTreeSet::new();
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut instructions = block.insns.clone();
        for index in 0..instructions.len() {
            let Some(source) = instructions[index].op.as_ref().map(|op| op.at) else { continue };
            let Some(row) = restores.get(&source) else { continue };
            if index == 0 {
                return emission("RESTORE label argument was separated from its call");
            }
            let argument = &instructions[index - 1];
            let well_formed = argument.what.as_ref().is_some_and(|what| {
                what.op == Operation::Push && what.sources.len() == 1 && matches!(what.sources[0], Loc::Imm(_))
            });
            if !well_formed {
                return emission("RESTORE label lost its typed placeholder push");
            }
            let Some(key) = data_keys.get(row) else {
                return emission(format!("RESTORE names missing DATA row {row}"));
            };
            let mut replaced = (**argument).clone();
            let what = replaced.what.take().expect("checked above");
            replaced.what = Some(Semantics { sources: vec![_label_imm(*key)], ..what });
            instructions[index - 1] = Arc::new(replaced);
            found.insert(source);
        }
        blocks.push(block.with_insns(instructions));
    }
    let missing: BTreeSet<i64> = restores.keys().copied().filter(|one| !found.contains(one)).collect();
    if !missing.is_empty() {
        return emission(format!("RESTORE label calls vanished before final layout: {}", _sorted_repr(&missing)));
    }
    Ok(body.with_blocks(blocks))
}

/// Turn DATA marker calls into BC's one-byte labeled NOPs.
#[allow(clippy::too_many_arguments)]
fn _remove_data_markers(
    body: &lir::LirBody,
    markers: &IndexMap<i64, i64>,
    marker_labels: &IndexMap<i64, i64>,
    procedure: usize,
    data_keys: &IndexMap<i64, i64>,
    code_names: &mut IndexMap<i64, String>,
    callees: &mut IndexMap<i64, masm::Callee>,
) -> Result<lir::LirBody, CompileError> {
    if markers.is_empty() {
        return Ok(body.clone());
    }
    let mut found: BTreeSet<i64> = BTreeSet::new();
    let mut retained: BTreeSet<i64> = BTreeSet::new();
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut instructions = Vec::new();
        for instruction in &block.insns {
            let source = instruction.op.as_ref().map(|op| op.at);
            let row = source.and_then(|source| markers.get(&source));
            let (Some(source), Some(row)) = (source, row) else {
                instructions.push(Arc::clone(instruction));
                continue;
            };
            let label_block = marker_labels.get(&source);
            if label_block != Some(&block.at) {
                return emission(format!(
                    "DATA marker {source} row {row} is in block {}, labeled {}",
                    block.at,
                    label_block.map_or_else(|| "None".to_owned(), ToString::to_string)
                ));
            }
            let key = data_keys[row];
            code_names.insert(key, masm::label(procedure, block.at));
            found.insert(source);
            if !retained.contains(&source) {
                callees.shift_remove(&source);
                let ax = _reg(Register::AX);
                let mut replaced = (**instruction).clone();
                // XCHG AX,AX is opcode 90h, the exact NOP BC emits to
                // give each DATA row a distinct relocatable code key.
                replaced.what =
                    Some(_semantics(Operation::Exchange, "xchg", vec![ax.clone(), ax.clone()], vec![ax.clone(), ax]));
                replaced.defines = vec![];
                replaced.uses = vec![];
                replaced.clobbers = BTreeSet::new();
                replaced.clobbers_high = BTreeSet::new();
                instructions.push(Arc::new(replaced));
                retained.insert(source);
            }
        }
        blocks.push(block.with_insns(instructions));
    }
    let missing: BTreeSet<i64> = markers.keys().copied().filter(|one| !found.contains(one)).collect();
    if !missing.is_empty() {
        return emission(format!("DATA markers vanished before final layout: {}", _sorted_repr(&missing)));
    }
    Ok(body.with_blocks(blocks))
}

/// Return the measured BC `U_FLAG` word for this compilation profile.
///
/// `U_FLAG` is consumed by the BASIC runtime, not descriptive metadata.
/// These are the exact optimized /FPi profiles emitted by each compatible
/// compiler; VBDOS additionally records its /R array layout choice.
fn _compiler_switches(program: &model::Program) -> Result<i64, CompileError> {
    if program.float_mode == model::FloatMode::Alternate {
        if program.runtime != model::RuntimeProfile::Pds71 {
            return emission("alternate floating-point runtime is only measured for PDS 7.1");
        }
        return Ok(0x1088); // BC /O /FPa /G2
    }
    let mut flags = match program.runtime {
        model::RuntimeProfile::Qb45 => 0x1080,  // BC /O /FPi
        model::RuntimeProfile::Pds71 => 0x1084, // BC /O /FPi /G2
        model::RuntimeProfile::Vbdos => 0x12C4, // BC /O /FPi /G3 /E
        model::RuntimeProfile::Freestanding => {
            return Err(CompileError::Value(format!("KeyError: {}", program.runtime.repr())));
        }
    };
    if program.runtime == model::RuntimeProfile::Vbdos && program.array_order == model::ArrayOrder::RowMajor {
        flags |= 0x0100; // /R
    }
    Ok(flags)
}

fn _header(program: &model::Program) -> Result<Vec<u8>, CompileError> {
    let module = &program.modules[0];
    let object_name = _object_name(&module.name);
    if !object_name.is_ascii() {
        return Err(CompileError::Value(format!("'ascii' codec can't encode {}", pyrepr::string(&object_name))));
    }
    let mut name = object_name.as_bytes()[..object_name.len().min(8)].to_vec();
    name.resize(8, b' ');
    // MODULE_CODE in runtime/inc/addr.inc. Every symbolic word is an offset,
    // framed through DGROUP by the shared writer.
    let mut out: Vec<u8> = [b"bl".to_vec(), name, vec![0; 34], vec![0xff, 0xff]].concat();
    out.extend((_compiler_switches(program)? as u16).to_le_bytes());
    if out.len() != 48 {
        return Err(CompileError::Value("MODULE_CODE is exactly O_ENT bytes".into()));
    }
    // The addends live in the image. The remaining words are zero.
    pack_into(&mut out, 12, 2); // OF_DS is BC_DS + 2.
    Ok(out)
}

/// Spell BASIC module fallthrough as the runtime's implicit B$CENP.
fn _ends_program(body: &lir::LirBody) -> (lir::LirBody, IndexMap<i64, masm::Callee>) {
    let mut sites: IndexMap<i64, masm::Callee> = IndexMap::default();
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut instructions = Vec::new();
        let mut exits = false;
        for instruction in &block.insns {
            let mut instruction = Arc::clone(instruction);
            if instruction.what.as_ref().is_some_and(|what| what.op == Operation::Return) {
                exits = true;
                sites.insert(instruction.at, masm::Callee::new("B$CENP", true));
                let mut replaced = (*instruction).clone();
                replaced.what = Some(_semantics(Operation::Call, "call", vec![], vec![]));
                instruction = Arc::new(replaced);
            }
            instructions.push(instruction);
        }
        // Only the rewritten return becomes non-returning. Branch and jump
        // blocks retain their CFG edges; MASM listing uses the untaken edge to
        // insert an explicit jump when it is not the next laid-out block.
        let succ = if exits { vec![] } else { block.succ.clone() };
        blocks.push(lir::LirBlock { succ, ..block.with_insns(instructions) });
    }
    (lir::LirBody { noreturn: true, ..body.with_blocks(blocks) }, sites)
}

/// Replace source-positioned ON ERROR markers with the runtime protocol.
fn _materialize_error_registrations(
    body: &lir::LirBody,
    sites: &IndexMap<i64, (Option<Addr>, bool)>,
) -> (lir::LirBody, IndexMap<i64, masm::Callee>) {
    if sites.is_empty() {
        return (body.clone(), IndexMap::default());
    }
    let mut serial = body.blocks.iter().flat_map(|block| &block.insns).map(|one| one.at).max().unwrap_or(0) + 1;
    let mut callees: IndexMap<i64, masm::Callee> = IndexMap::default();
    let mut blocks: Vec<lir::LirBlock> = Vec::new();
    let ax = _reg(Register::AX);
    for block in &body.blocks {
        let mut made: Vec<Arc<lir::Insn>> = Vec::new();
        for instruction in &block.insns {
            let Some((address, local)) = sites.get(&instruction.at) else {
                made.push(Arc::clone(instruction));
                continue;
            };
            made.push(Arc::new(lir::Insn::new(
                instruction.at,
                instruction.covers,
                Some(_semantics(
                    Operation::Move,
                    "mov",
                    vec![ax.clone()],
                    vec![Loc::Imm(ir::Imm { value: 0, width: 2, address: *address })],
                )),
                vec![],
                vec![],
            )));
            let pushed = if *local {
                vec![ax.clone()]
            } else if address.is_some() {
                vec![_reg(Register::CS), ax.clone()]
            } else {
                vec![ax.clone(), ax.clone()]
            };
            for operand in pushed {
                made.push(_insn(serial, _semantics(Operation::Push, "push", vec![], vec![operand])));
                serial += 1;
            }
            let call_at = serial;
            made.push(_insn(call_at, _semantics(Operation::Call, "on-error-register", vec![], vec![])));
            serial += 1;
            callees.insert(call_at, masm::Callee::new(if *local { "B$OEGP" } else { "B$OEGA" }, true));
        }
        blocks.push(block.with_insns(made));
    }
    (body.with_blocks(blocks), callees)
}

/// Zero a native frame as BASIC's runtime entry routines do.
///
/// QB variables begin at zero, and runtime-managed string/array descriptors
/// require that invariant before their first assignment.  The shared backend
/// deliberately owns only reservation; this source ABI initialization stays
/// in the frontend and runs before any source instruction.
fn _initialize_frame(
    body: &lir::LirBody,
    size: i64,
) -> Result<(lir::LirBody, IndexMap<i64, masm::Callee>), CompileError> {
    let size = size + (size & 1);
    if size == 0 {
        return Ok((body.clone(), IndexMap::default()));
    }
    if size > 0x7FFE {
        return emission(format!("{}: {size} byte native frame exceeds a 16-bit BP displacement", body.name));
    }
    let at = body.blocks.iter().flat_map(|block| &block.insns).map(|one| one.at).max().unwrap_or(0) + 1;
    let initialize = _insn(at, _semantics(Operation::Call, "frame-zero", vec![], vec![]));
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            if block.at == body.entry {
                let insns = std::iter::once(Arc::clone(&initialize)).chain(block.insns.iter().cloned()).collect();
                block.with_insns(insns)
            } else {
                block.clone()
            }
        })
        .collect();
    let code: Vec<u8> = [
        vec![0x06, 0x57, 0x16, 0x07, 0x31, 0xc0, 0x8d, 0xbe],
        ((-size) as i16).to_le_bytes().to_vec(),
        vec![0xB9],
        ((size / 2) as u16).to_le_bytes().to_vec(),
        vec![0xfc, 0xf3, 0xab, 0x5f, 0x07],
    ]
    .concat();
    Ok((
        body.with_blocks(blocks),
        IndexMap::from_iter([(
            at,
            masm::Callee { name: "$frame_zero".into(), far: false, code: vec![masm::InlinePart::Bytes(code)] },
        )]),
    ))
}

#[allow(non_snake_case)]
fn _RUNTIME_FRAME_HEADER(runtime: model::RuntimeProfile) -> Result<i64, CompileError> {
    match runtime {
        model::RuntimeProfile::Qb45 => Ok(10),
        model::RuntimeProfile::Pds71 => Ok(18),
        model::RuntimeProfile::Vbdos => Ok(20),
        model::RuntimeProfile::Freestanding => Err(CompileError::Value(format!("KeyError: {}", runtime.repr()))),
    }
}

/// Enter and leave a BASIC runtime frame.
///
/// B$FCMD and the managed-string runtime consult BASIC's current frame, so a
/// merely zeroed C-style frame is not sufficient. B$ENRA itself pushes BP,
/// installs the BASIC frame chain, saves SI/DI, and allocates the local bytes;
/// B$EXSA reverses that work. The frontend therefore emits these procedures
/// without MASM's native shell and rebases only locals below the runtime
/// header.
///
/// BX is the maximum number of runtime-produced STRING temporaries an HIR
/// instruction consumes and produces together. This count must come from
/// resolved typed expressions, never from allocator spill slots.
fn _runtime_frame(
    body: &lir::LirBody,
    size: i64,
    runtime: model::RuntimeProfile,
    temporary_strings: i64,
) -> Result<(lir::LirBody, IndexMap<i64, masm::Callee>), CompileError> {
    let size = size + (size & 1);
    let header = _RUNTIME_FRAME_HEADER(runtime)?;
    if size > 0x7FFE {
        return emission(format!("{}: {size} byte BASIC frame exceeds a 16-bit BP displacement", body.name));
    }
    if !(0..=0xFFFF).contains(&temporary_strings) {
        return emission(format!("{}: too many temporary STRING slots", body.name));
    }
    let serial = body.blocks.iter().flat_map(|block| &block.insns).map(|one| one.at).max().unwrap_or(0) + 1;
    let imm = |value: i64| Loc::Imm(ir::Imm { value, width: 2, address: None });
    let enter = [
        _insn(serial, _semantics(Operation::Move, "mov", vec![_reg(Register::CX)], vec![imm(size)])),
        _insn(serial + 1, _semantics(Operation::Move, "mov", vec![_reg(Register::BX)], vec![imm(temporary_strings)])),
        _insn(serial + 2, _semantics(Operation::Call, "call", vec![], vec![])),
    ];
    let leave_at = serial + 3;
    let leave = _insn(leave_at, _semantics(Operation::Call, "call", vec![], vec![]));
    let framed: Vec<lir::LirBlock> = body
        .blocks
        .iter()
        .map(|block| {
            let insns = if block.at == body.entry {
                enter.iter().cloned().chain(block.insns.iter().cloned()).collect()
            } else {
                block.insns.clone()
            };
            block.with_insns(insns)
        })
        .collect();
    let framed: Vec<lir::LirBlock> = framed
        .into_iter()
        .map(|block| {
            let mut insns = Vec::new();
            for one in &block.insns {
                if one.what.as_ref().is_some_and(|what| what.op == Operation::Return) {
                    insns.push(Arc::clone(&leave));
                }
                insns.push(Arc::clone(one));
            }
            lir::LirBlock { insns, ..block }
        })
        .collect();

    let operand = |r#where: &Loc| -> Loc {
        // B$ENRA preserves the ordinary far-Pascal parameter offsets and
        // inserts its own header below BP, between BP and source locals.
        let moved = |addr: &Addr| {
            let disp = if addr.disp > 0 { addr.disp } else { addr.disp - header };
            Addr { disp, ..*addr }
        };
        match r#where {
            Loc::Mem(mem) if mem.addr.is_some_and(|addr| addr.space == Space::Frame) => {
                Loc::Mem(ir::Mem { addr: Some(moved(&mem.addr.unwrap())), ..mem.clone() })
            }
            Loc::Address(address) if address.addr.is_some_and(|addr| addr.space == Space::Frame) => {
                Loc::Address(ir::Address { addr: Some(moved(&address.addr.unwrap())), ..address.clone() })
            }
            other => other.clone(),
        }
    };
    let framed: Vec<lir::LirBlock> = framed
        .into_iter()
        .map(|block| {
            let insns = block
                .insns
                .iter()
                .map(|one| match &one.what {
                    Some(what) => {
                        let mut replaced = (**one).clone();
                        replaced.what = Some(Semantics {
                            dests: what.dests.iter().map(operand).collect(),
                            sources: what.sources.iter().map(operand).collect(),
                            ..what.clone()
                        });
                        Arc::new(replaced)
                    }
                    None => Arc::clone(one),
                })
                .collect();
            lir::LirBlock { insns, ..block }
        })
        .collect();
    Ok((
        body.with_blocks(framed),
        IndexMap::from_iter([(serial + 2, masm::Callee::new("B$ENRA", true)), (leave_at, masm::Callee::new("B$EXSA", true))]),
    ))
}

/// Count frame-owned dynamic STRING descriptors for B$ENRA.
///
/// Runtime-produced descriptors live on the runtime temporary chain and do
/// not request entries in the procedure-local handle block. The count is a
/// property of typed frame places, not expression-result liveness.
fn _temporary_string_slots(module: &model::Module, function: &model::Function) -> i64 {
    let types: IndexMap<i64, &model::Type> = module.types.iter().map(|one| (one.id, one)).collect();
    function
        .places
        .iter()
        .filter(|place| place.storage == model::Storage::Local && types[&place.r#type].name == "string")
        .count() as i64
}

/// Drop source-generated ESCAPE markers, which own no legacy bytes.
fn _source_instructions(body: &lir::LirBody) -> lir::LirBody {
    body.with_blocks(body
            .blocks
            .iter()
            .map(|block| block.with_insns(block.insns.iter().filter(|one| one.what.is_some()).cloned().collect()))
            .collect())
}

fn _handler_at(function: &model::Function) -> Option<i64> {
    function.error_handler
}

fn _parsed_target(name: &str, prefix: &str, message: &str) -> Result<i64, CompileError> {
    name.strip_prefix(prefix)
        .unwrap_or(name)
        .trim()
        .parse::<i64>()
        .or_else(|_| emission(format!("{message} {}", pyrepr::string(name))))
}

type Restored = IndexMap<i64, Option<mir::Op>>;

/// Expose explicit RESUME transfers while MIR memory optimization runs.
///
/// B$RESA never returns to the following instruction, but it does transfer to
/// a known source block after unwinding the handler.  Without that semantic
/// edge, DSE cannot see a local store in the handler reaching a load after
/// RESUME and deletes the store.  The temporary jumps are removed after
/// optimization.
fn _optimizer_resume_edges(body: &mir::MirBody) -> Result<(mir::MirBody, Restored), CompileError> {
    let labels: BTreeSet<i64> = body.blocks.iter().map(|block| block.at).collect();
    let mut serial = body.blocks.iter().flat_map(|block| &block.ops).map(|op| op.at).max().unwrap_or(0);
    let mut restored: Restored = IndexMap::default();
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let resumes: Vec<usize> = block
            .ops
            .iter()
            .enumerate()
            .filter(|(_, op)| op.kind == Kind::Call && op.name.starts_with("$QB$RESA:"))
            .map(|(index, _)| index)
            .collect();
        if resumes.is_empty() {
            blocks.push(block.clone());
            continue;
        }
        if resumes.len() != 1 || block.ops.is_empty() {
            return emission(format!("{}: malformed explicit RESUME transfer in block {}", body.entry, block.at));
        }
        let resume = resumes[0];
        let count = block.ops.len();
        let source_form = block.ops[count - 1].kind == Kind::Escape && count >= 2 && resume == count - 2;
        let physical_form = resume == count - 1 && block.succ.is_empty();
        if !source_form && !physical_form {
            return emission(format!("{}: malformed explicit RESUME transfer in block {}", body.entry, block.at));
        }
        let target = _parsed_target(&block.ops[resume].name, "$QB$RESA:", "invalid RESUME target marker")?;
        if !labels.contains(&target) {
            return emission(format!("RESUME target block {target} does not exist"));
        }
        serial += 1;
        let mut jump = mir::Op::new(serial, OpCode::Operation(Operation::Jump), "", vec![], vec![]);
        jump.kind = Kind::Jump;
        jump.target = Some(target);
        jump.reads_complete = true;
        restored.insert(serial, if source_form { Some(block.ops[count - 1].clone()) } else { None });
        let prefix = if source_form { &block.ops[..count - 1] } else { &block.ops[..] };
        let ops = prefix.iter().cloned().chain(std::iter::once(jump)).collect();
        blocks.push(mir::MirBlock { succ: vec![target], ..block.with_ops(ops) });
    }
    Ok((body.with_blocks(blocks), restored))
}

fn _drop_optimizer_resume_edges(body: &mir::MirBody, restored: &Restored) -> Result<mir::MirBody, CompileError> {
    if restored.is_empty() {
        return Ok(body.clone());
    }
    let mut missing: BTreeSet<i64> = restored.keys().copied().collect();
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let markers: Vec<usize> =
            block.ops.iter().enumerate().filter(|(_, op)| restored.contains_key(&op.at)).map(|(index, _)| index).collect();
        if markers.is_empty() {
            blocks.push(block.clone());
            continue;
        }
        if markers.len() != 1 || markers[0] != block.ops.len() - 1 {
            return emission(format!("optimizer moved the temporary RESUME edge in block {}", block.at));
        }
        let marker = &block.ops[markers[0]];
        missing.remove(&marker.at);
        let original = &restored[&marker.at];
        let ops = block.ops[..block.ops.len() - 1].iter().cloned().chain(original.iter().cloned()).collect();
        blocks.push(mir::MirBlock { succ: vec![], ..block.with_ops(ops) });
    }
    if !missing.is_empty() {
        return emission(format!("optimizer deleted temporary RESUME edges {}", _sorted_repr(&missing)));
    }
    Ok(body.with_blocks(blocks))
}

/// `tuple(dict.fromkeys((*function.external_entries, *handler)))`.
fn _external_entries(function: &model::Function, handler_at: Option<i64>) -> Vec<i64> {
    let mut entries: IndexSet<i64> = function.external_entries.iter().copied().collect();
    entries.extend(handler_at);
    entries.into_iter().collect()
}

/// Run the shared MIR fixed point at one QB compilation boundary.
fn _optimized(
    program: &model::Program,
    function: &model::Function,
    body: &Lowered,
    options: &Options,
) -> Result<Lowered, CompileError> {
    let Some(module) = program.modules.iter().find(|one| one.functions.contains(function)) else {
        return emission(format!("{}: function is not part of this program", function.name));
    };
    let target = targets::profile(ProfileOrName::Name("386"))?;
    let dgroup: BTreeSet<i64> = module
        .data
        .iter()
        .filter(|one| {
            one.linkage == model::DataLinkage::Internal
                && one.name != _READ_DATA_OBJECT
                && !matches!(one.address, model::AddressKind::Far | model::AddressKind::Huge)
        })
        .map(|one| one.id)
        .collect();
    let (optimizer_body, resume_edges) = _optimizer_resume_edges(&body.body)?;
    let semantic_calls: IndexMap<i64, String> = optimizer_body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|operation| operation.kind == Kind::Call)
        .map(|operation| (operation.at, operation.name.clone()))
        .collect();
    let entries = _external_entries(function, function.error_handler);
    // RESUME and ON ERROR enter these blocks from the runtime, independently
    // of the ordinary source predecessor.  Make every such edge visible while
    // optimization is running.
    let (rooted, temporary_root) = _machine_side_entry(&optimizer_body, &entries)?;
    let transformed = flow::optimized(
        &Rc::new(rooted),
        &dgroup,
        &semantic_calls,
        ProfileOrName::Profile(target),
        options.clone(),
        None,
        None,
        None,
        None,
    )?;
    let mut transformed = _drop_optimizer_resume_edges(&transformed, &resume_edges)?;
    if let Some(temporary_root) = temporary_root {
        transformed = mir::MirBody {
            entry: body.body.entry,
            blocks: transformed.blocks.iter().filter(|block| block.at != temporary_root).cloned().collect(),
            ..transformed
        };
    }
    let (checked, _root) = _machine_side_entry(&transformed, &entries)?;
    let problems = mir::verify(&checked);
    if !problems.is_empty() {
        return emission(format!(
            "optimized external-entry body is invalid: {}",
            pyrepr::list(&problems[..problems.len().min(3)])
        ));
    }
    Ok(Lowered { body: transformed, ..body.clone() })
}

/// Optimize source MIR while preserving QB ABI side entries and RESUME semantics.
pub fn optimized(
    program: &model::Program,
    function: &model::Function,
    body: &Lowered,
    options: &Options,
) -> Result<Lowered, CompileError> {
    _optimized(program, function, body, options)
}

/// Optimize MIR introduced by ABI physicalization.
///
/// RESUME's target edge remains semantically necessary for memory
/// optimization, so expose the physical form and then remove only the
/// temporary edge rather than restoring a source marker.
pub fn optimized_physical(
    program: &model::Program,
    function: &model::Function,
    body: &Lowered,
    options: &Options,
) -> Result<Lowered, CompileError> {
    _optimized(program, function, body, options)
}

static LOWERING_TARGET: LazyLock<targets::Profile> = LazyLock::new(|| {
    let target = targets::profile(ProfileOrName::Name("386")).expect("the 386 profile exists").clone();
    let address_forms = target.address_forms.iter().filter(|form| !form.secondary).cloned().collect();
    targets::Profile { address_forms, ..target }
});

/// The 386 profile with only address forms valid for QB far-array HIR.
///
/// A dynamic BASIC array explicitly loads both words of its far data pointer.
/// The generic secondary SIB folder widens those word definitions before the
/// far-load selector combines them into LES, leaving the folder's promoted
/// values without definitions. Keep native 16-bit forms; only the conflicting
/// secondary form is outside this frontend's lowering contract.
pub fn lowering_target() -> &'static targets::Profile {
    &LOWERING_TARGET
}

/// Give the one-entry machine pipeline a temporary external-entry switch.
///
/// An empty block with several successors is sufficient for graph analyses,
/// but it is not a machine CFG: no instruction chooses an edge.  Use an
/// unconstrained entry value and a real semantic switch so every machine
/// phase sees valid control flow.  The switch and every comparison block its
/// lowering creates are discarded before source ABI emission.
pub(crate) fn _machine_side_entry(body: &mir::MirBody, entries: &[i64]) -> Result<(mir::MirBody, Option<i64>), CompileError> {
    // A statement's original block may have been merged into an earlier block
    // by MIR optimization.  This root exists only to retain independently
    // reachable blocks which are still physical CFG roots.
    let labels: BTreeSet<i64> = body.blocks.iter().map(|block| block.at).collect();
    let entries: Vec<i64> = std::iter::once(body.entry)
        .chain(entries.iter().copied())
        .collect::<IndexSet<_>>()
        .into_iter()
        .filter(|one| labels.contains(one))
        .collect();
    if entries.len() == 1 {
        return Ok((body.clone(), None));
    }
    let root = body.blocks.iter().map(|block| block.at).max().unwrap_or(0) + 1;
    let operation_at = body.blocks.iter().flat_map(|block| &block.ops).map(|op| op.at).max().unwrap_or(0) + 1;
    let value_id = body
        .blocks
        .iter()
        .flat_map(|block| {
            block.phis.iter().map(|phi| phi.result).chain(
                block.ops.iter().flat_map(|op| op.defines.iter().chain(&op.uses).copied()),
            )
        })
        .map(|value| value.id)
        .max()
        .unwrap_or(0)
        + 1;
    let selector = mir::Value { variable: value_id, version: 1, ..mir::Value::new(value_id, operation_at) };
    let mut switch = mir::Op::new(operation_at, OpCode::Operation(Operation::Jump), "", vec![], vec![selector]);
    switch.kind = Kind::Switch;
    switch.args = vec![Arg::Held(mir::Held { value: selector, width: 2 })];
    // The default is the real language entry.  FinalControlFlow lays a
    // switch's default path after its comparison chain; once that
    // analysis-only chain is removed, the BASIC runtime must still find
    // the ordinary entry at O_ENT rather than an error-handler side root.
    switch.target = Some(entries[0]);
    switch.cases = entries[1..].iter().enumerate().map(|(number, target)| (number as i64 + 1, *target)).collect();
    switch.reads_complete = true;
    let made = mir::MirBody { entry: root, ..body.with_blocks(std::iter::once(mir::MirBlock::new(root, vec![], vec![switch], entries))
            .chain(body.blocks.iter().cloned())
            .collect()) };
    let problems = mir::verify(&made);
    if !problems.is_empty() {
        return emission(format!("temporary ON ERROR root is invalid: {}", pyrepr::list(&problems[..problems.len().min(3)])));
    }
    Ok((made, Some(root)))
}

fn _drop_machine_side_entry(
    body: &lir::LirBody,
    roots: &BTreeSet<i64>,
    entry: i64,
    entry_fallback: Option<i64>,
) -> Result<lir::LirBody, CompileError> {
    if roots.is_empty() {
        return Ok(body.clone());
    }
    let mut blocks: Vec<lir::LirBlock> = body.blocks.iter().filter(|block| !roots.contains(&block.at)).cloned().collect();
    let labels: BTreeSet<i64> = blocks.iter().map(|block| block.at).collect();
    if !labels.contains(&entry) {
        // FinalControlFlow may thread an empty source entry into its first
        // statement. The source ABI still needs a distinct pre-statement
        // location for B$ENRA and ON LOCAL ERROR registration: putting those
        // on the resumable statement itself would re-enter the frame when the
        // runtime resumes there. Recreate only that ABI wrapper and make its
        // transfer explicit, since machine layout has already run.
        let Some(entry_fallback) = entry_fallback.filter(|one| labels.contains(one)) else {
            return emission(format!("{}: machine pipeline removed the BASIC entry target", body.name));
        };
        let at = blocks.iter().flat_map(|block| &block.insns).map(|one| one.at).max().unwrap_or(0) + 1;
        let jump = _insn(at, Semantics { target: Some(entry_fallback), ..(_semantics(Operation::Jump, "jmp", vec![], vec![])) });
        blocks.insert(0, lir::LirBlock { succ: vec![entry_fallback], ..lir::LirBlock::new(entry, vec![jump]) });
    }
    Ok(lir::LirBody { entry, ..body.with_blocks(blocks) })
}

#[allow(non_snake_case)]
fn _SEGMENT_SHAPE(name: &str) -> Option<(u8, &'static str)> {
    Some(match name {
        "BR_DATA" => (0x68, "BLANK"),
        "BR_SKYS" => (0x68, "BLANK"),
        "COMMON" => (0x78, "BLANK"),
        "BC_DATA" => (0x48, "BC_DATA"),
        "NMALLOC" => (0x58, "BC_VARS"),
        "ENMALLOC" => (0x58, "BC_VARS"),
        "BC_FT" => (0x48, "BC_SEGS"),
        "BC_CN" => (0x68, "BC_SEGS"),
        "BC_DS" => (0x68, "BC_SEGS"),
        "BC_SAB" => (0x48, "BC_SEGS"),
        "BC_SA" => (0x48, "BC_SEGS"),
        "FDATA" => (0x60, "FAR_DATA"),
        "FSL_CONST" => (0x60, "FAR_DATA"),
        _ => return None,
    })
}

/// Apply the BASIC segment classes/combine modes to a fresh OMF envelope.
fn _basic_segment_classes(data: &[u8], code: &str) -> Result<Vec<u8>, CompileError> {
    let value = |error: omf::ValueError| CompileError::Value(error.0);
    let records = omf::parse(data).map_err(value)?;
    let old_names = omf::names(&records);
    let mut names = old_names.clone();
    for name in ["BC_CODE", "BLANK", "BC_DATA", "BC_VARS", "BC_SEGS"] {
        if !names.iter().any(|one| one == name) {
            names.push(name.to_owned());
        }
    }
    let mut name_index: IndexMap<String, i64> = IndexMap::default();
    for (index, name) in names.iter().enumerate() {
        if index != 0 {
            name_index.insert(name.clone(), index as i64);
        }
    }
    let latin1 = |text: &str| -> Vec<u8> { text.chars().map(|one| one as u32 as u8).collect() };
    let mut rewritten: Vec<omf::Record> = Vec::new();
    let mut lnames_done = false;
    for record in &records {
        if record.r#type & 0xFE == omf::LNAMES {
            if lnames_done {
                return emission("fresh OMF unexpectedly contains multiple LNAMES records");
            }
            let body: Vec<u8> = names[1..]
                .iter()
                .flat_map(|name| {
                    let encoded = latin1(name);
                    std::iter::once(encoded.len() as u8).chain(encoded)
                })
                .collect();
            rewritten.push(omf::Record::new(record.r#type, body));
            lnames_done = true;
            continue;
        }
        if record.r#type & 0xFE != omf::SEGDEF {
            rewritten.push((**record).clone());
            continue;
        }
        let body = &record.body;
        let at = 1 + if body[0] >> 5 == 0 { 3 } else { 0 } + 2;
        let (segment_name_index, after_name) = omf::_index(body, at);
        let (_class_index, after_class) = omf::_index(body, after_name);
        let (overlay_index, after_overlay) = omf::_index(body, after_class);
        let segment_name = &old_names[segment_name_index as usize];
        let (acbp, class_name) = if segment_name == code {
            (0x68, "BC_CODE".to_owned())
        } else {
            match _SEGMENT_SHAPE(segment_name) {
                Some((acbp, class_name)) => (acbp, class_name.to_owned()),
                None => (body[0], old_names[_class_index as usize].clone()),
            }
        };
        let made: Vec<u8> = [
            vec![acbp],
            body[1..at].to_vec(),
            omf::as_index(segment_name_index).map_err(value)?,
            omf::as_index(name_index[&class_name]).map_err(value)?,
            omf::as_index(overlay_index).map_err(value)?,
            body[after_overlay..].to_vec(),
        ]
        .concat();
        rewritten.push(omf::Record::new(record.r#type, made));
    }
    Ok(rewritten.iter().flat_map(omf::Record::emit).collect())
}

/// Apply the shared source-level call-graph mod/ref fixed point.
pub(crate) fn _alias_annotated(
    module: &model::Module,
    functions: &[model::Function],
    semantic: &[Lowered],
) -> Result<Vec<Lowered>, CompileError> {
    callmemory::annotated(module, functions, semantic, Some(&_object_name))
        .map_err(|error| EmissionError(error).into())
}

#[allow(non_snake_case)]
fn _SCREEN_DRIVER(mode: i64) -> Option<&'static str> {
    Some(match mode {
        1 | 2 => "B$CGAUSED",
        3 => "B$HRCUSED",
        4 => "B$OLIUSED",
        7..=10 => "B$EGAUSED",
        11..=13 => "B$VGAUSED",
        _ => return None,
    })
}

/// Name the runtime graphics modules selected by source SCREEN calls.
///
/// Microsoft BC emits a reference to a mode-specific public for a constant
/// mode and B$GRPUSED for an expression.  The reference is a linker switch,
/// not a call: without it B$CSCN is present but has no device implementation
/// and reports BASIC error 5.
fn _graphics_dependencies(module: &model::Module) -> BTreeSet<String> {
    let mut required: BTreeSet<String> = BTreeSet::new();
    for function in &module.functions {
        for block in &function.blocks {
            for instruction in &block.instructions {
                if instruction.op != model::Op::Call || instruction.callee.as_deref() != Some("B$CSCN") {
                    continue;
                }
                let model::Operand::Constant(mode) = &instruction.operands[1] else {
                    required.insert("B$GRPUSED".to_owned());
                    continue;
                };
                let number = match mode.value {
                    model::Number::Int(one) => one,
                    model::Number::Float(one) => one.trunc() as i64,
                };
                if number != 0 {
                    required.insert(_SCREEN_DRIVER(number).unwrap_or("B$GRPUSED").to_owned());
                }
            }
        }
    }
    required
}

fn _checked(error: flow::Checked) -> CompileError {
    CompileError::Value(match error {
        flow::Checked::Refused(raised) => raised.message,
        flow::Checked::Malformed(malformed) => malformed.0,
    })
}

/// Compile one QB HIR module to the shared assembly model.
pub fn assembled(
    program: &model::Program,
    mut observer: Option<&mut StageObserver<'_>>,
    options: &Options,
) -> Result<masm::Module, CompileError> {
    hir::verify::verify(program).map_err(|error| CompileError::Value(error.0))?;
    _observe(&mut observer, "hir", StageValue::Program(program), None, None)?;
    if program.modules.len() != 1 {
        return emission("one OMF object represents exactly one QB module");
    }
    let module = &program.modules[0];
    let graphics = _graphics_dependencies(module);
    let semantic = hir::lower::lower(program).map_err(|error| CompileError::Value(error.0))?;
    let functions: Vec<&model::Function> = module.functions.iter().collect();
    if semantic.len() != functions.len() {
        return emission("HIR lowering did not preserve the function table");
    }
    let semantic = _alias_annotated(module, &module.functions, &semantic)?;

    let callable_names: IndexMap<&str, String> =
        module.callables.iter().map(|one| (one.name.as_str(), _object_name(&one.name))).collect();
    let defined: BTreeSet<String> =
        module.callables.iter().filter(|one| one.defined).map(|one| _object_name(&one.name)).collect();
    let mut procedures: Vec<masm::Procedure> = Vec::new();
    let data_rows = _read_data_lines(module)?;
    let data_keys: IndexMap<i64, i64> = (0..data_rows.len() as i64).map(|row| (row, -(row + 1))).collect();
    let mut code_names: IndexMap<i64, String> = data_keys.values().map(|key| (*key, String::new())).collect();
    let statement_metadata = _statement_metadata(module)?;
    let mut statement_targets: Vec<(i64, i64, String, i64)> = Vec::new();
    let mut referenced_calls: BTreeSet<String> = BTreeSet::new();
    let empty_occurrences = IndexMap::default();
    for (function, body) in functions.iter().copied().zip(&semantic) {
        let handler_at = _handler_at(function);
        _observe(&mut observer, "source-mir", StageValue::Lowered(body), Some(function), None)?;
        let body = optimized(program, function, body, options)?;
        _observe(&mut observer, "optimized-mir", StageValue::Lowered(&body), Some(function), None)?;
        let mut physical = physicalize(program, function, &body)?;
        _observe(&mut observer, "physical-mir", StageValue::Lowered(&physical.lowered), Some(function), None)?;
        // ABI physicalization is still MIR production: it introduces concrete
        // parameter loads, return extracts, call arguments, and frame copies.
        // Feed those operations through the same fixed point as source MIR so
        // code quality cannot depend on whether a frontend expressed work
        // before or during ABI adaptation.
        physical.lowered = optimized_physical(program, function, &physical.lowered, options)?;
        _observe(&mut observer, "optimized-physical-mir", StageValue::Lowered(&physical.lowered), Some(function), None)?;
        let ordinary_entry = physical.lowered.body.entry;
        let ordinary_block = physical.lowered.body.block(ordinary_entry);
        let ordinary_fallback = ordinary_block.filter(|block| block.succ.len() == 1).map(|block| block.succ[0]);
        let external_entries = _external_entries(function, handler_at);
        let (machine_body, temporary_root) = _machine_side_entry(&physical.lowered.body, &external_entries)?;
        let machine_body =
            rotate::entered(&Rc::new(machine_body)).map_err(|error| CompileError::Value(error.to_string()))?;
        let rotated = Lowered { body: mir::MirBody::clone(&machine_body), ..physical.lowered.clone() };
        _observe(&mut observer, "rotated-mir", StageValue::Lowered(&rotated), Some(function), None)?;
        let low = lower::lowered(
            &body.name,
            &machine_body,
            Some(&physical.calls),
            BTreeSet::new(),
            Some(&physical.contracts),
            ProfileOrName::Profile(lowering_target()),
            lower::Lowered {
                occurrences: Some(&empty_occurrences),
                hints: Some(&physical.hints),
                pointer_model: Some(physical.pointer_model.clone()),
                ..Default::default()
            },
        )
        .map_err(|error| CompileError::Value(error.0))?;
        let mut low = flow::verified(low, "lower", true).map_err(|error| CompileError::Value(error.0))?;
        low.source_order = function.error_handler.is_some();
        _observe(&mut observer, "initial-lir", StageValue::Lir(&low), Some(function), None)?;
        let temporary_blocks: BTreeSet<i64> = if temporary_root.is_some() {
            let physical_blocks: BTreeSet<i64> = physical.lowered.body.blocks.iter().map(|block| block.at).collect();
            low.blocks.iter().map(|block| block.at).filter(|at| !physical_blocks.contains(at)).collect()
        } else {
            BTreeSet::new()
        };
        let owned_frame = frame::of(&low, Some(&physical.calls), program.runtime.value(), None)
            .map_err(|error| CompileError::Value(error.0))?;
        let owned_frame = Rc::new(RefCell::new(owned_frame));
        let mut in_ssa = true;
        let mut phases = flow::machine(
            &IndexMap::default(),
            Some(Rc::clone(&owned_frame)),
            Some(&physical.calls),
            true,
            ProfileOrName::Name("386"),
        )?;
        for phase in phases.iter_mut() {
            // masm.Procedure owns a native BP frame and reserves the complete
            // local/spill extent. The shared Prologue pass is for an already
            // existing BC/runtime frame and would reserve the spill tail twice.
            if phase.class_name() == "Prologue" {
                continue;
            }
            if phase.class_name() == "PhiElimination" {
                in_ssa = false;
            }
            low = flow::checked(low, phase.as_mut(), in_ssa).map_err(_checked)?;
            _observe(&mut observer, &format!("machine:{}", phase.name()), StageValue::Lir(&low), Some(function), None)?;
        }
        let low = _drop_machine_side_entry(&low, &temporary_blocks, ordinary_entry, ordinary_fallback)?;
        let final_ = finalized(&low, function.abi.as_ref().map_or(0, |abi| abi.parameter_bytes))?;
        let mut callees = final_.callees.clone();
        let mut resume_blocks: IndexMap<i64, i64> = IndexMap::default();
        let mut data_markers: IndexMap<i64, i64> = IndexMap::default();
        let mut restore_markers: IndexMap<i64, i64> = IndexMap::default();
        let mut error_registrations: IndexMap<i64, (Option<Addr>, bool)> = IndexMap::default();
        let mut error_labels: IndexMap<i64, i64> = IndexMap::default();
        for (at, name) in &physical.calls {
            let object_name: String;
            if name.starts_with("$QB$RESA:") {
                let target_block = _parsed_target(name, "$QB$RESA:", "invalid RESUME target marker")?;
                resume_blocks.insert(*at, target_block);
                object_name = "B$RESA".to_owned();
            } else if name.starts_with("$QB$DATA:") {
                let row = _parsed_target(name, "$QB$DATA:", "invalid DATA marker")?;
                if !data_keys.contains_key(&row) {
                    return emission(format!("DATA marker names missing row {row}"));
                }
                data_markers.insert(*at, row);
                continue;
            } else if name.starts_with("$QB$RSTB:") {
                let row = _parsed_target(name, "$QB$RSTB:", "invalid RESTORE marker")?;
                if !data_keys.contains_key(&row) {
                    return emission(format!("RESTORE names missing DATA row {row}"));
                }
                restore_markers.insert(*at, row);
                object_name = "B$RSTB".to_owned();
            } else if name.starts_with("$QB$OERG:") {
                let parts: Vec<&str> = name.split(':').collect();
                if parts.len() != 3 || !(parts[2] == "G" || parts[2] == "L") {
                    return emission(format!("invalid ON ERROR registration marker {}", pyrepr::string(name)));
                }
                let Ok(target) = parts[1].trim().parse::<i64>() else {
                    return emission(format!("invalid ON ERROR target marker {}", pyrepr::string(name)));
                };
                let mut address = None;
                if target != 0 {
                    let key = match error_labels.get(&target) {
                        Some(key) => *key,
                        None => {
                            let key = -(code_names.len() as i64 + 1);
                            error_labels.insert(target, key);
                            code_names.insert(key, masm::label(procedures.len(), target));
                            key
                        }
                    };
                    address = Some(Addr { index: key, ..Addr::new(Space::Segment, 0) });
                }
                error_registrations.insert(*at, (address, parts[2] == "L"));
                continue;
            } else {
                object_name = callable_names.get(name.as_str()).cloned().unwrap_or_else(|| name.clone());
            }
            referenced_calls.insert(object_name.clone());
            callees.insert(*at, masm::Callee::new(object_name, physical.far_calls.contains(at)));
        }
        let reserve = {
            let frame = owned_frame.borrow();
            -std::cmp::min(frame.slots.values().copied().min().unwrap_or(0), frame.floor)
        };
        let mut final_body = _source_instructions(&final_.body);
        let module_body = function.name == "__main";
        let public = !module_body && function.linkage == model::FunctionLinkage::External;
        if !module_body {
            let (framed, runtime_frame) =
                _runtime_frame(&final_body, reserve, program.runtime, _temporary_string_slots(module, function))?;
            final_body = framed;
            callees.extend(runtime_frame);
            referenced_calls.extend(["B$ENRA".to_owned(), "B$EXSA".to_owned()]);
        } else {
            // A module body normally has no frame in BC output.  When our
            // allocator needs spill space, however, a merely native BP frame
            // is invisible to B$GETMODCODE: the first DATA/READ call then
            // finds no module header and reports a spurious syntax error.
            // B$ENRA/B$EXSA are the measured QB45/PDS/VBDOS frame protocol
            // used by every source procedure, and make the spill frame part
            // of the runtime's own chain.  With no spill there remains no
            // entry/exit overhead, matching the ordinary module shape.
            let initialize;
            if reserve != 0 {
                (final_body, initialize) = _runtime_frame(&final_body, reserve, program.runtime, 0)?;
                referenced_calls.extend(["B$ENRA".to_owned(), "B$EXSA".to_owned()]);
            } else {
                (final_body, initialize) = _initialize_frame(&final_body, reserve)?;
            }
            callees.extend(initialize);
        }
        let final_body_converted = addressvalues::converted(&final_body);
        let (mut final_body, registrations) = _materialize_error_registrations(&final_body_converted, &error_registrations);
        referenced_calls.extend(registrations.values().map(|callee| callee.name.clone()));
        callees.extend(registrations);
        if module_body {
            let exits;
            (final_body, exits) = _ends_program(&final_body);
            callees.extend(exits);
            referenced_calls.insert("B$CENP".to_owned());
        }
        let final_body = _drop_resume_successors(&final_body, &physical.calls);
        let procedure_number = procedures.len();
        let statement_blocks = _statement_table_blocks(function);
        let empty = IndexMap::default();
        let source_instructions = body.source_instructions.as_ref().unwrap_or(&empty);
        let all_rows: Vec<(i64, i64, i64)> = statement_metadata
            .iter()
            .filter(|(function_id, _, instruction, _)| {
                *function_id == function.id && source_instructions.contains_key(instruction)
            })
            .map(|(_, source_block, instruction, line)| (*source_block, source_instructions[instruction], *line))
            .collect();
        let statement_instructions: IndexMap<i64, i64> =
            all_rows.iter().map(|(source_block, instruction, _line)| (*source_block, *instruction)).collect();
        let rows: Vec<(i64, i64, i64)> =
            all_rows.iter().copied().filter(|(source_block, _, _)| statement_blocks.contains(source_block)).collect();
        let resume_markers: BTreeSet<i64> = resume_blocks
            .values()
            .filter(|target| statement_instructions.contains_key(*target))
            .map(|target| statement_instructions[target])
            .collect();
        let final_body = _restore_label_arguments(&final_body, &restore_markers, &data_keys)?;
        let markers: BTreeSet<i64> = rows
            .iter()
            .map(|(_block, instruction, _line)| *instruction)
            .chain(resume_markers)
            .chain(data_markers.keys().copied())
            .collect();
        let (final_body, statement_labels) = _split_statement_blocks(&final_body, &markers);
        let final_body = _resume_label_transfers(
            &final_body,
            &resume_blocks,
            &statement_instructions,
            &statement_labels,
            procedure_number,
            &mut code_names,
        )?;
        let final_body = _remove_data_markers(
            &final_body,
            &data_markers,
            &statement_labels,
            procedure_number,
            &data_keys,
            &mut code_names,
            &mut callees,
        )?;
        _observe(&mut observer, "final-lir", StageValue::Lir(&final_body), Some(function), Some(callees.clone()))?;
        let layout_order: IndexMap<i64, i64> =
            final_body.blocks.iter().enumerate().map(|(index, block)| (block.at, index as i64)).collect();
        for (_source_block, instruction, line) in &rows {
            if let Some(at) = statement_labels.get(instruction) {
                statement_targets.push((procedure_number as i64, layout_order[at], masm::label(procedure_number, *at), *line));
            }
        }
        procedures.push(masm::Procedure {
            name: if module_body { "$QB$MAIN".to_owned() } else { _object_name(&function.name) },
            public,
            far: true,
            body: final_body,
            // B$ENRA, when present, owns both the ten-byte runtime header
            // and the CX bytes of locals below BP.  Asking masm's native
            // shell to reserve those bytes first shifts FR_BFRAME,
            // FR_CLOCALS, and FR_GOSUB away from their documented offsets;
            // ON ERROR then reads a spill as the local count and reports
            // Out of stack space.  Every nonzero reserve selected the
            // runtime-frame path above, so the native shell owns none.
            reserve: 0,
            callees,
        });
    }

    statement_targets.sort();
    procedures.push(_statement_procedure(&statement_targets));
    let (mut names, data_by_segment) = _data(module)?;
    names.extend(code_names.iter().map(|(key, name)| ((Space::Segment, *key), name.clone())));
    if data_keys.values().any(|key| code_names[key].is_empty()) {
        return emission("one or more DATA rows have no final code label");
    }
    let read_data =
        _read_data_items(module, &data_keys.iter().map(|(row, key)| (*row, code_names[key].clone())).collect())?;
    let external_data: BTreeSet<String> = module
        .data
        .iter()
        .filter(|object_| object_.linkage == model::DataLinkage::External)
        .map(|object_| object_.name.clone())
        .collect();
    let mut externs: BTreeSet<(String, String)> =
        referenced_calls.difference(&defined).map(|name| (name.clone(), "far".to_owned())).collect();
    externs.extend(external_data.iter().map(|name| (name.clone(), "byte".to_owned())));
    externs.extend(graphics.iter().map(|name| (name.clone(), "near".to_owned())));
    let code = format!("{}_CODE", _object_name(&module.name));
    let vbdos = program.runtime == model::RuntimeProfile::Vbdos;
    if !vbdos && !data_by_segment["FSL_CONST"].is_empty() {
        return emission(format!("{} cannot place literals in VBDOS FSL_CONST", program.runtime.value()));
    }
    let label = |name: &str| masm::Datum::Label(masm::Label { name: name.to_owned() });
    let mut basic_data: Vec<(String, Vec<masm::Datum>)> = vec![
        ("BR_DATA".into(), vec![]),
        ("BR_SKYS".into(), vec![]),
        ("COMMON".into(), vec![label("$QB$COMMON")]),
        (
            "BC_DATA".into(),
            [vec![label("$QB$DATA"), masm::Datum::Bytes(vec![0; 6])], data_by_segment["BC_DATA"].clone()].concat(),
        ),
        ("NMALLOC".into(), vec![]),
        ("ENMALLOC".into(), vec![]),
        ("BC_FT".into(), vec![label("$QB$FT")]),
        ("BC_CN".into(), [vec![label("$QB$CN")], data_by_segment["BC_CN"].clone()].concat()),
        (
            "BC_DS".into(),
            [vec![label("$QB$DS")], read_data, vec![masm::Datum::Bytes(vec![0xff, 0xff, 0x01])]].concat(),
        ),
        ("BC_SAB".into(), vec![label("$QB$SAB")]),
        (
            "BC_SA".into(),
            vec![
                label("$QB$SA"),
                masm::Datum::Pointer(masm::Pointer { name: "$QB$HEADER".into(), offset: 0, far: true }),
            ],
        ),
    ];
    let mut private: BTreeSet<String> = BTreeSet::new();
    if vbdos {
        basic_data.push(("FDATA".into(), vec![]));
        basic_data.push(("FSL_CONST".into(), data_by_segment["FSL_CONST"].clone()));
        private.extend(["FDATA".to_owned(), "FSL_CONST".to_owned()]);
    }
    if vbdos && !graphics.is_empty() {
        basic_data.push((
            "QB_LINK".into(),
            graphics
                .iter()
                .map(|name| masm::Datum::Pointer(masm::Pointer { name: name.clone(), offset: 0, far: false }))
                .collect(),
        ));
        private.insert("QB_LINK".into());
    }
    let emitted = masm::Module {
        code,
        names,
        externs: externs.into_iter().collect(),
        publics: procedures.iter().filter(|procedure| procedure.public).map(|procedure| procedure.name.clone()).collect(),
        data: basic_data,
        procedures,
        private,
        requests: graphics,
    };
    _observe(&mut observer, "emitted-assembly", StageValue::Module(&emitted), None, None)?;
    Ok(emitted)
}

/// Remove the native shell when B$ENRA/B$EXSA own the whole frame.
///
/// The shared MASM model supplies a C-shaped BP shell whenever a body
/// addresses BP or calls anything. B$ENRA itself saves BP, SI and DI, and
/// B$EXSA restores them, so this source-ABI exception stays in the frontend.
pub fn _basic_listing(procedure: &masm::Procedure, number: usize) -> Result<Vec<masm::Item>, CompileError> {
    let listing = masm::listing(procedure, number).map_err(|error| CompileError::Value(error.0))?;
    let runtime_frame = procedure.callees.values().any(|callee| callee.name == "B$ENRA");
    let module_body = procedure.name == "$QB$MAIN";
    if !runtime_frame && !module_body {
        return Ok(listing);
    }
    let (enter, leave) = masm::_frame_parts(procedure);
    let same = |items: &[masm::Item], semantics: &[Semantics]| {
        items.len() == semantics.len()
            && items.iter().zip(semantics).all(|(item, one)| matches!(item, masm::Item::Semantics(what) if what == one))
    };
    if listing.len() < enter.len() || !same(&listing[..enter.len()], &enter) {
        return emission(format!("{}: native frame prefix changed shape", procedure.name));
    }
    let listing = &listing[enter.len()..];
    let mut stripped: Vec<masm::Item> = Vec::new();
    let mut at = 0;
    while at < listing.len() {
        let after = at + leave.len();
        if runtime_frame
            && !leave.is_empty()
            && after <= listing.len()
            && same(&listing[at..after], &leave)
            && after < listing.len()
            && matches!(&listing[after], masm::Item::Semantics(what) if what.op == Operation::Return)
        {
            at = after;
            continue;
        }
        stripped.push(listing[at].clone());
        at += 1;
    }
    Ok(stripped)
}

/// Encode BASIC listings with their frontend-owned runtime frame shell.
fn _basic_code(
    segment: &mut omfwrite::Segment,
    module: &masm::Module,
    symbols: &mut IndexMap<String, (usize, usize)>,
) -> Result<(), CompileError> {
    let unencodable = |error: omfwrite::Unencodable| CompileError::Value(error.0);
    let mut items: Vec<omfwrite::Encoded> = Vec::new();
    for (number, procedure) in module.procedures.iter().enumerate() {
        items.push(omfwrite::Encoded::Label(masm::Label { name: procedure.name.clone() }));
        for item in _basic_listing(procedure, number)? {
            match omfwrite::_items(&item, &module.names, number) {
                Ok(encoded) => items.extend(encoded),
                Err(error) => return Err(CompileError::Value(format!("{}: {error}", procedure.name))),
            }
        }
    }
    let labels = omfwrite::_relaxed(&mut items).map_err(unencodable)?;
    let mut at = 0;
    for item in &items {
        match item {
            omfwrite::Encoded::Label(masm::Label { name }) => {
                symbols.insert(name.clone(), (0, at));
            }
            omfwrite::Encoded::Piece(omfwrite::Piece { code, fixups }) => segment.put(code, fixups),
            omfwrite::Encoded::Jump(omfwrite::Jump { name, label, long }) => {
                segment.put(&omfwrite::_jump(name, labels[label], at, *long).map_err(unencodable)?.code, &[]);
            }
            omfwrite::Encoded::Near(omfwrite::Near { name }) if labels.contains_key(name) => {
                let distance = labels[name] - (at as i64 + 3);
                let Ok(distance) = i16::try_from(distance) else {
                    return Err(CompileError::Value("'h' format requires -32768 <= number <= 32767".into()));
                };
                segment.put(&[&[0xE8][..], &distance.to_le_bytes()].concat(), &[]);
            }
            omfwrite::Encoded::Near(omfwrite::Near { name }) => {
                segment.put(&[0; 3], &[omfwrite::Fixup { relative: true, ..omfwrite::Fixup::new(1, omfwrite::OFFSET, name.clone()) }]);
                segment.image[at] = 0xE8;
            }
        }
        at = segment.image.len();
    }
    Ok(())
}

/// Emit a complete fresh BASIC-envelope OMF object.
pub fn object_bytes(
    program: &model::Program,
    source: &Path,
    observer: Option<&mut StageObserver<'_>>,
    options: &Options,
) -> Result<Vec<u8>, CompileError> {
    let module = omfwrite::live(&assembled(program, observer, options)?)
        .map_err(|error| CompileError::Value(error.to_string()))?;
    // Build the same semantic segments as backend.omfwrite.written, then add
    // the BASIC-owned MODULE_CODE envelope before asking its canonical record
    // serializer to write OMF.
    let mut segments = vec![omfwrite::Segment::new(&module.code, "CODE", false)];
    // A BASIC object does not own C's `_DATA` segment.  Even a zero-length
    // declaration is observable: when this is the first link object it makes
    // LINK establish the DATA class before BC_DATA, unlike BC/PDS/VBDOS, and
    // the BASIC runtime then initializes its local heap against the wrong
    // DGROUP boundary.
    let mut named: IndexSet<String> = IndexSet::default();
    for (name, _items) in &module.data {
        if named.insert(name.clone()) {
            let private = module.private.contains(name);
            segments.push(omfwrite::Segment::new(name, if private { "FAR_DATA" } else { "DATA" }, !private));
        }
    }
    let mut symbols: IndexMap<String, (usize, usize)> = IndexMap::default();
    for (name, items) in &module.data {
        let index = segments.iter().position(|one| &one.name == name).expect("every data segment was made");
        _object_data(&mut segments[index], index, items, &mut symbols);
    }
    _basic_code(&mut segments[0], &module, &mut symbols)?;

    let header = _header(program)?;
    let code = &mut segments[0];
    code.image = [header, std::mem::take(&mut code.image)].concat();
    code.spans = std::iter::once([0, 48]).chain(code.spans.iter().map(|[start, end]| [start + 48, end + 48])).collect();
    if module.procedures.last().is_none_or(|last| last.name != "$QB$STAT") {
        return emission("the BASIC statement table must be the final code procedure");
    }
    let statement_data = masm::label(module.procedures.len() - 1, 1);
    // The table is data carried by an opaque inline item.  The generic
    // assembly model conservatively emits a private BP prologue before such
    // an item, so OF_STA must name its first block label after that
    // prologue, not the procedure symbol.
    let shifted: Vec<omfwrite::Fixup> =
        code.fixups.iter().map(|one| omfwrite::Fixup { at: one.at + 48, ..one.clone() }).collect();
    code.fixups = [
        omfwrite::Fixup::new(10, omfwrite::OFFSET, statement_data),
        omfwrite::Fixup::new(12, omfwrite::OFFSET, "$QB$DS"),
        omfwrite::Fixup::new(14, omfwrite::OFFSET, "$QB$DATA"),
        omfwrite::Fixup::new(16, omfwrite::OFFSET, "$QB$FT"),
        omfwrite::Fixup::new(24, omfwrite::OFFSET, "$QB$COMMON"),
        omfwrite::Fixup::new(32, omfwrite::OFFSET, "$QB$CN"),
    ]
    .into_iter()
    .chain(shifted)
    .collect();
    let mut symbols: IndexMap<String, (usize, usize)> = symbols
        .into_iter()
        .map(|(name, (segment, offset))| (name, (segment, if segment == 0 { offset + 48 } else { offset })))
        .collect();
    symbols.insert("$QB$HEADER".into(), (0, 0));
    // BC_DS stores DATA keys as literal final code offsets, not relocations.
    // Resolve the frontend's symbolic row labels only after the 30h module
    // header has shifted every code symbol, then remove their temporary
    // fixups. Leaving both the 0030h field and an OFFSET fixup made LINK add
    // them and B$RSTB searched for 0060h forever.
    let read_index = segments.iter().position(|one| one.name == "BC_DS").expect("BC_DS is always emitted");
    let read_segment = &mut segments[read_index];
    for fixup in read_segment.fixups.clone() {
        let (segment, offset) = symbols[&fixup.name];
        if fixup.loc != omfwrite::OFFSET || segment != 0 {
            return emission("BC_DS DATA key must resolve to a near code offset");
        }
        pack_into(&mut read_segment.image, fixup.at, offset as i64);
    }
    read_segment.fixups.clear();
    let externs: IndexMap<String, String> = module.externs.iter().cloned().collect();
    let name = source.file_name().map_or_else(String::new, |one| one.to_string_lossy().into_owned());
    let records = omfwrite::_records(&module, &name, &mut segments, &symbols, &externs)
        .map_err(|error| CompileError::Value(error.to_string()))?;
    let emitted: Vec<u8> = records.iter().flat_map(|record| record.emit()).collect();
    _basic_segment_classes(&emitted, &module.code)
}
