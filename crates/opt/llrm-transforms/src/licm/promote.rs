//! `collectPromotionCandidates` and `promoteLoopAccessesToScalars`
//! (`LICM.cpp`): the decisions only, nothing is rewritten.

use std::rc::Rc;

use llrm_analysis::aliassets::AliasSetTracker;
use llrm_analysis::cfg;
use llrm_analysis::mssa::Mssa;
use llrm_analysis::mssa::alias::{AliasQuery, Location};
use llrm_mir::context::ConstantKind;
use llrm_mir::dense::IdSet;
use llrm_mir::module::{BlockId, GlobalKind, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{self, Outer};

use super::{LoopCtx, LoopSafety};

/// What `promoteLoopAccessesToScalars` decided for a set of pointers that must
/// alias.
#[derive(Clone, Debug)]
pub struct Promotion {
    pub pointers: Vec<Operand>,
    /// The loop's loads and stores of the pointers.
    pub uses: Vec<InstId>,
    /// `StoreSafety == StoreSafe`: the stores leave the loop too
    /// (`NumLoadStorePromoted`), else only the loads (`NumLoadPromoted`).
    pub store_safe: bool,
    /// The type every access has.
    pub access_ty: llrm_mir::types::TypeId,
    /// `Alignment`: the most any access that is certain to be made proves.
    pub alignment: u64,
    pub found_load_to_promote: bool,
    pub store_is_guaranteed_to_execute: bool,
    pub load_is_guaranteed_to_execute: bool,
    /// `AATags`: the `!tbaa` node every access has, if they share one.
    pub tbaa: Option<llrm_mir::module::MetadataId>,
}

/// A load or store of the loop and the pointer it goes through.
fn access_pointer(
    ctx: &LoopCtx,
    inst: InstId,
) -> Option<Operand> {
    let instruction = ctx.function.instruction(inst);
    match instruction.opcode {
        Opcode::Load { .. } => Some(instruction.operands[0]),
        Opcode::Store { .. } => Some(instruction.operands[1]),
        _ => None,
    }
}

/// How many loads and stores of the loop go through `pointers`: what keeping
/// them in a register saves, for ranking the promotions a register count
/// allows.
pub fn accesses_of(
    unit: &passes::Unit,
    ctx: &LoopCtx,
    pointers: &[Operand],
) -> usize {
    ctx.loop_
        .body
        .iter()
        .flat_map(|&at| unit.function.iter_block(cfg::block(at)))
        .filter(|&inst| access_pointer(ctx, inst).is_some_and(|pointer| pointers.contains(&pointer)))
        .count()
}

/// Whether keeping `pointers`' accesses in a register frees the registers
/// that hold their addresses: each is a value only those accesses read in the
/// loop. A promotion that frees one takes none net.
pub fn frees_addresses(
    unit: &passes::Unit,
    ctx: &LoopCtx,
    pointers: &[Operand],
) -> bool {
    let values: Vec<_> = pointers
        .iter()
        .filter_map(|pointer| if let Operand::Value(value) = pointer { Some(*value) } else { None })
        .collect();
    !values.is_empty()
        && ctx.loop_.body.iter().flat_map(|&at| unit.function.iter_block(cfg::block(at))).all(|inst| {
            let reads = unit
                .function
                .instruction(inst)
                .operands
                .iter()
                .any(|operand| matches!(operand, Operand::Value(value) if values.contains(value)));
            !reads || access_pointer(ctx, inst).is_some_and(|pointer| pointers.contains(&pointer))
        })
}

/// [`foreachMemoryAccess`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L2011):
/// the instructions of the loop's memory accesses (those still in it).
fn memory_accesses(
    ctx: &LoopCtx,
    mssa: &Mssa,
) -> Vec<InstId> {
    let mut found = Vec::new();
    for &at in ctx.loop_.body.iter() {
        if let Some(accesses) = mssa.get_block_accesses(cfg::block(at)) {
            for &access in accesses {
                if let Some(inst) = mssa.access(access).inst
                    && !ctx.state.gone.borrow().contains(&inst)
                    && ctx.contains(inst)
                {
                    found.push(inst);
                }
            }
        }
    }
    found
}

/// [`isPotentiallyPromotable`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L2022):
/// a load or store through a loop-invariant pointer that is not a constant
/// datum.
fn is_potentially_promotable(
    unit: &passes::Unit,
    ctx: &LoopCtx,
    inst: InstId,
) -> bool {
    let Some(pointer) = access_pointer(ctx, inst) else { return false };
    let constant_data = match pointer {
        Operand::Constant(id) => !matches!(
            unit.context.get(id).kind,
            ConstantKind::Global(_) | ConstantKind::Expr(_)
        ),
        _ => false,
    };
    !constant_data && ctx.is_loop_invariant(pointer)
}

/// The blocks of the loop with an edge out of it.
fn exiting_blocks(ctx: &LoopCtx) -> Vec<BlockId> {
    ctx.loop_
        .body
        .iter()
        .map(|&at| cfg::block(at))
        .filter(|&block| ctx.function.successors(block).into_iter().any(|next| !ctx.contains_block(next)))
        .collect()
}

/// `Loop::getUniqueExitBlocks`.
pub fn unique_exit_blocks(ctx: &LoopCtx) -> Vec<BlockId> {
    let mut found: Vec<BlockId> = Vec::new();
    for &at in ctx.loop_.body.iter() {
        for next in ctx.function.successors(cfg::block(at)) {
            if !ctx.contains_block(next) && !found.contains(&next) {
                found.push(next);
            }
        }
    }
    found
}

/// [`collectStoresWithInvariantAATags`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L2068):
/// the promotable stores with a type tag whose location is stored on the way
/// to every exiting block.
fn stores_with_invariant_aa_tags(
    unit: &passes::Unit,
    ctx: &LoopCtx,
    mssa: &Mssa,
    aa: &AliasQuery,
) -> IdSet<InstId> {
    let mut by_location: Vec<(Rc<Location>, Vec<InstId>)> = Vec::new();
    for inst in memory_accesses(ctx, mssa) {
        if !matches!(unit.function.instruction(inst).opcode, Opcode::Store { .. })
            || !is_potentially_promotable(unit, ctx, inst)
        {
            continue;
        }
        let Some(location) = aa.location_of(inst) else { continue };
        if location.reference.typed.is_none() {
            continue;
        }
        match by_location
            .iter_mut()
            .find(|(one, _)| one.reference == location.reference && one.unbounded == location.unbounded)
        {
            Some((_, stores)) => stores.push(inst),
            None => by_location.push((location, vec![inst])),
        }
    }
    // This only looks at explicit exiting blocks.
    let exiting = exiting_blocks(ctx);
    let mut found = IdSet::new();
    for (_, stores) in &by_location {
        if exiting.iter().all(|&exiting| {
            stores
                .iter()
                .any(
                    |&store| {
                        let block = ctx.function.parent(store).expect("a store in a block");
                        ctx.shape.dominance.dominates(ctx.function, cfg::id(block), cfg::id(exiting))
                    },
                )
        }) {
            for &store in stores {
                found.insert(store);
            }
        }
    }
    found
}

/// A set of locations that must alias and is modified, as
/// `collectPromotionCandidates` returns it.
pub struct Candidate {
    pub pointers: Vec<Operand>,
    pub has_reads_outside_set: bool,
}

/// [`collectPromotionCandidates`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L2110).
pub fn collect_promotion_candidates(
    unit: &passes::Unit,
    ctx: &LoopCtx,
    mssa: &Mssa,
    aa: &AliasQuery,
    safety: &LoopSafety,
) -> Vec<Candidate> {
    let mut tracker = AliasSetTracker::new(aa);
    // Only conditionally executed stores need this, so compute it on demand to
    // keep the common case free.
    let mut invariant_tags: Option<IdSet<InstId>> = None;
    // Populate AST with potentially promotable accesses.
    let mut attempting: IdSet<InstId> = IdSet::new();
    let accesses = memory_accesses(ctx, mssa);
    for &inst in &accesses {
        if !is_potentially_promotable(unit, ctx, inst) {
            continue;
        }
        attempting.insert(inst);
        let Some(location) = aa.location_of(inst) else { continue };
        let is_store = matches!(unit.function.instruction(inst).opcode, Opcode::Store { .. });
        if is_store
            && location.reference.typed.is_some()
            && !safety.is_guaranteed_to_execute(inst)
            && !invariant_tags.get_or_insert_with(|| stores_with_invariant_aa_tags(unit, ctx, mssa, aa)).contains(&inst)
        {
            // Promotion requires inserting a new store at the loop exits; we
            // need to prove that store doesn't alias anything, in addition to
            // proving aliasing for the stores we're removing. The new store is
            // executed unconditionally, so when we're proving aliasing for that
            // store, we can only rely on AA tags that likewise hold
            // unconditionally.
            tracker.add_store_without_aa_tags(&location);
        } else if is_store {
            tracker.add_store(&location);
        } else {
            tracker.add_load(&location);
        }
    }
    // We're only interested in must-alias sets that contain a mod.
    let mut sets: Vec<(usize, bool)> = tracker
        .alias_sets()
        .filter(|(_, set)| set.is_mod() && set.is_must_alias())
        .map(|(at, _)| (at, false))
        .collect();
    if sets.is_empty() {
        return Vec::new(); // Nothing to promote...
    }
    // Discard any sets for which there is an aliasing non-promotable access.
    for &inst in &accesses {
        if attempting.contains(&inst) {
            continue;
        }
        let touches = true;
        sets.retain_mut(|(set, reads_outside)| {
            let found = tracker.aliases_unknown_inst(*set, inst, touches);
            // Cannot promote if there are writes outside the set.
            if found.is_mod() {
                return false;
            }
            if found.is_ref() {
                // Remember reads outside the set.
                *reads_outside = true;
                // If this is a mod-only set and there are reads outside the
                // set, we will not be able to promote, so bail out early.
                return tracker.alias_set(*set).is_ref();
            }
            true
        });
    }
    sets.into_iter()
        .map(|(set, has_reads_outside_set)| Candidate {
            pointers: tracker.alias_set(set).get_pointers(),
            has_reads_outside_set,
        })
        .collect()
}

/// The object `pointer` points into (`getUnderlyingObject`).
fn underlying_object(
    unit: &passes::Unit,
    pointer: Operand,
) -> Operand {
    llrm_mir::valuetracking::underlying(unit.context, unit.layout, unit.function, pointer).0
}

/// Whether `object` is an alloca.
fn is_alloca(
    unit: &passes::Unit,
    object: Operand,
) -> bool {
    is_alloca_in(&*unit.function, object)
}

/// Whether `object` is the value of an `alloca` of `function`.
pub(crate) fn is_alloca_in(
    function: &llrm_mir::module::Function,
    object: Operand,
) -> bool {
    match object {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => matches!(function.instruction(def).opcode, Opcode::Alloca { .. }),
            ValueDef::Argument(_) => false,
        },
        _ => false,
    }
}

