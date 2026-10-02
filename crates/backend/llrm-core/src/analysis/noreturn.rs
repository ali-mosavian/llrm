//! Direct port of `qbopt/analysis/noreturn.py`.

use std::collections::BTreeSet;

use crate::support::hash::IndexMap;

use crate::abi::runtime::{Contract, Control};
use crate::model::mir::{Kind, MirBlock, MirBody};

/// Call sites whose established contract says control never comes back.
pub fn terminal_sites(contracts: &IndexMap<i64, Contract>) -> BTreeSet<i64> {
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
pub fn cold(body: &MirBody, terminal_calls: &BTreeSet<i64>) -> BTreeSet<i64> {
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

/// Blocks from which no path reaches a return or `header`: a loop that
/// leaves into one stops the program rather than going on after the loop.
pub fn stranded(body: &MirBody, header: i64) -> BTreeSet<i64> {
    let predecessors = crate::analysis::loops::predecessors(&body.blocks);
    let mut returning = BTreeSet::new();
    let mut pending = body
        .blocks
        .iter()
        .filter(|block| block.ops.iter().any(|op| op.kind == Kind::Return))
        .map(|block| block.at)
        .chain([header])
        .collect::<Vec<_>>();
    while let Some(at) = pending.pop() {
        if returning.insert(at) {
            pending.extend(predecessors.get(&at).into_iter().flatten().copied());
        }
    }
    body.blocks.iter().map(|block| block.at).filter(|at| !returning.contains(at)).collect()
}
