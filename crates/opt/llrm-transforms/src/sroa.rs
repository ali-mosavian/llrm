//! `sroa-real`: scalar replacement of aggregates as LLVM's SROA does it: the
//! uses of an alloca are cut into slices, the slices into partitions that no
//! access straddles, each partition becomes an alloca of its own (or the old
//! one), the accesses are rewritten onto it, and what is then only loaded and
//! stored whole is promoted to registers. A translation of `SROA.cpp` at
//! d1106deb71cc (loads, stores, aggregate loads and stores, memset, memcpy and
//! lifetime intrinsics, phis and selects of pointers, pre-splitting).
//!
//! DEVIATE, each with its reason (line numbers are SROA.cpp's):
//! - The mode is `sroa<preserve-cfg>` (`PreserveCFG`, L174): a select whose
//!   load is not speculatable, a store through a select, and a store through a
//!   phi on a critical edge are not rewritten (they need
//!   `SplitBlockAndInsertIfThenElse` or `SplitBlockPredecessors`), and the
//!   alloca is left. LLVM's early and late `sroa` of -O2 run with CFG
//!   modification allowed.
//! - Vectors (`isVectorPromotionViable` L2374, `rewriteVectorized*`
//!   L3451-L3637, `rewriteTreeStructuredMerge` L3002,
//!   `tryCanonicalizeStructToVector` L5460): the targets have no vector
//!   registers, and `AggregateToVector` is off at -O2.
//! - Calls and invokes escape a pointer whatever their attributes
//!   (`visitCallBase` L1402 looks at read-only, nocapture operands).
//! - Address-space casts abort; pointers in a space that is a selector and an
//!   offset are not converted to integers (`canConvertValue` L2057).
//! - Debug records (`migrateDebugInfo` L348, the migration in `splitAlloca`
//!   L6066): an alloca a `Declare` names is processed like any other, so `-g`
//!   does not change the code. One that SROA splits loses its `Declare` with
//!   the alloca (the variable is reported dropped), where LLVM migrates it to
//!   the new allocas as fragments. DEVIATE SR-G1: 2 of 448 bench C variables
//!   lose their location at -O1 (190 -> 188 of 437 at -O2, 205 -> 203 of 461 at
//!   -Os; scope bytes covered unchanged, `tools/dwarf-coverage.sh`). Exit: a
//!   `Declare` that carries a fragment; owner mir-undef; review 2026-11-14.
//! - `unfoldGEPSelect`, `unfoldGEPPhi` (L4578, L4673): gep over select or phi.
//! - `isSafeToLoadUnconditionally` (L1655, L1840) is `dereferenceable` alone:
//!   an alloca or a sized argument, no alignment, no global, no "a load of it
//!   already dominates" proof. `mayWriteToMemory` (L1613) counts any call as a
//!   writer.
//! - `fixLoadStoreAlign` (L4180): MIR cannot rewrite a load's alignment in
//!   place, so the loads and stores through a rewritten phi or select keep
//!   theirs.
//! - A memset or memcpy keeps no alignment or alias metadata of its own
//!   (`setDestAlignment`, `setAAMetadata`); a memmove split to a memcpy keeps
//!   its callee (`CreateMemCpy`, L4020); a lifetime marker names its size, as
//!   MIR's does.
//! - `AssumptionCache`, `DomTreeUpdater`, statistics, names that follow
//!   `.sroa.` suffix rules.
//! - Offsets are `i128` kept in `0..2^index width` after each GEP, as the
//!   `APInt` of `PtrUseVisitor` wraps; one past the alloca is outside it, and
//!   so is a negative one, which wraps to a large number.
//! - `IRBuilder` here makes what it is told and does not fold constants
//!   (`ConstantFolder`) except the splat of a constant byte; `fold` does that
//!   after.
//! - Metadata: an access keeps its metadata where the type and offset are
//!   unchanged, and loses it otherwise (`adjustForAccess`).
//!
//! # Example
//! An alloca of two `i32` fields, each stored and loaded whole, becomes two
//! allocas and then registers:
//!
//! ```text
//! %s = alloca { i32, i32 }
//! store i32 1, ptr %s          ; field 0
//! %v = load i32, ptr %s        ; becomes the constant 1, the alloca is gone
//! ```

use std::cmp::Ordering;

use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Function, InstId, Operand, Use, ValueId};
use llrm_mir::opcode::{CastOp, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};
use llrm_mir::types::{Type, TypeId};
use llrm_mir::{Constant, ConstantKind, Context};
use llrm_support::hash::{HashMap, HashSet};

/// The pass: LLVM's `SROAPass` (`SROA.cpp` L6490).
pub struct Sroa;

impl FunctionPass for Sroa {
    fn name(&self) -> &'static str {
        "sroa-real"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("sroa")
    }

    fn reads(&self) -> llrm_mir::footprint::Footprint {
        llrm_mir::footprint::Footprint::MEMORY | llrm_mir::footprint::Footprint::STRUCTURE
    }

    fn adds_memory_operations(&self) -> bool {
        true
    }

    /// SROAPass::run. SROA.cpp#L6490
    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let changed = State::new(unit, analyses).run_sroa();
        if changed { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

// ---- types, as the datalayout sees them ----------------------------------

fn int_bits(
    context: &Context,
    ty: TypeId,
) -> Option<u32> {
    context.types.int_bits(ty)
}

fn is_pointer(
    context: &Context,
    ty: TypeId,
) -> bool {
    matches!(context.types.get(ty), Type::Pointer(_))
}

fn pointer_space(
    context: &Context,
    ty: TypeId,
) -> u32 {
    match context.types.get(ty) {
        Type::Pointer(space) => *space,
        _ => 0,
    }
}

fn is_single_value(
    context: &Context,
    ty: TypeId,
) -> bool {
    matches!(
        context.types.get(ty),
        Type::Int(_) | Type::Float(_) | Type::Pointer(_) | Type::Vector { .. }
    )
}

fn is_aggregate(
    context: &Context,
    ty: TypeId,
) -> bool {
    matches!(
        context.types.get(ty),
        Type::Struct { .. } | Type::Named(_) | Type::Array { .. }
    )
}

/// `DL.typeSizeEqualsStoreSize`.
fn size_equals_store_size(
    layout: &DataLayout,
    context: &Context,
    ty: TypeId,
) -> bool {
    layout.size_bits(&context.types, ty) == layout.store_size(&context.types, ty) * 8
}

/// DEVIATE: whether isel selects an integer of `bytes` bytes (LLVM legalizes
/// any width, so SROA makes i24 where a 3-byte slice is cut).
fn supported_int_size(
    layout: &DataLayout,
    bytes: u64,
) -> bool {
    matches!(bytes, 1 | 2 | 4 | 8) || layout.legal_integer((bytes * 8) as u32)
}

/// `commonAlignment(A, Offset)`: the alignment `offset` bytes into something
/// aligned by `align`.
/// `commonAlignment` (Alignment.h).
fn common_alignment(
    align: u64,
    offset: u64,
) -> u64 {
    if offset == 0 { align } else { align.min(offset & offset.wrapping_neg()) }
}

/// `canConvertValue`. SROA.cpp#L2057
fn can_convert_value(
    layout: &DataLayout,
    context: &Context,
    old: TypeId,
    new: TypeId,
) -> bool {
    if old == new {
        return true;
    }
    // Integers of different widths are not converted.
    if int_bits(context, old).is_some() && int_bits(context, new).is_some() {
        return false;
    }
    // DEVIATE: a float is not reinterpreted as an integer or a pointer, nor an
    // integer as a float: isel selects no such bitcast (x87 has no register
    // move to a general register).
    let is_float = |ty: TypeId| matches!(context.types.get(ty), Type::Float(_));
    if is_float(old) || is_float(new) {
        return false;
    }
    if layout.size_bits(&context.types, new) != layout.size_bits(&context.types, old) {
        return false;
    }
    if !is_single_value(context, new) || !is_single_value(context, old) {
        return false;
    }
    let (old_pointer, new_pointer) = (is_pointer(context, old), is_pointer(context, new));
    if old_pointer || new_pointer {
        let (old_space, new_space) = (pointer_space(context, old), pointer_space(context, new));
        // DEVIATE: a pointer that is a selector and an offset is not an
        // integer.
        let pair = |space| layout.is_pair(space);
        if old_pointer && new_pointer {
            return old_space == new_space
                || (!pair(old_space)
                    && !pair(new_space)
                    && layout.pointer(old_space).bits == layout.pointer(new_space).bits);
        }
        if int_bits(context, old).is_some() {
            return !pair(new_space);
        }
        if !pair(old_space) {
            return int_bits(context, new).is_some();
        }
        return false;
    }
    true
}

// ---- slices --------------------------------------------------------------

/// SROA.cpp#L529
#[derive(Clone, Copy, Debug)]
struct Slice {
    begin: u64,
    end: u64,
    /// The use the slice is of, none once the slice is killed.
    used: Option<Use>,
    splittable: bool,
    kind: Kind,
}

/// What a slice's user is, which SROA.cpp reads off the instruction's class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// A load or a store.
    Memory,
    /// `llvm.memset`.
    MemSet,
    /// `llvm.memcpy` or `llvm.memmove`.
    Transfer,
    /// `llvm.lifetime.start` or `.end`.
    Lifetime,
    /// A phi or select of pointers.
    PhiOrSelect,
}

impl Slice {
    /// `Slice::operator<` (L564): begin ascending, unsplittable before
    /// splittable, end descending.
    /// Slice::operator<. SROA.cpp#L564
    fn order(
        &self,
        other: &Slice,
    ) -> Ordering {
        self.begin
            .cmp(&other.begin)
            .then_with(|| self.splittable.cmp(&other.splittable))
            .then_with(|| other.end.cmp(&self.end))
    }
}

/// `AllocaSlices` (L598): what the uses of an alloca cut it into.
#[derive(Default)]
struct AllocaSlices {
    /// A pointer to the alloca leaves the analysis.
    escaped: bool,
    escaped_read_only: bool,
    slices: Vec<Slice>,
    dead_users: Vec<InstId>,
    dead_operands: Vec<Use>,
    /// Uses of the alloca that go once it is promoted: an `llvm.assume`'s.
    dead_use_if_promotable: Vec<Use>,
}

/// The constant value of an index operand, signed.
/// `GEPOperator::accumulateConstantOffset`, as `adjustOffsetForGEP`
/// (PtrUseVisitor.cpp) uses it.
fn constant_index(
    context: &Context,
    operand: Operand,
) -> Option<i128> {
    let Operand::Constant(id) = operand else { return None };
    let constant = context.get(id);
    match constant.kind {
        ConstantKind::Int(bits) => Some(llrm_mir::context::signed(bits, int_bits(context, constant.ty)?)),
        ConstantKind::Zero => Some(0),
        _ => None,
    }
}

/// `ConstantInt::getLimitedValue`: the unsigned value of a constant integer,
/// saturated at `u64::MAX`.
fn constant_unsigned(
    context: &Context,
    operand: Operand,
) -> Option<u64> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(bits) => Some(u64::try_from(bits).unwrap_or(u64::MAX)),
        ConstantKind::Zero => Some(0),
        _ => None,
    }
}

/// `foldPHINodeOrSelectInst`: the value a phi or select with one possible
/// result is. SROA.cpp#L1010
fn fold_phi_or_select(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> Option<Operand> {
    let instruction = function.instruction(inst);
    match instruction.opcode {
        // `PHINode::hasConstantValue`: the one value besides the phi itself.
        Opcode::Phi => {
            let own = instruction.result.map(Operand::Value);
            let mut common: Option<Operand> = None;
            for pair in instruction.operands.chunks(2) {
                if Some(pair[0]) == own {
                    continue;
                }
                match common {
                    None => common = Some(pair[0]),
                    Some(seen) if seen == pair[0] => {}
                    Some(_) => return None,
                }
            }
            common
        }
        // `foldSelectInst` (L997).
        Opcode::Select => {
            if let Some(condition) = constant_unsigned(context, instruction.operands[0]) {
                return Some(instruction.operands[if condition == 0 { 2 } else { 1 }]);
            }
            (instruction.operands[1] == instruction.operands[2]).then_some(instruction.operands[1])
        }
        _ => None,
    }
}

/// `hasUnsafePHIOrSelectUse`: the size of the largest load or store through
/// the phi or select `root`, or Err where a use is unsafe. SROA.cpp#L1288
fn has_unsafe_phi_or_select_use(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    root: InstId,
) -> Result<u64, ()> {
    let mut visited: llrm_mir::dense::IdSet<InstId> = Default::default();
    visited.insert(root);
    let mut uses: Vec<(Option<ValueId>, InstId)> = vec![(None, root)];
    // With no loads or stores the access is dead: a size of zero.
    let mut size = 0_u64;
    while let Some((used, inst)) = uses.pop() {
        let instruction = function.instruction(inst);
        match &instruction.opcode {
            Opcode::Load { .. } => {
                size = size.max(layout.store_size(&context.types, instruction.ty));
                continue;
            }
            Opcode::Store { .. } => {
                let stored = instruction.operands[0];
                if used.map(Operand::Value) == Some(stored) {
                    return Err(());
                }
                let ty = function.operand_type(context, stored).expect("a typed value");
                size = size.max(layout.store_size(&context.types, ty));
                continue;
            }
            Opcode::GetElementPtr { .. } => {
                if !instruction.operands[1..].iter().all(|&one| constant_index(context, one) == Some(0)) {
                    return Err(());
                }
            }
            Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) | Opcode::Phi | Opcode::Select => {}
            _ => return Err(()),
        }
        if let Some(result) = instruction.result {
            for &user in function.users(result) {
                if visited.insert(user.user) {
                    uses.push((Some(result), user.user));
                }
            }
        }
    }
    Ok(size)
}

