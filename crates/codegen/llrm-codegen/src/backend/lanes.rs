//! A set of register and flag lanes, as a 128-bit mask.
//!
//! # How it works
//! A lane is a byte of a general register root (4 each, 7 roots of a 32-bit
//! target), a byte of a segment register (2 each, 6 segments), or a flag bit
//! against `RegId::None` (32). Long mode adds 8 roots and a root's upper four
//! bytes, which no view splits, are one lane: 5 lanes for each of 15 roots, 119
//! lanes in all, which fit the mask a 32-bit target has always had. The table
//! sorts the lanes in `(RegId, u32)` order, so flag bit `i` is mask bit `i`,
//! and iteration visits lanes in the order a `BTreeSet<Lane>` would. Each
//! register records the mask bit of its byte 0 and its byte count, so a lane's
//! bit is a table lookup.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','textColor':'#64748b'}}}%%
//! block-beta
//!   columns 8
//!   o1["bit 32.."] c1["bytes, one run per register"]:3 s1[" "] n1["in RegId order"]:3
//!   o2["bit 0..31"] c2["flags (RegId::None, 0..31)"]:3 s2[" "] n2[" "]:3
//!   classDef off fill:transparent,stroke:transparent,stroke-width:2px,color:#64748b
//!   classDef cell fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff
//!   class o1,n1,s1,o2,n2,s2 off
//!   class c1,c2 cell
//! ```
//!
//! *Lane mask bits*
//!
//! Union, difference, intersection, subset and disjointness are single word
//! operations. Inserting a lane that is none of the 72 panics.
//!
//! # Example
//! No example: a bit set type. `liveness.rs` shows what it holds.
//!
//! # Used by
//! `liveness`, `peephole` and its sub-passes (`copysink`, `postrasink`,
//! `regthrash`, `exactaddress`, `sharedstores`), `loopslots`,
//! `schedule`, `parcopy`, and `peep/guards`, `peep/walk`, `peep/make`.

use std::fmt;
use std::sync::LazyLock;

use llrm_lir::registers::RegId;

/// One byte of a register root, or one flag bit against `RegId::None`.
pub type Lane = (RegId, u32);

const REGISTERS: usize = 256;

struct Table {
    // Every lane, in bit order.
    lanes: Vec<Lane>,
    // Per register, the bit of its byte 0, how many lanes it has and how many
    // bytes (a root's upper four share one lane).
    first: [u8; REGISTERS],
    count: [u8; REGISTERS],
    bytes: [u8; REGISTERS],
}

/// The bytes a root has: the lanes below `UPPER` are its own, and
/// `UPPER..BYTES` are one.
const BYTES: u32 = 8;
const UPPER: u32 = 4;

static TABLE: LazyLock<Table> = LazyLock::new(|| {
    let mut lanes: Vec<Lane> = (0..32).map(|bit| (RegId::None, bit)).collect();
    for root in llrm_x86::registers::ROOTS.into_iter().chain(llrm_x86::registers::ROOTS64) {
        lanes.extend((0..=UPPER).map(|byte| (root, byte)));
    }
    for segment in llrm_x86::registers::SEGMENTS {
        lanes.extend((0..2).map(|byte| (segment, byte)));
    }
    lanes.sort();
    assert!(lanes.len() <= 128 && (0..32).all(|bit| lanes[bit as usize] == (RegId::None, bit)));
    let (mut first, mut count, mut bytes) = ([0_u8; REGISTERS], [0_u8; REGISTERS], [0_u8; REGISTERS]);
    for (bit, (register, _)) in lanes.iter().enumerate() {
        let slot = (*register).index();
        if count[slot] == 0 {
            first[slot] = u8::try_from(bit).expect("fits");
        }
        count[slot] += 1;
    }
    for (slot, lanes) in count.iter().enumerate() {
        bytes[slot] = if *lanes == 0 {
            0
        } else if u32::from(*lanes) == UPPER + 1 {
            BYTES as u8
        } else {
            *lanes
        };
    }
    Table { lanes, first, count, bytes }
});

fn bit(lane: &Lane) -> Option<u32> {
    let table = &*TABLE;
    let slot = lane.0.index();
    (slot < REGISTERS && lane.1 < u32::from(table.bytes[slot]))
        .then(|| u32::from(table.first[slot]) + lane.1.min(u32::from(table.count[slot]) - 1))
}

#[derive(Clone, Copy, Default, Eq, Hash, PartialEq)]
pub struct Lanes(u128);

impl Lanes {
    pub const fn new() -> Self {
        Self(0)
    }

    /// The lanes as a bit mask, a lane to a bit (`Lanes::flags` for the flags).
    pub const fn mask(&self) -> u128 {
        self.0
    }

