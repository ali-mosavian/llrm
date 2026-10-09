//! The crate's hashed collections on a fixed-key hasher: std's `RandomState` reseeds every run, so a pass's work and
//! any order reaching output would vary (#992). The crate depends on nothing, so this is `DefaultHasher::new()`, not
//! `llrm_support::hash`'s Fx.

use std::collections::hash_map::DefaultHasher;
use std::hash::BuildHasherDefault;

#[allow(clippy::disallowed_types)]
pub type HashMap<K, V> = std::collections::HashMap<K, V, BuildHasherDefault<DefaultHasher>>;
#[allow(clippy::disallowed_types)]
pub type HashSet<T> = std::collections::HashSet<T, BuildHasherDefault<DefaultHasher>>;
