//! Adapted from llrm-core's `optimize/loopsimplify.rs`, the port of
//! `qbopt/optimize/loopsimplify.py`: every reducible loop gets one
//! preheader, one latch and dedicated exits.
//!
//! The old body was immutable, so a refused grouping left it as it was; here
//! each loop is simplified on a copy of the function, kept only when every
//! grouping it needed went through. Every block ends in a terminator, so the
//! old fall-through source, a block whose last operation merely precedes
//! `target`, is a `br label`.

use std::collections::BTreeSet;

use llrm_analysis::cfg;
use llrm_analysis::graph::loops;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, Operand};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};

use crate::edges;

pub struct LoopSimplify;

impl FunctionPass for LoopSimplify {
    fn name(&self) -> &'static str {
        "loopsimplify"
    }

    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        let shape = (*analyses.get::<cfg::Shape>(unit.context, unit.layout, unit.function)).clone();
        if _simplified(unit.function, shape) { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// The edges from `sources` into `target` routed through one new block,
/// which `target`'s phis read; the new block, or `None`, changing nothing,
/// where a source's terminator or a phi cannot be regrouped.
pub fn grouped(function: &mut Function, target: i64, sources: &BTreeSet<i64>) -> Option<BlockId> {
    let graph = cfg::graph(function);
    let predecessors = loops::predecessors(&graph);
    let entry = cfg::id(function.entry()?);
    let reaching = predecessors.get(&target)?;
    if sources.is_empty() || !sources.is_subset(reaching) || target == entry {
        return None;
    }
    let destination = cfg::block(target);
    for &parent in sources {
        let last = function.terminator(cfg::block(parent))?;
        match function.instruction(last).operands.len() {
            _ if function.instruction(last).opcode != Opcode::Br => return None,
            3 if !edges::conditional(function, cfg::block(parent), destination) => return None,
            1 if function.successors(cfg::block(parent)) != [destination] => return None,
            _ => {}
        }
    }
    let phis = edges::phis(function, destination);
    let incoming = |function: &Function, phi| {
        function.instruction(phi).operands.chunks(2).map(|pair| (pair[0], pair[1])).collect::<Vec<_>>()
    };
    let at = |block: Operand| match block {
        Operand::Block(block) => cfg::id(block),
        _ => unreachable!("a phi's block operand"),
    };
    if phis.iter().any(|&phi| incoming(function, phi).iter().map(|&(_, block)| at(block)).collect::<BTreeSet<_>>() != *reaching) {
        return None;
    }
    let bridge = function.create_block(None);
    function.insert_block(bridge, None).expect("a new block");
    for phi in phis {
        let pairs = incoming(function, phi);
        let (grouped, kept): (Vec<_>, Vec<_>) = pairs.into_iter().partition(|&(_, block)| sources.contains(&at(block)));
        let values = grouped.iter().map(|&(value, _)| value).collect::<Vec<_>>();
        let result = if values.iter().all(|&one| one == values[0]) {
            values[0]
        } else {
            let ty = function.instruction(phi).ty;
            let operands = grouped.iter().flat_map(|&(value, block)| [value, block]).collect();
            let merged = function.create_instruction(Opcode::Phi, ty, operands, Flags::default(), None);
            function.insert(merged, Position::End(bridge)).expect("a placed block");
            Operand::Value(function.instruction(merged).result.expect("a phi's value"))
        };
        let operands = kept.into_iter().chain([(result, Operand::Block(bridge))]).flat_map(|(value, block)| [value, block]).collect();
        function.set_operands(phi, operands);
    }
    let void = function.instruction(function.terminator(cfg::block(*sources.first().expect("nonempty"))).expect("checked above")).ty;
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(destination)], Flags::default(), None);
    function.insert(jump, Position::End(bridge)).expect("a placed block");
    for &parent in sources {
        let last = function.terminator(cfg::block(parent)).expect("checked above");
        edges::retarget(function, last, destination, bridge);
    }
    Some(bridge)
}

/// Whether any loop changed.
pub fn simplified(function: &mut Function) -> bool {
    let shape = cfg::Shape::of(function);
    _simplified(function, shape)
}

/// `simplified`, `shape` being `function`'s.
fn _simplified(function: &mut Function, mut shape: cfg::Shape) -> bool {
    if function.entry().is_none() || !shape.dominance.irreducible(function).is_empty() {
        return false;
    }
    let mut changed = false;
    let headers = shape.loops.iter().map(|loop_| loop_.header).collect::<Vec<_>>();
    for header in headers {
        let original = shape.loops.iter().find(|loop_| loop_.header == header).expect("StopIteration").clone();
        let mut candidate = function.clone();
        let mut grouping = false;
        let predecessors = loops::predecessors(&cfg::graph(&candidate));
        let outside = predecessors[&original.header].difference(&original.body).copied().collect::<BTreeSet<_>>();
        let Some(&parent) = outside.first() else { continue };
        if outside.len() != 1 || candidate.successors(cfg::block(parent)) != [cfg::block(original.header)] {
            if grouped(&mut candidate, original.header, &outside).is_none() {
                continue;
            }
            grouping = true;
        }
        if original.latches.len() != 1 {
            if grouped(&mut candidate, original.header, &original.latches).is_none() {
                continue;
            }
            grouping = true;
        }
        let graph = cfg::graph(&candidate);
        let current = if grouping {
            cfg::Shape::of(&candidate).loops.into_iter().find(|loop_| loop_.header == original.header).expect("StopIteration")
        } else {
            original.clone()
        };
        let predecessors = loops::predecessors(&graph);
        let exits = graph
            .iter()
            .filter(|block| current.body.contains(&block.at))
            .flat_map(|block| block.succ.iter().copied())
            .filter(|at| !current.body.contains(at))
            .collect::<BTreeSet<_>>();
        let empty = BTreeSet::new();
        let mut broke = false;
        for target in exits {
            let reaching = predecessors.get(&target).unwrap_or(&empty);
            let sources = reaching.intersection(&current.body).copied().collect::<BTreeSet<_>>();
            if !reaching.is_subset(&current.body) {
                if grouped(&mut candidate, target, &sources).is_none() {
                    broke = true;
                    break;
                }
                grouping = true;
            }
        }
        if !broke && grouping {
            *function = candidate;
            changed = true;
            shape = cfg::Shape::of(function);
        }
    }
    changed
}

#[cfg(test)]
#[path = "loopsimplify_tests.rs"]
mod tests;
