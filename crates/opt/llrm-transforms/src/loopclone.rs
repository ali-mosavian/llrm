//! Adapted from llrm-core's `optimize/loopclone.rs`, the port of
//! `qbopt/optimize/loopclone.py`: clone loop iterations as CFGs, retaining a
//! residual loop and every exit.
//!
//! This is the MIR building block for peeling and bounded full unrolling,
//! not a profitability decision. Loop live-outs must already be in LCSSA.
//! Python's `ValueError`s are the `Err` text.
//!
//! The old body's clones were found again by their fresh labels; here
//! `peeled` returns each iteration's block map. Its callers were peel and
//! unswitch. Not ported, meaning nothing here:
//! - `_block_local_floating`, whether cloning can join x87 stack values (a
//!   float is an ordinary value); its caller was `peeled`.
//! - Copying the old body's pointer and integer-range side tables and the
//!   `cloned` flag: the rewrite ledger records each `Cloned` instruction.
//!
//! Tests skipped:
//! `test_peeling_refuses_floating_work_behind_an_internal_branch`
//! and `test_peeling_accepts_block_local_floating_values_behind_a_branch`
//! (x87 regions), and the byte-ownership half of
//! `test_clones_read_their_own_values_and_do_not_duplicate_byte_ownership`.

use std::collections::BTreeMap;

use llrm_analysis::cfg;
use llrm_analysis::graph::loops::{self, Loop};
use llrm_mir::dense::IdMap;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::Opcode;

use crate::edges;
use crate::lcssa::{arms, definitions, from_arms, operations};

