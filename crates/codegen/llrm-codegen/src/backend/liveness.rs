//! Which physical register and flag lanes are live into and dead out of each
//! block, and what one instruction reads and writes.
//!
//! # How it works
//! A lane is one byte of a general or segment register, or one flag bit
//! ([`crate::backend::lanes`]). `effect` gives an instruction's reads and
//! writes: as it decodes (`_register_effects`), else as its contract declares
//! (`_declared`: a call's `requires`, `clobbers` and `delivers`, a return's
//! explicit results and the architectural state its epilogue needs). An
//! instruction with neither is unknown and may read anything. The answer is
//! worked out once per instruction and kept on it.
//!
//! A shift's effect also lists the bytes it moves, `(written, source)`: a
//! source is read only where its written byte, or a flag the shift sets, is
//! live after it. A parallel copy group is one effect at its last copy, since
//! it reads every source before it writes any destination.
//!
//! Inside a block the walk goes backward:
//!
//! ```text
//!   live_before = (live_after - writes) | reads
//! ```
//!
//! Across blocks `live_into` finds the least fixed point with a worklist that
//! starts from the last block: a block is recomputed when a successor's live
//! set changed. A block with no successor leaves everything live, so a copy
//! written as the last instruction of an exit block survives, and so does a
//! parallel copy for a phi, which is written exactly at a block end.
//! `dead_at_exit` is the complement, per block: the lanes nothing reads again.
//!
//! The answer is kept between asks and brought up to date from the blocks that
//! changed (see [`crate::backend::incremental`]), because the peephole
//! sub-passes ask in turn and each leaves most blocks as they were. A block's
//! step is the lanes it reads before writing them and those it writes, kept
//! while the block is the same. `LLRM_CHECK_LIVENESS=1` solves the whole body
//! from scratch after every answer and asserts they agree.
//!
//! SI, DI and the frame register are not return inputs: their incoming values
//! live in the prologue's saves, so a dead final write to one stays removable.
//!
//! # Example
//! No example: the lane sets it computes are printed by no dump, and the tests
//! assert them without printing. See `lanes.rs` for the lane model.
//!
//! # Used by
//! `peephole` and the sub-passes it runs (`regthrash`, `copysink`,
//! `postrasink`, `exactaddress`, `upperzero`, `sharedstores`, `machinedce`),
//! `loopslots`, and the peephole rule walker (`peep/walk`).

use std::rc::Rc;

use llrm_lir::registers::RegId;
use llrm_lir::registers::Regs;

use crate::backend::incremental::{Kept, Problem, Solution};
use crate::backend::peephole::{_branch_reads, _flag_lanes, _lanes, _moved_lanes, _register_effects, Lane, Lanes};
use crate::model::ir::{Held, Loc, Operation, Semantics};
use crate::model::lir::{Block, Insn, LirBlock, LirBody};
use crate::support::hash::IndexMap;

pub fn _terminator(what: Option<&Semantics>) -> bool {
    what.is_some_and(|what| {
        [Operation::Branch, Operation::Jump].contains(&what.op)
            && what.dests.is_empty()
            && what.sources.is_empty()
            && what.target.is_some()
    })
}

/// Whether `what` ends its block: a branch or a jump, a table jump too, which
/// reads the register that indexes the table and so is no `_terminator`.
pub fn _leaves(what: Option<&Semantics>) -> bool {
    _terminator(what) || what.is_some_and(|what| what.op == Operation::Jump && what.indirect && !what.table.is_empty())
}

// Machine state a generated return itself needs beside its explicit results.
// SI and DI are callee-saved at the ABI boundary, but their incoming values
// live in the prologue's stack saves, not in transient body registers.  The
// MASM emitter derives push/pop preservation from surviving body uses, so a
// dead final write to either register must remain removable.  Treating them as
// semantic return inputs retained one-use loads and other dead computations
// immediately before an epilogue. The frame register is no different: where a
// frame is kept the body never writes it, and where none is (`hasFP` false) it
// is a value register the epilogue restores, as for SI and DI.
pub fn _return_state(regs: Regs) -> Vec<RegId> {
    [Some(regs.stack), regs.data_segment, regs.stack_segment, regs.code_segment].into_iter().flatten().collect()
}

