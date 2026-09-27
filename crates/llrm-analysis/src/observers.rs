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
//! - Every access goes through a pointer, and `avail` treats one whose
//!   provenance names its object as naming its cell. So the old rule that
//!   an access through a pointer publishes what it reaches is gone, as are
//!   the raw frame offsets and the direct frame address operands.
//! - Dropped: `Exposure` and the BC descriptors, data segment and error
//!   and event handlers. A handler here is an `invoke`'s unwind edge.

use crate::alias::PointsTo;
use crate::memory::{MemRef, MemoryKind, Unit};

/// A test for the cells no call and no exit of `unit`'s function can
/// observe: each slice of the reference's provenance lies inside a frame
/// object whose address did not escape.
pub fn private<'a>(unit: Unit<'a>, pointers: &'a PointsTo) -> impl Fn(&MemRef) -> bool + 'a {
    move |reference: &MemRef| {
        let provenance = reference.provenance.clone().or_else(|| pointers.reference(&unit, reference));
        provenance.is_some_and(|provenance| {
            !provenance.slices.is_empty()
                && provenance.slices.iter().all(|one| {
                    one.object.kind == MemoryKind::Frame
                        && one.object.extent.is_some_and(|extent| 0 <= one.low && one.low < one.high && one.high + one.width - 1 <= extent)
                        && !pointers.escaped.contains(&one.object)
                })
        })
    }
}

#[cfg(test)]
#[path = "observers_tests.rs"]
mod tests;
