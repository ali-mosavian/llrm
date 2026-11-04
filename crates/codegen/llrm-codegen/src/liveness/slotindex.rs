//! `SlotIndex` and `SlotIndexes`: the positions at which a register can become
//! live or cease to be, translated from LLVM's
//! llvm/include/llvm/CodeGen/SlotIndexes.h and llvm/lib/CodeGen/SlotIndexes.cpp
//! at d1106deb71cc.
//!
//! # How it works
//! Every instruction has an entry in a doubly linked list that carries its
//! number. A `SlotIndex` points at an entry and names one of four slots in it
//! (block, early-clobber, register, dead), so a number is `entry.index | slot`
//! and a renumbering of the list moves every `SlotIndex` held anywhere without
//! touching them. Numbers are spaced `InstrDist` (16) apart so an instruction
//! put between two takes a number in the gap; when a gap is closed the entries
//! after it are renumbered locally.
//!
//! ```text
//!   entry   0      16       32       48       (block starts, instructions)
//!   slots   B E R D B E R D ...
//! ```
//!
//! DEVIATE (representation): a `SlotIndex` is the entry's number cell, a
//! reference to storage that is never freed, and the slot, so it compares by
//! reading the cell as LLVM's reads the entry through its tagged pointer. The
//! index is 16 bytes, not 8, and not `Send`.
//!
//! DEVIATE (container, no number): the entries live in a per-thread arena
//! (parallel columns indexed by `EntryId`, `prev`/`next` links) instead of an
//! intrusive `simple_ilist` in a bump allocator, so a `SlotIndex` is `Copy`
//! like the `PointerIntPair`; an instruction is known by `Insn::id()` instead
//! of a `MachineInstr*`. A
//! block is its position in the body. `repairIndexesInRange` is not translated:
//! it repairs a range of a `MachineBasicBlock` iterator pair, and our rewrites
//! name the instructions they put in and take out
//! (`insert_machine_instr_in_maps`, `remove_machine_instr_from_maps`).
//!
//! # Example
//! A block of three instructions numbers 16, 32, 48, then a blank entry at 64
//! that ends the block (and starts the next). The block's range is
//! `[0B, 64B)`.
//!
//! # Used by
//! `liverange` (the points of a live range), and the allocator's shadow core.

use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::fmt;
use std::mem::ManuallyDrop;

use crate::model::lir::LirBody;
use crate::support::hash::HashMap;

/// A stand-in for `MachineInstr*`: `Insn::id()`.
pub type InstrKey = u64;

/// An entry of the index list, by its place in the arena: `IndexListEntry*`.
type EntryId = u32;

const NONE: EntryId = u32::MAX;

/// `IndexListEntry` (SlotIndexes.h#L45): an entry in the slot index list,
/// held in the arena as parallel columns.
#[derive(Default)]
struct Arena {
    instr: Vec<Option<InstrKey>>,
    /// The entry's number, in a cell a `SlotIndex` holds.
    index: Vec<&'static Cell<u32>>,
    /// Cells cut from the last chunk and not yet given to an entry.
    spare: Vec<&'static Cell<u32>>,
    prev: Vec<EntryId>,
    next: Vec<EntryId>,
    free: Vec<EntryId>,
}

/// The thread's arena, without a destructor (see `ARENA`).
type ArenaCell = ManuallyDrop<RefCell<Arena>>;

thread_local! {
    /// The entries of every `SlotIndexes` of this thread: a `SlotIndex` is a
    /// copyable (entry, slot) pair whose number is read here, so renumbering
    /// the list moves every index held anywhere without touching them.
    ///
    /// Never dropped: a type with a destructor makes every access check and
    /// register its state through a call. The columns are emptied
    /// by `SlotIndexes::clear`; the capacity is kept to the thread's end.
    static ARENA: ArenaCell = const {
        ManuallyDrop::new(RefCell::new(Arena {
            instr: Vec::new(),
            index: Vec::new(),
            spare: Vec::new(),
            prev: Vec::new(),
            next: Vec::new(),
            free: Vec::new(),
        }))
    };
    /// How many bodies this thread has numbered: a `SlotIndexes` kept across
    /// another's numbering has lost its entries.
    static ANALYSED: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// How many number cells one chunk holds.
const CHUNK: usize = 256;

impl Arena {
    /// A cell for a new entry. Chunks are leaked: an entry's cell outlives
    /// every `SlotIndex` that holds it, and is reused with the entry's id.
    fn cell(&mut self) -> &'static Cell<u32> {
        if self.spare.is_empty() {
            let chunk: &'static [Cell<u32>] = Box::leak((0..CHUNK).map(|_| Cell::new(0)).collect());
            self.spare.extend(chunk);
        }
        self.spare.pop().expect("a chunk was just cut")
    }

    fn create(
        &mut self,
        instr: Option<InstrKey>,
        index: u32,
    ) -> EntryId {
        if let Some(id) = self.free.pop() {
            let at = id as usize;
            self.instr[at] = instr;
            self.index[at].set(index);
            self.prev[at] = NONE;
            self.next[at] = NONE;
            return id;
        }
        let cell = self.cell();
        cell.set(index);
        self.instr.push(instr);
        self.index.push(cell);
        self.prev.push(NONE);
        self.next.push(NONE);
        (self.index.len() - 1) as EntryId
    }
}

