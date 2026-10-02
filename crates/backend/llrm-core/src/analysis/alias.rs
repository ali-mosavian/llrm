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
use crate::model::mir::{self, Arg, MemRef, MirBody, Value};
use crate::objectfile::module::{Addr, Space};

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

/// Python `named_bytes`' dict, keyed by address and by `(space, index)`:
/// the two key shapes it mixes, as two maps.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NamedBytes {
    pub at: IndexMap<Addr, (MemoryObject, i64)>,
    pub spaces: IndexMap<(Space, i64), MemoryObject>,
}

/// The object and offset each directly addressed byte is, as the body's own references name it.
///
/// Keyed by address, and by (space, index) for a space that is one object
/// at its own displacements. A cell known only by its address takes its
/// object from here, so it carries the same object facts every other
/// reference to it does.
pub fn named_bytes(body: &MirBody) -> NamedBytes {
    let mut out: IndexMap<Addr, Option<(MemoryObject, i64)>> = IndexMap::default();
    let mut refs: Vec<&MemRef> = body.initial.iter().map(|(reference, _)| reference).collect();
    for block in &body.blocks {
        for op in &block.ops {
            let cells = op.args.iter().chain(&op.results).filter_map(|arg| match arg {
                Arg::Cell(cell) => Some(&cell.r#ref),
                _ => None,
            });
            refs.extend(op.loads.iter().chain(&op.stores).chain(cells).chain(op.memory_values.iter().map(|(reference, _)| reference)));
        }
    }
    for reference in refs.into_iter().map(mir::symbolic_ref) {
        let (Some(provenance), None, None, Some(addr)) =
            (&reference.provenance, reference.base, reference.segment, reference.addr)
        else {
            continue;
        };
        if addr.base != iced_x86::Register::None || provenance.slices.len() != 1 {
            continue;
        }
        let one = provenance.slices.first().expect("one slice");
        if one.stride != 1 || one.high + one.width - 1 - one.low != i64::from(reference.width) {
            continue;
        }
        for byte in 0..i64::from(reference.width) {
            let (at, named) = (addr.plus(byte), (one.object.clone(), one.low + byte));
            let same = out.get(&at).is_none_or(|previous| previous.as_ref() == Some(&named));
            out.insert(at, same.then_some(named));
        }
    }
    let named: IndexMap<Addr, (MemoryObject, i64)> =
        out.into_iter().filter_map(|(at, one)| one.map(|one| (at, one))).collect();
    // A space whose every named byte is one object at its own displacement
    // is that object throughout: BC's segments and frame are.
    let mut spaces: IndexMap<(Space, i64), Option<MemoryObject>> = IndexMap::default();
    for (at, (object, offset)) in &named {
        let key = (at.space, at.index);
        let agrees = spaces.get(&key).is_none_or(|previous| previous.as_ref() == Some(object));
        spaces.insert(key, (*offset == at.disp && agrees).then(|| object.clone()));
    }
    NamedBytes {
        at: named,
        spaces: spaces.into_iter().filter_map(|(key, one)| one.map(|one| (key, one))).collect(),
    }
}
