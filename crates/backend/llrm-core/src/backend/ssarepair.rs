//! SSA after a spiller gave some values more than one definition: a reload
//! brings a value back, and every use after it must read the reload, or a phi
//! where two definitions meet. The classic construction: phis at the
//! iterated dominance frontier of the definitions, pruned where the value is
//! not live in, then renaming down the dominator tree (Cytron et al.; Braun
//! et al. 2013 reach the same result on the fly).
//!
//! A definition is a reload when the instruction says so (`spill_reload` or
//! `rematerialized`) and defines one of `redefined`; the first definition of
//! each value keeps its name, and each reload is named afresh. A block that
//! holds the value in no register on entry (`held`) reloads it before every
//! use, so it needs no phi for it.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::analysis::loops;
use crate::backend::{allocate, spiller};
use crate::model::lir::{Insn, LirBlock, LirBody, Phi};
use crate::support::hash::IndexMap;

/// Whether `one` brings back a value `redefined` names.
fn reloads(
    one: &Insn,
    redefined: &BTreeSet<u32>,
) -> Option<u32> {
    match one.defines.as_slice() {
        [value] if (one.spill_reload || one.rematerialized) && redefined.contains(value) => Some(*value),
        _ => None,
    }
}

/// `body` in SSA again: every value in `redefined` has one definition per name.
pub fn repaired(
    body: &LirBody,
    redefined: &BTreeSet<u32>,
    held: &IndexMap<i64, BTreeSet<u32>>,
) -> LirBody {
    let (live_in, _) = allocate::live(body);
    let graph = &body.blocks;
    let by_at: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let idom = loops::immediate_dominators(&graph, Some(body.entry));
    let mut preds: IndexMap<i64, Vec<i64>> = IndexMap::default();
    for block in &body.blocks {
        for to in &block.succ {
            preds.entry(*to).or_default().push(block.at);
        }
    }
    // Dominance frontiers.
    let frontier = loops::frontiers(&graph, Some(body.entry));
    // Where each value is defined, and so where it needs a phi.
    let mut defined: IndexMap<u32, BTreeSet<i64>> = IndexMap::default();
    for block in &body.blocks {
        for phi in &block.phis {
            if redefined.contains(&phi.result) {
                defined.entry(phi.result).or_default().insert(block.at);
            }
        }
        for one in &block.insns {
            for value in &one.defines {
                if redefined.contains(value) {
                    defined.entry(*value).or_default().insert(block.at);
                }
            }
        }
    }
    let mut placed: IndexMap<i64, Vec<u32>> = IndexMap::default();
    for (value, blocks) in &defined {
        let mut work: Vec<i64> = blocks.iter().copied().collect();
        let mut has: BTreeSet<i64> = BTreeSet::new();
        while let Some(at) = work.pop() {
            for next in frontier.get(&at).into_iter().flatten() {
                if has.insert(*next) {
                    // A block that does not take the value into a register
                    // reloads it before every use: no phi.
                    let registered = held.get(next).is_none_or(|set| set.contains(value));
                    if registered
                        && live_in[next].contains(value)
                        && !by_at.get(next).is_some_and(|block| block.phis.iter().any(|phi| phi.result == *value))
                    {
                        placed.entry(*next).or_default().push(*value);
                    }
                    if !blocks.contains(next) {
                        work.push(*next);
                    }
                }
            }
        }
    }
    // Rename down the dominator tree.
    let mut children: IndexMap<i64, Vec<i64>> = IndexMap::default();
    for block in &body.blocks {
        if let Some(Some(parent)) = idom.get(&block.at) {
            children.entry(*parent).or_default().push(block.at);
        }
    }
    let mut next = spiller::_next_value(body);
    let mut stacks: IndexMap<u32, Vec<u32>> = IndexMap::default();
    let mut insns_of: IndexMap<i64, Vec<Arc<Insn>>> = IndexMap::default();
    let mut phis_of: IndexMap<i64, Vec<Phi>> = body.blocks.iter().map(|block| (block.at, block.phis.clone())).collect();
    // The result of the phi placed for a value in a block.
    let mut made: IndexMap<(i64, u32), u32> = IndexMap::default();
    // (block, value) whose phi still needs its operand from a predecessor, in
    // rename order. Each placed phi is named before any block is renamed: a
    // predecessor visited first hands it an operand.
    for (at, values) in &placed {
        for value in values {
            let name = next;
            next += 1;
            made.insert((*at, *value), name);
            phis_of.get_mut(at).expect("a block").push(Phi { result: name, incoming: Vec::new() });
        }
    }
    let mut todo: Vec<(i64, bool)> = vec![(body.entry, false)];
    let mut pushed: IndexMap<i64, Vec<u32>> = IndexMap::default();
    while let Some((at, leaving)) = todo.pop() {
        if leaving {
            for variable in pushed.shift_remove(&at).unwrap_or_default() {
                stacks.get_mut(&variable).expect("a stack").pop();
            }
            continue;
        }
        let block = by_at[&at];
        let mut mine: Vec<u32> = Vec::new();
        for phi in &block.phis {
            if redefined.contains(&phi.result) {
                stacks.entry(phi.result).or_default().push(phi.result);
                mine.push(phi.result);
            }
        }
        for value in placed.get(&at).into_iter().flatten() {
            stacks.entry(*value).or_default().push(made[&(at, *value)]);
            mine.push(*value);
        }
        let mut insns: Vec<Arc<Insn>> = Vec::new();
        for one in &block.insns {
            let mut rename: IndexMap<u32, u32> = IndexMap::default();
            for value in &one.uses {
                if redefined.contains(value) {
                    if let Some(top) = stacks.get(value).and_then(|stack| stack.last()) {
                        if top != value {
                            rename.insert(*value, *top);
                        }
                    }
                }
            }
            let mut one = if rename.is_empty() { Arc::clone(one) } else { spiller::_renamed(one, &rename) };
            if let Some(value) = reloads(&one, redefined) {
                let name = next;
                next += 1;
                one = spiller::_renamed(&one, &IndexMap::from_iter([(value, name)]));
                stacks.entry(value).or_default().push(name);
                mine.push(value);
            } else {
                for value in &one.defines {
                    if redefined.contains(value) {
                        stacks.entry(*value).or_default().push(*value);
                        mine.push(*value);
                    }
                }
            }
            insns.push(one);
        }
        insns_of.insert(at, insns);
        // The operands this block hands to the phis of its successors.
        for to in &block.succ {
            for value in placed.get(to).into_iter().flatten() {
                if let (Some(top), Some(name)) =
                    (stacks.get(value).and_then(|stack| stack.last()), made.get(&(*to, *value)))
                {
                    if let Some(phi) =
                        phis_of.get_mut(to).and_then(|phis| phis.iter_mut().find(|phi| phi.result == *name))
                    {
                        phi.incoming.push((at, *top));
                    }
                }
            }
            if let Some(phis) = phis_of.get_mut(to) {
                for phi in phis.iter_mut() {
                    if by_at[to].phis.iter().any(|own| own.result == phi.result) {
                        for (from, value) in &mut phi.incoming {
                            if *from == at && redefined.contains(value) {
                                if let Some(top) = stacks.get(value).and_then(|stack| stack.last()) {
                                    *value = *top;
                                }
                            }
                        }
                    }
                }
            }
        }
        pushed.insert(at, mine);
        todo.push((at, true));
        for child in children.get(&at).into_iter().flatten().rev() {
            todo.push((*child, false));
        }
    }
    let rebuilt = body
        .blocks
        .iter()
        .map(|block| {
            let insns = insns_of.get(&block.at).cloned().unwrap_or_else(|| block.insns.to_vec());
            let phis = phis_of.get(&block.at).cloned().unwrap_or_default();
            LirBlock { phis, ..block.with_insns(insns) }
        })
        .collect();
    body.with_blocks(rebuilt)
}