/// The slots of an instruction's entry (`SlotIndex::Slot`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Slot {
    /// Basic block boundary. Also the def slot of PHI-defs.
    Block = 0,
    /// Early-clobber register use/def slot.
    EarlyClobber = 1,
    /// Normal register use/def slot.
    Register = 2,
    /// Dead def kill point.
    Dead = 3,
}

const SLOT_COUNT: u32 = 4;

impl Slot {
    fn from(slot: u8) -> Self {
        match slot {
            0 => Slot::Block,
            1 => Slot::EarlyClobber,
            2 => Slot::Register,
            _ => Slot::Dead,
        }
    }
}

/// `SlotIndex` (SlotIndexes.h#L74): an opaque wrapper around machine indexes,
/// as C++'s `PointerIntPair<IndexListEntry*, 2>`: copyable, compared by the
/// number its entry holds now.
#[derive(Clone, Copy)]
pub struct SlotIndex {
    lie: EntryId,
    /// The entry's number, read on every compare (`IndexListEntry::getIndex`).
    cell: Option<&'static Cell<u32>>,
    slot: u8,
}

impl Default for SlotIndex {
    /// An invalid index.
    fn default() -> Self {
        Self { lie: NONE, cell: None, slot: 0 }
    }
}

impl SlotIndex {
    /// The default distance between instructions (`InstrDist`).
    pub const INSTR_DIST: u32 = 4 * SLOT_COUNT;

    fn at(
        entry: EntryId,
        slot: Slot,
    ) -> Self {
        ARENA.with(|arena| {
            let arena = arena.borrow();
            Self { lie: entry, cell: Some(arena.index[entry as usize]), slot: slot as u8 }
        })
    }

    /// Another slot of the same instruction, as old as this.
    fn same_entry(
        &self,
        slot: Slot,
    ) -> Self {
        assert!(self.lie != NONE, "Attempt to compare reserved index.");
        Self { slot: slot as u8, ..*self }
    }

    fn list_entry(&self) -> EntryId {
        assert!(self.lie != NONE, "Attempt to compare reserved index.");
        self.lie
    }

    fn entry_index(&self) -> u32 {
        self.cell.expect("Attempt to compare reserved index.").get()
    }

    /// The number: the entry's index with the slot in the low bits.
    pub fn get_index(&self) -> u32 {
        self.entry_index() | u32::from(self.slot)
    }

    pub fn get_slot(&self) -> Slot {
        Slot::from(self.slot)
    }

    /// A new index at `slot` of the instruction `li` points to.
    pub fn with_slot(
        li: &SlotIndex,
        slot: Slot,
    ) -> Self {
        li.same_entry(slot)
    }

    pub fn is_valid(&self) -> bool {
        self.lie != NONE
    }

    pub fn is_same_instr(
        a: &SlotIndex,
        b: &SlotIndex,
    ) -> bool {
        a.list_entry() == b.list_entry()
    }

    pub fn is_earlier_instr(
        a: &SlotIndex,
        b: &SlotIndex,
    ) -> bool {
        a.entry_index() < b.entry_index()
    }

    pub fn is_earlier_equal_instr(
        a: &SlotIndex,
        b: &SlotIndex,
    ) -> bool {
        !Self::is_earlier_instr(b, a)
    }

    pub fn distance(
        &self,
        other: &SlotIndex,
    ) -> i32 {
        other.get_index() as i32 - self.get_index() as i32
    }

    pub fn get_approx_instr_distance(
        &self,
        other: &SlotIndex,
    ) -> i32 {
        (other.entry_index() as i32 - self.entry_index() as i32) / SLOT_COUNT as i32
    }

    pub fn is_block(&self) -> bool {
        self.get_slot() == Slot::Block
    }

    pub fn is_early_clobber(&self) -> bool {
        self.get_slot() == Slot::EarlyClobber
    }

    pub fn is_register(&self) -> bool {
        self.get_slot() == Slot::Register
    }

    pub fn is_dead(&self) -> bool {
        self.get_slot() == Slot::Dead
    }

    pub fn get_base_index(&self) -> SlotIndex {
        self.same_entry(Slot::Block)
    }

    pub fn get_boundary_index(&self) -> SlotIndex {
        self.same_entry(Slot::Dead)
    }

    pub fn get_reg_slot(
        &self,
        early_clobber: bool,
    ) -> SlotIndex {
        self.same_entry(if early_clobber { Slot::EarlyClobber } else { Slot::Register })
    }

    pub fn get_dead_slot(&self) -> SlotIndex {
        self.same_entry(Slot::Dead)
    }

    fn next_entry(&self) -> EntryId {
        let next = ARENA.with(|arena| arena.borrow().next[self.list_entry() as usize]);
        assert!(next != NONE, "no entry after the last one");
        next
    }

    fn prev_entry(&self) -> EntryId {
        let prev = ARENA.with(|arena| arena.borrow().prev[self.list_entry() as usize]);
        assert!(prev != NONE, "no entry before the first one");
        prev
    }

    /// `getNextSlot`: the next slot of this instruction or the first slot of
    /// the next one.
    pub fn get_next_slot(&self) -> SlotIndex {
        match self.get_slot() {
            Slot::Dead => Self::at(self.next_entry(), Slot::Block),
            s => self.same_entry(Slot::from(s as u8 + 1)),
        }
    }

    pub fn get_next_index(&self) -> SlotIndex {
        Self::at(self.next_entry(), self.get_slot())
    }

