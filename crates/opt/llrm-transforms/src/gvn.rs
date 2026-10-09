//! Adapted from llrm-core's `optimize/gvn.rs`, the port of
//! `qbopt/optimize/gvn.py`: global value numbering and partial redundancy
//! elimination.
//!
//! One pass owns reuse, whether the value came from arithmetic or memory.
//! `Gvn` is the pass; the old one sat with the pipeline in `transform.rs`.
//!
//! What changed with the IR:
//! - A function argument is defined at the entry, ahead of everything.
//! - An inserted expression goes before its predecessor's unconditional
//!   `br`; a block leaves only by its terminator, so the old refusal of a
//!   predecessor holding a branch, return or escape is that terminator.
//! - The flags checks are gone: no value is the machine's flags. `Convert`
//!   (the register-pair extension) and `Copy` have no instruction, so what
//!   `joined` refuses of `_PURE` is the divisions, which may trap on a path
//!   that did not divide.
//! - `_on_edge` translates an instruction's operands; a phi's input may be
//!   a constant.
//!
//! `floatfold::checks` has no counterpart: the rich MIR observes no FP
//! exception.
//!
//! What each instruction touches is `memoryssa::Accesses`, asked once
//! before anything changes; without `Summaries` required, every call is
//! to an unknown callee.
//! `reused_divides` has no counterpart (see `transform`).
//!
//! The old module had no tests of its own; `subexpressions`' are in
//! `transform` and these in `gvn_tests.rs`.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::alias::PointsTo;
use llrm_analysis::manager::{Pointers, Registers};
use llrm_analysis::memory;
use llrm_analysis::memoryssa::Accesses;
use llrm_analysis::{cfg, ssa};
use llrm_analysis::graph::loops::{self, Loop};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Instruction, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, Outer, PreservedAnalyses, Unit};
use llrm_support::hash::IndexMap;

use crate::lcssa::{arms, from_arms, place_phi};
use crate::profit::{self, OperationCosts};
use crate::{edges, loadjoins, transform};

/// The single value-reuse pass: scalar GVN and memory-aware PRE. The
/// target's registers price it (`profit::registers`); none leaves pricing
/// out.
#[derive(Default)]
pub struct Gvn;

impl FunctionPass for Gvn {
    fn name(&self) -> &'static str {
        "gvn"
    }

    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        let accesses = Accesses::managed(unit.context, unit.layout, unit.function, analyses);
        let pointers = analyses.get::<Pointers>(unit.context, unit.layout, unit.function);
        // Only `loadjoins` changes the CFG, and only by splitting an edge.
        let blocks = unit.function.layout().len();
        let pointers = Result::as_ref(&*pointers).map_err(String::clone);
        // Numbering leaves the CFG alone: every candidate has these trips.
        let registers = analyses.get::<Registers>(unit.context, unit.layout, unit.function);
        let shape = analyses.get::<llrm_analysis::cfg::Shape>(unit.context, unit.layout, unit.function);
        let trips = if !profit::registers(analyses.outer()).priced() {
            IndexMap::default()
        } else {
            let counted = analyses.get::<llrm_analysis::manager::Counted>(unit.context, unit.layout, unit.function);
            profit::proven_trips(&memory::Unit::within(unit.context, unit.layout, unit.function, analyses.outer()).with_registers(&registers).with_shape(&shape).with_counted(&counted), &registers)
        };
        match accesses.and_then(|accesses| optimized(unit, analyses.outer(), &accesses, pointers?, &trips, &registers, &shape)) {
            Ok(true) if unit.function.layout().len() != blocks => PreservedAnalyses::none(),
            Ok(true) => PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("gvn: {error}"),
        }
    }
}

