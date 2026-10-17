//! `hoistRegion` and what it asks (`LICM.cpp`): decisions only.

use llrm_analysis::cfg;
use llrm_analysis::mssa::alias::AliasQuery;
use llrm_analysis::mssa::{AccessId, Kind, Mssa};
use llrm_mir::context::ConstantKind;
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::{Attribute, BinaryOp, Opcode};
use llrm_mir::passes::{self, Outer};

use super::{Flags, LoopCtx, LoopSafety, calls};

/// [`LoopBlocksRPO`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/LoopIterator.h#L110):
/// the loop's blocks in reverse postorder from the header, back edges not
/// followed.
fn loop_blocks_rpo(ctx: &LoopCtx) -> Vec<BlockId> {
    let header = cfg::block(ctx.loop_.header);
    let mut seen = llrm_mir::dense::IdSet::new();
    let mut post = Vec::new();
    let mut stack = vec![(header, ctx.function.successors(header), 0usize)];
    seen.insert(header);
    while let Some((block, successors, next)) = stack.last_mut() {
        if let Some(&succ) = successors.get(*next) {
            *next += 1;
            if ctx.contains_block(succ) && seen.insert(succ) {
                let successors = ctx.function.successors(succ);
                stack.push((succ, successors, 0));
            }
        } else {
            post.push(*block);
            stack.pop();
        }
    }
    post.reverse();
    post
}

/// [`hoistRegion`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L645):
/// the instructions it hoists to `preheader`. Not built (L1): the
/// reciprocal of an `fdiv` (MIR has no `arcp`), `hoistInsertPastInsert`
/// (no vectors), the invariant-start and guard intrinsics, and
/// `hoistArithmetics`.
#[allow(clippy::too_many_arguments)]
pub fn hoist_region(
    unit: &passes::Unit,
    outer: &Outer,
    ctx: &LoopCtx,
    preheader: BlockId,
    mssa: &mut Mssa,
    aa: &AliasQuery,
    flags: &mut Flags,
    safety: &LoopSafety,
) -> Vec<(InstId, BlockId)> {
    let mut found = Vec::new();
    for block in loop_blocks_rpo(ctx) {
        // Only the contents of a block outside the subloops: those have been
        // processed.
        if ctx.in_sub_loop(block) {
            continue;
        }
        for inst in unit.function.block(block).instructions().to_vec() {
            // Erased or sunk by `sinkRegion`.
            if ctx.state.gone.borrow().contains(&inst) {
                continue;
            }
            // Try hoisting the instruction out to the preheader. We can only do
            // this if all of the operands of the instruction are loop invariant
            // and if it is safe to hoist the instruction.
            if ctx.has_loop_invariant_operands(inst)
                && can_sink_or_hoist_inst(unit, outer, ctx, mssa, aa, flags, safety, inst, true)
                && is_safe_to_execute_unconditionally(unit, outer, ctx, safety, preheader, inst)
            {
                ctx.state.moved.borrow_mut().insert(inst, preheader);
                found.push((inst, preheader));
            }
        }
    }
    found
}

/// [`isSafeToExecuteUnconditionally`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L1507)
/// (`AllowSpeculation` is `LicmAllowSpeculation`, true).
fn is_safe_to_execute_unconditionally(
    unit: &passes::Unit,
    outer: &Outer,
    _ctx: &LoopCtx,
    safety: &LoopSafety,
    _preheader: BlockId,
    inst: InstId,
) -> bool {
    is_safe_to_speculatively_execute(unit, outer, inst) || safety.is_guaranteed_to_execute(inst)
}