    /// `getPrevSlot`: the previous slot of this instruction or the last slot of
    /// the previous one.
    pub fn get_prev_slot(&self) -> SlotIndex {
        match self.get_slot() {
            Slot::Block => Self::at(self.prev_entry(), Slot::Dead),
            s => self.same_entry(Slot::from(s as u8 - 1)),
        }
    }

    pub fn get_prev_index(&self) -> SlotIndex {
        Self::at(self.prev_entry(), self.get_slot())
    }
}

impl PartialEq for SlotIndex {
    #[inline(always)]
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.lie == other.lie && self.slot == other.slot
    }
}

impl Eq for SlotIndex {}

impl PartialOrd for SlotIndex {
    #[inline(always)]
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SlotIndex {
    /// `SlotIndex::operator<`: the two entries' numbers.
    #[inline(always)]
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        self.get_index().cmp(&other.get_index())
    }
}

impl std::hash::Hash for SlotIndex {
    fn hash<H: std::hash::Hasher>(
        &self,
        state: &mut H,
    ) {
        self.lie.hash(state);
        self.slot.hash(state);
    }
}

/// `SlotIndex::print`: the number and one of "Berd".
impl fmt::Display for SlotIndex {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        if self.is_valid() {
            write!(f, "{}{}", self.entry_index(), ["B", "e", "r", "d"][self.slot as usize])
        } else {
            write!(f, "invalid")
        }
    }
}

impl fmt::Debug for SlotIndex {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// `SlotIndexes` (SlotIndexes.h#L248): assigns indexes to each instruction.
#[derive(Default)]
pub struct SlotIndexes {
    first: EntryId,
    last: EntryId,
    mi2i_map: HashMap<InstrKey, SlotIndex>,
    /// The block each indexed instruction is in (`MI->getParent()`).
    mi2mbb: HashMap<InstrKey, usize>,
    /// The instructions of a bundle (a parallel copy) after the one the entry
    /// names: they share its index.
    bundled: HashMap<EntryId, Vec<InstrKey>>,
    /// `MBBRanges`: block number to (start, stop).
    mbb_ranges: Vec<(SlotIndex, SlotIndex)>,
    /// `idx2MBBMap`: sorted pairs of the index of a block's start and the
    /// block.
    idx2mbb_map: Vec<(SlotIndex, usize)>,
    local_renumberings: usize,
    /// Whether this holds entries of the arena.
    live: bool,
}

impl Drop for SlotIndexes {
    fn drop(&mut self) {
        self.clear();
    }
}

/// How many bodies this thread has numbered (`analyze`).
pub fn analysed() -> u64 {
    ANALYSED.with(std::cell::Cell::get)
}

impl SlotIndexes {
    /// `SlotIndexes(MachineFunction&)` / `analyze` (SlotIndexes.cpp#L66): the
    /// numbering of `body`, a block's start sharing the entry that ends the
    /// block before it and one blank entry between blocks. The instructions of
    /// a parallel copy share the entry of the first, as the instructions of a
    /// bundle share its index (`getInstructionIndex`).
    pub fn new(body: &LirBody) -> Self {
        let mut found = Self::default();
        found.analyze(body);
        found
    }

    fn push_back(
        &mut self,
        instr: Option<InstrKey>,
        index: u32,
    ) -> EntryId {
        ARENA.with(|arena| {
            let mut arena = arena.borrow_mut();
            let entry = arena.create(instr, index);
            if self.live {
                let last = self.last;
                arena.prev[entry as usize] = last;
                arena.next[last as usize] = entry;
            } else {
                self.first = entry;
                self.live = true;
            }
            self.last = entry;
            entry
        })
    }

    /// `clear`: frees the entries one by one.
    pub fn clear(&mut self) {
        self.mi2i_map.clear();
        self.mi2mbb.clear();
        self.bundled.clear();
        self.mbb_ranges.clear();
        self.idx2mbb_map.clear();
        if self.live {
            ARENA.with(|arena| {
                let mut arena = arena.borrow_mut();
                let mut at = self.first;
                while at != NONE {
                    let next = arena.next[at as usize];
                    arena.free.push(at);
                    at = next;
                }
            });
        }
        self.live = false;
        self.first = NONE;
        self.last = NONE;
    }

    pub fn analyze(
        &mut self,
        body: &LirBody,
    ) {
        assert!(!self.live, "Index list non-empty at initial numbering?");
        ANALYSED.with(|count| count.set(count.get() + 1));
        let mut index = 0u32;
        self.mbb_ranges = vec![(SlotIndex::default(), SlotIndex::default()); body.blocks.len()];
        self.push_back(None, index);
        for (number, block) in body.blocks.iter().enumerate() {
            let block_start_index = SlotIndex::at(self.last, Slot::Block);
            let mut previous_group: Option<i64> = None;
            for one in block.insns.iter() {
                // The instructions of a parallel copy are one bundle: one
                // index.
                if let (Some(group), Some(same)) = (one.group, previous_group) {
                    if group == same {
                        let at = SlotIndex::at(self.last, Slot::Block);
                        self.mi2i_map.insert(one.id(), at);
                        self.mi2mbb.insert(one.id(), number);
                        self.bundled.entry(at.list_entry()).or_default().push(one.id());
                        continue;
                    }
                }
                previous_group = one.group;
                index += SlotIndex::INSTR_DIST;
                self.push_back(Some(one.id()), index);
                let at = SlotIndex::at(self.last, Slot::Block);
                self.mi2i_map.insert(one.id(), at);
                self.mi2mbb.insert(one.id(), number);
            }
            // We insert one blank instructions between basic blocks.
            index += SlotIndex::INSTR_DIST;
            self.push_back(None, index);
            self.mbb_ranges[number].0 = block_start_index;
            self.mbb_ranges[number].1 = SlotIndex::at(self.last, Slot::Block);
            self.idx2mbb_map.push((block_start_index, number));
        }
        self.idx2mbb_map.sort_by(|a, b| a.0.cmp(&b.0));
    }