    /// How many lanes there are: the bits of a mask in use.
    pub fn count() -> usize {
        TABLE.lanes.len()
    }

    /// The lanes of a bit mask, as `mask` gives it.
    pub const fn from_mask(mask: u128) -> Self {
        Self(mask)
    }

    /// The flag lanes of an rflags mask. `RegId::None` sorts first, so flag
    /// bit `i` is lane bit `i`.
    pub fn flags(mask: u32) -> Self {
        Self(u128::from(mask))
    }

    /// The flags emitted code reads (`Flag::READ`).
    pub fn read_flags() -> Self {
        Self::flags(llrm_lir::flag::Flag::READ.bits())
    }

    /// Bytes `start..start + len` of a register: the lanes `(register, byte)`
    /// for each, without a table lookup per byte.
    pub fn span(
        register: RegId,
        start: u32,
        len: u32,
    ) -> Self {
        if len == 0 {
            return Self(0);
        }
        let table = &*TABLE;
        let slot = register.index();
        let last = start + len - 1;
        assert!(
            slot < REGISTERS && last < u32::from(table.bytes[slot]),
            "{register:?} bytes {start}..={last} are no lanes"
        );
        let (first, top) = (u32::from(table.first[slot]), u32::from(table.count[slot]) - 1);
        let (low, high) = (first + start.min(top), first + last.min(top));
        Self((u128::MAX >> (127 - (high - low))) << low)
    }

    pub fn insert(
        &mut self,
        lane: Lane,
    ) -> bool {
        let mask = 1 << bit(&lane).unwrap_or_else(|| panic!("{lane:?} is no lane"));
        let fresh = self.0 & mask == 0;
        self.0 |= mask;
        fresh
    }

    pub fn remove(
        &mut self,
        lane: &Lane,
    ) -> bool {
        let Some(bit) = bit(lane) else { return false };
        let present = self.0 & (1 << bit) != 0;
        self.0 &= !(1 << bit);
        present
    }

    pub fn contains(
        &self,
        lane: &Lane,
    ) -> bool {
        bit(lane).is_some_and(|bit| self.0 & (1 << bit) != 0)
    }

    pub fn clear(&mut self) {
        self.0 = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    pub fn is_subset(
        &self,
        other: &Self,
    ) -> bool {
        self.0 & !other.0 == 0
    }

    pub fn is_disjoint(
        &self,
        other: &Self,
    ) -> bool {
        self.0 & other.0 == 0
    }

    pub fn iter(&self) -> Iter {
        Iter(self.0)
    }

    // The set operations as `BTreeSet` spells them, yielding lanes in order.
    pub fn union(
        &self,
        other: &Self,
    ) -> Iter {
        Iter(self.0 | other.0)
    }

    pub fn difference(
        &self,
        other: &Self,
    ) -> Iter {
        Iter(self.0 & !other.0)
    }

    pub fn intersection(
        &self,
        other: &Self,
    ) -> Iter {
        Iter(self.0 & other.0)
    }

    // The same operations as sets, without an iterator between.
    pub fn or(
        &self,
        other: &Self,
    ) -> Self {
        Self(self.0 | other.0)
    }

    pub fn minus(
        &self,
        other: &Self,
    ) -> Self {
        Self(self.0 & !other.0)
    }

    pub fn and(
        &self,
        other: &Self,
    ) -> Self {
        Self(self.0 & other.0)
    }

    pub fn retain(
        &mut self,
        mut keep: impl FnMut(&Lane) -> bool,
    ) {
        for lane in self.iter() {
            if !keep(lane) {
                self.remove(lane);
            }
        }
    }
}

/// The lanes of a mask, lowest bit first.
#[derive(Clone)]
pub struct Iter(u128);

impl Iterator for Iter {
    type Item = &'static Lane;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0 == 0 {
            return None;
        }
        let bit = self.0.trailing_zeros();
        self.0 &= self.0 - 1;
        Some(&TABLE.lanes[bit as usize])
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.0.count_ones() as usize;
        (n, Some(n))
    }
}

impl DoubleEndedIterator for Iter {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.0 == 0 {
            return None;
        }
        let bit = 127 - self.0.leading_zeros();
        self.0 &= !(1 << bit);
        Some(&TABLE.lanes[bit as usize])
    }
}

impl ExactSizeIterator for Iter {}

impl IntoIterator for Lanes {
    type Item = Lane;
    type IntoIter = std::iter::Copied<Iter>;

    fn into_iter(self) -> Self::IntoIter {
        Iter(self.0).copied()
    }
}

impl<'a> IntoIterator for &'a Lanes {
    type Item = &'static Lane;
    type IntoIter = Iter;

