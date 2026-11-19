//! The entries of each variable the debug writer reads
//! (`calculateDbgEntityHistory`): a value from where a range opens, and a
//! clobber where one closes with no range following at once. The writer
//! writes a variable of one entry, or of one entry and its clobber, as a
//! single expression and any other as a location list
//! (`DwarfDebug::collectEntityInfo`).
//!
//! DEVIATE: the pin reads the `DBG_VALUE`s in the final code; this reads the
//! closed ranges `extend` reports.

use std::collections::BTreeMap;

use super::Var;
use crate::backend::livedebugvariables::{Located, Where};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Entry {
    Value { at: i64, place: Where },
    Clobber { at: i64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Form {
    Single,
    List,
}

/// Each variable's entries in position order.
/// `code_end`: a range that reaches it is open to the end and has no clobber.
pub fn history(
    ranges: &[Located],
    code_end: i64,
) -> BTreeMap<Var, Vec<Entry>> {
    let mut sorted: Vec<&Located> = ranges.iter().collect();
    sorted.sort_by_key(|one| (one.variable, one.piece, one.start));
    let mut out: BTreeMap<Var, Vec<Entry>> = BTreeMap::new();
    for one in sorted {
        let entries = out.entry(Var { variable: one.variable, piece: one.piece }).or_default();
        // A range that starts where the last ended: the variable was never
        // nowhere.
        if entries.last() == Some(&Entry::Clobber { at: one.start }) {
            entries.pop();
        }
        entries.push(Entry::Value { at: one.start, place: one.at });
        if one.end < code_end {
            entries.push(Entry::Clobber { at: one.end });
        }
    }
    out
}

/// How the writer writes a variable of these entries.
pub fn form(entries: &[Entry]) -> Form {
    match entries {
        [Entry::Value { .. }] | [Entry::Value { .. }, Entry::Clobber { .. }] => Form::Single,
        _ => Form::List,
    }
}

/// The ranges of a variable's entries (`DwarfDebug::buildLocationList`): a
/// value runs to the next entry; to the end of the instruction a clobber is
/// at, since that instruction still reads the place; to `code_end` when no
/// entry follows. `after` is the end of the instruction at a position. A
/// range with no length is dropped.
pub fn locations(
    entries: &[Entry],
    code_end: i64,
    after: &dyn Fn(i64) -> i64,
) -> Vec<(i64, i64, Where)> {
    let mut out = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let Entry::Value { at, place } = *entry else { continue };
        let end = match entries.get(index + 1) {
            Some(Entry::Clobber { at: clobbered }) => after(*clobbered),
            Some(Entry::Value { at: next, .. }) => *next,
            None => code_end,
        };
        if at < end {
            out.push((at, end, place));
        }
    }
    out
}

/// The ranges of every variable as byte offsets into the function, the way
/// the writer reads them: entries (`history`), then ranges
/// (`locations`), a clobbered range ending after the clobbering instruction.
/// `insns` is each instruction's slot and id in code order, `offset` the
/// byte offset an id's code starts at, `code_len` the function's size.
pub fn in_code(
    ranges: &[Located],
    insns: &[(i64, u64)],
    offset: &dyn Fn(u64) -> Option<usize>,
    code_len: usize,
) -> BTreeMap<Var, Vec<(usize, usize, Where)>> {
    let end = insns.last().map_or(0, |&(slot, _)| slot + 1);
    let byte = |slot: i64| -> Option<usize> {
        if slot >= end {
            return Some(code_len);
        }
        // An instruction a later pass removed has no code: the next one's.
        let at = insns.partition_point(|&(one, _)| one < slot);
        insns[at..].iter().find_map(|&(_, id)| offset(id)).or(Some(code_len))
    };
    // The instruction after the one at `slot`: where it stops being read.
    let after = |slot: i64| insns.iter().map(|&(one, _)| one).find(|&one| one > slot).unwrap_or(end);
    let mut out = BTreeMap::new();
    for (var, entries) in history(ranges, end) {
        let found: Vec<_> = locations(&entries, end, &after)
            .into_iter()
            .filter_map(|(from, to, place)| Some((byte(from)?, byte(to)?, place)))
            .filter(|(from, to, _)| from < to)
            .collect();
        if !found.is_empty() {
            out.insert(var, found);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use llrm_lir::registers::RegId;

    use super::*;
    use crate::backend::livedebugvariables::Place;

    fn range(
        variable: u32,
        start: i64,
        end: i64,
        register: u8,
    ) -> Located {
        let at = Where::Place(Place::Register(RegId::from_raw(register)));
        Located { variable, piece: None, start, end, at }
    }

    fn var(variable: u32) -> Var {
        Var { variable, piece: None }
    }

    /// A parameter in one register until the register is written has a value
    /// and then a clobber: the writer makes it an expression. Without the
    /// clobber the entries read as one range running to the end of the code.
    #[test]
    fn a_range_with_nothing_after_it_ends_in_a_clobber() {
        let found = history(&[range(1, 0, 14, 5)], 100);
        let place = Where::Place(Place::Register(RegId::from_raw(5)));
        assert_eq!(found[&var(1)], [Entry::Value { at: 0, place }, Entry::Clobber { at: 14 }]);
    }

    /// A range that starts where the one before ended is a second value, not
    /// a clobber and a value: the register changed, nothing was lost.
    #[test]
    fn a_range_that_follows_at_once_is_a_second_value() {
        let found = history(&[range(1, 0, 10, 5), range(1, 10, 20, 0)], 100);
        assert!(matches!(
            found[&var(1)].as_slice(),
            [Entry::Value { at: 0, .. }, Entry::Value { at: 10, .. }, Entry::Clobber { at: 20 }]
        ));
    }

    /// A range open to the end of the code is not clobbered there: the
    /// parameter that is never written is one entry, an expression.
    #[test]
    fn a_range_to_the_end_of_the_code_has_no_clobber() {
        let found = history(&[range(1, 0, 100, 5)], 100);
        assert!(matches!(found[&var(1)].as_slice(), [Entry::Value { at: 0, .. }]));
    }

    /// One value, or one value and its clobber, is a single expression: the
    /// pin's `HistSize == 1 || (HistSize == 2 && entries[1].isClobber())`.
    #[test]
    fn one_value_and_its_clobber_is_a_single_expression_and_two_values_are_a_list() {
        let place = Where::Place(Place::Register(RegId::from_raw(5)));
        let value = |at| Entry::Value { at, place };
        assert_eq!(form(&[value(0)]), Form::Single);
        assert_eq!(form(&[value(0), Entry::Clobber { at: 9 }]), Form::Single);
        assert_eq!(form(&[value(0), value(4)]), Form::List);
        assert_eq!(form(&[value(0), Entry::Clobber { at: 4 }, value(6)]), Form::List);
    }

    /// A parameter in `rdi` until an instruction at 9 writes it: the range
    /// runs to the end of that instruction (15), as in the pin, not to its
    /// start. Ending at the start left the instruction that still reads the
    /// register without a location.
    #[test]
    fn a_clobbered_range_ends_with_the_instruction_that_clobbers() {
        let place = Where::Place(Place::Register(RegId::from_raw(5)));
        let found = locations(&[Entry::Value { at: 0, place }, Entry::Clobber { at: 9 }], 100, &|at| at + 6);
        assert_eq!(found, [(0, 15, place)]);
    }

    /// A value followed by another ends where the next begins; the last one
    /// runs to the end of the code.
    #[test]
    fn a_value_runs_to_the_next_entry_and_the_last_to_the_end_of_the_code() {
        let (one, two) =
            (Where::Place(Place::Register(RegId::from_raw(5))), Where::Place(Place::Register(RegId::from_raw(0))));
        let entries = [Entry::Value { at: 0, place: one }, Entry::Value { at: 10, place: two }];
        assert_eq!(locations(&entries, 40, &|at| at + 1), [(0, 10, one), (10, 40, two)]);
    }

    /// A clobber at the position a value starts leaves a range of no length.
    #[test]
    fn a_range_of_no_length_is_dropped() {
        let place = Where::Place(Place::Register(RegId::from_raw(5)));
        assert_eq!(locations(&[Entry::Value { at: 7, place }, Entry::Clobber { at: 7 }], 40, &|at| at), []);
    }
    /// The writer joins on bytes, the allocator on slots: a parameter in one
    /// register until the second instruction writes it reads as the bytes
    /// of the first two instructions (a clobbered range ends after the
    /// clobbering instruction), and a variable's range to the end of the
    /// code stops at the function's size.
    #[test]
    fn slot_ranges_become_byte_ranges_of_the_function() {
        let insns = [(0, 100), (4, 101), (8, 102)];
        let offsets = |id: u64| Some([0usize, 3, 7][(id - 100) as usize]);
        let place = Where::Place(Place::Register(RegId::from_raw(5)));
        let clobbered = in_code(&[range(1, 0, 4, 5)], &insns, &offsets, 10);
        assert_eq!(clobbered[&var(1)], [(0, 7, place)]);
        let open = in_code(&[range(2, 4, 100, 5)], &insns, &offsets, 10);
        assert_eq!(open[&var(2)], [(3, 10, place)]);
    }

    /// A later pass removed the instruction a range starts at: it starts at
    /// the code of the next one that is left, not nowhere.
    #[test]
    fn a_range_at_a_removed_instruction_starts_at_the_next_one_left() {
        let insns = [(0, 100), (4, 101), (8, 102)];
        let offsets = |id: u64| [(100, 0usize), (102, 7)].into_iter().find(|&(one, _)| one == id).map(|(_, at)| at);
        let place = Where::Place(Place::Register(RegId::from_raw(5)));
        let found = in_code(&[range(1, 4, 100, 5)], &insns, &offsets, 10);
        assert_eq!(found[&var(1)], [(7, 10, place)]);
    }
}
