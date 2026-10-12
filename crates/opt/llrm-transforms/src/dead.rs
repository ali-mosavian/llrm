//! Instructions nothing needs, removed: LLVM's ADCE and DCE.
//!
//! # How it works
//! An instruction stays whatever reads it when `llrm_mir::memory::only_value`
//! says it is more than a value: a store, a volatile access, a call that may
//! touch memory or not return, a terminator. A call with no effect that returns
//! is a value and goes.
//!
//! `transform::live` marks, to a fixed point, every value a staying instruction
//! reads, directly or through the instructions defining them. An instruction
//! that stays nowhere and whose result is unmarked is dead. Dead instructions
//! read only dead ones, so their results are replaced by poison, then all are
//! erased in one step; phis are handled last (`ssa::pruned_phis`), so a phi
//! that only dead work read, including a loop-carried cycle nothing outside
//! reads, goes in the same run.
//!
//! ```text
//!  %q = mul i16 %a, %b           (nothing reads %q or %p)
//!  %p = add i16 %q, 1       ==>  (both gone)
//!  store i16 %a, ptr @g          store i16 %a, ptr @g
//! ```
//!
//! Lifetime markers go first: those of an object that, leaving the markers out,
//! nothing that stays reads are erased with it.
//!
//! `assumptions_dropped` is a separate step over a module. It erases every call
//! of `llvm.assume` and then runs `dead` on each function until nothing
//! changes, so a condition kept alive only by its assume costs no compare.
//!
//! # Runs
//! Once per pass of the function fixed point, after `algebraic`, and again
//! after the loop-header copy (`-ftree-ch`). `lsr`, `window`, `indvars` and
//! `jumpthread` call it on the code they leave. `assumptions_dropped` runs once
//! after the whole pipeline, before exception-handling preparation and
//! selection, once every reader has used the assumptions. `-fno-tree-dce` turns
//! off the pass only, not those calls; nothing runs at -O0.
//!
//! # Keeps
//! Blocks and edges are unchanged: dominators and loops are preserved.
//!
//! # See also
//! [`crate::dse`] removes dead stores; [`crate::promote`] makes the loads that
//! make a store dead; [`crate::loopmotion`] sinks stores out of loops.

use std::collections::BTreeSet;

use llrm_analysis::ssa;
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::memory::{self, Callees};
use llrm_mir::module::{Function, InstId, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};

use crate::transform::live;

pub struct Dead;

impl FunctionPass for Dead {
    fn name(&self) -> &'static str {
        "dead"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if dead(unit.context, analyses.outer().callees(), unit.function) {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// Instructions whose results nothing that stays reads, removed. Whether
/// any went.
pub fn dead(
    context: &mut Context,
    callees: &Callees,
    function: &mut Function,
) -> bool {
    let markers = _unneeded_markers(context, callees, function);
    for &marker in &markers {
        function.erase(marker).expect("a call with no result");
    }
    let alive = live(context, callees, function);
    let gone = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| {
            function.instruction(inst).opcode != Opcode::Phi && _removable(context, callees, function, inst, &alive)
        })
        .collect::<Vec<_>>();
    // Only the dead read the dead, and phis `pruned_phis` drops next.
    for &inst in &gone {
        let instruction = function.instruction(inst);
        if let Some(result) = instruction.result {
            let poison = context.constant(Constant { ty: instruction.ty, kind: ConstantKind::Poison });
            function.replace_all_uses_with(result, Operand::Constant(poison));
        }
    }
    for &inst in &gone {
        function.erase(inst).expect("nothing reads it");
    }
    ssa::pruned_phis(function, &alive) | !gone.is_empty() | !markers.is_empty()
}

/// The lifetime markers of an object nothing else that stays reads: only
/// the markers kept it, and they say nothing of a thing nobody uses.
fn _unneeded_markers(
    context: &Context,
    callees: &Callees,
    function: &Function,
) -> Vec<InstId> {
    let marker = |inst: InstId| memory::lifetime(context, callees, function, inst).is_some();
    let alive = crate::transform::live_except(context, callees, function, marker);
    function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| {
            matches!(
                memory::lifetime(context, callees, function, inst),
                Some(Operand::Value(object)) if !alive.contains(&object)
            )
        })
        .collect()
}

/// Whether `inst` stays whatever reads it.
pub fn _kept(
    context: &Context,
    callees: &Callees,
    function: &Function,
    inst: InstId,
) -> bool {
    !memory::only_value(context, callees, function, inst)
}

/// Whether anything at all would notice `inst` going.
pub fn _removable(
    context: &Context,
    callees: &Callees,
    function: &Function,
    inst: InstId,
    alive: &BTreeSet<ValueId>,
) -> bool {
    !_kept(context, callees, function, inst)
        && function.instruction(inst).result.is_none_or(|result| !alive.contains(&result))
}

#[cfg(test)]
#[path = "dead_tests.rs"]
mod tests;

/// The calls of `llvm.assume` gone from `module`, and what only they read:
/// the last step before selection, once every reader has used them. A
/// condition kept alive by its assume would cost a compare for nothing.
pub fn assumptions_dropped(module: &mut llrm_mir::Module) {
    let assumes = module
        .globals
        .iter()
        .enumerate()
        .filter(|(_, global)| {
            global.name.as_deref().and_then(llrm_mir::intrinsics::Intrinsic::named)
                == Some(llrm_mir::intrinsics::Intrinsic::Assume)
        })
        .map(|(at, _)| llrm_mir::GlobalId(at as u32))
        .collect::<BTreeSet<_>>();
    if assumes.is_empty() {
        return;
    }
    let callees = llrm_mir::memory::callees(module);
    let llrm_mir::Module { context, globals, .. } = module;
    for global in globals.iter_mut() {
        let llrm_mir::GlobalKind::Function(function) = &mut global.kind else { continue };
        let calls = function
            .walk()
            .map(|(_, inst)| inst)
            .filter(|&inst| {
                llrm_mir::memory::callee(context, function, inst).is_some_and(|callee| assumes.contains(&callee))
            })
            .collect::<Vec<_>>();
        for inst in calls {
            function.erase(inst).expect("a call of void is read by nothing");
        }
        while dead(context, &callees, function) {}
    }
}