/// `count` iterations of `loop_` cloned ahead of it, or `None`, changing
/// nothing, where the loop is not one this can clone. Each iteration's map
/// from an original block to its copy.
pub fn peeled(
    function: &mut Function,
    loop_: &Loop,
    count: i64,
) -> Result<Option<Vec<BTreeMap<i64, BlockId>>>, String> {
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
    let originals =
        graph.iter().filter(|block| loop_.body.contains(&block.at)).map(|block| block.at).collect::<Vec<_>>();
    if originals.iter().any(|&at| {
        blocks[&at].succ.len() > 1
            && function
                .terminator(cfg::block(at))
                .is_none_or(|last| !matches!(function.instruction(last).opcode, Opcode::Br | Opcode::Switch))
    }) {
        return Ok(None);
    }
    let defined = definitions(function, &loop_.body);
    for block in graph.iter().filter(|block| !loop_.body.contains(&block.at)) {
        let at = cfg::block(block.at);
        let reads = |inst: InstId| {
            function
                .instruction(inst)
                .operands
                .iter()
                .any(|operand| matches!(operand, Operand::Value(value) if defined.contains_key(value)))
        };
        if operations(function, at).into_iter().any(reads) {
            return Ok(None);
        }
        if edges::phis(function, at).into_iter().any(|phi| {
            arms(function, phi)
                .into_iter()
                .any(
                    |(value, source)| matches!(value, Operand::Value(value) if defined.contains_key(&value))
                        && !loop_.body.contains(&cfg::id(source)),
                )
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
                if let (Some(original), Some(result)) =
                    (function.instruction(inst).result, function.instruction(copy).result)
                {
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
        arms(function, phi)
            .into_iter()
            .find(|&(_, block)| cfg::id(block) == source)
            .map(|(value, _)| value)
            .ok_or("KeyError")
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
                arms(function, inst)
                    .into_iter()
                    .map(|(incoming, source)| (value(incoming, iteration), labels[iteration][&cfg::id(source)]))
                    .collect()
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

/// `loop_` with `copies` more bodies chained in it: the latch of each runs on
/// into the header's work of the next, whose test is not made, and the last
/// into the header. Right where the loop leaves only from its header and its
/// trip count is a multiple of `copies + 1`: the test of a later copy would
/// always continue. Each copy's map from an original block to its clone, the
/// first copy's first; `None`, changing nothing, where the loop is not one
/// this can chain. The loop's partial unrolling, as LLVM's `UnrollLoop` makes
/// it when no iteration is left over.
pub fn chained(
    function: &mut Function,
    loop_: &Loop,
    copies: usize,
) -> Result<Option<Vec<BTreeMap<i64, BlockId>>>, String> {
    if copies < 1 {
        return Err("chaining needs a positive number of copies".to_owned());
    }
    let graph = cfg::graph(function);
    let blocks = graph.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
    let predecessors = loops::predecessors(&graph);
    if Some(loop_.header) == function.entry().map(cfg::id) || loop_.latches.len() != 1 {
        return Ok(None);
    }
    let outside = predecessors[&loop_.header].difference(&loop_.body).copied().collect::<Vec<_>>();
    let [entry] = outside[..] else { return Ok(None) };
    let latch = *loop_.latches.first().expect("one latch");
    if blocks[&entry].succ != [loop_.header] || blocks[&latch].succ != [loop_.header] || latch == loop_.header {
        return Ok(None);
    }
    if loop_.body.iter().any(|at| *at != loop_.header && !predecessors[at].is_subset(&loop_.body)) {
        return Ok(None);
    }
    // The header alone leaves, by a conditional branch.
    let leaving = loop_.body.iter().filter(|at| blocks[at].succ.iter().any(|to| !loop_.body.contains(to))).count();
    let header_branch = function
        .terminator(cfg::block(loop_.header))
        .filter(
            |&last| {
                let op = function.instruction(last);
                op.opcode == Opcode::Br && op.operands.len() == 3
            },
        );
    let (1, Some(header_branch)) = (leaving, header_branch) else { return Ok(None) };
    if !blocks[&loop_.header].succ.iter().any(|to| loop_.body.contains(to)) {
        return Ok(None);
    }
    let originals =
        graph.iter().filter(|block| loop_.body.contains(&block.at)).map(|block| block.at).collect::<Vec<_>>();
    if originals.iter().any(|&at| {
        blocks[&at].succ.len() > 1
            && function
                .terminator(cfg::block(at))
                .is_none_or(|last| !matches!(function.instruction(last).opcode, Opcode::Br | Opcode::Switch))
    }) {
        return Ok(None);
    }
    let defined = definitions(function, &loop_.body);
    for block in graph.iter().filter(|block| !loop_.body.contains(&block.at)) {
        let at = cfg::block(block.at);
        let reads = |inst: InstId| {
            function
                .instruction(inst)
                .operands
                .iter()
                .any(|operand| matches!(operand, Operand::Value(value) if defined.contains_key(value)))
        };
        if operations(function, at).into_iter().any(reads) {
            return Ok(None);
        }
        if edges::phis(function, at).into_iter().any(|phi| {
            arms(function, phi)
                .into_iter()
                .any(
                    |(value, source)| matches!(value, Operand::Value(value) if defined.contains_key(&value))
                        && !loop_.body.contains(&cfg::id(source)),
                )
        }) {
            return Ok(None);
        }
    }
    let header_phis = edges::phis(function, cfg::block(loop_.header));
    let inside_target =
        blocks[&loop_.header].succ.iter().copied().find(|to| loop_.body.contains(to)).expect("a successor in the loop");

    let mut labels: Vec<BTreeMap<i64, BlockId>> = Vec::new();
    let mut copied: Vec<IdMap<ValueId, ValueId>> = Vec::new();
    let mut cloned = Vec::new();
    for copy in 0..copies {
        let map = originals.iter().map(|&at| (at, function.create_block(None))).collect::<BTreeMap<_, _>>();
        let mut values = IdMap::new();
        for &at in &originals {
            function.insert_block(map[&at], None)?;
            for inst in function.block(cfg::block(at)).instructions().to_vec() {
                // A later header's phis take the value the copy before flows
                // in; its branch is the way into the body.
                if at == loop_.header && function.instruction(inst).opcode == Opcode::Phi {
                    continue;
                }
                let made = function.clone_instruction(inst);
                function.insert(made, Position::End(map[&at]))?;
                if let (Some(original), Some(result)) =
                    (function.instruction(inst).result, function.instruction(made).result)
                {
                    values.insert(original, result);
                }
                cloned.push((copy, at, inst, made));
            }
        }
        labels.push(map);
        copied.push(values);
    }

    // What an original value is in copy `copy`: copy 0 is the original.
    let mut latch_values = Vec::new();
    for &phi in &header_phis {
        let arm = arms(function, phi)
            .into_iter()
            .find(|&(_, source)| cfg::id(source) == latch)
            .map(|(value, _)| value)
            .ok_or("KeyError")?;
        latch_values.push((function.instruction(phi).result.expect("a phi's value"), arm));
    }
    // `values[k]` reads copy k + 1 (k + 1 of 1..=copies); the header phis of
    // copy k + 1 are what copy k's latch supplied.
    let mut phi_values: Vec<IdMap<ValueId, Operand>> = vec![IdMap::new(); copies];
    let value = |phi_values: &[IdMap<ValueId, Operand>], original: Operand, copy: usize| match original {
        Operand::Value(one) => match phi_values[copy].get(&one) {
            Some(&supplied) => supplied,
            None => Operand::Value(copied[copy].get(&one).copied().unwrap_or(one)),
        },
        other => other,
    };
    // Copy c (0-based among the clones) is iteration c + 1: its header phis are
    // the latch's values of iteration c, itself a clone but for the first.
    for copy in 0..copies {
        for &(phi, arm) in &latch_values {
            let supplied = if copy == 0 { arm } else { value(&phi_values, arm, copy - 1) };
            phi_values[copy].insert(phi, supplied);
        }
    }
    let destination = |at: i64, source: i64, copy: usize| {
        if source == latch && at == loop_.header {
            return if copy + 1 < copies { labels[copy + 1][&at] } else { cfg::block(at) };
        }
        labels[copy].get(&at).copied().unwrap_or(cfg::block(at))
    };

    for &(copy, at, inst, made) in &cloned {
        let op = function.instruction(inst).clone();
        let operands = if op.opcode == Opcode::Phi {
            from_arms(
                &arms(function, inst)
                    .into_iter()
                    .map(|(incoming, source)| (value(&phi_values, incoming, copy), labels[copy][&cfg::id(source)]))
                    .collect::<Vec<_>>(),
            )
        } else if at == loop_.header && inst == header_branch {
            vec![Operand::Block(labels[copy][&inside_target])]
        } else {
            op.operands
                .iter()
                .map(|&operand| match operand {
                    Operand::Block(target) => Operand::Block(destination(cfg::id(target), at, copy)),
                    other => value(&phi_values, other, copy),
                })
                .collect()
        };
        function.set_operands(made, operands);
    }

    // The original latch runs on into the first copy; the header's phis take
    // what the last copy's latch supplies.
    let jump = function.terminator(cfg::block(latch)).expect("a terminated latch");
    edges::retarget(function, jump, cfg::block(loop_.header), labels[0][&loop_.header]);
    for &phi in &header_phis {
        let original = function.instruction(phi).result.expect("a phi's value");
        let supplied = value(&phi_values, latch_values.iter().find(|(one, _)| *one == original).unwrap().1, copies - 1);
        let mut incoming = arms(function, phi);
        incoming.retain(|&(_, source)| cfg::id(source) != latch);
        incoming.push((supplied, labels[copies - 1][&latch]));
        function.set_operands(phi, from_arms(&incoming));
    }
    Ok(Some(labels))
}

#[cfg(test)]
#[path = "loopclone_tests.rs"]
mod tests;
