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
//! Not ported yet, so `optimized` does without them: `transform::forwarded`
//! (store-to-load forwarding, waiting on `analysis::avail`), then
//! `loadjoins::reused` and `floatfold::checks` after the join.
//! `reused_divides` has no counterpart (see `transform`).
//!
//! The old module had no tests of its own; `subexpressions`' are in
//! `transform` and these in `gvn_tests.rs`.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::{cfg, liveness, memory, ssa};
use llrm_graph::loops::{self, Loop};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Instruction, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, Outer, PreservedAnalyses, Unit};
use llrm_support::hash::IndexMap;

use crate::lcssa::{arms, from_arms, place_phi};
use crate::profit::{self, OperationCosts};
use crate::{edges, transform};

/// The single value-reuse pass: scalar GVN and memory-aware PRE.
#[derive(Default)]
pub struct Gvn {
    pub costs: OperationCosts,
    /// Integer values that fit in registers; 0 leaves pricing out.
    pub registers: i64,
}

impl FunctionPass for Gvn {
    fn name(&self) -> &'static str {
        "gvn"
    }

    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        match optimized(unit, analyses.outer(), &self.costs, self.registers) {
            Ok(true) => PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("gvn: {error}"),
        }
    }
}

/// Number values, reuse dominating providers, and complete join PRE;
/// whether anything changed. `outer` is what the analyses read of the
/// module and target.
pub fn optimized(unit: &mut Unit, outer: &Outer, costs: &OperationCosts, registers: i64) -> Result<bool, String> {
    let numbered = _numbered(unit, outer, costs, registers)?;
    // PRE may add work to a previously missing path.  Do that only after
    // local numbering has stabilized.
    let combined = joined(unit.function, !numbered)?;
    Ok(numbered || combined)
}

/// Local numbering, crossing stores only where the whole function prices
/// lower for it: a provider held across a store saves loads but may spill.
/// Whether it changed anything.
fn _numbered(unit: &mut Unit, outer: &Outer, costs: &OperationCosts, registers: i64) -> Result<bool, String> {
    let accesses = transform::_accesses(&memory::Unit::within(unit.context, unit.layout, unit.function, outer))?;
    let numbered = |function: &Function, avoid_store_crossing: bool| -> Result<(Function, bool), String> {
        let mut function = function.clone();
        // `transform::forwarded(avoid_store_crossing)` goes here once
        // `analysis::avail` is ported.
        let changed = transform::subexpressions(&mut function, &accesses, avoid_store_crossing)?;
        Ok((function, changed))
    };
    let crossing = numbered(unit.function, false)?;
    let price = |one: &Function| profit::pressure_adjusted(unit.context, one, unit.callees, costs, registers, None, &liveness::live(one).live_out);
    let chosen = if registers == 0 {
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
    let dominators = loops::dominators(&graph, Some(cfg::id(entry)));
    let natural_loops = loops::loops(&graph, Some(cfg::id(entry)));

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
        function.replace_all_uses_with(value, with);
    }
    for inst in erased {
        function.erase(inst)?;
    }
    Ok(true)
}

#[cfg(test)]
#[path = "gvn_tests.rs"]
mod tests;
