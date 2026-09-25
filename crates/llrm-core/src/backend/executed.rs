//! A finished body's executed work, estimated without a profile.
//!
//! `tools/quality.py`'s `_transitions` and `_frequencies`: a loop branch continues with
//! its proved trip count, or nine times in ten; other branches split evenly. Each block's
//! expected executions per call weigh its instructions and memory operands, so spill code
//! the MIR estimate cannot see is counted.

use crate::analysis::loops;
use crate::model::ir::Loc;
use crate::model::lir::LirBody;
use crate::support::hash::IndexMap;

/// Expected per call: instructions executed, and memory operands they touch.
pub struct Executed {
    pub instructions: f64,
    pub memory: f64,
}

/// `executed` as one line, for the `cost` channel and dump.
pub fn summary(body: &LirBody) -> String {
    match executed(body) {
        Some(done) => format!("{} executes {:.0} instructions, {:.0} memory operands", body.name, done.instructions, done.memory),
        None => format!("{} executes an unbounded amount", body.name),
    }
}

/// `None` for control flow with no finite profile-free estimate.
pub fn executed(body: &LirBody) -> Option<Executed> {
    let graph = crate::analysis::intervals::_graph(&body.blocks);
    if !loops::irreducible(&graph, Some(body.entry)).is_empty() {
        return None;
    }
    let natural = loops::loops(&graph, Some(body.entry));
    let trips: IndexMap<i64, i64> = body.loop_trip_counts.iter().copied().collect();
    let position: IndexMap<i64, usize> = body.blocks.iter().enumerate().map(|(at, block)| (block.at, at)).collect();
    let size = body.blocks.len();
    // f = entry + P^T f, solved directly.
    let mut matrix = vec![vec![0.0f64; size]; size];
    for (at, row) in matrix.iter_mut().enumerate() {
        row[at] = 1.0;
    }
    let mut right = vec![0.0f64; size];
    right[*position.get(&body.entry)?] = 1.0;
    for block in &body.blocks {
        let successors: Vec<i64> = block.succ.iter().copied().filter(|at| position.contains_key(at)).collect();
        if successors.is_empty() {
            continue;
        }
        let mut split: Option<Vec<(i64, f64)>> = None;
        for one in &natural {
            if !one.body.contains(&block.at) {
                continue;
            }
            let inside: Vec<i64> = successors.iter().copied().filter(|at| one.body.contains(at)).collect();
            let outside: Vec<i64> = successors.iter().copied().filter(|at| !one.body.contains(at)).collect();
            if !inside.is_empty() && !outside.is_empty() {
                let count = if block.at == one.header || one.latches.contains(&block.at) { trips.get(&one.header) } else { None };
                let stay = count.map_or(0.9, |count| (*count as f64 - 1.0) / *count as f64);
                let mut parts: Vec<(i64, f64)> = inside.iter().map(|at| (*at, stay / inside.len() as f64)).collect();
                parts.extend(outside.iter().map(|at| (*at, (1.0 - stay) / outside.len() as f64)));
                split = Some(parts);
                break;
            }
        }
        if split.is_none() && successors.len() == 2 {
            let guarded: Vec<&loops::Loop> = natural
                .iter()
                .filter(|one| {
                    successors.contains(&one.header)
                        && successors.iter().all(|at| *at == one.header || !one.body.contains(at))
                })
                .collect();
            if let [one] = guarded.as_slice() {
                split = Some(successors.iter().map(|at| (*at, if *at == one.header { 0.9 } else { 0.1 })).collect());
            }
        }
        let split = split.unwrap_or_else(|| successors.iter().map(|at| (*at, 1.0 / successors.len() as f64)).collect());
        let column = position[&block.at];
        for (to, probability) in split {
            matrix[position[&to]][column] -= probability;
        }
    }
    for column in 0..size {
        let pivot = (column..size).max_by(|one, other| matrix[*one][column].abs().total_cmp(&matrix[*other][column].abs()))?;
        if matrix[pivot][column].abs() < 1e-12 {
            return None;
        }
        matrix.swap(column, pivot);
        right.swap(column, pivot);
        let scale = matrix[column][column];
        for value in &mut matrix[column] {
            *value /= scale;
        }
        right[column] /= scale;
        let pivoted = matrix[column].clone();
        for row in 0..size {
            let factor = matrix[row][column];
            if row == column || factor.abs() < 1e-15 {
                continue;
            }
            for (value, by) in matrix[row].iter_mut().zip(&pivoted) {
                *value -= factor * by;
            }
            right[row] -= factor * right[column];
        }
    }
    let mut out = Executed { instructions: 0.0, memory: 0.0 };
    for (block, frequency) in body.blocks.iter().zip(&right) {
        let frequency = frequency.max(0.0);
        for one in block.insns.iter().filter(|one| crate::backend::masm::prints(one)) {
            out.instructions += frequency;
            if let Some(what) = &one.what {
                out.memory += frequency * what.dests.iter().chain(&what.sources).filter(|at| matches!(at, Loc::Mem(_))).count() as f64;
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iced_x86::Register;

    use super::executed;
    use crate::model::ir::{Loc, Operation, Reg, Semantics};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn insn(at: i64, op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Arc<Insn> {
        let what = Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, 1)), Some(what), Vec::new(), Vec::new()))
    }

    /// NOTHING anchors print no bytes, yet each counted as an executed
    /// instruction: nbody.c read 144 more with its asm unchanged.
    #[test]
    fn test_anchors_that_print_nothing_do_not_execute() {
        let ax = Loc::Reg(Reg { register: Register::AX, width: 2 });
        let bx = Loc::Reg(Reg { register: Register::BX, width: 2 });
        let insns = vec![
            insn(1, Operation::Nothing, "", vec![], vec![]),
            insn(2, Operation::Move, "mov", vec![ax], vec![bx]),
            insn(3, Operation::Nothing, "", vec![], vec![]),
            insn(4, Operation::Return, "ret", vec![], vec![]),
        ];
        let body = LirBody::new("anchored", 1, vec![LirBlock::new(1, insns)], IndexMap::default(), IndexMap::default());
        assert_eq!(executed(&body).expect("straight-line").instructions, 2.0);
    }
}
