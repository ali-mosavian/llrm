//! Maps and sets keyed by dense ids (`ValueId`, `InstId`, `BlockId`): a vector
//! indexed by the id, as gcc's sbitmap and LLVM's IndexedMap / SparseBitVector
//! are. No hashing, no tree nodes, one allocation that grows by doubling.
//! Iteration is ascending by id, which is the order a `BTreeMap` of the same
//! keys has.

use std::marker::PhantomData;
use std::ops::{Index, IndexMut};
use std::rc::Rc;

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

/// A block's address (`cfg::id`): its position in the function, never negative.
impl Dense for i64 {
    fn index(self) -> usize {
        usize::try_from(self).expect("a dense address is not negative")
    }
    fn at(index: usize) -> Self {
        i64::try_from(index).expect("an id fits i64")
    }
}

/// `llvm::SetVector` over dense ids: insertion order kept, membership and
/// removal of an absent id O(1). A worklist that is told to forget an id
/// (SROA's `set_subtract` of the allocas it deleted) pays only when it holds
/// it.
#[derive(Clone)]
pub struct IdSetVector<K> {
    order: Vec<K>,
    members: IdSet<K>,
}

impl<K> Default for IdSetVector<K> {
    fn default() -> Self {
        Self { order: Vec::new(), members: IdSet::default() }
    }
}

impl<K: Dense + Copy + PartialEq> IdSetVector<K> {
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn contains(
        &self,
        key: &K,
    ) -> bool {
        self.members.contains(key)
    }

    /// Whether `key` was not in it (`insert`).
    pub fn push(
        &mut self,
        key: K,
    ) -> bool {
        let fresh = self.members.insert(key);
        if fresh {
            self.order.push(key);
        }
        fresh
    }

    /// The last pushed (`pop_back_val`).
    pub fn pop(&mut self) -> Option<K> {
        let key = self.order.pop()?;
        self.members.remove(&key);
        Some(key)
    }

    /// Whether `key` was in it (`remove`).
    pub fn remove(
        &mut self,
        key: &K,
    ) -> bool {
        if !self.members.remove(key) {
            return false;
        }
        let at = self.order.iter().rposition(|one| one == key).expect("a member is in the order");
        self.order.remove(at);
        true
    }

    /// Forgets every id of `gone` it holds (`set_subtract`). A list, not an
    /// `IdSet`: walking a bitset costs its width, not its members.
    pub fn set_subtract(
        &mut self,
        gone: impl IntoIterator<Item = K>,
    ) {
        for key in gone {
            self.remove(&key);
        }
    }

    /// Keeps the first `len` (`truncate`).
    pub fn truncate(
        &mut self,
        len: usize,
    ) {
        for key in self.order.drain(len.min(self.order.len())..) {
            self.members.remove(&key);
        }
    }

    /// Only the ids `keep` says to; O(n), for a pass over the whole list.
    pub fn retain(
        &mut self,
        mut keep: impl FnMut(&K) -> bool,
    ) {
        let members = &mut self.members;
        self.order.retain(|key| {
            let kept = keep(key);
            if !kept {
                members.remove(key);
            }
            kept
        });
    }

    pub fn iter(&self) -> impl Iterator<Item = K> + '_ {
        self.order.iter().copied()
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

    /// Whether every member of this set is in `other`.
    pub fn is_subset(
        &self,
        other: &Self,
    ) -> bool {
        self.words.iter().enumerate().all(|(at, word)| word & !other.words.get(at).copied().unwrap_or(0) == 0)
    }

    /// Whether no member is in both.
    pub fn is_disjoint(
        &self,
        other: &Self,
    ) -> bool {
        self.words.iter().zip(&other.words).all(|(one, two)| one & two == 0)
    }

    /// Removes and returns the least member.
    pub fn pop_first(&mut self) -> Option<K> {
        let (at, word) = self.words.iter_mut().enumerate().find(|(_, word)| **word != 0)?;
        let bit = word.trailing_zeros() as usize;
        *word &= *word - 1;
        self.len -= 1;
        Some(K::at(at * 64 + bit))
    }

    /// Takes the greatest member out (a worklist that works from the top, as
    /// `BTreeSet::pop_last`).
    pub fn pop_last(&mut self) -> Option<K> {
        let (at, word) = self.words.iter().enumerate().rfind(|(_, word)| **word != 0)?;
        let bit = 63 - word.leading_zeros() as usize;
        self.words[at] &= !(1 << bit);
        self.len -= 1;
        Some(K::at(at * 64 + bit))
    }

