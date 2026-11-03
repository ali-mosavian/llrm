//! `LiveDebugVariables`: where each user variable is, over the final code.
//!
//! A variable's notes say what it is from a point on (`DBG_VALUE`); the
//! allocator says where each value is held (a register or a frame cell) over
//! which stretches of the code. A note's range runs to the variable's next
//! note, and is cut to the stretches its value is held, each stretch with the
//! place it has there (`UserValue::computeIntervals`, `rewriteLocations`);
//! where the value is not held the variable is `Undef`.
//!
//! DEVIATE: the pin keeps `DBG_VALUE` instructions and rewrites them in
//! place; the notes here stand before the next instruction and the result is
//! a list of ranges. Slot indexes are the caller's numbers for instruction
//! positions; none is made here. O(n log n) in notes and segments.

use std::collections::BTreeMap;

use llrm_lir::registers::RegId;

use crate::liveness::liveintervals::LiveIntervals;
use crate::model::lir::{LirBody, NoteArg, NoteValue};
use crate::support::hash::{HashMap, IndexMap};

/// Where a value is held.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Place {
    Register(RegId),
    /// A frame cell, by displacement.
    Cell(i64),
}

/// A stretch `[start, end)` of the code over which a value is held in `place`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Segment {
    pub start: i64,
    pub end: i64,
    pub place: Place,
    /// The value number (`VNInfo`) the stretch is of: one register can hold
    /// several values over its interval once a tied instruction is joined
    /// into it, and a note is of the one it stands in. None for a cell.
    pub vn: Option<usize>,
}

/// What a variable is over a stretch, once the allocator has decided.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Where {
    Place(Place),
    /// The value held in the place, plus a constant (`DW_OP_plus_uconst`).
    Offset(Place, i64),
    Constant(i64),
    /// What the function was entered with in the register
    /// (`DW_OP_entry_value`).
    Entry(RegId),
    /// Nothing says: its value is not held here, or was deleted.
    Undef,
}

/// A note, positioned: variable `variable` (or its `piece`) is `value` from
/// `at`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Positioned {
    pub at: i64,
    pub variable: u32,
    pub piece: Option<(u32, u32)>,
    pub value: NoteValue,
}

/// One range of a variable: `[start, end)` it is `at`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Located {
    pub variable: u32,
    pub piece: Option<(u32, u32)>,
    pub start: i64,
    pub end: i64,
    pub at: Where,
}

/// The ranges of every variable. `segments` gives each value's stretches
/// (sorted, not overlapping); `end` is where the code ends. A note of a value
/// the table does not know, or of an `Expr` that is not its one argument as it
/// is, is `Undef` over its range (an address is not a value). A parameter that
/// `Arrived` is `Entry` where no place holds it, if the target has entry
/// values.
pub fn locate(
    notes: &[Positioned],
    segments: &BTreeMap<u32, Vec<Segment>>,
    end: i64,
    entry_values: bool,
) -> Vec<Located> {
    locate_with(notes, segments, end, entry_values, true)
}

/// `locate`, with no `Undef` for the stretches a value is not held in when
/// `gaps` is false: the pin's `LiveDebugVariables` emits a `DBG_VALUE` where
/// an interval starts and none where it ends, so the variable stays where
/// it was until `LiveDebugValues` sees the register written. (A note that
/// says `Undef` itself still does.)
pub fn locate_without_gaps(
    notes: &[Positioned],
    segments: &BTreeMap<u32, Vec<Segment>>,
    end: i64,
) -> Vec<Located> {
    locate_with(notes, segments, end, false, false)
}