/// [`isSafeToSpeculativelyExecuteWithOpcode`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7575).
/// DEVIATE: a load is safe where its bytes are dereferenceable or the target
/// does not trap on it at its alignment (`Machine::load_may_trap`), in place
/// of `isDereferenceableAndAlignedPointer` (the same rule as `hoist`'s
/// `_may_fault`).
pub fn is_safe_to_speculatively_execute(
    unit: &passes::Unit,
    outer: &Outer,
    inst: InstId,
) -> bool {
    let function = &*unit.function;
    let instruction = function.instruction(inst);
    let constant_int = |operand: Operand| match operand {
        Operand::Constant(id) => match unit.context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits),
            _ => None,
        },
        _ => None,
    };
    match &instruction.opcode {
        Opcode::Binary(BinaryOp::UDiv | BinaryOp::URem) => {
            constant_int(instruction.operands[1]).is_some_and(|d| d != 0)
        }
        Opcode::Binary(op @ (BinaryOp::SDiv | BinaryOp::SRem)) => {
            let _ = op;
            let width = unit.context.types.int_bits(instruction.ty).expect("an integer division");
            let Some(denominator) = constant_int(instruction.operands[1]) else { return false };
            if denominator == 0 {
                return false;
            }
            if denominator != llrm_mir::context::mask(width) {
                return true;
            }
            // The denominator is -1: safe as long as the numerator is not
            // INT_MIN.
            constant_int(instruction.operands[0]).is_some_and(|numerator| numerator != 1u128 << (width - 1))
        }
        Opcode::Load { volatile, .. } => {
            if *volatile {
                return false;
            }
            let (pointer, bytes) =
                (instruction.operands[0], unit.layout.store_size(&unit.context.types, instruction.ty));
            llrm_mir::valuetracking::dereferenceable(unit.context, unit.layout, outer.sizes(), function, pointer, bytes)
                || !outer.target().load_may_trap(
                    bytes,
                    llrm_mir::valuetracking::alignment(unit.context, unit.layout, &outer.globals, function, pointer),
                )
        }
        Opcode::Call(info) => {
            // The called function could have undefined behavior or
            // side-effects, even if marked readnone nounwind.
            if !calls::has_flag(outer, unit.context, function, inst, "speculatable") {
                return false;
            }
            // Since the operands may be changed after hoisting, undefined
            // behavior may be triggered by some UB-implying attributes.
            let implies_ub = |attrs: &Vec<Attribute>| {
                attrs
                    .iter()
                    .any(
                        |attr| match attr {
                            Attribute::Flag(name) => name == "noundef" || name == "nonnull",
                            Attribute::Int(name, _) => name == "align" || name.starts_with("dereferenceable"),
                            Attribute::Range { .. } => true,
                            _ => false,
                        },
                    )
            };
            !(implies_ub(&info.return_attrs) || info.argument_attrs.iter().any(implies_ub))
        }
        Opcode::Alloca { .. }
        | Opcode::Invoke(_)
        | Opcode::Phi
        | Opcode::Store { .. }
        | Opcode::Ret
        | Opcode::Br
        | Opcode::Switch
        | Opcode::Unreachable
        | Opcode::LandingPad { .. }
        | Opcode::Resume => false,
        _ => true,
    }
}

/// [`isHoistableAndSinkableInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L890).
fn is_hoistable_and_sinkable_inst(opcode: &Opcode) -> bool {
    matches!(
        opcode,
        Opcode::Load { .. }
            | Opcode::Store { .. }
            | Opcode::Call(_)
            | Opcode::Cast(_)
            | Opcode::FNeg
            | Opcode::Binary(_)
            | Opcode::Select
            | Opcode::GetElementPtr { .. }
            | Opcode::ICmp(_)
            | Opcode::FCmp(_)
            | Opcode::ExtractValue(_)
            | Opcode::InsertValue(_)
            | Opcode::Freeze
    )
}

