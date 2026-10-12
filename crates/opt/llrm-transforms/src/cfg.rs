//! Control-flow cleanup: a block that jumps unconditionally to a block only it
//! reaches takes that block's instructions (LLVM's SimplifyCFG
//! `MergeBlockIntoPredecessor`).
//!
//! # How it works
//! `merged` visits the blocks in layout order and, for each block `first`,
//! merges its successor into it while it can. The successor `second` merges
//! when
//!
//! - `first` ends in an unconditional `br` and `second` is its only successor;
//! - `first` is `second`'s only predecessor, and `second` is neither the entry
//!   nor `first`;
//! - every phi of `second` has `first` as its only incoming block, and the
//!   values they take are not themselves phis of `second` (a swap cycle is
//!   refused before anything changes).
//!
//! Then the `br` is erased, each phi's value is replaced by what it received
//! from `first`, the phis go, the instructions of `second` move to the end of
//! `first`, uses of `second` as a block name `first`, and `second` is erased.
//! `first` is asked again, since its new terminator may reach another
//! single-entry block. A two-way branch or any other terminator is never
//! merged.
//!
//! ```text
//!   first:  ...             first:  ...
//!           br second   ==>         <instructions of second>
//!   second: %p = phi [%v, first]    <terminator of second>
//!           ...
//!           <terminator>
//! ```
//!
//! The merged block keeps `first`'s place in the layout.
//!
//! # Runs
//! A library function, called by `decide` after it decides branches and by
//! `phiopt`, `rotate` and `lsr` after their rewrites. The pass `Merged` wraps
//! it; no pipeline stage runs it alone.
//!
//! # Keeps
//! Blocks change: every analysis is invalidated when anything merged.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::module::{Function, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};

use crate::edges;
use crate::lcssa::arms;

pub struct Merged;

impl FunctionPass for Merged {
    fn name(&self) -> &'static str {
        "merged"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        _: &mut Analyses,
    ) -> PreservedAnalyses {
        if merged(unit.function) { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// Merge single-entry chains: a block jumping unconditionally to a block
/// it alone reaches takes that block's instructions, and the target's phis
/// become the values they receive from it. Whether anything merged.
pub fn merged(function: &mut Function) -> bool {
    let mut changed = false;
    let Some(entry) = function.entry() else {
        return false;
    };
    // Block by block, each asked of its own neighbours (the uses of the
    // target), not of a graph of the whole body made again after each merge. A
    // block that merged is asked again: its new successor may merge too.
    for first in function.layout().to_vec() {
        while function.layout().contains(&first) && merged_into(function, first, entry) {
            changed = true;
        }
    }
    changed
}

/// Whether `first` took the block it jumps to, if it alone reaches it.
fn merged_into(
    function: &mut Function,
    first: llrm_mir::module::BlockId,
    entry: llrm_mir::module::BlockId,
) -> bool {
    let successors = function.successors(first);
    if successors.len() != 1 {
        return false;
    }
    let target = successors[0];
    if target == entry || target == first {
        return false;
    }
    if function.predecessors(target) != [first] {
        return false;
    }
    let (source, second) = (first, target);
    let phis = edges::phis(function, second);
    if phis.iter().any(|&phi| {
        let incoming = arms(function, phi);
        incoming.iter().map(|&(_, at)| at).collect::<BTreeSet<_>>() != BTreeSet::from([source])
            || incoming
                .iter()
                .any(|&(value, _)| Operand::Value(function.instruction(phi).result.expect("a phi's value")) == value)
    }) {
        return false;
    }

    // Only a jump: a two-way branch or any other terminator stays.
    let last = function.terminator(source).expect("a successor is a terminator's");
    if function.instruction(last).opcode != Opcode::Br || function.instruction(last).operands.len() != 1 {
        return false;
    }

    let swaps = phis
        .iter()
        .map(|&phi| (function.instruction(phi).result.expect("a phi's value"), arms(function, phi)[0].0))
        .collect::<BTreeMap<ValueId, Operand>>();
    if swaps.values().any(|value| matches!(value, Operand::Value(one) if swaps.contains_key(one))) {
        return false;
    }

    function.erase(last).expect("a jump defines nothing");
    for (value, with) in swaps {
        function.replace_value(value, with);
    }
    for phi in phis {
        function.erase(phi).expect("a replaced phi");
    }
    function.move_run(&function.block(second).instructions().to_vec(), source).expect("a placed block");
    function.replace_block_uses_with(second, source);
    function.erase_block(second).expect("an emptied block nothing names");
    true
}

#[cfg(test)]
#[path = "cfg_tests.rs"]
mod tests;
