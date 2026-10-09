//! Maps and sets keyed by dense ids (`ValueId`, `InstId`, `BlockId`): a vector indexed by the id, as gcc's sbitmap and
//! LLVM's IndexedMap / SparseBitVector are. No hashing, no tree nodes, one allocation that grows by doubling. Iteration
//! is ascending by id, which is the order a `BTreeMap` of the same keys has.

use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

/// An id that is a position in a vector.
pub trait Dense: Copy {
    fn index(self) -> usize;
    fn at(index: usize) -> Self;
}

impl Dense for u32 {
    fn index(self) -> usize {
        self as usize
    }
    fn at(index: usize) -> Self {
        u32::try_from(index).expect("an id fits u32")
    }
}

impl Dense for usize {
    fn index(self) -> usize {
        self
    }
    fn at(index: usize) -> Self {
        index
    }
}

/// A set of ids, a bit each.
#[derive(Clone)]
pub struct IdSet<K> {
    words: Vec<u64>,
    len: usize,
    key: PhantomData<K>,
}

impl<K> Default for IdSet<K> {
    fn default() -> Self {
        Self { words: Vec::new(), len: 0, key: PhantomData }
    }
}

impl<K: Dense> IdSet<K> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn contains(
        &self,
        key: &K,
    ) -> bool {
        let at = key.index();
        self.words.get(at / 64).is_some_and(|word| word & (1 << (at % 64)) != 0)
    }

    /// Whether `key` was not in the set.
    pub fn insert(
        &mut self,
        key: K,
    ) -> bool {
        let at = key.index();
        if at / 64 >= self.words.len() {
            self.words.resize((at / 64 + 1).next_power_of_two(), 0);
        }
        let word = &mut self.words[at / 64];
        let fresh = *word & (1 << (at % 64)) == 0;
        *word |= 1 << (at % 64);
        self.len += usize::from(fresh);
        fresh
    }

    /// Whether `key` was in the set.
    pub fn remove(
        &mut self,
        key: &K,
    ) -> bool {
        let at = key.index();
        let Some(word) = self.words.get_mut(at / 64) else { return false };
        let had = *word & (1 << (at % 64)) != 0;
        *word &= !(1 << (at % 64));
        self.len -= usize::from(had);
        had
    }

    pub fn clear(&mut self) {
        self.words.fill(0);
        self.len = 0;
    }

    /// The members, ascending.
    pub fn iter(&self) -> impl Iterator<Item = K> + '_ {
        self.words
            .iter()
            .enumerate()
            .flat_map(
                |(at, word)| {
                    let mut left = *word;
                    std::iter::from_fn(move || {
                        (left != 0).then(|| {
                            let bit = left.trailing_zeros() as usize;
                            left &= left - 1;
                            K::at(at * 64 + bit)
                        })
                    })
                },
            )
    }
}

impl<K: Dense> FromIterator<K> for IdSet<K> {
    fn from_iter<I: IntoIterator<Item = K>>(keys: I) -> Self {
        let mut set = Self::new();
        set.extend(keys);
        set
    }
}

impl<K: Dense> Extend<K> for IdSet<K> {
    fn extend<I: IntoIterator<Item = K>>(
        &mut self,
        keys: I,
    ) {
        for key in keys {
            self.insert(key);
        }
    }
}

impl<K: Dense + std::fmt::Debug> std::fmt::Debug for IdSet<K> {
    fn fmt(
        &self,
        out: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        out.debug_set().entries(self.iter()).finish()
    }
}

impl<K: Dense> PartialEq for IdSet<K> {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.len == other.len && self.iter().map(Dense::index).eq(other.iter().map(Dense::index))
    }
}

impl<K: Dense> Eq for IdSet<K> {}

/// A map from ids, a slot each.
#[derive(Clone)]
pub struct IdMap<K, V> {
    slots: Vec<Option<V>>,
    len: usize,
    key: PhantomData<K>,
}

impl<K, V> Default for IdMap<K, V> {
    fn default() -> Self {
        Self { slots: Vec::new(), len: 0, key: PhantomData }
    }
}