    /// Whether more than one instruction shares `index`.
    pub fn is_bundled(
        &self,
        index: &SlotIndex,
    ) -> bool {
        self.bundled.contains_key(&index.list_entry())
    }

    /// Every instruction at `index`: the first, then the rest of its bundle
    /// (the instructions of a parallel copy share one index).
    pub fn get_instructions_at(
        &self,
        index: &SlotIndex,
    ) -> Vec<InstrKey> {
        let Some(first) = self.get_instruction_from_index(index) else { return Vec::new() };
        let mut all = vec![first];
        if let Some(others) = self.bundled.get(&index.list_entry()) {
            all.extend(others.iter().copied());
        }
        all
    }

    /// `getZeroIndex`.
    pub fn get_zero_index(&self) -> SlotIndex {
        let first = SlotIndex::at(self.first, Slot::Block);
        assert_eq!(first.entry_index(), 0, "First index is not 0?");
        first
    }

    /// `getLastIndex`: the base index of the last slot.
    pub fn get_last_index(&self) -> SlotIndex {
        SlotIndex::at(self.last, Slot::Block)
    }

    /// `hasIndex`.
    pub fn has_index(
        &self,
        instr: InstrKey,
    ) -> bool {
        self.mi2i_map.contains_key(&instr)
    }

    /// `getInstructionIndex`: the base index of the instruction.
    pub fn get_instruction_index(
        &self,
        instr: InstrKey,
    ) -> SlotIndex {
        *self.mi2i_map.get(&instr).expect("Instruction not found in maps.")
    }

    /// `getInstructionFromIndex`: the instruction at `index`, or None.
    pub fn get_instruction_from_index(
        &self,
        index: &SlotIndex,
    ) -> Option<InstrKey> {
        ARENA.with(|arena| arena.borrow().instr[index.list_entry() as usize])
    }

    /// `getNextNonNullIndex`.
    pub fn get_next_non_null_index(
        &self,
        index: &SlotIndex,
    ) -> SlotIndex {
        ARENA.with(|arena| {
            let arena = arena.borrow();
            let mut at = arena.next[index.list_entry() as usize];
            while at != NONE {
                if arena.instr[at as usize].is_some() {
                    return SlotIndex::at(at, index.get_slot());
                }
                at = arena.next[at as usize];
            }
            // We reached the end of the function.
            self.get_last_index()
        })
    }

    /// `getIndexBefore`: the index of the last indexed instruction before the
    /// one at `position` of block `block` (whose instructions are `insns`), or
    /// the start index of its block.
    pub fn get_index_before(
        &self,
        block: usize,
        insns: &[std::sync::Arc<crate::model::lir::Insn>],
        position: usize,
    ) -> SlotIndex {
        for one in insns[..position].iter().rev() {
            if let Some(found) = self.mi2i_map.get(&one.id()) {
                return *found;
            }
        }
        self.get_mbb_start_idx(block)
    }

    /// `getIndexAfter`: the index of the first indexed instruction after the
    /// one at `position`, or the end index of its block.
    pub fn get_index_after(
        &self,
        block: usize,
        insns: &[std::sync::Arc<crate::model::lir::Insn>],
        position: usize,
    ) -> SlotIndex {
        for one in &insns[position + 1..] {
            if let Some(found) = self.mi2i_map.get(&one.id()) {
                return *found;
            }
        }
        self.get_mbb_end_idx(block)
    }

    /// `getMBBRange`.
    pub fn get_mbb_range(
        &self,
        block: usize,
    ) -> &(SlotIndex, SlotIndex) {
        &self.mbb_ranges[block]
    }

    pub fn get_mbb_start_idx(
        &self,
        block: usize,
    ) -> SlotIndex {
        self.mbb_ranges[block].0
    }

    pub fn get_mbb_end_idx(
        &self,
        block: usize,
    ) -> SlotIndex {
        self.mbb_ranges[block].1
    }

    /// `getMBBLastIdx`: the dead slot of the last instruction of the block.
    pub fn get_mbb_last_idx(
        &self,
        block: usize,
    ) -> SlotIndex {
        self.get_mbb_end_idx(block).get_prev_slot()
    }

    /// `getMBBLowerBound`: the position in `idx2MBBMap` of the first pair with
    /// an index at or after `idx`.
    pub fn get_mbb_lower_bound(
        &self,
        idx: &SlotIndex,
    ) -> usize {
        self.idx2mbb_map.partition_point(|pair| pair.0 < *idx)
    }

    /// `getMBBUpperBound`: the first pair with an index after `idx`.
    pub fn get_mbb_upper_bound(
        &self,
        idx: &SlotIndex,
    ) -> usize {
        self.idx2mbb_map.partition_point(|pair| pair.0 <= *idx)
    }

