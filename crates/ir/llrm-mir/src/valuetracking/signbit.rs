//! `canIgnoreSignBitOfZero` and `canIgnoreSignBitOfNaN`: whether the user of a
//! value reads neither the sign of a zero nor the sign of a NaN in it.
//!
//! Translated from
//! [ValueTracking.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L6580)
//! at d1106deb71cc.
//!
//! DEVIATE 1: no `nofpclass` return or parameter attributes (MIR keeps none):
//! a `ret` and a call to a function that is no intrinsic answer false.
//!
//! DEVIATE 2: `llvm.fptoui.sat` and `llvm.fptosi.sat` are not intrinsics MIR
//! knows; a call to them answers false.
//!
//! # Example
//! ```text
//! %s = select i1 %c, float %x, float %y
//! %i = fptosi float %s to i32       ; ignores both: true, true
//! %a = fadd float %s, %z            ; ignores the NaN's only
//! ```

use crate::fpclass::{FC_NONE, FC_ZERO, FpClassTest};
use crate::module::Use;
use crate::opcode::{BinaryOp, CastOp, Flags, Opcode};
use crate::valuetracking::Query;
use crate::valuetracking::pattern::{self, IntrinsicId};

/// `canIgnoreSignBitOfZero(U)` (:6580).
pub fn can_ignore_sign_bit_of_zero(
    q: &Query,
    used: Use,
) -> bool {
    let user = q.function.instruction(used.user);
    if user.flags.contains(Flags::NSZ) {
        return true;
    }
    match user.opcode {
        Opcode::Cast(CastOp::FPToSI | CastOp::FPToUI) => true,
        // fcmp treats both positive and negative zero as equal.
        Opcode::FCmp(_) => true,
        Opcode::Call(_) => match pattern::call_intrinsic(q, used.user) {
            Some(IntrinsicId::Fabs) => true,
            Some(IntrinsicId::Copysign) => used.index == 0,
            Some(IntrinsicId::IsFpclass) => {
                let test = pattern::call_arguments(user)
                    .get(1)
                    .and_then(|&test| pattern::constant_int_u64(q, test))
                    .map(|test| FpClassTest(test as _) & FC_ZERO);
                test.is_some_and(|test| test == FC_ZERO || test == FC_NONE)
            }
            _ => false,
        },
        _ => false,
    }
}

/// `canIgnoreSignBitOfNaN(U)` (:6618).
pub fn can_ignore_sign_bit_of_nan(
    q: &Query,
    used: Use,
) -> bool {
    let user = q.function.instruction(used.user);
    if user.flags.contains(Flags::NNAN) {
        return true;
    }
    match user.opcode {
        Opcode::Cast(CastOp::FPToSI | CastOp::FPToUI | CastOp::FPTrunc | CastOp::FPExt) => true,
        // Proper FP math operations ignore the sign bit of NaN.
        Opcode::Binary(BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul | BinaryOp::FDiv | BinaryOp::FRem) => true,
        Opcode::FCmp(_) => true,
        // Bitwise FP operations should preserve the sign bit of NaN.
        Opcode::Call(_) => match pattern::call_intrinsic(q, used.user) {
            Some(IntrinsicId::Fabs) => true,
            Some(IntrinsicId::Copysign) => used.index == 0,
            // Other proper FP math intrinsics ignore the sign bit of NaN.
            Some(
                IntrinsicId::Maxnum
                | IntrinsicId::Minnum
                | IntrinsicId::Maximum
                | IntrinsicId::Minimum
                | IntrinsicId::Maximumnum
                | IntrinsicId::Minimumnum
                | IntrinsicId::Canonicalize
                | IntrinsicId::Fma
                | IntrinsicId::Fmuladd
                | IntrinsicId::Sqrt
                | IntrinsicId::Pow
                | IntrinsicId::Powi
                | IntrinsicId::IsFpclass,
            ) => true,
            _ => false,
        },
        _ => false,
    }
}