    /// The members not in `other`, ascending.
    pub fn difference<'a>(
        &'a self,
        other: &'a Self,
    ) -> impl Iterator<Item = K> + 'a {
        self.iter().filter(|key| !other.contains(key))
    }

    /// Adds every member of `other`; whether any was new.
    pub fn union_with(
        &mut self,
        other: &Self,
    ) -> bool {
        if self.words.len() < other.words.len() {
            self.words.resize(other.words.len(), 0);
        }
        let mut len = 0;
        for (at, word) in self.words.iter_mut().enumerate() {
            *word |= other.words.get(at).copied().unwrap_or(0);
            len += word.count_ones() as usize;
        }
        let grew = len != self.len;
        self.len = len;
        grew
    }

    /// Only the members `keep` says to.
    pub fn retain(
        &mut self,
        mut keep: impl FnMut(K) -> bool,
    ) {
        for key in self.iter().collect::<Vec<_>>() {
            if !keep(key) {
                self.remove(&key);
            }
        }
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

/// Ids to a page: a page is allocated when an id in it is first set, so a map
/// of a few ids among many costs a page each, not a slot for every id below the
/// largest, and growing a map copies the page table, not the entries.
const PAGE_BITS: usize = 5;
const PAGE: usize = 1 << PAGE_BITS;

/// A map from ids, a slot each, in pages.
#[derive(Clone)]
pub struct IdMap<K, V> {
    pages: Vec<Option<Box<[Option<V>]>>>,
    len: usize,
    key: PhantomData<K>,
}

impl<K, V> Default for IdMap<K, V> {
    fn default() -> Self {
        Self { pages: Vec::new(), len: 0, key: PhantomData }
    }
}

impl<K: Dense, V> IdMap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Room for ids below `ids` without growing the page table.
    pub fn with_capacity(ids: usize) -> Self {
        Self { pages: Vec::with_capacity(ids.div_ceil(PAGE)), len: 0, key: PhantomData }
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
        let at = key.index();
        self.pages.get(at >> PAGE_BITS)?.as_ref()?[at & (PAGE - 1)].as_ref()
    }

    pub fn get_mut(
        &mut self,
        key: &K,
    ) -> Option<&mut V> {
        let at = key.index();
        self.pages.get_mut(at >> PAGE_BITS)?.as_mut()?[at & (PAGE - 1)].as_mut()
    }

    pub fn contains_key(
        &self,
        key: &K,
    ) -> bool {
        self.get(key).is_some()
    }

    /// Only the entries `keep` says to.
    pub fn retain(
        &mut self,
        mut keep: impl FnMut(K, &mut V) -> bool,
    ) {
        for (page, slots) in self.pages.iter_mut().enumerate() {
            let Some(slots) = slots else { continue };
            for (at, slot) in slots.iter_mut().enumerate() {
                if let Some(value) = slot
                    && !keep(K::at((page << PAGE_BITS) + at), value)
                {
                    *slot = None;
                    self.len -= 1;
                }
            }
        }
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
        let at = key.index();
        let old = self.pages.get_mut(at >> PAGE_BITS)?.as_mut()?[at & (PAGE - 1)].take();
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
        let slot = &mut Self::page(&mut self.pages, at >> PAGE_BITS)[at & (PAGE - 1)];
        if slot.is_none() {
            *slot = Some(make());
            self.len += 1;
        }
        slot.as_mut().expect("just made")
    }

    pub fn clear(&mut self) {
        self.pages.clear();
        self.len = 0;
    }

    /// The pairs, ascending by key.
    pub fn iter(&self) -> impl Iterator<Item = (K, &V)> + '_ {
        self.pages
            .iter()
            .enumerate()
            .filter_map(|(page, slots)| Some((page, slots.as_ref()?)))
            .flat_map(
                |(page, slots)| {
                    slots
                        .iter()
                        .enumerate()
                        .filter_map(move |(at, slot)| Some((K::at((page << PAGE_BITS) + at), slot.as_ref()?)))
                },
            )
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (K, &mut V)> + '_ {
        self.pages
            .iter_mut()
            .enumerate()
            .filter_map(|(page, slots)| Some((page, slots.as_mut()?)))
            .flat_map(
                |(page, slots)| {
                    slots
                        .iter_mut()
                        .enumerate()
                        .filter_map(move |(at, slot)| Some((K::at((page << PAGE_BITS) + at), slot.as_mut()?)))
                },
            )
    }

    pub fn keys(&self) -> impl Iterator<Item = K> + '_ {
        self.iter().map(|(key, _)| key)
    }

    pub fn values(&self) -> impl Iterator<Item = &V> + '_ {
        self.pages.iter().flatten().flat_map(|slots| slots.iter().flatten())
    }

    /// The slots allocated, for a test that a sparse map holds few.
    #[cfg(test)]
    fn held_slots(&self) -> usize {
        self.pages.iter().flatten().map(|page| page.len()).sum()
    }

    fn slot(
        &mut self,
        key: K,
    ) -> &mut Option<V> {
        let at = key.index();
        &mut Self::page(&mut self.pages, at >> PAGE_BITS)[at & (PAGE - 1)]
    }

    /// The slots of page `page`, made if it has none.
    fn page(
        pages: &mut Vec<Option<Box<[Option<V>]>>>,
        page: usize,
    ) -> &mut [Option<V>] {
        if page >= pages.len() {
            pages.resize_with(page + 1, || None);
        }
        pages[page].get_or_insert_with(|| (0..PAGE).map(|_| None).collect())
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

    /// Block addresses stand in for the B-trees of them (`BTreeSet<i64>`,
    /// `BTreeMap<i64, _>`): the same members in the same ascending order,
    /// and the set operations the passes used on them.
    /// A map of a few ids among many held a slot for every id below the
    /// largest: 1,000,001 for two entries (`get_constant_evolving_phi_operands`
    /// memoised a few instructions of a body of thousands, 22 K instructions
    /// a call in `reserve`). It holds pages, one per few ids.
    #[test]
    fn test_a_map_of_two_far_ids_holds_two_pages_not_a_million_slots() {
        let mut map: IdMap<u32, u8> = IdMap::new();
        map.insert(3, 1);
        map.insert(1_000_000, 2);
        assert_eq!((map.get(&3), map.get(&1_000_000), map.get(&4)), (Some(&1), Some(&2), None));
        assert_eq!(map.len(), 2);
        assert!(map.held_slots() <= 128, "{} slots held", map.held_slots());
    }

    #[test]
    fn test_a_set_of_block_addresses_agrees_with_the_btree_it_replaces() {
        let (a, b): (Vec<i64>, Vec<i64>) =
            ((0..300).filter(|n| n % 3 != 0).collect(), (0..300).filter(|n| n % 2 == 0).collect());
        let (set_a, set_b): (IdSet<i64>, IdSet<i64>) = (a.iter().copied().collect(), b.iter().copied().collect());
        let (tree_a, tree_b): (BTreeSet<i64>, BTreeSet<i64>) =
            (a.iter().copied().collect(), b.iter().copied().collect());
        assert_eq!(set_a.iter().collect::<Vec<_>>(), tree_a.iter().copied().collect::<Vec<_>>());
        assert_eq!(
            set_a.difference(&set_b).collect::<Vec<_>>(),
            tree_a.difference(&tree_b).copied().collect::<Vec<_>>()
        );
        assert_eq!(set_a.is_subset(&set_b), tree_a.is_subset(&tree_b));
        assert!(IdSet::<i64>::from_iter([2, 4]).is_subset(&set_b) && !set_a.is_subset(&set_b));
        assert_eq!(set_a.is_disjoint(&set_b), tree_a.is_disjoint(&tree_b));
        let mut joined = set_a.clone();
        assert!(joined.union_with(&set_b) && !joined.union_with(&set_b));
        assert_eq!(joined.iter().collect::<Vec<_>>(), tree_a.union(&tree_b).copied().collect::<Vec<_>>());
        assert_eq!(joined.len(), tree_a.union(&tree_b).count());
        let mut queue = set_a.clone();
        let mut tree = tree_a.clone();
        while let Some(first) = tree.pop_first() {
            assert_eq!(queue.pop_first(), Some(first));
        }
        assert_eq!((queue.pop_first(), queue.len()), (None, 0));
    }

    /// A map keeps what `retain` says to, ascending, and counts it.
    #[test]
    fn test_a_map_of_block_addresses_retains_and_counts() {
        let mut map: IdMap<i64, i64> = (0..10).map(|n| (n, n * n)).collect();
        map.retain(|at, _| at % 3 == 0);
        assert_eq!(map.iter().map(|(at, value)| (at, *value)).collect::<Vec<_>>(), [(0, 0), (3, 9), (6, 36), (9, 81)]);
        assert_eq!(map.len(), 4);
    }

    /// A set agrees with a `BTreeSet` of the same inserts and removes:
    /// membership, count, ascending order, what each call returns.
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

    /// A map agrees with a `BTreeMap`: what insert returns, lookups, the count
    /// after removals, ascending pairs.
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

const SHARE_BITS: usize = 3;
const SHARE_WIDTH: usize = 1 << SHARE_BITS;

/// How a `ShareMap` shares its nodes: by `Rc` (`ByRc`, the default) or, where
/// the map is kept in something that moves between threads, by `Arc` (`ByArc`).
pub trait Link {
    type Ptr<T>: Clone + std::ops::Deref<Target = T>;
    fn new<T>(value: T) -> Self::Ptr<T>;
    /// The value, copied first if others hold it.
    fn make_mut<T: Clone>(ptr: &mut Self::Ptr<T>) -> &mut T;
    fn ptr_eq<T>(
        one: &Self::Ptr<T>,
        other: &Self::Ptr<T>,
    ) -> bool;
    fn try_unwrap<T>(ptr: Self::Ptr<T>) -> Result<T, Self::Ptr<T>>;
}

pub struct ByRc;
pub struct ByArc;

impl Link for ByRc {
    type Ptr<T> = Rc<T>;
    fn new<T>(value: T) -> Rc<T> {
        Rc::new(value)
    }
    fn make_mut<T: Clone>(ptr: &mut Rc<T>) -> &mut T {
        Rc::make_mut(ptr)
    }
    fn ptr_eq<T>(
        one: &Rc<T>,
        other: &Rc<T>,
    ) -> bool {
        Rc::ptr_eq(one, other)
    }
    fn try_unwrap<T>(ptr: Rc<T>) -> Result<T, Rc<T>> {
        Rc::try_unwrap(ptr)
    }
}

impl Link for ByArc {
    type Ptr<T> = std::sync::Arc<T>;
    fn new<T>(value: T) -> std::sync::Arc<T> {
        std::sync::Arc::new(value)
    }
    fn make_mut<T: Clone>(ptr: &mut std::sync::Arc<T>) -> &mut T {
        std::sync::Arc::make_mut(ptr)
    }
    fn ptr_eq<T>(
        one: &std::sync::Arc<T>,
        other: &std::sync::Arc<T>,
    ) -> bool {
        std::sync::Arc::ptr_eq(one, other)
    }
    fn try_unwrap<T>(ptr: std::sync::Arc<T>) -> Result<T, std::sync::Arc<T>> {
        std::sync::Arc::try_unwrap(ptr)
    }
}

enum Node<V, L: Link> {
    Branch([Option<L::Ptr<Node<V, L>>>; SHARE_WIDTH]),
    Leaf([Option<L::Ptr<V>>; SHARE_WIDTH]),
}

impl<V, L: Link> Clone for Node<V, L> {
    fn clone(&self) -> Self {
        match self {
            Self::Branch(children) => Self::Branch(children.clone()),
            Self::Leaf(slots) => Self::Leaf(slots.clone()),
        }
    }
}

/// A map from ids that copies in constant time and writes by copying only the
/// path it changes: a radix tree of shared nodes, as Clojure's vector and
/// LLVM's persistent maps are. For a map each block of a function starts from
/// its parent's and changes a few entries of (an `IdMap` or a hash map is
/// copied whole, the work of a function of N blocks N times what they hold).
/// Iteration is ascending by id, as `IdMap`'s. Nodes are shared by `Rc`; for a
/// map that must be `Send`, `ShareMap<K, V, ByArc>`.
pub struct ShareMap<K, V, L: Link = ByRc> {
    root: Option<L::Ptr<Node<V, L>>>,
    /// Levels of branches above the leaves.
    levels: u32,
    len: usize,
    key: PhantomData<K>,
}

impl<K, V, L: Link> Clone for ShareMap<K, V, L> {
    fn clone(&self) -> Self {
        Self { root: self.root.clone(), levels: self.levels, len: self.len, key: PhantomData }
    }
}

impl<K, V, L: Link> Default for ShareMap<K, V, L> {
    fn default() -> Self {
        Self { root: None, levels: 0, len: 0, key: PhantomData }
    }
}

impl<K: Dense, V, L: Link> ShareMap<K, V, L> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether the two share all they hold: one is a copy of the other, or of
    /// what the other was, with no write to either since. A cheap test that
    /// they are equal; false does not say they differ.
    pub fn same(
        &self,
        other: &Self,
    ) -> bool {
        match (&self.root, &other.root) {
            (Some(one), Some(two)) => L::ptr_eq(one, two),
            (None, None) => true,
            _ => false,
        }
    }

    fn reaches(
        &self,
        at: usize,
    ) -> bool {
        at >> (SHARE_BITS * (self.levels as usize + 1)) == 0
    }

    pub fn get(
        &self,
        key: &K,
    ) -> Option<&V> {
        let at = key.index();
        if !self.reaches(at) {
            return None;
        }
        let mut node: &Node<V, L> = self.root.as_deref()?;
        let mut level = self.levels as usize;
        loop {
            match node {
                Node::Branch(children) => {
                    node = children[(at >> (SHARE_BITS * level)) & (SHARE_WIDTH - 1)].as_deref()?;
                    level -= 1;
                }
                Node::Leaf(slots) => return slots[at & (SHARE_WIDTH - 1)].as_deref(),
            }
        }
    }

    pub fn contains_key(
        &self,
        key: &K,
    ) -> bool {
        self.get(key).is_some()
    }

    /// The value at `key`, copied first if others share it.
    pub fn get_mut(
        &mut self,
        key: &K,
    ) -> Option<&mut V>
    where
        V: Clone,
    {
        let at = key.index();
        self.get(key)?;
        let mut node = L::make_mut(self.root.as_mut()?);
        let mut level = self.levels as usize;
        loop {
            match node {
                Node::Branch(children) => {
                    node = L::make_mut(children[(at >> (SHARE_BITS * level)) & (SHARE_WIDTH - 1)].as_mut()?);
                    level -= 1;
                }
                Node::Leaf(slots) => return slots[at & (SHARE_WIDTH - 1)].as_mut().map(L::make_mut),
            }
        }
    }

    fn fresh(level: usize) -> L::Ptr<Node<V, L>> {
        L::new(if level == 0 {
            Node::Leaf(std::array::from_fn(|_| None))
        } else {
            Node::Branch(std::array::from_fn(|_| None))
        })
    }

    /// Writes `value` at `at` below `node`, copying the nodes others share.
    fn put(
        node: &mut L::Ptr<Node<V, L>>,
        level: usize,
        at: usize,
        value: Option<L::Ptr<V>>,
    ) -> Option<L::Ptr<V>> {
        match L::make_mut(node) {
            Node::Leaf(slots) => std::mem::replace(&mut slots[at & (SHARE_WIDTH - 1)], value),
            Node::Branch(children) => {
                let child = &mut children[(at >> (SHARE_BITS * level)) & (SHARE_WIDTH - 1)];
                if child.is_none() && value.is_none() {
                    return None;
                }
                Self::put(child.get_or_insert_with(|| Self::fresh(level - 1)), level - 1, at, value)
            }
        }
    }

    /// The value `key` had.
    pub fn insert(
        &mut self,
        key: K,
        value: V,
    ) -> Option<V>
    where
        V: Clone,
    {
        let at = key.index();
        while !self.reaches(at) {
            if let Some(old) = self.root.take() {
                let mut children: [Option<L::Ptr<Node<V, L>>>; SHARE_WIDTH] = std::array::from_fn(|_| None);
                children[0] = Some(old);
                self.root = Some(L::new(Node::Branch(children)));
            }
            self.levels += 1;
        }
        let levels = self.levels as usize;
        let root = self.root.get_or_insert_with(|| Self::fresh(levels));
        let old = Self::put(root, levels, at, Some(L::new(value)));
        self.len += usize::from(old.is_none());
        old.map(|old| L::try_unwrap(old).unwrap_or_else(|shared| (*shared).clone()))
    }

    pub fn remove(
        &mut self,
        key: &K,
    ) -> Option<V>
    where
        V: Clone,
    {
        let at = key.index();
        if !self.reaches(at) || self.get(key).is_none() {
            return None;
        }
        let levels = self.levels as usize;
        let old = Self::put(self.root.as_mut()?, levels, at, None)?;
        self.len -= 1;
        Some(L::try_unwrap(old).unwrap_or_else(|shared| (*shared).clone()))
    }

    /// The pairs, ascending by key.
    pub fn iter(&self) -> impl Iterator<Item = (K, &V)> + '_ {
        fn walk<'a, K: Dense, V, L: Link>(
            node: &'a Node<V, L>,
            level: usize,
            base: usize,
            out: &mut Vec<(K, &'a V)>,
        ) {
            match node {
                Node::Leaf(slots) => {
                    out.extend(
                        slots.iter().enumerate().filter_map(|(at, slot)| Some((K::at(base + at), slot.as_deref()?))),
                    );
                }
                Node::Branch(children) => {
                    for (at, child) in children.iter().enumerate() {
                        if let Some(child) = child {
                            walk(child, level - 1, base + (at << (SHARE_BITS * level)), out);
                        }
                    }
                }
            }
        }
        let mut found = Vec::with_capacity(self.len);
        if let Some(root) = self.root.as_deref() {
            walk(root, self.levels as usize, 0, &mut found);
        }
        found.into_iter()
    }

    pub fn keys(&self) -> impl Iterator<Item = K> + '_ {
        self.iter().map(|(key, _)| key)
    }

    pub fn values(&self) -> impl Iterator<Item = &V> + '_ {
        self.iter().map(|(_, value)| value)
    }
}

