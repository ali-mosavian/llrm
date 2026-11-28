//! InstCombine's pointer-integer casts: `visitIntToPtr` and `visitPtrToInt`
//! ([InstCombineCasts.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp#L2673)).
//!
//! # How it works
//! A cast through an integer of the wrong width goes through the pointer-sized
//! integer first, so later folds see it. `inttoptr (ptrtoint p + n)` that is
//! only compared or turned back into an integer is `p + n` as a GEP, and
//! `ptrtoint` of a one-use GEP chain on `null` or on an `inttoptr` is integer
//! arithmetic.
//!
//! # Example
//! ```text
//! %i = ptrtoint ptr %p to i64              ==>   %q = getelementptr i8, ptr %p, i64 %n
//! %a = add i64 %i, %n
//! %q = inttoptr i64 %a to ptr              ; when %q is only compared or ptrtoint'ed
//! ```
//!
//! MIRROR: the order of the folds and each guard.
//!
//! DEVIATE 1: the `ptrmask` fold of `visitPtrToInt` (no `llvm.ptrmask`
//! intrinsic in MIR) and the vector `insertelement` fold are not translated.
//! Exit 2026-11-12.
//!
//! # Used by
//! `combine` (its dispatch on `PtrToInt` and `IntToPtr`).

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_casts::common_cast_transforms;
use crate::combine_shared::query;