fn locate_with(
    notes: &[Positioned],
    segments: &BTreeMap<u32, Vec<Segment>>,
    end: i64,
    entry_values: bool,
    gaps: bool,
) -> Vec<Located> {
    let mut by_variable: BTreeMap<(u32, Option<(u32, u32)>), Vec<&Positioned>> = BTreeMap::new();
    for note in notes {
        by_variable.entry((note.variable, note.piece)).or_default().push(note);
    }
    let mut out = Vec::new();
    for ((variable, piece), mut list) in by_variable {
        // Stable: notes standing together keep their order; the last says.
        list.sort_by_key(|note| note.at);
        for (index, note) in list.iter().enumerate() {
            let start = note.at;
            // A later note at the same position replaces this one.
            if list.get(index + 1).is_some_and(|next| next.at == start) {
                continue;
            }
            let stop = list.get(index + 1).map_or(end, |next| next.at);
            if start >= stop {
                continue;
            }
            let mut push = |start: i64, end: i64, at: Where| {
                if start < end {
                    out.push(Located { variable, piece, start, end, at });
                }
            };
            match &note.value {
                NoteValue::Constant(value) => push(start, stop, Where::Constant(*value)),
                NoteValue::Value(_) | NoteValue::Offset(..) | NoteValue::Arrived(..) | NoteValue::Expr(_) => {
                    let (value, add, gap, lead) = match &note.value {
                        NoteValue::Value(value) => (*value, None, Where::Undef, Where::Undef),
                        NoteValue::Offset(value, add) => (*value, Some(*add), Where::Undef, Where::Undef),
                        NoteValue::Arrived(value, register) => {
                            let gap = if entry_values { Where::Entry(*register) } else { Where::Undef };
                            // Before its first place it is still in the register it
                            // arrived in (the pin's live-in `DBG_VALUE $reg`).
                            (*value, None, gap, if entry_values { gap } else { Where::Place(Place::Register(*register)) })
                        }
                        NoteValue::Expr(expr) => match (expr.args.as_slice(), expr.ops.is_empty()) {
                            ([NoteArg::Value(value)], true) if expr.value => (*value, None, Where::Undef, Where::Undef),
                            _ => {
                                push(start, stop, Where::Undef);
                                continue;
                            }
                        },
                        _ => unreachable!(),
                    };
                    let mut from = start;
                    let fill = |push: &mut dyn FnMut(i64, i64, Where), from: i64, to: i64| {
                        let at = if from == start { lead } else { gap };
                        if gaps || at != Where::Undef {
                            push(from, to, at);
                        }
                    };
                    // Over the stretches of the value number the note stands in
                    // (`extendDef` follows the `VNInfo` of the def), not of
                    // whatever else the register's interval holds.
                    // A note before the value's first stretch (an argument's
                    // arrival) is of the first one.
                    let number = segments
                        .get(&value)
                        .into_iter()
                        .flatten()
                        .find(|segment| start < segment.end)
                        .and_then(|segment| segment.vn);
                    for segment in segments
                        .get(&value)
                        .into_iter()
                        .flatten()
                        .filter(|segment| number.is_none() || segment.vn == number)
                    {
                        let (low, high) = (segment.start.max(start), segment.end.min(stop));
                        if low >= high {
                            continue;
                        }
                        fill(&mut push, from, low);
                        push(
                            low,
                            high,
                            add.map_or(Where::Place(segment.place), |add| Where::Offset(segment.place, add)),
                        );
                        from = high;
                    }
                    fill(&mut push, from, stop);
                }
                _ => push(start, stop, Where::Undef),
            }
        }
    }
    out
}

/// The stretches each value is held in a register, by the number its register
/// had in SSA (what a note names): the live range of every placed virtual
/// register, and of the clones a split made, which `parents` leads back to the
/// register they were cloned from. A spilled value has no register stretch
/// here; its cell is not known yet, so it reads `Undef` (DEVIATE: the pin's
/// `rewriteLocations` takes the stack slot from `VirtRegMap`).
/// O(segments).
pub fn segments_of(
    lis: &mut LiveIntervals,
    placed: &IndexMap<u32, RegId>,
    parents: &HashMap<u32, u32>,
) -> BTreeMap<u32, Vec<Segment>> {
    let mut out: BTreeMap<u32, Vec<Segment>> = BTreeMap::new();
    for value in lis.virt_regs() {
        let Some(&register) = placed.get(&value) else { continue };
        let root = parents.get(&value).copied().unwrap_or(value);
        let list = out.entry(root).or_default();
        for segment in &lis.interval(value).segments {
            list.push(Segment {
                vn: segment.valno.as_ref().map(|vn| std::rc::Rc::as_ptr(vn) as usize),
                start: i64::from(segment.start.get_index()),
                end: i64::from(segment.end.get_index()),
                place: Place::Register(register),
            });
        }
    }
    for list in out.values_mut() {
        list.sort_by_key(|segment| (segment.start, segment.end));
    }
    out
}

