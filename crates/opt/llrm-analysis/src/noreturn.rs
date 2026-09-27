//! Which bodies and blocks cannot reach a normal return, adapted from
//! llrm-core's `analysis/noreturn.rs` (a port of
//! `qbopt/analysis/noreturn.py`), with the edit that cuts a body after a
//! call that cannot return.
//!
//! What changed with the representation:
//! - A call names its callee, so the old local call table is the call's
//!   operand, and the runtime contracts' `Control::Never` is the call's or
//!   callee's `noreturn` attribute. `terminal_sites` joins the two, and
//!   interprocedural's `noreturn_procedures` and `terminal_calls` ask this
//!   module.
//! - Every block ends in a terminator, so the old malformed fallthrough is
//!   gone. `unreachable` states that control stops there; only `ret`
//!   returns.
//! - The frontend's cold mark is a call stating `cold`, as LLVM spells it.
//! - The old edit kept the cut tail and orphaned blocks as inert source-byte
//!   owners (`_without`); the rich MIR owns no bytes, so they go. The
//!   block ends in `unreachable`, and `cfg::_unreachable` removes what no
//!   longer runs.
//!
//! Skipped: `test_qrender_main_spill_uses_shutdown_control_proof`'s frame
//! and prologue half, which is the backend's; its noreturn half is
//! `test_a_handler_ending_in_a_runtime_exit_is_noreturn_and_cut`.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::callgraph::Defined;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{Function, InstId, Module, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Analyses, Analysis};
use llrm_mir::program::Program;
use llrm_mir::{Constant, ConstantKind, Context, GlobalId, Position};

use crate::cfg::{self, id};
use crate::effects::{self, Declarations};

/// Bodies whose CFG cannot reach a normal return.
///
/// The result is the greatest fixed point over direct calls between
/// `bodies`.  Starting at every body permits a closed recursive SCC to
/// prove terminal; every member with a reachable return or path through a
/// nonterminal call is removed, and that removal propagates to its callers.
/// A stated `noreturn` remains the independently established fact.
pub fn inferred(module: &Module, declarations: &Declarations, bodies: &BTreeSet<GlobalId>) -> BTreeSet<GlobalId> {
    let bodies: Vec<_> = module.functions().filter(|(id, _, function)| bodies.contains(id) && !function.is_declaration()).map(|(id, _, function)| (id, &module.context, declarations, function)).collect();
    fixed(&bodies, |_, proven| proven.clone())
}

/// `inferred` over a program, `declarations` each module's: a declaration
/// of a body proven stops as the body does.
pub fn inferred_in(program: &Program, declarations: &[&Declarations], bodies: &BTreeSet<Defined>) -> BTreeSet<Defined> {
    let bodies: Vec<_> = bodies
        .iter()
        .filter_map(|&(at, id)| {
            let module = &program.modules[at];
            let function = module.global(id).function().filter(|one| !one.is_declaration())?;
            Some(((at, id), &module.context, declarations[at], function))
        })
        .collect();
    fixed(&bodies, |(at, _), proven| program.local(at, proven))
}

/// The greatest set of `bodies` none of which returns, where a call stops
/// when `local` of the set so far names its callee.
fn fixed<K: Copy + Ord>(bodies: &[(K, &Context, &Declarations, &Function)], local: impl Fn(K, &BTreeSet<K>) -> BTreeSet<GlobalId>) -> BTreeSet<K> {
    let mut proven = bodies.iter().map(|&(key, ..)| key).collect::<BTreeSet<_>>();
    loop {
        let found = bodies
            .iter()
            .filter(|&&(key, context, declarations, function)| _cannot_return(function, &terminal_sites(context, declarations, function, &local(key, &proven))))
            .map(|&(key, ..)| key)
            .collect::<BTreeSet<_>>();
        if found == proven {
            return proven;
        }
        proven = found;
    }
}

/// Direct calls whose callee cannot return: named in `noreturn`, or stated
/// `noreturn`. An `invoke` may still unwind to its handler, so only a call
/// counts.
/// `terminal_sites` of no function proven noreturn.
pub struct TerminalSites;

impl Analysis for TerminalSites {
    type Result = BTreeSet<InstId>;
    const NAME: &'static str = "terminal-sites";

    fn run(context: &Context, _: &DataLayout, function: &Function, analyses: &mut Analyses) -> Self::Result {
        terminal_sites(context, &analyses.outer().globals, function, &BTreeSet::new())
    }
}

pub fn terminal_sites(context: &Context, declarations: &Declarations, function: &Function, noreturn: &BTreeSet<GlobalId>) -> BTreeSet<InstId> {
    function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::Call(_)))
        .filter(|&inst| {
            effects::callee(context, function, inst).is_some_and(|target| noreturn.contains(&target))
                || effects::states(context, declarations, function, inst, "noreturn")
        })
        .collect()
}