/// `findCommonTypeThroughPHIOrSelect`: the type every user of a phi or select
/// loads or stores. SROA.cpp#L1490
fn find_common_type_through_phi_or_select(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> Option<TypeId> {
    let result = function.instruction(inst).result?;
    let mut ty: Option<TypeId> = None;
    for user in function.users(result) {
        let instruction = function.instruction(user.user);
        let user_ty = match instruction.opcode {
            Opcode::Load { .. } => Some(instruction.ty),
            Opcode::Store { .. } => function.operand_type(context, instruction.operands[0]),
            _ => None,
        };
        match user_ty {
            Some(one) if ty.is_none_or(|current| current == one) => ty = Some(one),
            _ => return None,
        }
    }
    ty
}

/// `AllocaSlices::AllocaSlices` with `SliceBuilder` and `PtrUseVisitor`
/// (SROA.cpp#L1416, L1022; PtrUseVisitor.h `visitPtr`).
fn build_slices(
    context: &Context,
    layout: &DataLayout,
    globals: &[llrm_mir::module::GlobalValue],
    function: &Function,
    alloca: InstId,
) -> AllocaSlices {
    let mut found = AllocaSlices::default();
    // `MemTransferSliceMap` (L1034): the slice a transfer made from its first
    // side.
    let mut transfer_slice: llrm_mir::dense::IdMap<InstId, usize> = Default::default();
    // `PHIOrSelectSizes` (L1035): the largest access through a phi or select.
    let mut phi_or_select_sizes: llrm_mir::dense::IdMap<InstId, u64> = Default::default();
    let intrinsic = |inst: InstId| {
        let callee = llrm_mir::memory::callee(context, function, inst)?;
        globals.get(callee.0 as usize)?.intrinsic()
    };
    let root = function.instruction(alloca).result.expect("an alloca has a result");
    let Opcode::Alloca { allocated, .. } = function.instruction(alloca).opcode else { unreachable!("an alloca") };
    let alloc_size = layout.alloc_size(&context.types, allocated);
    let mut worklist: Vec<(Use, bool, i128)> = Vec::new();
    let mut visited_uses: HashSet<Use> = HashSet::default();
    let mut visited_dead: llrm_mir::dense::IdSet<InstId> = Default::default();
    let mut aborted = false;

    // `enqueueUsers` (PtrUseVisitor.cpp).
    let enqueue = |worklist: &mut Vec<(Use, bool, i128)>,
                   visited_uses: &mut HashSet<Use>,
                   value: ValueId,
                   known: bool,
                   offset: i128| {
        for &used in function.users(value) {
            if visited_uses.insert(used) {
                worklist.push((used, known, offset));
            }
        }
    };
    enqueue(&mut worklist, &mut visited_uses, root, true, 0);

    while let Some((used, known, offset)) = worklist.pop() {
        let user = used.user;
        let instruction = function.instruction(user);
        let mut mark_dead = |found: &mut AllocaSlices| {
            if visited_dead.insert(user) {
                found.dead_users.push(user);
            }
        };
        // `insertUse` (L1048).
        let insert_use = |found: &mut AllocaSlices,
                          visited_dead: &mut llrm_mir::dense::IdSet<InstId>,
                          offset: i128,
                          size: u64,
                          splittable: bool,
                          kind: Kind| {
            if size == 0 || offset < 0 || offset as u128 >= u128::from(alloc_size) {
                if visited_dead.insert(user) {
                    found.dead_users.push(user);
                }
                return;
            }
            let begin = offset as u64;
            let mut end = begin.saturating_add(size);
            if size > alloc_size - begin {
                end = alloc_size;
            }
            found.slices.push(Slice { begin, end, used: Some(used), splittable, kind });
        };
        let result_unused = |instruction: &llrm_mir::module::Instruction| {
            instruction.result.is_none_or(|result| function.users(result).is_empty())
        };
        match &instruction.opcode {
            // `visitBitCastInst` (L1084) and `visitAddrSpaceCastInst` (L1091):
            // the pointer's users are the alloca's.
            Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast)
                if used.index == 0 && is_pointer(context, instruction.ty) =>
            {
                if result_unused(instruction) {
                    mark_dead(&mut found);
                } else if let Some(result) = instruction.result {
                    enqueue(&mut worklist, &mut visited_uses, result, known, offset);
                }
            }
            // `visitPtrToIntInst`: the pointer escapes.
            Opcode::Cast(CastOp::PtrToInt) => {
                found.escaped = true;
                aborted = true;
            }
            // `visitGetElementPtrInst` (L1098) and `adjustOffsetForGEP`.
            Opcode::GetElementPtr { source } if used.index == 0 => {
                if result_unused(instruction) {
                    mark_dead(&mut found);
                    continue;
                }
                let indices: Vec<Option<i128>> =
                    instruction.operands[1..].iter().map(|&one| constant_index(context, one)).collect();
                let (known, offset) = if known && indices.iter().all(Option::is_some) {
                    let (constant, variable) = layout.collect_offset(&context.types, *source, &indices);
                    if variable.is_empty() {
                        // `Offset += GEPOffset` on an `APInt` of the index
                        // width: it wraps, and every range check is unsigned.
                        let bits = layout.pointer(pointer_space(context, instruction.ty)).index_bits;
                        (true, (offset + constant).rem_euclid(1i128 << bits))
                    } else {
                        (false, 0)
                    }
                } else {
                    (false, 0)
                };
                if let Some(result) = instruction.result {
                    enqueue(&mut worklist, &mut visited_uses, result, known, offset);
                }
            }
            // `visitLoadInst` (L1116).
            Opcode::Load { volatile, .. } => {
                if !known {
                    found.escaped_read_only = true;
                    continue;
                }
                let size = layout.store_size(&context.types, instruction.ty);
                // `handleLoadOrStore` (L1105).
                let splittable = int_bits(context, instruction.ty).is_some()
                    && !volatile
                    && size_equals_store_size(layout, context, instruction.ty);
                insert_use(&mut found, &mut visited_dead, offset, size, splittable, Kind::Memory);
            }
            // `visitStoreInst` (L1138).
            Opcode::Store { volatile, .. } => {
                if used.index == 0 {
                    found.escaped = true;
                    aborted = true;
                    continue;
                }
                if !known {
                    aborted = true;
                    continue;
                }
                let value_ty = function.operand_type(context, instruction.operands[0]).expect("a typed value");
                let size = layout.store_size(&context.types, value_ty);
                if size > alloc_size || offset < 0 || offset as u128 > u128::from(alloc_size - size) {
                    mark_dead(&mut found);
                    continue;
                }
                let splittable = int_bits(context, value_ty).is_some()
                    && !volatile
                    && size_equals_store_size(layout, context, value_ty);
                insert_use(&mut found, &mut visited_dead, offset, size, splittable, Kind::Memory);
            }
            // `visitPHINodeOrSelectInst` (L1338). A select's condition is no
            // pointer.
            Opcode::Phi | Opcode::Select if !(matches!(instruction.opcode, Opcode::Select) && used.index == 0) => {
                if result_unused(instruction) {
                    mark_dead(&mut found);
                    continue;
                }
                // The result of a constant fold: the pointer itself is as if
                // the phi were replaced by it, any other operand is dead.
                if let Some(result) = fold_phi_or_select(context, function, user) {
                    if result == instruction.operands[used.index as usize] {
                        if let Some(value) = instruction.result {
                            enqueue(&mut worklist, &mut visited_uses, value, known, offset);
                        }
                    } else {
                        found.dead_operands.push(used);
                    }
                    continue;
                }
                if !known {
                    aborted = true;
                    continue;
                }
                // The size is worked out once for each phi or select.
                let size = match phi_or_select_sizes.get(&user).copied().filter(|&size| size != 0) {
                    Some(size) => size,
                    None => match has_unsafe_phi_or_select_use(context, layout, function, user) {
                        Ok(size) => {
                            phi_or_select_sizes.insert(user, size);
                            size
                        }
                        Err(()) => {
                            aborted = true;
                            continue;
                        }
                    },
                };
                // An operand outside the alloca is dead; the other side may
                // matter.
                if offset < 0 || offset as u128 >= u128::from(alloc_size) {
                    found.dead_operands.push(used);
                    continue;
                }
                insert_use(&mut found, &mut visited_dead, offset, size, false, Kind::PhiOrSelect);
            }
            Opcode::Call(_) => match intrinsic(user) {
                // `visitMemSetInst` (L1177).
                Some(Intrinsic::MemSet) if used.index == 0 => {
                    let length = constant_unsigned(context, instruction.operands[2]);
                    if length == Some(0) || (known && (offset < 0 || offset as u128 >= u128::from(alloc_size))) {
                        mark_dead(&mut found);
                    } else if !known {
                        aborted = true;
                    } else {
                        let size = length.unwrap_or(alloc_size - offset as u64);
                        insert_use(&mut found, &mut visited_dead, offset, size, length.is_some(), Kind::MemSet);
                    }
                }
                // `visitMemTransferInst` (L1194).
                Some(Intrinsic::MemCpy | Intrinsic::MemMove) if used.index <= 1 => {
                    let length = constant_unsigned(context, instruction.operands[2]);
                    let volatile = constant_unsigned(context, instruction.operands[3]) != Some(0);
                    if length == Some(0) {
                        mark_dead(&mut found);
                        continue;
                    }
                    // Because these are visited twice, the first time may have
                    // marked it dead.
                    if found.dead_users.contains(&user) {
                        continue;
                    }
                    if !known {
                        aborted = true;
                        continue;
                    }
                    // This side of the transfer is out of bounds: the whole
                    // transfer goes, with the other side's slice if made.
                    if offset < 0 || offset as u128 >= u128::from(alloc_size) {
                        if let Some(&made) = transfer_slice.get(&user) {
                            found.slices[made].used = None;
                        }
                        mark_dead(&mut found);
                        continue;
                    }
                    let raw = offset as u64;
                    let size = length.unwrap_or(alloc_size - raw);
                    // The same value as source and destination.
                    let operand = instruction.operands[used.index as usize];
                    if operand == instruction.operands[0] && operand == instruction.operands[1] {
                        if !volatile {
                            mark_dead(&mut found);
                        } else {
                            insert_use(&mut found, &mut visited_dead, offset, size, false, Kind::Transfer);
                        }
                        continue;
                    }
                    // Both sides seen: both point into this alloca.
                    let inserted = !transfer_slice.contains_key(&user);
                    let at = *transfer_slice.get_or_insert_with(user, || found.slices.len());
                    if !inserted {
                        // Equal begin offsets and not volatile: the transfer is
                        // elided.
                        if !volatile && found.slices[at].begin == raw {
                            found.slices[at].used = None;
                            mark_dead(&mut found);
                            continue;
                        }
                        // An offset transfer within the same alloca cannot be
                        // split.
                        found.slices[at].splittable = false;
                    }
                    insert_use(
                        &mut found,
                        &mut visited_dead,
                        offset,
                        size,
                        inserted && length.is_some(),
                        Kind::Transfer,
                    );
                }
                // `visitIntrinsicInst` (L1266): a lifetime marker.
                Some(Intrinsic::LifetimeStart | Intrinsic::LifetimeEnd) if used.index == 1 => {
                    if !known {
                        aborted = true;
                    } else {
                        insert_use(&mut found, &mut visited_dead, offset, alloc_size, true, Kind::Lifetime);
                    }
                }
                // A droppable intrinsic (`llvm.assume`).
                Some(Intrinsic::Assume) => found.dead_use_if_promotable.push(used),
                // `visitCallBase` (L1402): any other call escapes the pointer.
                _ => {
                    found.escaped = true;
                    aborted = true;
                }
            },
            // DEVIATE: `visitCallBase` (L1402): an invoke escapes the pointer.
            Opcode::Invoke(_) => {
                found.escaped = true;
                aborted = true;
            }
            // `visitInstruction` (L1400).
            _ => aborted = true,
        }
        if aborted {
            break;
        }
    }
    if aborted || found.escaped {
        found.escaped = true;
        return found;
    }
    // Slices killed or never made are dropped, the rest ordered.
    found.slices.retain(|one| one.used.is_some());
    found.slices.sort_by(Slice::order);
    found
}

// ---- partitions ----------------------------------------------------------

/// `Partition` (L732): a span of the alloca no slice crosses the edge of.
#[derive(Clone, Debug, Default)]
struct Partition {
    begin: u64,
    end: u64,
    /// The slices that start in the partition, `slices[first..last]`.
    first: usize,
    last: usize,
    /// Splittable slices that started earlier and run into the partition.
    split_tails: Vec<usize>,
}

impl Partition {
    /// Partition::size. SROA.cpp#L767
    fn size(&self) -> u64 {
        self.end - self.begin
    }

    /// Partition::empty. SROA.cpp#L774
    fn is_empty(&self) -> bool {
        self.first == self.last
    }
}

/// `AllocaSlices::partitions()` (L992) with `partition_iterator::advance`
/// (L837), each partition in turn.
/// AllocaSlices::partitions. SROA.cpp#L992
fn partitions(slices: &[Slice]) -> Vec<Partition> {
    let se = slices.len();
    let mut p = Partition::default();
    let mut max_split_slice_end_offset = 0;
    let mut out = Vec::new();
    // The constructor: an empty partition at the start, advanced once.
    let advance = |p: &mut Partition, max_split: &mut u64| {
        // Clear out any split uses which have ended.
        if !p.split_tails.is_empty() {
            if p.end >= *max_split {
                p.split_tails.clear();
                *max_split = 0;
            } else {
                let end = p.end;
                p.split_tails.retain(|&tail| slices[tail].end > end);
            }
        }
        if p.first == se {
            return;
        }
        // If we had a non-empty partition previously, set up the state for
        // subsequent partitions.
        if p.first != p.last {
            for at in p.first..p.last {
                if slices[at].splittable && slices[at].end > p.end {
                    p.split_tails.push(at);
                    *max_split = (*max_split).max(slices[at].end);
                }
            }
            p.first = p.last;
            if p.first == se {
                p.begin = p.end;
                p.end = *max_split;
                return;
            }
            if !p.split_tails.is_empty() && slices[p.first].begin != p.end && !slices[p.first].splittable {
                p.begin = p.end;
                p.end = slices[p.first].begin;
                return;
            }
        }
        p.begin = if p.split_tails.is_empty() { slices[p.first].begin } else { p.end };
        p.end = slices[p.first].end;
        p.last += 1;
        if !slices[p.first].splittable {
            // An unsplittable region starts at its first slice and extends
            // through all that overlap it.
            while p.last != se && slices[p.last].begin < p.end {
                if !slices[p.last].splittable {
                    p.end = p.end.max(slices[p.last].end);
                }
                p.last += 1;
            }
            return;
        }
        // A splittable slice starts a synthetic partition spanning it and the
        // overlapping splittable ones.
        while p.last != se && slices[p.last].begin < p.end && slices[p.last].splittable {
            p.end = p.end.max(slices[p.last].end);
            p.last += 1;
        }
        if p.last != se && slices[p.last].begin < p.end {
            p.end = slices[p.last].begin;
        }
    };
    if se != 0 {
        advance(&mut p, &mut max_split_slice_end_offset);
    }
    while !(p.first == se && p.split_tails.is_empty()) {
        out.push(p.clone());
        advance(&mut p, &mut max_split_slice_end_offset);
    }
    out
}

// ---- types of partitions -------------------------------------------------

/// `stripAggregateTypeWrapping`. SROA.cpp#L4794
fn strip_aggregate_type_wrapping(
    layout: &DataLayout,
    context: &Context,
    ty: TypeId,
) -> TypeId {
    if is_single_value(context, ty) {
        return ty;
    }
    let types = &context.types;
    let alloc = layout.alloc_size(types, ty);
    let bits = layout.size_bits(types, ty);
    let inner = match types.get(ty) {
        Type::Array { element, .. } => *element,
        Type::Struct { .. } | Type::Named(_) => {
            let (_, offsets) = layout.struct_layout(types, ty);
            // `getElementContainingOffset(0)`: the last field at offset 0.
            let index = offsets.iter().rposition(|&one| one == 0).unwrap_or(0);
            match types.member(ty, index as u64) {
                Some(member) => member,
                None => return ty,
            }
        }
        _ => return ty,
    };
    if alloc > layout.alloc_size(types, inner) || bits > layout.size_bits(types, inner) {
        return ty;
    }
    strip_aggregate_type_wrapping(layout, context, inner)
}

/// `StructLayout::getElementContainingOffset`.
/// `StructLayout::getElementContainingOffset` (DataLayout.cpp).
fn element_containing_offset(
    offsets: &[u64],
    offset: u64,
) -> usize {
    offsets.partition_point(|&one| one <= offset).saturating_sub(1)
}

/// `getTypePartition`. SROA.cpp#L4832
fn get_type_partition(
    layout: &DataLayout,
    context: &Context,
    ty: TypeId,
    offset: u64,
    size: u64,
) -> Option<TypeId> {
    let alloc = layout.alloc_size(&context.types, ty);
    if offset == 0 && alloc == size {
        return Some(strip_aggregate_type_wrapping(layout, context, ty));
    }
    if offset > alloc || alloc - offset < size {
        return None;
    }
    if let Type::Array { element, count } = context.types.get(ty).clone() {
        let element_size = layout.alloc_size(&context.types, element);
        let skipped = offset / element_size;
        if skipped >= count {
            return None;
        }
        let offset = offset - skipped * element_size;
        if offset > 0 || size < element_size {
            if offset + size > element_size {
                return None;
            }
            return get_type_partition(layout, context, element, offset, size);
        }
        if size == element_size {
            return Some(strip_aggregate_type_wrapping(layout, context, element));
        }
        let elements = size / element_size;
        if elements * element_size != size {
            return None;
        }
        return Some(context.types.intern(Type::Array { element, count: elements }));
    }
    if !matches!(context.types.get(ty), Type::Struct { .. } | Type::Named(_)) {
        return None;
    }
    let (struct_size, offsets) = layout.struct_layout(&context.types, ty);
    if offset >= struct_size {
        return None;
    }
    let end_offset = offset + size;
    if end_offset > struct_size {
        return None;
    }
    let index = element_containing_offset(&offsets, offset);
    let offset = offset - offsets[index];
    let element = context.types.member(ty, index as u64)?;
    let element_size = layout.alloc_size(&context.types, element);
    if offset >= element_size {
        return None;
    }
    if offset > 0 || size < element_size {
        if offset + size > element_size {
            return None;
        }
        return get_type_partition(layout, context, element, offset, size);
    }
    if size == element_size {
        return Some(strip_aggregate_type_wrapping(layout, context, element));
    }
    let fields: Vec<TypeId> = context.types.fields(ty)?.to_vec();
    let packed = match context.types.get(ty) {
        Type::Struct { packed, .. } => *packed,
        Type::Named(name) => context.types.body(name).is_some_and(|body| body.packed),
        _ => false,
    };
    let mut end_index = fields.len();
    if end_offset < struct_size {
        let at = element_containing_offset(&offsets, end_offset);
        if index == at {
            return None;
        }
        if offsets[at] != end_offset {
            return None;
        }
        end_index = at;
    }
    let sub = context.types.intern(Type::Struct { fields: fields[index..end_index].to_vec(), packed });
    let (sub_size, _) = layout.struct_layout(&context.types, sub);
    if size != sub_size {
        return None;
    }
    Some(sub)
}

