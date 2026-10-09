//! Loop stores nothing inside the loop observes, sunk to its single exit:
//! LICM's store sinking, as `promoteLoopAccessesToScalars` leaves one store
//! after the loop. Adapted from llrm-core's `optimize/loopmotion.rs`, the
//! port of `qbopt/optimize/loopmotion.py`.
//!
//! Whether two accesses meet is `regions::overlapping` of the references
//! and call writes `memoryssa::Accesses` holds, asked again after each loop
//! that changed: a moved store's narrowing was its old block's.
//!
//! What changed with the IR:
//! - A store is moved, not copied; one that goes with another value has it as
//!   its operand. An address is an operand too, and must reach the exit.
//! - What refuses a loop is what `llrm_mir::memory` says is more than a value,
//!   a store aside: a volatile access (the old barrier), a call that may touch
//!   memory, an `invoke` (a raise a handler here observes). The old refused
//!   every call; one with no effect that returns is a value.
//! - A stored cell is in an object (`MemRef::object`), as the old `Segment` and
//!   `Frame` spaces were, with no selector.
//! - `root` followed copies, which have no instruction.
//!
//! Dropped, no rich MIR analogue: the `excludes` an indexed store had to
//! carry (a hole of the x86 region lattice), and the per-block intervals
//! `sunk_stores` built, which alias's `annotated` now applies.
//!
//! llrm-mir's `licm` hoists invariant instructions and loads to the
//! preheader; it sinks no store.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::consts::{self, Calls, Known, masked};
use llrm_analysis::graph::loops::{self, Loop};
use llrm_analysis::manager::{
    Annotated, AssumptionCache, Bounded, Counted, DominatedEdges, ExposedFrames, MemoryCells, Registers,
};
use llrm_analysis::memory::{MemRef, Unit};
use llrm_analysis::memoryssa::Accesses;
use llrm_analysis::{cfg, induction, regions};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::memory::{self, Callees};
use llrm_mir::module::{Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses};
use llrm_support::hash::{HashMap, IndexMap};
use num_bigint::BigInt;

use crate::counting;
use crate::edges;
use crate::lcssa::{arms, operations};

pub struct LoopMotion;

impl FunctionPass for LoopMotion {
    fn name(&self) -> &'static str {
        "loopmotion"
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        match sunk_stores(unit.context, unit.layout, outer.callees(), unit.function, analyses) {
            Ok(true) => kept_when_stores_move(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("loopmotion: {error}"),
        }
    }
}

/// What a store moved from a loop to its exit leaves: the blocks, the values
/// and the facts about them, not the memory. The points-to solve is not kept:
/// `LLRM_CHECK_PRESERVED` finds it differs after a store takes the header's phi
/// as its value.
fn kept_when_stores_move() -> PreservedAnalyses {
    PreservedAnalyses::none()
        .preserve::<ExposedFrames>()
        .preserve::<Annotated>()
        .preserve::<Dominators>()
        .preserve::<Loops>()
        .preserve::<Registers>()
        .preserve::<AssumptionCache>()
        .preserve::<Counted>()
        .preserve::<DominatedEdges>()
        .preserve::<Bounded>()
}