/// Blocks from which every path ends in a terminal call, `unreachable`, or
/// a block the frontend marked cold with a `cold` call.
///
/// A least fixed point, so a loop that never exits is not cold. When the
/// entry is cold the whole body stops, and no block is colder than another.
pub fn cold(context: &Context, declarations: &Declarations, function: &Function, terminal_calls: &BTreeSet<InstId>) -> BTreeSet<i64> {
    let blocks = cfg::graph(function);
    let marked = |at: i64| {
        function.block(cfg::block(at)).instructions().iter().any(|&inst| {
            matches!(function.instruction(inst).opcode, Opcode::Call(_) | Opcode::Invoke(_)) && effects::states(context, declarations, function, inst, "cold")
        })
    };
    let mut found = BTreeSet::new();
    let mut changed = true;
    while changed {
        changed = false;
        for block in &blocks {
            if !found.contains(&block.at) && (marked(block.at) || _ends_cold(function, block, terminal_calls, &found)) {
                found.insert(block.at);
                changed = true;
            }
        }
    }
    if blocks.first().is_some_and(|entry| found.contains(&entry.at)) { BTreeSet::new() } else { found }
}

fn _ends_cold(function: &Function, block: &cfg::Block, terminal_calls: &BTreeSet<InstId>, found: &BTreeSet<i64>) -> bool {
    let instructions = function.block(cfg::block(block.at)).instructions();
    if instructions.iter().any(|inst| terminal_calls.contains(inst)) || _stops(function, block.at) {
        return true;
    }
    !block.succ.is_empty() && block.succ.iter().all(|at| found.contains(at))
}

/// Whether `at` ends in `unreachable`: control stops there.
fn _stops(function: &Function, at: i64) -> bool {
    function.terminator(cfg::block(at)).is_some_and(|last| function.instruction(last).opcode == Opcode::Unreachable)
}

/// Whether `at` ends in `ret`.
fn _returns(function: &Function, at: i64) -> bool {
    function.terminator(cfg::block(at)).is_some_and(|last| function.instruction(last).opcode == Opcode::Ret)
}

/// Blocks from which no path reaches a return or `header`: a loop that
/// leaves into one stops the program rather than going on after the loop.
pub fn stranded(function: &Function, header: i64) -> BTreeSet<i64> {
    let blocks = cfg::graph(function);
    let predecessors = crate::graph::loops::predecessors(&blocks);
    let mut returning = BTreeSet::new();
    let mut pending = blocks.iter().map(|block| block.at).filter(|&at| _returns(function, at)).chain([header]).collect::<Vec<_>>();
    while let Some(at) = pending.pop() {
        if returning.insert(at) {
            pending.extend(predecessors.get(&at).into_iter().flatten().copied());
        }
    }
    blocks.iter().map(|block| block.at).filter(|at| !returning.contains(at)).collect()
}

pub fn _cannot_return(function: &Function, terminal_calls: &BTreeSet<InstId>) -> bool {
    let blocks = cfg::graph(function);
    let Some(entry) = blocks.first().map(|block| block.at) else { return false };
    let successors = blocks.iter().map(|block| (block.at, &block.succ)).collect::<BTreeMap<_, _>>();
    let mut pending = vec![entry];
    let mut visited = BTreeSet::new();
    while let Some(at) = pending.pop() {
        if !visited.insert(at) {
            continue;
        }
        if function.block(cfg::block(at)).instructions().iter().any(|inst| terminal_calls.contains(inst)) {
            continue;
        }
        if _returns(function, at) {
            return false;
        }
        pending.extend(successors[&at].iter().copied());
    }
    true
}

/// Remove MIR work whose execution requires a proven terminal call.
///
/// The call remains, in program order, because it is the observable
/// terminal action.  Everything after its first occurrence in that block
/// goes, and the block ends in `unreachable`, so its outgoing CFG edges go
/// too.  A block the entry no longer reaches goes with them; one other
/// predecessors still reach stays, without this block's phi inputs.
/// Already-truncated blocks are unchanged, making it safe to use at the
/// no-return fixed point boundary.
pub fn after_terminal_calls(context: &mut Context, function: &mut Function, terminal_calls: &BTreeSet<InstId>) -> bool {
    let void = context.types.void();
    let mut changed = false;
    for block in function.layout().to_vec() {
        let instructions = function.block(block).instructions().to_vec();
        let cut = instructions.iter().position(|inst| terminal_calls.contains(inst) && matches!(function.instruction(*inst).opcode, Opcode::Call(_)));
        let Some(cut) = cut else { continue };
        let tail = &instructions[cut + 1..];
        if tail.len() == 1 && _stops(function, id(block)) {
            continue;
        }
        for &inst in tail.iter().rev() {
            if let Some(result) = function.instruction(inst).result {
                let poison = context.constant(Constant { ty: function.value(result).ty, kind: ConstantKind::Poison });
                function.replace_all_uses_with(result, Operand::Constant(poison));
            }
            function.erase(inst).expect("its uses were replaced");
        }
        let stop = function.create_instruction(Opcode::Unreachable, void, Vec::new(), Default::default(), None);
        function.insert(stop, Position::End(block)).expect("a placed terminator");
        changed = true;
    }
    if changed {
        cfg::_unreachable(context, function);
    }
    changed
}

#[cfg(test)]
#[path = "noreturn_tests.rs"]
mod tests;