/// [`canSinkOrHoistInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L973).
#[allow(clippy::too_many_arguments)]
pub fn can_sink_or_hoist_inst(
    unit: &passes::Unit,
    outer: &Outer,
    ctx: &LoopCtx,
    mssa: &mut Mssa,
    aa: &AliasQuery,
    flags: &mut Flags,
    safety: &LoopSafety,
    inst: InstId,
    target_executes_once_per_loop: bool,
) -> bool {
    let _ = (safety, target_executes_once_per_loop);
    let function = &*unit.function;
    let instruction = function.instruction(inst);
    // If we don't understand the instruction, bail early.
    if !is_hoistable_and_sinkable_inst(&instruction.opcode) {
        return false;
    }
    match &instruction.opcode {
        // Loads have extra constraints we have to verify before we can hoist
        // them.
        Opcode::Load { .. } => can_hoist_load(unit, ctx, mssa, aa, flags, inst),
        Opcode::Call(_) => {
            // Don't sink calls which can throw.
            if calls::may_throw(outer, unit.context, function, inst) {
                return false;
            }
            // Convergent operations may not move across control flow.
            if calls::has_flag(outer, unit.context, function, inst, "convergent") {
                return false;
            }
            // Handle simple cases by querying alias analysis.
            let (reads, writes) = call_behavior(aa, inst);
            if !reads && !writes {
                return true;
            }
            if !writes {
                // Might have stale MemoryDef for call that was later inferred
                // to be read-only.
                let Some(access) = mssa.get_memory_access(inst) else { return false };
                if mssa.access(access).kind != Kind::Use {
                    return false;
                }
                // If we can prove there are no writes to the memory read by the
                // call, we can hoist or sink.
                return !pointer_invalidated_by_loop(ctx, mssa, aa, access, inst, flags, false);
            }
            if !reads {
                // Can hoist or sink if there are no conflicting read/writes to
                // the memory location written to by the call.
                return no_conflicting_read_writes(ctx, mssa, aa, flags, inst);
            }
            false
        }
        Opcode::Store { volatile, .. } => {
            if *volatile {
                // Don't sink/hoist volatile or ordered atomic store!
                return false;
            }
            // We can only hoist a store that we can prove writes a value which
            // is not read or overwritten within the loop. For those cases, we
            // fall back to load store promotion instead.
            if is_only_memory_access(ctx, mssa, inst) {
                return true;
            }
            no_conflicting_read_writes(ctx, mssa, aa, flags, inst)
        }
        // We've established mechanical ability and aliasing, it's up to the
        // caller to check fault safety.
        _ => true,
    }
}

/// `AA->getMemoryEffects(CI)`: whether the call reads and writes memory.
fn call_behavior(
    aa: &AliasQuery,
    call: InstId,
) -> (bool, bool) {
    aa.call_effects(call)
}

/// [`canHoistLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L932).
/// No `invariant.start` or `invariant.group` in MIR, no atomic loads.
pub fn can_hoist_load(
    unit: &passes::Unit,
    ctx: &LoopCtx,
    mssa: &mut Mssa,
    aa: &AliasQuery,
    flags: &mut Flags,
    load: InstId,
) -> bool {
    if matches!(
        unit.function.instruction(load).opcode,
        Opcode::Load { volatile: true, .. }
    ) {
        return false; // Don't sink/hoist volatile or ordered atomic loads!
    }
    // Loads from constant memory are always safe to move, even if they end up
    // in the same alias set as something that ends up being modified. Loads
    // marked `invariant` are too.
    if aa.is_use_trivially_optimizable_to_live_on_entry(load) {
        return true;
    }
    let Some(access) = mssa.get_memory_access(load) else { return false };
    !pointer_invalidated_by_loop(ctx, mssa, aa, access, load, flags, false)
}

/// [`isOnlyMemoryAccess`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L902):
/// `inst` is the only instruction with a memory access in the loop.
fn is_only_memory_access(
    ctx: &LoopCtx,
    mssa: &Mssa,
    inst: InstId,
) -> bool {
    for &at in ctx.loop_.body.iter() {
        if let Some(accesses) = mssa.get_block_accesses(cfg::block(at)) {
            let mut not_a_phi = 0;
            for &access in accesses {
                let one = mssa.access(access);
                if one.kind == Kind::Phi {
                    continue;
                }
                let first = not_a_phi == 0;
                not_a_phi += 1;
                if one.inst != Some(inst) || !first {
                    return false;
                }
            }
        }
    }
    true
}