impl<K: Dense, V: Clone, L: Link> FromIterator<(K, V)> for ShareMap<K, V, L> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(pairs: I) -> Self {
        let mut map = Self::new();
        map.extend(pairs);
        map
    }
}

impl<K: Dense, V: Clone, L: Link> Extend<(K, V)> for ShareMap<K, V, L> {
    fn extend<I: IntoIterator<Item = (K, V)>>(
        &mut self,
        pairs: I,
    ) {
        for (key, value) in pairs {
            self.insert(key, value);
        }
    }
}

impl<K: Dense, V, L: Link> std::ops::Index<&K> for ShareMap<K, V, L> {
    type Output = V;
    fn index(
        &self,
        key: &K,
    ) -> &V {
        self.get(key).expect("no entry for the key")
    }
}

impl<K: Dense, V: PartialEq, L: Link> PartialEq for ShareMap<K, V, L> {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        fn same<V: PartialEq, L: Link>(
            one: &L::Ptr<Node<V, L>>,
            other: &L::Ptr<Node<V, L>>,
        ) -> bool {
            if L::ptr_eq(one, other) {
                return true;
            }
            match (&**one, &**other) {
                (Node::Leaf(these), Node::Leaf(those)) => these.iter().zip(those).all(|(a, b)| match (a, b) {
                    (Some(a), Some(b)) => L::ptr_eq(a, b) || **a == **b,
                    (None, None) => true,
                    _ => false,
                }),
                (Node::Branch(these), Node::Branch(those)) => these.iter().zip(those).all(|(a, b)| match (a, b) {
                    (Some(a), Some(b)) => same::<V, L>(a, b),
                    (None, None) => true,
                    _ => false,
                }),
                _ => false,
            }
        }
        if self.len != other.len {
            return false;
        }
        match (&self.root, &other.root) {
            (Some(one), Some(two)) if self.levels == other.levels => same::<V, L>(one, two),
            (None, None) => true,
            _ => self.iter().zip(other.iter()).all(|((a, x), (b, y))| a.index() == b.index() && x == y),
        }
    }
}

