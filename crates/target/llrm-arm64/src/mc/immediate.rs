//! Immediates the A64 encodings hold in a coded form, translated from LLVM's
//! `AArch64AddressingModes.h`
//! (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/MCTargetDesc/AArch64AddressingModes.h>).

/// `N:immr:imms` of a logical-immediate instruction that holds `value` in a
/// register of `bits` (32 or 64) bits: `processLogicalImmediate`. None where
/// the value is not a rotated run of ones repeated at a power-of-two size, or
/// is all zeros or all ones.
pub fn logical(
    value: u64,
    bits: u32,
) -> Option<u32> {
    if value == 0 || value == u64::MAX || (bits != 64 && (value >> bits != 0 || value == u64::MAX >> (64 - bits))) {
        return None;
    }
    // The element size: the smallest power of two the value repeats at.
    let mut size = bits;
    loop {
        size /= 2;
        let mask = (1u64 << size) - 1;
        if value & mask != (value >> size) & mask {
            size *= 2;
            break;
        }
        if size <= 2 {
            break;
        }
    }
    // The rotation that makes the element 0^m 1^n.
    let mask = u64::MAX >> (64 - size);
    let mut value = value & mask;
    let shifted_mask = |one: u64| {
        one != 0 && {
            let filled = one | (one - 1);
            filled & filled.wrapping_add(1) == 0
        }
    };
    let (rotation, ones) = if shifted_mask(value) {
        let low = value.trailing_zeros();
        (low, (value >> low).trailing_ones())
    } else {
        value |= !mask;
        if !shifted_mask(!value) {
            return None;
        }
        let leading = value.leading_ones();
        (64 - leading, leading + value.trailing_ones() - (64 - size))
    };
    let immr = (size - rotation) & (size - 1);
    let mut size_and_ones = (!(u64::from(size) - 1)) << 1;
    size_and_ones |= u64::from(ones) - 1;
    let n = (((size_and_ones >> 6) & 1) ^ 1) as u32;
    Some(n << 12 | immr << 6 | (size_and_ones & 0x3F) as u32)
}

/// The value `N:immr:imms` holds in a register of `bits` bits:
/// `decodeLogicalImmediate`. The inverse of [`logical`].
pub fn decode_logical(
    code: u32,
    bits: u32,
) -> u64 {
    let (n, immr, imms) = (code >> 12 & 1, code >> 6 & 0x3F, code & 0x3F);
    let len = 31 - (n << 6 | !imms & 0x3F).leading_zeros();
    let size = 1u32 << len;
    let (ones, rotation) = ((imms & (size - 1)) + 1, immr & (size - 1));
    let mask = if size == 64 { u64::MAX } else { (1u64 << size) - 1 };
    let run = if ones == 64 { u64::MAX } else { (1u64 << ones) - 1 };
    let element = if rotation == 0 { run } else { (run >> rotation | run << (size - rotation)) & mask };
    let mut value = element;
    let mut at = size;
    while at < 64 {
        value |= value << at;
        at *= 2;
    }
    if bits == 64 { value } else { value & 0xFFFF_FFFF }
}

/// The 8-bit code of a floating-point immediate (`getFP64Imm`, and `getFP32Imm`
/// for the single the caller widened): the sign, three bits of exponent and
/// four of mantissa. None where the double is not one of those 256 values.
pub fn fp_immediate(value: f64) -> Option<u8> {
    let bits = value.to_bits();
    let sign = (bits >> 63) & 1;
    let exponent = ((bits >> 52) & 0x7FF) as i64 - 1023;
    let mantissa = bits & 0xF_FFFF_FFFF_FFFF;
    if mantissa & 0xFFFF_FFFF_FFFF != 0 {
        return None;
    }
    let mantissa = mantissa >> 48;
    if mantissa & 0xF != mantissa || !(-3..=4).contains(&exponent) {
        return None;
    }
    let exponent = (((exponent + 3) & 0x7) ^ 4) as u64;
    Some((sign << 7 | exponent << 4 | mantissa) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Values the pinned `llvm-mc` encodes: `and x0, x0, #0xff` is
    /// `N=1 immr=0 imms=7`, a repeated byte pattern uses the 8-bit element.
    #[test]
    fn logical_immediates_have_llvms_codes() {
        assert_eq!(logical(0xFF, 64), Some(0x1007));
        assert_eq!(logical(0x5555_5555_5555_5555, 64), Some(0x3C));
        assert_eq!(logical(0, 64), None);
        assert_eq!(logical(u64::MAX, 64), None);
        assert_eq!(logical(5, 64), None);
        assert_eq!(logical(0x1_0000_0000, 32), None);
    }

    /// A decoder that disagreed with the encoder would run `and w0, w0, #0xff`
    /// as some other mask.
    #[test]
    fn logical_immediates_decode_to_what_they_encode() {
        for bits in [32, 64] {
            for value in [0xFFu64, 0x5555_5555, 0x8000_0001, 0xFFFF_0000, 0x0F0F_0F0F, 0x7FFF_FFFE, 0x1, 0x8000_0000] {
                // 0x5555_5555 is a pattern at 32 bits only.
                let Some(code) = logical(value, bits) else { continue };
                assert_eq!(decode_logical(code, bits), value, "{value:#x} at {bits}");
            }
        }
        assert_eq!(decode_logical(logical(0x0000_00FF_FF00_0000, 64).unwrap(), 64), 0x0000_00FF_FF00_0000);
    }

    #[test]
    fn fp_immediates_are_the_256_values() {
        assert_eq!(fp_immediate(1.0), Some(0x70));
        assert_eq!(fp_immediate(-2.0), Some(0x80));
        assert_eq!(fp_immediate(0.1), None);
        assert_eq!(fp_immediate(0.0), None);
    }
}
