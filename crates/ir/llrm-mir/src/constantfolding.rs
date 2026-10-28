//! Folding an instruction whose operands are constants, with the data layout:
//! the integer and pointer arms of LLVM's `ConstantFolding.cpp`.
//!
//! Each function translates one function of
//! [`ConstantFolding.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp)
//! at d1106deb71cc, in its order. The fold of an operation on two numbers is
//! [`constfold`](crate::constfold) (`ConstantFold.cpp`), called here, not
//! translated again.
//!
//! # How it works
//! A fold takes `&Context` (it interns constants), the data layout and the
//! module's globals (a load reads a global's initializer, a call names its
//! callee) and answers a constant or none. `ConstantExpr::get*` is
//! [`constant_expr_get_cast`] and [`constant_fold_get_element_ptr`]: the fold
//! of `constfold`, else the expression MIR can hold.
//!
//! DEVIATE, permanent (MIR lacks the construct; the fact is stated):
//! 1. Vectors: `FoldBitCast`'s vector arms (L189-L406),
//!    `ConstantFoldFixedVectorCall`, the vector intrinsics, `masked_load`
//!    (L1776-L1801); `is_constant` of a vector. MIR types vectors but has no
//!    lane operation and nothing produces one (#2201).
//! 2. `ptrtoaddr`: MIR has no `ptrtoaddr` opcode (#2202, #2204). Byte types and
//!    `inrange`: MIR has no byte type and its constant GEP has no `inrange`.
//! 3. Target intrinsics (AMDGCN, ARM, AArch64, Wasm, NVVM, x86): the MIR
//!    verifier rejects a `llvm.*` name `Intrinsic` does not enumerate, and it
//!    enumerates none of them.
//! 9. `llvm.lrint`/`llvm.llrint` of an integral constant fold to that integer
//!    (the pin's `opt -passes=instsimplify,instcombine` leaves `lrint(double
//!    1.0496e6)`): an integral value converts exactly in every rounding mode.
//!    QB's `CLNG` of a constant sum was 1,378 B at m16 -Os against main's 1,231
//!    (fpemu). Exit: none; the rest of `lrint` waits for the FP intrinsics
//!    (#2246).
//! 8. A GEP with an index that is not an integer constant (`ptrtoint (@g)`)
//!    stays an instruction: the backend lowers constant GEP offsets only
//!    (`globals::constant_offset`), LLVM's x86 ISel lowers any constant.
//!
//! OWED (MIR can represent it; each has an issue and its exit):
//! 5. The constant expressions `add`, `sub`, `xor`, `select`: `ConstantExpr`
//!    has GEP and cast only, so a fold that would make one (L1463) gives none,
//!    as does the `(sub 0, V)` arm (L1658). Exit: add the variants (#2262).
//! 6. `HasExternalState` pointers: the `pe` flag of a `p` datalayout spec is
//!    not parsed. Exit: parse it into `PointerSpec` (#2263).
//! 7. Floating point: `fneg` and the FP binary operations (L1138-L1159, L1574),
//!    `fcmp`, FP intrinsics and libcalls (#2246); an `x86_fp80` load, held as a
//!    double (#2251).
//!
//! `tools/llvmport/cfcompare.py` lists them against LLVM's answers.
//!
//! # Example
//! `load i32, ptr @g` with `@g = constant [2 x i32] [i32 1, i32 2]` and a
//! `getelementptr inbounds ([2 x i32], ptr @g, i64 0, i64 1)` operand folds
//! to `i32 2` ([`constant_fold_instruction`]).

use crate::constfold::{self, is_null_value, null_value};
use crate::context::{Constant, ConstantExpr, ConstantId, ConstantKind, Context, GlobalId, mask, signed};
use crate::datalayout::DataLayout;
use crate::fpclass::RoundingMode;
use crate::hash::HashMap;
use crate::knownbits::KnownBits;
use crate::module::{Function, GlobalKind, GlobalValue, InstId, Instruction, Linkage, Operand};
use crate::opcode::{Attribute, BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use crate::types::{FloatKind, Type, TypeId};
use crate::valuetracking::Query;
use crate::valuetracking::pattern::{IntrinsicId, intrinsic_id_of_name};

/// What a fold reads besides the context: the globals and the layout.
#[derive(Clone, Copy)]
pub struct Env<'a> {
    pub globals: &'a [GlobalValue],
    pub layout: &'a DataLayout,
}

/// `GEPNoWrapFlags`: inbounds (which implies nusw), nusw, nuw.
const IN_BOUNDS: u8 = 1;
const NUSW: u8 = 2;
const NUW: u8 = 4;

// ---- constants and types ------------------------------------------------

fn ty_of(
    context: &Context,
    c: ConstantId,
) -> TypeId {
    context.get(c).ty
}

/// `dyn_cast<ConstantInt>`: the bits and the width.
fn as_int(
    context: &Context,
    c: ConstantId,
) -> Option<(u128, u32)> {
    let constant = context.get(c);
    let width = context.types.int_bits(constant.ty)?;
    match constant.kind {
        ConstantKind::Int(bits) => Some((bits & mask(width), width)),
        ConstantKind::Null | ConstantKind::Zero => Some((0, width)),
        _ => None,
    }
}

/// `isa<UndefValue>`: poison is one.
fn is_undef(
    context: &Context,
    c: ConstantId,
) -> bool {
    matches!(context.get(c).kind, ConstantKind::Undef) || is_poison(context, c)
}

/// `isa<PoisonValue>`: LLVM makes an aggregate of poison one poison, and the
/// parser reads the array `[]` as a poison.
pub(crate) fn is_poison(
    context: &Context,
    c: ConstantId,
) -> bool {
    match &context.get(c).kind {
        ConstantKind::Poison => true,
        ConstantKind::Aggregate(members) if members.is_empty() => {
            matches!(context.types.get(context.get(c).ty), Type::Array { .. })
        }
        ConstantKind::Aggregate(members) => members.iter().all(|&one| is_poison(context, one)),
        _ => false,
    }
}

/// `Constant::isAllOnesValue`.
fn is_all_ones_value(
    context: &Context,
    c: ConstantId,
) -> bool {
    let constant = context.get(c);
    match (&constant.kind, context.types.get(constant.ty)) {
        (ConstantKind::Int(bits), Type::Int(width)) => *bits == mask(*width),
        (ConstantKind::Float(bits), Type::Float(FloatKind::Float)) => *bits == u64::from(u32::MAX),
        (ConstantKind::Float(bits), Type::Float(FloatKind::Double)) => *bits == u64::MAX,
        _ => false,
    }
}

fn int(
    context: &Context,
    width: u32,
    value: u128,
) -> ConstantId {
    let ty = context.types.int(width);
    context.constant(Constant { ty, kind: ConstantKind::Int(value & mask(width)) })
}

fn undef(
    context: &Context,
    ty: TypeId,
) -> ConstantId {
    context.constant(Constant { ty, kind: ConstantKind::Undef })
}

fn poison(
    context: &Context,
    ty: TypeId,
) -> ConstantId {
    context.constant(Constant { ty, kind: ConstantKind::Poison })
}

/// `Constant::getAllOnesValue` of an integer or floating type.
fn all_ones_value(
    context: &Context,
    ty: TypeId,
) -> Option<ConstantId> {
    let kind = match *context.types.get(ty) {
        Type::Int(width) => ConstantKind::Int(mask(width)),
        Type::Float(FloatKind::Float) => ConstantKind::Float(u64::from(u32::MAX)),
        Type::Float(FloatKind::Double) => ConstantKind::Float(u64::MAX),
        _ => return None,
    };
    Some(context.constant(Constant { ty, kind }))
}

/// `ConstantStruct::get`/`ConstantArray::get`: all-zero and all-poison
/// members make a `zeroinitializer` and a poison; all-undef, an undef.
fn aggregate(
    context: &Context,
    ty: TypeId,
    members: Vec<ConstantId>,
) -> ConstantId {
    let kind = if members.iter().all(|&one| is_null_value(context, one)) {
        ConstantKind::Zero
    } else if !members.is_empty() && members.iter().all(|&one| is_poison(context, one)) {
        ConstantKind::Poison
    } else if !members.is_empty() && members.iter().all(|&one| is_undef(context, one)) {
        ConstantKind::Undef
    } else {
        ConstantKind::Aggregate(members)
    };
    context.constant(Constant { ty, kind })
}

/// `Constant::getAggregateElement`.
fn aggregate_element(
    context: &Context,
    c: ConstantId,
    index: u64,
) -> Option<ConstantId> {
    let Constant { ty, kind } = context.get(c).clone();
    match kind {
        ConstantKind::Aggregate(members) => members.get(usize::try_from(index).ok()?).copied(),
        ConstantKind::Bytes(bytes) => {
            let byte = *bytes.get(usize::try_from(index).ok()?)?;
            Some(int(context, 8, u128::from(byte)))
        }
        ConstantKind::Zero => {
            let member = context.types.member(ty, index)?;
            Some(null_value(context, member))
        }
        ConstantKind::Poison => {
            let member = context.types.member(ty, index)?;
            Some(poison(context, member))
        }
        ConstantKind::Undef => {
            let member = context.types.member(ty, index)?;
            Some(undef(context, member))
        }
        _ => None,
    }
}

/// `isa<ConstantAggregate>(C) || isa<ConstantDataSequential>(C)`.
fn is_sequence_or_aggregate(
    context: &Context,
    c: ConstantId,
) -> bool {
    matches!(
        context.get(c).kind,
        ConstantKind::Aggregate(_) | ConstantKind::Bytes(_)
    )
        && !is_null_value(context, c)
        && !is_poison(context, c)
}

/// `C` as LLVM uniques it: the aggregates made of zeros or poison are one.
fn canonical(
    context: &Context,
    c: ConstantId,
) -> ConstantId {
    let Constant { ty, kind } = context.get(c).clone();
    match kind {
        ConstantKind::Aggregate(members) => {
            let members = members.into_iter().map(|one| canonical(context, one)).collect();
            aggregate(context, ty, members)
        }
        ConstantKind::Bytes(bytes) if bytes.iter().all(|&one| one == 0) => {
            context.constant(Constant { ty, kind: ConstantKind::Zero })
        }
        ConstantKind::Null | ConstantKind::Zero if context.types.int_bits(ty).is_some() => {
            context.constant(Constant { ty, kind: ConstantKind::Int(0) })
        }
        _ => c,
    }
}

/// `Constant::isManifestConstant`.
pub fn is_manifest_constant(
    context: &Context,
    c: ConstantId,
) -> bool {
    match &context.get(c).kind {
        ConstantKind::Global(_) => false,
        ConstantKind::Aggregate(members) => members.iter().all(|&one| is_manifest_constant(context, one)),
        ConstantKind::Expr(ConstantExpr::Cast { value, .. }) => is_manifest_constant(context, *value),
        ConstantKind::Expr(ConstantExpr::GetElementPtr { operands, .. }) => {
            operands.iter().all(|&one| is_manifest_constant(context, one))
        }
        _ => true,
    }
}

fn expr(
    context: &Context,
    c: ConstantId,
) -> Option<ConstantExpr> {
    match &context.get(c).kind {
        ConstantKind::Expr(expr) => Some(expr.clone()),
        _ => None,
    }
}

/// `DL.isNonIntegralPointerType(Ty)`: a pointer in a non-integral space.
fn non_integral(
    context: &Context,
    layout: &DataLayout,
    ty: TypeId,
) -> bool {
    pointer_space(context, ty).is_some_and(|space| layout.is_non_integral_address_space(space))
}

fn pointer_space(
    context: &Context,
    ty: TypeId,
) -> Option<u32> {
    match context.types.get(ty) {
        Type::Pointer(space) => Some(*space),
        _ => None,
    }
}

fn is_pointer(
    context: &Context,
    ty: TypeId,
) -> bool {
    pointer_space(context, ty).is_some()
}

/// `Type::isSized`.
fn is_sized(
    context: &Context,
    ty: TypeId,
) -> bool {
    match context.types.get(ty) {
        Type::Int(_) | Type::Float(_) | Type::Pointer(_) | Type::Vector { .. } => true,
        Type::Array { element, .. } => is_sized(context, *element),
        Type::Struct { .. } | Type::Named(_) => {
            context.types.fields(ty).is_some_and(|fields| fields.iter().all(|&one| is_sized(context, one)))
        }
        Type::Void | Type::Label | Type::Metadata | Type::Token | Type::Function { .. } => false,
    }
}

