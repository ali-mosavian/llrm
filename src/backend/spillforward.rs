//! Port of `qbopt/backend/spillforward.py`: remove reloads of a slot a
//! register already holds on every path here.
//!
//! Within a block a register holds the bytes of every slot it has been read out
//! of or written into since. Across blocks that fact has to be met at every
//! incoming edge, which is an availability problem and not something the first
//! instruction of a block and the last of its predecessor can answer.

use std::collections::HashSet;
use std::sync::Arc;

use iced_x86::Register;
use indexmap::IndexMap;

use crate::backend::peephole::{_frame_cell, _frame_written, _lanes, _overlapping, _register_effects, id};
use crate::model::ir::{Loc, Mem, Operation, Reg};
use crate::model::lir::{self, Insn, LirBlock, LirBody};

/// Python's `frozenset[tuple[ir.Reg, ir.Mem]]`.  Only membership, overlap and
/// equality are read; which of two equal cells a meet keeps reaches nothing.
pub type Facts = HashSet<(Reg, Mem)>;

fn _plain(one: &Insn) -> bool {
    one.what.is_some() && one.clobbers.is_empty() && one.requires.is_empty() && one.delivers.is_empty()
}

fn _empty(one: &Insn) -> bool {
    _plain(one)
        && one.what.as_ref().is_some_and(|what| {
            what.op == Operation::Nothing
                && what.name.as_deref().is_none_or(str::is_empty)
                && what.dests.is_empty()
                && what.sources.is_empty()
                && what.target.is_none()
        })
}

/// `facts` after `one`, and whether it is a reload `facts` makes redundant.
///
/// A fact is a (register, slot) pair meaning the register holds that slot's
/// bytes. Anything whose register effects cannot be read, or which writes
/// memory the displacement alone does not name, ends every fact: the write
/// could be to any slot.
fn _held(one: &Insn, facts: Facts) -> (Facts, bool) {
    if _empty(one) {
        return (facts, false);
    }
    let Some(what) = &one.what else {
        return (Facts::new(), false);
    };
    if [Operation::Branch, Operation::Jump].contains(&what.op)
        && what.dests.is_empty()
        && what.sources.is_empty()
        && what.target.is_some()
    {
        // Its own effects cannot be read -- `_register_effects` answers only for
        // instructions that fall through. It writes no register and no memory,
        // and a block ending in one is otherwise the end of every fact.
        return (facts, false);
    }
    let Some(effects) = _register_effects(one, false, false) else {
        return (Facts::new(), false);
    };
    let writes = effects.1;
    if !writes.is_disjoint(&_lanes(Register::EBP)) {
        return (Facts::new(), false);
    }

    // `op.stores` is the last word only for an instruction that is its own op.
    // A reload carries the op of whatever it stands beside, stores and all, and
    // is a load: reading it as a write to memory ended every fact at the very
    // instruction the facts were there to answer.
    let writing = what.dests.iter().any(|dest| matches!(dest, Loc::Mem(_)))
        || !one.spill_reload && one.op.as_ref().is_some_and(|op| !op.stores.is_empty());
    let mut facts = facts;
    let written = if writing {
        let Some(written) = _frame_written(one) else {
            return (Facts::new(), false);
        };
        facts.retain(|(_register, cell)| !_overlapping(cell, &written));
        Some(written)
    } else {
        None
    };

    if let (Operation::Move, Some("mov"), [Loc::Reg(register)], [Loc::Mem(cell)]) =
        (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice())
    {
        if _frame_cell(cell) && register.width == cell.width {
            if facts.contains(&(*register, cell.clone())) {
                // The program's own load of a local is as redundant as the
                // allocator's: cycleblobs keeps `x%` in bx across its inner
                // loop and reloads it in the latch to increment it.
                let drop = !(!one.clobbers.is_empty()
                    || !one.requires.is_empty()
                    || !one.delivers.is_empty()
                    || one.group.is_some()
                    || one.symbol == Some(true));
                return (facts, drop);
            }
            facts.retain(|pair| _lanes(pair.0.register).is_disjoint(&writes));
            facts.insert((*register, cell.clone()));
            return (facts, false);
        }
    }

    facts.retain(|pair| _lanes(pair.0.register).is_disjoint(&writes));
    if let Some(written) = written {
        if what.op == Operation::Move && what.sources.len() == 1 {
            if let Loc::Reg(source) = &what.sources[0] {
                if source.width == written.width && _lanes(source.register).is_disjoint(&writes) {
                    facts.insert((*source, written));
                }
            }
        }
    }
    (facts, false)
}

/// The facts true on entry to each block, met over every incoming edge.
///
/// `None` stands for the top of the lattice -- a block not reached yet, whose
/// contribution to the meet is every fact. A loop header needs that: met
/// against nothing its backedge would start out killing the very fact the
/// header is there to establish.
fn _available(body: &LirBody) -> IndexMap<i64, Facts> {
    let mut predecessors: IndexMap<i64, Vec<i64>> = body.blocks.iter().map(|block| (block.at, Vec::new())).collect();
    for block in &body.blocks {
        for at in &block.succ {
            if let Some(found) = predecessors.get_mut(at) {
                found.push(block.at);
            }
        }
    }
    let blocks: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut into: IndexMap<i64, Option<Facts>> = body
        .blocks
        .iter()
        .map(|block| {
            (
                block.at,
                if block.at == body.entry || predecessors[&block.at].is_empty() { Some(Facts::new()) } else { None },
            )
        })
        .collect();
    let mut outof: IndexMap<i64, Facts> = IndexMap::new();
    let mut changing = true;
    while changing {
        changing = false;
        for (at, facts) in &into {
            if let Some(facts) = facts {
                let leaving = _transfer(blocks[at], facts.clone()).1;
                if outof.get(at) != Some(&leaving) {
                    outof.insert(*at, leaving);
                    changing = true;
                }
            }
        }
        let ats: Vec<i64> = into.keys().copied().collect();
        for at in ats {
            if at == body.entry || predecessors[&at].is_empty() {
                continue;
            }
            let mut met: Option<Facts> = None;
            for parent in &predecessors[&at] {
                let Some(leaving) = outof.get(parent) else {
                    continue;
                };
                met = Some(match met {
                    None => leaving.clone(),
                    Some(met) => met.iter().filter(|fact| leaving.contains(fact)).cloned().collect(),
                });
            }
            if let Some(met) = met {
                if into[&at].as_ref() != Some(&met) {
                    into.insert(at, Some(met));
                    changing = true;
                }
            }
        }
    }
    into.into_iter().map(|(at, facts)| (at, facts.unwrap_or_default())).collect()
}

/// The reloads `facts` makes redundant in `block`, and the facts after it.
fn _transfer(block: &LirBlock, facts: Facts) -> (Vec<usize>, Facts) {
    let mut facts = facts;
    let mut redundant = Vec::new();
    for one in &block.insns {
        let (after, drop) = _held(one, facts);
        facts = after;
        if drop {
            redundant.push(id(one));
        }
    }
    (redundant, facts)
}

pub fn forwarded(body: &LirBody) -> LirBody {
    let into = _available(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let redundant: HashSet<usize> = _transfer(block, into[&block.at].clone()).0.into_iter().collect();
        if redundant.is_empty() {
            blocks.push(block.clone());
            continue;
        }
        let insns = block
            .insns
            .iter()
            .map(|one| if redundant.contains(&id(one)) { lir::anchor(Arc::clone(one)) } else { Arc::clone(one) })
            .collect();
        blocks.push(LirBlock { insns, ..block.clone() });
    }
    LirBody { blocks, ..body.clone() }
}