/// Number values, reuse dominating providers, and complete join PRE;
/// whether anything changed. `outer` is what the analyses read of the
/// module and target, `accesses` what each instruction of `unit` touches,
/// `pointers` what each pointer points to, `trips` each loop's proven
/// trips by latch.
///
/// Every edit replaces a value with an equal one and adds no memory
/// access before `loadjoins`, so `accesses` stays true throughout.
pub fn optimized(unit: &mut Unit, outer: &Outer, accesses: &Accesses, pointers: &PointsTo, trips: &IndexMap<i64, i64>, registers: &IndexMap<ValueId, llrm_analysis::consts::Known>, shape: &cfg::Shape) -> Result<bool, String> {
    let equal = propagated(unit);
    // What is known of the body the manager saw, unless propagating a branch's condition changed it.
    let fresh;
    let registers = if equal {
        fresh = llrm_analysis::consts::known(&memory::Unit::within(unit.context, unit.layout, unit.function, outer), None, None, None);
        &fresh
    } else {
        registers
    };
    let (numbered, subexpressed) = _numbered(unit, outer, accesses, &profit::costs(outer), profit::registers(outer), trips, registers, shape)?;
    // PRE may add work to a previously missing path.  Do that only after
    // local numbering has stabilized.
    let combined = joined(unit.function, !subexpressed)?;
    // Joining may split an edge, which the manager's shape has not seen.
    let joined_shape;
    let shape = if combined {
        joined_shape = cfg::Shape::of(unit.function);
        &joined_shape
    } else {
        shape
    };
    let loaded = loadjoins::reused(unit.context, unit.layout, unit.function, outer, outer.callees(), accesses, pointers, !combined, shape)?;
    Ok(equal || numbered || combined || loaded)
}

/// LLVM GVN's `propagateEquality`, for a branch's condition: in what the
/// edge to a successor dominates, the condition is that edge's constant.
/// Whether any use changed.
fn propagated(unit: &mut Unit) -> bool {
    let shape = cfg::Shape::of(unit.function);
    let mut found = Vec::new();
    for &block in unit.function.layout() {
        let Some(last) = unit.function.terminator(block) else { continue };
        let branch = unit.function.instruction(last);
        let [Operand::Value(condition), Operand::Block(taken), Operand::Block(other)] = branch.operands[..] else { continue };
        if branch.opcode != Opcode::Br || taken == other {
            continue;
        }
        for (target, holds) in [(taken, 1), (other, 0)] {
            // The edge dominates what its target does only when it is the one
            // way in, and not back into the branch's own block.
            if target == block || unit.function.predecessors(target) != [block] {
                continue;
            }
            for one in unit.function.users(condition) {
                let user = unit.function.instruction(one.user);
                let at = match user.opcode {
                    Opcode::Phi => match user.operands[one.index as usize + 1] {
                        Operand::Block(from) => from,
                        _ => continue,
                    },
                    _ => match unit.function.parent(one.user) {
                        Some(at) => at,
                        None => continue,
                    },
                };
                if shape.dominance.dominates(cfg::id(target), cfg::id(at)) {
                    found.push((*one, condition, holds));
                }
            }
        }
    }
    for &(one, condition, holds) in &found {
        let ty = unit.function.value(condition).ty;
        let constant = unit.context.int(ty, holds);
        unit.function.set_operand(one.user, one.index as usize, Operand::Constant(constant));
    }
    !found.is_empty()
}

