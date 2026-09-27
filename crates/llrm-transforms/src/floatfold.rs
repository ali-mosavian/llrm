//! Float values floatfacts proves exact, replaced by their constants: LLVM's
//! InstSimplify and InstCombine for floats. Adapted from llrm-core's
//! `optimize/floatfold.rs`, the port of `qbopt/optimize/floatfold.py`.
//!
//! What changed with the IR:
//! - `stored` wrote an exact x87 value's bits where it was stored, x87
//!   having no immediate operand. Here every read of the value reads the
//!   constant, a store's included, and Dead takes the definition, as Fold's
//!   integers (`_dead_values` is Dead's).
//! - `discarded` dropped an unread exact conversion. Here every read of the
//!   conversion's integer reads the number, and Dead takes the conversion.
//!
//! Dropped, no rich MIR analogue: `checks` and `_checked`, the `Fcheck`
//! (FWAIT) kept for each removed operation's exceptions: the rich MIR
//! observes no FP exception, so gvn has nothing to call.
//!
//! Tests, in `floatfold_tests.rs`: `test_storage_requires_exact_bits` and
//! `test_exact_pair_keeps_checks_and_refuses_observable_results`, less
//! their checks. Skipped: the `checks` tests, and BC object corpora
//! (`test_fpdeep_exact_double_stores_do_not_execute_floating_arithmetic`,
//! `test_qb_fpcse_preserves_entry_when_first_load_disappears`,
//! `test_collapsed_fpcse_has_no_empty_jump_trampoline`,
//! `test_original_wait_is_an_explicit_checkpoint_with_encoding_provenance`).

use llrm_analysis::consts::{Calls, Known};
use llrm_analysis::floatfacts::{self, Finite, Format};
use llrm_analysis::manager;
use llrm_analysis::memory::Unit;
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{Function, Operand, ValueId};
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, Outer, PreservedAnalyses};
use llrm_support::hash::IndexMap;

pub struct FloatFold;

impl FunctionPass for FloatFold {
    fn name(&self) -> &'static str {
        "floatfold"
    }

    fn run(&mut self, unit: &mut passes::Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        let calls = manager::writes(unit.context, unit.layout, unit.function, analyses);
        if folded(unit.context, unit.layout, unit.function, analyses.outer(), &calls) {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// `discarded`, then `stored`, with what floatfacts knows of `function`;
/// `outer` is its module and target, `calls` what each call writes.
/// Whether anything changed.
pub fn folded(context: &mut Context, layout: &DataLayout, function: &mut Function, outer: &Outer, calls: &Calls) -> bool {
    let (facts, conversions) = {
        let unit = Unit::within(context, layout, function, outer);
        let facts = floatfacts::known(&unit, calls, None);
        let conversions = floatfacts::converted(&unit, calls, Some(&facts));
        (facts, conversions)
    };
    discarded(context, function, &conversions) | stored(context, function, &facts)
}

/// Every read of a known float value reads its constant.
pub fn stored(context: &mut Context, function: &mut Function, facts: &IndexMap<ValueId, Finite>) -> bool {
    let mut changed = false;
    for (&value, fact) in facts {
        let ty = function.value(value).ty;
        if function.users(value).is_empty() {
            continue;
        }
        let format = Format::of(&context.types, ty).expect("a float");
        let bits = floatfacts::encoded(fact, format).expect("a fact fits its format");
        let constant = context.constant(Constant { ty, kind: ConstantKind::Float(u64::try_from(bits).expect("64 bits")) });
        function.replace_all_uses_with(value, Operand::Constant(constant));
        changed = true;
    }
    changed
}

/// Every read of a known conversion result reads its number.
pub fn discarded(context: &mut Context, function: &mut Function, converted: &IndexMap<ValueId, Known>) -> bool {
    let mut changed = false;
    for (&value, fact) in converted {
        if function.users(value).is_empty() {
            continue;
        }
        let bits = u128::try_from(&fact.n).expect("a masked number") as i128;
        let constant = context.int(function.value(value).ty, bits);
        function.replace_all_uses_with(value, Operand::Constant(constant));
        changed = true;
    }
    changed
}

#[cfg(test)]
#[path = "floatfold_tests.rs"]
pub(crate) mod tests;
