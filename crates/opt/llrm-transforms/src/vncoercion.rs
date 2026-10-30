//! A value stored or loaded in one type, made the value a load of another type
//! at a must-aliased address reads: GVN's `VNCoercion`, a translation of
//! [`VNCoercion.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp)
//! at d1106deb71cc. The `analyze_*` functions say whether, and at which byte
//! of the write, a load is contained in it; `get_*` and `coerce_*` build the
//! value with an [`Inserter`], which is `IRBuilder` at an insertion point and
//! folds an operation of constants as it does.
//!
//! # How it works
//! A byte offset or `-1` in LLVM is `Option<u64>` here: none is `-1`.
//!
//! DEVIATE 6, the types MIR does not have: no scalable vector, no target
//! extension type, no non-integral pointer (`Type` has none of them, and the
//! datalayout string has no `ni`). The arms of
//! `canCoerceMustAliasedValueToLoad`, `getStoreValueForLoadHelper` and
//! `analyzeLoadFromClobberingMemInst` for them are not written: no `Type` value
//! reaches them.
//!
//! Owed by fold-poc, not a DEVIATE: `ConstantFoldLoadFromConst` and
//! `ConstantFoldLoadFromConstPtr` (ConstantFolding.cpp). Until they land,
//! `analyze_load_from_clobbering_mem_inst` of a `memcpy`/`memmove` (which
//! needs them to accept) is none, `get_constant_value_for_load` is none, and
//! `get_constant_mem_inst_value_for_load` folds only into an integer type (the
//! splat itself). Each is a load GVN then leaves in place.
//!
//! # Example
//! A word stored, its upper half loaded. Before:
//!
//! ```mir
//! store i32 %x, ptr %p
//! %q = getelementptr i8, ptr %p, i16 2
//! %v = load i16, ptr %q
//! ```
//!
//! `analyze_load_from_clobbering_store` says the load starts at byte 2 of the
//! store, and `get_value_for_load` builds what it reads:
//!
//! ```mir
//! %s = lshr i32 %x, 16
//! %v = trunc i32 %s to i16
//! ```
//!
//! From unit test `test_a_half_of_a_word_is_shifted_and_truncated` in
//! `vncoercion_tests.rs`.
//!
//! # Used by
//! Nothing yet; `gvn`'s load elimination (stage 4 of the GVN port) is the
//! caller this is for.

use llrm_mir::constfold;
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Function, InstId, Module, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::pointer_base_with_constant_offset;

/// `IRBuilder<>(InsertPt)`: instructions go before `before`, an operation of
/// constants is a constant.
pub struct Inserter<'a> {
    pub context: &'a mut Context,
    pub function: &'a mut Function,
    pub layout: &'a DataLayout,
    pub before: InstId,
}

impl Inserter<'_> {
    fn type_of(
        &self,
        value: Operand,
    ) -> TypeId {
        self.function.operand_type(self.context, value).expect("a value")
    }

    fn int(
        &mut self,
        bits: u64,
    ) -> TypeId {
        self.context.types.int(bits as u32)
    }

    fn constant(
        &mut self,
        ty: TypeId,
        value: u64,
    ) -> Operand {
        Operand::Constant(self.context.int(ty, i128::from(value)))
    }

    fn emit(
        &mut self,
        opcode: Opcode,
        ty: TypeId,
        operands: Vec<Operand>,
    ) -> Operand {
        let inst = self.function.create_instruction(opcode, ty, operands, Flags::default(), None);
        self.function.insert(inst, Position::Before(self.before)).expect("a placed instruction");
        Operand::Value(self.function.instruction(inst).result.expect("a result"))
    }

    /// `CreateCast`: the value itself when it has the type already.
    fn cast(
        &mut self,
        op: CastOp,
        value: Operand,
        to: TypeId,
    ) -> Operand {
        if self.type_of(value) == to {
            return value;
        }
        if let Operand::Constant(c) = value
            && let Some(folded) = constfold::cast(self.context, op, c, to)
        {
            return Operand::Constant(folded);
        }
        self.emit(Opcode::Cast(op), to, vec![value])
    }

    fn binary(
        &mut self,
        op: BinaryOp,
        a: Operand,
        b: Operand,
    ) -> Operand {
        let ty = self.type_of(a);
        if let (Operand::Constant(x), Operand::Constant(y)) = (a, b)
            && let Some(folded) = constfold::binary(self.context, op, x, y)
        {
            return Operand::Constant(folded);
        }
        self.emit(Opcode::Binary(op), ty, vec![a, b])
    }

    /// `CreateTruncOrBitCast`.
    fn trunc_or_bitcast(
        &mut self,
        value: Operand,
        to: TypeId,
    ) -> Operand {
        let from = self.type_of(value);
        let op = if size_bits(self.context, self.layout, from) > size_bits(self.context, self.layout, to) {
            CastOp::Trunc
        } else {
            CastOp::BitCast
        };
        self.cast(op, value, to)
    }

    /// `CreateZExtOrBitCast`.
    fn zext_or_bitcast(
        &mut self,
        value: Operand,
        to: TypeId,
    ) -> Operand {
        let from = self.type_of(value);
        let op = if size_bits(self.context, self.layout, from) < size_bits(self.context, self.layout, to) {
            CastOp::ZExt
        } else {
            CastOp::BitCast
        };
        self.cast(op, value, to)
    }

    /// `DataLayout::getIntPtrType`: the integer, or vector of integers, as
    /// wide as the pointer, or vector of pointers, `ty`.
    fn int_ptr_type(
        &mut self,
        ty: TypeId,
    ) -> TypeId {
        match self.context.types.get(ty).clone() {
            Type::Pointer(space) => self.int(u64::from(self.layout.pointer(space).bits)),
            Type::Vector { element, count } => {
                let element = self.int_ptr_type(element);
                self.context.types.intern(Type::Vector { element, count })
            }
            other => panic!("{other:?} is not a pointer type"),
        }
    }
}

