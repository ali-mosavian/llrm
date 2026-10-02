//! The assembler: instructions in, one image and its relocations out.
//!
//! Port of `qbopt/backend/asm.py`. LLVM's `MCAssembler`: how long each
//! instruction is, where each therefore lands, which branches can shrink now
//! that everything is closer, and where each fixup ended up. `select` is the
//! code emitter above it and `omfwrite` the object writer below.
//!
//! An assembler does not allocate. Handed `assignment=None` this remaps
//! nothing, which is the right answer for a body whose registers are already
//! chosen.
//!
//! Python's `id(op)` is the `Arc` pointer of an LIR occurrence.

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
    /// Reported rather than silently omitted: omfwrite refuses a fixup it
    /// cannot place, and can only tell the two apart if told which were
    /// meant to go.
    pub dropped: BTreeSet<i64>,
    /// Every original address a transform folded into a surviving op, mapped
    /// to where that op went. A branch target never resolves through this.
    pub covered: IndexMap<i64, i64>,
    pub symbols: Vec<(i64, Addr)>,
}

impl Laid {
}

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

impl Table {
}

/// An op, which select encodes, or a Table, which is copied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Item {
    Op(Arc<Insn>),
    Table(Table),
}

impl Item {
}