/// `findCommonType`. SROA.cpp#L1514
fn find_common_type(
    context: &Context,
    function: &Function,
    slices: &[Slice],
    first: usize,
    last: usize,
    end_offset: u64,
) -> (Option<TypeId>, Option<TypeId>) {
    let mut ty: Option<TypeId> = None;
    let mut ty_is_common = true;
    let mut integer: Option<TypeId> = None;
    // A partition of split tails alone has no slice of its own.
    let Some(begin) = slices.get(first).filter(|_| first < last).map(|one| one.begin) else { return (None, None) };
    for slice in &slices[first..last] {
        let used = slice.used.expect("a live slice");
        if slice.begin != begin || slice.end != end_offset {
            continue;
        }
        let user = function.instruction(used.user);
        // An intrinsic says nothing of the type.
        if slice.kind != Kind::Memory && slice.kind != Kind::PhiOrSelect {
            continue;
        }
        let user_ty = match user.opcode {
            Opcode::Load { .. } => Some(user.ty),
            Opcode::Store { .. } => function.operand_type(context, user.operands[0]),
            Opcode::Phi | Opcode::Select => find_common_type_through_phi_or_select(context, function, used.user),
            _ => None,
        };
        if let Some(bits) = user_ty.and_then(|one| int_bits(context, one)) {
            // Larger than the partition, or not a multiple of bytes: skipped.
            if bits % 8 != 0 || u64::from(bits / 8) > end_offset - begin {
                continue;
            }
            let largest = integer.and_then(|one| int_bits(context, one));
            if largest.is_none_or(|one| one < bits) {
                integer = user_ty;
            }
        }
        match user_ty {
            Some(one) if ty.is_none_or(|current| current == one) => ty = Some(one),
            _ => ty_is_common = false,
        }
    }
    (if ty_is_common { ty } else { None }, integer)
}

/// `isIntegerWideningViableForSlice`. SROA.cpp#L2456
fn is_integer_widening_viable_for_slice(
    layout: &DataLayout,
    context: &Context,
    function: &Function,
    slice: &Slice,
    alloc_begin_offset: u64,
    alloca_ty: TypeId,
    whole_alloca_op: &mut bool,
) -> bool {
    let size = layout.store_size(&context.types, alloca_ty);
    // A split tail starts before the partition.
    let Some(rel_begin) = slice.begin.checked_sub(alloc_begin_offset) else { return false };
    let rel_end = slice.end - alloc_begin_offset;
    let used = slice.used.expect("a live slice");
    let user = function.instruction(used.user);
    // Lifetime markers are always promotable and do not count.
    if slice.kind == Kind::Lifetime {
        return true;
    }
    if rel_end > size {
        return false;
    }
    // DEVIATE: an odd-sized slice would be cut as an i24 or the like.
    if !supported_int_size(layout, rel_end - rel_begin) {
        return false;
    }
    match user.opcode {
        Opcode::Load { volatile, .. } => {
            if volatile {
                return false;
            }
            if layout.store_size(&context.types, user.ty) > size {
                return false;
            }
            if rel_begin == 0 && rel_end == size {
                *whole_alloca_op = true;
            }
            if let Some(bits) = int_bits(context, user.ty) {
                if u64::from(bits) < layout.store_size(&context.types, user.ty) * 8 {
                    return false;
                }
            } else if rel_begin != 0 || rel_end != size || !can_convert_value(layout, context, alloca_ty, user.ty) {
                return false;
            }
        }
        Opcode::Store { volatile, .. } => {
            let value_ty = function.operand_type(context, user.operands[0]).expect("a typed value");
            if volatile {
                return false;
            }
            if layout.store_size(&context.types, value_ty) > size {
                return false;
            }
            if rel_begin == 0 && rel_end == size {
                *whole_alloca_op = true;
            }
            if let Some(bits) = int_bits(context, value_ty) {
                if u64::from(bits) < layout.store_size(&context.types, value_ty) * 8 {
                    return false;
                }
            } else if rel_begin != 0 || rel_end != size || !can_convert_value(layout, context, value_ty, alloca_ty) {
                return false;
            }
        }
        // A memory intrinsic: a constant length, not volatile, splittable.
        Opcode::Call(_) if slice.kind == Kind::MemSet || slice.kind == Kind::Transfer => {
            let volatile = constant_unsigned(context, user.operands[3]) != Some(0);
            if volatile || !matches!(user.operands[2], Operand::Constant(_)) {
                return false;
            }
            if !slice.splittable {
                return false;
            }
        }
        _ => return false,
    }
    true
}

/// `isIntegerWideningViable`. SROA.cpp#L2551
fn is_integer_widening_viable(
    layout: &DataLayout,
    context: &Context,
    function: &Function,
    slices: &[Slice],
    p: &Partition,
    alloca_ty: TypeId,
) -> bool {
    let size_in_bits = layout.size_bits(&context.types, alloca_ty);
    // The widest integer MIR names.
    if size_in_bits > 128 {
        return false;
    }
    if size_in_bits != layout.store_size(&context.types, alloca_ty) * 8 {
        return false;
    }
    let int_ty = context.types.int(size_in_bits as u32);
    if !can_convert_value(layout, context, alloca_ty, int_ty) || !can_convert_value(layout, context, int_ty, alloca_ty)
    {
        return false;
    }
    let mut whole_alloca_op = p.is_empty() && layout.legal_integer(size_in_bits as u32);
    for slice in &slices[p.first..p.last] {
        if !is_integer_widening_viable_for_slice(
            layout,
            context,
            function,
            slice,
            p.begin,
            alloca_ty,
            &mut whole_alloca_op,
        ) {
            return false;
        }
    }
    for &tail in &p.split_tails {
        if !is_integer_widening_viable_for_slice(
            layout,
            context,
            function,
            &slices[tail],
            p.begin,
            alloca_ty,
            &mut whole_alloca_op,
        ) {
            return false;
        }
    }
    whole_alloca_op
}

/// `BasicLoadAndStorePromoter`
/// ([L6189](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SROA.cpp#L6189)):
/// a promotion that keeps the stores and the alloca, and reads an undefined
/// value where the alloca stands for a store.
struct BasicPromotion {
    undef: Operand,
    alloca: InstId,
}

impl llrm_analysis::loadstorepromoter::Promotion for BasicPromotion {
    fn should_delete(
        &self,
        function: &Function,
        inst: InstId,
    ) -> bool {
        inst != self.alloca && !matches!(function.instruction(inst).opcode, Opcode::Store { .. })
    }

    fn value_for_alloca(
        &self,
        _context: &Context,
        _alloca: InstId,
    ) -> Operand {
        self.undef
    }
}

/// The state of a run: `SROA` (L174).
struct State<'a, 'b, 'c> {
    unit: &'a mut passes::Unit<'b>,
    analyses: &'c mut Analyses,
    worklist: llrm_mir::dense::IdSetVector<InstId>,
    post_promotion_worklist: llrm_mir::dense::IdSetVector<InstId>,
    promotable_allocas: llrm_mir::dense::IdSetVector<InstId>,
    dead_insts: Vec<InstId>,
    globals: std::rc::Rc<Vec<llrm_mir::module::GlobalValue>>,
    /// `SpeculatablePHIs`, `PHIsWithStoreToRewrite`, `SelectsToRewrite`.
    speculatable_phis: Vec<InstId>,
    phis_with_store_to_rewrite: Vec<InstId>,
    selects_to_rewrite: Vec<(InstId, Vec<InstId>)>,
}

/// What `AllocaSliceRewriter` carries from slice to slice (L2811).
struct Rewriter {
    new_ai: InstId,
    new_ai_value: ValueId,
    new_alloca_begin: u64,
    new_alloca_end: u64,
    new_alloca_ty: TypeId,
    /// The integer the accesses are widened to, when the partition passed
    /// `isIntegerWideningViable`.
    int_ty: Option<TypeId>,
    begin: u64,
    end: u64,
    new_begin: u64,
    new_end: u64,
    slice_size: u64,
    is_split: bool,
    /// The slice is splittable (`IsSplittable`, L2866).
    is_splittable: bool,
    old_ai: InstId,
    old_use: Option<Use>,
    old_ptr: Option<Operand>,
    /// The phis and selects the partition's pointer reached (`PHIUsers`,
    /// `SelectUsers`).
    phi_users: Vec<InstId>,
    select_users: Vec<InstId>,
}

impl<'a, 'b, 'c> State<'a, 'b, 'c> {
    fn new(
        unit: &'a mut passes::Unit<'b>,
        analyses: &'c mut Analyses,
    ) -> Self {
        let globals = std::rc::Rc::clone(&analyses.outer().globals);
        Self {
            unit,
            analyses,
            globals,
            worklist: Default::default(),
            post_promotion_worklist: Default::default(),
            promotable_allocas: Default::default(),
            dead_insts: Vec::new(),
            speculatable_phis: Vec::new(),
            phis_with_store_to_rewrite: Vec::new(),
            selects_to_rewrite: Vec::new(),
        }
    }

    fn function(&self) -> &Function {
        self.unit.function
    }

    /// The intrinsic `inst` calls, if it calls one.
    fn intrinsic(
        &self,
        inst: InstId,
    ) -> Option<Intrinsic> {
        let callee = llrm_mir::memory::callee(self.context(), self.function(), inst)?;
        self.globals.get(callee.0 as usize)?.intrinsic()
    }

    fn context(&self) -> &Context {
        self.unit.context
    }

    /// `SROA::runSROA`. SROA.cpp#L6430
    fn run_sroa(&mut self) -> bool {
        let Some(entry) = self.function().entry() else { return false };
        let allocas: Vec<InstId> = self
            .function()
            .iter_block(entry)
            .filter(|&inst| matches!(
                self.function().instruction(inst).opcode,
                Opcode::Alloca { .. }
            ))
            .collect();
        for inst in allocas {
            self.worklist.push(inst);
        }
        let mut changed = false;
        loop {
            while let Some(alloca) = self.worklist.pop() {
                if self.function().is_erased(alloca) {
                    continue;
                }
                changed |= self.run_on_alloca(alloca);
                let mut deleted: Vec<InstId> = Vec::new();
                changed |= self.delete_dead_instructions(&mut deleted);
                if !deleted.is_empty() {
                    self.worklist.set_subtract(deleted.iter().copied());
                    self.post_promotion_worklist.set_subtract(deleted.iter().copied());
                    self.promotable_allocas.set_subtract(deleted.iter().copied());
                }
            }
            changed |= self.promote_allocas();
            self.worklist = std::mem::take(&mut self.post_promotion_worklist);
            if self.worklist.is_empty() {
                break;
            }
        }
        changed
    }

    /// `SROA::promoteAllocas`. SROA.cpp#L6414
    fn promote_allocas(&mut self) -> bool {
        let function = &*self.unit.function;
        self.promotable_allocas.retain(|&one| !function.is_erased(one));
        if self.promotable_allocas.is_empty() {
            return false;
        }
        let allocas: Vec<InstId> = std::mem::take(&mut self.promotable_allocas).iter().collect();
        let outer = std::rc::Rc::clone(self.analyses.outer());
        crate::mem2reg::promote_mem_to_reg(
            self.unit.context,
            self.unit.layout,
            outer.callees(),
            self.unit.function,
            &allocas,
        );
        true
    }

    /// `isInstructionTriviallyDead` (Local.cpp), used as SROA.cpp#L3445
    /// `deleteIfTriviallyDead` does: nothing uses it and it does nothing else.
    fn is_trivially_dead(
        &self,
        inst: InstId,
    ) -> bool {
        let function = self.function();
        if function.is_erased(inst) {
            return false;
        }
        let outer = self.analyses.outer();
        crate::local::is_instruction_trivially_dead(self.context(), outer.callees(), &self.globals, function, inst)
    }

    /// `PoisonValue::get`.
    fn poison_of(
        &mut self,
        ty: TypeId,
    ) -> Operand {
        Operand::Constant(self.unit.context.constant(Constant { ty, kind: ConstantKind::Poison }))
    }

    /// `clobberUse`. SROA.cpp#L6168
    fn clobber_use(
        &mut self,
        used: Use,
    ) {
        let old = self.function().instruction(used.user).operands[used.index as usize];
        let Some(ty) = self.function().operand_type(self.context(), old) else { return };
        let poison = self.poison_of(ty);
        self.unit.function.set_operand(used.user, used.index as usize, poison);
        if let Operand::Value(value) = old {
            if let llrm_mir::module::ValueDef::Instruction(def) = self.function().value(value).def {
                if self.is_trivially_dead(def) {
                    self.dead_insts.push(def);
                }
            }
        }
    }

    /// `SROA::deleteDeadInstructions`. SROA.cpp#L6374
    fn delete_dead_instructions(
        &mut self,
        deleted_allocas: &mut Vec<InstId>,
    ) -> bool {
        let mut changed = false;
        while let Some(inst) = self.dead_insts.pop() {
            if self.function().is_erased(inst) {
                continue;
            }
            if matches!(
                self.function().instruction(inst).opcode,
                Opcode::Alloca { .. }
            ) {
                deleted_allocas.push(inst);
            }
            if let Some(result) = self.function().instruction(inst).result {
                if !self.function().users(result).is_empty() {
                    // `UndefValue::get` (SROA.cpp#L6393).
                    let ty = self.function().value(result).ty;
                    let undef =
                        Operand::Constant(self.unit.context.constant(Constant { ty, kind: ConstantKind::Undef }));
                    self.unit.function.replace_value(result, undef);
                }
            }
            let operands: Vec<InstId> = self
                .function()
                .instruction(inst)
                .operands
                .iter()
                .filter_map(|operand| match operand {
                    Operand::Value(value) => match self.function().value(*value).def {
                        llrm_mir::module::ValueDef::Instruction(def) => Some(def),
                        _ => None,
                    },
                    _ => None,
                })
                .collect();
            self.unit.function.erase(inst).expect("its uses were replaced");
            for def in operands {
                if self.is_trivially_dead(def) {
                    self.dead_insts.push(def);
                }
            }
            changed = true;
        }
        changed
    }

    /// `propagateStoredValuesToLoads`
    /// ([L6200](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SROA.cpp#L6200)):
    /// where a pointer into the alloca only escapes read-only (a load at an
    /// unknown offset), each run of slices with one range and one type of
    /// plain loads and stores has its loads read the stored values, the
    /// stores staying. DEVIATE: the pin reports a change whether or not a run
    /// was promoted; this reports the runs promoted.
    fn propagate_stored_values_to_loads(
        &mut self,
        alloca: InstId,
        slices: &AllocaSlices,
    ) -> bool {
        let mut changed = false;
        let mut all_same_and_valid = true;
        let mut partition_ty: Option<TypeId> = None;
        let mut insts: Vec<InstId> = Vec::new();
        let (mut begin, mut end) = (0u64, 0u64);
        for slice in &slices.slices {
            let Some(used) = slice.used else { continue };
            let user = used.user;
            // `isAssumeLikeIntrinsic`: a lifetime marker.
            if slice.kind == Kind::Lifetime {
                continue;
            }
            if slice.begin >= end {
                changed |= self.flush_stored_values(alloca, all_same_and_valid, partition_ty, &mut insts);
                all_same_and_valid = true;
                partition_ty = None;
                begin = slice.begin;
                end = slice.end;
            } else if slice.begin != begin || slice.end != end {
                all_same_and_valid = false;
                end = end.max(slice.end);
                continue;
            }
            let instruction = self.function().instruction(user);
            let access_ty = match instruction.opcode {
                Opcode::Load { volatile, .. } => Some((instruction.ty, !volatile)),
                Opcode::Store { volatile, .. } => {
                    let ty = self.function().operand_type(self.context(), instruction.operands[0]);
                    ty.map(|ty| (ty, !volatile))
                }
                _ => None,
            };
            match access_ty {
                Some((ty, simple)) => {
                    // `LoadAndStorePromoter` requires all the types to be the
                    // same.
                    if !simple || partition_ty.is_some_and(|seen| seen != ty) {
                        all_same_and_valid = false;
                    }
                    partition_ty = Some(ty);
                    insts.push(user);
                }
                None => all_same_and_valid = false,
            }
        }
        changed | self.flush_stored_values(alloca, all_same_and_valid, partition_ty, &mut insts)
    }

