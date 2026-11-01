//! A block that only a branch reaches and that jumps back to the branch's
//! other target runs before the branch, its phis in the target becoming
//! selects: the `speculativelyExecuteBB` step of SimplifyCFG.
//!
//! # LLVM structure
//! | LLVM | here | |
//! |---|---|---|
//! | [`speculativelyExecuteBB`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L3244) | `executed` | MIRROR; D1, D2, D3 |
//! | [`validateAndCostRequiredSelects`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L3130) | `selects_priced` | MIRROR; D4 |
//! | [`passingValueIsAlwaysUndefined`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9094) | [`alwaysundef::always_undefined`](crate::alwaysundef) | MIRROR |
//! | `phi-node-folding-threshold` (2) | `THRESHOLD` | MIRROR |
//!
//! The call sites are in
//! [`simplifyCondBranch`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9060),
//! behind `SpeculateBlocks`. One instruction at most runs early, and a
//! value of the branch's block that only that instruction reads counts as one
//! more.
//!
//! - D1. Loads that fault conditionally (`HoistLoadsStoresWithCondFaulting`, a
//!   target option) are not ported: a load runs early only where it is safe to
//!   speculate. A store runs early (`HoistCondStores`) with
//!   `isSafeToSpeculateStore` (D6).
//! - D2. No branch weights or `!unpredictable` (`isProfitableToSpeculate` is
//!   true), no ephemeral values (no assumption cache), no pseudo probes. A
//!   `minsize` function is priced as `CodeSize` (`cost_kind`).
//! - D3. A hoisted instruction keeps the metadata kinds LLVM keeps and loses
//!   the rest, as in [`twoentryphi`](crate::twoentryphi) (D4 there).
//! - D4. No constant expressions in function bodies: LLVM's price of one is not
//!   needed.
//!
//! - D5. A target whose `select` is a branch (`Tti::select_is_branch`) does not
//!   run an arm early to form one, as in [`twoentryphi`](crate::twoentryphi).
//! - D6. `isSafeToSpeculateStore` finds a writable object only among the
//!   allocas that are not captured: `isWritableObject` of a global would need
//!   `PointerMayBeCaptured` over the module.
//!
//! # Example
//! The arm's one cheap instruction runs before the branch and the phi becomes a
//! select:
//!
//! ```text
//! entry: br i1 %c, label %then, label %end
//! then:  %t = add i32 %x, 1 ; br label %end
//! end:   %p = phi i32 [ %t, %then ], [ %x, %entry ]
//! ; becomes, in entry: %t = add i32 %x, 1 ; %p = select i1 %c, i32 %t, i32 %x
//! ```

use llrm_mir::context::Context;
use llrm_mir::dense::IdMap;
use llrm_mir::edit::Position;
use llrm_mir::instructioncost::instruction_cost;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::Opcode;
use llrm_mir::tti::{CmpSel, Cost, CostKind, Ty};
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::underlying::get_underlying_object;

use crate::twoentryphi::{Speculation, metadata_dropped};

/// `phi-node-folding-threshold`, in `TCC_Basic`.
const THRESHOLD: i64 = 2;

/// The cost kind SimplifyCFG prices speculation with: `TCK_CodeSize` for a
/// `minsize` function, else `TCK_SizeAndLatency`
/// (`SimplifyCFG.cpp` `validateAndCostRequiredSelects`,
/// `performBranchToCommonDestFolding`).
pub(crate) fn cost_kind(function: &Function) -> CostKind {
    let minsize = function
        .attrs
        .iter()
        .any(
            |attr| matches!(
                attr,
                llrm_mir::opcode::Attribute::Flag(name) if name == "minsize"
            ),
        );
    if minsize { CostKind::CodeSize } else { CostKind::SizeAndLatency }
}

