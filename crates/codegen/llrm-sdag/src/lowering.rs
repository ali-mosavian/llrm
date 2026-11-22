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
//! DEVIATE SD14: the floating types are legal (x87 register classes) unless
//! the target is built with [`TargetLoweringInfo::with_soft_float`]; the
//! pin's `ExpandFloat` arms are not translated. Reason: no target of ours has
//! a float type wider than the x87's. Exit: none. Owner sdag-b. Review
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
    /// A float with no register class (`TypeSoftenFloat`): it travels as the
    /// integer of its width and its operations are library calls.
    SoftenFloat,
    /// A vector type no register class holds (`TypeScalarizeVector` and its split/widen kin; the
    /// DEVIATE SD13 subset only names it): the builder never creates such a value, it lowers a
    /// vector IR type this way only where the target lists the vector as a register class.
    ScalarizeVector,
    /// A vector of two or more lanes no register class holds, whose half does
    /// or splits again (`TypeSplitVector`): the type legalizer makes two
    /// values of the half type.
    SplitVector,
    /// A vector that a register class holds with more lanes
    /// (`TypeWidenVector`): the legalizer makes one value of the wider
    /// type, its extra lanes undefined.
    WidenVector,
}

/// The simple integer types, narrowest first (the pin's `i1`..`i128` without
/// the odd widths, which are [`EVT::Int`]).
const INTEGERS: [MVT; 6] = [MVT::I1, MVT::I8, MVT::I16, MVT::I32, MVT::I64, MVT::I128];

