//! Whether a loop is worth copying out completely, decided before anything
//! is cloned: llrm-core's `analysis/peelsize.rs`, the port of
//! `qbopt/analysis/peelsize.py`, adapted to the rich MIR.
//! LLVM: LoopUnrollPass's full-unroll cost model, `analyzeLoopUnrollCost` and
//! `shouldFullUnroll`.
//!
//! The decision is GCC's `try_unroll_loop_completely`
//! (gcc/tree-ssa-loop-ivcanon.cc). The size it is given is LLVM's: each
//! iteration is run over the values it knows, what folds is free, and only
//! the successors a folded branch leaves are followed. The budgets count
//! MIR instructions, as GCC's `--param`s count insns, so no machine number
//! enters.
//!
//! What changed with the IR: a branch is decided by the bit consts folds
//! its `icmp` to, so a compare is free exactly where it folds. Dropped: the
//! carry between word halves (`_carry`), which MIR does not have.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::facts::Facts;
use llrm_mir::module::{InstId, Instruction, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_support::hash::IndexMap;
use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::cfg;
use crate::consts::{self, Calls, Cells, Known};
use crate::graph::loops::Loop;
use crate::induction::{self, AffineOperand};
use crate::memory::{self, Unit};

/// GCC's `--param max-peel-branches`: undecided branches a copied sequence may
/// hold.
const MAX_PEEL_BRANCHES: i64 = 32;

/// GCC's `optimize_loop_nest_for_speed_p`: a loop entered less often than this,
/// in percent of its function's entries, is cold, and no copy of it grows the
/// code.
const COLD_PERCENT: i64 = 5;

/// What one entry of a function weighs in `entries`: `profit::UNIT`.
pub const ENTRY: i64 = 256;

/// What the loop's surroundings say, which the loop alone does not.
#[derive(Clone, Copy, Debug)]
pub struct Site {
    /// How often the loop is entered for each entry of its function, in 256ths.
    pub entries: i64,
    /// The loop calls a function that may touch memory.
    pub writes: bool,
}

impl Default for Site {
    fn default() -> Self {
        Self { entries: ENTRY, writes: false }
    }
}

/// LLVM's `-unroll-threshold` for a loop the language marks (`#pragma unroll`):
/// operations a copy the language asked for may hold, past the size budget.
const HINTED_OPERATIONS: i64 = 16384;

/// LLVM's `-unroll-max-percent-threshold-boost`: how far saved work may raise
/// the budget.
const MAX_PERCENT_THRESHOLD_BOOST: i64 = 400;

/// GCC's copy budgets, independent of the CPU. `grows: false` is -Os's
/// `UL_NO_GROWTH`: a copy is taken only when it is no larger.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Limits {
    /// GCC's `max-completely-peel-times` (16; LLVM's
    /// `-unroll-max-iteration-count-to-analyze` is 10); 0 is unbounded.
    pub max_unroll_iterations: i64,
    /// `--param max-completely-peeled-insns`; 0 is unbounded. Where
    /// `target_percent` is set, the target's own
    /// (`OperationCosts::unroll_budget`) in that percent replaces it.
    pub max_unrolled_operations: i64,
    /// What share of the target's `unroll_budget` the budget is, in percent:
    /// 100, and -O3's 200 (LLVM's 300 over 150).
    /// 0: `max_unrolled_operations` as it stands.
    pub target_percent: i64,
    pub grows: bool,
    /// With `grows`, the share of the loop's clocks, in percent, a growing copy
    /// must remove: a copy that keeps the loop's work (a float body, a divide)
    /// saves its overhead only, and not worth the bytes. Ours: neither GCC nor
    /// LLVM has it, LLVM's size threshold is the cap. 0: none.
    pub saved_percent: i64,
    /// Clocks an inline that grows the code must save for each byte it adds:
    /// `--clocks-per-byte`.
    pub milliclocks_per_byte: i64,
}

