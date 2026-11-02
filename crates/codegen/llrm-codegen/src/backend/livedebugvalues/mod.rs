//! `LiveDebugValues` (`VarLocBasedLDV`): where each user variable is over the
//! final code, found by following the locations the notes give through the
//! instructions that write registers.
//!
//! The ranges of a variable open at a note, close at the variable's next note
//! (or one of an overlapping piece), when an instruction writes the register
//! it is in, when a call clobbers it, and at the end of a block. Each closed
//! range is reported with the positions it spans.
//!
//! A copy into a callee-saved register whose source dies, a spill of a
//! register and a reload of a spill slot move a variable's range to the new
//! place (`transferRegisterCopy`, `transferSpillOrRestoreInst`), which ends
//! at the old one.
//!
//! DEVIATE: the pin emits `DBG_VALUE` instructions where a range opens; this
//! reports the closed ranges (`Located`) at slot positions the caller numbers.
//! The pin's first-candidate-only rule picks by `VarLocMap` id; here by the
//! order the ranges opened. The block join and entry values are later
//! slices.
//! O(1) per event plus the open ranges of the variable or register it touches.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use llrm_lir::registers::RegId;

use super::livedebugvariables::{Located, Place, Where};
use crate::liveness::liveintervals::LiveIntervals;
use crate::model::lir::LirBody;
use crate::support::hash::{HashMap, IndexMap};

/// A variable, or a byte piece of it (`DebugVariable`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Var {
    pub variable: u32,
    pub piece: Option<(u32, u32)>,
}

impl Var {
    /// Whether the two cover a byte in common: a piece that is all of the
    /// variable overlaps every piece of it (`OverlappingFragments`).
    fn overlaps(
        self,
        other: Var,
    ) -> bool {
        self.variable == other.variable
            && match (self.piece, other.piece) {
                (Some((from, size)), Some((other_from, other_size))) => {
                    from < other_from + other_size && other_from < from + size
                }
                _ => true,
            }
    }
}

/// What an instruction does to the registers.
pub struct Clobber<'a> {
    /// The registers it writes, with their aliases.
    pub written: &'a [RegId],
    /// Whether a call or other regmask clobbers a register.
    pub mask: Option<&'a dyn Fn(RegId) -> bool>,
}

/// The registers a location lives in.
fn registers(at: Where) -> Option<RegId> {
    match at {
        Where::Place(Place::Register(register)) | Where::Offset(Place::Register(register), _) => Some(register),
        _ => None,
    }
}

/// The ranges open at the current position, and those closed so far.
#[derive(Default)]
pub struct Tracker {
    open: BTreeMap<Var, (Where, i64, u64)>,
    opened: u64,
    closed: Vec<Located>,
}

impl Tracker {
    fn close(
        &mut self,
        var: Var,
        at: i64,
    ) {
        if let Some((place, start, _)) = self.open.remove(&var)
            && start < at
        {
            self.closed.push(Located { variable: var.variable, piece: var.piece, start, end: at, at: place });
        }
    }

    /// A note at `at` says `var` is `place` (`transferDebugValue`); none says
    /// it is nowhere. The variable's earlier ranges, and those of the pieces
    /// that overlap it, end here.
    pub fn debug_value(
        &mut self,
        at: i64,
        var: Var,
        place: Option<Where>,
    ) {
        let overlapping: Vec<Var> = self.open.keys().copied().filter(|open| open.overlaps(var)).collect();
        for open in overlapping {
            self.close(open, at);
        }
        if let Some(place) = place.filter(|place| *place != Where::Undef) {
            self.opened += 1;
            self.open.insert(var, (place, at, self.opened));
        }
    }

    /// An instruction at `at` writes registers or clobbers some
    /// (`transferRegisterDef`): the variables in them are nowhere from here.
    /// The stack pointer survives a mask, as the pin assumes of calls.
    pub fn register_def(
        &mut self,
        at: i64,
        clobber: &Clobber<'_>,
        stack_pointer: RegId,
    ) {
        let killed: Vec<Var> = self
            .open
            .iter()
            .filter(|(_, (place, ..))| {
                registers(*place).is_some_and(|register| {
                    clobber.written.contains(&register)
                        || (register != stack_pointer && clobber.mask.is_some_and(|mask| mask(register)))
                })
            })
            .map(|(var, _)| *var)
            .collect();
        for var in killed {
            self.close(var, at);
        }
    }