impl<K: Dense, V: std::fmt::Debug, L: Link> std::fmt::Debug for ShareMap<K, V, L> {
    fn fmt(
        &self,
        out: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        out.debug_map().entries(self.iter().map(|(key, value)| (key.index(), value))).finish()
    }
}

impl<K: Dense, V: Clone, L: Link> IntoIterator for ShareMap<K, V, L> {
    type Item = (K, V);
    type IntoIter = std::vec::IntoIter<(K, V)>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter().map(|(key, value)| (key, value.clone())).collect::<Vec<_>>().into_iter()
    }
}

impl<'a, K: Dense, V, L: Link> IntoIterator for &'a ShareMap<K, V, L> {
    type Item = (K, &'a V);
    type IntoIter = std::vec::IntoIter<(K, &'a V)>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter().collect::<Vec<_>>().into_iter()
    }
}

#[cfg(test)]
mod share_tests {
    use std::collections::BTreeMap;

    use super::{ByArc, ShareMap};

    /// A map and a `BTreeMap` given the same writes, in a spread of ids that
    /// grows the tree, agree on every read, and iterate ascending.
    #[test]
    fn a_share_map_agrees_with_a_btree_map() {
        let mut shared = ShareMap::<u32, u64>::new();
        let mut model = BTreeMap::<u32, u64>::new();
        let mut seed = 12345_u64;
        for step in 0..4000_u64 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let key = ((seed >> 33) % 3000) as u32 * if step % 7 == 0 { 97 } else { 1 };
            if (seed >> 20) % 5 == 0 {
                assert_eq!(shared.remove(&key), model.remove(&key));
            } else {
                assert_eq!(shared.insert(key, step), model.insert(key, step));
            }
            assert_eq!(shared.len(), model.len());
        }
        assert_eq!(shared.iter().map(|(k, v)| (k, *v)).collect::<Vec<_>>(), model.into_iter().collect::<Vec<_>>());
    }

