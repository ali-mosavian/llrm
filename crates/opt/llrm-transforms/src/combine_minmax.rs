//! InstCombine's min/max intrinsics: the narrowing folds of
//! `InstCombinerImpl::visitCallInst` for `umin`, `umax`, `smin` and `smax`
//! ([InstCombineCalls.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCalls.cpp#L2261)).
//!
//! # How it works
//! A min or max of two operands extended from one type, or of one extended
//! operand and a constant that fits the narrow type, is the extension of the
//! narrow min or max. The unsigned intrinsics try `zext` first, then `sext`
//! (the pin falls through from its unsigned case into its signed one); the
//! signed ones only `sext`.
//!
//! # Example
//! ```text
//! umin.i33(zext i16 %x to i33, 9)  ==>  zext (umin.i16(%x, 9)) to i33
//! ```
//!
//! MIRROR: both folds, their one-use tests and the lossless-truncation test
//! of the constant, and the `i1` fold (`and` for `umin` and `smax`, `or` for
//! `umax` and `smin`).
//!
//! DEVIATE 1: the rest of the min/max case is not translated (clamp forms,
//! `neg`, `xor` of a power of two, `not`, nested min/max,
//! `cttz`/`ctlz` and the saturating folds). Exit: the next slice of
//! `visitCallInst`.
//!
//! # Used by
//! `visit_call` (`combine_memintrinsics`).

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::query;

fn mask(bits: u32) -> u128 {
    if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 }
}

/// `getLosslessUnsignedTrunc` / `getLosslessSignedTrunc`: `value`, of `bits`,
/// in `small` bits when extending it again gives `value` back.
pub(crate) fn lossless_trunc(
    value: u128,
    bits: u32,
    small: u32,
    ext: CastOp,
) -> Option<u128> {
    let narrow = value & mask(small);
    let back = match ext {
        CastOp::SExt if narrow >> (small - 1) & 1 == 1 => narrow | (mask(bits) & !mask(small)),
        _ => narrow,
    };
    (back == value & mask(bits)).then_some(narrow)
}

impl Combiner<'_> {
    /// The operand `v` is an `ext` of, when it is one.
    fn extended_from(
        &self,
        v: Operand,
        ext: CastOp,
    ) -> Option<Operand> {
        let def = self.definition_of(v)?;
        let inst = self.function.instruction(def);
        (inst.opcode == Opcode::Cast(ext)).then(|| inst.operands[0])
    }

    /// The narrowing folds of `umin`/`umax`/`smin`/`smax`.
    pub(crate) fn narrow_min_max(
        &mut self,
        call: InstId,
        signed: bool,
        max: bool,
    ) -> Visit {
        let operands = self.function.instruction(call).operands.clone();
        let [i0, i1, ..] = operands[..] else { return Visit::None };
        let ty = self.function.instruction(call).ty;
        let Some(bits) = self.context.types.int_bits(ty) else { return Visit::None };
        let base = match (signed, max) {
            (false, false) => "umin",
            (false, true) => "umax",
            (true, false) => "smin",
            (true, true) => "smax",
        };
        let exts: &[CastOp] = if signed { &[CastOp::SExt] } else { &[CastOp::ZExt, CastOp::SExt] };
        for &ext in exts {
            if let (Some(x), Some(y)) = (self.extended_from(i0, ext), self.extended_from(i1, ext))
                && self.type_of(x) == self.type_of(y)
                && (self.has_one_use(i0) || self.has_one_use(i1))
            {
                let narrow = self.create_intrinsic(call, base, self.type_of(x), &[x, y]);
                return Visit::Replace(self.create_cast(call, ext, narrow, ty, Flags::default()));
            }
            if let Some(x) = self.extended_from(i0, ext)
                && self.has_one_use(i0)
                && let Some((value, _)) = pattern::apint(&query(self, None), i1)
            {
                let small_ty = self.type_of(x);
                let small = self.context.types.int_bits(small_ty).expect("an integer");
                if let Some(narrow_c) = lossless_trunc(value, bits, small, ext) {
                    let constant = self.int(small_ty, narrow_c);
                    let narrow = self.create_intrinsic(call, base, small_ty, &[x, constant]);
                    return Visit::Replace(self.create_cast(call, ext, narrow, ty, Flags::default()));
                }
            }
        }
        // umin(i1 X, i1 Y) -> and i1 X, Y
        // smax(i1 X, i1 Y) -> and i1 X, Y
        // umax(i1 X, i1 Y) -> or i1 X, Y
        // smin(i1 X, i1 Y) -> or i1 X, Y
        if bits == 1 {
            let op = if max == signed { BinaryOp::And } else { BinaryOp::Or };
            return self.replace_with_binop(call, op, i0, i1, Flags::default());
        }
        Visit::None
    }
}

#[cfg(test)]
#[path = "combine_minmax_tests.rs"]
mod tests;
