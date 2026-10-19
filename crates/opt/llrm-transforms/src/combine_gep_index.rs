//! The rewrites of `visitGetElementPtrInst` that read its indices and flags
//! (:3513-3705): a scaled single index, `inbounds` and `nuw` the object or the
//! indices prove, a sum split in two GEPs, a select of constants, and `srem`
//! as `urem`.
//!
//! Translation of
//! [`InstCombinerImpl::visitGetElementPtrInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L3513)
//! and `foldSelectGEP` (:2759) at d1106deb71cc. Each is a `Fold` of
//! `combine_gep::FOLDS`.
//!
//! DEVIATE 1: `ptrtoaddr` is not a MIR instruction: only `ptrtoint` users and
//! compares take the pointer a difference of pointers rebuilds.
//!
//! # Example
//! ```text
//! %i = add i64 %x, %y
//! %p = getelementptr i32, ptr %b, i64 %i         %q = getelementptr [4 x i8], ptr %b, i64 %x
//!                                          ==>   %p = getelementptr [4 x i8], ptr %q, i64 %y
//! ```

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::valuetracking::known::{is_known_non_negative, pointer_dereferenceable_bytes};
use llrm_mir::valuetracking::nonzero::strip_and_accumulate_constant_offsets;
use llrm_mir::valuetracking::power2::is_known_to_be_a_power_of_two;
use llrm_mir::valuetracking::{Query, pattern};

use crate::combine::{Combiner, Visit};
use crate::combine_gep::{Gep, GepFlags};
use crate::combine_shared::{has_one_use, query};

/// The `idx_begin()` of a GEP with one index.
fn only_index(gep: &Gep) -> Option<Operand> {
    match gep.indices[..] {
        [index] => Some(index),
        _ => None,
    }
}

impl Combiner<'_> {
    /// `v` as `op a, b`: its instruction, operands and flags.
    fn binary_parts(
        &self,
        v: Operand,
    ) -> Option<(InstId, BinaryOp, Operand, Operand, Flags)> {
        let def = self.definition_of(v)?;
        let instruction = self.function.instruction(def);
        let Opcode::Binary(op) = instruction.opcode else { return None };
        let [a, b] = instruction.operands[..] else { return None };
        Some((def, op, a, b, instruction.flags))
    }

    /// `v` as `op x`: the cast's operand.
    fn cast_parts(
        &self,
        v: Operand,
    ) -> Option<(CastOp, Operand, Flags)> {
        let instruction = self.function.instruction(self.definition_of(v)?);
        let Opcode::Cast(op) = instruction.opcode else { return None };
        Some((op, instruction.operands[0], instruction.flags))
    }

    /// The value of the constant integer `v` truncated to its width.
    fn is_specific_int(
        &self,
        v: Operand,
        value: u64,
    ) -> bool {
        self.constant_int(v) == Some(u128::from(value))
    }
}

