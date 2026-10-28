//! An index of where each value is defined, read and required in a body, kept
//! current as the body is rewritten.
//!
//! # How it works
//! A spill or split rewrites only the instructions that name its values and
//! keeps every other `Arc<Insn>`. The spiller's scans look for a few values, so
//! they read an index of their occurrences instead of walking the body.
//! `Postings` holds, per value, the positions `(block, instruction)` of its
//! definitions (`defs`), reads (`uses`) and register requirements (`needs`), in
//! body order, and per block the positions of instructions with a frame cell
//! operand.
//!
//! `following(body, read)` hands `read` the postings of `body`. One copy lives
//! per thread, with the instruction lists it was made of. Asked of a body, it
//! compares each block's list with the held one by `Arc` identity. If the
//! blocks are the same (same positions, same instruction pointers) it answers
//! at once. If the body has another shape, as a different function does, it
//! makes the postings whole (`Postings::of`). Otherwise it redoes only the
//! blocks that differ (`replaced`):
//!
//! ```text
//!  block before: [a b c d]           block after: [a b' x d]
//!  identity compare:  = ≠ . =        changed instructions: b -> b', c -> x
//!  values named by a b c d b' x are "touched"; for each, the block's entries
//!  are cut out of its list and the new ones spliced in, in position order.
//! ```
//!
//! Occurrences are kept by slot, not by position: each block numbers its
//! instructions with ascending slots, `STRIDE` apart, and an entry names
//! `(block, slot)`. A rewrite that keeps most of a block (`aligned_insns` finds
//! the kept instructions by pointer) leaves their slots and entries alone, cuts
//! the entries of the instructions it removed and puts those of the ones it
//! added at slots between their neighbours', as LLVM's `SlotIndexes` numbers an
//! instruction put between two (and renumbers a block only when the room is
//! used up). The cost of an edit is the entries of the instructions it changed,
//! not those of every instruction after it. A position, where a reader wants
//! one, is where the slot is in the block's list of slots.
//!
//! A block the rewrite made over (little of it is the same, or the kept
//! instructions changed their order) is made again: the entries of the values
//! it names are cut out and the new ones spliced in.
//!
//! `LLRM_CHECK_POSTINGS` asserts after each update that the followed postings
//! equal ones worked out whole. `redone()` counts the blocks worked again, for
//! a test that a rewrite redoes only the blocks it changed.
//!
//! # Example
//! None: the index is plumbing that changes no instruction, so there is no LIR
//! to show before and after. Read `Postings::of` for what it holds.
//! # Used by
//! [`crate::backend::allocate`], [`crate::coalescer::pass`],
//! [`crate::backend::splitkit`], [`crate::backend::spiller`] and
//! [`crate::backend::storedhomes`].

use std::cell::RefCell;
use std::sync::Arc;

use crate::model::ir::{Loc, Space};
use crate::model::lir::{Block, Insn, Insns, LirBody};
use crate::support::hash::HashMap;

/// An instruction by its block's position in the body and its own in the block.
pub type At = (u32, u32);

/// An instruction by its block and its slot: the number it was given in the
/// block, which a rewrite of the rest of the block leaves.
#[derive(Clone, Debug)]
struct Entry {
    block: u32,
    slot: u32,
    /// The instruction itself, so that a reader that wants the instruction and
    /// not its place does not look the place up.
    insn: Arc<Insn>,
    /// Where the slot was in its block when last looked for: a guess to start
    /// from.
    hint: std::cell::Cell<u32>,
}

impl Entry {
    fn key(&self) -> (u32, u32) {
        (self.block, self.slot)
    }
}

/// The gap between the slots a block is first numbered with: an instruction put
/// between two takes the middle of the gap, so ten are put between two before
/// a block is numbered again.
const STRIDE: u32 = 1 << 10;

/// The instructions that define and read each value, in body order: one entry
/// for each time the instruction's `defines` or `uses` names it.
#[derive(Debug, Default)]
pub struct Postings {
    defs: HashMap<u32, Vec<Entry>>,
    uses: HashMap<u32, Vec<Entry>>,
    /// The instructions that `require` the value be in a register.
    needs: HashMap<u32, Vec<Entry>>,
    /// For each block, the slot of each of its instructions, ascending: the
    /// position of an instruction is where its slot is in the list.
    slots: Vec<Vec<u32>>,
    /// For each block, the slots of its instructions with a frame cell among
    /// their operands, ascending.
    frames: Vec<Vec<u32>>,
    /// The largest value `defs` or `uses` hold a key for, where known: kept as
    /// they change, found again by one pass only after the key it was is cut.
    largest: std::cell::Cell<Option<u32>>,
}