/// Every lane a body can name. "Dead" here means every lane but the live ones.
pub fn _universe(regs: Regs) -> Lanes {
    let mut lanes = _flag_lanes(0xFFFF_FFFF);
    for register in regs.integer_registers().iter().copied().chain(regs.segments()) {
        lanes.extend(_lanes(regs, register));
    }
    lanes
}

/// What one instruction does to register and flag lanes.
#[derive(Clone, Debug)]
pub struct Effect {
    /// Read whatever is live after it.
    pub reads: Lanes,
    pub writes: Lanes,
    /// A constant shift's bytes, `(written, source)`: a source is read only
    /// where its written byte, or a flag the shift sets, is live after it.
    moved: Vec<(Lane, Lane)>,
}

impl Effect {
    /// Every lane it may read: `reads` and the sources of a shift's bytes,
    /// which are read only where live.
    pub fn all_reads(&self) -> Lanes {
        self.moved.iter().fold(self.reads, |reads, (_, source)| reads.or(&Lanes::from([*source])))
    }

    /// This effect with the upper halves a call only may clobber
    /// (`clobbers_high`) left as they were: a callee that keeps them is not
    /// a write of theirs, so a read after the call still reads the value
    /// before it.
    pub fn keeping_high(
        &self,
        regs: Regs,
        one: &Insn,
    ) -> Effect {
        let mut kept = self.clone();
        let mut certain: Lanes = one.clobbers.iter().flat_map(|register| _lanes(regs, *register)).collect();
        certain.extend(
            one.delivers.iter().flat_map(|(held, register)| _lanes(regs, regs.named(*register, i64::from(held.width)))),
        );
        for register in &one.clobbers_high {
            for lane in _lanes(regs, *register).into_iter().filter(|lane| lane.1 >= 2) {
                if !certain.contains(&lane) {
                    kept.writes = kept.writes.minus(&Lanes::from([lane]));
                }
            }
        }
        kept
    }

    /// The lanes live before it, given those live after.
    pub fn live_before(
        &self,
        live: &Lanes,
    ) -> Lanes {
        live.minus(&self.writes).or(&self.read(|lane| live.contains(lane)))
    }

    /// The lanes dead before it, given those dead after.
    pub fn dead_before(
        &self,
        dead: &Lanes,
    ) -> Lanes {
        dead.or(&self.writes).minus(&self.read(|lane| !dead.contains(lane)))
    }

    fn read(
        &self,
        live: impl Fn(&Lane) -> bool,
    ) -> Lanes {
        let mut reads = self.reads;
        // A shift's flags come from the bits it moves.
        let flagged = self.writes.iter().any(|lane| lane.0 == RegId::None && live(lane));
        for (written, source) in &self.moved {
            if flagged || live(written) {
                reads.insert(*source);
            }
        }
        reads
    }
}