    /// The variable open at `place` that opened first (the pin's lowest
    /// `VarLocMap` id), if any.
    fn first_in(
        &self,
        place: impl Fn(Where) -> bool,
    ) -> Option<Var> {
        self.open.iter().filter(|(_, (at, ..))| place(*at)).min_by_key(|(_, (.., seq))| *seq).map(|(var, _)| *var)
    }

    /// Moves `var`'s range to `to` from `at` on: the old one ends here.
    fn transfer(
        &mut self,
        at: i64,
        var: Var,
        to: Where,
    ) {
        self.close(var, at);
        self.debug_value(at, var, Some(to));
    }

    /// A copy at `at` into `dest` from `src` (`transferRegisterCopy`): only
    /// into a callee-saved register, and only when `src` dies, the first
    /// variable in `src` goes with it.
    pub fn register_copy(
        &mut self,
        at: i64,
        dest: RegId,
        src: RegId,
        src_dies: bool,
        dest_callee_saved: bool,
    ) {
        if !dest_callee_saved || !src_dies {
            return;
        }
        let Some(var) = self.first_in(|place| registers(place) == Some(src)) else { return };
        let moved = match self.open[&var].0 {
            Where::Offset(_, add) => Where::Offset(Place::Register(dest), add),
            _ => Where::Place(Place::Register(dest)),
        };
        self.transfer(at, var, moved);
    }

    /// A store at `at` of `from` into the spill cell `slot`
    /// (`transferSpillOrRestoreInst`): what was in the cell is gone, and the
    /// first variable in `from` is in the cell from here.
    pub fn spill(
        &mut self,
        at: i64,
        from: RegId,
        slot: i64,
    ) {
        let overwritten: Vec<Var> = self
            .open
            .iter()
            .filter(|(_, (place, ..))| *place == Where::Place(Place::Cell(slot)))
            .map(|(var, _)| *var)
            .collect();
        for var in overwritten {
            self.close(var, at);
        }
        let Some(var) = self.first_in(|place| registers(place) == Some(from)) else { return };
        self.transfer(at, var, Where::Place(Place::Cell(slot)));
    }

    /// A load at `at` of the spill cell `slot` into `into`: the first variable
    /// in the cell is in the register from here.
    pub fn reload(
        &mut self,
        at: i64,
        slot: i64,
        into: RegId,
    ) {
        let Some(var) = self.first_in(|place| place == Where::Place(Place::Cell(slot))) else { return };
        self.transfer(at, var, Where::Place(Place::Register(into)));
    }

    /// The end of a block at `at` (`transferTerminator`): every range ends.
    pub fn terminator(
        &mut self,
        at: i64,
    ) {
        let open: Vec<Var> = self.open.keys().copied().collect();
        for var in open {
            self.close(var, at);
        }
    }

    /// A tracker for a block at `start` that begins with `ins` open
    /// (`InLocs`): their ranges start at the block's first position.
    pub fn entering(
        start: i64,
        ins: &BTreeMap<Var, Where>,
    ) -> Tracker {
        let mut tracker = Tracker::default();
        for (&var, &place) in ins {
            tracker.debug_value(start, var, Some(place));
        }
        tracker
    }

    /// What is open now: the block's `OutLocs` once its last instruction ran.
    pub fn open_locs(&self) -> BTreeMap<Var, Where> {
        self.open.iter().map(|(var, (place, ..))| (*var, *place)).collect()
    }

    /// The ranges closed so far, in the order they closed.
    pub fn finish(self) -> Vec<Located> {
        self.closed
    }
}

/// What happens at a position of a block.
#[derive(Clone)]
pub enum Event {
    /// A note: the variable is `place`, or nowhere.
    Note(Var, Option<Where>),
    /// An instruction writes `written` and a call's mask clobbers what it
    /// names.
    Def {
        written: Vec<RegId>,
        mask: Option<Rc<dyn Fn(RegId) -> bool>>,
    },
    Copy {
        dest: RegId,
        src: RegId,
        src_dies: bool,
        dest_callee_saved: bool,
    },
    Spill {
        from: RegId,
        slot: i64,
    },
    Reload {
        slot: i64,
        into: RegId,
    },
}

/// A block: where it starts and ends, its predecessors (indexes into the
/// list, which is in reverse post-order) and what happens in it.
pub struct FlowBlock {
    pub start: i64,
    pub end: i64,
    pub preds: Vec<usize>,
    pub events: Vec<(i64, Event)>,
}

