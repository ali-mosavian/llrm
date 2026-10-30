//! Which classes a floating-point value can be, and what its sign bit is:
//! LLVM's `KnownFPClass`, the type `computeKnownFPClass` returns.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` (`llvm/lib/Support/KnownFPClass.cpp`,
//! `llvm/include/llvm/Support/KnownFPClass.h`): same operations, same order of
//! cases, so a class LLVM rules out is a class this rules out. Each function
//! links the line it translates. A [`KnownFpClass`] is the mask of
//! [`FpClassTest`] the value may be (`fcNone` is poison) and the sign bit when
//! known.
//!
//! DEVIATE: a default argument (`DenormalMode Mode =
//! DenormalMode::getDynamic()`) is an explicit argument; callers pass
//! `DenormalMode::get_dynamic()`. A static and a member of one name (`fneg`,
//! `fabs`, `copysign`) are `fneg` and `fneg_of`; the overloads `fmul` of an
//! `APFloat` and `ldexp` of a `KnownBits` are `fmul_apfloat` and
//! `ldexp_known_bits`. `ldexp`'s two `APInt` bounds are the signed numbers they
//! hold. `bitcast` and `to_known_bits` answer for `float`, `double` and x87 and
//! return unknown for the 8-bit test formats, as LLVM does for every format it
//! has not validated; half, bfloat and quad are not here
//! (see [`crate::apfloat`]). `operator<<` is not translated.
//!
//! # Example
//! `fadd` of a value that is never negative and a value that is never negative
//! is never negative, and never a NaN unless an operand is one.
//!
//! # See also
//! - Uses: [`crate::apfloat`], [`crate::fpclass`], [`crate::knownbits`]

use std::ops::{BitOr, BitOrAssign};

use crate::apfloat::{
    ApFloat, CmpResult, Sem, SemanticsKind, is_ieee_like_fp, is_representable_as_normal_in, semantics_precision,
};
use crate::apint::{bit, mask};
use crate::fpclass::*;
use crate::knownbits::KnownBits;

/// `KnownFPClass::MinMaxKind`.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L233>
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MinMaxKind {
    Minimum,
    Maximum,
    Minimumnum,
    Maximumnum,
    Minnum,
    Maxnum,
}

/// The classes a value could be one of, and its sign bit if known.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L27>
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KnownFpClass {
    pub known_fp_classes_value: FpClassTest,
    pub sign_bit_value: Option<bool>,
}

impl Default for KnownFpClass {
    /// `KnownFPClass()`: nothing known.
    fn default() -> Self {
        Self { known_fp_classes_value: FC_ALL_FLAGS, sign_bit_value: None }
    }
}

/// `std::optional<bool> == bool`.
fn is_sign(
    sign: Option<bool>,
    value: bool,
) -> bool {
    sign == Some(value)
}

