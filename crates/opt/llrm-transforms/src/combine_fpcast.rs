//! InstCombine's `visitUIToFP`, `visitSIToFP` and `visitFPExt`, from
//! [`InstCombineCasts.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp)
//! at d1106deb71cc.
//!
//! [`visit_uitofp`] is
//! [`visitUIToFP`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp#L2626),
//! [`visit_sitofp`] is `visitSIToFP` (:2656) and [`visit_fpext`] is
//! `visitFPExt` (:2450). Each opens with, or ends in, `commonCastTransforms`,
//! which is what folds `sitofp (sext X)` to `sitofp X`.
//!
//! MIRROR: the order of the steps and each condition.
//!
//! DEVIATE 1: MIR has no vectors.
//!
//! # Example
//! ```text
//! %w = sext i32 %x to i64                %d = sitofp i32 %x to double
//! %d = sitofp i64 %w to double
//! ```
//!
//! # Used by
//! `combine` (its `visit`, the `UIToFP`, `SIToFP` and `FPExt` opcodes).

use llrm_mir::module::InstId;
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::valuetracking::known;

use crate::combine::{Combiner, Visit};
use crate::combine_casts::common_cast_transforms;
use crate::combine_fptoint::{def_of, is_known_exact_cast_int_to_fp};
use crate::combine_shared::{self as shared, query};

/// `visitUIToFP`.
pub fn visit_uitofp(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if let Some(found) = common_cast_transforms(c, inst) {
        return found;
    }
    let flags = c.function.instruction(inst).flags;
    let src = c.function.instruction(inst).operands[0];
    if !flags.contains(Flags::NNEG) && known::is_known_non_negative(&query(c, Some(inst)), src, 0) {
        let mut nneg = flags;
        nneg.insert(Flags::NNEG);
        c.function.set_flags(inst, nneg);
        return Visit::Changed;
    }
    fold_masked_trunc(c, inst).unwrap_or(Visit::None)
}

/// uitofp (and (trunc X), Mask) --> uitofp (and X, zext(Mask))
fn fold_masked_trunc(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let src = c.function.instruction(inst).operands[0];
    if !shared::has_one_use(c, src) {
        return None;
    }
    let and = def_of(c, src).filter(|&def| c.function.instruction(def).opcode == Opcode::Binary(BinaryOp::And))?;
    let [truncated, mask] = c.function.instruction(and).operands[..] else { return None };
    let (mask_bits, _) = query(c, Some(inst)).int_constant(mask)?;
    if !shared::has_one_use(c, truncated) {
        return None;
    }
    let trunc = def_of(c, truncated).filter(|&def| c.function.instruction(def).opcode == Opcode::Cast(CastOp::Trunc))?;
    let x = c.function.instruction(trunc).operands[0];
    let source_width = c.context.types.int_bits(c.type_of(src))?;
    let input_width = c.context.types.int_bits(c.type_of(x))?;
    if input_width > 128 || c.env.layout.legal_integer(source_width) || !shared::should_change_type(c.env.layout, source_width, input_width) {
        return None;
    }
    let wide_mask = llrm_mir::module::Operand::Constant(c.context.int(c.type_of(x), mask_bits as i128));
    let masked = c.create_binop(inst, BinaryOp::And, x, wide_mask, Flags::default());
    let ty = c.function.instruction(inst).ty;
    let mut flags = Flags::default();
    if c.function.instruction(inst).flags.contains(Flags::NNEG) {
        flags.insert(Flags::NNEG);
    }
    Some(c.replace_with_new(inst, Opcode::Cast(CastOp::UIToFP), ty, vec![masked], flags))
}

/// `visitSIToFP`.
pub fn visit_sitofp(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if let Some(found) = common_cast_transforms(c, inst) {
        return found;
    }
    let src = c.function.instruction(inst).operands[0];
    if !known::is_known_non_negative(&query(c, Some(inst)), src, 0) {
        return Visit::None;
    }
    // nnan/afn/reassoc/contract/arcp carry no meaning for a value-preserving
    // cast, but ninf/nsz are semantically meaningful for {u,s}itofp and
    // remain valid after reinterpreting the operand as unsigned.
    let old = c.function.instruction(inst).flags;
    let mut flags = Flags::NNEG;
    for kept in [Flags::NINF, Flags::NSZ] {
        if old.contains(kept) {
            flags.insert(kept);
        }
    }
    let ty = c.function.instruction(inst).ty;
    c.replace_with_new(inst, Opcode::Cast(CastOp::UIToFP), ty, vec![src], flags)
}

/// `visitFPExt`.
pub fn visit_fpext(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    // If the source operand is a cast from integer to FP and known exact, then
    // cast the integer operand directly to the destination type.
    let src = c.function.instruction(inst).operands[0];
    if let Some(from_int) = def_of(c, src)
        && let Opcode::Cast(op @ (CastOp::SIToFP | CastOp::UIToFP)) = c.function.instruction(from_int).opcode
        && is_known_exact_cast_int_to_fp(c, from_int)
    {
        let x = c.function.instruction(from_int).operands[0];
        let ty = c.function.instruction(inst).ty;
        return c.replace_with_new(inst, Opcode::Cast(op), ty, vec![x], Flags::default());
    }
    common_cast_transforms(c, inst).unwrap_or(Visit::None)
}