fn size_bits(
    context: &Context,
    layout: &DataLayout,
    ty: TypeId,
) -> u64 {
    layout.size_bits(&context.types, ty)
}

fn is_first_class_aggregate(
    context: &Context,
    ty: TypeId,
) -> bool {
    matches!(
        context.types.get(ty),
        Type::Struct { .. } | Type::Named(_) | Type::Array { .. }
    )
}

fn is_ptr_or_ptr_vector(
    context: &Context,
    ty: TypeId,
) -> bool {
    match context.types.get(ty) {
        Type::Pointer(_) => true,
        Type::Vector { element, .. } => matches!(context.types.get(*element), Type::Pointer(_)),
        _ => false,
    }
}

/// A `memset`, `memcpy` or `memmove` call: LLVM's `MemIntrinsic`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemIntrinsic {
    Set { dest: Operand, value: Operand, length: Operand },
    Transfer { dest: Operand, source: Operand, length: Operand },
}

impl MemIntrinsic {
    /// `inst` of `function`, which is a body of `module`, as one.
    pub fn of(
        module: &Module,
        function: &Function,
        inst: InstId,
    ) -> Option<Self> {
        Self::of_in(&module.context, &module.globals, function, inst)
    }

    /// `inst` of `function` as one, where `globals` name the callees.
    pub fn of_in(
        context: &Context,
        globals: &[llrm_mir::module::GlobalValue],
        function: &Function,
        inst: InstId,
    ) -> Option<Self> {
        let instruction = function.instruction(inst);
        let Some(&Operand::Constant(callee)) = instruction.operands.last() else { return None };
        let ConstantKind::Global(callee) = context.get(callee).kind else { return None };
        let [dest, second, length, ..] = instruction.operands[..] else { return None };
        match globals.get(callee.0 as usize)?.intrinsic()? {
            Intrinsic::MemSet => Some(Self::Set { dest, value: second, length }),
            Intrinsic::MemCpy | Intrinsic::MemMove => Some(Self::Transfer { dest, source: second, length }),
            _ => None,
        }
    }
}

/// `canCoerceMustAliasedValueToLoad`
/// ([VNCoercion.cpp:17](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L17)).
pub fn can_coerce_must_aliased_value_to_load(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    stored: Operand,
    load_ty: TypeId,
) -> bool {
    let stored_ty = function.operand_type(context, stored).expect("a value");
    if stored_ty == load_ty {
        return true;
    }
    let min_store_size = size_bits(context, layout, stored_ty);
    let load_size = size_bits(context, layout, load_ty);

    // If the loaded/stored value is a first class array/struct, don't try to
    // transform them. We need to be able to bitcast to integer.
    if is_first_class_aggregate(context, load_ty) || is_first_class_aggregate(context, stored_ty) {
        return false;
    }
    // The store size must be byte-aligned to support future type casts.
    if !min_store_size.is_multiple_of(8) {
        return false;
    }
    // The store has to be at least as big as the load.
    min_store_size >= load_size
}

