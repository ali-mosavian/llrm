//! Adapted from llrm-core's `optimize/loopclone.rs`, the port of
//! `qbopt/optimize/loopclone.py`: clone loop iterations as CFGs, retaining a
//! residual loop and every exit.
//!
//! This is the MIR building block for peeling and bounded full unrolling,
//! not a profitability decision. Loop live-outs must already be in LCSSA.
//! Python's `ValueError`s are the `Err` text.
//!
//! What changed with the IR: the old body was immutable, so `peeled`
//! returns a changed copy of the function for its callers (peel, unswitch)
//! to weigh. A floating operation is one with a floating operand or result.
//! A clone keeps its original's metadata, so `ssa::cloned_pointer_metadata`
//! and `ssa::cloned_integer_ranges` have nothing to copy; raw raised stack
//! effects (`op.stack`), which refused floating cloning, and the old
//! operation's absorbed, raised and symbol fields have no counterpart. A
//! block with several successors ends in `br`, `switch` or `invoke`; the
//! last is the old opaque dispatch.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::cfg;
use crate::graph::loops::{self, Loop};
use llrm_mir::context::Context;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_mir::types::Type;

use crate::edges;

/// Whether `inst` reads or makes a floating value.
fn floating(context: &Context, function: &Function, inst: InstId) -> bool {
    let instruction = function.instruction(inst);
    let is_float = |operand: Operand| function.operand_type(context, operand).is_some_and(|ty| matches!(context.types.get(ty), Type::Float(_)));
    instruction.result.is_some_and(|value| is_float(Operand::Value(value))) || instruction.operands.iter().any(|&operand| is_float(operand))
}

fn is_phi(function: &Function, inst: InstId) -> bool {
    function.instruction(inst).opcode == Opcode::Phi
}

/// A phi's `(value, block)` pairs.
fn incoming(function: &Function, phi: InstId) -> Vec<(Operand, i64)> {
    function
        .instruction(phi)
        .operands
        .chunks(2)
        .map(|pair| match pair[1] {
            Operand::Block(block) => (pair[0], cfg::id(block)),
            _ => unreachable!("a phi's block operand"),
        })
        .collect()
}

/// Whether conditional cloning cannot create a floating-value join.
///
/// If every floating value is defined and consumed in its own block,
/// lowering can allocate an independent floating region for either arm.
pub fn _block_local_floating(context: &Context, function: &Function, originals: &[BlockId]) -> bool {
    let inside = originals.iter().copied().collect::<BTreeSet<_>>();
    let mut owners: BTreeMap<ValueId, BlockId> = BTreeMap::new();
    for &block in originals {
        for &inst in function.block(block).instructions() {
            if is_phi(function, inst) || !floating(context, function, inst) {
                continue;
            }
            if let Some(value) = function.instruction(inst).result {
                owners.insert(value, block);
            }
        }
    }
    if owners.is_empty() {
        return true;
    }
    let mut users: BTreeMap<ValueId, BTreeSet<BlockId>> = owners.keys().map(|value| (*value, BTreeSet::new())).collect();
    for &block in function.layout() {
        for &inst in function.block(block).instructions() {
            for operand in &function.instruction(inst).operands {
                if let Operand::Value(value) = operand
                    && let Some(found) = users.get_mut(value)
                {
                    found.insert(block);
                }
            }
            if !inside.contains(&block) || is_phi(function, inst) || !floating(context, function, inst) {
                continue;
            }
            for &operand in &function.instruction(inst).operands {
                let Operand::Value(value) = operand else { continue };
                let float = function.operand_type(context, operand).is_some_and(|ty| matches!(context.types.get(ty), Type::Float(_)));
                if float && owners.get(&value) != Some(&block) {
                    return false;
                }
            }
        }
    }
    owners.iter().all(|(value, owner)| users[value] == BTreeSet::from([*owner]))
}

