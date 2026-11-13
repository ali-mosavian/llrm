//! CodeGenPrepare's `eliminateMostlyEmptyBlocks`: a block of phis and a jump
//! is merged into its successor before isel
//! ([CodeGenPrepare.cpp#L939](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L939)).
//!
//! # How it works
//! Passes before isel (LSR, loop-simplify) split edges in ways that cost a
//! jump each; SimplifyCFG keeps those that hold a loop's simple form up
//! (`loop_form_blocks`), and CodeGenPrepare takes them out again just before
//! isel. For each block but the entry (`findDestBlockOfMergeableEmptyBlock`):
//! it ends in an unconditional jump to another block, holds nothing but phis
//! before it, and merges with that block without a phi conflict
//! (`canMergeBlocks`: a block both reach must give the successor's phis one
//! value). `isMergingEmptyBlockProfitable` keeps a loop's preheader whose
//! predecessor has other exits, and the target of a `callbr`. The merge is
//! `eliminateMostlyEmptyBlock`: the successor's phis take the block's
//! predecessors in its place, and the block goes.
//!
//! ```text
//!  b1: ...; br label %b2            b1: ...; br label %b3
//!  b2: br label %b3          ==>    b3: %p = phi [ %x, %b1 ], ...
//!  b3: %p = phi [ %x, %b2 ], ...
//! ```
//!
//! MIRROR: the dead-phi sweep, `findDestBlockOfMergeableEmptyBlock`,
//! `canMergeBlocks`, and the preheader and switch-frequency rules of
//! `isMergingEmptyBlockProfitable` (`FreqRatioToSkipMerge` 2, frequencies from
//! before the first merge, as the pin's `BFI`). No `indirectbr` in this IR.
//!
//! DEVIATE ECB2: the merge itself is `simplifycfg::empty_block_removed`
//! (`TryToSimplifyUncondBranchFromEmptyBlock`), which does the same to the
//! phis; it refuses a merge that would add more than 1000 phi entries and one
//! whose loop metadata the predecessors already carry, which
//! `eliminateMostlyEmptyBlock` does not. Exit: none.
//!
//! # Example
//! `test_an_empty_block_before_a_join_is_merged_into_the_join` in
//! `emptyblocks_tests.rs` is the sketch above.
//!
//! # Runs
//! First in `cgpaddr`'s run, before the sinks: it changes the CFG.

use llrm_analysis::blockfreq::BlockFrequencies;
use llrm_analysis::cfg;
use llrm_mir::context::Context;
use llrm_mir::dense::IdSet;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::Analyses;

use crate::edges;
use crate::simplifycfg::empty_block_removed;

/// The block `block` jumps to, if it is one
/// `findDestBlockOfMergeableEmptyBlock` merges: it ends in an unconditional
/// jump, holds only phis before it, does not jump to itself, and
/// `canMergeBlocks`.
fn destination(
    function: &Function,
    block: BlockId,
) -> Option<BlockId> {
    let last = function.terminator(block)?;
    let (Opcode::Br, [Operand::Block(dest)]) =
        (&function.instruction(last).opcode, &function.instruction(last).operands[..])
    else {
        return None;
    };
    let dest = *dest;
    // Nothing but phis before the jump.
    if function.iter_block(block).any(|inst| inst != last && function.instruction(inst).opcode != Opcode::Phi) {
        return None;
    }
    if dest == block || !can_merge(function, block, dest) {
        return None;
    }
    Some(dest)
}

/// `canMergeBlocks`.
fn can_merge(
    function: &Function,
    block: BlockId,
    dest: BlockId,
) -> bool {
    for phi in edges::phis(function, block) {
        let result = function.instruction(phi).result.expect("a phi's value");
        for one in function.users(result).iter() {
            let user = function.instruction(one.user);
            if function.parent(one.user) != Some(dest) || user.opcode != Opcode::Phi {
                return false;
            }
            for (value, from) in llrm_analysis::occurrence::arms(function, one.user) {
                if let Operand::Value(v) = value
                    && let llrm_mir::module::ValueDef::Instruction(def) = function.value(v).def
                    && function.parent(def) == Some(block)
                    && from != block
                {
                    return false;
                }
            }
        }
    }
    let dest_phis = edges::phis(function, dest);
    if dest_phis.is_empty() {
        return true;
    }
    // The blocks that reach `block`: its phis' blocks, else its predecessors.
    let block_preds: Vec<BlockId> = match edges::phis(function, block).first() {
        Some(&phi) => llrm_analysis::occurrence::arms(function, phi).into_iter().map(|(_, from)| from).collect(),
        None => function.predecessors(block).iter().copied().collect(),
    };
    let first = dest_phis[0];
    for (_, pred) in llrm_analysis::occurrence::arms(function, first) {
        if !block_preds.contains(&pred) {
            continue;
        }
        for &phi in &dest_phis {
            let arms = llrm_analysis::occurrence::arms(function, phi);
            let from = |at: BlockId| arms.iter().find(|&&(_, one)| one == at).map(|&(value, _)| value);
            let direct = from(pred);
            let mut through = from(block);
            // A value that is a phi of `block` is its value along that edge.
            if let Some(Operand::Value(v)) = through
                && let llrm_mir::module::ValueDef::Instruction(def) = function.value(v).def
                && function.instruction(def).opcode == Opcode::Phi
                && function.parent(def) == Some(block)
            {
                through = llrm_analysis::occurrence::arms(function, def)
                    .into_iter()
                    .find(|&(_, one)| one == pred)
                    .map(|(value, _)| value);
            }
            if direct != through {
                return false;
            }
        }
    }
    true
}

