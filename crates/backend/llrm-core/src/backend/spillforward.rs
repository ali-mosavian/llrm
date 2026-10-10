//! Port of `qbopt/backend/spillforward.py`: remove reloads of a slot a
//! register already holds on every path here, and stores of such a register
//! back into that slot.
//!
//! Within a block a register holds the bytes of every slot it has been read out
//! of or written into since. Across blocks that fact has to be met at every
//! incoming edge, which is an availability problem and not something the first
//! instruction of a block and the last of its predecessor can answer.

use std::sync::Arc;

use crate::analysis::dataflow::{self, Direction};
use crate::backend::peephole::{_frame_cell, _frame_written, _lanes, _overlapping, _register_effects, id};
use crate::model::ir::{Loc, Mem, Operation, Reg};
use crate::model::lir::{self, Insn, LirBlock, LirBody};
use crate::support::hash::HashSet;
use crate::support::hash::IndexMap;

/// Python's `frozenset[tuple[ir.Reg, ir.Mem]]`.  Only membership, overlap and
/// equality are read; which of two equal cells a meet keeps reaches nothing.
/// Each fact says whether the register is what a store wrote the slot from: a
/// register a reload filled may be a dead one, which a read of it would keep.
pub type Facts = crate::support::hash::HashMap<(Reg, Mem), bool>;

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

/// `facts` after `one`, and whether it is a reload or store `facts` makes
/// redundant.
///
/// A fact is a (register, slot) pair meaning the register holds that slot's
/// bytes. Anything whose register effects cannot be read, or which writes
/// memory the displacement alone does not name, ends every fact: the write
/// could be to any slot.
fn _held(
    bits: u32,
    one: &Insn,
    facts: Facts,
) -> (Facts, bool, Option<(usize, Reg)>) {
    if _empty(one) {
        return (facts, false, None);
    }
    let Some(what) = &one.what else {
        return (Facts::default(), false, None);
    };
    if [Operation::Branch, Operation::Jump].contains(&what.op)
        && what.dests.is_empty()
        && what.sources.is_empty()
        && what.target.is_some()
    {
        // Its own effects cannot be read -- `_register_effects` answers only
        // for instructions that fall through. It writes no register and
        // no memory, and a block ending in one is otherwise the end of
        // every fact.
        return (facts, false, None);
    }
    let Some(effects) = _register_effects(bits, one, false, false) else {
        return (Facts::default(), false, None);
    };
    let writes = effects.1;
    if !writes.is_disjoint(&_lanes(crate::backend::registerinfo::frame_root())) {
        return (Facts::default(), false, None);
    }
    if let (Operation::Move, Some("mov"), [Loc::Mem(cell)], [Loc::Reg(register)]) =
        (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice())
        && _plain(one)
        && !one.volatile
        && one.group.is_none()
        && facts.contains_key(&(*register, cell.clone()))
    {
        return (facts, true, None);
    }

    // `op.stores` is the last word only for an instruction that is its own op.
    // A reload carries the op of whatever it stands beside, stores and all, and
    // is a load: reading it as a write to memory ended every fact at the very
    // instruction the facts were there to answer.
    let writing = what.dests.iter().any(|dest| matches!(dest, Loc::Mem(_)))
        || !one.spill_reload && one.call.as_ref().is_some_and(|call| call.writes());
    let mut facts = facts;
    let written = if writing {
        let Some(written) = _frame_written(one) else {
            return (Facts::default(), false, None);
        };
        facts.retain(|(_register, cell), _| !_overlapping(cell, &written));
        Some(written)
    } else {
        None
    };

    if let (Operation::Move, Some("mov"), [Loc::Reg(register)], [Loc::Mem(cell)]) =
        (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice())
    {
        if _frame_cell(cell) && register.width == cell.width {
            if facts.contains_key(&(*register, cell.clone())) {
                // The program's own load of a local is as redundant as the
                // allocator's: cycleblobs keeps `x%` in bx across its inner
                // loop and reloads it in the latch to increment it.
                let drop = !(!one.clobbers.is_empty()
                    || !one.requires.is_empty()
                    || !one.delivers.is_empty()
                    || one.group.is_some()
                    || one.symbol == Some(true));
                return (facts, drop, None);
            }
            // Another register of the width holds it: the reload is a copy of
            // that register.
            let held_in = facts
                .iter()
                .filter(|((other, held), stored)| {
                    **stored
                        && held == cell
                        && other.width == register.width
                        && other.register != register.register
                        && other.register.is_gpr()
                        && register.register.is_gpr()
                })
                .map(|((other, _), _)| *other)
                .min_by_key(|other| other.register as u32)
                .filter(|_| _plain(one) && !one.volatile && one.group.is_none() && one.symbol != Some(true));
            facts.retain(|pair, _| _lanes(pair.0.register).is_disjoint(&writes));
            facts.insert((*register, cell.clone()), false);
            return (facts, false, held_in.map(|from| (0, from)));
        }
    }

    // An ALU or compare operand read from a slot a register holds is that
    // register.
    let held_source = (|| {
        let name = what.name.as_deref()?;
        if !matches!(
            name,
            "add" | "sub" | "and" | "or" | "xor" | "cmp" | "test" | "adc" | "sbb"
        )
            || !_plain(one)
            || one.volatile
            || one.group.is_some()
            || one.symbol == Some(true)
        {
            return None;
        }
        let memories: Vec<usize> = what
            .sources
            .iter()
            .enumerate()
            .filter(|(_, place)| matches!(place, Loc::Mem(_)))
            .map(|(at, _)| at)
            .collect();
        let [at] = memories[..] else { return None };
        let Loc::Mem(cell) = &what.sources[at] else { return None };
        if !_frame_cell(cell) || what.dests.iter().any(|dest| matches!(dest, Loc::Mem(_))) {
            return None;
        }
        facts
            .iter()
            .filter(|((other, held), stored)| {
                **stored && held == cell && other.width == cell.width && other.register.is_gpr()
            })
            .map(|((other, _), _)| *other)
            .min_by_key(|other| other.register as u32)
            .map(|from| (at, from))
    })();
    facts.retain(|pair, _| _lanes(pair.0.register).is_disjoint(&writes));
    if let Some(written) = written {
        if what.op == Operation::Move && what.sources.len() == 1 {
            if let Loc::Reg(source) = &what.sources[0] {
                if source.width == written.width && _lanes(source.register).is_disjoint(&writes) {
                    facts.insert((*source, written), true);
                }
            }
        }
    }
    (facts, false, held_source)
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
    let nodes: Vec<&LirBlock> = body.blocks.iter().collect();
    let solved = dataflow::solve(
        &nodes,
        Direction::Forward,
        |_| None::<Facts>,
        |at, outs| {
            if at == body.entry || predecessors[&at].is_empty() {
                return Some(Facts::default());
            }
            let mut met: Option<Facts> = None;
            for parent in &predecessors[&at] {
                let Some(Some(leaving)) = outs.get(parent) else {
                    continue;
                };
                met = Some(match met {
                    None => leaving.clone(),
                    Some(met) => met
                        .iter()
                        .filter(|(fact, _)| leaving.contains_key(*fact))
                        .map(|(fact, stored)| (fact.clone(), *stored && leaving[fact]))
                        .collect(),
                });
            }
            met
        },
        |at, facts| facts.as_ref().map(|facts| _transfer(body.bits, blocks[&at], facts.clone()).2),
    );
    solved.input.into_iter().map(|(at, facts)| (at, facts.unwrap_or_default())).collect()
}

