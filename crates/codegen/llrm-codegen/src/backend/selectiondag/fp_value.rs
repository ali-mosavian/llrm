//! `ConstantFPSDNode` values as the bits of their type (DEVIATE SD11: no
//! `APFloat`): classification, negation and the round-to-nearest-even
//! arithmetic of `foldConstantFPMath`.
//!
//! DEVIATE FP1: `f32` and `f64` fold on the host's IEEE arithmetic; `f80`
//! (x87) and `f16` classify and negate but do not fold. A NaN result or NaN
//! operand does not fold either (`APFloat` makes a canonical quiet NaN, the
//! host keeps a payload and a sign), so the node stays and runs. Exit: an
//! `APFloat` port. Owner sdag-c.
//!
//! # See also
//! LLVM's `SelectionDAG::foldConstantFPMath`
//! ([L8229](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAG.cpp#L8229)).

use super::isd::ISD;
use super::mvt::{EVT, MVT};

/// What `APFloat` answers about a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FpClass {
    pub negative: bool,
    pub zero: bool,
    pub nan: bool,
    pub infinite: bool,
    pub one: bool,
}

/// Sign-bit position and the (exponent, fraction) split of `vt`.
fn layout(vt: EVT) -> Option<(u32, u32, u32)> {
    match vt {
        EVT::Simple(MVT::F32) => Some((31, 8, 23)),
        EVT::Simple(MVT::F64) => Some((63, 11, 52)),
        EVT::Simple(MVT::F80) => Some((79, 15, 64)),
        _ => None,
    }
}

pub fn classify(
    vt: EVT,
    bits: u128,
) -> Option<FpClass> {
    let (sign, exp_bits, frac_bits) = layout(vt)?;
    let exp = (bits >> frac_bits) & ((1u128 << exp_bits) - 1);
    let frac = bits & ((1u128 << frac_bits) - 1);
    let all_ones = (1u128 << exp_bits) - 1;
    // x87 keeps the integer bit in the fraction: the value's own fraction
    // is what is below it.
    let (frac, one_frac) = if vt == EVT::Simple(MVT::F80) { (frac & !(1 << 63), 1u128 << 63) } else { (frac, 0) };
    let bias = all_ones >> 1;
    Some(FpClass {
        negative: (bits >> sign) & 1 == 1,
        zero: exp == 0 && bits & ((1u128 << frac_bits) - 1) == 0,
        nan: exp == all_ones && frac != 0,
        infinite: exp == all_ones && frac == 0,
        one: exp == bias && (bits & ((1u128 << frac_bits) - 1)) == one_frac && (bits >> sign) & 1 == 0,
    })
}

/// `-x` (`changeSign`).
pub fn negate(
    vt: EVT,
    bits: u128,
) -> Option<u128> {
    let (sign, ..) = layout(vt)?;
    Some(bits ^ (1u128 << sign))
}

/// `|x|` (`clearSign`).
pub fn abs(
    vt: EVT,
    bits: u128,
) -> Option<u128> {
    let (sign, ..) = layout(vt)?;
    Some(bits & !(1u128 << sign))
}

/// The positive integer `value` (`getConstantFP(2.0, ..)`).
pub fn from_integer(
    vt: EVT,
    value: u64,
) -> Option<u128> {
    match vt {
        EVT::Simple(MVT::F32) => Some((value as f32).to_bits() as u128),
        EVT::Simple(MVT::F64) => Some((value as f64).to_bits() as u128),
        EVT::Simple(MVT::F80) if value == 0 => Some(0),
        EVT::Simple(MVT::F80) => {
            let lead = value.leading_zeros();
            Some((((0x3FFF + 63 - lead) as u128) << 64) | ((value << lead) as u128))
        }
        _ => None,
    }
}

/// `-0.0` of `vt`.
pub fn negative_zero(vt: EVT) -> Option<u128> {
    negate(vt, 0)
}

/// `C1 op C2` rounded to nearest even; `None` where the type does not fold,
/// or a NaN is involved.
pub fn fold(
    opcode: ISD,
    vt: EVT,
    a: u128,
    b: u128,
) -> Option<u128> {
    fn apply(
        opcode: ISD,
        x: f64,
        y: f64,
    ) -> Option<f64> {
        Some(match opcode {
            ISD::FAdd => x + y,
            ISD::FSub => x - y,
            ISD::FMul => x * y,
            ISD::FDiv => x / y,
            _ => return None,
        })
    }
    match vt {
        // A double holds the exact result of a single's `+ - * /` to
        // 2p+2 bits, so rounding it once more is the single's own rounding.
        EVT::Simple(MVT::F32) => {
            let (x, y) = (f32::from_bits(a as u32), f32::from_bits(b as u32));
            let r = apply(opcode, f64::from(x), f64::from(y))? as f32;
            (!x.is_nan() && !y.is_nan() && !r.is_nan()).then(|| u128::from(r.to_bits()))
        }
        EVT::Simple(MVT::F64) => {
            let (x, y) = (f64::from_bits(a as u64), f64::from_bits(b as u64));
            let r = apply(opcode, x, y)?;
            (!x.is_nan() && !y.is_nan() && !r.is_nan()).then(|| r.to_bits() as u128)
        }
        _ => None,
    }
}

