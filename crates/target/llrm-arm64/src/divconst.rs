//! The magic numbers that turn a division by a constant into a multiply, from
//! LLVM's `DivisionByConstantInfo.cpp`
//! (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/DivisionByConstantInfo.cpp>),
//! "Hacker's Delight", chapter 10. Every quantity is a `bits`-bit value that
//! wraps, held in a `u128` and masked after each operation.

/// `UnsignedDivisionByConstantInfo`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Unsigned {
    pub magic: u128,
    /// The product needs the dividend added back (`NPQ`).
    pub is_add: bool,
    /// A 32-bit division done as one 64-bit high multiply: `magic` is
    /// pre-shifted so the high half is the quotient.
    pub widen: bool,
    pub pre_shift: u32,
    pub post_shift: u32,
}

/// `SignedDivisionByConstantInfo`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Signed {
    /// As a `bits`-bit pattern.
    pub magic: u128,
    pub shift: u32,
}

fn mask(bits: u32) -> u128 {
    if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 }
}

fn is_negative(
    value: u128,
    bits: u32,
) -> bool {
    value >> (bits - 1) & 1 == 1
}

/// `SignedDivisionByConstantInfo::get(D)`; `d` is not 0, and `bits` at least 3.
pub fn signed(
    d: u128,
    bits: u32,
) -> Signed {
    assert!(d & mask(bits) != 0 && bits >= 3);
    let m = mask(bits);
    let d = d & m;
    let negative = is_negative(d, bits);
    let signed_min = 1u128 << (bits - 1);
    let ad = if negative { d.wrapping_neg() & m } else { d };
    let t = (signed_min + u128::from(negative)) & m;
    let anc = t.wrapping_sub(1).wrapping_sub(t % ad) & m;
    let mut p = bits - 1;
    let (mut q1, mut r1) = (signed_min / anc, signed_min % anc);
    let (mut q2, mut r2) = (signed_min / ad, signed_min % ad);
    loop {
        p += 1;
        q1 = (q1 << 1) & m;
        r1 = (r1 << 1) & m;
        if r1 >= anc {
            q1 = (q1 + 1) & m;
            r1 = r1.wrapping_sub(anc) & m;
        }
        q2 = (q2 << 1) & m;
        r2 = (r2 << 1) & m;
        if r2 >= ad {
            q2 = (q2 + 1) & m;
            r2 = r2.wrapping_sub(ad) & m;
        }
        let delta = ad.wrapping_sub(r2) & m;
        if !(q1 < delta || (q1 == delta && r1 == 0)) {
            break;
        }
    }
    let mut magic = (q2 + 1) & m;
    if negative {
        magic = magic.wrapping_neg() & m;
    }
    Signed { magic, shift: p - bits }
}