#[derive(Clone, Debug)]
pub struct TargetLoweringInfo {
    /// Per entry of `INTEGERS`: its action and the type it becomes.
    integers: [(TypeAction, MVT); 6],
    far: bool,
    /// No FPU: `f32` and `f64` have no register class.
    soft_float: bool,
    /// `getScalarShiftAmountTy`.
    shift_amount: ShiftAmount,
    /// `getSetCCResultType` for a scalar: x86 compares into an `i8`.
    set_cc_result: MVT,
    /// `TargetLibraryInfo::getIntSize`: the C `int` in bits, 16 for the 16-bit
    /// arches and 32 otherwise (`TargetLibraryInfo.cpp#L123`).
    int_size: u32,
    /// A library shift keeps the shift-amount type (z80's asm helpers read a
    /// byte).
    narrow_library_shift: bool,
    soft_compare_classes: bool,
    /// `f128` has a register class (AArch64 `FPR128`), no arithmetic: its
    /// operations are library calls on `q` registers.
    legal_f128: bool,
    /// `getPreferredVectorAction` is `TypeWidenVector` for every vector of two
    /// or more lanes that is not an `i1` vector (x86).
    widen_vectors: bool,
    /// The vector types a register class holds (`addRegisterClass(MVT::v16i8,
    /// &X86::VR128RegClass)`).
    vectors: Vec<MVT>,
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
        TargetLoweringInfo {
            integers,
            far,
            soft_float: false,
            shift_amount: ShiftAmount::I8,
            set_cc_result: MVT::I8,
            int_size: 32,
            narrow_library_shift: false,
            soft_compare_classes: false,
            legal_f128: false,
            widen_vectors: false,
            vectors: legal.iter().copied().filter(|vt| vt.is_vector()).collect(),
        }
    }

    /// The target has no FPU: `f32` and `f64` are softened to `i32` and
    /// `i64` (`TypeSoftenFloat`).
    pub fn with_soft_float(mut self) -> TargetLoweringInfo {
        self.soft_float = true;
        self
    }

    /// `f128` is a legal type (`addRegisterClass(MVT::f128,
    /// &AArch64::FPR128RegClass)`).
    pub fn with_widen_vectors(mut self) -> TargetLoweringInfo {
        self.widen_vectors = true;
        self
    }

    pub fn with_legal_f128(mut self) -> TargetLoweringInfo {
        self.legal_f128 = true;
        self
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

    /// The C `int` of a 16-bit target (`isArch16Bit`).
    pub fn with_int_size(
        mut self,
        bits: u32,
    ) -> TargetLoweringInfo {
        self.int_size = bits;
        self
    }

    /// DEVIATE: the library shifts of this target take the amount in the
    /// shift-amount type, where the pin makes it an `int`. Owner:
    /// llrm-target-z80 (asm helpers read a byte). Exit: the z80 change that
    /// sets `with_int_size(16)` with `ld b,c` in the helpers, in one commit
    /// (2026-11-20).
    pub fn with_narrow_library_shift(mut self) -> TargetLoweringInfo {
        self.narrow_library_shift = true;
        self
    }

    pub fn narrow_library_shift(&self) -> bool {
        self.narrow_library_shift
    }

    /// DEVIATE SOFT-CMP-CSE: soften a float compare by the routine's class,
    /// not its name: `eq`, `ne`, `lt` and `le` are all `__lesf2`'s three-way
    /// result (-1, 0, 1; unordered 1), `gt` and `ge` are `__gesf2`'s (unordered
    /// -1). The pin calls the routine named for each predicate, so six
    /// predicates of one pair are six calls where two answer them. For a
    /// runtime whose four low routines are aliases of one core and whose two
    /// high ones are (compiler-rt's, which m68k, z80 and 6502 ship). Owner w8,
    /// exit 2026-11-27.
    pub fn with_soft_compare_classes(mut self) -> TargetLoweringInfo {
        self.soft_compare_classes = true;
        self
    }

    pub fn soft_compare_classes(&self) -> bool {
        self.soft_compare_classes
    }

    /// `getIntSize`: the type a library shift takes its amount in
    /// (`ExpandIntRes_Shift`).
    pub fn int_size(&self) -> u32 {
        self.int_size
    }

    pub fn shift_amount(&self) -> ShiftAmount {
        self.shift_amount
    }

    /// AArch64: `addRegisterClass` of `i32` and `i64`; a compare makes an `i32`
    /// and a shift amount is an `i64`
    /// (`AArch64TargetLowering::getSetCCResultType`). The 128-bit integer and
    /// float vectors are in `FPR128` (`addQRTypeForNEON`); the builder makes
    /// one only where `DagConfig::vectors` is on.
    pub fn aarch64() -> TargetLoweringInfo {
        TargetLoweringInfo::new(
            &[
                MVT::I32,
                MVT::I64,
                MVT::V16I8,
                MVT::V8I16,
                MVT::V4I32,
                MVT::V2I64,
                MVT::V4F32,
                MVT::V2F64,
                MVT::V2F32,
                MVT::V8I8,
                MVT::V4I16,
                MVT::V2I32,
            ],
            false,
        )
        .with_set_cc_result(MVT::I32)
        .with_shift_amount_type(MVT::I64)
        .with_legal_f128()
    }

    /// m32: `addRegisterClass` of `i8`, `i16`, `i32`; no far pointer.
    pub fn x86_32() -> TargetLoweringInfo {
        TargetLoweringInfo::new(&[MVT::I8, MVT::I16, MVT::I32], false)
    }

    /// m64: `addRegisterClass` of `i8`..`i64`.
    pub fn x86_64() -> TargetLoweringInfo {
        TargetLoweringInfo::new(&[MVT::I8, MVT::I16, MVT::I32, MVT::I64], false)
    }

    /// m64 with `hasSSE2()`: the integer vectors join `VR128` (`X86ISelLowering.cpp`, the SSE2 arm:
    /// `addRegisterClass(MVT::v16i8, ...)` and the other three).
    pub fn x86_64_sse2() -> TargetLoweringInfo {
        TargetLoweringInfo::new(
            &[
                MVT::I8,
                MVT::I16,
                MVT::I32,
                MVT::I64,
                MVT::V16I8,
                MVT::V8I16,
                MVT::V4I32,
                MVT::V2I64,
                MVT::V4F32,
                MVT::V2F64,
            ],
            false,
        )
        .with_widen_vectors()
    }

    /// m16: the same registers, and the far pointer (SD2).
    pub fn x86_16() -> TargetLoweringInfo {
        TargetLoweringInfo::new(&[MVT::I8, MVT::I16, MVT::I32], true).with_int_size(16)
    }

    /// The vector types with a register class.
    pub fn legal_vectors(&self) -> &[MVT] {
        &self.vectors
    }

    /// The vector types a register holds after one conversion, as
    /// `getVectorTypeBreakdown` first asks: a type widened or promoted to a
    /// legal one (`v2i8` is `v16i8` on x86, `v4i8` is `v4i16` on arm64),
    /// each with that type.
    pub fn vector_registers(&self) -> Vec<(MVT, MVT)> {
        MVT::vectors()
            .filter(|&vt| !self.is_type_legal(vt.into()))
            .filter_map(|vt| match self.type_conversion(vt.into()) {
                (TypeAction::WidenVector | TypeAction::PromoteInteger, EVT::Simple(to)) if self.vectors.contains(&to) => {
                    Some((vt, to))
                }
                _ => None,
            })
            .collect()
    }

    /// `computeRegisterProperties`, the `TypeWidenVector` case: a power-of-two
    /// count of lanes widens to the narrowest legal vector of the same
    /// element with more lanes, any other count to the next power of two if
    /// that is legal.
    fn widened_vector(
        &self,
        mvt: MVT,
    ) -> Option<MVT> {
        let (element, lanes) = (mvt.element()?, mvt.lanes()?);
        if !self.widen_vectors || lanes == 1 || element == MVT::I1 {
            return None;
        }
        if lanes.is_power_of_two() {
            MVT::vectors()
                .filter(|wider| wider.element() == Some(element) && wider.lanes() > Some(lanes))
                .filter(|wider| self.vectors.contains(wider))
                .min_by_key(|wider| wider.lanes())
        } else {
            MVT::vector(element, lanes.next_power_of_two()).filter(|wider| self.vectors.contains(wider))
        }
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
                None => match mvt {
                    _ if mvt.is_vector() => {
                        // `getPreferredVectorAction` (power-of-two lanes) and
                        // `computeRegisterProperties`: promote the lanes to the
                        // narrowest wider integer type that makes a legal
                        // vector of as many lanes (`v4i1` is `v4i16` where
                        // `v4i8` is not a type, `v4i8` is `v4i16`, `v2i8` is
                        // `v2i32`).
                        let promoted = mvt
                            .element()
                            .zip(mvt.lanes())
                            .filter(|&(element, lanes)| {
                                lanes.is_power_of_two()
                                    && EVT::Simple(element).is_integer()
                                    && !(self.widen_vectors && element != MVT::I1)
                            })
                            .and_then(|(element, lanes)| {
                                let bits = EVT::Simple(element).size_in_bits();
                                [MVT::I8, MVT::I16, MVT::I32, MVT::I64]
                                    .into_iter()
                                    .filter(|&lane| EVT::Simple(lane).size_in_bits() > bits)
                                    .filter_map(|lane| MVT::vector(lane, lanes))
                                    .find(|wider| self.vectors.contains(wider))
                            });
                        if self.vectors.contains(&mvt) {
                            (TypeAction::Legal, vt)
                        } else if let Some(wider) = self.widened_vector(mvt) {
                            (TypeAction::WidenVector, EVT::Simple(wider))
                        } else if let Some(wider) = promoted {
                            (TypeAction::PromoteInteger, EVT::Simple(wider))
                        } else if let Some(half) = mvt
                            .element()
                            .zip(mvt.lanes())
                            .and_then(|(element, lanes)| MVT::vector(element, lanes / 2))
                            .filter(|_| !self.vectors.is_empty())
                        {
                            // `getTypeConversion`: split in two, if a half is a
                            // type.
                            (TypeAction::SplitVector, EVT::Simple(half))
                        } else {
                            (TypeAction::ScalarizeVector, EVT::Simple(mvt.element().expect("a vector")))
                        }
                    }
                    MVT::F32 if self.soft_float => (TypeAction::SoftenFloat, EVT::I32),
                    MVT::F64 if self.soft_float => (TypeAction::SoftenFloat, EVT::I64),
                    // `f128` is softened to the `i128` of its bits unless the
                    // target has a quad register
                    MVT::F128 if !self.legal_f128 => (TypeAction::SoftenFloat, EVT::I128),
                    _ => (TypeAction::Legal, vt),
                },
            },
            // `getTypeConversion`, `TypeWidenVector` of a lane count that is no power of two (`v3i8`): the
            // next power of two lanes (`v4i8`), which is a vector the pass looks at again.
            EVT::Vector(element, lanes) => (TypeAction::WidenVector, EVT::vector(element, lanes.next_power_of_two())),
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

    /// `getSetCCResultType`: one type for scalars; a vector compare makes the
    /// integer vector of its operand's lane width (AArch64).
    pub fn set_cc_result_type(
        &self,
        operand: EVT,
    ) -> EVT {
        if operand.is_vector() {
            return operand.change_vector_element_type_to_integer();
        }
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

    /// `getTypeToExpandTo`: `vt` halved until a legal type is reached.
    pub fn type_to_expand_to(
        &self,
        mut vt: EVT,
    ) -> EVT {
        while !self.is_type_legal(vt) {
            vt = self.type_conversion(vt).1;
        }
        vt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A soft-float target would keep `f32` in an x87 register without the
    /// flag; with it, `f32` and `f64` travel as `i32` and `i64`.
    #[test]
    fn soft_float_softens_f32_and_f64_only_when_asked() {
        let fpu = TargetLoweringInfo::x86_16();
        assert_eq!(fpu.type_conversion(EVT::Simple(MVT::F32)), (TypeAction::Legal, EVT::Simple(MVT::F32)));
        let soft = TargetLoweringInfo::x86_16().with_soft_float();
        assert_eq!(soft.type_conversion(EVT::Simple(MVT::F32)), (TypeAction::SoftenFloat, EVT::I32));
        assert_eq!(soft.type_conversion(EVT::Simple(MVT::F64)), (TypeAction::SoftenFloat, EVT::I64));
        assert_eq!(soft.type_conversion(EVT::I32), (TypeAction::Legal, EVT::I32));
    }

    /// A vector type is legal only where a register class holds it: SSE2 m64 yes, every other target
    /// scalarizes (so no other target ever builds a vector value).
    #[test]
    fn a_vector_type_is_legal_only_with_a_register_class() {
        let v = EVT::Simple(MVT::V16I8);
        assert!(TargetLoweringInfo::x86_64_sse2().is_type_legal(v));
        assert_eq!(TargetLoweringInfo::x86_64().type_conversion(v), (TypeAction::ScalarizeVector, EVT::I8));
        assert_eq!(TargetLoweringInfo::x86_32().type_action(EVT::Simple(MVT::V2I64)), TypeAction::ScalarizeVector);
        assert_eq!(TargetLoweringInfo::aarch64().type_action(EVT::Simple(MVT::V4I32)), TypeAction::Legal);
        assert_eq!(TargetLoweringInfo::aarch64().type_action(EVT::Simple(MVT::V16I8)), TypeAction::Legal);
    }

    /// A vector under a D register has no register class: its lanes are
    /// promoted to the narrowest wider type that makes a legal vector of as many
    /// lanes (`getPreferredVectorAction`); `v4i8` was `ScalarizeVector`, which no
    /// backend could build.
    #[test]
    fn a_vector_under_a_d_register_has_its_lanes_promoted() {
        let aarch64 = TargetLoweringInfo::aarch64();
        let promoted = |vt| aarch64.type_conversion(EVT::Simple(vt));
        assert_eq!(promoted(MVT::V4I8), (TypeAction::PromoteInteger, EVT::Simple(MVT::V4I16)));
        assert_eq!(promoted(MVT::V2I8), (TypeAction::PromoteInteger, EVT::Simple(MVT::V2I32)));
        assert_eq!(promoted(MVT::V2I16), (TypeAction::PromoteInteger, EVT::Simple(MVT::V2I32)));
        assert_eq!(promoted(MVT::V4I1), (TypeAction::PromoteInteger, EVT::Simple(MVT::V4I16)));
        assert_eq!(promoted(MVT::V8I8), (TypeAction::Legal, EVT::Simple(MVT::V8I8)));
    }

    /// x86 widens `v4i8` (`X86TargetLowering::getPreferredVectorAction`); promoting
    /// it to `v4i32` would pre-empt the widening and diverge from the pin.
    #[test]
    fn x86_does_not_promote_the_lanes_of_a_narrow_vector() {
        let x86 = TargetLoweringInfo::x86_64_sse2();
        assert_eq!(x86.type_conversion(EVT::Simple(MVT::V4I8)), (TypeAction::WidenVector, MVT::V16I8.into()));
        assert_eq!(x86.type_conversion(EVT::Simple(MVT::V2I16)), (TypeAction::WidenVector, MVT::V8I16.into()));
    }

    /// A narrow vector is carried in the legal register type its conversion
    /// names (`getVectorTypeBreakdown`'s first step); a legal one is not listed.
    #[test]
    fn a_narrow_vector_is_carried_in_the_register_its_conversion_names() {
        let x86 = TargetLoweringInfo::x86_64_sse2().vector_registers();
        assert!(x86.contains(&(MVT::V2I8, MVT::V16I8)) && x86.contains(&(MVT::V4I16, MVT::V8I16)));
        assert!(!x86.iter().any(|&(from, _)| from == MVT::V16I8));
        let arm = TargetLoweringInfo::aarch64().vector_registers();
        assert!(arm.contains(&(MVT::V4I8, MVT::V4I16)));
    }

    /// Every target compared into an `i8`; AArch64 compares into an `i32`.
    #[test]
    fn a_target_can_choose_the_type_a_compare_makes() {
        let x86 = TargetLoweringInfo::x86_32();
        assert_eq!(x86.set_cc_result_type(EVT::I64), EVT::I8);
        let wide = TargetLoweringInfo::new(&[MVT::I32, MVT::I64], false).with_set_cc_result(MVT::I32);
        assert_eq!(wide.set_cc_result_type(EVT::I64), EVT::I32);
    }

    /// A vector under a register has no action but widening on x86
    /// (`getPreferredVectorAction`): `v8i8` was a split down to a lane with
    /// no register, and the compile stopped.
    #[test]
    fn x86_widens_a_vector_under_a_register_and_arm64_holds_it() {
        let x86 = TargetLoweringInfo::x86_64_sse2();
        for (narrow, wide) in [(MVT::V8I8, MVT::V16I8), (MVT::V4I16, MVT::V8I16), (MVT::V2I32, MVT::V4I32)] {
            assert_eq!(x86.type_conversion(narrow.into()), (TypeAction::WidenVector, wide.into()));
        }
        assert_eq!(x86.type_action(MVT::V32I8.into()), TypeAction::SplitVector);
        assert_eq!(x86.type_action(MVT::V16I8.into()), TypeAction::Legal);
        assert_eq!(TargetLoweringInfo::aarch64().type_action(MVT::V8I8.into()), TypeAction::Legal);
    }
}
