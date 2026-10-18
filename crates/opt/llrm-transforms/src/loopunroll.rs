//! Complete unrolling of a loop over any exiting block: LLVM's `UnrollLoop`
//! (`LoopUnroll.cpp`) for the count that is the loop's maximum trip count.
//!
//! # How it works
//! The caller says how many times the loop's blocks run (`Options::count`,
//! the maximum trip count) and, for each exiting block that ends in a
//! conditional `br`, how many times that block runs when it is the one that
//! leaves (`Exit::trip_count`, ScalarEvolution's `getSmallConstantTripCount`;
//! 0 where unknown). The loop needs one preheader, one latch that is either
//! unconditional or an exiting `br`, and closed SSA form (`lcssa::closed`):
//! a value defined in the loop is read outside it only by a phi of an exit.
//!
//! Iteration 0 is the original blocks. Each later iteration is a copy of every
//! block of the loop, made in reverse postorder so that the value an exit
//! phi takes from a block is already copied: the header's phis are not copied
//! but read, in iteration `n`, what the latch gave them in iteration `n - 1`;
//! every other operand is renamed to its copy. Each exit's phis gain an arm
//! from each copy of the exiting block. Then the original header's phis are
//! replaced by their preheader values, each latch jumps to the next
//! iteration's header, and a branch whose way an iteration knows becomes a
//! jump: the last iteration's latch leaves, an iteration before the exiting
//! block's `trip_count` stays in. An exiting block that is not the latch is
//! not folded where it leaves, so every block stays reachable. When the latch
//! is unconditional the last copy's jump back is `unreachable`. Finally a
//! block a latch jumps to, if only that latch reaches it, is merged into it.
//!
//! The loop may exit from any of its blocks and may hold loops of its own. A
//! loop that is the loop of a rotated function (test in the latch) and one
//! that is not (test in the header) are both this function's input: neither
//! shape is a guard.
//!
//! # Deviations
//! - D2 No partial, runtime or epilog unrolling: `options.count` must be the
//!   maximum trip count (`unroll_loop` refuses otherwise). The pinned `opt`
//!   leaves `UP.Partial` and `UP.Runtime` off at `-mcpu=i486` and `pentiumpro`
//!   ([the plan](https://github.com/ali-mosavian/llrm/blob/main/docs/architecture/loop-passes-port.md)).
//! - D8 No `LoopInfo`, `DominatorTree`, `ScalarEvolution` or `AssumptionCache`
//!   to update: llrm derives its loop shape and dominators from the function
//!   each time they are asked, so none is kept up to date here, and nothing
//!   holds one across this call.
//! - D9 `simplifyLoopAfterUnroll` (`simplifyInstruction`, dead code) is the
//!   `fold` and `dead` that follow in the pipeline's round, until the
//!   `InstructionSimplify` translation has an entry that takes blocks. LCSSA is
//!   closed over the function (`lcssa::closed`) after the unroll, where LLVM
//!   closes the outer loop only when `needToInsertPhisForLCSSA`. `simplifyLoop`
//!   of the outer loop is the next `LoopSimplify`.
//!
//! # Runs
//! Not a pass: a library, called by the full-unroll pass when it is wired in.
//!
//! # Needs
//! `lcssa`, `cfg`, `edges`.
//!
//! # See also
//! [`UnrollLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUnroll.cpp#L929).

use std::collections::BTreeSet;

use llrm_analysis::cfg;
use llrm_analysis::graph::loops::Loop;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_support::hash::HashMap;

use crate::edges;
use crate::lcssa::{self, arms, from_arms};

/// How many times an exiting block runs when it is the one that leaves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Exit {
    pub block: BlockId,
    /// `getSmallConstantTripCount(L, block)`; 0 where unknown.
    pub trip_count: u32,
}

/// `UnrollLoopOptions`, for a complete unroll.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Options {
    /// Copies of the loop: its maximum trip count.
    pub count: u32,
    /// `getSmallConstantMaxTripCount(L)`; 0 where unknown.
    pub max_trip_count: u32,
    /// `isBackedgeTakenCountMaxOrZero(L)`.
    pub max_or_zero: bool,
    pub exits: Vec<Exit>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Unrolled {
    /// `LoopUnrollResult::Unmodified`.
    Unmodified,
    /// `LoopUnrollResult::FullyUnrolled`.
    Fully,
}