/// The allocas of the function whose address is exposed, found on the first
/// ask: only a loop that may throw asks (`isNotVisibleOnUnwindInLoop`), and
/// finding them scans the function. An alloca's exposure does not change as
/// instructions move between blocks or accesses leave a loop.
#[derive(Default)]
pub struct Exposed {
    found: std::cell::OnceCell<IdSet<ValueId>>,
}

impl Exposed {
    fn get(
        &self,
        unit: &passes::Unit,
        outer: &Outer,
    ) -> &IdSet<ValueId> {
        self.found.get_or_init(|| {
            let memory = llrm_analysis::memory::Unit::within(unit.context, unit.layout, unit.function, outer);
            llrm_analysis::memory::exposed_frames(&memory).into_iter().collect()
        })
    }
}

/// Whether the address of the alloca `object` is exposed anywhere in the
/// function. DEVIATE: `PointerMayBeCapturedBefore` asks only before the loop
/// header; this asks of the whole function, which captures more.
fn is_captured(
    unit: &passes::Unit,
    outer: &Outer,
    exposed: &Exposed,
    object: Operand,
) -> bool {
    match object {
        Operand::Value(value) => exposed.get(unit, outer).contains(&value),
        _ => true,
    }
}

/// `isNotVisibleOnUnwindInLoop`: a caller cannot inspect the object if an
/// unwind occurs inside the loop (an alloca whose address is not captured).
fn is_not_visible_on_unwind_in_loop(
    unit: &passes::Unit,
    outer: &Outer,
    exposed: &Exposed,
    object: Operand,
) -> bool {
    is_alloca(unit, object) && !is_captured(unit, outer, exposed, object)
}

