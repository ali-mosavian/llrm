//! Integers of a given width held in an `i64`: the masks and wraps that stay defined at eight bytes, where
//! `1 << 64` is not.

/// The low `width` bytes of an `i64` as a mask: all of it from eight bytes.
pub fn low_mask(width: u32) -> i64 {
    if width >= 8 { -1 } else { (1i64 << (width * 8)) - 1 }
}

/// The least signed integer of `bits` bits.
pub fn signed_min(bits: i64) -> i64 {
    if bits >= 64 { i64::MIN } else { -(1 << (bits - 1)) }
}

/// `value` as a signed integer of `bits` bits, wrapped.
pub fn wrap_to(
    value: i64,
    bits: i64,
) -> i64 {
    if bits >= 64 { value } else { (value + (1 << (bits - 1))).rem_euclid(1 << bits) - (1 << (bits - 1)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `wrapped` once shifted 1 by 64 in `stepflags` and `peephole` (a compare of an i64 against a constant
    /// panicked every x86-64 compile of a loop over a `long`).
    #[test]
    fn eight_bytes_are_the_whole_integer() {
        assert_eq!((low_mask(4), low_mask(8)), (0xFFFF_FFFF, -1));
        assert_eq!((signed_min(32), signed_min(64)), (-(1 << 31), i64::MIN));
        assert_eq!((wrap_to(0x1_0000_0001, 32), wrap_to(-5, 64), wrap_to(200, 8)), (1, -5, -56));
    }
}
