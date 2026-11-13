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
//! # Other register files
//! The table above is the x86 one and the default. A file of another target
//! (arm64's 31 roots and 32 vector registers) has its own, built from the
//! file's roots: one lane for each root and every view of it (a write of a
//! `w` register writes the whole `x`, so a finer lane would never be told
//! apart), the 32 flag lanes, and at most 128 in all. [`enter`] makes a file's
//! table the one this thread's `Lanes` read until the guard drops; a mask
//! belongs to the table it was made under.
//!
//! # Example
//! No example: a bit set type. `liveness.rs` shows what it holds.
//!
//! # Used by
//! `liveness`, `peephole` and its sub-passes (`copysink`, `postrasink`,
//! `regthrash`, `exactaddress`, `sharedstores`), `loopslots`,
//! `schedule`, `parcopy`, and `peep/guards`, `peep/walk`, `peep/make`.

use std::cell::Cell;
use std::fmt;
use std::sync::{LazyLock, Mutex};

use llrm_lir::registers::{RegId, Regs};

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

static X86: LazyLock<Table> = LazyLock::new(|| {
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

impl Table {
    /// The table of a file that is not x86's: the 32 flag lanes, then one lane
    /// for each root, which each of its views names.
    fn of_file(regs: Regs) -> Table {
        let mut lanes: Vec<Lane> = (0..32).map(|bit| (RegId::None, bit)).collect();
        let entries = || regs.table.iter().flatten();
        lanes.extend(entries().filter(|entry| entry.root == entry.id).map(|entry| (entry.id, 0)));
        lanes.sort();
        assert!(lanes.len() <= 128, "{} lanes of a register file are more than a mask holds", lanes.len());
        let (mut first, mut count, mut bytes) = ([0_u8; REGISTERS], [0_u8; REGISTERS], [0_u8; REGISTERS]);
        for (bit, lane) in lanes.iter().enumerate() {
            let slot = lane.0.index();
            if count[slot] == 0 {
                first[slot] = u8::try_from(bit).expect("fits");
            }
            count[slot] += 1;
            bytes[slot] = count[slot];
        }
        for entry in entries() {
            let root = entry.root.index();
            let slot = entry.id.index();
            first[slot] = first[root];
            count[slot] = 1;
            bytes[slot] = u8::try_from((entry.bits / 8).max(1)).expect("a register is under 256 bytes");
        }
        Table { lanes, first, count, bytes }
    }
}

impl Table {
    /// The table of a file that states `@lanes bytes`: the 32 flag lanes, then
    /// a lane for each byte of each root. A view covers the bytes `lane / 8
    /// .. (lane + bits) / 8` of its root (the 6502's `rc3` is byte 1 of
    /// `rs1`).
    fn of_bytes(regs: Regs) -> Table {
        let entries = || regs.table.iter().flatten();
        let bytes_of = |entry: &llrm_lir::registers::Entry| ((entry.bits / 8).max(1)) as usize;
        let mut lanes: Vec<Lane> = (0..32).map(|bit| (RegId::None, bit)).collect();
        for root in entries().filter(|entry| entry.root == entry.id) {
            let top = entries()
                .filter(|view| view.root == root.id)
                .map(|view| (view.lane / 8) as usize + bytes_of(view))
                .max()
                .unwrap_or(1);
            lanes.extend((0..top).map(|byte| (root.id, byte as u32)));
        }
        lanes.sort();
        assert!(lanes.len() <= 128, "{} lanes of a register file are more than a mask holds", lanes.len());
        let (mut first, mut count, mut bytes) = ([0_u8; REGISTERS], [0_u8; REGISTERS], [0_u8; REGISTERS]);
        for (bit, lane) in lanes.iter().enumerate() {
            let slot = lane.0.index();
            if count[slot] == 0 {
                first[slot] = u8::try_from(bit).expect("fits");
            }
            count[slot] += 1;
            bytes[slot] = count[slot];
        }
        for entry in entries().filter(|entry| entry.root != entry.id) {
            let root = entry.root.index();
            let slot = entry.id.index();
            first[slot] = first[root] + u8::try_from(entry.lane / 8).expect("a byte offset");
            count[slot] = u8::try_from(bytes_of(entry)).expect("a register is under 256 bytes");
            bytes[slot] = count[slot];
        }
        Table { lanes, first, count, bytes }
    }
}

/// The table the file of `regs` stands under: x86's for an x86 file, else one
/// built once and kept.
fn table_of(regs: Regs) -> &'static Table {
    static FILES: Mutex<Vec<(usize, &'static Table)>> = Mutex::new(Vec::new());
    // A file with an integer view under four bytes (`al`, `ax`) has byte lanes;
    // one without writes a root whole (arm64's `w` write clears the `x`).
    if !regs.0.byte_lanes
        && let Some((_, small)) = regs
            .0
            .entries()
            .find(|(_, entry)| entry.bits / 8 < 4 && entry.classes & llrm_lir::registers::class::INT != 0)
    {
        // Not a default: a z80 file without the line read x86's table for every
        // register (no effect decoded, every instruction a scheduling
        // barrier).
        assert!(
            regs.0.lanes_x86 || regs.0.lanes_none,
            "a register file with integer views under 32 bits (`{small:?}`) must say `@lanes bytes`, `@lanes x86` \
             or `@lanes none` in its registers.regs"
        );
        return &X86;
    }
    let key = regs.0 as *const _ as usize;
    let mut files = FILES.lock().unwrap();
    if let Some((_, table)) = files.iter().find(|(one, _)| *one == key) {
        return table;
    }
    let table: &'static Table =
        Box::leak(Box::new(if regs.0.byte_lanes { Table::of_bytes(regs) } else { Table::of_file(regs) }));
    files.push((key, table));
    table
}

#[cfg(test)]
thread_local! {
    /// Lane-by-lane walks of a set this thread has begun.
    pub(crate) static WALKS: Cell<usize> = const { Cell::new(0) };
}

thread_local! {
    /// The table this thread's lanes read; none is x86's.
    static ACTIVE: Cell<Option<&'static Table>> = const { Cell::new(None) };
}

fn active() -> &'static Table {
    #[cfg(test)]
    TABLE_READS.with(|count| count.set(count.get() + 1));
    ACTIVE.with(Cell::get).unwrap_or(&X86)
}

#[cfg(test)]
thread_local! {
    static TABLE_READS: Cell<usize> = const { Cell::new(0) };
}

/// The register file's lane table in force until this drops.
pub struct Guard(Option<&'static Table>, #[allow(dead_code)] llrm_lir::registers::NamedBy);

impl Drop for Guard {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.set(self.0));
    }
}

