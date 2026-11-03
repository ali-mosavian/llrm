//! A decimal literal rounded once to an `x86_fp80`
//! (`APFloat::convertFromString`).
//!
//! The IR writer prints an extended constant in decimal when the shortest
//! decimal reads back as the same value (`1.1`, `0.1`, `2.5`), so the reader
//! must round that text at 64 bits. Rounding it to a double first made `0.1L`
//! the double 0.1.

use std::cmp::Ordering;

/// An unsigned integer of any size, least significant limb first, without
/// leading zero limbs.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Big(Vec<u32>);

impl Big {
    fn zero() -> Big {
        Big(Vec::new())
    }

    fn is_zero(&self) -> bool {
        self.0.is_empty()
    }

    fn mul_add(
        &mut self,
        factor: u32,
        add: u32,
    ) {
        let mut carry = u64::from(add);
        for limb in &mut self.0 {
            let product = u64::from(*limb) * u64::from(factor) + carry;
            *limb = product as u32;
            carry = product >> 32;
        }
        if carry != 0 {
            self.0.push(carry as u32);
        }
    }

    fn bits(&self) -> i64 {
        self.0.last().map_or(0, |top| 32 * (self.0.len() as i64 - 1) + i64::from(32 - top.leading_zeros()))
    }

    fn shl(
        &self,
        by: u32,
    ) -> Big {
        if self.is_zero() {
            return Big::zero();
        }
        let (limbs, bits) = ((by / 32) as usize, by % 32);
        let mut out = vec![0u32; limbs];
        let mut carry = 0u32;
        for limb in &self.0 {
            out.push(if bits == 0 { *limb } else { (*limb << bits) | carry });
            carry = if bits == 0 { 0 } else { *limb >> (32 - bits) };
        }
        if carry != 0 {
            out.push(carry);
        }
        Big(out)
    }

    fn sub(
        &mut self,
        other: &Big,
    ) {
        let mut borrow = 0i64;
        for (at, limb) in self.0.iter_mut().enumerate() {
            let taken = i64::from(*limb) - i64::from(other.0.get(at).copied().unwrap_or(0)) - borrow;
            borrow = i64::from(taken < 0);
            *limb = taken.rem_euclid(1 << 32) as u32;
        }
        while self.0.last() == Some(&0) {
            self.0.pop();
        }
    }

    /// `self / divisor` and the remainder, for a quotient below
    /// 2^`quotient_bits`.
    fn div_rem(
        &self,
        divisor: &Big,
        quotient_bits: u32,
    ) -> (u128, Big) {
        let (mut rest, mut quotient) = (self.clone(), 0u128);
        for at in (0..quotient_bits).rev() {
            let step = divisor.shl(at);
            if rest >= step {
                rest.sub(&step);
                quotient |= 1 << at;
            }
        }
        (quotient, rest)
    }
}

impl Ord for Big {
    fn cmp(
        &self,
        other: &Big,
    ) -> Ordering {
        self.0.len().cmp(&other.0.len()).then_with(|| self.0.iter().rev().cmp(other.0.iter().rev()))
    }
}

