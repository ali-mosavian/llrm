//! Port of `qbopt/model/mir.py`'s absorbable call sites: `_ABSORBS`,
//! `absorbs`, `_within`, `_disjoint`, `_absorbing`, `CONSUMES`, `_consumed`,
//! `_hands_back`, `_handing_back`, `_sites`, `_folded`, `_flags_after`.

use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::{Arc, LazyLock};

use iced_x86::Register;

use super::{
    Arg, Cell, Const, Held, Kind, MemRef, Op, OpCode, OrderedMap, RaisedBody, Raising, Synth, Value, next_id,
    raising_ranges, restore_pair,
};
use crate::analysis::flags as flagged;
use crate::frontend::blocks::Block;
use crate::legacy::calls::{self as machine, CallSite};
use crate::model::ir::nodes::{Node, RESTORE_EFFECTS, Restore};
use crate::objectfile::module::{Module, Object};
use crate::support::hash::IndexMap;

/// What each absorbable routine computes, in MIR's own vocabulary.
pub static _ABSORBS: LazyLock<IndexMap<&'static str, Kind>> = LazyLock::new(|| {
    IndexMap::from_iter([
        ("B$MUI4", Kind::Mul),
        ("B$DVI4", Kind::Divmod),
        ("B$RMI4", Kind::Divmod),
        ("B$CPI4", Kind::Sub), // a comparison subtracts and keeps only the flags
    ])
});

/// The kind a site of this name raises as, or None if it stays a call.
pub fn absorbs(name: &str) -> Option<Kind> {
    _ABSORBS.get(name.to_uppercase().as_str()).copied()
}

/// Every byte a site's pushes occupy, so the raise can pass over them.
pub fn _within(sites: &IndexMap<i64, CallSite>) -> BTreeSet<i64> {
    let mut out: BTreeSet<i64> =
        sites.values().flat_map(|site| (site.start..site.at).map(|at| at as i64)).collect();
    out.extend(sites.values().flat_map(|site| site.consume.iter()).flat_map(|one| (one.at..one.end()).map(|at| at as i64)));
    out
}

/// A site's pushes as contiguous runs, where they sit apart from `covers`.
///
/// A site frames() found may have a real instruction between its last push
/// and its call -- lngmix spills there -- so it stands for two runs.
pub fn _disjoint(site: &CallSite) -> Vec<(i64, i64)> {
    let mut runs: Vec<(i64, i64)> = Vec::new();
    for insn in &site.consume {
        match runs.last_mut() {
            Some(last) if last.1 == insn.at as i64 => last.1 = insn.end() as i64,
            _ => runs.push((insn.at as i64, insn.end() as i64)),
        }
    }
    runs
}