fn bits_of(
    context: &Context,
    layout: &DataLayout,
    ty: TypeId,
) -> u64 {
    layout.size_bits(&context.types, ty)
}

fn store_size(
    context: &Context,
    layout: &DataLayout,
    ty: TypeId,
) -> u64 {
    layout.store_size(&context.types, ty)
}

fn alloc_size(
    context: &Context,
    layout: &DataLayout,
    ty: TypeId,
) -> u64 {
    layout.alloc_size(&context.types, ty)
}

/// `DL.typeSizeEqualsStoreSize`.
fn type_size_equals_store_size(
    context: &Context,
    layout: &DataLayout,
    ty: TypeId,
) -> bool {
    bits_of(context, layout, ty) == store_size(context, layout, ty) * 8
}

/// `DL.getIndexSizeInBits` (`DL.getAddressSizeInBits` is the same).
fn index_bits(
    layout: &DataLayout,
    space: u32,
) -> u32 {
    layout.pointer(space).index_bits
}

/// `DL.getPointerSizeInBits`.
fn pointer_bits(
    layout: &DataLayout,
    space: u32,
) -> u32 {
    layout.pointer(space).bits
}

/// `PrimitiveSizeInBits` of a scalar: 0 for a pointer or an aggregate.
fn primitive_bits(
    context: &Context,
    ty: TypeId,
) -> u64 {
    match context.types.get(ty) {
        Type::Int(width) => u64::from(*width),
        Type::Float(kind) => u64::from(crate::datalayout::float_bits(*kind)),
        _ => 0,
    }
}

fn sext_or_trunc(
    value: u128,
    from: u32,
    to: u32,
) -> u128 {
    (signed(value, from) as u128) & mask(to)
}

/// `CastInst::castIsValid`.
fn cast_is_valid(
    context: &Context,
    op: CastOp,
    src: TypeId,
    dst: TypeId,
) -> bool {
    let (s, d) = (context.types.get(src), context.types.get(dst));
    let first_class = |t: &Type| matches!(
        t,
        Type::Int(_) | Type::Float(_) | Type::Pointer(_) | Type::Vector { .. }
    );
    if !first_class(s) || !first_class(d) || matches!(s, Type::Vector { .. }) || matches!(d, Type::Vector { .. }) {
        return false;
    }
    match op {
        CastOp::PtrToInt => matches!((s, d), (Type::Pointer(_), Type::Int(_))),
        CastOp::IntToPtr => matches!((s, d), (Type::Int(_), Type::Pointer(_))),
        CastOp::BitCast => match (s, d) {
            (Type::Pointer(a), Type::Pointer(b)) => a == b,
            (Type::Pointer(_), _) | (_, Type::Pointer(_)) => false,
            _ => primitive_bits(context, src) == primitive_bits(context, dst),
        },
        _ => false,
    }
}

// ---- Constants.cpp: ConstantExpr::get* ----------------------------------

/// `ConstantExpr::getCast`
/// ([Constants.cpp:2362](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Constants.cpp#L2362)):
/// `getFoldedCast`, the fold of `ConstantFoldCastInstruction` or the
/// expression.
pub fn constant_expr_get_cast(
    context: &Context,
    op: CastOp,
    c: ConstantId,
    to: TypeId,
) -> Option<ConstantId> {
    if ty_of(context, c) == to && matches!(op, CastOp::BitCast | CastOp::AddrSpaceCast) {
        return Some(c);
    }
    if let Some(folded) = constfold::cast(context, op, c, to) {
        return Some(folded);
    }
    if matches!(
        op,
        CastOp::Trunc | CastOp::PtrToInt | CastOp::IntToPtr | CastOp::BitCast | CastOp::AddrSpaceCast
    ) {
        return Some(
            context.constant(Constant { ty: to, kind: ConstantKind::Expr(ConstantExpr::Cast { op, value: c }) }),
        );
    }
    None
}

/// `ConstantFoldGetElementPtr`
/// ([ConstantFold.cpp:1349](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFold.cpp#L1349))
/// and the expression `ConstantExpr::getGetElementPtr` makes otherwise
/// ([Constants.cpp:2662](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Constants.cpp#L2662)).
pub fn constant_fold_get_element_ptr(
    context: &Context,
    source: TypeId,
    c: ConstantId,
    indices: &[ConstantId],
    nw: u8,
) -> ConstantId {
    if indices.is_empty() {
        return c;
    }
    let ty = ty_of(context, c);
    if is_poison(context, c) {
        return poison(context, ty);
    }
    // `isa<UndefValue>` is true of poison.
    if indices.iter().all(|&one| is_null_value(context, one) || is_poison(context, one)) {
        return c;
    }
    let mut operands = vec![c];
    operands.extend_from_slice(indices);
    context.constant(Constant {
        ty,
        kind: ConstantKind::Expr(ConstantExpr::GetElementPtr {
            source,
            inbounds: nw & IN_BOUNDS != 0,
            nusw: nw & (IN_BOUNDS | NUSW) != 0,
            nuw: nw & NUW != 0,
            operands,
        }),
    })
}

/// `ConstantExpr::getPtrAdd`.
fn constant_expr_get_ptr_add(
    context: &Context,
    ptr: ConstantId,
    offset: ConstantId,
    nw: u8,
) -> ConstantId {
    let i8 = context.types.int(8);
    constant_fold_get_element_ptr(context, i8, ptr, &[offset], nw)
}

/// `ConstantExpr::getGetElementPtr` with the data layout
/// ([Constants.cpp:2713](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Constants.cpp#L2713)).
pub fn constant_expr_get_gep_with_layout(
    context: &Context,
    layout: &DataLayout,
    source: TypeId,
    c: ConstantId,
    indices: &[ConstantId],
    nw: u8,
) -> Option<ConstantId> {
    let space = pointer_space(context, ty_of(context, c))?;
    let idx_bits = index_bits(layout, space);
    let idx_ty = context.types.int(idx_bits);
    if context.types.int_bits(source) == Some(8) && ty_of(context, indices[0]) == idx_ty {
        return Some(constant_expr_get_ptr_add(context, c, indices[0], nw));
    }
    if !is_sized(context, source) {
        return None;
    }
    let mut offset = int(context, idx_bits, 0);
    let mut current = source;
    for (at, &index) in indices.iter().enumerate() {
        let struct_like = at > 0 && context.types.fields(current).is_some();
        let stride;
        if at == 0 {
            stride = alloc_size(context, layout, source);
        } else if struct_like {
            let (value, _) = as_int(context, index)?;
            let (_, offsets) = layout.struct_layout(&context.types, current);
            let size = *offsets.get(value as usize)?;
            current = context.types.member(current, value as u64)?;
            if is_null_value(context, index) || size == 0 {
                continue;
            }
            let size = int(context, idx_bits, u128::from(size));
            offset = constfold::binary(context, BinaryOp::Add, offset, size)?;
            continue;
        } else {
            current = match context.types.get(current) {
                Type::Array { element, .. } | Type::Vector { element, .. } => *element,
                _ => return None,
            };
            stride = alloc_size(context, layout, current);
        }
        if is_null_value(context, index) {
            continue;
        }
        let mut index = index;
        if ty_of(context, index) != idx_ty {
            let from = context.types.int_bits(ty_of(context, index))?;
            let op = if from < idx_bits { CastOp::SExt } else { CastOp::Trunc };
            index = constfold::cast(context, op, index, idx_ty)?;
        }
        if stride != 1 {
            let scale = int(context, idx_bits, u128::from(stride));
            index = constfold::binary(context, BinaryOp::Mul, index, scale)?;
        }
        offset = constfold::binary(context, BinaryOp::Add, offset, index)?;
    }
    Some(constant_expr_get_ptr_add(context, c, offset, nw))
}

// ---- Operator.cpp, Value.cpp, DataLayout.cpp ----------------------------

/// `DataLayout::getIndexedOffsetInType`
/// ([DataLayout.cpp:1054](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/DataLayout.cpp#L1054)).
fn get_indexed_offset_in_type(
    context: &Context,
    layout: &DataLayout,
    source: TypeId,
    indices: &[ConstantId],
) -> i64 {
    let mut result: i64 = 0;
    let mut current = source;
    for (at, &index) in indices.iter().enumerate() {
        let Some((value, width)) = as_int(context, index) else { return result };
        if at > 0 && context.types.fields(current).is_some() {
            let (_, offsets) = layout.struct_layout(&context.types, current);
            result = result.wrapping_add(offsets.get(value as usize).copied().unwrap_or(0) as i64);
            current = context.types.member(current, value as u64).unwrap_or(current);
            continue;
        }
        let stride = if at == 0 {
            alloc_size(context, layout, source)
        } else {
            current = match context.types.get(current) {
                Type::Array { element, .. } | Type::Vector { element, .. } => *element,
                _ => return result,
            };
            alloc_size(context, layout, current)
        };
        let array_index = signed(value, width) as i64;
        if array_index != 0 {
            result = result.wrapping_add(array_index.wrapping_mul(stride as i64));
        }
    }
    result
}

/// `GEPOperator::accumulateConstantOffset`
/// ([Operator.cpp:141](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Operator.cpp#L141)),
/// the offset modulo 2^`width`.
pub fn accumulate_constant_offset(
    context: &Context,
    layout: &DataLayout,
    source: TypeId,
    indices: &[ConstantId],
    width: u32,
) -> Option<u128> {
    if context.types.int_bits(source) == Some(8) && !indices.is_empty() {
        let (value, from) = as_int(context, indices[0])?;
        return Some(sext_or_trunc(value, from, width));
    }
    let wrap = mask(width);
    let mut offset: u128 = 0;
    let mut current = source;
    for (at, &index) in indices.iter().enumerate() {
        let is_struct = at > 0 && context.types.fields(current).is_some();
        let stride;
        let mut struct_offset = 0;
        if at == 0 {
            stride = alloc_size(context, layout, source);
        } else if is_struct {
            let (value, _) = as_int(context, index)?;
            let (_, offsets) = layout.struct_layout(&context.types, current);
            struct_offset = *offsets.get(value as usize)?;
            current = context.types.member(current, value as u64)?;
            stride = 1;
        } else {
            current = match context.types.get(current) {
                Type::Array { element, .. } | Type::Vector { element, .. } => *element,
                _ => return None,
            };
            stride = alloc_size(context, layout, current);
        }
        let (value, from) = as_int(context, index)?;
        if value == 0 {
            continue;
        }
        let scaled = if is_struct {
            u128::from(struct_offset) & wrap
        } else {
            sext_or_trunc(value, from, width).wrapping_mul(u128::from(stride) & wrap)
        };
        offset = offset.wrapping_add(scaled) & wrap;
    }
    Some(offset)
}

/// `Value::stripAndAccumulateConstantOffsets`
/// ([Value.cpp:732](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Value.cpp#L732))
/// of a constant; `offset` is `width` bits. The `inttoptr (add (ptrtoint p),
/// C)` arm needs an `add` expression, which MIR lacks.
fn strip_and_accumulate_constant_offsets(
    context: &Context,
    layout: &DataLayout,
    c: ConstantId,
    offset: &mut u128,
    width: u32,
    allow_non_inbounds: bool,
) -> ConstantId {
    if !is_pointer(context, ty_of(context, c)) {
        return c;
    }
    let mut value = c;
    loop {
        match expr(context, value) {
            Some(ConstantExpr::GetElementPtr { source, inbounds, operands, .. }) => {
                if !allow_non_inbounds && !inbounds {
                    return value;
                }
                let Some(space) = pointer_space(context, ty_of(context, value)) else { return value };
                let gep_width = index_bits(layout, space);
                let Some(gep_offset) = accumulate_constant_offset(context, layout, source, &operands[1..], gep_width)
                else {
                    return value;
                };
                if crate::apint::significant_bits(gep_offset, gep_width) > width {
                    return value;
                }
                *offset = offset.wrapping_add(sext_or_trunc(gep_offset, gep_width, width)) & mask(width);
                value = operands[0];
            }
            Some(ConstantExpr::Cast { op: CastOp::BitCast, value: source }) => {
                if !is_pointer(context, ty_of(context, source)) {
                    return value;
                }
                value = source;
            }
            // A selector alone is no pointer into the object it was made from:
            // back to far it is segment:0 (DEVIATE C1).
            Some(ConstantExpr::Cast { op: CastOp::AddrSpaceCast, value: source }) => {
                if pointer_space(context, ty_of(context, source))
                    .is_some_and(|space| layout.has_unstable_representation(space))
                {
                    return value;
                }
                value = source
            }
            _ => return value,
        }
    }
}