    /// Two maps with the same pairs are equal however they were made, and a
    /// copy is equal to its original without a walk.
    #[test]
    fn maps_with_the_same_pairs_are_equal_whatever_their_history() {
        let mut one = ShareMap::<u32, u32>::new();
        let mut two = ShareMap::<u32, u32>::new();
        (0..100).for_each(|at| {
            one.insert(at, at);
        });
        (0..100)
            .rev()
            .for_each(
                |at| {
                    two.insert(at, at);
                },
            );
        two.insert(5000, 1);
        two.remove(&5000);
        assert!(one == two);
        two.insert(7, 8);
        assert!(one != two);
    }

    /// A copy is made in constant time and a write to it leaves the original:
    /// the scope of a block starts from its parent's.
    #[test]
    fn a_write_to_a_copy_leaves_the_original_and_shares_the_rest() {
        let mut parent = ShareMap::<u32, String>::new();
        (0..1000).for_each(|at| {
            parent.insert(at, at.to_string());
        });
        let mut child = parent.clone();
        assert!(child == parent);
        child.insert(500, "changed".to_owned());
        child.remove(&7);
        assert_eq!(parent.get(&500).map(String::as_str), Some("500"));
        assert_eq!(child.get(&500).map(String::as_str), Some("changed"));
        assert!(parent.get(&7).is_some() && child.get(&7).is_none());
        assert!(child != parent);
        assert_eq!(parent.len(), 1000);
        assert_eq!(child.len(), 999);
    }

