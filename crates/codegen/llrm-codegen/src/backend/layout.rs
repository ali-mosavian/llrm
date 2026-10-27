//! The pairs of branch conditions that are each other's inverse.
//!
//! # How it works
//! `_PAIRS` lists each condition mnemonic with the one that is true exactly
//! when it is false (`je`/`jne`, `jb`/`jae`, and the aliases of each).
//! `_OPPOSITE` is the table in both directions. `jcxz` and `loop*` are absent
//! on purpose: they have no inverse to name, so a block ending in one keeps its
//! jump.
//!
//! # Example
//! No example: a table of condition names. See `jumps.rs` for the inversions it
//! serves.
//!
//! # Used by
//! `jumps`, to invert a branch when it makes the other successor the
//! fall-through.

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
