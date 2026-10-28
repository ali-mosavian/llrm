//! How wide each value is where an instruction names it.
//!
//! # How it works
//! [`_named_widths`] lists the (value, width) pairs one instruction names;
//! [`_widest`] keeps the largest per value over a body. [`_read_widths`] and
//! [`_widest_read`] count only the places an instruction reads, so a value no
//! instruction reads wider than 2 bytes never needs its high half, however
//! wide its definition is.
//!
//! # Used by
//! The greedy allocator's inputs (`greedy::shadow`), which size each value's
//! register and decide which values need only a half register.
//!
//! # Example
//! `v1 := mov32 [bp+8]; v2 := mov16 v1`: `v1` is named at 4 bytes and 2, so
//! [`_widest`] is 4; only 2 bytes of it are read, so [`_widest_read`] is 2.

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
            out.push((place.value, place.width));
        }
        for dest in &what.dests {
            for place in ir::values(dest) {
                if !matches!(dest, Loc::Held(_)) || one.uses.contains(&place.value) {
                    out.push((place.value, place.width));
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

/// Each (value, width) an instruction names.
pub(crate) fn _named_widths(one: &Insn) -> impl Iterator<Item = (u32, u32)> + '_ {
    one.what
        .iter()
        .flat_map(|what| what.dests.iter().chain(&what.sources))
        .flat_map(ir::values)
        .chain(one.requires.iter().chain(&one.delivers).map(|(place, _register)| *place))
        .map(|place| (place.value, place.width))
        .chain(one.widths.iter().copied())
}
