//! Full unrolling of a loop with an exact trip count of at least two: its
//! trips become one straight line in the latch and the loop is gone.
//!
//! # How it works
//! A loop qualifies when it has one latch and one entering block; its header
//! tests with a conditional `br` whose one successor is the body and the other
//! the exit; the exit has the header as its only predecessor; the body is a
//! chain of single-successor blocks from the header to the latch with no
//! phis; and every header phi has exactly the two arms, entry and latch.
//! The body and latch may not call out (`calls_out`) or touch volatile memory.
//! The header's work runs once more than the body, so it may not store, call
//! or allocate. `induction::trip_count` must prove the count, and
//! `peelsize::admitted` (in `llrm_analysis`) must price it as worth copying:
//! the budgets, the hint a language may state, and the reasons it refuses are
//! documented there.
//!
//! The first trip is the original blocks. Each later trip is a clone of the
//! header's work and of the body's, appended in the latch before its jump,
//! with operands renamed to the previous trip's values; the header phis'
//! latch arms are what carry a value from one trip to the next. A last clone
//! of the header's work supplies what the exit reads. Then the header phis are
//! replaced by their entry values and erased, the header's test becomes a
//! `br` to the body, and the latch jumps to the exit.
//!
//! ```text
//!   before                           after (count = 3)
//!   E: br H                          E: br H
//!   H: i = phi [0,E] [i1,L]          H: c0 = i0 < 3        ; dead
//!      c = i < 3                        br B
//!      br c, B, X                    B: w0 = work(i0)
//!   B: w = work(i)                   L: i1 = i0 + 1
//!   L: i1 = i + 1 ; br H                c1 = i1 < 3 ; w1 = work(i1)
//!   X: uses i                           i2 = i1 + 1
//!                                       c2 = i2 < 3 ; w2 = work(i2)
//!                                       i3 = i2 + 1 ; c3 = i3 < 3
//!                                       br X
//!                                    X: uses i3
//! ```
//!
//! Uses outside the loop are rewritten to the last trip's values. The
//! comparisons left behind are removed by `dead`, and constants that appear
//! (a copy's indices) by `fold` and the rest of the fixed point.
//!
//! A function holding an instruction the target does not price
//! (`profit::priced`) is left alone. The pass expands one loop at a time and
//! starts again, so a loop around an expanded one may follow. A loop `peelsize`
//! refused is not priced again while its blocks are unchanged.
//!
//! Loops with a branch in the body are copied by [`crate::peel`] instead; a
//! nest's outer loop can be unrolled only by [`crate::unroll_jam`].
//!
//! `LLRM_DEBUG=unroll` prints one line per loop priced (size, unrolled size,
//! boost, undecided branches, and the reason for a refusal; or "admitted")
//! and `expanded a loop` for each loop copied.
//!
//! # Runs
//! In the pipeline's fixed point, after each round of scalar passes and when
//! the body changed since the last time it found nothing. After it expands a
//! loop, `sroa` runs first (a copy's constant indices are new exact leaves),
//! then the scalar passes. On at -O1 and above and -Os; off at -Oz and by
//! `-fno-unroll-loops`. A copy may grow the code only where
//! `Limits::grows`: -O3 and -Omax (`target_percent` 200). Elsewhere a copy
//! must be no larger than the loop. See `docs/reference/levels.md`.
//!
//! # Needs
//! `induction` (trip count), `peelsize`, `consts` and `Registers`
//! (`llrm_analysis`); [`crate::profit`] for prices and the site.
//!
//! # Keeps
//! Nothing: a change invalidates every analysis.
//!
//! # See also
//! LLVM's LoopUnroll, full unroll (`UnrollLoop` at the trip count; cost model
//! `analyzeLoopUnrollCost`); gcc's `try_unroll_loop_completely`.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::graph::loops::{self, Loop};
use llrm_analysis::manager::Registers;
use llrm_analysis::peelsize::{self, Limits};
use llrm_analysis::{cfg, induction, memory};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};
use llrm_support::hash::HashMap;
use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::lcssa::{arms, from_arms, operations};
use crate::profit;

