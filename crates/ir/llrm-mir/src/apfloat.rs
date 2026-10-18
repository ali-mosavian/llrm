//! `APFloat` for the formats MIR has: IEEE single, double and x87 extended, and
//! two 8-bit IEEE formats that exist to be tested exhaustively.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` (`llvm/lib/Support/APFloat.cpp`,
//! `llvm/include/llvm/ADT/APFloat.h`): the operations
//! `KnownFPClass.cpp`, `ConstantFPRange.cpp` and `ValueTracking.cpp` call,
//! each linking the line it translates. A value is a sign, a category, an
//! exponent and a significand in a `u128`, exactly as `IEEEFloat` keeps them,
//! so the quirks (a NaN carries its significand through `convert`, x87's
//! integer bit) are LLVM's. A [`Semantics`] says how many bits the
//! significand has and where the exponent ends, so one implementation serves
//! every format, and [`FLOAT8_E5M2`] and [`FLOAT8_E4M3`] are small enough to
//! run every operation on every pair of values against LLVM.
//!
//! DEVIATE: MIR has `float`, `double` and `x86_fp80` ([`crate::FloatKind`])
//! and no half, bfloat, fp128 or ppc_fp128: those semantics are not here
//! (`mir_has_only_the_float_kinds_apfloat_has` fails the day MIR gains one).
//! `DoubleAPFloat` (APFloat.cpp:4800-5900) and the `NanOnly`/`FiniteOnly`
//! formats and the arbitrary-FP names (`getArbitraryFPSemantics`) go with
//! them. Operations no translated caller uses are not here: parsing and
//! printing, `fusedMultiplyAdd`, `mod`, `remainder`, `scalbn`, `frexp`,
//! integer conversion.
//!
//! A value here is the IR value's own format. Whether an operation is
//! evaluated wider (x87 excess precision) is the target description's fact, not
//! decided in this module.
//!
//! # Example
//! `ApFloat::from_bits(IEEE_SINGLE, 0x3f80_0000)` is 1.0; its `next(false)` is
//! `0x3f80_0001`; `convert`ing it to [`FLOAT8_E5M2`] gives 1.0 exactly.
//!
//! # See also
//! - Used by: [`crate::knownfpclass`], [`crate::constantfprange`]

use std::cmp::max;

use crate::fpclass::{
    FC_NEG_INF, FC_NEG_NORMAL, FC_NEG_SUBNORMAL, FC_NEG_ZERO, FC_POS_INF, FC_POS_NORMAL, FC_POS_SUBNORMAL, FC_POS_ZERO,
    FC_QNAN, FC_SNAN, FpClassTest, RoundingMode,
};
use crate::types::FloatKind;

/// `APFloat::opStatus`.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L377>
pub type OpStatus = u32;
pub const OP_OK: OpStatus = 0x00;
pub const OP_INVALID_OP: OpStatus = 0x01;
pub const OP_DIV_BY_ZERO: OpStatus = 0x02;
pub const OP_OVERFLOW: OpStatus = 0x04;
pub const OP_UNDERFLOW: OpStatus = 0x08;
pub const OP_INEXACT: OpStatus = 0x10;

/// `APFloat::cmpResult`, with LLVM's numbering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CmpResult {
    LessThan = 0,
    Equal = 1,
    GreaterThan = 2,
    Unordered = 3,
}

/// `APFloat::fltCategory`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Category {
    Infinity,
    NaN,
    Normal,
    Zero,
}

/// `ilogb` of a NaN, of a zero and of an infinity.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L400>
pub const IEK_ZERO: i32 = i32::MIN + 1;
pub const IEK_NAN: i32 = i32::MIN;
pub const IEK_INF: i32 = i32::MAX;

/// Which format, for the places LLVM compares a semantics with an enumerator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticsKind {
    IeeeSingle,
    IeeeDouble,
    X87DoubleExtended,
    Float8E5M2,
    Float8E4M3,
}

/// `fltSemantics`: the shape of an IEEE-754 format. All of them have zeros,
/// denormals, signed values, infinities and NaNs.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L75>
#[derive(Debug, Eq, PartialEq)]
pub struct Semantics {
    pub kind: SemanticsKind,
    pub max_exponent: i32,
    pub min_exponent: i32,
    pub precision: u32,
    pub size_in_bits: u32,
    pub has_explicit_integer_bit: bool,
}

pub type Sem = &'static Semantics;

pub const IEEE_SINGLE: Sem = &Semantics {
    kind: SemanticsKind::IeeeSingle,
    max_exponent: 127,
    min_exponent: -126,
    precision: 24,
    size_in_bits: 32,
    has_explicit_integer_bit: false,
};
pub const IEEE_DOUBLE: Sem = &Semantics {
    kind: SemanticsKind::IeeeDouble,
    max_exponent: 1023,
    min_exponent: -1022,
    precision: 53,
    size_in_bits: 64,
    has_explicit_integer_bit: false,
};
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L119>
pub const X87_DOUBLE_EXTENDED: Sem = &Semantics {
    kind: SemanticsKind::X87DoubleExtended,
    max_exponent: 16383,
    min_exponent: -16382,
    precision: 64,
    size_in_bits: 80,
    has_explicit_integer_bit: true,
};
pub const FLOAT8_E5M2: Sem = &Semantics {
    kind: SemanticsKind::Float8E5M2,
    max_exponent: 15,
    min_exponent: -14,
    precision: 3,
    size_in_bits: 8,
    has_explicit_integer_bit: false,
};
pub const FLOAT8_E4M3: Sem = &Semantics {
    kind: SemanticsKind::Float8E4M3,
    max_exponent: 7,
    min_exponent: -6,
    precision: 4,
    size_in_bits: 8,
    has_explicit_integer_bit: false,
};