    /// A map shared by `Arc` crosses threads, and shares what it holds with its
    /// copies.
    #[test]
    fn a_map_shared_by_arc_is_send_and_shares_with_its_copies() {
        fn send<T: Send>(_: &T) {}
        let mut one = ShareMap::<u32, u32, ByArc>::new();
        (0..50).for_each(|at| {
            one.insert(at, at);
        });
        let two = one.clone();
        send(&one);
        assert!(one.same(&two));
        one.insert(3, 99);
        assert!(!one.same(&two));
        assert_eq!(std::thread::spawn(move || two.get(&3).copied()).join().unwrap(), Some(3));
    }

    /// A worklist that works from the top takes the greatest id out each time,
    /// as `BTreeSet::pop_last` does, and allocates nothing once the set is
    /// sized.
    #[test]
    fn pop_last_takes_the_greatest_member_out_as_a_btree_set_does() {
        use super::IdSet;
        let ids = [3_usize, 64, 65, 500, 7, 4095];
        let mut set: IdSet<usize> = ids.iter().copied().collect();
        let mut tree: std::collections::BTreeSet<usize> = ids.iter().copied().collect();
        loop {
            let (one, other) = (set.pop_last(), tree.pop_last());
            assert_eq!(one, other);
            assert_eq!(set.len(), tree.len());
            if one.is_none() {
                break;
            }
            // Pushing back below the top keeps both in step.
            if tree.len() == 3 {
                set.insert(10);
                tree.insert(10);
            }
        }
        assert!(set.is_empty() && IdSet::<usize>::new().pop_last().is_none());
    }
}
