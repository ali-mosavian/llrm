//! Facts keyed by memory cell, indexed by the object each cell lies in.
//!
//! Port of `qbopt/analysis/cellmap.py`. Python passes `bucket_of` and
//! `span_of` to the map; here the caller hands each new cell's bucket and
//! span to `insert`, since a bucket may come from a cache the caller also
//! mutates, and a bucket remembers each cell's start where Python asks
//! `span_of` again.

use std::collections::BTreeMap;
use std::hash::Hash;
use std::ops::Deref;

use crate::analysis::regions::ByteRange;
use crate::support::hash::{HashMap, HashSet, IndexMap};

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

/// One bucket's cells by the displacement they start at: Python `_Spans`.
///
/// Python keeps a list it sorts before a query; this is kept sorted, by
/// (low, the order cells came in). `widest` is the widest cell ever held, a
/// bound on how far below a write a cell meeting it can start; it does not
/// shrink when that cell goes.
#[derive(Clone)]
struct Spans<K> {
    cells: BTreeMap<(i128, u64), (i128, K)>,
    added: u64,
    widest: i128,
}

impl<K: Clone + Eq + Hash> Spans<K> {
    fn new() -> Self {
        Self { cells: BTreeMap::new(), added: 0, widest: 0 }
    }

    /// Index `key`, returning where it is held.
    fn add(&mut self, key: K, (low, high): ByteRange) -> (i128, u64) {
        self.added += 1;
        self.cells.insert((low, self.added), (high, key));
        self.widest = self.widest.max(high - low);
        (low, self.added)
    }

    fn remove(&mut self, at: (i128, u64)) {
        self.cells.remove(&at).expect("a held cell's start is indexed");
    }

    /// The cells whose bytes meet [low, high).
    fn meeting(&self, (low, high): ByteRange) -> impl Iterator<Item = &K> {
        self.cells
            .range((low - self.widest + 1, 0)..(high, 0))
            .filter(move |(_, (end, _))| low < *end)
            .map(|(_, (_, key))| key)
    }
}

/// A cell-to-fact map that also buckets its cells by object.
///
/// A write can only reach cells in objects it may alias, so `kill` tests
/// those buckets and never the rest. A cell's bucket must not change while
/// the cell is held. A bucket's cells with a span are also indexed by it,
/// for `kill`'s `displaced`.
#[derive(Clone)]
pub(crate) struct CellMap<K, V, B: Bucket> {
    items: IndexMap<K, V>,
    /// Each bucket's cells, with where its spans hold each spanned one.
    pub buckets: HashMap<B, IndexMap<K, Option<(i128, u64)>>>,
    pub parts: B::Parts,
    spans: HashMap<B, Spans<K>>,
}

impl<K: Clone + Eq + Hash, V, B: Bucket> CellMap<K, V, B> {
    pub(crate) fn new(items: IndexMap<K, V>, mut placed: impl FnMut(&K) -> (B, Option<ByteRange>)) -> Self {
        let mut map = Self {
            items: IndexMap::default(),
            buckets: HashMap::default(),
            parts: B::Parts::default(),
            spans: HashMap::default(),
        };
        for (key, value) in items {
            map.insert(key, value, &mut placed);
        }
        map
    }

    /// Python `__setitem__`: `placed`, a cell's bucket and span, is asked
    /// only of a new cell.
    pub(crate) fn insert(&mut self, key: K, value: V, placed: impl FnOnce(&K) -> (B, Option<ByteRange>)) {
        if !self.items.contains_key(&key) {
            let (bucket, span) = placed(&key);
            let keys = self.buckets.entry(bucket.clone()).or_insert_with(|| {
                bucket.held(&mut self.parts);
                IndexMap::default()
            });
            let at = span.map(|span| self.spans.entry(bucket).or_insert_with(Spans::new).add(key.clone(), span));
            keys.insert(key.clone(), at);
        }
        self.items.insert(key, value);
    }

    /// Delete every cell a write reaches.
    ///
    /// `reached` are the buckets the write may touch (every bucket when
    /// None); `overlaps` is the exact test, asked only of their cells.
    /// `displaced` is (buckets, span): a cell in those buckets can only be
    /// reached if its bytes meet the span, so only those are asked.
    pub(crate) fn kill(
        &mut self,
        reached: Option<Vec<B>>,
        mut overlaps: impl FnMut(&K) -> bool,
        displaced: Option<(HashSet<B>, ByteRange)>,
    ) {
        let mut doomed = Vec::new();
        let mut test = |bucket: &B| {
            let mut ask = |key: &K| {
                if overlaps(key) {
                    doomed.push((bucket.clone(), key.clone()));
                }
            };
            match self._spanned(bucket, displaced.as_ref()) {
                Some((spans, span)) => spans.meeting(span).for_each(&mut ask),
                None => self.buckets.get(bucket).into_iter().flat_map(IndexMap::keys).for_each(&mut ask),
            }
        };
        match &reached {
            None => self.buckets.keys().for_each(&mut test),
            Some(reached) => reached.iter().for_each(&mut test),
        }
        #[cfg(test)]
        if VERIFYING.with(std::cell::Cell::get) {
            let buckets = reached.clone().unwrap_or_else(|| self.buckets.keys().cloned().collect());
            let mut asked = HashSet::default();
            for bucket in &buckets {
                match self._spanned(bucket, displaced.as_ref()) {
                    Some((spans, span)) => asked.extend(spans.meeting(span)),
                    None => asked.extend(self.buckets.get(bucket).into_iter().flat_map(IndexMap::keys)),
                }
            }
            for key in self.items.keys().filter(|key| !asked.contains(key)) {
                VERIFIED.with(|checked| checked.set(checked.get() + 1));
                assert!(!overlaps(key), "a skipped cell is one the write reaches");
            }
        }
        if doomed.is_empty() {
            return;
        }
        for (bucket, key) in &doomed {
            let keys = self.buckets.get_mut(bucket).expect("a doomed cell's bucket is held");
            if let Some(at) = keys.swap_remove(key).expect("a doomed cell is held") {
                self.spans.get_mut(bucket).expect("a spanned cell's bucket is indexed").remove(at);
            }
            if keys.is_empty() {
                self.buckets.remove(bucket);
                self.spans.remove(bucket);
                bucket.released(&mut self.parts);
            }
        }
        // Python's `del` keeps the rest in order; so does one retain.
        let gone = doomed.into_iter().map(|(_, key)| key).collect::<HashSet<_>>();
        self.items.retain(|key, _| !gone.contains(key));
    }

    /// Python `_asked`'s first case: `bucket`'s spans, where `displaced` asks
    /// only the cells meeting its span.
    fn _spanned<'a>(
        &'a self,
        bucket: &B,
        displaced: Option<&(HashSet<B>, ByteRange)>,
    ) -> Option<(&'a Spans<K>, ByteRange)> {
        let (near, span) = displaced?;
        if !near.contains(bucket) {
            return None;
        }
        self.spans.get(bucket).map(|spans| (spans, *span))
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