/// [`getClobberingMemoryAccess`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L919):
/// the skip-self walk, until the loop has asked `LicmMssaOptCap` of them.
fn get_clobbering_memory_access(
    mssa: &mut Mssa,
    aa: &AliasQuery,
    flags: &mut Flags,
    access: AccessId,
) -> AccessId {
    // See the declaration of `SetLicmMssaOptCap` for usage details.
    if flags.too_many_clobbering_calls() {
        return mssa.get_defining_access(access).expect("a defining access");
    }
    let source = mssa.get_clobbering_memory_access_skip_self(aa, access);
    flags.increment_clobbering_calls();
    source
}

/// [`noConflictingReadWrites`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L2153).
fn no_conflicting_read_writes(
    ctx: &LoopCtx,
    mssa: &mut Mssa,
    aa: &AliasQuery,
    flags: &mut Flags,
    inst: InstId,
) -> bool {
    // If there are more accesses than the Promotion cap, then give up as we're
    // not walking a list that long.
    if flags.too_many_memory_accesses() {
        return false;
    }
    let imd = mssa.get_memory_access(inst).expect("an access of the instruction");
    let source = get_clobbering_memory_access(mssa, aa, flags, imd);
    // Make sure there are no clobbers inside the loop.
    if !mssa.is_live_on_entry_def(source) && ctx.contains_block(mssa.access(source).block) {
        return false;
    }
    // If there are interfering Uses don't move this store.
    for &at in ctx.loop_.body.iter() {
        let Some(accesses) = mssa.get_block_accesses(cfg::block(at)) else { continue };
        for &access in accesses {
            // Accesses are ordered. If we find one that I dominates we can
            // stop.
            if !flags.is_sink() && mssa.dominates(imd, access) {
                break;
            }
            if let Some(other) = mssa.access(access).inst {
                // Skip unrelated accesses.
                if !aa.mod_ref_instructions(other, inst) {
                    continue;
                }
                return false;
            }
        }
    }
    true
}

/// [`pointerInvalidatedByLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L2193).
fn pointer_invalidated_by_loop(
    ctx: &LoopCtx,
    mssa: &mut Mssa,
    aa: &AliasQuery,
    mu: AccessId,
    inst: InstId,
    flags: &mut Flags,
    invariant_group: bool,
) -> bool {
    // For hoisting, use the walker to determine safety.
    if !flags.is_sink() {
        let source = get_clobbering_memory_access(mssa, aa, flags, mu);
        return !mssa.is_live_on_entry_def(source)
            && ctx.contains_block(mssa.access(source).block)
            && !(invariant_group
                && cfg::id(mssa.access(source).block) == ctx.loop_.header
                && mssa.access(source).kind == Kind::Phi);
    }
    // For sinking, we'd need to check all Defs below this use. The
    // getClobbering call will look on the backedge of the loop, but will check
    // aliasing with the instructions on the previous iteration. So: only sink
    // if there are no Defs in the loop, and the existing ones precede the use
    // and are in the same block.
    if flags.too_many_memory_accesses() {
        return true;
    }
    for &at in ctx.loop_.body.iter() {
        if pointer_invalidated_by_block(mssa, cfg::block(at), mu) {
            return true;
        }
    }
    // When sinking, the source block may not be part of the loop so check it.
    if !ctx.contains(inst) {
        let parent = ctx.function.parent(inst).expect("an instruction in a block");
        return pointer_invalidated_by_block(mssa, parent, mu);
    }
    false
}

/// [`pointerInvalidatedByBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L2243).
fn pointer_invalidated_by_block(
    mssa: &Mssa,
    block: BlockId,
    mu: AccessId,
) -> bool {
    if let Some(defs) = mssa.get_block_defs(block) {
        for &def in defs {
            if mssa.access(def).kind == Kind::Def
                && (mssa.access(mu).block != mssa.access(def).block || !mssa.locally_dominates(def, mu))
            {
                return true;
            }
        }
    }
    false
}