/// `isSafeToSpeculateStore`: where `store` writes what a store or a load just
/// before the branch of `block` read or wrote, the location is known to be
/// writable and the value that was there is the value to store instead.
fn store_speculated(
    context: &Context,
    spec: &Speculation,
    function: &Function,
    store: InstId,
    block: BlockId,
) -> Option<Operand> {
    let Opcode::Store { align, volatile: false } = function.instruction(store).opcode else { return None };
    let (value, pointer) = (function.instruction(store).operands[0], function.instruction(store).operands[1]);
    let ty = function.operand_type(context, value)?;
    // The nine instructions before the branch.
    for inst in function.iter_block_rev(block).take(9) {
        let operation = function.instruction(inst);
        if llrm_mir::memory::of(context, spec.outer.callees(), function, inst).writes
            && !matches!(operation.opcode, Opcode::Store { .. })
        {
            return None;
        }
        match operation.opcode {
            Opcode::Store { align: held, volatile } => {
                // The previous store of the location, as simple and as aligned.
                return (operation.operands[1] == pointer
                    && function.operand_type(context, operation.operands[0]) == Some(ty)
                    && !volatile
                    && held >= align)
                    .then_some(operation.operands[0]);
            }
            Opcode::Load { align: held, volatile: false }
                if operation.operands[0] == pointer && operation.ty == ty && held >= align =>
            {
                let query = Query::bare(context, function, spec.layout);
                let object = get_underlying_object(&query, pointer, 0);
                // `isWritableObject` and `PointerMayBeCaptured`: an alloca
                // that does not escape. D6.
                if let Operand::Value(root) = object
                    && let ValueDef::Instruction(def) = function.value(root).def
                    && matches!(function.instruction(def).opcode, Opcode::Alloca { .. })
                    && llrm_analysis::capturetracking::pointer_may_be_captured(
                        function,
                        &spec.outer.globals,
                        context,
                        root,
                        llrm_analysis::capturetracking::CaptureComponents::PROVENANCE,
                        |cc| {
                            cc.intersect(llrm_analysis::capturetracking::CaptureComponents::PROVENANCE)
                                == llrm_analysis::capturetracking::CaptureComponents::PROVENANCE
                        },
                        llrm_analysis::capturetracking::DEFAULT_MAX_USES_TO_EXPLORE,
                    )
                    .without_ret
                    .captures_nothing()
                {
                    return operation.result.map(Operand::Value);
                }
            }
            _ => {}
        }
    }
    None
}

/// The cost of the selects `end`'s phis need once `then` runs early, and
/// whether any phi needs one; None where a phi cannot be a select.
fn selects_priced(
    context: &Context,
    spec: &Speculation,
    function: &Function,
    block: BlockId,
    then: BlockId,
    end: BlockId,
) -> Option<(Cost, bool)> {
    let mut cost = Cost::FREE;
    let mut rewritable = false;
    for phi in crate::edges::phis(function, end) {
        let arms = llrm_analysis::occurrence::arms(function, phi);
        let from = |at: BlockId| arms.iter().find(|&&(_, one)| one == at).map(|&(value, _)| value);
        let (Some(original), Some(speculated)) = (from(block), from(then)) else { return None };
        if original == speculated {
            continue;
        }
        // D5 (see `twoentryphi`): a select that is a branch is no saving.
        if spec.tti.select_is_branch() {
            return None;
        }
        let ty = Ty::of(&context.types, function.instruction(phi).ty)?;
        cost = cost + spec.tti.cmp_sel_cost(CmpSel::Select, ty, cost_kind(function));
        if crate::alwaysundef::always_undefined(context, spec, function, original, phi, false)
            || crate::alwaysundef::always_undefined(context, spec, function, speculated, phi, false)
        {
            // `validateAndCostRequiredSelects` is false, a speculated store
            // converts regardless.
            return Some((cost, false));
        }
        rewritable = true;
    }
    Some((cost, rewritable))
}