/// `coerceAvailableValueToLoadType`
/// ([VNCoercion.cpp:90](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L90)).
pub fn coerce_available_value_to_load_type(
    helper: &mut Inserter,
    mut stored_val: Operand,
    loaded_ty: TypeId,
) -> Operand {
    assert!(
        can_coerce_must_aliased_value_to_load(helper.context, helper.layout, helper.function, stored_val, loaded_ty),
        "precondition violation - materialization can't fail"
    );
    let mut stored_val_ty = helper.type_of(stored_val);
    let stored_val_size = size_bits(helper.context, helper.layout, stored_val_ty);
    let loaded_val_size = size_bits(helper.context, helper.layout, loaded_ty);

    // If the store and reload are the same size, we can always reuse it.
    if stored_val_size == loaded_val_size {
        let stored_ptr = is_ptr_or_ptr_vector(helper.context, stored_val_ty);
        let loaded_ptr = is_ptr_or_ptr_vector(helper.context, loaded_ty);
        // Pointer to Pointer -> use bitcast.
        if stored_ptr && loaded_ptr {
            stored_val = helper.cast(CastOp::BitCast, stored_val, loaded_ty);
        } else {
            // Convert source pointers to integers, which can be bitcast.
            if stored_ptr {
                stored_val_ty = helper.int_ptr_type(stored_val_ty);
                stored_val = helper.cast(CastOp::PtrToInt, stored_val, stored_val_ty);
            }
            let mut type_to_cast_to = loaded_ty;
            if loaded_ptr {
                type_to_cast_to = helper.int_ptr_type(type_to_cast_to);
            }
            if stored_val_ty != type_to_cast_to {
                stored_val = helper.cast(CastOp::BitCast, stored_val, type_to_cast_to);
            }
            // Cast to pointer if the load needs a pointer type.
            if loaded_ptr {
                stored_val = helper.cast(CastOp::IntToPtr, stored_val, loaded_ty);
            }
        }
        return stored_val;
    }

    // If the loaded value is smaller than the available value, then we can
    // extract out a piece from it.
    assert!(stored_val_size >= loaded_val_size, "can_coerce_must_aliased_value_to_load fail");

    // Convert source pointers to integers, which can be manipulated.
    if is_ptr_or_ptr_vector(helper.context, stored_val_ty) {
        stored_val_ty = helper.int_ptr_type(stored_val_ty);
        stored_val = helper.cast(CastOp::PtrToInt, stored_val, stored_val_ty);
    }
    // Convert vectors and fp to integer, which can be manipulated.
    if helper.context.types.int_bits(stored_val_ty).is_none() {
        stored_val_ty = helper.int(stored_val_size);
        stored_val = helper.cast(CastOp::BitCast, stored_val, stored_val_ty);
    }
    // If this is a big-endian system, we need to shift the value down to the
    // low bits so that a truncate will work.
    if helper.layout.big_endian {
        let shift_amt = helper.layout.store_size(&helper.context.types, stored_val_ty) * 8
            - helper.layout.store_size(&helper.context.types, loaded_ty) * 8;
        let ty = helper.type_of(stored_val);
        let amount = helper.constant(ty, shift_amt);
        stored_val = helper.binary(BinaryOp::LShr, stored_val, amount);
    }
    // Truncate the integer to the right size now.
    let new_int_ty = helper.int(loaded_val_size);
    stored_val = helper.trunc_or_bitcast(stored_val, new_int_ty);

    if loaded_ty != new_int_ty {
        // If the result is a pointer, inttoptr; otherwise, bitcast.
        let op = if is_ptr_or_ptr_vector(helper.context, loaded_ty) { CastOp::IntToPtr } else { CastOp::BitCast };
        stored_val = helper.cast(op, stored_val, loaded_ty);
    }
    stored_val
}

/// `analyzeLoadFromClobberingWrite`
/// ([VNCoercion.cpp:198](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L198)):
/// the byte of the write the load starts at, when the write holds all of it.
fn analyze_load_from_clobbering_write(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    load_ty: TypeId,
    load_ptr: Operand,
    write_ptr: Operand,
    write_size_in_bits: u64,
) -> Option<u64> {
    // If the loaded/stored value is a first class array/struct, don't try to
    // transform them. We need to be able to bitcast to integer.
    if is_first_class_aggregate(context, load_ty) {
        return None;
    }
    let (store_base, store_offset) = pointer_base_with_constant_offset(context, layout, function, write_ptr);
    let (load_base, load_offset) = pointer_base_with_constant_offset(context, layout, function, load_ptr);
    if store_base != load_base {
        return None;
    }
    let load_size = size_bits(context, layout, load_ty);
    if (write_size_in_bits & 7) | (load_size & 7) != 0 {
        return None;
    }
    let store_size = (write_size_in_bits / 8) as i64;
    let load_size = (load_size / 8) as i64;

    // If the Load isn't completely contained within the stored bits, we don't
    // have all the bits to feed it.
    if store_offset > load_offset || store_offset + store_size < load_offset + load_size {
        return None;
    }
    Some((load_offset - store_offset) as u64)
}

