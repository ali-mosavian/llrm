//! Strong, flow-sensitive alias analysis over MIR.
//!
//! Direct port of `qbopt/analysis/alias.py`.  The analysis has one
//! vocabulary for every frontend: canonical objects, subobject byte slices,
//! pointer provenance and C restrict roots.  Pointer facts flow through SSA
//! phis and through exact pointer spill slots.  Unknown stores kill spill
//! facts; they never manufacture a disjointness proof.

use std::collections::BTreeSet;

use crate::support::hash::IndexMap;

use super::cellmap::{Bucket, CellMap};
use super::regions::ByteRange;
use crate::model::memory::{Identity, MemoryObject, Provenance, Slice};
use crate::model::mir::{MirBody, Value};
use crate::objectfile::module::Space;

/// One element of Python's `Procedure.arguments` tuples, which hold a
/// provenance, a `(pointer value, displacement)` pair or `None`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Actual {
    Provenance(Provenance),
    Pointer(Value, i64),
    Absent,
}

/// Python's `_cell_key` tuples: `(object, low, high)` or
/// `(space, index, disp, width)`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CellKey {
    Object(MemoryObject, i64, i64),
    Address(Space, i64, i64, i64),
}

/// One procedure's transitive memory effects in its own object space.
///
/// Python `qbopt.analysis.alias:Summary`.  `captures` holds PARAMETER object
/// identities, which Python does not restrict to ints.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Summary {
    pub reads: BTreeSet<Slice>,
    pub writes: BTreeSet<Slice>,
    pub captures: BTreeSet<Option<Identity>>,
    pub unknown_read: bool,
    pub unknown_write: bool,
}

impl Summary {
}

/// Python `qbopt.analysis.alias:Procedure`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Procedure {
    pub body: MirBody,
    pub calls: IndexMap<i64, String>,
    pub arguments: IndexMap<i64, Vec<Actual>>,
    /// Objects no pointer reaches that a callee outside the unit names.
    pub named: BTreeSet<MemoryObject>,
    /// Of `named`, what each outside callee whose writes are known writes.
    /// Any other outside callee writes them all.
    pub outside: IndexMap<String, BTreeSet<MemoryObject>>,
}

/// The object a cell key lies in: only keys sharing it can overlap.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum KeyBucket {
    Object(MemoryObject),
    Address(Space, i64),
}

impl Bucket for KeyBucket {
    // Nothing looks a key bucket up by a component.
    type Parts = ();

    fn held(&self, _: &mut ()) {}

    fn released(&self, _: &mut ()) {}
}

pub fn _key_bucket(key: &CellKey) -> KeyBucket {
    match key {
        CellKey::Object(object, _, _) => KeyBucket::Object(object.clone()),
        CellKey::Address(space, index, _, _) => KeyBucket::Address(*space, *index),
    }
}

/// A key's bucket, and no span: Python's map has no `span_of`.
pub fn _key_place(key: &CellKey) -> (KeyBucket, Option<ByteRange>) {
    (_key_bucket(key), None)
}

/// Drop the cells a store to `key` may overwrite, keeping `key` itself.
pub fn _kill<V>(cells: &mut CellMap<CellKey, V, KeyBucket>, key: Option<&CellKey>) {
    let reached = key.map(|key| std::iter::once(_key_bucket(key)).collect());
    cells.kill(
        reached,
        |old| {
            #[cfg(any(test, feature = "testing"))]
            ASKED.with(|asked| asked.borrow_mut().push(old.clone()));
            Some(old) != key && _keys_overlap(Some(old), key)
        },
        None,
    );
}

#[cfg(any(test, feature = "testing"))]
thread_local! {
    /// The cells `_kill` asked the exact test of.
    pub static ASKED: std::cell::RefCell<Vec<CellKey>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub fn _keys_overlap(one: Option<&CellKey>, other: Option<&CellKey>) -> bool {
    let (Some(one), Some(other)) = (one, other) else {
        return true;
    };
    match (one, other) {
        (CellKey::Address(space, index, disp, width), CellKey::Address(other_space, other_index, other_disp, other_width))
            if (space, index) == (other_space, other_index) =>
        {
            disp < &(other_disp + other_width) && other_disp < &(disp + width)
        }
        (CellKey::Object(object, low, high), CellKey::Object(other_object, other_low, other_high))
            if object == other_object =>
        {
            low < other_high && other_low < high
        }
        _ => one == other,
    }
}