/// What `Postings` holds, by position: the equality of two sets of postings is
/// the equality of what they say, not of the numbers they said it with.
type Listing = (std::collections::BTreeMap<u32, Vec<At>>, [std::collections::BTreeMap<u32, Vec<At>>; 2], Vec<Vec<u32>>);

impl PartialEq for Postings {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.listing() == other.listing()
    }
}

impl Postings {
    /// Every occurrence in `body`, worked out whole.
    pub fn of(body: &LirBody) -> Self {
        let mut found = Self::default();
        for (block, one) in body.blocks.iter().enumerate() {
            found.add(block as u32, &one.insns);
        }
        found
    }

    fn position(
        &self,
        block: u32,
        slot: u32,
    ) -> u32 {
        Self::locate(&self.slots[block as usize], slot, (slot / STRIDE) as usize) as u32
    }

    /// Where `slot` is in `list`: a block is first numbered `STRIDE` apart, so
    /// the guess is the slot's place there, and it is looked for outward from
    /// it, which is a step or two where the block has been edited little.
    fn locate(
        list: &[u32],
        slot: u32,
        guess: usize,
    ) -> usize {
        let guess = guess.min(list.len().saturating_sub(1));
        if list.get(guess) == Some(&slot) {
            return guess;
        }
        let (low, high);
        if list.get(guess).is_some_and(|here| *here < slot) {
            let mut start = guess + 1;
            let mut step = 1;
            while start + step < list.len() && list[start + step] < slot {
                start += step;
                step *= 2;
            }
            low = start;
            high = (start + step + 1).min(list.len());
        } else {
            let mut end = guess;
            let mut step = 1;
            while end > step && list[end - step] >= slot {
                end -= step;
                step *= 2;
            }
            low = end.saturating_sub(step);
            high = (end + 1).min(list.len());
        }
        let found = low + list[low..high].partition_point(|one| *one < slot);
        debug_assert_eq!(list.get(found), Some(&slot), "a slot the block holds");
        found
    }