/// `isWritableObject`: an alloca or a variable that is not constant.
pub(crate) fn is_writable_object(
    context: &llrm_mir::Context,
    function: &llrm_mir::module::Function,
    outer: &Outer,
    object: Operand,
) -> bool {
    if is_alloca_in(function, object) {
        return true;
    }
    match object {
        Operand::Constant(id) => match context.get(id).kind {
            ConstantKind::Global(global) => outer.globals.get(global.0 as usize).is_some_and(|one| {
                matches!(
                    &one.kind,
                    GlobalKind::Variable(variable) if !variable.constant
                )
            }),
            _ => false,
        },
        _ => false,
    }
}

/// `isThreadLocalObject`: LLVM's `isIdentifiedFunctionLocal` and not captured
/// before the loop, or a single-threaded module. DEVIATE: llrm's targets run
/// one thread, so every object is thread-local (`ThreadModel::Single`).
fn is_thread_local_object() -> bool {
    true
}

/// [`promoteLoopAccessesToScalars`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L1692):
/// whether the accesses of the must-aliasing `pointers` leave the loop, and
/// whether the stores do. No atomics in MIR, so none of the ordering checks;
/// the alias tags merged for the new store are not decided here.
#[allow(clippy::too_many_arguments)]
pub fn promote_loop_accesses_to_scalars(
    unit: &passes::Unit,
    outer: &Outer,
    ctx: &LoopCtx,
    safety: &LoopSafety,
    exposed: &Exposed,
    exit_blocks: &[BlockId],
    pointers: &[Operand],
    has_reads_outside_set: bool,
) -> Option<Promotion> {
    let function = &*unit.function;
    let some_ptr = *pointers.first()?;
    let _ = some_ptr;
    // Whether the preheader may load the location, and whether the stores may
    // be moved after the loop.
    let mut dereferenceable_in_ph = false;
    let mut store_is_guaranteed_to_execute = false;
    let mut found_load_to_promote = false;
    #[derive(PartialEq)]
    enum StoreSafety {
        Safe,
        Unsafe,
        Unknown,
    }
    let mut store_safety = StoreSafety::Unknown;
    let mut loop_uses: Vec<InstId> = Vec::new();
    let mut alignment: u64 = 1;
    let mut load_is_guaranteed_to_execute = false;
    // `AATags`: unset until the first access, then the node all share.
    let mut tbaa: Option<Option<llrm_mir::module::MetadataId>> = None;
    // If there are reads outside the promoted set, then promoting stores is
    // definitely not safe.
    if has_reads_outside_set {
        store_safety = StoreSafety::Unsafe;
    }
    let first_pointer = pointers[0];
    // Experiment: leave the allocas to `promote` (LLVM runs mem2reg and SROA
    // first).
    if llrm_support::env_flag!("LLRM_EXP_LICM_NO_ALLOCA") && is_alloca(unit, underlying_object(unit, first_pointer)) {
        return None;
    }
    if store_safety == StoreSafety::Unknown && safety.any_block_may_throw() {
        // If a loop can throw, we have to insert a store along each unwind
        // edge. That said, we can't actually make the unwind edge explicit.
        // Therefore, we have to prove that the store is dead along the unwind
        // edge.
        let object = underlying_object(unit, first_pointer);
        if !is_not_visible_on_unwind_in_loop(unit, outer, exposed, object) {
            store_safety = StoreSafety::Unsafe;
        }
    }
    // Check that all accesses to pointers in the alias set use the same type.
    let mut access_ty = None;
    for &pointer in pointers {
        for &at in ctx.loop_.body.iter() {
            for inst in function.iter_block(cfg::block(at)) {
                if ctx.state.gone.borrow().contains(&inst) || !ctx.contains(inst) {
                    continue;
                }
                let instruction = function.instruction(inst);
                match instruction.opcode {
                    Opcode::Load { volatile, align } => {
                        if instruction.operands[0] != pointer {
                            continue;
                        }
                        if volatile {
                            return None;
                        }
                        found_load_to_promote = true;
                        let inst_alignment = align.unwrap_or(1);
                        if !load_is_guaranteed_to_execute {
                            load_is_guaranteed_to_execute = safety.is_guaranteed_to_execute(inst);
                        }
                        // Note that proving a load safe to speculate requires
                        // proving sufficient alignment at the target location.
                        // Proving it guaranteed to execute does as well. Thus
                        // we can increase our
                        // guaranteed alignment as well.
                        if (!dereferenceable_in_ph || inst_alignment > alignment)
                            && is_safe_to_execute_unconditionally(unit, outer, safety, inst)
                        {
                            dereferenceable_in_ph = true;
                            alignment = alignment.max(inst_alignment);
                        }
                    }
                    Opcode::Store { volatile, align } => {
                        // Stores *of* the pointer are not interesting, only
                        // stores *to* the pointer.
                        if instruction.operands[1] != pointer {
                            continue;
                        }
                        if volatile {
                            return None;
                        }
                        // If the store is guaranteed to execute, both
                        // properties are satisfied.
                        let guaranteed = safety.is_guaranteed_to_execute(inst);
                        store_is_guaranteed_to_execute |= guaranteed;
                        if guaranteed {
                            alignment = alignment.max(align.unwrap_or(1));
                            dereferenceable_in_ph = true;
                            if store_safety == StoreSafety::Unknown {
                                store_safety = StoreSafety::Safe;
                            }
                        }
                        // If a store dominates all exit blocks, it is safe to
                        // sink.
                        if store_safety == StoreSafety::Unknown
                            && exit_blocks.iter().all(|&exit| {
                                let block = function.parent(inst).expect("a store in a block");
                                ctx.shape.dominance.dominates(ctx.function, cfg::id(block), cfg::id(exit))
                            })
                        {
                            store_safety = StoreSafety::Safe;
                        }
                        // If the store is not guaranteed to execute, we may
                        // still get deref info through it.
                        if !dereferenceable_in_ph {
                            let bytes =
                                unit.layout.store_size(&unit.context.types, value_type(unit, instruction.operands[0]));
                            dereferenceable_in_ph = llrm_mir::valuetracking::dereferenceable(
                                unit.context,
                                unit.layout,
                                outer.sizes(),
                                function,
                                pointer,
                                bytes,
                            ) || !outer.target().load_may_trap(
                                bytes,
                                llrm_mir::valuetracking::alignment(
                                    unit.context,
                                    unit.layout,
                                    &outer.globals,
                                    function,
                                    pointer,
                                ),
                            );
                        }
                    }
                    _ => continue, // Not a load or store.
                }
                let ty = match instruction.opcode {
                    Opcode::Load { .. } => instruction.ty,
                    _ => value_type(unit, instruction.operands[0]),
                };
                match access_ty {
                    None => access_ty = Some(ty),
                    Some(seen) if seen != ty => return None,
                    Some(_) => {}
                }
                // Merge the AA tags.
                let node = instruction.metadata.iter().find(|(kind, _)| kind == "tbaa").map(|(_, node)| *node);
                tbaa = Some(match tbaa {
                    None => node,
                    Some(seen) if seen == node => seen,
                    Some(_) => None,
                });
                loop_uses.push(inst);
            }
        }
    }
    // If we couldn't prove we can hoist the load, bail.
    if !dereferenceable_in_ph {
        return None;
    }
    // We know we can hoist the load, but don't have a guaranteed store. Check
    // whether the location is writable and thread-local. If it is, then we can
    // insert stores along paths which originally didn't have them without
    // violating the memory model.
    if store_safety == StoreSafety::Unknown {
        let object = underlying_object(unit, first_pointer);
        if is_writable_object(&*unit.context, &*unit.function, outer, object) && is_thread_local_object() {
            store_safety = StoreSafety::Safe;
        }
    }
    // If we've still failed to prove we can sink the store, hoist the load
    // only, if possible.
    if store_safety != StoreSafety::Safe && !found_load_to_promote {
        // If we cannot hoist the load either, give up.
        return None;
    }
    // A location the loop only stores to, on some paths, is promoted only
    // where the pin's `StoreSafety` allows the store after the loop
    // ([L1921](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L1921)):
    // the pin (`ThreadModel::POSIX` for this triple,
    // [L1672](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L1672))
    // calls only a function-local object thread-local, so a global is left:
    // promoting it would load it in the preheader and store it after the
    // loop for no load saved, and the load keeps `writeonly` from removing
    // the global (nbody.bas, +43 B at -Os m16, once its division guard
    // stopped taking a slot of `Pressure`).
    if !found_load_to_promote
        && !store_is_guaranteed_to_execute
        && !is_alloca(unit, underlying_object(unit, first_pointer))
    {
        return None;
    }
    Some(Promotion {
        pointers: pointers.to_vec(),
        uses: loop_uses,
        store_safe: store_safety == StoreSafety::Safe,
        access_ty: access_ty?,
        alignment,
        found_load_to_promote,
        store_is_guaranteed_to_execute,
        load_is_guaranteed_to_execute,
        tbaa: tbaa.flatten(),
    })
}

