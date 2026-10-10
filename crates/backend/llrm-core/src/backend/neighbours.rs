//! The values one value interferes with: a sorted set that is a bit vector
//! once it is dense, as GCC's IRA keeps the conflicts of an allocno (a vector
//! of ids, or a bit vector over its id range, whichever is smaller:
//! `ira_allocate_object_conflicts`, ira-build.cc). A function with N values
//! live together has N^2 pairs; as tree sets each costs a node and a descent,
//! as bits one shift.

use std::collections::BTreeSet;

/// A set of value numbers, iterated ascending.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Neighbours {
    Few(BTreeSet<u32>),
    Dense { words: Vec<u64>, len: usize },
}

impl Default for Neighbours {
    fn default() -> Self {
        Self::Few(BTreeSet::new())
    }
}

/// Fewest members before a set looks at turning into bits.
const DENSE_AT: usize = 64;
/// Most bits a dense set spends on a member: a set of ids spread wider stays a
/// tree.
const BITS_PER_MEMBER: usize = 64;

impl Neighbours {
    pub fn len(&self) -> usize {
        match self {
            Self::Few(set) => set.len(),
            Self::Dense { len, .. } => *len,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn contains(
        &self,
        value: &u32,
    ) -> bool {
        match self {
            Self::Few(set) => set.contains(value),
            Self::Dense { words, .. } => {
                words.get(*value as usize / 64).is_some_and(|word| word >> (*value % 64) & 1 == 1)
            }
        }
    }

    /// Whether `value` was not a member.
    pub fn insert(
        &mut self,
        value: u32,
    ) -> bool {
        match self {
            Self::Few(set) => {
                let added = set.insert(value);
                if added && set.len() >= DENSE_AT && set.len().is_multiple_of(DENSE_AT) {
                    let last = *set.last().expect("not empty") as usize;
                    if last < set.len() * BITS_PER_MEMBER {
                        let mut words = vec![0u64; last / 64 + 1];
                        for member in set.iter() {
                            words[*member as usize / 64] |= 1 << (*member % 64);
                        }
                        *self = Self::Dense { words, len: set.len() };
                    }
                }
                added
            }
            Self::Dense { words, len } => {
                let (at, bit) = (value as usize / 64, 1u64 << (value % 64));
                if at >= words.len() {
                    words.resize(at + 1, 0);
                }
                let added = words[at] & bit == 0;
                words[at] |= bit;
                *len += usize::from(added);
                added
            }
        }
    }

    /// Whether `value` was a member.
    pub fn remove(
        &mut self,
        value: &u32,
    ) -> bool {
        match self {
            Self::Few(set) => set.remove(value),
            Self::Dense { words, len } => {
                let Some(word) = words.get_mut(*value as usize / 64) else { return false };
                let bit = 1u64 << (*value % 64);
                let was = *word & bit != 0;
                *word &= !bit;
                *len -= usize::from(was);
                was
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        let (few, dense) = match self {
            Self::Few(set) => (Some(set.iter().copied()), None),
            Self::Dense { words, .. } => (None, Some(words)),
        };
        few.into_iter()
            .flatten()
            .chain(
                dense.into_iter().flat_map(|words| {
                    words
                        .iter()
                        .enumerate()
                        .flat_map(
                            |(at, word)| {
                                let mut rest = *word;
                                std::iter::from_fn(move || {
                                    if rest == 0 {
                                        return None;
                                    }
                                    let bit = rest.trailing_zeros();
                                    rest &= rest - 1;
                                    Some(at as u32 * 64 + bit)
                                })
                            },
                        )
                }),
            )
    }
}

impl FromIterator<u32> for Neighbours {
    fn from_iter<I: IntoIterator<Item = u32>>(members: I) -> Self {
        let mut set = Self::default();
        for member in members {
            set.insert(member);
        }
        set
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::Neighbours;

    #[test]
    fn a_dense_set_is_the_tree_set_of_the_same_members() {
        let (mut ours, mut theirs) = (Neighbours::default(), BTreeSet::new());
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        for step in 0..6000u32 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let value = (seed >> 33) as u32 % 700;
            if step % 5 == 4 {
                assert_eq!(ours.remove(&value), theirs.remove(&value), "remove {value}");
            } else {
                assert_eq!(ours.insert(value), theirs.insert(value), "insert {value}");
            }
            assert_eq!((ours.len(), ours.contains(&value)), (theirs.len(), theirs.contains(&value)));
        }
        assert!(matches!(ours, Neighbours::Dense { .. }), "700 ids of which hundreds are members are bits");
        assert!(ours.iter().eq(theirs.iter().copied()));
    }

    #[test]
    fn ids_spread_wide_stay_a_tree() {
        let wide: Neighbours = (0..200u32).map(|one| one * 10_000).collect();
        assert!(matches!(wide, Neighbours::Few(_)));
        assert_eq!(wide.len(), 200);
        assert!(wide.contains(&1_990_000) && !wide.contains(&1_990_001));
    }
}
