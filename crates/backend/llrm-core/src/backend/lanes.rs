//! A set of register and flag lanes, as a 256-bit mask.
//!
//! # How it works
//! A lane is a byte of a general register root (4 each, 7 roots of a 32-bit
//! target; 8 each, 15 roots of x86-64), a byte of a segment register (2 each,
//! 6 segments), or a flag bit against `RegId::None` (32): 72 in all for a
//! 32-bit target, 172 for x86-64. The table sorts the lanes in `(RegId, u32)`
//! order, so flag bit `i` is mask bit `i`, and iteration visits lanes in the
//! order a `BTreeSet<Lane>` would. Each register records the mask bit of its
//! byte 0 and its byte count, so a lane's bit is a table lookup.
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
//! `regthrash`, `exactaddress`, `sharedstores`, `machinecse`), `loopslots`,
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
    // Per register, the bit of its byte 0 and how many bytes it has.
    first: [u8; REGISTERS],
    count: [u8; REGISTERS],
}

static TABLE: LazyLock<Table> = LazyLock::new(|| {
    let mut lanes: Vec<Lane> = (0..32).map(|bit| (RegId::None, bit)).collect();
    for root in llrm_x86::registers::ROOTS {
        lanes.extend((0..4).map(|byte| (root, byte)));
    }
    for root in llrm_x86::registers::ROOTS64 {
        lanes.extend((0..8).map(|byte| (root, byte)));
    }
    for segment in llrm_x86::registers::SEGMENTS {
        lanes.extend((0..2).map(|byte| (segment, byte)));
    }
    lanes.sort();
    assert!(lanes.len() <= Wide::BITS as usize && (0..32).all(|bit| lanes[bit as usize] == (RegId::None, bit)));
    let (mut first, mut count) = ([0_u8; REGISTERS], [0_u8; REGISTERS]);
    for (bit, (register, _)) in lanes.iter().enumerate() {
        let slot = (*register).index();
        if count[slot] == 0 {
            first[slot] = u8::try_from(bit).expect("fits");
        }
        count[slot] += 1;
    }
    Table { lanes, first, count }
});

fn bit(lane: &Lane) -> Option<u32> {
    let table = &*TABLE;
    let slot = lane.0.index();
    (slot < REGISTERS && lane.1 < u32::from(table.count[slot])).then(|| u32::from(table.first[slot]) + lane.1)
}

/// A 256-bit mask, low word first: a lane set is a few word operations.
#[derive(Clone, Copy, Default, Eq, Hash, PartialEq)]
struct Wide([u128; 2]);

impl Wide {
    const BITS: u32 = 256;
    const ZERO: Self = Self([0; 2]);

    const fn from_low(low: u128) -> Self {
        Self([low, 0])
    }

    fn bit(at: u32) -> Self {
        let mut words = [0; 2];
        words[(at / 128) as usize] = 1 << (at % 128);
        Self(words)
    }

    fn is_zero(self) -> bool {
        self.0 == [0; 2]
    }

    fn or(
        self,
        other: Self,
    ) -> Self {
        Self([self.0[0] | other.0[0], self.0[1] | other.0[1]])
    }

    fn and(
        self,
        other: Self,
    ) -> Self {
        Self([self.0[0] & other.0[0], self.0[1] & other.0[1]])
    }

    fn minus(
        self,
        other: Self,
    ) -> Self {
        Self([self.0[0] & !other.0[0], self.0[1] & !other.0[1]])
    }

    fn count(self) -> u32 {
        self.0[0].count_ones() + self.0[1].count_ones()
    }

    fn lowest(self) -> Option<u32> {
        match self.0 {
            [0, 0] => None,
            [0, high] => Some(128 + high.trailing_zeros()),
            [low, _] => Some(low.trailing_zeros()),
        }
    }

    fn highest(self) -> Option<u32> {
        match self.0 {
            [0, 0] => None,
            [low, 0] => Some(127 - low.leading_zeros()),
            [_, high] => Some(255 - high.leading_zeros()),
        }
    }
}

#[derive(Clone, Copy, Default, Eq, Hash, PartialEq)]
pub struct Lanes(Wide);

impl Lanes {
    pub const fn new() -> Self {
        Self(Wide::ZERO)
    }

    /// The flag lanes of an rflags mask. `RegId::None` sorts first, so flag
    /// bit `i` is lane bit `i`.
    pub fn flags(mask: u32) -> Self {
        Self(Wide::from_low(u128::from(mask)))
    }

    pub fn insert(
        &mut self,
        lane: Lane,
    ) -> bool {
        let mask = Wide::bit(bit(&lane).unwrap_or_else(|| panic!("{lane:?} is no lane")));
        let fresh = self.0.and(mask).is_zero();
        self.0 = self.0.or(mask);
        fresh
    }

    pub fn remove(
        &mut self,
        lane: &Lane,
    ) -> bool {
        let Some(bit) = bit(lane) else { return false };
        let present = !self.0.and(Wide::bit(bit)).is_zero();
        self.0 = self.0.minus(Wide::bit(bit));
        present
    }

    pub fn contains(
        &self,
        lane: &Lane,
    ) -> bool {
        bit(lane).is_some_and(|bit| !self.0.and(Wide::bit(bit)).is_zero())
    }

    pub fn clear(&mut self) {
        self.0 = Wide::ZERO;
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_zero()
    }

    pub fn is_subset(
        &self,
        other: &Self,
    ) -> bool {
        self.0.minus(other.0).is_zero()
    }

    pub fn is_disjoint(
        &self,
        other: &Self,
    ) -> bool {
        self.0.and(other.0).is_zero()
    }

    pub fn iter(&self) -> Iter {
        Iter(self.0)
    }

    // The set operations as `BTreeSet` spells them, yielding lanes in order.
    pub fn union(
        &self,
        other: &Self,
    ) -> Iter {
        Iter(self.0.or(other.0))
    }

    pub fn difference(
        &self,
        other: &Self,
    ) -> Iter {
        Iter(self.0.minus(other.0))
    }

    pub fn intersection(
        &self,
        other: &Self,
    ) -> Iter {
        Iter(self.0.and(other.0))
    }

    // The same operations as sets, without an iterator between.
    pub fn or(
        &self,
        other: &Self,
    ) -> Self {
        Self(self.0.or(other.0))
    }

    pub fn minus(
        &self,
        other: &Self,
    ) -> Self {
        Self(self.0.minus(other.0))
    }

    pub fn and(
        &self,
        other: &Self,
    ) -> Self {
        Self(self.0.and(other.0))
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
pub struct Iter(Wide);

impl Iterator for Iter {
    type Item = &'static Lane;

    fn next(&mut self) -> Option<Self::Item> {
        let bit = self.0.lowest()?;
        self.0 = self.0.minus(Wide::bit(bit));
        Some(&TABLE.lanes[bit as usize])
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.0.count() as usize;
        (n, Some(n))
    }
}

impl DoubleEndedIterator for Iter {
    fn next_back(&mut self) -> Option<Self::Item> {
        let bit = self.0.highest()?;
        self.0 = self.0.minus(Wide::bit(bit));
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
}