/// `then` (reached only by the branch ending `block`, and jumping to `end`,
/// the branch's other target) run before the branch. Whether it was.
pub fn executed(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
    then: BlockId,
) -> bool {
    let Some(branch) = function.terminator(block) else { return false };
    let operands = function.instruction(branch).operands.clone();
    let [condition, Operand::Block(first), Operand::Block(second)] = operands[..] else { return false };
    let Some(end) = function.terminator(then).and_then(|last| match function.instruction(last).operands[..] {
        [Operand::Block(end)] => Some(end),
        _ => None,
    }) else {
        return false;
    };
    let invert = then != first;
    if end != if invert { first } else { second } {
        return false;
    }

    let mut speculated = 0;
    // The store that runs early, and the value the location holds without it.
    let mut stored: Option<(InstId, Operand)> = None;
    // Uses in `then` of the values of `block` that nothing else reads.
    let mut sinkable: IdMap<InstId, usize> = IdMap::new();
    let body = function.iter_block(then).collect::<Vec<_>>();
    for &inst in body[..body.len() - 1].iter().rev() {
        speculated += 1;
        if speculated > 1 {
            return false;
        }
        // Don't hoist the instruction if it's unsafe or expensive.
        if !crate::licm::hoist::is_safe_to_speculatively_execute_for_cfg(
            context,
            spec.layout,
            function,
            spec.outer,
            inst,
        ) {
            if stored.is_some() {
                return false;
            }
            let Some(held) = store_speculated(context, spec, function, inst, block) else { return false };
            stored = Some((inst, held));
        }
        if stored.is_none() {
            let price = instruction_cost(
                spec.tti,
                context,
                spec.layout,
                &spec.outer.globals,
                function,
                inst,
                cost_kind(function),
            );
            if !price.valid || price.value > THRESHOLD {
                return false;
            }
        }
        // An operand of `block` that only `then` reads stays behind otherwise.
        for &operand in &function.instruction(inst).operands {
            let Operand::Value(id) = operand else { continue };
            let ValueDef::Instruction(def) = function.value(id).def else { continue };
            if function.parent(def) != Some(block)
                || !llrm_mir::memory::only_value(context, spec.outer.callees(), function, def)
            {
                continue;
            }
            let seen = sinkable.get(&def).copied().unwrap_or(0);
            sinkable.insert(def, seen + 1);
        }
    }
    for (def, uses) in sinkable.iter() {
        let result = function.instruction(def).result.expect("a value used as an operand");
        if function.users(result).has_n_uses(*uses) {
            speculated += 1;
            if speculated > 1 {
                return false;
            }
        }
    }

    let Some((cost, rewritable)) = selects_priced(context, spec, function, block, then, end) else { return false };
    if !(stored.is_some() || rewritable) || cost.value > THRESHOLD * Cost::BASIC.value || !cost.valid {
        return false;
    }

    // The stored value is the one the location already holds unless the
    // branch is taken.
    if let Some((store, held)) = stored {
        let value = function.instruction(store).operands[0];
        let (on_true, on_false) = if invert { (held, value) } else { (value, held) };
        let ty = function.operand_type(context, value).expect("a stored value");
        let select = function.create_instruction(
            Opcode::Select,
            ty,
            vec![condition, on_true, on_false],
            Default::default(),
            Some("spec.store.select"),
        );
        function.insert(select, Position::Before(branch)).expect("a new instruction");
        let chosen = Operand::Value(function.instruction(select).result.expect("a select's value"));
        function.set_operand(store, 0, chosen);
    }

    for &inst in &body[..body.len() - 1] {
        metadata_dropped(function, inst);
        function.move_to(inst, Position::Before(branch)).expect("a placed instruction");
    }
    for phi in crate::edges::phis(function, end) {
        let arms = llrm_analysis::occurrence::arms(function, phi);
        let at = |from: BlockId| arms.iter().position(|&(_, one)| one == from).expect("an arm");
        let (original_at, then_at) = (at(block), at(then));
        let (original, speculated) = (arms[original_at].0, arms[then_at].0);
        if original == speculated {
            continue;
        }
        let (on_true, on_false) = if invert { (original, speculated) } else { (speculated, original) };
        let operation = function.instruction(phi);
        let (ty, flags) = (operation.ty, operation.flags);
        let select = function.create_instruction(Opcode::Select, ty, vec![condition, on_true, on_false], flags, None);
        function.insert(select, Position::Before(branch)).expect("a new instruction");
        let chosen = Operand::Value(function.instruction(select).result.expect("a select's value"));
        function.set_operand(phi, 2 * original_at, chosen);
        function.set_operand(phi, 2 * then_at, chosen);
    }
    true
}