impl KnownFpClass {
    pub const ORDERED_LESS_THAN_ZERO_MASK: FpClassTest =
        FpClassTest(FC_NEG_SUBNORMAL.0 | FC_NEG_NORMAL.0 | FC_NEG_INF.0);
    pub const ORDERED_GREATER_THAN_ZERO_MASK: FpClassTest =
        FpClassTest(FC_POS_SUBNORMAL.0 | FC_POS_NORMAL.0 | FC_POS_INF.0);

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L31>
    pub fn get_known_fp_classes(&self) -> FpClassTest {
        self.known_fp_classes_value
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L34>
    pub fn set_known_fp_classes(
        &mut self,
        classes: FpClassTest,
    ) {
        self.known_fp_classes_value = classes;
    }

    /// `None` if the sign bit is unknown.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L38>
    pub fn get_sign_bit(&self) -> Option<bool> {
        self.sign_bit_value
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L40>
    pub fn set_sign_bit(
        &mut self,
        sign: Option<bool>,
    ) {
        self.sign_bit_value = sign;
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L42>
    pub fn new(
        known: FpClassTest,
        sign: Option<bool>,
    ) -> Self {
        Self { known_fp_classes_value: known, sign_bit_value: sign }
    }

    /// The class and sign of a constant.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L21>
    pub fn from_apfloat(c: &ApFloat) -> Self {
        let mut known = Self { known_fp_classes_value: c.classify(), sign_bit_value: None };
        known.set_sign_bit(Some(c.is_negative()));
        known
    }

    /// Whether it's known this can never be one of the mask entries.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L52>
    pub fn is_known_never(
        &self,
        mask: FpClassTest,
    ) -> bool {
        (self.get_known_fp_classes() & mask) == FC_NONE
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L56>
    pub fn is_known_always(
        &self,
        mask: FpClassTest,
    ) -> bool {
        self.is_known_never(!mask)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L58>
    pub fn is_unknown(&self) -> bool {
        self.get_known_fp_classes() == FC_ALL_FLAGS && self.get_sign_bit().is_none()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L63>
    pub fn is_known_never_nan(&self) -> bool {
        self.is_known_never(FC_NAN)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L66>
    pub fn is_known_always_nan(&self) -> bool {
        self.is_known_always(FC_NAN)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L69>
    pub fn is_known_never_infinity(&self) -> bool {
        self.is_known_never(FC_INF)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L72>
    pub fn is_known_never_inf_or_nan(&self) -> bool {
        self.is_known_never(FC_INF | FC_NAN)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L75>
    pub fn is_known_never_pos_infinity(&self) -> bool {
        self.is_known_never(FC_POS_INF)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L78>
    pub fn is_known_never_neg_infinity(&self) -> bool {
        self.is_known_never(FC_NEG_INF)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L81>
    pub fn is_known_never_subnormal(&self) -> bool {
        self.is_known_never(FC_SUBNORMAL)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L84>
    pub fn is_known_never_pos_subnormal(&self) -> bool {
        self.is_known_never(FC_POS_SUBNORMAL)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L87>
    pub fn is_known_never_neg_subnormal(&self) -> bool {
        self.is_known_never(FC_NEG_SUBNORMAL)
    }

    /// A literal [+-]0; denormal inputs implicitly treated as 0 are not
    /// included.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L91>
    pub fn is_known_never_zero(&self) -> bool {
        self.is_known_never(FC_ZERO)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L94>
    pub fn is_known_never_pos_zero(&self) -> bool {
        self.is_known_never(FC_POS_ZERO)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L98>
    pub fn is_known_never_neg_zero(&self) -> bool {
        self.is_known_never(FC_NEG_ZERO)
    }

    /// Whether it's known this can never be interpreted as a zero.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L37>
    pub fn is_known_never_logical_zero(
        &self,
        mode: DenormalMode,
    ) -> bool {
        self.is_known_never_zero() && (self.is_known_never_subnormal() || input_denormal_is_ieee(mode))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L42>
    pub fn is_known_never_logical_neg_zero(
        &self,
        mode: DenormalMode,
    ) -> bool {
        self.is_known_never_neg_zero()
            && (self.is_known_never_neg_subnormal() || input_denormal_is_ieee_or_pos_zero(mode))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L47>
    pub fn is_known_never_logical_pos_zero(
        &self,
        mode: DenormalMode,
    ) -> bool {
        if !self.is_known_never_pos_zero() {
            return false;
        }
        // If we know there are no denormals, nothing can be flushed to zero.
        if self.is_known_never_subnormal() {
            return true;
        }
        match mode.input {
            DenormalModeKind::Ieee => true,
            // Negative subnormal won't flush to +0
            DenormalModeKind::PreserveSign => self.is_known_never_pos_subnormal(),
            // Both positive and negative subnormal could flush to +0
            _ => false,
        }
    }

    /// Whether the value is NaN or never less than -0.0.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L126>
    pub fn cannot_be_ordered_less_than_zero(&self) -> bool {
        self.is_known_never(Self::ORDERED_LESS_THAN_ZERO_MASK)
    }

    /// Whether the value is NaN or never greater than -0.0.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L137>
    pub fn cannot_be_ordered_greater_than_zero(&self) -> bool {
        self.is_known_never(Self::ORDERED_GREATER_THAN_ZERO_MASK)
    }

    /// Whether this can never be a positive value or a logical 0.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L148>
    pub fn cannot_be_ordered_less_eq_zero(
        &self,
        mode: DenormalMode,
    ) -> bool {
        self.is_known_never(FC_NEGATIVE) && self.is_known_never_logical_pos_zero(mode)
    }

    /// Whether this can never be a negative value or a logical 0.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L159>
    pub fn cannot_be_ordered_greater_eq_zero(
        &self,
        mode: DenormalMode,
    ) -> bool {
        self.is_known_never(FC_POSITIVE) && self.is_known_never_logical_neg_zero(mode)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L163>
    pub fn intersect_with(
        &self,
        rhs: &KnownFpClass,
    ) -> KnownFpClass {
        KnownFpClass::new(
            self.get_known_fp_classes() | rhs.get_known_fp_classes(),
            if self.get_sign_bit() == rhs.get_sign_bit() { self.get_sign_bit() } else { None },
        )
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L169>
    pub fn union_with(
        &self,
        rhs: &KnownFpClass,
    ) -> KnownFpClass {
        let mut merged_sign_bit = None;
        if self.get_sign_bit().is_some() && rhs.get_sign_bit().is_none() {
            merged_sign_bit = self.get_sign_bit();
        } else if self.get_sign_bit().is_none() && rhs.get_sign_bit().is_some() {
            merged_sign_bit = rhs.get_sign_bit();
        }
        KnownFpClass::new(self.get_known_fp_classes() & rhs.get_known_fp_classes(), merged_sign_bit)
    }

    /// `operator|=`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L180>
    pub fn or_assign(
        &mut self,
        rhs: &KnownFpClass,
    ) {
        self.set_known_fp_classes(self.get_known_fp_classes() | rhs.get_known_fp_classes());
        if self.get_sign_bit() != rhs.get_sign_bit() {
            self.set_sign_bit(None);
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L188>
    pub fn known_not(
        &mut self,
        rule_out: FpClassTest,
    ) {
        self.set_known_fp_classes(self.get_known_fp_classes() & !rule_out);
        if self.is_known_never(FC_NAN) && self.get_sign_bit().is_none() {
            if self.is_known_never(FC_NEGATIVE) {
                self.set_sign_bit(Some(false));
            } else if self.is_known_never(FC_POSITIVE) {
                self.set_sign_bit(Some(true));
            }
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L198>
    pub fn fneg(&mut self) {
        self.set_known_fp_classes(crate::fpclass::fneg(self.get_known_fp_classes()));
        if let Some(sign) = self.get_sign_bit() {
            self.set_sign_bit(Some(!sign));
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L204>
    pub fn fneg_of(src: &KnownFpClass) -> KnownFpClass {
        let mut known = *src;
        known.fneg();
        known
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L210>
    pub fn fabs(&mut self) {
        if self.get_known_fp_classes().any(FC_NEG_ZERO) {
            self.set_known_fp_classes(self.get_known_fp_classes() | FC_POS_ZERO);
        }
        if self.get_known_fp_classes().any(FC_NEG_INF) {
            self.set_known_fp_classes(self.get_known_fp_classes() | FC_POS_INF);
        }
        if self.get_known_fp_classes().any(FC_NEG_SUBNORMAL) {
            self.set_known_fp_classes(self.get_known_fp_classes() | FC_POS_SUBNORMAL);
        }
        if self.get_known_fp_classes().any(FC_NEG_NORMAL) {
            self.set_known_fp_classes(self.get_known_fp_classes() | FC_POS_NORMAL);
        }
        self.sign_bit_must_be_zero();
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L226>
    pub fn fabs_of(src: &KnownFpClass) -> KnownFpClass {
        let mut known = *src;
        known.fabs();
        known
    }

    /// Min and max of two values.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L97>
    pub fn min_max_like(
        lhs: &KnownFpClass,
        rhs: &KnownFpClass,
        kind: MinMaxKind,
        mode: DenormalMode,
    ) -> KnownFpClass {
        let mut known_lhs = *lhs;
        let mut known_rhs = *rhs;
        let never_nan = known_lhs.is_known_never_nan() || known_rhs.is_known_never_nan();
        let mut known = known_lhs | known_rhs;

        // If either operand is not NaN, the result is not NaN.
        if never_nan
            && (kind == MinMaxKind::Minnum
                || kind == MinMaxKind::Maxnum
                || kind == MinMaxKind::Minimumnum
                || kind == MinMaxKind::Maximumnum)
        {
            known.known_not(FC_NAN);
        }

        if kind == MinMaxKind::Maxnum || kind == MinMaxKind::Maximumnum {
            if known_lhs.is_known_never_nan() {
                known.known_not(ordered_strictly_less(known_lhs.get_known_fp_classes(), false));
            }
            if known_rhs.is_known_never_nan() {
                known.known_not(ordered_strictly_less(known_rhs.get_known_fp_classes(), false));
            }
        } else if kind == MinMaxKind::Maximum {
            known.known_not(
                ordered_strictly_less(known_lhs.get_known_fp_classes(), false)
                    | ordered_strictly_less(known_rhs.get_known_fp_classes(), false),
            );
        } else if kind == MinMaxKind::Minnum || kind == MinMaxKind::Minimumnum {
            if known_lhs.is_known_never_nan() {
                known.known_not(ordered_strictly_greater(known_lhs.get_known_fp_classes(), false));
            }
            if known_rhs.is_known_never_nan() {
                known.known_not(ordered_strictly_greater(known_rhs.get_known_fp_classes(), false));
            }
        } else if kind == MinMaxKind::Minimum {
            known.known_not(
                ordered_strictly_greater(known_lhs.get_known_fp_classes(), false)
                    | ordered_strictly_greater(known_rhs.get_known_fp_classes(), false),
            );
        } else {
            unreachable!("unhandled intrinsic");
        }

        // Fixup zero handling if denormals could be returned as a zero.
        //
        // As there's no spec for denormal flushing, be conservative with the
        // treatment of denormals that could be flushed to zero. For older
        // subtargets on AMDGPU the min/max instructions would not flush the
        // output and return the original value.
        if (known.get_known_fp_classes() & FC_ZERO) != FC_NONE
            && !known.is_known_never_subnormal()
            && mode != DenormalMode::get_ieee()
        {
            known.set_known_fp_classes(known.get_known_fp_classes() | FC_ZERO);
        }

        if known.is_known_never_nan() {
            if known_lhs.get_sign_bit().is_some()
                && known_rhs.get_sign_bit().is_some()
                && known_lhs.get_sign_bit() == known_rhs.get_sign_bit()
            {
                if known_lhs.get_sign_bit() == Some(true) {
                    known.sign_bit_must_be_one();
                } else {
                    known.sign_bit_must_be_zero();
                }
            } else if (kind == MinMaxKind::Maximum
                || kind == MinMaxKind::Minimum
                || kind == MinMaxKind::Maximumnum
                || kind == MinMaxKind::Minimumnum)
                // FIXME: Should be using logical zero versions
                || ((known_lhs.is_known_never_neg_zero() || known_rhs.is_known_never_pos_zero())
                    && (known_lhs.is_known_never_pos_zero() || known_rhs.is_known_never_neg_zero()))
            {
                // Don't take sign bit from NaN operands.
                if !known_lhs.is_known_never_nan() {
                    known_lhs.set_sign_bit(None);
                }
                if !known_rhs.is_known_never_nan() {
                    known_rhs.set_sign_bit(None);
                }
                if (kind == MinMaxKind::Maximum || kind == MinMaxKind::Maximumnum || kind == MinMaxKind::Maxnum)
                    && (is_sign(known_lhs.get_sign_bit(), false) || is_sign(known_rhs.get_sign_bit(), false))
                {
                    known.sign_bit_must_be_zero();
                } else if (kind == MinMaxKind::Minimum || kind == MinMaxKind::Minimumnum || kind == MinMaxKind::Minnum)
                    && (is_sign(known_lhs.get_sign_bit(), true) || is_sign(known_rhs.get_sign_bit(), true))
                {
                    known.sign_bit_must_be_one();
                }
            }
        }

        known
    }

    /// Apply the canonicalize intrinsic to this value.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L178>
    pub fn canonicalize(
        known_src: &KnownFpClass,
        denorm_mode: DenormalMode,
    ) -> KnownFpClass {
        let mut known = KnownFpClass::default();

        // This is essentially a stronger form of propagateCanonicalizingSrc.
        // Other "canonicalizing" operations don't actually have an IR
        // canonicalization guarantee.

        // Canonicalize may flush denormals to zero, so we have to consider the
        // denormal mode to preserve known-not-0 knowledge.
        known.set_known_fp_classes(known_src.get_known_fp_classes() | FC_ZERO | FC_QNAN);

        // Stronger version of propagateNaN. Canonicalize is guaranteed to
        // quiet signaling nans.
        if known_src.is_known_never_nan() {
            known.known_not(FC_NAN);
        } else {
            known.known_not(FC_SNAN);
        }

        // FIXME: Missing check of IEEE like types.

        // If the parent function flushes denormals, the canonical output cannot
        // be a denormal.
        if denorm_mode == DenormalMode::get_ieee() {
            if known_src.is_known_never(FC_POS_ZERO) {
                known.known_not(FC_POS_ZERO);
            }
            if known_src.is_known_never(FC_NEG_ZERO) {
                known.known_not(FC_NEG_ZERO);
            }
            return known;
        }

        if denorm_mode.inputs_are_zero() || denorm_mode.outputs_are_zero() {
            known.known_not(FC_SUBNORMAL);
        }

        if denorm_mode == DenormalMode::get_preserve_sign() {
            if known_src.is_known_never(FC_POS_ZERO | FC_POS_SUBNORMAL) {
                known.known_not(FC_POS_ZERO);
            }
            if known_src.is_known_never(FC_NEG_ZERO | FC_NEG_SUBNORMAL) {
                known.known_not(FC_NEG_ZERO);
            }
            return known;
        }

        if denorm_mode.input == DenormalModeKind::PositiveZero
            || (denorm_mode.output == DenormalModeKind::PositiveZero && denorm_mode.input == DenormalModeKind::Ieee)
        {
            // -0.0 is not a subnormal and should not be flushed.
            if known_src.is_known_never(FC_NEG_ZERO) {
                known.known_not(FC_NEG_ZERO);
            }
            if known_src.is_known_never(FC_POS_ZERO | FC_SUBNORMAL) {
                known.known_not(FC_POS_ZERO);
            }
        }

        known
    }

    /// Report known values for a bitcast into a float with provided semantics.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L234>
    pub fn bitcast(
        flt_semantics: Sem,
        bits: &KnownBits,
    ) -> KnownFpClass {
        assert!(flt_semantics.size_in_bits == bits.width, "Bitcast operand has incorrect bit width");
        let mut known = KnownFpClass::default();

        // Conflicting known bits do not describe a concrete value. Return
        // unknown.
        if bits.has_conflict() {
            return known;
        }

        // Return unknown for types we have not validated.
        if !is_supported(flt_semantics) {
            return known;
        }

        // Transfer information from the sign bit.
        if bits.is_non_negative() {
            known.sign_bit_must_be_zero();
        } else if bits.is_negative() {
            known.sign_bit_must_be_one();
        }

        if is_ieee_like_fp(flt_semantics) {
            let mantissa_bits = semantics_precision(flt_semantics) - 1;
            let exponent_mask = mask::<u128>(flt_semantics.size_in_bits - 1) & !mask::<u128>(mantissa_bits);
            let mantissa_mask = mask::<u128>(mantissa_bits);

            let exponent_known_all_zeros = (bits.zero & exponent_mask) == exponent_mask;
            let exponent_known_all_ones = (bits.one & exponent_mask) == exponent_mask;
            let exponent_known_not_all_zeros = (bits.one & exponent_mask) != 0;
            let exponent_known_not_all_ones = (bits.zero & exponent_mask) != 0;

            let mantissa_known_all_zeros = (bits.zero & mantissa_mask) == mantissa_mask;
            let mantissa_known_not_all_zeros = (bits.one & mantissa_mask) != 0;

            // Zero and subnormal require an exponent with all zero bits.
            if exponent_known_not_all_zeros {
                known.known_not(FC_ZERO | FC_SUBNORMAL);
            }

            // Infinity and NaN require an exponent with all one bits.
            if exponent_known_not_all_ones {
                known.known_not(FC_INF | FC_NAN);
            }

            // Normal values have an exponent that is not all zeros or all
            // ones.
            if exponent_known_all_zeros || exponent_known_all_ones {
                known.known_not(FC_NORMAL);
            }

            // Zero and infinity require a mantissa with all zero bits.
            if mantissa_known_not_all_zeros {
                known.known_not(FC_ZERO | FC_INF);
            }

            // Subnormal and NaN require a non-zero mantissa.
            if mantissa_known_all_zeros {
                known.known_not(FC_SUBNORMAL | FC_NAN);
            }

            let quiet_bit_known_set = bit(bits.one, mantissa_bits - 1);
            let quiet_bit_known_clear = bit(bits.zero, mantissa_bits - 1);

            if quiet_bit_known_set {
                known.known_not(FC_SNAN);
            } else if quiet_bit_known_clear {
                known.known_not(FC_QNAN);
            }
        }

        known
    }

    /// Report known bits for a float with provided semantics.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L316>
    pub fn to_known_bits(
        &self,
        flt_semantics: Sem,
    ) -> KnownBits {
        let mut known = KnownBits::unknown(flt_semantics.size_in_bits);
        let fp_classes = self.get_known_fp_classes();

        // Return unknown if poison.
        if fp_classes == FC_NONE {
            return known;
        }

        // Return unknown for types we have not validated.
        if !is_supported(flt_semantics) {
            return known;
        }

        if self.is_known_never(FC_NORMAL | FC_SUBNORMAL | FC_NAN) {
            known.set_all_conflict();

            if fp_classes.any(FC_INF) {
                known = known.intersect_with(&KnownBits::constant(
                    flt_semantics.size_in_bits,
                    ApFloat::get_inf(flt_semantics, false).to_bits(),
                ));
            }

            if fp_classes.any(FC_ZERO) {
                known = known.intersect_with(&KnownBits::constant(flt_semantics.size_in_bits, 0));
            }

            let sign = 1_u128 << (flt_semantics.size_in_bits - 1);
            known.zero &= !sign;
            known.one &= !sign;
        }

        if let Some(sign) = self.get_sign_bit() {
            if sign {
                known.make_negative();
            } else {
                known.make_non_negative();
            }
        }

        known
    }

    /// Report known values for fadd.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L401>
    pub fn fadd(
        known_lhs: &KnownFpClass,
        known_rhs: &KnownFpClass,
        mode: DenormalMode,
    ) -> KnownFpClass {
        let mut known = fadd_impl(known_lhs, known_rhs, mode);

        // (fadd x, 0.0) is guaranteed to return +0.0, not -0.0.
        if (known_lhs.is_known_never_logical_neg_zero(mode) || known_rhs.is_known_never_logical_neg_zero(mode))
            // Make sure output negative denormal can't flush to -0
            && (mode.output == DenormalModeKind::Ieee || mode.output == DenormalModeKind::PositiveZero)
        {
            known.known_not(FC_NEG_ZERO);
        }

        known.propagate_non_snan2(known_lhs, known_rhs);

        known
    }

    /// Report known values for fadd x, x.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L419>
    pub fn fadd_self(
        known_src: &KnownFpClass,
        mode: DenormalMode,
    ) -> KnownFpClass {
        let mut known = KnownFpClass::fadd(known_src, known_src, mode);

        // Doubling 0 will give the same 0.
        if known_src.is_known_never_logical_pos_zero(mode)
            && (mode.output == DenormalModeKind::Ieee
                || (mode.output == DenormalModeKind::PreserveSign && known_src.is_known_never_pos_subnormal())
                || (mode.output == DenormalModeKind::PositiveZero && known_src.is_known_never_subnormal()))
        {
            known.known_not(FC_POS_ZERO);
        }

        known
    }

    /// Report known values for fsub.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L435>
    pub fn fsub(
        known_lhs: &KnownFpClass,
        known_rhs: &KnownFpClass,
        mode: DenormalMode,
    ) -> KnownFpClass {
        KnownFpClass::fadd(known_lhs, &KnownFpClass::fneg_of(known_rhs), mode)
    }

    /// Report known values for fmul.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L441>
    pub fn fmul(
        known_lhs: &KnownFpClass,
        known_rhs: &KnownFpClass,
        mode: DenormalMode,
    ) -> KnownFpClass {
        let mut known = KnownFpClass::default();

        known.propagate_non_snan2(known_lhs, known_rhs);

        // +X * +Y or -X * -Y => +Q
        // +X * -Y or -X * +Y => -Q
        known.propagate_xor_sign(known_lhs, known_rhs);

        // Inf * Y => Inf or NaN
        if known_lhs.is_known_always(FC_INF | FC_NAN) || known_rhs.is_known_always(FC_INF | FC_NAN) {
            known.known_not(FC_NORMAL | FC_SUBNORMAL | FC_ZERO);
        }

        // 0 * Y => 0 or NaN
        if known_rhs.is_known_always(FC_ZERO | FC_NAN) || known_lhs.is_known_always(FC_ZERO | FC_NAN) {
            known.known_not(FC_NORMAL | FC_SUBNORMAL | FC_INF);
        }

        if !known_lhs.is_known_never_nan() || !known_rhs.is_known_never_nan() {
            return known;
        }

        // 0 * +/-inf => NaN
        if (known_rhs.is_known_never_infinity() || known_lhs.is_known_never_logical_zero(mode))
            && (known_lhs.is_known_never_infinity() || known_rhs.is_known_never_logical_zero(mode))
        {
            known.known_not(FC_NAN);
        }

        known
    }

    /// Special case of fmul x, x.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L280>
    pub fn square(
        src: &KnownFpClass,
        mode: DenormalMode,
    ) -> KnownFpClass {
        let mut known = KnownFpClass::fmul(src, src, mode);

        // X * X is always non-negative or a NaN.
        known.known_not(FC_NEGATIVE);
        known.propagate_non_nan(src);
        known
    }

    /// Report known values for fmul by a constant.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L476>
    pub fn fmul_apfloat(
        known_lhs: &KnownFpClass,
        crhs: &ApFloat,
        mode: DenormalMode,
    ) -> KnownFpClass {
        // Match denormal scaling pattern, similar to the case in ldexp. If the
        // constant's exponent is sufficiently large, the result cannot be
        // subnormal.
        let flt = crhs.get_semantics();
        let precision = semantics_precision(flt);
        let mantissa_bits = precision as i32 - 1;

        let min_known_exponent = crhs.ilogb();
        let cannot_be_subnormal = min_known_exponent >= mantissa_bits;

        let mut known = KnownFpClass::fmul(known_lhs, &KnownFpClass::from_apfloat(crhs), mode);
        if cannot_be_subnormal {
            known.known_not(FC_SUBNORMAL);
        }

        // Multiply of values <= 1 cannot introduce overflow.
        if known_lhs.is_known_never(FC_INF) {
            if min_known_exponent < 0 {
                known.known_not(FC_INF);
            } else if min_known_exponent == 0
                && crhs.compare_absolute_value(&ApFloat::get_one(flt, false)) == CmpResult::Equal
            {
                known.known_not(FC_INF);
            }
        }

        known
    }

    /// Report known values for fdiv.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L504>
    pub fn fdiv(
        known_lhs: &KnownFpClass,
        known_rhs: &KnownFpClass,
        mode: DenormalMode,
    ) -> KnownFpClass {
        let mut known = KnownFpClass::default();

        known.propagate_non_snan2(known_lhs, known_rhs);

        // Only 0/0, Inf/Inf produce NaN.
        if known_lhs.is_known_never_nan()
            && known_rhs.is_known_never_nan()
            && (known_lhs.is_known_never_infinity() || known_rhs.is_known_never_infinity())
            && (known_lhs.is_known_never_logical_zero(mode) || known_rhs.is_known_never_logical_zero(mode))
        {
            known.known_not(FC_NAN);
        }

        //  X / -0.0 => -Inf (or NaN)
        // +X / +Y or -X / -Y => +Q
        // +X / -Y or -X / +Y => -Q
        known.propagate_xor_sign(known_lhs, known_rhs);

        // Normal and subnormal results require two non-zero finite operands.
        if (known_lhs.is_known_never(FC_NEG_NORMAL | FC_NEG_SUBNORMAL)
            && known_rhs.is_known_never(FC_NEG_NORMAL | FC_NEG_SUBNORMAL))
            || (known_lhs.is_known_never(FC_POS_NORMAL | FC_POS_SUBNORMAL)
                && known_rhs.is_known_never(FC_POS_NORMAL | FC_POS_SUBNORMAL))
        {
            known.known_not(FC_NEG_NORMAL | FC_NEG_SUBNORMAL);
        }
        if (known_lhs.is_known_never(FC_NEG_NORMAL | FC_NEG_SUBNORMAL)
            && known_rhs.is_known_never(FC_POS_NORMAL | FC_POS_SUBNORMAL))
            || (known_lhs.is_known_never(FC_POS_NORMAL | FC_POS_SUBNORMAL)
                && known_rhs.is_known_never(FC_NEG_NORMAL | FC_NEG_SUBNORMAL))
        {
            known.known_not(FC_POS_NORMAL | FC_POS_SUBNORMAL);
        }

        // 0 / X => 0 or NaN
        if known_lhs.is_known_always(FC_ZERO) {
            known.known_not(FC_SUBNORMAL | FC_NORMAL | FC_INF);
        }

        // X / 0 => NaN or Inf
        if known_rhs.is_known_always(FC_ZERO) {
            known.known_not(FC_FINITE);
        }

        known
    }

    /// Report known values for fdiv x, x.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L547>
    pub fn fdiv_self(
        known_src: &KnownFpClass,
        mode: DenormalMode,
    ) -> KnownFpClass {
        // X / X is always exactly 1.0 or a NaN.
        let mut known = KnownFpClass::new(FC_NAN | FC_POS_NORMAL, None);

        known.propagate_non_snan(known_src);

        if known_src.is_known_never_inf_or_nan() && known_src.is_known_never_logical_zero(mode) {
            known.known_not(FC_NAN);
        }

        known
    }

    /// Report known values for frem.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L560>
    pub fn frem(
        known_lhs: &KnownFpClass,
        known_rhs: &KnownFpClass,
        mode: DenormalMode,
    ) -> KnownFpClass {
        let mut known = KnownFpClass::default();

        known.known_not(FC_INF);

        // Inf REM x and x REM 0 produce NaN.
        if known_lhs.is_known_never_nan()
            && known_rhs.is_known_never_nan()
            && known_lhs.is_known_never_infinity()
            && known_rhs.is_known_never_logical_zero(mode)
        {
            known.known_not(FC_NAN);
        }

        // The sign for frem is the same as the first operand.
        if known_lhs.cannot_be_ordered_less_than_zero() {
            known.known_not(KnownFpClass::ORDERED_LESS_THAN_ZERO_MASK);
        }
        if known_lhs.cannot_be_ordered_greater_than_zero() {
            known.known_not(KnownFpClass::ORDERED_GREATER_THAN_ZERO_MASK);
        }

        // See if we can be more aggressive about the sign of 0.
        if known_lhs.is_known_never(FC_NEGATIVE) {
            known.known_not(FC_NEGATIVE);
        }
        if known_lhs.is_known_never(FC_POSITIVE) {
            known.known_not(FC_POSITIVE);
        }

        known
    }

    /// Report known values for frem x, x.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L589>
    pub fn frem_self(
        known_src: &KnownFpClass,
        mode: DenormalMode,
    ) -> KnownFpClass {
        // X % X is always exactly [+-]0.0 or a NaN.
        let mut known = KnownFpClass::new(FC_NAN | FC_ZERO, None);

        if known_src.is_known_never_inf_or_nan() && known_src.is_known_never_logical_zero(mode) {
            known.known_not(FC_NAN);
        } else if known_src.is_known_never(FC_SNAN) {
            known.known_not(FC_SNAN);
        }

        known
    }

    /// Report known values for fma.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L602>
    pub fn fma(
        known_lhs: &KnownFpClass,
        known_rhs: &KnownFpClass,
        known_addend: &KnownFpClass,
        mode: DenormalMode,
    ) -> KnownFpClass {
        let mul = KnownFpClass::fmul(known_lhs, known_rhs, mode);

        // FMA differs from the base fmul + fadd handling only in the treatment
        // of -0 results.
        //
        // If the multiply is a -0 due to rounding, the final -0 + 0 will be
        // -0, unlike for a separate fadd.
        let mut known = fadd_impl(&mul, known_addend, mode);

        // propagateNonSNaN for 3 arguments.
        if known_lhs.is_known_never(FC_SNAN)
            && known_rhs.is_known_never(FC_SNAN)
            && known_addend.is_known_never(FC_SNAN)
        {
            known.known_not(FC_SNAN);
        }

        known
    }

    /// Report known values for fma squared, squared, addend.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L623>
    pub fn fma_square(
        known_squared: &KnownFpClass,
        known_addend: &KnownFpClass,
        mode: DenormalMode,
    ) -> KnownFpClass {
        let squared = KnownFpClass::square(known_squared, mode);
        let mut known = fadd_impl(&squared, known_addend, mode);

        // Since we know the squared input must be positive, the add of
        // opposite sign infinities nan hazard only applies for negative inf.
        //
        // TODO: Alternatively to proving addend is not -inf, we could know
        // Squared is not pinf. Other than the degenerate always-subnormal input
        // case, we can't prove that without a known range.
        if known_addend.is_known_never(FC_NEG_INF | FC_NAN) && squared.is_known_never(FC_NAN) {
            known.known_not(FC_NAN);
        }

        known.propagate_non_snan2(known_squared, known_addend);

        known
    }

    /// Report known values for exp, exp2 and exp10.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L643>
    pub fn exp(known_src: &KnownFpClass) -> KnownFpClass {
        let mut known = KnownFpClass::default();
        known.known_not(FC_NEGATIVE);

        known.propagate_non_nan(known_src);

        if known_src.cannot_be_ordered_less_than_zero() {
            // If the source is positive this cannot underflow.
            known.known_not(FC_POS_ZERO);

            // Cannot introduce denormal values.
            known.known_not(FC_POS_SUBNORMAL);
        }

        // If the source is negative, this cannot overflow to infinity.
        if known_src.cannot_be_ordered_greater_than_zero() {
            known.known_not(FC_POS_INF);
        }

        known
    }

    /// Propagate known classes from a canonicalizing source.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L664>
    pub fn propagate_canonicalizing_src(
        &mut self,
        src: &KnownFpClass,
        mode: DenormalMode,
    ) {
        self.propagate_denormal(src, mode);
        self.propagate_non_nan(src);
    }

    /// Propagate known class for log/log2/log10.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L670>
    pub fn log(
        known_src: &KnownFpClass,
        mode: DenormalMode,
    ) -> KnownFpClass {
        let mut known = KnownFpClass::default();
        known.known_not(FC_NEG_ZERO | FC_SUBNORMAL);

        known.propagate_non_snan(known_src);

        if known_src.is_known_never_pos_infinity() {
            known.known_not(FC_POS_INF);
        }

        if known_src.is_known_never_nan() && known_src.cannot_be_ordered_less_than_zero() {
            known.known_not(FC_NAN);
        }

        if known_src.is_known_never_logical_zero(mode) {
            known.known_not(FC_NEG_INF);
        }

        known
    }

    /// Propagate known class for sqrt.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L689>
    pub fn sqrt(
        known_src: &KnownFpClass,
        mode: DenormalMode,
    ) -> KnownFpClass {
        let mut known = KnownFpClass::default();
        known.known_not(FC_POS_SUBNORMAL);

        if known_src.is_known_never_pos_infinity() {
            known.known_not(FC_POS_INF);
        }

        known.propagate_non_snan(known_src);

        // Any negative value besides -0 returns a nan.
        if known_src.is_known_never_nan() && known_src.cannot_be_ordered_less_than_zero() {
            known.known_not(FC_NAN);
        }

        // The only negative value that can be returned is -0 for -0 inputs.
        known.known_not(FC_NEG_INF | FC_NEG_SUBNORMAL | FC_NEG_NORMAL);

        // If the input denormal mode could be PreserveSign, a negative
        // subnormal input could produce a negative zero output.
        if known_src.is_known_never_logical_neg_zero(mode) {
            known.known_not(FC_NEG_ZERO);
        }

        known
    }

    /// Report known values for sin.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L714>
    pub fn sin(known_src: &KnownFpClass) -> KnownFpClass {
        let mut known = KnownFpClass::default();

        known.propagate_non_snan(known_src);

        // Return NaN on infinite inputs.
        known.known_not(FC_INF);
        if known_src.is_known_never_nan() && known_src.is_known_never_infinity() {
            known.known_not(FC_NAN);
        }

        known
    }

    /// Report known values for cos.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L727>
    pub fn cos(known_src: &KnownFpClass) -> KnownFpClass {
        KnownFpClass::sin(known_src)
    }

    /// Report known values for tan.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L731>
    pub fn tan(known_src: &KnownFpClass) -> KnownFpClass {
        let mut known = KnownFpClass::default();

        // tan never returns Inf (tan(+-Inf) = NaN; tan(finite) = finite).
        known.known_not(FC_INF);

        known.propagate_non_snan(known_src);

        // NaN propagates. tan(+-Inf) is NaN.
        if known_src.is_known_never_nan() && known_src.is_known_never_infinity() {
            known.known_not(FC_NAN);
        }

        known
    }

    /// Report known values for sinh.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L746>
    pub fn sinh(known_src: &KnownFpClass) -> KnownFpClass {
        let mut known = KnownFpClass::default();

        // sinh is sign-preserving: sinh(x) < 0 iff x < 0.
        if known_src.is_known_never(FC_NEGATIVE) {
            known.known_not(FC_NEGATIVE);
        }

        known.propagate_non_nan(known_src);

        known
    }

    /// Report known values for cosh.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L758>
    pub fn cosh(known_src: &KnownFpClass) -> KnownFpClass {
        let mut known = KnownFpClass::default();

        // cosh(x) >= 1 for all real x; cosh(+-Inf) = +Inf. Never negative,
        // zero, or subnormal.
        known.known_not(FC_NEGATIVE | FC_ZERO | FC_SUBNORMAL);

        known.propagate_non_nan(known_src);

        known
    }

    /// Report known values for tanh.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L770>
    pub fn tanh(known_src: &KnownFpClass) -> KnownFpClass {
        let mut known = KnownFpClass::default();

        // tanh is bounded to (-1, 1), never Inf.
        known.known_not(FC_INF);

        // tanh is sign-preserving: tanh(x) < 0 iff x < 0.
        if known_src.is_known_never(FC_NEGATIVE) {
            known.known_not(FC_NEGATIVE);
        }

        known.propagate_non_nan(known_src);

        known
    }

    /// Report known values for asin.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L785>
    pub fn asin(known_src: &KnownFpClass) -> KnownFpClass {
        let mut known = KnownFpClass::default();

        // asin is bounded to [-pi/2, pi/2], never Inf.
        known.known_not(FC_INF);

        known.propagate_non_snan(known_src);

        // asin is sign-preserving for finite arguments.
        if known_src.is_known_never(FC_NEG_FINITE) {
            known.known_not(FC_NEG_FINITE);
        }

        // NaN propagates. asin(x) is also NaN for |x| > 1, so we cannot rule
        // out NaN without knowing the source is in [-1, 1].
        known
    }

    /// Report known values for acos.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L802>
    pub fn acos(known_src: &KnownFpClass) -> KnownFpClass {
        let mut known = KnownFpClass::default();

        // acos(x) is bounded to [0, pi] for -1 <= x <= 1, and is never
        // negative, infinite, or subnormal. The smallest non-zero value occurs
        // when x is close to 1.0, where acos(x) can be approximated by
        // sqrt(2 * (1 - x)). Since sqrt cannot produce a subnormal result, we
        // can conclude that acos(x) will also never produce a subnormal result.
        known.known_not(FC_NEGATIVE | FC_INF | FC_SUBNORMAL);

        // acos(x) == +0.0 iff x == +1.0
        if known_src.is_known_never(FC_POS_NORMAL) {
            known.known_not(FC_ZERO);
        }

        known.propagate_non_snan(known_src);

        // NaN propagates. acos(x) is also NaN for |x| > 1, so we cannot rule
        // out NaN without knowing the source is in [-1, 1].
        known
    }

    /// Report known values for atan.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L823>
    pub fn atan(known_src: &KnownFpClass) -> KnownFpClass {
        let mut known = KnownFpClass::default();

        // atan is bounded to (-pi/2, pi/2), never Inf. atan(+-Inf) = +-pi/2
        // (finite).
        known.known_not(FC_INF);

        // atan is sign-preserving: atan(x) < 0 iff x < 0.
        if known_src.is_known_never(FC_NEGATIVE) {
            known.known_not(FC_NEGATIVE);
        }

        known.propagate_non_nan(known_src);

        known
    }

    /// Report known values for atan2.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L838>
    pub fn atan2(
        known_y: &KnownFpClass,
        known_x: &KnownFpClass,
        mode: DenormalMode,
    ) -> KnownFpClass {
        let mut known = KnownFpClass::default();

        // Even though these deductions are correct, we are ignoring the
        // following potentially erroneous cases:
        //   * atan2(y, inf) is not subnormal
        //   * atan2(inf, x) is not zero or subnormal

        // atan2 result is in (-pi, pi], never Inf.
        known.known_not(FC_INF);

        known.propagate_non_nan2(known_y, known_x);

        // Negative subnormals could be treated like positive zero.
        let x_cannot_have_positive_input =
            known_x.is_known_never(FC_POSITIVE) && known_x.is_known_never_logical_pos_zero(mode);
        let y_cannot_have_positive_input =
            known_y.is_known_never(FC_POSITIVE) && known_y.is_known_never_logical_pos_zero(mode);

        // If x <= -0.0, then |atan2(y, x)| >= pi/2
        if x_cannot_have_positive_input {
            known.known_not(FC_ZERO | FC_SUBNORMAL);
        }

        // If y >= +0.0, then atan2(y, x) >= +0.0
        if known_y.is_known_never(FC_NEGATIVE) {
            known.known_not(FC_NEGATIVE);
        }

        // If y <= -0.0, then atan2(y, x) <= -0.0
        // We do this deduction last in case we were able to rule out a
        // negative subnormal result earlier.
        if y_cannot_have_positive_input {
            known.known_not(FC_POS_SUBNORMAL | FC_POS_NORMAL | FC_POS_INF);
            // Negative subnormal results can flush to +0.0.
            if known.is_known_never(FC_NEG_SUBNORMAL) || !mode.outputs_may_be_positive_zero() {
                known.known_not(FC_POS_ZERO);
            }
        }

        known
    }

    /// Return true if the sign bit must be 0, ignoring the sign of nans.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L369>
    pub fn sign_bit_is_zero_or_nan(&self) -> bool {
        self.is_known_never(FC_NEGATIVE)
    }

    /// Assume the sign bit is zero.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L372>
    pub fn sign_bit_must_be_zero(&mut self) {
        self.set_known_fp_classes(self.get_known_fp_classes() & (FC_POSITIVE | FC_NAN));
        self.set_sign_bit(Some(false));
    }

    /// Assume the sign bit is one.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L378>
    pub fn sign_bit_must_be_one(&mut self) {
        self.set_known_fp_classes(self.get_known_fp_classes() & (FC_NEGATIVE | FC_NAN));
        self.set_sign_bit(Some(true));
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L383>
    pub fn copysign(
        &mut self,
        sign: &KnownFpClass,
    ) {
        // Don't know anything about the sign of the source. Expand the
        // possible set to its opposite sign pair.
        if self.get_known_fp_classes().any(FC_ZERO) {
            self.set_known_fp_classes(self.get_known_fp_classes() | FC_ZERO);
        }
        if self.get_known_fp_classes().any(FC_SUBNORMAL) {
            self.set_known_fp_classes(self.get_known_fp_classes() | FC_SUBNORMAL);
        }
        if self.get_known_fp_classes().any(FC_NORMAL) {
            self.set_known_fp_classes(self.get_known_fp_classes() | FC_NORMAL);
        }
        if self.get_known_fp_classes().any(FC_INF) {
            self.set_known_fp_classes(self.get_known_fp_classes() | FC_INF);
        }

        // Sign bit is exactly preserved even for nans.
        self.set_sign_bit(sign.get_sign_bit());

        // Clear sign bits based on the input sign mask.
        if sign.is_known_never(FC_POSITIVE | FC_NAN) || self.get_sign_bit() == Some(true) {
            self.set_known_fp_classes(self.get_known_fp_classes() & (FC_NEGATIVE | FC_NAN));
        }
        if sign.is_known_never(FC_NEGATIVE | FC_NAN) || self.get_sign_bit() == Some(false) {
            self.set_known_fp_classes(self.get_known_fp_classes() & (FC_POSITIVE | FC_NAN));
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L407>
    pub fn copysign_of(
        known_mag: &KnownFpClass,
        known_sign: &KnownFpClass,
    ) -> KnownFpClass {
        let mut known = *known_mag;
        known.copysign(known_sign);
        known
    }

    /// Propagate knowledge that an operation cannot introduce a signaling NaN.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L415>
    pub fn propagate_non_snan(
        &mut self,
        src: &KnownFpClass,
    ) {
        if src.is_known_never(FC_SNAN) {
            self.known_not(FC_SNAN);
        }
    }

    /// `propagateNonSNaN` of two sources.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L421>
    pub fn propagate_non_snan2(
        &mut self,
        lhs: &KnownFpClass,
        rhs: &KnownFpClass,
    ) {
        if lhs.is_known_never(FC_SNAN) && rhs.is_known_never(FC_SNAN) {
            self.known_not(FC_SNAN);
        }
    }

    /// Propagate knowledge that a non-NaN source implies the result can also
    /// not be a NaN.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L429>
    pub fn propagate_non_nan(
        &mut self,
        src: &KnownFpClass,
    ) {
        self.propagate_non_snan(src);
        if src.is_known_never(FC_NAN) {
            self.known_not(FC_NAN);
        }
    }

    /// `propagateNonNaN` of two sources.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L435>
    pub fn propagate_non_nan2(
        &mut self,
        lhs: &KnownFpClass,
        rhs: &KnownFpClass,
    ) {
        self.propagate_non_snan2(lhs, rhs);
        if lhs.is_known_never_nan() && rhs.is_known_never_nan() {
            self.known_not(FC_NAN);
        }
    }

    /// Propagate knowledge for operations whose result sign is the xor of the
    /// operand signs, such as multiply and divide.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L444>
    pub fn propagate_xor_sign(
        &mut self,
        lhs: &KnownFpClass,
        rhs: &KnownFpClass,
    ) {
        if (lhs.is_known_never(FC_NEGATIVE) && rhs.is_known_never(FC_NEGATIVE))
            || (lhs.is_known_never(FC_POSITIVE) && rhs.is_known_never(FC_POSITIVE))
        {
            self.known_not(FC_NEGATIVE);
        }

        if (lhs.is_known_never(FC_POSITIVE) && rhs.is_known_never(FC_NEGATIVE))
            || (lhs.is_known_never(FC_NEGATIVE) && rhs.is_known_never(FC_POSITIVE))
        {
            self.known_not(FC_POSITIVE);
        }
    }

    /// Propagate knowledge from a source value that could be a denormal or
    /// zero. This assumes a copy-like operation and will replace any currently
    /// known information.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L70>
    pub fn propagate_denormal(
        &mut self,
        src: &KnownFpClass,
        mode: DenormalMode,
    ) {
        self.set_known_fp_classes(src.get_known_fp_classes());
        // If we aren't assuming the source can't be a zero, we don't have to
        // check if a denormal input could be flushed.
        if !src.is_known_never_pos_zero() && !src.is_known_never_neg_zero() {
            return;
        }

        // If we know the input can't be a denormal, it can't be flushed to 0.
        if src.is_known_never_subnormal() {
            return;
        }

        if !src.is_known_never_pos_subnormal() && mode != DenormalMode::get_ieee() {
            self.set_known_fp_classes(self.get_known_fp_classes() | FC_POS_ZERO);
        }

        if !src.is_known_never_neg_subnormal() && mode != DenormalMode::get_ieee() {
            if mode != DenormalMode::get_positive_zero() {
                self.set_known_fp_classes(self.get_known_fp_classes() | FC_NEG_ZERO);
            }

            if mode.input == DenormalModeKind::PositiveZero
                || mode.output == DenormalModeKind::PositiveZero
                || mode.input == DenormalModeKind::Dynamic
                || mode.output == DenormalModeKind::Dynamic
            {
                self.set_known_fp_classes(self.get_known_fp_classes() | FC_POS_ZERO);
            }
        }
    }

    /// Propagate known class for fpext.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L880>
    pub fn fpext(
        known_src: &KnownFpClass,
        dst_ty: Sem,
        src_ty: Sem,
    ) -> KnownFpClass {
        // Infinity, nan and zero propagate from source.
        let mut known = *known_src;

        // All subnormal inputs should be in the normal range in the result
        // type.
        if is_representable_as_normal_in(src_ty, dst_ty) {
            if known.get_known_fp_classes().any(FC_POS_SUBNORMAL) {
                known.set_known_fp_classes(known.get_known_fp_classes() | FC_POS_NORMAL);
            }
            if known.get_known_fp_classes().any(FC_NEG_SUBNORMAL) {
                known.set_known_fp_classes(known.get_known_fp_classes() | FC_NEG_NORMAL);
            }
            known.known_not(FC_SUBNORMAL);
        }

        // Sign bit of a nan isn't guaranteed.
        if !known.is_known_never_nan() {
            known.set_sign_bit(None);
        }

        known
    }

    /// Propagate known class for fptrunc.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L902>
    pub fn fptrunc(known_src: &KnownFpClass) -> KnownFpClass {
        let mut known = KnownFpClass::default();

        // Sign should be preserved
        // TODO: Handle cannot be ordered greater than zero
        if known_src.cannot_be_ordered_less_than_zero() {
            known.known_not(KnownFpClass::ORDERED_LESS_THAN_ZERO_MASK);
        }

        known.propagate_non_nan(known_src);

        // Infinity needs a range check.
        known
    }

    /// Propagate known class for rounding intrinsics (trunc, floor, ceil, rint,
    /// nearbyint, round, roundeven).
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L916>
    pub fn round_to_integral(
        known_src: &KnownFpClass,
        is_trunc: bool,
        is_multi_unit_fp_type: bool,
    ) -> KnownFpClass {
        let mut known = KnownFpClass::default();

        // Integer results cannot be subnormal.
        known.known_not(FC_SUBNORMAL);

        known.propagate_non_nan(known_src);

        // Pass through infinities, except PPC_FP128 is a special case for
        // intrinsics other than trunc.
        if is_trunc || !is_multi_unit_fp_type {
            if known_src.is_known_never_pos_infinity() {
                known.known_not(FC_POS_INF);
            }
            if known_src.is_known_never_neg_infinity() {
                known.known_not(FC_NEG_INF);
            }
        }

        // Negative round ups to 0 produce -0
        if known_src.is_known_never(FC_POS_FINITE) {
            known.known_not(FC_POS_FINITE);
        }
        if known_src.is_known_never(FC_NEG_FINITE) {
            known.known_not(FC_NEG_FINITE);
        }

        known
    }

    /// Propagate known class for mantissa component of frexp.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L944>
    pub fn frexp_mant(
        known_src: &KnownFpClass,
        mode: DenormalMode,
    ) -> KnownFpClass {
        let mut known = KnownFpClass::default();
        known.known_not(FC_SUBNORMAL);

        if known_src.is_known_never(FC_NEGATIVE) {
            known.known_not(FC_NEGATIVE);
        } else {
            if known_src.is_known_never_logical_neg_zero(mode) {
                known.known_not(FC_NEG_ZERO);
            }
            if known_src.is_known_never(FC_NEG_INF) {
                known.known_not(FC_NEG_INF);
            }
        }

        if known_src.is_known_never(FC_POSITIVE) {
            known.known_not(FC_POSITIVE);
        } else {
            if known_src.is_known_never_logical_pos_zero(mode) {
                known.known_not(FC_POS_ZERO);
            }
            if known_src.is_known_never(FC_POS_INF) {
                known.known_not(FC_POS_INF);
            }
        }

        known.propagate_non_nan(known_src);
        known
    }

    /// Propagate known class for ldexp, assuming the exponent is known to be
    /// within the signed range `[exp_min, exp_max]`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L971>
    pub fn ldexp(
        known_src: &KnownFpClass,
        exp_min: i128,
        exp_max: i128,
        flt: Sem,
        mode: DenormalMode,
    ) -> KnownFpClass {
        let mut known = KnownFpClass::default();
        known.propagate_non_nan(known_src);

        // Sign is preserved, but underflows may produce zeroes.
        if known_src.is_known_never(FC_NEGATIVE) {
            known.known_not(FC_NEGATIVE);
        } else if known_src.cannot_be_ordered_less_than_zero() {
            known.known_not(KnownFpClass::ORDERED_LESS_THAN_ZERO_MASK);
        }

        if known_src.is_known_never(FC_POSITIVE) {
            known.known_not(FC_POSITIVE);
        } else if known_src.cannot_be_ordered_greater_than_zero() {
            known.known_not(KnownFpClass::ORDERED_GREATER_THAN_ZERO_MASK);
        }

        let precision = semantics_precision(flt);
        let mantissa_bits = precision as i128 - 1;
        if exp_min >= mantissa_bits {
            known.known_not(FC_SUBNORMAL);
        }

        if exp_min == 0 && exp_max == 0 {
            // ldexp(x, 0) -> x, so propagate everything.
            known.propagate_canonicalizing_src(known_src, mode);
        } else if exp_max <= 0 {
            // If we know the power is <= 0, can't introduce inf
            if known_src.is_known_never_pos_infinity() {
                known.known_not(FC_POS_INF);
            }
            if known_src.is_known_never_neg_infinity() {
                known.known_not(FC_NEG_INF);
            }
        } else if exp_min >= 0 {
            // If we know the power is >= 0, can't introduce subnormal or zero
            if known_src.is_known_never_pos_subnormal() {
                known.known_not(FC_POS_SUBNORMAL);
            }
            if known_src.is_known_never_neg_subnormal() {
                known.known_not(FC_NEG_SUBNORMAL);
            }
            if known_src.is_known_never_logical_pos_zero(mode) {
                known.known_not(FC_POS_ZERO);
            }
            if known_src.is_known_never_logical_neg_zero(mode) {
                known.known_not(FC_NEG_ZERO);
            }
        }

        known
    }

    /// `ldexp` with the exponent's known bits.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L1018>
    pub fn ldexp_known_bits(
        known_src: &KnownFpClass,
        exp_bits: &KnownBits,
        flt: Sem,
        mode: DenormalMode,
    ) -> KnownFpClass {
        KnownFpClass::ldexp(
            known_src,
            crate::apint::signed(exp_bits.signed_min_value(), exp_bits.width),
            crate::apint::signed(exp_bits.signed_max_value(), exp_bits.width),
            flt,
            mode,
        )
    }

    /// Propagate known class for pow.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L1025>
    pub fn pow(
        known_lhs: &KnownFpClass,
        known_rhs: &KnownFpClass,
    ) -> KnownFpClass {
        let mut known = KnownFpClass::default();

        known.propagate_non_snan2(known_lhs, known_rhs);

        // pow may return NaN if one of the arguments is NaN. NaN may be
        // produced from a non-zero-finite-negative base and a non-integer
        // exponent.
        if known_lhs.is_known_never(FC_NAN | FC_NEG_NORMAL | FC_NEG_SUBNORMAL) && known_rhs.is_known_never_nan() {
            known.known_not(FC_NAN);
        }

        // We could rule out negative and subnormal results when exponent is
        // known to never be a normal value, but having either argument being
        // known to never be normal is unlikely and not worth considering.

        // Only a negative base raised to an odd power returns a negative
        // value.
        if known_lhs.is_known_never(FC_NEGATIVE) {
            known.known_not(FC_NEGATIVE);
        } else if known_lhs.is_known_never(FC_NEG_NORMAL | FC_NEG_SUBNORMAL) {
            known.known_not(FC_NEG_NORMAL | FC_NEG_SUBNORMAL);
            // See if we can also rule out -0.0 or -inf. Here at least one of
            // -0.0 or -inf is a possible base.

            // pow(-0.0, odd-positive) = -0.0
            // pow(-inf, odd-negative) = -0.0
            if (known_lhs.is_known_never(FC_NEG_ZERO) || known_rhs.is_known_never(FC_POS_NORMAL))
                && (known_lhs.is_known_never(FC_NEG_INF) || known_rhs.is_known_never(FC_NEG_NORMAL))
            {
                known.known_not(FC_NEG_ZERO);
            }

            // pow(-0.0, odd-negative) = -inf
            // pow(-inf, odd-positive) = -inf
            if (known_lhs.is_known_never(FC_NEG_ZERO) || known_rhs.is_known_never(FC_NEG_NORMAL))
                && (known_lhs.is_known_never(FC_NEG_INF) || known_rhs.is_known_never(FC_POS_NORMAL))
            {
                known.known_not(FC_NEG_INF);
            }
        }

        known
    }

    /// Propagate known class for powi.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L1067>
    pub fn powi(
        known_src: &KnownFpClass,
        exponent_known_bits: &KnownBits,
    ) -> KnownFpClass {
        let mut known = KnownFpClass::default();
        known.propagate_non_nan(known_src);

        if exponent_known_bits.is_zero() {
            // powi(QNaN, 0) returns 1.0, and powi(SNaN, 0) may
            // non-deterministically return 1.0 or a NaN.
            if known_src.is_known_never(FC_SNAN) {
                known.known_not(!FC_POS_NORMAL);
                return known;
            }

            known.known_not(!(FC_POS_NORMAL | FC_NAN));
            return known;
        }

        // Given that exp is an integer, here are the ways that powi can return
        // a negative value:
        //
        //   powi(x, exp)    --> negative if exp is odd and x is negative.
        //   powi(-0, exp)   --> -inf if exp is negative odd.
        //   powi(-0, exp)   --> -0 if exp is positive odd.
        //   powi(-inf, exp) --> -0 if exp is negative odd.
        //   powi(-inf, exp) --> -inf if exp is positive odd.
        if known_src.is_known_never(FC_NEGATIVE) || exponent_known_bits.is_even() {
            known.known_not(FC_NEGATIVE);
        } else if known_src.is_known_never(FC_NEG_NORMAL | FC_NEG_SUBNORMAL) {
            known.known_not(FC_NEG_NORMAL | FC_NEG_SUBNORMAL);
            // See if we can also rule out -0.0 or -inf. Here at least one of
            // -0.0 or -inf is a possible base.

            // We already know that ExponentKnownBits.isEven() is false here.
            let is_known_never_odd_positive = exponent_known_bits.is_negative();
            let is_known_never_odd_negative = exponent_known_bits.is_non_negative();

            // powi(-0.0, odd-positive) = -0.0
            // powi(-inf, odd-negative) = -0.0
            if (known_src.is_known_never(FC_NEG_ZERO) || is_known_never_odd_positive)
                && (known_src.is_known_never(FC_NEG_INF) || is_known_never_odd_negative)
            {
                known.known_not(FC_NEG_ZERO);
            }

            // powi(-0.0, odd-negative) = -inf
            // powi(-inf, odd-positive) = -inf
            if (known_src.is_known_never(FC_NEG_ZERO) || is_known_never_odd_negative)
                && (known_src.is_known_never(FC_NEG_INF) || is_known_never_odd_positive)
            {
                known.known_not(FC_NEG_INF);
            }
        }

        // powi(x, exp) --> inf
        // when:
        //   * powi(inf, exp), exp > 0
        //   * powi(+/-0, exp), exp < 0
        //   * powi(finite, exp), |exp| > 1
        //   * powi(subnormal, -1)
        // TODO:
        //   1. This simple all or nothing approach. We can do better and cover
        //      sign/parity and exp > 1 vs exp < -1 separately.
        //   2. powi(0/nan, exp), exp > 0 can be refinable to fcNan | fcZero |
        //      fcPosNormal.
        {
            let min_exp = crate::apint::signed(exponent_known_bits.signed_min_value(), exponent_known_bits.width);
            let max_exp = crate::apint::signed(exponent_known_bits.signed_max_value(), exponent_known_bits.width);

            // powi(inf, exp), exp > 0
            let may_inf_src = !known_src.is_known_never(FC_INF) && max_exp > 0;

            // powi(+/-0, exp), exp < 0
            let may_div_by_zero = !known_src.is_known_never(FC_ZERO) && min_exp < 0;

            // powi(finite, exp), |exp| > 1
            let may_finite = !known_src.is_known_never(FC_NORMAL | FC_SUBNORMAL);
            let may_abs_exp_gt1 = min_exp < -1 || max_exp > 1;
            let may_finite_overflow = may_finite && may_abs_exp_gt1;

            // powi(subnormal, -1)
            let may_be_neg_one = exponent_known_bits.zero == 0;
            let may_subnorm_inv = !known_src.is_known_never(FC_SUBNORMAL) && may_be_neg_one;

            if !may_inf_src && !may_div_by_zero && !may_finite_overflow && !may_subnorm_inv {
                known.known_not(FC_INF);
            }
        }

        known
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L512>
    pub fn reset_all(&mut self) {
        *self = KnownFpClass::default();
    }
}

impl BitOr for KnownFpClass {
    type Output = KnownFpClass;

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownFPClass.h#L515>
    fn bitor(
        mut self,
        rhs: KnownFpClass,
    ) -> KnownFpClass {
        self.or_assign(&rhs);
        self
    }
}

impl BitOrAssign for KnownFpClass {
    fn bitor_assign(
        &mut self,
        rhs: KnownFpClass,
    ) {
        self.or_assign(&rhs);
    }
}

/// Whether input denormals are IEEE in `mode`.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L28>
fn input_denormal_is_ieee(mode: DenormalMode) -> bool {
    mode.input == DenormalModeKind::Ieee
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L32>
fn input_denormal_is_ieee_or_pos_zero(mode: DenormalMode) -> bool {
    mode.input == DenormalModeKind::Ieee || mode.input == DenormalModeKind::PositiveZero
}

/// The formats LLVM has validated `bitcast` and `toKnownBits` for.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L245>
fn is_supported(semantics: Sem) -> bool {
    matches!(
        semantics.kind,
        SemanticsKind::IeeeSingle | SemanticsKind::IeeeDouble | SemanticsKind::X87DoubleExtended
    )
}

/// Handle known sign bit and nan cases for fadd.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownFPClass.cpp#L367>
fn fadd_impl(
    known_lhs: &KnownFpClass,
    known_rhs: &KnownFpClass,
    _mode: DenormalMode,
) -> KnownFpClass {
    let mut known = KnownFpClass::default();

    // Adding positive and negative infinity produces NaN, but only if both
    // opposite-sign infinity combinations are possible.
    if known_lhs.is_known_never_nan()
        && known_rhs.is_known_never_nan()
        && (known_lhs.is_known_never(FC_POS_INF) || known_rhs.is_known_never(FC_NEG_INF))
        && (known_lhs.is_known_never(FC_NEG_INF) || known_rhs.is_known_never(FC_POS_INF))
    {
        known.known_not(FC_NAN);
    }

    if known_lhs.cannot_be_ordered_less_than_zero() && known_rhs.cannot_be_ordered_less_than_zero() {
        known.known_not(KnownFpClass::ORDERED_LESS_THAN_ZERO_MASK);

        // This can't underflow if one of the operands is known normal.
        if known_lhs.is_known_never(FC_ZERO | FC_POS_SUBNORMAL) || known_rhs.is_known_never(FC_ZERO | FC_POS_SUBNORMAL)
        {
            known.known_not(FC_ZERO | FC_POS_SUBNORMAL);
        }
    }

    if known_lhs.cannot_be_ordered_greater_than_zero() && known_rhs.cannot_be_ordered_greater_than_zero() {
        known.known_not(KnownFpClass::ORDERED_GREATER_THAN_ZERO_MASK);

        // This can't underflow if one of the operands is known normal.
        if known_lhs.is_known_never(FC_ZERO | FC_NEG_SUBNORMAL) || known_rhs.is_known_never(FC_ZERO | FC_NEG_SUBNORMAL)
        {
            known.known_not(FC_ZERO | FC_NEG_SUBNORMAL);
        }
    }

    known
}
