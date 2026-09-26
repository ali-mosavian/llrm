//! A whole body, emitted -- and everything that has to move when it does.
//!
//! Port of `qbopt/backend/layout.py`. Blocks are placed, implicit edges made
//! explicit and branches threaded; `asm` then measures, shrinks every branch
//! to a fixed point and emits. What comes back is bytes plus the relocations,
//! because a relocated displacement is emitted as zero and the fixup that
//! names it has to be moved to wherever the field ended up.
//!
//! Refuses the whole body where it cannot emit one op.
//!
//! Python's `id(op)` is the `Arc` pointer of an LIR occurrence.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, LazyLock};

use iced_x86::Register;

use crate::backend::asm;
use crate::model::ir::nodes::Node;
use crate::model::ir::{Operation, Semantics};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::model::mir;
use crate::model::mir::SourceMap;
use crate::objectfile::module::Module;
use crate::support::hash::IndexMap;

// The assembler's, re-exported: this module builds them and hands them over.
pub use crate::backend::asm::{Item, Laid, Table};

/// Whether this block puts any byte in the output.
pub fn _emits(block: &LirBlock) -> bool {
    block.insns.iter().any(|op| op.emits())
}

fn by_address(body: &LirBody) -> Vec<&LirBlock> {
    let mut ordered: Vec<&LirBlock> = body.blocks.iter().collect();
    ordered.sort_by_key(|block| block.at);
    ordered
}

/// Per block, the next one that emits anything.
///
/// A block whose every operation is NOTHING keeps its address and its
/// `covers` and contributes no bytes, so control reaching its predecessor's
/// end falls through it to whatever comes after.
pub fn _following(body: &LirBody) -> IndexMap<i64, i64> {
    let ordered = by_address(body);
    let mut out = IndexMap::default();
    for (index, block) in ordered.iter().enumerate() {
        if let Some(after) = ordered[index + 1..].iter().find(|after| _emits(after)) {
            out.insert(block.at, after.at);
        }
    }
    out
}

/// Make implicit CFG edges explicit when address-order placement breaks them.
///
/// To a fixed point: a jump given to an empty block makes it emit, and the
/// block before it, which fell through past it, now falls into the jump.
pub fn _fallthroughs(body: LirBody) -> LirBody {
    let mut body = body;
    loop {
        let settled = _fallthroughs_once(&body);
        if settled == body {
            return body;
        }
        body = settled;
    }
}

fn last_what(block: &LirBlock) -> Option<&Semantics> {
    block.insns.last().and_then(|op| op.what.as_ref())
}

fn semantics(op: Operation, name: &str, target: Option<i64>) -> Semantics {
    Semantics { op, name: Some(name.to_owned()), dests: Vec::new(), sources: Vec::new(), target, indirect: false }
}

pub fn _fallthroughs_once(body: &LirBody) -> LirBody {
    let following = _following(body);
    let mut changed = Vec::new();
    for block in &body.blocks {
        let last = last_what(block);
        let mut destination = None;
        if block.succ.len() == 1
            && last.is_none_or(|last| !matches!(last.op, Operation::Jump | Operation::Branch | Operation::Return))
        {
            destination = Some(block.succ[0]);
        } else if block.succ.len() == 2 {
            if let Some(last) = last.filter(|last| last.op == Operation::Branch) {
                if last.target.is_some_and(|target| block.succ.contains(&target)) {
                    destination = block.succ.iter().copied().find(|at| Some(*at) != last.target);
                }
            }
        }
        let after = following.get(&block.at).copied();
        if let Some(destination) = destination.filter(|destination| Some(*destination) != after) {
            if let Some(turned) = _turned(block, last, destination, after) {
                changed.push(turned);
                continue;
            }
            let anchor = block.insns.last().map_or(block.at, |op| op.at);
            let mut jump = Insn::new(
                anchor,
                Some((anchor, anchor)),
                Some(semantics(Operation::Jump, "jmp", Some(destination))),
                Vec::new(),
                Vec::new(),
            );
            jump.symbol = Some(false);
            let mut insns = block.insns.clone();
            insns.push(Arc::new(jump));
            changed.push(block.with_insns(insns));
            continue;
        }
        changed.push(block.clone());
    }
    body.with_blocks(changed)
}