/// Canonicalizes a scaled single index: `gep i8, X, (ptrtoint Y) - (ptrtoint
/// X)` is `Y` for the users that read it as an integer; `gep T, X, V /
/// sizeof(T)` is `gep i8, X, V` when the division is exact.
pub(crate) fn scaled_single_index(
    c: &mut Combiner,
    inst: InstId,
    gep: &Gep,
) -> Option<Visit> {
    let index = only_index(gep)?;
    let space = match c.context.types.get(gep.ty) {
        llrm_mir::types::Type::Pointer(space) => *space,
        _ => return None,
    };
    let pointer = c.env.layout.pointer(space);
    if c.context.types.int_bits(c.type_of(index)) != Some(pointer.index_bits) {
        return None;
    }
    let size = c.alloc_size(gep.source);
    if size == 1 {
        // (ptrtoint Y) - (ptrtoint X), X the base
        let (_, BinaryOp::Sub, lhs, rhs, _) = c.binary_parts(index)? else { return None };
        let (CastOp::PtrToInt, y, _) = c.cast_parts(lhs)? else { return None };
        let (CastOp::PtrToInt, x, _) = c.cast_parts(rhs)? else { return None };
        if x != gep.ptr || c.type_of(y) != gep.ty {
            return Some(Visit::None);
        }
        // The users that read the pointer as an integer read `Y` instead.
        let has_non_address_bits = pointer.index_bits != pointer.bits;
        let Operand::Value(result) = Operand::Value(c.function.instruction(inst).result?) else { return None };
        let users: Vec<(InstId, usize)> =
            c.function.users(result).iter().map(|one| (one.user, one.index as usize)).collect();
        let mut changed = false;
        for (user, slot) in users {
            let reads_as_integer = match c.function.instruction(user).opcode {
                Opcode::ICmp(_) => true,
                Opcode::Cast(CastOp::PtrToInt) => !has_non_address_bits,
                _ => false,
            };
            if reads_as_integer {
                c.function.set_operand(user, slot, y);
                c.worklist.push(user);
                changed = true;
            }
        }
        return Some(if changed { Visit::Changed } else { Visit::None });
    }

    let (def, op, value, divisor, flags) = c.binary_parts(index)?;
    if !flags.contains(Flags::EXACT) || !matches!(
        op,
        BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::LShr | BinaryOp::AShr
    )
    {
        return None;
    }
    let byte = c.context.types.int(8);
    let shift = u64::from(size.trailing_zeros());
    let is_shift = matches!(op, BinaryOp::LShr | BinaryOp::AShr);
    let is_div = matches!(op, BinaryOp::UDiv | BinaryOp::SDiv);
    // gep T* X, V / sizeof(T) is gep i8* X, V
    if (size.is_power_of_two() && is_shift && c.is_specific_int(divisor, shift))
        || (is_div && c.is_specific_int(divisor, size))
    {
        let operands = vec![gep.ptr, value];
        return Some(c.replace_with_new(inst, Opcode::GetElementPtr { source: byte }, gep.ty, operands, gep.flags));
    }
    // With one use the scale folds into the division: V / C is V / (C /
    // sizeof(T)) when that divides.
    if !has_one_use(c, index) {
        return None;
    }
    let (constant, width) = (c.constant_int(divisor)?, c.context.types.int_bits(c.type_of(divisor))?);
    let new_constant = if size.is_power_of_two() && is_shift && constant >= u128::from(shift) {
        Some(constant - u128::from(shift))
    } else if op == BinaryOp::UDiv {
        (constant % u128::from(size) == 0).then(|| constant / u128::from(size))
    } else if op == BinaryOp::SDiv {
        let signed = llrm_mir::context::signed(constant, width);
        let quotient = signed / i128::from(size);
        // An all-ones quotient would be INT_MIN / -1 undone.
        (signed % i128::from(size) == 0 && quotient != -1).then_some(quotient as u128)
    } else {
        None
    }?;
    let _ = def;
    let ty = c.type_of(value);
    let new_constant = c.int(ty, new_constant);
    let scaled = c.create_binop(inst, op, value, new_constant, Flags::EXACT);
    let operands = vec![gep.ptr, scaled];
    Some(c.replace_with_new(inst, Opcode::GetElementPtr { source: byte }, gep.ty, operands, gep.flags))
}

