//! `LoopVectorizationLegality`: whether an innermost loop may be vectorized.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `LoopVectorizationLegality.cpp`
//! (`canVectorize`,
//! [L1889](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Vectorize/LoopVectorizationLegality.cpp#L1889)),
//! in the pin's order: the loop's CFG (`canVectorizeLoopNestCFG`, L1626), one
//! block (`canVectorizeWithIfConvert`, L1449), every instruction
//! (`canVectorizeInstrs`, L799: header phis are reductions, inductions or
//! refused; calls and unwidenable types are refused), a countable trip count,
//! then the memory accesses (`canVectorizeMemory`, L1193, from
//! [`LoopAccessInfo`]). The primary induction (`addInductionPhi`, L706) is the
//! widest integer induction that starts at 0 and steps by 1.
//!
//! # Example
//! `loop: %i = phi [0, %ph], [%n, %loop]; %p = gep %a, %i; %v = load %p;
//! store %v, gep %b, %i; %n = add %i, 1; br (%n == 1920), exit, loop` is
//! legal with primary induction `%i`; the same loop with a `call` in it is not.
//!
//! # DEVIATE rows (features the slice does not need; each exit is its piece)
//! - LV-D1 outer loops and the VPlan-native path (`canVectorizeOuterLoop`):
//!   only innermost loops.
//! - LV-D2 if-conversion (`canVectorizeWithIfConvert`,
//!   `blockNeedsPredication`): a loop of more than one block is legal only
//!   where every block runs on every trip (it dominates the latch), so no block
//!   is predicated. Exit: predicated blocks (`blockCanBePredicated`).
//! - LV-D3 early-exit loops (`isVectorizableEarlyExitLoop`): read-only loops
//!   whose loads are dereferenceable in the loop only; no stores (the pin's
//!   `-enable-early-exit-vectorization-with-side-effects` is off), no
//!   fault-only-first loads (`-enable-early-exit-with-ffload` is off), no SCEV
//!   predicates. The planner takes them through `loopvectorizeearlyexit`.
//! - LV-D4 calls: an intrinsic `getVectorIntrinsicIDForCall` takes is legal (a
//!   scalar operand loop invariant, L965); a call the `VFDatabase` or
//!   `isTLIScalarize` would take is refused. The transform widens fewer.
//! - LV-D5 fixed-order recurrences and the coerced-AddRec induction retry
//!   (`isFixedOrderRecurrence`, `PredicatedScalarEvolution`): such a phi is
//!   refused as unidentified.
//! - LV-D6 histograms (`EnableHistogramVectorization` is off in the pin) and
//!   non-temporal accesses (`isLegalNTLoad`/`isLegalNTStore`, metadata).
//! - LV-D7 `LoopVectorizeHints` (`llvm.loop.vectorize.*` metadata): every loop
//!   allows vectorization and reordering.

use llrm_analysis::ivdescriptors::{
    Analyses, InductionDescriptor, InductionKind, InductionStep, RecurrenceDescriptor, is_induction_phi,
    is_reduction_phi,
};
use llrm_analysis::loopaccessinfo::LoopAccessInfo;
use llrm_analysis::scev::backedge::ExitCountKind;
use llrm_analysis::scev::expr::Kind;
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_analysis::vectorutils::{get_vector_intrinsic_id_for_call, is_vector_intrinsic_with_scalar_op_at_arg};
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::loopinfo::Loop;
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::types::{Type, TypeId};