impl Limits {
    /// These limits on a target whose description states `unroll_budget`.
    pub fn on(
        &self,
        unroll_budget: i64,
    ) -> Self {
        if self.target_percent > 0 && unroll_budget > 0 {
            Self { max_unrolled_operations: unroll_budget * self.target_percent / 100, ..self.clone() }
        } else {
            self.clone()
        }
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_unroll_iterations: 16,
            max_unrolled_operations: 200,
            target_percent: 0,
            grows: true,
            saved_percent: 0,
            milliclocks_per_byte: 16_000,
        }
    }
}

/// Whether copying `loop_` out `count` times pays: GCC's
/// `try_unroll_loop_completely`.
///
/// Past `max-completely-peel-times` iterations nothing is copied, however small
/// the copy would settle: building it is the cost. A copy no larger than the
/// loop always pays. Otherwise GCC refuses growth under -Os, past
/// `max-peel-branches` undecided branches, and past
/// `max-completely-peeled-insns` instructions -- a budget raised, as LLVM's
/// `shouldFullUnroll` raises it, by the share of the rolled work the copy no
/// longer does (`getFullUnrollBoostingFactor`). A loop holding another is
/// copied only when that shrinks it, as GCC does for outer loops.
///
/// `site` is where the loop stands: GCC refuses a growing copy of a loop with a
/// call that touches memory, too, as little is left to fold.
///
/// GCC also refuses a call on the path, guessing little is left to fold; the
/// simulation measures what folds, so a call is priced as LLVM's cost model
/// prices one instead.
pub fn admitted(
    unit: &Unit,
    loop_: &Loop,
    count: &BigInt,
    facts: &IndexMap<ValueId, Known>,
    limits: &Limits,
    site: Site,
    price: &dyn Fn(InstId) -> i64,
) -> bool {
    // What the language says of copying this loop: never, or as many as it
    // permits, which at least the trip count is asked, and is then copied
    // past the budget. Fewer than the trip count is no partial unrolling,
    // which does not exist here: it is a refusal.
    let stated = loop_
        .latches
        .iter()
        .filter_map(|&latch| unit.function.terminator(cfg::block(latch)))
        .filter_map(|branch| Facts::of_terminator(unit.context, unit.metadata, unit.function, branch).unroll())
        .min();
    let asked = match stated {
        Some(0) => {
            llrm_support::debug!("unroll", "loop b{} x{count}: refused: the language says never", loop_.header);
            return false;
        }
        Some(copies) if BigInt::from(copies) < *count => {
            llrm_support::debug!(
                "unroll",
                "loop b{} x{count}: refused: the language permits only {copies} copies",
                loop_.header
            );
            return false;
        }
        Some(_) => true,
        None => false,
    };
    if !asked && limits.max_unroll_iterations != 0 && *count > BigInt::from(limits.max_unroll_iterations) {
        llrm_support::debug!("unroll", "loop b{} x{count}: refused: max-completely-peel-times", loop_.header);
        return false;
    }
    let (size, folded) = _sizes(unit, loop_, facts);
    let graph = cfg::graph(unit.function);
    let blocks = graph
        .iter()
        .filter(|block| loop_.body.contains(&block.at))
        .map(|block| (block.at, block))
        .collect::<BTreeMap<i64, &cfg::Block>>();
    let (Some(order), Some(count)) = (_ordered(&blocks, loop_.header), count.to_i64()) else {
        let shrinks = asked || count * BigInt::from(size - folded) <= BigInt::from(size);
        llrm_support::debug!(
            "unroll",
            "loop b{} x{count}: holds a loop, {size} ops, {}",
            loop_.header,
            if shrinks { "shrinks" } else { "refused: not innermost and code would grow" }
        );
        return shrinks;
    };
    let budget = if limits.max_unrolled_operations == 0 { i64::MAX } else { limits.max_unrolled_operations };
    let limit = if asked { HINTED_OPERATIONS } else { budget.saturating_mul(MAX_PERCENT_THRESHOLD_BOOST) / 100 };
    let Some(unrolled) = unrolled(unit, &blocks, &order, loop_, count, facts, limit.max(size), price) else {
        llrm_support::debug!(
            "unroll",
            "loop b{} x{count}: {size} ops, refused: over {} ops unrolled",
            loop_.header,
            limit.max(size)
        );
        return false;
    };
    let boost = _boost(&unrolled);
    // GCC's `estimated_unrolled_size` takes two thirds of the copies' size, for
    // what later passes still remove from them.
    let estimate = (unrolled.size * 2 / 3).max(1);
    // LLVM's `UnrolledCost` has no discount, and is what a level that asks for
    // a share of the loop's clocks is held to.
    let counted = if limits.saved_percent > 0 { unrolled.size } else { estimate };
    // GCC's reasons, in its order.
    let refusal = if asked || estimate <= size {
        None
    } else if !limits.grows {
        Some("size would grow")
    } else if limits.saved_percent * unrolled.rolled_clocks > 100 * (unrolled.rolled_clocks - unrolled.kept_clocks) {
        Some("saves too little of the loop's clocks")
    } else if site.entries * 100 < COLD_PERCENT * ENTRY {
        Some("cold")
    } else if site.writes {
        Some("a call that touches memory")
    } else if unrolled.branches > MAX_PEEL_BRANCHES {
        Some("max-peel-branches")
    } else if counted > budget.saturating_mul(boost) / 100 {
        Some("max-completely-peeled-insns")
    } else {
        None
    };
    llrm_support::debug!(
        "unroll",
        "loop b{} x{count}: {size} ops -> {} unrolled ({} rolled, boost {boost}%, {} branches), scaled {estimate}, budget {budget}, {}% of {} clocks saved, {}",
        loop_.header,
        unrolled.size,
        unrolled.rolled,
        unrolled.branches,
        100 * (unrolled.rolled_clocks - unrolled.kept_clocks) / unrolled.rolled_clocks.max(1),
        unrolled.rolled_clocks,
        refusal.map_or("admitted".to_owned(), |why| format!("refused: {why}"))
    );
    refusal.is_none()
}