thread_local! {
    static EFFECT_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many effects this thread has worked out by `effect`, for a test that a
/// pass does not ask of an instruction it has seen.
pub fn effects_worked_out() -> usize {
    EFFECT_CALLS.with(std::cell::Cell::get)
}

/// `with` given `one`'s effect, as `effect` has it but without a copy of it.
pub fn with_effect<R>(
    regs: Regs,
    bits: u32,
    one: &Insn,
    with: impl FnOnce(Option<&Effect>) -> R,
) -> R {
    let (was, answer) = one.effect.get_or_init(|| (bits, worked_out(regs, bits, one)));
    if *was == bits { with(answer.as_ref()) } else { with(worked_out(regs, bits, one).as_ref()) }
}

/// `one`'s effect as it decodes, else as its contract declares; None when
/// unknown. Worked out once for the instruction, whoever asks.
pub fn effect(
    regs: Regs,
    bits: u32,
    one: &Insn,
) -> Option<Effect> {
    let (was, answer) = one.effect.get_or_init(|| (bits, worked_out(regs, bits, one)));
    // Asked at another width than the first time: not the answer kept.
    let found = if *was == bits { answer.clone() } else { worked_out(regs, bits, one) };
    if llrm_support::env_flag!("LLRM_CHECK_EFFECT") {
        assert!(
            format!("{found:?}") == format!("{:?}", worked_out(regs, bits, one)),
            "an instruction's kept effect is not the one its fields give"
        );
    }
    found
}

fn worked_out(
    regs: Regs,
    bits: u32,
    one: &Insn,
) -> Option<Effect> {
    EFFECT_CALLS.with(|count| count.set(count.get() + 1));
    if _terminator(one.what.as_ref()) {
        // A jump or branch writes nothing; a branch reads its flags.
        let what = one.what.as_ref().expect("a terminator has semantics");
        let reads = if what.op == Operation::Branch { _branch_reads(what) } else { Lanes::new() };
        return Some(Effect { reads, writes: Lanes::new(), moved: Vec::new() });
    }
    let (reads, writes) = _register_effects(regs, bits, one, false, true).or_else(|| _declared(regs, one))?;
    Some(match _moved_lanes(regs, bits, one) {
        Some((moved, operands)) => Effect { reads: reads.minus(&operands), writes, moved },
        None => Effect { reads, writes, moved: Vec::new() },
    })
}

/// Each instruction's effect in `block`, decoded once for a fixed point to
/// reuse.
pub fn _effects(
    regs: Regs,
    bits: u32,
    block: &LirBlock,
) -> Vec<Option<Effect>> {
    let mut effects: Vec<Option<Effect>> = block.insns.iter().map(|one| effect(regs, bits, one)).collect();
    // A parallel copy reads every source before it writes any destination,
    // whatever order its copies are kept in: one effect, at its last copy,
    // and none at the rest. Walked a copy at a time, a source another copy of
    // the group writes first in that order was dead before the group, and
    // the register was handed out as free.
    let nothing = || Effect { reads: Lanes::new(), writes: Lanes::new(), moved: Vec::new() };
    let mut at = 0;
    while at < block.insns.len() {
        let Some(group) = block.insns[at].group else {
            at += 1;
            continue;
        };
        let mut end = at + 1;
        while end < block.insns.len() && block.insns[end].group == Some(group) {
            end += 1;
        }
        if end - at > 1 {
            let merged = effects[at..end]
                .iter()
                .try_fold(
                    nothing(),
                    |all, one| one.as_ref().map(|one| Effect {
                        reads: all.reads.or(&one.reads),
                        writes: all.writes.or(&one.writes),
                        moved: [all.moved, one.moved.clone()].concat(),
                    }),
                );
            effects[at..end - 1].iter_mut().for_each(|one| *one = Some(nothing()));
            effects[end - 1] = merged;
        }
        at = end;
    }
    effects
}

/// The lanes live before `effects`, given those live after them. An unknown
/// instruction may read anything.
pub fn _before(
    effects: &[Option<Effect>],
    live: Lanes,
    universe: &Lanes,
) -> Lanes {
    effects.iter().rev().fold(live, |live, one| one.as_ref().map_or_else(|| *universe, |one| one.live_before(&live)))
}

/// The lanes live before `block`, given those live after it.
pub fn _backwards(
    regs: Regs,
    bits: u32,
    block: &LirBlock,
    live: Lanes,
    universe: &Lanes,
) -> Lanes {
    // Each effect is read where it is kept: a copy of every instruction's, for
    // each round of a fixed point, was a tenth of peephole.
    block.insns.iter().rev().fold(live, |live, one| {
        with_effect(regs, bits, one, |effect| effect.map_or_else(|| *universe, |effect| effect.live_before(&live)))
    })
}

/// What a call says it reads and writes, for an instruction no decoder covers.
///
/// `requires` and `clobbers` are the contract the allocation is already built
/// on, so reading a call as touching every register only makes a register the
/// callee never names look live -- which kept every value a phi copies alive
/// across the whole loop.
pub fn _declared(
    regs: Regs,
    one: &Insn,
) -> Option<(Lanes, Lanes)> {
    let held_lanes = |held: &Held, register: RegId| _lanes(regs, regs.named(register, i64::from(held.width)));

    if one.what.as_ref().is_some_and(|what| what.op == Operation::Return) && one.reads_complete() {
        // Nothing runs after it: it reads explicit results and only the
        // architectural state its generated epilogue itself needs.
        let mut reads: Lanes = one.requires.iter().flat_map(|(held, register)| held_lanes(held, *register)).collect();
        for register in _return_state(regs) {
            reads.extend(_lanes(regs, register));
        }
        let writes = _universe(regs).minus(&reads);
        return Some((reads, writes));
    }
    if one.clobbers.is_empty() || one.symbol == Some(true) {
        return None;
    }
    let mut reads: Lanes = one.requires.iter().flat_map(|(held, register)| held_lanes(held, *register)).collect();
    // A callee runs on the caller's frame chain, stack and data group.
    if one.what.as_ref().is_some_and(|what| what.op == Operation::Call) {
        for register in _return_state(regs) {
            reads.extend(_lanes(regs, register));
        }
    }
    // A transfer's decoded effects are unavailable, but its explicit operands
    // are still real reads. In particular an indirect `call bx` reads BX
    // before the calling convention clobbers it.
    for source in one.what.as_ref().map_or(&[][..], |what| what.sources.as_slice()) {
        if let Loc::Reg(source) = source {
            reads.extend(_lanes(regs, source.register));
        }
        // `selector` is a `Held`, never an `ir.Reg`.
        if let Some(at) = source.address() {
            reads.extend(_lanes(regs, at.through));
            reads.extend(_lanes(regs, at.index_through));
        }
    }
    let mut writes: Lanes = one.delivers.iter().flat_map(|(held, register)| held_lanes(held, *register)).collect();
    for register in &one.clobbers {
        writes.extend(_lanes(regs, *register));
    }
    for register in &one.clobbers_high {
        writes.extend(_lanes(regs, *register).into_iter().filter(|lane| lane.1 >= 2));
    }
    writes.extend(_flag_lanes(0xFFFF_FFFF));
    Some((reads, writes))
}

/// Lane liveness as a [`Problem`]: what a block reads before it writes it and
/// what it writes, flowing back from the blocks that follow.
pub struct LaneLive {
    regs: Regs,
    bits: u32,
    universe: std::cell::OnceCell<Lanes>,
}

/// What one block does to the lanes live: its instructions' effects, and as
/// one step `(read, written)` where no instruction moves bytes by a shift.
pub struct LaneStep {
    effects: Vec<Option<Effect>>,
    step: Option<(Lanes, Lanes)>,
}

impl LaneLive {
    fn of(body: &LirBody) -> Self {
        Self { regs: body.regs(), bits: body.bits, universe: std::cell::OnceCell::new() }
    }

    fn universe(&self) -> Lanes {
        *self.universe.get_or_init(|| _universe(self.regs))
    }
}

impl Problem for LaneLive {
    type Summary = LaneStep;
    type Value = Lanes;
    const FORWARD: bool = false;

    fn summarize(
        &self,
        _body: &LirBody,
        block: &Block,
    ) -> LaneStep {
        let effects = _effects(self.regs, self.bits, block);
        let step = summarized(&effects, &self.universe());
        LaneStep { effects, step }
    }

    fn start(&self) -> Lanes {
        Lanes::new()
    }

    /// Lanes live after a block: everything at an exit, which no later code
    /// narrows, else what its successors read.
    fn entering(
        &self,
        _at: i64,
        _entry: i64,
        present: &[Lanes],
        _missing: usize,
    ) -> Lanes {
        if present.is_empty() { self.universe() } else { present.iter().fold(Lanes::new(), |all, next| all.or(next)) }
    }

    fn leaving(
        &self,
        summary: &LaneStep,
        after: Lanes,
    ) -> Lanes {
        match &summary.step {
            Some((read, written)) => after.minus(written).or(read),
            None => _before(&summary.effects, after, &self.universe()),
        }
    }
}

thread_local! {
    static KEPT: Kept<LaneLive> = const { Kept::new() };
    /// The exits of the answer last asked for, which `dead_at_exit` hands out.
    static EXITS: std::cell::RefCell<Option<(Rc<Solution<LaneLive>>, Rc<IndexMap<i64, Lanes>>)>> =
        const { std::cell::RefCell::new(None) };
}

/// How many times this thread has solved a body's lanes from scratch (no block
/// left as the last answer had it), brought a held answer up to date, and been
/// asked of a body no pass changed, for a test that a clean-up does not solve
/// a function again for each sub-pass.
pub fn asks() -> [usize; 3] {
    KEPT.with(Kept::asks)
}

/// How many graphs of a body's edges this thread has made for lane liveness.
pub fn graphs() -> usize {
    KEPT.with(Kept::graphs)
}

/// How many answers this thread has worked out, whole or in part.
pub fn exits_computed() -> usize {
    let [whole, partly, _] = asks();
    whole + partly
}

/// How many times this thread has worked out a block's lanes, for a test that a
/// chain of blocks does not take a round of the body for each.
pub fn visits() -> usize {
    KEPT.with(Kept::visits)
}

/// The lane liveness of `body`: `leaving` is the lanes live on entry to each
/// block, `entering` those live after it. See [`crate::backend::incremental`].
pub fn lanewise(body: &LirBody) -> Rc<Solution<LaneLive>> {
    let problem = LaneLive::of(body);
    let solved = KEPT.with(|kept| kept.ask(&problem, body));
    if llrm_support::env_flag!("LLRM_CHECK_LIVENESS") {
        assert!(
            from_scratch(body) == solved.leaving,
            "{}: lane liveness brought up to date differs from one solved from scratch",
            body.name
        );
    }
    solved
}

/// A block's effects as one step, `(read, written)`, so that the lanes live
/// before it are `(after - written) | read`; None where a shift moves bytes,
/// whose reads depend on what is live after it. An unknown instruction reads
/// anything and so is a step that writes and reads everything.
fn summarized(
    effects: &[Option<Effect>],
    universe: &Lanes,
) -> Option<(Lanes, Lanes)> {
    let (mut read, mut written) = (Lanes::new(), Lanes::new());
    for one in effects.iter().rev() {
        match one {
            None => (read, written) = (*universe, *universe),
            Some(one) if !one.moved.is_empty() => return None,
            Some(one) => {
                read = read.minus(&one.writes).or(&one.reads);
                written = written.or(&one.writes);
            }
        }
    }
    Some((read, written))
}

/// The lanes live on entry to each block of `body`, solved from nothing held:
/// one worklist over all the blocks. What `LLRM_CHECK_LIVENESS` compares an
/// answer brought up to date with.
fn from_scratch(body: &LirBody) -> IndexMap<i64, Lanes> {
    let regs = body.regs();
    let universe = _universe(regs);
    let at_of: crate::support::hash::HashSet<i64> = body.blocks.iter().map(|block| block.at).collect();
    let blocks: IndexMap<i64, &Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let successors: IndexMap<i64, Vec<i64>> = body
        .blocks
        .iter()
        .map(|block| (block.at, block.succ.iter().copied().filter(|at| at_of.contains(at)).collect()))
        .collect();
    let mut into: IndexMap<i64, Lanes> = blocks.keys().map(|at| (*at, Lanes::new())).collect();
    let mut predecessors: IndexMap<i64, Vec<i64>> = blocks.keys().map(|at| (*at, Vec::new())).collect();
    for (at, to) in &successors {
        for next in to {
            predecessors[next].push(*at);
        }
    }
    let mut pending: Vec<i64> = blocks.keys().copied().collect();
    let mut queued: crate::support::hash::HashSet<i64> = pending.iter().copied().collect();
    while let Some(at) = pending.pop() {
        queued.remove(&at);
        let after = if successors[&at].is_empty() {
            universe
        } else {
            successors[&at].iter().fold(Lanes::new(), |all, to| all.or(&into[to]))
        };
        let before = _before(&_effects(regs, body.bits, blocks[&at]), after, &universe);
        if before != into[&at] {
            into.insert(at, before);
            for &pred in &predecessors[&at] {
                if queued.insert(pred) {
                    pending.push(pred);
                }
            }
        }
    }
    into
}

/// Per block, the lanes live on entry -- with its successors and the universe.
pub fn live_into(body: &LirBody) -> (IndexMap<i64, Lanes>, IndexMap<i64, Vec<i64>>, Lanes) {
    let solved = lanewise(body);
    let at_of: crate::support::hash::HashSet<i64> = body.blocks.iter().map(|block| block.at).collect();
    let successors = body
        .blocks
        .iter()
        .map(|block| (block.at, block.succ.iter().copied().filter(|at| at_of.contains(at)).collect()))
        .collect();
    (solved.leaving.clone(), successors, LaneLive::of(body).universe())
}

/// Per block, the lanes nothing reads again after it.
pub fn dead_at_exit(body: &LirBody) -> Rc<IndexMap<i64, Lanes>> {
    let solved = lanewise(body);
    if let Some(held) = EXITS.with(|held| {
        held.borrow().as_ref().filter(|(of, _)| Rc::ptr_eq(of, &solved)).map(|(_, exits)| Rc::clone(exits))
    }) {
        return held;
    }
    let universe = LaneLive::of(body).universe();
    let exits: Rc<IndexMap<i64, Lanes>> =
        Rc::new(solved.entering.iter().map(|(at, live)| (*at, universe.minus(live))).collect());
    EXITS.with(|held| *held.borrow_mut() = Some((Rc::clone(&solved), Rc::clone(&exits))));
    exits
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use llrm_lir::registers::RegId;

    use super::{asks, from_scratch, graphs, lanewise, live_into};
    use crate::backend::peephole::_lanes;
    use crate::model::ir::{Held, Loc, Operation, Reg, Semantics};
    use crate::model::lir::{Block, Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    const AX: Reg = Reg { register: RegId::AX, width: 2 };
    const DX: Reg = Reg { register: RegId::DX, width: 2 };

    fn _insn(
        at: i64,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
    ) -> Arc<Insn> {
        Arc::new(Insn::new(
            at,
            Some((at, at)),
            Some(Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) }),
            vec![],
            vec![],
        ))
    }

    #[test]
    fn test_a_register_written_before_an_unknown_instruction_is_not_live_into_its_block() {
        let regs = crate::backend::registerinfo::test_regs();
        // A return, which nothing decodes, made its whole block live-in for
        // every lane: the `mov edx,eax` before it did not count.
        let block = LirBlock::new(
            1,
            vec![
                _insn(1, Operation::Move, "mov", vec![Loc::Reg(DX)], vec![Loc::Reg(AX)]),
                _insn(2, Operation::Return, "ret", vec![], vec![]),
            ],
        );
        let body = LirBody::new("f", 1, vec![block.into()], IndexMap::default(), IndexMap::default());
        let (into, _successors, _universe) = live_into(&body);
        assert!(_lanes(regs, RegId::DX).is_disjoint(&into[&1]));
        assert!(_lanes(regs, RegId::AX).is_subset(&into[&1]));
    }

    /// A parallel copy kept as `bx <- dx` then `ax <- bx` reads bx before it
    /// writes it: grep at -O2 -m32 saw bx dead before the copy, was told it
    /// was free, and carried a cycle in it while the next block still read it.
    #[test]
    fn test_a_parallel_copy_reads_its_sources_before_any_destination_is_written() {
        let regs = crate::backend::registerinfo::test_regs();
        let bx = Reg { register: RegId::BX, width: 2 };
        let cx = Reg { register: RegId::CX, width: 2 };
        let grouped = |at: i64, into: Reg, from: Reg| {
            let mut one = Insn::new(
                at,
                Some((at, at)),
                Some(Semantics {
                    name: Some("mov".into()),
                    dests: vec![Loc::Reg(into)],
                    sources: vec![Loc::Reg(from)],
                    ..Semantics::new(Operation::Move)
                }),
                vec![],
                vec![],
            );
            one.group = Some(1);
            Arc::new(one)
        };
        let first = LirBlock { succ: vec![2].into(), ..LirBlock::new(1, vec![grouped(1, bx, DX), grouped(2, AX, bx)]) };
        let last = LirBlock::new(2, vec![_insn(3, Operation::Move, "mov", vec![Loc::Reg(cx)], vec![Loc::Reg(AX)])]);
        let body = LirBody::new("f", 1, vec![first.into(), last.into()], IndexMap::default(), IndexMap::default());
        let (into, _successors, _universe) = live_into(&body);
        assert!(_lanes(regs, RegId::BX).is_subset(&into[&1]), "bx is read by `ax <- bx` before `bx <- dx` writes it");
        assert!(_lanes(regs, RegId::DX).is_subset(&into[&1]));
        assert!(_lanes(regs, RegId::AX).is_disjoint(&into[&1]));
    }

    /// A chain of blocks, the last reading AX and each before it writing a
    /// register: the lanes live into the first.
    fn chain(blocks: i64) -> LirBody {
        let bx = Reg { register: RegId::BX, width: 2 };
        let blocks: Vec<Block> = (1..=blocks)
            .map(|at| {
                let insns = if at == blocks {
                    vec![_insn(at, Operation::Move, "mov", vec![Loc::Reg(DX)], vec![Loc::Reg(AX)])]
                } else {
                    vec![_insn(at, Operation::Move, "mov", vec![Loc::Reg(bx)], vec![Loc::Reg(DX)])]
                };
                LirBlock {
                    succ: if at == blocks { vec![].into() } else { vec![at + 1].into() },
                    ..LirBlock::new(at, insns)
                }
            })
            .collect();
        LirBody::new("f", 1, blocks, IndexMap::default(), IndexMap::default())
    }

    /// Taken in layout order from the first block, each round of the fixed
    /// point carried a change one block back: a chain of n blocks was n
    /// rounds of n blocks, and peephole's `dead_at_exit` was 18% of compiling
    /// 800 blocks (#560). Starting from the last, each block is worked out
    /// about once.
    #[test]
    fn test_a_chain_of_blocks_is_not_a_round_of_the_body_for_each_block() {
        let regs = crate::backend::registerinfo::test_regs();
        let body = chain(200);
        let before = super::visits();
        let (into, _successors, _universe) = live_into(&body);
        assert!(super::visits() - before <= 3 * 200, "{} block visits for 200 blocks", super::visits() - before);
        assert!(
            _lanes(regs, RegId::AX).is_subset(&into[&1]),
            "AX is read at the end of the chain and written nowhere before"
        );
    }

    #[test]
    fn test_a_return_whose_reads_are_complete_reads_only_results_and_return_state() {
        let regs = crate::backend::registerinfo::test_regs();
        // Every return read every register; one the raise wrote reads its
        // results and the registers the return itself needs.
        let cx = Reg { register: RegId::CX, width: 2 };
        let mut ret = Insn::new(
            3,
            Some((3, 3)),
            Some(Semantics { name: Some("".into()), ..Semantics::new(Operation::Return) }),
            vec![],
            vec![1],
        );
        ret.requires = vec![(Held { value: 1, width: 2 }, RegId::AX)];
        ret.reads_complete = true;
        let block = LirBlock::new(
            1,
            vec![_insn(1, Operation::Move, "mov", vec![Loc::Reg(cx)], vec![Loc::Reg(AX)]), Arc::new(ret)],
        );
        let body = LirBody::new("f", 1, vec![block.into()], IndexMap::default(), IndexMap::default());
        let (into, _successors, _universe) = live_into(&body);
        assert!(_lanes(regs, RegId::AX).is_subset(&into[&1]));
        assert!(_lanes(regs, RegId::SI).is_disjoint(&into[&1]));
        assert!(_lanes(regs, RegId::BP).is_disjoint(&into[&1]));
        assert!(_lanes(regs, RegId::DX).is_disjoint(&into[&1]));
    }

    /// A loop's header read AX each trip, which kept AX live round the loop;
    /// the read goes. Solved again from the answer that was held, AX stayed
    /// live round the loop on its own (a larger fixed point than the one
    /// the body has), and everything that asked was told AX was in use.
    #[test]
    fn test_a_lane_kept_live_round_a_loop_by_a_read_that_is_gone_is_not_live() {
        let regs = crate::backend::registerinfo::test_regs();
        let (bx, cx) = (Reg { register: RegId::BX, width: 2 }, Reg { register: RegId::CX, width: 2 });
        let header = |reads: Reg| LirBlock {
            succ: vec![2, 3].into(),
            ..LirBlock::new(1, vec![_insn(1, Operation::Move, "mov", vec![Loc::Reg(cx)], vec![Loc::Reg(reads)])])
        };
        let trip = LirBlock {
            succ: vec![1].into(),
            ..LirBlock::new(2, vec![_insn(2, Operation::Move, "mov", vec![Loc::Reg(DX)], vec![Loc::Reg(bx)])])
        };
        let leave = LirBlock::new(3, vec![_insn(3, Operation::Move, "mov", vec![Loc::Reg(AX)], vec![Loc::Reg(bx)])]);
        let (trip, leave): (Block, Block) = (trip.into(), leave.into());
        let body = |header: LirBlock| {
            LirBody::new(
                "f",
                1,
                vec![header.into(), trip.clone(), leave.clone()],
                IndexMap::default(),
                IndexMap::default(),
            )
        };
        let before = lanewise(&body(header(AX)));
        assert!(_lanes(regs, RegId::AX).is_subset(&before.leaving[&2]), "premise: AX is live round the loop");
        let after = lanewise(&body(header(bx)));
        assert!(_lanes(regs, RegId::AX).is_disjoint(&after.leaving[&1]), "AX is live into the header");
        assert!(_lanes(regs, RegId::AX).is_disjoint(&after.leaving[&2]), "AX is live into the loop's body");
        assert!(after.leaving == from_scratch(&body(header(bx))));
    }

    /// Asked of a body whose blocks are the same instructions, nothing is
    /// worked out; one block changed, the others' effects are kept.
    #[test]
    fn test_a_body_asked_again_is_answered_and_a_changed_block_is_the_only_one_worked_out() {
        let (bx, cx) = (Reg { register: RegId::BX, width: 2 }, Reg { register: RegId::CX, width: 2 });
        let mov =
            |at: i64, to: Reg, from: Reg| _insn(at, Operation::Move, "mov", vec![Loc::Reg(to)], vec![Loc::Reg(from)]);
        let first = LirBlock { succ: vec![2].into(), ..LirBlock::new(1, vec![mov(1, cx, AX)]) };
        let last = LirBlock::new(2, vec![mov(2, DX, cx)]);
        let (first, last): (Block, Block) = (first.into(), last.into());
        let body = |last: &Block| {
            LirBody::new("f", 1, vec![first.clone(), last.clone()], IndexMap::default(), IndexMap::default())
        };
        let [whole, partly, same] = asks();
        lanewise(&body(&last));
        lanewise(&body(&last));
        let changed: Block = LirBlock::new(2, vec![mov(2, DX, bx)]).into();
        lanewise(&body(&changed));
        let [whole_now, partly_now, same_now] = asks();
        assert_eq!((whole_now - whole, partly_now - partly, same_now - same), (1, 1, 1));
    }

    /// A block that goes elsewhere is a new graph and its lanes follow the new
    /// edge; one whose instructions alone changed keeps the graph. Reusing the
    /// graph after a reroute answered from the old edges: block 1 still read
    /// AX through block 2 after it jumped to block 3.
    #[test]
    fn test_a_graph_is_made_again_only_where_a_block_goes_elsewhere() {
        let regs = crate::backend::registerinfo::test_regs();
        let (bx, cx) = (Reg { register: RegId::BX, width: 2 }, Reg { register: RegId::CX, width: 2 });
        let mov =
            |at: i64, to: Reg, from: Reg| _insn(at, Operation::Move, "mov", vec![Loc::Reg(to)], vec![Loc::Reg(from)]);
        let first =
            |to: i64| -> Block { LirBlock { succ: vec![to].into(), ..LirBlock::new(1, vec![mov(1, bx, DX)]) }.into() };
        // Each loops on itself, so that nothing past it is assumed live.
        let reads_ax: Block = LirBlock { succ: vec![2].into(), ..LirBlock::new(2, vec![mov(2, DX, AX)]) }.into();
        let reads_cx: Block = LirBlock { succ: vec![3].into(), ..LirBlock::new(3, vec![mov(3, DX, cx)]) }.into();
        let body = |first: Block, second: &Block| {
            LirBody::new(
                "f",
                1,
                vec![first, second.clone(), reads_cx.clone()],
                IndexMap::default(),
                IndexMap::default(),
            )
        };
        let made = graphs();
        let at_two = lanewise(&body(first(2), &reads_ax));
        assert!(_lanes(regs, RegId::AX).is_subset(&at_two.leaving[&1]), "premise: AX is read through block 2");
        // Block 2 changed alone: the same edges.
        let reads_bx: Block = LirBlock { succ: vec![2].into(), ..LirBlock::new(2, vec![mov(2, DX, bx)]) }.into();
        lanewise(&body(first(2), &reads_bx));
        assert_eq!(graphs() - made, 1, "an instruction changed and no edge: the graph is kept");
        // Block 1 goes to block 3 instead.
        let at_three = lanewise(&body(first(3), &reads_ax));
        assert_eq!(graphs() - made, 2, "an edge changed: the graph is made again");
        assert!(_lanes(regs, RegId::AX).is_disjoint(&at_three.leaving[&1]), "AX is read only through block 2");
        assert!(_lanes(regs, RegId::CX).is_subset(&at_three.leaving[&1]), "CX is read through block 3");
        assert!(at_three.leaving == from_scratch(&body(first(3), &reads_ax)));
    }
}
