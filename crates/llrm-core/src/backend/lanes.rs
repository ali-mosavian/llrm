//! Python's `set[tuple[Register_, int]]` of register and flag lanes, as a bitmask.
//!
//! Every lane a body can name is a byte of a general register root, a byte of
//! a segment register, or a flag bit against `Register::None`: 72 in all. Bit
//! `i` is the `i`-th lane in `(Register, u32)` order, so iteration visits lanes
//! in the order a `BTreeSet<Lane>` would.

use std::fmt;
use std::sync::LazyLock;

use iced_x86::Register;

/// One byte of a register root, or one flag bit against `Register::None`.
pub type Lane = (Register, u32);

const REGISTERS: usize = 256;

struct Table {
    // Every lane, in bit order.
    lanes: Vec<Lane>,
    // Per register, the bit of its byte 0 and how many bytes it has.
    first: [u8; REGISTERS],
    count: [u8; REGISTERS],
}

static TABLE: LazyLock<Table> = LazyLock::new(|| {
    let mut lanes: Vec<Lane> = (0..32).map(|bit| (Register::None, bit)).collect();
    for root in [Register::EAX, Register::EBX, Register::ECX, Register::EDX, Register::ESI, Register::EDI, Register::EBP] {
        lanes.extend((0..4).map(|byte| (root, byte)));
    }
    for segment in [Register::ES, Register::CS, Register::SS, Register::DS, Register::FS, Register::GS] {
        lanes.extend((0..2).map(|byte| (segment, byte)));
    }
    lanes.sort();
    assert!(lanes.len() <= 128 && (0..32).all(|bit| lanes[bit as usize] == (Register::None, bit)));
    let (mut first, mut count) = ([0_u8; REGISTERS], [0_u8; REGISTERS]);
    for (bit, (register, _)) in lanes.iter().enumerate() {
        let slot = *register as usize;
        if count[slot] == 0 {
            first[slot] = u8::try_from(bit).expect("fits");
        }
        count[slot] += 1;
    }
    Table { lanes, first, count }
});

fn bit(lane: &Lane) -> Option<u32> {
    let table = &*TABLE;
    let slot = lane.0 as usize;
    (slot < REGISTERS && lane.1 < u32::from(table.count[slot])).then(|| u32::from(table.first[slot]) + lane.1)
}

#[derive(Clone, Copy, Default, Eq, Hash, PartialEq)]
pub struct Lanes(u128);

impl Lanes {
    pub const fn new() -> Self {
        Self(0)
    }

    /// The flag lanes of an rflags mask. `Register::None` sorts first, so flag bit `i` is lane bit `i`.
    pub fn flags(mask: u32) -> Self {
        Self(u128::from(mask))
    }

    pub fn insert(&mut self, lane: Lane) -> bool {
        let mask = 1 << bit(&lane).unwrap_or_else(|| panic!("{lane:?} is no lane"));
        let fresh = self.0 & mask == 0;
        self.0 |= mask;
        fresh
    }

    pub fn remove(&mut self, lane: &Lane) -> bool {
        let Some(bit) = bit(lane) else { return false };
        let present = self.0 & (1 << bit) != 0;
        self.0 &= !(1 << bit);
        present
    }

    pub fn contains(&self, lane: &Lane) -> bool {
        bit(lane).is_some_and(|bit| self.0 & (1 << bit) != 0)
    }

    pub fn clear(&mut self) {
        self.0 = 0;
    }

    pub fn len(&self) -> usize {
        self.0.count_ones() as usize
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    pub fn is_subset(&self, other: &Self) -> bool {
        self.0 & !other.0 == 0
    }

    pub fn is_superset(&self, other: &Self) -> bool {
        other.is_subset(self)
    }

    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.0 & other.0 == 0
    }

    pub fn iter(&self) -> Iter {
        Iter(self.0)
    }

    pub fn first(&self) -> Option<&'static Lane> {
        self.iter().next()
    }

    // The set operations as `BTreeSet` spells them, yielding lanes in order.
    pub fn union(&self, other: &Self) -> Iter {
        Iter(self.0 | other.0)
    }

    pub fn difference(&self, other: &Self) -> Iter {
        Iter(self.0 & !other.0)
    }

    pub fn intersection(&self, other: &Self) -> Iter {
        Iter(self.0 & other.0)
    }

    pub fn symmetric_difference(&self, other: &Self) -> Iter {
        Iter(self.0 ^ other.0)
    }

    // The same operations as sets, without an iterator between.
    pub fn or(&self, other: &Self) -> Self {
        Self(self.0 | other.0)
    }

    pub fn minus(&self, other: &Self) -> Self {
        Self(self.0 & !other.0)
    }

    pub fn and(&self, other: &Self) -> Self {
        Self(self.0 & other.0)
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&Lane) -> bool) {
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
    fn extend<I: IntoIterator<Item = Lane>>(&mut self, lanes: I) {
        for lane in lanes {
            self.insert(lane);
        }
    }
}

impl<'a> Extend<&'a Lane> for Lanes {
    fn extend<I: IntoIterator<Item = &'a Lane>>(&mut self, lanes: I) {
        self.extend(lanes.into_iter().copied());
    }
}

impl<const N: usize> From<[Lane; N]> for Lanes {
    fn from(lanes: [Lane; N]) -> Self {
        lanes.into_iter().collect()
    }
}

impl PartialOrd for Lanes {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// `BTreeSet`'s order: lexicographic over the sorted lanes.
impl Ord for Lanes {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.iter().cmp(other.iter())
    }
}

impl fmt::Debug for Lanes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn test_lanes_iterate_and_compare_as_a_btreeset_does() {
        assert_eq!(Lanes::flags(0b101), Lanes::from([(Register::None, 0), (Register::None, 2)]));
        let every: Vec<Lane> = TABLE.lanes.clone();
        let picks: Vec<Vec<Lane>> = vec![
            vec![],
            every.clone(),
            every.iter().copied().step_by(3).collect(),
            every.iter().copied().rev().step_by(5).collect(),
            vec![(Register::GS, 1), (Register::None, 0), (Register::EAX, 3)],
        ];
        for one in &picks {
            let (set, tree): (Lanes, BTreeSet<Lane>) = (one.iter().collect(), one.iter().copied().collect());
            assert_eq!(set.iter().copied().collect::<Vec<_>>(), tree.iter().copied().collect::<Vec<_>>());
            assert_eq!(set.iter().rev().copied().collect::<Vec<_>>(), tree.iter().rev().copied().collect::<Vec<_>>());
            assert_eq!(format!("{set:?}"), format!("{tree:?}"));
            for other in &picks {
                let (theirs, their_tree): (Lanes, BTreeSet<Lane>) = (other.iter().collect(), other.iter().copied().collect());
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
