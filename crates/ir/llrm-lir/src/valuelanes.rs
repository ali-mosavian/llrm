//! The lanes of a value an operand names: bit `n` is byte `n` (LLVM's
//! `LaneBitmask` at byte granularity, which is what x86's sub-registers split
//! on: `sub_8bit` is byte 0, `sub_8bit_hi` byte 1, `sub_16bit` bytes 0-1).

/// A set of the bytes of one value.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct ValueLanes(pub u16);

impl ValueLanes {
    pub const NONE: Self = Self(0);

    /// Bytes `0..width`: the lanes an access of `width` bytes at the bottom of
    /// a value names. A width past 16 bytes names all 16.
    pub const fn low(width: u32) -> Self {
        if width >= 16 { Self(u16::MAX) } else { Self((1u16 << width) - 1) }
    }

    /// `width` bytes from byte `from`: the high byte `AH` is `at(1, 1)`.
    pub const fn at(
        from: u32,
        width: u32,
    ) -> Self {
        Self(Self::low(from + width).0 & !Self::low(from).0)
    }

    pub const fn union(
        self,
        other: Self,
    ) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn intersection(
        self,
        other: Self,
    ) -> Self {
        Self(self.0 & other.0)
    }

    pub const fn overlaps(
        self,
        other: Self,
    ) -> bool {
        self.0 & other.0 != 0
    }

    pub const fn covers(
        self,
        other: Self,
    ) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[cfg(test)]
mod tests {
    use super::ValueLanes;

    #[test]
    fn test_an_access_names_the_low_bytes_of_its_width() {
        assert_eq!(ValueLanes::low(1), ValueLanes(0b1));
        assert_eq!(ValueLanes::low(2), ValueLanes(0b11));
        assert_eq!(ValueLanes::low(4), ValueLanes(0b1111));
        assert_eq!(ValueLanes::low(0), ValueLanes::NONE);
        assert_eq!(ValueLanes::low(40), ValueLanes(u16::MAX));
    }

    /// `sub_8bit_hi` (AH) shares no byte with `sub_8bit` (AL) but both are
    /// inside `sub_16bit`.
    #[test]
    fn test_the_high_byte_is_byte_one_and_overlaps_only_the_wider_lanes() {
        let (al, ah, ax) = (ValueLanes::low(1), ValueLanes::at(1, 1), ValueLanes::low(2));
        assert_eq!(ah, ValueLanes(0b10));
        assert!(!al.overlaps(ah));
        assert!(ax.covers(al) && ax.covers(ah) && ax.covers(al.union(ah)));
        assert!(!al.covers(ax));
        assert_eq!(ax.intersection(ah), ah);
    }
}
