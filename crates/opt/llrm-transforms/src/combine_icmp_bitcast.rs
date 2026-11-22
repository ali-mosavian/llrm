//! InstCombine's `foldICmpBitCast`, the `sitofp` and `uitofp` arms
//! ([`InstCombineCompares.cpp:3491`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L3491))
//! at d1106deb71cc.
//!
//! MIRROR: the order of the arms and each condition.
//!
//! OWED, in place as `// OWED:` comments: the sign-bit test of a bitcast of an
//! `fpext`/`fptrunc`, and the `is.fpclass` fold of a special value (no backend
//! lowers the intrinsic yet).
//!
//! # How it works
//! A converted integer is zero exactly when its bits are, and `sitofp` keeps
//! its sign in the sign bit, so a compare of the bits against zero (or a sign
//! test) is the compare of the integer.
//!
//! # Used by
//! `combine_compares::visit_icmp`.
//!
//! # Example
//! ```text
//! %f = uitofp i8 %x to float          %r = icmp eq i8 %x, 0
//! %b = bitcast float %f to i32
//! %r = icmp eq i32 %b, 0
//! ```

use llrm_mir::context::mask;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{CastOp, Flags, IntPredicate, Opcode};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::query;

/// The operand of the cast `v` when it is `op`.
fn cast_source(
    c: &Combiner,
    v: Operand,
    op: CastOp,
) -> Option<Operand> {
    let (_, instruction) = query(c, None).instruction_of(v)?;
    matches!(instruction.opcode, Opcode::Cast(kind) if kind == op).then(|| instruction.operands[0])
}

/// `foldICmpBitCast`, `icmp pred (bitcast (sitofp|uitofp X)), C`.
pub fn fold_icmp_bit_cast(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let source = cast_source(c, op0, CastOp::BitCast)?;
    let (value, width) = query(c, None).int_constant(op1)?;
    if let Some(x) = cast_source(c, source, CastOp::SIToFP) {
        // Zero-equality and sign-bit checks are preserved through sitofp +
        // bitcast.
        let zero = value == 0;
        let one = value == 1 && pred == IntPredicate::Slt;
        let minus_one = value == mask(width) && pred == IntPredicate::Sgt;
        if (zero && matches!(
            pred,
            IntPredicate::Eq | IntPredicate::Ne | IntPredicate::Slt | IntPredicate::Sgt
        ))
            || one
            || minus_one
        {
            let ty = c.type_of(x);
            let rhs = match (zero, one) {
                (true, _) => c.int(ty, 0),
                (_, true) => c.int(ty, 1),
                _ => c.all_ones(ty),
            };
            return Some(new_icmp(c, inst, pred, x, rhs));
        }
    }
    // Zero-equality checks are preserved through unsigned floating-point casts.
    if let Some(x) = cast_source(c, source, CastOp::UIToFP)
        && matches!(pred, IntPredicate::Eq | IntPredicate::Ne)
        && value == 0
    {
        let ty = c.type_of(x);
        let zero = c.int(ty, 0);
        return Some(new_icmp(c, inst, pred, x, zero));
    }
    // OWED: the sign-bit test of a bitcast of an fpext/fptrunc, and the
    // is.fpclass fold of a special value.
    None
}

/// `new ICmpInst(pred, a, b)` in place of `inst`.
fn new_icmp(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    a: Operand,
    b: Operand,
) -> Visit {
    let i1 = c.context.types.int(1);
    c.replace_with_new(inst, Opcode::ICmp(pred), i1, vec![a, b], Flags::default())
}

#[cfg(test)]
#[path = "combine_icmp_bitcast_tests.rs"]
mod tests;
