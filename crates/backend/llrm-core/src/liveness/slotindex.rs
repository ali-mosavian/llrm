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
//! DEVIATE (container, no number): `IndexListEntry` is an `Rc` node with a
//! strong `next` and a weak `prev` instead of an intrusive `simple_ilist`, and
//! an instruction is known by `Insn::id()` instead of a `MachineInstr*`. A
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
use std::collections::HashMap;
use std::fmt;
use std::rc::{Rc, Weak};

use crate::model::lir::LirBody;

/// A stand-in for `MachineInstr*`: `Insn::id()`.
pub type InstrKey = u64;

/// `IndexListEntry` (SlotIndexes.h#L45): an entry in the slot index list.
pub struct IndexListEntry {
    instr: Cell<Option<InstrKey>>,
    index: Cell<u32>,
    prev: RefCell<Weak<IndexListEntry>>,
    next: RefCell<Option<Rc<IndexListEntry>>>,
}

impl IndexListEntry {
    fn new(
        instr: Option<InstrKey>,
        index: u32,
    ) -> Rc<Self> {
        Rc::new(Self {
            instr: Cell::new(instr),
            index: Cell::new(index),
            prev: RefCell::new(Weak::new()),
            next: RefCell::new(None),
        })
    }

    pub fn instr(&self) -> Option<InstrKey> {
        self.instr.get()
    }

    pub fn index(&self) -> u32 {
        self.index.get()
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

/// `SlotIndex` (SlotIndexes.h#L74): an opaque wrapper around machine indexes.
#[derive(Clone, Default)]
pub struct SlotIndex {
    lie: Option<Rc<IndexListEntry>>,
    slot: u8,
}

impl SlotIndex {
    /// The default distance between instructions (`InstrDist`).
    pub const INSTR_DIST: u32 = 4 * SLOT_COUNT;

    fn at(
        entry: &Rc<IndexListEntry>,
        slot: Slot,
    ) -> Self {
        Self { lie: Some(Rc::clone(entry)), slot: slot as u8 }
    }

    fn list_entry(&self) -> &Rc<IndexListEntry> {
        self.lie.as_ref().expect("Attempt to compare reserved index.")
    }

    /// The number: the entry's index with the slot in the low bits.
    pub fn get_index(&self) -> u32 {
        self.list_entry().index.get() | u32::from(self.slot)
    }

    pub fn get_slot(&self) -> Slot {
        Slot::from(self.slot)
    }

    /// A new index at `slot` of the instruction `li` points to.
    pub fn with_slot(
        li: &SlotIndex,
        slot: Slot,
    ) -> Self {
        Self::at(li.list_entry(), slot)
    }

    pub fn is_valid(&self) -> bool {
        self.lie.is_some()
    }

    pub fn is_same_instr(
        a: &SlotIndex,
        b: &SlotIndex,
    ) -> bool {
        Rc::ptr_eq(a.list_entry(), b.list_entry())
    }

    pub fn is_earlier_instr(
        a: &SlotIndex,
        b: &SlotIndex,
    ) -> bool {
        a.list_entry().index.get() < b.list_entry().index.get()
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
        (other.list_entry().index.get() as i32 - self.list_entry().index.get() as i32) / SLOT_COUNT as i32
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
        Self::at(self.list_entry(), Slot::Block)
    }

    pub fn get_boundary_index(&self) -> SlotIndex {
        Self::at(self.list_entry(), Slot::Dead)
    }

    pub fn get_reg_slot(
        &self,
        early_clobber: bool,
    ) -> SlotIndex {
        Self::at(self.list_entry(), if early_clobber { Slot::EarlyClobber } else { Slot::Register })
    }

    pub fn get_dead_slot(&self) -> SlotIndex {
        Self::at(self.list_entry(), Slot::Dead)
    }

    fn next_entry(&self) -> Rc<IndexListEntry> {
        self.list_entry().next.borrow().clone().expect("no entry after the last one")
    }

    fn prev_entry(&self) -> Rc<IndexListEntry> {
        self.list_entry().prev.borrow().upgrade().expect("no entry before the first one")
    }

    /// `getNextSlot`: the next slot of this instruction or the first slot of
    /// the next one.
    pub fn get_next_slot(&self) -> SlotIndex {
        match self.get_slot() {
            Slot::Dead => Self::at(&self.next_entry(), Slot::Block),
            s => Self::at(self.list_entry(), Slot::from(s as u8 + 1)),
        }
    }

    pub fn get_next_index(&self) -> SlotIndex {
        Self::at(&self.next_entry(), self.get_slot())
    }

    /// `getPrevSlot`: the previous slot of this instruction or the last slot of
    /// the previous one.
    pub fn get_prev_slot(&self) -> SlotIndex {
        match self.get_slot() {
            Slot::Block => Self::at(&self.prev_entry(), Slot::Dead),
            s => Self::at(self.list_entry(), Slot::from(s as u8 - 1)),
        }
    }

    pub fn get_prev_index(&self) -> SlotIndex {
        Self::at(&self.prev_entry(), self.get_slot())
    }
}

impl PartialEq for SlotIndex {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        match (&self.lie, &other.lie) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b) && self.slot == other.slot,
            (None, None) => true,
            _ => false,
        }
    }
}