/// Each loop's unobserved stores moved to the front of its one exit;
/// whether any moved.
pub fn sunk_stores(
    context: &mut Context,
    layout: &DataLayout,
    callees: &Callees,
    function: &mut Function,
    analyses: &mut Analyses,
) -> Result<bool, String> {
    let graph = cfg::graph(function);
    let predecessors = loops::predecessors(&graph);
    let successors = graph.iter().map(|block| (block.at, block.succ.clone())).collect::<BTreeMap<_, _>>();
    let shape = cfg::Shape::of(function);
    let dominators = shape.dominance.dominators(function);
    let mut changed = false;
    for loop_ in shape.loops {
        let mut exits = Vec::new();
        for &at in &loop_.body {
            for &to in &successors[&at] {
                if !loop_.body.contains(&to) && !exits.contains(&(at, to)) {
                    exits.push((at, to));
                }
            }
        }
        let [(source, destination)] = exits[..] else { continue };
        if predecessors.get(&destination) != Some(&BTreeSet::from([source]))
            || loop_.body.iter().any(|at| successors[at].is_empty())
        {
            continue;
        }
        let inside = loop_
            .body
            .iter()
            .flat_map(|&at| function.block(cfg::block(at)).instructions().to_vec())
            .collect::<Vec<_>>();
        let refused = |inst: InstId| match function.instruction(inst).opcode {
            Opcode::Store { volatile, .. } => volatile,
            Opcode::Br | Opcode::Switch => false,
            _ => !memory::only_value(context, callees, function, inst),
        };
        if inside.iter().any(|&inst| refused(inst)) {
            continue;
        }
        let accesses = Accesses::managed(context, layout, function, analyses)?;
        let registers = analyses.get::<llrm_analysis::manager::Registers>(context, layout, function);
        let shape = analyses.get::<llrm_analysis::cfg::Shape>(context, layout, function);
        let outer = std::rc::Rc::clone(analyses.outer());
        let unit = Unit::within(context, layout, function, &outer).with_registers(&registers).with_shape(&shape);
        let moved =
            _moved(&unit, analyses, &accesses, &loop_, &inside, &predecessors, &successors, &dominators, source)?;
        if moved.is_empty() {
            continue;
        }
        let anchor = operations(function, cfg::block(destination))[0];
        for (store, value) in moved {
            let value = match value {
                Some(Stored::Operand(value)) => value,
                Some(Stored::Number(number)) => {
                    let stored = function
                        .operand_type(context, function.instruction(store).operands[0])
                        .expect("a stored value");
                    let width = context.types.int_bits(stored).expect("an integer counter");
                    counting::constant(context, &number, width)
                }
                None => function.instruction(store).operands[0],
            };
            function.set_operand(store, 0, value);
            function.move_to(store, Position::Before(anchor))?;
        }
        // What alias said of the old placement no longer holds; no value, block
        // or edge changed, so what is said of them does.
        analyses.invalidate(&kept_when_stores_move());
        changed = true;
    }
    Ok(changed)
}

/// What a moved store writes in place of its own value.
enum Stored {
    Operand(Operand),
    Number(BigInt),
}

/// The stores of one loop that go to its exit, each with the value it then
/// stores where that is not its own.
#[allow(clippy::too_many_arguments)]
fn _moved(
    unit: &Unit,
    analyses: &mut Analyses,
    accesses: &Accesses,
    loop_: &Loop,
    operations: &[InstId],
    predecessors: &BTreeMap<i64, BTreeSet<i64>>,
    successors: &BTreeMap<i64, Vec<i64>>,
    dominators: &BTreeMap<i64, BTreeSet<i64>>,
    source: i64,
) -> Result<Vec<(InstId, Option<Stored>)>, String> {
    let function = unit.function;
    let references = &accesses.references;
    let empty = BTreeSet::new();
    let header_dominators = dominators.get(&loop_.header).unwrap_or(&empty);
    let defined_in = |value: ValueId| match function.value(value).def {
        ValueDef::Instruction(inst) => function.parent(inst).map(cfg::id),
        ValueDef::Argument(_) => None,
    };
    let address_values = |value: ValueId| {
        defined_in(value).is_none_or(|at| !loop_.body.contains(&at) && header_dominators.contains(&at))
    };
    // What a moved store reads must reach the exit, whose one way in is
    // `source`.
    let reaches = |operand: Operand| match operand {
        Operand::Value(value) => defined_in(value)
            .is_none_or(|at| dominators.get(&source).is_some_and(|dominating| dominating.contains(&at))),
        _ => true,
    };
    // What the loop's accesses reach, found by object: asked of each store.
    let mut index = llrm_analysis::regions::Index::default();
    for &inst in operations {
        if let Some(reference) = references.get(&inst) {
            index.push(inst, reference);
        }
    }
    let unobserved = |inst: InstId| _unobserved(unit, inst, &index, references, &address_values);
    let mut moved = Vec::new();
    for &inst in function.block(cfg::block(source)).instructions() {
        if unobserved(inst) && function.instruction(inst).operands.iter().all(|&one| reaches(one)) {
            moved.push((inst, None));
        }
    }
    let Some(&latch) = loop_.latches.first() else { return Ok(moved) };
    let outside = predecessors[&source].difference(&loop_.body).copied().collect::<Vec<_>>();
    if loop_.latches.len() != 1
        || source != loop_.header
        || outside.len() != 1
        || latch == source
        || successors[&latch] != [source]
    {
        return Ok(moved);
    }
    let entry = outside[0];
    let nonempty = induction::nonempty(unit, loop_);
    let invariant =
        if nonempty { induction::invariant(function, &loop_.body) } else { induction::Invariant::default() };
    let mut exit =
        _Exit { unit, accesses, predecessors, entry: cfg::id(function.entry().expect("an entry")), analyses };
    for &inst in operations {
        if moved.iter().any(|(one, _)| *one == inst)
            || !unobserved(inst)
            || !reaches(function.instruction(inst).operands[1])
        {
            continue;
        }
        let reference = &references[&inst];
        let mut value = exit
            .exit_value(inst, reference, loop_.header, entry, latch)?
            .map(|phi| Stored::Operand(Operand::Value(phi)));
        if value.is_none() && function.parent(inst) == Some(cfg::block(latch)) {
            value = _invariant_value(function, inst, &invariant, nonempty)
                .map(Stored::Operand)
                .or_else(|| _last_counter_value(unit, inst, loop_).map(Stored::Number));
        }
        if let Some(value) = value {
            moved.push((inst, Some(value)));
        }
    }
    Ok(moved)
}

