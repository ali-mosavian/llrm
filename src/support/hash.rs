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
pub type HashMap<K, V> = std::collections::HashMap<K, V, FxBuild>;
pub type HashSet<T> = std::collections::HashSet<T, FxBuild>;