/// Whether `regs` is a file the x86 table serves: byte lanes under a root of
/// four, the shapes `machinecp` has its fixed units for.
pub fn is_x86(regs: Regs) -> bool {
    // a file that says `@lanes none` has no table; it is not x86's
    !regs.0.lanes_none && std::ptr::eq(table_of(regs), &*X86)
}

/// Makes the lanes of `regs`' file the ones this thread reads. A mask made
/// under one file is not meaningful under another.
pub fn enter(regs: Regs) -> Guard {
    // not a default: a consumer of lanes on a target that has none must say so,
    // by stating its lanes
    assert!(
        !regs.0.lanes_none,
        "this register file says `@lanes none` (its target reads no lanes yet), but something reads its lanes: \
         it needs `@lanes bytes` and a table of its own in registers.regs"
    );
    let table = table_of(regs);
    let was = ACTIVE.with(|active| active.replace((!std::ptr::eq(table, &*X86)).then_some(table)));
    // A register prints by its file's name, iced's only for x86.
    let named = llrm_lir::registers::named_by((!std::ptr::eq(table, &*X86)).then_some(regs.0));
    Guard(was, named)
}

fn bit(lane: &Lane) -> Option<u32> {
    bit_in(active(), lane)
}

fn bit_in(
    table: &Table,
    lane: &Lane,
) -> Option<u32> {
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
        active().lanes.len()
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

    /// The flag bit a scheduler stands memory for: a load reads it and a store
    /// writes it, so a store keeps its place against every other access. No
    /// x86 flag uses the bit.
    pub const MEMORY: u32 = 1 << 30;

    /// The flags emitted code reads (`Flag::READ`).
    pub fn read_flags() -> Self {
        Self::flags(llrm_lir::flag::Flag::READ.bits())
    }

    /// Whether `register` has lanes at all: the stack pointer and the like
    /// have none.
    pub fn exists(register: RegId) -> bool {
        let table = active();
        let slot = register.index();
        slot < REGISTERS && table.bytes[slot] > 0
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
        let table = active();
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
        #[cfg(test)]
        WALKS.with(|walks| walks.set(walks.get() + 1));
        Iter(self.0, active())
    }

    /// The flag lanes among these (`RegId::None`'s), without walking the rest.
    pub fn flag_part(&self) -> Self {
        let table = active();
        let slot = RegId::None.index();
        let (first, count) = (u32::from(table.first[slot]), u32::from(table.count[slot]));
        Self(self.0 & (((1u128 << count) - 1) << first))
    }

    // The set operations as `BTreeSet` spells them, yielding lanes in order.
    pub fn union(
        &self,
        other: &Self,
    ) -> Iter {
        Iter(self.0 | other.0, active())
    }

    pub fn difference(
        &self,
        other: &Self,
    ) -> Iter {
        Iter(self.0 & !other.0, active())
    }

    pub fn intersection(
        &self,
        other: &Self,
    ) -> Iter {
        Iter(self.0 & other.0, active())
    }

    /// Every lane of `other` added: `extend` of its lanes, as one word
    /// operation.
    pub fn include(
        &mut self,
        other: Self,
    ) {
        self.0 |= other.0;
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
pub struct Iter(u128, &'static Table);

impl Iterator for Iter {
    type Item = &'static Lane;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0 == 0 {
            return None;
        }
        let bit = self.0.trailing_zeros();
        self.0 &= self.0 - 1;
        Some(&self.1.lanes[bit as usize])
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
        Some(&self.1.lanes[bit as usize])
    }
}

impl ExactSizeIterator for Iter {}

impl IntoIterator for Lanes {
    type Item = Lane;
    type IntoIter = std::iter::Copied<Iter>;

    fn into_iter(self) -> Self::IntoIter {
        Iter(self.0, active()).copied()
    }
}

impl<'a> IntoIterator for &'a Lanes {
    type Item = &'static Lane;
    type IntoIter = Iter;

    fn into_iter(self) -> Iter {
        Iter(self.0, active())
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
        let table = active();
        for lane in lanes {
            self.0 |= 1 << bit_in(table, &lane).unwrap_or_else(|| panic!("{lane:?} is no lane"));
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
        assert!(X86.lanes.len() <= 128);
    }

    /// Every x86 file stands under x86's table, so a pass of an x86 target
    /// reads the lanes it always did.
    #[test]
    fn an_x86_register_is_as_wide_in_the_file_as_iced_says() {
        // `peephole::_lanes` sizes a register by its file; the x86 numbers must
        // not move by it.
        for regs in
            [Regs(&llrm_x86_m16::REGISTER_INFO), Regs(&llrm_x86_m32::REGISTER_INFO), Regs(&llrm_x86_m64::REGISTER_INFO)]
        {
            for (id, entry) in regs.entries().filter(|(_, entry)| entry.classes & llrm_lir::registers::class::INT != 0)
            {
                assert_eq!(regs.bytes(id), Some(id.size() as i64), "{id:?}");
            }
        }
    }

    #[test]
    fn an_x86_file_enters_the_x86_table() {
        for regs in
            [Regs(&llrm_x86_m16::REGISTER_INFO), Regs(&llrm_x86_m32::REGISTER_INFO), Regs(&llrm_x86_m64::REGISTER_INFO)]
        {
            assert!(std::ptr::eq(table_of(regs), &*X86), "{regs:?}");
            let _file = enter(regs);
            assert_eq!(Lanes::count(), X86.lanes.len());
        }
    }

    /// An instruction the decoder cannot read (a name no table has) is of
    /// unknown effect, so a scheduler cannot move anything across it; one
    /// that declares its effect answers with it.
    #[test]
    fn a_declared_effect_answers_for_an_instruction_the_decoder_cannot_read() {
        use std::sync::Arc;

        use crate::backend::liveness;
        use crate::model::ir::{Operation, Semantics};
        use crate::model::lir::Insn;

        let regs = Regs(&llrm_x86_m32::REGISTER_INFO);
        let what = Semantics { name: Some("NoSuchOp".into()), ..Semantics::new(Operation::Binary) };
        let mut one = Insn::new(0, None, Some(what), vec![1], vec![2]);
        assert!(liveness::effect(regs, 32, &Arc::new(one.clone())).is_none(), "premise: the name is unknown");
        let (reads, writes) = (Lanes::flags(0b1), Lanes::flags(0b10));
        one.declared = Some((reads, writes));
        let effect = liveness::effect(regs, 32, &Arc::new(one)).expect("declared");
        assert_eq!((effect.reads, effect.writes), (reads, writes));
    }

    #[test]
    fn test_lanes_iterate_and_compare_as_a_btreeset_does() {
        assert_eq!(Lanes::flags(0b101), Lanes::from([(RegId::None, 0), (RegId::None, 2)]));
        let every: Vec<Lane> = X86.lanes.clone();
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
    /// `Lanes::extend` read the thread's lane table once for each lane it added
    /// (27 M Ir self over a memory_expansion -O2 compile, 111,780 `_lanes`
    /// asks in the peephole alone); it reads it once, and `include` of a
    /// mask reads it never.
    #[test]
    fn extend_reads_the_lane_table_once_and_include_never() {
        let lanes: Vec<Lane> = X86.lanes.iter().copied().take(6).collect();
        let before = TABLE_READS.with(Cell::get);
        let mut extended = Lanes::new();
        extended.extend(lanes.iter().copied());
        assert_eq!(TABLE_READS.with(Cell::get) - before, 1, "one table read for six lanes");
        let before = TABLE_READS.with(Cell::get);
        let mut included = Lanes::new();
        included.include(extended);
        assert_eq!(TABLE_READS.with(Cell::get) - before, 0);
        assert_eq!(included, extended);
    }

    #[test]
    fn span_matches_the_lanes_of_each_byte() {
        let registers: BTreeSet<RegId> = X86.lanes.iter().map(|lane| lane.0).collect();
        for register in registers.into_iter().filter(|register| *register != RegId::None) {
            let bytes = u32::from(X86.bytes[register.index()]);
            for start in 0..bytes {
                for len in 0..=bytes - start {
                    let each: Lanes = (start..start + len).map(|byte| (register, byte)).collect();
                    assert_eq!(Lanes::span(register, start, len), each, "{register:?} {start}+{len}");
                }
            }
        }
    }

    /// A z80 file with 8-bit integer views and no `@lanes` line read x86's
    /// table for every register: the scheduler found no effect for 94 of 94
    /// instructions of `three` and ordered none of them. A file with
    /// such views and no lane declaration is refused by name.
    #[test]
    #[should_panic(expected = "@lanes")]
    fn a_register_file_with_small_integer_views_and_no_lane_line_is_refused() {
        use llrm_lir::registers::{Info, Regs};
        let base = &llrm_x86_m16::REGISTER_INFO;
        assert!(base.lanes_x86, "the x86 files say so");
        let bare = Regs(Box::leak(Box::new(Info {
            table: base.table,
            frame: base.frame,
            stack: base.stack,
            loads: base.loads,
            scratch: base.scratch,
            bytes: base.bytes,
            data_segment: base.data_segment,
            stack_segment: base.stack_segment,
            code_segment: base.code_segment,
            far_segment: base.far_segment,
            widths: base.widths,
            effects: base.effects,
            cache: Default::default(),
            views: base.views,
            byte_lanes: false,
            lanes_x86: false,
            lanes_none: false,
            ties_by_row: base.ties_by_row,
        })));
        let _ = table_of(bare);
    }

    /// m68k's file says `@lanes none`: its private pipeline reads no lanes. A
    /// consumer that later reaches `enter` (the scheduler, machinecp's
    /// fixed units) got x86's table for ids that are not x86's, without a
    /// word; it is refused by name, and `is_x86` is false for the file.
    #[test]
    #[should_panic(expected = "@lanes none")]
    fn reading_the_lanes_of_a_file_that_says_none_is_refused() {
        use llrm_lir::registers::{Info, Regs};
        let base = &llrm_x86_m16::REGISTER_INFO;
        let none = Regs(Box::leak(Box::new(Info {
            table: base.table,
            frame: base.frame,
            stack: base.stack,
            loads: base.loads,
            scratch: base.scratch,
            bytes: base.bytes,
            data_segment: base.data_segment,
            stack_segment: base.stack_segment,
            code_segment: base.code_segment,
            far_segment: base.far_segment,
            widths: base.widths,
            effects: base.effects,
            cache: Default::default(),
            views: base.views,
            byte_lanes: false,
            lanes_x86: false,
            lanes_none: true,
            ties_by_row: base.ties_by_row,
        })));
        assert!(!is_x86(none), "a file that says none is not x86's");
        let _ = enter(none);
    }
}