/// `getUnderlyingObject` of a constant.
fn get_underlying_object(
    context: &Context,
    layout: &DataLayout,
    c: ConstantId,
) -> ConstantId {
    let mut value = c;
    if !is_pointer(context, ty_of(context, c)) {
        return c;
    }
    for _ in 0..6 {
        // Read in place: a clone of the expression's operands per step was most
        // of this function's cost (every load is asked).
        match &context.get(value).kind {
            ConstantKind::Expr(ConstantExpr::GetElementPtr { operands, .. }) => value = operands[0],
            ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::BitCast | CastOp::AddrSpaceCast, value: source }) => {
                let source = *source;
                if !is_pointer(context, ty_of(context, source)) {
                    return value;
                }
                // A selector alone names no object (DEVIATE C1).
                if pointer_space(context, ty_of(context, source))
                    .is_some_and(|space| layout.has_unstable_representation(space))
                {
                    return value;
                }
                value = source;
            }
            _ => return value,
        }
    }
    value
}

/// `GlobalVariable` the constant names.
fn global_variable<'a>(
    env: Env<'a>,
    context: &Context,
    c: ConstantId,
) -> Option<(GlobalId, &'a GlobalValue)> {
    let ConstantKind::Global(id) = context.get(c).kind else { return None };
    let global = env.globals.get(id.0 as usize)?;
    matches!(global.kind, GlobalKind::Variable(_)).then_some((id, global))
}

/// `GlobalVariable::isConstant() && hasDefinitiveInitializer()`: the
/// initializer.
fn definitive_constant_initializer(
    env: Env,
    context: &Context,
    c: ConstantId,
) -> Option<(GlobalId, ConstantId)> {
    let (id, global) = global_variable(env, context, c)?;
    let GlobalKind::Variable(variable) = &global.kind else { return None };
    let interposable =
        matches!(
            global.linkage,
            Linkage::Weak | Linkage::LinkOnce | Linkage::Common | Linkage::ExternWeak
        );
    (variable.constant && !interposable && !variable.externally_initialized).then_some(())?;
    Some((id, variable.initializer?))
}

// ---- the translated functions -------------------------------------------

/// `FoldBitCast`
/// ([L181](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L181)),
/// for scalars.
fn fold_bit_cast(
    context: &Context,
    layout: &DataLayout,
    c: ConstantId,
    dest: TypeId,
) -> ConstantId {
    if let Some(folded) = constant_fold_load_from_uniform_value(context, layout, c, dest) {
        return folded;
    }
    constant_expr_get_cast(context, CastOp::BitCast, c, dest).unwrap_or(c)
}

/// `IsConstantOffsetFromGlobal`
/// ([L412](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L412)):
/// the global and the offset (modulo 2^width) from it.
pub fn is_constant_offset_from_global(
    context: &Context,
    layout: &DataLayout,
    c: ConstantId,
) -> Option<(GlobalId, u128, u32)> {
    if let ConstantKind::Global(global) = context.get(c).kind {
        let space = pointer_space(context, ty_of(context, c))?;
        return Some((global, 0, index_bits(layout, space)));
    }
    match expr(context, c)? {
        ConstantExpr::Cast { op: CastOp::PtrToInt, value } => is_constant_offset_from_global(context, layout, value),
        ConstantExpr::GetElementPtr { source, operands, .. } => {
            let width = index_bits(layout, pointer_space(context, ty_of(context, c))?);
            let (global, mut offset, base_width) = is_constant_offset_from_global(context, layout, operands[0])?;
            if base_width != width {
                offset &= mask(width);
            }
            let added = accumulate_constant_offset(context, layout, source, &operands[1..], width)?;
            Some((global, offset.wrapping_add(added) & mask(width), width))
        }
        _ => None,
    }
}

/// `CastInst::getCastOpcode`-free half of `isEliminableCastPair`
/// ([Instructions.cpp:2884](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instructions.cpp#L2884)).
pub fn is_eliminable_cast_pair(
    context: &Context,
    layout: &DataLayout,
    first: CastOp,
    second: CastOp,
    src: TypeId,
    mid: TypeId,
    dst: TypeId,
) -> Option<CastOp> {
    // Rows: first; columns: second. Trunc ZExt SExt FPToUI FPToSI UIToFP
    // SIToFP FPTrunc FPExt PtrToInt PtrToAddr IntToPtr BitCast AddrSpaceCast.
    const TABLE: [[u8; 14]; 14] = [
        [1, 0, 0, 99, 99, 0, 0, 99, 99, 99, 99, 0, 3, 0],
        [8, 1, 9, 99, 99, 2, 17, 99, 99, 99, 99, 2, 3, 0],
        [8, 0, 1, 99, 99, 0, 2, 99, 99, 99, 99, 0, 3, 0],
        [0, 0, 0, 99, 99, 0, 0, 99, 99, 99, 99, 0, 3, 0],
        [0, 0, 0, 99, 99, 0, 0, 99, 99, 99, 99, 0, 3, 0],
        [99, 99, 99, 0, 0, 99, 99, 0, 0, 99, 99, 99, 4, 0],
        [99, 99, 99, 0, 0, 99, 99, 0, 0, 99, 99, 99, 4, 0],
        [99, 99, 99, 0, 0, 99, 99, 0, 0, 99, 99, 99, 4, 0],
        [99, 99, 99, 2, 2, 99, 99, 8, 2, 99, 99, 99, 4, 0],
        [1, 0, 0, 99, 99, 0, 0, 99, 99, 99, 99, 7, 3, 0],
        [0, 0, 0, 99, 99, 0, 0, 99, 99, 99, 99, 0, 3, 0],
        [99, 99, 99, 99, 99, 99, 99, 99, 99, 11, 11, 99, 15, 0],
        [5, 5, 5, 0, 0, 5, 5, 0, 0, 16, 16, 5, 1, 14],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 13, 12],
    ];
    let row = |op: CastOp| -> Option<usize> {
        Some(match op {
            CastOp::Trunc => 0,
            CastOp::ZExt => 1,
            CastOp::SExt => 2,
            CastOp::FPToUI => 3,
            CastOp::FPToSI => 4,
            CastOp::UIToFP => 5,
            CastOp::SIToFP => 6,
            CastOp::FPTrunc => 7,
            CastOp::FPExt => 8,
            CastOp::PtrToInt => 9,
            CastOp::IntToPtr => 11,
            CastOp::BitCast => 12,
            CastOp::AddrSpaceCast => 13,
            CastOp::FPRound => return None,
        })
    };
    let case = TABLE[row(first)?][row(second)?];
    let types = &context.types;
    let is_int = |ty: TypeId| matches!(types.get(ty), Type::Int(_));
    let scalar_bits = |ty: TypeId| primitive_bits(context, ty);
    match case {
        1 => Some(first),
        2 => Some(second),
        3 => (!matches!(types.get(src), Type::Vector { .. }) && is_int(dst)).then_some(first),
        4 => (dst == mid).then_some(first),
        5 => is_int(src).then_some(second),
        7 => {
            if src != dst {
                return None;
            }
            let space = pointer_space(context, src)?;
            (scalar_bits(mid) >= u64::from(pointer_bits(layout, space))).then_some(CastOp::BitCast)
        }
        8 => {
            let (src_size, dst_size) = (scalar_bits(src), scalar_bits(dst));
            if src == dst {
                Some(CastOp::BitCast)
            } else if src_size < dst_size {
                Some(first)
            } else if src_size > dst_size {
                Some(second)
            } else {
                None
            }
        }
        9 => Some(CastOp::ZExt),
        11 => {
            let mid_size = u64::from(pointer_bits(layout, pointer_space(context, mid)?));
            let (src_size, dst_size) = (scalar_bits(src), scalar_bits(dst));
            if mid_size < src_size && mid_size < dst_size {
                return None;
            }
            Some(match dst_size.cmp(&src_size) {
                std::cmp::Ordering::Less => CastOp::Trunc,
                std::cmp::Ordering::Greater => CastOp::ZExt,
                std::cmp::Ordering::Equal => CastOp::BitCast,
            })
        }
        // DEVIATE C1: the pin composes any two addrspacecasts. A space the
        // target declares non-integral (`ni:` in its datalayout: the selector-
        // alone space) is not a point on the way: near -> selector keeps the
        // segment and drops the offset, selector -> far gives segment:0, and
        // near -> far keeps the offset. Exit: the day our casts through the
        // selector space compose (a pin-shaped far-pointer model).
        12 if pointer_space(context, mid).is_some_and(|space| layout.has_unstable_representation(space)) => None,
        12 => Some(if pointer_space(context, src) != pointer_space(context, dst) {
            CastOp::AddrSpaceCast
        } else {
            CastOp::BitCast
        }),
        13 => is_pointer(context, dst).then_some(first),
        14 => is_pointer(context, src).then_some(CastOp::AddrSpaceCast),
        15 => Some(first),
        16 => Some(second),
        17 => Some(CastOp::UIToFP),
        _ => None,
    }
}

/// `ConstantFoldLoadThroughBitcast`
/// ([L465](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L465)).
pub fn constant_fold_load_through_bitcast(
    context: &Context,
    layout: &DataLayout,
    c: ConstantId,
    dest: TypeId,
) -> Option<ConstantId> {
    let mut c = c;
    loop {
        let src = ty_of(context, c);
        if src == dest {
            return Some(c);
        }
        let dest_size = bits_of(context, layout, dest);
        let src_size = bits_of(context, layout, src);
        if src_size < dest_size {
            return None;
        }
        if let Some(folded) = constant_fold_load_from_uniform_value(context, layout, c, dest) {
            return Some(folded);
        }
        if src_size == dest_size && non_integral(context, layout, src) == non_integral(context, layout, dest) {
            let cast = match (context.types.get(src), context.types.get(dest)) {
                (Type::Int(_), Type::Pointer(_)) => CastOp::IntToPtr,
                (Type::Pointer(_), Type::Int(_)) => CastOp::PtrToInt,
                _ => CastOp::BitCast,
            };
            if cast_is_valid(context, cast, src, dest) {
                return constant_fold_cast_operand(context, layout, cast, c, dest);
            }
        }
        match context.types.get(src) {
            Type::Struct { .. } | Type::Named(_) => {
                let mut element = 0;
                loop {
                    let found = aggregate_element(context, c, element)?;
                    element += 1;
                    if bits_of(context, layout, ty_of(context, found)) != 0 {
                        c = found;
                        break;
                    }
                }
            }
            Type::Array { .. } => c = aggregate_element(context, c, 0)?,
            Type::Vector { element, .. } => {
                let element = *element;
                if !type_size_equals_store_size(context, layout, element) {
                    return None;
                }
                c = aggregate_element(context, c, 0)?;
            }
            _ => return None,
        }
    }
}

