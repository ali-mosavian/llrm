//! Port of `qbopt/analysis/avail.py`: which value holds a cell's contents,
//! across blocks.
//!
//! A forward dataflow whose fact is
//!
//!     MemRef -> Value
//!
//! meaning "these bytes are this SSA value". Keyed on MemRef rather than on
//! Addr because MemRef carries the SSA values its own address is reached
//! through, so the fact survives anything that reallocates registers -- which
//! is the whole reason to state it over MIR instead of over machine code.
//!
//! An entry identifies an SSA value, not a physical register. Forwarding adds
//! a use and extends that value's lifetime; allocation preserves or spills it
//! as needed. Requiring it to be live already would retain BC's statement-local
//! lifetimes instead of optimizing them.
//!
//! This lattice intersects at joins. Where predecessor values differ,
//! `optimize/loadjoins.rs` can form a value phi using MemorySSA's per-edge
//! availability proof. Lowering and allocation handle that phi normally.

#![allow(private_interfaces)] // `RegionLayout` and `Interval` are crate-private types.

use std::collections::BTreeMap;

use crate::support::hash::IndexMap;

use super::ranges::Interval;
use crate::model::mir::{Const, MemRef, Symbol, Value};

/// What a cell maps to: the value, or the constant a store wrote outright.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Holder {
    Value(Value),
    Const(Const),
    Symbol(Symbol),
}

// What a cell maps to, and the whole lattice element.
pub type Holders = IndexMap<MemRef, Holder>;

/// The map on entry to and exit from each block.
#[derive(Clone, Debug)]
pub struct Held {
    pub into: IndexMap<i64, Holders>,
    pub outof: IndexMap<i64, Holders>,
    // Each value's constant, so a far access through a literal selector is
    // not taken to reach the frame.
    pub known: BTreeMap<Value, Interval>,
}

#[cfg(any(test, feature = "testing"))]
thread_local! {
    /// Overlap tests dead stores asked, for the test that pins its index.
    pub static DEAD_OVERLAPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