/// LLVM's `getFullUnrollBoostingFactor`: the rolled work per unrolled
/// operation, in percent, capped.
fn _boost(unrolled: &Unrolled) -> i64 {
    if unrolled.size == 0 {
        return MAX_PERCENT_THRESHOLD_BOOST;
    }
    (100 * unrolled.rolled / unrolled.size).min(MAX_PERCENT_THRESHOLD_BOOST)
}

/// A complete copy's size, simulated rather than guessed.
struct Unrolled {
    /// Instructions no iteration folds, over every iteration.
    size: i64,
    /// Conditional branches no iteration decides, over every iteration.
    branches: i64,
    /// Instructions the rolled loop executes over every iteration: LLVM's
    /// `RolledDynamicCost`.
    rolled: i64,
    /// `rolled` and `size` in the target's clocks.
    rolled_clocks: i64,
    kept_clocks: i64,
}

/// The value a phi takes from the block `from`.
fn _incoming(
    phi: &Instruction,
    from: impl Fn(i64) -> bool,
) -> Option<Operand> {
    phi.operands.chunks(2).find(|arm| matches!(arm[1], Operand::Block(block) if from(cfg::id(block)))).map(|arm| arm[0])
}

/// LLVM's `analyzeLoopUnrollCost`: run each of `count` iterations over the
/// values it knows and the memory it has written, count what does not fold, and
/// follow only the successors a folded branch leaves. `None` once more than
/// `limit` instructions remain, where LLVM bails out too.
fn unrolled(
    unit: &Unit,
    blocks: &BTreeMap<i64, &cfg::Block>,
    order: &[i64],
    loop_: &Loop,
    count: i64,
    facts: &IndexMap<ValueId, Known>,
    limit: i64,
    price: &dyn Fn(InstId) -> i64,
) -> Option<Unrolled> {
    let latch = *loop_.latches.first()?;
    let function = unit.function;
    let calls = Calls::default();
    let mut out = Unrolled { size: 0, branches: 0, rolled: 0, rolled_clocks: 0, kept_clocks: 0 };
    let mut cells = Cells::default();
    let mut previous = facts.clone();
    for iteration in 0..count {
        let mut values = facts.clone();
        let mut came = BTreeMap::<i64, Vec<i64>>::from([(loop_.header, Vec::new())]);
        for &at in order {
            let Some(from) = came.get(&at).cloned() else {
                continue;
            };
            let block = cfg::block(at);
            for &inst in function.block(block).instructions() {
                let op = function.instruction(inst);
                if op.opcode != Opcode::Phi {
                    continue;
                }
                // The header's value comes from before the loop, then from the
                // last iteration.
                let incoming = if at == loop_.header {
                    let source = if iteration == 0 {
                        _incoming(op, |pred| !loop_.body.contains(&pred))
                    } else {
                        _incoming(op, |pred| pred == latch)
                    };
                    source.and_then(|operand| consts::_operand(unit, operand, &previous, None))
                } else if let [pred] = from.as_slice() {
                    _incoming(op, |one| one == *pred).and_then(|operand| consts::_operand(unit, operand, &values, None))
                } else {
                    None
                };
                let result = op.result.expect("a phi's value");
                match incoming {
                    Some(known) => values.insert(result, known),
                    None => values.shift_remove(&result),
                };
            }
            for &inst in function.block(block).instructions() {
                let op = function.instruction(inst);
                if op.opcode == Opcode::Phi {
                    continue;
                }
                out.rolled += _size(unit, inst);
                out.rolled_clocks += price(inst);
                // A branch is counted below, where it is decided or not.
                if matches!(op.opcode, Opcode::Br | Opcode::Switch) {
                    continue;
                }
                let defined = consts::_defined(unit, inst);
                match (defined, defined.and_then(|_| consts::_result(unit, inst, &values, Some(&cells)))) {
                    (Some(value), Some(known)) => {
                        values.insert(value, known);
                    }
                    _ => {
                        if let Some(value) = op.result {
                            values.shift_remove(&value);
                        }
                        out.size += _size(unit, inst);
                        out.kept_clocks += price(inst);
                    }
                }
                if matches!(
                    op.opcode,
                    Opcode::Store { .. } | Opcode::Call(_) | Opcode::Invoke(_)
                )
                    || memory::unmodeled_write(unit, inst)
                {
                    let mut queries = consts::memory_queries(*unit, &values);
                    cells = consts::_kills(cells, inst, &values, &calls, None, None, false, &mut queries);
                }
            }
            let known = |operand: Operand| consts::_operand(unit, operand, &values, None);
            let target = |operand: Operand| match operand {
                Operand::Block(target) => cfg::id(target),
                _ => unreachable!("a branch names blocks"),
            };
            let decided = match function.terminator(block).map(|last| function.instruction(last)) {
                Some(last) if last.opcode == Opcode::Br && last.operands.len() == 3 => known(last.operands[0])
                    .map(|bit| vec![target(last.operands[if bit.n != BigInt::from(0) { 1 } else { 2 }])]),
                Some(last) if last.opcode == Opcode::Switch => known(last.operands[0]).map(|tested| {
                    let case =
                        last.operands[2..].chunks(2).find(|arm| known(arm[0]).is_some_and(|one| one.n == tested.n));
                    vec![target(case.map_or(last.operands[1], |arm| arm[1]))]
                }),
                _ => Some(blocks[&at].succ.clone()),
            };
            let successors = decided.unwrap_or_else(|| {
                out.branches += 1;
                out.size += 1;
                out.kept_clocks += function.terminator(block).map_or(0, price);
                blocks[&at].succ.clone()
            });
            for successor in successors {
                if successor != loop_.header && loop_.body.contains(&successor) {
                    came.entry(successor).or_default().push(at);
                }
            }
            if out.size > limit {
                return None;
            }
        }
        previous = values;
    }
    Some(out)
}

