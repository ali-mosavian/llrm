//! The assembler: instructions in, one image and its relocations out.
//!
//! Port of `qbopt/backend/asm.py`. LLVM's `MCAssembler`: how long each
//! instruction is, where each therefore lands, which branches can shrink now
//! that everything is closer, and where each fixup ended up. `select` is the
//! code emitter above it and `omfwrite` the object writer below.
//!
//! An assembler does not allocate. Handed `assignment=None` this remaps
//! nothing, which is the right answer for a body whose registers are already
//! chosen.
//!
//! Python's `id(op)` is the `Arc` pointer of an LIR occurrence.

use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, LazyLock};

use iced_x86::{OpKind, Register};

use crate::analysis::flags::Flag;
use crate::backend::select::{self, Emitted, HeldMap, RegisterMap, Where};
use crate::backend::{fpu, target};
use crate::frontends::bc::declen::STANDS_IN;
use crate::legacy::calls::{self as machine, CallSite};
use crate::model::ir::nodes::{self, Node};
use crate::model::ir::{Loc, Operation, Semantics};
use crate::model::lir::Insn;
use crate::model::mir;
use crate::objectfile::module::{Addr, Module, SourceMap, Space};
use crate::support::hash::IndexMap;

/// A body's new bytes, and what moved.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Laid {
    pub code: Vec<u8>,
    /// Where each op ended up, old address -> new.
    pub moved: IndexMap<i64, i64>,
    /// (offset within `code`, the original field's own address) for every
    /// relocated displacement, so the fixup that names it can be moved.
    pub relocations: Vec<(i64, i64)>,
    /// Fixups that belonged to an instruction this body no longer contains.
    /// Reported rather than silently omitted: omfwrite refuses a fixup it
    /// cannot place, and can only tell the two apart if told which were
    /// meant to go.
    pub dropped: BTreeSet<i64>,
    /// Every original address a transform folded into a surviving op, mapped
    /// to where that op went. A branch target never resolves through this.
    pub covered: IndexMap<i64, i64>,
    pub symbols: Vec<(i64, Addr)>,
}

impl Laid {
    pub fn grew(&self) -> usize {
        self.code.len()
    }
}

/// A run of bytes between the instructions, copied rather than selected.
///
/// BC drops an ON GOTO table inline: a count byte and one relocated word
/// per destination. Copying the bytes and moving the fixups is enough.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Table {
    pub lo: i64,
    pub hi: i64,
    pub discarded: bool,
}

impl Table {
    pub const fn new(lo: i64, hi: i64) -> Self {
        Table { lo, hi, discarded: false }
    }

    pub const fn at(&self) -> i64 {
        self.lo
    }
}

/// An op, which select encodes, or a Table, which is copied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Item {
    Op(Arc<Insn>),
    Table(Table),
}

impl Item {
    pub fn at(&self) -> i64 {
        match self {
            Item::Op(op) => op.at,
            Item::Table(table) => table.at(),
        }
    }
}

/// Explicit raise provenance, with a compatibility view for unit callers.
pub fn _source<'a>(found: &Module, source: Option<&'a SourceMap>) -> Cow<'a, SourceMap> {
    match source {
        Some(source) => Cow::Borrowed(source),
        None => Cow::Owned(SourceMap::from_module(found)),
    }
}

fn span_of(node: &Node) -> (i64, i64) {
    let (lo, hi) = nodes::span(node);
    (lo as i64, hi as i64)
}

/// Every disjoint range of original bytes this op stands for.
///
/// `source.coverage` is the one authoritative record for an op whose bytes
/// are not a single run; every other op has no entry there and `covers`
/// alone is still the whole answer.
pub fn _ranges_of(op: &Item, found: &Module, source: Option<&SourceMap>) -> Vec<(i64, i64)> {
    let op = match op {
        Item::Table(table) => return vec![(table.lo, table.hi)],
        Item::Op(op) => op,
    };
    _insn_ranges_of(op, found, source)
}

fn _insn_ranges_of(op: &Insn, found: &Module, source: Option<&SourceMap>) -> Vec<(i64, i64)> {
    if !op.spread.is_empty() {
        return op.spread.clone();
    }
    let extra = op.extra_covers();
    if !extra.is_empty() {
        let mut all: Vec<(i64, i64)> = extra;
        all.extend(op.covers);
        all.sort();
        let mut ranges: Vec<(i64, i64)> = Vec::new();
        for (lo, hi) in all {
            match ranges.last_mut() {
                Some(last) if lo <= last.1 => *last = (last.0, hi.max(last.1)),
                _ => ranges.push((lo, hi)),
            }
        }
        return ranges;
    }
    if op.inserted() {
        // Explicitly inserted; its id names operands, not owned bytes.
        return vec![op.covers.expect("an inserted op covers an empty range")];
    }
    let full = op.id().and_then(|id| _source(found, source).coverage.get(&id).cloned());
    if let Some(full) = full {
        return full;
    }
    if let Some(covers) = op.covers {
        return vec![covers];
    }
    match _length_of(op, found, source) {
        None => Vec::new(),
        Some(length) => vec![(op.at, op.at + length)],
    }
}

