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

use crate::model::lir::NoteValue;

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
/// the table does not know, or of a form this does not place yet (`Expr`,
/// `Arrived`), is `Undef` over its range.
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
                NoteValue::Value(value) | NoteValue::Offset(value, _) => {
                    let add = if let NoteValue::Offset(_, add) = note.value { Some(add) } else { None };
                    let mut from = start;
                    for segment in segments.get(value).into_iter().flatten() {
                        let (low, high) = (segment.start.max(start), segment.end.min(stop));
                        if low >= high {
                            continue;
                        }
                        push(from, low, Where::Undef);
                        push(
                            low,
                            high,
                            add.map_or(Where::Place(segment.place), |add| Where::Offset(segment.place, add)),
                        );
                        from = high;
                    }
                    push(from, stop, Where::Undef);
                }
                _ => push(start, stop, Where::Undef),
            }
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
}
