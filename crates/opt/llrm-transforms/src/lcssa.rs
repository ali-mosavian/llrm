//! Adapted from llrm-core's `optimize/lcssa.rs`, the port of
//! `qbopt/optimize/lcssa.py`: every supported natural loop in closed SSA
//! form.
//!
//! The old body was immutable, so a closing was a substitution applied to a
//! copy; here each use is rewritten in place. An exit phi takes its value's
//! name with `.lcssa`, as LLVM names it, where the old one kept the variable
//! and took the next version. The old MIR's flag values, never closed, have
//! no counterpart: a condition is an ordinary `i1` value.
//!
//! Every old test is ported, in `lcssa_tests.rs`.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::{cfg, ssa};
use llrm_analysis::graph::loops::{self, Loop};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};

use crate::{edges, lcssamerges};

pub struct LoopClosedSSA;

impl FunctionPass for LoopClosedSSA {
    fn name(&self) -> &'static str {
        "lcssa"
    }

    fn run(&mut self, unit: &mut Unit, _: &mut Analyses) -> PreservedAnalyses {
        match closed(unit.function) {
            Ok(true) => PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("lcssa: {error}"),
        }
    }
}

/// `function` with every supported natural loop in closed SSA form; whether
/// anything changed.
pub fn closed(function: &mut Function) -> Result<bool, String> {
    let mut changed = _opened(function)?;
    // Loops come inner first. Closing an inner loop first makes its exit
    // value an ordinary definition in an enclosing loop.
    for loop_ in cfg::Shape::of(function).loops {
        changed |= _closed_loop(function, &loop_)?;
    }
    Ok(changed)
}

/// `block`'s instructions other than its phis, in order.
pub(crate) fn operations(function: &Function, block: BlockId) -> Vec<InstId> {
    function.block(block).instructions().iter().copied().filter(|&one| function.instruction(one).opcode != Opcode::Phi).collect()
}

/// A phi's inputs, each value with the block it comes from.
pub(crate) fn arms(function: &Function, phi: InstId) -> Vec<(Operand, BlockId)> {
    function
        .instruction(phi)
        .operands
        .chunks(2)
        .map(|pair| match pair[1] {
            Operand::Block(block) => (pair[0], block),
            other => unreachable!("a phi's block operand, not {other:?}"),
        })
        .collect()
}

pub(crate) fn from_arms(arms: &[(Operand, BlockId)]) -> Vec<Operand> {
    arms.iter().flat_map(|&(value, block)| [value, Operand::Block(block)]).collect()
}

/// The values defined in `blocks`, each with its block.
pub(crate) fn definitions(function: &Function, blocks: &BTreeSet<i64>) -> BTreeMap<ValueId, i64> {
    let mut defined = BTreeMap::new();
    for &block in function.layout().iter().filter(|&&one| blocks.contains(&cfg::id(one))) {
        for &inst in function.block(block).instructions() {
            if let Some(value) = function.instruction(inst).result {
                defined.insert(value, cfg::id(block));
            }
        }
    }
    defined
}

/// A new, unplaced phi of `value`'s type and name, without inputs yet.
pub(crate) fn exit_phi(function: &mut Function, value: ValueId) -> InstId {
    let data = function.value(value);
    let name = data.name.as_ref().map(|name| format!("{name}.lcssa"));
    function.create_instruction(Opcode::Phi, data.ty, Vec::new(), Flags::default(), name.as_deref())
}

/// Places `phi` after `block`'s phis.
pub(crate) fn place_phi(function: &mut Function, block: BlockId, phi: InstId) -> Result<(), String> {
    let first = operations(function, block).first().copied();
    function.insert(phi, first.map_or(Position::End(block), Position::Before))
}

/// Without the phis outside any loop exit that name one value: no loop
/// closes there any longer (unrolled or peeled away), and each is a copy.
fn _opened(function: &mut Function) -> Result<bool, String> {
    let graph = cfg::graph(function);
    let mut exits = BTreeSet::new();
    for loop_ in cfg::Shape::of(function).loops {
        for block in graph.iter().filter(|block| loop_.body.contains(&block.at)) {
            exits.extend(block.succ.iter().copied().filter(|successor| !loop_.body.contains(successor)));
        }
    }
    let one = |function: &Function, phi: InstId| {
        let values = arms(function, phi).into_iter().map(|(value, _)| value).collect::<Vec<_>>();
        values.first().copied().filter(|&first| values.iter().all(|&value| value == first))
    };
    let swap = graph
        .iter()
        .filter(|block| !exits.contains(&block.at))
        .flat_map(|block| edges::phis(function, cfg::block(block.at)))
        .filter(|&phi| one(function, phi).is_some())
        .collect::<Vec<_>>();
    // Each is read again as it goes: an earlier one may have been its value.
    for &phi in &swap {
        let value = one(function, phi).expect("one value");
        function.replace_all_uses_with(function.instruction(phi).result.expect("a phi's value"), value);
        function.set_operands(phi, Vec::new());
        function.erase(phi)?;
    }
    Ok(!swap.is_empty())
}

