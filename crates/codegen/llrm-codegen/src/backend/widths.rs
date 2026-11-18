//! How wide each value is where an instruction names it.
//!
//! # How it works
//! [`_named_widths`] lists the (value, width) pairs one instruction names;
//! [`_widest`] keeps the largest per value over a body. [`_read_widths`] and
//! [`_widest_read`] count only the places an instruction reads, so a value no
//! instruction reads wider than 2 bytes never needs its high half, however
//! wide its definition is.
//!
//! [`_zero_extended`] lists the values every definition of which is a write the
//! target extends to the whole register, so their upper bytes are zero.
//!
//! # Used by
//! The greedy allocator's inputs (`greedy::shadow`), which size each value's
//! register and decide which values need only a half register.
//!
//! # Example
//! `v1 := mov32 [bp+8]; v2 := mov16 v1`: `v1` is named at 4 bytes and 2, so
//! [`_widest`] is 4; only 2 bytes of it are read, so [`_widest_read`] is 2.

use std::sync::Arc;

use crate::model::ir::{self, Loc};
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::IndexMap;

/// How wide each value is anywhere it is read or written.
pub fn _widest(body: &LirBody) -> IndexMap<u32, u32> {
    let mut out: IndexMap<u32, u32> = IndexMap::default();
    for block in &body.blocks {
        for one in &block.insns {
            for (value, width) in _named_widths(one) {
                let widest = out.entry(value).or_insert(0);
                *widest = (*widest).max(width);
            }
        }
    }
    out
}

/// The (value, width) each place an instruction reads is: what its definition
/// writes does not count. A destination in memory reads its address; a
/// destination the instruction also uses (a partial write) is read at its own
/// width.
pub(crate) fn _read_widths(one: &Insn) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    if let Some(what) = &one.what {
        for place in what.sources.iter().flat_map(ir::values) {
            out.push((place.value, place.offset + place.width));
        }
        for dest in &what.dests {
            for place in ir::values(dest) {
                if !matches!(dest, Loc::Held(_)) || one.uses.contains(&place.value) {
                    out.push((place.value, place.offset + place.width));
                }
            }
        }
    }
    out.extend(one.requires.iter().map(|(place, _)| (place.value, place.width)));
    out.extend(one.widths.iter().copied());
    out
}

/// The widest each value is read (bytes): a value no instruction reads wider
/// than 2 bytes never needs its high half, however wide its definition is.
pub fn _widest_read(body: &LirBody) -> IndexMap<u32, u32> {
    let mut out: IndexMap<u32, u32> = IndexMap::default();
    for one in body.blocks.iter().flat_map(|block| block.insns.iter()) {
        for (value, width) in _read_widths(one) {
            let widest = out.entry(value).or_insert(0);
            *widest = (*widest).max(width);
        }
    }
    out
}

/// The values whose every definition writes them as a plain destination of a
/// width the target extends to the value's whole width (`zeroes(written,
/// whole)`: a dword write zeroes the upper half of a 64-bit register). Their
/// upper bytes are zero wherever the value lives, so a copy of the low bytes
/// is the whole value.
pub fn _zero_extended<'a>(
    insns: impl Iterator<Item = &'a Arc<Insn>> + Clone,
    widest: &IndexMap<u32, u32>,
    zeroes: &dyn Fn(u32, u32) -> bool,
) -> crate::support::hash::HashSet<u32> {
    let mut out: crate::support::hash::HashSet<u32> =
        widest.iter().filter(|(_, whole)| **whole > 1).map(|(value, _)| *value).collect();
    for one in insns.clone() {
        for &value in &one.defines {
            let Some(&whole) = widest.get(&value) else { continue };
            let plain = one.group.is_none()
                && !one.partial_defs.contains(&value)
                && !one.def_lanes.iter().any(|(lane, _)| *lane == value)
                && !one.requires.iter().chain(&one.delivers).any(|(held, _)| held.value == value)
                && one.what.as_ref().is_some_and(|what| {
                    matches!(
                        what.dests.iter().find(|loc| matches!(loc, Loc::Held(held) if held.value == value)),
                        Some(Loc::Held(held)) if held.offset == 0 && held.width < whole && zeroes(held.width, whole)
                    )
                });
            if !plain {
                out.remove(&value);
            }
        }
    }
    // A value no instruction defines (an argument) is not known to be zero.
    let defined: crate::support::hash::HashSet<u32> = insns.flat_map(|one| one.defines.iter().copied()).collect();
    out.retain(|value| defined.contains(value));
    out
}

/// Each (value, width) an instruction names.
pub(crate) fn _named_widths(one: &Insn) -> impl Iterator<Item = (u32, u32)> + '_ {
    one.what
        .iter()
        .flat_map(|what| what.dests.iter().chain(&what.sources))
        .flat_map(ir::values)
        .chain(one.requires.iter().chain(&one.delivers).map(|(place, _register)| *place))
        // A lane of a value (`offset` bytes up) is named at its extent: a pair
        // only its bytes are copied into is still a pair, not a byte.
        .map(|place| (place.value, place.offset + place.width))
        .chain(one.widths.iter().copied())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::model::ir::{Held, Loc, Operation, Semantics};

    /// `v1:1@1 := mov v2:1` copies a byte into the high byte of a pair: the
    /// pair is named two bytes wide there. Named by its own width alone it was
    /// one byte, a value only bytes were copied into got a byte register's
    /// order, and its clone was placed in `d` and reloaded 16 bits wide (lz4c
    /// -Oz on Z80: "d has no byte halves").
    #[test]
    fn test_a_lane_is_named_at_its_extent() {
        let what = Semantics {
            name: Some("mov".into()),
            dests: vec![Loc::Held(Held { value: 1, width: 1, offset: 1 })],
            sources: vec![Loc::Held(Held { value: 2, width: 1, offset: 0 })],
            ..Semantics::new(Operation::Move)
        };
        let copy = Arc::new(Insn::new(0, None, Some(what), vec![1], vec![2]));
        let named: Vec<(u32, u32)> = _named_widths(&copy).collect();
        assert!(named.contains(&(1, 2)), "the pair is two bytes wide there: {named:?}");
        assert!(named.contains(&(2, 1)), "the byte stays one: {named:?}");
    }

    fn write(
        name: &str,
        value: u32,
        width: u32,
    ) -> Arc<Insn> {
        let what = Semantics {
            name: Some(name.into()),
            dests: vec![Loc::Held(Held { value, width, offset: 0 })],
            sources: Vec::new(),
            ..Semantics::new(Operation::Move)
        };
        Arc::new(Insn::new(0, None, Some(what), vec![value], Vec::new()))
    }

    /// `v1` is set by `mov r32, 1` only and read whole; `v2` also by a
    /// qword write. The jpeg_single spill of `v1` took 8 bytes (+48 B in all).
    #[test]
    fn test_a_value_set_only_by_dword_writes_is_zero_extended() {
        let insns = [write("mov", 1, 4), write("add", 1, 4), write("mov", 2, 4), write("mov", 2, 8)];
        let widest: IndexMap<u32, u32> = [(1, 8), (2, 8)].into_iter().collect();
        let zero = _zero_extended(insns.iter(), &widest, &|written, whole| written == 4 && whole == 8);
        assert!(zero.contains(&1), "{zero:?}");
        assert!(!zero.contains(&2), "a qword write sets the upper half: {zero:?}");
    }
}
