//! A finished body's executed work, estimated without a profile.
//!
//! Each block's expected executions per call (`analysis::frequency`: the
//! branch heuristics, a loop's proven trips) weigh its instructions and memory
//! operands, so spill code the MIR estimate cannot see is counted. On the
//! placed body, each edge's odds say how often a branch is taken.

use crate::analysis::frequency::Frequency;
use crate::analysis::loops;
use crate::backend::cpu::Profile;
use crate::model::ir::{Addr, Loc, Operation, Space};
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::IndexSet;

/// Expected per call: instructions executed, and memory operands they touch;
/// of those instructions, the integer allocator's reloads, spill stores and
/// remats. A reload is any read of a spill slot: most are folded into their
/// use or lose the flag. x87 spills are left out: their restores carry none.
/// Of the instructions, the conditional branches, how many of those are
/// taken, and the unconditional jumps.
#[derive(Default)]
pub struct Executed {
    pub instructions: f64,
    pub memory: f64,
    pub reloads: f64,
    pub stores: f64,
    pub remats: f64,
    pub branches: f64,
    pub taken: f64,
    pub jumps: f64,
}

impl Executed {
    /// The branches' and jumps' clocks on `cpu`.
    pub fn jump_cycles(&self, cpu: &Profile) -> Result<f64, String> {
        let cost = |form: &str| cpu.cost(form).map(|one| one as f64);
        Ok(self.taken * cost("jcc")? + (self.branches - self.taken) * cost("jcc_not_taken")? + self.jumps * cost("jmp_short")?)
    }
}

/// What the MIR spill model forecast for a function (`driver::spill_model`), per entry, priced by the
/// target's opcosts: the other side of the `pressure` channel's comparison with `executed`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Predicted {
    pub peak: i64,
    pub spilled: usize,
    pub price: f64,
    /// What the target charges a reload and a spill store, the units of `price`.
    pub load: i64,
    pub store: i64,
}

static PREDICTED: std::sync::Mutex<Vec<(String, Predicted)>> = std::sync::Mutex::new(Vec::new());

/// Remember `forecast` for the function `name`; the model runs before selection.
pub fn predict(name: &str, forecast: Predicted) {
    PREDICTED.lock().expect("the forecasts").push((name.to_owned(), forecast));
}

/// The `pressure` channel's row: the forecast for `body` beside the spill code the allocator left in it.
pub fn pressure(body: &LirBody) -> Option<String> {
    let forecast = PREDICTED.lock().expect("the forecasts").iter().rev().find(|(name, _)| *name == body.name).map(|(_, one)| *one)?;
    let done = executed(body)?;
    let actual = done.reloads * forecast.load as f64 + done.stores * forecast.store as f64;
    Some(format!(
        "{} forecast peak {} spilled {} price {:.0}; allocator {:.0} reloads, {:.0} stores, {:.0} remats, price {:.0}",
        body.name, forecast.peak, forecast.spilled, forecast.price, done.reloads, done.stores, done.remats, actual
    ))
}

/// `executed` as one line, for the `cost` channel and dump, its jumps priced on `cpu`.
pub fn summary(body: &LirBody, cpu: &Profile) -> String {
    match executed(body) {
        Some(done) => format!(
            "{} executes {:.0} instructions, {:.0} memory operands; {:.0} reloads, {:.0} spill stores, {:.0} remats; {:.0} branches, {:.0} taken, {:.0} jumps, {:.0} jump cycles",
            body.name,
            done.instructions,
            done.memory,
            done.reloads,
            done.stores,
            done.remats,
            done.branches,
            done.taken,
            done.jumps,
            done.jump_cycles(cpu).unwrap_or(f64::NAN)
        ),
        None => format!("{} executes an unbounded amount", body.name),
    }
}

/// What a body costs to run, one unit for an instruction and one for a memory operand: what `executed` counts, as the
/// number the allocator and the route choice compare alternatives by. `None` where there is no finite estimate.
pub fn work(body: &LirBody) -> Option<f64> {
    executed(body).map(|done| done.instructions + done.memory)
}