/// `ReadDataFromGlobal`
/// ([L540](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L540)):
/// `buf` is `CurPtr`/`BytesLeft`, zero filled by the caller. `IsByteLoad` is
/// false: MIR has no byte type.
fn read_data_from_global(
    context: &Context,
    layout: &DataLayout,
    c: ConstantId,
    byte_offset: u64,
    buf: &mut [u8],
) -> bool {
    let constant = context.get(c);
    if byte_offset >= store_size(context, layout, constant.ty) {
        return true;
    }
    let scalar = matches!(
        context.types.get(constant.ty),
        Type::Int(_) | Type::Float(_) | Type::Pointer(_)
    );
    if constant.kind == ConstantKind::Poison || (constant.kind == ConstantKind::Zero && !scalar) {
        return true;
    }
    if let ConstantKind::Aggregate(_) | ConstantKind::Bytes(_) = constant.kind
        && (is_null_value(context, c) || is_poison(context, c))
    {
        return true;
    }
    // `ConstantInt`, and `ConstantFP` as the integer of its bits.
    let number = match (&constant.kind, context.types.get(constant.ty)) {
        (ConstantKind::Int(_) | ConstantKind::Null | ConstantKind::Zero, Type::Int(_)) => as_int(context, c),
        (ConstantKind::Float(bits), Type::Float(FloatKind::Double)) => Some((u128::from(*bits), 64)),
        (ConstantKind::Float(bits), Type::Float(FloatKind::Float)) => Some((u128::from(*bits & 0xFFFF_FFFF), 32)),
        (ConstantKind::Zero, Type::Float(FloatKind::Double)) => Some((0, 64)),
        (ConstantKind::Zero, Type::Float(FloatKind::Float)) => Some((0, 32)),
        (ConstantKind::Float(_) | ConstantKind::Zero, Type::Float(_)) => return false,
        _ => None,
    };
    if let Some((value, width)) = number {
        if width & 7 != 0 {
            return false;
        }
        let int_bytes = u64::from(width / 8);
        let mut offset = byte_offset;
        for slot in buf.iter_mut() {
            if offset == int_bytes {
                break;
            }
            let n = if layout.big_endian { int_bytes - offset - 1 } else { offset };
            *slot = (value >> (n * 8)) as u8;
            offset += 1;
        }
        return true;
    }
    if let ConstantKind::Aggregate(members) = &constant.kind
        && matches!(
            context.types.get(constant.ty),
            Type::Struct { .. } | Type::Named(_)
        )
    {
        let (_, offsets) = layout.struct_layout(&context.types, constant.ty);
        let mut index = offsets.partition_point(|&one| one <= byte_offset).saturating_sub(1);
        let mut cur_elt_offset = offsets[index];
        let mut byte_offset = byte_offset - cur_elt_offset;
        let mut buf = buf;
        loop {
            let member = members[index];
            let elt_size = alloc_size(context, layout, ty_of(context, member));
            if byte_offset < elt_size && !read_data_from_global(context, layout, member, byte_offset, buf) {
                return false;
            }
            index += 1;
            if index == members.len() {
                return true;
            }
            let next_elt_offset = offsets[index];
            let advance = next_elt_offset - cur_elt_offset - byte_offset;
            if buf.len() as u64 <= advance {
                return true;
            }
            buf = &mut buf[advance as usize..];
            byte_offset = 0;
            cur_elt_offset = next_elt_offset;
        }
    }
    if matches!(
        constant.kind,
        ConstantKind::Aggregate(_) | ConstantKind::Bytes(_)
    ) {
        let (count, elt_size) = match context.types.get(constant.ty) {
            Type::Array { element, count } => (*count, alloc_size(context, layout, *element)),
            Type::Vector { element, count } => {
                if !type_size_equals_store_size(context, layout, *element) {
                    return false;
                }
                (u64::from(*count), store_size(context, layout, *element))
            }
            _ => return false,
        };
        if elt_size == 0 {
            return false;
        }
        let mut index = byte_offset / elt_size;
        let mut offset = byte_offset - index * elt_size;
        let mut buf = buf;
        while index != count {
            match &constant.kind {
                ConstantKind::Aggregate(members) => {
                    if !read_data_from_global(context, layout, members[index as usize], offset, buf) {
                        return false;
                    }
                }
                ConstantKind::Bytes(bytes) => {
                    if offset < 1 {
                        buf[0] = bytes[index as usize];
                    }
                }
                _ => unreachable!("matched above"),
            }
            let written = elt_size - offset;
            if written >= buf.len() as u64 {
                return true;
            }
            offset = 0;
            buf = &mut buf[written as usize..];
            index += 1;
        }
        return true;
    }
    if let Some(ConstantExpr::Cast { op: CastOp::IntToPtr, value }) = expr(context, c) {
        let space = pointer_space(context, constant.ty).unwrap_or(0);
        if context.types.int_bits(ty_of(context, value)) == Some(pointer_bits(layout, space)) {
            return read_data_from_global(context, layout, value, byte_offset, buf);
        }
    }
    false
}

/// `FoldReinterpretLoadFromConst`
/// ([L686](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L686)).
fn fold_reinterpret_load_from_const(
    context: &Context,
    layout: &DataLayout,
    c: ConstantId,
    load_ty: TypeId,
    orig_load_ty: TypeId,
    offset: i64,
) -> Option<ConstantId> {
    let Some(width) = context.types.int_bits(load_ty) else {
        let loadable = match context.types.get(load_ty) {
            Type::Float(FloatKind::X86Fp80) => false,
            Type::Float(_) | Type::Pointer(_) => true,
            _ => false,
        };
        if !loadable {
            return None;
        }
        let map_bits = bits_of(context, layout, load_ty) as u32;
        let map_ty = context.types.int(map_bits);
        let mut result = fold_reinterpret_load_from_const(context, layout, c, map_ty, orig_load_ty, offset)?;
        if is_null_value(context, result) {
            return Some(null_value(context, load_ty));
        }
        let is_ptr = pointer_space(context, load_ty);
        let cast_ty = match is_ptr {
            Some(space) => context.types.int(pointer_bits(layout, space)),
            None => load_ty,
        };
        result = fold_bit_cast(context, layout, result, cast_ty);
        if is_ptr.is_some() {
            if is_null_value(context, result) {
                return Some(null_value(context, load_ty));
            }
            if is_ptr.is_some_and(|space| layout.is_non_integral_address_space(space)) {
                return None;
            }
            result = constant_expr_get_cast(context, CastOp::IntToPtr, result, load_ty)?;
        }
        return Some(result);
    };
    let bytes_loaded = width.div_ceil(8) as usize;
    if bytes_loaded > 128 || bytes_loaded == 0 {
        return None;
    }
    if bytes_loaded > 32 && context.types.int_bits(orig_load_ty).is_some() {
        return None;
    }
    if offset <= -(bytes_loaded as i64) {
        return Some(poison(context, load_ty));
    }
    let initializer_size = alloc_size(context, layout, ty_of(context, c));
    if offset >= initializer_size as i64 {
        return Some(poison(context, load_ty));
    }
    let mut raw = vec![0_u8; bytes_loaded];
    let mut offset = offset;
    let mut start = 0;
    if offset < 0 {
        start = (-offset) as usize;
        offset = 0;
    }
    if !read_data_from_global(context, layout, c, offset as u64, &mut raw[start..]) {
        return None;
    }
    let mut value: u128 = 0;
    for i in 0..bytes_loaded {
        let byte = if layout.big_endian { raw[i] } else { raw[bytes_loaded - 1 - i] };
        value = (value << 8) | u128::from(byte);
    }
    Some(int(context, width, value))
}

/// `ReadByteArrayFromGlobal`
/// ([L789](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L789)):
/// the bytes of a constant global from `offset`, as a `c"..."` array.
pub fn read_byte_array_from_global(
    context: &Context,
    env: Env,
    global: GlobalId,
    offset: u64,
) -> Option<ConstantId> {
    let value = env.globals.get(global.0 as usize)?;
    let GlobalKind::Variable(variable) = &value.kind else { return None };
    let initializer = variable.initializer?;
    let interposable =
        matches!(
            value.linkage,
            Linkage::Weak | Linkage::LinkOnce | Linkage::Common | Linkage::ExternWeak
        );
    if !variable.constant || interposable || variable.externally_initialized {
        return None;
    }
    let init_size = alloc_size(context, env.layout, ty_of(context, initializer));
    if init_size < offset {
        return None;
    }
    let bytes = init_size - offset;
    if bytes > u64::from(u16::MAX) {
        return None;
    }
    let mut raw = vec![0_u8; bytes as usize];
    if !read_data_from_global(context, env.layout, initializer, offset, &mut raw) {
        return None;
    }
    let i8 = context.types.int(8);
    let ty = context.types.intern(Type::Array { element: i8, count: raw.len() as u64 });
    Some(context.constant(Constant { ty, kind: ConstantKind::Bytes(raw) }))
}

/// `DataLayout::getGEPIndicesForOffset`
/// ([DataLayout.cpp:1133](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/DataLayout.cpp#L1133)):
/// `offset` is a signed number of `width` bits.
fn get_gep_indices_for_offset(
    context: &Context,
    layout: &DataLayout,
    elem_ty: TypeId,
    offset: &mut i128,
    width: u32,
) -> Vec<i128> {
    let element_index = |size: u64, offset: &mut i128| -> i128 {
        if size == 0 || u128::from(size) >= 1_u128 << (width - 1) {
            return 0;
        }
        let size = i128::from(size);
        let mut index = *offset / size;
        *offset -= index * size;
        if *offset < 0 {
            index -= 1;
            *offset += size;
        }
        index
    };
    let mut current = elem_ty;
    let mut indices = vec![element_index(alloc_size(context, layout, current), offset)];
    while *offset != 0 {
        match context.types.get(current) {
            Type::Array { element, .. } => {
                current = *element;
                let index = element_index(alloc_size(context, layout, current), offset);
                indices.push(index);
            }
            Type::Struct { .. } | Type::Named(_) => {
                let (size, offsets) = layout.struct_layout(&context.types, current);
                let int_offset = (*offset as u128) & mask(width);
                if int_offset >= u128::from(size) || offsets.is_empty() {
                    break;
                }
                let index = offsets.partition_point(|&one| u128::from(one) <= int_offset) - 1;
                *offset -= offsets[index] as i128;
                current = context.types.member(current, index as u64).expect("a field");
                indices.push(index as i128);
            }
            _ => break,
        }
    }
    indices
}

/// `getConstantAtOffset`
/// ([L819](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L819)).
fn get_constant_at_offset(
    context: &Context,
    layout: &DataLayout,
    base: ConstantId,
    offset: i64,
) -> Option<ConstantId> {
    if offset == 0 {
        return Some(base);
    }
    if !is_sequence_or_aggregate(context, base) {
        return None;
    }
    let mut offset = i128::from(offset);
    let indices = get_gep_indices_for_offset(context, layout, ty_of(context, base), &mut offset, 64);
    if offset != 0 || indices[0] != 0 {
        return None;
    }
    let mut c = base;
    for &index in &indices[1..] {
        if index < 0 || index >= 1 << 32 {
            return None;
        }
        c = aggregate_element(context, c, index as u64)?;
    }
    Some(c)
}

/// `ConstantFoldLoadFromConst`
/// ([L845](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L845)):
/// a load of `ty` at `offset` bytes (a signed 64-bit number) into `c`.
pub fn constant_fold_load_from_const(
    context: &Context,
    layout: &DataLayout,
    c: ConstantId,
    ty: TypeId,
    offset: u64,
) -> Option<ConstantId> {
    let offset = offset as i64;
    if let Some(at_offset) = get_constant_at_offset(context, layout, c, offset)
        && let Some(result) = constant_fold_load_through_bitcast(context, layout, at_offset, ty)
    {
        return Some(result);
    }
    let size = alloc_size(context, layout, ty_of(context, c));
    if offset >= size as i64 {
        return Some(poison(context, ty));
    }
    if let Some(result) = constant_fold_load_from_uniform_value(context, layout, c, ty) {
        return Some(result);
    }
    fold_reinterpret_load_from_const(context, layout, c, ty, ty, offset)
}

/// `ConstantFoldLoadFromConstPtr`
/// ([L876](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L876))
/// with `offset` already added to `ptr` (`width` bits).
fn constant_fold_load_from_const_ptr_at(
    context: &Context,
    env: Env,
    ptr: ConstantId,
    ty: TypeId,
    mut offset: u128,
    width: u32,
) -> Option<ConstantId> {
    let underlying = get_underlying_object(context, env.layout, ptr);
    let (global, initializer) = definitive_constant_initializer(env, context, underlying)?;
    let stripped = strip_and_accumulate_constant_offsets(context, env.layout, ptr, &mut offset, width, true);
    if let ConstantKind::Global(found) = context.get(stripped).kind
        && found == global
        && let Some(result) =
            constant_fold_load_from_const(context, env.layout, initializer, ty, signed(offset, width) as i64 as u64)
    {
        return Some(result);
    }
    constant_fold_load_from_uniform_value(context, env.layout, initializer, ty)
}

/// `ConstantFoldLoadFromConstPtr`
/// ([L898](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L898)).
pub fn constant_fold_load_from_const_ptr(
    context: &Context,
    layout: &DataLayout,
    globals: &[GlobalValue],
    ptr: ConstantId,
    ty: TypeId,
) -> Option<ConstantId> {
    let space = pointer_space(context, ty_of(context, ptr))?;
    let width = index_bits(layout, space);
    constant_fold_load_from_const_ptr_at(context, Env { globals, layout }, ptr, ty, 0, width)
}

/// The load half of `simplifyLoadInst`
/// ([L7714](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L7714))
/// for an address that is an instruction: `object` is its underlying global,
/// `offset` how far into it when every step is constant. A uniform initializer
/// gives its value at any offset.
pub fn constant_fold_load_from_global_object(
    context: &Context,
    layout: &DataLayout,
    globals: &[GlobalValue],
    object: ConstantId,
    offset: Option<i64>,
    ty: TypeId,
) -> Option<ConstantId> {
    let (_, initializer) = definitive_constant_initializer(Env { globals, layout }, context, object)?;
    if let Some(result) = constant_fold_load_from_uniform_value(context, layout, initializer, ty) {
        return Some(result);
    }
    constant_fold_load_from_const(context, layout, initializer, ty, offset? as u64)
}