/// The counter's value on the last trip, where the latch stores the counter.
fn _last_counter_value(
    unit: &Unit,
    inst: InstId,
    loop_: &Loop,
) -> Option<BigInt> {
    let Operand::Value(stored) = unit.function.instruction(inst).operands[0] else { return None };
    let counter = induction::basics(unit, loop_).get(&stored)?.clone();
    let proof = induction::controlling(unit, loop_, &counter, &unit.registers())?;
    Some(masked(proof.last.as_ref()?, counter.start.width()))
}

/// The stored value, where the loop runs and does not change it.
fn _invariant_value(
    function: &Function,
    inst: InstId,
    invariant: &induction::Invariant,
    nonempty: bool,
) -> Option<Operand> {
    let stored = function.instruction(inst).operands[0];
    (nonempty && invariant.operand(stored)).then_some(stored)
}

/// What `_exit_value` closes over.
struct _Exit<'a> {
    unit: &'a Unit<'a>,
    accesses: &'a Accesses,
    predecessors: &'a BTreeMap<i64, BTreeSet<i64>>,
    entry: i64,
    analyses: &'a mut Analyses,
}

impl _Exit<'_> {
    /// A header phi the stored cell holds on every trip: the value the
    /// store may write once the loop is left.
    fn exit_value(
        &mut self,
        inst: InstId,
        reference: &MemRef,
        header: i64,
        entry: i64,
        latch: i64,
    ) -> Result<Option<ValueId>, String> {
        let function = self.unit.function;
        if !matches!(function.instruction(inst).operands[0], Operand::Value(_)) {
            return Ok(None);
        }
        for phi in edges::phis(function, cfg::block(header)) {
            let incoming = arms(function, phi);
            if incoming.iter().map(|(_, block)| cfg::id(*block)).collect::<BTreeSet<_>>()
                != BTreeSet::from([entry, latch])
            {
                continue;
            }
            let from = |at: i64| incoming.iter().find(|(_, block)| cfg::id(*block) == at).expect("an arm").0;
            if self.stored_at(reference, entry, from(entry)) && self.stored_at(reference, latch, from(latch)) {
                return Ok(function.instruction(phi).result);
            }
        }
        Ok(None)
    }

    /// Whether `reference` holds `expected` as block `at` ends, on every
    /// path into it.
    fn stored_at(
        &mut self,
        reference: &MemRef,
        at: i64,
        expected: Operand,
    ) -> bool {
        self.stored_on(reference, at, expected, &[], &mut HashMap::default()).0
    }

    /// `stored_at`, and the shallowest query on the path it assumed true,
    /// as Tarjan's lowlink. A block's answer is asked once, or a run of
    /// if/else is 2^n paths: a false one is final, and so is a true one
    /// assuming nothing shallower than itself.
    fn stored_on(
        &mut self,
        reference: &MemRef,
        at: i64,
        expected: Operand,
        active: &[(i64, Operand)],
        known: &mut HashMap<(i64, Operand), bool>,
    ) -> (bool, usize) {
        let key = (at, expected);
        if let Some(depth) = active.iter().position(|one| *one == key) {
            // inductive backedge; every entry path still needs a matching store
            return (true, depth);
        }
        if let Some(&answer) = known.get(&key) {
            return (answer, usize::MAX);
        }
        let (answer, lowest) = self.stored_in(reference, at, expected, active, known);
        if !answer || lowest >= active.len() {
            known.insert(key, answer);
            return (answer, usize::MAX);
        }
        (answer, lowest)
    }

    fn stored_in(
        &mut self,
        reference: &MemRef,
        at: i64,
        expected: Operand,
        active: &[(i64, Operand)],
        known: &mut HashMap<(i64, Operand), bool>,
    ) -> (bool, usize) {
        let key = (at, expected);
        let unit = self.unit;
        let function = unit.function;
        let block = cfg::block(at);
        for &inst in operations(function, block).iter().rev() {
            let op = function.instruction(inst);
            let writes = self.accesses.writes(inst);
            let overlaps =
                |written: &MemRef| regions::overlapping(reference, written, None, None, unit.program).unwrap_or(true);
            if !llrm_analysis::memoryssa::changes(reference, false, writes, overlaps) {
                continue;
            }
            if writes.is_none() {
                return (false, usize::MAX);
            }
            let (Opcode::Store { volatile: false, .. }, Some(written)) =
                (&op.opcode, self.accesses.references.get(&inst))
            else {
                return (false, usize::MAX);
            };
            if let Some(wanted) = _known(unit, expected) {
                let fact = consts::initialized(unit, inst, reference).or_else(|| self.after(inst, reference));
                if fact == Some(wanted) {
                    return (true, usize::MAX);
                }
            }
            return (_same_cell(reference, written) && op.operands[0] == expected, usize::MAX);
        }
        let parents = self.predecessors.get(&at).cloned().unwrap_or_default();
        if at == self.entry || parents.is_empty() {
            return (false, usize::MAX);
        }
        let phi = match expected {
            Operand::Value(value) => {
                edges::phis(function, block).into_iter().find(|&phi| function.instruction(phi).result == Some(value))
            }
            _ => None,
        };
        let incoming = phi.map(|phi| arms(function, phi));
        if incoming
            .as_ref()
            .is_some_and(|incoming| incoming.iter().map(|(_, from)| cfg::id(*from)).collect::<BTreeSet<_>>() != parents)
        {
            return (false, usize::MAX);
        }
        let mut active = active.to_vec();
        active.push(key);
        let mut lowest = usize::MAX;
        let answer = parents
            .into_iter()
            .all(
                |parent| {
                    let next = incoming.as_ref().map_or(expected, |incoming| {
                        incoming.iter().find(|(_, from)| cfg::id(*from) == parent).expect("an arm per parent").0
                    });
                    let (answer, low) = self.stored_on(reference, parent, next, &active, known);
                    lowest = lowest.min(low);
                    answer
                },
            );
        (answer, lowest)
    }

    /// What memory says `reference` holds once `inst` has run.
    fn after(
        &mut self,
        inst: InstId,
        reference: &MemRef,
    ) -> Option<Known> {
        let unit = self.unit;
        let calls = Calls::default();
        let cells = self.analyses.get::<MemoryCells>(unit.context, unit.layout, unit.function);
        let before = cells.held.get(&inst).map(|here| (**here).clone()).unwrap_or_default();
        let nothing = IndexMap::default();
        let mut queries = consts::memory_queries(*unit, &nothing);
        let after = consts::_kills(before, inst, &nothing, &calls, None, None, false, &mut queries);
        consts::_cell(&after, reference)
    }
}