/// The reloads `facts` makes redundant in `block`, and the facts after it.
fn _transfer(
    bits: u32,
    block: &LirBlock,
    facts: Facts,
) -> (Vec<usize>, IndexMap<usize, (usize, Reg)>, Facts) {
    let mut facts = facts;
    let mut redundant = Vec::new();
    let mut copies = IndexMap::default();
    for one in &block.insns {
        let (after, drop, copy) = _held(bits, one, facts);
        facts = after;
        if drop {
            redundant.push(id(one));
        }
        if let Some(from) = copy {
            copies.insert(id(one), from);
        }
    }
    (redundant, copies, facts)
}

pub fn forwarded(body: &LirBody) -> LirBody {
    let into = _available(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let (redundant, copies, _) = _transfer(body.bits, block, into[&block.at].clone());
        if redundant.is_empty() && copies.is_empty() {
            blocks.push(block.clone());
            continue;
        }
        let redundant: HashSet<usize> = redundant.into_iter().collect();
        let insns = block
            .insns
            .iter()
            .map(|one| {
                if redundant.contains(&id(one)) {
                    lir::anchor(Arc::clone(one))
                } else if let Some(from) = copies.get(&id(one)) {
                    // A read of a slot a register holds is a read of that
                    // register: `mov r, [slot]` is `mov r, from`.
                    let mut what = one.what.clone().expect("a reload is an instruction");
                    what.sources[from.0] = Loc::Reg(from.1);
                    Arc::new(Insn { what: Some(what), spill_reload: false, ..(**one).clone() })
                } else {
                    Arc::clone(one)
                }
            })
            .collect();
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}
