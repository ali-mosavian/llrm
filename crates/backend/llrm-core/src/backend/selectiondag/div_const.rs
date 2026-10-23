//! Magic numbers for division by a constant: a multiply-high, shifts and
//! adds in place of a divide ("Hacker's Delight", chapter 10).
//!
//! LLVM's `SignedDivisionByConstantInfo::get` and
//! `UnsignedDivisionByConstantInfo::get`
//! ([DivisionByConstantInfo.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/DivisionByConstantInfo.cpp)).
//! `APInt` is a `u128` kept to `bits` bits: every operation wraps there.

/// The multiplier and the shift of `SignedDivisionByConstantInfo`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignedMagic {
    pub magic: u128,
    pub shift: u32,
}

/// `UnsignedDivisionByConstantInfo`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnsignedMagic {
    pub magic: u128,
    /// The quotient needs the "add" fixup (`(x - q) / 2 + q`).
    pub is_add: bool,
    /// `magic` is `2 * bits` wide: multiply a widened `x` instead.
    pub widen: bool,
    pub pre_shift: u32,
    pub post_shift: u32,
}

fn mask(bits: u32) -> u128 {
    if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 }
}

/// `SignedDivisionByConstantInfo::get`: `d` is a nonzero `bits`-bit value
/// (`bits >= 3`), not 1 or -1.
pub fn signed_magic(
    d: u128,
    bits: u32,
) -> SignedMagic {
    let m = mask(bits);
    let signed_min = 1u128 << (bits - 1);
    let negative = d & signed_min != 0;
    let ad = if negative { d.wrapping_neg() & m } else { d };
    let t = (signed_min + (d >> (bits - 1))) & m;
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
    SignedMagic { magic, shift: p - bits }
}