/// Adds the stretches each of `values` is in its spill cell: the segments the
/// spiller wrote for the value's slot (`LiveStacks`), at the cell `home` names
/// (`rewriteLocations` takes it from `VirtRegMap::getStackSlot`). A value with
/// no slot, or whose slot has no stretch, gains none; its table stays sorted.
/// O(values + segments).
pub fn add_cells(
    table: &mut BTreeMap<u32, Vec<Segment>>,
    values: impl IntoIterator<Item = u32>,
    home: impl Fn(u32) -> Option<i64>,
    stacks: &IndexMap<i64, Vec<(i64, i64)>>,
) {
    for value in values {
        let Some(slot) = home(value) else { continue };
        let Some(stretches) = stacks.get(&slot) else { continue };
        let list = table.entry(value).or_default();
        list.extend(stretches.iter().map(|&(start, end)| Segment { start, end, place: Place::Cell(slot), vn: None }));
        list.sort_by_key(|segment| (segment.start, segment.end));
    }
}

/// The notes of `body` at the slot index of the instruction each stands
/// before.
pub fn positioned(
    body: &LirBody,
    lis: &LiveIntervals,
) -> Vec<Positioned> {
    let mut out = Vec::new();
    for one in body.blocks.iter().flat_map(|block| block.insns.iter()) {
        if one.debug.before.is_empty() {
            continue;
        }
        let at = i64::from(lis.indexes.get_instruction_index(one.id()).get_index());
        for &note in &one.debug.before {
            let note = &body.notes[note as usize];
            out.push(Positioned { at, variable: note.variable, piece: note.piece, value: note.value.clone() });
        }
    }
    out
}

/// Extends each value's stretches over the registers its copies were made
/// into (`UserValue::addDefsFromCopies`): a value copied into another register
/// is held there too, over that register's stretches, and a copy of a copy is
/// followed. `table` is keyed by the allocator's registers, as `segments_of`
/// leaves it.
///
/// DEVIATE: the pin takes the copy destination's one value; a register here
/// that several values share gives the source all of its stretches.
/// O(copies x chain length + segments).
pub fn add_copies(
    body: &LirBody,
    table: &mut BTreeMap<u32, Vec<Segment>>,
    parents: &HashMap<u32, u32>,
) {
    let root = |value: u32| parents.get(&value).copied().unwrap_or(value);
    let copies: Vec<(u32, u32)> = body
        .blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .filter(|one| one.what.as_ref().is_some_and(|what| what.op == crate::model::ir::Operation::Move))
        .filter_map(|one| match (one.defines.as_slice(), one.uses.as_slice()) {
            ([dest], [source]) if !one.spill_store && !one.spill_reload => Some((root(*source), root(*dest))),
            _ => None,
        })
        .filter(|(source, dest)| source != dest)
        .collect();
    let mut changed = true;
    while changed {
        changed = false;
        for &(source, dest) in &copies {
            let Some(found) = table.get(&dest).cloned() else { continue };
            let list = table.entry(source).or_default();
            for segment in found {
                if !list.contains(&segment) {
                    list.push(segment);
                    changed = true;
                }
            }
        }
    }
    for list in table.values_mut() {
        list.sort_by_key(|segment| (segment.start, segment.end));
    }
}

/// Re-keys `table` (by the allocator's registers) by the number a note names
/// a value with: the register the instruction defining the value writes first
/// (what `masm` marks as the value's place; the entry's, one per register it
/// delivers), or the register a split cloned it from. O(instructions +
/// segments).
pub fn by_note_value(
    body: &LirBody,
    table: &BTreeMap<u32, Vec<Segment>>,
    parents: &HashMap<u32, u32>,
) -> BTreeMap<u32, Vec<Segment>> {
    let mut out: BTreeMap<u32, Vec<Segment>> = BTreeMap::new();
    for one in body.blocks.iter().flat_map(|block| block.insns.iter()) {
        for (index, &tag) in one.debug.defines.iter().enumerate().filter(|(_, tag)| **tag != u32::MAX) {
            // The entry's values are marked one per register it delivers;
            // any other instruction's all at the register it writes first.
            let defined = if one.arrival() { one.defines.get(index) } else { one.defines.first() };
            let Some(&defined) = defined else { continue };
            let Some(segments) = table.get(&parents.get(&defined).copied().unwrap_or(defined)) else { continue };
            out.entry(tag).or_insert_with(|| segments.clone());
        }
    }
    out
}

/// What the writer reads of `Ranges`: the ranges `LiveDebugValues` leaves
/// and every instruction's slot and id.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Placed {
    pub ranges: Vec<Located>,
    pub insns: Vec<(i64, u64)>,
}