impl Combiner<'_> {
    fn address_space(
        &self,
        ty: TypeId,
    ) -> u32 {
        match self.context.types.get(ty) {
            Type::Pointer(space) => *space,
            _ => 0,
        }
    }

    fn pointer_bits(
        &self,
        space: u32,
    ) -> u32 {
        self.env.layout.pointer(space).bits
    }

    /// `Builder.CreateZExtOrTrunc(v, ty)` with a result of the new width.
    fn zext_or_trunc(
        &mut self,
        at: InstId,
        v: Operand,
        ty: TypeId,
    ) -> Operand {
        let (from, to) = (self.int_width(v), self.context.types.int_bits(ty).expect("an integer"));
        match from.cmp(&to) {
            std::cmp::Ordering::Less => self.create_cast(at, CastOp::ZExt, v, ty, Flags::default()),
            std::cmp::Ordering::Greater => self.create_cast(at, CastOp::Trunc, v, ty, Flags::default()),
            std::cmp::Ordering::Equal => v,
        }
    }

    fn int_width(
        &self,
        v: Operand,
    ) -> u32 {
        self.context.types.int_bits(self.type_of(v)).expect("an integer")
    }

    /// `visitIntToPtr` (:2673).
    pub(crate) fn visit_int_to_ptr(
        &mut self,
        cast: InstId,
    ) -> Visit {
        let src = self.function.instruction(cast).operands[0];
        let ty = self.function.instruction(cast).ty;
        let space = self.address_space(ty);
        // Not the intptr_t width: zext or trunc to it first.
        if self.int_width(src) != self.pointer_bits(space) {
            let int_ptr = self.context.types.int(self.pointer_bits(space));
            let widened = self.zext_or_trunc(cast, src, int_ptr);
            return self.replace_with_new(cast, Opcode::Cast(CastOp::IntToPtr), ty, vec![widened], Flags::default());
        }
        // inttoptr (add (ptrtoint %Base), %Offset) -> gep i8, %Base, %Offset
        // when the pointer is only used as an integer.
        let q = query(self, Some(cast));
        if let Some((_, _, lhs, rhs)) = pattern::binary(&q, src, BinaryOp::Add)
            && self.has_one_use(src)
        {
            for (ptr_int, offset) in [(lhs, rhs), (rhs, lhs)] {
                let Some((_, instruction)) = q.instruction_of(ptr_int) else { continue };
                if instruction.opcode != Opcode::Cast(CastOp::PtrToInt) {
                    continue;
                }
                let base = instruction.operands[0];
                let same_size = self.int_width(ptr_int) == self.pointer_bits(self.address_space(self.type_of(base)));
                if same_size && self.address_space(self.type_of(base)) == space && self.uses_pointer_as_int(cast) {
                    let byte = self.context.types.int(8);
                    return self.replace_with_new(
                        cast,
                        Opcode::GetElementPtr { source: byte },
                        ty,
                        vec![base, offset],
                        Flags::default(),
                    );
                }
            }
        }
        common_cast_transforms(self, cast).unwrap_or(Visit::None)
    }

    /// `all_of(CI.users(), UsesPointerAsInt)`.
    fn uses_pointer_as_int(
        &self,
        cast: InstId,
    ) -> bool {
        let result = self.function.instruction(cast).result.expect("a value");
        let is_int_use = |user: InstId| {
            matches!(
                self.function.instruction(user).opcode,
                Opcode::ICmp(_) | Opcode::Cast(CastOp::PtrToInt)
            )
        };
        self.function
            .users(result)
            .iter()
            .all(
                |used| {
                    let user = used.user;
                    if is_int_use(user) {
                        return true;
                    }
                    if self.function.instruction(user).opcode == Opcode::Phi {
                        let phi = self.function.instruction(user).result.expect("a value");
                        return self.function.users(phi).only().is_some_and(|only| is_int_use(only.user));
                    }
                    false
                },
            )
    }

    /// `visitPtrToInt` (:2749).
    pub(crate) fn visit_ptr_to_int(
        &mut self,
        cast: InstId,
    ) -> Visit {
        let src = self.function.instruction(cast).operands[0];
        let ty = self.function.instruction(cast).ty;
        let space = self.address_space(self.type_of(src));
        let (size, ptr_size) = (self.context.types.int_bits(ty).expect("an integer"), self.pointer_bits(space));
        // Not the intptr_t width: ptrtoint to it, then zext or trunc.
        if size != ptr_size {
            let int_ptr = self.context.types.int(ptr_size);
            let pointer = self.create_cast(cast, CastOp::PtrToInt, src, int_ptr, Flags::default());
            let resized = self.zext_or_trunc(cast, pointer, ty);
            return Visit::Replace(resized);
        }
        if let Some(found) = self.fold_ptr_to_int_or_addr_of_gep(cast, ty, src) {
            return Visit::Replace(found);
        }
        common_cast_transforms(self, cast).unwrap_or(Visit::None)
    }

    /// `foldPtrToIntOrAddrOfGEP` (:2723): the chain of one-use GEPs on `null`
    /// or on an `inttoptr` of the same integer type, as integer additions.
    fn fold_ptr_to_int_or_addr_of_gep(
        &mut self,
        at: InstId,
        int_ty: TypeId,
        pointer: Operand,
    ) -> Option<Operand> {
        let mut geps = Vec::new();
        let mut base = pointer;
        while let Some((gep, instruction)) = query(self, Some(at)).instruction_of(base) {
            if !matches!(instruction.opcode, Opcode::GetElementPtr { .. }) || !self.has_one_use(base) {
                break;
            }
            geps.push(gep);
            base = instruction.operands[0];
        }
        if geps.is_empty() || self.type_of(base) != self.type_of(pointer) {
            return None;
        }
        let index_ty = self.index_type(self.type_of(pointer));
        let q = query(self, Some(at));
        let mut result = if let Some((_, cast)) = q.instruction_of(base)
            && cast.opcode == Opcode::Cast(CastOp::IntToPtr)
            && self.has_one_use(base)
            && self.type_of(cast.operands[0]) == int_ty
            && int_ty == index_ty
        {
            cast.operands[0]
        } else if matches!(
            base,
            Operand::Constant(id) if matches!(self.context.get(id).kind, llrm_mir::context::ConstantKind::Null)
        ) {
            self.int(index_ty, 0)
        } else {
            return None;
        };
        for &gep in geps.iter().rev() {
            let offset = self.emit_gep_offset(gep, false);
            let nuw = self.function.instruction(gep).flags.contains(Flags::NUW);
            let flags = if nuw { Flags::NUW } else { Flags::default() };
            result = self.create_binop(at, BinaryOp::Add, result, offset, flags);
        }
        Some(self.zext_or_trunc(at, result, int_ty))
    }
}

#[cfg(test)]
#[path = "combine_ptrcast_tests.rs"]
mod tests;