/// The semantics of a MIR float type (`Type::getFltSemantics`).
pub fn semantics_of(kind: FloatKind) -> Sem {
    match kind {
        FloatKind::Float => IEEE_SINGLE,
        FloatKind::Double => IEEE_DOUBLE,
        FloatKind::X86Fp80 => X87_DOUBLE_EXTENDED,
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L329>
pub fn semantics_precision(semantics: Sem) -> u32 {
    semantics.precision
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L333>
pub fn semantics_max_exponent(semantics: Sem) -> i32 {
    semantics.max_exponent
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L337>
pub fn semantics_min_exponent(semantics: Sem) -> i32 {
    semantics.min_exponent
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L340>
pub fn semantics_size_in_bits(semantics: Sem) -> u32 {
    semantics.size_in_bits
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L354>
pub fn semantics_has_zero(_semantics: Sem) -> bool {
    true
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L358>
pub fn semantics_has_signed_repr(_semantics: Sem) -> bool {
    true
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L362>
pub fn semantics_has_inf(_semantics: Sem) -> bool {
    true
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L366>
pub fn semantics_has_nan(_semantics: Sem) -> bool {
    true
}

/// Whether the format is one of half, bfloat, single, double and quad.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L370>
pub fn is_ieee_like_fp(semantics: Sem) -> bool {
    matches!(
        semantics.kind,
        SemanticsKind::IeeeSingle | SemanticsKind::IeeeDouble
    )
}

/// Whether `Src`'s subnormals are normal in `Dst`.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L379>
pub fn is_representable_as_normal_in(
    src: Sem,
    dst: Sem,
) -> bool {
    // Exponent range must be larger.
    if src.max_exponent >= dst.max_exponent || src.min_exponent <= dst.min_exponent {
        return false;
    }
    dst.precision >= src.precision
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L393>
pub fn get_size_in_bits(semantics: Sem) -> u32 {
    semantics.size_in_bits
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L397>
fn exponent_zero(semantics: Sem) -> i32 {
    semantics.min_exponent - 1
}

fn exponent_inf(semantics: Sem) -> i32 {
    semantics.max_exponent + 1
}

fn exponent_nan(semantics: Sem) -> i32 {
    semantics.max_exponent + 1
}

/// `lostFraction`: what a right shift dropped, as a fraction of the last bit
/// kept.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LostFraction {
    ExactlyZero,
    LessThanHalf,
    ExactlyHalf,
    MoreThanHalf,
}

/// The 1-based index of the most significant set bit, 0 for zero.
fn msb1(value: u128) -> u32 {
    128 - value.leading_zeros()
}

/// The fraction lost were `value` truncated losing its `bits` low bits.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L670>
fn lost_fraction_through_truncation(
    value: u128,
    bits: u32,
) -> LostFraction {
    let lsb = if value == 0 { u32::MAX } else { value.trailing_zeros() };
    if bits <= lsb {
        return LostFraction::ExactlyZero;
    }
    if bits == lsb + 1 {
        return LostFraction::ExactlyHalf;
    }
    if bits <= 128 && (value >> (bits - 1)) & 1 == 1 {
        return LostFraction::MoreThanHalf;
    }
    LostFraction::LessThanHalf
}

/// Shift right noting the lost fraction.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L690>
fn shift_right(
    value: &mut u128,
    bits: u32,
) -> LostFraction {
    let lost = lost_fraction_through_truncation(*value, bits);
    *value = if bits >= 128 { 0 } else { *value >> bits };
    lost
}

/// Combine the effect of two lost fractions.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L701>
fn combine_lost_fractions(
    more_significant: LostFraction,
    less_significant: LostFraction,
) -> LostFraction {
    if less_significant != LostFraction::ExactlyZero {
        if more_significant == LostFraction::ExactlyZero {
            return LostFraction::LessThanHalf;
        } else if more_significant == LostFraction::ExactlyHalf {
            return LostFraction::MoreThanHalf;
        }
    }
    more_significant
}

/// `APFloat` over [`Semantics`].
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1074>
#[derive(Clone, Copy, Debug)]
pub struct ApFloat {
    sem: Sem,
    sign: bool,
    category: Category,
    exponent: i32,
    sig: u128,
}

impl ApFloat {
    fn uninitialized(semantics: Sem) -> Self {
        let mut value = Self { sem: semantics, sign: false, category: Category::Zero, exponent: 0, sig: 0 };
        value.make_zero(false);
        value
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1183>
    pub fn get_zero(
        semantics: Sem,
        negative: bool,
    ) -> Self {
        let mut value = Self::uninitialized(semantics);
        value.make_zero(negative);
        value
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1192>
    pub fn get_one(
        semantics: Sem,
        negative: bool,
    ) -> Self {
        let mut value = Self {
            sem: semantics,
            sign: false,
            category: Category::Normal,
            exponent: 0,
            sig: 1 << (semantics.precision - 1),
        };
        if negative {
            value.change_sign();
        }
        value
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1202>
    pub fn get_inf(
        semantics: Sem,
        negative: bool,
    ) -> Self {
        let mut value = Self::uninitialized(semantics);
        value.make_inf(negative);
        value
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1213>
    pub fn get_nan(
        semantics: Sem,
        negative: bool,
        payload: u64,
    ) -> Self {
        if payload != 0 {
            Self::get_qnan(semantics, negative, Some(u128::from(payload)))
        } else {
            Self::get_qnan(semantics, negative, None)
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1224>
    pub fn get_qnan(
        semantics: Sem,
        negative: bool,
        payload: Option<u128>,
    ) -> Self {
        let mut value = Self::uninitialized(semantics);
        value.make_nan(false, negative, payload);
        value
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1232>
    pub fn get_snan(
        semantics: Sem,
        negative: bool,
        payload: Option<u128>,
    ) -> Self {
        let mut value = Self::uninitialized(semantics);
        value.make_nan(true, negative, payload);
        value
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1242>
    pub fn get_largest(
        semantics: Sem,
        negative: bool,
    ) -> Self {
        let mut value = Self::uninitialized(semantics);
        value.make_largest(negative);
        value
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1252>
    pub fn get_smallest(
        semantics: Sem,
        negative: bool,
    ) -> Self {
        let mut value = Self::uninitialized(semantics);
        value.make_smallest(negative);
        value
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1262>
    pub fn get_smallest_normalized(
        semantics: Sem,
        negative: bool,
    ) -> Self {
        let mut value = Self::uninitialized(semantics);
        value.make_smallest_normalized(negative);
        value
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L606>
    pub fn get_semantics(&self) -> Sem {
        self.sem
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L605>
    pub fn get_category(&self) -> Category {
        self.category
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1583>
    pub fn is_negative(&self) -> bool {
        self.sign
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L595>
    pub fn is_nan(&self) -> bool {
        self.category == Category::NaN
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1580>
    pub fn is_infinity(&self) -> bool {
        self.category == Category::Infinity
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1579>
    pub fn is_zero(&self) -> bool {
        self.category == Category::Zero
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1596>
    pub fn is_pos_infinity(&self) -> bool {
        self.is_infinity() && !self.is_negative()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1597>
    pub fn is_neg_infinity(&self) -> bool {
        self.is_infinity() && self.is_negative()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1594>
    pub fn is_pos_zero(&self) -> bool {
        self.is_zero() && !self.is_negative()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1595>
    pub fn is_neg_zero(&self) -> bool {
        self.is_zero() && self.is_negative()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1588>
    pub fn is_finite(&self) -> bool {
        !self.is_nan() && !self.is_infinity()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1592>
    pub fn is_non_zero(&self) -> bool {
        !self.is_zero()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1593>
    pub fn is_finite_non_zero(&self) -> bool {
        self.is_finite() && !self.is_zero()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1587>
    pub fn is_normal(&self) -> bool {
        !self.is_denormal() && self.is_finite_non_zero()
    }

    fn bit(
        &self,
        index: u32,
    ) -> bool {
        (self.sig >> index) & 1 == 1
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L1054>
    pub fn is_denormal(&self) -> bool {
        self.is_finite_non_zero() && self.exponent == self.sem.min_exponent && !self.bit(self.sem.precision - 1)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L1061>
    pub fn is_smallest(&self) -> bool {
        self.is_finite_non_zero() && self.exponent == self.sem.min_exponent && msb1(self.sig) == 1
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L1069>
    pub fn is_smallest_normalized(&self) -> bool {
        self.category == Category::Normal
            && self.exponent == self.sem.min_exponent
            && self.sig == 1 << (self.sem.precision - 1)
    }

    /// Whether the significand, integer bit included, is all ones.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L1087>
    fn is_significand_all_ones(&self) -> bool {
        self.sig & ones(self.sem.precision) == ones(self.sem.precision)
    }

    /// Whether the significand without its integer bit is all zeros.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L1134>
    fn is_significand_all_zeros(&self) -> bool {
        self.sig & ones(self.sem.precision - 1) == 0
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L1171>
    pub fn is_largest(&self) -> bool {
        let is_max_exp = self.is_finite_non_zero() && self.exponent == self.sem.max_exponent;
        is_max_exp && self.is_significand_all_ones()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L1188>
    pub fn is_integer(&self) -> bool {
        if !self.is_finite() {
            return false;
        }
        let mut truncated = *self;
        truncated.round_to_integral(RoundingMode::TowardZero);
        self.compare(&truncated) == CmpResult::Equal
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L4506>
    pub fn is_signaling(&self) -> bool {
        if !self.is_nan() {
            return false;
        }
        // IEEE-754R 2008 6.2.1: A signaling NaN bit string should be encoded
        // with the first bit of the trailing significand being 0.
        !self.bit(self.sem.precision - 2)
    }

    /// The class of the value.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L5963>
    pub fn classify(&self) -> FpClassTest {
        if self.is_zero() {
            return if self.is_negative() { FC_NEG_ZERO } else { FC_POS_ZERO };
        }
        if self.is_normal() {
            return if self.is_negative() { FC_NEG_NORMAL } else { FC_POS_NORMAL };
        }
        if self.is_denormal() {
            return if self.is_negative() { FC_NEG_SUBNORMAL } else { FC_POS_SUBNORMAL };
        }
        if self.is_infinity() {
            return if self.is_negative() { FC_NEG_INF } else { FC_POS_INF };
        }
        assert!(self.is_nan(), "Other class of FP constant");
        if self.is_signaling() { FC_SNAN } else { FC_QNAN }
    }

    /// The exponent of a power of two, `i32::MIN` for anything else.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L4477>
    pub fn get_exact_log2_abs(&self) -> i32 {
        if !self.is_finite() || self.is_zero() {
            return i32::MIN;
        }
        if self.sig.count_ones() > 1 {
            return i32::MIN;
        }
        if self.exponent != self.sem.min_exponent {
            return self.exponent;
        }
        self.exponent - self.sem.precision as i32 + self.sig.trailing_zeros() as i32 + 1
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1646>
    pub fn get_exact_log2(&self) -> i32 {
        if self.is_negative() { i32::MIN } else { self.get_exact_log2_abs() }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1652>
    pub fn is_power_of2(
        &self,
        n: i32,
    ) -> bool {
        n != i32::MIN && self.get_exact_log2() == n
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1656>
    pub fn is_neg_power_of2(
        &self,
        n: i32,
    ) -> bool {
        n != i32::MIN && self.is_negative() && self.get_exact_log2_abs() == n
    }

    /// The unbiased exponent, or one of the `IEK_` values.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L4709>
    pub fn ilogb(&self) -> i32 {
        if self.is_nan() {
            return IEK_NAN;
        }
        if self.is_zero() {
            return IEK_ZERO;
        }
        if self.is_infinity() {
            return IEK_INF;
        }
        if !self.is_denormal() {
            return self.exponent;
        }
        let mut normalized = *self;
        let significand_bits = self.sem.precision as i32 - 1;
        normalized.exponent += significand_bits;
        normalized.normalize(RoundingMode::NearestTiesToEven, LostFraction::ExactlyZero);
        normalized.exponent - significand_bits
    }

    /// IEEE comparison (NaNs compare unordered, 0 == -0).
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L2472>
    pub fn compare(
        &self,
        rhs: &ApFloat,
    ) -> CmpResult {
        use Category::*;
        match (self.category, rhs.category) {
            (NaN, _) | (_, NaN) => return CmpResult::Unordered,
            (Infinity, Normal) | (Infinity, Zero) | (Normal, Zero) => {
                return if self.sign { CmpResult::LessThan } else { CmpResult::GreaterThan };
            }
            (Normal, Infinity) | (Zero, Infinity) | (Zero, Normal) => {
                return if rhs.sign { CmpResult::GreaterThan } else { CmpResult::LessThan };
            }
            (Infinity, Infinity) => {
                return if self.sign == rhs.sign {
                    CmpResult::Equal
                } else if self.sign {
                    CmpResult::LessThan
                } else {
                    CmpResult::GreaterThan
                };
            }
            (Zero, Zero) => return CmpResult::Equal,
            (Normal, Normal) => {}
        }
        // Two normal numbers. Do they have the same sign?
        if self.sign != rhs.sign {
            return if self.sign { CmpResult::LessThan } else { CmpResult::GreaterThan };
        }
        // Compare absolute values; invert result if negative.
        let result = self.compare_absolute_value(rhs);
        if self.sign {
            match result {
                CmpResult::LessThan => return CmpResult::GreaterThan,
                CmpResult::GreaterThan => return CmpResult::LessThan,
                _ => {}
            }
        }
        result
    }

    /// Compare magnitudes; both must be finite and non-zero.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L1548>
    pub fn compare_absolute_value(
        &self,
        rhs: &ApFloat,
    ) -> CmpResult {
        assert!(self.is_finite_non_zero() && rhs.is_finite_non_zero());
        let mut compare = (self.exponent - rhs.exponent).signum();
        // If exponents are equal, do an unsigned comparison of the
        // significands.
        if compare == 0 {
            compare = match self.sig.cmp(&rhs.sig) {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            };
        }
        match compare {
            1 => CmpResult::GreaterThan,
            -1 => CmpResult::LessThan,
            _ => CmpResult::Equal,
        }
    }

    /// Bitwise equality (QNaNs compare equal, 0 != -0).
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L1196>
    pub fn bitwise_is_equal(
        &self,
        rhs: &ApFloat,
    ) -> bool {
        if self.sem != rhs.sem || self.category != rhs.category || self.sign != rhs.sign {
            return false;
        }
        if self.category == Category::Zero || self.category == Category::Infinity {
            return true;
        }
        if self.is_finite_non_zero() && self.exponent != rhs.exponent {
            return false;
        }
        self.sig == rhs.sig
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L2106>
    pub fn change_sign(&mut self) {
        self.sign = !self.sign;
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1402>
    pub fn clear_sign(&mut self) {
        if self.is_negative() {
            self.change_sign();
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1406>
    pub fn copy_sign(
        &mut self,
        rhs: &ApFloat,
    ) {
        if self.is_negative() != rhs.is_negative() {
            self.change_sign();
        }
    }

    /// The absolute value.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1721>
    pub fn abs(mut self) -> ApFloat {
        self.clear_sign();
        self
    }

    /// The negated value (`operator-` and `neg`).
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1727>
    pub fn neg(mut self) -> ApFloat {
        self.change_sign();
        self
    }

    /// Assuming a NaN, quiet its signaling bit.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L4703>
    pub fn make_quiet(&self) -> ApFloat {
        let mut result = *self;
        assert!(result.is_nan());
        result.sig |= 1 << (result.sem.precision - 2);
        result
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L957>
    fn make_nan(
        &mut self,
        snan: bool,
        negative: bool,
        fill: Option<u128>,
    ) {
        self.category = Category::NaN;
        self.sign = negative;
        self.exponent = exponent_nan(self.sem);
        // Set the significand bits to the fill, without the excess bits.
        self.sig = fill.map_or(0, |fill| fill & ones(self.sem.precision - 1));
        let qnan_bit = if self.sem.precision >= 2 { self.sem.precision - 2 } else { 0 };
        if snan {
            // We always have to clear the QNaN bit to make it an SNaN.
            self.sig &= !(1 << qnan_bit);
            // If there are no bits set in the payload, we have to set
            // *something* to make it a NaN instead of an infinity;
            // conventionally, this is the next bit down from the QNaN bit.
            if self.sig == 0 {
                self.sig |= 1 << (qnan_bit - 1);
            }
        } else {
            // We always have to set the QNaN bit to make it a QNaN.
            self.sig |= 1 << qnan_bit;
        }
        // For x87 extended precision, we want to make a NaN, not a pseudo-NaN.
        if self.sem.kind == SemanticsKind::X87DoubleExtended {
            self.sig |= 1 << (qnan_bit + 1);
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L4674>
    fn make_inf(
        &mut self,
        negative: bool,
    ) {
        self.category = Category::Infinity;
        self.sign = negative;
        self.exponent = exponent_inf(self.sem);
        self.sig = 0;
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L4689>
    fn make_zero(
        &mut self,
        negative: bool,
    ) {
        self.category = Category::Zero;
        self.sign = negative;
        self.exponent = exponent_zero(self.sem);
        self.sig = 0;
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L4082>
    fn make_largest(
        &mut self,
        negative: bool,
    ) {
        self.category = Category::Normal;
        self.sign = negative;
        self.exponent = self.sem.max_exponent;
        self.sig = ones(self.sem.precision);
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L4114>
    fn make_smallest(
        &mut self,
        negative: bool,
    ) {
        self.category = Category::Normal;
        self.sign = negative;
        self.exponent = self.sem.min_exponent;
        self.sig = 1;
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L4128>
    fn make_smallest_normalized(
        &mut self,
        negative: bool,
    ) {
        self.category = Category::Normal;
        self.sig = 1 << (self.sem.precision - 1);
        self.sign = negative;
        self.exponent = self.sem.min_exponent;
    }

    /// The value of the bit pattern `bits` in the format.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L3832>
    pub fn from_bits(
        semantics: Sem,
        bits: u128,
    ) -> ApFloat {
        let explicit = u32::from(semantics.has_explicit_integer_bit);
        let trailing_significand_bits = semantics.precision - 1 + explicit;
        let exponent_bits = semantics.size_in_bits - 1 - trailing_significand_bits;
        let exponent_mask = (1_u128 << exponent_bits) - 1;
        let bias = -(semantics.min_exponent - 1);

        let my_significand = bits & ones(trailing_significand_bits);
        let my_exponent = ((bits >> trailing_significand_bits) & exponent_mask) as i32;
        let sign = (bits >> (semantics.size_in_bits - 1)) & 1 == 1;

        let mut value = ApFloat::uninitialized(semantics);
        value.sign = sign;
        let all_zero_significand = my_significand == 0;
        let is_zero = my_exponent == 0 && all_zero_significand;

        let integer_bit = my_significand >> (trailing_significand_bits - 1) & 1;
        let no_integer_bit = my_significand & ones(trailing_significand_bits - 1);
        let is_inf = if semantics.has_explicit_integer_bit {
            my_exponent - bias == exponent_inf(semantics) && integer_bit == 1 && no_integer_bit == 0
        } else {
            my_exponent - bias == exponent_inf(semantics) && all_zero_significand
        };
        if is_inf {
            value.make_inf(sign);
            return value;
        }

        let is_nan = if semantics.has_explicit_integer_bit {
            if my_exponent - bias == exponent_nan(semantics) && no_integer_bit != 0 {
                // regular NaN and pseudoNaN
                true
            } else if my_exponent - bias == exponent_nan(semantics) && no_integer_bit == 0 {
                // pseudoinfinity
                true
            } else {
                // unnormal
                my_exponent - bias != exponent_nan(semantics) && my_exponent != 0 && integer_bit == 0
            }
        } else {
            my_exponent - bias == exponent_nan(semantics) && !all_zero_significand
        };
        if is_nan {
            value.category = Category::NaN;
            value.exponent = exponent_nan(semantics);
            value.sig = my_significand;
            return value;
        }

        if is_zero {
            value.make_zero(sign);
            return value;
        }

        value.category = Category::Normal;
        value.exponent = my_exponent - bias;
        value.sig = my_significand;
        if my_exponent == 0 {
            // denormal
            value.exponent = semantics.min_exponent;
        } else if !semantics.has_explicit_integer_bit {
            value.sig |= 1 << (semantics.precision - 1);
        }
        value
    }

    /// The bit pattern of the value in its format.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L3535>
    pub fn to_bits(&self) -> u128 {
        let semantics = self.sem;
        let explicit = u32::from(semantics.has_explicit_integer_bit);
        let trailing_significand_bits = semantics.precision - 1 + explicit;
        let exponent_bits = semantics.size_in_bits - 1 - trailing_significand_bits;
        let exponent_mask = (1_u128 << exponent_bits) - 1;
        let bias = -(semantics.min_exponent - 1);

        let my_exponent: i32;
        let mut my_significand: u128;
        if self.is_finite_non_zero() {
            let mut exponent = self.exponent + bias;
            my_significand = self.sig;
            if exponent == 1 && !self.bit(semantics.precision - 1) {
                exponent = 0; // denormal
            }
            my_exponent = exponent;
        } else if self.category == Category::Zero {
            my_exponent = exponent_zero(semantics) + bias;
            my_significand = 0;
        } else if self.category == Category::Infinity {
            my_exponent = exponent_inf(semantics) + bias;
            my_significand = 0;
            if semantics.has_explicit_integer_bit {
                my_significand = 1 << (trailing_significand_bits - 1);
            }
        } else {
            my_exponent = exponent_nan(semantics) + bias;
            my_significand = self.sig;
        }
        if !semantics.has_explicit_integer_bit {
            // Clear the integer bit.
            my_significand &= ones(semantics.precision - 1);
        }
        let mut words = my_significand;
        words |= u128::from(self.sign) << (semantics.size_in_bits - 1);
        words |= ((my_exponent as u128) & exponent_mask) << trailing_significand_bits;
        words
    }

    /// The next value up (or down), `IEEE-754R 2008 5.3.1`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L4522>
    pub fn next(
        &mut self,
        next_down: bool,
    ) -> OpStatus {
        // If we are performing nextDown, swap sign so we have -x.
        if next_down {
            self.change_sign();
        }
        let mut result = OP_OK;
        match self.category {
            Category::Infinity => {
                // nextUp(+inf) = +inf
                if self.is_negative() {
                    // nextUp(-inf) = -getLargest()
                    self.make_largest(true);
                }
            }
            Category::NaN => {
                // nextUp(sNaN) = qNaN. Set Invalid flag. nextUp(qNaN) = qNaN,
                // the identity, so we do not change the payload.
                if self.is_signaling() {
                    result = OP_INVALID_OP;
                    // For consistency, propagate the sign of the sNaN to the
                    // qNaN.
                    let negative = self.is_negative();
                    self.make_nan(false, negative, None);
                }
            }
            Category::Zero => {
                // nextUp(pm 0) = +getSmallest()
                self.make_smallest(false);
            }
            Category::Normal => {
                if self.is_smallest() && self.is_negative() {
                    // nextUp(-getSmallest()) = -0
                    self.sig = 0;
                    self.category = Category::Zero;
                    self.exponent = 0;
                } else if self.is_largest() && !self.is_negative() {
                    // nextUp(getLargest()) == INFINITY
                    self.sig = 0;
                    self.category = Category::Infinity;
                    self.exponent = self.sem.max_exponent + 1;
                } else if self.is_negative() {
                    // We only cross a binade boundary that requires adjusting
                    // the exponent if the exponent is not the minimum and the
                    // significand without its integral bit is all zeros.
                    let will_cross_binade_boundary =
                        self.exponent != self.sem.min_exponent && self.is_significand_all_zeros();
                    self.sig -= 1;
                    if will_cross_binade_boundary {
                        // Our result is a normal number: set the integral bit
                        // and decrement the exponent.
                        self.sig |= 1 << (self.sem.precision - 1);
                        self.exponent -= 1;
                    }
                } else {
                    // We only cross a binade boundary if the input is not a
                    // denormal and all of its significand bits are set.
                    let will_cross_binade_boundary = !self.is_denormal() && self.is_significand_all_ones();
                    if will_cross_binade_boundary {
                        self.sig = 1 << (self.sem.precision - 1);
                        assert!(self.exponent != self.sem.max_exponent);
                        self.exponent += 1;
                    } else {
                        self.sig += 1;
                    }
                }
            }
        }
        // If we are performing nextDown, swap sign so we have -nextUp(-x)
        if next_down {
            self.change_sign();
        }
        result
    }

    /// Shift the significand left, subtracting from the exponent.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L1534>
    fn shift_significand_left(
        &mut self,
        bits: u32,
    ) {
        if bits != 0 {
            self.sig <<= bits;
            self.exponent -= bits as i32;
            assert!(self.sig != 0);
        }
    }

    /// Note that a zero result is NOT normalized to a zero.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L1524>
    fn shift_significand_right(
        &mut self,
        bits: u32,
    ) -> LostFraction {
        self.exponent += bits as i32;
        shift_right(&mut self.sig, bits)
    }

    /// Handle overflow: infinity or the largest finite number.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L1588>
    fn handle_overflow(
        &mut self,
        rounding_mode: RoundingMode,
    ) -> OpStatus {
        // Infinity?
        if rounding_mode == RoundingMode::NearestTiesToEven
            || rounding_mode == RoundingMode::NearestTiesToAway
            || (rounding_mode == RoundingMode::TowardPositive && !self.sign)
            || (rounding_mode == RoundingMode::TowardNegative && self.sign)
        {
            self.category = Category::Infinity;
            return OP_OVERFLOW | OP_INEXACT;
        }
        // Otherwise we become the largest finite number.
        self.category = Category::Normal;
        self.exponent = self.sem.max_exponent;
        self.sig = ones(self.sem.precision);
        OP_INEXACT
    }

    /// Whether truncating with the lost fraction rounds away from zero.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L1620>
    fn round_away_from_zero(
        &self,
        rounding_mode: RoundingMode,
        lost_fraction: LostFraction,
        bit: u32,
    ) -> bool {
        assert!(lost_fraction != LostFraction::ExactlyZero);
        match rounding_mode {
            RoundingMode::NearestTiesToAway => {
                lost_fraction == LostFraction::ExactlyHalf || lost_fraction == LostFraction::MoreThanHalf
            }
            RoundingMode::NearestTiesToEven => {
                if lost_fraction == LostFraction::MoreThanHalf {
                    return true;
                }
                // Our zeroes don't have a significand to test.
                if lost_fraction == LostFraction::ExactlyHalf && self.category != Category::Zero {
                    return self.bit(bit);
                }
                false
            }
            RoundingMode::TowardZero => false,
            RoundingMode::TowardPositive => !self.sign,
            RoundingMode::TowardNegative => self.sign,
            _ => panic!("Invalid rounding mode found"),
        }
    }

    /// Normalize and round a finite non-zero value.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L1658>
    fn normalize(
        &mut self,
        rounding_mode: RoundingMode,
        mut lost_fraction: LostFraction,
    ) -> OpStatus {
        if !self.is_finite_non_zero() {
            return OP_OK;
        }
        let precision = self.sem.precision;
        // Before rounding normalize the exponent of normal numbers. One, not
        // zero, based MSB.
        let mut omsb = msb1(self.sig);
        // Only skip this `if` if the value is exactly zero.
        if omsb != 0 || lost_fraction != LostFraction::ExactlyZero {
            // OMSB is numbered from 1. We want to place it in the integer bit
            // numbered PRECISION if possible, with a compensating change in
            // the exponent.
            let mut exponent_change = omsb as i32 - precision as i32;
            // If the resulting exponent is too high, overflow according to the
            // rounding mode.
            if self.exponent + exponent_change > self.sem.max_exponent {
                return self.handle_overflow(rounding_mode);
            }
            // Subnormal numbers have exponent minExponent, and their MSB is
            // forced based on that.
            if self.exponent + exponent_change < self.sem.min_exponent {
                exponent_change = self.sem.min_exponent - self.exponent;
            }
            // Shifting left is easy as we don't lose precision.
            if exponent_change < 0 {
                assert!(lost_fraction == LostFraction::ExactlyZero);
                self.shift_significand_left((-exponent_change) as u32);
                return OP_OK;
            }
            if exponent_change > 0 {
                // Shift right and capture any new lost fraction.
                let lf = self.shift_significand_right(exponent_change as u32);
                lost_fraction = combine_lost_fractions(lf, lost_fraction);
                // Keep OMSB up-to-date.
                omsb = if omsb > exponent_change as u32 { omsb - exponent_change as u32 } else { 0 };
            }
        }

        // Now round the number according to rounding_mode given the lost
        // fraction. As specified in IEEE 754, since we do not trap we do not
        // report underflow for exact results.
        if lost_fraction == LostFraction::ExactlyZero {
            // Canonicalize zeroes.
            if omsb == 0 {
                self.category = Category::Zero;
            }
            return OP_OK;
        }

        // Increment the significand if we're rounding away from zero.
        if self.round_away_from_zero(rounding_mode, lost_fraction, 0) {
            if omsb == 0 {
                self.exponent = self.sem.min_exponent;
            }
            self.sig += 1;
            omsb = msb1(self.sig);
            // Did the significand increment overflow?
            if omsb == precision + 1 {
                // Renormalize by incrementing the exponent and shifting our
                // significand right one. However if we already have the
                // maximum exponent we overflow to infinity.
                if self.exponent == self.sem.max_exponent {
                    let toward = if self.sign { RoundingMode::TowardNegative } else { RoundingMode::TowardPositive };
                    return self.handle_overflow(toward);
                }
                self.shift_significand_right(1);
                return OP_INEXACT;
            }
        }

        // The normal case - we were and are not denormal, and any significand
        // increment above didn't overflow.
        if omsb == precision {
            return OP_INEXACT;
        }

        // We have a non-zero denormal. Canonicalize zeroes.
        assert!(omsb < precision);
        if omsb == 0 {
            self.category = Category::Zero;
        }
        // The zero case is a denormal that underflowed to zero.
        OP_UNDERFLOW | OP_INEXACT
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L1791>
    fn add_or_subtract_specials(
        &mut self,
        rhs: &ApFloat,
        subtract: bool,
    ) -> OpStatus {
        use Category::*;
        match (self.category, rhs.category) {
            (Zero, NaN) | (Normal, NaN) | (Infinity, NaN) => {
                *self = *rhs;
                self.nan_status(rhs)
            }
            (NaN, _) => self.nan_status(rhs),
            (Normal, Zero) | (Infinity, Normal) | (Infinity, Zero) => OP_OK,
            (Normal, Infinity) | (Zero, Infinity) => {
                self.category = Infinity;
                self.sign = rhs.sign ^ subtract;
                OP_OK
            }
            (Zero, Normal) => {
                *self = *rhs;
                self.sign = rhs.sign ^ subtract;
                OP_OK
            }
            (Zero, Zero) => {
                // Sign depends on rounding mode; handled by caller.
                OP_OK
            }
            (Infinity, Infinity) => {
                // Differently signed infinities can only be validly
                // subtracted.
                if (self.sign ^ rhs.sign) != subtract {
                    self.make_nan(false, false, None);
                    return OP_INVALID_OP;
                }
                OP_OK
            }
            (Normal, Normal) => OP_DIV_BY_ZERO,
        }
    }

    /// The tail the NaN cases of the `*Specials` share.
    fn nan_status(
        &mut self,
        rhs: &ApFloat,
    ) -> OpStatus {
        if self.is_signaling() {
            *self = self.make_quiet();
            return OP_INVALID_OP;
        }
        if rhs.is_signaling() { OP_INVALID_OP } else { OP_OK }
    }

    /// Add or subtract two normal numbers' significands.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L1848>
    fn add_or_subtract_significand(
        &mut self,
        rhs: &ApFloat,
        mut subtract: bool,
    ) -> LostFraction {
        let lost_fraction;
        // Determine if the operation on the absolute values is effectively an
        // addition or subtraction.
        subtract ^= self.sign ^ rhs.sign;
        // Are we bigger exponent-wise than the RHS?
        let bits = self.exponent - rhs.exponent;
        // Subtraction is more subtle than one might naively expect.
        if subtract {
            let mut temp_rhs = *rhs;
            let mut lost_fraction_is_from_rhs = false;
            let mut lost;
            if bits == 0 {
                lost = LostFraction::ExactlyZero;
            } else if bits > 0 {
                lost = temp_rhs.shift_significand_right((bits - 1) as u32);
                lost_fraction_is_from_rhs = true;
                self.shift_significand_left(1);
            } else {
                lost = self.shift_significand_right((-bits - 1) as u32);
                temp_rhs.shift_significand_left(1);
            }
            // Should we reverse the subtraction.
            let cmp_result = self.compare_absolute_value(&temp_rhs);
            if cmp_result == CmpResult::LessThan {
                let borrow = lost != LostFraction::ExactlyZero && !lost_fraction_is_from_rhs;
                if borrow {
                    // The lost fraction is being subtracted, borrow from the
                    // significand and invert `lost`.
                    if lost == LostFraction::LessThanHalf {
                        lost = LostFraction::MoreThanHalf;
                    } else if lost == LostFraction::MoreThanHalf {
                        lost = LostFraction::LessThanHalf;
                    }
                }
                self.sig = temp_rhs.sig - self.sig - u128::from(borrow);
                self.sign = !self.sign;
            } else if cmp_result == CmpResult::GreaterThan {
                let borrow = lost != LostFraction::ExactlyZero && lost_fraction_is_from_rhs;
                if borrow {
                    if lost == LostFraction::LessThanHalf {
                        lost = LostFraction::MoreThanHalf;
                    } else if lost == LostFraction::MoreThanHalf {
                        lost = LostFraction::LessThanHalf;
                    }
                }
                self.sig = self.sig - temp_rhs.sig - u128::from(borrow);
            } else {
                self.sig = 0;
                if lost != LostFraction::ExactlyZero && lost_fraction_is_from_rhs {
                    // rhs is slightly larger due to the lost fraction, flip
                    // the sign.
                    self.sign = !self.sign;
                }
            }
            lost_fraction = lost;
        } else if bits > 0 {
            let mut temp_rhs = *rhs;
            lost_fraction = temp_rhs.shift_significand_right(bits as u32);
            self.sig += temp_rhs.sig;
        } else {
            lost_fraction = self.shift_significand_right((-bits) as u32);
            self.sig += rhs.sig;
        }
        lost_fraction
    }

    /// Normalized addition or subtraction.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L2117>
    fn add_or_subtract(
        &mut self,
        rhs: &ApFloat,
        rounding_mode: RoundingMode,
        subtract: bool,
    ) -> OpStatus {
        let mut fs = self.add_or_subtract_specials(rhs, subtract);
        // This return code means it was not a simple case.
        if fs == OP_DIV_BY_ZERO {
            let lost_fraction = self.add_or_subtract_significand(rhs, subtract);
            fs = self.normalize(rounding_mode, lost_fraction);
            // Can only be zero if we lost no fraction.
            assert!(self.category != Category::Zero || lost_fraction == LostFraction::ExactlyZero);
        }
        // If two numbers add (exactly) to zero, IEEE 754 decrees it is a
        // positive zero unless rounding to minus infinity, except that adding
        // two like-signed zeroes gives that zero.
        if self.category == Category::Zero && (rhs.category != Category::Zero || (self.sign == rhs.sign) == subtract) {
            self.sign = rounding_mode == RoundingMode::TowardNegative;
        }
        fs
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L2148>
    pub fn add(
        &mut self,
        rhs: &ApFloat,
        rounding_mode: RoundingMode,
    ) -> OpStatus {
        self.add_or_subtract(rhs, rounding_mode, false)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L2154>
    pub fn subtract(
        &mut self,
        rhs: &ApFloat,
        rounding_mode: RoundingMode,
    ) -> OpStatus {
        self.add_or_subtract(rhs, rounding_mode, true)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L1936>
    fn multiply_specials(
        &mut self,
        rhs: &ApFloat,
    ) -> OpStatus {
        use Category::*;
        match (self.category, rhs.category) {
            (Zero, NaN) | (Normal, NaN) | (Infinity, NaN) => {
                *self = *rhs;
                self.sign = false;
                self.sign ^= rhs.sign; // restore the original sign
                self.nan_status(rhs)
            }
            (NaN, _) => {
                self.sign ^= rhs.sign; // restore the original sign
                self.nan_status(rhs)
            }
            (Normal, Infinity) | (Infinity, Normal) | (Infinity, Infinity) => {
                self.category = Infinity;
                OP_OK
            }
            (Zero, Normal) | (Normal, Zero) | (Zero, Zero) => {
                self.category = Zero;
                OP_OK
            }
            (Zero, Infinity) | (Infinity, Zero) => {
                self.make_nan(false, false, None);
                OP_INVALID_OP
            }
            (Normal, Normal) => OP_OK,
        }
    }

    /// Multiply the significands, returning the lost fraction.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L1304>
    fn multiply_significand(
        &mut self,
        rhs: &ApFloat,
    ) -> LostFraction {
        let precision = self.sem.precision;
        // Twice as many bits as the original significand.
        let mut full_significand = self.sig * rhs.sig;
        let mut lost_fraction = LostFraction::ExactlyZero;
        // One, not zero, based MSB.
        let omsb = msb1(full_significand);
        self.exponent += rhs.exponent;
        // The result is c(2p) c(2p-1) . c(2p-2) ... c0 * 2^(e1+e2): move the
        // radix point toward left by two bits, and adjust the exponent.
        self.exponent += 2;
        // Move the radix point from position "2*precision - 1" to
        // "precision - 1".
        self.exponent -= precision as i32 + 1;
        // In case MSB resides at the left-hand side of radix point, shift the
        // mantissa right by some amount to make sure the MSB reside right
        // before the radix point. The result is not normalized when
        // "omsb < precision"; the caller calls normalize().
        if omsb > precision {
            let bits = omsb - precision;
            let lf = shift_right(&mut full_significand, bits);
            lost_fraction = combine_lost_fractions(lf, lost_fraction);
            self.exponent += bits as i32;
        }
        self.sig = full_significand;
        lost_fraction
    }

    /// Normalized multiply.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L2160>
    pub fn multiply(
        &mut self,
        rhs: &ApFloat,
        rounding_mode: RoundingMode,
    ) -> OpStatus {
        self.sign ^= rhs.sign;
        let mut fs = self.multiply_specials(rhs);
        if self.is_finite_non_zero() {
            let lost_fraction = self.multiply_significand(rhs);
            fs = self.normalize(rounding_mode, lost_fraction);
            if lost_fraction != LostFraction::ExactlyZero {
                fs |= OP_INEXACT;
            }
        }
        fs
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L1980>
    fn divide_specials(
        &mut self,
        rhs: &ApFloat,
    ) -> OpStatus {
        use Category::*;
        match (self.category, rhs.category) {
            (Zero, NaN) | (Normal, NaN) | (Infinity, NaN) => {
                *self = *rhs;
                self.sign = false;
                self.sign ^= rhs.sign; // restore the original sign
                self.nan_status(rhs)
            }
            (NaN, _) => {
                self.sign ^= rhs.sign; // restore the original sign
                self.nan_status(rhs)
            }
            (Infinity, Zero) | (Infinity, Normal) | (Zero, Infinity) | (Zero, Normal) => OP_OK,
            (Normal, Infinity) => {
                self.category = Zero;
                OP_OK
            }
            (Normal, Zero) => {
                self.category = Infinity;
                OP_DIV_BY_ZERO
            }
            (Infinity, Infinity) | (Zero, Zero) => {
                self.make_nan(false, false, None);
                OP_INVALID_OP
            }
            (Normal, Normal) => OP_OK,
        }
    }

    /// Divide the significands by long division, returning the lost fraction.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L1439>
    fn divide_significand(
        &mut self,
        rhs: &ApFloat,
    ) -> LostFraction {
        let mut dividend = self.sig;
        let mut divisor = rhs.sig;
        self.sig = 0;
        self.exponent -= rhs.exponent;
        let precision = self.sem.precision;
        // Normalize the divisor.
        let mut bit = precision - msb1(divisor);
        if bit != 0 {
            self.exponent += bit as i32;
            divisor <<= bit;
        }
        // Normalize the dividend.
        bit = precision - msb1(dividend);
        if bit != 0 {
            self.exponent -= bit as i32;
            dividend <<= bit;
        }
        // Ensure the dividend >= divisor initially for the loop below.
        // Incidentally, this means that the division loop below is guaranteed
        // to set the integer bit to one.
        if dividend < divisor {
            self.exponent -= 1;
            dividend <<= 1;
            assert!(dividend >= divisor);
        }
        // Long division.
        let mut bit = precision;
        while bit != 0 {
            if dividend >= divisor {
                dividend -= divisor;
                self.sig |= 1 << (bit - 1);
            }
            dividend <<= 1;
            bit -= 1;
        }
        // Figure out the lost fraction.
        if dividend > divisor {
            LostFraction::MoreThanHalf
        } else if dividend == divisor {
            LostFraction::ExactlyHalf
        } else if dividend == 0 {
            LostFraction::ExactlyZero
        } else {
            LostFraction::LessThanHalf
        }
    }

    /// Normalized divide.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L2178>
    pub fn divide(
        &mut self,
        rhs: &ApFloat,
        rounding_mode: RoundingMode,
    ) -> OpStatus {
        self.sign ^= rhs.sign;
        let mut fs = self.divide_specials(rhs);
        if self.is_finite_non_zero() {
            let lost_fraction = self.divide_significand(rhs);
            fs = self.normalize(rounding_mode, lost_fraction);
            if lost_fraction != LostFraction::ExactlyZero {
                fs |= OP_INEXACT;
            }
        }
        fs
    }

    /// `operator+`: rounding ties to the nearest even.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1371>
    pub fn plus(
        &self,
        rhs: &ApFloat,
    ) -> ApFloat {
        let mut result = *self;
        result.add(rhs, RoundingMode::NearestTiesToEven);
        result
    }

    /// `operator-`: rounding ties to the nearest even.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1379>
    pub fn minus(
        &self,
        rhs: &ApFloat,
    ) -> ApFloat {
        let mut result = *self;
        result.subtract(rhs, RoundingMode::NearestTiesToEven);
        result
    }

    /// `operator*`: rounding ties to the nearest even.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1387>
    pub fn times(
        &self,
        rhs: &ApFloat,
    ) -> ApFloat {
        let mut result = *self;
        result.multiply(rhs, RoundingMode::NearestTiesToEven);
        result
    }

    /// `operator/`: rounding ties to the nearest even.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1395>
    pub fn divided_by(
        &self,
        rhs: &ApFloat,
    ) -> ApFloat {
        let mut result = *self;
        result.divide(rhs, RoundingMode::NearestTiesToEven);
        result
    }

    /// Round to an integral value in the rounding mode.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L2387>
    pub fn round_to_integral(
        &mut self,
        rounding_mode: RoundingMode,
    ) -> OpStatus {
        if self.is_infinity() {
            return OP_OK;
        }
        if self.is_nan() {
            if self.is_signaling() {
                *self = self.make_quiet();
                return OP_INVALID_OP;
            }
            return OP_OK;
        }
        if self.is_zero() {
            return OP_OK;
        }
        // If the exponent is large enough, we know that this value is already
        // integral, and the arithmetic below would potentially cause it to
        // saturate to +/-Inf. Bail out early instead.
        if self.exponent + 1 >= self.sem.precision as i32 {
            return OP_OK;
        }
        // We add 2^(p-1), where p is the precision of our format, and then
        // subtract it back off again. The choice of rounding modes for the
        // addition/subtraction determines the rounding mode for our integral
        // rounding as well.
        let mut magic_constant = ApFloat {
            sem: self.sem,
            sign: false,
            category: Category::Normal,
            exponent: self.sem.precision as i32 - 1,
            sig: 1 << (self.sem.precision - 1),
        };
        magic_constant.sign = self.sign;
        // Preserve the input sign so that we can handle the case of zero
        // result correctly.
        let input_sign = self.is_negative();
        let fs = self.add(&magic_constant, rounding_mode);
        // Current value and 'MagicConstant' are both integers, so the result
        // of the subtraction is always exact according to Sterbenz' lemma.
        self.subtract(&magic_constant, rounding_mode);
        // Restore the input sign.
        if input_sign != self.is_negative() {
            self.change_sign();
        }
        fs
    }

    /// Convert to another format, returning the status and whether the
    /// conversion lost information.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/APFloat.cpp#L2548>
    pub fn convert(
        &mut self,
        to_semantics: Sem,
        rounding_mode: RoundingMode,
    ) -> (OpStatus, bool) {
        if self.sem == to_semantics {
            return (OP_OK, false);
        }
        let from_semantics = self.sem;
        let is_signaling = self.is_signaling();
        let mut lost_fraction = LostFraction::ExactlyZero;
        let mut shift = to_semantics.precision as i32 - from_semantics.precision as i32;
        let mut x86_special_nan = false;
        if from_semantics.kind == SemanticsKind::X87DoubleExtended
            && to_semantics.kind != SemanticsKind::X87DoubleExtended
            && self.category == Category::NaN
            && (self.sig & (1 << 63) == 0 || self.sig & (1 << 62) == 0)
        {
            // x86 has some unusual NaNs which cannot be represented in any
            // other format; note them here.
            x86_special_nan = true;
        }
        // If this is a truncation of a denormal number, and the target
        // semantics has larger exponent range than the source semantics, the
        // right shift could lose result mantissa bits. Adjust exponent instead
        // of performing excessive shift. Also do a similar trick in case
        // shifting denormal would produce zero significand as this case isn't
        // handled correctly by normalize.
        if shift < 0 && self.is_finite_non_zero() {
            let omsb = msb1(self.sig) as i32;
            let mut exponent_change = omsb - from_semantics.precision as i32;
            if self.exponent + exponent_change < to_semantics.min_exponent {
                exponent_change = to_semantics.min_exponent - self.exponent;
            }
            exponent_change = max(exponent_change, shift);
            if exponent_change < 0 {
                shift -= exponent_change;
                self.exponent += exponent_change;
            } else if omsb <= -shift {
                // leave at least one bit set
                exponent_change = omsb + shift - 1;
                shift -= exponent_change;
                self.exponent += exponent_change;
            }
        }
        // If this is a truncation, perform the shift before we narrow the
        // storage.
        if shift < 0 && (self.is_finite_non_zero() || self.category == Category::NaN) {
            lost_fraction = shift_right(&mut self.sig, (-shift) as u32);
        }
        // Now that we have the right storage, switch the semantics.
        self.sem = to_semantics;
        // If this is an extension, perform the shift now that the storage is
        // available.
        if shift > 0 && (self.is_finite_non_zero() || self.category == Category::NaN) {
            self.sig <<= shift as u32;
        }
        let loses_info;
        let fs;
        if self.is_finite_non_zero() {
            fs = self.normalize(rounding_mode, lost_fraction);
            loses_info = fs != OP_OK;
        } else if self.category == Category::NaN {
            loses_info = lost_fraction != LostFraction::ExactlyZero || x86_special_nan;
            // For x87 extended precision, we want to make a NaN, not a special
            // NaN if the input wasn't special either.
            if !x86_special_nan && self.sem.kind == SemanticsKind::X87DoubleExtended {
                self.sig |= 1 << (self.sem.precision - 1);
            }
            // Convert of sNaN creates qNaN and raises an exception (invalid
            // op). This also guarantees that a sNaN does not become Inf on a
            // truncation that loses all payload bits.
            if is_signaling {
                *self = self.make_quiet();
                fs = OP_INVALID_OP;
            } else {
                fs = OP_OK;
            }
        } else {
            loses_info = false;
            fs = OP_OK;
        }
        (fs, loses_info)
    }
}

/// The `bits` low bits set.
fn ones(bits: u32) -> u128 {
    if bits >= 128 { u128::MAX } else { (1_u128 << bits) - 1 }
}

/// IEEE-754 2008 minNum: the smaller of two, the other if one is a quiet NaN,
/// a quiet NaN if one is signaling; -0 is less than +0.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1737>
pub fn minnum(
    a: &ApFloat,
    b: &ApFloat,
) -> ApFloat {
    if a.is_signaling() {
        return a.make_quiet();
    }
    if b.is_signaling() {
        return b.make_quiet();
    }
    if a.is_nan() {
        return *b;
    }
    if b.is_nan() {
        return *a;
    }
    if a.is_zero() && b.is_zero() && (a.is_negative() != b.is_negative()) {
        return if a.is_negative() { *a } else { *b };
    }
    if b.compare(a) == CmpResult::LessThan { *b } else { *a }
}

/// IEEE-754 2008 maxNum; +0 is greater than -0.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1756>
pub fn maxnum(
    a: &ApFloat,
    b: &ApFloat,
) -> ApFloat {
    if a.is_signaling() {
        return a.make_quiet();
    }
    if b.is_signaling() {
        return b.make_quiet();
    }
    if a.is_nan() {
        return *b;
    }
    if b.is_nan() {
        return *a;
    }
    if a.is_zero() && b.is_zero() && (a.is_negative() != b.is_negative()) {
        return if a.is_negative() { *b } else { *a };
    }
    if a.compare(b) == CmpResult::LessThan { *b } else { *a }
}

/// IEEE 754-2019 minimum: a quiet NaN if an argument is a NaN, -0 is less
/// than +0.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1774>
pub fn minimum(
    a: &ApFloat,
    b: &ApFloat,
) -> ApFloat {
    if a.is_nan() {
        return a.make_quiet();
    }
    if b.is_nan() {
        return b.make_quiet();
    }
    if a.is_zero() && b.is_zero() && (a.is_negative() != b.is_negative()) {
        return if a.is_negative() { *a } else { *b };
    }
    if b.compare(a) == CmpResult::LessThan { *b } else { *a }
}

/// IEEE 754-2019 minimumNumber: does not propagate NaNs.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1787>
pub fn minimumnum(
    a: &ApFloat,
    b: &ApFloat,
) -> ApFloat {
    if a.is_nan() {
        return if b.is_nan() { b.make_quiet() } else { *b };
    }
    if b.is_nan() {
        return *a;
    }
    if a.is_zero() && b.is_zero() && (a.is_negative() != b.is_negative()) {
        return if a.is_negative() { *a } else { *b };
    }
    if b.compare(a) == CmpResult::LessThan { *b } else { *a }
}

/// IEEE 754-2019 maximum.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1801>
pub fn maximum(
    a: &ApFloat,
    b: &ApFloat,
) -> ApFloat {
    if a.is_nan() {
        return a.make_quiet();
    }
    if b.is_nan() {
        return b.make_quiet();
    }
    if a.is_zero() && b.is_zero() && (a.is_negative() != b.is_negative()) {
        return if a.is_negative() { *b } else { *a };
    }
    if a.compare(b) == CmpResult::LessThan { *b } else { *a }
}

/// IEEE 754-2019 maximumNumber.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/APFloat.h#L1814>
pub fn maximumnum(
    a: &ApFloat,
    b: &ApFloat,
) -> ApFloat {
    if a.is_nan() {
        return if b.is_nan() { b.make_quiet() } else { *b };
    }
    if b.is_nan() {
        return *a;
    }
    if a.is_zero() && b.is_zero() && (a.is_negative() != b.is_negative()) {
        return if a.is_negative() { *b } else { *a };
    }
    if a.compare(b) == CmpResult::LessThan { *b } else { *a }
}