/// The predecessor whose `switch` the rule weighs `block` against: `block`'s
/// unique predecessor, if it ends in a `switch` and `dest` starts with a phi.
fn switch_predecessor(
    function: &Function,
    block: BlockId,
    dest: BlockId,
) -> Option<BlockId> {
    let pred = function.sole_predecessor(block)?;
    let ends_in_switch =
        function.terminator(pred).is_some_and(|last| function.instruction(last).opcode == Opcode::Switch);
    (ends_in_switch && !edges::phis(function, dest).is_empty()).then_some(pred)
}

/// `isMergingEmptyBlockProfitable`.
fn profitable(
    function: &Function,
    block: BlockId,
    dest: BlockId,
    is_preheader: bool,
    freqs: Option<&BlockFrequencies>,
) -> bool {
    if is_preheader {
        let single = function.sole_predecessor(block);
        if !single.is_some_and(|pred| function.successors(pred).len() == 1) {
            return false;
        }
    }
    let Some(pred) = switch_predecessor(function, block, dest) else { return true };
    let freqs = freqs.expect("frequencies are computed when a switch precedes an empty block");
    // The blocks that give the join's phis the values `block` gives them.
    let dest_phis = edges::phis(function, dest);
    let incoming = |phi: InstId, from: BlockId| {
        llrm_analysis::occurrence::arms(function, phi).into_iter().find(|&(_, one)| one == from).map(|(value, _)| value)
    };
    let mut same: Vec<BlockId> = Vec::new();
    for &one in function.predecessors(dest).iter() {
        if one != block
            && !same.contains(&one)
            && dest_phis.iter().all(|&phi| incoming(phi, block) == incoming(phi, one))
        {
            same.push(one);
        }
    }
    // The copies are in `pred` already.
    if same.contains(&pred) {
        return true;
    }
    let mut together = freqs.get_block_freq(block);
    for &twin in &same {
        if function.sole_predecessor(twin) == Some(pred) && destination(function, twin) == Some(dest) {
            together = together + freqs.get_block_freq(twin);
        }
    }
    // Skipping costs the copy and a branch in the block, merging a copy in
    // `pred`.
    together.mul_u64(FREQ_RATIO_TO_SKIP_MERGE).is_none_or(|limit| freqs.get_block_freq(pred) <= limit)
}

/// `FreqRatioToSkipMerge`.
const FREQ_RATIO_TO_SKIP_MERGE: u64 = 2;

/// The blocks that are some loop's preheader: the one block outside a loop
/// that jumps to its header and to nothing else (`getLoopPreheader`).
fn preheaders(function: &Function) -> IdSet<BlockId> {
    let graph = cfg::graph(function);
    let mut found = IdSet::new();
    let predecessors = llrm_support::graph::predecessors(&graph);
    for one in cfg::loops_of(function) {
        let outside: Vec<i64> = predecessors
            .get(&one.header)
            .map(|list| list.iter().copied().filter(|at| !one.body.contains(at)).collect())
            .unwrap_or_default();
        if let [parent] = outside[..]
            && let Some(entry) = graph.iter().find(|block| block.at == parent)
            && entry.succ.len() == 1
            && let Some(block) = function.layout().iter().copied().find(|&b| cfg::id(b) == parent)
        {
            found.insert(block);
        }
    }
    found
}

/// Every phi nothing reads, in the blocks after the first, goes: the
/// `DeleteDeadPHIs` sweep that `eliminateMostlyEmptyBlocks` starts with.
fn dead_phis_deleted(function: &mut Function) -> bool {
    let mut changed = false;
    let blocks: Vec<BlockId> = function.layout().iter().copied().skip(1).collect();
    for block in blocks {
        loop {
            let dead: Vec<InstId> = edges::phis(function, block)
                .into_iter()
                .filter(|&phi| function.instruction(phi).result.is_some_and(|result| function.users(result).is_empty()))
                .collect();
            if dead.is_empty() {
                break;
            }
            for phi in dead {
                function.set_operands(phi, Vec::new());
                function.erase(phi).expect("a phi nothing reads goes");
                changed = true;
            }
        }
    }
    changed
}

/// `eliminateMostlyEmptyBlocks`: whether any block was merged (or any dead phi
/// went).
pub fn eliminated(
    context: &Context,
    layout: &llrm_mir::datalayout::DataLayout,
    analyses: &mut Analyses,
    function: &mut Function,
) -> bool {
    let mut changed = dead_phis_deleted(function);
    let preheaders = preheaders(function);
    let blocks: Vec<BlockId> = function.layout().iter().copied().skip(1).collect();
    // The pin's `BFI` is computed once, before the first merge.
    let weighs = blocks.iter().any(|&block| {
        destination(function, block).is_some_and(|dest| switch_predecessor(function, block, dest).is_some())
    });
    let freqs = weighs.then(|| analyses.get::<BlockFrequencies>(context, layout, function));
    for block in blocks {
        if !function.is_placed(block) {
            continue;
        }
        let Some(dest) = destination(function, block) else { continue };
        if !profitable(function, block, dest, preheaders.contains(&block), freqs.as_deref()) {
            continue;
        }
        changed |= empty_block_removed(context, function, block, dest);
    }
    changed
}

#[cfg(test)]
#[path = "emptyblocks_tests.rs"]
mod tests;