/// Each condition and the one that is true exactly when it is false. `jcxz`
/// and `loop*` are absent on purpose: they have no inverse to name, so a
/// block ending in one keeps its jump.
pub const _PAIRS: [(&str, &str); 15] = [
    ("je", "jne"),
    ("jz", "jnz"),
    ("jl", "jge"),
    ("jnge", "jnl"),
    ("jle", "jg"),
    ("jng", "jnle"),
    ("jb", "jae"),
    ("jc", "jnc"),
    ("jnae", "jnb"),
    ("jbe", "ja"),
    ("jna", "jnbe"),
    ("js", "jns"),
    ("jo", "jno"),
    ("jp", "jnp"),
    ("jpe", "jpo"),
];

pub static _OPPOSITE: LazyLock<IndexMap<&'static str, &'static str>> = LazyLock::new(|| {
    _PAIRS.iter().flat_map(|&(one, other)| [(one, other), (other, one)]).collect()
});

/// `block` with its branch inverted, where that is what removes the jump:
/// only when the branch already goes to the block placed next.
pub fn _turned(block: &LirBlock, last: Option<&Semantics>, destination: i64, after: Option<i64>) -> Option<LirBlock> {
    let last = last?;
    if last.op != Operation::Branch || after.is_none() || last.target != after {
        return None;
    }
    _inverted(block, last.name.as_deref(), Some(destination))
}