/// `ConstantFoldLoadFromUniformValue`
/// ([L904](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L904)).
pub fn constant_fold_load_from_uniform_value(
    context: &Context,
    layout: &DataLayout,
    c: ConstantId,
    ty: TypeId,
) -> Option<ConstantId> {
    if is_poison(context, c) {
        return Some(poison(context, ty));
    }
    if !type_size_equals_store_size(context, layout, ty_of(context, c)) {
        return None;
    }
    if is_null_value(context, c) {
        return Some(null_value(context, ty));
    }
    if is_all_ones_value(context, c) && matches!(context.types.get(ty), Type::Int(_) | Type::Float(_)) {
        return all_ones_value(context, ty);
    }
    None
}

/// A query on `context` for the ValueTracking calls of a fold that has no
/// function: the one of an empty declaration.
fn with_query<R>(
    context: &Context,
    env: Env,
    f: impl FnOnce(&Query) -> R,
) -> R {
    thread_local! {
        static EMPTY: &'static Function = {
            let module = crate::parse::module("declare void @empty()").expect("a declaration parses");
            match module.globals.into_iter().next().map(|global| global.kind) {
                Some(GlobalKind::Function(function)) => &*Box::leak(function),
                _ => unreachable!("one function"),
            }
        };
    }
    let function: &'static Function = EMPTY.with(|function| *function);
    f(&Query { globals: env.globals, ..Query::bare(context, function, env.layout) })
}

/// `SymbolicallyEvaluateBinop`
/// ([L929](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L929)).
fn symbolically_evaluate_binop(
    context: &Context,
    env: Env,
    op: BinaryOp,
    op0: ConstantId,
    op1: ConstantId,
) -> Option<ConstantId> {
    let layout = env.layout;
    let width = context.types.int_bits(ty_of(context, op0))?;
    if op == BinaryOp::And {
        let known = |context: &Context, c: ConstantId| {
            with_query(context, env, |q| crate::valuetracking::known::compute_known_bits(q, Operand::Constant(c), 0))
        };
        let (known0, known1) = (known(context, op0), known(context, op1));
        let all = mask(width);
        if (known1.one | known0.zero) & all == all {
            return Some(op0);
        }
        if (known0.one | known1.zero) & all == all {
            return Some(op1);
        }
        let (zero, one) = (known0.zero | known1.zero, known0.one & known1.one);
        if (zero | one) & all == all && zero & one == 0 {
            return Some(int(context, width, one));
        }
    }
    if op == BinaryOp::Sub
        && let Some((global1, offset1, _)) = is_constant_offset_from_global(context, layout, op0)
        && let Some((global2, offset2, _)) = is_constant_offset_from_global(context, layout, op1)
        && global1 == global2
    {
        let op_size = bits_of(context, layout, ty_of(context, op0)) as u32;
        let (a, b) = (offset1 & mask(op_size), offset2 & mask(op_size));
        return Some(int(context, op_size, a.wrapping_sub(b)));
    }
    None
}

/// `CastGEPIndices`
/// ([L977](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L977)).
fn cast_gep_indices(
    context: &Context,
    env: Env,
    source: TypeId,
    ops: &[ConstantId],
    result_ty: TypeId,
    nw: u8,
) -> Option<ConstantId> {
    let layout = env.layout;
    let idx_bits = index_bits(layout, pointer_space(context, result_ty)?);
    let idx_ty = context.types.int(idx_bits);
    let mut any = false;
    let mut new_indices = Vec::new();
    let mut current = source;
    for i in 1..ops.len() {
        // The type `ops[i]` indexes: `source`, then what `ops[2..i]` reach.
        if i >= 3 {
            current = match context.types.get(current).clone() {
                Type::Array { element, .. } | Type::Vector { element, .. } => element,
                Type::Struct { .. } | Type::Named(_) => {
                    let (field, _) = as_int(context, ops[i - 1])?;
                    context.types.member(current, field as u64)?
                }
                _ => return None,
            };
        }
        let is_struct = i >= 2 && context.types.fields(current).is_some();
        if (i == 1 || !is_struct) && ty_of(context, ops[i]) != idx_ty {
            any = true;
            let from = context.types.int_bits(ty_of(context, ops[i]))?;
            let op = match from.cmp(&idx_bits) {
                std::cmp::Ordering::Greater => CastOp::Trunc,
                std::cmp::Ordering::Less => CastOp::SExt,
                std::cmp::Ordering::Equal => CastOp::BitCast,
            };
            let new_index = constant_fold_cast_operand(context, layout, op, ops[i], idx_ty)?;
            new_indices.push(new_index);
        } else {
            new_indices.push(ops[i]);
        }
    }
    if !any {
        return None;
    }
    let folded = constant_expr_get_gep_with_layout(context, layout, source, ops[0], &new_indices, nw)?;
    Some(constant_fold_constant(context, env, folded))
}

/// `SymbolicallyEvaluateGEP`
/// ([L1015](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L1015)).
fn symbolically_evaluate_gep(
    context: &Context,
    env: Env,
    source: TypeId,
    nw: u8,
    res_ty: TypeId,
    ops: &[ConstantId],
) -> Option<ConstantId> {
    let layout = env.layout;
    let mut source = source;
    if !is_sized(context, source) {
        return None;
    }
    if let Some(folded) = cast_gep_indices(context, env, source, ops, res_ty, nw) {
        return Some(folded);
    }
    let mut ptr = ops[0];
    let space = pointer_space(context, ty_of(context, ptr))?;
    if ops[1..].iter().any(|&one| as_int(context, one).is_none()) {
        return None;
    }
    let width = index_bits(layout, space);
    let wrap = mask(width);
    let mut offset = get_indexed_offset_in_type(context, layout, source, &ops[1..]) as u128 & wrap;
    let mut nw = nw;
    let mut overflow = false;
    while let Some(ConstantExpr::GetElementPtr { source: nested_source, inbounds, nusw, nuw, operands }) =
        expr(context, ptr)
    {
        nw &= u8::from(inbounds) * IN_BOUNDS | u8::from(nusw) * NUSW | u8::from(nuw) * NUW;
        let nested = &operands[1..];
        if nested.iter().any(|&one| as_int(context, one).is_none()) {
            break;
        }
        ptr = operands[0];
        source = nested_source;
        let added = get_indexed_offset_in_type(context, layout, source, nested) as u128 & wrap;
        let sum = offset.wrapping_add(added) & wrap;
        let negative = |v: u128| v >> (width - 1) & 1 == 1;
        overflow = negative(offset) == negative(added) && negative(sum) != negative(offset);
        offset = sum;
    }
    if nw & NUSW != 0 && nw & IN_BOUNDS == 0 && overflow {
        nw &= !(IN_BOUNDS | NUSW);
    }
    let pointer_width = pointer_bits(layout, pointer_space(context, ty_of(context, ptr))?);
    let mut base_int_val: u128 = 0;
    if let Some(ConstantExpr::Cast { op: CastOp::IntToPtr, value }) = expr(context, ptr)
        && let Some((base, _)) = as_int(context, value)
    {
        base_int_val = base & mask(pointer_width);
    }
    if (is_null_value(context, ptr) || base_int_val != 0)
        && !pointer_space(context, ty_of(context, ptr)).is_some_and(|space| layout.must_not_introduce_int_to_ptr(space))
    {
        let low = (base_int_val & wrap).wrapping_add(offset) & wrap;
        let base_int_val = (base_int_val & !wrap) | low;
        let c = int(context, pointer_width, base_int_val);
        return constant_expr_get_cast(context, CastOp::IntToPtr, c, res_ty);
    }
    let negative = offset >> (width - 1) & 1 == 1;
    if nw & IN_BOUNDS == 0 && !negative {
        let bytes = if let Some((_, global)) = global_variable(env, context, ptr)
            && let GlobalKind::Variable(variable) = &global.kind
            && is_sized(context, variable.ty)
            && global.linkage != Linkage::ExternWeak
        {
            store_size(context, layout, variable.ty)
        } else {
            0
        };
        if bytes != 0 && offset <= u128::from(bytes) {
            nw |= IN_BOUNDS | NUSW;
        }
    }
    if nw & NUSW != 0 && !negative {
        nw |= NUW;
    }
    let offset = int(context, width, offset);
    Some(constant_expr_get_ptr_add(context, ptr, offset, nw))
}

/// `ConstantFoldInstOperandsImpl`
/// ([L1131](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L1131)).
/// `site` is the function of the instruction, or none for a constant
/// expression.
fn constant_fold_inst_operands_impl(
    context: &Context,
    env: Env,
    site: Option<&Function>,
    dest: TypeId,
    opcode: &Opcode,
    flags: Flags,
    ops: &[ConstantId],
) -> Option<ConstantId> {
    let vector = |ty: TypeId| matches!(context.types.get(ty), Type::Vector { .. });
    if vector(dest) || ops.iter().any(|&one| vector(ty_of(context, one))) {
        return None;
    }
    match opcode {
        Opcode::FNeg => constant_fold_unary_op_operand(context, ops[0]),
        Opcode::Binary(op) => {
            if matches!(
                op,
                BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul | BinaryOp::FDiv | BinaryOp::FRem
            ) {
                return constfold::float_binary(context, *op, flags, ops[0], ops[1]);
            }
            constant_fold_binary_op_operands(context, env, *op, ops[0], ops[1])
        }
        Opcode::Cast(op) => constant_fold_cast_operand(context, env.layout, *op, ops[0], dest),
        Opcode::GetElementPtr { source } => {
            let nw = nw_of_flags(flags);
            if let Some(c) = symbolically_evaluate_gep(context, env, *source, nw, dest, ops) {
                return Some(c);
            }
            if !integer_indices(context, &ops[1..]) {
                return None;
            }
            constant_expr_get_gep_with_layout(context, env.layout, *source, ops[0], &ops[1..], nw)
        }
        Opcode::ICmp(predicate) => {
            constant_fold_compare_inst_operands(context, env.globals, env.layout, *predicate, ops[0], ops[1])
        }
        Opcode::FCmp(predicate) => constfold::compare_float(context, *predicate, ops[0], ops[1]),
        Opcode::Freeze => {
            let guaranteed = with_query(context, env, |q| {
                crate::valuetracking::guarantee::is_guaranteed_not_to_be_undef_or_poison(
                    q,
                    Operand::Constant(ops[0]),
                    0,
                )
            });
            guaranteed.then_some(ops[0])
        }
        Opcode::Call(info) => {
            let function = site?;
            let callee = *ops.last()?;
            let ConstantKind::Global(id) = context.get(callee).kind else { return None };
            let global = env.globals.get(id.0 as usize)?;
            let GlobalKind::Function(callee_function) = &global.kind else { return None };
            let name = global.name.as_deref()?;
            if can_constant_fold_call_to(info, callee_function, name) {
                return constant_fold_call(context, Some(function), info, callee_function, name, &ops[..ops.len() - 1]);
            }
            None
        }
        Opcode::Select => constfold::select(context, ops[0], ops[1], ops[2]),
        Opcode::ExtractValue(indices) => constant_fold_extract_value_instruction(context, ops[0], indices),
        Opcode::InsertValue(indices) => constant_fold_insert_value_instruction(context, ops[0], ops[1], indices),
        Opcode::Load { volatile, .. } => {
            if *volatile {
                return None;
            }
            constant_fold_load_from_const_ptr(context, env.layout, env.globals, ops[0], dest)
        }
        _ => None,
    }
}

/// DEVIATE: a constant GEP is made only of integer indices, the only ones
/// `globals::constant_offset` lowers; LLVM's x86 ISel lowers any constant.
pub(crate) fn integer_indices(
    context: &Context,
    indices: &[ConstantId],
) -> bool {
    indices
        .iter()
        .all(|&one| matches!(
            context.get(one).kind,
            ConstantKind::Int(_) | ConstantKind::Null | ConstantKind::Zero
        ))
}

/// `GEPNoWrapFlags` of an instruction's flags.
pub(crate) fn nw_of_flags(flags: Flags) -> u8 {
    let mut nw = 0;
    if flags.contains(Flags::INBOUNDS) {
        nw |= IN_BOUNDS | NUSW;
    }
    if flags.contains(Flags::NUSW) {
        nw |= NUSW;
    }
    if flags.contains(Flags::NUW) {
        nw |= NUW;
    }
    nw
}