    fn into_iter(self) -> Iter {
        Iter(self.0)
    }
}

impl FromIterator<Lane> for Lanes {
    fn from_iter<I: IntoIterator<Item = Lane>>(lanes: I) -> Self {
        let mut out = Self::new();
        out.extend(lanes);
        out
    }
}

impl<'a> FromIterator<&'a Lane> for Lanes {
    fn from_iter<I: IntoIterator<Item = &'a Lane>>(lanes: I) -> Self {
        lanes.into_iter().copied().collect()
    }
}

impl Extend<Lane> for Lanes {
    fn extend<I: IntoIterator<Item = Lane>>(
        &mut self,
        lanes: I,
    ) {
        for lane in lanes {
            self.insert(lane);
        }
    }
}

impl<'a> Extend<&'a Lane> for Lanes {
    fn extend<I: IntoIterator<Item = &'a Lane>>(
        &mut self,
        lanes: I,
    ) {
        self.extend(lanes.into_iter().copied());
    }
}

impl<const N: usize> From<[Lane; N]> for Lanes {
    fn from(lanes: [Lane; N]) -> Self {
        lanes.into_iter().collect()
    }
}

impl PartialOrd for Lanes {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// `BTreeSet`'s order: lexicographic over the sorted lanes.
impl Ord for Lanes {
    fn cmp(
        &self,
        other: &Self,
    ) -> std::cmp::Ordering {
        self.iter().cmp(other.iter())
    }
}

impl fmt::Debug for Lanes {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// A root's upper four bytes are one lane (long mode's quadword), so a
    /// quadword's lanes fit the mask: writing `rax` writes the same set as
    /// its dword and the upper four bytes together.
    #[test]
    fn a_roots_upper_four_bytes_are_one_lane() {
        let upper = Lanes::from([(RegId::EAX, 4)]);
        assert_eq!(Lanes::from([(RegId::EAX, 7), (RegId::EAX, 5)]), upper);
        assert!(!Lanes::from([(RegId::EAX, 3)]).contains(&(RegId::EAX, 4)));
        assert!(std::panic::catch_unwind(|| Lanes::from([(RegId::EAX, 8)])).is_err(), "a ninth byte is no lane");
        assert!(TABLE.lanes.len() <= 128);
    }

    #[test]
    fn test_lanes_iterate_and_compare_as_a_btreeset_does() {
        assert_eq!(Lanes::flags(0b101), Lanes::from([(RegId::None, 0), (RegId::None, 2)]));
        let every: Vec<Lane> = TABLE.lanes.clone();
        let picks: Vec<Vec<Lane>> = vec![
            vec![],
            every.clone(),
            every.iter().copied().step_by(3).collect(),
            every.iter().copied().rev().step_by(5).collect(),
            vec![(RegId::GS, 1), (RegId::None, 0), (RegId::EAX, 3)],
        ];
        for one in &picks {
            let (set, tree): (Lanes, BTreeSet<Lane>) = (one.iter().collect(), one.iter().copied().collect());
            assert_eq!(set.iter().copied().collect::<Vec<_>>(), tree.iter().copied().collect::<Vec<_>>());
            assert_eq!(set.iter().rev().copied().collect::<Vec<_>>(), tree.iter().rev().copied().collect::<Vec<_>>());
            assert_eq!(format!("{set:?}"), format!("{tree:?}"));
            for other in &picks {
                let (theirs, their_tree): (Lanes, BTreeSet<Lane>) =
                    (other.iter().collect(), other.iter().copied().collect());
                assert_eq!(set.cmp(&theirs), tree.cmp(&their_tree));
                assert_eq!(set.is_subset(&theirs), tree.is_subset(&their_tree));
                assert_eq!(
                    set.difference(&theirs).copied().collect::<Vec<_>>(),
                    tree.difference(&their_tree).copied().collect::<Vec<_>>()
                );
            }
        }
    }

    /// `span` must name the lanes a byte-by-byte `collect` names, including a
    /// root's upper bytes folded into one lane.
    #[test]
    fn span_matches_the_lanes_of_each_byte() {
        let registers: BTreeSet<RegId> = TABLE.lanes.iter().map(|lane| lane.0).collect();
        for register in registers.into_iter().filter(|register| *register != RegId::None) {
            let bytes = u32::from(TABLE.bytes[register.index()]);
            for start in 0..bytes {
                for len in 0..=bytes - start {
                    let each: Lanes = (start..start + len).map(|byte| (register, byte)).collect();
                    assert_eq!(Lanes::span(register, start, len), each, "{register:?} {start}+{len}");
                }
            }
        }
    }
}
