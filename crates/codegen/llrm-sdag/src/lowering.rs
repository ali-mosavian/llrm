//! What the target lets the legalizer assume: which integer types a register
//! holds and what becomes of the others (the pin's `TargetLoweringBase`
//! type half).
//!
//! # How it works
//! [`TargetLoweringInfo::new`] is `computeRegisterProperties` for integers:
//! every integer type wider than the widest legal one expands to the
//! previous type, and every smaller illegal one promotes to the nearest legal
//! type above it. [`TargetLoweringInfo::type_conversion`] is
//! `getTypeConversion`: a simple type reads that table; any other integer is
//! first rounded to a power of two (at least `i8`), promoted when the rounded
//! type promotes, else that rounded type, and an exact power of two that is
//! not a simple type expands to half.
//!
//! # Runs
//! Built once per target from its description; read by the type legalizer.
//!
//! DEVIATE SD2: [`MVT::Far`] (m16) expands into two `i16` as an integer of
//! twice the width would. The pin has no far pointer; its address spaces
//! 256..258 are segment overrides on an `i32`. Exit: none. Owner sdag-b.
//! Review 2026-11-13.
//!
//! DEVIATE SD14: the floating types are always legal (x87 register classes);
//! the pin's `SoftenFloat` and `ExpandFloat` arms are not translated. Reason:
//! no target of ours lacks the x87. Exit: none. Owner sdag-b. Review
//! 2026-11-13.
//!
//! # See also
//! LLVM's `computeRegisterProperties`
//! ([L1425](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TargetLoweringBase.cpp#L1425))
//! and `getTypeConversion`
//! ([L1100](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TargetLoweringBase.cpp#L1100)).

use super::mvt::{EVT, MVT};

/// `LegalizeTypeAction`, for the integer, float and far types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeAction {
    Legal,
    PromoteInteger,
    ExpandInteger,
}

/// The simple integer types, narrowest first (the pin's `i1`..`i128` without
/// the odd widths, which are [`EVT::Int`]).
const INTEGERS: [MVT; 6] = [MVT::I1, MVT::I8, MVT::I16, MVT::I32, MVT::I64, MVT::I128];

#[derive(Clone, Debug)]
pub struct TargetLoweringInfo {
    /// Per entry of `INTEGERS`: its action and the type it becomes.
    integers: [(TypeAction, MVT); 6],
    far: bool,
    /// `getScalarShiftAmountTy`.
    shift_amount: ShiftAmount,
    /// `getSetCCResultType` for a scalar: x86 compares into an `i8`.
    set_cc_result: MVT,
}

/// What `getScalarShiftAmountTy` returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShiftAmount {
    /// x86.
    I8,
    /// The shifted type (m68k).
    ByValue,
    /// The pointer-sized integer, the default (AArch64: `i64`).
    Fixed(MVT),
}

impl TargetLoweringInfo {
    /// `legal` are the integer types a register class holds
    /// (`addRegisterClass`); `far` says the target has the far pointer
    /// type.
    pub fn new(
        legal: &[MVT],
        far: bool,
    ) -> TargetLoweringInfo {
        let is_legal = |vt: MVT| legal.contains(&vt);
        let largest = INTEGERS.iter().rposition(|&vt| is_legal(vt)).expect("no integer register class");
        let mut integers = INTEGERS.map(|vt| (TypeAction::Legal, vt));
        for above in largest + 1..INTEGERS.len() {
            integers[above] = (TypeAction::ExpandInteger, INTEGERS[above - 1]);
        }
        let mut nearest_legal = INTEGERS[largest];
        for below in (0..largest).rev() {
            if is_legal(INTEGERS[below]) {
                nearest_legal = INTEGERS[below];
            } else {
                integers[below] = (TypeAction::PromoteInteger, nearest_legal);
            }
        }
        TargetLoweringInfo { integers, far, shift_amount: ShiftAmount::I8, set_cc_result: MVT::I8 }
    }

    /// A shift's amount has the type of the value shifted
    /// (`M68kTargetLowering::getScalarShiftAmountTy`).
    pub fn with_shift_by_value_type(mut self) -> TargetLoweringInfo {
        self.shift_amount = ShiftAmount::ByValue;
        self
    }