/// `ConstantFoldExtractValueInstruction`
/// ([ConstantFold.cpp:538](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFold.cpp#L538)).
pub fn constant_fold_extract_value_instruction(
    context: &Context,
    agg: ConstantId,
    indices: &[u32],
) -> Option<ConstantId> {
    let Some((&first, rest)) = indices.split_first() else { return Some(agg) };
    let element = aggregate_element(context, agg, u64::from(first))?;
    constant_fold_extract_value_instruction(context, element, rest)
}

/// `ConstantFoldInsertValueInstruction`
/// ([ConstantFold.cpp:550](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFold.cpp#L550)).
pub fn constant_fold_insert_value_instruction(
    context: &Context,
    agg: ConstantId,
    value: ConstantId,
    indices: &[u32],
) -> Option<ConstantId> {
    let Some((&first, rest)) = indices.split_first() else { return Some(value) };
    let ty = ty_of(context, agg);
    let count = match context.types.get(ty) {
        Type::Array { count, .. } => *count,
        _ => context.types.fields(ty)?.len() as u64,
    };
    let mut result = Vec::new();
    for i in 0..count {
        let mut element = aggregate_element(context, agg, i)?;
        if u64::from(first) == i {
            element = constant_fold_insert_value_instruction(context, element, value, rest)?;
        }
        result.push(element);
    }
    Some(aggregate(context, ty, result))
}

/// `ConstantFoldConstantImpl`
/// ([L1231](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L1231)).
fn constant_fold_constant_impl(
    context: &Context,
    env: Env,
    c: ConstantId,
    folded_ops: &mut HashMap<ConstantId, ConstantId>,
) -> ConstantId {
    let Some(ce) = expr(context, c) else { return c };
    let operands: Vec<ConstantId> = match &ce {
        ConstantExpr::Cast { value, .. } => vec![*value],
        ConstantExpr::Binary { lhs, rhs, .. } => vec![*lhs, *rhs],
        ConstantExpr::GetElementPtr { operands, .. } => operands.clone(),
    };
    let mut ops = Vec::new();
    for old in operands {
        let mut new = old;
        if expr(context, old).is_some() {
            new = match folded_ops.get(&old) {
                Some(&done) => done,
                None => {
                    let done = constant_fold_constant_impl(context, env, old, folded_ops);
                    folded_ops.insert(old, done);
                    done
                }
            };
        }
        ops.push(new);
    }
    let dest = ty_of(context, c);
    let (opcode, flags) = match ce {
        ConstantExpr::Cast { op, .. } => (Opcode::Cast(op), Flags::default()),
        ConstantExpr::Binary { op, .. } => (Opcode::Binary(op), Flags::default()),
        ConstantExpr::GetElementPtr { source, inbounds, nusw, nuw, .. } => {
            let mut flags = Flags::default();
            if inbounds {
                flags.insert(Flags::INBOUNDS);
            }
            if nusw {
                flags.insert(Flags::NUSW);
            }
            if nuw {
                flags.insert(Flags::NUW);
            }
            (Opcode::GetElementPtr { source }, flags)
        }
    };
    constant_fold_inst_operands_impl(context, env, None, dest, &opcode, flags, &ops).unwrap_or(c)
}

/// `ConstantFoldInstruction`
/// ([L1268](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L1268)).
pub fn constant_fold_instruction(
    context: &Context,
    env: Env,
    function: &Function,
    inst: InstId,
) -> Option<ConstantId> {
    let instruction = function.instruction(inst);
    let mut folded_ops = HashMap::default();
    if instruction.opcode == Opcode::Phi {
        let mut common: Option<ConstantId> = None;
        for pair in instruction.operands.chunks(2) {
            let Operand::Constant(incoming) = pair[0] else { return None };
            // `PoisonValue` is an `UndefValue`: skipped.
            if is_poison(context, incoming) {
                continue;
            }
            let c = constant_fold_constant_impl(context, env, incoming, &mut folded_ops);
            let c = canonical(context, c);
            if common.is_some_and(|common| common != c) {
                return None;
            }
            common = Some(c);
        }
        return common;
    }
    let mut ops = Vec::new();
    for &operand in &instruction.operands {
        let Operand::Constant(c) = operand else { return None };
        ops.push(constant_fold_constant_impl(context, env, c, &mut folded_ops));
    }
    constant_fold_inst_operands_at(context, env, function, inst, &ops)
}

/// `ConstantFoldConstant`
/// ([L1317](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L1317)).
pub fn constant_fold_constant(
    context: &Context,
    env: Env,
    c: ConstantId,
) -> ConstantId {
    constant_fold_constant_impl(context, env, c, &mut HashMap::default())
}

/// `ConstantFoldInstOperands`
/// ([L1323](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L1323)).
pub fn constant_fold_inst_operands_at(
    context: &Context,
    env: Env,
    function: &Function,
    inst: InstId,
    ops: &[ConstantId],
) -> Option<ConstantId> {
    let Instruction { opcode, ty, flags, .. } = function.instruction(inst);
    constant_fold_inst_operands_impl(context, env, Some(function), *ty, opcode, *flags, ops)
}

/// `ConstantFoldCompareInstOperands`
/// ([L1332](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L1332)).
pub fn constant_fold_compare_inst_operands(
    context: &Context,
    globals: &[GlobalValue],
    layout: &DataLayout,
    predicate: IntPredicate,
    ops0: ConstantId,
    ops1: ConstantId,
) -> Option<ConstantId> {
    if let Some(ce0) = expr(context, ops0) {
        if is_null_value(context, ops1) {
            match ce0 {
                ConstantExpr::Cast { op: CastOp::IntToPtr, value } => {
                    let space = pointer_space(context, ty_of(context, ops0))?;
                    let int_ptr = context.types.int(pointer_bits(layout, space));
                    if let Some(c) = constant_fold_integer_cast(context, layout, value, int_ptr, false) {
                        let null = null_value(context, ty_of(context, c));
                        return constant_fold_compare_inst_operands(context, globals, layout, predicate, c, null);
                    }
                }
                ConstantExpr::Cast { op: CastOp::PtrToInt, value } => {
                    let space = pointer_space(context, ty_of(context, value))?;
                    let addr = context.types.int(index_bits(layout, space));
                    if ty_of(context, ops0) == addr {
                        let null = null_value(context, ty_of(context, value));
                        return constant_fold_compare_inst_operands(context, globals, layout, predicate, value, null);
                    }
                }
                _ => {}
            }
        }
        if let Some(ce1) = expr(context, ops1) {
            match (&ce0, &ce1) {
                (
                    ConstantExpr::Cast { op: CastOp::IntToPtr, value: v0 },
                    ConstantExpr::Cast { op: CastOp::IntToPtr, value: v1 },
                ) => {
                    let space = pointer_space(context, ty_of(context, ops0))?;
                    let int_ptr = context.types.int(pointer_bits(layout, space));
                    let c0 = constant_fold_integer_cast(context, layout, *v0, int_ptr, false);
                    let c1 = constant_fold_integer_cast(context, layout, *v1, int_ptr, false);
                    if let (Some(c0), Some(c1)) = (c0, c1) {
                        return constant_fold_compare_inst_operands(context, globals, layout, predicate, c0, c1);
                    }
                }
                (
                    ConstantExpr::Cast { op: CastOp::PtrToInt, value: v0 },
                    ConstantExpr::Cast { op: CastOp::PtrToInt, value: v1 },
                ) => {
                    let space = pointer_space(context, ty_of(context, *v0))?;
                    let addr = context.types.int(index_bits(layout, space));
                    if ty_of(context, ops0) == addr && ty_of(context, *v0) == ty_of(context, *v1) {
                        return constant_fold_compare_inst_operands(context, globals, layout, predicate, *v0, *v1);
                    }
                }
                _ => {}
            }
        }
        let is_signed =
            matches!(
                predicate,
                IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
            );
        if let Some(space) = pointer_space(context, ty_of(context, ops0))
            && !is_signed
        {
            let width = index_bits(layout, space);
            let is_eq = matches!(predicate, IntPredicate::Eq | IntPredicate::Ne);
            let (mut offset0, mut offset1) = (0, 0);
            let stripped0 = strip_and_accumulate_constant_offsets(context, layout, ops0, &mut offset0, width, is_eq);
            let stripped1 = strip_and_accumulate_constant_offsets(context, layout, ops1, &mut offset1, width, is_eq);
            if stripped0 == stripped1 {
                let (a, b) = (signed(offset0, width), signed(offset1, width));
                let held = match predicate {
                    IntPredicate::Eq => a == b,
                    IntPredicate::Ne => a != b,
                    IntPredicate::Ugt => a > b,
                    IntPredicate::Uge => a >= b,
                    IntPredicate::Ult => a < b,
                    IntPredicate::Ule => a <= b,
                    _ => unreachable!("unsigned"),
                };
                return Some(int(context, 1, u128::from(held)));
            }
        }
    } else if expr(context, ops1).is_some() {
        return constant_fold_compare_inst_operands(context, globals, layout, predicate.swapped(), ops1, ops0);
    }
    constfold::compare_in(context, globals, predicate, ops0, ops1)
}

/// `ConstantFoldUnaryOpOperand`
/// ([L1448](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L1448)):
/// the only unary operation is `fneg`, which this crate does not fold.
pub fn constant_fold_unary_op_operand(
    _context: &Context,
    _op: ConstantId,
) -> Option<ConstantId> {
    None
}

/// `ConstantFoldBinaryOpOperands`
/// ([L1455](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L1455)).
/// `ConstantExpr::get` of `add`, `sub` and `xor` (L1463) folds as
/// `ConstantFoldBinaryInstruction` and otherwise makes a binary expression,
/// which MIR cannot hold.
pub fn constant_fold_binary_op_operands(
    context: &Context,
    env: Env,
    op: BinaryOp,
    lhs: ConstantId,
    rhs: ConstantId,
) -> Option<ConstantId> {
    if (expr(context, lhs).is_some() || expr(context, rhs).is_some())
        && let Some(c) = symbolically_evaluate_binop(context, env, op, lhs, rhs)
    {
        return Some(c);
    }
    constfold::binary(context, op, lhs, rhs)
}

/// `ConstantFoldCastOperand`
/// ([L1619](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L1619)).
pub fn constant_fold_cast_operand(
    context: &Context,
    layout: &DataLayout,
    op: CastOp,
    c: ConstantId,
    dest: TypeId,
) -> Option<ConstantId> {
    if let Some(ConstantExpr::Cast { op: first, value }) = expr(context, c) {
        let (src, mid) = (ty_of(context, value), ty_of(context, c));
        if let Some(new_op) = is_eliminable_cast_pair(context, layout, first, op, src, mid, dest) {
            return constant_fold_cast_operand(context, layout, new_op, value, dest);
        }
    }
    match op {
        CastOp::PtrToInt => {
            if let Some(ce) = expr(context, c) {
                let mut folded = None;
                match ce {
                    ConstantExpr::Cast { op: CastOp::IntToPtr, value } => {
                        let space = pointer_space(context, ty_of(context, c))?;
                        let mid = context.types.int(index_bits(layout, space));
                        folded = constant_fold_integer_cast(context, layout, value, mid, false);
                    }
                    ConstantExpr::GetElementPtr { .. } => {
                        let width = index_bits(layout, pointer_space(context, ty_of(context, c))?);
                        let mut base_offset = 0;
                        let base =
                            strip_and_accumulate_constant_offsets(context, layout, c, &mut base_offset, width, true);
                        if is_null_value(context, base) {
                            folded = Some(int(context, width, base_offset));
                        }
                    }
                    _ => {}
                }
                if let Some(folded) = folded {
                    return constant_fold_integer_cast(context, layout, folded, dest, false);
                }
            }
        }
        CastOp::IntToPtr => {
            if let Some(ConstantExpr::Cast { op: CastOp::PtrToInt, value: src_ptr }) = expr(context, c) {
                let src_space = pointer_space(context, ty_of(context, src_ptr))?;
                let mid_bits = context.types.int_bits(ty_of(context, c))?;
                if mid_bits >= pointer_bits(layout, src_space) && Some(src_space) == pointer_space(context, dest) {
                    return Some(fold_bit_cast(context, layout, src_ptr, dest));
                }
            }
        }
        CastOp::BitCast => return Some(fold_bit_cast(context, layout, c, dest)),
        _ => {}
    }
    if matches!(
        op,
        CastOp::Trunc | CastOp::PtrToInt | CastOp::IntToPtr | CastOp::BitCast | CastOp::AddrSpaceCast
    ) {
        return constant_expr_get_cast(context, op, c, dest);
    }
    constfold::cast(context, op, c, dest)
}