/// `analyzeLoadFromClobberingStore`
/// ([VNCoercion.cpp:236](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L236)).
pub fn analyze_load_from_clobbering_store(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    load_ty: TypeId,
    load_ptr: Operand,
    dep_si: InstId,
) -> Option<u64> {
    let store = function.instruction(dep_si);
    let (stored_val, store_ptr) = (store.operands[0], store.operands[1]);
    let stored_ty = function.operand_type(context, stored_val).expect("a value");

    // Cannot handle reading from store of first-class aggregate.
    if is_first_class_aggregate(context, stored_ty) {
        return None;
    }
    if !can_coerce_must_aliased_value_to_load(context, layout, function, stored_val, load_ty) {
        return None;
    }
    let store_size = size_bits(context, layout, stored_ty);
    analyze_load_from_clobbering_write(context, layout, function, load_ty, load_ptr, store_ptr, store_size)
}

/// `analyzeLoadFromClobberingLoad`
/// ([VNCoercion.cpp:258](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L258)).
pub fn analyze_load_from_clobbering_load(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    load_ty: TypeId,
    load_ptr: Operand,
    dep_li: InstId,
) -> Option<u64> {
    let dep = function.instruction(dep_li);
    // Cannot handle reading from store of first-class aggregate.
    if is_first_class_aggregate(context, dep.ty) {
        return None;
    }
    let dep_val = Operand::Value(dep.result.expect("a load has a result"));
    if !can_coerce_must_aliased_value_to_load(context, layout, function, dep_val, load_ty) {
        return None;
    }
    let dep_size = size_bits(context, layout, dep.ty);
    analyze_load_from_clobbering_write(context, layout, function, load_ty, load_ptr, dep.operands[0], dep_size)
}

/// The value of a constant integer length or count.
fn constant_length(
    context: &Context,
    length: Operand,
) -> Option<u64> {
    let Operand::Constant(id) = length else { return None };
    match context.get(id).kind {
        ConstantKind::Int(bits) => u64::try_from(bits).ok(),
        ConstantKind::Zero | ConstantKind::Null => Some(0),
        _ => None,
    }
}

/// `analyzeLoadFromClobberingMemInst`
/// ([VNCoercion.cpp:273](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L273)).
pub fn analyze_load_from_clobbering_mem_inst(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    load_ty: TypeId,
    load_ptr: Operand,
    mi: MemIntrinsic,
) -> Option<u64> {
    match mi {
        MemIntrinsic::Set { dest, length, .. } => {
            // If the mem operation is a non-constant size, we can't handle it.
            let mem_size_in_bits = constant_length(context, length)?.checked_mul(8)?;
            // A memset needs only that the offset is valid in the size of it.
            analyze_load_from_clobbering_write(context, layout, function, load_ty, load_ptr, dest, mem_size_in_bits)
        }
        // Owed by fold-poc: accepting a copy from constant memory needs
        // `ConstantFoldLoadFromConstPtr`.
        MemIntrinsic::Transfer { .. } => None,
    }
}

/// `getStoreValueForLoadHelper`
/// ([VNCoercion.cpp:321](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L321)).
fn get_store_value_for_load_helper(
    builder: &mut Inserter,
    mut src_val: Operand,
    offset: u64,
    load_ty: TypeId,
) -> Operand {
    let src_ty = builder.type_of(src_val);
    // If two pointers are in the same address space, they have the same size,
    // so we don't need to do any truncation, etc. This avoids introducing
    // ptrtoint instructions for pointers that may be non-integral.
    if let (Type::Pointer(a), Type::Pointer(b)) =
        (builder.context.types.get(src_ty), builder.context.types.get(load_ty))
        && a == b
    {
        return src_val;
    }
    let store_size = size_bits(builder.context, builder.layout, src_ty).div_ceil(8);
    let load_size = size_bits(builder.context, builder.layout, load_ty).div_ceil(8);

    // Compute which bits of the stored value are being used by the load.
    // Convert to an integer type to start with.
    if is_ptr_or_ptr_vector(builder.context, src_ty) {
        let to = builder.int_ptr_type(src_ty);
        src_val = builder.cast(CastOp::PtrToInt, src_val, to);
    }
    let src_ty = builder.type_of(src_val);
    if builder.context.types.int_bits(src_ty).is_none() {
        let to = builder.int(store_size * 8);
        src_val = builder.cast(CastOp::BitCast, src_val, to);
    }
    // Shift the bits to the least significant depending on endianness.
    let shift_amt = if builder.layout.big_endian { (store_size - load_size - offset) * 8 } else { offset * 8 };
    if shift_amt != 0 {
        let ty = builder.type_of(src_val);
        let amount = builder.constant(ty, shift_amt);
        src_val = builder.binary(BinaryOp::LShr, src_val, amount);
    }
    if load_size != store_size {
        let to = builder.int(load_size * 8);
        src_val = builder.trunc_or_bitcast(src_val, to);
    }
    src_val
}