/// What `canVectorize` found, for the planner: `Inductions`, `Reductions`,
/// `PrimaryInduction` and `WidestIndTy`.
#[derive(Clone, Debug)]
pub struct Legality {
    pub inductions: Vec<(InstId, InductionDescriptor)>,
    pub reductions: Vec<(InstId, RecurrenceDescriptor)>,
    /// The canonical induction when it has the widest induction type; the
    /// vectorizer makes one otherwise.
    pub primary_induction: Option<InstId>,
    /// `getWidestInductionType`, as its width in bits (`getInductionIntegerTy`:
    /// a pointer is the pointer-sized integer, an integer narrower than 32
    /// bits is `i32`).
    pub widest_induction_bits: Option<u32>,
    /// `Hints->setPotentiallyUnsafe`: a floating-point operation without
    /// fast-math flags.
    pub potentially_unsafe: bool,
    /// `UncountableExitType`: the loop leaves early where a computed condition
    /// says so (`isVectorizableEarlyExitLoop`). Read-only loops only.
    pub early_exit: Option<EarlyExit>,
}

/// The exits of an early-exit loop (`CountableExitingBlocks` and
/// `UncountableExitingBlocks`).
#[derive(Clone, Debug)]
pub struct EarlyExit {
    pub countable: Vec<BlockId>,
    pub uncountable: Vec<BlockId>,
}

/// Why a loop is not vectorized (`reportVectorizationFailure`'s debug message).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Refusal(pub &'static str);

/// `LoopVectorizationLegality::canVectorize(UseVPlanNativePath=false)`.
pub fn can_vectorize(
    cx: &Cx,
    se: &mut ScalarEvolution,
    lai: &LoopAccessInfo,
    the_loop: Loop,
) -> Result<Legality, Refusal> {
    let function = cx.function;
    // canVectorizeLoopNestCFG -> canVectorizeLoopCFG (L1570), and the sub-loops
    // (none: LV-D1).
    if the_loop.get_loop_preheader(function).is_none() {
        return Err(Refusal("Loop doesn't have a legal pre-header"));
    }
    if the_loop.get_num_back_edges(function) != 1 {
        return Err(Refusal("The loop must have a single backedge"));
    }
    if let Some(latch) = the_loop.get_loop_latch(function)
        && !function.terminator(latch).is_some_and(|term| function.instruction(term).opcode == Opcode::Br)
    {
        return Err(Refusal("The loop latch terminator is not a branch"));
    }
    if !the_loop.is_innermost() {
        return Err(Refusal("Unsupported outer loop"));
    }
    // NumBlocks != 1 && !canVectorizeWithIfConvert() (LV-D2).
    if the_loop.get_num_blocks() != 1 {
        can_vectorize_with_if_convert(cx, the_loop)?;
    }

    let mut found = Legality {
        inductions: Vec::new(),
        reductions: Vec::new(),
        primary_induction: None,
        widest_induction_bits: None,
        potentially_unsafe: false,
        early_exit: None,
    };
    can_vectorize_instrs(cx, se, the_loop, &mut found)?;

    // isa<SCEVCouldNotCompute>(PSE.getBackedgeTakenCount()) (LV-D3).
    let header = the_loop.get_header();
    let btc = se.get_backedge_taken_count(cx, header, ExitCountKind::Exact);
    if se.is_could_not_compute(btc) {
        if the_loop.get_exiting_block(cx.function).is_some() {
            return Err(Refusal("Cannot vectorize uncountable loop"));
        }
        found.early_exit = Some(is_vectorizable_early_exit_loop(cx, se, the_loop, &found)?);
    }

    can_vectorize_memory(cx, se, lai, the_loop, &found)?;
    Ok(found)
}

/// `canVectorizeWithIfConvert` (L1449) for a loop whose blocks all run on
/// every trip (LV-D2): the terminators are branches, and
/// `blockNeedsPredication` (the block does not dominate the latch) is false
/// everywhere.
fn can_vectorize_with_if_convert(
    cx: &Cx,
    the_loop: Loop,
) -> Result<(), Refusal> {
    let function = cx.function;
    let latch = the_loop.get_loop_latch(function).ok_or(Refusal("Can't if-convert the loop"))?;
    for &block in the_loop.get_blocks() {
        let Some(term) = function.terminator(block) else { return Err(Refusal("Can't if-convert the loop")) };
        match function.instruction(term).opcode {
            Opcode::Br => {}
            // A switch that exits the loop is refused in the pin; one that does
            // not is predicated here.
            Opcode::Switch => return Err(Refusal("Loop contains an unsupported switch")),
            _ => return Err(Refusal("Loop contains an unsupported terminator")),
        }
        if !cx.tree.dominates(block, latch) {
            return Err(Refusal("Control flow cannot be substituted for a select"));
        }
    }
    Ok(())
}