impl<K: Dense, V> IdMap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Room for ids below `ids` without growing.
    pub fn with_capacity(ids: usize) -> Self {
        Self { slots: Vec::with_capacity(ids), len: 0, key: PhantomData }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(
        &self,
        key: &K,
    ) -> Option<&V> {
        self.slots.get(key.index())?.as_ref()
    }

    pub fn get_mut(
        &mut self,
        key: &K,
    ) -> Option<&mut V> {
        self.slots.get_mut(key.index())?.as_mut()
    }

    pub fn contains_key(
        &self,
        key: &K,
    ) -> bool {
        self.get(key).is_some()
    }

    /// The value `key` had.
    pub fn insert(
        &mut self,
        key: K,
        value: V,
    ) -> Option<V> {
        let slot = self.slot(key);
        let old = slot.replace(value);
        self.len += usize::from(old.is_none());
        old
    }

    pub fn remove(
        &mut self,
        key: &K,
    ) -> Option<V> {
        let old = self.slots.get_mut(key.index())?.take();
        self.len -= usize::from(old.is_some());
        old
    }

    /// `key`'s value, made by `make` where it has none.
    pub fn get_or_insert_with(
        &mut self,
        key: K,
        make: impl FnOnce() -> V,
    ) -> &mut V {
        let at = key.index();
        if self.get(&key).is_none() {
            self.insert(key, make());
        }
        self.slots[at].as_mut().expect("just made")
    }

    pub fn clear(&mut self) {
        self.slots.clear();
        self.len = 0;
    }

    /// The pairs, ascending by key.
    pub fn iter(&self) -> impl Iterator<Item = (K, &V)> + '_ {
        self.slots.iter().enumerate().filter_map(|(at, slot)| Some((K::at(at), slot.as_ref()?)))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (K, &mut V)> + '_ {
        self.slots.iter_mut().enumerate().filter_map(|(at, slot)| Some((K::at(at), slot.as_mut()?)))
    }

    pub fn keys(&self) -> impl Iterator<Item = K> + '_ {
        self.iter().map(|(key, _)| key)
    }

    pub fn values(&self) -> impl Iterator<Item = &V> + '_ {
        self.slots.iter().flatten()
    }

    fn slot(
        &mut self,
        key: K,
    ) -> &mut Option<V> {
        let at = key.index();
        if at >= self.slots.len() {
            self.slots.resize_with(at + 1, || None);
        }
        &mut self.slots[at]
    }
}

impl<K: Dense, V> FromIterator<(K, V)> for IdMap<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(pairs: I) -> Self {
        let mut map = Self::new();
        map.extend(pairs);
        map
    }
}

impl<K: Dense, V> Extend<(K, V)> for IdMap<K, V> {
    fn extend<I: IntoIterator<Item = (K, V)>>(
        &mut self,
        pairs: I,
    ) {
        for (key, value) in pairs {
            self.insert(key, value);
        }
    }
}

impl<K: Dense, V> Index<&K> for IdMap<K, V> {
    type Output = V;
    fn index(
        &self,
        key: &K,
    ) -> &V {
        self.get(key).expect("the key is in the map")
    }
}

impl<K: Dense, V> IndexMut<&K> for IdMap<K, V> {
    fn index_mut(
        &mut self,
        key: &K,
    ) -> &mut V {
        self.get_mut(key).expect("the key is in the map")
    }
}

impl<K: Dense + std::fmt::Debug, V: std::fmt::Debug> std::fmt::Debug for IdMap<K, V> {
    fn fmt(
        &self,
        out: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        out.debug_map().entries(self.iter()).finish()
    }
}

impl<K: Dense, V: PartialEq> PartialEq for IdMap<K, V> {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.len == other.len
            && self
                .iter()
                .map(|(key, value)| (key.index(), value))
                .eq(other.iter().map(|(key, value)| (key.index(), value)))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;

    /// A set agrees with a `BTreeSet` of the same inserts and removes: membership, count, ascending order, what each
    /// call returns.
    #[test]
    fn a_set_does_what_a_btreeset_does() {
        let (mut ours, mut theirs) = (IdSet::<u32>::new(), BTreeSet::new());
        let mut seed = 12345_u64;
        for _ in 0..4000 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let key = ((seed >> 33) % 700) as u32;
            if seed >> 62 == 0 {
                assert_eq!(ours.remove(&key), theirs.remove(&key));
            } else {
                assert_eq!(ours.insert(key), theirs.insert(key));
            }
            assert_eq!((ours.len(), ours.contains(&key)), (theirs.len(), theirs.contains(&key)));
        }
        assert!(ours.iter().eq(theirs.iter().copied()));
        assert!(!ours.contains(&1_000_000), "a key past the end is absent");
        assert_eq!(ours.clone(), ours);
    }

    /// A map agrees with a `BTreeMap`: what insert returns, lookups, the count after removals, ascending pairs.
    #[test]
    fn a_map_does_what_a_btreemap_does() {
        let (mut ours, mut theirs) = (IdMap::<u32, u64>::new(), BTreeMap::new());
        let mut seed = 99_u64;
        for step in 0..4000_u64 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let key = ((seed >> 33) % 500) as u32;
            if seed >> 62 == 0 {
                assert_eq!(ours.remove(&key), theirs.remove(&key));
            } else {
                assert_eq!(ours.insert(key, step), theirs.insert(key, step));
            }
            assert_eq!((ours.len(), ours.get(&key)), (theirs.len(), theirs.get(&key)));
        }
        assert!(ours.iter().map(|(key, value)| (key, *value)).eq(theirs.iter().map(|(key, value)| (*key, *value))));
        assert_eq!(*ours.get_or_insert_with(900, || 7), 7);
        assert_eq!(*ours.get_or_insert_with(900, || 8), 7, "made once");
        assert!(ours.get(&2_000_000).is_none());
    }
}