/// The type of an operand.
fn value_type(
    unit: &passes::Unit,
    operand: Operand,
) -> llrm_mir::types::TypeId {
    match operand {
        Operand::Value(value) => unit.function.value(value).ty,
        Operand::Constant(id) => unit.context.get(id).ty,
        Operand::Block(_) => unreachable!("a block is not a stored value"),
    }
}

/// `isSafeToExecuteUnconditionally` of a load (`AllowSpeculation` the run's).
fn is_safe_to_execute_unconditionally(
    unit: &passes::Unit,
    outer: &Outer,
    safety: &LoopSafety,
    load: InstId,
) -> bool {
    (safety.allow_speculation() && super::hoist::is_safe_to_speculatively_execute(unit, outer, load))
        || safety.is_guaranteed_to_execute(load)
}

/// What promoting `found` does to the loop as the later decisions see it: the
/// loads are gone, and each stands for a value from the preheader when nothing
/// in the set stores (then it is loop-invariant, and the pointers loaded
/// through it can be promoted next); the stores are gone when they too leave.
pub fn take_out_of_loop(
    unit: &passes::Unit,
    ctx: &LoopCtx,
    preheader: BlockId,
    found: &Promotion,
) {
    let stores = found.uses.iter().any(|&inst| matches!(unit.function.instruction(inst).opcode, Opcode::Store { .. }));
    for &inst in &found.uses {
        match unit.function.instruction(inst).opcode {
            Opcode::Load { .. } => {
                ctx.state.gone.borrow_mut().insert(inst);
                if !stores {
                    ctx.state.moved.borrow_mut().insert(inst, preheader);
                }
            }
            _ => {
                if found.store_safe {
                    ctx.state.gone.borrow_mut().insert(inst);
                }
            }
        }
    }
}
