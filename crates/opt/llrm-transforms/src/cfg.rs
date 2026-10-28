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
//! # Example
//! Before:
//! ```mir
//! define i16 @f(i16 %x) {
//! b0:
//!   %v = add i16 %x, 7
//!   br label %b1
//!
//! b1:
//!   %joined = phi i16 [ %v, %b0 ]
//!   ret i16 %joined
//! }
//! ```
//! After:
//! ```mir
//! define i16 @f(i16 %x) {
//! b0:
//!   %v = add i16 %x, 7
//!   ret i16 %v
//! }
//! ```
//! `b1` had `b0` as its only predecessor, so its phi became `%v` and its `ret`
//! joined `b0`.
//!
//! From unit test `test_single_entry_phi_is_replaced` in `cfg_tests.rs`.
//!
//! # Runs
//! A library function, called by `decide` after it decides branches and by
//! `rotate` and `lsr` after their rewrites. The pass `Merged` wraps
//! it; no pipeline stage runs it alone.
//!
//! # Keeps
//! Blocks change: every analysis is invalidated when anything merged.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::context::Context;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};

use crate::edges;
use crate::lcssa::arms;
use crate::local;

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
    let mut gone = llrm_support::hash::HashSet::default();
    let mut emptied = Vec::new();
    for first in function.layout().to_vec() {
        // Blocks go only as the target of a merge here, so the layout need not
        // be searched for `first` (N blocks, N^2: `switchlower` of 2048 cases).
        while !gone.contains(&first) {
            let Some(second) = emptied_into(function, first, entry) else { break };
            gone.insert(second);
            emptied.push(second);
            changed = true;
        }
    }
    // Together: the layout is searched once, not once per block merged.
    function.erase_blocks(&emptied).expect("emptied blocks nothing names");
    changed
}

/// Whether `block` was merged into its predecessor: LLVM's
/// `MergeBlockIntoPredecessor(BB)`, the one predecessor jumping only to it.
pub fn merged_into_predecessor(
    function: &mut Function,
    block: llrm_mir::module::BlockId,
) -> bool {
    let Some(entry) = function.entry() else { return false };
    let Some(first) = function.sole_predecessor(block) else { return false };
    merged_into(function, first, entry) == Some(block)
}

/// Whether `first` took the block it jumps to, if it alone reaches it.
pub(crate) fn merged_into(
    function: &mut Function,
    first: llrm_mir::module::BlockId,
    entry: llrm_mir::module::BlockId,
) -> Option<llrm_mir::module::BlockId> {
    let second = emptied_into(function, first, entry)?;
    function.erase_block(second).expect("an emptied block nothing names");
    Some(second)
}

/// `merged_into` that leaves the emptied block placed, for the caller to
/// `erase_blocks` together with the others.
pub(crate) fn emptied_into(
    function: &mut Function,
    first: llrm_mir::module::BlockId,
    entry: llrm_mir::module::BlockId,
) -> Option<llrm_mir::module::BlockId> {
    let target = function.sole_successor(first)?;
    if target == entry || target == first {
        return None;
    }
    // Only `first` names it: the first other terminator found ends the look,
    // so a join of N cases is not read through once for each of its arms.
    let named_by_another = function
        .block_users(target)
        .iter()
        .any(|one| function.instruction(one.user).opcode.is_terminator() && function.parent(one.user) != Some(first));
    if named_by_another {
        return None;
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
        return None;
    }

    // Only a jump: a two-way branch or any other terminator stays.
    let last = function.terminator(source).expect("a successor is a terminator's");
    if function.instruction(last).opcode != Opcode::Br || function.instruction(last).operands.len() != 1 {
        return None;
    }

    let swaps = phis
        .iter()
        .map(|&phi| (function.instruction(phi).result.expect("a phi's value"), arms(function, phi)[0].0))
        .collect::<BTreeMap<ValueId, Operand>>();
    if swaps.values().any(|value| matches!(value, Operand::Value(one) if swaps.contains_key(one))) {
        return None;
    }

    function.erase(last).expect("a jump defines nothing");
    for (value, with) in swaps {
        function.replace_value(value, with);
    }
    for phi in phis {
        function.erase(phi).expect("a replaced phi");
    }
    function.move_run(&function.iter_block(second).collect::<Vec<_>>(), source).expect("a placed block");
    function.replace_block_uses_with(second, source);
    Some(second)
}

/// `MergeBlockIntoPredecessor`
/// ([BasicBlockUtils.cpp:237](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L237))
/// without the analyses it updates: `bb` takes its one predecessor's place.
/// Where `with_two_successors` the predecessor ends in a conditional branch
/// and `bb` in a jump, and `bb`'s instructions are speculated before that
/// branch, which then goes where `bb` went. The emptied `bb` is left placed,
/// for the caller to `erase_blocks` with the others; whether it merged.
pub fn merge_block_into_predecessor(
    context: &Context,
    function: &mut Function,
    bb: BlockId,
    with_two_successors: bool,
) -> bool {
    // Can't merge if there are multiple predecessors, or no predecessors.
    let predecessors = function.predecessors(bb);
    let [pred] = predecessors[..] else { return false };
    // Don't break self-loops.
    if pred == bb {
        return false;
    }
    // Don't break unwinding instructions or terminators with other
    // side-effects.
    let Some(pti) = function.terminator(pred) else { return false };
    if matches!(
        function.instruction(pti).opcode,
        Opcode::Invoke(_) | Opcode::Resume
    ) {
        return false;
    }
    // Can't merge if there are multiple distinct successors.
    if !with_two_successors && function.sole_successor(pred) != Some(bb) {
        return false;
    }
    let mut fall_through = 0;
    let mut new_successor = None;
    if with_two_successors {
        let operands = &function.instruction(pti).operands;
        if function.instruction(pti).opcode != Opcode::Br || operands.len() != 3 {
            return false;
        }
        let Some(end) = function.terminator(bb) else { return false };
        let jump = function.instruction(end);
        let (Opcode::Br, [Operand::Block(to)]) = (&jump.opcode, &jump.operands[..]) else { return false };
        new_successor = Some(*to);
        fall_through = usize::from(operands[1] != Operand::Block(bb));
    }
    // Can't merge if there is a phi loop.
    for phi in edges::phis(function, bb) {
        let result = Operand::Value(function.instruction(phi).result.expect("a phi's value"));
        if function.instruction(phi).operands.iter().step_by(2).any(|&value| value == result) {
            return false;
        }
    }

    // Begin by getting rid of unneeded phis.
    local::fold_single_entry_phi_nodes(context, function, bb);

    // Move all definitions in the successor to the predecessor...
    let end = function.terminator(bb).expect("a terminated block");
    let moved = function.iter_block(bb).filter(|&one| one != end).collect::<Vec<_>>();
    for inst in moved {
        function.move_to(inst, Position::Before(pti)).expect("a placed predecessor");
    }

    // Make all phis that referred to bb now refer to pred as their source...
    function.replace_block_uses_with(bb, pred);

    if let Some(to) = new_successor {
        // Delete the unconditional branch from bb, and update the branch in
        // the predecessor.
        function.erase(end).expect("a jump defines nothing");
        function.set_operand(pti, 1 + fall_through, Operand::Block(to));
    } else {
        // Delete the unconditional branch from the predecessor, and move bb's
        // terminator there.
        function.erase(pti).expect("a jump defines nothing");
        function.move_to(end, Position::End(pred)).expect("a placed predecessor");
    }
    true
}

#[cfg(test)]
#[path = "cfg_tests.rs"]
mod tests;
