//! The result of code layout: a body's bytes, where each instruction landed and
//! the relocations that must follow, as the object writer needs them.
//!
//! # How it works
//! `layout.rs` places the blocks, measures, shrinks branches to a fixed point
//! and emits; it returns a `Laid`. The types live here and `layout` re-exports
//! them.
//!
//! - `Laid` is the code bytes, where each instruction ended up (`moved`, old
//!   address to new), each relocated displacement as (offset in `code`, the
//!   field's original address) so its fixup can follow, the fixups whose
//!   instruction no longer exists (`dropped`), the addresses a transform folded
//!   into a surviving instruction (`covered`; a branch never resolves through
//!   it), and the symbols.
//! - `Item` is an instruction to encode (`Op`) or a `Table`.
//! - `Table` is a run of bytes between instructions that is copied, not
//!   selected: an `ON GOTO` table of a count byte and one relocated word per
//!   target. Only its fixups move.
//!
//! `dropped` is reported rather than omitted so `objbuild` can tell a fixup
//! meant to go from one it cannot place, which it refuses.
//!
//! # Example
//! No honest example: the module declares types and has no behaviour. Read
//! `layout.rs`, which builds a `Laid`, for the bytes and fixups of a real body.
//!
//! # Used by
//! `layout.rs`, `objbuild.rs`.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::model::lir::Insn;
use crate::objectfile::module::Addr;
use crate::support::hash::IndexMap;

/// A body's new bytes, and what moved.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Laid {
    pub code: Vec<u8>,
    /// Where each op ended up, old address -> new.
    pub moved: IndexMap<i64, i64>,
    /// (offset within `code`, the original field's own address) for every
    /// relocated displacement, so the fixup that names it can be moved.
    pub relocations: Vec<(i64, i64)>,
    /// Fixups that belonged to an instruction this body no longer contains.
    /// Reported rather than silently omitted: objbuild refuses a fixup it
    /// cannot place, and can only tell the two apart if told which were
    /// meant to go.
    pub dropped: BTreeSet<i64>,
    /// Every original address a transform folded into a surviving op, mapped
    /// to where that op went. A branch target never resolves through this.
    pub covered: IndexMap<i64, i64>,
    pub symbols: Vec<(i64, Addr)>,
}

impl Laid {}

/// A run of bytes between the instructions, copied rather than selected.
///
/// BC drops an ON GOTO table inline: a count byte and one relocated word
/// per destination. Copying the bytes and moving the fixups is enough.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Table {
    pub lo: i64,
    pub hi: i64,
    pub discarded: bool,
}

impl Table {}

/// An op, which select encodes, or a Table, which is copied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Item {
    Op(Arc<Insn>),
    Table(Table),
}

impl Item {}
