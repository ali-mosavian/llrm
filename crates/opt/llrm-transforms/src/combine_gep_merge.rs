//! `visitGEPOfGEP`: a `getelementptr` of a `getelementptr` as one.
//!
//! Translation of
//! [`InstCombinerImpl::visitGEPOfGEP`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L2885)
//! with `shouldMergeGEPs` (:2345), `canonicalizeGEPOfConstGEPI8` (:2786) and
//! `combineConstantOffsets` (:2831) at d1106deb71cc.
//!
//! MIRROR: the order of the four folds; the flag algebra of
//! `GEPNoWrapFlags::intersectForOffsetAdd` and `intersectForReassociate`
//! ([`GepFlags`]); merged only where the sum of the two indices simplifies and
//! at most one index of the result is non-zero.
//!
//! DEVIATE 1: the source `GEPOperator` is a `getelementptr` instruction: MIR
//! has constant GEP expressions, and they are folded to a base and an offset
//! by `ConstantFolding` before they reach here.
//!
//! # Example
//! ```text
//! %q = getelementptr i32, ptr %p, i64 %i              %r = getelementptr i32, ptr %p, i64 %s
//! %r = getelementptr i32, ptr %q, i64 1         ==>   (%s = add i64 %i, 1, when that simplifies)
//! ```
//!
//! # Used by
//! [`combine_gep`](crate::combine_gep), which calls `visit_gep_of_gep` when a
//! `getelementptr`'s source is one.

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::simplify::simplify_binop_at;
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_gep::{Gep, GepFlags, Step};
use crate::combine_shared::{has_one_use, query, replace_operand};