    /// The positions of `entries`, which are by block and slot: within a block
    /// each is looked for after the one before.
    fn walk<'a>(
        &'a self,
        entries: Option<&'a Vec<Entry>>,
    ) -> impl Iterator<Item = At> + 'a {
        entries
            .into_iter()
            .flatten()
            .map(
                move |entry| {
                    let at = Self::locate(&self.slots[entry.block as usize], entry.slot, entry.hint.get() as usize);
                    entry.hint.set(at as u32);
                    (entry.block, at as u32)
                },
            )
    }

    fn positions(
        &self,
        entries: Option<&Vec<Entry>>,
    ) -> Vec<At> {
        self.walk(entries).collect()
    }

    /// `defs`, as it is read: no list is made.
    pub fn defs_at(
        &self,
        value: u32,
    ) -> impl Iterator<Item = At> + '_ {
        self.walk(self.defs.get(&value))
    }

    pub fn uses_at(
        &self,
        value: u32,
    ) -> impl Iterator<Item = At> + '_ {
        self.walk(self.uses.get(&value))
    }

    pub fn needs_at(
        &self,
        value: u32,
    ) -> impl Iterator<Item = At> + '_ {
        self.walk(self.needs.get(&value))
    }

    /// The instructions that define `value`, with their blocks: no place is
    /// looked up.
    pub fn defs_insns(
        &self,
        value: u32,
    ) -> impl Iterator<Item = (u32, &Arc<Insn>)> + '_ {
        self.defs.get(&value).into_iter().flatten().map(|entry| (entry.block, &entry.insn))
    }

    pub fn uses_insns(
        &self,
        value: u32,
    ) -> impl Iterator<Item = (u32, &Arc<Insn>)> + '_ {
        self.uses.get(&value).into_iter().flatten().map(|entry| (entry.block, &entry.insn))
    }

    pub fn needs_insns(
        &self,
        value: u32,
    ) -> impl Iterator<Item = (u32, &Arc<Insn>)> + '_ {
        self.needs.get(&value).into_iter().flatten().map(|entry| (entry.block, &entry.insn))
    }

    /// The instructions that name `value` (and, if asked, require it in a
    /// register), each once, in body order, with their blocks.
    pub fn named_insns(
        &self,
        value: u32,
        with_needs: bool,
    ) -> Vec<(u32, &Arc<Insn>)> {
        let empty: &[Entry] = &[];
        let lists = [
            self.defs.get(&value).map_or(empty, Vec::as_slice),
            self.uses.get(&value).map_or(empty, Vec::as_slice),
            if with_needs { self.needs.get(&value).map_or(empty, Vec::as_slice) } else { empty },
        ];
        let mut at = [0usize; 3];
        let mut found: Vec<(u32, &Arc<Insn>)> = Vec::with_capacity(lists.iter().map(|list| list.len()).sum());
        let mut last: Option<(u32, u32)> = None;
        loop {
            // The earliest entry not yet taken among the three lists.
            let mut best: Option<usize> = None;
            for (which, list) in lists.iter().enumerate() {
                if let Some(entry) = list.get(at[which]) {
                    if best.is_none_or(|other| entry.key() < lists[other][at[other]].key()) {
                        best = Some(which);
                    }
                }
            }
            let Some(which) = best else { break };
            let entry = &lists[which][at[which]];
            at[which] += 1;
            if last != Some(entry.key()) {
                found.push((entry.block, &entry.insn));
                last = Some(entry.key());
            }
        }
        found
    }

    /// How many instructions define, read or require `value`.
    pub fn defs_count(
        &self,
        value: u32,
    ) -> usize {
        self.defs.get(&value).map_or(0, Vec::len)
    }

    pub fn uses_count(
        &self,
        value: u32,
    ) -> usize {
        self.uses.get(&value).map_or(0, Vec::len)
    }

    pub fn needs_count(
        &self,
        value: u32,
    ) -> usize {
        self.needs.get(&value).map_or(0, Vec::len)
    }

    pub fn defs(
        &self,
        value: u32,
    ) -> Vec<At> {
        self.positions(self.defs.get(&value))
    }

    pub fn uses(
        &self,
        value: u32,
    ) -> Vec<At> {
        self.positions(self.uses.get(&value))
    }

    pub fn needs(
        &self,
        value: u32,
    ) -> Vec<At> {
        self.positions(self.needs.get(&value))
    }

    /// The positions in block `block` of the instructions that read or write a
    /// frame cell.
    pub fn frames(
        &self,
        block: usize,
    ) -> Vec<u32> {
        self.frames
            .get(block)
            .map_or_else(Vec::new, |list| list.iter().map(|slot| self.position(block as u32, *slot)).collect())
    }

    /// The largest value an instruction defines or reads, or 0.
    pub fn largest(&self) -> u32 {
        if let Some(known) = self.largest.get() {
            return known;
        }
        let found = self.defs.keys().chain(self.uses.keys()).copied().max().unwrap_or(0);
        self.largest.set(Some(found));
        found
    }

    fn listing(&self) -> Listing {
        let by_position = |map: &HashMap<u32, Vec<Entry>>| -> std::collections::BTreeMap<u32, Vec<At>> {
            map.iter().map(|(value, list)| (*value, self.positions(Some(list)))).collect()
        };
        (
            by_position(&self.defs),
            [by_position(&self.uses), by_position(&self.needs)],
            (0..self.frames.len()).map(|block| self.frames(block)).collect(),
        )
    }

    /// The slots a block of `len` instructions is first numbered with.
    fn numbered(len: usize) -> Vec<u32> {
        (1..=len)
            .map(|at| u32::try_from(at as u64 * u64::from(STRIDE)).expect("a block of under four million instructions"))
            .collect()
    }

    /// The occurrences of the instructions of `insns`, block `block`, appended:
    /// that block is the last made.
    fn add(
        &mut self,
        block: u32,
        insns: &[Arc<Insn>],
    ) {
        if self.slots.len() <= block as usize {
            self.slots.resize(block as usize + 1, Vec::new());
            self.frames.resize(block as usize + 1, Vec::new());
        }
        let slots = Self::numbered(insns.len());
        self.frames[block as usize] =
            insns.iter().zip(&slots).filter(|(one, _)| is_framed(one)).map(|(_, slot)| *slot).collect();
        for (at, (one, slot)) in insns.iter().zip(&slots).enumerate() {
            let entry = || Entry { block, slot: *slot, insn: Arc::clone(one), hint: std::cell::Cell::new(at as u32) };
            for value in &one.defines {
                self.defs.entry(*value).or_default().push(entry());
                self.raise(*value);
            }
            for value in &one.uses {
                self.uses.entry(*value).or_default().push(entry());
                self.raise(*value);
            }
            for (held, _) in &one.requires {
                self.needs.entry(held.value).or_default().push(entry());
            }
        }
        self.slots[block as usize] = slots;
    }

    /// Block `block` as `old` was, as `new` is.
    fn replaced(
        &mut self,
        block: u32,
        old: &Insns,
        new: &Insns,
    ) {
        if let Some(found) = crate::analysis::intervals::aligned_insns(old, new) {
            if self.patched(block, old, new, &found) {
                return;
            }
        }
        self.rebuilt(block, old, new);
    }

    /// Block `block` made again: every value either list names has its entries
    /// of the block cut out and the new ones spliced in.
    fn rebuilt(
        &mut self,
        block: u32,
        old: &Insns,
        new: &Insns,
    ) {
        let mut touched: Vec<u32> = old
            .iter()
            .chain(new)
            .flat_map(|one| {
                one.defines.iter().chain(&one.uses).copied().chain(one.requires.iter().map(|(held, _)| held.value))
            })
            .collect();
        touched.sort_unstable();
        touched.dedup();
        let mut fresh = Postings::default();
        fresh.add(block, new);
        if self.slots.len() <= block as usize {
            self.slots.resize(block as usize + 1, Vec::new());
            self.frames.resize(block as usize + 1, Vec::new());
        }
        self.slots[block as usize] = std::mem::take(&mut fresh.slots[block as usize]);
        self.frames[block as usize] = std::mem::take(&mut fresh.frames[block as usize]);
        for value in touched {
            for (mine, theirs) in
                [(&mut self.defs, &fresh.defs), (&mut self.uses, &fresh.uses), (&mut self.needs, &fresh.needs)]
            {
                let list = mine.entry(value).or_default();
                let from = list.partition_point(|at| at.block < block);
                let to = list.partition_point(|at| at.block <= block);
                list.splice(from..to, theirs.get(&value).into_iter().flatten().cloned());
                if list.is_empty() {
                    mine.remove(&value);
                }
            }
            if self.defs.contains_key(&value) || self.uses.contains_key(&value) {
                self.raise(value);
            } else {
                self.lowered(value);
            }
        }
    }

    /// `value` has a key in `defs` or `uses`.
    fn raise(
        &self,
        value: u32,
    ) {
        if self.largest.get().is_some_and(|known| value > known) {
            self.largest.set(Some(value));
        }
    }

    /// `value` has no key in `defs` or `uses` any more: if it was the largest,
    /// which one is now is not known.
    fn lowered(
        &self,
        value: u32,
    ) {
        if self.largest.get() == Some(value) {
            self.largest.set(None);
        }
    }

    /// Block `block` as `found` says `new` differs from `old`: the entries of
    /// the instructions it kept stay where they are, whatever their positions
    /// became; those of the ones it removed are cut out and those of the ones
    /// it added put in at slots between their neighbours'. False, and
    /// nothing changed, where the kept instructions are not in the order
    /// they were or there is no room between two slots.
    fn patched(
        &mut self,
        block: u32,
        old: &[Arc<Insn>],
        new: &[Arc<Insn>],
        found: &crate::analysis::intervals::Aligned,
    ) -> bool {
        let Some(old_slots) = self.slots.get(block as usize) else { return false };
        if old_slots.len() != found.len_old || old.len() != found.len_old {
            return false;
        }
        if !found.runs.windows(2).all(|pair| pair[0].1 + pair[0].2 <= pair[1].1) {
            return false;
        }
        let mut new_slots = vec![0u32; new.len()];
        let mut kept = vec![false; new.len()];
        for &(from, now, len) in &found.runs {
            for k in 0..len {
                new_slots[now + k] = old_slots[from + k];
                kept[now + k] = true;
            }
        }
        let mut at = 0;
        while at < new.len() {
            if kept[at] {
                at += 1;
                continue;
            }
            let start = at;
            while at < new.len() && !kept[at] {
                at += 1;
            }
            let count = (at - start) as u64;
            let low = if start == 0 { 0 } else { u64::from(new_slots[start - 1]) };
            let high = if at == new.len() { low + (count + 1) * u64::from(STRIDE) } else { u64::from(new_slots[at]) };
            let step = (high - low) / (count + 1);
            if step == 0 || high > u64::from(u32::MAX) {
                return false;
            }
            for k in 0..count {
                new_slots[start + k as usize] = (low + step * (k + 1)) as u32;
            }
        }
        for &position in &found.gone {
            self.remove_entries(block, old_slots_at(&self.slots, block, position), &old[position]);
        }
        for &position in &found.added {
            self.insert_entries(block, new_slots[position], position, &new[position]);
        }
        let frames = &mut self.frames[block as usize];
        for &position in &found.gone {
            if is_framed(&old[position]) {
                let slot = self.slots[block as usize][position];
                if let Ok(at) = frames.binary_search(&slot) {
                    frames.remove(at);
                }
            }
        }
        for &position in &found.added {
            if is_framed(&new[position]) {
                let at = frames.partition_point(|slot| *slot < new_slots[position]);
                frames.insert(at, new_slots[position]);
            }
        }
        self.slots[block as usize] = new_slots;
        true
    }

    fn remove_entries(
        &mut self,
        block: u32,
        slot: u32,
        one: &Insn,
    ) {
        let entry = (block, slot);
        let cut = |map: &mut HashMap<u32, Vec<Entry>>, value: u32| {
            if let Some(list) = map.get_mut(&value) {
                if let Ok(at) = list.binary_search_by_key(&entry, Entry::key) {
                    list.remove(at);
                }
                if list.is_empty() {
                    map.remove(&value);
                }
            }
        };
        for value in &one.defines {
            cut(&mut self.defs, *value);
        }
        for value in &one.uses {
            cut(&mut self.uses, *value);
        }
        for value in one.defines.iter().chain(&one.uses) {
            if !self.defs.contains_key(value) && !self.uses.contains_key(value) {
                self.lowered(*value);
            }
        }
        for (held, _) in &one.requires {
            cut(&mut self.needs, held.value);
        }
    }

    fn insert_entries(
        &mut self,
        block: u32,
        slot: u32,
        position: usize,
        one: &Arc<Insn>,
    ) {
        let put = |map: &mut HashMap<u32, Vec<Entry>>, value: u32| {
            let list = map.entry(value).or_default();
            let at = list.partition_point(|held| held.key() < (block, slot));
            list.insert(at, Entry { block, slot, insn: Arc::clone(one), hint: std::cell::Cell::new(position as u32) });
        };
        for value in &one.defines {
            put(&mut self.defs, *value);
            self.raise(*value);
        }
        for value in &one.uses {
            put(&mut self.uses, *value);
            self.raise(*value);
        }
        for (held, _) in &one.requires {
            put(&mut self.needs, held.value);
        }
    }
}