/// `isVectorizableEarlyExitLoop` (L1652), for a read-only loop (LV-D3).
fn is_vectorizable_early_exit_loop(
    cx: &Cx,
    se: &mut ScalarEvolution,
    the_loop: Loop,
    found: &Legality,
) -> Result<EarlyExit, Refusal> {
    let function = cx.function;
    let header = the_loop.get_header();
    let latch = the_loop.get_loop_latch(function).ok_or(Refusal("Cannot vectorize early exit loop"))?;
    // Reductions, or fixed-order recurrences (refused as unidentified phis).
    if !found.reductions.is_empty() {
        return Err(Refusal("Cannot vectorize early exit loop with reductions or recurrences"));
    }
    let mut early = EarlyExit { countable: Vec::new(), uncountable: Vec::new() };
    for block in the_loop.get_exiting_blocks(function) {
        let count = se.get_exit_count(cx, header, block, ExitCountKind::Exact);
        if se.is_could_not_compute(count) {
            let successors = function.successors(block).len();
            if successors != 2 {
                return Err(Refusal("Incorrect number of successors from early exiting block"));
            }
            early.uncountable.push(block);
        } else {
            early.countable.push(block);
        }
    }
    if early.uncountable.is_empty() {
        return Err(Refusal("Cannot vectorize uncountable loop"));
    }
    if !early.countable.contains(&latch) {
        return Err(Refusal("Cannot vectorize early exit loop"));
    }
    let q = llrm_mir::valuetracking::Query::bare(cx.context, function, cx.layout);
    for &block in the_loop.get_blocks() {
        for inst in function.iter_block(block) {
            let data = function.instruction(inst);
            match data.opcode {
                // Checked separately (the loads below); a store is a side
                // effect.
                Opcode::Load { .. } | Opcode::Phi | Opcode::Br => {}
                Opcode::Store { .. } => {
                    return Err(Refusal("Cannot vectorize early exit loop with side effects"));
                }
                _ if llrm_mir::valuetracking::guarantee::may_write_to_memory(&q, inst) => {
                    return Err(Refusal("Cannot vectorize early exit loop with complex writes to memory"));
                }
                _ if !is_safe_to_speculate(cx, inst) => {
                    return Err(Refusal("Early exit loop contains operations that cannot be speculatively executed"));
                }
                _ => {}
            }
        }
    }
    // A read-only loop's loads are dereferenceable for every trip: faulting
    // ones need fault-only-first loads, which the pin leaves off.
    for &block in the_loop.get_blocks() {
        for inst in function.iter_block(block) {
            let data = function.instruction(inst);
            let Opcode::Load { align, .. } = data.opcode else { continue };
            let align = align.unwrap_or_else(|| cx.layout.align(&cx.context.types, data.ty));
            if !llrm_analysis::loopderef::is_dereferenceable_and_aligned_in_loop(
                se,
                cx,
                header,
                data.operands[0],
                data.ty,
                align,
            ) {
                return Err(Refusal("Cannot vectorize non-read-only early exit loop"));
            }
        }
    }
    Ok(early)
}