/// The overall span of original bytes this op accounts for: the outer
/// envelope of every disjoint range `_ranges_of` returns.
pub fn _stands_for(op: &Item, found: &Module, source: Option<&SourceMap>) -> Option<(i64, i64)> {
    let ranges = _ranges_of(op, found, source);
    if ranges.is_empty() {
        return None;
    }
    Some((
        ranges.iter().map(|&(lo, _)| lo).min().expect("non-empty"),
        ranges.iter().map(|&(_, hi)| hi).max().expect("non-empty"),
    ))
}

/// The original instruction bytes, independent of replaced-byte ownership.
pub fn _raw_span(op: &Insn) -> Option<(i64, i64)> {
    match &op.node {
        Some(node) if !op.inserted() => Some(span_of(node)),
        _ => None,
    }
}

/// How many bytes the op occupied in the image it came from.
///
/// `covers` overrides the node's own span, and is how a transform accounts
/// for what it replaced. `source.coverage` is asked first and, where it has
/// an answer, is the whole of it.
pub fn _length_of(op: &Insn, found: &Module, source: Option<&SourceMap>) -> Option<i64> {
    if op.inserted() {
        return Some(0);
    }
    if !op.spread.is_empty() {
        return Some(op.spread.iter().map(|(lo, hi)| hi - lo).sum());
    }
    if !op.extra_covers().is_empty() {
        return Some(_insn_ranges_of(op, found, source).iter().map(|(lo, hi)| hi - lo).sum());
    }
    let full = op.id().and_then(|id| _source(found, source).coverage.get(&id).cloned());
    if let Some(full) = full {
        return Some(full.iter().map(|(lo, hi)| hi - lo).sum());
    }
    if let Some((lo, hi)) = op.covers {
        return Some(hi - lo);
    }
    let node = op.node.as_ref()?;
    let (lo, hi) = span_of(node);
    Some(hi - lo)
}

/// `what` with its target moved to wherever that instruction went, or None
/// where the target is not in this body.
pub fn _retargeted(what: &Semantics, moved: &IndexMap<i64, i64>) -> Option<Semantics> {
    let Some(target) = what.target else {
        return Some(what.clone());
    };
    let landed = *moved.get(&target)?;
    Some(Semantics { target: Some(landed), ..what.clone() })
}

fn holds_memory(one: &Loc) -> bool {
    matches!(one, Loc::Mem(_) | Loc::Address(_))
}

fn holds_field(one: &Loc) -> bool {
    matches!(one, Loc::Mem(_) | Loc::Address(_) | Loc::Imm(_))
}

fn node_semantics(op: &Insn) -> Option<&Semantics> {
    op.node.as_deref().map(Node::semantics)
}

/// Whether the operand the fixup named is still in this operation.
///
/// The question is whether a memory operand *went*, not whether a
/// transform touched the operation.
pub fn _still_has_an_operand_for_it(op: &Insn) -> bool {
    let Some(what) = &op.what else { return true };
    if !op.rewritten() || (what.op == Operation::Barrier && _raw_span(op).is_some()) {
        return true;
    }
    let holds: Vec<&Loc> = what.dests.iter().chain(&what.sources).filter(|one| holds_field(one)).collect();
    if holds.is_empty() {
        return false;
    }
    let had = node_semantics(op).is_some_and(|was| was.dests.iter().chain(&was.sources).any(holds_memory));
    !(had && !holds.iter().any(|one| holds_memory(one)))
}

pub const _TRANSFERS: [Operation; 4] = [Operation::Call, Operation::Jump, Operation::Branch, Operation::Escape];

/// Whether this instruction has an operand a relocation can sit in: a
/// symbolic memory operand's displacement, or a symbolic immediate.
pub fn _relocatable(what: Option<&Semantics>) -> bool {
    let Some(what) = what else { return true };
    if what.target.is_some() || _TRANSFERS.contains(&what.op) {
        // A call or jump carries its fixup in the target, not in an operand.
        return true;
    }
    // An Address with no `addr` is register arithmetic -- `lea eax,[eax+eax*2]`
    // -- where a Mem with none is a cell whose address is unknown.
    what.dests.iter().chain(&what.sources).any(|one| match one {
        Loc::Imm(_) => true,
        Loc::Address(address) => address.addr.is_some_and(|addr| matches!(addr.space, Space::Segment | Space::External)),
        Loc::Mem(mem) => mem.addr.is_none_or(|addr| matches!(addr.space, Space::Segment | Space::External)),
        _ => false,
    })
}

/// The instructions one folded runtime call becomes.
pub fn _absorbed(site: &CallSite, read: Flag) -> Option<Emitted> {
    // A divide hands its answer's high half back through an operation of
    // its own, so the sequence must not end in the idiom as well.
    let restore = !(machine::DIVIDES.contains(&site.name.as_str()) || site.name == machine::MULTIPLY);
    select::absorbed(site, read, restore).ok()
}

/// What `source.absorbed` records per folded op: Python's `(site, read)`.
pub type FoldedSite = (CallSite, Flag);