/// An integer constant as a fact.
fn _known(
    unit: &Unit,
    operand: Operand,
) -> Option<Known> {
    let bits = unit.int_constant(operand)?;
    let width = unit.int_bits(operand)?;
    Some(Known::new(masked(&BigInt::from(bits), width), width))
}

/// Whether two references name the same bytes the same way.
fn _same_cell(
    one: &MemRef,
    other: &MemRef,
) -> bool {
    (one.root, one.disp, one.base, one.scale, one.segment, one.width)
        == (other.root, other.disp, other.base, other.scale, other.segment, other.width)
}

/// Whether `inst` stores to an object's fixed or invariantly indexed cell
/// that nothing else in the loop reads or writes.
fn _unobserved(
    unit: &Unit,
    inst: InstId,
    near: &llrm_analysis::regions::Index<InstId>,
    references: &IndexMap<InstId, MemRef>,
    address_values: &dyn Fn(ValueId) -> bool,
) -> bool {
    if !matches!(
        unit.function.instruction(inst).opcode,
        Opcode::Store { volatile: false, .. }
    ) {
        return false;
    }
    let Some(reference) = references.get(&inst) else { return false };
    if !reference.object || reference.segment.is_some() || reference.base.is_some_and(|base| !address_values(base)) {
        return false;
    }
    if matches!(
        reference.root,
        Some(Operand::Value(root)) if !address_values(root)
    ) {
        return false;
    }
    near.near(reference)
        .into_iter()
        .filter(|&(one, _)| one != inst)
        .all(|(_, other)| !regions::overlapping(reference, other, None, None, unit.program).unwrap_or(true))
}

#[cfg(test)]
#[path = "loopmotion_tests.rs"]
mod tests;