/// `isSafeToSpeculativelyExecute` for the operations a loop of the slice can
/// hold: arithmetic that cannot trap, casts, compares, selects and address
/// computations. DEVIATE LV-D3a (exit: none): a division is safe only by a
/// constant that is not zero (or -1 for a signed one); any call is not.
fn is_safe_to_speculate(
    cx: &Cx,
    inst: InstId,
) -> bool {
    let data = cx.function.instruction(inst);
    match data.opcode {
        Opcode::Binary(BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem) => {
            let q = llrm_mir::valuetracking::Query::bare(cx.context, cx.function, cx.layout);
            data.operands
                .get(1)
                .and_then(|&divisor| q.int_constant(divisor))
                .is_some_and(
                    |(bits, width)| {
                        let all_ones = if width >= 128 { u128::MAX } else { (1u128 << width) - 1 };
                        bits != 0 && bits != all_ones
                    },
                )
        }
        Opcode::Binary(_)
        | Opcode::Cast(_)
        | Opcode::ICmp(_)
        | Opcode::FCmp(_)
        | Opcode::Select
        | Opcode::GetElementPtr { .. }
        | Opcode::FNeg => true,
        _ => false,
    }
}

/// `canVectorizeInstrs` (L799) over the loop's blocks.
fn can_vectorize_instrs(
    cx: &Cx,
    se: &mut ScalarEvolution,
    the_loop: Loop,
    found: &mut Legality,
) -> Result<(), Refusal> {
    for &block in the_loop.get_blocks() {
        for inst in cx.function.iter_block(block).collect::<Vec<_>>() {
            can_vectorize_instr(cx, se, the_loop, inst, found)?;
        }
    }
    if found.primary_induction.is_none() {
        if found.inductions.is_empty() {
            return Err(Refusal("Did not find one integer induction var"));
        }
        if found.widest_induction_bits.is_none() {
            return Err(Refusal("Did not find one integer induction var"));
        }
    }
    // A primary induction narrower than the widest is dropped; the vectorizer
    // makes another.
    if let Some(primary) = found.primary_induction
        && Some(cx.layout.size_bits(&cx.context.types, cx.function.instruction(primary).ty) as u32)
            != found.widest_induction_bits
    {
        found.primary_induction = None;
    }
    Ok(())
}

/// The intrinsic `inst` calls, if legality takes it
/// (`getVectorIntrinsicIDForCall`) and the transform widens it. DEVIATE LV-D4
/// (owner llrm-arm64-vec, exit LV-W4): only `llvm.fmuladd` is widened; legality
/// takes the pin's whole list, and a loop with any other is skipped where the
/// transform meets it.
pub(crate) fn widened_intrinsic(
    cx: &Cx,
    inst: InstId,
) -> Option<Intrinsic> {
    get_vector_intrinsic_id_for_call(cx, inst).filter(|id| matches!(id, Intrinsic::FMulAdd | Intrinsic::MinMax { .. }))
}