/// The site this op stands for, where it still stands for one.
///
/// `Ok(None)` is Python's None, `Err` its refusal string.
pub fn _folded_site(op: &Insn, found: &Module, source: Option<&SourceMap>) -> Result<Option<FoldedSite>, String> {
    let Some(id) = op.id() else { return Ok(None) };
    // An instruction a pass lifted the symbolic operand onto keeps the id
    // because the id is how its fixup is found. It is not the site.
    if op.symbol == Some(true) {
        return Ok(None);
    }
    let map = _source(found, source);
    let Some(folded) = map.absorbed.get(&id) else {
        if op.kind() == mir::Kind::Divmod {
            return Err(format!("{:#06x}: a divide with no site of its own cannot be emitted", op.at));
        }
        return Ok(None);
    };
    let folded = folded
        .downcast_ref::<FoldedSite>()
        .unwrap_or_else(|| panic!("source.absorbed[{id}] is not a (CallSite, Flag)"))
        .clone();
    let kind = mir::absorbs(&folded.0.name);
    if kind.is_some_and(|kind| op.kind() != kind) {
        return Ok(None);
    }
    if op.at != folded.0.start as i64 {
        return Err(format!(
            "{:#06x}: {} was raised at {:#06x} and no longer stands there",
            op.at, folded.0.name, folded.0.start
        ));
    }
    // A moved operation emits from its operands or it does not emit.
    Ok(Some(folded))
}

/// Which register each of an operation's results is in.
///
/// Origin answers only when there is no allocation at all; a value missing
/// from one that exists refuses the emission.
pub fn _seats(
    op: &Insn,
    assignment: Option<&IndexMap<u32, Register>>,
    origin: Option<&IndexMap<u32, Register>>,
) -> Option<(Register, Register)> {
    let mut seats = Vec::new();
    for one in op.results() {
        let mir::Arg::Held(held) = one else { return None };
        let value = held.value.id;
        let r#where = match assignment {
            Some(assignment) => assignment.get(&value),
            None => origin.and_then(|origin| origin.get(&value)),
        };
        seats.push(*r#where?);
    }
    match seats.as_slice() {
        [one, other] => Some((*one, *other)),
        _ => None,
    }
}

/// The fixup each of this op's memory operands still names, in order, or
/// None where an operand was pointed at a different cell.
pub fn _divide_fields(
    op: &Insn,
    found: &Module,
    fields: &BTreeSet<i64>,
    source: Option<&SourceMap>,
) -> Option<Vec<i64>> {
    let was: Vec<mir::Arg> = match op.raised() {
        Some(raised) => raised.0.clone(),
        None => op.args().to_vec(),
    };
    let recorded = _fields_in(found, op, fields, source);
    let known: IndexMap<usize, i64> = was
        .iter()
        .enumerate()
        .filter(|(_, one)| matches!(one, mir::Arg::Cell(_)))
        .map(|(index, _)| index)
        .enumerate()
        .filter(|&(order, _)| order < recorded.len())
        .map(|(order, position)| (position, recorded[order]))
        .collect();
    let mut out = Vec::new();
    for (index, one) in op.args().iter().enumerate() {
        let mir::Arg::Cell(cell) = one else { continue };
        let Some(mir::Arg::Cell(before)) = was.get(index) else { return None };
        let Some(&field) = known.get(&index) else { return None };
        let moved = mir::MemRef { base: cell.r#ref.base, segment: cell.r#ref.segment, ..before.r#ref.clone() };
        if moved != cell.r#ref {
            return None;
        }
        out.push(field);
    }
    Some(out)
}

/// `select.divides` asked of an LIR occurrence. An occurrence with no source
/// op has no operands, which is `divides`' own first refusal.
fn _divides_of(op: &Insn, seats: (Register, Register)) -> Result<Emitted, String> {
    match op.source() {
        Some(source) => select::divides(source, seats, false),
        None => Err(format!("{}: not a divide over two operands", op.name().unwrap_or("None"))),
    }
}

/// A divide emitted from its own operands: Ok(Some((bytes, its fixups))),
/// Ok(None) where this op is not one, or Err(why not).
///
/// A string is a refusal of the whole emission and never a signal to fall
/// back: emitting the frozen site after a pass rewrote the operation would
/// run the operands BC pushed as if they were the new ones.
pub fn _selected_divide(
    op: &Insn,
    found: &Module,
    assignment: Option<&IndexMap<u32, Register>>,
    origin: Option<&IndexMap<u32, Register>>,
    fields: &BTreeSet<i64>,
    source: Option<&SourceMap>,
) -> Result<Option<(Emitted, Vec<i64>)>, String> {
    if op.kind() != mir::Kind::Divmod {
        return Ok(None);
    }
    // An instruction standing beside the site is not the site.
    if op.id().is_none() {
        return Ok(None);
    }
    let seats = _seats(op, assignment, origin);
    let made = match seats {
        Some(seats) => _divides_of(op, seats),
        None => Err("no register holds a result".to_owned()),
    };
    let wanted = _divide_fields(op, found, fields, source);
    if let (Ok(made), Some(wanted)) = (&made, &wanted) {
        if made.places().len() == wanted.len() {
            return Ok(Some((made.clone(), wanted.clone())));
        }
    }
    if op.rewritten() {
        let why = match &made {
            Err(why) => why.clone(),
            Ok(_) => "no fixup here names the operands it now reads".to_owned(),
        };
        return Err(format!(
            "{:#06x}: {why}, and the bytes the site was raised with are not this operation",
            op.at
        ));
    }
    Ok(None)
}

