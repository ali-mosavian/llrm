//! `visitGetElementPtrInst`: canonical forms of a `getelementptr`.
//!
//! Translation of
//! [`InstCombinerImpl::visitGetElementPtrInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L3321)
//! and `emitGEPOffset`
//! ([Local.cpp:22](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/Local.cpp#L22))
//! at d1106deb71cc, up to the one-index rewrites (:3513), with `visitGEPOfGEP`
//! in `combine_gep_merge.rs`; the rest follows in later PRs.
//!
//! MIRROR: indices over a zero-size type become zero, an index is cast to the
//! layout's index type, a GEP of constants is a byte `ptradd`, a GEP whose
//! single index is a multiply or shift by a constant, or whose base is a
//! constant-index GEP, is a `ptradd` of its byte offset, trailing and leading
//! zero indices are dropped, a GEP with two non-zero indices is split in two,
//! and a single-index GEP over `T` is over `[sizeof T x i8]`.
//!
//! DEVIATE 1: no vector or scalable GEPs: MIR has neither.
//!
//! DEVIATE 2: `simplifyGEPInst` is `combine::visit`'s `simplify_instruction_in`
//! before this runs.
//!
//! DEVIATE 3: a source type changed in place returns `Visit::Changed` (the
//! instruction is looked at again and finds nothing more) where LLVM sets
//! `MadeIRChange` and goes on.
//!
//! # Not yet translated
//! `foldGEPOfPhi`, the single-index rewrites (:3513-3640), the
//! derived `inbounds`/`nuw` (:3641-3680), `foldSelectGEP` and the `srem` form.
//!
//! # Example
//! ```text
//! %p = getelementptr [4 x i32], ptr %b, i64 0, i64 1      %p = getelementptr i8, ptr %b, i64 4
//! ```

use llrm_mir::constantfolding::accumulate_constant_offset;
use llrm_mir::context::{ConstantId, ConstantKind};
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::simplify::simplify_gep_operands;
use llrm_mir::types::{Type, TypeId};

use crate::combine::{Combiner, Visit};

/// What an index of a `getelementptr` steps over (`gep_type_iterator`).
#[derive(Clone, Copy)]
pub(crate) enum Step {
    /// A struct: the index is a constant field number.
    Struct(TypeId),
    /// A sequence: the index scales by the size of its element.
    Sequence(TypeId),
}

/// The parts of a `getelementptr`.
pub(crate) struct Gep {
    pub source: TypeId,
    pub ptr: Operand,
    pub indices: Vec<Operand>,
    pub flags: Flags,
    pub ty: TypeId,
}

/// `GEPNoWrapFlags`: `inbounds` implies `nusw`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct GepFlags {
    pub in_bounds: bool,
    pub nusw: bool,
    pub nuw: bool,
}

impl GepFlags {
    pub(crate) const ALL: GepFlags = GepFlags { in_bounds: true, nusw: true, nuw: true };

    pub(crate) fn of(flags: Flags) -> Self {
        let in_bounds = flags.contains(Flags::INBOUNDS);
        GepFlags { in_bounds, nusw: in_bounds || flags.contains(Flags::NUSW), nuw: flags.contains(Flags::NUW) }
    }

    /// The instruction's flags; `inbounds` is not also spelled `nusw`.
    pub(crate) fn flags(self) -> Flags {
        let mut flags = Flags::default();
        if self.in_bounds {
            flags.insert(Flags::INBOUNDS);
        } else if self.nusw {
            flags.insert(Flags::NUSW);
        }
        if self.nuw {
            flags.insert(Flags::NUW);
        }
        flags
    }

    fn and(
        self,
        other: GepFlags,
    ) -> GepFlags {
        GepFlags {
            in_bounds: self.in_bounds && other.in_bounds,
            nusw: self.nusw && other.nusw,
            nuw: self.nuw && other.nuw,
        }
    }

    /// Given `(gep (gep p, x), y)`, the flags of `(gep p, x+y)`: without
    /// `inbounds` `nusw` needs `x + y` not to wrap.
    pub(crate) fn intersect_for_offset_add(
        self,
        other: GepFlags,
    ) -> GepFlags {
        let mut common = self.and(other);
        if !common.in_bounds {
            common.nusw = false;
        }
        common
    }

    /// Given `(gep (gep p, x), y)`, the flags of `(gep (gep p, y), x)`: only
    /// with `nuw` do `inbounds` and `nusw` survive.
    pub(crate) fn intersect_for_reassociate(
        self,
        other: GepFlags,
    ) -> GepFlags {
        let common = self.and(other);
        if common.nuw { common } else { GepFlags::default() }
    }
}