/// `UnsignedDivisionByConstantInfo::get`: `d` is a `bits`-bit value other than
/// 0 and 1. `leading_zeros` are the dividend's known leading zero bits.
pub fn unsigned_magic(
    d: u128,
    bits: u32,
    leading_zeros: u32,
    allow_even_divisor: bool,
    allow_widen: bool,
) -> UnsignedMagic {
    let m = mask(bits);
    let all_ones = mask(bits - leading_zeros);
    let signed_min = 1u128 << (bits - 1);
    let signed_max = signed_min - 1;
    let nc = all_ones.wrapping_sub((((all_ones + 1).wrapping_sub(d)) & m) % d) & m;
    let mut is_add = false;
    let mut p = bits - 1;
    let (mut q1, mut r1) = (signed_min / nc, signed_min % nc);
    let (mut q2, mut r2) = (signed_max / d, signed_max % d);
    loop {
        p += 1;
        if r1 >= nc.wrapping_sub(r1) & m {
            q1 = ((q1 << 1) + 1) & m;
            r1 = ((r1 << 1) & m).wrapping_sub(nc) & m;
        } else {
            q1 = (q1 << 1) & m;
            r1 = (r1 << 1) & m;
        }
        if (r2 + 1) & m >= d.wrapping_sub(r2) & m {
            if q2 >= signed_max {
                is_add = true;
            }
            q2 = ((q2 << 1) + 1) & m;
            r2 = (((r2 << 1) & m) + 1).wrapping_sub(d) & m;
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
    if is_add && d & 1 == 0 && allow_even_divisor {
        let pre_shift = d.trailing_zeros();
        let shifted = unsigned_magic(d >> pre_shift, bits, leading_zeros + pre_shift, true, false);
        debug_assert!(!shifted.is_add && shifted.pre_shift == 0);
        return UnsignedMagic { pre_shift, ..shifted };
    }
    let mut result = UnsignedMagic { magic: (q2 + 1) & m, is_add, widen: false, pre_shift: 0, post_shift: p - bits };
    if is_add {
        result.post_shift -= 1;
    }
    if is_add && allow_widen {
        // The multiplier is `2^bits + magic`, a `bits + 1` bit number, shifted
        // left so that no shift is left to do at run time.
        let original_shift = result.post_shift + bits + 1;
        result.magic = ((1u128 << bits) + result.magic) << (bits * 2 - original_shift);
        result.is_add = false;
        result.post_shift = 0;
        result.widen = true;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signed(
        value: u128,
        bits: u32,
    ) -> i128 {
        ((value << (128 - bits)) as i128) >> (128 - bits)
    }

    /// The sequence `BuildSDIV` emits, evaluated.
    fn sdiv(
        x: u128,
        d: u128,
        bits: u32,
    ) -> u128 {
        let m = mask(bits);
        let magic = signed_magic(d, bits);
        let high = ((signed(x, bits) * signed(magic.magic, bits)) >> bits) as u128 & m;
        let factor: i128 = if signed(d, bits) > 0 && signed(magic.magic, bits) < 0 {
            1
        } else if signed(d, bits) < 0 && signed(magic.magic, bits) > 0 {
            -1
        } else {
            0
        };
        let q = (high as i128 + factor * signed(x, bits)) as u128 & m;
        let q = (signed(q, bits) >> magic.shift) as u128 & m;
        (q + (q >> (bits - 1))) & m
    }

    /// The sequence `BuildUDIV` emits, evaluated.
    fn udiv(
        x: u128,
        d: u128,
        bits: u32,
        leading_zeros: u32,
        allow_even: bool,
        allow_widen: bool,
    ) -> u128 {
        let m = mask(bits);
        let magic = unsigned_magic(d, bits, leading_zeros, allow_even, allow_widen);
        if magic.widen {
            return ((x * magic.magic) >> (bits * 2)) & m;
        }
        let q = (x >> magic.pre_shift) * magic.magic >> bits;
        let q = if magic.is_add { (((x - q) >> 1) + q) & m } else { q };
        q >> magic.post_shift
    }

    /// A multiply-high for every divisor: the quotient of the emitted
    /// sequence is the quotient, at 8 bits for every dividend and divisor.
    #[test]
    fn the_magic_sequences_divide_every_byte() {
        for d in 2u128..256 {
            for x in 0u128..256 {
                for (allow_even, allow_widen) in [(true, false), (false, true)] {
                    assert_eq!(udiv(x, d, 8, 0, allow_even, allow_widen), x / d, "{x} / {d} unsigned");
                }
            }
        }
        for d in (3u128..256).filter(|&d| d != 255) {
            for x in 0u128..256 {
                let want = (signed(x, 8) / signed(d, 8)) as u128 & 0xff;
                assert_eq!(sdiv(x, d, 8), want, "{} / {} signed", signed(x, 8), signed(d, 8));
            }
        }
    }

    /// Known 32-bit pairs: 3 is `0x55555556`, 7 needs the add fixup.
    #[test]
    fn the_32_bit_constants_are_the_known_ones() {
        assert_eq!(signed_magic(3, 32), SignedMagic { magic: 0x5555_5556, shift: 0 });
        assert_eq!(signed_magic(7, 32), SignedMagic { magic: 0x9249_2493, shift: 2 });
        let three = unsigned_magic(3, 32, 0, true, false);
        assert_eq!((three.magic, three.is_add, three.post_shift), (0xAAAA_AAAB, false, 1));
        let seven = unsigned_magic(7, 32, 0, true, false);
        assert_eq!((seven.magic, seven.is_add, seven.post_shift), (0x2492_4925, true, 2));
    }

    /// 16 bits at a spread of dividends, every divisor.
    #[test]
    fn the_magic_sequences_divide_words() {
        for d in 2u128..65536 {
            for x in [0u128, 1, 2, 3, 100, 255, 256, 32767, 32768, 40000, 65534, 65535, 12345, 54321] {
                assert_eq!(udiv(x, d, 16, 0, true, false), x / d, "{x} / {d}");
            }
        }
    }
}
