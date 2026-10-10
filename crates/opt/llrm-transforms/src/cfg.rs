//! Adapted from llrm-core's `optimize/cfg.rs`, the port of
//! `qbopt/optimize/cfg.py`: exact MIR control-flow cleanup.
//!
//! What the old `merged` checked only for the old representation is gone:
//! - Byte ownership: the erased jump's emptied operation, the dead
//!   ownership-only blocks moved along (`_empty` and transform's
//!   `_empty_operation`), and the forward-only walk that kept blocks placed by
//!   their source bytes. Layout is free here, so any chain merges and the
//!   merged block keeps the first block's place.
//! - Unrolling provenance (`repetitions`) and floating sequences
//!   (`floating_origin`): the rich MIR has neither.
//! - The substitution error: `replace_all_uses_with` cannot fail, and a swap
//!   cycle is still refused before anything changes.
//!
//! Tests that stay behind, each of the old byte ownership or the BC raise:
//! - test_cloned_chain_without_source_bytes_merges and
//!   test_transferred_byte_ownership_does_not_block_chain_merge: the chain
//!   differs from the first test only in owned bytes.
//! - test_unreachable_ownership_between_blocks_moves_without_losing_spans and
//!   test_empty_accepts_zero_stack_depth: `_empty` is not ported.
//! - test_merge_preserves_alternate_entries_and_layout's entry, repetition and
//!   intervening-block cases: the entry has no predecessors (the verifier), and
//!   there are no repetitions or source placement; its other predecessor case
//!   is ported.
//! - test_end_guards_have_no_return_edge_in_raised_control_flow,
//!   test_udtrng_bounds_compare_explicit_values,
//!   test_udtrng_guards_constrain_subsequent_reads_of_slot,
//!   test_only_established_terminal_contracts_remove_return_edges,
//!   test_bools_constant_program_is_one_live_block and
//!   test_localp_keeps_termination_after_interleaved_procedure: they read BC
//!   fixtures through the raise and the old pipeline, not `merged`.

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
