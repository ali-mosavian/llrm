//! Instructions nothing needs, removed: LLVM's ADCE and DCE. Work whose result
//! no one reads only costs code and clocks, and other passes leave it behind.
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
//! `markers_dropped` is a separate step over a module. It erases every call
//! of `llvm.assume` and then runs `dead` on each function until nothing
//! changes, so a condition kept alive only by its assume costs no compare.
//!
//! # Example
//! Before:
//! ```mir
//! define i16 @f(i16 %x) {
//! b0:
//!   %doomed = mul i16 %x, 3
//!   %kept = add i16 %x, 1
//!   ret i16 %kept
//! }
//! ```
//! After:
//! ```mir
//! define i16 @f(i16 %x) {
//! b0:
//!   %kept = add i16 %x, 1
//!   ret i16 %kept
//! }
//! ```
//! `%doomed` has no reader and no effect, so it goes.
//!
//! From unit test `test_dead_code_goes_and_the_bytes_are_still_accounted_for`
//! in `dead_tests.rs`.
//!
//! # Runs
//! Once per pass of the function fixed point, after `algebraic`, and again
//! after the loop-header copy (`-ftree-ch`). `lsr`, `window` and `indvars`
//! call it on the code they leave. `markers_dropped` runs once
//! after the whole pipeline, before exception-handling preparation and
//! selection, once every reader has used the assumptions. `-fno-tree-dce` turns
//! off the pass only, not those calls; nothing runs at -O0.
//!
//! # Keeps
//! Blocks and edges are unchanged: dominators and loops are preserved.
//!
//! # See also
//! [`crate::deadstore`] removes dead stores; [`crate::promote`] makes the loads
//! that make a store dead; [`crate::loopmotion`] sinks stores out of loops.

use std::collections::BTreeSet;

use llrm_analysis::ssa;
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::memory::{self, Callees};
use llrm_mir::module::{Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};

use crate::transform::live;

#[cfg(test)]
thread_local! {
    /// Instructions the whole-function `dead` was given, for the tests that
    /// count them.
    pub(crate) static SCANNED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub struct Dead;

impl FunctionPass for Dead {
    fn adds_memory_operations(&self) -> bool {
        false
    }

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
    context: &Context,
    callees: &Callees,
    function: &mut Function,
) -> bool {
    #[cfg(test)]
    SCANNED.with(|scanned| {
        scanned.set(scanned.get() + function.layout().iter().map(|&block| function.block(block).len()).sum::<usize>())
    });
    let markers = _unneeded_markers(context, callees, function);
    function.erase_all(&markers).expect("a call with no result");
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
    function.erase_all(&gone).expect("nothing reads it");
    ssa::pruned_phis(function, &alive) | !gone.is_empty() | !markers.is_empty()
}

/// `dead`, for a function that was free of dead code before an edit: only what
/// the edit stopped reading (`seeds`) can have died. An instruction is dead
/// when nothing that stays is reachable from it through its readers; so the
/// readers of a seed are followed until one that stays is met (the seed lives),
/// or none is (all of them go, a loop-carried cycle too), and what the dead
/// read becomes a seed in its turn. The cost is the cone of the seeds, not the
/// function.
pub fn dead_from(
    context: &Context,
    callees: &Callees,
    function: &mut Function,
    seeds: impl IntoIterator<Item = ValueId>,
) -> bool {
    let mut pending: Vec<ValueId> = seeds.into_iter().collect();
    let mut asked = llrm_mir::dense::IdSet::<ValueId>::new();
    let mut gone = false;
    while let Some(value) = pending.pop() {
        if !asked.insert(value) {
            continue;
        }
        let ValueDef::Instruction(start) = function.value(value).def else { continue };
        if function.parent(start).is_none() {
            continue;
        }
        let Some(cone) = _cone(context, callees, function, start) else { continue };
        let members: llrm_mir::dense::IdSet<InstId> = cone.iter().copied().collect();
        for &inst in &cone {
            for &operand in &function.instruction(inst).operands {
                if let Operand::Value(read) = operand
                    && let ValueDef::Instruction(defining) = function.value(read).def
                    && !members.contains(&defining)
                {
                    asked.remove(&read);
                    pending.push(read);
                }
            }
        }
        for &inst in &cone {
            function.set_operands(inst, Vec::new());
        }
        for &inst in &cone {
            function.erase(inst).expect("only the dead read the dead");
        }
        gone = true;
    }
    gone
}

/// `start` and every instruction that reads it, and those, so on: none where
/// one of them stays whatever reads it, which keeps `start`.
fn _cone(
    context: &Context,
    callees: &Callees,
    function: &Function,
    start: InstId,
) -> Option<Vec<InstId>> {
    let mut seen = llrm_mir::dense::IdSet::<InstId>::new();
    let (mut stack, mut cone) = (vec![start], Vec::new());
    while let Some(inst) = stack.pop() {
        if !seen.insert(inst) {
            continue;
        }
        if _kept(context, callees, function, inst) {
            return None;
        }
        cone.push(inst);
        if let Some(result) = function.instruction(inst).result {
            stack.extend(function.users(result).iter().map(|one| one.user));
        }
    }
    Some(cone)
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
    alive: &llrm_mir::dense::IdSet<ValueId>,
) -> bool {
    !_kept(context, callees, function, inst)
        && function.instruction(inst).result.is_none_or(|result| !alive.contains(&result))
}

#[cfg(test)]
#[path = "dead_tests.rs"]
mod tests;

/// The calls of `llvm.assume` and `llrm.noalias` gone from `module`, and what
/// only they read: the last step before selection, once every reader has used
/// them. A condition kept alive by its assume would cost a compare for nothing;
/// a restrict pointer is its operand.
pub fn markers_dropped(module: &mut llrm_mir::Module) {
    use llrm_mir::intrinsics::Intrinsic;
    let named = |wanted: Intrinsic| {
        module
            .globals
            .iter()
            .enumerate()
            .filter(|(_, global)| global.intrinsic() == Some(wanted))
            .map(|(at, _)| llrm_mir::GlobalId(at as u32))
            .collect::<BTreeSet<_>>()
    };
    let (assumes, restricts) = (named(Intrinsic::Assume), named(Intrinsic::Restrict));
    if assumes.is_empty() && restricts.is_empty() {
        return;
    }
    let callees = llrm_mir::memory::callees(module);
    let llrm_mir::Module { context, globals, .. } = module;
    for global in globals.iter_mut() {
        let llrm_mir::GlobalKind::Function(function) = &mut global.kind else { continue };
        let calls = function
            .walk()
            .map(|(_, inst)| inst)
            .filter_map(|inst| {
                let callee = llrm_mir::memory::callee(context, function, inst)?;
                (assumes.contains(&callee) || restricts.contains(&callee))
                    .then_some((inst, restricts.contains(&callee)))
            })
            .collect::<Vec<_>>();
        for (inst, restrict) in calls {
            if restrict {
                let result = function.instruction(inst).result.expect("a restrict pointer has a value");
                let pointer = function.instruction(inst).operands[0];
                function.replace_all_uses_with(result, pointer);
            }
            function.erase(inst).expect("a call nothing reads");
        }
        while dead(context, &callees, function) {}
    }
}