impl Eq for SlotIndex {}

impl PartialOrd for SlotIndex {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SlotIndex {
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
        self.lie.as_ref().map(|entry| Rc::as_ptr(entry) as usize).hash(state);
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
            write!(f, "{}{}", self.list_entry().index.get(), ["B", "e", "r", "d"][self.slot as usize])
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
    first: Option<Rc<IndexListEntry>>,
    last: Option<Rc<IndexListEntry>>,
    mi2i_map: HashMap<InstrKey, SlotIndex>,
    /// `MBBRanges`: block number to (start, stop).
    mbb_ranges: Vec<(SlotIndex, SlotIndex)>,
    /// `idx2MBBMap`: sorted pairs of the index of a block's start and the
    /// block.
    idx2mbb_map: Vec<(SlotIndex, usize)>,
    local_renumberings: usize,
}

impl Drop for SlotIndexes {
    fn drop(&mut self) {
        self.clear();
    }
}

impl SlotIndexes {
    /// `SlotIndexes(MachineFunction&)` / `analyze` (SlotIndexes.cpp#L66): the
    /// numbering of `body`, a block's start sharing the entry that ends the
    /// block before it and one blank entry between blocks.
    pub fn new(body: &LirBody) -> Self {
        let mut found = Self::default();
        found.analyze(body);
        found
    }

    fn push_back(
        &mut self,
        entry: Rc<IndexListEntry>,
    ) {
        match self.last.take() {
            Some(last) => {
                *entry.prev.borrow_mut() = Rc::downgrade(&last);
                *last.next.borrow_mut() = Some(Rc::clone(&entry));
            }
            None => self.first = Some(Rc::clone(&entry)),
        }
        self.last = Some(entry);
    }

    /// `clear`: breaks the chain iteratively (a long list dropped through its
    /// `next` links would recurse once per entry).
    pub fn clear(&mut self) {
        self.mi2i_map.clear();
        self.mbb_ranges.clear();
        self.idx2mbb_map.clear();
        self.last = None;
        let mut at = self.first.take();
        while let Some(entry) = at {
            at = entry.next.borrow_mut().take();
        }
    }

    pub fn analyze(
        &mut self,
        body: &LirBody,
    ) {
        assert!(self.first.is_none(), "Index list non-empty at initial numbering?");
        let mut index = 0u32;
        self.mbb_ranges = vec![(SlotIndex::default(), SlotIndex::default()); body.blocks.len()];
        self.push_back(IndexListEntry::new(None, index));
        for (number, block) in body.blocks.iter().enumerate() {
            let block_start_index = SlotIndex::at(self.last.as_ref().expect("an entry"), Slot::Block);
            for one in block.insns.iter() {
                index += SlotIndex::INSTR_DIST;
                self.push_back(IndexListEntry::new(Some(one.id()), index));
                let at = SlotIndex::at(self.last.as_ref().expect("an entry"), Slot::Block);
                self.mi2i_map.insert(one.id(), at);
            }
            // We insert one blank instructions between basic blocks.
            index += SlotIndex::INSTR_DIST;
            self.push_back(IndexListEntry::new(None, index));
            self.mbb_ranges[number].0 = block_start_index.clone();
            self.mbb_ranges[number].1 = SlotIndex::at(self.last.as_ref().expect("an entry"), Slot::Block);
            self.idx2mbb_map.push((block_start_index, number));
        }
        self.idx2mbb_map.sort_by(|a, b| a.0.cmp(&b.0));
    }

    /// `getZeroIndex`.
    pub fn get_zero_index(&self) -> SlotIndex {
        let first = self.first.as_ref().expect("an index list");
        assert_eq!(first.index.get(), 0, "First index is not 0?");
        SlotIndex::at(first, Slot::Block)
    }

