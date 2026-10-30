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
use crate::combine_gep_index::{
    infer_in_bounds, nusw_nneg_is_nuw, scaled_single_index, select_of_constants, split_added_index, srem_is_urem,
};

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

    pub(crate) fn and(
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
        self.emit_gep_offset_at(gep, gep, no_assumptions)
    }

    /// `emit_gep_offset` with the builder before `at`: `foldGEPICmp` emits an
    /// offset at the compare, `EmitGEPOffsets` at the GEP it rewrites.
    pub fn emit_gep_offset_at(
        &mut self,
        at: InstId,
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
                    let index = self.int_cast(at, index, int_ty);
                    match self.alloc_size(element) {
                        1 => index,
                        stride => {
                            let scale = self.int(int_ty, u128::from(stride));
                            self.create_binop(at, BinaryOp::Mul, index, scale, wrap)
                        }
                    }
                }
            };
            result = Some(match result {
                Some(sum) => self.create_binop(at, BinaryOp::Add, sum, term, wrap),
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

    /// `visitGetElementPtrInst`: the first of `FOLDS` that applies.
    pub fn visit_get_element_ptr(
        &mut self,
        inst: InstId,
    ) -> Visit {
        let Some(gep) = self.gep_of(inst) else { return Visit::None };
        FOLDS.iter().find_map(|fold| fold(self, inst, &gep)).unwrap_or(Visit::None)
    }
}

/// One rewrite of `visitGetElementPtrInst`: `Some` when it applies.
pub(crate) type Fold = fn(&mut Combiner, InstId, &Gep) -> Option<Visit>;

/// The rewrites in the order the pin tries them (:3321-3707). A new one is a
/// function and an entry here.
const FOLDS: &[Fold] = &[
    normalize_indices,
    constant_gep_to_ptr_add,
    canonicalize_to_ptr_add,
    strip_trailing_zero_index,
    strip_leading_zero_index,
    split_non_zero_indices,
    canonical_source_type,
    gep_of_phi,
    gep_of_gep,
    scaled_single_index,
    infer_in_bounds,
    nusw_nneg_is_nuw,
    split_added_index,
    select_of_constants,
    srem_is_urem,
];

/// Replaces the indices over a zero-size type by zero, and casts each to the
/// layout's index type.
fn normalize_indices(
    c: &mut Combiner,
    inst: InstId,
    gep: &Gep,
) -> Option<Visit> {
    let index_ty = c.index_type(gep.ty);
    let steps = c.steps(gep.source, &gep.indices)?;
    let nw = GepFlags::of(gep.flags);
    let mut changed = false;
    for (slot, (&index, step)) in gep.indices.iter().zip(&steps).enumerate() {
        let Step::Sequence(element) = *step else { continue };
        let mut current = index;
        if c.alloc_size(element) == 0 && !c.is_zero_constant(index) {
            current = c.int(index_ty, 0);
            changed = true;
        }
        let from = c.type_of(index);
        if from != index_ty {
            current = if c.context.types.int_bits(from) < c.context.types.int_bits(index_ty) {
                // zext nneg where the GEP cannot wrap either way, else sext
                if nw.nuw && nw.nusw {
                    c.create_cast(inst, CastOp::ZExt, current, index_ty, Flags::NNEG)
                } else {
                    c.create_cast(inst, CastOp::SExt, current, index_ty, Flags::default())
                }
            } else {
                let mut wrap = Flags::default();
                if nw.nuw {
                    wrap.insert(Flags::NUW);
                }
                if nw.nusw {
                    wrap.insert(Flags::NSW);
                }
                c.create_cast(inst, CastOp::Trunc, current, index_ty, wrap)
            };
            changed = true;
        }
        if current != index {
            c.function.set_operand(inst, slot + 1, current);
        }
    }
    changed.then_some(Visit::Changed)
}

/// Canonicalizes constant GEPs to i8.
fn constant_gep_to_ptr_add(
    c: &mut Combiner,
    inst: InstId,
    gep: &Gep,
) -> Option<Visit> {
    let byte = c.context.types.int(8);
    let index_ty = c.index_type(gep.ty);
    let constants: Vec<ConstantId> = gep
        .indices
        .iter()
        .map(|&index| match index {
            Operand::Constant(id) => Some(id),
            _ => None,
        })
        .collect::<Option<_>>()?;
    if gep.source == byte {
        return None;
    }
    let width = c.context.types.int_bits(index_ty).expect("an integer");
    let offset = accumulate_constant_offset(c.context, c.env.layout, gep.source, &constants, width)?;
    let offset = c.int(index_ty, offset);
    Some(Visit::Replace(c.create_ptr_add(inst, gep.ptr, offset, gep.flags)))
}

/// `shouldCanonicalizeGEPToPtrAdd`: the GEP as a `ptradd` of its byte offset.
fn canonicalize_to_ptr_add(
    c: &mut Combiner,
    inst: InstId,
    gep: &Gep,
) -> Option<Visit> {
    if !c.should_canonicalize_gep_to_ptr_add(gep) {
        return None;
    }
    let offset = c.emit_gep_offset(inst, false);
    Some(Visit::Replace(c.create_ptr_add(inst, gep.ptr, offset, gep.flags)))
}

fn strip_trailing_zero_index(
    c: &mut Combiner,
    inst: InstId,
    gep: &Gep,
) -> Option<Visit> {
    let (&last, front) = gep.indices.split_last()?;
    c.is_zero_constant(last).then(|| Visit::Replace(c.create_gep(inst, gep.source, gep.ptr, front, gep.flags)))
}

fn strip_leading_zero_index(
    c: &mut Combiner,
    inst: InstId,
    gep: &Gep,
) -> Option<Visit> {
    let &first = gep.indices.first()?;
    if !c.is_zero_constant(first) {
        return None;
    }
    // The second index steps over the source's element (not a struct field).
    let Some(Step::Sequence(element)) = c.steps(gep.source, &gep.indices)?.get(1).copied() else { return None };
    Some(Visit::Replace(c.create_gep(inst, element, gep.ptr, &gep.indices[1..], gep.flags)))
}

/// A GEP with several non-zero indices is split in two.
fn split_non_zero_indices(
    c: &mut Combiner,
    inst: InstId,
    gep: &Gep,
) -> Option<Visit> {
    // The first index may be a zero that steps over nothing.
    let skipped = usize::from(gep.indices.first().is_some_and(|&first| c.is_zero_constant(first)));
    let at = gep.indices.iter().enumerate().skip(skipped + 1).map(|(at, _)| at).next()?;
    let front = c.create_gep(inst, gep.source, gep.ptr, &gep.indices[..at], gep.flags);
    let reached = c.indexed_type(gep.source, &gep.indices[..at])?;
    let zero = c.int(c.index_type(gep.ty), 0);
    let operands = [front, zero].into_iter().chain(gep.indices[at..].iter().copied()).collect();
    Some(c.replace_with_new(inst, Opcode::GetElementPtr { source: reached }, gep.ty, operands, gep.flags))
}

/// Canonicalizes `gep %T` to `gep [sizeof(%T) x i8]`.
fn canonical_source_type(
    c: &mut Combiner,
    inst: InstId,
    gep: &Gep,
) -> Option<Visit> {
    if gep.indices.len() != 1 {
        return None;
    }
    let element = match c.context.types.get(gep.source) {
        Type::Array { element, .. } => *element,
        _ => gep.source,
    };
    let byte = c.context.types.int(8);
    if element == byte {
        return None;
    }
    let scale = c.alloc_size(gep.source);
    let new_source =
        if scale == 1 { byte } else { c.context.types.intern(Type::Array { element: byte, count: scale }) };
    c.function.set_opcode(inst, Opcode::GetElementPtr { source: new_source });
    Some(Visit::Changed)
}

/// A GEP of a phi of GEPs: the phi is replaced by one GEP and a phi of the
/// index they differ in.
fn gep_of_phi(
    c: &mut Combiner,
    inst: InstId,
    gep: &Gep,
) -> Option<Visit> {
    let phi = c.definition_of(gep.ptr).filter(|&def| c.function.instruction(def).opcode == Opcode::Phi)?;
    let new_ptr = c.fold_gep_of_phi(inst, phi)?;
    crate::combine_shared::replace_operand(c, inst, 0, new_ptr);
    Some(Visit::Changed)
}

fn gep_of_gep(
    c: &mut Combiner,
    inst: InstId,
    gep: &Gep,
) -> Option<Visit> {
    let src = c.definition_of(gep.ptr).filter(|&def| c.gep_of(def).is_some())?;
    Some(c.visit_gep_of_gep(inst, gep, src)).filter(|found| !matches!(found, Visit::None))
}

#[cfg(test)]
#[path = "combine_gep_tests.rs"]
mod tests;
