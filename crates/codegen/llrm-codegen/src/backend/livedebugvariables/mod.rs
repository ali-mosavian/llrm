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
/// is, is `Undef` over its range. A parameter that `Arrived` is `Entry` where
/// no place holds it.
pub fn locate(
    notes: &[Positioned],
    segments: &BTreeMap<u32, Vec<Segment>>,
    end: i64,
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
                    let (value, add, gap) = match &note.value {
                        NoteValue::Value(value) => (*value, None, Where::Undef),
                        NoteValue::Offset(value, add) => (*value, Some(*add), Where::Undef),
                        NoteValue::Arrived(value, register) => (*value, None, Where::Entry(*register)),
                        NoteValue::Expr(expr) => match (expr.args.as_slice(), expr.ops.is_empty()) {
                            ([NoteArg::Value(value)], true) => (*value, None, Where::Undef),
                            _ => {
                                push(start, stop, Where::Undef);
                                continue;
                            }
                        },
                        _ => unreachable!(),
                    };
                    let mut from = start;
                    for segment in segments.get(&value).into_iter().flatten() {
                        let (low, high) = (segment.start.max(start), segment.end.min(stop));
                        if low >= high {
                            continue;
                        }
                        push(from, low, gap);
                        push(
                            low,
                            high,
                            add.map_or(Where::Place(segment.place), |add| Where::Offset(segment.place, add)),
                        );
                        from = high;
                    }
                    push(from, stop, gap);
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
        list.extend(stretches.iter().map(|&(start, end)| Segment { start, end, place: Place::Cell(slot) }));
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
        Segment { start, end, place }
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
        let found = locate(&[note(10, 1, NoteValue::Arrived(5, entry))], &table, 40);
        assert_eq!(
            ranges(&found),
            [(10, 20, Where::Entry(entry)), (20, 30, Where::Place(reg(1))), (30, 40, Where::Entry(entry))]
        );
        let same = NoteValue::Expr(std::sync::Arc::new(crate::model::lir::NoteExpr {
            args: vec![NoteArg::Value(5)],
            ops: vec![],
            value: true,
        }));
        let found = locate(&[note(10, 1, same)], &table, 40);
        assert_eq!(ranges(&found), [(10, 20, Where::Undef), (20, 30, Where::Place(reg(1))), (30, 40, Where::Undef)]);
    }

    /// A value split into a register and then a cell is two ranges, one per
    /// place; the pin's `DBG_VALUE` per interval of the virtual register.
    #[test]
    fn a_value_moved_between_places_is_a_range_for_each() {
        let table = BTreeMap::from([(7, vec![seg(10, 20, reg(1)), seg(20, 40, Place::Cell(-4))])]);
        let found = locate(&[note(10, 1, NoteValue::Value(7))], &table, 50);
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
        let found = locate(&[note(10, 1, NoteValue::Value(7)), note(30, 1, NoteValue::Value(8))], &table, 100);
        assert_eq!(ranges(&found), [(10, 30, Where::Place(reg(1))), (30, 100, Where::Place(reg(2)))]);
    }

    /// Where the value is not held the variable is undefined, and is again
    /// when the value is held later: a hole, not an end.
    #[test]
    fn a_hole_in_the_value_is_undef_between_its_places() {
        let table = BTreeMap::from([(7, vec![seg(10, 20, reg(1)), seg(30, 40, reg(1))])]);
        let found = locate(&[note(10, 1, NoteValue::Value(7))], &table, 40);
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
        let found = locate(&[note(5, 1, NoteValue::Constant(3)), note(5, 1, NoteValue::Value(7))], &table, 10);
        assert_eq!(ranges(&found), [(5, 10, Where::Place(reg(1)))]);
    }

    /// A constant is its own place; another variable's notes do not end it.
    #[test]
    fn a_constant_holds_until_its_variables_next_note() {
        let found = locate(&[note(2, 1, NoteValue::Constant(9)), note(4, 2, NoteValue::Nothing)], &BTreeMap::new(), 8);
        let one = found.iter().find(|one| one.variable == 1).unwrap();
        assert_eq!((one.start, one.end, one.at), (2, 8, Where::Constant(9)));
    }

    /// A value plus a constant keeps the constant with the place, in each
    /// range of the value.
    #[test]
    fn an_offset_note_keeps_its_constant_in_every_place() {
        let table = BTreeMap::from([(7, vec![seg(0, 5, reg(1)), seg(5, 9, Place::Cell(-8))])]);
        let found = locate(&[note(0, 1, NoteValue::Offset(7, 12))], &table, 9);
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
        let found = locate(&notes, &table, end);
        assert_eq!(found.first().map(|one| one.at), Some(Where::Place(reg(3))), "{found:?} from {table:?}");
        assert_eq!(found.last().map(|one| (one.at, one.end)), Some((Where::Undef, end)), "{found:?}");
    }

    /// A value spilled to a cell is in the cell over the stretches the spiller
    /// wrote, in the register over the rest: its variable has both places, in
    /// order, where it had only the register's before.
    #[test]
    fn a_spilled_value_is_in_its_cell_over_the_spillers_stretches() {
        let mut table = BTreeMap::from([(7, vec![seg(10, 20, reg(1))])]);
        let stacks: IndexMap<i64, Vec<(i64, i64)>> = [(-8, vec![(20, 40)])].into_iter().collect();
        add_cells(&mut table, [7, 8], |value| (value == 7).then_some(-8), &stacks);
        let found = locate(&[note(10, 1, NoteValue::Value(7))], &table, 50);
        assert_eq!(
            ranges(&found),
            [(10, 20, Where::Place(reg(1))), (20, 40, Where::Place(Place::Cell(-8))), (40, 50, Where::Undef)]
        );
        assert!(!table.contains_key(&8), "a value with no slot gains no entry");
    }
}
