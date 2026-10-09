//! A whole body, emitted -- and everything that has to move when it does.
//!
//! Port of `qbopt/backend/layout.py`. Blocks are placed, implicit edges made
//! explicit and branches threaded; `asm` then measures, shrinks every branch
//! to a fixed point and emits. What comes back is bytes plus the relocations,
//! because a relocated displacement is emitted as zero and the fixup that
//! names it has to be moved to wherever the field ended up.
//!
//! Refuses the whole body where it cannot emit one op.
//!
//! Python's `id(op)` is the `Arc` pointer of an LIR occurrence.

use std::sync::LazyLock;

// The assembler's, re-exported: this module builds them and hands them over.
pub use crate::backend::asm::{Item, Laid, Table};
use crate::support::hash::IndexMap;

/// Each condition and the one that is true exactly when it is false. `jcxz`
/// and `loop*` are absent on purpose: they have no inverse to name, so a
/// block ending in one keeps its jump.
pub const _PAIRS: [(&str, &str); 15] = [
    ("je", "jne"),
    ("jz", "jnz"),
    ("jl", "jge"),
    ("jnge", "jnl"),
    ("jle", "jg"),
    ("jng", "jnle"),
    ("jb", "jae"),
    ("jc", "jnc"),
    ("jnae", "jnb"),
    ("jbe", "ja"),
    ("jna", "jnbe"),
    ("js", "jns"),
    ("jo", "jno"),
    ("jp", "jnp"),
    ("jpe", "jpo"),
];

pub static _OPPOSITE: LazyLock<IndexMap<&'static str, &'static str>> =
    LazyLock::new(|| _PAIRS.iter().flat_map(|&(one, other)| [(one, other), (other, one)]).collect());