/// `None` for control flow with no finite profile-free estimate.
pub fn executed(body: &LirBody) -> Option<Executed> {
    let graph = &body.blocks;
    if !loops::irreducible(graph, Some(body.entry)).is_empty() {
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
    // An argument's incoming home is above the frame: read where it runs more often than the function does,
    // it is a register's one load made again.
    let incoming = |one: &Insn| one.what.as_ref().is_some_and(|what| what.sources.iter().any(|at| matches!(at, Loc::Mem(cell) if cell.addr.is_some_and(|addr| addr.space == Space::Frame && addr.disp > 0))));
    let entry = frequency.block(body.entry);
    let mut out = Executed::default();
    for block in &body.blocks {
        // What reaches each instruction: the block's runs less those an
        // earlier branch took.
        let mut reaching = frequency.block(block.at);
        for one in block.insns.iter().filter(|one| crate::backend::masm::prints(one)) {
            let runs = reaching;
            out.instructions += runs;
            match one.what.as_ref().map(|what| (what.op, what.target)) {
                Some((Operation::Branch, target)) => {
                    let taken = target.map_or(0.0, |to| frequency.edge(block.at, to)).min(reaching);
                    (out.branches, out.taken, reaching) = (out.branches + reaching, out.taken + taken, reaching - taken);
                }
                Some((Operation::Jump, _)) => out.jumps += reaching,
                _ => {}
            }
            // A remat may be built as a reload; it counts as a remat.
            let spill = match () {
                _ if x87(one) => None,
                _ if one.rematerialized => Some(&mut out.remats),
                _ if one.spill_reload || reads_slot(one) || (incoming(one) && runs > entry) => Some(&mut out.reloads),
                _ if one.spill_store => Some(&mut out.stores),
                _ => None,
            };
            if let Some(count) = spill {
                *count += runs;
            }
            if let Some(what) = &one.what {
                out.memory += runs * what.dests.iter().chain(&what.sources).filter(|at| matches!(at, Loc::Mem(_))).count() as f64;
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
    use crate::backend::cpu;
    use crate::model::ir::{Addr, Loc, Mem, Operation, Reg, Semantics, Space};
    use crate::model::lir::{BlockOdds, Insn, LirBlock, LirBody};
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

    /// An argument passed on the stack is read from its incoming home, a memory operand in the loop that
    /// reads it (`sub esi,[esp+32]`): the work a register holds once, per trip. It carried no spill flag and
    /// showed 0 reloads beside a forecast that put the value in a register (queens `safe`).
    #[test]
    fn test_a_loop_reading_an_incoming_argument_home_reloads_it_each_trip() {
        let ax = Loc::Reg(Reg { register: Register::AX, width: 2 });
        let home = Loc::Mem(Mem::new(Some(Addr::new(Space::Frame, 8)), 2));
        let block = |at, insns: Vec<Arc<Insn>>, succ: Vec<i64>| LirBlock { succ, ..LirBlock::new(at, insns) };
        let blocks = vec![
            block(1, vec![insn(1, Operation::Move, "mov", vec![ax.clone()], vec![home.clone()]), insn(2, Operation::Jump, "jmp", vec![], vec![])], vec![2]),
            block(2, vec![insn(3, Operation::Multiply, "imul", vec![ax.clone()], vec![ax, home]), insn(4, Operation::Branch, "jne", vec![], vec![])], vec![2, 3]),
            block(3, vec![insn(5, Operation::Return, "ret", vec![], vec![])], vec![]),
        ];
        let mut body = LirBody::new("args", 1, blocks, IndexMap::default(), IndexMap::default());
        body.loop_trip_counts = vec![(2, 5)];
        // The entry's own read is the load every path makes; the loop's five trips are the reloads.
        assert_eq!(executed(&body).expect("a counted loop").reloads.round(), 5.0);
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

    /// Placement moves no instruction the estimate saw differently from a
    /// fall-through, so no column said which of a diamond's layouts costs
    /// fewer jump clocks. On the 486, a taken branch is 3, a fall-through 1,
    /// a jmp 3: the likelier arm (3 in 4) as the branch's target, falling
    /// into the join, saves the rarer arm's jmp on the common path.
    #[test]
    fn test_a_diamonds_jumps_are_counted_taken_or_not_and_priced() {
        let ax = Loc::Reg(Reg { register: Register::AX, width: 2 });
        let bx = Loc::Reg(Reg { register: Register::BX, width: 2 });
        let block = |at, insns: Vec<Arc<Insn>>, succ: Vec<i64>| LirBlock { succ, ..LirBlock::new(at, insns) };
        let targeted = |at, op, name, target| {
            let mut one = (*insn(at, op, name, vec![], vec![])).clone();
            one.what.as_mut().unwrap().target = Some(target);
            Arc::new(one)
        };
        let mov = |at| insn(at, Operation::Move, "mov", vec![ax.clone()], vec![bx.clone()]);
        // Blocks 2 (likely) and 3 (rare) join at 4; `second` falls into it.
        let placed = |second: i64| {
            let first = 5 - second;
            let blocks = vec![
                block(1, vec![targeted(1, Operation::Branch, "jne", second)], vec![second, first]),
                block(first, vec![mov(2), targeted(3, Operation::Jump, "jmp", 4)], vec![4]),
                block(second, vec![mov(4)], vec![4]),
                block(4, vec![insn(5, Operation::Return, "ret", vec![], vec![])], vec![]),
            ];
            let mut body = LirBody::new("diamond", 1, blocks, IndexMap::default(), IndexMap::default());
            for (to, probability) in [(2, 0.75), (3, 0.25)] {
                body.odds.taken.insert((1, to), (probability * BlockOdds::CERTAIN).round() as u32);
            }
            let done = executed(&body).expect("a diamond");
            (done.branches, done.taken, done.jumps, done.jump_cycles(cpu::named("486").unwrap()).unwrap())
        };
        let close = |(a, b, c, d): (f64, f64, f64, f64), want: [f64; 4]| [a, b, c, d].iter().zip(want).all(|(got, want)| (got - want).abs() < 1e-3);
        let (rare_second, likely_second) = (placed(3), placed(2));
        assert!(close(rare_second, [1.0, 0.25, 0.75, 3.75]), "{rare_second:?}");
        assert!(close(likely_second, [1.0, 0.75, 0.25, 3.25]), "{likely_second:?}");
    }

    /// Work after a branch runs only as often as the branch falls through:
    /// a `jne loop; jmp exit` block counted its `jmp` on every pass, so the
    /// instruction column could not see a jump that tail duplication moved
    /// off the loop (PLASMABLOBS read 35,468,659 either way).
    #[test]
    fn test_work_after_a_branch_runs_as_often_as_it_falls_through() {
        let ax = Loc::Reg(Reg { register: Register::AX, width: 2 });
        let bx = Loc::Reg(Reg { register: Register::BX, width: 2 });
        let block = |at, insns: Vec<Arc<Insn>>, succ: Vec<i64>| LirBlock { succ, ..LirBlock::new(at, insns) };
        let targeted = |at, op, name, target| {
            let mut one = (*insn(at, op, name, vec![], vec![])).clone();
            one.what.as_mut().unwrap().target = Some(target);
            Arc::new(one)
        };
        let blocks = vec![
            block(1, vec![insn(1, Operation::Compare, "cmp", vec![], vec![ax.clone(), bx.clone()]), targeted(2, Operation::Branch, "jne", 3), insn(3, Operation::Move, "mov", vec![ax.clone()], vec![bx.clone()]), targeted(4, Operation::Jump, "jmp", 4)], vec![3, 4]),
            block(3, vec![insn(5, Operation::Move, "mov", vec![ax.clone()], vec![bx.clone()])], vec![4]),
            block(4, vec![insn(6, Operation::Return, "ret", vec![], vec![])], vec![]),
        ];
        let mut body = LirBody::new("tail", 1, blocks, IndexMap::default(), IndexMap::default());
        body.odds.taken.insert((1, 3), (0.75 * BlockOdds::CERTAIN) as u32);
        body.odds.taken.insert((1, 4), (0.25 * BlockOdds::CERTAIN) as u32);
        let done = executed(&body).expect("straight branches");
        // cmp and jne every time, mov and jmp a quarter, block 3's mov three quarters, ret.
        assert!((done.instructions - 4.25).abs() < 1e-3, "{}", done.instructions);
    }
}