fn run(
    block: &FlowBlock,
    ins: &BTreeMap<Var, Where>,
    stack_pointer: RegId,
) -> Tracker {
    let mut tracker = Tracker::entering(block.start, ins);
    for (at, event) in &block.events {
        match event {
            Event::Note(var, place) => tracker.debug_value(*at, *var, *place),
            Event::Def { written, mask } => {
                let mask = mask.as_ref().map(|mask| move |register: RegId| mask(register));
                let clobber = Clobber { written, mask: mask.as_ref().map(|mask| mask as &dyn Fn(RegId) -> bool) };
                tracker.register_def(*at, &clobber, stack_pointer);
            }
            Event::Copy { dest, src, src_dies, dest_callee_saved } => {
                tracker.register_copy(*at, *dest, *src, *src_dies, *dest_callee_saved);
            }
            Event::Spill { from, slot } => tracker.spill(*at, *from, *slot),
            Event::Reload { slot, into } => tracker.reload(*at, *slot, *into),
        }
    }
    tracker
}

/// The ranges of every variable over a function (`ExtendRanges`): what the
/// predecessors agree is open at a block's start is open there, found by
/// visiting the blocks in reverse post-order until no block's start changes.
/// A predecessor not visited yet is ignored (`join`).
///
/// DEVIATE: the pin drops the locations whose variable's lexical scope does
/// not dominate the block; there are no scopes here. The worklist is one
/// ordered set, not the pin's worklist and pending pair; both reach the
/// fixpoint.
pub fn extend(
    blocks: &[FlowBlock],
    stack_pointer: RegId,
) -> Vec<Located> {
    let mut succs: Vec<Vec<usize>> = vec![Vec::new(); blocks.len()];
    for (index, block) in blocks.iter().enumerate() {
        for &pred in &block.preds {
            succs[pred].push(index);
        }
    }
    let mut ins: Vec<BTreeMap<Var, Where>> = vec![BTreeMap::new(); blocks.len()];
    let mut outs: Vec<Option<BTreeMap<Var, Where>>> = vec![None; blocks.len()];
    let mut worklist: BTreeSet<usize> = (0..blocks.len()).collect();
    while let Some(index) = worklist.pop_first() {
        let mut joined: Option<BTreeMap<Var, Where>> = None;
        for out in blocks[index].preds.iter().filter_map(|&pred| outs[pred].as_ref()) {
            joined = Some(match joined {
                None => out.clone(),
                Some(mut so_far) => {
                    so_far.retain(|var, place| out.get(var) == Some(place));
                    so_far
                }
            });
        }
        let joined = joined.unwrap_or_default();
        let visited = outs[index].is_some();
        if visited && joined == ins[index] {
            continue;
        }
        ins[index] = joined;
        let out = run(&blocks[index], &ins[index], stack_pointer).open_locs();
        if outs[index].as_ref() != Some(&out) || !visited {
            outs[index] = Some(out);
            worklist.extend(succs[index].iter().copied());
        }
    }
    let mut found = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        let mut tracker = run(block, &ins[index], stack_pointer);
        tracker.terminator(block.end);
        found.extend(tracker.finish());
    }
    found
}

