//! `sroa-real`: scalar replacement of aggregates as LLVM's SROA does it: the
//! uses of an alloca are cut into slices, the slices into partitions that no
//! access straddles, each partition becomes an alloca of its own (or the old
//! one), the accesses are rewritten onto it, and what is then only loaded and
//! stored whole is promoted to registers. A translation of `SROA.cpp` at
//! d1106deb71cc (step 2: loads, stores and aggregate loads and stores).
//!
//! DEVIATE, each with its reason (line numbers are SROA.cpp's):
//! - memset, memcpy and other intrinsics (`visitMemSetInst` L1177,
//!   `visitMemTransferInst` L1194, `visitIntrinsicInst` L1266, the rewriter's
//!   L3771-L4204), `presplitLoadsAndStores` (L4965): the next step. A call
//!   using the alloca escapes it, so those allocas stay.
//! - PHI and select of pointers (`visitPHINodeOrSelectInst` L1338, the
//!   speculation L1583-L2036, the rewriter's L4204-L4262): a use by a phi or
//!   select that has users aborts the alloca.
//! - Vectors (`isVectorPromotionViable` L2374, `rewriteVectorized*`
//!   L3451-L3637, `rewriteTreeStructuredMerge` L3002,
//!   `tryCanonicalizeStructToVector` L5460): the targets have no vector
//!   registers, and `AggregateToVector` is off at -O2.
//! - `propagateStoredValuesToLoads` (L6200), reached when a pointer only
//!   escapes read-only: such an alloca is left as it is.
//! - Calls and invokes escape a pointer whatever their attributes
//!   (`visitCallBase` L1402 looks at read-only, nocapture operands).
//! - Address-space casts abort; pointers in a space that is a selector and an
//!   offset are not converted to integers (`canConvertValue` L2057).
//! - Debug records (`migrateDebugInfo` L348, the migration in `splitAlloca`
//!   L6066): an alloca a `Declare` names is left alone.
//! - `unfoldGEPSelect`, `unfoldGEPPhi` (L4578, L4673): gep over select or phi.
//! - `AssumptionCache`, `DomTreeUpdater`, statistics, names that follow
//!   `.sroa.` suffix rules.
//! - Offsets are `i128` and do not wrap at the index width; an offset below
//!   zero is outside the alloca.
//! - `IRBuilder` here makes what it is told and does not fold constants
//!   (`ConstantFolder`); `fold` does that after.
//! - Metadata: an access keeps its metadata where the type and offset are
//!   unchanged, and loses it otherwise (`adjustForAccess`).
//! - `promoteAllocas` (L6414) runs [`crate::mem2reg::Mem2Reg`], which promotes
//!   every promotable alloca of the entry block, not only the ones listed.

use std::cmp::Ordering;

use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
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

    fn reads(&self) -> llrm_mir::footprint::Footprint {
        llrm_mir::footprint::Footprint::MEMORY | llrm_mir::footprint::Footprint::STRUCTURE
    }

    fn adds_memory_operations(&self) -> bool {
        true
    }

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

/// `commonAlignment(A, Offset)`: the alignment `offset` bytes into something
/// aligned by `align`.
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
}

impl Slice {
    /// `Slice::operator<` (L564): begin ascending, unsplittable before
    /// splittable, end descending.
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
}

/// The constant value of an index operand, signed.
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