impl Combiner<'_> {
    pub(crate) fn gep_of(
        &self,
        inst: InstId,
    ) -> Option<Gep> {
        let instruction = self.function.instruction(inst);
        let Opcode::GetElementPtr { source } = instruction.opcode else { return None };
        let (&ptr, indices) = instruction.operands.split_first()?;
        Some(Gep { source, ptr, indices: indices.to_vec(), flags: instruction.flags, ty: instruction.ty })
    }

    /// The value of a constant integer operand.
    pub(crate) fn constant_int(
        &self,
        v: Operand,
    ) -> Option<u128> {
        let Operand::Constant(id) = v else { return None };
        match self.context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits),
            ConstantKind::Null | ConstantKind::Zero => Some(0),
            _ => None,
        }
    }

    pub(crate) fn is_zero_constant(
        &self,
        v: Operand,
    ) -> bool {
        self.constant_int(v) == Some(0)
    }

    /// `DL.getIndexType(PtrTy)`.
    pub(crate) fn index_type(
        &self,
        ptr_ty: TypeId,
    ) -> TypeId {
        let space = match self.context.types.get(ptr_ty) {
            Type::Pointer(space) => *space,
            _ => 0,
        };
        self.context.types.int(self.env.layout.pointer(space).index_bits)
    }

    /// `DL.getTypeAllocSize(ty)`.
    pub(crate) fn alloc_size(
        &self,
        ty: TypeId,
    ) -> u64 {
        self.env.layout.alloc_size(&self.context.types, ty)
    }

    /// What each index steps over; `None` if a struct's index is not a
    /// constant that names a field.
    pub(crate) fn steps(
        &self,
        source: TypeId,
        indices: &[Operand],
    ) -> Option<Vec<Step>> {
        let mut steps = Vec::with_capacity(indices.len());
        let mut current = source;
        for (at, &index) in indices.iter().enumerate() {
            if at == 0 {
                steps.push(Step::Sequence(source));
                continue;
            }
            if self.context.types.fields(current).is_some() {
                steps.push(Step::Struct(current));
                current = self.context.types.member(current, self.constant_int(index)? as u64)?;
            } else {
                current = match self.context.types.get(current) {
                    Type::Array { element, .. } | Type::Vector { element, .. } => *element,
                    _ => return None,
                };
                steps.push(Step::Sequence(current));
            }
        }
        Some(steps)
    }

    /// `GetElementPtrInst::getIndexedType(source, indices)`: the type the
    /// indices reach.
    pub(crate) fn indexed_type(
        &self,
        source: TypeId,
        indices: &[Operand],
    ) -> Option<TypeId> {
        let mut current = source;
        for &index in indices.iter().skip(1) {
            current = match self.context.types.get(current) {
                Type::Array { element, .. } | Type::Vector { element, .. } => *element,
                _ => self.context.types.member(current, self.constant_int(index)? as u64)?,
            };
        }
        Some(current)
    }

    /// `Builder.CreateGEP(source, ptr, indices, "", flags)`: what `simplifyGEP`
    /// finds, else a new instruction before `at`.
    pub(crate) fn create_gep(
        &mut self,
        at: InstId,
        source: TypeId,
        ptr: Operand,
        indices: &[Operand],
        flags: Flags,
    ) -> Operand {
        if let Some(found) =
            simplify_gep_operands(self.context, self.env.layout, self.function, source, ptr, indices, flags)
        {
            return found;
        }
        let ty = self.type_of(ptr);
        let operands = std::iter::once(ptr).chain(indices.iter().copied()).collect();
        let new = self.insert_before(at, Opcode::GetElementPtr { source }, ty, operands, flags);
        Operand::Value(self.function.instruction(new).result.expect("a value"))
    }

    /// `Builder.CreatePtrAdd(ptr, offset, "", flags)`.
    pub(crate) fn create_ptr_add(
        &mut self,
        at: InstId,
        ptr: Operand,
        offset: Operand,
        flags: Flags,
    ) -> Operand {
        let byte = self.context.types.int(8);
        self.create_gep(at, byte, ptr, &[offset], flags)
    }

    /// `emitGEPOffset(Builder, DL, GEP, NoAssumptions)`: the byte offset the
    /// GEP adds, computed before `gep`.
    pub fn emit_gep_offset(
        &mut self,
        gep: InstId,
        no_assumptions: bool,
    ) -> Operand {
        let Gep { source, indices, flags, ty, .. } = self.gep_of(gep).expect("a getelementptr");
        let int_ty = self.index_type(ty);
        // nusw implies nsw for the offset arithmetic.
        let gep_flags = GepFlags::of(flags);
        let mut wrap = Flags::default();
        if gep_flags.nusw && !no_assumptions {
            wrap.insert(Flags::NSW);
        }
        if gep_flags.nuw && !no_assumptions {
            wrap.insert(Flags::NUW);
        }
        let steps = self.steps(source, &indices).expect("a well-formed getelementptr");
        let mut result: Option<Operand> = None;
        for (&index, step) in indices.iter().zip(steps) {
            let term = match (step, self.constant_int(index)) {
                (_, Some(0)) => continue,
                // A struct index adds its field's offset.
                (Step::Struct(struct_ty), Some(field)) => {
                    let (_, offsets) = self.env.layout.struct_layout(&self.context.types, struct_ty);
                    match offsets.get(field as usize).copied() {
                        None | Some(0) => continue,
                        Some(offset) => self.int(int_ty, u128::from(offset)),
                    }
                }
                (Step::Struct(_), None) => unreachable!("a struct index is a constant"),
                (Step::Sequence(element), _) => {
                    let index = self.int_cast(gep, index, int_ty);
                    match self.alloc_size(element) {
                        1 => index,
                        stride => {
                            let scale = self.int(int_ty, u128::from(stride));
                            self.create_binop(gep, BinaryOp::Mul, index, scale, wrap)
                        }
                    }
                }
            };
            result = Some(match result {
                Some(sum) => self.create_binop(gep, BinaryOp::Add, sum, term, wrap),
                None => term,
            });
        }
        result.unwrap_or_else(|| self.int(int_ty, 0))
    }

    /// `Builder.CreateIntCast(v, ty, /*isSigned=*/true)`.
    fn int_cast(
        &mut self,
        at: InstId,
        v: Operand,
        ty: TypeId,
    ) -> Operand {
        let (from, to) = (self.context.types.int_bits(self.type_of(v)), self.context.types.int_bits(ty));
        match from.cmp(&to) {
            std::cmp::Ordering::Equal => v,
            std::cmp::Ordering::Less => self.create_cast(at, CastOp::SExt, v, ty, Flags::default()),
            std::cmp::Ordering::Greater => self.create_cast(at, CastOp::Trunc, v, ty, Flags::default()),
        }
    }

    /// `shouldCanonicalizeGEPToPtrAdd`.
    fn should_canonicalize_gep_to_ptr_add(
        &self,
        gep: &Gep,
    ) -> bool {
        if self.context.types.int_bits(gep.source) == Some(8) {
            return false;
        }
        // gep i32 p, mul(O, C) -> gep i8, p, mul(O, C*4) to fold the two
        // multiplies together.
        if let [index] = gep.indices[..]
            && let Operand::Value(value) = index
            && self.function.users(value).has_one_use()
            && let Some(def) = self.definition_of(index)
        {
            let scaled = self.function.instruction(def);
            let by_constant = matches!(
                scaled.operands[..],
                [_, rhs] if self.constant_int(rhs).is_some()
            );
            if by_constant && matches!(scaled.opcode, Opcode::Binary(BinaryOp::Mul | BinaryOp::Shl)) {
                return true;
            }
        }
        // gep (gep %p, C1), %x, C2 is expanded so the two constants can
        // possibly be merged together.
        self.definition_of(gep.ptr)
            .and_then(|def| self.gep_of(def))
            .is_some_and(
                |base| base.indices.iter().all(|&index| self.constant_int(index).is_some())
                    && gep.indices.iter().any(|&index| self.constant_int(index).is_some_and(|value| value != 0)),
            )
    }

    pub(crate) fn definition_of(
        &self,
        v: Operand,
    ) -> Option<InstId> {
        let Operand::Value(value) = v else { return None };
        match self.function.value(value).def {
            llrm_mir::module::ValueDef::Instruction(def) => Some(def),
            _ => None,
        }
    }

    /// `visitGetElementPtrInst`.
    pub fn visit_get_element_ptr(
        &mut self,
        inst: InstId,
    ) -> Visit {
        let Some(Gep { source, ptr, indices, flags, ty }) = self.gep_of(inst) else { return Visit::None };
        let index_ty = self.index_type(ty);
        let Some(steps) = self.steps(source, &indices) else { return Visit::None };

        // Replace the indices over a zero-size type by zero, and cast each
        // to the layout's index type.
        let mut changed = false;
        for (slot, (&index, step)) in indices.iter().zip(&steps).enumerate() {
            let Step::Sequence(element) = *step else { continue };
            let mut current = index;
            if self.alloc_size(element) == 0 && !self.is_zero_constant(index) {
                current = self.int(index_ty, 0);
                changed = true;
            }
            let from = self.type_of(index);
            if from != index_ty {
                let narrower = self.context.types.int_bits(from) < self.context.types.int_bits(index_ty);
                current = if narrower {
                    // sext, or zext nneg where the GEP cannot wrap either way
                    if GepFlags::of(flags).nuw && GepFlags::of(flags).nusw {
                        self.create_cast(inst, CastOp::ZExt, current, index_ty, Flags::NNEG)
                    } else {
                        self.create_cast(inst, CastOp::SExt, current, index_ty, Flags::default())
                    }
                } else {
                    let mut wrap = Flags::default();
                    if GepFlags::of(flags).nuw {
                        wrap.insert(Flags::NUW);
                    }
                    if GepFlags::of(flags).nusw {
                        wrap.insert(Flags::NSW);
                    }
                    self.create_cast(inst, CastOp::Trunc, current, index_ty, wrap)
                };
                changed = true;
            }
            if current != index {
                self.function.set_operand(inst, slot + 1, current);
            }
        }
        if changed {
            return Visit::Changed;
        }

        // Canonicalize constant GEPs to i8 type.
        let byte = self.context.types.int(8);
        let constants: Option<Vec<ConstantId>> = indices
            .iter()
            .map(|&index| match index {
                Operand::Constant(id) => Some(id),
                _ => None,
            })
            .collect();
        if source != byte
            && let Some(constants) = constants
            && let width = self.context.types.int_bits(index_ty).expect("an integer")
            && let Some(offset) = accumulate_constant_offset(self.context, self.env.layout, source, &constants, width)
        {
            let offset = self.int(index_ty, offset);
            let found = self.create_ptr_add(inst, ptr, offset, flags);
            return Visit::Replace(found);
        }

        let gep = Gep { source, ptr, indices: indices.clone(), flags, ty };
        if self.should_canonicalize_gep_to_ptr_add(&gep) {
            let offset = self.emit_gep_offset(inst, false);
            let found = self.create_ptr_add(inst, ptr, offset, flags);
            return Visit::Replace(found);
        }

        // Strip trailing zero indices.
        if let Some((&last, front)) = indices.split_last()
            && self.is_zero_constant(last)
        {
            let found = self.create_gep(inst, source, ptr, front, flags);
            return Visit::Replace(found);
        }

        // Strip leading zero indices.
        if let Some(&first) = indices.first()
            && self.is_zero_constant(first)
            && let Some(Step::Sequence(element)) = steps.get(1).copied()
        {
            let found = self.create_gep(inst, element, ptr, &indices[1..], flags);
            return Visit::Replace(found);
        }

        // A GEP with several non-zero indices is split in two.
        let mut seen_non_zero = false;
        for (at, &index) in indices.iter().enumerate() {
            // Ignore one leading zero index.
            if at == 0 && self.is_zero_constant(index) {
                continue;
            }
            if !seen_non_zero {
                seen_non_zero = true;
                continue;
            }
            let front = self.create_gep(inst, source, ptr, &indices[..at], flags);
            let reached = self.indexed_type(source, &indices[..at]).expect("a well-formed getelementptr");
            let mut back = vec![self.int(index_ty, 0)];
            back.extend_from_slice(&indices[at..]);
            let operands = std::iter::once(front).chain(back).collect();
            return self.replace_with_new(inst, Opcode::GetElementPtr { source: reached }, ty, operands, flags);
        }

        // Canonicalize gep %T to gep [sizeof(%T) x i8].
        let canonical = |c: &Self, ty: TypeId| {
            let element = match c.context.types.get(ty) {
                Type::Array { element, .. } => *element,
                _ => ty,
            };
            c.context.types.int_bits(element) == Some(8)
        };
        if indices.len() == 1 && !canonical(self, source) {
            let scale = self.alloc_size(source);
            let new_source =
                if scale == 1 { byte } else { self.context.types.intern(Type::Array { element: byte, count: scale }) };
            self.function.set_opcode(inst, Opcode::GetElementPtr { source: new_source });
            return Visit::Changed;
        }

        // A GEP of a GEP.
        if let Some(src) = self.definition_of(ptr).filter(|&def| self.gep_of(def).is_some()) {
            let gep = Gep { source, ptr, indices, flags, ty };
            let found = self.visit_gep_of_gep(inst, &gep, src);
            if !matches!(found, Visit::None) {
                return found;
            }
        }
        Visit::None
    }
}

#[cfg(test)]
#[path = "combine_gep_tests.rs"]
mod tests;
