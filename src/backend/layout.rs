//! Port of `qbopt/backend/layout.py`, only what the C path reaches.
//!
//! `written` reaches nothing here; `_OPPOSITE` is ported for jumps.
//! Not ported yet: `_emits`, `_emitting`, `_following`, `_fallthroughs`,
//! `_turned`, `_inverted`, `_threaded`, `_emptied`, `_fallen`, `_ordered`,
//! `_trailing_zeros`, `PADDING`, `_padding_runs`, `selectable`, `lay_out`,
//! `_labels`, `_anchors`, `rebuild`, `_starts_at`, `_interleaved`.

use std::sync::LazyLock;

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

pub static _OPPOSITE: LazyLock<IndexMap<&'static str, &'static str>> = LazyLock::new(|| {
    _PAIRS.iter().flat_map(|&(one, other)| [(one, other), (other, one)]).collect()
});