    /// The `Flush` of `propagateStoredValuesToLoads`: the run of accesses
    /// `insts`, if all alike, with the alloca as an undefined store, through a
    /// [`BasicPromotion`].
    fn flush_stored_values(
        &mut self,
        alloca: InstId,
        all_same_and_valid: bool,
        partition_ty: Option<TypeId>,
        insts: &mut Vec<InstId>,
    ) -> bool {
        let run = std::mem::take(insts);
        let (true, Some(ty), Some(&first)) = (all_same_and_valid, partition_ty, run.first()) else { return false };
        // `SSAUpdater::Initialize` names its phis after the first access's
        // value.
        let first_value = match self.function().instruction(first).result {
            Some(result) => Some(Operand::Value(result)),
            None => Some(self.function().instruction(first).operands[0]),
        };
        let name = match first_value {
            Some(Operand::Value(value)) => self.function().value(value).name.clone(),
            _ => None,
        };
        let promotion = BasicPromotion {
            undef: Operand::Constant(self.unit.context.constant(Constant { ty, kind: ConstantKind::Undef })),
            alloca,
        };
        let mut accesses = run;
        accesses.push(alloca);
        let mut ssa = llrm_analysis::ssaupdater::SsaUpdater::new(ty, name.as_deref());
        llrm_analysis::loadstorepromoter::run(
            self.unit.context,
            self.unit.layout,
            self.unit.function,
            &mut ssa,
            &accesses,
            &promotion,
        );
        true
    }

    /// `SROA::runOnAlloca`. SROA.cpp#L6281
    fn run_on_alloca(
        &mut self,
        alloca: InstId,
    ) -> bool {
        let mut changed = false;
        let result = self.function().instruction(alloca).result.expect("an alloca has a result");
        if self.function().users(result).is_empty() {
            self.unit.function.erase(alloca).expect("no users");
            return true;
        }
        let Opcode::Alloca { allocated, .. } = self.function().instruction(alloca).opcode else { return false };
        let size = self.unit.layout.alloc_size(&self.context().types, allocated);
        if !self.function().instruction(alloca).operands.is_empty() || size == 0 {
            return false;
        }
        // First, split any aggregate loads and stores touching this alloca.
        changed |= self.agg_rewrite(alloca);
        let mut slices = build_slices(self.context(), self.unit.layout, &self.globals, self.function(), alloca);
        if slices.escaped {
            return changed;
        }
        // A pointer that only escapes read-only: the stored values still reach
        // the loads at known offsets.
        if slices.escaped_read_only {
            return changed | self.propagate_stored_values_to_loads(alloca, &slices);
        }
        // Delete all the dead users of this alloca before splitting it.
        for dead in std::mem::take(&mut slices.dead_users) {
            let uses: Vec<Use> = (0..self.function().instruction(dead).operands.len())
                .map(|index| Use { user: dead, index: index as u32 })
                .collect();
            for used in uses {
                self.clobber_use(used);
            }
            if let Some(result) = self.function().instruction(dead).result {
                let ty = self.function().value(result).ty;
                let poison = self.poison_of(ty);
                self.unit.function.replace_value(result, poison);
            }
            self.dead_insts.push(dead);
            changed = true;
        }
        for used in std::mem::take(&mut slices.dead_operands) {
            self.clobber_use(used);
            changed = true;
        }
        if slices.slices.is_empty() {
            return changed;
        }
        changed |= self.split_alloca(alloca, &mut slices);
        // Speculating phis, rewriting stores through phis, rewriting selects.
        while let Some(phi) = self.speculatable_phis.pop() {
            self.speculate_phi_node_loads(phi);
        }
        for phi in std::mem::take(&mut self.phis_with_store_to_rewrite).into_iter().rev() {
            self.rewrite_phi_node_store(phi);
        }
        for (select, loads) in std::mem::take(&mut self.selects_to_rewrite).into_iter().rev() {
            self.rewrite_select_inst_mem_ops(select, &loads);
        }
        changed
    }

    /// `SROA::splitAlloca`. SROA.cpp#L5964
    fn split_alloca(
        &mut self,
        alloca: InstId,
        slices: &mut AllocaSlices,
    ) -> bool {
        let mut changed = false;
        // First try to pre-split loads and stores.
        changed |= self.presplit_loads_and_stores(alloca, slices);
        let Opcode::Alloca { allocated, .. } = self.function().instruction(alloca).opcode else { return false };
        let alloca_size = self.unit.layout.alloc_size(&self.context().types, allocated);
        // We can split at the begin and end offsets of each slice, but only if
        // those offsets do not lie inside another slice.
        let mut splittable_offset: std::collections::BTreeSet<u64> = Default::default();
        let (mut cur_begin, mut cur_end) = (0_u64, 0_u64);
        for slice in &slices.slices {
            if slice.begin > cur_begin || slice.end > cur_end {
                if slice.begin >= cur_end {
                    splittable_offset.insert(slice.begin);
                }
                if cur_end > slice.begin && cur_end < slice.end {
                    splittable_offset.remove(&cur_end);
                }
                cur_begin = slice.begin;
                if slice.end > cur_end {
                    cur_end = slice.end;
                    splittable_offset.insert(cur_end);
                }
            }
        }
        let mut sorted = true;
        for slice in &mut slices.slices {
            if !slice.splittable {
                continue;
            }
            if (slice.begin > alloca_size || splittable_offset.contains(&slice.begin))
                && (slice.end > alloca_size || splittable_offset.contains(&slice.end))
            {
                continue;
            }
            let user = slice.used.expect("a live slice").user;
            if matches!(
                self.unit.function.instruction(user).opcode,
                Opcode::Load { .. } | Opcode::Store { .. }
            ) {
                slice.splittable = false;
                sorted = false;
            }
        }
        if !sorted {
            slices.slices.sort_by(Slice::order);
        }
        // DEVIATE: a load or store is not cut into a piece isel has no integer
        // for (an i24 from a 3-byte piece): it is made unsplittable, which
        // changes the partitions, until none is.
        let layout = self.unit.layout;
        loop {
            let mut odd: Vec<usize> = Vec::new();
            for p in partitions(&slices.slices) {
                for at in (p.first..p.last).chain(p.split_tails.iter().copied()) {
                    let slice = &slices.slices[at];
                    if !slice.splittable || (slice.begin >= p.begin && slice.end <= p.end) {
                        continue;
                    }
                    let piece = slice.end.min(p.end) - slice.begin.max(p.begin);
                    let user = slice.used.expect("a live slice").user;
                    if !supported_int_size(layout, piece)
                        && matches!(
                            self.unit.function.instruction(user).opcode,
                            Opcode::Load { .. } | Opcode::Store { .. }
                        )
                    {
                        odd.push(at);
                    }
                }
            }
            if odd.is_empty() {
                break;
            }
            for at in odd {
                slices.slices[at].splittable = false;
            }
            slices.slices.sort_by(Slice::order);
        }
        // Rewrite each partition.
        for (number, p) in partitions(&slices.slices).iter().enumerate() {
            if self.rewrite_partition(alloca, &slices.slices, &slices.dead_use_if_promotable, p, number).is_some() {
                changed = true;
            }
        }
        changed
    }