/// The ranges `LiveDebugVariables` places (`location`, with entry values) and
/// those `LiveDebugValues` leaves after them (`flow`), over the body the
/// allocator wrote; the notes' slot positions as (note number, position).
pub struct Ranges {
    pub location: Vec<Located>,
    pub flow: Vec<Located>,
    pub notes: Vec<(u32, i64)>,
    /// Every instruction's slot position and `Insn::id`, in code order: what
    /// the writer joins on, since it knows no slot.
    pub insns: Vec<(i64, u64)>,
}

/// The ranges of every variable over `edited` (see `Ranges`). `home` is the
/// cell a spilled value was given.
pub fn ranges_of(
    edited: &LirBody,
    placed: &[(u32, RegId)],
    spilled: &[u32],
    parents: &HashMap<u32, u32>,
    home: &dyn Fn(u32) -> Option<i64>,
    stacks: &IndexMap<i64, Vec<(i64, i64)>>,
    saved: &[RegId],
) -> Ranges {
    let mut lis = LiveIntervals::new(edited);
    let mut placed: IndexMap<u32, RegId> = placed.iter().copied().collect();
    // A pinned value (an argument, a call's result) is where its pin says.
    for (&value, &register) in &edited.pins {
        placed.entry(value).or_insert(register);
    }
    let mut table = segments_of(&mut lis, &placed, parents);
    let root = |value: u32| parents.get(&value).copied().unwrap_or(value);
    add_cells(&mut table, spilled.iter().map(|&value| root(value)), home, stacks);
    let at = |id| i64::from(lis.indexes.get_instruction_index(id).get_index());
    let mut notes = Vec::new();
    for one in edited.blocks.iter().flat_map(|block| block.insns.iter()) {
        for &note in &one.debug.before {
            notes.push((note, at(one.id())));
        }
    }
    let insns: Vec<(i64, u64)> =
        edited.blocks.iter().flat_map(|block| block.insns.iter()).map(|one| (at(one.id()), one.id())).collect();
    let end = lis.indexes.get_last_index().get_index();
    if llrm_support::env_flag!("LLRM_LDV_DEBUG") {
        eprintln!("LDVTABLE\t{}\t{:?}\tpins={:?}\tparents={:?}", edited.name, table, edited.pins, parents);
        for one in positioned(edited, &lis) {
            eprintln!("LDVPOS\t{:?}", one);
        }
    }
    add_copies(edited, &mut table, parents);
    let table = by_note_value(edited, &table, parents);
    let positioned = positioned(edited, &lis);
    let location = locate(&positioned, &table, i64::from(end), true);
    // LiveDebugVariables knows no entry values: where no place holds the
    // value it says nothing, and LiveDebugValues does the rest.
    let bare = locate_without_gaps(&positioned, &table, i64::from(end));
    // The pin's LiveDebugValues over those ranges: the stack pointer is not
    // known here, so no register is spared a call's mask (none is given).
    let target = crate::backend::livedebugvalues::FlowTarget {
        parents,
        home: &|value| home(value),
        callee_saved: &|register| saved.contains(&register),
    };
    // A parameter is a variable the entry says arrived in a register
    // (DEVIATE: the pin reads `DILocalVariable::isParameter`).
    let params: std::collections::BTreeSet<u32> = edited
        .notes
        .iter()
        .filter(|note| matches!(note.value, NoteValue::Arrived(..)))
        .map(|note| note.variable)
        .collect();
    let flow = crate::backend::livedebugvalues::flow_of(edited, &mut lis, &placed, &bare, &target);
    let flow = crate::backend::livedebugvalues::extend(
        &flow,
        &crate::backend::livedebugvalues::Flow { stack_pointer: RegId::None, params: &params },
    );
    Ranges { location, flow, notes, insns }
}

