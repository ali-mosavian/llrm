//! Bounded full unrolling of small exact-trip loops: llrm-core's
//! `optimize/unroll.rs`, the port of `qbopt/optimize/unroll.py`, adapted to
//! the rich MIR.
//! LLVM: LoopUnroll's full unroll (`UnrollLoop` at the trip count), priced by its cost model, `analyzeLoopUnrollCost`.
//!
//! A loop tested in its header, whose body is a chain of blocks to its
//! latch, becomes one straight line in the latch: the first trip runs the
//! original blocks, each later trip is a copy of the header's work and the
//! body's, and a last copy of the header's work supplies what leaves. The
//! header's test is gone; Dead takes the compares left behind.
//!
//! The copy budgets are peelsize's `Limits`, which count instructions. The
//! one machine number is `costs`: a function with an instruction the target
//! does not price is left alone (`profit::priced`). The target in `outer`
//! carries no costs yet, so they are the pass's, as `Gvn`'s and `Unswitch`'s.
//!
//! What changed with the IR: an expanded value is a clone with a fresh
//! result, and its origin the `Cloned` change, so the old fresh ids, byte
//! ownership (`absorbed`, `symbol`, `raised`), pointer and integer-range
//! side tables are gone; a use outside the loop is rewritten by its block,
//! not by dominance. The header's test and the latch's jump are a `br`
//! each, so the old frontends' explicit and implicit back edges are one.
//! Dropped: `repetitions` (the old body's record of each expansion, for
//! nothing here to read), and the floating-loop rules -- the x87 header
//! contract and `floatfacts`' exactness check on more than four trips; a
//! float is an ordinary value, so a floating loop meets the integer rules.
//! The stage records and `watch` hook are the pass manager's dump.
//!
//! Tests, in `unroll_tests.rs`, are new: the old ones read BC fixtures
//! (FPDEEP, FPCSE) through the whole pipeline, or tested DSE's x87
//! checkpoints. `unroll_budget_tests.rs` was `profit::spill_risk`'s, in
//! `profit_tests.rs`.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::manager::Registers;
use llrm_analysis::peelsize::{self, Limits};
use llrm_analysis::{cfg, induction, memory};
use llrm_graph::loops::{self, Loop};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Instruction, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};
use llrm_support::hash::HashMap;
use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::lcssa::{arms, from_arms, operations};
use crate::profit::{self, OperationCosts};

#[derive(Default)]
pub struct Unroll {
    pub costs: OperationCosts,
    pub limits: Limits,
}

impl FunctionPass for Unroll {
    fn name(&self) -> &'static str {
        "unroll"
    }

    fn run(&mut self, unit: &mut passes::Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        match optimized(unit, analyses, &self.costs, &self.limits) {
            Ok(true) => PreservedAnalyses::none(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("unroll: {error}"),
        }
    }
}

