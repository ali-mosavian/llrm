//! LLVM's `DropUnnecessaryAssumes` (`drop-unnecessary-assumes`;
//! DropUnnecessaryAssumes.cpp at `d1106deb71cc`): an `llvm.assume` whose
//! facts nothing else uses is deleted, with the condition only it read.
//!
//! # How it works
//! For each `llvm.assume` in layout order, `findValuesAffectedByCondition`
//! lists the values its condition says something about. If every transitive
//! user of those values is an assume or a side-effect-free, non-terminator
//! instruction (the values are ephemeral), the assume informs no code and goes,
//! then the condition and whatever only it read
//! (`RecursivelyDeleteTriviallyDeadInstructions`). The walk reads at most 32
//! users and gives up past that; an affected global keeps the assume, as its
//! use list reaches other functions. A `type_test` or `public_type_test`
//! condition is never dropped.
//!
//! # Example
//! `%x = add i16 %a, 1; %c = icmp ult i16 %x, 10; assume(%c); ret i16 %a`
//! becomes `ret i16 %a`: `%x` is read only by `%c`, which only the assume
//! reads.
//!
//! # Runs
//! Twice in the pin, in `drop-unnecessary-assumes@1577` (the start of the
//! optimizer function passes) and `drop-unnecessary-assumes@1360` (after the
//! vectorizer, `DropDereferenceable`). Late, because inlining and CSE add
//! users.
//!
//! # Limits
//! O(n) in the users of the affected values (at most 32 per assume).
//!
//! DEVIATE 1: operand-bundle assumes are not translated (the IR has no operand
//! bundles), so `DropDereferenceable` has no effect. Exit 2026-11-14.

use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::memory::Callees;
use llrm_mir::module::{Function, GlobalValue, InstId, MetadataNode, Operand};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::context::find_values_affected_by_condition;

use crate::instsimplify::delete_dead;
use crate::local::may_have_side_effects;

/// `DropUnnecessaryAssumesPass`.
pub struct DropUnnecessaryAssumes {
    /// `DropDereferenceable`: the pass the vectorizer's slot runs.
    pub drop_dereferenceable: bool,
}

impl FunctionPass for DropUnnecessaryAssumes {
    fn name(&self) -> &'static str {
        "drop-unnecessary-assumes"
    }

    fn translations(&self) -> Vec<&'static str> {
        vec![if self.drop_dereferenceable {
            "drop-unnecessary-assumes<drop-deref>"
        } else {
            "drop-unnecessary-assumes"
        }]
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = analyses.outer().clone();
        if drop_unnecessary_assumes(
            unit.context,
            &outer.globals,
            unit.metadata,
            unit.layout,
            outer.callees(),
            unit.function,
        ) {
            // `PA.preserveSet<CFGAnalyses>()`.
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// Whether the call `inst` is to the function named `name`.
fn calls(
    query: &Query,
    inst: InstId,
    name: &str,
) -> bool {
    query.callee_name(inst) == Some(name)
}

/// `affectedValuesAreEphemeral`: every user of `affected`, transitively, is an
/// assume or an instruction without side effects.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DropUnnecessaryAssumes.cpp#L20>
fn affected_values_are_ephemeral(
    context: &Context,
    callees: &Callees,
    query: &Query,
    affected: &[Operand],
) -> bool {
    const CAP: usize = 32;
    let function = query.function;
    // `SmallSetVector<Instruction *, 32>`: at most 32 entries, so a scan finds.
    let mut worklist: Vec<InstId> = Vec::with_capacity(CAP);
    let add_users = |worklist: &mut Vec<InstId>, value: Operand| -> bool {
        let Operand::Value(value) = value else { return true };
        for used in function.users(value).iter() {
            // Bail out if we need to inspect too many users.
            if worklist.len() >= CAP {
                return false;
            }
            if !worklist.contains(&used.user) {
                worklist.push(used.user);
            }
        }
        true
    };

    for &value in affected {
        // Do not handle assumes on globals for now. The use list for them may
        // contain uses in other functions.
        if !matches!(value, Operand::Value(_)) || !add_users(&mut worklist, value) {
            return false;
        }
    }

    let mut index = 0;
    while index < worklist.len() {
        let inst = worklist[index];
        index += 1;
        // Use in assume is ephemeral.
        if calls(query, inst, "llvm.assume") {
            continue;
        }
        // Use in side-effecting instruction is non-ephemeral.
        if may_have_side_effects(context, callees, query.globals, function, inst)
            || function.instruction(inst).opcode.is_terminator()
        {
            return false;
        }
        // Otherwise, recursively look at the users.
        if let Some(result) = function.instruction(inst).result
            && !add_users(&mut worklist, Operand::Value(result))
        {
            return false;
        }
    }
    true
}

/// `DropUnnecessaryAssumesPass::run`: whether any assume was dropped.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DropUnnecessaryAssumes.cpp#L61>
pub fn drop_unnecessary_assumes(
    context: &Context,
    globals: &[GlobalValue],
    metadata: &[MetadataNode],
    layout: &DataLayout,
    callees: &Callees,
    function: &mut Function,
) -> bool {
    let assumes: Vec<InstId> = {
        let query = Query { globals, metadata, ..Query::bare(context, function, layout) };
        function
            .layout()
            .iter()
            .flat_map(|&block| function.iter_block(block))
            .filter(|&inst| calls(&query, inst, "llvm.assume"))
            .collect()
    };

    let mut changed = false;
    for assume in assumes {
        let (cond, ephemeral) = {
            let query = Query { globals, metadata, ..Query::bare(context, function, layout) };
            let cond = function.instruction(assume).operands[0];
            // Don't drop type tests, which have special semantics.
            if let Some((def, _)) = query.instruction_of(cond)
                && (calls(&query, def, "llvm.type.test") || calls(&query, def, "llvm.public.type.test"))
            {
                continue;
            }
            let mut affected: Vec<Operand> = Vec::new();
            find_values_affected_by_condition(&query, cond, true, &mut |value| affected.push(value));
            (cond, affected_values_are_ephemeral(context, callees, &query, &affected))
        };
        if !ephemeral {
            continue;
        }
        function.erase(assume).expect("an assume nothing reads");
        let query = Query { globals, metadata, ..Query::bare(context, function, layout) };
        if let Some((def, _)) = query.instruction_of(cond) {
            delete_dead(context, callees, globals, function, vec![def]);
        }
        changed = true;
    }
    changed
}

#[cfg(test)]
#[path = "dropunnecessaryassumes_tests.rs"]
mod tests;
