//! Facts keyed by memory cell, indexed by the object each cell lies in.
//!
//! Port of `qbopt/analysis/cellmap.py`. Python passes `bucket_of` to the
//! map; here the caller hands each new cell's bucket to `insert`, since a
//! bucket may come from a cache the caller also mutates.

use std::hash::Hash;
use std::ops::Deref;

use crate::support::hash::{HashMap, HashSet, IndexMap, IndexSet};

/// A cell's bucket, and how it indexes itself: Python's `parts[i]`, the
/// buckets held by their i-th component, so a write can look its buckets
/// up rather than test each one.
pub(crate) trait Bucket: Clone + Eq + Hash {
    type Parts: Default + Clone;
    /// Index a bucket that just gained its first cell.
    fn held(&self, parts: &mut Self::Parts);
    /// Unindex a bucket that just lost its last cell.
    fn released(&self, parts: &mut Self::Parts);
}

/// A cell-to-fact map that also buckets its cells by object.
///
/// A write can only reach cells in objects it may alias, so `kill` tests
/// those buckets and never the rest. A cell's bucket must not change while
/// the cell is held.
#[derive(Clone)]
pub(crate) struct CellMap<K, V, B: Bucket> {
    items: IndexMap<K, V>,
    pub buckets: HashMap<B, IndexSet<K>>,
    pub parts: B::Parts,
}

impl<K: Clone + Eq + Hash, V, B: Bucket> CellMap<K, V, B> {
    pub(crate) fn new(items: IndexMap<K, V>, mut bucket_of: impl FnMut(&K) -> B) -> Self {
        let mut map = Self { items: IndexMap::default(), buckets: HashMap::default(), parts: B::Parts::default() };
        for (key, value) in items {
            map.insert(key, value, &mut bucket_of);
        }
        map
    }

    /// Python `__setitem__`: `bucket_of` is asked only of a new cell.
    pub(crate) fn insert(&mut self, key: K, value: V, bucket_of: impl FnOnce(&K) -> B) {
        if !self.items.contains_key(&key) {
            let bucket = bucket_of(&key);
            let keys = self.buckets.entry(bucket.clone()).or_insert_with(|| {
                bucket.held(&mut self.parts);
                IndexSet::default()
            });
            keys.insert(key.clone());
        }
        self.items.insert(key, value);
    }

    /// Delete every cell a write reaches.
    ///
    /// `reached` are the buckets the write may touch (every bucket when
    /// None); `overlaps` is the exact test, asked only of their cells.
    pub(crate) fn kill(&mut self, reached: Option<HashSet<B>>, mut overlaps: impl FnMut(&K) -> bool) {
        let mut doomed = Vec::new();
        let mut test = |bucket: &B, keys: &IndexSet<K>| {
            doomed.extend(keys.iter().filter(|key| overlaps(key)).map(|key| (bucket.clone(), key.clone())));
        };
        match &reached {
            None => self.buckets.iter().for_each(|(bucket, keys)| test(bucket, keys)),
            Some(reached) => reached
                .iter()
                .filter_map(|bucket| self.buckets.get(bucket).map(|keys| (bucket, keys)))
                .for_each(|(bucket, keys)| test(bucket, keys)),
        }
        #[cfg(test)]
        if let Some(reached) = &reached {
            if VERIFYING.with(std::cell::Cell::get) {
                for (bucket, keys) in &self.buckets {
                    if !reached.contains(bucket) {
                        VERIFIED.with(|checked| checked.set(checked.get() + keys.len()));
                        assert!(!keys.iter().any(&mut overlaps), "a skipped bucket holds a cell the write reaches");
                    }
                }
            }
        }
        if doomed.is_empty() {
            return;
        }
        for (bucket, key) in &doomed {
            let keys = self.buckets.get_mut(bucket).expect("a doomed cell's bucket is held");
            keys.swap_remove(key);
            if keys.is_empty() {
                self.buckets.remove(bucket);
                bucket.released(&mut self.parts);
            }
        }
        // Python's `del` keeps the rest in order; so does one retain.
        let gone = doomed.into_iter().map(|(_, key)| key).collect::<HashSet<_>>();
        self.items.retain(|key, _| !gone.contains(key));
    }

    pub(crate) fn into_items(self) -> IndexMap<K, V> {
        self.items
    }
}

#[cfg(test)]
thread_local! {
    /// While set, `kill` asks the exact test of every cell it skipped and
    /// counts them in `VERIFIED`: the index may only skip work.
    pub(crate) static VERIFYING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(crate) static VERIFIED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl<K, V, B: Bucket> Deref for CellMap<K, V, B> {
    type Target = IndexMap<K, V>;

    fn deref(&self) -> &IndexMap<K, V> {
        &self.items
    }
}

#[cfg(test)]
#[path = "cellmap_tests.rs"]
mod tests;
