//! A finished body's executed work, estimated without a profile.
//!
//! `tools/quality.py`'s `_transitions` and `_frequencies`: a loop branch continues with
//! its proved trip count, or nine times in ten; other branches split evenly. A loop
//! tested at its header tests once more than it trips. Each block's
//! expected executions per call weigh its instructions and memory operands, so spill code
//! the MIR estimate cannot see is counted.

use crate::analysis::loops;
use crate::model::ir::{Addr, Loc, Space};
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::{IndexMap, IndexSet};

/// Expected per call: instructions executed, and memory operands they touch;
/// of those instructions, the integer allocator's reloads, spill stores and
/// remats. A reload is any read of a spill slot: most are folded into their
/// use or lose the flag. x87 spills are left out: their restores carry none.
#[derive(Default)]
pub struct Executed {
    pub instructions: f64,
    pub memory: f64,
    pub reloads: f64,
    pub stores: f64,
    pub remats: f64,
}

/// `executed` as one line, for the `cost` channel and dump.
pub fn summary(body: &LirBody) -> String {
    match executed(body) {
        Some(done) => format!(
            "{} executes {:.0} instructions, {:.0} memory operands; {:.0} reloads, {:.0} spill stores, {:.0} remats",
            body.name, done.instructions, done.memory, done.reloads, done.stores, done.remats
        ),
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
    let successors_of: IndexMap<i64, Vec<i64>> = body.blocks.iter().map(|block| (block.at, block.succ.clone())).collect();
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
                // A proven count holds where the loop's one exit is tested,
                // whichever block that is: a split edge may make the way back
                // a block of its own.
                let exiting = one.body.iter().filter(|&&at| successors_of.get(&at).is_some_and(|succ| succ.iter().any(|to| !one.body.contains(to)))).count();
                let count = if exiting == 1 || block.at == one.header || one.latches.contains(&block.at) { trips.get(&one.header) } else { None };
                // Tested after a trip, the loop stays for all but its last;
                // tested at its header before one, for every trip.
                let tested = if block.at == one.header && !one.latches.contains(&block.at) { 0.0 } else { 1.0 };
                let stay = count.map_or(0.9, |count| (*count as f64 - tested) / (*count as f64 + 1.0 - tested));
                let mut parts: Vec<(i64, f64)> = inside.iter().map(|at| (*at, stay / inside.len() as f64)).collect();
                parts.extend(outside.iter().map(|at| (*at, (1.0 - stay) / outside.len() as f64)));
                split = Some(parts);
                break;
            }
        }
        if split.is_none() && successors.len() == 2 {
            // A block that only falls into a loop enters it as its header does.
            let entered = |at: i64, header: i64| at == header || successors_of.get(&at).is_some_and(|succ| succ == &[header]);
            let guarded: Vec<(&loops::Loop, i64)> = natural
                .iter()
                .filter_map(|one| {
                    let into = successors.iter().copied().find(|&at| entered(at, one.header) && (at == one.header || !one.body.contains(&at)))?;
                    successors.iter().all(|at| *at == into || !one.body.contains(at)).then_some((one, into))
                })
                .collect();
            if let [(_, into)] = guarded.as_slice() {
                split = Some(successors.iter().map(|at| (*at, if at == into { 0.9 } else { 0.1 })).collect());
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
    let x87 = |one: &Insn| one.what.as_ref().is_some_and(|what| what.op.is_x87());
    let slots: IndexSet<Addr> = body
        .blocks
        .iter()
        .flat_map(|block| &block.insns)
        .filter(|one| one.spill_store && !x87(one))
        .flat_map(|one| one.what.iter().flat_map(|what| &what.dests))
        .filter_map(|at| match at {
            Loc::Mem(cell) => cell.addr.filter(|addr| addr.space == Space::Frame),
            _ => None,
        })
        .collect();
    let reads_slot = |one: &Insn| {
        one.what.as_ref().is_some_and(|what| what.sources.iter().any(|at| matches!(at, Loc::Mem(cell) if cell.addr.is_some_and(|addr| slots.contains(&addr)))))
    };
    let mut out = Executed::default();
    for (block, frequency) in body.blocks.iter().zip(&right) {
        let frequency = frequency.max(0.0);
        for one in block.insns.iter().filter(|one| crate::backend::masm::prints(one)) {
            out.instructions += frequency;
            // A remat may be built as a reload; it counts as a remat.
            let spill = match () {
                _ if x87(one) => None,
                _ if one.rematerialized => Some(&mut out.remats),
                _ if one.spill_reload || reads_slot(one) => Some(&mut out.reloads),
                _ if one.spill_store => Some(&mut out.stores),
                _ => None,
            };
            if let Some(count) = spill {
                *count += frequency;
            }
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
    use crate::model::ir::{Addr, Loc, Mem, Operation, Reg, Semantics, Space};
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

    /// Spill traffic was only visible inside loops, weighted by depth, before
    /// loopslots promoted any of it: nothing measured what finally executes.
    #[test]
    fn test_reloads_stores_and_remats_are_counted_apart() {
        let ax = Loc::Reg(Reg { register: Register::AX, width: 2 });
        let bx = Loc::Reg(Reg { register: Register::BX, width: 2 });
        let marked = |at, reload, store, remat| {
            let mut one = (*insn(at, Operation::Move, "mov", vec![ax.clone()], vec![bx.clone()])).clone();
            (one.spill_reload, one.spill_store, one.rematerialized) = (reload, store, remat);
            Arc::new(one)
        };
        let insns = vec![
            marked(1, true, false, false),
            marked(2, false, true, false),
            marked(3, true, false, true),
            marked(4, false, false, false),
            insn(5, Operation::Return, "ret", vec![], vec![]),
        ];
        let body = LirBody::new("spilled", 1, vec![LirBlock::new(1, insns)], IndexMap::default(), IndexMap::default());
        let done = executed(&body).expect("straight-line");
        assert_eq!((done.reloads, done.stores, done.remats, done.instructions), (1.0, 1.0, 1.0, 5.0));
    }

    /// A reload folded into its use carries no flag: matmul's
    /// `imul eax,[bp-520]` read its spill slot, and 57 stores showed 0 reloads.
    #[test]
    fn test_a_use_reading_a_spill_slot_is_a_reload() {
        let eax = Loc::Reg(Reg { register: Register::EAX, width: 4 });
        let cell = Loc::Mem(Mem::new(Some(Addr::new(Space::Frame, -520)), 4));
        let mut store = (*insn(1, Operation::Move, "mov", vec![cell.clone()], vec![eax.clone()])).clone();
        store.spill_store = true;
        let insns = vec![Arc::new(store), insn(2, Operation::Multiply, "imul", vec![eax.clone()], vec![eax, cell]), insn(3, Operation::Return, "ret", vec![], vec![])];
        let body = LirBody::new("folded", 1, vec![LirBlock::new(1, insns)], IndexMap::default(), IndexMap::default());
        let done = executed(&body).expect("straight-line");
        assert_eq!((done.stores, done.reloads), (1.0, 1.0));
    }

    /// A loop tested at its header runs its body as many times as its trip
    /// count, the header once more. The estimate gave the body one trip
    /// fewer, so each such loop read cheaper than the same loop entered at
    /// its body: suite/ivchan's 21 trips counted as 20.
    #[test]
    fn test_a_loop_tested_at_its_header_runs_its_body_every_trip() {
        let ax = Loc::Reg(Reg { register: Register::AX, width: 2 });
        let bx = Loc::Reg(Reg { register: Register::BX, width: 2 });
        let block = |at, insns: Vec<Arc<Insn>>, succ: Vec<i64>| LirBlock { succ, ..LirBlock::new(at, insns) };
        let blocks = vec![
            block(1, vec![insn(1, Operation::Jump, "jmp", vec![], vec![])], vec![2]),
            block(2, vec![insn(2, Operation::Branch, "jne", vec![], vec![])], vec![3, 4]),
            block(3, vec![insn(3, Operation::Move, "mov", vec![ax], vec![bx]), insn(4, Operation::Jump, "jmp", vec![], vec![])], vec![2]),
            block(4, vec![insn(5, Operation::Return, "ret", vec![], vec![])], vec![]),
        ];
        let mut body = LirBody::new("counted", 1, blocks, IndexMap::default(), IndexMap::default());
        body.loop_trip_counts = vec![(2, 5)];
        // The entry's jump, the header's six tests, five trips of two, the return.
        assert_eq!(executed(&body).expect("a counted loop").instructions.round(), 1.0 + 6.0 + 10.0 + 1.0);
    }

    /// A loop whose way back runs through a block of its own, as a split
    /// edge makes one, tests its exit in neither its header nor its latch:
    /// its proven count went unread and it ran ten trips, not 24, so the
    /// mandel whose outer loop LSR entered at its body read 57% cheaper.
    #[test]
    fn test_a_loop_counted_at_its_one_exit_runs_its_count() {
        let ax = Loc::Reg(Reg { register: Register::AX, width: 2 });
        let bx = Loc::Reg(Reg { register: Register::BX, width: 2 });
        let block = |at, insns: Vec<Arc<Insn>>, succ: Vec<i64>| LirBlock { succ, ..LirBlock::new(at, insns) };
        let blocks = vec![
            block(1, vec![insn(1, Operation::Jump, "jmp", vec![], vec![])], vec![2]),
            block(2, vec![insn(2, Operation::Move, "mov", vec![ax], vec![bx])], vec![3]),
            block(3, vec![insn(3, Operation::Branch, "jne", vec![], vec![])], vec![5, 4]),
            block(5, vec![insn(4, Operation::Jump, "jmp", vec![], vec![])], vec![2]),
            block(4, vec![insn(5, Operation::Return, "ret", vec![], vec![])], vec![]),
        ];
        let mut body = LirBody::new("split", 1, blocks, IndexMap::default(), IndexMap::default());
        body.loop_trip_counts = vec![(2, 5)];
        // The entry's jump, five trips of the body's move and test, four of
        // the way back, the return.
        assert_eq!(executed(&body).expect("a counted loop").instructions.round(), 1.0 + 10.0 + 4.0 + 1.0);
    }

    /// A guard entering a loop through a preheader enters it as often as
    /// one branching to its header: nbody's inner loop read 180 entries
    /// where its guard jumped to the header and 100 where it jumped to a
    /// block that jumped there, the same code either way.
    #[test]
    fn test_a_guard_into_a_preheader_enters_its_loop_as_into_the_header() {
        let ax = Loc::Reg(Reg { register: Register::AX, width: 2 });
        let bx = Loc::Reg(Reg { register: Register::BX, width: 2 });
        let block = |at, insns: Vec<Arc<Insn>>, succ: Vec<i64>| LirBlock { succ, ..LirBlock::new(at, insns) };
        let looped = |preheader: bool| {
            let mut blocks = vec![block(1, vec![insn(1, Operation::Branch, "jne", vec![], vec![])], vec![if preheader { 2 } else { 3 }, 4])];
            if preheader {
                blocks.push(block(2, vec![insn(2, Operation::Jump, "jmp", vec![], vec![])], vec![3]));
            }
            blocks.push(block(3, vec![insn(3, Operation::Move, "mov", vec![ax.clone()], vec![bx.clone()]), insn(4, Operation::Branch, "jne", vec![], vec![])], vec![3, 4]));
            blocks.push(block(4, vec![insn(5, Operation::Return, "ret", vec![], vec![])], vec![]));
            executed(&LirBody::new("guarded", 1, blocks, IndexMap::default(), IndexMap::default())).expect("a loop").instructions
        };
        // The preheader's jump, taken nine times in ten, is all that differs.
        assert!((looped(true) - 0.9 - looped(false)).abs() < 1e-9, "{} {}", looped(true), looped(false));
    }

    /// An x87 spill store was counted while its restores, unflagged, were
    /// not: qbdemo's BENCHMARK read 64003 stores against 5 reloads.
    #[test]
    fn test_x87_spill_stores_are_not_counted() {
        let cell = Loc::Mem(Mem::new(Some(Addr::new(Space::Frame, -10)), 10));
        let mut store = (*insn(1, Operation::FloatStore, "fstp", vec![cell], vec![])).clone();
        store.spill_store = true;
        let insns = vec![Arc::new(store), insn(2, Operation::Return, "ret", vec![], vec![])];
        let body = LirBody::new("x87", 1, vec![LirBlock::new(1, insns)], IndexMap::default(), IndexMap::default());
        assert_eq!(executed(&body).expect("straight-line").stores, 0.0);
    }
}