    /// `SROA::presplitLoadsAndStores`: the splittable load and store pairs
    /// broken at the partitions' edges before any rewriting. SROA.cpp#L4965
    fn presplit_loads_and_stores(
        &mut self,
        alloca: InstId,
        alloca_slices: &mut AllocaSlices,
    ) -> bool {
        #[derive(Default)]
        struct SplitOffsets {
            slice: usize,
            splits: Vec<u64>,
        }
        let mut loads: Vec<InstId> = Vec::new();
        let mut stores: Vec<InstId> = Vec::new();
        let mut split_offsets: llrm_mir::dense::IdMap<InstId, SplitOffsets> = Default::default();
        // Loads out of this alloca which cannot be pre-split, and so neither
        // can the stores of them.
        let mut unsplittable_loads: llrm_mir::dense::IdSet<InstId> = Default::default();
        let value_load = |this: &Self, store: InstId| -> Option<InstId> {
            let Operand::Value(value) = this.function().instruction(store).operands[0] else { return None };
            match this.function().value(value).def {
                llrm_mir::module::ValueDef::Instruction(def)
                    if matches!(this.function().instruction(def).opcode, Opcode::Load { .. }) =>
                {
                    Some(def)
                }
                _ => None,
            }
        };
        let is_simple = |this: &Self, inst: InstId| {
            matches!(
                this.function().instruction(inst).opcode,
                Opcode::Load { volatile: false, .. } | Opcode::Store { volatile: false, .. }
            )
        };
        let parts = partitions(&alloca_slices.slices);
        for p in &parts {
            for at in p.first..p.last {
                let slice = alloca_slices.slices[at];
                let used = slice.used.expect("a live slice");
                let inst = used.user;
                if !slice.splittable || slice.end <= p.end {
                    // A load that cannot be pre-split, or a store of one.
                    match self.function().instruction(inst).opcode {
                        Opcode::Load { .. } => {
                            unsplittable_loads.insert(inst);
                        }
                        Opcode::Store { .. } => {
                            if let Some(load) = value_load(self, inst) {
                                unsplittable_loads.insert(load);
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                match self.function().instruction(inst).opcode {
                    Opcode::Load { .. } => {
                        // The load must be used only to store, simply, into
                        // other pointers.
                        let result = self.function().instruction(inst).result.expect("a load");
                        let simply_stored = self
                            .function()
                            .users(result)
                            .iter()
                            .all(
                                |one| matches!(
                                    self.function().instruction(one.user).opcode,
                                    Opcode::Store { .. }
                                )
                                    && one.index == 0
                                    && is_simple(self, one.user),
                            );
                        if !simply_stored {
                            unsplittable_loads.insert(inst);
                            continue;
                        }
                        loads.push(inst);
                    }
                    Opcode::Store { .. } => {
                        // Skip stores of pointers.
                        if used.index != 1 {
                            continue;
                        }
                        let Some(stored) = value_load(self, inst) else { continue };
                        if !is_simple(self, stored) {
                            continue;
                        }
                        stores.push(inst);
                    }
                    // Other uses cannot be pre-split.
                    _ => continue,
                }
                // Record the initial split.
                let offsets = split_offsets.get_or_insert_with(inst, Default::default);
                offsets.slice = at;
                offsets.splits.push(p.end - slice.begin);
            }
            // Add a split for each already split slice that is to be pre-split.
            for &tail in &p.split_tails {
                let inst = alloca_slices.slices[tail].used.expect("a live slice").user;
                let Some(offsets) = split_offsets.get_mut(&inst) else { continue };
                if alloca_slices.slices[tail].end > p.end {
                    let base = alloca_slices.slices[offsets.slice].begin;
                    offsets.splits.push(p.end - base);
                }
            }
        }
        // DEVIATE: a candidate cut into a part isel has no integer for stays.
        let layout_for_parts = self.unit.layout;
        let unsupported: Vec<InstId> = split_offsets
            .iter()
            .filter(|(_, offsets)| {
                let slice = alloca_slices.slices[offsets.slice];
                let mut from = 0_u64;
                offsets
                    .splits
                    .iter()
                    .copied()
                    .chain(std::iter::once(slice.end - slice.begin))
                    .any(
                        |to| {
                            let bad = !supported_int_size(layout_for_parts, to - from);
                            from = to;
                            bad
                        },
                    )
            })
            .map(|(inst, _)| inst)
            .collect();
        for inst in unsupported {
            match self.function().instruction(inst).opcode {
                Opcode::Load { .. } => {
                    unsplittable_loads.insert(inst);
                }
                _ => {
                    if let Some(load) = value_load(self, inst) {
                        unsplittable_loads.insert(load);
                    }
                    stores.retain(|&one| one != inst);
                }
            }
        }
        // A split load with split stores can only be pre-split if the splits
        // match exactly, relative to their start.
        stores.retain(|&store| {
            let load = value_load(self, store).expect("a store of a load");
            if unsplittable_loads.contains(&load) {
                return false;
            }
            let Some(load_offsets) = split_offsets.get(&load) else { return true };
            let store_offsets = split_offsets.get(&store).expect("a candidate");
            if load_offsets.splits == store_offsets.splits {
                return true;
            }
            unsplittable_loads.insert(load);
            false
        });
        // A later store may have made an earlier store's load unsplittable.
        stores.retain(|&store| !unsplittable_loads.contains(&value_load(self, store).expect("a store of a load")));
        loads.retain(|load| !unsplittable_loads.contains(load));
        if loads.is_empty() && stores.is_empty() {
            return false;
        }
        let layout = self.unit.layout;
        let mut new_slices: Vec<Slice> = Vec::new();
        let mut resplit_promotable: Vec<InstId> = Vec::new();
        let load_align = |this: &Self, inst: InstId, offset: u64| -> u64 {
            let instruction = this.function().instruction(inst);
            let base = match instruction.opcode {
                Opcode::Load { align: Some(align), .. } | Opcode::Store { align: Some(align), .. } => align,
                _ => {
                    let ty = match instruction.opcode {
                        Opcode::Store { .. } => this.function().operand_type(this.context(), instruction.operands[0]),
                        _ => Some(instruction.ty),
                    };
                    layout.align(&this.context().types, ty.expect("a typed access"))
                }
            };
            common_alignment(base, offset)
        };
        let is_alloca = |this: &Self, operand: Operand| -> Option<InstId> {
            let Operand::Value(value) = operand else { return None };
            match this.function().value(value).def {
                llrm_mir::module::ValueDef::Instruction(def)
                    if matches!(this.function().instruction(def).opcode, Opcode::Alloca { .. }) =>
                {
                    Some(def)
                }
                _ => None,
            }
        };
        // First, rewrite all of the split loads, building slices for them.
        let mut split_loads_map: llrm_mir::dense::IdMap<InstId, Vec<InstId>> = Default::default();
        for &load in &loads {
            let offsets = split_offsets.get(&load).expect("a candidate");
            let (slice_index, splits) = (offsets.slice, offsets.splits.clone());
            let original = alloca_slices.slices[slice_index];
            let slice_size = original.end - original.begin;
            let base_offset = original.begin;
            let base_ptr = self.function().instruction(load).operands[0];
            let mut split_loads: Vec<InstId> = Vec::new();
            let mut part_offset = 0_u64;
            let mut part_size = splits[0];
            let mut index = 0;
            loop {
                let part_ty = self.unit.context.types.int((part_size * 8) as u32);
                let pointer = self.adjusted_ptr(load, base_ptr, part_offset);
                let align = load_align(self, load, part_offset);
                let name = self
                    .function()
                    .instruction(load)
                    .result
                    .and_then(|result| self.function().value(result).name.clone());
                let part_load = self.build_inst(
                    load,
                    Opcode::Load { align: Some(align), volatile: false },
                    part_ty,
                    vec![pointer],
                    name.as_deref(),
                );
                split_loads.push(part_load);
                new_slices.push(Slice {
                    begin: base_offset + part_offset,
                    end: base_offset + part_offset + part_size,
                    used: Some(Use { user: part_load, index: 0 }),
                    splittable: false,
                    kind: Kind::Memory,
                });
                if index >= splits.len() {
                    break;
                }
                part_offset = splits[index];
                index += 1;
                part_size = (if index < splits.len() { splits[index] } else { slice_size }) - part_offset;
            }
            // Rewrite every store of the load as split stores, or save the
            // split loads for a store that is split anyway.
            let mut deferred = false;
            let result = self.function().instruction(load).result.expect("a load");
            let users: Vec<InstId> = self.function().users(result).iter().map(|one| one.user).collect();
            for store in users {
                if !stores.is_empty() && split_offsets.contains_key(&store) {
                    deferred = true;
                    continue;
                }
                let store_base = self.function().instruction(store).operands[1];
                for (part_index, &part_load) in split_loads.iter().enumerate() {
                    let part_offset = if part_index == 0 { 0 } else { splits[part_index - 1] };
                    let pointer = self.adjusted_ptr(store, store_base, part_offset);
                    let align = load_align(self, store, part_offset);
                    let value = Operand::Value(self.function().instruction(part_load).result.expect("a load"));
                    let void = self.unit.context.types.void();
                    self.build(
                        store,
                        Opcode::Store { align: Some(align), volatile: false },
                        void,
                        vec![value, pointer],
                        None,
                    );
                }
                // Look again at any alloca this store reached.
                if let Some(other) = is_alloca(self, store_base) {
                    resplit_promotable.push(other);
                    self.worklist.push(other);
                } else if let Some(other) = self.strip_in_bounds_offsets(store_base) {
                    self.worklist.push(other);
                }
                self.dead_insts.push(store);
            }
            if deferred {
                split_loads_map.insert(load, split_loads);
            }
            self.dead_insts.push(load);
            alloca_slices.slices[slice_index].used = None;
        }
        // Second, rewrite the split stores: of a load already split, from its
        // parts; of another, splitting that load first.
        for &store in &stores {
            let load = value_load(self, store).expect("a store of a load");
            let offsets = split_offsets.get(&store).expect("a candidate");
            let (slice_index, splits) = (offsets.slice, offsets.splits.clone());
            let original = alloca_slices.slices[slice_index];
            let ty = self.function().instruction(load).ty;
            let store_size = u64::from(int_bits(self.context(), ty).expect("an integer load")) / 8;
            let base_offset = original.begin;
            let load_base = self.function().instruction(load).operands[0];
            let store_base = self.function().instruction(store).operands[1];
            let already = split_loads_map.get(&load).cloned();
            let mut part_offset = 0_u64;
            let mut part_size = splits[0];
            let mut index = 0;
            loop {
                let part_ty = self.unit.context.types.int((part_size * 8) as u32);
                let part_load = match &already {
                    Some(parts) => parts[index],
                    None => {
                        let pointer = self.adjusted_ptr(load, load_base, part_offset);
                        let align = load_align(self, load, part_offset);
                        let name = self
                            .function()
                            .instruction(load)
                            .result
                            .and_then(|result| self.function().value(result).name.clone());
                        self.build_inst(
                            load,
                            Opcode::Load { align: Some(align), volatile: false },
                            part_ty,
                            vec![pointer],
                            name.as_deref(),
                        )
                    }
                };
                let pointer = self.adjusted_ptr(store, store_base, part_offset);
                let align = load_align(self, store, part_offset);
                let value = Operand::Value(self.function().instruction(part_load).result.expect("a load"));
                let void = self.unit.context.types.void();
                let part_store = self.build_inst(
                    store,
                    Opcode::Store { align: Some(align), volatile: false },
                    void,
                    vec![value, pointer],
                    None,
                );
                new_slices.push(Slice {
                    begin: base_offset + part_offset,
                    end: base_offset + part_offset + part_size,
                    used: Some(Use { user: part_store, index: 1 }),
                    splittable: false,
                    kind: Kind::Memory,
                });
                if index >= splits.len() {
                    break;
                }
                part_offset = splits[index];
                index += 1;
                part_size = (if index < splits.len() { splits[index] } else { store_size }) - part_offset;
            }
            // Look again at the other alloca a load not already split reached.
            if already.is_none() {
                if let Some(other) = is_alloca(self, load_base) {
                    resplit_promotable.push(other);
                    self.worklist.push(other);
                } else if let Some(other) = self.strip_in_bounds_offsets(load_base) {
                    self.worklist.push(other);
                }
            }
            // The original load goes with its only user.
            let load_result = self.function().instruction(load).result.expect("a load");
            if self.function().users(load_result).has_one_use() {
                self.dead_insts.push(load);
            }
            self.dead_insts.push(store);
            alloca_slices.slices[slice_index].used = None;
        }
        // Remove the killed slices, merge the new ones into the sorted
        // sequence.
        alloca_slices.slices.retain(|one| one.used.is_some());
        alloca_slices.slices.extend(new_slices);
        alloca_slices.slices.sort_by(Slice::order);
        // The allocas that need re-splitting are not promoted yet.
        self.promotable_allocas.set_subtract(resplit_promotable.iter().copied());
        let _ = alloca;
        true
    }

    /// `selectPartitionType`. SROA.cpp#L5527
    fn select_partition_type(
        &mut self,
        alloca: InstId,
        slices: &[Slice],
        p: &Partition,
    ) -> (TypeId, bool) {
        let layout = self.unit.layout;
        let Opcode::Alloca { allocated, .. } = self.function().instruction(alloca).opcode else { unreachable!() };
        // DEVIATE: no vector promotion (`isVectorPromotionViable`, L5559).
        let (common, largest_int) = find_common_type(self.context(), self.function(), slices, p.first, p.last, p.end);
        if let Some(common) = common {
            let common_size = layout.alloc_size(&self.context().types, common);
            if common_size >= p.size() {
                let widen =
                    is_integer_widening_viable(layout, self.unit.context, self.unit.function, slices, p, common);
                return (common, widen);
            }
        }
        // Can we find an appropriate subtype in the original allocated type?
        if let Some(mut partition_ty) = get_type_partition(layout, self.unit.context, allocated, p.begin, p.size()) {
            // An integer array a legal integer spans is represented by it.
            if let Type::Array { element, .. } = self.context().types.get(partition_ty).clone() {
                if int_bits(self.context(), element).is_some() && layout.legal_integer((p.size() * 8) as u32) {
                    partition_ty = self.unit.context.types.int((p.size() * 8) as u32);
                }
            }
            if is_integer_widening_viable(layout, self.unit.context, self.unit.function, slices, p, partition_ty) {
                return (partition_ty, true);
            }
            if let Some(largest) = largest_int {
                if layout.alloc_size(&self.context().types, largest) >= p.size()
                    && is_integer_widening_viable(layout, self.unit.context, self.unit.function, slices, p, largest)
                {
                    return (largest, true);
                }
            }
            return (partition_ty, false);
        }
        if let Some(largest) = largest_int {
            if layout.alloc_size(&self.context().types, largest) >= p.size() {
                return (largest, false);
            }
        }
        if layout.legal_integer((p.size() * 8) as u32) {
            return (self.unit.context.types.int((p.size() * 8) as u32), false);
        }
        let byte = self.unit.context.types.int(8);
        (self.unit.context.types.intern(Type::Array { element: byte, count: p.size() }), false)
    }

    /// `SROA::rewritePartition`. SROA.cpp#L5666
    fn rewrite_partition(
        &mut self,
        alloca: InstId,
        slices: &[Slice],
        dead_if_promotable: &[Use],
        p: &Partition,
        number: usize,
    ) -> Option<InstId> {
        let layout = self.unit.layout;
        let (partition_ty, widen) = self.select_partition_type(alloca, slices, p);
        let Opcode::Alloca { allocated, align, address_space } = self.function().instruction(alloca).opcode else {
            unreachable!("an alloca")
        };
        // The same type with the same offsets reuses the alloca.
        let new_ai = if partition_ty == allocated && p.begin == 0 {
            alloca
        } else {
            let old_align = align.unwrap_or_else(|| layout.align(&self.context().types, allocated));
            let alignment = common_alignment(old_align, p.begin);
            let ptr_ty = self.function().instruction(alloca).ty;
            let name = self
                .function()
                .instruction(alloca)
                .result
                .and_then(|result| self.function().value(result).name.clone())
                .map(|one| format!("{one}.sroa.{number}"));
            let created = self
                .unit
                .function
                .create_instruction(
                    Opcode::Alloca { allocated: partition_ty, align: Some(alignment), address_space },
                    ptr_ty,
                    Vec::new(),
                    Flags::default(),
                    name.as_deref(),
                );
            self.unit.function.insert(created, Position::Before(alloca)).expect("a placed alloca");
            created
        };
        let post_promotion_old_size = self.post_promotion_worklist.len();
        let int_ty =
            widen.then(|| self.unit.context.types.int(layout.size_bits(&self.context().types, partition_ty) as u32));
        let mut rewriter = Rewriter {
            new_ai,
            new_ai_value: self.function().instruction(new_ai).result.expect("an alloca has a result"),
            new_alloca_begin: p.begin,
            new_alloca_end: p.end,
            new_alloca_ty: partition_ty,
            int_ty,
            begin: 0,
            end: 0,
            new_begin: 0,
            new_end: 0,
            slice_size: 0,
            is_split: false,
            is_splittable: false,
            old_ai: alloca,
            old_use: None,
            old_ptr: None,
            phi_users: Vec::new(),
            select_users: Vec::new(),
        };
        let mut promotable = true;
        for &tail in &p.split_tails {
            promotable &= self.visit_slice(&mut rewriter, &slices[tail]);
        }
        for slice in &slices[p.first..p.last] {
            promotable &= self.visit_slice(&mut rewriter, slice);
        }
        // Now that the slices are processed, check whether any phi or select
        // would block promotion.
        let mut new_speculatable_phis: Vec<InstId> = Vec::new();
        let mut new_phis_with_store: Vec<InstId> = Vec::new();
        let mut select_users = std::mem::take(&mut rewriter.select_users);
        for phi in std::mem::take(&mut rewriter.phi_users) {
            if self.is_safe_phi_to_speculate(phi) {
                new_speculatable_phis.push(phi);
            } else if self.get_phi_store_to_rewrite(phi).is_some() {
                new_phis_with_store.push(phi);
            } else {
                promotable = false;
                select_users.clear();
                break;
            }
        }
        let mut new_selects: Vec<(InstId, Vec<InstId>)> = Vec::new();
        for &select in &select_users {
            match self.is_safe_select_to_speculate(select) {
                Some(loads) => new_selects.push((select, loads)),
                None => {
                    promotable = false;
                    break;
                }
            }
        }
        if promotable {
            // `dropDroppableUse`: an `llvm.assume`'s condition becomes true.
            for &used in dead_if_promotable {
                let old = self.function().instruction(used.user).operands[used.index as usize];
                let ty = self.type_of(old);
                let one = self.int_constant(ty, 1);
                self.unit.function.set_operand(used.user, used.index as usize, one);
                self.delete_if_trivially_dead(old);
            }
            if new_speculatable_phis.is_empty() && new_phis_with_store.is_empty() && select_users.is_empty() {
                // Promote the alloca.
                self.promotable_allocas.push(new_ai);
            } else {
                // The phis and selects to rewrite are queued and the new
                // alloca is looked at again to promote it next time.
                self.speculatable_phis.extend(new_speculatable_phis);
                self.phis_with_store_to_rewrite.extend(new_phis_with_store);
                self.selects_to_rewrite.extend(new_selects);
                self.worklist.push(new_ai);
            }
        } else {
            // Drop any post-promotion work items if promotion didn't happen.
            self.post_promotion_worklist.truncate(post_promotion_old_size);
            // We couldn't promote and we didn't create a new partition.
            if new_ai == alloca {
                return None;
            }
            self.worklist.push(new_ai);
        }
        Some(new_ai)
    }

    // ---- the rewriter ----------------------------------------------------

    /// `AllocaSliceRewriter::visit` (L2906).
    /// AllocaSliceRewriter::visit. SROA.cpp#L2906
    fn visit_slice(
        &mut self,
        rewriter: &mut Rewriter,
        slice: &Slice,
    ) -> bool {
        rewriter.begin = slice.begin;
        rewriter.end = slice.end;
        rewriter.is_split = slice.begin < rewriter.new_alloca_begin || slice.end > rewriter.new_alloca_end;
        rewriter.new_begin = slice.begin.max(rewriter.new_alloca_begin);
        rewriter.new_end = slice.end.min(rewriter.new_alloca_end);
        rewriter.slice_size = rewriter.new_end - rewriter.new_begin;
        let used = slice.used.expect("a live slice");
        rewriter.is_splittable = slice.splittable;
        rewriter.old_use = Some(used);
        rewriter.old_ptr = Some(self.function().instruction(used.user).operands[used.index as usize]);
        match self.function().instruction(used.user).opcode {
            Opcode::Load { .. } => self.visit_load_inst(rewriter, used.user),
            Opcode::Store { .. } => self.visit_store_inst(rewriter, used.user),
            Opcode::Call(_) => match self.intrinsic(used.user) {
                Some(Intrinsic::MemSet) => self.visit_mem_set_inst(rewriter, used.user),
                Some(Intrinsic::MemCpy | Intrinsic::MemMove) => self.visit_mem_transfer_inst(rewriter, used.user),
                _ => self.visit_intrinsic_inst(rewriter, used.user),
            },
            Opcode::Phi => self.visit_phi_node(rewriter, used.user),
            Opcode::Select => self.visit_select_inst(rewriter, used.user),
            _ => unreachable!("only loads, stores, memory intrinsics, phis and selects make slices"),
        }
    }

    /// A new instruction before `at`, its result as an operand.
    /// The `IRBuilderTy IRB` (SROA.cpp#L2864) insertion at the user.
    fn build_inst(
        &mut self,
        at: InstId,
        opcode: Opcode,
        ty: TypeId,
        operands: Vec<Operand>,
        name: Option<&str>,
    ) -> InstId {
        let created = self.unit.function.create_instruction(opcode, ty, operands, Flags::default(), name);
        self.unit.function.insert(created, Position::Before(at)).expect("a placed instruction");
        created
    }

    /// `build_inst`, its result as an operand.
    fn build(
        &mut self,
        at: InstId,
        opcode: Opcode,
        ty: TypeId,
        operands: Vec<Operand>,
        name: Option<&str>,
    ) -> Operand {
        let created = self.build_inst(at, opcode, ty, operands, name);
        match self.function().instruction(created).result {
            Some(result) => Operand::Value(result),
            None => Operand::Constant(self.unit.context.constant(Constant { ty, kind: ConstantKind::Zero })),
        }
    }

    fn int_constant(
        &mut self,
        ty: TypeId,
        value: i128,
    ) -> Operand {
        Operand::Constant(self.unit.context.int(ty, value))
    }

    fn type_of(
        &self,
        operand: Operand,
    ) -> TypeId {
        self.function().operand_type(self.context(), operand).expect("a typed operand")
    }

    /// `CreateBitPreservingCastChain` (IRBuilder.cpp#L107), for the
    /// conversions `canConvertValue` allows.
    fn bit_preserving_cast_chain(
        &mut self,
        at: InstId,
        value: Operand,
        new_ty: TypeId,
    ) -> Operand {
        let old_ty = self.type_of(value);
        if old_ty == new_ty {
            return value;
        }
        let op = if int_bits(self.context(), old_ty).is_some() && is_pointer(self.context(), new_ty) {
            CastOp::IntToPtr
        } else if is_pointer(self.context(), old_ty) && int_bits(self.context(), new_ty).is_some() {
            CastOp::PtrToInt
        } else if is_pointer(self.context(), old_ty) && is_pointer(self.context(), new_ty) {
            CastOp::AddrSpaceCast
        } else {
            CastOp::BitCast
        };
        self.build(at, Opcode::Cast(op), new_ty, vec![value], None)
    }

    /// `extractInteger`. SROA.cpp#L2592
    fn extract_integer(
        &mut self,
        at: InstId,
        value: Operand,
        ty: TypeId,
        offset: u64,
        name: &str,
    ) -> Operand {
        let layout = self.unit.layout;
        let int_ty = self.type_of(value);
        let int_size = layout.store_size(&self.context().types, int_ty);
        let size = layout.store_size(&self.context().types, ty);
        let mut shift = 8 * offset;
        if layout.big_endian {
            shift = 8 * (int_size - size - offset);
        }
        let mut value = value;
        if shift != 0 {
            let amount = self.int_constant(int_ty, shift as i128);
            value = self.build(
                at,
                Opcode::Binary(llrm_mir::BinaryOp::LShr),
                int_ty,
                vec![value, amount],
                Some(&format!("{name}.shift")),
            );
        }
        if ty != int_ty {
            value = self.build(at, Opcode::Cast(CastOp::Trunc), ty, vec![value], Some(&format!("{name}.trunc")));
        }
        value
    }

    /// `insertInteger`. SROA.cpp#L2617
    fn insert_integer(
        &mut self,
        at: InstId,
        old: Operand,
        value: Operand,
        offset: u64,
        name: &str,
    ) -> Operand {
        let layout = self.unit.layout;
        let int_ty = self.type_of(old);
        let ty = self.type_of(value);
        let int_width = int_bits(self.context(), int_ty).expect("an integer");
        let width = int_bits(self.context(), ty).expect("an integer");
        let mut value = value;
        if ty != int_ty {
            value = self.build(at, Opcode::Cast(CastOp::ZExt), int_ty, vec![value], Some(&format!("{name}.ext")));
        }
        let int_size = layout.store_size(&self.context().types, int_ty);
        let size = layout.store_size(&self.context().types, ty);
        let mut shift = 8 * offset;
        if layout.big_endian {
            shift = 8 * (int_size - size - offset);
        }
        if shift != 0 {
            let amount = self.int_constant(int_ty, shift as i128);
            value = self.build(
                at,
                Opcode::Binary(llrm_mir::BinaryOp::Shl),
                int_ty,
                vec![value, amount],
                Some(&format!("{name}.shift")),
            );
        }
        if shift != 0 || width < int_width {
            let mask = llrm_mir::context::mask(width) << shift;
            let inverted = !mask & llrm_mir::context::mask(int_width);
            let mask = self.int_constant(int_ty, inverted as i128);
            let old = self.build(
                at,
                Opcode::Binary(llrm_mir::BinaryOp::And),
                int_ty,
                vec![old, mask],
                Some(&format!("{name}.mask")),
            );
            value = self.build(
                at,
                Opcode::Binary(llrm_mir::BinaryOp::Or),
                int_ty,
                vec![old, value],
                Some(&format!("{name}.insert")),
            );
        }
        value
    }

    /// `getNewAllocaSlicePtr` and `getAdjustedPtr`. SROA.cpp#L3396, L2036
    ///
    /// The pointer is made in the old pointer's type
    /// (`CreatePointerBitCastOrAddrSpaceCast(Ptr, PointerTy)`, L2042): a use
    /// through an `addrspacecast` gets a cast of the new alloca back, so a call
    /// that takes the pointer still takes the type it declares.
    fn new_alloca_slice_ptr(
        &mut self,
        rewriter: &Rewriter,
        at: InstId,
    ) -> Operand {
        let offset = rewriter.new_begin - rewriter.new_alloca_begin;
        let base = Operand::Value(rewriter.new_ai_value);
        let ptr_ty = self.function().instruction(rewriter.new_ai).ty;
        let pointer = if offset == 0 {
            base
        } else {
            let space = pointer_space(self.context(), ptr_ty);
            let index_ty = self.unit.context.types.int(self.unit.layout.pointer(space).index_bits);
            let byte = self.unit.context.types.int(8);
            let index = self.int_constant(index_ty, offset as i128);
            let created = self
                .unit
                .function
                .create_instruction(
                    Opcode::GetElementPtr { source: byte },
                    ptr_ty,
                    vec![base, index],
                    Flags::INBOUNDS,
                    Some("sroa_idx"),
                );
            self.unit.function.insert(created, Position::Before(at)).expect("a placed instruction");
            Operand::Value(self.function().instruction(created).result.expect("a pointer"))
        };
        let wanted = rewriter.old_ptr.map(|old| self.type_of(old));
        match wanted {
            Some(wanted) if wanted != ptr_ty && is_pointer(self.context(), wanted) => {
                self.build(at, Opcode::Cast(CastOp::AddrSpaceCast), wanted, vec![pointer], Some("sroa_cast"))
            }
            _ => pointer,
        }
    }

    /// `getSliceAlign`. SROA.cpp#L3431
    fn slice_align(
        &self,
        rewriter: &Rewriter,
    ) -> u64 {
        common_alignment(self.new_ai_align(rewriter), rewriter.new_begin - rewriter.new_alloca_begin)
    }

    /// `NewAI.getAlign()`, SROA.cpp#L3468.
    fn new_ai_align(
        &self,
        rewriter: &Rewriter,
    ) -> u64 {
        match self.function().instruction(rewriter.new_ai).opcode {
            Opcode::Alloca { align: Some(align), .. } => align,
            _ => self.unit.layout.align(&self.context().types, rewriter.new_alloca_ty),
        }
    }

    /// `deleteIfTriviallyDead`. SROA.cpp#L3445
    fn delete_if_trivially_dead(
        &mut self,
        operand: Operand,
    ) {
        if let Operand::Value(value) = operand {
            if let llrm_mir::module::ValueDef::Instruction(def) = self.function().value(value).def {
                if self.is_trivially_dead(def) {
                    self.dead_insts.push(def);
                }
            }
        }
    }

    /// The metadata of `from` where `to` reads the same bytes.
    /// `copyMetadataForLoad` / `copyMetadata` (SROA.cpp#L3523, L3556).
    fn copy_metadata(
        &mut self,
        from: InstId,
        to: Operand,
        same: bool,
    ) {
        if !same {
            return;
        }
        let Operand::Value(value) = to else { return };
        let llrm_mir::module::ValueDef::Instruction(def) = self.function().value(value).def else { return };
        let metadata = self.function().instruction(from).metadata.clone();
        for (kind, node) in metadata {
            self.unit.function.annotate(def, &kind, node);
        }
    }

    /// `AllocaSliceRewriter::rewriteIntegerLoad`. SROA.cpp#L3464
    fn rewrite_integer_load(
        &mut self,
        rewriter: &Rewriter,
        load: InstId,
    ) -> Operand {
        let int_ty = rewriter.int_ty.expect("an integer partition");
        let align = self.new_ai_align(rewriter);
        let mut value = self.build(
            load,
            Opcode::Load { align: Some(align), volatile: false },
            rewriter.new_alloca_ty,
            vec![Operand::Value(rewriter.new_ai_value)],
            Some("load"),
        );
        value = self.bit_preserving_cast_chain(load, value, int_ty);
        let offset = rewriter.new_begin - rewriter.new_alloca_begin;
        if offset > 0 || rewriter.new_end < rewriter.new_alloca_end {
            let extract_ty = self.unit.context.types.int((rewriter.slice_size * 8) as u32);
            value = self.extract_integer(load, value, extract_ty, offset, "extract");
        }
        // The load may be wider than the slice (past the end of the alloca).
        let load_ty = self.function().instruction(load).ty;
        if u64::from(int_bits(self.context(), load_ty).expect("an integer load")) > rewriter.slice_size * 8 {
            value = self.build(load, Opcode::Cast(CastOp::ZExt), load_ty, vec![value], None);
        }
        value
    }

    /// `AllocaSliceRewriter::visitLoadInst`. SROA.cpp#L3488
    fn visit_load_inst(
        &mut self,
        rewriter: &mut Rewriter,
        load: InstId,
    ) -> bool {
        let layout = self.unit.layout;
        let old_op = self.function().instruction(load).operands[0];
        let load_ty = self.function().instruction(load).ty;
        let volatile = matches!(
            self.function().instruction(load).opcode,
            Opcode::Load { volatile: true, .. }
        );
        let target_ty =
            if rewriter.is_split { self.unit.context.types.int((rewriter.slice_size * 8) as u32) } else { load_ty };
        let whole = rewriter.new_begin == rewriter.new_alloca_begin && rewriter.new_end == rewriter.new_alloca_end;
        let mut ptr_adjusted = false;
        let mut value;
        if rewriter.int_ty.is_some() && int_bits(self.context(), load_ty).is_some() {
            value = self.rewrite_integer_load(rewriter, load);
        } else if whole
            && (can_convert_value(layout, self.context(), rewriter.new_alloca_ty, target_ty)
                || (int_bits(self.context(), rewriter.new_alloca_ty).is_some()
                    && int_bits(self.context(), target_ty).is_some()
                    && layout.store_size(&self.context().types, target_ty) > rewriter.slice_size
                    && !volatile))
        {
            let align = self.new_ai_align(rewriter);
            let name =
                self.function().instruction(load).result.and_then(|result| self.function().value(result).name.clone());
            value = self.build(
                load,
                Opcode::Load { align: Some(align), volatile },
                rewriter.new_alloca_ty,
                vec![Operand::Value(rewriter.new_ai_value)],
                name.as_deref(),
            );
            self.copy_metadata(load, value, rewriter.new_alloca_ty == load_ty);
            // An integer load past the end of the slice: the bytes outside are
            // undef or the load is dead.
            if let (Some(have), Some(want)) =
                (int_bits(self.context(), rewriter.new_alloca_ty), int_bits(self.context(), target_ty))
            {
                if have < want {
                    value = self.build(load, Opcode::Cast(CastOp::ZExt), target_ty, vec![value], Some("load.ext"));
                    if layout.big_endian {
                        let amount = self.int_constant(target_ty, i128::from(want - have));
                        value = self.build(
                            load,
                            Opcode::Binary(llrm_mir::BinaryOp::Shl),
                            target_ty,
                            vec![value, amount],
                            Some("endian_shift"),
                        );
                    }
                }
            }
        } else {
            let pointer = self.new_alloca_slice_ptr(rewriter, load);
            let align = self.slice_align(rewriter);
            let name =
                self.function().instruction(load).result.and_then(|result| self.function().value(result).name.clone());
            value = self.build(
                load,
                Opcode::Load { align: Some(align), volatile },
                target_ty,
                vec![pointer],
                name.as_deref(),
            );
            self.copy_metadata(load, value, target_ty == load_ty && rewriter.new_begin == rewriter.begin);
            ptr_adjusted = true;
        }
        value = self.bit_preserving_cast_chain(load, value, target_ty);
        let result = self.function().instruction(load).result.expect("a load has a result");
        if rewriter.is_split {
            // Insert just past the load, into a placeholder of the load's type
            // that then stands for the load.
            let after = self.next_instruction(load);
            let poison_ptr = {
                let ptr_ty = self.function().operand_type(self.context(), old_op).expect("a pointer");
                self.poison_of(ptr_ty)
            };
            let placeholder = self
                .unit
                .function
                .create_instruction(
                    Opcode::Load { align: Some(1), volatile: false },
                    load_ty,
                    vec![poison_ptr],
                    Flags::default(),
                    None,
                );
            self.unit.function.insert(placeholder, Position::Before(after)).expect("a placed instruction");
            let placeholder_value = Operand::Value(self.function().instruction(placeholder).result.expect("a result"));
            let inserted =
                self.insert_integer(after, placeholder_value, value, rewriter.new_begin - rewriter.begin, "insert");
            self.unit.function.replace_value(result, inserted);
            self.unit
                .function
                .replace_value(
                    self.function().instruction(placeholder).result.expect("a result"),
                    Operand::Value(result),
                );
            self.unit.function.erase(placeholder).expect("no uses");
        } else {
            self.unit.function.replace_value(result, value);
        }
        self.dead_insts.push(load);
        self.delete_if_trivially_dead(old_op);
        !volatile && !ptr_adjusted
    }

    /// The instruction after `inst` in its block.
    /// `std::next(LI.getIterator())`, SROA.cpp#L3573
    fn next_instruction(
        &self,
        inst: InstId,
    ) -> InstId {
        self.function().next_in_block(inst).expect("a terminator follows")
    }

    /// `AllocaSliceRewriter::rewriteIntegerStore`. SROA.cpp#L3639
    fn rewrite_integer_store(
        &mut self,
        rewriter: &Rewriter,
        store: InstId,
        value: Operand,
    ) -> bool {
        let int_ty = rewriter.int_ty.expect("an integer partition");
        let int_width = int_bits(self.context(), int_ty).expect("an integer");
        let align = self.new_ai_align(rewriter);
        let mut value = value;
        let value_ty = self.type_of(value);
        if self.unit.layout.size_bits(&self.context().types, value_ty) != u64::from(int_width) {
            let old = self.build(
                store,
                Opcode::Load { align: Some(align), volatile: false },
                rewriter.new_alloca_ty,
                vec![Operand::Value(rewriter.new_ai_value)],
                Some("oldload"),
            );
            let old = self.bit_preserving_cast_chain(store, old, int_ty);
            let offset = rewriter.begin - rewriter.new_alloca_begin;
            let stored = self.function().instruction(store).operands[0];
            value = self.insert_integer(store, old, stored, offset, "insert");
        }
        value = self.bit_preserving_cast_chain(store, value, rewriter.new_alloca_ty);
        let void = self.unit.context.types.void();
        let created = self.build(
            store,
            Opcode::Store { align: Some(align), volatile: false },
            void,
            vec![value, Operand::Value(rewriter.new_ai_value)],
            None,
        );
        let _ = created;
        self.dead_insts.push(store);
        true
    }

    /// `AllocaSliceRewriter::visitStoreInst`. SROA.cpp#L3668
    fn visit_store_inst(
        &mut self,
        rewriter: &mut Rewriter,
        store: InstId,
    ) -> bool {
        let layout = self.unit.layout;
        let old_op = self.function().instruction(store).operands[1];
        let volatile = matches!(
            self.function().instruction(store).opcode,
            Opcode::Store { volatile: true, .. }
        );
        let mut value = self.function().instruction(store).operands[0];
        let mut value_ty = self.type_of(value);
        // A pointer stored may reach an alloca again once this one is promoted.
        if is_pointer(self.context(), value_ty) {
            if let Some(root) = self.strip_in_bounds_offsets(value) {
                self.post_promotion_worklist.push(root);
            }
        }
        let store_size = layout.store_size(&self.context().types, value_ty);
        if rewriter.slice_size < store_size {
            let narrow = self.unit.context.types.int((rewriter.slice_size * 8) as u32);
            value = self.extract_integer(store, value, narrow, rewriter.new_begin - rewriter.begin, "extract");
            value_ty = self.type_of(value);
        }
        if rewriter.int_ty.is_some() && int_bits(self.context(), value_ty).is_some() {
            return self.rewrite_integer_store(rewriter, store, value);
        }
        let void = self.unit.context.types.void();
        let whole = rewriter.new_begin == rewriter.new_alloca_begin
            && rewriter.new_end == rewriter.new_alloca_end
            && can_convert_value(layout, self.context(), value_ty, rewriter.new_alloca_ty);
        let (new_pointer, align, stored) = if whole {
            let converted = self.bit_preserving_cast_chain(store, value, rewriter.new_alloca_ty);
            (Operand::Value(rewriter.new_ai_value), self.new_ai_align(rewriter), converted)
        } else {
            let pointer = self.new_alloca_slice_ptr(rewriter, store);
            (pointer, self.slice_align(rewriter), value)
        };
        let created =
            self.build(store, Opcode::Store { align: Some(align), volatile }, void, vec![stored, new_pointer], None);
        let _ = created;
        self.dead_insts.push(store);
        self.delete_if_trivially_dead(old_op);
        new_pointer == Operand::Value(rewriter.new_ai_value)
            && self.type_of(stored) == rewriter.new_alloca_ty
            && !volatile
    }

    /// `getAdjustedPtr`: `ptr` advanced by `offset` bytes, in bounds.
    /// getAdjustedPtr. SROA.cpp#L2036
    fn adjusted_ptr(
        &mut self,
        at: InstId,
        ptr: Operand,
        offset: u64,
    ) -> Operand {
        if offset == 0 {
            return ptr;
        }
        let ptr_ty = self.type_of(ptr);
        let space = pointer_space(self.context(), ptr_ty);
        let index_ty = self.unit.context.types.int(self.unit.layout.pointer(space).index_bits);
        let byte = self.unit.context.types.int(8);
        let index = self.int_constant(index_ty, offset as i128);
        let created = self
            .unit
            .function
            .create_instruction(
                Opcode::GetElementPtr { source: byte },
                ptr_ty,
                vec![ptr, index],
                Flags::INBOUNDS,
                Some("sroa_idx"),
            );
        self.unit.function.insert(created, Position::Before(at)).expect("a placed instruction");
        Operand::Value(self.function().instruction(created).result.expect("a pointer"))
    }

    /// `getIntegerSplat`: an i8 spread over `size` bytes.
    /// AllocaSliceRewriter::getIntegerSplat. SROA.cpp#L3747
    fn get_integer_splat(
        &mut self,
        at: InstId,
        byte: Operand,
        size: u64,
    ) -> Operand {
        if size == 1 {
            return byte;
        }
        let splat_ty = self.unit.context.types.int((size * 8) as u32);
        // `IRBuilder` folds a constant byte.
        if let (Operand::Constant(id), true) = (byte, size <= 16) {
            if let ConstantKind::Int(bits) = self.context().get(id).kind {
                let mut value: u128 = 0;
                for _ in 0..size {
                    value = (value << 8) | (bits & 0xff);
                }
                return self.int_constant(splat_ty, value as i128);
            }
        }
        let extended = self.build(at, Opcode::Cast(CastOp::ZExt), splat_ty, vec![byte], Some("zext"));
        let all = self.int_constant(splat_ty, -1);
        let byte_ty = self.type_of(byte);
        let byte_ones = self.int_constant(byte_ty, -1);
        let ones = self.build(at, Opcode::Cast(CastOp::ZExt), splat_ty, vec![byte_ones], None);
        let unit = self.build(at, Opcode::Binary(llrm_mir::BinaryOp::UDiv), splat_ty, vec![all, ones], None);
        self.build(at, Opcode::Binary(llrm_mir::BinaryOp::Mul), splat_ty, vec![extended, unit], Some("isplat"))
    }

    /// A call like `call`, with other operands.
    /// The `IRB.CreateMemSet`, `CreateMemCpy`, `CreateLifetimeStart` of
    /// SROA.cpp#L3834, L4020, L4170.
    fn call_like(
        &mut self,
        call: InstId,
        operands: Vec<Operand>,
    ) -> InstId {
        let instruction = self.function().instruction(call);
        let (opcode, ty, flags) = (instruction.opcode.clone(), instruction.ty, instruction.flags);
        let created = self.unit.function.create_instruction(opcode, ty, operands, flags, None);
        self.unit.function.insert(created, Position::Before(call)).expect("a placed instruction");
        created
    }

    /// `AllocaSliceRewriter::visitMemSetInst`. SROA.cpp#L3771
    fn visit_mem_set_inst(
        &mut self,
        rewriter: &mut Rewriter,
        memset: InstId,
    ) -> bool {
        let layout = self.unit.layout;
        let operands = self.function().instruction(memset).operands.clone();
        let (dest, byte, length) = (operands[0], operands[1], operands[2]);
        let volatile = constant_unsigned(self.context(), operands[3]) != Some(0);
        // A memset of variable size cannot be split: only its pointer moves.
        let Some(len) = constant_unsigned(self.context(), length) else {
            let pointer = self.new_alloca_slice_ptr(rewriter, memset);
            self.unit.function.set_operand(memset, 0, pointer);
            self.delete_if_trivially_dead(dest);
            return false;
        };
        self.dead_insts.push(memset);
        let new_ty = rewriter.new_alloca_ty;
        let can_continue = rewriter.int_ty.is_some() || {
            if rewriter.begin > rewriter.new_alloca_begin || rewriter.end < rewriter.new_alloca_end {
                false
            } else if len > u64::from(u32::MAX) {
                false
            } else {
                // `<len x i8>` converts as the integer of its width.
                let source = self.unit.context.types.int((len * 8) as u32);
                can_convert_value(layout, self.context(), source, new_ty)
                    && layout.legal_integer(layout.size_bits(&self.context().types, new_ty) as u32)
            }
        };
        // Not representable as one value: a memset of the slice.
        if !can_continue {
            let pointer = self.new_alloca_slice_ptr(rewriter, memset);
            let size = self.int_constant(self.type_of(length), (rewriter.new_end - rewriter.new_begin) as i128);
            let mut new_operands = operands.clone();
            new_operands[0] = pointer;
            new_operands[2] = size;
            self.call_like(memset, new_operands);
            return false;
        }
        let value = if let Some(int_ty) = rewriter.int_ty {
            let size = rewriter.new_end - rewriter.new_begin;
            let mut value = self.get_integer_splat(memset, byte, size);
            if rewriter.new_begin != rewriter.new_alloca_begin || rewriter.new_end != rewriter.new_alloca_end {
                let align = self.new_ai_align(rewriter);
                let old = self.build(
                    memset,
                    Opcode::Load { align: Some(align), volatile: false },
                    new_ty,
                    vec![Operand::Value(rewriter.new_ai_value)],
                    Some("oldload"),
                );
                let old = self.bit_preserving_cast_chain(memset, old, int_ty);
                let offset = rewriter.new_begin - rewriter.new_alloca_begin;
                value = self.insert_integer(memset, old, value, offset, "insert");
            }
            self.bit_preserving_cast_chain(memset, value, new_ty)
        } else {
            let bytes = layout.size_bits(&self.context().types, new_ty) / 8;
            let splat = self.get_integer_splat(memset, byte, bytes);
            self.bit_preserving_cast_chain(memset, splat, new_ty)
        };
        let void = self.unit.context.types.void();
        let align = self.new_ai_align(rewriter);
        self.build(
            memset,
            Opcode::Store { align: Some(align), volatile },
            void,
            vec![value, Operand::Value(rewriter.new_ai_value)],
            None,
        );
        !volatile
    }

    /// `AllocaSliceRewriter::visitMemTransferInst`. SROA.cpp#L3911
    fn visit_mem_transfer_inst(
        &mut self,
        rewriter: &mut Rewriter,
        transfer: InstId,
    ) -> bool {
        let layout = self.unit.layout;
        let operands = self.function().instruction(transfer).operands.clone();
        let used = rewriter.old_use.expect("a slice's use");
        let is_dest = used.index == 0;
        let volatile = constant_unsigned(self.context(), operands[3]) != Some(0);
        // An unsplit transfer only has its pointer moved: it may be within one
        // alloca, of variable length, or a memmove.
        if !rewriter.is_splittable {
            let adjusted = self.new_alloca_slice_ptr(rewriter, transfer);
            self.unit.function.set_operand(transfer, used.index as usize, adjusted);
            self.delete_if_trivially_dead(rewriter.old_ptr.expect("a slice's pointer"));
            return false;
        }
        // A split transfer: the two sides are in different allocas, one of
        // which does not escape, so a memmove is a memcpy.
        let new_ty = rewriter.new_alloca_ty;
        let emit_memcpy = rewriter.int_ty.is_none()
            && (rewriter.begin > rewriter.new_alloca_begin
                || rewriter.end < rewriter.new_alloca_end
                || rewriter.slice_size != layout.store_size(&self.context().types, new_ty)
                || !size_equals_store_size(layout, self.context(), new_ty)
                || !is_single_value(self.context(), new_ty));
        // The alloca unchanged and the size not shrunk: nothing to do.
        if emit_memcpy && rewriter.old_ai == rewriter.new_ai {
            if rewriter.new_end != rewriter.end {
                let size =
                    self.int_constant(self.type_of(operands[2]), (rewriter.new_end - rewriter.new_begin) as i128);
                self.unit.function.set_operand(transfer, 2, size);
            }
            return false;
        }
        self.dead_insts.push(transfer);
        // The other side's root alloca is looked at again once this is
        // rewritten.
        let other = if is_dest { operands[1] } else { operands[0] };
        if let Some(root) = self.strip_in_bounds_offsets(other) {
            self.worklist.push(root);
        }
        let other_offset = rewriter.new_begin - rewriter.begin;
        if emit_memcpy {
            let other_ptr = self.adjusted_ptr(transfer, other, other_offset);
            let our = self.new_alloca_slice_ptr(rewriter, transfer);
            let size = self.int_constant(self.type_of(operands[2]), (rewriter.new_end - rewriter.new_begin) as i128);
            let (destination, source) = if is_dest { (our, other_ptr) } else { (other_ptr, our) };
            let mut new_operands = operands.clone();
            // `IRB.CreateMemCpy`: a memcpy even where it was a memmove is not
            // made here: the callee stays, DEVIATE.
            new_operands[0] = destination;
            new_operands[1] = source;
            new_operands[2] = size;
            self.call_like(transfer, new_operands);
            return false;
        }
        let whole = rewriter.new_begin == rewriter.new_alloca_begin && rewriter.new_end == rewriter.new_alloca_end;
        let size = rewriter.new_end - rewriter.new_begin;
        let sub_int_ty = rewriter.int_ty.map(|_| self.unit.context.types.int((size * 8) as u32));
        let other_ty = match (rewriter.int_ty, whole) {
            (Some(_), false) => sub_int_ty.expect("an integer"),
            _ => new_ty,
        };
        let adjusted = self.adjusted_ptr(transfer, other, other_offset);
        let (destination, source) = if is_dest {
            (Operand::Value(rewriter.new_ai_value), adjusted)
        } else {
            (adjusted, Operand::Value(rewriter.new_ai_value))
        };
        let new_align = self.new_ai_align(rewriter);
        let mut value;
        if let (Some(int_ty), false, false) = (rewriter.int_ty, whole, is_dest) {
            value = self.build(
                transfer,
                Opcode::Load { align: Some(new_align), volatile: false },
                new_ty,
                vec![Operand::Value(rewriter.new_ai_value)],
                Some("load"),
            );
            value = self.bit_preserving_cast_chain(transfer, value, int_ty);
            let offset = rewriter.new_begin - rewriter.new_alloca_begin;
            value = self.extract_integer(transfer, value, sub_int_ty.expect("an integer"), offset, "extract");
        } else {
            value = self.build(
                transfer,
                Opcode::Load { align: Some(1), volatile },
                other_ty,
                vec![source],
                Some("copyload"),
            );
        }
        if let (Some(int_ty), false, true) = (rewriter.int_ty, whole, is_dest) {
            let old = self.build(
                transfer,
                Opcode::Load { align: Some(new_align), volatile: false },
                new_ty,
                vec![Operand::Value(rewriter.new_ai_value)],
                Some("oldload"),
            );
            let old = self.bit_preserving_cast_chain(transfer, old, int_ty);
            let offset = rewriter.new_begin - rewriter.new_alloca_begin;
            value = self.insert_integer(transfer, old, value, offset, "insert");
            value = self.bit_preserving_cast_chain(transfer, value, new_ty);
        }
        let void = self.unit.context.types.void();
        self.build(transfer, Opcode::Store { align: Some(1), volatile }, void, vec![value, destination], None);
        !volatile
    }

    /// `AllocaSliceRewriter::visitIntrinsicInst`: a lifetime marker.
    /// AllocaSliceRewriter::visitIntrinsicInst. SROA.cpp#L4145
    fn visit_intrinsic_inst(
        &mut self,
        rewriter: &mut Rewriter,
        marker: InstId,
    ) -> bool {
        self.dead_insts.push(marker);
        let operands = self.function().instruction(marker).operands.clone();
        let pointer = self.new_alloca_slice_ptr(rewriter, marker);
        // DEVIATE: MIR's marker names the size, which LLVM's has dropped.
        let size = self.int_constant(self.type_of(operands[0]), (rewriter.new_end - rewriter.new_begin) as i128);
        let mut new_operands = operands;
        new_operands[0] = size;
        new_operands[1] = pointer;
        self.call_like(marker, new_operands);
        true
    }

    /// Where a pointer made for a phi or select goes: at the old pointer,
    /// which dominates it (after the phis of its block, where it is one).
    /// AllocaSliceRewriter::visitPHINode. SROA.cpp#L4204
    fn place_for_old_pointer(
        &self,
        old: Operand,
    ) -> InstId {
        let Operand::Value(value) = old else { unreachable!("a pointer value") };
        let llrm_mir::module::ValueDef::Instruction(def) = self.function().value(value).def else {
            unreachable!("an instruction")
        };
        if self.function().instruction(def).opcode == Opcode::Phi {
            let block = self.function().parent(def).expect("a placed phi");
            return self
                .function()
                .iter_block(block)
                .find(|&one| self.function().instruction(one).opcode != Opcode::Phi)
                .expect("a block ends in a terminator");
        }
        def
    }

    /// `AllocaSliceRewriter::visitPHINode`. SROA.cpp#L4204
    fn visit_phi_node(
        &mut self,
        rewriter: &mut Rewriter,
        phi: InstId,
    ) -> bool {
        let old = rewriter.old_ptr.expect("a slice's pointer");
        // The new pointer is made once, at the old one, which dominates the
        // phi.
        let at = self.place_for_old_pointer(old);
        let new_ptr = self.new_alloca_slice_ptr(rewriter, at);
        // Replace the operands which were using the old pointer.
        let operands = self.function().instruction(phi).operands.clone();
        for (index, operand) in operands.iter().enumerate() {
            if index % 2 == 0 && *operand == old {
                self.unit.function.set_operand(phi, index, new_ptr);
            }
        }
        self.delete_if_trivially_dead(old);
        // DEVIATE: `fixLoadStoreAlign` (L4180): a load's alignment cannot be
        // rewritten in place in MIR.
        // PHIs can't be promoted on their own, but often can be speculated:
        // checked outside the rewriter, on the fully rewritten alloca.
        if !rewriter.phi_users.contains(&phi) {
            rewriter.phi_users.push(phi);
        }
        true
    }

    /// `AllocaSliceRewriter::visitSelectInst`. SROA.cpp#L4238
    fn visit_select_inst(
        &mut self,
        rewriter: &mut Rewriter,
        select: InstId,
    ) -> bool {
        let old = rewriter.old_ptr.expect("a slice's pointer");
        let new_ptr = self.new_alloca_slice_ptr(rewriter, select);
        // Replace the operands which were using the old pointer.
        for index in [1, 2] {
            if self.function().instruction(select).operands[index] == old {
                self.unit.function.set_operand(select, index, new_ptr);
            }
        }
        self.delete_if_trivially_dead(old);
        if !rewriter.select_users.contains(&select) {
            rewriter.select_users.push(select);
        }
        true
    }

    /// Whether a load is simple (not volatile).
    fn is_simple_load(
        &self,
        inst: InstId,
    ) -> bool {
        matches!(
            self.function().instruction(inst).opcode,
            Opcode::Load { volatile: false, .. }
        )
    }

    /// The number of edges leaving `block`.
    fn successor_edges(
        &self,
        block: llrm_mir::module::BlockId,
    ) -> usize {
        let Some(terminator) = self.function().terminator(block) else { return 0 };
        self.function().instruction(terminator).operands.iter().filter(|one| matches!(one, Operand::Block(_))).count()
    }

    /// `isSafeToLoadUnconditionally` for a load of `bytes` from `pointer`.
    /// DEVIATE: dereferenceable alone (an alloca or a sized argument, not a
    /// global: no alignment, no "a load already dominates" proof).
    fn safe_to_load_unconditionally(
        &self,
        pointer: Operand,
        bytes: u64,
    ) -> bool {
        llrm_mir::valuetracking::dereferenceable(
            self.context(),
            self.unit.layout,
            &llrm_mir::valuetracking::Sizes::default(),
            self.function(),
            pointer,
            bytes,
        )
    }

    /// `isSafePHIToSpeculate`. SROA.cpp#L1583
    fn is_safe_phi_to_speculate(
        &self,
        phi: InstId,
    ) -> bool {
        let function = self.function();
        let Some(result) = function.instruction(phi).result else { return false };
        let block = function.parent(phi).expect("a placed phi");
        let mut load_type: Option<TypeId> = None;
        let users: Vec<Use> = function.users(result).snapshot();
        for user in &users {
            let load = user.user;
            if !self.is_simple_load(load) || function.parent(load) != Some(block) {
                return false;
            }
            let ty = function.instruction(load).ty;
            if load_type.is_some_and(|one| one != ty) {
                return false;
            }
            load_type = Some(ty);
            // Nothing between the phi and the load may store.
            if !function.comes_before(phi, load) {
                return false;
            }
            let mut walk = Some(phi);
            while let Some(between) = walk.filter(|&one| one != load) {
                walk = function.next_in_block(between);
                let writes = match function.instruction(between).opcode {
                    Opcode::Store { .. } | Opcode::Call(_) | Opcode::Invoke(_) | Opcode::Resume => true,
                    Opcode::Load { volatile, .. } => volatile,
                    _ => false,
                };
                if writes {
                    return false;
                }
            }
        }
        let Some(load_type) = load_type else { return false };
        let load_size = self.unit.layout.store_size(&self.context().types, load_type);
        // The loads go into the predecessors: no critical edge may take a
        // possibly trapping one.
        for pair in function.instruction(phi).operands.chunks(2) {
            let Operand::Block(pred) = pair[1] else { return false };
            let Some(terminator) = function.terminator(pred) else { return false };
            if matches!(function.instruction(terminator).opcode, Opcode::Invoke(_)) {
                return false;
            }
            if self.successor_edges(pred) == 1 {
                continue;
            }
            if self.safe_to_load_unconditionally(pair[0], load_size) {
                continue;
            }
            return false;
        }
        true
    }

    /// Whether `value` is available at the end of `block`.
    fn available_at_end_of(
        &self,
        value: Operand,
        block: llrm_mir::module::BlockId,
    ) -> bool {
        let Operand::Value(id) = value else { return true };
        match self.function().value(id).def {
            llrm_mir::module::ValueDef::Instruction(def) => {
                let defined = self.function().parent(def).expect("a placed definition");
                defined == block
                    || llrm_analysis::cfg::Dominance::of(self.function()).dominates(
                        self.function(),
                        llrm_analysis::cfg::id(defined),
                        llrm_analysis::cfg::id(block),
                    )
            }
            _ => true,
        }
    }

    /// `getPHIStoreToRewrite` (preserving the CFG: every predecessor has the
    /// block as its only successor). SROA.cpp#L1659
    fn get_phi_store_to_rewrite(
        &self,
        phi: InstId,
    ) -> Option<InstId> {
        let function = self.function();
        let result = function.instruction(phi).result?;
        let users = function.users(result);
        let only = users.only()?;
        let store = only.user;
        let Opcode::Store { volatile: false, .. } = function.instruction(store).opcode else { return None };
        if only.index != 1 {
            return None;
        }
        let block = function.parent(phi).expect("a placed phi");
        let first = function.iter_block(block).find(|&one| function.instruction(one).opcode != Opcode::Phi);
        if first != Some(store) {
            return None;
        }
        let stored = function.instruction(store).operands[0];
        let mut seen: Vec<llrm_mir::module::BlockId> = Vec::new();
        for pair in function.instruction(phi).operands.chunks(2) {
            let Operand::Block(pred) = pair[1] else { return None };
            if seen.contains(&pred) {
                continue;
            }
            seen.push(pred);
            if pred == block || self.successor_edges(pred) != 1 {
                return None;
            }
            if !self.available_at_end_of(stored, pred) {
                return None;
            }
        }
        Some(store)
    }

    /// `speculatePHINodeLoads`. SROA.cpp#L1712
    fn speculate_phi_node_loads(
        &mut self,
        phi: InstId,
    ) {
        let result = self.function().instruction(phi).result.expect("a phi");
        let users: Vec<InstId> = self.function().users(result).iter().map(|one| one.user).collect();
        let some_load = *users.last().expect("a phi with loads");
        let load_ty = self.function().instruction(some_load).ty;
        let Opcode::Load { align, .. } = self.function().instruction(some_load).opcode else { unreachable!("a load") };
        let name = self.function().value(result).name.clone().map(|one| format!("{one}.sroa.speculated"));
        let new_phi =
            self.unit.function.create_instruction(Opcode::Phi, load_ty, Vec::new(), Flags::default(), name.as_deref());
        self.unit.function.insert(new_phi, Position::Before(phi)).expect("a placed phi");
        let new_value = Operand::Value(self.function().instruction(new_phi).result.expect("a phi"));
        // Every load of the phi reads the new one.
        for load in users {
            let loaded = self.function().instruction(load).result.expect("a load");
            self.unit.function.replace_value(loaded, new_value);
            self.unit.function.erase(load).expect("its uses were replaced");
        }
        // Loads in all of the predecessors; one for duplicate edges.
        let mut injected: Vec<(llrm_mir::module::BlockId, Operand)> = Vec::new();
        let mut incoming: Vec<Operand> = Vec::new();
        let pairs: Vec<Operand> = self.function().instruction(phi).operands.clone();
        for pair in pairs.chunks(2) {
            let Operand::Block(pred) = pair[1] else { unreachable!("a phi's block") };
            if let Some(&(_, load)) = injected.iter().find(|(block, _)| *block == pred) {
                incoming.extend([load, Operand::Block(pred)]);
                continue;
            }
            let terminator = self.function().terminator(pred).expect("a terminated block");
            let load = self.build(
                terminator,
                Opcode::Load { align, volatile: false },
                load_ty,
                vec![pair[0]],
                Some("sroa.speculate.load"),
            );
            incoming.extend([load, Operand::Block(pred)]);
            injected.push((pred, load));
        }
        self.unit.function.set_operands(new_phi.to_owned(), incoming);
        self.unit.function.erase(phi).expect("its loads were replaced");
    }

    /// `rewritePHINodeStore` for a phi whose predecessors have no other
    /// successor. SROA.cpp#L1767
    fn rewrite_phi_node_store(
        &mut self,
        phi: InstId,
    ) {
        let result = self.function().instruction(phi).result.expect("a phi");
        let store = self.function().users(result).first().expect("the phi's store").user;
        let pairs: Vec<Operand> = self.function().instruction(phi).operands.clone();
        let mut seen: Vec<llrm_mir::module::BlockId> = Vec::new();
        for pair in pairs.chunks(2) {
            let Operand::Block(pred) = pair[1] else { unreachable!("a phi's block") };
            if seen.contains(&pred) {
                continue;
            }
            seen.push(pred);
            // Revisit the alloca exposed by removing the pointer phi.
            if let Some(root) = self.strip_in_bounds_offsets(pair[0]) {
                self.worklist.push(root);
            }
            let terminator = self.function().terminator(pred).expect("a terminated block");
            let mut operands = self.function().instruction(store).operands.clone();
            operands[1] = pair[0];
            let opcode = self.function().instruction(store).opcode.clone();
            let void = self.unit.context.types.void();
            self.build(terminator, opcode, void, operands, None);
        }
        self.unit.function.erase(store).expect("a store");
        self.unit.function.erase(phi).expect("its store is gone");
    }

    /// `isSafeSelectToSpeculate` (preserving the CFG: simple loads whose both
    /// pointers are safe to load). SROA.cpp#L1854
    fn is_safe_select_to_speculate(
        &self,
        select: InstId,
    ) -> Option<Vec<InstId>> {
        let function = self.function();
        let result = function.instruction(select).result?;
        let mut loads = Vec::new();
        for user in function.users(result) {
            if !self.is_simple_load(user.user) {
                return None;
            }
            let ty = function.instruction(user.user).ty;
            let size = self.unit.layout.store_size(&self.context().types, ty);
            let operands = &function.instruction(select).operands;
            if !self.safe_to_load_unconditionally(operands[1], size)
                || !self.safe_to_load_unconditionally(operands[2], size)
            {
                return None;
            }
            loads.push(user.user);
        }
        Some(loads)
    }

    /// `rewriteSelectInstMemOps` with every load speculatable: each load of the
    /// select is a select of two loads. SROA.cpp#L2002, L1897
    fn rewrite_select_inst_mem_ops(
        &mut self,
        select: InstId,
        loads: &[InstId],
    ) {
        let operands = self.function().instruction(select).operands.clone();
        for &load in loads {
            let ty = self.function().instruction(load).ty;
            let Opcode::Load { align, .. } = self.function().instruction(load).opcode else { unreachable!("a load") };
            let true_load = self.build(
                load,
                Opcode::Load { align, volatile: false },
                ty,
                vec![operands[1]],
                Some("sroa.speculate.load.true"),
            );
            let false_load = self.build(
                load,
                Opcode::Load { align, volatile: false },
                ty,
                vec![operands[2]],
                Some("sroa.speculate.load.false"),
            );
            let chosen =
                self.build(load, Opcode::Select, ty, vec![operands[0], true_load, false_load], Some("sroa.speculated"));
            let loaded = self.function().instruction(load).result.expect("a load");
            self.unit.function.replace_value(loaded, chosen);
            self.unit.function.erase(load).expect("its uses were replaced");
        }
        // The select goes with the pointers it chose between.
        let result = self.function().instruction(select).result.expect("a select");
        for user in self.function().users(result).iter().map(|one| one.user).collect::<Vec<_>>() {
            self.unit.function.erase(user).ok();
        }
        self.dead_insts.push(select);
    }

    /// `Value::stripInBoundsOffsets` to an alloca of the entry block.
    /// `Value::stripInBoundsOffsets` (Value.cpp), as SROA.cpp#L3679 uses it.
    fn strip_in_bounds_offsets(
        &self,
        operand: Operand,
    ) -> Option<InstId> {
        let mut current = operand;
        loop {
            let Operand::Value(value) = current else { return None };
            let llrm_mir::module::ValueDef::Instruction(def) = self.function().value(value).def else { return None };
            let instruction = self.function().instruction(def);
            match &instruction.opcode {
                Opcode::Alloca { .. } => return Some(def),
                Opcode::GetElementPtr { .. } if instruction.flags.contains(Flags::INBOUNDS) => {
                    current = instruction.operands[0];
                }
                Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) => current = instruction.operands[0],
                _ => return None,
            }
        }
    }

    // ---- aggregate loads and stores --------------------------------------

    /// `AggLoadStoreRewriter::rewrite`. SROA.cpp#L4296
    fn agg_rewrite(
        &mut self,
        alloca: InstId,
    ) -> bool {
        let mut queue: Vec<Use> = Vec::new();
        let mut visited: llrm_mir::dense::IdSet<InstId> = Default::default();
        let root = self.function().instruction(alloca).result.expect("an alloca has a result");
        let enqueue = |function: &Function,
                       queue: &mut Vec<Use>,
                       visited: &mut llrm_mir::dense::IdSet<InstId>,
                       value: ValueId| {
            for &used in function.users(value) {
                if visited.insert(used.user) {
                    queue.push(used);
                }
            }
        };
        enqueue(self.unit.function, &mut queue, &mut visited, root);
        let mut changed = false;
        while let Some(used) = queue.pop() {
            if self.function().is_erased(used.user) {
                continue;
            }
            let instruction = self.function().instruction(used.user);
            match &instruction.opcode {
                Opcode::Load { volatile: false, .. } if is_aggregate(self.context(), instruction.ty) => {
                    self.split_aggregate_load(used.user, &mut visited);
                    changed = true;
                }
                Opcode::Store { volatile: false, .. } if used.index == 1 => {
                    let value_ty = self.type_of(instruction.operands[0]);
                    if is_aggregate(self.context(), value_ty) {
                        self.split_aggregate_store(used.user, &mut visited);
                        changed = true;
                    }
                }
                Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast)
                | Opcode::GetElementPtr { .. }
                | Opcode::Phi
                | Opcode::Select => {
                    if let Some(result) = instruction.result {
                        enqueue(self.unit.function, &mut queue, &mut visited, result);
                    }
                }
                _ => {}
            }
        }
        changed
    }

    /// The leaves of `ty` as (indices, type) in order.
    /// `OpSplitter::emitSplitOps` (L4370) walks the same leaves. SROA.cpp#L4370
    fn leaves(
        &self,
        ty: TypeId,
        indices: &mut Vec<u32>,
        out: &mut Vec<(Vec<u32>, TypeId)>,
    ) {
        if !is_aggregate(self.context(), ty) {
            out.push((indices.clone(), ty));
            return;
        }
        let count = match self.context().types.get(ty) {
            Type::Array { count, .. } => *count,
            _ => self.context().types.fields(ty).map_or(0, |fields| fields.len() as u64),
        };
        for index in 0..count {
            indices.push(index as u32);
            let member = self.context().types.member(ty, index).expect("a member");
            self.leaves(member, indices, out);
            indices.pop();
        }
    }

    /// The pointer to the leaf at `indices` of `base_ty` through `pointer`, and
    /// its byte offset.
    /// `OpSplitter` GEPIndices (L4349) and `emitFunc`. SROA.cpp#L4429
    fn leaf_pointer(
        &mut self,
        at: InstId,
        pointer: Operand,
        base_ty: TypeId,
        indices: &[u32],
        name: &str,
    ) -> (Operand, u64) {
        let i32_ty = self.unit.context.types.int(32);
        let mut operands = vec![pointer, self.int_constant(i32_ty, 0)];
        let mut constants: Vec<Option<i128>> = vec![Some(0)];
        for &index in indices {
            operands.push(self.int_constant(i32_ty, i128::from(index)));
            constants.push(Some(i128::from(index)));
        }
        let (offset, _) = self.unit.layout.collect_offset(&self.context().types, base_ty, &constants);
        let ptr_ty = self.type_of(pointer);
        let created = self
            .unit
            .function
            .create_instruction(
                Opcode::GetElementPtr { source: base_ty },
                ptr_ty,
                operands,
                Flags::INBOUNDS,
                Some(name),
            );
        self.unit.function.insert(created, Position::Before(at)).expect("a placed instruction");
        (Operand::Value(self.function().instruction(created).result.expect("a pointer")), offset as u64)
    }

    /// `visitLoadInst` of the aggregate rewriter with `LoadOpSplitter`.
    /// SROA.cpp#L4471, L4411
    fn split_aggregate_load(
        &mut self,
        load: InstId,
        visited: &mut llrm_mir::dense::IdSet<InstId>,
    ) {
        let layout = self.unit.layout;
        let ty = self.function().instruction(load).ty;
        let pointer = self.function().instruction(load).operands[0];
        let base_align = match self.function().instruction(load).opcode {
            Opcode::Load { align: Some(align), .. } => align,
            _ => layout.align(&self.context().types, ty),
        };
        let mut leaves = Vec::new();
        self.leaves(ty, &mut Vec::new(), &mut leaves);
        let mut aggregate = self.poison_of(ty);
        for (indices, leaf_ty) in leaves {
            let (address, offset) = self.leaf_pointer(load, pointer, ty, &indices, "fca.gep");
            let piece = self.build(
                load,
                Opcode::Load { align: Some(common_alignment(base_align, offset)), volatile: false },
                leaf_ty,
                vec![address],
                Some("fca.load"),
            );
            aggregate = self.build(load, Opcode::InsertValue(indices), ty, vec![aggregate, piece], Some("fca.insert"));
        }
        visited.remove(&load);
        let result = self.function().instruction(load).result.expect("a load has a result");
        self.unit.function.replace_value(result, aggregate);
        self.unit.function.erase(load).expect("its uses were replaced");
    }

    /// `visitStoreInst` of the aggregate rewriter with `StoreOpSplitter`.
    /// SROA.cpp#L4543, L4490
    fn split_aggregate_store(
        &mut self,
        store: InstId,
        visited: &mut llrm_mir::dense::IdSet<InstId>,
    ) {
        let layout = self.unit.layout;
        let value = self.function().instruction(store).operands[0];
        let pointer = self.function().instruction(store).operands[1];
        let ty = self.type_of(value);
        let base_align = match self.function().instruction(store).opcode {
            Opcode::Store { align: Some(align), .. } => align,
            _ => layout.align(&self.context().types, ty),
        };
        let void = self.unit.context.types.void();
        let mut leaves = Vec::new();
        self.leaves(ty, &mut Vec::new(), &mut leaves);
        for (indices, leaf_ty) in leaves {
            let piece =
                self.build(store, Opcode::ExtractValue(indices.clone()), leaf_ty, vec![value], Some("fca.extract"));
            let (address, offset) = self.leaf_pointer(store, pointer, ty, &indices, "fca.gep");
            self.build(
                store,
                Opcode::Store { align: Some(common_alignment(base_align, offset)), volatile: false },
                void,
                vec![piece, address],
                None,
            );
        }
        visited.remove(&store);
        self.unit.function.erase(store).expect("a store has no result");
    }
}

#[allow(dead_code)]
fn unused(_: HashMap<u8, u8>) {}

#[cfg(test)]
#[path = "sroa_tests.rs"]
mod tests;
