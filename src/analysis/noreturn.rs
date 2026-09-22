//! Direct port of `qbopt/analysis/noreturn.py`.

#![allow(dead_code)] // The cfront optimizer port is its first production caller.

use std::rc::Rc;
use std::collections::{BTreeMap, BTreeSet};

use crate::support::hash::IndexMap;

use crate::abi::runtime::{Contract, Control};
use crate::model::mir::{Kind, MirBlock, MirBody};
use crate::optimize::transform;

/// Bodies whose CFG cannot reach a normal return.
///
/// The result is the greatest fixed point over local direct calls.  Starting
/// at every local body permits a closed recursive SCC to prove terminal;
/// every member with a real return, malformed fallthrough, or path through a
/// nonterminal call is removed, and that removal propagates to its callers.
/// `terminal_calls` remains the independently established runtime fact.
pub(crate) fn inferred(
    bodies: &IndexMap<i64, MirBody>,
    local_calls: &IndexMap<i64, i64>,
    terminal_calls: &BTreeSet<i64>,
) -> BTreeSet<i64> {
    let mut proven = bodies.keys().copied().collect::<BTreeSet<_>>();
    loop {
        let mut terminals = terminal_calls.clone();
        terminals.extend(local_calls.iter().filter(|(_, target)| proven.contains(target)).map(|(at, _)| *at));
        let found = bodies
            .iter()
            .filter(|(_, body)| _cannot_return(body, &terminals))
            .map(|(entry, _)| *entry)
            .collect::<BTreeSet<_>>();
        if found == proven {
            return proven;
        }
        proven = found;
    }
}

/// Call sites whose established contract says control never comes back.
pub(crate) fn terminal_sites(contracts: &IndexMap<i64, Contract>) -> BTreeSet<i64> {
    contracts
        .iter()
        .filter(|(_, contract)| contract.established && contract.control == Control::Never)
        .map(|(at, _)| *at)
        .collect()
}

/// Blocks from which every path ends in a terminal call or a block the frontend marked cold.
///
/// A least fixed point, so a loop that never exits is not cold. When the
/// entry is cold the whole body stops, and no block is colder than another.
pub(crate) fn cold(body: &MirBody, terminal_calls: &BTreeSet<i64>) -> BTreeSet<i64> {
    let mut found = BTreeSet::new();
    let mut changed = true;
    while changed {
        changed = false;
        for block in &body.blocks {
            if !found.contains(&block.at) && (block.cold || _ends_cold(block, terminal_calls, &found)) {
                found.insert(block.at);
                changed = true;
            }
        }
    }
    if found.contains(&body.entry) {
        BTreeSet::new()
    } else {
        found
    }
}

fn _ends_cold(block: &MirBlock, terminal_calls: &BTreeSet<i64>, found: &BTreeSet<i64>) -> bool {
    for op in &block.ops {
        if op.kind == Kind::Return {
            return false;
        }
        if op.kind == Kind::Call && terminal_calls.contains(&op.at) {
            return true;
        }
    }
    !block.succ.is_empty() && block.succ.iter().all(|at| found.contains(at))
}

pub(crate) fn _cannot_return(body: &MirBody, terminal_calls: &BTreeSet<i64>) -> bool {
    let blocks = body.blocks.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
    let mut pending = vec![body.entry];
    let mut visited = BTreeSet::new();
    while let Some(at) = pending.pop() {
        if visited.contains(&at) {
            continue;
        }
        visited.insert(at);
        let Some(block) = blocks.get(&at) else {
            return false;
        };
        let mut stopped = false;
        for op in &block.ops {
            if op.kind == Kind::Return {
                return false;
            }
            if op.kind == Kind::Call && terminal_calls.contains(&op.at) {
                stopped = true;
                break;
            }
        }
        if !stopped {
            if block.succ.is_empty() {
                return false;
            }
            pending.extend(block.succ.iter().copied());
        }
    }
    true
}

/// Remove MIR work whose execution requires a proven terminal call.
///
/// The call remains, in program order, because it is the observable terminal
/// action.  Everything after its first occurrence in that block and the
/// block's outgoing CFG edges are unreachable.  Other predecessors may still
/// reach the former successors, so this local transform deliberately leaves
/// their blocks in place for ordinary CFG cleanup.  Already-truncated blocks
/// are returned unchanged, making it safe to use at the no-return fixed
/// point boundary.
pub(crate) fn after_terminal_calls(body: &Rc<MirBody>, terminal_calls: &BTreeSet<i64>) -> Rc<MirBody> {
    let mut blocks = Vec::new();
    let mut changed = false;
    for block in &body.blocks {
        let cut = block
            .ops
            .iter()
            .position(|op| op.kind == Kind::Call && terminal_calls.contains(&op.at));
        let Some(cut) = cut else {
            blocks.push(block.clone());
            continue;
        };
        let inert_tail = transform::_without(&block.ops[cut + 1..], |_op| true);
        let ops = block.ops[..=cut].iter().cloned().chain(inert_tail).collect::<Vec<_>>();
        if ops == block.ops && block.succ.is_empty() {
            blocks.push(block.clone());
            continue;
        }
        let mut made = block.clone();
        made.ops = ops;
        made.succ = Vec::new();
        blocks.push(made);
        changed = true;
    }
    if !changed {
        return body.clone();
    }

    // This runs after the object path's ordinary fixed point.  A block which
    // was reachable only through the just-removed edge must therefore be
    // normalized here, rather than left as executable work for lowering.  The
    // shared CFG normalizer retains its source-byte owner as inert MIR, which
    // is the required object-emission provenance contract.
    let mut made = MirBody::clone(body);
    made.blocks = blocks;
    Rc::new(transform::_unreachable(&made))
}

#[cfg(test)]
#[path = "noreturn_tests.rs"]
mod tests;
