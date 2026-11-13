//! `VectorUtils`: which intrinsic calls a vectorizer may widen.
//!
//! A translation of LLVM `d1106deb71cc` `VectorUtils.cpp`:
//! `getVectorIntrinsicIDForCall`
//! ([L234](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/VectorUtils.cpp#L234)),
//! `isTriviallyVectorizable` (L46) and `isVectorIntrinsicWithScalarOpAtArg`
//! (L142). Users: `LoopAccessInfo` (a widenable call reads no memory the
//! analysis must order) and `LoopVectorizationLegality` (the call arm of
//! `canVectorizeInstrs`).
//!
//! # Example
//! `%m = call i16 @llvm.smax.i16(i16 %v, i16 7)` is trivially vectorizable;
//! `llvm.ctlz.i32(%v, i1 false)` is too, its second argument staying scalar.
//!
//! # DEVIATE rows
//! - VU1 `llvm.powi` and `llvm.ldexp` (a scalar operand that is not an
//!   immediate): MIR has no intrinsic for them, so a call of either is not an
//!   intrinsic here. Exit: when MIR names them.
//! - VU2 the `VFDatabase` and `isTLIScalarize` ways of accepting a call: no
//!   vector function mappings exist.

use llrm_mir::intrinsics::{BitCount, Intrinsic};
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use crate::scev::Cx;

/// `getVectorIntrinsicIDForCall(Call, TLI) != not_intrinsic`, as the
/// intrinsic: a call of one that is trivially vectorizable, or
/// `lifetime.start`/`end`, `assume` or `noalias.scope.decl`, which the pin lets
/// through the same way (`sideeffect` and `pseudoprobe` are not in MIR).
pub fn get_vector_intrinsic_id_for_call(
    cx: &Cx,
    inst: InstId,
) -> Option<Intrinsic> {
    if !matches!(cx.function.instruction(inst).opcode, Opcode::Call(_)) {
        return None;
    }
    cx.query(None)
        .intrinsic(inst)
        .filter(
            |&id| is_trivially_vectorizable(id)
                || matches!(
                    id,
                    Intrinsic::LifetimeStart | Intrinsic::LifetimeEnd | Intrinsic::Assume | Intrinsic::NoAliasScopeDecl
                ),
        )
}

/// `isTriviallyVectorizable`: the intrinsic computes each lane from the same
/// lane of its arguments.
pub fn is_trivially_vectorizable(id: Intrinsic) -> bool {
    matches!(
        id,
        Intrinsic::WithOverflow { .. }
            | Intrinsic::Saturating { .. }
            | Intrinsic::MinMax { .. }
            | Intrinsic::Fixed { divide: false }
            | Intrinsic::FixedMul { .. }
            | Intrinsic::BitCount(_)
            | Intrinsic::ByteSwap
            | Intrinsic::BitReverse
            | Intrinsic::FunnelShift { .. }
            | Intrinsic::Abs
            | Intrinsic::Compare { .. }
            | Intrinsic::FMulAdd
            | Intrinsic::CopySign
            | Intrinsic::Unary(_)
            | Intrinsic::LRint
            | Intrinsic::LLRint
    )
}

/// `isVectorIntrinsicWithScalarOpAtArg`: the argument at `index` stays a
/// scalar in the vector call (the zero-poison flag of `ctlz`, `cttz` and `abs`,
/// the scale of the fixed-point operations).
pub fn is_vector_intrinsic_with_scalar_op_at_arg(
    id: Intrinsic,
    index: usize,
) -> bool {
    match id {
        Intrinsic::BitCount(BitCount::LeadingZeros | BitCount::TrailingZeros) | Intrinsic::Abs => index == 1,
        Intrinsic::Fixed { divide: false } | Intrinsic::FixedMul { .. } => index == 2,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flag_of_a_count_and_the_scale_of_a_fixed_multiply_stay_scalar() {
        assert!(is_vector_intrinsic_with_scalar_op_at_arg(Intrinsic::BitCount(BitCount::LeadingZeros), 1));
        assert!(is_vector_intrinsic_with_scalar_op_at_arg(Intrinsic::Abs, 1));
        assert!(is_vector_intrinsic_with_scalar_op_at_arg(Intrinsic::Fixed { divide: false }, 2));
        assert!(!is_vector_intrinsic_with_scalar_op_at_arg(Intrinsic::MinMax { signed: true, max: true }, 1));
        assert!(!is_vector_intrinsic_with_scalar_op_at_arg(Intrinsic::BitCount(BitCount::Population), 0));
    }

    #[test]
    fn a_memory_intrinsic_is_not_trivially_vectorizable() {
        assert!(!is_trivially_vectorizable(Intrinsic::MemCpy));
        assert!(is_trivially_vectorizable(Intrinsic::MinMax { signed: false, max: false }));
        assert!(is_trivially_vectorizable(Intrinsic::FixedMul { signed: false, saturating: false }));
        // `sdiv.fix` is not in the pin's list.
        assert!(!is_trivially_vectorizable(Intrinsic::Fixed { divide: true }));
    }
}
