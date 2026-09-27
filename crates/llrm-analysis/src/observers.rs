//! Adapted from llrm-core's `analysis/observers.rs`: which cells nothing
//! outside a function can read.
//!
//! A store is dead when everything after it overwrites the cell before
//! reading it, or when nothing after it can read the cell at all.
//! `avail::dead_stores` answers the first; this answers what the second
//! needs: at a call or an exit, which cells are still observable.
//!
//! A frame object is gone when the function returns, and nothing a call
//! runs can reach it unless its address escaped: `alias::points_to` says
//! which did, through a call, a return, a `ptrtoint` or a store outside the
//! frame.
//!
//! What changed with the IR:
//! - A global is never private. The old main body owned a program variable
//!   no other code named; a global here is reached by the module's other
//!   functions and, but for `internal` linkage, by other modules.
//! - Every access goes through a pointer. The old rule that an access
//!   through a pointer publishes what it reaches holds of one that does not
//!   name its bytes (`MemRef::named`); the raw frame offsets and the direct
//!   frame address operands are gone.
//! - Dropped: `Exposure` and the BC descriptors, data segment and error
//!   and event handlers. A handler here is an `invoke`'s unwind edge.

use std::collections::BTreeSet;

use crate::alias::PointsTo;
use crate::memory::{MemRef, MemoryKind, MemoryObject, Unit};

/// A test for the cells no call and no exit of `unit`'s function can
/// observe: each slice of the reference's provenance lies inside a frame
/// object whose address did not escape, and that no access reaches without
/// naming it.
pub fn private<'a>(unit: Unit<'a>, pointers: &'a PointsTo) -> impl Fn(&MemRef) -> bool + 'a {
    let published = _published(&unit, pointers);
    move |reference: &MemRef| {
        let provenance = reference.provenance.clone().or_else(|| pointers.reference(&unit, reference));
        provenance.is_some_and(|provenance| {
            !provenance.slices.is_empty()
                && provenance.slices.iter().all(|one| {
                    one.object.kind == MemoryKind::Frame
                        && one.object.extent.is_some_and(|extent| 0 <= one.low && one.low < one.high && one.high + one.width - 1 <= extent)
                        && !pointers.escaped.contains(&one.object)
                        && !published.contains(&one.object)
                })
        })
    }
}

/// Objects some load or store reaches through a value rather than by name:
/// what its provenance holds, the old `observers` rule for an access
/// through a pointer. A slot read back through its address kept in memory
/// is not private to its name.
fn _published(unit: &Unit, pointers: &PointsTo) -> BTreeSet<MemoryObject> {
    let mut published = BTreeSet::new();
    for (_, inst) in unit.function.walk() {
        let Some(reference) = MemRef::of(unit, inst).or_else(|| MemRef::filled(unit, inst)) else { continue };
        let Some(provenance) = reference.provenance.clone().or_else(|| pointers.reference(unit, &reference)) else { continue };
        if !(MemRef { provenance: Some(provenance.clone()), ..reference }).named() {
            published.extend(provenance.slices.into_iter().map(|one| one.object).filter(|one| one.kind != MemoryKind::Unknown));
        }
    }
    published
}

#[cfg(test)]
#[path = "observers_tests.rs"]
mod tests;