/// Every fixup this operation's own operands carry, in operand order.
pub fn _fields_in(found: &Module, op: &Insn, fields: &BTreeSet<i64>, source: Option<&SourceMap>) -> Vec<i64> {
    if op.symbol == Some(false) {
        return Vec::new(); // the operand went to another instruction, and the fixup with it
    }
    let said = op.id().and_then(|id| _source(found, source).refs.get(&id).cloned());
    if let Some(said) = said {
        if said.len() > 1 {
            let wanted: Vec<i64> =
                said.iter().copied().filter(|one| fields.is_empty() || fields.contains(one)).collect();
            if !wanted.is_empty() && _still_has_an_operand_for_it(op) {
                return wanted;
            }
        }
    }
    _field_in(found, op, fields, source).into_iter().collect()
}

/// The address a symbolic immediate carries, when this op has no fixup of
/// its own.
pub fn _generated_immediate(op: &Insn, what: Option<&Semantics>) -> Option<Addr> {
    let what = what?;
    if op.symbol == Some(false) {
        return None;
    }
    let found: Vec<Addr> = what
        .dests
        .iter()
        .chain(&what.sources)
        .filter_map(|one| match one {
            Loc::Imm(imm) => imm.address,
            _ => None,
        })
        .collect();
    match found.as_slice() {
        [one] => Some(*one),
        _ => None,
    }
}

fn code_at(found: &Module, at: i64) -> Option<u8> {
    usize::try_from(at).ok().and_then(|at| found.code.get(at).copied())
}