    /// `getMBBFromIndex`: the block `index` falls in.
    pub fn get_mbb_from_index(
        &self,
        index: &SlotIndex,
    ) -> usize {
        if let Some(instr) = self.get_instruction_from_index(index) {
            return self.mi2mbb[&instr];
        }
        let at = self.get_mbb_upper_bound(index) - 1;
        let (start, block) = &self.idx2mbb_map[at];
        assert!(start <= index && *index < self.get_mbb_end_idx(*block), "index does not correspond to an MBB");
        *block
    }

    /// `insertMachineInstrInMaps` (SlotIndexes.h#L497): gives `instr` an index
    /// between `prev` and `next`, the base indexes of its neighbours (from
    /// `get_index_before` and `get_index_after`; with `late` it goes
    /// immediately before `next`, after any null entries).
    pub fn insert_machine_instr_in_maps(
        &mut self,
        instr: InstrKey,
        block: usize,
        prev: &SlotIndex,
        next: &SlotIndex,
        late: bool,
    ) -> SlotIndex {
        assert!(!self.mi2i_map.contains_key(&instr), "Instr already indexed.");
        let (entry, dist) = ARENA.with(|arena| {
            let mut arena = arena.borrow_mut();
            let (prev_entry, next_entry) = if late {
                // Insert MI's index immediately before the following
                // instruction.
                let next_entry = next.list_entry();
                (arena.prev[next_entry as usize], next_entry)
            } else {
                // Insert MI's index immediately after the preceding
                // instruction.
                let prev_entry = prev.list_entry();
                (prev_entry, arena.next[prev_entry as usize])
            };
            // Get a number for the new instr, or 0 if there's no room
            // currently.
            let dist = ((arena.index[next_entry as usize].get() - arena.index[prev_entry as usize].get()) / 2) & !3u32;
            let new_number = arena.index[prev_entry as usize].get() + dist;
            let entry = arena.create(Some(instr), new_number);
            arena.prev[entry as usize] = prev_entry;
            arena.next[entry as usize] = next_entry;
            arena.prev[next_entry as usize] = entry;
            arena.next[prev_entry as usize] = entry;
            (entry, dist)
        });
        // Renumber locally if we need to.
        if dist == 0 {
            self.renumber_indexes(entry);
        }
        let new_index = SlotIndex::at(entry, Slot::Block);
        self.mi2i_map.insert(instr, new_index);
        self.mi2mbb.insert(instr, block);
        new_index
    }

    /// `removeMachineInstrFromMaps` (SlotIndexes.cpp#L113): the entry stays,
    /// with no instruction.
    pub fn remove_machine_instr_from_maps(
        &mut self,
        instr: InstrKey,
    ) {
        let Some(index) = self.mi2i_map.remove(&instr) else { return };
        self.mi2mbb.remove(&instr);
        let entry = index.list_entry();
        let others = self.bundled.get_mut(&entry);
        ARENA.with(|arena| {
            let mut arena = arena.borrow_mut();
            let at = entry as usize;
            if arena.instr[at] == Some(instr) {
                // The first instruction of a bundle: the entry goes to the next
                // (removeSingleMachineInstrFromMaps, SlotIndexes.cpp#L127).
                // FIXME (LLVM): Eventually we want to actually delete these
                // indexes.
                arena.instr[at] = match others {
                    Some(list) if !list.is_empty() => Some(list.remove(0)),
                    _ => None,
                };
            } else if let Some(list) = others {
                list.retain(|one| *one != instr);
            }
        });
    }

    /// `replaceMachineInstrInMaps`: `new` takes the index of `old`.
    pub fn replace_machine_instr_in_maps(
        &mut self,
        old: InstrKey,
        new: InstrKey,
    ) -> SlotIndex {
        let Some(base) = self.mi2i_map.remove(&old) else { return SlotIndex::default() };
        if let Some(block) = self.mi2mbb.remove(&old) {
            self.mi2mbb.insert(new, block);
        }
        let entry = base.list_entry();
        ARENA.with(|arena| {
            let mut arena = arena.borrow_mut();
            let at = entry as usize;
            if arena.instr[at] == Some(old) {
                arena.instr[at] = Some(new);
            } else {
                // A later instruction of a bundle.
                let list = self.bundled.get_mut(&entry).expect("a bundled instruction");
                let place = list.iter().position(|one| *one == old).expect("Mismatched instruction in index tables.");
                list[place] = new;
            }
        });
        self.mi2i_map.insert(new, base);
        base
    }

    /// `renumberIndexes` (SlotIndexes.cpp#L196): renumbers locally after the
    /// entry `cur` was inserted but failed to get a new index.
    fn renumber_indexes(
        &mut self,
        cur: EntryId,
    ) {
        // Number indexes with half the default spacing so we can catch up
        // quickly.
        const SPACE: u32 = SlotIndex::INSTR_DIST / 2;
        let (begin_index, index) = ARENA.with(|arena| {
            let arena = arena.borrow_mut();
            let start = arena.prev[cur as usize];
            let mut index = arena.index[start as usize].get();
            let begin_index = index;
            let mut at = cur;
            loop {
                index += SPACE;
                arena.index[at as usize].set(index);
                at = arena.next[at as usize];
                // If the next index is bigger, we have caught up.
                if at == NONE || arena.index[at as usize].get() > index {
                    break;
                }
            }
            (begin_index, index)
        });
        // If we repack more than 20% of a function, add spacing in between the
        // instructions so that future renumberings are able to catch up
        // without also renumbering so much.
        if index - begin_index > (self.get_last_index().get_index() - self.get_zero_index().get_index()) / 5 {
            self.pack_indexes();
        }
        self.local_renumberings += 1;
    }