/// `AllocaSlices::AllocaSlices` with `SliceBuilder` and `PtrUseVisitor`
/// (SROA.cpp#L1416, L1022; PtrUseVisitor.h `visitPtr`).
fn build_slices(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    alloca: InstId,
) -> AllocaSlices {
    let mut found = AllocaSlices::default();
    let root = function.instruction(alloca).result.expect("an alloca has a result");
    let Opcode::Alloca { allocated, .. } = function.instruction(alloca).opcode else { unreachable!("an alloca") };
    let alloc_size = layout.alloc_size(&context.types, allocated);
    let mut worklist: Vec<(Use, bool, i128)> = Vec::new();
    let mut visited_uses: HashSet<Use> = HashSet::default();
    let mut visited_dead: HashSet<InstId> = HashSet::default();
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
                          visited_dead: &mut HashSet<InstId>,
                          offset: i128,
                          size: u64,
                          splittable: bool| {
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
            found.slices.push(Slice { begin, end, used: Some(used), splittable });
        };
        let result_users = |instruction: &llrm_mir::module::Instruction| {
            instruction.result.map_or(0, |result| function.users(result).len())
        };
        match &instruction.opcode {
            // `visitBitCastInst` (L1084).
            Opcode::Cast(CastOp::BitCast) if used.index == 0 && is_pointer(context, instruction.ty) => {
                if result_users(instruction) == 0 {
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
                if result_users(instruction) == 0 {
                    mark_dead(&mut found);
                    continue;
                }
                let indices: Vec<Option<i128>> =
                    instruction.operands[1..].iter().map(|&one| constant_index(context, one)).collect();
                let (known, offset) = if known && indices.iter().all(Option::is_some) {
                    let (constant, variable) = layout.collect_offset(&context.types, *source, &indices);
                    if variable.is_empty() { (true, offset + constant) } else { (false, 0) }
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
                insert_use(&mut found, &mut visited_dead, offset, size, splittable);
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
                insert_use(&mut found, &mut visited_dead, offset, size, splittable);
            }
            // DEVIATE: `visitPHINodeOrSelectInst` (L1338) beyond the dead case.
            Opcode::Phi | Opcode::Select => {
                if result_users(instruction) == 0 {
                    mark_dead(&mut found);
                } else {
                    aborted = true;
                }
            }
            // DEVIATE: `visitCallBase` (L1402): every call escapes the pointer.
            Opcode::Call(_) | Opcode::Invoke(_) => {
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
    fn size(&self) -> u64 {
        self.end - self.begin
    }

    fn is_empty(&self) -> bool {
        self.first == self.last
    }
}

/// `AllocaSlices::partitions()` (L992) with `partition_iterator::advance`
/// (L837), each partition in turn.
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
fn element_containing_offset(
    offsets: &[u64],
    offset: u64,
) -> usize {
    offsets.partition_point(|&one| one <= offset).saturating_sub(1)
}

/// `getTypePartition`. SROA.cpp#L4832
fn get_type_partition(
    layout: &DataLayout,
    context: &mut Context,
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
    let begin = slices[first].begin;
    for slice in &slices[first..last] {
        let used = slice.used.expect("a live slice");
        if slice.begin != begin || slice.end != end_offset {
            continue;
        }
        let user = function.instruction(used.user);
        let user_ty = match user.opcode {
            Opcode::Load { .. } => Some(user.ty),
            Opcode::Store { .. } => function.operand_type(context, user.operands[0]),
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
    if rel_end > size {
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
        _ => return false,
    }
    true
}

/// `isIntegerWideningViable`. SROA.cpp#L2551
fn is_integer_widening_viable(
    layout: &DataLayout,
    context: &mut Context,
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

/// The state of a run: `SROA` (L174).
struct State<'a, 'b, 'c> {
    unit: &'a mut passes::Unit<'b>,
    analyses: &'c mut Analyses,
    worklist: Vec<InstId>,
    post_promotion_worklist: Vec<InstId>,
    promotable_allocas: Vec<InstId>,
    dead_insts: Vec<InstId>,
}

/// What `AllocaSliceRewriter` carries from slice to slice (L2811).
struct Rewriter {
    old_ai: InstId,
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
    old_use: Option<Use>,
    old_ptr: Option<Operand>,
}

impl<'a, 'b, 'c> State<'a, 'b, 'c> {
    fn new(
        unit: &'a mut passes::Unit<'b>,
        analyses: &'c mut Analyses,
    ) -> Self {
        Self {
            unit,
            analyses,
            worklist: Vec::new(),
            post_promotion_worklist: Vec::new(),
            promotable_allocas: Vec::new(),
            dead_insts: Vec::new(),
        }
    }

    fn function(&self) -> &Function {
        self.unit.function
    }

    fn context(&self) -> &Context {
        self.unit.context
    }

    /// The allocas a debug record names (DEVIATE: left alone).
    fn declared_allocas(&self) -> HashSet<InstId> {
        self.function()
            .debug_records()
            .iter()
            .filter_map(|one| match one.what {
                llrm_mir::DebugWhat::Declare(Operand::Value(value)) => Some(value),
                _ => None,
            })
            .filter_map(|value| match self.function().value(value).def {
                llrm_mir::module::ValueDef::Instruction(inst) => Some(inst),
                _ => None,
            })
            .collect()
    }

    /// `SROA::runSROA`. SROA.cpp#L6430
    fn run_sroa(&mut self) -> bool {
        let Some(entry) = self.function().entry() else { return false };
        let declared = self.declared_allocas();
        let allocas: Vec<InstId> = self
            .function()
            .block(entry)
            .instructions()
            .iter()
            .copied()
            .filter(|&inst| matches!(
                self.function().instruction(inst).opcode,
                Opcode::Alloca { .. }
            ))
            .filter(|inst| !declared.contains(inst))
            .collect();
        for inst in allocas {
            if !self.worklist.contains(&inst) {
                self.worklist.push(inst);
            }
        }
        let mut changed = false;
        loop {
            while let Some(alloca) = self.worklist.pop() {
                if self.function().is_erased(alloca) {
                    continue;
                }
                changed |= self.run_on_alloca(alloca);
                let mut deleted: HashSet<InstId> = HashSet::default();
                changed |= self.delete_dead_instructions(&mut deleted);
                if !deleted.is_empty() {
                    self.worklist.retain(|one| !deleted.contains(one));
                    self.post_promotion_worklist.retain(|one| !deleted.contains(one));
                    self.promotable_allocas.retain(|one| !deleted.contains(one));
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
        self.promotable_allocas.retain(|&one| !self.unit.function.is_erased(one));
        if self.promotable_allocas.is_empty() {
            return false;
        }
        self.promotable_allocas.clear();
        crate::mem2reg::Mem2Reg.run(self.unit, self.analyses);
        true
    }

    /// `isInstructionTriviallyDead`: nothing uses it and it does nothing else.
    fn is_trivially_dead(
        &self,
        inst: InstId,
    ) -> bool {
        let function = self.function();
        if function.is_erased(inst) {
            return false;
        }
        let instruction = function.instruction(inst);
        let unused = instruction.result.is_none_or(|result| function.users(result).is_empty());
        let pure = match &instruction.opcode {
            Opcode::Binary(_)
            | Opcode::Cast(_)
            | Opcode::ExtractValue(_)
            | Opcode::InsertValue(_)
            | Opcode::Alloca { .. }
            | Opcode::GetElementPtr { .. }
            | Opcode::ICmp(_)
            | Opcode::FCmp(_)
            | Opcode::Phi
            | Opcode::Select
            | Opcode::Freeze
            | Opcode::FNeg => true,
            Opcode::Load { volatile, .. } => !volatile,
            _ => false,
        };
        unused && pure
    }

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
        deleted_allocas: &mut HashSet<InstId>,
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
                deleted_allocas.insert(inst);
            }
            if let Some(result) = self.function().instruction(inst).result {
                if !self.function().users(result).is_empty() {
                    let ty = self.function().value(result).ty;
                    let poison = self.poison_of(ty);
                    self.unit.function.replace_value(result, poison);
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
        let mut slices = build_slices(self.context(), self.unit.layout, self.function(), alloca);
        if slices.escaped || slices.escaped_read_only {
            return changed;
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
        changed
    }

    /// `SROA::splitAlloca`. SROA.cpp#L5964
    fn split_alloca(
        &mut self,
        alloca: InstId,
        slices: &mut AllocaSlices,
    ) -> bool {
        let mut changed = false;
        // DEVIATE: no `presplitLoadsAndStores` (L4965).
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
        // Rewrite each partition.
        for (number, p) in partitions(&slices.slices).iter().enumerate() {
            if self.rewrite_partition(alloca, &slices.slices, p, number).is_some() {
                changed = true;
            }
        }
        changed
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
            old_ai: alloca,
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
            old_use: None,
            old_ptr: None,
        };
        let mut promotable = true;
        for &tail in &p.split_tails {
            promotable &= self.visit_slice(&mut rewriter, &slices[tail]);
        }
        for slice in &slices[p.first..p.last] {
            promotable &= self.visit_slice(&mut rewriter, slice);
        }
        if promotable {
            // Promote the alloca.
            if !self.promotable_allocas.contains(&new_ai) {
                self.promotable_allocas.push(new_ai);
            }
        } else {
            // Drop any post-promotion work items if promotion didn't happen.
            self.post_promotion_worklist.truncate(post_promotion_old_size);
            // We couldn't promote and we didn't create a new partition.
            if new_ai == alloca {
                return None;
            }
            if !self.worklist.contains(&new_ai) {
                self.worklist.push(new_ai);
            }
        }
        Some(new_ai)
    }

    // ---- the rewriter ----------------------------------------------------

    /// `AllocaSliceRewriter::visit` (L2906).
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
        rewriter.old_use = Some(used);
        rewriter.old_ptr = Some(self.function().instruction(used.user).operands[used.index as usize]);
        match self.function().instruction(used.user).opcode {
            Opcode::Load { .. } => self.visit_load_inst(rewriter, used.user),
            Opcode::Store { .. } => self.visit_store_inst(rewriter, used.user),
            _ => unreachable!("only loads and stores make slices in this step"),
        }
    }

    /// A new instruction before `at`, its result as an operand.
    fn build(
        &mut self,
        at: InstId,
        opcode: Opcode,
        ty: TypeId,
        operands: Vec<Operand>,
        name: Option<&str>,
    ) -> Operand {
        let created = self.unit.function.create_instruction(opcode, ty, operands, Flags::default(), name);
        self.unit.function.insert(created, Position::Before(at)).expect("a placed instruction");
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
    fn new_alloca_slice_ptr(
        &mut self,
        rewriter: &Rewriter,
        at: InstId,
    ) -> Operand {
        let offset = rewriter.new_begin - rewriter.new_alloca_begin;
        let base = Operand::Value(rewriter.new_ai_value);
        if offset == 0 {
            return base;
        }
        let ptr_ty = self.function().instruction(rewriter.new_ai).ty;
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
    }

    /// `getSliceAlign`. SROA.cpp#L3431
    fn slice_align(
        &self,
        rewriter: &Rewriter,
    ) -> u64 {
        common_alignment(self.new_ai_align(rewriter), rewriter.new_begin - rewriter.new_alloca_begin)
    }

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
    fn next_instruction(
        &self,
        inst: InstId,
    ) -> InstId {
        let block = self.function().parent(inst).expect("a placed instruction");
        let list = self.function().block(block).instructions();
        let at = list.iter().position(|&one| one == inst).expect("in its block");
        list[at + 1]
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
                if !self.post_promotion_worklist.contains(&root) {
                    self.post_promotion_worklist.push(root);
                }
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

    /// `Value::stripInBoundsOffsets` to an alloca of the entry block.
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
                Opcode::Cast(CastOp::BitCast) => current = instruction.operands[0],
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
        let mut visited: HashSet<InstId> = HashSet::default();
        let root = self.function().instruction(alloca).result.expect("an alloca has a result");
        let enqueue = |function: &Function, queue: &mut Vec<Use>, visited: &mut HashSet<InstId>, value: ValueId| {
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
                Opcode::Cast(CastOp::BitCast) | Opcode::GetElementPtr { .. } | Opcode::Phi | Opcode::Select => {
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
        visited: &mut HashSet<InstId>,
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
        visited: &mut HashSet<InstId>,
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
