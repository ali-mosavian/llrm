//! Whether a floating-point value is never a NaN (the pin's
//! `SelectionDAG::isKnownNeverNaN`).
//!
//! # How it works
//! [`SelectionDAG::is_known_never_nan`] walks the operands to the pin's
//! `MaxRecursionDepth` (6). A node with the `nnan` flag is never a NaN; the
//! arithmetic and the libm nodes can make a quiet NaN but never a signalling
//! one (`SNaN` asks for that); the conversions from an integer never do; the
//! sign operations, `select`, vector extracts and `BUILD_VECTOR` follow their
//! operands, lane by lane for the lanes asked about; a `ConstantFP` is
//! classified.
//!
//! DEVIATE NN1 (owner arm64, exit 2026-11-30): the pin ends in
//! `computeKnownFPClass` and the target hook `isKnownNeverNaNForTargetNode`;
//! neither is translated, so a node they alone would know (a `fabs` of an
//! `fcmp`-masked value, an `fmaxnum` pair that the class analysis proves)
//! answers "may be a NaN". The answer is conservative: a caller that gets
//! `false` keeps the code it had.
//!
//! # Runs
//! Called by the arm64 vector `setcc` lowering (`LowerVSETCC`).
//!
//! # See also
//! LLVM's `SelectionDAG::isKnownNeverNaN`
//! ([L6350](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAG.cpp#L6350)).

use super::dag::{SDNodeFlags, SDValue, SelectionDAG};
use super::fp_value;
use super::isd::ISD;

/// `MaxRecursionDepth`.
const MAX_DEPTH: u32 = 6;

impl SelectionDAG {
    /// `isKnownNeverNaN(Op, SNaN)`: every lane of `op` is a number (or, with
    /// `snan`, is not a signalling NaN).
    pub fn is_known_never_nan(
        &self,
        op: SDValue,
        snan: bool,
    ) -> bool {
        self.never_nan(op, self.all_lanes(op), snan, 0)
    }

    /// `getDemandAllEltsMask`: one bit per lane, or the one bit of a scalar.
    fn all_lanes(
        &self,
        op: SDValue,
    ) -> u128 {
        let vt = self.value_type(op);
        if !vt.is_vector() {
            return 1;
        }
        match vt.lanes() {
            lanes @ 1..=127 => (1u128 << lanes) - 1,
            _ => u128::MAX,
        }
    }

    /// `isKnownNeverNaN(Op, DemandedElts, SNaN, Depth)`.
    fn never_nan(
        &self,
        op: SDValue,
        demanded: u128,
        snan: bool,
        depth: u32,
    ) -> bool {
        // "If we're told that NaNs won't happen, assume they won't."
        if self.node(op).flags.has(SDNodeFlags::NO_NANS) {
            return true;
        }
        if depth >= MAX_DEPTH {
            return false;
        }
        let first = |this: &Self| this.never_nan(this.operand(op, 0), demanded, snan, depth + 1);
        match self.opcode(op) {
            // "TODO: Need isKnownNeverInfinity": a quiet NaN may come out of
            // two infinities, never a signalling one.
            ISD::FAdd
            | ISD::FSub
            | ISD::FMul
            | ISD::FDiv
            | ISD::FRem
            | ISD::FSin
            | ISD::FCos
            | ISD::FAtan
            | ISD::FMA => snan,
            ISD::FExp | ISD::FExp2 | ISD::FRint | ISD::FpExtend | ISD::FpRound => snan || first(self),
            ISD::FAbs | ISD::FNeg | ISD::FCopySign => first(self),
            ISD::Select => {
                self.never_nan(self.operand(op, 1), demanded, snan, depth + 1)
                    && self.never_nan(self.operand(op, 2), demanded, snan, depth + 1)
            }
            ISD::SIntToFp | ISD::UIntToFp => true,
            // "Need is known positive", "Refine on operand".
            ISD::FSqrt | ISD::FLog | ISD::FLog2 => snan,
            ISD::ExtractElement => {
                let source = self.operand(op, 0);
                let lanes = self.value_type(source).is_vector().then(|| self.value_type(source).lanes());
                match (lanes, self.constant(self.operand(op, 1))) {
                    (Some(lanes), Some(index)) if index < u128::from(lanes) && lanes < 128 => {
                        self.never_nan(source, 1u128 << index, snan, depth + 1)
                    }
                    _ => self.never_nan(source, self.all_lanes(source), snan, depth + 1),
                }
            }
            ISD::ExtractSubvector => {
                let source = self.operand(op, 0);
                let lanes = self.value_type(source).lanes();
                let Some(index) = self.constant(self.operand(op, 1)).filter(|&index| index < 128) else {
                    return self.never_nan(source, self.all_lanes(source), snan, depth + 1);
                };
                // `DemandedElts.zext(NumSrcElts).shl(Idx)`.
                let wanted = if lanes >= 128 { u128::MAX } else { demanded << index };
                self.never_nan(source, wanted, snan, depth + 1)
            }
            ISD::BuildVector => (0..self.node(op).ops.len()).all(|lane| {
                demanded & (1u128 << lane.min(127)) == 0 || self.never_nan(self.operand(op, lane), 1, snan, depth + 1)
            }),
            // The fallthrough: `computeKnownFPClass` of a constant is exact.
            ISD::ConstantFP | ISD::TargetConstantFP => self
                .fp_constant(op)
                .and_then(|bits| fp_value::classify(self.value_type(op), bits))
                .is_some_and(|class| !class.nan),
            _ => false,
        }
    }
}