    /// `packIndexes`: renumbers all indexes using the default distance.
    pub fn pack_indexes(&mut self) {
        // Oracle runs only (`LLRM_PACK_DENSE`): see `pack`.
        self.pack(llrm_support::env_flag!("LLRM_PACK_DENSE"));
    }

    /// `packIndexes`; `dense` leaves out the entries an erased instruction left
    /// empty. The pin parses a MIR with no instruction removed, so a same-MIR
    /// comparison with it needs them out: on queens `place_` the entries made
    /// every interval 1.3x longer and its spill weights 3-50% lower than the
    /// pin's for the same intervals.
    pub fn pack(
        &mut self,
        dense: bool,
    ) {
        let boundaries: llrm_support::hash::HashSet<EntryId> = self
            .mbb_ranges
            .iter()
            .flat_map(|(start, end)| [start.list_entry(), end.list_entry()])
            .chain([self.first])
            .collect();
        ARENA.with(|arena| {
            let arena = arena.borrow_mut();
            let mut at = self.first;
            let mut number = 0u32;
            while at != NONE {
                arena.index[at as usize].set(number * SlotIndex::INSTR_DIST);
                if !(dense && arena.instr[at as usize].is_none() && !boundaries.contains(&at)) {
                    number += 1;
                }
                at = arena.next[at as usize];
            }
        });
    }

    /// `insertMBBInMaps` (SlotIndexes.h#L549): after block `block` was made by
    /// moving the suffix of block `block - 1` into it. `first_instr` is its
    /// first instruction, if it has any (it must be indexed already).
    pub fn insert_mbb_in_maps(
        &mut self,
        block: usize,
        first_instr: Option<InstrKey>,
    ) {
        assert!(block != 0, "Can't insert a new block at the beginning of a function.");
        // Create a new entry to be used for the start of mbb and the end of
        // prevMBB.
        let end_entry = self.mbb_ranges[block - 1].1.list_entry();
        let ins_entry = match first_instr {
            Some(instr) => self.get_instruction_index(instr).list_entry(),
            None => end_entry,
        };
        // indexList.insert(insEntry, startEntry): before insEntry.
        let start_entry = ARENA.with(|arena| {
            let mut arena = arena.borrow_mut();
            let start_entry = arena.create(None, 0);
            let before = arena.prev[ins_entry as usize];
            arena.prev[start_entry as usize] = before;
            arena.next[start_entry as usize] = ins_entry;
            arena.prev[ins_entry as usize] = start_entry;
            arena.next[before as usize] = start_entry;
            start_entry
        });
        let start_idx = SlotIndex::at(start_entry, Slot::Block);
        let end_idx = SlotIndex::at(end_entry, Slot::Block);
        self.mbb_ranges[block - 1].1 = start_idx;
        assert_eq!(block, self.mbb_ranges.len(), "Blocks must be added in order");
        self.mbb_ranges.push((start_idx, end_idx));
        self.idx2mbb_map.push((start_idx, block));
        self.renumber_indexes(start_entry);
        self.idx2mbb_map.sort_by(|a, b| a.0.cmp(&b.0));
    }

    /// `removeMBBFromMaps`: gives the last block's range to its predecessor.
    pub fn remove_mbb_from_maps(
        &mut self,
        block: usize,
    ) {
        assert!(block != 0, "Can't remove the first block of a function.");
        let (start, end) = self.mbb_ranges[block];
        // Give MBB's slot range to its layout predecessor so blocks stay
        // contiguous.
        self.mbb_ranges[block - 1].1 = end;
        // Drop MBB's index -> MBB entry.
        let at = self.get_mbb_lower_bound(&start);
        assert!(at < self.idx2mbb_map.len() && self.idx2mbb_map[at].0 == start && self.idx2mbb_map[at].1 == block);
        self.idx2mbb_map.remove(at);
        // Clear the block-start boundary entry.
        ARENA.with(|arena| arena.borrow_mut().instr[start.list_entry() as usize] = None);
    }

    /// How many local renumberings this has done (`NumLocalRenum`).
    pub fn local_renumberings(&self) -> usize {
        self.local_renumberings
    }

