//! Adapted from llrm-core's `optimize/unswitch.rs`, the port of
//! `qbopt/optimize/unswitch.py`: specialize a loop around a pure invariant
//! condition, entirely in MIR.
//!
//! What changed with the IR:
//! - The old candidate was re-optimized by `transform::recorded`, the whole
//!   pipeline with unswitching off and the machine's tuning forwarded. That
//!   pipeline is not ported, so the re-optimization is the passes the pass
//!   is given; its price is `profit::weighted` at the given costs, each
//!   loop weighted by the trips induction proves.
//! - The old stage records and `watch` hook are the pass manager's dump and
//!   change log.
//! - A condition's purity was checked on the old operations' memory, flag,
//!   stack and x87 fields; an `icmp` has none, so what is left is that its
//!   operands are defined before the loop.
//! - The guard and dispatch dropped the old operations' byte provenance; a
//!   clone's is its `Cloned` change.
//!
//! Python's `ValueError`s are the `Err` text.
//!
//! Tests, in `unswitch_tests.rs`: `test_condition_must_be_pure_and_loop_invariant`
//! is ported with a written loop. The three monkeypatching tests that
//! replaced the re-optimization are ported by passing it:
//! `test_unswitch_rejects_a_candidate_without_loop_removal`,
//! `test_unswitch_rejects_lower_count_but_higher_target_cost` and
//! `test_unswitch_rejects_semantic_work_without_a_target_price`. Skipped:
//! `test_unswitch_reoptimization_preserves_mir_target_costs`, since nothing
//! is forwarded to passes the caller builds. Stay behind, reading BC
//! corpora (`ivarm-*.obj`, `ivproc-*.obj`):
//! `test_production_ivarm_has_no_loop_and_stores_last_value`,
//! `test_specialized_main_and_legacy_procedure_emit_together`,
//! `test_invariant_branch_specialization_exposes_loop_deletion`,
//! `test_cloning_provenance_survives_ssa_reconstruction` and
//! `test_implicit_edge_bridge_does_not_retarget_the_taken_arm`.

use std::collections::BTreeMap;

use llrm_analysis::{cfg, consts, memory, occurrence};
use llrm_graph::loops::{self, Loop};
use llrm_mir::edit::Position;
use llrm_mir::context::Context;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};

use crate::lcssa::{self, arms, from_arms, operations};
use crate::profit::{self, OperationCosts};
use crate::{edges, loopclone, transform};

pub struct Unswitch {
    pub costs: OperationCosts,
    /// What a specialized candidate goes through before it is judged.
    pub passes: Vec<Box<dyn FunctionPass>>,
}

impl FunctionPass for Unswitch {
    fn name(&self) -> &'static str {
        "unswitch"
    }

    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        let passes = &mut self.passes;
        let mut reoptimize = |candidate: &mut Unit| {
            for pass in passes.iter_mut() {
                pass.run(candidate, &mut analyses.fresh());
            }
        };
        match optimized(unit, &self.costs, &mut reoptimize) {
            Ok(true) => PreservedAnalyses::none(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("unswitch: {error}"),
        }
    }
}

/// `unit`'s function specialized and re-optimized, kept only when that
/// removed a loop without growing the function or its price; whether it was.
pub fn optimized(unit: &mut Unit, costs: &OperationCosts, reoptimize: &mut dyn FnMut(&mut Unit)) -> Result<bool, String> {
    let Some(mut candidate) = specialized(unit.context, unit.function)? else {
        return Ok(false);
    };
    reoptimize(&mut Unit {
        context: &mut *unit.context,
        layout: unit.layout,
        function: &mut candidate,
        callees: unit.callees,
        metadata: unit.metadata,
        sizes: unit.sizes,
    });

    let size = |state: &Function| occurrence::operations(state).count();
    let count = |state: &Function| loops::loops(&cfg::graph(state), state.entry().map(cfg::id)).len();
    let price = |state: &Function| {
        // Registers alone: no global is read.
        let within = memory::Unit { machine: None, context: unit.context, layout: unit.layout, metadata: unit.metadata, globals: &[], function: state, references: None, globals_aa: None };
        let trips = profit::proven_trips(&within, &consts::known(&within, None, None, None));
        profit::weighted(unit.context, state, unit.callees, costs, Some(&trips))
    };
    let worse = match (price(unit.function), price(&candidate)) {
        (Some(before), Some(after)) => after > before,
        _ => true,
    };
    if count(&candidate) >= count(unit.function) || size(&candidate) > size(unit.function) || worse {
        return Ok(false);
    }
    *unit.function = candidate;
    Ok(true)
}