/// The blocks of `body` as `extend` takes them, in reverse post-order, from
/// what the allocator decided: a register an instruction defines (through
/// `placed`), clobbers or delivers is written there, and a located range
/// starts a note at its first position. A write at a position goes before
/// the note that starts there: the note's register holds its value after the
/// instruction.
///
/// DEVIATE: the pin reads the final instructions' operands; this reads the
/// allocator's body and its decisions, which agree. Copies, spills and
/// reloads are not recognised yet.
pub fn flow_of(
    body: &LirBody,
    lis: &LiveIntervals,
    placed: &IndexMap<u32, RegId>,
    located: &[Located],
) -> Vec<FlowBlock> {
    let order = crate::backend::postrasink::reverse_post_order(body);
    let index_of: HashMap<i64, usize> = order.iter().enumerate().map(|(rank, &at)| (at, rank)).collect();
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); order.len()];
    for block in body.blocks.iter() {
        let Some(&from) = index_of.get(&block.at) else { continue };
        for to in block.succ.iter().filter_map(|to| index_of.get(to)) {
            preds[*to].push(from);
        }
    }
    let mut blocks: Vec<FlowBlock> = Vec::new();
    for (rank, &at) in order.iter().enumerate() {
        let Some(position) = body.blocks.iter().position(|block| block.at == at) else { continue };
        let (start, end) = *lis.indexes.get_mbb_range(position);
        let mut events: Vec<(i64, u8, Event)> = Vec::new();
        for one in body.blocks[position].insns.iter() {
            let here = i64::from(lis.indexes.get_instruction_index(one.id()).get_index());
            let mut written: Vec<RegId> = one.defines.iter().filter_map(|value| placed.get(value).copied()).collect();
            written.extend(one.clobbers.iter().copied());
            written.extend(one.delivers.iter().map(|(_, register)| *register));
            if !written.is_empty() {
                events.push((here, 0, Event::Def { written, mask: None }));
            }
        }
        let (low, high) = (i64::from(start.get_index()), i64::from(end.get_index()));
        for one in located.iter().filter(|one| (low..high).contains(&one.start)) {
            let place = (one.at != Where::Undef).then_some(one.at);
            events.push((one.start, 1, Event::Note(Var { variable: one.variable, piece: one.piece }, place)));
        }
        events.sort_by_key(|(here, phase, _)| (*here, *phase));
        blocks.push(FlowBlock {
            start: low,
            end: high,
            preds: std::mem::take(&mut preds[rank]),
            events: events.into_iter().map(|(here, _, event)| (here, event)).collect(),
        });
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg(n: u8) -> RegId {
        RegId::from_raw(n)
    }

    fn at(n: u8) -> Where {
        Where::Place(Place::Register(reg(n)))
    }

    fn var(variable: u32) -> Var {
        Var { variable, piece: None }
    }

    fn ranges(found: &[Located]) -> Vec<(u32, i64, i64, Where)> {
        found.iter().map(|one| (one.variable, one.start, one.end, one.at)).collect()
    }

    /// A variable's next note ends its range and starts another.
    #[test]
    fn a_variables_next_note_ends_its_range() {
        let mut tracker = Tracker::default();
        tracker.debug_value(10, var(1), Some(at(1)));
        tracker.debug_value(30, var(1), Some(at(2)));
        tracker.terminator(50);
        assert_eq!(ranges(&tracker.finish()), [(1, 10, 30, at(1)), (1, 30, 50, at(2))]);
    }

    /// An instruction that writes the register a variable is in ends the range
    /// there; one that writes another register leaves it.
    #[test]
    fn a_write_to_the_variables_register_ends_its_range() {
        let mut tracker = Tracker::default();
        tracker.debug_value(10, var(1), Some(at(1)));
        tracker.debug_value(10, var(2), Some(at(2)));
        tracker.register_def(20, &Clobber { written: &[reg(1)], mask: None }, reg(9));
        tracker.terminator(40);
        assert_eq!(ranges(&tracker.finish()), [(1, 10, 20, at(1)), (2, 10, 40, at(2))]);
    }

    /// A call's mask clobbers the registers it names, not the stack pointer.
    #[test]
    fn a_call_clobbers_what_its_mask_names_but_not_the_stack_pointer() {
        let mut tracker = Tracker::default();
        tracker.debug_value(10, var(1), Some(at(1)));
        tracker.debug_value(10, var(2), Some(at(9)));
        tracker.debug_value(10, var(3), Some(at(3)));
        let mask = |register: RegId| register != reg(3);
        tracker.register_def(20, &Clobber { written: &[], mask: Some(&mask) }, reg(9));
        tracker.terminator(40);
        assert_eq!(ranges(&tracker.finish()), [(1, 10, 20, at(1)), (2, 10, 40, at(9)), (3, 10, 40, at(3))]);
    }

    /// A note of a piece ends the open ranges of the pieces it overlaps and of
    /// the whole variable, and no others.
    #[test]
    fn a_note_of_a_piece_ends_the_pieces_it_overlaps() {
        let (low, high, whole) =
            (Var { variable: 1, piece: Some((0, 4)) }, Var { variable: 1, piece: Some((4, 4)) }, var(1));
        let mut tracker = Tracker::default();
        tracker.debug_value(10, low, Some(at(1)));
        tracker.debug_value(10, high, Some(at(2)));
        tracker.debug_value(20, whole, Some(at(3)));
        tracker.debug_value(30, low, None);
        tracker.terminator(40);
        let found = tracker.finish();
        assert_eq!(
            found.iter().map(|one| (one.piece, one.start, one.end)).collect::<Vec<_>>(),
            [(Some((0, 4)), 10, 20), (Some((4, 4)), 10, 20), (None, 20, 30)]
        );
    }

    /// A note that says nothing ends the range and starts none.
    #[test]
    fn a_note_of_nothing_ends_the_range_and_starts_none() {
        let mut tracker = Tracker::default();
        tracker.debug_value(10, var(1), Some(at(1)));
        tracker.debug_value(20, var(1), None);
        tracker.terminator(40);
        assert_eq!(ranges(&tracker.finish()), [(1, 10, 20, at(1))]);
    }
    /// A copy into a callee-saved register whose source dies carries the
    /// variable; into a caller-saved one, or from a source that lives on, it
    /// does not (the range stays where it was).
    #[test]
    fn a_dying_copy_into_a_callee_saved_register_carries_the_variable() {
        let mut tracker = Tracker::default();
        tracker.debug_value(10, var(1), Some(at(1)));
        tracker.register_copy(20, reg(5), reg(1), true, true);
        tracker.debug_value(20, var(2), Some(at(2)));
        tracker.register_copy(25, reg(6), reg(2), true, false);
        tracker.register_copy(26, reg(7), reg(2), false, true);
        tracker.terminator(40);
        assert_eq!(ranges(&tracker.finish()), [(1, 10, 20, at(1)), (1, 20, 40, at(5)), (2, 20, 40, at(2))]);
    }

    /// Of several variables in the copied register only the first to open
    /// moves, as the pin returns after the first transfer.
    #[test]
    fn a_copy_moves_the_first_variable_in_the_register_only() {
        let mut tracker = Tracker::default();
        tracker.debug_value(10, var(2), Some(at(1)));
        tracker.debug_value(10, var(1), Some(at(1)));
        tracker.register_copy(20, reg(5), reg(1), true, true);
        tracker.terminator(40);
        assert_eq!(ranges(&tracker.finish()), [(2, 10, 20, at(1)), (1, 10, 40, at(1)), (2, 20, 40, at(5))]);
    }

    /// A spill puts the variable in the cell, a store over the cell ends what
    /// was there, and a reload brings it back to a register.
    #[test]
    fn a_spill_and_a_reload_move_the_variable_through_the_cell() {
        let cell = |slot| Where::Place(Place::Cell(slot));
        let mut tracker = Tracker::default();
        tracker.debug_value(10, var(1), Some(at(1)));
        tracker.spill(20, reg(1), -8);
        tracker.reload(30, -8, reg(2));
        tracker.spill(35, reg(2), -8);
        tracker.spill(38, reg(3), -8);
        tracker.terminator(40);
        assert_eq!(
            ranges(&tracker.finish()),
            [(1, 10, 20, at(1)), (1, 20, 30, cell(-8)), (1, 30, 35, at(2)), (1, 35, 38, cell(-8))]
        );
    }
    fn block(
        start: i64,
        end: i64,
        preds: &[usize],
        events: Vec<(i64, Event)>,
    ) -> FlowBlock {
        FlowBlock { start, end, preds: preds.to_vec(), events }
    }

    /// A diamond: a variable noted in the entry and untouched on both arms is
    /// open in the join; one a single arm overwrites is not.
    #[test]
    fn what_both_arms_agree_on_is_open_in_the_join() {
        let kill = |register| Event::Def { written: vec![register], mask: None };
        let blocks = [
            block(0, 10, &[], vec![(0, Event::Note(var(1), Some(at(1)))), (0, Event::Note(var(2), Some(at(2))))]),
            block(10, 20, &[0], vec![]),
            block(20, 30, &[0], vec![(22, kill(reg(2)))]),
            block(30, 40, &[1, 2], vec![]),
        ];
        let mut found = ranges(&extend(&blocks, reg(9)));
        found.sort_by_key(|one| (one.0, one.1, one.2));
        assert_eq!(
            found,
            [
                (1, 0, 10, at(1)),
                (1, 10, 20, at(1)),
                (1, 20, 30, at(1)),
                (1, 30, 40, at(1)),
                (2, 0, 10, at(2)),
                (2, 10, 20, at(2)),
                (2, 20, 22, at(2)),
            ]
        );
    }

    /// A loop: a back edge from a block not visited yet is ignored the first
    /// time, and the fixpoint takes back a location the body kills.
    #[test]
    fn a_location_the_loop_body_kills_is_not_open_at_the_header() {
        let kill = Event::Def { written: vec![reg(1)], mask: None };
        let blocks = [
            block(0, 10, &[], vec![(0, Event::Note(var(1), Some(at(1))))]),
            block(10, 20, &[0, 2], vec![]),
            block(20, 30, &[1], vec![(25, kill)]),
        ];
        let mut found = ranges(&extend(&blocks, reg(9)));
        found.sort_by_key(|one| (one.0, one.1, one.2));
        // The first visit takes the header's start from block 0 alone; the
        // body's kill comes back over the edge and takes it away again.
        assert_eq!(found, [(1, 0, 10, at(1))]);
    }
}