/// One absorbable call as (kind, args, results), or None.
///
/// A static address becomes a four-byte cell and an immediate a constant;
/// the results are the values the call defines.
pub fn _absorbing(site: &CallSite, written: &IndexMap<Register, Value>) -> Option<(Kind, Vec<Arg>, Vec<Arg>)> {
    let kind = *_ABSORBS.get(site.name.to_uppercase().as_str())?;
    let mut args = Vec::new();
    let (left, right) = site.operands();
    for one in [left, right] {
        match (one.kind, one.addr) {
            (machine::Kind::Static, Some(addr)) => args.push(Arg::Cell(Cell { r#ref: MemRef::new(Some(addr), 4) })),
            (machine::Kind::Constant, _) => args.push(Arg::Const(Const::new(one.value, 4))),
            _ => return None,
        }
    }
    if kind == Kind::Divmod {
        // By role, not by register number: the visible long is whichever
        // answer the routine's name promises, the other is the one it was
        // not asked for. Register order named the divisor's register.
        let visible = written.get(&machine::RESULT);
        let other = machine::other_result(site).and_then(|register| written.get(&register));
        let (Some(&visible), Some(&other)) = (visible, other) else {
            return None;
        };
        if visible.flags {
            return None;
        }
        let pair = if site.name.to_uppercase() == machine::REMAINDER { [other, visible] } else { [visible, other] };
        return Some((kind, args, pair.into_iter().map(|one| Arg::Held(Held { value: one, width: 4 })).collect()));
    }
    let mut items: Vec<(&Register, &Value)> = written.iter().collect();
    items.sort();
    let made: Vec<Arg> = items
        .into_iter()
        .filter(|(_, value)| !value.flags)
        .map(|(_, value)| Arg::Held(Held { value: *value, width: 4 }))
        .take(2)
        .collect();
    Some((kind, args, made))
}

// How many bytes each runtime routine pops off before it returns: four per
// long argument, and only for the routines runtime.py has read.
pub static CONSUMES: LazyLock<IndexMap<&'static str, i64>> =
    LazyLock::new(|| IndexMap::from_iter([("B$MUI4", 8), ("B$DVI4", 8), ("B$RMI4", 8), ("B$CPI4", 8)]));

/// The bytes this routine takes off the stack, or None if unknown.
pub fn _consumed(name: &str) -> Option<i64> {
    CONSUMES.get(name.to_uppercase().as_str()).copied()
}

/// (what the handed-back half defines, what it was, the answer it is of).
///
/// An absorbed divide's visible answer is one 32-bit value and BC reads a
/// long as two halves, so the site ends by handing the high one over.
pub fn _hands_back(
    kind: Kind,
    written: &IndexMap<Register, Value>,
    before: &IndexMap<Register, Value>,
) -> Option<(Value, Value, Value)> {
    if kind != Kind::Divmod && kind != Kind::Mul {
        return None;
    }
    let (source, into) = restore_pair(0)?;
    if machine::RESULT != source {
        return None;
    }
    let (answer, was, now) = (written.get(&source)?, before.get(&into)?, written.get(&into)?);
    Some((*now, *was, *answer))
}

/// The half a site hands back, as what it is rather than as a clobber.
///
/// It owns none of BC's bytes; `was` is the previous contents of the place
/// the half lands in, a merge rather than an input.
pub fn _handing_back(at: i64, _node: &Arc<Node>, now: Value, was: Value, answer: Value) -> Op {
    let mut made = Op::new(at, OpCode::Synth(Synth::HalfToLow), "restore", vec![now], vec![was, answer]);
    made.kind = Kind::Extract;
    made.args = vec![Arg::Held(Held { value: answer, width: 4 }), Arg::Const(Const::new(16, 4))];
    made.results = vec![Arg::Held(Held { value: now, width: 2 })];
    made.merges = OrderedMap::from_iter([(was, now)]);
    made.id = Some(next_id());
    let node = Node::Restore(Restore::new(at as usize, at as usize, 0, RESTORE_EFFECTS[&0].clone()));
    made.raising = Some(Box::new(Raising { node: Some(Arc::new(node)), covers: Some((at, at)), extra_covers: Vec::new() }));
    made
}

/// Every absorbable runtime call, by the address of its call.
///
/// Python catches any exception from `calls.sites` and answers `{}`; the
/// Rust `sites` has no failure path to catch.
pub fn _sites(found: &Module, blocks: &[Block]) -> IndexMap<i64, CallSite> {
    let reached: Vec<_> = blocks.iter().flat_map(|block| block.insns.iter().cloned()).collect();
    let found_sites = machine::sites(found, &reached, blocks);
    let live = flagged::live_in(blocks);
    let mut out = IndexMap::default();
    for one in found_sites {
        if machine::DIVIDES.contains(&one.name.as_str()) || one.name == machine::MULTIPLY {
            continue; // raising_calls recovers values at the pushes, not a frozen machine sequence.
        }
        if one.pushed.is_empty() || !_ABSORBS.contains_key(one.name.to_uppercase().as_str()) {
            continue;
        }
        // Only where the sequence exists: the raise and the emitter both ask
        // calls.py which sites are folded.
        if machine::absorb(&one, _flags_after(blocks, &live, one.start, one.end), true).is_err() {
            continue;
        }
        out.insert(one.at as i64, one);
    }
    out
}

/// What `_folded` answers: (held, absorbed, refs, coverage).
pub type Folded = (
    OrderedMap<Value, Register>,
    IndexMap<u32, Object>,
    IndexMap<u32, Vec<i64>>,
    IndexMap<u32, Vec<(i64, i64)>>,
);

/// Each folded operation told which site it stands for, and which fixups.
///
/// `absorbed` holds `(CallSite, Flag)` per op id. The held values pin a
/// site's results to the registers its fixed sequence ends in.
pub fn _folded(body: &RaisedBody, found: &Module, blocks: &[Block]) -> Folded {
    let sites: IndexMap<i64, CallSite> =
        _sites(found, blocks).into_values().map(|one| (one.start as i64, one)).collect();
    if sites.is_empty() {
        return (OrderedMap::new(), IndexMap::default(), IndexMap::default(), IndexMap::default());
    }
    let mut held: OrderedMap<Value, Register> = OrderedMap::new();
    let mut absorbed: IndexMap<u32, Object> = IndexMap::default();
    let mut refs: IndexMap<u32, Vec<i64>> = IndexMap::default();
    let mut coverage: IndexMap<u32, Vec<(i64, i64)>> = IndexMap::default();
    let live = flagged::live_in(blocks);
    for block in &body.blocks {
        for op in &block.ops {
            let Some(site) = sites.get(&op.at) else { continue };
            let Some(id) = op.id else { continue };
            if op.kind == Kind::Call {
                continue;
            }
            // The half a site hands back stands at the same address.
            if op.op == Some(OpCode::Synth(Synth::HalfToLow)) {
                continue;
            }
            // The emitter is given the flags the raise filtered on.
            let read = _flags_after(blocks, &live, site.start, site.end);
            absorbed.insert(id, Rc::new((site.clone(), read)) as Object);
            for one in &op.defines {
                if let Some(&register) = body.origin.get(one).filter(|_| !one.flags) {
                    held.insert(*one, register);
                }
            }
            if let Ok(made) = machine::absorb(site, read, true) {
                if !made.relocations.is_empty() {
                    refs.insert(id, made.relocations.iter().map(|&(_where, field)| field as i64).collect());
                }
            }
            let pushes = _disjoint(site);
            let ranges = raising_ranges(op);
            if !pushes.is_empty() && !ranges.is_empty() {
                coverage.insert(id, std::iter::once(ranges[0]).chain(pushes).collect());
            }
        }
    }
    (held, absorbed, refs, coverage)
}

/// Which flags something reads after this region, by flags.py's analysis:
/// MIR's one FLAGS value cannot say which flag.
pub fn _flags_after(blocks: &[Block], live: &IndexMap<usize, flagged::Flag>, lo: usize, hi: usize) -> flagged::Flag {
    for block in blocks {
        if block.at <= lo && lo < block.end {
            return flagged::live_after(block, hi, live);
        }
    }
    flagged::Flag(0)
}