thread_local! {
    static NUMBERINGS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has numbered a function (once, or twice where a load could be served across a store), for a test
/// that a function with no such load is numbered once.
pub fn numberings() -> usize {
    NUMBERINGS.with(std::cell::Cell::get)
}

/// Local numbering, crossing stores only where the whole function prices
/// lower for it: a provider held across a store saves loads but may spill.
/// Whether it changed anything, and whether `subexpressions` did.
fn _numbered(unit: &mut Unit, outer: &Outer, accesses: &Accesses, costs: &OperationCosts, room: crate::spill::Room, trips: &IndexMap<i64, i64>, registers: &IndexMap<ValueId, llrm_analysis::consts::Known>, shape: &cfg::Shape) -> Result<(bool, bool), String> {
    // The availability of the function as it comes in: the same for both runs below, each of which changes a copy.
    let held = std::cell::OnceCell::new();
    // Whether some load was served across a store: the one thing the second numbering does differently from the first.
    let crossed = std::cell::Cell::new(false);
    let numbered = |function: &Function, avoid_store_crossing: bool| -> Result<(Function, (bool, bool)), String> {
        NUMBERINGS.with(|runs| runs.set(runs.get() + 1));
        let mut function = function.clone();
        let forwarded = transform::forwarded(unit.context, unit.layout, &mut function, outer, accesses, registers, shape, avoid_store_crossing, &held, &crossed)?;
        let subexpressed = transform::subexpressions(&mut function, accesses, avoid_store_crossing, Some(outer.program()), &crossed)?;
        Ok((function, (forwarded || subexpressed, subexpressed)))
    };
    let crossing = numbered(unit.function, false)?;
    let price = |one: &Function| {
        let frequency = profit::_frequencies(unit.context, unit.metadata, &outer.globals, one, Some(trips))?;
        profit::motion_price(unit.context, unit.layout, outer, one, costs, room, &frequency)
    };
    // Where no load crossed a store, the second numbering is the first: it is neither made nor priced.
    let chosen = if !room.priced() || !crossed.get() {
        crossing
    } else if let Some(crossed) = price(&crossing.0) {
        let careful = numbered(unit.function, true)?;
        if price(&careful.0).is_some_and(|kept| kept < crossed) { careful } else { crossing }
    } else {
        crossing
    };
    *unit.function = chosen.0;
    Ok(chosen.1)
}

/// Translate simultaneously: an incoming phi value belongs to the prior edge.
fn _on_edge(function: &Function, op: &Instruction, phis: &[InstId], predecessor: BlockId) -> Option<Instruction> {
    let incoming = phis
        .iter()
        .map(|&phi| {
            let value = arms(function, phi).into_iter().find(|&(_, at)| at == predecessor).map(|(value, _)| value);
            (function.instruction(phi).result.expect("a phi's value"), value)
        })
        .collect::<IndexMap<_, _>>();
    let mut operands = Vec::new();
    for &operand in &op.operands {
        match operand {
            Operand::Value(value) if incoming.contains_key(&value) => operands.push(incoming[&value]?),
            _ => operands.push(operand),
        }
    }
    Some(Instruction { operands, ..op.clone() })
}

/// Insert only on an unconditional edge, with available scalar inputs:
/// before which instruction.
fn _insertion(
    function: &Function,
    op: &Instruction,
    parent: BlockId,
    join: BlockId,
    definitions: &IndexMap<ValueId, (i64, i64)>,
    dominators: &BTreeMap<i64, BTreeSet<i64>>,
    natural_loops: &[Loop],
) -> Option<InstId> {
    let (parent_at, join_at) = (cfg::id(parent), cfg::id(join));
    if function.successors(parent) != [join]
        || dominators[&parent_at].contains(&join_at)
        || natural_loops.iter().any(|loop_| loop_.body.contains(&parent_at) != loop_.body.contains(&join_at))
        || op.operands.iter().any(|operand| !matches!(operand, Operand::Value(_) | Operand::Constant(_)))
    {
        return None;
    }
    let cut = function.terminator(parent)?;
    if function.instruction(cut).opcode != Opcode::Br || function.instruction(cut).operands.len() != 1 {
        return None;
    }
    let cut_index = function.block(parent).instructions().iter().position(|&one| one == cut)? as i64;
    for operand in &op.operands {
        let Operand::Value(value) = operand else {
            continue;
        };
        let (at, index) = *definitions.get(value)?;
        if !dominators[&parent_at].contains(&at) || (at == parent_at && index >= cut_index) {
            return None;
        }
    }
    Some(cut)
}

/// Eliminate scalar redundancy without adding execution to any path;
/// whether anything changed.
///
/// A phi combines independently dominating providers. Missing providers may
/// be inserted on unconditional incoming edges, but only when another edge
/// already supplies the result. Memory and floating expressions stay out.
pub fn joined(function: &mut Function, insert: bool) -> Result<bool, String> {
    let graph = cfg::graph(function);
    let predecessors = loops::predecessors(&graph);
    if !predecessors.values().any(|parents| parents.len() > 1) {
        return Ok(false);
    }
    let Some(entry) = function.entry() else {
        return Ok(false);
    };
    let shape = cfg::Shape::of(function);
    let dominators = shape.dominance.dominators(function);
    let natural_loops = shape.loops;

    let key = |op: &Instruction| {
        op.result?;
        if matches!(op.opcode, Opcode::Binary(BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem) | Opcode::Load { .. })
            || transform::_floating(&op.opcode)
        {
            return None;
        }
        transform::_computation(op, &BTreeMap::new())
    };

    let mut expressions = IndexMap::<_, Vec<(i64, usize, ValueId)>>::default();
    let mut definitions = IndexMap::<ValueId, (i64, i64)>::default();
    definitions.extend(function.parameters().iter().map(|&value| (value, (cfg::id(entry), -1))));
    for &block in function.layout() {
        for (index, &inst) in function.block(block).instructions().iter().enumerate() {
            let op = function.instruction(inst);
            let Some(result) = op.result else {
                continue;
            };
            let index = if op.opcode == Opcode::Phi { -1 } else { index as i64 };
            definitions.insert(result, (cfg::id(block), index));
            if let Some(expression) = key(op) {
                expressions.entry(expression).or_default().push((cfg::id(block), index as usize, result));
            }
        }
    }

    let mut changed = false;
    let mut replacements = BTreeMap::<ValueId, Operand>::new();
    let mut erased = Vec::new();
    for block in function.layout().to_vec() {
        let at = cfg::id(block);
        let parents = &predecessors[&at];
        if block == entry || parents.len() < 2 {
            continue;
        }
        for inst in function.block(block).instructions().to_vec() {
            let op = function.instruction(inst).clone();
            if op.opcode == Opcode::Phi {
                continue;
            }
            let expression = key(&op);
            let mut incoming = Vec::<(Operand, BlockId)>::new();
            let mut missing = Vec::<(BlockId, InstId, Instruction)>::new();
            if expression.is_some() {
                for &parent in parents {
                    let parent = cfg::block(parent);
                    let substituted = Instruction { operands: ssa::substituted(&op, &replacements).map_err(|error| error.to_string())?, ..op.clone() };
                    let translated = _on_edge(function, &substituted, &edges::phis(function, block), parent);
                    let edge_expression = translated.as_ref().and_then(key);
                    let candidates = edge_expression
                        .as_ref()
                        .and_then(|edge| expressions.get(edge))
                        .into_iter()
                        .flatten()
                        .filter(|(provider, _, _)| {
                            *provider != at
                                && dominators[&cfg::id(parent)].contains(provider)
                                && !dominators[provider].contains(&at)
                                && natural_loops.iter().all(|loop_| !loop_.body.contains(provider) || loop_.body.contains(&at))
                        })
                        .collect::<Vec<_>>();
                    if candidates.is_empty() {
                        if !insert {
                            break;
                        }
                        let cut = match (&edge_expression, &translated) {
                            (Some(_), Some(translated)) => _insertion(function, translated, parent, block, &definitions, &dominators, &natural_loops),
                            _ => None,
                        };
                        let Some(cut) = cut else {
                            break;
                        };
                        missing.push((parent, cut, translated.expect("an edge expression has a translation")));
                        continue;
                    }
                    // Python's `max` keeps the first of equal keys.
                    let mut best = candidates[0];
                    for item in &candidates[1..] {
                        if (dominators[&item.0].len(), item.1) > (dominators[&best.0].len(), best.1) {
                            best = item;
                        }
                    }
                    incoming.push((Operand::Value(best.2), parent));
                }
            }
            if incoming.is_empty() || incoming.len() + missing.len() != parents.len() {
                continue;
            }
            let result = op.result.expect("key needs a result");
            let name = function.value(result).name.clone();
            for (parent, cut, translated) in missing {
                let made = function.create_instruction(
                    translated.opcode,
                    translated.ty,
                    translated.operands,
                    translated.flags,
                    name.as_ref().map(|name| format!("{name}.pre")).as_deref(),
                );
                function.insert(made, Position::Before(cut))?;
                incoming.push((Operand::Value(function.instruction(made).result.expect("an expression's value")), parent));
            }
            let phi = function.create_instruction(Opcode::Phi, op.ty, from_arms(&incoming), Default::default(), name.as_ref().map(|name| format!("{name}.pre-phi")).as_deref());
            place_phi(function, block, phi)?;
            replacements.insert(result, Operand::Value(function.instruction(phi).result.expect("a phi's value")));
            erased.push(inst);
            changed = true;
        }
    }
    if !changed {
        return Ok(false);
    }
    // A provider replaced in turn reaches each phi naming it through this.
    for (&value, &with) in &replacements {
        function.replace_value(value, with);
    }
    for inst in erased {
        function.erase(inst)?;
    }
    Ok(true)
}

#[cfg(test)]
#[path = "gvn_tests.rs"]
mod tests;
