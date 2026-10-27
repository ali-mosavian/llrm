//! The values one value interferes with: a sorted set that turns into a bit
//! vector once it is dense.
//!
//! # How it works
//! A function with N values live together has up to N^2 interference pairs.
//! As tree sets each costs a node and a descent; as bits, one shift. A set
//! starts as a `BTreeSet`. When it reaches `DENSE_AT` members, and again at
//! each multiple, it becomes `Dense` if its largest id is below
//! `BITS_PER_MEMBER` times its size, so a set of widely spread ids stays a
//! tree. This is GCC IRA's choice for an allocno's conflicts: a vector of ids
//! or a bit vector over its id range, whichever is smaller
//! (`ira_allocate_object_conflicts`, `ira-build.cc`).
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart LR
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   a["<b>Few(BTreeSet)</b>"]:::mid
//!   b["<b>Dense #123; words, len #125;</b><br/><small>bit v of word v/64</small>"]:::mid
//!   a -->|"len #gt;= 64,<br/>ids dense enough"| b
//! ```
//!
//! *Neighbour set representations*
//!
//! `join` adds a whole group at once: every value in a mask except one becomes
//! a neighbour, a word at a time when the set is dense. Iteration is ascending
//! in both forms.
//!
//! # Example
//! No honest example: a set type inside the allocator's interference graph,
//! with no LIR of its own. Read `tests` in this file.
//!
//! # Used by
//! `coalesce`, for its interference graph.

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

    /// The members of `mask` (bit `v` for value `v`) except `except` join this
    /// set: every value of a group is next to every other. A dense set
    /// takes a word at a time.
    pub fn join(
        &mut self,
        mask: &[u64],
        except: u32,
    ) {
        match self {
            Self::Dense { words, len } => {
                if words.len() < mask.len() {
                    words.resize(mask.len(), 0);
                }
                for (word, more) in words.iter_mut().zip(mask) {
                    *word |= more;
                }
                if let Some(word) = words.get_mut(except as usize / 64) {
                    *word &= !(1u64 << (except % 64));
                }
                *len = words.iter().map(|word| word.count_ones() as usize).sum();
            }
            Self::Few(_) => {
                for (at, word) in mask.iter().enumerate() {
                    let mut rest = *word;
                    while rest != 0 {
                        let member = at as u32 * 64 + rest.trailing_zeros();
                        rest &= rest - 1;
                        if member != except {
                            self.insert(member);
                        }
                    }
                }
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

#[cfg(test)]
mod join_tests {
    use std::collections::BTreeSet;

    use super::Neighbours;

    /// A group of values live together are next to one another: `join` gives a
    /// row the members of a mask but itself, as a pair at a time would,
    /// whether the row is bits or a tree.
    #[test]
    fn a_row_joined_with_a_group_has_the_group_but_itself() {
        let group: BTreeSet<u32> = [3, 9, 64, 65, 130, 200].into_iter().collect();
        let mut mask = vec![0u64; 4];
        for value in &group {
            mask[*value as usize / 64] |= 1u64 << (*value % 64);
        }
        for start in [Neighbours::Dense { words: Vec::new(), len: 0 }, Neighbours::default()] {
            let mut row = start.clone();
            row.insert(7);
            row.join(&mask, 64);
            let expected: BTreeSet<u32> = group.iter().copied().filter(|one| *one != 64).chain([7]).collect();
            assert!(row.iter().eq(expected.iter().copied()), "{:?}", row.iter().collect::<Vec<_>>());
            assert_eq!(row.len(), expected.len());
        }
    }
}