/// `canVectorizeInstr` (L840).
fn can_vectorize_instr(
    cx: &Cx,
    se: &mut ScalarEvolution,
    the_loop: Loop,
    inst: InstId,
    found: &mut Legality,
) -> Result<(), Refusal> {
    let function = cx.function;
    let data = function.instruction(inst);
    let types = &cx.context.types;

    if data.opcode == Opcode::Phi {
        if !is_valid_element_type(cx, data.ty) {
            return Err(Refusal("Found a non-int non-pointer PHI"));
        }
        // A phi outside the header can become a select (none: one block).
        if function.parent(inst) != Some(the_loop.get_header()) {
            return Ok(());
        }
        // Only if-converted phis with exactly two incoming values.
        if data.operands.len() != 4 {
            return Err(Refusal("Found an invalid PHI"));
        }
        if let Some(descriptor) = is_reduction_phi(cx, Some(se), inst, the_loop, &Analyses::NONE) {
            found.reductions.push((inst, descriptor));
            return Ok(());
        }
        // `IsDisallowedStridedPointerInduction`: AllowStridedPointerIVs is off.
        if let Some(descriptor) = is_induction_phi(cx, se, inst, the_loop)
            && !(descriptor.kind == InductionKind::Ptr && const_int_step(se, &descriptor).is_none())
        {
            add_induction_phi(cx, se, inst, descriptor, found);
            return Ok(());
        }
        // Fixed-order recurrences and the coerced retry: LV-D5.
        return Err(Refusal("Found an unidentified PHI"));
    }

    // Calls (L930): an intrinsic `getVectorIntrinsicIDForCall` takes, else
    // refused (LV-D4: no `VFDatabase` or TLI-scalarized calls).
    if matches!(data.opcode, Opcode::Call(_) | Opcode::Invoke(_)) {
        let Some(id) = get_vector_intrinsic_id_for_call(cx, inst) else {
            return Err(Refusal("Found a non-intrinsic callsite"));
        };
        // Some intrinsics have scalar arguments and should be the same in
        // every lane (loop invariant) to be vectorized (L965). The callee is
        // the last operand.
        let arguments = &data.operands[..data.operands.len() - 1];
        for (index, &argument) in arguments.iter().enumerate() {
            if is_vector_intrinsic_with_scalar_op_at_arg(id, index) && !matches!(argument, Operand::Constant(_)) {
                let scev = se.get_scev(cx, argument);
                if !se.is_loop_invariant(cx, scev, Some(the_loop.get_header())) {
                    return Err(Refusal("Found unvectorizable intrinsic"));
                }
            }
        }
    }

    // The result type is widenable; a cast from an unwidenable type and
    // extractelement are not.
    let widenable = data.ty == types.void() || is_valid_element_type(cx, data.ty);
    let bad_cast = matches!(data.opcode, Opcode::Cast(_))
        && !matches!(
            data.operands.first(),
            Some(&operand) if is_valid_element_type_of(cx, function, operand)
        );
    if !widenable || bad_cast || matches!(data.opcode, Opcode::ExtractElement) {
        return Err(Refusal("Found unvectorizable type"));
    }

    match data.opcode {
        // The stored type is vectorizable.
        Opcode::Store { .. } => {
            let valid = data.operands.first().is_some_and(|&value| is_valid_element_type_of(cx, function, value));
            if !valid {
                return Err(Refusal("Store instruction cannot be vectorized"));
            }
        }
        Opcode::Load { .. } => {}
        // FP math without fast-math flags: `Hints->setPotentiallyUnsafe`.
        Opcode::Binary(_) | Opcode::FNeg
            if matches!(types.get(data.ty), Type::Float(_)) && !data.flags.contains(Flags::FAST) =>
        {
            found.potentially_unsafe = true;
        }
        _ => {}
    }
    Ok(())
}

/// `addInductionPhi` (L706): record the induction; the widest integer one is
/// the type, and an integer induction that starts at 0 and steps by 1 may be
/// the primary.
fn add_induction_phi(
    cx: &Cx,
    se: &mut ScalarEvolution,
    phi: InstId,
    descriptor: InductionDescriptor,
    found: &mut Legality,
) {
    let phi_ty = cx.function.instruction(phi).ty;
    let types = &cx.context.types;
    // `getInductionIntegerTy`, then `getWiderInductionTy`: the new type wins
    // only if strictly wider.
    let bits = match types.get(phi_ty) {
        Type::Int(bits) => Some((*bits).max(32)),
        Type::Pointer(space) => Some(cx.layout.pointer(*space).bits),
        _ => None,
    };
    if let Some(bits) = bits {
        found.widest_induction_bits = Some(found.widest_induction_bits.map_or(bits, |wide| wide.max(bits)));
    }
    // Int inductions are special: only one IV; a canonical one starts at 0 and
    // steps by 1.
    let canonical = descriptor.kind == InductionKind::Int
        && const_int_step(se, &descriptor).is_some_and(|step| step.is_one())
        && matches!(descriptor.start_value, Operand::Constant(_))
        && start_is_zero(cx, se, descriptor.start_value);
    if canonical && (found.primary_induction.is_none() || bits == found.widest_induction_bits) {
        found.primary_induction = Some(phi);
    }
    found.inductions.push((phi, descriptor));
}

/// `ID.getConstIntStepValue()`.
pub(crate) fn const_int_step(
    se: &ScalarEvolution,
    descriptor: &InductionDescriptor,
) -> Option<llrm_analysis::scev::ap::Ap> {
    match descriptor.step {
        InductionStep::Scev(step) => se.as_constant(step),
        InductionStep::Fp(_) => None,
    }
}

