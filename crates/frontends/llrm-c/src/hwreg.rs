//! Open Watcom's register sets as the front end records them (`3c:0`: two
//! hex words, `cgx86reg.h`'s bits), as register names in the order a value
//! travels: the high half first, as `[__dx __ax]` writes it.

const BITS: [(u32, &str); 21] = [
    (0x03, "ax"),
    (0x0c, "bx"),
    (0x30, "cx"),
    (0xc0, "dx"),
    (0x100, "si"),
    (0x200, "di"),
    (0x400, "bp"),
    (0x800, "sp"),
    (0x1000, "ds"),
    (0x2000, "es"),
    (0x4000, "cs"),
    (0x8000, "ss"),
    (0x01, "ah"),
    (0x02, "al"),
    (0x04, "bh"),
    (0x08, "bl"),
    (0x10, "ch"),
    (0x20, "cl"),
    (0x40, "dh"),
    (0x80, "dl"),
    (0x1_0000, "st0"),
];

/// A register set: the front end's two words.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Set {
    pub low: u32,
    pub high: u32,
}

impl Set {
    /// `3c:0`; `-` and an empty text are the empty set.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() || text == "-" {
            return Some(Self::default());
        }
        let (low, high) = text.split_once(':')?;
        Some(Self { low: u32::from_str_radix(low, 16).ok()?, high: u32::from_str_radix(high, 16).ok()? })
    }

    pub fn is_empty(self) -> bool {
        self == Self::default()
    }

    pub fn has(
        self,
        other: Self,
    ) -> bool {
        self.low & other.low == other.low && self.high & other.high == other.high
    }

    pub fn without(
        self,
        other: Self,
    ) -> Self {
        Self { low: self.low & !other.low, high: self.high & !other.high }
    }

    pub fn with(
        self,
        other: Self,
    ) -> Self {
        Self { low: self.low | other.low, high: self.high | other.high }
    }

    /// The set widened to the whole registers it touches (`al` to `ax`).
    pub fn whole(self) -> Self {
        let mut low = self.low;
        for word in [0x03, 0x0c, 0x30, 0xc0] {
            if low & word != 0 {
                low |= word;
            }
        }
        Self { low, high: self.high }
    }

    /// The names in the set, whole registers where both halves are in, the
    /// high half first: a segment register above an offset, an `si` or `di`
    /// below the register it pairs with, else the later of `a`, `b`, `c`, `d`
    /// above the earlier. None where a bit has no name here (FS, GS, the
    /// 32-bit halves).
    pub fn names(self) -> Option<Vec<&'static str>> {
        if self.high != 0 || self.low & 0xffff_0000 & !0x1_0000 != 0 {
            return None;
        }
        let mut found: Vec<&'static str> = Vec::new();
        let mut left = self.low;
        for (bits, name) in BITS.iter().filter(|(bits, _)| bits.count_ones() > 1 || *bits >= 0x100) {
            if left & bits == *bits {
                found.push(name);
                left &= !bits;
            }
        }
        for (bits, name) in BITS.iter().filter(|(bits, _)| bits.count_ones() == 1 && *bits < 0x100) {
            if left & bits != 0 {
                found.push(name);
                left &= !bits;
            }
        }
        let rank = |name: &str| match name {
            "ds" | "es" | "cs" | "ss" => 8,
            "dx" | "dh" | "dl" => 4,
            "cx" | "ch" | "cl" => 3,
            "bx" | "bh" | "bl" | "bp" => 2,
            "ax" | "ah" | "al" => 1,
            _ => 0,
        };
        found.sort_by_key(|name| std::cmp::Reverse(rank(name)));
        Some(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(text: &str) -> Vec<&'static str> {
        Set::parse(text).unwrap().names().unwrap()
    }

    /// Open Watcom's `[__dx __ax]` is a long: the front end writes `c3:0` and
    /// the high half is dx. Read as a set of names it came out in bit order.
    #[test]
    fn test_a_pair_is_named_high_half_first() {
        assert_eq!(names("c3:0"), ["dx", "ax"]);
        assert_eq!(names("3c:0"), ["cx", "bx"]);
        assert_eq!(names("1100:0"), ["ds", "si"]);
        assert_eq!(names("2200:0"), ["es", "di"]);
        assert_eq!(names("c:0"), ["bx"]);
        assert_eq!(names("2:0"), ["al"]);
        assert_eq!(names("10103:0"), ["ax", "si", "st0"]);
    }

    #[test]
    fn test_a_set_with_a_register_this_target_lacks_has_no_names() {
        assert!(Set::parse("0:1").unwrap().names().is_none());
        assert!(Set::parse("1000000:0").unwrap().names().is_none());
    }

    #[test]
    fn test_the_empty_set_and_set_arithmetic() {
        assert!(Set::parse("-").unwrap().is_empty());
        let all = Set::parse("ffffdfff:ffffffff").unwrap();
        let kept = Set::parse("2300:0").unwrap();
        assert_eq!(all.without(kept).low, 0xffff_dcff);
        assert!(Set::parse("c3:0").unwrap().whole().has(Set::parse("c0:0").unwrap()));
    }
}
