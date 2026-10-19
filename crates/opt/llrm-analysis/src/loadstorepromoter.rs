//! `LoadAndStorePromoter`: the loads and stores of one memory slot, rewritten
//! so each load reads the value that reaches it.
//!
//! # How it works
//! Within a block the order of the accesses says what a load reads: the last
//! store before it, or the value that enters the block when there is none.
//! Across blocks [`SsaUpdater`] places the phis. The loads that read a stored
//! value are replaced by it; the client decides which of the accesses go
//! ([`Promotion::should_delete`]) and what an alloca stands for as a store
//! ([`Promotion::value_for_alloca`]).
//!
//! # Example
//! A slot stored in `b1` and `b2` and loaded in `b3`:
//! ```mir
//! b1: store float 1.0, ptr %p      br label %b3
//! b2: store float 2.0, ptr %p      br label %b3
//! b3: %v = load float, ptr %p
//! ```
//! `run` replaces `%v` by `phi [ 1.0, %b1 ], [ 2.0, %b2 ]` and deletes the
//! load when the client says so; the stores stay when it does not.
//!
//! # Deviations
//! - L1 No debug records: `updateDebugInfo` and `replaceLoadWithValue` are not
//!   hooks (the base class does nothing in either), and
//!   `doExtraRewritesBeforeFinalDeletion` and `instructionDeleted` are not
//!   either: no client here needs them.
//!
//! # See also
//! [`LoadAndStorePromoter::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SSAUpdater.cpp#L360).

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::module::{Function, InstId, Operand, ValueId};
use llrm_mir::opcode::Opcode;

use crate::ssaupdater::SsaUpdater;

/// What the client of the promotion decides.
pub trait Promotion {
    /// `shouldDelete`: whether `inst`, a load, a store or the alloca, goes
    /// once the loads are rewritten.
    fn should_delete(
        &self,
        function: &Function,
        inst: InstId,
    ) -> bool;

    /// `getValueToUseForAlloca`: the value the alloca stands for as a store.
    fn value_for_alloca(
        &self,
        context: &Context,
        alloca: InstId,
    ) -> Operand;
}

/// `LoadAndStorePromoter::run`. `insts` are the slot's loads, stores and, if
/// the client counts it, the alloca; `ssa` was started with the type they
/// carry.
pub fn run(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    ssa: &mut SsaUpdater,
    insts: &[InstId],
    promotion: &impl Promotion,
) {
    let is_def = |function: &Function, inst: InstId| {
        matches!(
            function.instruction(inst).opcode,
            Opcode::Store { .. } | Opcode::Alloca { .. }
        )
    };
    // The value a store or the alloca defines.
    let defined = |function: &Function, context: &Context, inst: InstId| match function.instruction(inst).opcode {
        Opcode::Alloca { .. } => promotion.value_for_alloca(context, inst),
        _ => function.instruction(inst).operands[0],
    };

    // First step: bucket up the uses by the block they occur in. This is
    // important because multiple defs and uses in a block are ours to handle:
    // `SsaUpdater` is purely for cross-block references.
    let mut by_block: IdMap<llrm_mir::module::BlockId, Vec<InstId>> = IdMap::new();
    for &user in insts {
        let block = function.parent(user).expect("an access in a block");
        by_block.get_or_insert_with(block, Vec::new).push(user);
    }

    // Now each block with uses: which loads read a value that enters the
    // block, and what each block leaves.
    let mut live_in_loads: Vec<InstId> = Vec::new();
    let mut replaced: IdMap<ValueId, Operand> = IdMap::new();
    let mut done = IdSet::new();
    for &user in insts {
        let block = function.parent(user).expect("an access in a block");
        // A block that was processed already: ignore the repeat.
        if !done.insert(block) {
            continue;
        }
        let mut uses = by_block.get(&block).cloned().unwrap_or_default();

        // A block with a single use: a store defines the value in the block,
        // a load reads the value on entry.
        if let [only] = uses[..] {
            if is_def(function, only) {
                ssa.add_available_value(block, defined(function, context, only));
            } else {
                live_in_loads.push(only);
            }
            continue;
        }

        // A block of loads alone reads the value on entry.
        if !uses.iter().any(|&one| is_def(function, one)) {
            live_in_loads.extend(uses);
            continue;
        }

        // Mixed loads and stores: sort them by their place in the block, the
        // first load before any store reads the value on entry, the last
        // store defines the value at the end.
        uses.sort_by(|&a, &b| {
            if a == b {
                std::cmp::Ordering::Equal
            } else if function.comes_before(a, b) {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        });
        let mut stored: Option<Operand> = None;
        for one in uses {
            if is_def(function, one) {
                stored = Some(defined(function, context, one));
                continue;
            }
            let load = function.instruction(one).result.expect("a load's result");
            match stored {
                // A load after a store reads the stored value.
                Some(mut value) => {
                    // Avoid assertions in unreachable code.
                    if value == Operand::Value(load) {
                        value = poison_of(context, function.value(load).ty);
                    }
                    function.replace_all_uses_with(load, value);
                    replaced.insert(load, value);
                }
                None => live_in_loads.push(one),
            }
        }
        ssa.add_available_value(block, stored.expect("a store in the block"));
    }

    // Now the loads of a value that enters their block, with phis where
    // needed.
    for load_inst in live_in_loads {
        let block = function.parent(load_inst).expect("a load in a block");
        let load = function.instruction(load_inst).result.expect("a load's result");
        let mut value = ssa.get_value_in_middle_of_block(context, layout, function, block);
        // Avoid assertions in unreachable code.
        if value == Operand::Value(load) {
            value = poison_of(context, function.value(load).ty);
        }
        function.replace_all_uses_with(load, value);
        replaced.insert(load, value);
    }

    // Everything is rewritten: the old instructions go.
    for &user in insts {
        if !promotion.should_delete(function, user) {
            continue;
        }
        // A load that still has uses was a value the updater handed out (a
        // load of a value stored later): follow the replacements to the real
        // value.
        if let Some(result) = function.instruction(user).result
            && !function.users(result).is_empty()
        {
            let mut value = *replaced.get(&result).expect("a replaced load");
            while let Operand::Value(next) = value
                && let Some(&again) = replaced.get(&next)
            {
                value = again;
            }
            function.replace_all_uses_with(result, value);
        }
        function.erase(user).expect("no users left");
    }
}

/// `PoisonValue::get`.
fn poison_of(
    context: &Context,
    ty: llrm_mir::types::TypeId,
) -> Operand {
    Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Poison }))
}