struct ExitInfo {
    block: BlockId,
    trip_count: u32,
    exit_on_true: bool,
    /// The block in each iteration, the original first.
    copies: Vec<BlockId>,
}

/// `loop_` copied `options.count` times in place, its branches folded.
/// `Unmodified`, changing nothing, where it is a loop `UnrollLoop` refuses.
pub fn unroll_loop(
    function: &mut Function,
    loop_: &Loop,
    options: &Options,
) -> Result<Unrolled, String> {
    if options.count == 0 || options.max_trip_count == 0 || options.count != options.max_trip_count {
        return Err("a complete unroll's count is the maximum trip count (D2)".to_owned());
    }
    let header = cfg::block(loop_.header);
    let inside = |at: BlockId| loop_.body.contains(&cfg::id(at));
    let outside_predecessors = function.predecessors(header).into_iter().filter(|&at| !inside(at)).collect::<Vec<_>>();
    let [preheader] = outside_predecessors[..] else { return Ok(Unrolled::Unmodified) };
    if function.successors(preheader).len() != 1 {
        return Ok(Unrolled::Unmodified);
    }
    let [latch] = loop_.latches.iter().copied().collect::<Vec<_>>()[..] else { return Ok(Unrolled::Unmodified) };
    let latch = cfg::block(latch);

    let original = loop_.body.iter().map(|&at| cfg::block(at)).collect::<Vec<_>>();
    let exit_blocks = _exit_blocks(function, &original, &inside);
    let count = options.count as usize;

    let mut infos = Vec::<ExitInfo>::new();
    for &block in &original {
        if !function.successors(block).iter().any(|&to| !inside(to)) {
            continue;
        }
        let Some(branch) = _conditional(function, block) else { continue };
        let Operand::Block(first) = function.instruction(branch).operands[1] else {
            return Err("a branch's target is a block".to_owned());
        };
        let trip_count = options.exits.iter().find(|one| one.block == block).map_or(0, |one| one.trip_count);
        infos.push(ExitInfo { block, trip_count, exit_on_true: !inside(first), copies: vec![block] });
    }

    let latch_is_exiting = function.successors(latch).iter().any(|&to| !inside(to));
    let latch_term = function.terminator(latch).ok_or("a latch without a terminator")?;
    let uncond_latch =
        function.instruction(latch_term).opcode == Opcode::Br && function.instruction(latch_term).operands.len() == 1;
    if !uncond_latch && !(_conditional(function, latch).is_some() && latch_is_exiting) {
        return Ok(Unrolled::Unmodified);
    }
    if !_closed(function, &original, &exit_blocks, &inside) {
        return Err("the loop is not in closed SSA form".to_owned());
    }

    let header_phis = edges::phis(function, header);
    let order = _reverse_postorder(function, header, &inside);
    let mut headers = vec![header];
    let mut latches = vec![latch];
    let mut last_value = HashMap::<ValueId, Operand>::default();
    let mut last_block = HashMap::<BlockId, BlockId>::default();
    let mut anchor = latch;

    for it in 1..count {
        let mut fresh = Vec::<BlockId>::new();
        for &at in &order {
            let new = function.create_block(None);
            function.insert_block(new, Some(anchor))?;
            anchor = new;
            let mut map = Vec::<(ValueId, Operand)>::new();
            if at == header {
                // The header's phis take what the latch gave them last
                // iteration.
                let incoming = header_phis
                    .iter()
                    .map(|&phi| {
                        let from = arms(function, phi)
                            .into_iter()
                            .find(|&(_, source)| source == latch)
                            .map(|(value, _)| value)
                            .ok_or("a header phi without an arm from the latch")?;
                        let carried = match from {
                            Operand::Value(one) if it > 1 && last_value.contains_key(&one) => last_value[&one],
                            other => other,
                        };
                        Ok((function.instruction(phi).result.expect("a phi's value"), carried))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                map.extend(incoming);
            }
            for inst in function.block(at).instructions().to_vec() {
                if at == header && function.instruction(inst).opcode == Opcode::Phi {
                    continue;
                }
                let copy = function.clone_instruction(inst);
                function.insert(copy, Position::End(new))?;
                if let (Some(was), Some(now)) = (function.instruction(inst).result, function.instruction(copy).result) {
                    map.push((was, Operand::Value(now)));
                }
            }
            last_block.insert(at, new);
            for (was, now) in map {
                last_value.insert(was, now);
            }
            // The exits take what this copy computed.
            for to in function.successors(at) {
                if inside(to) {
                    continue;
                }
                for phi in edges::phis(function, to) {
                    let mut incoming = arms(function, phi);
                    let Some(&(value, _)) = incoming.iter().find(|&&(_, source)| source == at) else { continue };
                    let value = match value {
                        Operand::Value(one) => last_value.get(&one).copied().unwrap_or(value),
                        other => other,
                    };
                    incoming.push((value, new));
                    function.set_operands(phi, from_arms(&incoming));
                }
            }
            if at == header {
                headers.push(new);
            }
            if at == latch {
                latches.push(new);
            }
            if let Some(info) = infos.iter_mut().find(|one| one.block == at) {
                info.copies.push(new);
            }
            fresh.push(new);
        }
        for &new in &fresh {
            for inst in function.block(new).instructions().to_vec() {
                let operands = function
                    .instruction(inst)
                    .operands
                    .iter()
                    .map(|&operand| match operand {
                        Operand::Value(one) => last_value.get(&one).copied().unwrap_or(operand),
                        Operand::Block(one) => Operand::Block(last_block.get(&one).copied().unwrap_or(one)),
                        other => other,
                    })
                    .collect();
                function.set_operands(inst, operands);
            }
        }
        debug_assert_eq!(headers.len(), it + 1);
    }

    // The original header's phis are the preheader's values.
    for &phi in &header_phis {
        let value = function.instruction(phi).result.expect("a phi's value");
        let entry = arms(function, phi)
            .into_iter()
            .find(|&(_, source)| source == preheader)
            .map(|(value, _)| value)
            .ok_or("a header phi without an arm from the preheader")?;
        function.set_operands(phi, Vec::new());
        function.replace_value(value, entry);
        function.erase(phi)?;
    }

    // Each latch goes to the next iteration's header; the last one's back edge
    // stays.
    for index in 0..latches.len() {
        let next = headers[(index + 1) % headers.len()];
        let jump = function.terminator(latches[index]).expect("a terminated latch");
        edges::retarget(function, jump, headers[index], next);
    }

    // Branches whose way the iteration knows.
    let preserve_only_first = options.max_or_zero;
    for info in &infos {
        let is_latch = info.block == latch;
        let size = info.copies.len();
        for (index, &source) in info.copies.iter().enumerate() {
            let next = (index + 1) % size;
            let known = if preserve_only_first {
                (index != 0).then_some(next == 0)
            } else if next == 0 {
                Some(true)
            } else if info.trip_count != 0 && next as u32 != info.trip_count {
                Some(false)
            } else {
                None
            };
            let Some(leaves) = known else { continue };
            if leaves && !is_latch {
                continue;
            }
            _decided(function, source, leaves, info.exit_on_true)?;
        }
    }

    // The last copy's jump back to the header can no longer be taken.
    if !latch_is_exiting {
        let last = *latches.last().expect("a latch per iteration");
        _unreachable(function, last)?;
    }

    // A block only the latch before it reaches is part of it.
    let entry = function.entry().expect("a function with a body");
    for index in 0..latches.len() {
        let at = latches[index];
        let Some(jump) = function.terminator(at) else { continue };
        if function.instruction(jump).opcode == Opcode::Br && function.instruction(jump).operands.len() == 1 {
            if let Some(gone) = crate::cfg::merged_into(function, at, entry) {
                for one in latches.iter_mut().filter(|one| **one == gone) {
                    *one = at;
                }
            }
        }
    }

    lcssa::closed(function)?;
    Ok(Unrolled::Fully)
}

/// The blocks outside `blocks` that `blocks` jump to.
fn _exit_blocks(
    function: &Function,
    blocks: &[BlockId],
    inside: &impl Fn(BlockId) -> bool,
) -> BTreeSet<BlockId> {
    blocks.iter().flat_map(|&at| function.successors(at)).filter(|&to| !inside(to)).collect()
}

/// `block` ends in a two-way `br i1`.
fn _conditional(
    function: &Function,
    block: BlockId,
) -> Option<InstId> {
    function
        .terminator(block)
        .filter(
            |&one| {
                let instruction = function.instruction(one);
                instruction.opcode == Opcode::Br && instruction.operands.len() == 3
            },
        )
}

/// Loop-closed: only a phi of an exit reads a value defined outside its block.
fn _closed(
    function: &Function,
    blocks: &[BlockId],
    exits: &BTreeSet<BlockId>,
    inside: &impl Fn(BlockId) -> bool,
) -> bool {
    blocks.iter().all(|&at| {
        function.block(at).instructions().iter().filter_map(|&inst| function.instruction(inst).result).all(|value| {
            function.users(value).iter().all(|one| {
                function.parent(one.user).is_some_and(|block| {
                    inside(block) || (exits.contains(&block) && function.instruction(one.user).opcode == Opcode::Phi)
                })
            })
        })
    })
}

/// The loop's blocks from `header`, in reverse postorder (`LoopBlocksDFS`).
fn _reverse_postorder(
    function: &Function,
    header: BlockId,
    inside: &impl Fn(BlockId) -> bool,
) -> Vec<BlockId> {
    let mut seen = BTreeSet::from([header]);
    let mut post = Vec::new();
    let mut stack = vec![(header, function.successors(header).into_iter().collect::<Vec<_>>())];
    while let Some((at, pending)) = stack.last_mut() {
        if let Some(next) = pending.pop() {
            if inside(next) && seen.insert(next) {
                let successors = function.successors(next).into_iter().collect::<Vec<_>>();
                stack.push((next, successors));
            }
        } else {
            post.push(*at);
            stack.pop();
        }
    }
    post.reverse();
    post
}

/// `source`'s conditional branch made the jump it takes where the exit is
/// taken (`leaves`) or not; the other target loses `source`'s phi arms.
fn _decided(
    function: &mut Function,
    source: BlockId,
    leaves: bool,
    exit_on_true: bool,
) -> Result<(), String> {
    let branch = _conditional(function, source).ok_or("a folded exit is a conditional branch")?;
    let operands = function.instruction(branch).operands.clone();
    let index = usize::from(exit_on_true ^ leaves);
    let (Operand::Block(to), Operand::Block(dead)) = (operands[1 + index], operands[2 - index]) else {
        return Err("a branch's targets are blocks".to_owned());
    };
    if to != dead {
        _without_arms_from(function, dead, source);
    }
    let void = function.instruction(branch).ty;
    function.erase(branch)?;
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(to)], Flags::default(), None);
    function.insert(jump, Position::End(source))
}

/// `block`'s phis without their arms from `source`.
fn _without_arms_from(
    function: &mut Function,
    block: BlockId,
    source: BlockId,
) {
    for phi in edges::phis(function, block) {
        let kept = arms(function, phi).into_iter().filter(|&(_, from)| from != source).collect::<Vec<_>>();
        function.set_operands(phi, from_arms(&kept));
    }
}

/// `block`'s terminator made `unreachable`, its successors forgetting it.
fn _unreachable(
    function: &mut Function,
    block: BlockId,
) -> Result<(), String> {
    for to in function.successors(block) {
        _without_arms_from(function, to, block);
    }
    let jump = function.terminator(block).ok_or("a block without a terminator")?;
    let void = function.instruction(jump).ty;
    function.erase(jump)?;
    let stop = function.create_instruction(Opcode::Unreachable, void, Vec::new(), Flags::default(), None);
    function.insert(stop, Position::End(block))
}

#[cfg(test)]
#[path = "loopunroll_tests.rs"]
mod tests;
