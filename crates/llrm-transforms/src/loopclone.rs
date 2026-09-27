//! Adapted from llrm-core's `optimize/loopclone.rs`, the port of
//! `qbopt/optimize/loopclone.py`: clone loop iterations as CFGs, retaining a
//! residual loop and every exit.
//!
//! This is the MIR building block for peeling and bounded full unrolling,
//! not a profitability decision. Loop live-outs must already be in LCSSA.
//! Python's `ValueError`s are the `Err` text.
//!
//! Only `peeled` is here so far, for unswitch. The old body's clones were
//! found again by their fresh labels; here `peeled` returns each iteration's
//! block map. Not ported, meaning nothing here:
//! - `_block_local_floating`, whether cloning can join x87 stack values (a
//!   float is an ordinary value); its caller was `peeled`.
//! - Copying the old body's pointer and integer-range side tables and the
//!   `cloned` flag: the rewrite ledger records each `Cloned` instruction.
//!
//! Tests skipped: `test_peeling_refuses_floating_work_behind_an_internal_branch`
//! and `test_peeling_accepts_block_local_floating_values_behind_a_branch`
//! (x87 regions), `test_peeling_clones_pointer_identity_and_seed_facts` (side
//! tables), and the byte-ownership half of
//! `test_clones_read_their_own_values_and_do_not_duplicate_byte_ownership`.

use std::collections::BTreeMap;

use llrm_analysis::cfg;
use llrm_graph::loops::{self, Loop};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::Opcode;

use crate::edges;
use crate::lcssa::{arms, definitions, from_arms, operations};

/// `count` iterations of `loop_` cloned ahead of it, or `None`, changing
/// nothing, where the loop is not one this can clone. Each iteration's map
/// from an original block to its copy.
pub fn peeled(function: &mut Function, loop_: &Loop, count: i64) -> Result<Option<Vec<BTreeMap<i64, BlockId>>>, String> {
    if count < 1 {
        return Err("peeling needs a positive iteration count".to_owned());
    }
    let count = count as usize;
    let graph = cfg::graph(function);
    let blocks = graph.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
    let predecessors = loops::predecessors(&graph);
    if Some(loop_.header) == function.entry().map(cfg::id) || loop_.latches.len() != 1 {
        return Ok(None);
    }
    let outside = predecessors[&loop_.header].difference(&loop_.body).copied().collect::<Vec<_>>();
    if outside.len() != 1 {
        return Ok(None);
    }
    let entry = outside[0];
    let latch = *loop_.latches.first().expect("one latch");
    if blocks[&entry].succ != [loop_.header] || blocks[&latch].succ != [loop_.header] {
        return Ok(None);
    }
    if loop_.body.iter().any(|at| *at != loop_.header && !predecessors[at].is_subset(&loop_.body)) {
        return Ok(None);
    }
    let originals = graph.iter().filter(|block| loop_.body.contains(&block.at)).map(|block| block.at).collect::<Vec<_>>();
    if originals.iter().any(|&at| {
        blocks[&at].succ.len() > 1
            && function.terminator(cfg::block(at)).is_none_or(|last| !matches!(function.instruction(last).opcode, Opcode::Br | Opcode::Switch))
    }) {
        return Ok(None);
    }
    let defined = definitions(function, &loop_.body);
    for block in graph.iter().filter(|block| !loop_.body.contains(&block.at)) {
        let at = cfg::block(block.at);
        let reads = |inst: InstId| function.instruction(inst).operands.iter().any(|operand| matches!(operand, Operand::Value(value) if defined.contains_key(value)));
        if operations(function, at).into_iter().any(reads) {
            return Ok(None);
        }
        if edges::phis(function, at).into_iter().any(|phi| {
            arms(function, phi)
                .into_iter()
                .any(|(value, source)| matches!(value, Operand::Value(value) if defined.contains_key(&value)) && !loop_.body.contains(&cfg::id(source)))
        }) {
            return Ok(None);
        }
    }

    let mut labels: Vec<BTreeMap<i64, BlockId>> = Vec::new();
    let mut copies: Vec<BTreeMap<ValueId, ValueId>> = Vec::new();
    let mut cloned = Vec::new();
    for iteration in 0..count {
        let map = originals.iter().map(|&at| (at, function.create_block(None))).collect::<BTreeMap<_, _>>();
        let mut values = BTreeMap::new();
        for &at in &originals {
            function.insert_block(map[&at], None)?;
            for inst in function.block(cfg::block(at)).instructions().to_vec() {
                let copy = function.clone_instruction(inst);
                function.insert(copy, Position::End(map[&at]))?;
                if let (Some(original), Some(result)) = (function.instruction(inst).result, function.instruction(copy).result) {
                    values.insert(original, result);
                }
                cloned.push((iteration, at, inst, copy));
            }
        }
        labels.push(map);
        copies.push(values);
    }

    let value = |original: Operand, iteration: usize| match original {
        Operand::Value(one) => Operand::Value(copies[iteration].get(&one).copied().unwrap_or(one)),
        other => other,
    };
    let destination = |at: i64, source: i64, iteration: usize| {
        if source == latch && at == loop_.header {
            return if iteration + 1 < count { labels[iteration + 1][&at] } else { cfg::block(at) };
        }
        labels[iteration].get(&at).copied().unwrap_or(cfg::block(at))
    };
    let from = |function: &Function, phi: InstId, source: i64| {
        arms(function, phi).into_iter().find(|&(_, block)| cfg::id(block) == source).map(|(value, _)| value).ok_or("KeyError")
    };

    for &(iteration, at, inst, copy) in &cloned {
        let operands = if function.instruction(inst).opcode == Opcode::Phi {
            let incoming = if at == loop_.header {
                if iteration == 0 {
                    vec![(from(function, inst, entry)?, cfg::block(entry))]
                } else {
                    vec![(value(from(function, inst, latch)?, iteration - 1), labels[iteration - 1][&latch])]
                }
            } else {
                arms(function, inst).into_iter().map(|(incoming, source)| (value(incoming, iteration), labels[iteration][&cfg::id(source)])).collect()
            };
            from_arms(&incoming)
        } else {
            function
                .instruction(inst)
                .operands
                .iter()
                .map(|&operand| match operand {
                    Operand::Block(target) => Operand::Block(destination(cfg::id(target), at, iteration)),
                    other => value(other, iteration),
                })
                .collect()
        };
        function.set_operands(copy, operands);
    }

    let first = labels[0][&loop_.header];
    let jump = function.terminator(cfg::block(entry)).expect("a terminated block");
    edges::retarget(function, jump, cfg::block(loop_.header), first);
    for block in graph.iter().map(|block| block.at) {
        for phi in edges::phis(function, cfg::block(block)) {
            let mut incoming = arms(function, phi);
            if block == loop_.header {
                let residual = from(function, phi, latch)?;
                incoming.retain(|&(_, source)| cfg::id(source) != entry);
                incoming.push((value(residual, count - 1), labels[count - 1][&latch]));
            } else if !loop_.body.contains(&block) {
                for (original, source) in arms(function, phi) {
                    if loop_.body.contains(&cfg::id(source)) {
                        for (iteration, label) in labels.iter().enumerate() {
                            incoming.push((value(original, iteration), label[&cfg::id(source)]));
                        }
                    }
                }
            } else {
                continue;
            }
            function.set_operands(phi, from_arms(&incoming));
        }
    }
    Ok(Some(labels))
}

#[cfg(test)]
#[path = "loopclone_tests.rs"]
mod tests;
