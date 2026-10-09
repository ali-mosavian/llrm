//! The crate's hashed collections, on a fast non-cryptographic hasher.
//!
//! SipHash guards against adversarial keys; none reach a compiler's internal
//! maps. `IndexMap` order is insertion order whatever the hasher, and no
//! output may depend on a `HashMap`'s order.

use std::hash::BuildHasherDefault;

use rustc_hash::FxHasher;

pub type FxBuild = BuildHasherDefault<FxHasher>;
pub type IndexMap<K, V> = indexmap::IndexMap<K, V, FxBuild>;
pub type IndexSet<T> = indexmap::IndexSet<T, FxBuild>;

/// A map keyed by ids that holds a small fraction of a function's: a scope that knows a hundred of three thousand
/// values. A slot per id (`llrm_mir::dense::IdMap`) costs more than hashing here (a copy and a scan of mostly empty
/// slots: bounded 45% slower at nest depth 16, and a sorted vector 10%). The choice is deliberate and named once:
/// `dense-keys.txt`'s scan does not count it.
pub type SparseIdMap<K, V> = IndexMap<K, V>;
#[allow(clippy::disallowed_types)]
pub type HashMap<K, V> = std::collections::HashMap<K, V, FxBuild>;
#[allow(clippy::disallowed_types)]
pub type HashSet<T> = std::collections::HashSet<T, FxBuild>;