    /// `print`: each entry's number and whether it is an instruction.
    pub fn print(&self) -> String {
        let mut out = String::new();
        ARENA.with(|arena| {
            let arena = arena.borrow();
            let mut at = self.first;
            while at != NONE {
                out.push_str(&format!(
                    "{} {}\n",
                    arena.index[at as usize].get(),
                    arena.instr[at as usize].map_or("-".to_owned(), |one| one.to_string())
                ));
                at = arena.next[at as usize];
            }
        });
        for (number, (start, end)) in self.mbb_ranges.iter().enumerate() {
            out.push_str(&format!("bb.{number}\t[{start};{end})\n"));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    /// Comparing and numbering read the entry's cell, not the arena: with the
    /// arena borrowed for writing they still work. Through the arena each
    /// `get_index` was a thread-local access and a borrow (10 M Ir on d_faces
    /// -O2, and a panic here).
    #[test]
    fn test_compare_and_number_do_not_read_the_arena() {
        let body = body(&[3]);
        let indexes = SlotIndexes::new(&body);
        let (a, b) = (indexes.get_zero_index(), indexes.get_last_index());
        ARENA.with(|arena| {
            let _held = arena.borrow_mut();
            assert!(a < b && a.distance(&b) > 0 && SlotIndex::is_earlier_instr(&a, &b));
            assert_eq!(a.get_index() | 3, a.get_boundary_index().get_index());
        });
    }

    /// A thread-local with a destructor is reached through a registration
    /// check and a call on every access: 48 Ir a stale compare, 131 K of them
    /// (0.8 M Ir with the other accesses) on d_faces -O2.
    #[test]
    fn test_arena_has_no_destructor_so_access_is_not_a_call() {
        assert!(!std::mem::needs_drop::<ArenaCell>());
    }

    fn insn() -> Arc<Insn> {
        Arc::new(Insn::new(0, None, None, vec![], vec![]))
    }

    fn body(sizes: &[usize]) -> LirBody {
        let blocks = sizes
            .iter()
            .enumerate()
            .map(|(at, size)| {
                let insns: Vec<Arc<Insn>> = (0..*size).map(|_| insn()).collect();
                crate::model::lir::Block::from(LirBlock::new(at as i64, insns))
            })
            .collect::<Vec<_>>();
        LirBody::new("indexes", 0, blocks, IndexMap::default(), IndexMap::default())
    }

    /// SlotIndexes::analyze: 16 apart, a blank entry between blocks, a block's
    /// range from its start entry to the blank entry that ends it.
    /// An erased instruction leaves its entry in the list, as LLVM does, and
    /// `packIndexes` counts it; the dense packing does not.
    #[test]
    fn test_packing_counts_the_entry_an_erased_instruction_left_unless_dense() {
        let body = body(&[3]);
        let last = body.blocks[0].insns[2].id();
        let mut indexes = SlotIndexes::new(&body);
        indexes.remove_machine_instr_from_maps(body.blocks[0].insns[1].id());
        indexes.pack(false);
        assert_eq!(indexes.get_instruction_index(last).get_index(), 48);
        indexes.pack(true);
        assert_eq!(indexes.get_instruction_index(last).get_index(), 32);
    }

    #[test]
    fn test_blocks_are_numbered_sixteen_apart_with_a_blank_entry_between() {
        let body = body(&[3, 2]);
        let indexes = SlotIndexes::new(&body);
        let numbers: Vec<u32> = body
            .blocks
            .iter()
            .flat_map(|b| b.insns.iter())
            .map(|one| indexes.get_instruction_index(one.id()).get_index())
            .collect();
        assert_eq!(numbers, vec![16, 32, 48, 80, 96]);
        assert_eq!(indexes.get_mbb_start_idx(0).get_index(), 0);
        assert_eq!(indexes.get_mbb_end_idx(0).get_index(), 64);
        assert_eq!(indexes.get_mbb_start_idx(1).get_index(), 64, "a block starts where the one before ends");
        assert_eq!(indexes.get_mbb_end_idx(1).get_index(), 112);
        assert_eq!(indexes.get_mbb_last_idx(0).to_string(), "48d");
    }

    /// The slots of an entry are ordered block < early-clobber < register <
    /// dead and an instruction's slots are all before the next
    /// instruction's block slot.
    #[test]
    fn test_slots_of_an_instruction_come_before_the_next_instructions() {
        let body = body(&[2]);
        let indexes = SlotIndexes::new(&body);
        let a = indexes.get_instruction_index(body.blocks[0].insns[0].id());
        let b = indexes.get_instruction_index(body.blocks[0].insns[1].id());
        assert!(
            a < a.get_reg_slot(true)
                && a.get_reg_slot(true) < a.get_reg_slot(false)
                && a.get_reg_slot(false) < a.get_dead_slot()
        );
        assert!(a.get_dead_slot() < b);
        assert_eq!(a.get_dead_slot().get_next_slot(), b);
        assert_eq!(b.get_prev_slot(), a.get_dead_slot());
        assert!(SlotIndex::is_same_instr(&a, &a.get_reg_slot(false)) && !SlotIndex::is_same_instr(&a, &b));
        assert_eq!(a.get_next_index(), b);
        assert_eq!(b.get_prev_index(), a);
    }

    /// An instruction put between two takes the middle of their gap, so a
    /// SlotIndex held for a neighbour is still its neighbour's; a closed
    /// gap renumbers the entries after it and every index held follows.
    #[test]
    fn test_an_instruction_put_between_two_takes_the_gap_and_a_closed_gap_renumbers() {
        let body = body(&[2]);
        let mut indexes = SlotIndexes::new(&body);
        let (first, second) = (body.blocks[0].insns[0].clone(), body.blocks[0].insns[1].clone());
        let a = indexes.get_instruction_index(first.id());
        let b = indexes.get_instruction_index(second.id());
        let previous = a.clone();
        let mut made = Vec::new();
        for n in 0..40u64 {
            let key = 1_000_000 + n;
            // each goes immediately before `second`
            let at = indexes.insert_machine_instr_in_maps(key, 0, &previous, &b, true);
            assert!(a < at && at < b, "insert {n}: {a} < {at} < {b}");
            made.push(at);
        }
        assert!(indexes.local_renumberings() > 0, "premise: a gap closed and entries were renumbered");
        for pair in made.windows(2) {
            assert!(pair[0] < pair[1], "later inserts come after earlier ones: {} {}", pair[0], pair[1]);
        }
        assert_eq!(indexes.get_instruction_index(first.id()), a);
        assert_eq!(indexes.get_instruction_from_index(&made[7]), Some(1_000_007));
    }

    #[test]
    fn test_removed_and_replaced_instructions_keep_their_entries() {
        let body = body(&[2]);
        let mut indexes = SlotIndexes::new(&body);
        let first = body.blocks[0].insns[0].id();
        let at = indexes.get_instruction_index(first);
        let again = indexes.replace_machine_instr_in_maps(first, 77);
        assert_eq!(again, at);
        assert_eq!(indexes.get_instruction_from_index(&at), Some(77));
        assert!(!indexes.has_index(first) && indexes.has_index(77));
        indexes.remove_machine_instr_from_maps(77);
        assert_eq!(indexes.get_instruction_from_index(&at), None);
        assert_eq!(
            indexes.get_next_non_null_index(&at).get_index(),
            32,
            "the next non-null entry is the second instruction"
        );
    }

    /// Dropping the list followed its `next` links one entry at a time: a
    /// function of 300 K instructions overflowed the stack.
    #[test]
    fn test_a_long_list_is_dropped_without_recursion() {
        let body = body(&[300_000]);
        let indexes = SlotIndexes::new(&body);
        assert!(indexes.get_last_index().get_index() > 4_000_000);
        drop(indexes);
    }

    /// `getMBBFromIndex`: an index with no instruction falls in the block whose
    /// range holds it.
    #[test]
    fn test_a_blank_index_belongs_to_the_block_whose_range_holds_it() {
        let body = body(&[1, 1]);
        let indexes = SlotIndexes::new(&body);
        // The blank entry that ends block 0 starts block 1 (its range is half
        // open).
        let blank_between = indexes.get_mbb_end_idx(0);
        assert_eq!(indexes.get_mbb_from_index(&blank_between), 1);
        let last_of_first = indexes.get_mbb_last_idx(0);
        assert_eq!(indexes.get_mbb_from_index(&last_of_first), 0, "the dead slot of an instruction is in its block");
        let in_second = indexes.get_instruction_index(body.blocks[1].insns[0].id());
        assert_eq!(indexes.get_mbb_from_index(&in_second), 1);
    }

    /// The instructions of a parallel copy are one bundle: they share the
    /// index of the first, so the copy reads and writes at one point.
    #[test]
    fn test_the_instructions_of_a_parallel_copy_share_one_index() {
        let mut insns: Vec<Arc<Insn>> = (0..4).map(|_| insn()).collect();
        for at in 1..3 {
            let mut one = (*insns[at]).clone();
            one.group = Some(7);
            insns[at] = Arc::new(one);
        }
        let body = LirBody::new(
            "bundle",
            0,
            vec![crate::model::lir::Block::from(LirBlock::new(0, insns.clone()))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let indexes = SlotIndexes::new(&body);
        let at: Vec<u32> = insns.iter().map(|one| indexes.get_instruction_index(one.id()).get_index()).collect();
        assert_eq!(at, vec![16, 32, 32, 48]);
    }

    /// Entries of a dropped `SlotIndexes` are reused: numbering a body over and
    /// over does not grow the arena.
    #[test]
    fn test_the_entries_of_a_dropped_numbering_are_reused() {
        let body = body(&[50]);
        let size = || ARENA.with(|arena| arena.borrow().index.len());
        drop(SlotIndexes::new(&body));
        let after_one = size();
        for _ in 0..20 {
            drop(SlotIndexes::new(&body));
        }
        assert_eq!(size(), after_one);
    }

    /// A rewrite of the second instruction of a parallel copy asked the entry
    /// for the first ("Mismatched instruction in index tables", d_faces -O2
    /// under the shadow allocator). Replacing and removing the members of a
    /// bundle keeps the others' index.
    #[test]
    fn test_replacing_and_removing_members_of_a_parallel_copy_keeps_the_index() {
        let mut insns: Vec<Arc<Insn>> = (0..4).map(|_| insn()).collect();
        for at in 1..4 {
            let mut one = (*insns[at]).clone();
            one.group = Some(7);
            insns[at] = Arc::new(one);
        }
        let body = LirBody::new(
            "bundle",
            0,
            vec![crate::model::lir::Block::from(LirBlock::new(0, insns.clone()))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let mut indexes = SlotIndexes::new(&body);
        let (first, second, third) = (insns[1].id(), insns[2].id(), insns[3].id());
        let at = indexes.get_instruction_index(first);
        let replaced = indexes.replace_machine_instr_in_maps(second, 9_000_001);
        assert_eq!(replaced, at, "a later member keeps the shared index");
        assert_eq!(indexes.get_instruction_from_index(&at), Some(first), "the entry still names the first");
        indexes.remove_machine_instr_from_maps(first);
        assert_eq!(indexes.get_instruction_from_index(&at), Some(9_000_001), "the entry goes to the next member");
        indexes.remove_machine_instr_from_maps(9_000_001);
        assert_eq!(indexes.get_instruction_from_index(&at), Some(third));
        indexes.remove_machine_instr_from_maps(third);
        assert_eq!(indexes.get_instruction_from_index(&at), None);
    }
}