    /// A shift's amount is a constant of `vt` (`getScalarShiftAmountTy`).
    pub fn with_shift_amount_type(
        mut self,
        vt: MVT,
    ) -> TargetLoweringInfo {
        self.shift_amount = ShiftAmount::Fixed(vt);
        self
    }

    /// A compare makes a value of `vt` (AArch64: `i32`).
    pub fn with_set_cc_result(
        mut self,
        vt: MVT,
    ) -> TargetLoweringInfo {
        self.set_cc_result = vt;
        self
    }

    pub fn shift_amount(&self) -> ShiftAmount {
        self.shift_amount
    }

    /// AArch64: `addRegisterClass` of `i32` and `i64`; a compare makes an `i32`
    /// and a shift amount is an `i64`
    /// (`AArch64TargetLowering::getSetCCResultType`).
    pub fn aarch64() -> TargetLoweringInfo {
        TargetLoweringInfo::new(&[MVT::I32, MVT::I64], false)
            .with_set_cc_result(MVT::I32)
            .with_shift_amount_type(MVT::I64)
    }

    /// m32: `addRegisterClass` of `i8`, `i16`, `i32`; no far pointer.
    pub fn x86_32() -> TargetLoweringInfo {
        TargetLoweringInfo::new(&[MVT::I8, MVT::I16, MVT::I32], false)
    }

    /// m64: `addRegisterClass` of `i8`..`i64`.
    pub fn x86_64() -> TargetLoweringInfo {
        TargetLoweringInfo::new(&[MVT::I8, MVT::I16, MVT::I32, MVT::I64], false)
    }

    /// m16: the same registers, and the far pointer (SD2).
    pub fn x86_16() -> TargetLoweringInfo {
        TargetLoweringInfo::new(&[MVT::I8, MVT::I16, MVT::I32], true)
    }

    /// `getTypeConversion`: the action for `vt` and the type it becomes.
    pub fn type_conversion(
        &self,
        vt: EVT,
    ) -> (TypeAction, EVT) {
        match vt {
            EVT::Simple(MVT::Far) => {
                assert!(self.far, "this target has no far pointer");
                (TypeAction::ExpandInteger, EVT::I16)
            }
            EVT::Simple(mvt) => match INTEGERS.iter().position(|&int| int == mvt) {
                Some(at) => {
                    let (action, to) = self.integers[at];
                    (action, EVT::Simple(to))
                }
                None => (TypeAction::Legal, vt),
            },
            EVT::Int(bits) => {
                if bits < 8 || !bits.is_power_of_two() {
                    let rounded = EVT::integer(if bits < 8 { 8 } else { bits.next_power_of_two() });
                    let next = self.type_conversion(rounded);
                    // Avoid multi-step promotion.
                    return if next.0 == TypeAction::PromoteInteger {
                        next
                    } else {
                        (TypeAction::PromoteInteger, rounded)
                    };
                }
                (TypeAction::ExpandInteger, EVT::integer(bits / 2))
            }
        }
    }

    /// `getSetCCResultType`: one type, whatever the operands (scalars only).
    pub fn set_cc_result_type(
        &self,
        _operand: EVT,
    ) -> EVT {
        EVT::Simple(self.set_cc_result)
    }

    /// The target has the far pointer type (m16).
    pub fn has_far_pointer(&self) -> bool {
        self.far
    }

    pub fn type_action(
        &self,
        vt: EVT,
    ) -> TypeAction {
        self.type_conversion(vt).0
    }

    pub fn is_type_legal(
        &self,
        vt: EVT,
    ) -> bool {
        self.type_action(vt) == TypeAction::Legal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every target compared into an `i8`; AArch64 compares into an `i32`.
    #[test]
    fn a_target_can_choose_the_type_a_compare_makes() {
        let x86 = TargetLoweringInfo::x86_32();
        assert_eq!(x86.set_cc_result_type(EVT::I64), EVT::I8);
        let wide = TargetLoweringInfo::new(&[MVT::I32, MVT::I64], false).with_set_cc_result(MVT::I32);
        assert_eq!(wide.set_cc_result_type(EVT::I64), EVT::I32);
    }
}