impl Combiner<'_> {
    fn one_user_of(
        &self,
        inst: InstId,
    ) -> bool {
        self.function.instruction(inst).result.is_some_and(|result| self.function.users(result).has_one_use())
    }

    /// `GEP.hasAllZeroIndices()`.
    fn has_all_zero_indices(
        &self,
        gep: &Gep,
    ) -> bool {
        gep.indices.iter().all(|&index| self.is_zero_constant(index))
    }

    /// `GEP.hasAllConstantIndices()`.
    fn has_all_constant_indices(
        &self,
        gep: &Gep,
    ) -> bool {
        gep.indices.iter().all(|&index| self.constant_int(index).is_some())
    }

    /// `m_APInt` of an operand of integer type: its value and width.
    fn apint(
        &self,
        v: Operand,
    ) -> Option<(u128, u32)> {
        let value = self.constant_int(v)?;
        Some((value, self.context.types.int_bits(self.type_of(v))?))
    }

    /// `shouldMergeGEPs`: a GEP of only zeros is the same pointer as its
    /// source, which is not worth merging into a shared non-trivial one.
    fn should_merge_geps(
        &self,
        gep: &Gep,
        src: InstId,
    ) -> bool {
        let source = self.gep_of(src).expect("a getelementptr");
        !(self.has_all_zero_indices(gep) && !self.has_all_zero_indices(&source) && !self.one_user_of(src))
    }

    /// `canonicalizeGEPOfConstGEPI8`: `gep T, (gep i8, base, C1), (x + C2)` is
    /// `gep T, (gep i8, base, C1 + C2 * sizeof(T)), x`.
    fn canonicalize_gep_of_const_gep_i8(
        &mut self,
        inst: InstId,
        gep: &Gep,
        src: InstId,
    ) -> Visit {
        let source = self.gep_of(src).expect("a getelementptr");
        let [index] = gep.indices[..] else { return Visit::None };
        let byte = self.context.types.int(8);
        // m_PtrAdd(m_Value(Base), m_APInt(C1))
        let [offset] = source.indices[..] else { return Visit::None };
        if source.source != byte {
            return Visit::None;
        }
        let Some((c1, width)) = self.apint(offset) else { return Visit::None };
        // m_AddLike(m_Value(VarIndex), m_APInt(C2))
        let q = query(self, Some(inst));
        let Some((var_index, c2)) = pattern::add_like(&q, index, Flags::default()) else { return Visit::None };
        let nuw_add = pattern::add_like(&q, index, Flags::NUW).is_some();
        let Some((c2, c2_width)) = self.apint(c2) else { return Visit::None };
        let index_bits = self.context.types.int_bits(self.index_type(gep.ty)).expect("an integer");
        if width != index_bits || c2_width != index_bits {
            return Visit::None;
        }
        let mask = if index_bits == 128 { u128::MAX } else { (1u128 << index_bits) - 1 };
        let size = u128::from(self.alloc_size(gep.source));
        let new_offset = size.wrapping_mul(c2).wrapping_add(c1) & mask;
        if new_offset != 0 && !(self.one_user_of(src) && has_one_use(self, index)) {
            return Visit::None;
        }
        let mut flags = GepFlags::default();
        let (outer, inner) = (GepFlags::of(gep.flags), GepFlags::of(source.flags));
        if outer.nuw && inner.nuw && nuw_add {
            flags.nuw = true;
            flags.nusw = outer.in_bounds && inner.in_bounds;
            flags.in_bounds = flags.nusw;
        }
        let offset = self.int(self.index_type(gep.ty), new_offset);
        let constant = self.create_ptr_add(inst, source.ptr, offset, flags.flags());
        let operands = vec![constant, var_index];
        self.replace_with_new(inst, Opcode::GetElementPtr { source: gep.source }, gep.ty, operands, flags.flags())
    }

    /// `combineConstantOffsets`: `ptradd (ptradd (ptradd p, C1), x), C2` is
    /// `ptradd (ptradd p, x), C1+C2`.
    fn combine_constant_offsets(
        &mut self,
        inst: InstId,
        gep: &Gep,
    ) -> Visit {
        if !self.has_all_constant_indices(gep) {
            return Visit::None;
        }
        let mut flags = GepFlags::ALL;
        let mut skipped: Vec<InstId> = Vec::new();
        let mut inner = self.definition_of(gep.ptr).filter(|&def| self.gep_of(def).is_some());
        let inner = loop {
            let Some(current) = inner else { return Visit::None };
            let parts = self.gep_of(current).expect("a getelementptr");
            flags = flags.intersect_for_reassociate(GepFlags::of(parts.flags));
            if self.has_all_constant_indices(&parts) {
                break current;
            }
            if !self.one_user_of(current) {
                return Visit::None;
            }
            skipped.push(current);
            inner = self.definition_of(parts.ptr).filter(|&def| self.gep_of(def).is_some());
        };
        // The two constant offset GEPs are adjacent: left to the offset merge.
        let (Some(&first), Some(&last)) = (skipped.first(), skipped.last()) else { return Visit::None };
        let inner_parts = self.gep_of(inner).expect("a getelementptr");
        if !self.one_user_of(inner) || inner_parts.ty != gep.ty {
            return Visit::None;
        }
        let index_ty = self.index_type(gep.ty);
        let width = self.context.types.int_bits(index_ty).expect("an integer");
        let constants = |c: &Self, parts: &Gep| -> Option<u128> {
            let ids: Option<Vec<_>> = parts
                .indices
                .iter()
                .map(|&index| match index {
                    Operand::Constant(id) => Some(id),
                    _ => None,
                })
                .collect();
            llrm_mir::constantfolding::accumulate_constant_offset(c.context, c.env.layout, parts.source, &ids?, width)
        };
        let (Some(outer_offset), Some(inner_offset)) = (constants(self, gep), constants(self, &inner_parts)) else {
            return Visit::None;
        };
        let mask = if width == 128 { u128::MAX } else { (1u128 << width) - 1 };
        let offset = outer_offset.wrapping_add(inner_offset) & mask;

        replace_operand(self, last, 0, inner_parts.ptr);
        for &one in &skipped {
            self.function.set_flags(one, flags.flags());
        }
        let offset = self.int(index_ty, offset);
        let result_flags = flags.intersect_for_offset_add(GepFlags::of(gep.flags));
        let first_value = Operand::Value(self.function.instruction(first).result.expect("a value"));
        let found = self.create_ptr_add(inst, first_value, offset, result_flags.flags());
        Visit::Replace(found)
    }

    /// `visitGEPOfGEP(GEP, Src)`.
    pub(crate) fn visit_gep_of_gep(
        &mut self,
        inst: InstId,
        gep: &Gep,
        src: InstId,
    ) -> Visit {
        if !self.should_merge_geps(gep, src) {
            return Visit::None;
        }
        let found = self.canonicalize_gep_of_const_gep_i8(inst, gep, src);
        if !matches!(found, Visit::None) {
            return found;
        }
        let found = self.combine_constant_offsets(inst, gep);
        if !matches!(found, Visit::None) {
            return found;
        }

        let source = self.gep_of(src).expect("a getelementptr");
        if self.indexed_type(source.source, &source.indices) != Some(gep.source) {
            return Visit::None;
        }
        let merged = GepFlags::of(source.flags).intersect_for_offset_add(GepFlags::of(gep.flags)).flags();

        // gep i8, (gep i8, %base, C1), (select Cond, C2, C3)
        //   -> gep i8, %base, (select Cond, C1+C2, C1+C3)
        if self.one_user_of(src)
            && let [gep_index] = gep.indices[..]
            && let [src_index] = source.indices[..]
        {
            let selected = |c: &Self, v: Operand| {
                c.select_of(v).and_then(|(def, [cond, t, f])| Some((def, cond, c.apint(t)?, c.apint(f)?)))
            };
            let pair = match (self.apint(src_index), selected(self, gep_index)) {
                (Some(constant), Some(select)) => Some((constant, select)),
                _ => match (self.apint(gep_index), selected(self, src_index)) {
                    (Some(constant), Some(select)) => Some((constant, select)),
                    _ => None,
                },
            };
            if let Some(((offset, width), (select, cond, (on_true, true_width), (on_false, false_width)))) = pair {
                if !self.one_user_of(select) || true_width != width || false_width != width {
                    return Visit::None;
                }
                let mask = if width == 128 { u128::MAX } else { (1u128 << width) - 1 };
                let ty = self.function.instruction(select).ty;
                let new_true = self.int(ty, offset.wrapping_add(on_true) & mask);
                let new_false = self.int(ty, offset.wrapping_add(on_false) & mask);
                let new_select = self.select_like(inst, select, [cond, new_true, new_false], Flags::default());
                let found = self.create_gep(inst, gep.source, source.ptr, &[new_select], merged);
                return Visit::Replace(found);
            }
        }

        // Does the source GEP end with a sequential index?
        let Some(steps) = self.steps(source.source, &source.indices) else { return Visit::None };
        if !matches!(steps.last(), Some(Step::Sequence(_))) {
            return Visit::None;
        }

        // gep (gep %P, long B), long A, ... is gep %P, (A+B), ...
        let (Some(&src_last), Some(&gep_first)) = (source.indices.last(), gep.indices.first()) else {
            return Visit::None;
        };
        // Types differ when the input has not been normalized yet.
        if self.type_of(src_last) != self.type_of(gep_first) {
            return Visit::None;
        }
        let env = self.env;
        let Some(sum) = simplify_binop_at(
            self.context,
            env.globals,
            env.layout,
            Some(&env.analyses),
            self.function,
            inst,
            BinaryOp::Add,
            gep_first,
            src_last,
        ) else {
            return Visit::None;
        };
        let mut indices: Vec<Operand> = source.indices[..source.indices.len() - 1].to_vec();
        indices.push(sum);
        indices.extend_from_slice(&gep.indices[1..]);
        // No GEPs with more than one non-zero index.
        if indices.iter().filter(|&&index| !self.is_zero_constant(index)).count() > 1 {
            return Visit::None;
        }
        let found = self.create_gep(inst, source.source, source.ptr, &indices, merged);
        Visit::Replace(found)
    }
}