    /// `getLastIndex`: the base index of the last slot.
    pub fn get_last_index(&self) -> SlotIndex {
        SlotIndex::at(self.last.as_ref().expect("an index list"), Slot::Block)
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
        self.mi2i_map.get(&instr).expect("Instruction not found in maps.").clone()
    }

    /// `getInstructionFromIndex`: the instruction at `index`, or None.
    pub fn get_instruction_from_index(
        &self,
        index: &SlotIndex,
    ) -> Option<InstrKey> {
        index.list_entry().instr()
    }

    /// `getNextNonNullIndex`.
    pub fn get_next_non_null_index(
        &self,
        index: &SlotIndex,
    ) -> SlotIndex {
        let mut at = index.list_entry().next.borrow().clone();
        while let Some(entry) = at {
            if entry.instr().is_some() {
                return SlotIndex::at(&entry, index.get_slot());
            }
            at = entry.next.borrow().clone();
        }
        // We reached the end of the function.
        self.get_last_index()
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
                return found.clone();
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
                return found.clone();
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
        self.mbb_ranges[block].0.clone()
    }

    pub fn get_mbb_end_idx(
        &self,
        block: usize,
    ) -> SlotIndex {
        self.mbb_ranges[block].1.clone()
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

    /// `getMBBFromIndex`: the block `index` falls in. `block_of` gives the
    /// block of an instruction (an entry with an instruction is inside its
    /// block).
    pub fn get_mbb_from_index(
        &self,
        index: &SlotIndex,
        block_of: &dyn Fn(InstrKey) -> usize,
    ) -> usize {
        if let Some(instr) = self.get_instruction_from_index(index) {
            return block_of(instr);
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
        prev: &SlotIndex,
        next: &SlotIndex,
        late: bool,
    ) -> SlotIndex {
        assert!(!self.mi2i_map.contains_key(&instr), "Instr already indexed.");
        let (prev_entry, next_entry) = if late {
            // Insert MI's index immediately before the following instruction.
            let next_entry = Rc::clone(next.list_entry());
            let prev_entry = next_entry.prev.borrow().upgrade().expect("an entry before");
            (prev_entry, next_entry)
        } else {
            // Insert MI's index immediately after the preceding instruction.
            let prev_entry = Rc::clone(prev.list_entry());
            let next_entry = prev_entry.next.borrow().clone().expect("an entry after");
            (prev_entry, next_entry)
        };
        // Get a number for the new instr, or 0 if there's no room currently.
        let dist = ((next_entry.index.get() - prev_entry.index.get()) / 2) & !3u32;
        let new_number = prev_entry.index.get() + dist;
        let entry = IndexListEntry::new(Some(instr), new_number);
        *entry.prev.borrow_mut() = Rc::downgrade(&prev_entry);
        *entry.next.borrow_mut() = Some(Rc::clone(&next_entry));
        *next_entry.prev.borrow_mut() = Rc::downgrade(&entry);
        *prev_entry.next.borrow_mut() = Some(Rc::clone(&entry));
        // Renumber locally if we need to.
        if dist == 0 {
            self.renumber_indexes(&entry);
        }
        let new_index = SlotIndex::at(&entry, Slot::Block);
        self.mi2i_map.insert(instr, new_index.clone());
        new_index
    }

    /// `removeMachineInstrFromMaps` (SlotIndexes.cpp#L113): the entry stays,
    /// with no instruction.
    pub fn remove_machine_instr_from_maps(
        &mut self,
        instr: InstrKey,
    ) {
        let Some(index) = self.mi2i_map.remove(&instr) else { return };
        let entry = index.list_entry();
        assert_eq!(entry.instr(), Some(instr), "Instruction indexes broken.");
        // FIXME (LLVM): Eventually we want to actually delete these indexes.
        entry.instr.set(None);
    }

    /// `replaceMachineInstrInMaps`: `new` takes the index of `old`.
    pub fn replace_machine_instr_in_maps(
        &mut self,
        old: InstrKey,
        new: InstrKey,
    ) -> SlotIndex {
        let Some(base) = self.mi2i_map.remove(&old) else { return SlotIndex::default() };
        assert_eq!(base.list_entry().instr(), Some(old), "Mismatched instruction in index tables.");
        base.list_entry().instr.set(Some(new));
        self.mi2i_map.insert(new, base.clone());
        base
    }

    /// `renumberIndexes` (SlotIndexes.cpp#L196): renumbers locally after the
    /// entry `cur` was inserted but failed to get a new index.
    fn renumber_indexes(
        &mut self,
        cur: &Rc<IndexListEntry>,
    ) {
        // Number indexes with half the default spacing so we can catch up
        // quickly.
        const SPACE: u32 = SlotIndex::INSTR_DIST / 2;
        let start = cur.prev.borrow().upgrade().expect("an entry before");
        let mut index = start.index.get();
        let begin_index = index;
        let mut at = Some(Rc::clone(cur));
        while let Some(entry) = at {
            index += SPACE;
            entry.index.set(index);
            at = entry.next.borrow().clone();
            // If the next index is bigger, we have caught up.
            match &at {
                Some(next) if next.index.get() <= index => {}
                _ => break,
            }
        }
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
        let mut at = self.first.clone();
        let mut number = 0u32;
        while let Some(entry) = at {
            entry.index.set(number * SlotIndex::INSTR_DIST);
            number += 1;
            at = entry.next.borrow().clone();
        }
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
        let start_entry = IndexListEntry::new(None, 0);
        let end_entry = Rc::clone(self.mbb_ranges[block - 1].1.list_entry());
        let ins_entry = match first_instr {
            Some(instr) => Rc::clone(self.get_instruction_index(instr).list_entry()),
            None => Rc::clone(&end_entry),
        };
        // indexList.insert(insEntry, startEntry): before insEntry.
        let before = ins_entry.prev.borrow().upgrade().expect("an entry before");
        *start_entry.prev.borrow_mut() = Rc::downgrade(&before);
        *start_entry.next.borrow_mut() = Some(Rc::clone(&ins_entry));
        *ins_entry.prev.borrow_mut() = Rc::downgrade(&start_entry);
        *before.next.borrow_mut() = Some(Rc::clone(&start_entry));
        let start_idx = SlotIndex::at(&start_entry, Slot::Block);
        let end_idx = SlotIndex::at(&end_entry, Slot::Block);
        self.mbb_ranges[block - 1].1 = start_idx.clone();
        assert_eq!(block, self.mbb_ranges.len(), "Blocks must be added in order");
        self.mbb_ranges.push((start_idx.clone(), end_idx));
        self.idx2mbb_map.push((start_idx, block));
        self.renumber_indexes(&start_entry);
        self.idx2mbb_map.sort_by(|a, b| a.0.cmp(&b.0));
    }

    /// `removeMBBFromMaps`: gives the last block's range to its predecessor.
    pub fn remove_mbb_from_maps(
        &mut self,
        block: usize,
    ) {
        assert!(block != 0, "Can't remove the first block of a function.");
        let (start, end) = self.mbb_ranges[block].clone();
        // Give MBB's slot range to its layout predecessor so blocks stay
        // contiguous.
        self.mbb_ranges[block - 1].1 = end;
        // Drop MBB's index -> MBB entry.
        let at = self.get_mbb_lower_bound(&start);
        assert!(at < self.idx2mbb_map.len() && self.idx2mbb_map[at].0 == start && self.idx2mbb_map[at].1 == block);
        self.idx2mbb_map.remove(at);
        // Clear the block-start boundary entry.
        start.list_entry().instr.set(None);
    }

    /// How many local renumberings this has done (`NumLocalRenum`).
    pub fn local_renumberings(&self) -> usize {
        self.local_renumberings
    }

    /// `print`: each entry's number and whether it is an instruction.
    pub fn print(&self) -> String {
        let mut out = String::new();
        let mut at = self.first.clone();
        while let Some(entry) = at {
            out.push_str(&format!(
                "{} {}\n",
                entry.index.get(),
                entry.instr().map_or("-".to_owned(), |one| one.to_string())
            ));
            at = entry.next.borrow().clone();
        }
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
        let mut previous = a.clone();
        let mut made = Vec::new();
        for n in 0..40u64 {
            let key = 1_000_000 + n;
            // each goes immediately before `second`
            let at = indexes.insert_machine_instr_in_maps(key, &previous, &b, true);
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
        assert_eq!(
            indexes.get_mbb_from_index(&blank_between, &|_| unreachable!("a blank index names no instruction")),
            1
        );
        let last_of_first = indexes.get_mbb_last_idx(0);
        assert_eq!(
            indexes.get_mbb_from_index(&last_of_first, &|_| 0),
            0,
            "the dead slot of an instruction is in its block"
        );
        let in_second = indexes.get_instruction_index(body.blocks[1].insns[0].id());
        assert_eq!(indexes.get_mbb_from_index(&in_second, &|_| 1), 1);
    }
}