#[derive(Default)]
pub struct Unroll {
    pub limits: Limits,
}

impl FunctionPass for Unroll {
    fn name(&self) -> &'static str {
        "unroll"
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        match optimized(unit, analyses, &self.limits) {
            Ok(true) => PreservedAnalyses::none(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("unroll: {error}"),
        }
    }
}

/// Every exact loop `peelsize::admitted` prices as worth it expanded, once
/// each; whether any was.
pub fn optimized(
    unit: &mut passes::Unit,
    analyses: &Analyses,
    limits: &Limits,
) -> Result<bool, String> {
    let costs = &profit::costs(analyses.outer());
    let limits = &limits.on(costs.unroll_budget);
    if !profit::priced(unit.context, unit.layout, unit.function, analyses.outer().callees(), costs) {
        return Ok(false);
    }
    let mut changed = false;
    let mut refused = BTreeSet::new();
    while expanded(unit.context, unit.layout, unit.function, analyses, limits, &mut refused)? {
        llrm_support::debug!("unroll", "expanded a loop");
        changed = true;
    }
    Ok(changed)
}

/// A loop this can expand: its header tests, `path` runs straight from
/// `first` to the latch, and it leaves only from its header, into `exit`.
struct Shape {
    header: BlockId,
    entry: BlockId,
    latch: BlockId,
    first: BlockId,
    exit: BlockId,
    path: Vec<BlockId>,
}

