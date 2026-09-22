//! CPython 3.13's `set`, for iteration order.
//!
//! A Python set iterates in hash-table slot order. Where the elements hash
//! deterministically -- ints, bools and tuples of them, so any frozen
//! dataclass of those -- that order is reproducible, and a port that iterates
//! the same set must reproduce it: `lower_int64` numbers fresh values by it.
//! This mirrors `Objects/setobject.c` (`set_add_entry`, `set_insert_clean`,
//! `set_table_resize`, `set_discard_entry`) and `Objects/tupleobject.c`'s hash.

/// `sys.hash_info.modulus`.
const MODULUS: u64 = (1 << 61) - 1;
const MINSIZE: usize = 8;
const LINEAR_PROBES: usize = 9;
const PERTURB_SHIFT: u32 = 5;

/// `hash(value)`, as CPython computes it.
pub trait PyHash {
    fn py_hash(&self) -> i64;
}

/// `hash(int)`: the value modulo 2**61 - 1, keeping its sign; -1 is reserved.
pub fn int_hash(value: i64) -> i64 {
    let reduced = (value.unsigned_abs() % MODULUS) as i64;
    let hashed = if value < 0 { -reduced } else { reduced };
    if hashed == -1 { -2 } else { hashed }
}

/// `hash(tuple)`: CPython's xxHash-derived tuple hash over the items' hashes.
pub fn tuple_hash(items: &[i64]) -> i64 {
    const PRIME_1: u64 = 11400714785074694791;
    const PRIME_2: u64 = 14029467366897019727;
    const PRIME_5: u64 = 2870177450012600261;
    let mut acc = PRIME_5;
    for &lane in items {
        acc = acc.wrapping_add((lane as u64).wrapping_mul(PRIME_2));
        acc = acc.rotate_left(31);
        acc = acc.wrapping_mul(PRIME_1);
    }
    acc = acc.wrapping_add((items.len() as u64) ^ (PRIME_5 ^ 3527539));
    if acc == u64::MAX { 1546275796 } else { acc as i64 }
}

impl PyHash for i64 {
    fn py_hash(&self) -> i64 {
        int_hash(*self)
    }
}

impl PyHash for bool {
    fn py_hash(&self) -> i64 {
        i64::from(*self)
    }
}

#[derive(Clone, Debug)]
enum Slot<T> {
    Unused,
    Dummy,
    Active(i64, T),
}

/// A Python set: membership by equality, iteration in CPython's slot order.
#[derive(Clone, Debug)]
pub struct PySet<T> {
    table: Vec<Slot<T>>,
    fill: usize,
    used: usize,
}

impl<T: PyHash + PartialEq> Default for PySet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: PyHash + PartialEq> PySet<T> {
    pub fn new() -> Self {
        Self { table: (0..MINSIZE).map(|_| Slot::Unused).collect(), fill: 0, used: 0 }
    }

    fn mask(&self) -> usize {
        self.table.len() - 1
    }

    pub fn len(&self) -> usize {
        self.used
    }

    pub fn is_empty(&self) -> bool {
        self.used == 0
    }

    /// `set_add_entry`.
    pub fn add(&mut self, key: T) {
        let hash = key.py_hash();
        let mask = self.mask();
        let mut perturb = hash as u64;
        let mut i = (hash as u64 as usize) & mask;
        let mut free: Option<usize> = None;
        let found = loop {
            let probes = if i + LINEAR_PROBES <= mask { LINEAR_PROBES } else { 0 };
            let mut at = i;
            let mut hit = None;
            for _ in 0..=probes {
                match &self.table[at] {
                    Slot::Unused => {
                        hit = Some(free.unwrap_or(at));
                        break;
                    }
                    Slot::Active(stored, one) if *stored == hash && *one == key => return,
                    Slot::Dummy if free.is_none() => free = Some(at),
                    _ => {}
                }
                at += 1;
            }
            if let Some(hit) = hit {
                break hit;
            }
            perturb >>= PERTURB_SHIFT;
            i = (i.wrapping_mul(5).wrapping_add(1).wrapping_add(perturb as usize)) & mask;
        };
        if matches!(self.table[found], Slot::Unused) {
            self.fill += 1;
        }
        self.used += 1;
        self.table[found] = Slot::Active(hash, key);
        if self.fill * 5 < mask * 3 {
            return;
        }
        let size = if self.used > 50000 { self.used * 2 } else { self.used * 4 };
        self.resize(size);
    }

    /// `set_table_resize`: reinsert the active entries in slot order.
    fn resize(&mut self, minused: usize) {
        let mut size = MINSIZE;
        while size <= minused {
            size <<= 1;
        }
        let old = std::mem::replace(&mut self.table, (0..size).map(|_| Slot::Unused).collect());
        self.fill = self.used;
        for slot in old {
            if let Slot::Active(hash, key) = slot {
                self.insert_clean(hash, key);
            }
        }
    }

    /// `set_insert_clean`.
    fn insert_clean(&mut self, hash: i64, key: T) {
        let mask = self.mask();
        let mut perturb = hash as u64;
        let mut i = (hash as u64 as usize) & mask;
        loop {
            if matches!(self.table[i], Slot::Unused) {
                self.table[i] = Slot::Active(hash, key);
                return;
            }
            if i + LINEAR_PROBES <= mask {
                for at in i + 1..=i + LINEAR_PROBES {
                    if matches!(self.table[at], Slot::Unused) {
                        self.table[at] = Slot::Active(hash, key);
                        return;
                    }
                }
            }
            perturb >>= PERTURB_SHIFT;
            i = (i.wrapping_mul(5).wrapping_add(1).wrapping_add(perturb as usize)) & mask;
        }
    }

    pub fn contains(&self, key: &T) -> bool {
        self.iter().any(|one| one == key)
    }

    /// `set.discard`: the slot becomes a dummy, so later order is unchanged.
    pub fn discard(&mut self, key: &T) {
        for slot in &mut self.table {
            if matches!(slot, Slot::Active(_, one) if one == key) {
                *slot = Slot::Dummy;
                self.used -= 1;
                return;
            }
        }
    }

    /// Iteration in slot order.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.table.iter().filter_map(|slot| match slot {
            Slot::Active(_, key) => Some(key),
            _ => None,
        })
    }
}

/// A set display or comprehension: `{one for one in items}`.
impl<T: PyHash + PartialEq> FromIterator<T> for PySet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(items: I) -> Self {
        let mut set = PySet::new();
        for one in items {
            set.add(one);
        }
        set
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Pair(i64, i64);

    impl PyHash for Pair {
        fn py_hash(&self) -> i64 {
            tuple_hash(&[int_hash(self.0), int_hash(self.1)])
        }
    }

    /// Expected values printed by CPython 3.13.
    #[test]
    fn hashes_match_cpython() {
        assert_eq!(int_hash(-1), -2);
        assert_eq!(int_hash(1 << 61), 1);
        assert_eq!(tuple_hash(&[]), 5740354900026072187);
        assert_eq!(tuple_hash(&[3, 5, 0, 3, 1]), 5904129870890306468);
    }

    /// `list({(i, i + 1) for i in range(1, 20)})` in CPython 3.13, crossing two resizes.
    #[test]
    fn iteration_follows_cpython_slot_order() {
        let set: PySet<Pair> = (1..20).map(|i| Pair(i, i + 1)).collect();
        let got: Vec<i64> = set.iter().map(|one| one.0).collect();
        assert_eq!(got, [3, 12, 8, 17, 13, 18, 4, 5, 14, 9, 1, 10, 19, 6, 15, 2, 11, 7, 16]);
    }
}