/// `function` with the first `count` iterations of `loop_` cloned ahead of
/// it, or `None` where the loop's shape refuses.
pub fn peeled(context: &Context, function: &Function, loop_: &Loop, count: i64) -> Result<Option<Function>, String> {
    if count < 1 {
        return Err("peeling needs a positive iteration count".to_owned());
    }
    let count = count as usize;
    let predecessors = loops::predecessors(&cfg::graph(function));
    if function.entry().map(cfg::id) == Some(loop_.header) || loop_.latches.len() != 1 {
        return Ok(None);
    }
    let outside = predecessors[&loop_.header].difference(&loop_.body).copied().collect::<Vec<_>>();
    if outside.len() != 1 {
        return Ok(None);
    }
    let entry = outside[0];
    let latch = *loop_.latches.iter().next().expect("one latch");
    let header = cfg::block(loop_.header);
    if function.successors(cfg::block(entry)) != [header] || function.successors(cfg::block(latch)) != [header] {
        return Ok(None);
    }
    if loop_.body.iter().any(|at| *at != loop_.header && !predecessors[at].is_subset(&loop_.body)) {
        return Ok(None);
    }
    let originals = function.layout().iter().copied().filter(|block| loop_.body.contains(&cfg::id(*block))).collect::<Vec<_>>();
    let instructions = |block: BlockId| function.block(block).instructions().iter().copied();
    // A straight-line floating region can be cloned and allocated as one
    // sequence.  An internal conditional needs a stronger proof: folding a
    // cloned selector may delete a different arm in every copy.
    if originals.iter().any(|&block| instructions(block).any(|inst| !is_phi(function, inst) && floating(context, function, inst)))
        && originals.iter().any(|&block| block != header && function.successors(block).len() > 1)
        && !_block_local_floating(context, function, &originals)
    {
        return Ok(None);
    }
    if originals.iter().any(|&block| {
        function.successors(block).len() > 1
            && !function.terminator(block).is_some_and(|last| matches!(function.instruction(last).opcode, Opcode::Br | Opcode::Switch))
    }) {
        return Ok(None);
    }
    let defined = originals.iter().flat_map(|&block| instructions(block)).filter_map(|inst| function.instruction(inst).result).collect::<BTreeSet<_>>();
    for &block in function.layout() {
        if loop_.body.contains(&cfg::id(block)) {
            continue;
        }
        for inst in instructions(block) {
            if is_phi(function, inst) {
                if incoming(function, inst)
                    .iter()
                    .any(|(value, source)| matches!(value, Operand::Value(one) if defined.contains(one)) && !loop_.body.contains(source))
                {
                    return Ok(None);
                }
            } else if function.instruction(inst).operands.iter().any(|operand| matches!(operand, Operand::Value(one) if defined.contains(one))) {
                return Ok(None);
            }
        }
    }

    let mut changed = function.clone();
    let mut labels: Vec<BTreeMap<i64, BlockId>> = Vec::new();
    let mut copies: Vec<BTreeMap<ValueId, ValueId>> = Vec::new();
    // Each iteration's clones, by original block: (original, clone).
    let mut clones: Vec<Vec<(BlockId, Vec<(InstId, InstId)>)>> = Vec::new();
    for _ in 0..count {
        let mut label = BTreeMap::new();
        let mut copy = BTreeMap::new();
        let mut made = Vec::new();
        for &block in &originals {
            let bridge = changed.create_block(None);
            changed.insert_block(bridge, None)?;
            label.insert(cfg::id(block), bridge);
            let mut pairs = Vec::new();
            for inst in instructions(block) {
                let clone = changed.clone_instruction(inst);
                if let (Some(from), Some(to)) = (function.instruction(inst).result, changed.instruction(clone).result) {
                    copy.insert(from, to);
                }
                pairs.push((inst, clone));
            }
            made.push((block, pairs));
        }
        labels.push(label);
        copies.push(copy);
        clones.push(made);
    }

    let value = |original: Operand, iteration: usize| match original {
        Operand::Value(one) => copies[iteration].get(&one).map_or(original, |&copy| Operand::Value(copy)),
        _ => original,
    };
    let destination = |at: i64, source: i64, iteration: usize| -> BlockId {
        if source == latch && at == loop_.header {
            return if iteration + 1 < count { labels[iteration + 1][&at] } else { cfg::block(at) };
        }
        labels[iteration].get(&at).copied().unwrap_or(cfg::block(at))
    };
    let from_latch = |phi: InstId| incoming(function, phi).into_iter().find(|(_, source)| *source == latch).expect("KeyError").0;
    let pairs = |pairs: Vec<(Operand, BlockId)>| pairs.into_iter().flat_map(|(value, block)| [value, Operand::Block(block)]).collect::<Vec<_>>();

    for (iteration, made) in clones.iter().enumerate() {
        for (block, instructions) in made {
            let at = cfg::id(*block);
            for &(inst, clone) in instructions {
                let operands = if is_phi(function, inst) {
                    if at == loop_.header {
                        if iteration == 0 {
                            let from = incoming(function, inst).into_iter().find(|(_, source)| *source == entry).expect("KeyError").0;
                            pairs(vec![(from, cfg::block(entry))])
                        } else {
                            pairs(vec![(value(from_latch(inst), iteration - 1), labels[iteration - 1][&latch])])
                        }
                    } else {
                        pairs(incoming(function, inst).into_iter().map(|(one, source)| (value(one, iteration), labels[iteration][&source])).collect())
                    }
                } else {
                    function
                        .instruction(inst)
                        .operands
                        .iter()
                        .map(|&operand| match operand {
                            Operand::Block(target) => Operand::Block(destination(cfg::id(target), at, iteration)),
                            _ => value(operand, iteration),
                        })
                        .collect()
                };
                changed.set_operands(clone, operands);
                changed.insert(clone, Position::End(labels[iteration][&at]))?;
            }
        }
    }

    let terminator = changed.terminator(cfg::block(entry)).expect("an entry into the loop");
    edges::retarget(&mut changed, terminator, header, labels[0][&loop_.header]);
    for &block in function.layout() {
        let at = cfg::id(block);
        for inst in instructions(block).filter(|&inst| is_phi(function, inst)) {
            let mut incoming = incoming(function, inst).into_iter().map(|(one, source)| (one, cfg::block(source))).collect::<Vec<_>>();
            if at == loop_.header {
                incoming.retain(|(_, source)| cfg::id(*source) != entry);
                incoming.push((value(from_latch(inst), count - 1), labels[count - 1][&latch]));
            } else if !loop_.body.contains(&at) {
                let exits = incoming.iter().filter(|(_, source)| loop_.body.contains(&cfg::id(*source))).copied().collect::<Vec<_>>();
                for (original, source) in exits {
                    for (iteration, label) in labels.iter().enumerate() {
                        incoming.push((value(original, iteration), label[&cfg::id(source)]));
                    }
                }
            } else {
                continue;
            }
            changed.set_operands(inst, pairs(incoming));
        }
    }
    Ok(Some(changed))
}

#[cfg(test)]
#[path = "loopclone_tests.rs"]
mod tests;