pub fn _closed_loop(function: &mut Function, loop_: &Loop) -> Result<bool, String> {
    let graph = cfg::graph(function);
    let exiting = graph
        .iter()
        .filter(|block| loop_.body.contains(&block.at))
        .flat_map(|block| block.succ.iter().map(move |&successor| (block.at, successor)))
        .filter(|(_, successor)| !loop_.body.contains(successor))
        .collect::<Vec<_>>();
    let exits = exiting.iter().map(|&(_, target)| target).collect::<BTreeSet<_>>();
    if exits.len() > 1 {
        return lcssamerges::closed(function, loop_);
    }
    let Some(&exit_at) = exits.first() else {
        return Ok(false);
    };
    let sources = exiting.iter().map(|&(source, _)| source).collect::<BTreeSet<_>>();
    let predecessors = loops::predecessors(&graph);
    if predecessors[&exit_at] != sources {
        return Ok(false);
    }

    let defined = definitions(function, &loop_.body);
    if defined.is_empty() {
        return Ok(false);
    }

    // Phi inputs are used on their incoming edge.  A phi in the dedicated
    // exit is already the LCSSA boundary, so only downstream phis count here.
    let mut use_sites: BTreeMap<ValueId, BTreeSet<i64>> = BTreeMap::new();
    for block in graph.iter().filter(|block| !loop_.body.contains(&block.at)) {
        for inst in operations(function, cfg::block(block.at)) {
            for operand in &function.instruction(inst).operands {
                if let Operand::Value(value) = operand
                    && defined.contains_key(value)
                {
                    use_sites.entry(*value).or_default().insert(block.at);
                }
            }
        }
        if block.at == exit_at {
            continue;
        }
        for phi in edges::phis(function, cfg::block(block.at)) {
            for (value, predecessor) in arms(function, phi) {
                if let Operand::Value(value) = value
                    && defined.contains_key(&value)
                {
                    use_sites.entry(value).or_default().insert(cfg::id(predecessor));
                }
            }
        }
    }

    let dominance = cfg::Dominance::of(function);
    let crossing = use_sites
        .iter()
        .filter(|(value, sites)| {
            !sites.is_empty()
                && sites.iter().all(|&site| dominance.dominates(exit_at, site))
                && sources.iter().all(|&source| dominance.dominates(defined[*value], source))
        })
        .map(|(value, _)| *value)
        .collect::<Vec<_>>();
    if crossing.is_empty() {
        return Ok(false);
    }

    let mut swap: BTreeMap<ValueId, Operand> = BTreeMap::new();
    let mut phis = Vec::new();
    for &value in &crossing {
        let phi = exit_phi(function, value);
        let incoming = sources.iter().map(|&source| (Operand::Value(value), cfg::block(source))).collect::<Vec<_>>();
        function.set_operands(phi, from_arms(&incoming));
        swap.insert(value, Operand::Value(function.instruction(phi).result.expect("a phi's value")));
        phis.push(phi);
    }

    let error = |error: ssa::SubstitutionError| error.to_string();
    for block in graph.iter().filter(|block| !loop_.body.contains(&block.at)) {
        let at = cfg::block(block.at);
        if block.at != exit_at {
            for phi in edges::phis(function, at) {
                let incoming = arms(function, phi)
                    .into_iter()
                    .map(|(value, predecessor)| {
                        if dominance.dominates(exit_at, cfg::id(predecessor)) {
                            ssa::provider(value, &swap).map(|one| (one, predecessor))
                        } else {
                            Ok((value, predecessor))
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(error)?;
                if from_arms(&incoming) != function.instruction(phi).operands {
                    function.set_operands(phi, from_arms(&incoming));
                }
            }
        }
        if dominance.dominates(exit_at, block.at) {
            for inst in operations(function, at) {
                let operands = ssa::substituted(function.instruction(inst), &swap).map_err(error)?;
                if operands != function.instruction(inst).operands {
                    function.set_operands(inst, operands);
                }
            }
        }
    }
    for phi in phis {
        place_phi(function, cfg::block(exit_at), phi)?;
    }
    Ok(true)
}

#[cfg(test)]
#[path = "lcssa_tests.rs"]
pub mod tests;