/// `cast<Constant>(Start)->isNullValue()` for an integer start.
fn start_is_zero(
    cx: &Cx,
    se: &mut ScalarEvolution,
    start: Operand,
) -> bool {
    let scev = se.get_scev(cx, start);
    matches!(&se.node(scev).kind, Kind::Constant(value) if value.is_zero())
}

/// `VectorType::isValidElementType`: an integer, a floating-point type or a
/// pointer.
fn is_valid_element_type(
    cx: &Cx,
    ty: TypeId,
) -> bool {
    matches!(
        cx.context.types.get(ty),
        Type::Int(_) | Type::Float(_) | Type::Pointer(_)
    )
}

fn is_valid_element_type_of(
    cx: &Cx,
    function: &llrm_mir::module::Function,
    operand: Operand,
) -> bool {
    function.operand_type(cx.context, operand).is_some_and(|ty| is_valid_element_type(cx, ty))
}

/// `canVectorizeMemory` (L1193). Stores to loop-invariant addresses are refused
/// unless a reduction stores them (the slice has no reduction stores: such a
/// store is refused).
fn can_vectorize_memory(
    cx: &Cx,
    se: &mut ScalarEvolution,
    lai: &LoopAccessInfo,
    the_loop: Loop,
    found: &Legality,
) -> Result<(), Refusal> {
    if !lai.can_vectorize_memory() {
        // canVectorizeIndirectUnsafeDependences: histograms are off in the pin
        // (LV-D6).
        return Err(Refusal("Can't vectorize due to memory conflicts"));
    }
    if lai.has_load_store_dependence_involving_loop_invariant_address() {
        return Err(Refusal("We don't allow storing to uniform addresses"));
    }
    let stores = lai.stores_to_invariant_addresses();
    if stores.is_empty() {
        return Ok(());
    }
    let is_reduction_store =
        |store: InstId| found.reductions.iter().any(|(_, reduction)| reduction.intermediate_store == Some(store));
    for &store in stores {
        if !is_reduction_store(store) {
            continue;
        }
        // The address is defined outside the loop (blockNeedsPredication is
        // false: one block).
        if let Operand::Value(value) = cx.function.instruction(store).operands[1]
            && let llrm_mir::module::ValueDef::Instruction(def) = cx.function.value(value).def
            && the_loop.contains_instruction(cx.function, def)
        {
            return Err(Refusal("Invariant address is calculated inside the loop"));
        }
    }
    if lai.has_store_store_dependence_involving_loop_invariant_address() {
        // Every invariant store must be a reduction's, the later one
        // overwriting the earlier.
        let mut unhandled: Vec<InstId> = Vec::new();
        for &store in stores {
            if is_reduction_store(store) {
                unhandled.retain(|&other| {
                    !(store_to_same_address(cx, se, store, other) && stored_type(cx, store) == stored_type(cx, other))
                });
                continue;
            }
            unhandled.push(store);
        }
        if !unhandled.is_empty() {
            return Err(Refusal("We don't allow storing to uniform addresses"));
        }
    }
    Ok(())
}

/// `storeToSameAddress` (L429).
fn store_to_same_address(
    cx: &Cx,
    se: &mut ScalarEvolution,
    a: InstId,
    b: InstId,
) -> bool {
    if a == b {
        return true;
    }
    let (a_ptr, b_ptr) = (cx.function.instruction(a).operands[1], cx.function.instruction(b).operands[1]);
    if a_ptr == b_ptr {
        return true;
    }
    se.get_scev(cx, a_ptr) == se.get_scev(cx, b_ptr)
}

fn stored_type(
    cx: &Cx,
    store: InstId,
) -> Option<TypeId> {
    let value = cx.function.instruction(store).operands[0];
    cx.function.operand_type(cx.context, value)
}

#[cfg(test)]
#[path = "loopvectorizelegality_tests.rs"]
mod tests;