fn old_slots_at(
    slots: &[Vec<u32>],
    block: u32,
    position: usize,
) -> u32 {
    slots[block as usize][position]
}

/// Whether `one` has a frame cell among its operands.
fn is_framed(one: &Insn) -> bool {
    one.what
        .as_ref()
        .is_some_and(
            |what| what.dests
                .iter()
                .chain(&what.sources)
                .any(
                    |operand| matches!(
                        operand,
                        Loc::Mem(cell) if cell.addr.is_some_and(|addr| addr.space == Space::Frame)
                    ),
                ),
        )
}

/// The body the postings were made of, held so that an instruction's address is
/// not reused while it is compared with.
struct Followed {
    blocks: Vec<(i64, Insns)>,
    /// The blocks of the body the postings were last made current for, as the
    /// allocations they were: a body whose blocks all are these needs no look
    /// at what is in them.
    seen: Vec<Block>,
    found: Postings,
}

thread_local! {
    static CURRENT: RefCell<Option<Followed>> = const { RefCell::new(None) };
    static REDONE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    #[cfg(test)]
    static LOOKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many blocks `following` has looked into to tell they were the ones the
/// postings were made of, for a test that asking again of one body looks at
/// none.
#[cfg(test)]
pub fn looked() -> usize {
    LOOKED.with(std::cell::Cell::get)
}

/// How many blocks this thread has worked the occurrences of again, for a test
/// that a rewrite redoes only the blocks it changed.
pub fn redone() -> usize {
    REDONE.with(std::cell::Cell::get)
}

fn check() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| llrm_support::env_flag!("LLRM_CHECK_POSTINGS"))
}