impl PartialOrd for Big {
    fn partial_cmp(
        &self,
        other: &Big,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

const SMALLEST_EXPONENT: i64 = -16382;
const LARGEST_EXPONENT: i64 = 16383;
const BIAS: i64 = 16383;
/// The quantum of the smallest denormal: 2^-16445.
const DENORMAL_QUANTUM: i64 = SMALLEST_EXPONENT - 63;

/// The 80 bits of the `x86_fp80` nearest `text`, ties to even; None if `text`
/// is not `[-]digits[.digits][e[+-]digits]`.
pub fn x87_bits(text: &str) -> Option<u128> {
    let (negative, rest) =
        text.strip_prefix('-').map_or((false, text.strip_prefix('+').unwrap_or(text)), |rest| (true, rest));
    let (number, exponent) = rest
        .split_once(['e', 'E'])
        .map_or((rest, 0), |(number, exponent)| (number, exponent.parse::<i64>().unwrap_or(i64::MAX)));
    if exponent == i64::MAX {
        return None;
    }
    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    let digits = format!("{whole}{fraction}");
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let digits = digits.trim_start_matches('0');
    let sign = u128::from(negative) << 79;
    if digits.is_empty() {
        return Some(sign);
    }
    let scale = exponent - fraction.len() as i64;
    let magnitude = digits.len() as i64 + scale;
    // Above 10^4933 or below 10^-4960 whatever the digits are: infinity, zero.
    if magnitude > 4933 + 1 {
        return Some(sign | (0x7FFF << 64) | (1 << 63));
    }
    if magnitude < -4960 {
        return Some(sign);
    }
    let mut numerator = Big::zero();
    for chunk in digits.as_bytes().chunks(9) {
        let value = chunk.iter().fold(0u32, |all, digit| all * 10 + u32::from(digit - b'0'));
        numerator.mul_add(10u32.pow(chunk.len() as u32), value);
    }
    let mut denominator = Big(vec![1]);
    let scaled = if scale > 0 { &mut numerator } else { &mut denominator };
    for _ in 0..scale.unsigned_abs() {
        scaled.mul_add(10, 0);
    }
    // The leading bit of the value is 2^e, with e one of these two.
    let mut e = numerator.bits() - denominator.bits() - 1;
    let (m, e) = loop {
        let quantum = (e - 63).max(DENORMAL_QUANTUM);
        let (num, den) = if quantum < 0 {
            (numerator.shl(quantum.unsigned_abs() as u32), denominator.clone())
        } else {
            (numerator.clone(), denominator.shl(quantum as u32))
        };
        let (floor, rest) = num.div_rem(&den, 70);
        if floor >> 64 != 0 {
            e += 1;
            continue;
        }
        let m = match rest.shl(1).cmp(&den) {
            Ordering::Greater => floor + 1,
            Ordering::Equal => floor + (floor & 1),
            Ordering::Less => floor,
        };
        break if m >> 64 != 0 { (m >> 1, e + 1) } else { (m, e) };
    };
    if e > LARGEST_EXPONENT {
        return Some(sign | (0x7FFF << 64) | (1 << 63));
    }
    // A denormal that rounded up to the smallest normal has the integer bit
    // set, and exponent 1.
    let biased = if m >> 63 == 0 { 0 } else { (e.max(SMALLEST_EXPONENT) + BIAS) as u128 };
    Some(sign | (biased << 64) | m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decimals_clang_prints_for_extended_constants_round_at_64_bits() {
        // 0.1L, 1.1L and 2.5L as the pin's clang encodes them; rounding to a
        // double first left the low 11 mantissa bits zero.
        for (text, bits) in [
            ("1.100000e+00", 0x3FFF_8CCC_CCCC_CCCC_CCCDu128),
            ("1.000000e-01", 0x3FFB_CCCC_CCCC_CCCC_CCCD),
            ("2.500000e+00", 0x4000_A000_0000_0000_0000),
            ("0.01", 0x3FF8_A3D7_0A3D_70A3_D70A),
            ("3.14159265358979323846", 0x4000_C90F_DAA2_2168_C235),
            ("-1.5", 0xBFFF_C000_0000_0000_0000),
        ] {
            assert_eq!(x87_bits(text), Some(bits), "{text}: {:X?}", x87_bits(text));
        }
    }

    #[test]
    fn test_ties_round_to_even_and_the_ends_of_the_range_saturate() {
        // 2^64 + 1 is halfway between 2^64 and 2^64 + 2; 2^64 + 3 between 2^64
        // + 2 and 2^64 + 4.
        assert_eq!(x87_bits("18446744073709551617"), Some(0x403F_8000_0000_0000_0000));
        assert_eq!(x87_bits("18446744073709551619"), Some(0x403F_8000_0000_0000_0002));
        assert_eq!(x87_bits("0.0"), Some(0));
        assert_eq!(x87_bits("-0.0"), Some(1 << 79));
        assert_eq!(x87_bits("1e5000"), Some(0x7FFF_8000_0000_0000_0000));
        assert_eq!(x87_bits("1e-5000"), Some(0));
        assert_eq!(x87_bits("1e4932").map(|bits| bits >> 64), Some(0x7FFE));
        assert_eq!(x87_bits("not a number"), None);
    }

    #[test]
    fn test_the_exact_expansion_of_a_double_is_the_double_widened() {
        for value in [0.5f64, 1.0 / 3.0, 5e-324, f64::MAX, 123456.789, 1e-310, 2.2250738585072014e-308] {
            let text = format!("{value:.800e}");
            let wide = x87_bits(&text).unwrap();
            let mut one = crate::apfloat::ApFloat::from_bits(crate::apfloat::IEEE_DOUBLE, u128::from(value.to_bits()));
            one.convert(crate::apfloat::X87_DOUBLE_EXTENDED, crate::fpclass::RoundingMode::NearestTiesToEven);
            assert_eq!(wide, one.to_bits(), "{value:e}");
        }
    }

    #[test]
    fn test_a_denormal_is_rounded_at_its_own_quantum() {
        // The smallest denormal 2^-16445 and the smallest normal 2^-16382, from
        // their decimal expansions.
        assert_eq!(x87_bits("3.6451995318824746e-4951"), Some(1));
        assert_eq!(x87_bits("3.3621031431120935063e-4932"), Some(0x0001_8000_0000_0000_0000));
    }
}