/// A GEP whose constant offset stays within the object it starts in is
/// `inbounds`.
pub(crate) fn infer_in_bounds(
    c: &mut Combiner,
    inst: InstId,
    gep: &Gep,
) -> Option<Visit> {
    if GepFlags::of(gep.flags).in_bounds {
        return None;
    }
    let width = c.context.types.int_bits(c.index_type(gep.ty))?;
    let q = query(c, None);
    let (underlying, base_offset) = strip_and_accumulate_constant_offsets(&q, gep.ptr, width, false);
    let (bytes, can_be_null) = pointer_dereferenceable_bytes(&q, underlying);
    if can_be_null || bytes == 0 {
        return None;
    }
    let constants: Vec<_> = gep
        .indices
        .iter()
        .map(|&index| match index {
            Operand::Constant(id) => Some(id),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let offset =
        llrm_mir::constantfolding::accumulate_constant_offset(c.context, c.env.layout, gep.source, &constants, width)?;
    let mask = if width == 128 { u128::MAX } else { (1u128 << width) - 1 };
    let total = base_offset.wrapping_add(offset) & mask;
    // Not negative, and at most the object's size.
    if total >> (width - 1) != 0 || total > u128::from(bytes) {
        return None;
    }
    let operands = std::iter::once(gep.ptr).chain(gep.indices.iter().copied()).collect();
    let flags = GepFlags { in_bounds: true, nusw: true, nuw: false }.flags();
    Some(c.replace_with_new(inst, Opcode::GetElementPtr { source: gep.source }, gep.ty, operands, flags))
}

/// `nusw` with only non-negative indices is `nuw`.
pub(crate) fn nusw_nneg_is_nuw(
    c: &mut Combiner,
    inst: InstId,
    gep: &Gep,
) -> Option<Visit> {
    let nw = GepFlags::of(gep.flags);
    if !nw.nusw || nw.nuw {
        return None;
    }
    let q = query(c, Some(inst));
    if !gep.indices.iter().all(|&index| is_known_non_negative(&q, index, 0)) {
        return None;
    }
    let mut flags = gep.flags;
    flags.insert(Flags::NUW);
    c.function.set_flags(inst, flags);
    Some(Visit::Changed)
}

/// `gep p, x+y` is `gep (gep p, x), y`, keeping `nuw` where the sum has it.
pub(crate) fn split_added_index(
    c: &mut Combiner,
    inst: InstId,
    gep: &Gep,
) -> Option<Visit> {
    let index = only_index(gep)?;
    if !has_one_use(c, index) {
        return None;
    }
    let nw = GepFlags::of(gep.flags);
    // The flags of both new GEPs: all of the GEP's if x + y cannot wrap
    // unsigned, else none.
    let preserved = |add_is_nuw: bool| if nw.nuw && add_is_nuw { gep.flags } else { Flags::default() };
    let q = query(c, Some(inst));
    if let Some((idx1, idx2)) = pattern::add_like(&q, index, Flags::default()) {
        let flags = preserved(pattern::add_like(&q, index, Flags::NUW).is_some());
        let near = c.create_gep(inst, gep.source, gep.ptr, &[idx1], flags);
        return Some(Visit::Replace(c.create_gep(inst, gep.source, near, &[idx2], flags)));
    }
    // sext (add nsw x, C) is gep (gep p, sext x), sext C.
    let (cast, sum, cast_flags) = c.cast_parts(index)?;
    let sext_like = cast == CastOp::SExt || (cast == CastOp::ZExt && cast_flags.contains(Flags::NNEG));
    if !sext_like || !has_one_use(c, sum) {
        return None;
    }
    let (idx1, constant) = pattern::add_like(&q, sum, Flags::NSW)?;
    c.constant_int(constant)?;
    let add_is_nuw = cast == CastOp::ZExt && pattern::add_like(&q, sum, Flags::NUW).is_some();
    let flags = preserved(add_is_nuw);
    let ty = c.type_of(index);
    let wide = c.create_cast(inst, CastOp::SExt, idx1, ty, Flags::default());
    let near = c.create_gep(inst, gep.source, gep.ptr, &[wide], flags);
    let wide_constant = c.create_cast(inst, CastOp::SExt, constant, ty, Flags::default());
    Some(Visit::Replace(c.create_gep(inst, gep.source, near, &[wide_constant], flags)))
}

/// `foldSelectGEP`: a GEP of constant indices through a select of constants
/// is a select of the GEP of each.
pub(crate) fn select_of_constants(
    c: &mut Combiner,
    inst: InstId,
    gep: &Gep,
) -> Option<Visit> {
    if !gep.indices.iter().all(|&index| c.constant_int(index).is_some()) {
        return None;
    }
    let (select, [cond, if_true, if_false]) = c.select_of(gep.ptr)?;
    if !matches!(
        (if_true, if_false),
        (Operand::Constant(_), Operand::Constant(_))
    ) {
        return None;
    }
    let on_true = c.create_gep(inst, gep.source, if_true, &gep.indices, gep.flags);
    let on_false = c.create_gep(inst, gep.source, if_false, &gep.indices, gep.flags);
    Some(Visit::Replace(c.select_like(inst, select, [cond, on_true, on_false], Flags::default())))
}

/// An `inbounds nuw` index `srem x, 2^k` cannot be negative: `urem`.
pub(crate) fn srem_is_urem(
    c: &mut Combiner,
    inst: InstId,
    gep: &Gep,
) -> Option<Visit> {
    let nw = GepFlags::of(gep.flags);
    let index = only_index(gep)?;
    if !(nw.in_bounds && nw.nuw) || !has_one_use(c, index) {
        return None;
    }
    let (_, BinaryOp::SRem, x, y, _) = c.binary_parts(index)? else { return None };
    let q: Query = query(c, Some(inst));
    if !is_known_to_be_a_power_of_two(&q, y, true, 0) {
        return None;
    }
    let new_index = c.create_binop(inst, BinaryOp::URem, x, y, Flags::default());
    let operands = vec![gep.ptr, new_index];
    Some(c.replace_with_new(inst, Opcode::GetElementPtr { source: gep.source }, gep.ty, operands, gep.flags))
}