/// Whether `body` has the very blocks the postings were last made current for.
fn same_blocks(
    held: &Followed,
    body: &LirBody,
) -> bool {
    held.seen.len() == body.blocks.len() && held.seen.iter().zip(&body.blocks).all(|(was, now)| was.same_as(now))
}

/// Whether the postings already are those of `body`: its blocks, each of the
/// same instructions.
fn current(
    held: &Followed,
    body: &LirBody,
) -> bool {
    held.blocks.len() == body.blocks.len()
        && held.blocks.iter().zip(&body.blocks).all(|((at, before), block)| {
            #[cfg(test)]
            LOOKED.with(|looked| looked.set(looked.get() + 1));
            *at == block.at
                && (before.same_as(&block.insns)
                    || before.len() == block.insns.len()
                        && before.iter().zip(&block.insns).all(|(old, new)| Arc::ptr_eq(old, new)))
        })
}

/// `read` of the postings of `body`, made to follow it first. A read may ask
/// again of the same body.
pub fn following<R>(
    body: &LirBody,
    read: impl FnOnce(&Postings) -> R,
) -> R {
    let up_to_date = CURRENT.with(|state| {
        let remade = {
            let guard = state.borrow();
            let Some(held) = guard.as_ref() else { return false };
            if same_blocks(held, body) {
                return true;
            }
            if !current(held, body) {
                return false;
            }
            true
        };
        // Blocks made again of the same instructions: noted, so that the next
        // ask of this body is told by the blocks alone (a read that asks again
        // of the same body has the state borrowed: it notes nothing).
        if remade && let Ok(mut state) = state.try_borrow_mut() {
            state.as_mut().expect("held").seen = body.blocks.clone();
        }
        remade
    });
    if !up_to_date {
        CURRENT.with(|state| {
            let mut state = state.borrow_mut();
            let same_shape = state
                .as_ref()
                .is_some_and(
                    |held| held.blocks.len() == body.blocks.len()
                        && held.blocks.iter().zip(&body.blocks).all(|((at, _), block)| *at == block.at),
                );
            if !same_shape {
                REDONE.with(|redone| redone.set(redone.get() + body.blocks.len()));
                *state = Some(Followed {
                    blocks: body.blocks.iter().map(|block| (block.at, block.insns.clone())).collect(),
                    seen: body.blocks.clone(),
                    found: Postings::of(body),
                });
            } else {
                let held = state.as_mut().expect("checked");
                for (block, (one, (_, before))) in body.blocks.iter().zip(&mut held.blocks).enumerate() {
                    let unchanged = before.same_as(&one.insns)
                        || before.len() == one.insns.len()
                            && before.iter().zip(&one.insns).all(|(old, new)| Arc::ptr_eq(old, new));
                    if unchanged {
                        continue;
                    }
                    REDONE.with(|redone| redone.set(redone.get() + 1));
                    held.found.replaced(block as u32, before, &one.insns);
                    *before = one.insns.clone();
                }
                held.seen = body.blocks.clone();
            }
            if check() {
                let held = state.as_ref().expect("made");
                assert!(
                    held.found == Postings::of(body),
                    "{}: the postings that followed the body differ from working them out",
                    body.name
                );
            }
        });
    }
    CURRENT.with(|state| read(&state.borrow().as_ref().expect("made").found))
}

/// `LirBody::next_value`, from the postings the allocator follows the body
/// with: the largest value they hold and the phis' results, not a walk of
/// every instruction.
pub(crate) fn next_value_following(body: &LirBody) -> u32 {
    let phis = body.blocks.iter().flat_map(|block| block.phis.iter().map(|phi| phi.result)).max().unwrap_or(0);
    let found = following(body, |postings| postings.largest()).max(phis) + 1;
    if llrm_support::env_flag!("LLRM_CHECK_POSTINGS") {
        assert_eq!(found, body.next_value(), "{}: the next value from the postings differs from the walk", body.name);
    }
    found
}