/// `getValueForLoad`
/// ([VNCoercion.cpp:380](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L380)).
pub fn get_value_for_load(
    builder: &mut Inserter,
    src_val: Operand,
    offset: u64,
    load_ty: TypeId,
) -> Operand {
    let src_ty = builder.type_of(src_val);
    let src_size = builder.layout.store_size(&builder.context.types, src_ty);
    let load_size = builder.layout.store_size(&builder.context.types, load_ty);
    assert!(offset + load_size <= src_size, "Expected Offset + LoadSize <= SrcValSize");
    let src_val = get_store_value_for_load_helper(builder, src_val, offset, load_ty);
    coerce_available_value_to_load_type(builder, src_val, load_ty)
}

/// `getConstantValueForLoad`
/// ([VNCoercion.cpp:401](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L401)).
/// Owed by fold-poc (`ConstantFoldLoadFromConst`): none until it lands.
pub fn get_constant_value_for_load(
    context: &Context,
    layout: &DataLayout,
    src_ty: TypeId,
    offset: u64,
    load_ty: TypeId,
) -> Option<Operand> {
    let src_size = layout.store_size(&context.types, src_ty);
    let load_size = layout.store_size(&context.types, load_ty);
    assert!(offset + load_size <= src_size);
    None
}

/// `getMemInstValueForLoad`
/// ([VNCoercion.cpp:414](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L414)):
/// the value of a load fully inside a `memset`; a transfer is owed by fold-poc.
pub fn get_mem_inst_value_for_load(
    builder: &mut Inserter,
    src_inst: MemIntrinsic,
    load_ty: TypeId,
) -> Option<Operand> {
    let MemIntrinsic::Set { value, .. } = src_inst else { return None };
    let load_size = size_bits(builder.context, builder.layout, load_ty) / 8;

    // memset(P, 'x', 1234) -> splat('x'), even if x is a variable, and
    // independently of what the offset is.
    let mut val = value;
    if load_size != 1 {
        let to = builder.int(load_size * 8);
        val = builder.zext_or_bitcast(val, to);
    }
    let one_elt = val;

    // Splat the value out to the right number of bits.
    let mut num_bytes_set = 1;
    while num_bytes_set != load_size {
        let ty = builder.type_of(val);
        // If we can double the number of bytes set, do it.
        if num_bytes_set * 2 <= load_size {
            let by = builder.constant(ty, num_bytes_set * 8);
            let sh_val = builder.binary(BinaryOp::Shl, val, by);
            val = builder.binary(BinaryOp::Or, val, sh_val);
            num_bytes_set <<= 1;
            continue;
        }
        // Otherwise insert one byte at a time.
        let by = builder.constant(ty, 8);
        let sh_val = builder.binary(BinaryOp::Shl, val, by);
        val = builder.binary(BinaryOp::Or, one_elt, sh_val);
        num_bytes_set += 1;
    }
    Some(coerce_available_value_to_load_type(builder, val, load_ty))
}

/// `getConstantMemInstValueForLoad`
/// ([VNCoercion.cpp:463](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp#L463)):
/// a `memset` of a constant byte, into an integer type (owed by fold-poc).
pub fn get_constant_mem_inst_value_for_load(
    context: &Context,
    layout: &DataLayout,
    src_inst: MemIntrinsic,
    load_ty: TypeId,
) -> Option<Operand> {
    let MemIntrinsic::Set { value, .. } = src_inst else { return None };
    let load_size = size_bits(context, layout, load_ty) / 8;
    let Operand::Constant(byte) = value else { return None };
    let byte = match context.get(byte).kind {
        ConstantKind::Int(bits) => bits as u8,
        ConstantKind::Zero | ConstantKind::Null => 0,
        _ => return None,
    };
    let width = context.types.int_bits(load_ty).filter(|&width| width <= 128)?;
    let splat = (0..load_size).fold(0_u128, |splat, _| (splat << 8) | u128::from(byte));
    Some(Operand::Constant(
        context.constant(Constant { ty: load_ty, kind: ConstantKind::Int(splat & llrm_mir::context::mask(width)) }),
    ))
}