/// The address of the one relocated field inside `op`'s own bytes.
///
/// A far call's four relocated bytes are a target rather than a
/// displacement, and `at + 1` is not a guess: `9a` then four bytes is the
/// only encoding a far call has. Otherwise exactly one fixup in the
/// instruction's own span, or nothing.
pub fn _field_in(found: &Module, op: &Insn, fields: &BTreeSet<i64>, source: Option<&SourceMap>) -> Option<i64> {
    // An instruction a pass lifted the symbolic operand onto: one recorded
    // fixup, or none of them.
    if op.symbol == Some(true) {
        let what = _semantics(op);
        if op.id().is_some()
            && what.is_some_and(|what| what.op == Operation::Call && what.target.is_none())
            && found.calls.contains_key(&op.at)
            && code_at(found, op.at) == Some(0x9A)
            && fields.contains(&(op.at + 1))
        {
            return Some(op.at + 1);
        }
        let said = op.id().and_then(|id| _source(found, source).refs.get(&id).cloned())?;
        if said.len() != 1 || !_still_has_an_operand_for_it(op) {
            return None;
        }
        let r#ref = said[0];
        return (fields.is_empty() || fields.contains(&r#ref)).then_some(r#ref);
    }
    let node = op.node.as_deref()?;
    // `push eax / pop ax / pop dx` has no field for a relocation to go in.
    if matches!(node, Node::Restore(_)) {
        return None;
    }
    if op.symbol == Some(false) {
        return None; // the operand went to another instruction, and the fixup with it
    }
    // What the operation says it carries, established at the raise.
    let said = op.id().and_then(|id| _source(found, source).refs.get(&id).cloned());
    let r#ref = said.and_then(|said| said.first().copied());
    if let Some(r#ref) = r#ref {
        if fields.is_empty() || fields.contains(&r#ref) {
            return _still_has_an_operand_for_it(op).then_some(r#ref);
        }
    }
    let known: BTreeSet<i64> = if fields.is_empty() { found.fixup_at.keys().copied().collect() } else { fields.clone() };
    let (lo, hi) = span_of(node);
    // A far call and a far jmp put their four relocated bytes right after a
    // one-byte opcode.
    if matches!(code_at(found, lo), Some(0x9A | 0xEA)) && known.contains(&(lo + 1)) {
        return Some(lo + 1);
    }
    // An operation with nothing but registers has no field to put one in.
    if let Some(what) = _semantics(op) {
        let holds: Vec<&Loc> = what.dests.iter().chain(&what.sources).filter(|one| holds_field(one)).collect();
        if holds.is_empty() {
            return None;
        }
        // Whether a memory operand *went*: `cmp word [k],1` became `cmp ax,1`.
        let had = node_semantics(op).is_some_and(|was| was.dests.iter().chain(&was.sources).any(holds_memory));
        let rewrote = op.rewritten();
        if rewrote && had && !holds.iter().any(|one| holds_memory(one)) {
            return None;
        }
    }
    let inside: Vec<i64> = known.iter().copied().filter(|one| lo <= *one && *one < hi).collect();
    match inside.as_slice() {
        [one] => Some(*one),
        _ => None,
    }
}

/// A signed byte's worth of displacement, measured from the end of the
/// instruction. The short branch's whole range.
pub const REACH: std::ops::Range<i64> = -128..128;

pub const _FLOATING_MACHINE: [Operation; 6] = [
    Operation::FloatLoad,
    Operation::FloatStore,
    Operation::FloatArith,
    Operation::FloatArithPop,
    Operation::FloatUnary,
    Operation::Exchange,
];

fn same(one: &Arc<Insn>) -> *const Insn {
    Arc::as_ptr(one)
}

/// Where each op lands, given what each one measures, and what an
/// *address* means afterwards. The first of a group is what that address
/// means to everything outside.
pub fn _placed(
    ops: &[Item],
    at: i64,
    lengths: &[i64],
    labels: Option<&IndexMap<i64, i64>>,
    anchors: Option<&IndexMap<i64, Arc<Insn>>>,
) -> (Vec<i64>, IndexMap<i64, i64>) {
    let mut placed: Vec<i64> = Vec::new();
    let mut moved: IndexMap<i64, i64> = IndexMap::default();
    let mut r#where = at;
    for (op, &length) in ops.iter().zip(lengths) {
        placed.push(r#where);
        moved.entry(op.at()).or_insert(r#where);
        if let Item::Table(table) = op {
            if length != 0 {
                // A copied run preserves the relative position of every byte,
                // not only its first one.
                for old in table.lo..table.hi {
                    moved.entry(old).or_insert(r#where + old - table.lo);
                }
            }
        }
        r#where += length;
    }
    for (&label, destination) in labels.into_iter().flatten() {
        if let Some(&landed) = moved.get(destination) {
            moved.entry(label).or_insert(landed);
        }
    }
    let positions: HashMap<*const Insn, i64> = ops
        .iter()
        .zip(&placed)
        .filter_map(|(op, &position)| match op {
            Item::Op(op) => Some((same(op), position)),
            Item::Table(_) => None,
        })
        .collect();
    for (&label, operation) in anchors.into_iter().flatten() {
        moved.insert(label, positions[&same(operation)]);
    }
    (placed, moved)
}

pub fn _emulator_protocol(op: &Insn, found: &Module, native_fpu: bool, source: Option<&SourceMap>) -> Option<u8> {
    let protocols = &_source(found, source).float_protocols;
    if !native_fpu {
        if let Some(&protocol) = op.id().and_then(|id| protocols.get(&id)) {
            if op.what.as_ref().is_some_and(|what| what.op == Operation::FloatLoad) {
                return Some(u8::try_from(protocol).expect("a protocol is a byte"));
            }
        }
    }
    let at = usize::try_from(op.at).ok()?;
    if native_fpu || !fpu::emulated_at(&found.code, at) {
        return None;
    }
    // An inserted or materializing instruction can inherit an emulated x87
    // operation's source address while computing an ordinary integer move.
    let what = op.what.as_ref();
    if let Some(what) = what {
        let wait = what.op == Operation::Nothing && matches!(what.name.as_deref(), Some("wait" | "fwait"));
        if !_FLOATING_MACHINE.contains(&what.op) && !wait {
            return None;
        }
    }
    let protocol = found.code[at + 1];
    if op.node.is_some() {
        return Some(protocol);
    }
    if op.covers == Some((op.at, op.at))
        && STANDS_IN.contains(&protocol)
        && what.is_some_and(|what| {
            matches!(what.op, Operation::FloatLoad | Operation::Exchange)
                && matches!(what.name.as_deref(), Some("fld" | "fxch"))
                && !what.sources.is_empty()
                && what.sources.iter().chain(&what.dests).all(|arg| matches!(arg, Loc::St(_)))
        })
    {
        // Allocator moves inherit their anchor's mode, not its memory prefix.
        return Some(0x34);
    }
    None
}

/// Whether BC's bytes at `at` are an emulator interrupt.
fn interrupt_at(found: &Module, at: i64) -> bool {
    code_at(found, at) == Some(0xCD)
}

fn stands_in_at(found: &Module, at: i64) -> bool {
    code_at(found, at + 1).is_some_and(|byte| STANDS_IN.contains(&byte))
}

fn bytes_of(found: &Module, lo: i64, hi: i64) -> &[u8] {
    &found.code[lo as usize..hi as usize]
}

/// Every item in order from `at`, shrunk to a fixed point and emitted.
///
/// An item is an op, which select encodes, or a Table, which is copied.
/// `Err` is Python's `str` answer.
#[allow(clippy::too_many_arguments)]
pub fn assemble(
    ops: &[Item],
    at: i64,
    found: &Module,
    fields: &BTreeSet<i64>,
    native_fpu: bool,
    assignment: Option<&IndexMap<u32, Register>>,
    origin: Option<&IndexMap<u32, Register>>,
    labels: Option<&IndexMap<i64, i64>>,
    anchors: Option<&IndexMap<i64, Arc<Insn>>>,
    source: Option<&SourceMap>,
) -> Result<Laid, String> {
    if ops.is_empty() {
        return Err("no ops to lay out".to_owned());
    }
    let held = _held(assignment);

    // Kept per op rather than per address: several ops may share one.
    let mut lengths: Vec<i64> = Vec::new();
    for item in ops {
        let op = match item {
            Item::Table(table) => {
                lengths.push(if table.discarded { 0 } else { table.hi - table.lo });
                continue;
            }
            Item::Op(op) => op,
        };
        let what = _semantics(op);
        // `op.node is not None` because this reads the original bytes at
        // `op.at`, and an inserted instruction has none.
        let emulated = !native_fpu && op.node.is_some() && interrupt_at(found, op.at);
        let folded = _folded_site(op, found, source);
        let chosen = _selected_divide(op, found, assignment, origin, fields, source)?;
        // Selection first, and the refusal only where it had no answer.
        if chosen.is_none() {
            if let Err(why) = &folded {
                return Err(why.clone());
            }
        }
        if !matches!(folded, Ok(None)) || chosen.is_some() {
            let made = match &chosen {
                Some(chosen) => Some(chosen.0.clone()),
                None => {
                    let (site, read) = folded.as_ref().expect("checked").as_ref().expect("checked");
                    _absorbed(site, *read)
                }
            };
            let Some(made) = made else {
                return Err(format!("{:#06x}: the absorbed call is not one select.py can emit", op.at));
            };
            lengths.push(made.code.len() as i64);
            continue;
        }
        if let Some(Node::Restore(restore)) = op.node.as_deref() {
            // Measured from what it emits, not from `covers`.
            let Some(made) = select::restore(restore.pair as i64) else {
                return Err(format!("{:#06x}: the restore idiom is not one select.py can emit", op.at));
            };
            lengths.push(made.code.len() as i64);
            continue;
        }
        if what.is_none() || emulated && !stands_in_at(found, op.at) {
            let span = _raw_span(op);
            lengths.push(span.map_or(0, |(lo, hi)| hi - lo));
            continue;
        }
        let what = what.expect("checked");
        let r#where = _where(op, assignment, origin);
        let relocated =
            _field_in(found, op, fields, source).is_some() || _generated_immediate(op, Some(what)).is_some();
        let mut made = select::emit(what, at as u64, r#where.as_ref().map(Remap::as_where), false, relocated, held.as_ref());
        if let Some(one) = &made {
            if let Some(protocol) = _emulator_protocol(op, found, native_fpu, source) {
                made = fpu::wrapped(one, protocol);
            }
        }
        let Some(made) = made else {
            return Err(format!("{:#06x}: {} is not one select.py can emit", op.at, op.name().unwrap_or("None")));
        };
        lengths.push(made.code.len() as i64);
    }

    // Shrink to a fixed point. Every branch starts long; one that reaches its
    // target within a signed byte becomes short, which moves everything after
    // it closer and can only let more of them shrink.
    let mut short: BTreeSet<usize> = BTreeSet::new(); // by position, since an address may hold several
    let mut fallthrough: BTreeSet<usize> = BTreeSet::new();
    let occurrences: BTreeSet<*const Insn> = ops
        .iter()
        .filter_map(|one| match one {
            Item::Op(op) => Some(same(op)),
            Item::Table(_) => None,
        })
        .collect();
    if anchors.is_some_and(|anchors| anchors.values().any(|op| !occurrences.contains(&same(op)))) {
        return Err("a block entry has no emitted occurrence".to_owned());
    }
    let (mut placed, mut moved) = _placed(ops, at, &lengths, labels, anchors);
    let mut changing = true;
    while changing {
        changing = false;
        for (index, item) in ops.iter().enumerate() {
            let Item::Op(op) = item else { continue };
            let Some(what) = _semantics(op) else { continue };
            let Some(target) = what.target else { continue };
            if fallthrough.contains(&index) {
                continue;
            }
            let Some(&landed) = moved.get(&target) else { continue };
            if what.op == Operation::Jump
                && what.name.as_deref() == Some("jmp")
                && lengths[index] > 0
                && landed == placed[index] + lengths[index]
                && _fields_in(found, op, fields, source).is_empty()
            {
                fallthrough.insert(index);
                lengths[index] = 0;
                changing = true;
                continue;
            }
            if short.contains(&index) {
                continue;
            }
            // Through `moved`, the same as the emission below.
            let Some(aimed) = _retargeted(what, &moved) else { continue };
            let r#where = _where(op, assignment, origin);
            let relocated =
                _field_in(found, op, fields, source).is_some() || _generated_immediate(op, Some(what)).is_some();
            let made =
                select::emit(&aimed, placed[index] as u64, r#where.as_ref().map(Remap::as_where), true, relocated, held.as_ref());
            let Some(made) = made else {
                continue; // a call has no short form, and says so by refusing
            };
            if !REACH.contains(&(landed - (placed[index] + made.code.len() as i64))) {
                continue;
            }
            short.insert(index);
            lengths[index] = made.code.len() as i64;
            changing = true;
        }
        if changing {
            (placed, moved) = _placed(ops, at, &lengths, labels, anchors);
        }
    }

    // The bytes, at the addresses the fixed point settled on.
    let mut out: Vec<u8> = Vec::new();
    let mut relocations: Vec<(i64, i64)> = Vec::new();
    let mut symbols: Vec<(i64, Addr)> = Vec::new();
    let known: BTreeSet<i64> = if fields.is_empty() { found.fixup_at.keys().copied().collect() } else { fields.clone() };
    for (index, item) in ops.iter().enumerate() {
        if fallthrough.contains(&index) {
            continue;
        }
        let op = match item {
            Item::Table(table) => {
                if table.discarded {
                    continue;
                }
                // Copied verbatim, with every fixup inside it moved by the
                // same amount the table itself moved.
                out.extend_from_slice(bytes_of(found, table.lo, table.hi));
                for &field in known.range(table.lo..table.hi) {
                    relocations.push((placed[index] - at + (field - table.lo), field));
                }
                continue;
            }
            Item::Op(op) => op,
        };
        // Unmodelled interrupts retain their original encoding.
        if !native_fpu
            && op.node.is_some() // an inserted instruction has no original bytes
            && interrupt_at(found, op.at)
            && (_semantics(op).is_none() || !stands_in_at(found, op.at))
        {
            if let Some(span) = _raw_span(op) {
                // Copied, so any fixup inside it keeps its place within the
                // instruction and only the instruction itself has moved.
                if let Some(field) = _field_in(found, op, fields, source) {
                    relocations.push((out.len() as i64 + (field - span.0), field));
                }
                out.extend_from_slice(bytes_of(found, span.0, span.1));
                continue;
            }
        }
        // Before the carry below, which is the order the length pass asks in.
        if let Some(Node::Restore(restore)) = op.node.as_deref() {
            let made = select::restore(restore.pair as i64);
            match made {
                Some(made) if made.code.len() as i64 == lengths[index] => {
                    out.extend_from_slice(&made.code);
                    continue;
                }
                _ => return Err(format!("{:#06x}: the restore idiom did not come back its own length", op.at)),
            }
        }
        // The length pass asks whether a recognized source idiom has a
        // selected replacement before it considers carrying source bytes.
        // Emission must ask in the same order.
        let folded = _folded_site(op, found, source);
        let chosen = _selected_divide(op, found, assignment, origin, fields, source)?;
        if chosen.is_none() {
            if let Err(why) = &folded {
                return Err(why.clone());
            }
        }
        if !matches!(folded, Ok(None)) || chosen.is_some() {
            let made = match &chosen {
                Some(chosen) => Some(chosen.0.clone()),
                None => {
                    let (site, read) = folded.as_ref().expect("checked").as_ref().expect("checked");
                    _absorbed(site, *read)
                }
            };
            let Some(made) = made.filter(|made| made.code.len() as i64 == lengths[index]) else {
                return Err(format!("{:#06x}: the absorbed call changed length between the two passes", op.at));
            };
            let binds = match &chosen {
                Some(chosen) => chosen.1.clone(),
                None => _fields_in(found, op, fields, source),
            };
            for (r#where, field) in made.places().into_iter().zip(binds) {
                relocations.push((out.len() as i64 + r#where as i64, field));
            }
            out.extend_from_slice(&made.code);
            continue;
        }
        // A barrier is an instruction ir.py models nothing about, so its own
        // bytes are the only right answer -- unless it names a branch target.
        if _semantics(op).is_none() {
            if let Some(span) = _raw_span(op) {
                let near = match op.node.as_deref() {
                    Some(Node::Opaque(node)) => Some(&node.insn),
                    Some(Node::Long(node)) => Some(&node.insn),
                    Some(Node::Call(node)) => Some(&node.insn),
                    _ => None,
                };
                if near.is_some_and(|found_insn| found_insn.insn.op0_kind() == OpKind::NearBranch16) {
                    return Err(format!("{:#06x}: a branch this cannot model would keep a stale target", op.at));
                }
                if let Some(field) = _field_in(found, op, fields, source) {
                    relocations.push((out.len() as i64 + (field - span.0), field));
                }
                out.extend_from_slice(bytes_of(found, span.0, span.1));
                continue;
            }
        }
        // A source-level NOTHING may own bytes deleted by a transform while
        // emitting no replacement instruction.
        if _semantics(op).is_none() && op.node.is_none() && op.kind() == mir::Kind::Nothing {
            continue;
        }
        let Some(before) = _semantics(op) else {
            return Err(format!("{:#06x}: {} has no semantics to select from", op.at, op.name().unwrap_or("None")));
        };
        let Some(what) = _retargeted(before, &moved) else {
            return Err(format!("{:#06x}: its target is not in this body", op.at));
        };
        let r#where = _where(op, assignment, origin);
        let relocated =
            _field_in(found, op, fields, source).is_some() || _generated_immediate(op, Some(&what)).is_some();
        let mut made = select::emit(
            &what,
            placed[index] as u64,
            r#where.as_ref().map(Remap::as_where),
            short.contains(&index),
            relocated,
            held.as_ref(),
        );
        if let Some(one) = &made {
            if let Some(protocol) = _emulator_protocol(op, found, native_fpu, source) {
                made = fpu::wrapped(one, protocol);
            }
        }
        let Some(made) = made.filter(|made| made.code.len() as i64 == lengths[index]) else {
            return Err(format!("{:#06x}: it changed length between the two passes", op.at));
        };
        // A fixup goes wherever the field it names landed, paired in order.
        let mut wanted = _fields_in(found, op, fields, source);
        if !wanted.is_empty() && !_relocatable(Some(&what)) {
            // A fixup belongs to an operand, and this instruction has none a
            // relocation could sit in.
            wanted = Vec::new();
        }
        if !wanted.is_empty() {
            let landed = made.places();
            if landed.len() < wanted.len() {
                return Err(format!(
                    "{:#06x}: {} has {} fixups and {} fields to put them in",
                    op.at,
                    op.name().unwrap_or("None"),
                    wanted.len(),
                    landed.len()
                ));
            }
            for (r#where, field) in landed.into_iter().zip(wanted) {
                relocations.push((out.len() as i64 + r#where as i64, field));
            }
        } else {
            // A read-modify-write names its one memory operand as a
            // destination and a source; it is still one field.
            let mut addresses: Vec<Addr> = Vec::new();
            for arg in what.dests.iter().chain(&what.sources) {
                if let Loc::Mem(mem) = arg {
                    if let Some(addr) = mem.addr.filter(|addr| matches!(addr.space, Space::Segment | Space::External)) {
                        if !addresses.contains(&addr) {
                            addresses.push(addr);
                        }
                    }
                }
            }
            let immediate = _generated_immediate(op, Some(&what));
            if let Some(immediate) = immediate {
                if matches!(immediate.space, Space::Segment | Space::External) {
                    addresses.push(immediate);
                }
            }
            if !addresses.is_empty() {
                let places = made.places();
                if addresses.len() != 1 || places.len() != 1 {
                    return Err(format!("{:#06x}: cannot bind a generated symbolic memory operand", op.at));
                }
                let mut address = addresses[0];
                // A symbolic immediate introduced after allocation has no
                // source field to carry through omfwrite's retargeting, so
                // its address is rewritten here using this layout's map.
                if address.space == Space::Segment && address.index == found.seg {
                    let Some(&landed) = moved.get(&address.disp) else {
                        return Err(format!(
                            "{:#06x}: generated code address {:#x} is not placed",
                            op.at, address.disp
                        ));
                    };
                    address = Addr { disp: landed, ..address };
                }
                symbols.push((out.len() as i64 + places[0] as i64, address));
            }
        }
        out.extend_from_slice(&made.code);
    }
    // Every fixup inside a surviving op's `covers` but outside its own
    // node's span belonged to something a transform folded away.
    let kept_fields: BTreeSet<i64> = relocations.iter().map(|&(_where, one)| one).collect();
    let mut explained: BTreeSet<i64> = BTreeSet::new();
    let mut folded: IndexMap<i64, i64> = IndexMap::default();
    for item in ops {
        let op = match item {
            Item::Table(table) => {
                if table.discarded {
                    explained.extend(known.range(table.lo..table.hi));
                }
                continue;
            }
            Item::Op(op) => op,
        };
        let landed = moved.get(&op.at).copied();
        for (lo, hi) in _insn_ranges_of(op, found, source) {
            explained.extend(known.iter().filter(|one| lo <= **one && **one < hi));
            if let Some(landed) = landed {
                for one in lo..hi {
                    if !moved.contains_key(&one) {
                        folded.insert(one, landed);
                    }
                }
            }
        }
    }
    let measured: i64 = lengths.iter().sum();
    if out.len() as i64 != measured {
        return Err(format!("layout measured {measured} emitted bytes but produced {}", out.len()));
    }
    Ok(Laid {
        code: out,
        moved,
        relocations,
        dropped: explained.difference(&kept_fields).copied().collect(),
        covered: folded,
        symbols,
    })
}

/// What to select for this op, or None to carry its bytes.
///
/// A decoded BARRIER means only that the raise cannot describe the source
/// instruction and its original bytes remain authoritative.
pub fn _semantics(op: &Insn) -> Option<&Semantics> {
    let what = op.what.as_ref()?;
    if what.op == Operation::Barrier && op.node.is_some() {
        return None;
    }
    Some(what)
}

/// The registers a remap may name: the register file less sp.
pub static _RENAMEABLE: LazyLock<IndexMap<Register, IndexMap<i64, Register>>> = LazyLock::new(|| {
    target::AT_WIDTH
        .iter()
        .filter(|(r#where, _)| **r#where != Register::ESP)
        .map(|(r#where, widths)| (*r#where, widths.clone()))
        .collect()
});

/// One side's register remap, out of a whole-body allocation.
pub fn _remap(
    values: &[u32],
    assignment: &IndexMap<u32, Register>,
    origin: &IndexMap<u32, Register>,
) -> RegisterMap {
    let mut out = RegisterMap::default();
    for value in values {
        let (Some(&want), Some(&was)) = (assignment.get(value), origin.get(value)) else { continue };
        if want == was {
            continue;
        }
        // At every width, not only the root.
        for width in [4, 2, 1] {
            let here = _RENAMEABLE.get(&was).and_then(|widths| widths.get(&width));
            let there = _RENAMEABLE.get(&want).and_then(|widths| widths.get(&width));
            if let (Some(&here), Some(&there)) = (here, there) {
                out.insert(here, there);
            }
        }
    }
    out
}

/// The allocation keyed by value id, which is what ir.Held names.
pub fn _held(assignment: Option<&IndexMap<u32, Register>>) -> Option<HeldMap> {
    let assignment = assignment.filter(|assignment| !assignment.is_empty())?;
    Some(assignment.iter().map(|(&value, &register)| (value, register)).collect())
}

/// `_where`'s answer: `(into, outof)`.
#[derive(Clone, Debug)]
pub struct Remap(pub RegisterMap, pub RegisterMap);

impl Remap {
    fn as_where(&self) -> Where<'_> {
        Where::Pair(&self.0, &self.1)
    }
}

/// This op's register remap, by side, out of a whole-body allocation.
///
/// By side, because one map cannot say two things about one register:
/// `mov ax,1` defines a value and *uses* the eax before it.
pub fn _where(
    op: &Insn,
    assignment: Option<&IndexMap<u32, Register>>,
    origin: Option<&IndexMap<u32, Register>>,
) -> Option<Remap> {
    let assignment = assignment.filter(|assignment| !assignment.is_empty())?;
    let origin = origin?;
    let into = _remap(&op.defines, assignment, origin);
    let outof = _remap(&op.uses, assignment, origin);
    (!into.is_empty() || !outof.is_empty()).then_some(Remap(into, outof))
}