/// `function` with its first loop that has a pure invariant condition
/// specialized into a copy for each way the condition goes, or `None`.
pub fn specialized(context: &mut Context, function: &Function) -> Result<Option<Function>, String> {
    let mut closed = function.clone();
    lcssa::closed(&mut closed)?;
    let mut owners = BTreeMap::new();
    for (block, inst) in closed.walk() {
        if let Some(value) = closed.instruction(inst).result {
            owners.insert(value, cfg::id(block));
        }
    }
    let graph = cfg::graph(&closed);
    let dominance = loops::dominance(&graph, closed.entry().map(cfg::id));
    let predecessors = loops::predecessors(&graph);
    for loop_ in loops::loops(&graph, closed.entry().map(cfg::id)) {
        let outside = predecessors[&loop_.header].difference(&loop_.body).copied().collect::<Vec<_>>();
        if outside.len() != 1 || loop_.body.iter().map(|&at| operations(&closed, cfg::block(at)).len()).sum::<usize>() > 128 {
            continue;
        }
        let entry = outside[0];
        for block in &graph {
            let at = cfg::block(block.at);
            if !loop_.body.contains(&block.at) || block.at == loop_.header || block.succ.len() != 2 || operations(&closed, at).is_empty() {
                continue;
            }
            let branch = closed.terminator(at).expect("a terminated block");
            let Some(compare) = transform::_comparison(&closed, at, branch) else {
                continue;
            };
            if closed.instruction(compare).operands.iter().any(|operand| match operand {
                Operand::Value(value) => owners.get(value).is_some_and(|&owner| loop_.body.contains(&owner) || !dominance.dominates(owner, entry)),
                _ => false,
            }) {
                continue;
            }
            let mut copied = closed.clone();
            let Some(labels) = loopclone::peeled(&mut copied, &loop_, 1)? else {
                continue;
            };
            _specialized(&closed, &mut copied, &loop_, entry, block.at, compare, &labels[0])?;
            llrm_analysis::cfg::_unreachable(context, &mut copied);
            transform::_trivial_phis(&mut copied)?;
            return Ok(Some(copied));
        }
    }
    Ok(None)
}

/// `copied`, `body` with one iteration of `loop_` peeled into `labels`,
/// made a second loop: `entry` tests `compare` once and runs the copy when
/// it holds, the original when not, and in each `selected` goes one way.
pub fn _specialized(
    body: &Function,
    copied: &mut Function,
    loop_: &Loop,
    entry: i64,
    selected: i64,
    compare: InstId,
    labels: &BTreeMap<i64, BlockId>,
) -> Result<(), String> {
    let latch = *loop_.latches.first().expect("one latch");
    let header = cfg::block(loop_.header);
    let (cloned_header, cloned_latch) = (labels[&loop_.header], labels[&latch]);
    let parent = cfg::block(entry);

    let last = copied.terminator(parent).expect("a terminated block");
    let guard = copied.clone_instruction(compare);
    copied.insert(guard, Position::Before(last))?;
    let condition = Operand::Value(copied.instruction(guard).result.expect("a compare's value"));
    let operands = vec![condition, Operand::Block(cloned_header), Operand::Block(header)];
    let dispatch = copied.create_instruction(Opcode::Br, copied.instruction(last).ty, operands, Flags::default(), None);
    copied.insert(dispatch, Position::Before(last))?;
    copied.erase(last)?;

    // The original header reads the original latch again; the copy's header
    // takes the copy's latch in its place.
    for (phi, cloned) in edges::phis(copied, header).into_iter().zip(edges::phis(copied, cloned_header)) {
        let residual = arms(copied, phi).into_iter().find(|&(_, source)| source == cloned_latch).ok_or("KeyError")?;
        copied.set_operands(phi, body.instruction(phi).operands.clone());
        let mut incoming = arms(copied, cloned);
        incoming.push(residual);
        copied.set_operands(cloned, from_arms(&incoming));
    }
    let jump = copied.terminator(cloned_latch).expect("a terminated block");
    edges::retarget(copied, jump, header, cloned_header);

    for (at, taken) in [(cfg::block(selected), false), (labels[&selected], true)] {
        let branch = copied.terminator(at).ok_or("IndexError")?;
        let destination = copied.instruction(branch).operands[if taken { 1 } else { 2 }];
        copied.set_operands(branch, vec![destination]);
    }

    for target in [header, cloned_header] {
        edges::split(copied, parent, target, Vec::new())?;
    }
    for version in [loop_.body.iter().map(|&at| cfg::block(at)).collect::<Vec<_>>(), labels.values().copied().collect()] {
        let exits = version
            .iter()
            .flat_map(|&source| copied.successors(source).into_iter().map(move |target| (source, target)))
            .filter(|(_, target)| !version.contains(target))
            .collect::<Vec<_>>();
        for (source, target) in exits {
            if edges::conditional(copied, source, target) {
                edges::split(copied, source, target, Vec::new())?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "unswitch_tests.rs"]
mod tests;