/// `bits` of `from` as a value of `to` (`FP_ROUND` and `FP_EXTEND` of a
/// constant), where both are `f32` or `f64` and the value is not a NaN.
pub fn convert(
    from: EVT,
    to: EVT,
    bits: u128,
) -> Option<u128> {
    let value = match from {
        EVT::Simple(MVT::F32) => f64::from(f32::from_bits(bits as u32)),
        EVT::Simple(MVT::F64) => f64::from_bits(bits as u64),
        _ => return None,
    };
    if value.is_nan() {
        return None;
    }
    match to {
        EVT::Simple(MVT::F32) => Some(u128::from((value as f32).to_bits())),
        EVT::Simple(MVT::F64) => Some(u128::from(value.to_bits())),
        _ => None,
    }
}

/// The signed integer `value` as a value of `vt` (`SINT_TO_FP` of a constant),
/// rounded to nearest even.
pub fn from_signed(
    vt: EVT,
    value: i64,
) -> Option<u128> {
    match vt {
        EVT::Simple(MVT::F32) => Some(u128::from((value as f32).to_bits())),
        EVT::Simple(MVT::F64) => Some((value as f64).to_bits() as u128),
        _ => None,
    }
}

/// `bits` of `vt` truncated to a signed integer of `width` bits, where it
/// fits (`FP_TO_SINT` of a constant; `APFloat` reports an overflow or a NaN
/// as invalid and the node stays).
pub fn to_signed(
    vt: EVT,
    bits: u128,
    width: u32,
) -> Option<i64> {
    let value = match vt {
        EVT::Simple(MVT::F32) => f64::from(f32::from_bits(bits as u32)),
        EVT::Simple(MVT::F64) => f64::from_bits(bits as u64),
        _ => return None,
    };
    let limit = (width - 1).min(62);
    let bound = (1i64 << limit) as f64;
    (value.is_finite() && value.trunc() >= -bound && value.trunc() < bound && width <= 63)
        .then_some(value.trunc() as i64)
}

/// [`fold`], but only where the result is exact: what a function that
/// evaluates at the x87's extended precision may fold (it rounds once, at the
/// store).
pub fn fold_exact(
    opcode: ISD,
    vt: EVT,
    a: u128,
    b: u128,
) -> Option<u128> {
    let bits = fold(opcode, vt, a, b)?;
    let exact = match vt {
        EVT::Simple(MVT::F32) => {
            let (x, y) = (f64::from(f32::from_bits(a as u32)), f64::from(f32::from_bits(b as u32)));
            let r = f64::from(f32::from_bits(bits as u32));
            let wide = match opcode {
                ISD::FAdd => x + y,
                ISD::FSub => x - y,
                ISD::FMul => x * y,
                _ => x / y,
            };
            r == wide
        }
        EVT::Simple(MVT::F64) => {
            let (x, y) = (f64::from_bits(a as u64), f64::from_bits(b as u64));
            let r = f64::from_bits(bits as u64);
            match opcode {
                ISD::FAdd => (r - x) - y == 0.0 && (r - y) - x == 0.0,
                ISD::FSub => (r - x) + y == 0.0 && (x - r) - y == 0.0,
                ISD::FMul => (-x).mul_add(y, r) == 0.0 || x.mul_add(y, -r) == 0.0,
                _ => (-r).mul_add(y, x) == 0.0,
            }
        }
        _ => false,
    };
    exact.then_some(bits)
}

/// `1.0 / x` when it is a normal finite value; `exact` is `opOK` (a power of
/// two), otherwise `opInexact`. `None` where `APFloat` reports overflow,
/// underflow or a denormal, or the type does not fold.
pub fn reciprocal(
    vt: EVT,
    bits: u128,
) -> Option<(u128, bool)> {
    let (r, exact_input) = match vt {
        EVT::Simple(MVT::F32) => {
            let x = f32::from_bits(bits as u32);
            let r = 1.0f32 / x;
            (r.is_normal().then(|| u128::from(r.to_bits()))?, x.is_normal() && bits & 0x7F_FFFF == 0)
        }
        EVT::Simple(MVT::F64) => {
            let x = f64::from_bits(bits as u64);
            let r = 1.0f64 / x;
            (r.is_normal().then(|| u128::from(r.to_bits()))?, x.is_normal() && bits & ((1 << 52) - 1) == 0)
        }
        _ => return None,
    };
    Some((r, exact_input))
}

/// The quiet NaN `getConstantFP(APFloat::getNaN(..))` makes.
pub fn quiet_nan(vt: EVT) -> Option<u128> {
    match vt {
        EVT::Simple(MVT::F32) => Some(0x7FC0_0000),
        EVT::Simple(MVT::F64) => Some(0x7FF8_0000_0000_0000),
        EVT::Simple(MVT::F80) => Some((0x7FFF << 64) | (3u128 << 62)),
        _ => None,
    }
}