/// `ConstantFoldIntegerCast`
/// ([L1720](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L1720)).
pub fn constant_fold_integer_cast(
    context: &Context,
    layout: &DataLayout,
    c: ConstantId,
    dest: TypeId,
    is_signed: bool,
) -> Option<ConstantId> {
    let src = ty_of(context, c);
    if src == dest {
        return Some(c);
    }
    let op = if primitive_bits(context, src) > primitive_bits(context, dest) {
        CastOp::Trunc
    } else if is_signed {
        CastOp::SExt
    } else {
        CastOp::ZExt
    };
    constant_fold_cast_operand(context, layout, op, c, dest)
}

// ---- calls ----------------------------------------------------------------

/// The integer intrinsics, by `Intrinsic::ID`; `is_constant` has none in
/// [`IntrinsicId`].
#[derive(Clone, Copy, PartialEq)]
enum Folded {
    Id(IntrinsicId),
    IsConstant,
}

fn folded_intrinsic(name: &str) -> Option<Folded> {
    if name.strip_prefix("llvm.is.constant").is_some_and(|rest| rest.is_empty() || rest.starts_with('.')) {
        return Some(Folded::IsConstant);
    }
    intrinsic_id_of_name(name).map(Folded::Id)
}

/// `canConstantFoldIntrinsic`
/// ([L1737](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L1737)),
/// for the integer intrinsics.
fn can_constant_fold_intrinsic(id: Folded) -> bool {
    use IntrinsicId::*;
    match id {
        Folded::IsConstant => true,
        Folded::Id(id) => matches!(
            id,
            Bswap
                | Ctpop
                | Ctlz
                | Cttz
                | Fshl
                | Fshr
                | Clmul
                | Pdep
                | Pext
                | LaunderInvariantGroup
                | Abs
                | Smax
                | Smin
                | Umax
                | Umin
                | Scmp
                | Ucmp
                | SaddWithOverflow
                | UaddWithOverflow
                | SsubWithOverflow
                | UsubWithOverflow
                | SmulWithOverflow
                | UmulWithOverflow
                | Smulh
                | Umulh
                | Fma
                | Fmuladd
                | Fabs
                | Floor
                | Ceil
                | Trunc
                | Round
                | Roundeven
                | Rint
                | Nearbyint
                | Sqrt
                | Sin
                | Cos
                | Atan
                | Sinh
                | Cosh
                | Log
                | Log2
                | Log10
                | Exp
                | Exp2
                | Exp10
                | SaddSat
                | UaddSat
                | SsubSat
                | UsubSat
                | SmulFix
                | SmulFixSat
                | SdivFix
                | Bitreverse
                | Lrint
                | Llrint
        ),
    }
}

fn has_flag(
    attrs: &[Attribute],
    name: &str,
) -> bool {
    attrs.iter().any(|attr| matches!(attr, Attribute::Flag(flag) if flag == name))
}

/// `canConstantFoldCallTo`
/// ([L2102](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L2102)).
/// There is no TLI, so a library function is not folded.
pub fn can_constant_fold_call_to(
    call: &crate::opcode::CallInfo,
    callee: &Function,
    name: &str,
) -> bool {
    let strict_fp = has_flag(&call.attrs, "strictfp");
    let no_builtin = (has_flag(&call.attrs, "nobuiltin") || has_flag(&callee.attrs, "nobuiltin")) && !strict_fp;
    if no_builtin || call.function_type != callee.ty {
        return false;
    }
    folded_intrinsic(name).is_some_and(can_constant_fold_intrinsic)
}

/// `intrinsicPropagatesPoison`
/// ([ValueTracking.cpp:8447](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8447)),
/// for the integer intrinsics this file folds.
fn intrinsic_propagates_poison(id: Folded) -> bool {
    use IntrinsicId::*;
    matches!(
        id,
        Folded::Id(
            SaddWithOverflow
                | SsubWithOverflow
                | SmulWithOverflow
                | UaddWithOverflow
                | UsubWithOverflow
                | UmulWithOverflow
                | Ctpop
                | Ctlz
                | Cttz
                | Abs
                | Smax
                | Smin
                | Umax
                | Umin
                | Scmp
                | Ucmp
                | Bitreverse
                | Bswap
                | SaddSat
                | SsubSat
                | UaddSat
                | UsubSat
                | SmulFix
                | SmulFixSat
                | Fshl
                | Fshr
        )
    )
}

/// `ConstantFoldScalarCall1`
/// ([L2610](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L2610)).
fn constant_fold_scalar_call1(
    context: &Context,
    id: Folded,
    ty: TypeId,
    operand: ConstantId,
    caller: Option<&Function>,
) -> Option<ConstantId> {
    if id == Folded::IsConstant {
        return is_manifest_constant(context, operand).then(|| int(context, 1, 1));
    }
    if matches!(
        context.get(operand).kind,
        ConstantKind::Null | ConstantKind::Zero
    )
        && let Some(space) = pointer_space(context, ty_of(context, operand))
        && id == Folded::Id(IntrinsicId::LaunderInvariantGroup)
    {
        let caller = caller?;
        let null_defined = space == 0
            && caller.attrs.iter().any(|attr| {
                matches!(
                    attr,
                    Attribute::Str(name, Some(value)) if name == "null-pointer-is-valid" && value == "true"
                )
            });
        return (!null_defined).then_some(operand);
    }
    if matches!(id, Folded::Id(IntrinsicId::Lrint | IntrinsicId::Llrint)) {
        return exact_rint(context, ty, operand, caller);
    }
    if let Folded::Id(math) = id
        && let ConstantKind::Float(_) = context.get(operand).kind
    {
        return constant_fold_float_call1(context, math, ty, operand, caller);
    }
    let (value, width) = as_int(context, operand)?;
    let Folded::Id(id) = id else { return None };
    match id {
        IntrinsicId::Bswap if width % 16 == 0 => Some(int(context, width, value.swap_bytes() >> (128 - width))),
        IntrinsicId::Ctpop => {
            let ty_width = context.types.int_bits(ty)?;
            Some(int(context, ty_width, u128::from(value.count_ones())))
        }
        IntrinsicId::Bitreverse => Some(int(context, width, value.reverse_bits() >> (128 - width))),
        _ => None,
    }
}

/// The function attribute a lowering sets when it evaluates float math at the
/// x87's extended precision (BASIC's runtime does); without it a call is
/// evaluated by the host libm at double precision, as the pin folds it.
pub const FP_EVAL: &str = "llrm-fp-eval";
pub const FP_EVAL_EXTENDED: &str = "x87-extended";

/// `ConstantFoldScalarCall1`'s float arms
/// ([L2738](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L2738)
/// rounding,
/// [L2899](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L2899)
/// the host libm).
///
/// MIRROR: the rounding group is exact at any width and folds anywhere; the
/// libm group folds a finite argument whose double result raised no
/// exception (`ConstantFoldFP`: invalid, divide by zero, overflow,
/// underflow; inexact is allowed), floats through double.
///
/// DEVIATE 1: the libm group folds only where the caller is known and does
/// not evaluate at extended precision (`FP_EVAL`): BASIC's `LOG` and `^` are
/// computed in 80 bits by the runtime sequence, and a double constant is a
/// different last digit (rp5, log_exp_digits). The pin has no such function
/// attribute. DEVIATE 2: a NaN or infinite argument of the rounding group is
/// not folded. DEVIATE 3: no `fenv`; the exceptions are read off the result
/// (a non-finite result, or a subnormal or zero one from a nonzero argument).
fn constant_fold_float_call1(
    context: &Context,
    id: IntrinsicId,
    ty: TypeId,
    operand: ConstantId,
    caller: Option<&Function>,
) -> Option<ConstantId> {
    use IntrinsicId::*;
    let Type::Float(kind) = context.types.get(ty) else { return None };
    let ConstantKind::Float(bits) = context.get(operand).kind else { return None };
    let value = match (kind, context.types.get(ty_of(context, operand))) {
        (FloatKind::Double, Type::Float(FloatKind::Double)) => f64::from_bits(bits),
        (FloatKind::Float, Type::Float(FloatKind::Float)) => f64::from(f32::from_bits(bits as u32)),
        _ => return None,
    };
    if !value.is_finite() {
        return None;
    }
    let result = match id {
        Fabs => value.abs(),
        Floor => value.floor(),
        Ceil => value.ceil(),
        Trunc => value.trunc(),
        Round => value.round(),
        Roundeven | Rint | Nearbyint => value.round_ties_even(),
        Sqrt | Sin | Cos | Atan | Sinh | Cosh | Log | Log2 | Log10 | Exp | Exp2 | Exp10 => {
            let host_double = caller.is_some_and(|caller| {
                !caller
                    .attrs
                    .iter()
                    .any(
                        |attr| matches!(
                            attr,
                            Attribute::Str(name, Some(value)) if name == FP_EVAL && value == FP_EVAL_EXTENDED
                        ),
                    )
            });
            if !host_double {
                return None;
            }
            match id {
                Log | Log2 | Log10 if value == 0.0 => return None,
                Log | Log2 | Log10 if value < 0.0 => return None,
                Log | Log2 | Log10 if value == 1.0 => 0.0,
                Atan if value == 0.0 => value,
                Sqrt => value.sqrt(),
                Sin => value.sin(),
                Cos => value.cos(),
                Atan => value.atan(),
                Sinh => value.sinh(),
                Cosh => value.cosh(),
                Log => value.ln(),
                Log2 => value.log2(),
                Log10 => value.log10(),
                Exp => value.exp(),
                Exp2 => 2f64.powf(value),
                Exp10 => 10f64.powf(value),
                _ => return None,
            }
        }
        _ => return None,
    };
    let libm = matches!(
        id,
        Sqrt | Sin | Cos | Atan | Sinh | Cosh | Log | Log2 | Log10 | Exp | Exp2 | Exp10
    );
    if libm && (!result.is_finite() || (result.abs() < f64::MIN_POSITIVE && value != 0.0 && result != 0.0)) {
        return None;
    }
    let bits = match kind {
        FloatKind::Double => result.to_bits(),
        _ => u64::from((result as f32).to_bits()),
    };
    Some(context.constant(Constant { ty, kind: ConstantKind::Float(bits) }))
}

/// `lrint` of an integral float constant that fits `ty`: the integer, whatever
/// the rounding mode (DEVIATE 9).
fn exact_rint(
    context: &Context,
    ty: TypeId,
    operand: ConstantId,
    caller: Option<&Function>,
) -> Option<ConstantId> {
    if matches!(
        context.types.get(ty_of(context, operand)),
        Type::Float(FloatKind::X86Fp80)
    ) {
        return x87_rint(context, ty, operand, caller);
    }
    let value = match (&context.get(operand).kind, context.types.get(ty_of(context, operand))) {
        (ConstantKind::Float(bits), Type::Float(FloatKind::Double)) => f64::from_bits(*bits),
        (ConstantKind::Float(bits), Type::Float(FloatKind::Float)) => f64::from(f32::from_bits(*bits as u32)),
        (ConstantKind::Zero, Type::Float(_)) => 0.0,
        _ => return None,
    };
    let width = context.types.int_bits(ty)?;
    if width > 64 || !value.is_finite() || value.fract() != 0.0 {
        return None;
    }
    let limit = 2f64.powi(width as i32 - 1);
    (-limit <= value && value < limit).then(|| int(context, width, (value as i128 as u128) & mask(width)))
}

/// `lrint` of an `x86_fp80` constant. A caller that evaluates at the x87's
/// extended precision (BASIC's runtime) runs with the control word at round to
/// nearest even, so the integer is the value rounded so; for any other the
/// value must be integral (DEVIATE 9).
fn x87_rint(
    context: &Context,
    ty: TypeId,
    operand: ConstantId,
    caller: Option<&Function>,
) -> Option<ConstantId> {
    let number = crate::fp80::value(&context.get(operand).kind)?;
    let width = context.types.int_bits(ty)?;
    if width > 64 || !number.is_finite() {
        return None;
    }
    let nearest = caller.is_some_and(|caller| {
        caller
            .attrs
            .iter()
            .any(
                |attr| matches!(
                    attr,
                    Attribute::Str(name, Some(value)) if name == FP_EVAL && value == FP_EVAL_EXTENDED
                ),
            )
    });
    if !nearest {
        let mut whole = number;
        whole.round_to_integral(RoundingMode::NearestTiesToEven);
        if whole.compare(&number) != crate::apfloat::CmpResult::Equal {
            return None;
        }
    }
    let value = crate::fp80::to_integer(&number, RoundingMode::NearestTiesToEven, width, true)?;
    Some(int(context, width, value as u128 & mask(width)))
}