// Loops `peelsize` was asked about.
#[cfg(test)]
thread_local! {
    pub(crate) static PRICED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// A loop as `refused` names it: its header, blocks and instruction count. A
/// loop `peelsize` priced out stays priced out until one of them changes, so
/// the next round, after another loop was expanded, does not simulate it again.
type Refused = BTreeSet<(i64, Vec<i64>, usize)>;

/// The first loop of the expandable shape with an exact count `peelsize`
/// admits, expanded in place; whether there was one.
pub fn expanded(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    analyses: &Analyses,
    limits: &Limits,
    refused: &mut Refused,
) -> Result<bool, String> {
    let facts = analyses.fresh().get::<Registers>(context, layout, function);
    let graph = cfg::graph(function);
    let mut found = None;
    let found_shape = cfg::Shape::of(function);
    let frequencies = profit::Frequencies::default();
    for loop_ in found_shape.loops.clone() {
        let unit = memory::Unit::within(context, layout, function, analyses.outer())
            .with_registers(&facts)
            .with_shape(&found_shape);
        let Some(shape) = _shape(&unit, &graph, &loop_) else {
            continue;
        };
        let Some(count) = induction::trip_count(&unit, &loop_, &facts) else {
            continue;
        };
        let name = (
            loop_.header,
            loop_.body.iter().copied().collect::<Vec<_>>(),
            loop_.body.iter().map(|&block| function.block(cfg::block(block)).instructions().len()).sum::<usize>(),
        );
        if count < BigInt::from(2) || refused.contains(&name) {
            continue;
        }
        #[cfg(test)]
        PRICED.with(|one| one.set(one.get() + 1));
        if !peelsize::admitted(
            &unit,
            &loop_,
            &count,
            &facts,
            limits,
            profit::site(&unit, analyses.outer(), &loop_, &frequencies),
        ) {
            refused.insert(name);
            continue;
        }
        if let Some(count) = count.to_i64() {
            found = Some((shape, loop_, count));
            break;
        }
    }
    let Some((shape, loop_, count)) = found else {
        return Ok(false);
    };
    _expanded(function, &shape, &loop_.body, count)?;
    Ok(true)
}

fn _phis(
    function: &Function,
    block: BlockId,
) -> Vec<InstId> {
    function
        .block(block)
        .instructions()
        .iter()
        .copied()
        .filter(|&inst| function.instruction(inst).opcode == Opcode::Phi)
        .collect()
}

/// A block's instructions, its phis and terminator aside.
fn _work(
    function: &Function,
    block: BlockId,
) -> Vec<InstId> {
    operations(function, block).into_iter().filter(|&inst| !function.instruction(inst).opcode.is_terminator()).collect()
}

/// Whether `inst` may run once a trip in a straight line: no control, no
/// call out, no volatile access.
fn _repeatable(
    unit: &memory::Unit,
    inst: InstId,
) -> bool {
    let op = unit.function.instruction(inst);
    !unit.calls_out(inst)
        && !matches!(
            op.opcode,
            Opcode::LandingPad { .. } | Opcode::Load { volatile: true, .. } | Opcode::Store { volatile: true, .. }
        )
        && !op.opcode.is_terminator()
}

/// `loop_`'s shape, where it is one this can expand.
fn _shape(
    unit: &memory::Unit,
    graph: &[cfg::Block],
    loop_: &Loop,
) -> Option<Shape> {
    let function = unit.function;
    let blocks = graph.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
    let predecessors = loops::predecessors(graph);
    let [latch] = loop_.latches.iter().copied().collect::<Vec<_>>()[..] else {
        return None;
    };
    let header = loop_.header;
    let [entry] = predecessors[&header].difference(&loop_.body).copied().collect::<Vec<_>>()[..] else {
        return None;
    };
    let succ = &blocks[&header].succ;
    let [exit] = succ.iter().copied().filter(|at| !loop_.body.contains(at)).collect::<Vec<_>>()[..] else {
        return None;
    };
    let [first] = succ.iter().copied().filter(|at| loop_.body.contains(at)).collect::<Vec<_>>()[..] else {
        return None;
    };
    if latch == header || predecessors[&exit] != BTreeSet::from([header]) || blocks[&latch].succ != [header] {
        return None;
    }
    let branch = function.instruction(function.terminator(cfg::block(header))?);
    if branch.opcode != Opcode::Br || branch.operands.len() != 3 {
        return None;
    }
    let bridges = loop_.body.iter().copied().filter(|at| *at != header && *at != latch).collect::<BTreeSet<_>>();
    if bridges.iter().any(|&at| blocks[&at].succ.len() != 1 || !_phis(function, cfg::block(at)).is_empty())
        || !_phis(function, cfg::block(latch)).is_empty()
    {
        return None;
    }
    let mut path = Vec::new();
    let mut at = first;
    while at != latch && !path.contains(&at) {
        path.push(at);
        at = blocks[&at].succ[0];
    }
    if at != latch || path.iter().copied().collect::<BTreeSet<_>>() != bridges {
        return None;
    }
    let repeated = path.iter().chain([&latch]).flat_map(|&at| _work(function, cfg::block(at))).collect::<Vec<_>>();
    if repeated.iter().any(|&inst| !_repeatable(unit, inst)) {
        return None;
    }
    // The header's work runs once more than the body: it may not write.
    let tested = _work(function, cfg::block(header));
    if tested.iter().any(|&inst| {
        let op = function.instruction(inst);
        !_repeatable(unit, inst)
            || matches!(
                op.opcode,
                Opcode::Store { .. } | Opcode::Call(_) | Opcode::Invoke(_) | Opcode::Alloca { .. }
            )
    }) {
        return None;
    }
    let sides = BTreeSet::from([entry, latch]);
    if _phis(function, cfg::block(header))
        .into_iter()
        .any(|phi| arms(function, phi).into_iter().map(|(_, from)| cfg::id(from)).collect::<BTreeSet<_>>() != sides)
    {
        return None;
    }
    let block = cfg::block;
    Some(Shape {
        header: block(header),
        entry: block(entry),
        latch: block(latch),
        first: block(first),
        exit: block(exit),
        path: path.into_iter().map(block).collect(),
    })
}

/// `operand`, as the trip `swap` describes reads it.
fn _provided(
    operand: Operand,
    swap: &HashMap<ValueId, Operand>,
) -> Operand {
    match operand {
        Operand::Value(value) => swap.get(&value).copied().unwrap_or(operand),
        other => other,
    }
}

/// The phi's value from `from`.
fn _from(
    function: &Function,
    phi: InstId,
    from: BlockId,
) -> Operand {
    arms(function, phi).into_iter().find(|&(_, source)| source == from).expect("an arm from each side").0
}

/// `count` trips of `shape`'s loop, straight in its latch.
fn _expanded(
    function: &mut Function,
    shape: &Shape,
    body: &BTreeSet<i64>,
    count: i64,
) -> Result<(), String> {
    let header_phis = _phis(function, shape.header);
    let header_work = _work(function, shape.header);
    let repeated = shape.path.iter().chain([&shape.latch]).flat_map(|&at| _work(function, at)).collect::<Vec<_>>();
    let jump = function.terminator(shape.latch).expect("a terminated latch");
    let branch = function.terminator(shape.header).expect("a terminated header");
    let value = |function: &Function, inst: InstId| function.instruction(inst).result;

    let initial = header_phis
        .iter()
        .map(|&phi| (value(function, phi).expect("a phi's value"), _from(function, phi, shape.entry)))
        .collect::<HashMap<_, _>>();
    let mut swap = initial.clone();
    let clone = |function: &mut Function, inst: InstId, swap: &mut HashMap<ValueId, Operand>| -> Result<(), String> {
        let copy = function.clone_instruction(inst);
        let operands = function.instruction(inst).operands.iter().map(|&operand| _provided(operand, swap)).collect();
        function.set_operands(copy, operands);
        function.insert(copy, Position::Before(jump))?;
        if let (Some(original), Some(fresh)) = (value(function, inst), value(function, copy)) {
            swap.insert(original, Operand::Value(fresh));
        }
        Ok(())
    };
    for trip in 0..count {
        // The first trip is the original blocks.
        if trip != 0 {
            for &inst in header_work.iter().chain(&repeated) {
                clone(function, inst, &mut swap)?;
            }
        }
        let carried = header_phis
            .iter()
            .map(|&phi| {
                (value(function, phi).expect("a phi's value"), _provided(_from(function, phi, shape.latch), &swap))
            })
            .collect::<Vec<_>>();
        swap.extend(carried);
    }
    // The test that leaves, once more: what the exit reads.
    for &inst in &header_work {
        clone(function, inst, &mut swap)?;
    }

    let header_values =
        header_phis.iter().chain(&header_work).filter_map(|&inst| value(function, inst)).collect::<Vec<_>>();
    for &defined in &header_values {
        for one in function.users(defined).to_vec() {
            let outside = function.parent(one.user).is_some_and(|block| !body.contains(&cfg::id(block)));
            if outside {
                function.set_operand(one.user, one.index as usize, _provided(Operand::Value(defined), &swap));
            }
        }
    }
    for phi in _phis(function, shape.exit) {
        let incoming = arms(function, phi).into_iter().map(|(value, _)| (value, shape.latch)).collect::<Vec<_>>();
        function.set_operands(phi, from_arms(&incoming));
    }
    for &phi in &header_phis {
        function.set_operands(phi, Vec::new());
    }
    for &phi in &header_phis {
        let defined = value(function, phi).expect("a phi's value");
        function.replace_all_uses_with(defined, initial[&defined]);
        function.erase(phi)?;
    }
    function.set_operands(branch, vec![Operand::Block(shape.first)]);
    function.set_operands(jump, vec![Operand::Block(shape.exit)]);
    Ok(())
}

#[cfg(test)]
#[path = "unroll_tests.rs"]
mod tests;