/// `block` with its closing branch's sense reversed, or None.
///
/// Inverting a condition is the same control flow either way, so this needs
/// no proof beyond the mnemonic having an opposite.
pub fn _inverted(block: &LirBlock, name: Option<&str>, target: Option<i64>) -> Option<LirBlock> {
    let opposite = *_OPPOSITE.get(name.unwrap_or("").to_lowercase().as_str())?;
    let branch = block.insns.last()?;
    let what = branch.what.as_ref().filter(|what| what.op == Operation::Branch)?;
    let r#where = if target.is_none() { what.target } else { target };
    let mut turned = (**branch).clone();
    turned.what = Some(Semantics { name: Some(opposite.to_owned()), target: r#where, ..what.clone() });
    let mut insns = block.insns[..block.insns.len() - 1].to_vec();
    insns.push(Arc::new(turned));
    Some(block.with_insns(insns))
}

/// Branch past a block that only jumps somewhere else.
///
/// QB's BC writes `je dc / jmp f5 / dc:` where PDS's BC writes the inverted
/// branch and no jump at all. Reversing the condition and sending it to `f5`
/// leaves `dc` as the fall-through and the jump with nothing to do. The
/// emptied block keeps its place and its `covers`, because BC's bytes have
/// to stay owned by something.
pub fn _threaded(body: LirBody) -> LirBody {
    let following = _following(&body);
    let at_of: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut reached: HashMap<i64, usize> = HashMap::new();
    for block in &body.blocks {
        for &at in &block.succ {
            *reached.entry(at).or_insert(0) += 1;
        }
    }

    let mut turned: IndexMap<i64, LirBlock> = IndexMap::default();
    for block in &body.blocks {
        let Some(last) = last_what(block) else { continue };
        if last.op != Operation::Branch || block.succ.len() != 2 {
            continue;
        }
        let Some(target) = last.target.filter(|target| block.succ.contains(target)) else { continue };
        let through = block.succ.iter().copied().find(|at| *at != target).expect("two successors");
        if Some(through) != following.get(&block.at).copied()
            || reached.get(&through).copied().unwrap_or(0) != 1
            || turned.contains_key(&through)
        {
            continue;
        }
        let Some(middle) = at_of.get(&through) else { continue };
        if Some(target) != following.get(&through).copied() {
            continue;
        }
        let alive: Vec<&Arc<Insn>> = middle.insns.iter().filter(|op| op.emits()).collect();
        if alive.len() != 1 || alive[0].kind() != mir::Kind::Jump || middle.succ.len() != 1 {
            continue;
        }
        let beyond = middle.succ[0];
        if beyond == through || !at_of.contains_key(&beyond) {
            continue;
        }
        let Some(inverted) = _inverted(block, last.name.as_deref(), Some(beyond)) else { continue };
        turned.insert(block.at, LirBlock { succ: vec![beyond, target], ..inverted });
        turned.insert(
            through,
            LirBlock { succ: Vec::new(), ..middle.with_insns(middle.insns.iter().map(|op| _emptied(op)).collect()) },
        );
    }
    if turned.is_empty() {
        return body;
    }
    let blocks = body.blocks.iter().map(|block| turned.get(&block.at).cloned().unwrap_or_else(|| block.clone())).collect();
    body.with_blocks(blocks)
}

/// `op` emitting nothing, still owning the bytes it covers.
pub fn _emptied(op: &Arc<Insn>) -> Arc<Insn> {
    let mut emptied = (**op).clone();
    emptied.defines = Vec::new();
    emptied.uses = Vec::new();
    emptied.what = Some(Semantics {
        op: Operation::Nothing,
        name: Some(String::new()),
        dests: Vec::new(),
        sources: Vec::new(),
        target: None,
        indirect: false,
    });
    emptied.clobbers = BTreeSet::new();
    emptied.clobbers_high = BTreeSet::new();
    emptied.requires = Vec::new();
    emptied.delivers = Vec::new();
    emptied.symbol = Some(false);
    Arc::new(emptied)
}

/// A jump to the block placed next emits nothing: control falls through to it.
pub fn _fallen(body: LirBody) -> LirBody {
    let following = _following(&body);
    let mut changed = Vec::new();
    for block in &body.blocks {
        let last = last_what(block);
        if let Some(last) = last {
            if last.op == Operation::Jump
                && last.target.is_some_and(|target| block.succ == [target])
                && last.target == following.get(&block.at).copied()
            {
                let mut insns = block.insns.clone();
                let emptied = _emptied(insns.last().expect("a last op"));
                *insns.last_mut().expect("a last op") = emptied;
                changed.push(block.with_insns(insns));
                continue;
            }
        }
        changed.push(block.clone());
    }
    body.with_blocks(changed)
}

/// Every op, in the order they are emitted: blocks in address order, and
/// within a block the order the block lists them.
///
/// An authoritative sequence may lay an entire acyclic single-successor body
/// out in execution order. Branching, cycles and disconnected blocks retain
/// their original placement.
pub fn _ordered(body: &LirBody, linear: bool) -> Vec<Arc<Insn>> {
    let mut blocks = by_address(body);
    if linear {
        let at_of: IndexMap<i64, &LirBlock> = blocks.iter().map(|block| (block.at, *block)).collect();
        let mut chain: Vec<&LirBlock> = Vec::new();
        let mut seen: BTreeSet<i64> = BTreeSet::new();
        let mut at = body.entry;
        while let Some(&block) = at_of.get(&at) {
            if seen.contains(&at) || block.succ.len() > 1 {
                break;
            }
            chain.push(block);
            seen.insert(at);
            if block.succ.is_empty() {
                if chain.len() == blocks.len() {
                    blocks = chain;
                }
                break;
            }
            at = block.succ[0];
        }
    }
    blocks.iter().flat_map(|block| block.insns.iter().cloned()).collect()
}

fn length_of(one: &Insn, found: &Module, source: Option<&SourceMap>) -> i64 {
    asm::_length_of(one, found, source).unwrap_or(0)
}

/// The run of zero bytes the ops end on, where it reaches the segment's end.
///
/// Only at the very end, and only all-zero: anything else that happens to
/// decode is code until something proves otherwise.
pub fn _trailing_zeros(found: &Module, ops: &[Arc<Insn>], source: Option<&SourceMap>) -> Option<Table> {
    let highest = ops.iter().map(|one| one.at + length_of(one, found, source)).max().expect("max() arg is an empty sequence");
    if highest != found.end {
        return None;
    }
    let mut lo = found.end;
    let mut sorted: Vec<&Arc<Insn>> = ops.iter().collect();
    sorted.sort_by(|one, other| other.at.cmp(&one.at));
    for one in sorted {
        let length = length_of(one, found, source);
        if one.at + length != lo || found.code[one.at as usize..lo as usize].iter().any(|byte| *byte != 0) {
            break;
        }
        lo = one.at;
    }
    (lo != found.end).then_some(Table::new(lo, found.end))
}

pub const PADDING: [u8; 2] = [0x90, 0x00];

/// The gaps between the items that may be carried rather than selected.
///
/// Padding is the easy half: BC aligns its procedures, so runs of `90` sit
/// between them. The other half is code the decoder never reached; those
/// are discarded, not carried, since what kept them dead was the `jmp` in
/// front and layout drops a jump to the next block. `reached` is what makes
/// the question askable here.
pub fn _padding_runs(
    found: &Module,
    ops: &[Arc<Insn>],
    carried: &[Table],
    lowest: i64,
    highest: i64,
    reached: Option<&BTreeSet<i64>>,
    source: Option<&SourceMap>,
) -> Vec<Table> {
    let mut covered: BTreeSet<i64> = BTreeSet::new();
    for one in ops {
        for (lo, hi) in asm::_ranges_of(&Item::Op(one.clone()), found, source) {
            covered.extend(lo..hi);
        }
    }
    for one in carried {
        covered.extend(one.lo..one.hi);
    }

    let mut out: Vec<Table> = Vec::new();
    let mut start: Option<i64> = None;
    for at in lowest..=highest {
        let empty = at < highest && !covered.contains(&at);
        if empty && start.is_none() {
            start = Some(at);
        } else if !empty {
            if let Some(begun) = start {
                let padding = found.code[begun as usize..at as usize].iter().all(|one| PADDING.contains(one));
                if padding || reached.is_some_and(|reached| !(begun..at).any(|one| reached.contains(&one))) {
                    out.push(Table { lo: begun, hi: at, discarded: !padding });
                }
                start = None;
            }
        }
    }
    out
}

/// Whether this op's bytes come from select rather than from the image.
///
/// An op emitted verbatim is exactly as long as the bytes it copies, so its
/// `covers` and its length are the same number and a transform may not make
/// them differ.
pub fn selectable(op: &Insn) -> bool {
    op.what.as_ref().is_some_and(|what| what.op != Operation::Barrier)
}

/// Every op in `body`, emitted in order from `at`, or why it could not be.
pub fn lay_out(
    body: &LirBody,
    at: i64,
    found: &Module,
    fields: &BTreeSet<i64>,
    source: Option<&SourceMap>,
) -> Result<Laid, String> {
    let ops: Vec<Item> = _ordered(body, false).into_iter().map(Item::Op).collect();
    asm::assemble(&ops, at, found, fields, false, None, None, Some(&_labels(body)), Some(&_anchors(body)), source)
}

pub fn _labels(body: &LirBody) -> IndexMap<i64, i64> {
    let mut labels = IndexMap::default();
    let mut following: Option<i64> = None;
    for block in reversed_by_address(body) {
        if let Some(first) = block.insns.first() {
            following = Some(first.at);
        }
        if let Some(following) = following {
            labels.insert(block.at, following);
        }
    }
    labels
}

/// Block labels designate occurrences, not repeated source addresses.
pub fn _anchors(body: &LirBody) -> IndexMap<i64, Arc<Insn>> {
    let mut labels = IndexMap::default();
    let mut following: Option<Arc<Insn>> = None;
    for block in reversed_by_address(body) {
        if let Some(first) = block.insns.first() {
            following = Some(first.clone());
        }
        if let Some(following) = &following {
            labels.insert(block.at, following.clone());
        }
    }
    labels
}

fn reversed_by_address(body: &LirBody) -> Vec<&LirBlock> {
    // Python's `sorted(..., reverse=True)` keeps equal keys in their order.
    let mut blocks: Vec<&LirBlock> = body.blocks.iter().collect();
    blocks.sort_by(|one, other| other.at.cmp(&one.at));
    blocks
}

/// Every body in the module, laid out one after another.
///
/// Whole-segment rather than per-body, because per-body does not work:
/// splicing one back into BC's own layout is possible for 1 of the corpus's
/// 171 bodies. What comes back starts at the first body's own address;
/// whatever sits before it is the caller's to keep.
#[allow(clippy::too_many_arguments)]
pub fn rebuild(
    found: &Module,
    bodies: Vec<(String, LirBody)>,
    tables: &[(i64, i64)],
    fields: &BTreeSet<i64>,
    reached: Option<&BTreeSet<i64>>,
    native_fpu: bool,
    assignment: Option<&IndexMap<u32, Register>>,
    ordered: bool,
    ordered_entries: &BTreeSet<i64>,
    source: Option<&SourceMap>,
) -> Result<Laid, String> {
    // Bodies are allocated LIR. Layout changes placement and branches; it
    // never chooses registers or reconstructs machine form from MIR.
    let sequenced: BTreeSet<i64> =
        if ordered { bodies.iter().map(|(_, body)| body.entry).collect() } else { ordered_entries.clone() };
    let bodies: Vec<(String, LirBody)> =
        bodies.into_iter().map(|(name, body)| (name, _fallen(_fallthroughs(_threaded(body))))).collect();
    // Sorted on the address an operation's bytes start at, unless a caller
    // with an authoritative emission sequence opts into `ordered`.
    let mut groups: Vec<(i64, Vec<Arc<Insn>>)> = Vec::new();
    for (_, body) in &bodies {
        let end = body
            .blocks
            .iter()
            .flat_map(|block| &block.insns)
            .filter_map(|op| op.covers.map(|covers| covers.1))
            .max()
            .unwrap_or(body.entry);
        let embedded = tables.iter().any(|&(start, stop)| start < end && stop > body.entry);
        let sequence = _ordered(body, sequenced.contains(&body.entry) && !embedded);
        if ordered || sequenced.contains(&body.entry) {
            groups.push((body.entry, sequence));
        } else {
            groups.extend(sequence.into_iter().map(|op| (op.at, vec![op])));
        }
    }
    if !ordered {
        groups.sort_by_key(|group| group.0);
    }
    let mut ops: Vec<Arc<Insn>> = groups.into_iter().flat_map(|(_, sequence)| sequence).collect();
    if ops.is_empty() {
        return Err("no bodies to rebuild".to_owned());
    }
    if ops.iter().any(|one| asm::_length_of(one, found, source).is_none()) {
        return Err(format!("{:#06x}: an op with no instruction behind it", ops[0].at));
    }

    let lowest = bodies.iter().map(|(_, body)| body.entry).min().expect("ops came from a body");
    let mut highest = ops
        .iter()
        .filter_map(|one| asm::_stands_for(&Item::Op(one.clone()), found, source))
        .filter(|span| span.0 < span.1)
        .map(|span| span.1)
        .max()
        .unwrap_or(lowest);
    let dead_dispatch_ends: BTreeSet<i64> = ops
        .iter()
        .filter(|op| op.what.as_ref().is_some_and(|what| what.op == Operation::Nothing))
        .filter_map(|op| match op.node.as_deref() {
            Some(Node::Call(call)) if call.name == "B$OGTA" => Some(call.insn.end() as i64),
            _ => None,
        })
        .collect();
    let mut inside: Vec<Table> = tables
        .iter()
        .filter(|&&(lo, hi)| lowest <= lo && hi <= found.end)
        .map(|&(lo, hi)| Table { lo, hi, discarded: dead_dispatch_ends.contains(&lo) })
        .collect();
    if let Some(top) = inside.iter().map(|one| one.hi).max() {
        highest = highest.max(top);
    }

    // BC pads the end of its code segment with zeros. They are not
    // instructions and are carried rather than selected.
    if let Some(padding) = _trailing_zeros(found, &ops, source) {
        inside.push(padding);
        ops.retain(|one| one.at < padding.lo);
        if ops.is_empty() {
            return Err("the body is nothing but padding".to_owned());
        }
        highest = padding.hi;
    }

    // BC aligns its procedures, so runs of `90` sit between them, and
    // nothing reaches those.
    let runs = _padding_runs(found, &ops, &inside, lowest, highest, reached, source);
    inside.extend(runs);

    // Every byte between the first item and the last has to be one of them.
    let covered: i64 = ops.iter().map(|one| length_of(one, found, source)).sum::<i64>()
        + inside.iter().map(|one| one.hi - one.lo).sum::<i64>();
    if covered != highest - lowest {
        // Named where the gap is, not where the layout starts.
        let mut held: BTreeSet<i64> = BTreeSet::new();
        let mut claims: IndexMap<i64, Vec<i64>> = IndexMap::default();
        for one in &ops {
            if let Some((lo, hi)) = asm::_stands_for(&Item::Op(one.clone()), found, source) {
                held.extend(lo..hi);
                for byte in lo..hi {
                    claims.entry(byte).or_default().push(one.at);
                }
            }
        }
        for one in &inside {
            held.extend(one.lo..one.hi);
        }
        let first = (lowest..highest).find(|one| !held.contains(one));
        let Some(first) = first else {
            // Every byte is accounted for and the total still disagrees, so
            // two ops claim the same ones.
            let twice: Vec<i64> =
                (lowest..highest).filter(|one| claims.get(one).is_some_and(|claims| claims.len() > 1)).collect();
            let r#where = twice.first().map_or_else(|| "nowhere".to_owned(), |one| format!("{one:#06x}"));
            return Err(format!(
                "{where}: {} bytes are claimed by more than one op",
                covered - (highest - lowest)
            ));
        };
        return Err(format!(
            "{first:#06x}: {} bytes between the ops are not instructions",
            highest - lowest - covered
        ));
    }

    let mut origin: IndexMap<u32, Register> = IndexMap::default();
    for (_name, body) in &bodies {
        origin.extend(body.origin.iter().map(|(value, register)| (*value, *register)));
    }
    let labels: IndexMap<i64, i64> = bodies.iter().flat_map(|(_, body)| _labels(body)).collect();
    // A block label names an instruction occurrence. Its source address is
    // only provenance and may be shared by inserted instructions in several
    // blocks.
    let anchors: IndexMap<i64, Arc<Insn>> = bodies.iter().flat_map(|(_, body)| _anchors(body)).collect();
    asm::assemble(
        &_interleaved(&ops, &inside),
        lowest,
        found,
        fields,
        native_fpu,
        assignment,
        Some(&origin),
        Some(&labels),
        Some(&anchors),
        source,
    )
}

/// The first original byte this operation stands for.
///
/// `covers`, not `at`: the raise gives every operation folded out of one
/// runtime call the same `at`.
pub fn _starts_at(op: &Insn) -> i64 {
    op.covers.map_or(op.at, |covers| covers.0)
}

/// `ops` in their own order, with each carried run back where it sat.
///
/// A table follows the operation owning its preceding original bytes, even
/// when that operation moved. Clones own no bytes and cannot become anchors.
pub fn _interleaved(ops: &[Arc<Insn>], inside: &[Table]) -> Vec<Item> {
    let mut rank_of: HashMap<*const Insn, (usize, usize)> = HashMap::new();
    for (index, one) in ops.iter().enumerate() {
        rank_of.insert(Arc::as_ptr(one), (index, 1));
    }
    let mut ranked: Vec<((usize, usize), Item)> =
        ops.iter().map(|one| (rank_of[&Arc::as_ptr(one)], Item::Op(one.clone()))).collect();
    for one in inside {
        let preceding = ops
            .iter()
            .enumerate()
            .flat_map(|(index, op)| {
                op.covers.into_iter().chain(op.extra_covers()).map(move |(low, high)| (low, high, index))
            })
            .filter(|&(low, high, _)| low < high && high <= one.lo)
            .map(|(_, high, index)| (high, index))
            .max();
        let index = preceding.map_or(0, |(_, index)| index + 1);
        ranked.push(((index, 0), Item::Table(*one)));
    }
    ranked.sort_by_key(|(rank, _)| *rank);
    ranked.into_iter().map(|(_, item)| item).collect()
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