/// `APIntOps::pext`/`pdep`.
fn pext(
    value: u128,
    mask_bits: u128,
    width: u32,
) -> u128 {
    let (mut result, mut at) = (0, 0);
    for i in 0..width {
        if mask_bits >> i & 1 == 1 {
            result |= (value >> i & 1) << at;
            at += 1;
        }
    }
    result
}

fn pdep(
    value: u128,
    mask_bits: u128,
    width: u32,
) -> u128 {
    let (mut result, mut at) = (0, 0);
    for i in 0..width {
        if mask_bits >> i & 1 == 1 {
            result |= (value >> at & 1) << i;
            at += 1;
        }
    }
    result
}

/// `ConstantFoldIntrinsicCall2`
/// ([L3491](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L3491)),
/// the integer arm (L3808).
fn constant_fold_intrinsic_call2(
    context: &Context,
    id: Folded,
    ty: TypeId,
    op0: ConstantId,
    op1: ConstantId,
) -> Option<ConstantId> {
    use IntrinsicId::*;
    let Folded::Id(id) = id else { return None };
    // `getConstIntOrUndef` reads a poison as undef: these five fold it to zero.
    if matches!(id, Clmul | Pdep | Pext | Smulh | Umulh)
        && (is_poison(context, op0) || is_poison(context, op1))
        && context.types.int_bits(ty_of(context, op0)).is_some()
        && context.types.int_bits(ty_of(context, op1)).is_some()
    {
        return Some(null_value(context, ty));
    }
    let (c0, width) = as_int(context, op0)?;
    let (c1, _) = as_int(context, op1)?;
    let ty_width = context.types.int_bits(ty);
    let wide = mask(width);
    let (s0, s1) = (signed(c0, width), signed(c1, width));
    let result = |context: &Context, value: u128| ty_width.map(|w| int(context, w, value));
    match id {
        Smax => result(context, if s0 > s1 { c0 } else { c1 }),
        Smin => result(context, if s0 < s1 { c0 } else { c1 }),
        Umax => result(context, if c0 > c1 { c0 } else { c1 }),
        Umin => result(context, if c0 < c1 { c0 } else { c1 }),
        Scmp => result(
            context,
            match s0.cmp(&s1) {
                std::cmp::Ordering::Greater => 1,
                std::cmp::Ordering::Less => u128::MAX,
                std::cmp::Ordering::Equal => 0,
            },
        ),
        Ucmp => result(
            context,
            match c0.cmp(&c1) {
                std::cmp::Ordering::Greater => 1,
                std::cmp::Ordering::Less => u128::MAX,
                std::cmp::Ordering::Equal => 0,
            },
        ),
        SaddWithOverflow | UaddWithOverflow | SsubWithOverflow | UsubWithOverflow | SmulWithOverflow
        | UmulWithOverflow => {
            let (value, overflow) = match id {
                UaddWithOverflow => crate::apint::uadd_ov(c0, c1, width),
                UsubWithOverflow => crate::apint::usub_ov(c0, c1, width),
                UmulWithOverflow => crate::apint::umul_ov(c0, c1, width),
                SmulWithOverflow => crate::apint::smul_ov(c0, c1, width),
                SaddWithOverflow => {
                    let sum = c0.wrapping_add(c1) & wide;
                    let negative = |v: u128| v >> (width - 1) & 1 == 1;
                    (sum, negative(c0) == negative(c1) && negative(sum) != negative(c0))
                }
                _ => {
                    let difference = c0.wrapping_sub(c1) & wide;
                    let negative = |v: u128| v >> (width - 1) & 1 == 1;
                    (difference, negative(c0) != negative(c1) && negative(difference) != negative(c0))
                }
            };
            let members = vec![int(context, width, value), int(context, 1, u128::from(overflow))];
            Some(aggregate(context, ty, members))
        }
        UaddSat => result(context, crate::apint::uadd_sat(c0, c1, width)),
        SaddSat => result(context, crate::apint::sadd_sat(c0, c1, width)),
        UsubSat => result(context, crate::apint::usub_sat(c0, c1, width)),
        SsubSat => result(context, crate::apint::ssub_sat(c0, c1, width)),
        Cttz | Ctlz => {
            if c1 == 1 && c0 == 0 {
                return Some(poison(context, ty));
            }
            let count =
                if id == Cttz { crate::apint::countr_zero(c0, width) } else { crate::apint::countl_zero(c0, width) };
            result(context, u128::from(count))
        }
        Abs => {
            if c1 == 1 && c0 == 1_u128 << (width - 1) {
                return Some(poison(context, ty));
            }
            result(context, s0.unsigned_abs() & wide)
        }
        Clmul => result(context, crate::apint::clmul(c0, c1, width)),
        Pdep => result(context, pdep(c0, c1, width)),
        Pext => result(context, pext(c0, c1, width)),
        Smulh => {
            let (a, b) = (KnownBits::constant(width, c0), KnownBits::constant(width, c1));
            result(context, KnownBits::mulhs(&a, &b).constant_value())
        }
        Umulh => {
            let (a, b) = (KnownBits::constant(width, c0), KnownBits::constant(width, c1));
            result(context, KnownBits::mulhu(&a, &b).constant_value())
        }
        _ => None,
    }
}

/// The high 128 bits of the product of two unsigned 128-bit numbers.
fn mul_high(
    a: u128,
    b: u128,
) -> u128 {
    let low = u128::from(u64::MAX);
    let (a0, a1, b0, b1) = (a & low, a >> 64, b & low, b >> 64);
    let (p00, p01, p10, p11) = (a0 * b0, a0 * b1, a1 * b0, a1 * b1);
    let middle = (p00 >> 64) + (p01 & low) + (p10 & low);
    p11 + (p01 >> 64) + (p10 >> 64) + (middle >> 64)
}

/// The `smul_fix` of L4265: the signed product at twice the width, shifted
/// right by `scale`, cut to `width` bits (`saturate`: clamped to them first).
fn smul_fix(
    c0: u128,
    c1: u128,
    width: u32,
    scale: u32,
    saturate: bool,
) -> u128 {
    let (a, b) = (signed(c0, width), signed(c1, width));
    let low = (a as u128).wrapping_mul(b as u128);
    let mut high = mul_high(a as u128, b as u128);
    if a < 0 {
        high = high.wrapping_sub(b as u128);
    }
    if b < 0 {
        high = high.wrapping_sub(a as u128);
    }
    let (mut high, mut low) = (high as i128, low);
    if scale > 0 {
        low = (low >> scale) | ((high as u128) << (128 - scale));
        high >>= scale;
    }
    if saturate {
        let fits = signed(low & mask(width), width);
        if high != fits >> 127 || low != fits as u128 {
            low = if high < 0 { crate::apint::smin_of(width) } else { crate::apint::smax_of(width) } as u128;
        }
    }
    low & mask(width)
}

/// `ConstantFoldScalarCall3`
/// ([L4149](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L4149)),
/// the integer arms (L4265, L4298).
fn constant_fold_scalar_call3(
    context: &Context,
    id: Folded,
    ty: TypeId,
    ops: &[ConstantId],
) -> Option<ConstantId> {
    let Folded::Id(id) = id else { return None };
    match id {
        IntrinsicId::SmulFix | IntrinsicId::SmulFixSat => {
            let (c0, width) = as_int(context, ops[0])?;
            let (c1, _) = as_int(context, ops[1])?;
            let (scale, _) = as_int(context, ops[2])?;
            let product = smul_fix(c0, c1, width, scale as u32, id == IntrinsicId::SmulFixSat);
            let ty_width = context.types.int_bits(ty)?;
            Some(int(context, ty_width, product))
        }
        // DEVIATE: LLVM folds no `sdiv.fix`, so a front end that emits one for
        // constants leaves a run-time `idiv`. Rounded toward negative infinity,
        // as the backend's expansion is; a zero divisor or a quotient that does
        // not fit is left alone.
        IntrinsicId::SdivFix => {
            let (c0, width) = as_int(context, ops[0])?;
            let (c1, _) = as_int(context, ops[1])?;
            let (scale, _) = as_int(context, ops[2])?;
            let (a, b) = (signed(c0, width), signed(c1, width));
            if b == 0 || scale > 64 || width > 64 {
                return None;
            }
            let numerator = a << scale;
            let mut quotient = numerator / b;
            if numerator % b != 0 && (numerator < 0) != (b < 0) {
                quotient -= 1;
            }
            let ty_width = context.types.int_bits(ty)?;
            let fits = signed(quotient as u128 & mask(width), width) == quotient;
            fits.then(|| int(context, ty_width, quotient as u128 & mask(width)))
        }
        // `fusedMultiplyAdd` to nearest, ties to even, for both: LLVM folds `fmuladd` fused.
        IntrinsicId::Fma | IntrinsicId::Fmuladd => {
            let Type::Float(kind) = context.types.get(ty) else { return None };
            let bits = |operand: ConstantId| match context.get(operand).kind {
                ConstantKind::Float(bits) => Some(bits),
                _ => None,
            };
            let (a, b, c) = (bits(ops[0])?, bits(ops[1])?, bits(ops[2])?);
            let fused = match kind {
                FloatKind::Double => f64::from_bits(a).mul_add(f64::from_bits(b), f64::from_bits(c)).to_bits(),
                FloatKind::Float => u64::from(
                    f32::from_bits(a as u32).mul_add(f32::from_bits(b as u32), f32::from_bits(c as u32)).to_bits(),
                ),
                _ => return None,
            };
            Some(context.constant(Constant { ty, kind: ConstantKind::Float(fused) }))
        }
        IntrinsicId::Fshl | IntrinsicId::Fshr => {
            let (c0, width) = as_int(context, ops[0])?;
            let (c1, _) = as_int(context, ops[1])?;
            let (c2, _) = as_int(context, ops[2])?;
            let is_right = id == IntrinsicId::Fshr;
            let amount = (c2 % u128::from(width)) as u32;
            if amount == 0 {
                return Some(ops[usize::from(is_right)]);
            }
            let lshr_amount = if is_right { amount } else { width - amount };
            let shl_amount = if !is_right { amount } else { width - amount };
            let value = ((c0 << shl_amount) & mask(width)) | (c1 >> lshr_amount);
            let ty_width = context.types.int_bits(ty)?;
            Some(int(context, ty_width, value))
        }
        _ => None,
    }
}

/// `ConstantFoldScalarCall`
/// ([L4334](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L4334)).
fn constant_fold_scalar_call(
    context: &Context,
    id: Folded,
    ty: TypeId,
    ops: &[ConstantId],
    caller: Option<&Function>,
) -> Option<ConstantId> {
    if ops.iter().any(|&one| is_poison(context, one)) && intrinsic_propagates_poison(id) {
        return Some(poison(context, ty));
    }
    match ops.len() {
        1 => constant_fold_scalar_call1(context, id, ty, ops[0], caller),
        2 => constant_fold_intrinsic_call2(context, id, ty, ops[0], ops[1]),
        3 => constant_fold_scalar_call3(context, id, ty, ops),
        _ => None,
    }
}

/// `ConstantFoldIntrinsic`
/// ([L4788](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L4788)):
/// the intrinsic named `name` (`llvm.smax.i32`).
pub fn constant_fold_intrinsic(
    context: &Context,
    name: &str,
    ops: &[ConstantId],
    ty: TypeId,
    caller: Option<&Function>,
) -> Option<ConstantId> {
    let id = folded_intrinsic(name)?;
    if !can_constant_fold_intrinsic(id) || matches!(context.types.get(ty), Type::Vector { .. }) {
        return None;
    }
    constant_fold_scalar_call(context, id, ty, ops, caller)
}

/// `ConstantFoldCall`
/// ([L4803](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L4803)).
pub fn constant_fold_call(
    context: &Context,
    caller: Option<&Function>,
    call: &crate::opcode::CallInfo,
    callee: &Function,
    name: &str,
    ops: &[ConstantId],
) -> Option<ConstantId> {
    let strict_fp = has_flag(&call.attrs, "strictfp");
    if (has_flag(&call.attrs, "nobuiltin") || has_flag(&callee.attrs, "nobuiltin")) && !strict_fp {
        return None;
    }
    let id = folded_intrinsic(name)?;
    let Type::Function { returns, .. } = context.types.get(callee.ty).clone() else { return None };
    if matches!(context.types.get(returns), Type::Vector { .. }) {
        return None;
    }
    constant_fold_scalar_call(context, id, returns, ops, caller)
}