/// Every exact loop `peelsize::admitted` prices as worth it expanded, once
/// each; whether any was.
pub fn optimized(unit: &mut passes::Unit, analyses: &Analyses, costs: &OperationCosts, limits: &Limits) -> Result<bool, String> {
    if !profit::priced(unit.context, unit.function, unit.callees, costs) {
        return Ok(false);
    }
    let mut changed = false;
    while expanded(unit.context, unit.layout, unit.function, analyses, limits)? {
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

/// The first loop of the expandable shape with an exact count `peelsize`
/// admits, expanded in place; whether there was one.
pub fn expanded(context: &Context, layout: &DataLayout, function: &mut Function, analyses: &Analyses, limits: &Limits) -> Result<bool, String> {
    let facts = analyses.fresh().get::<Registers>(context, layout, function);
    let graph = cfg::graph(function);
    let mut found = None;
    for loop_ in loops::loops(&graph, function.entry().map(cfg::id)) {
        let Some(shape) = _shape(function, &graph, &loop_) else {
            continue;
        };
        let unit = memory::Unit::within(context, layout, function, analyses.outer());
        let Some(count) = induction::trip_count(&unit, &loop_, &facts) else {
            continue;
        };
        if count < BigInt::from(2) || !peelsize::admitted(&unit, &loop_, &count, &facts, limits) {
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

fn _phis(function: &Function, block: BlockId) -> Vec<InstId> {
    function.block(block).instructions().iter().copied().filter(|&inst| function.instruction(inst).opcode == Opcode::Phi).collect()
}

/// A block's instructions, its phis and terminator aside.
fn _work(function: &Function, block: BlockId) -> Vec<InstId> {
    operations(function, block).into_iter().filter(|&inst| !function.instruction(inst).opcode.is_terminator()).collect()
}

/// Whether `op` may run once a trip in a straight line: no control, no
/// call, no volatile access.
fn _repeatable(op: &Instruction) -> bool {
    !matches!(
        op.opcode,
        Opcode::Call(_) | Opcode::LandingPad { .. } | Opcode::Load { volatile: true, .. } | Opcode::Store { volatile: true, .. }
    ) && !op.opcode.is_terminator()
}

/// `loop_`'s shape, where it is one this can expand.
fn _shape(function: &Function, graph: &[cfg::Block], loop_: &Loop) -> Option<Shape> {
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
    if bridges.iter().any(|&at| blocks[&at].succ.len() != 1 || !_phis(function, cfg::block(at)).is_empty()) || !_phis(function, cfg::block(latch)).is_empty() {
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
    if repeated.iter().any(|&inst| !_repeatable(function.instruction(inst))) {
        return None;
    }
    // The header's work runs once more than the body: it may not write.
    let tested = _work(function, cfg::block(header));
    if tested.iter().any(|&inst| {
        let op = function.instruction(inst);
        !_repeatable(op) || matches!(op.opcode, Opcode::Store { .. } | Opcode::Invoke(_) | Opcode::Alloca { .. })
    }) {
        return None;
    }
    let sides = BTreeSet::from([entry, latch]);
    if _phis(function, cfg::block(header)).into_iter().any(|phi| arms(function, phi).into_iter().map(|(_, from)| cfg::id(from)).collect::<BTreeSet<_>>() != sides) {
        return None;
    }
    let block = cfg::block;
    Some(Shape { header: block(header), entry: block(entry), latch: block(latch), first: block(first), exit: block(exit), path: path.into_iter().map(block).collect() })
}

/// `operand`, as the trip `swap` describes reads it.
fn _provided(operand: Operand, swap: &HashMap<ValueId, Operand>) -> Operand {
    match operand {
        Operand::Value(value) => swap.get(&value).copied().unwrap_or(operand),
        other => other,
    }
}

/// The phi's value from `from`.
fn _from(function: &Function, phi: InstId, from: BlockId) -> Operand {
    arms(function, phi).into_iter().find(|&(_, source)| source == from).expect("an arm from each side").0
}

/// `count` trips of `shape`'s loop, straight in its latch.
fn _expanded(function: &mut Function, shape: &Shape, body: &BTreeSet<i64>, count: i64) -> Result<(), String> {
    let header_phis = _phis(function, shape.header);
    let header_work = _work(function, shape.header);
    let repeated = shape.path.iter().chain([&shape.latch]).flat_map(|&at| _work(function, at)).collect::<Vec<_>>();
    let jump = function.terminator(shape.latch).expect("a terminated latch");
    let branch = function.terminator(shape.header).expect("a terminated header");
    let value = |function: &Function, inst: InstId| function.instruction(inst).result;

    let initial = header_phis.iter().map(|&phi| (value(function, phi).expect("a phi's value"), _from(function, phi, shape.entry))).collect::<HashMap<_, _>>();
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
            .map(|&phi| (value(function, phi).expect("a phi's value"), _provided(_from(function, phi, shape.latch), &swap)))
            .collect::<Vec<_>>();
        swap.extend(carried);
    }
    // The test that leaves, once more: what the exit reads.
    for &inst in &header_work {
        clone(function, inst, &mut swap)?;
    }

    let header_values = header_phis.iter().chain(&header_work).filter_map(|&inst| value(function, inst)).collect::<Vec<_>>();
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