/// The loop's blocks, each after every block reaching it inside one iteration;
/// `None` when the loop holds another.
fn _ordered(
    blocks: &BTreeMap<i64, &cfg::Block>,
    header: i64,
) -> Option<Vec<i64>> {
    let inner = |at: &i64| *at != header && blocks.contains_key(at);
    let mut waiting = blocks.keys().map(|at| (*at, 0)).collect::<BTreeMap<i64, usize>>();
    for block in blocks.values() {
        for successor in block.succ.iter().filter(|at| inner(at)) {
            *waiting.get_mut(successor).expect("inside") += 1;
        }
    }
    let mut ready = vec![header];
    let mut order = Vec::new();
    while let Some(at) = ready.pop() {
        order.push(at);
        for successor in blocks[&at].succ.iter().filter(|at| inner(at)) {
            let left = waiting.get_mut(successor).expect("inside");
            *left -= 1;
            if *left == 0 {
                ready.push(*successor);
            }
        }
    }
    (order.len() == blocks.len()).then_some(order)
}

/// The loop's instructions, and how many of them fold once the iteration is
/// fixed.
fn _sizes(
    unit: &Unit,
    loop_: &Loop,
    facts: &IndexMap<ValueId, Known>,
) -> (i64, i64) {
    let function = unit.function;
    let inside = function
        .walk()
        .filter(|(block, _)| loop_.body.contains(&cfg::id(*block)))
        .map(|(_, inst)| inst)
        .collect::<Vec<_>>();
    let size = inside.iter().map(|&inst| _size(unit, inst)).sum();
    let inside = inside.into_iter().map(|inst| function.instruction(inst)).collect::<Vec<_>>();
    let mut known = induction::basics(unit, loop_)
        .values()
        .filter(|one| _constant(&one.start, facts) && _constant(&one.step, facts))
        .map(|one| one.value)
        .collect::<BTreeSet<ValueId>>();
    known.extend(facts.keys().copied());
    let (phis, ops): (Vec<&Instruction>, Vec<&Instruction>) =
        inside.into_iter().partition(|op| op.opcode == Opcode::Phi);
    let mut folded = BTreeSet::<usize>::new();
    let mut changed = true;
    while changed {
        changed = false;
        for (index, op) in ops.iter().enumerate() {
            if folded.contains(&index) || !_pure(op) {
                continue;
            }
            if op.operands.iter().all(|operand| !matches!(operand, Operand::Value(value) if !known.contains(value))) {
                folded.insert(index);
                known.extend(op.result);
                changed = true;
            }
        }
    }
    let folded_phis = phis.iter().filter(|phi| phi.result.is_some_and(|value| known.contains(&value))).count();
    (size, (folded.len() + folded_phis) as i64)
}

/// What `inst` adds to a copy: LLVM's `TTI::getInstructionCost`, one for each
/// instruction, and a call lowered to one its arguments besides.
fn _size(
    unit: &Unit,
    inst: InstId,
) -> i64 {
    // A call's operands are its arguments and its callee.
    if unit.calls_out(inst) { unit.function.instruction(inst).operands.len() as i64 } else { 1 }
}

fn _constant(
    arg: &AffineOperand,
    facts: &IndexMap<ValueId, Known>,
) -> bool {
    match arg {
        AffineOperand::Const(_) => true,
        AffineOperand::Value(value, _) => facts.contains_key(value),
    }
}

/// Whether an instruction's result is a function of its operands alone.
fn _pure(op: &Instruction) -> bool {
    !matches!(
        op.opcode,
        Opcode::Load { .. }
            | Opcode::Store { .. }
            | Opcode::Call(_)
            | Opcode::Invoke(_)
            | Opcode::Alloca { .. }
            | Opcode::LandingPad { .. }
    )
}

#[cfg(test)]
#[path = "peelsize_tests.rs"]
mod tests;
