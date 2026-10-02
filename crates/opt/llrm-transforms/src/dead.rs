//! Instructions nothing needs, removed: LLVM's ADCE and DCE. Adapted from
//! llrm-core's `optimize/transform.rs` `Dead` (`dead`, `_kept`,
//! `_removable`).
//!
//! What changed with the IR:
//! - What stays whatever reads it (`_kept`: the old observed kinds, stores
//!   and barriers) is what `llrm_mir::memory::only_value` says is more than
//!   a value. A call with no effect that returns goes; the old MIR kept
//!   every call.
//! - A removed operation left an empty byte-owning marker; here it goes.
//! - Phis go after the operations, so a phi only dead work read goes in the
//!   same run rather than the next.
//!
//! Dropped, no rich MIR analogue: `_overwritten_locally` and the limited
//! mode it served (an opaque or barrier operation reading registers its
//! operands do not name; every read here is an operand), and the cleanup of
//! merged halves.
//!
//! Tests, in `dead_tests.rs`: `test_dead_code_goes_and_the_bytes_are_still_accounted_for`
//! is ported. Skipped: `test_dead_code_leaves_a_body_it_cannot_read_alone`
//! (the limited mode).

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

    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        if dead(unit.context, analyses.outer().callees(), unit.function) {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// Instructions whose results nothing that stays reads, removed. Whether
/// any went.
pub fn dead(context: &mut Context, callees: &Callees, function: &mut Function) -> bool {
    let markers = _unneeded_markers(context, callees, function);
    for &marker in &markers {
        function.erase(marker).expect("a call with no result");
    }
    let alive = live(context, callees, function);
    let gone = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| function.instruction(inst).opcode != Opcode::Phi && _removable(context, callees, function, inst, &alive))
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
fn _unneeded_markers(context: &Context, callees: &Callees, function: &Function) -> Vec<InstId> {
    let marker = |inst: InstId| memory::lifetime(context, callees, function, inst).is_some();
    let alive = crate::transform::live_except(context, callees, function, marker);
    function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| matches!(memory::lifetime(context, callees, function, inst), Some(Operand::Value(object)) if !alive.contains(&object)))
        .collect()
}

/// Whether `inst` stays whatever reads it.
pub fn _kept(context: &Context, callees: &Callees, function: &Function, inst: InstId) -> bool {
    !memory::only_value(context, callees, function, inst)
}

/// Whether anything at all would notice `inst` going.
pub fn _removable(context: &Context, callees: &Callees, function: &Function, inst: InstId, alive: &BTreeSet<ValueId>) -> bool {
    !_kept(context, callees, function, inst) && function.instruction(inst).result.is_none_or(|result| !alive.contains(&result))
}

#[cfg(test)]
#[path = "dead_tests.rs"]
mod tests;