/// `body` without the phis that name one value: a phi whose arguments are all
/// one value `x` (or itself) is `x`, and every use of it reads `x`. Removing
/// one can make another trivial, so this runs to a fixed point.
pub fn simplified(body: &LirBody) -> Option<LirBody> {
    let mut found = false;
    let mut body = body.clone();
    loop {
        let mut rename: IndexMap<u32, u32> = IndexMap::default();
        for block in &body.blocks {
            for phi in block.phis.iter().filter(|phi| !body.pins.contains_key(&phi.result)) {
                let mut others = phi.incoming.iter().map(|(_, value)| *value).filter(|value| *value != phi.result);
                if let Some(only) = others.next().filter(|first| others.all(|value| value == *first)) {
                    rename.insert(phi.result, only);
                }
            }
        }
        if rename.is_empty() {
            return found.then_some(body);
        }
        found = true;
        // A chain of trivial phis reads through to its end.
        let ends: IndexMap<u32, u32> = rename
            .keys()
            .map(|value| {
                let mut at = rename[value];
                while let Some(next) = rename.get(&at).filter(|next| **next != *value) {
                    at = *next;
                }
                (*value, at)
            })
            .collect();
        let blocks = body
            .blocks
            .iter()
            .map(|block| {
                let insns = block
                    .insns
                    .iter()
                    .map(|one| {
                        if one.uses.iter().any(|value| ends.contains_key(value)) {
                            spiller::_renamed(one, &ends)
                        } else {
                            Arc::clone(one)
                        }
                    })
                    .collect();
                let phis = block
                    .phis
                    .iter()
                    .filter(|phi| !ends.contains_key(&phi.result))
                    .map(|phi| Phi {
                        result: phi.result,
                        incoming: phi
                            .incoming
                            .iter()
                            .map(|(from, value)| (*from, ends.get(value).copied().unwrap_or(*value)))
                            .collect(),
                    })
                    .collect();
                LirBlock { phis, ..block.with_insns(insns) }
            })
            .collect();
        body = body.with_blocks(blocks);
    }
}
