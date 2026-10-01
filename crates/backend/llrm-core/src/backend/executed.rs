//! A finished body's executed work, estimated without a profile.
//!
//! Each block's expected executions per call (`analysis::frequency`: the
//! branch heuristics, a loop's proven trips) weigh its instructions and memory
//! operands, so spill code the MIR estimate cannot see is counted.

use crate::analysis::frequency::Frequency;
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
    let frequency = Frequency::of(body);
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
    for block in &body.blocks {
        let frequency = frequency.block(block.at);
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
        // The preheader's jump, taken as often as the guard takes it (half, with no odds), is all that differs.
        assert!((looped(true) - 0.5 - looped(false)).abs() < 1e-9, "{} {}", looped(true), looped(false));
    }

    /// An uncounted loop goes round as often as its exits' odds say: each exit
    /// a branch isel gave the loop heuristic's 31 in 32 to stay, so one
    /// tested twice a trip runs (1 / (1 - (31/32)^2)) times, where the
    /// instrument once gave every uncounted loop ten, whatever its exits.
    #[test]
    fn test_an_uncounted_loop_runs_as_often_as_its_exits_odds_say() {
        let ax = Loc::Reg(Reg { register: Register::AX, width: 2 });
        let bx = Loc::Reg(Reg { register: Register::BX, width: 2 });
        let block = |at, insns: Vec<Arc<Insn>>, succ: Vec<i64>| LirBlock { succ, ..LirBlock::new(at, insns) };
        let moved = |at| insn(at, Operation::Move, "mov", vec![ax.clone()], vec![bx.clone()]);
        let tested = |at| insn(at, Operation::Branch, "jne", vec![], vec![]);
        let stay = 124.0 / 128.0;
        let run = |blocks: Vec<LirBlock>, edges: &[(i64, i64, f64)]| {
            let mut body = LirBody::new("uncounted", 1, blocks, IndexMap::default(), IndexMap::default());
            for &(from, to, probability) in edges {
                body.odds.taken.insert((from, to), (probability * crate::model::lir::BlockOdds::CERTAIN).round() as u32);
            }
            executed(&body).expect("a loop").instructions
        };
        let ret = |at| block(at, vec![insn(at, Operation::Return, "ret", vec![], vec![])], vec![]);
        // The header, a move and a test: 32 times, and the return once.
        let once = run(vec![block(1, vec![moved(1)], vec![2]), block(2, vec![moved(2), tested(3)], vec![2, 4]), ret(4)], &[(2, 2, stay), (2, 4, 1.0 - stay)]);
        assert!((once - (1.0 + 32.0 * 2.0 + 1.0)).abs() < 0.01, "{once}");
        // A second exit tested in the same trip.
        let twice = run(
            vec![block(1, vec![moved(1)], vec![2]), block(2, vec![moved(2), tested(3)], vec![3, 5]), block(3, vec![tested(4)], vec![2, 5]), ret(5)],
            &[(2, 3, stay), (2, 5, 1.0 - stay), (3, 2, stay), (3, 5, 1.0 - stay)],
        );
        let header = 1.0 / (1.0 - stay * stay);
        let want = 1.0 + header * 2.0 + header * stay + 1.0;
        assert!((twice - want).abs() < 0.01, "{twice} {want}");
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