/// `UnsignedDivisionByConstantInfo::get(D, LeadingZeros, ...)`; `d` is not 0 or
/// 1.
pub fn unsigned(
    d: u128,
    bits: u32,
    leading_zeros: u32,
    allow_even_divisor_optimization: bool,
    allow_widen_optimization: bool,
) -> Unsigned {
    let m = mask(bits);
    let d = d & m;
    assert!(d > 1 && bits > 1);
    let all_ones = mask(bits - leading_zeros);
    let signed_min = 1u128 << (bits - 1);
    let signed_max = signed_min - 1;
    // NC, the largest dividend such that NC urem D == D - 1.
    let rem = (((all_ones + 1) & m).wrapping_sub(d) & m) % d;
    let nc = all_ones.wrapping_sub(rem) & m;
    let mut p = bits - 1;
    let (mut q1, mut r1) = (signed_min / nc, signed_min % nc);
    let (mut q2, mut r2) = (signed_max / d, signed_max % d);
    let mut is_add = false;
    loop {
        p += 1;
        if r1 >= (nc.wrapping_sub(r1) & m) {
            q1 = ((q1 << 1) + 1) & m;
            r1 = ((r1 << 1) & m).wrapping_sub(nc) & m;
        } else {
            q1 = (q1 << 1) & m;
            r1 = (r1 << 1) & m;
        }
        if ((r2 + 1) & m) >= (d.wrapping_sub(r2) & m) {
            if q2 >= signed_max {
                is_add = true;
            }
            q2 = ((q2 << 1) + 1) & m;
            r2 = ((((r2 << 1) & m) + 1) & m).wrapping_sub(d) & m;
        } else {
            if q2 >= signed_min {
                is_add = true;
            }
            q2 = (q2 << 1) & m;
            r2 = (((r2 << 1) & m) + 1) & m;
        }
        let delta = d.wrapping_sub(1).wrapping_sub(r2) & m;
        if !(p < bits * 2 && (q1 < delta || (q1 == delta && r1 == 0))) {
            break;
        }
    }
    if is_add && d & 1 == 0 && allow_even_divisor_optimization {
        let pre_shift = d.trailing_zeros();
        let shifted = d >> pre_shift;
        let mut found = unsigned(shifted, bits, leading_zeros + pre_shift, true, false);
        debug_assert!(!found.is_add && found.pre_shift == 0);
        found.pre_shift = pre_shift;
        return found;
    }
    let mut magic = (q2 + 1) & m;
    let mut post_shift = p - bits;
    if is_add {
        post_shift -= 1;
    }
    let mut widen = false;
    // A 32-bit division by a 33-bit magic number is one 64-bit high multiply
    // with the magic pre-shifted (2^W + magic), so that no fix-up remains.
    if is_add && allow_widen_optimization {
        let original_shift = post_shift + bits + 1;
        magic = ((1u128 << bits) + magic) << (bits * 2 - original_shift);
        is_add = false;
        post_shift = 0;
        widen = true;
    }
    Unsigned { magic, is_add, widen, pre_shift: 0, post_shift }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn divide_unsigned(
        x: u64,
        d: u64,
        bits: u32,
    ) -> u64 {
        let m = unsigned(u128::from(d), bits, 0, bits != 32, bits == 32);
        let mask = mask(bits) as u64;
        let mul_high = |a: u128, b: u128| -> u128 { (a * b) >> bits };
        if m.widen {
            // The high 64 bits of x * magic, as a 64-bit high multiply.
            return ((u128::from(x) * m.magic) >> 64) as u64 & mask;
        }
        let mut q = u128::from(x);
        q >>= m.pre_shift;
        q = mul_high(q, m.magic);
        if m.is_add {
            let npq = (u128::from(x) - q) >> 1;
            q = npq + q;
        }
        q >>= m.post_shift;
        q as u64 & mask
    }

    fn divide_signed(
        x: i64,
        d: i64,
        bits: u32,
    ) -> i64 {
        let m = signed(d as u128 & mask(bits), bits);
        let magic =
            if is_negative(m.magic, bits) { (m.magic as i128 - (1i128 << bits)) as i128 } else { m.magic as i128 };
        let mut q = (i128::from(x) * magic) >> bits;
        if d > 0 && magic < 0 {
            q += i128::from(x);
        } else if d < 0 && magic > 0 {
            q -= i128::from(x);
        }
        q >>= m.shift;
        q += ((q as u128) >> 127) as i128 & 1;
        q as i64
    }

    /// The magic numbers divide as `/` does, at the widths and for the divisors
    /// the selector uses them for.
    #[test]
    fn unsigned_magic_numbers_divide() {
        for bits in [32u32, 64] {
            for d in [3u64, 5, 6, 7, 9, 10, 11, 12, 13, 25, 100, 125, 641, 1000, 1000003, 0x7fff_ffff, 0xffff_fffe] {
                for x in [0u64, 1, 2, 99, 100, 101, 12345, 0x7fff_ffff, 0x8000_0000, 0xffff_fffe, 0xffff_ffff] {
                    assert_eq!(divide_unsigned(x, d, bits), x / d, "{x} / {d} at {bits}");
                }
            }
        }
        for d in [3u64, 7, 10, 1_000_000_007, 0xffff_ffff_ffff_fff1] {
            for x in [0u64, 1, 12345678901234567, u64::MAX, u64::MAX - 1, 1 << 63] {
                assert_eq!(divide_unsigned(x, d, 64), x / d, "{x} / {d}");
            }
        }
    }

    #[test]
    fn signed_magic_numbers_divide() {
        for bits in [32u32, 64] {
            for d in [3i64, 5, 6, 7, 9, 10, 12, 25, 100, 641, 1000, -3, -7, -10, -1000, 1_000_003] {
                for x in [0i64, 1, -1, 2, -2, 99, -99, 12345, -12345, 0x7fff_ffff, -0x8000_0000] {
                    assert_eq!(divide_signed(x, d, bits), x / d, "{x} / {d} at {bits}");
                }
            }
        }
    }
}