/// Prints, for `LLRM_LDV_DUMP`, the notes' slot positions (`LDVNOTE`) and the
/// ranges of every variable (`LDV`) over the body the allocator wrote, to be
/// joined by note index with the same function's `valuetrack` rows. Reads
/// only; the code is unchanged.
pub fn dump(
    edited: &LirBody,
    placed: &[(u32, RegId)],
    spilled: &[u32],
    parents: &HashMap<u32, u32>,
    frame: &crate::backend::frame::Frame,
    saved: &[RegId],
) {
    use crate::backend::frame::FrameInfo;
    let found = ranges_of(edited, placed, spilled, parents, &|value| frame.home(value), &frame.stacks, saved);
    for (note, position) in &found.notes {
        eprintln!("LDVNOTE\t{}\t{note}\t{position}", edited.name);
    }
    for one in found.flow {
        eprintln!("VLD\t{}\t{}\t{:?}\t{}\t{}\t{:?}", edited.name, one.variable, one.piece, one.start, one.end, one.at);
    }
    for one in found.location {
        eprintln!("LDV\t{}\t{}\t{:?}\t{}\t{}\t{:?}", edited.name, one.variable, one.piece, one.start, one.end, one.at);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg(n: u32) -> Place {
        Place::Register(RegId::from_raw(n as u8))
    }

    fn note(
        at: i64,
        variable: u32,
        value: NoteValue,
    ) -> Positioned {
        Positioned { at, variable, piece: None, value }
    }

    fn seg(
        start: i64,
        end: i64,
        place: Place,
    ) -> Segment {
        Segment { start, end, place, vn: None }
    }

    fn ranges(found: &[Located]) -> Vec<(i64, i64, Where)> {
        found.iter().map(|one| (one.start, one.end, one.at)).collect()
    }

    /// A parameter that arrived in a register is `Entry` where no place holds
    /// it, and its place where one does; an `Expr` that is its one argument
    /// is that argument. Both were `Undef` over their whole range.
    #[test]
    fn an_arrived_parameter_falls_back_to_its_entry_register() {
        let entry = RegId::from_raw(7);
        let table = BTreeMap::from([(5, vec![seg(20, 30, reg(1))])]);
        let found = locate(&[note(10, 1, NoteValue::Arrived(5, entry))], &table, 40, true);
        assert_eq!(
            ranges(&found),
            [(10, 20, Where::Entry(entry)), (20, 30, Where::Place(reg(1))), (30, 40, Where::Entry(entry))]
        );
        let same = NoteValue::Expr(std::sync::Arc::new(crate::model::lir::NoteExpr {
            args: vec![NoteArg::Value(5)],
            ops: vec![],
            value: true,
        }));
        let found = locate(&[note(10, 1, same)], &table, 40, true);
        assert_eq!(ranges(&found), [(10, 20, Where::Undef), (20, 30, Where::Place(reg(1))), (30, 40, Where::Undef)]);
    }

    /// Without entry values a parameter is `Undef` off its place, and an
    /// `Expr` that makes the argument's address is never the argument's place:
    /// both read as a place or an entry value they are not.
    #[test]
    fn no_entry_value_and_no_address_as_a_place() {
        let table = BTreeMap::from([(5, vec![seg(20, 30, reg(1))])]);
        let found = locate(&[note(10, 1, NoteValue::Arrived(5, RegId::from_raw(7)))], &table, 40, false);
        assert_eq!(
            ranges(&found),
            [(10, 20, Where::Place(Place::Register(RegId::from_raw(7)))), (20, 30, Where::Place(reg(1))), (30, 40, Where::Undef)]
        );
        let address = NoteValue::Expr(std::sync::Arc::new(crate::model::lir::NoteExpr {
            args: vec![NoteArg::Value(5)],
            ops: vec![],
            value: false,
        }));
        assert_eq!(ranges(&locate(&[note(10, 1, address)], &table, 40, true)), [(10, 40, Where::Undef)]);
    }

    /// A value split into a register and then a cell is two ranges, one per
    /// place; the pin's `DBG_VALUE` per interval of the virtual register.
    #[test]
    fn a_value_moved_between_places_is_a_range_for_each() {
        let table = BTreeMap::from([(7, vec![seg(10, 20, reg(1)), seg(20, 40, Place::Cell(-4))])]);
        let found = locate(&[note(10, 1, NoteValue::Value(7))], &table, 50, true);
        assert_eq!(
            ranges(&found),
            [(10, 20, Where::Place(reg(1))), (20, 40, Where::Place(Place::Cell(-4))), (40, 50, Where::Undef)]
        );
    }

    /// The next note of the variable ends a range, though the value is held
    /// on: the variable is the new value from there.
    #[test]
    fn the_next_note_of_a_variable_ends_the_range() {
        let table = BTreeMap::from([(7, vec![seg(0, 100, reg(1))]), (8, vec![seg(0, 100, reg(2))])]);
        let found = locate(&[note(10, 1, NoteValue::Value(7)), note(30, 1, NoteValue::Value(8))], &table, 100, true);
        assert_eq!(ranges(&found), [(10, 30, Where::Place(reg(1))), (30, 100, Where::Place(reg(2)))]);
    }

    /// Where the value is not held the variable is undefined, and is again
    /// when the value is held later: a hole, not an end.
    #[test]
    fn a_hole_in_the_value_is_undef_between_its_places() {
        let table = BTreeMap::from([(7, vec![seg(10, 20, reg(1)), seg(30, 40, reg(1))])]);
        let found = locate(&[note(10, 1, NoteValue::Value(7))], &table, 40, true);
        assert_eq!(
            ranges(&found),
            [(10, 20, Where::Place(reg(1))), (20, 30, Where::Undef), (30, 40, Where::Place(reg(1)))]
        );
    }

    /// Two notes at one position: the later says (a variable cannot be two
    /// things at a point).
    #[test]
    fn of_two_notes_at_one_position_the_later_says() {
        let table = BTreeMap::from([(7, vec![seg(0, 10, reg(1))])]);
        let found = locate(&[note(5, 1, NoteValue::Constant(3)), note(5, 1, NoteValue::Value(7))], &table, 10, true);
        assert_eq!(ranges(&found), [(5, 10, Where::Place(reg(1)))]);
    }

    /// A constant is its own place; another variable's notes do not end it.
    #[test]
    fn a_constant_holds_until_its_variables_next_note() {
        let found =
            locate(&[note(2, 1, NoteValue::Constant(9)), note(4, 2, NoteValue::Nothing)], &BTreeMap::new(), 8, true);
        let one = found.iter().find(|one| one.variable == 1).unwrap();
        assert_eq!((one.start, one.end, one.at), (2, 8, Where::Constant(9)));
    }

    /// A value plus a constant keeps the constant with the place, in each
    /// range of the value.
    #[test]
    fn an_offset_note_keeps_its_constant_in_every_place() {
        let table = BTreeMap::from([(7, vec![seg(0, 5, reg(1)), seg(5, 9, Place::Cell(-8))])]);
        let found = locate(&[note(0, 1, NoteValue::Offset(7, 12))], &table, 9, true);
        assert_eq!(ranges(&found), [(0, 5, Where::Offset(reg(1), 12)), (5, 9, Where::Offset(Place::Cell(-8), 12))]);
    }

    /// A note ahead of the use of a value the allocator put in a register is
    /// that register from the note to the value's last use, and `Undef` after
    /// it; a clone a split made counts for the register it was cloned from.
    #[test]
    fn a_noted_value_is_where_the_allocator_put_it_over_its_live_range() {
        use std::sync::Arc;

        use crate::model::lir::{Block, DebugNote, Insn, LirBlock};

        let define = Arc::new(Insn::new(0, None, None, vec![1], vec![]));
        let mut noted = Insn::new(0, None, None, vec![2], vec![1]);
        noted.debug.before = vec![0];
        let last = Arc::new(Insn::new(0, None, None, vec![], vec![2]));
        let mut body = LirBody::new(
            "f",
            0,
            vec![Block::from(LirBlock::new(0, vec![define, Arc::new(noted), last]))],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.notes = Arc::new(vec![DebugNote { variable: 9, piece: None, value: NoteValue::Value(1) }]);
        let mut lis = LiveIntervals::new(&body);
        let placed: IndexMap<u32, RegId> = [(1, RegId::from_raw(3)), (2, RegId::from_raw(5))].into_iter().collect();
        // v2 is a clone of v1: the note about v1 follows its value there.
        let parents: HashMap<u32, u32> = [(2, 1)].into_iter().collect();
        let table = segments_of(&mut lis, &placed, &parents);
        let notes = positioned(&body, &lis);
        assert_eq!(notes.len(), 1);
        let end = notes[0].at + 100;
        let found = locate(&notes, &table, end, true);
        assert_eq!(found.first().map(|one| one.at), Some(Where::Place(reg(3))), "{found:?} from {table:?}");
        assert_eq!(found.last().map(|one| (one.at, one.end)), Some((Where::Undef, end)), "{found:?}");
    }

    /// A note names a value by its SSA number, but the allocator's body has
    /// other numbers: the table is found through the instruction that defines
    /// the value (and the register a split cloned it from). Keyed by the
    /// allocator's numbers, every `-g` variable read `Undef`.
    #[test]
    fn a_note_value_is_found_through_the_instruction_that_defines_it() {
        use std::sync::Arc;

        use crate::model::lir::{Block, Insn, LirBlock};

        let mut define = Insn::new(0, None, None, vec![21], vec![]);
        define.debug.defines = vec![7];
        let body = LirBody::new(
            "f",
            0,
            vec![Block::from(LirBlock::new(0, vec![Arc::new(define)]))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let table = BTreeMap::from([(20, vec![seg(4, 9, reg(3))]), (7, vec![seg(0, 1, reg(9))])]);
        let parents: HashMap<u32, u32> = [(21, 20)].into_iter().collect();
        let found = by_note_value(&body, &table, &parents);
        assert_eq!(found, BTreeMap::from([(7, vec![seg(4, 9, reg(3))])]), "register 7 of the table is not value 7");
    }

    /// The entry marks one value per register it delivers: the second value of
    /// an entry that delivers two is the second register it defines, not the
    /// first, which gave a second parameter its first parameter's place.
    #[test]
    fn the_entrys_values_are_found_one_per_register_it_defines() {
        use std::sync::Arc;

        use crate::model::ir::{Held, Operation, Semantics};
        use crate::model::lir::{Block, Insn, LirBlock};

        let what = Semantics { name: Some("".into()), ..Semantics::new(Operation::Nothing) };
        let mut entry = Insn::new(0, None, Some(what), vec![21, 22], vec![]);
        entry.delivers =
            vec![(Held { value: 7, width: 4 }, RegId::from_raw(1)), (Held { value: 8, width: 4 }, RegId::from_raw(2))];
        entry.debug.defines = vec![7, 8];
        let body = LirBody::new(
            "f",
            0,
            vec![Block::from(LirBlock::new(0, vec![Arc::new(entry)]))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let table = BTreeMap::from([(21, vec![seg(0, 5, reg(1))]), (22, vec![seg(0, 6, reg(2))])]);
        let found = by_note_value(&body, &table, &HashMap::default());
        assert_eq!(found, BTreeMap::from([(7, vec![seg(0, 5, reg(1))]), (8, vec![seg(0, 6, reg(2))])]));
    }

    /// A value copied into another register is held there too, and a copy of
    /// a copy as well: the stretches of a variable ended where the value moved.
    #[test]
    fn a_value_is_held_in_the_registers_its_copies_were_made_into() {
        use std::sync::Arc;

        use crate::model::ir::{Operation, Semantics};
        use crate::model::lir::{Block, Insn, LirBlock};

        let mov = |dest: u32, source: u32| {
            Arc::new(Insn::new(0, None, Some(Semantics::new(Operation::Move)), vec![dest], vec![source]))
        };
        let body = LirBody::new(
            "f",
            0,
            vec![Block::from(LirBlock::new(0, vec![mov(2, 1), mov(3, 2)]))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let mut table = BTreeMap::from([
            (1, vec![seg(0, 10, reg(1))]),
            (2, vec![seg(10, 20, reg(2))]),
            (3, vec![seg(20, 30, reg(3))]),
        ]);
        add_copies(&body, &mut table, &HashMap::default());
        assert_eq!(table[&1], [seg(0, 10, reg(1)), seg(10, 20, reg(2)), seg(20, 30, reg(3))]);
        assert_eq!(table[&2], [seg(10, 20, reg(2)), seg(20, 30, reg(3))]);
    }

    /// A value spilled to a cell is in the cell over the stretches the spiller
    /// wrote, in the register over the rest: its variable has both places, in
    /// order, where it had only the register's before.
    #[test]
    fn a_spilled_value_is_in_its_cell_over_the_spillers_stretches() {
        let mut table = BTreeMap::from([(7, vec![seg(10, 20, reg(1))])]);
        let stacks: IndexMap<i64, Vec<(i64, i64)>> = [(-8, vec![(20, 40)])].into_iter().collect();
        add_cells(&mut table, [7, 8], |value| (value == 7).then_some(-8), &stacks);
        let found = locate(&[note(10, 1, NoteValue::Value(7))], &table, 50, true);
        assert_eq!(
            ranges(&found),
            [(10, 20, Where::Place(reg(1))), (20, 40, Where::Place(Place::Cell(-8))), (40, 50, Where::Undef)]
        );
        assert!(!table.contains_key(&8), "a value with no slot gains no entry");
    }

    /// A tied instruction joined into its source's register makes one interval
    /// of two values (gdb.c `l`, `add eax,eax`): the note is of the value it
    /// stands in, so the variable ended at the redefinition (the pin's
    /// 5..7) and ours listed it to the end of the interval (5..11).
    #[test]
    fn a_note_is_of_its_value_number_not_of_the_whole_interval() {
        let held = |start, end, vn| Segment { start, end, place: reg(1), vn: Some(vn) };
        let table = BTreeMap::from([(7, vec![held(10, 20, 1), held(20, 30, 2)])]);
        let found = locate_without_gaps(&[note(12, 1, NoteValue::Value(7))], &table, 50);
        assert_eq!(ranges(&found), [(12, 20, Where::Place(reg(1)))]);
    }

    /// An argument said at the entry is of the value it arrived as: a copy of
    /// it into another register is a value of its own, and the variable stays
    /// in the argument's register (observed.c `bump.by`: ours moved it to
    /// `eax` at the copy, the pin kept `esi` throughout).
    #[test]
    fn an_argument_stays_in_its_register_after_a_copy_of_it_is_made() {
        let held = |start, end, register: u32, vn| Segment { start, end, place: reg(register), vn: Some(vn) };
        let table = BTreeMap::from([(7, vec![held(18, 50, 1, 1), held(50, 98, 2, 2)])]);
        let said = [note(16, 1, NoteValue::Arrived(7, RegId::from_raw(1)))];
        let found = locate_without_gaps(&said, &table, 100);
        assert_eq!(found.iter().filter(|one| one.at == Where::Place(reg(2))).count(), 0, "{found:?}");
    }

    /// A parameter whose register is last read at the first instruction read
    /// as nowhere from there on, and the tracker closed it at that read: the
    /// pin leaves one `DBG_VALUE` until the register is written, so `id(x)`
    /// had one location for the whole function and ours ended at the return.
    #[test]
    fn a_value_that_dies_is_not_nowhere_afterwards_unless_a_note_says_so() {
        let mut table = BTreeMap::new();
        table.insert(7, vec![Segment { start: 10, end: 20, place: reg(1), vn: None }]);
        let held = [note(10, 1, NoteValue::Value(7))];
        assert_eq!(ranges(&locate_without_gaps(&held, &table, 50)), [(10, 20, Where::Place(reg(1)))]);
        let said = [note(10, 1, NoteValue::Value(7)), note(30, 1, NoteValue::Nothing)];
        assert_eq!(
            ranges(&locate_without_gaps(&said, &table, 50)),
            [(10, 20, Where::Place(reg(1))), (30, 50, Where::Undef)]
        );
        assert_eq!(
            ranges(&locate(&held, &table, 50, false)),
            [(10, 20, Where::Place(reg(1))), (20, 50, Where::Undef)],
            "the ranges LiveDebugVariables reports keep the gap"
        );
    }

    /// The ranges the notes make were only printed (`LLRM_LDV_DUMP`), so
    /// nothing else could read them: a value noted before an instruction that
    /// uses it, in a register, is a range of the variable in that register.
    #[test]
    fn the_ranges_of_a_body_are_returned_not_only_printed() {
        use std::sync::Arc;

        use crate::model::lir::{Block, DebugNote, Insn, LirBlock};

        let mut define = Insn::new(0, None, None, vec![21], vec![]);
        define.debug.defines = vec![7];
        let mut user = Insn::new(0, None, None, vec![], vec![21]);
        user.debug.before = vec![0];
        let mut body = LirBody::new(
            "f",
            0,
            vec![Block::from(LirBlock::new(0, vec![Arc::new(define), Arc::new(user)]))],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.notes = Arc::new(vec![DebugNote { variable: 9, piece: None, value: NoteValue::Value(7) }]);
        let found = ranges_of(
            &body,
            &[(21, RegId::from_raw(3))],
            &[],
            &HashMap::default(),
            &|_| None,
            &IndexMap::default(),
            &[],
        );
        let register = Where::Place(Place::Register(RegId::from_raw(3)));
        assert!(found.location.iter().any(|one| one.variable == 9 && one.at == register), "{:?}", found.location);
        assert!(found.flow.iter().any(|one| one.variable == 9 && one.at == register), "{:?}", found.flow);
        assert_eq!(found.notes.len(), 1);
        let ids: Vec<u64> = found.insns.iter().map(|&(_, id)| id).collect();
        assert_eq!(ids, body.blocks[0].insns.iter().map(|one| one.id()).collect::<Vec<_>>());
        assert!(found.insns.windows(2).all(|pair| pair[0].0 < pair[1].0), "{:?}", found.insns);
        assert_eq!(found.notes[0].1, found.insns[1].0, "a note stands at its instruction's slot");
    }
}
