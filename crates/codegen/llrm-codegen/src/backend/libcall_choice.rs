//! Which operations a size-built module calls a runtime routine for, and which
//! it expands inline, by the bytes of the whole linked program.
//!
//! # How it works
//! A target describes each operation that can go either way as a
//! [`Candidate`]: the bytes of the inline expansion at one site, the bytes of
//! the call at one site, and the runtime routine it calls. A routine is a
//! [`Member`]: its bytes, and the members it calls. [`choose`] takes the
//! number of sites of each candidate in the translation unit and returns the
//! candidates to call: the set whose total (inline bytes of the rest, call
//! bytes of the chosen, and every member they link, once) is the least. A tie is
//! inline.
//!
//! DEVIATE M68K-SIZE-LIBCALL: the pin's `Expand` against `LibCall` is a fact of
//! the operation's legality (`setOperationAction`) with no accounting of the
//! library, so it picks one for every function. Reason: best of gcc and clang is
//! the goal at `-Os`, and the bytes of a call are the bytes of the routine too.
//! A member is charged in full to this translation unit's sites: a unit cannot
//! know the uses of another (conservative). Sizes come from the target
//! description. Owner m68k. Review: 2026-12-20.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

/// An operation with an inline expansion and a runtime routine.
#[derive(Clone, Debug)]
pub struct Candidate<K> {
    pub key: K,
    /// The member it calls.
    pub routine: String,
    /// Bytes of the expansion at one site.
    pub inline_bytes: u32,
    /// Bytes of the call at one site.
    pub call_bytes: u32,
}

/// A runtime routine: its bytes, and the members it calls.
#[derive(Clone, Debug, Default)]
pub struct Member {
    pub bytes: u32,
    pub needs: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Costs<K> {
    pub candidates: Vec<Candidate<K>>,
    pub members: HashMap<String, Member>,
}

/// The most candidates with sites that are searched exhaustively; past it each
/// candidate is judged alone with its members charged in full.
const EXHAUSTIVE: usize = 16;

impl<K: Copy + Eq + Hash> Costs<K> {
    /// Bytes of the members `routines` link, each once, through what they call.
    fn linked<'a>(
        &self,
        routines: impl Iterator<Item = &'a str>,
    ) -> u32 {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut pending: Vec<&str> = routines.collect();
        while let Some(name) = pending.pop() {
            if let Some(member) = self.members.get(name)
                && seen.insert(name)
            {
                pending.extend(member.needs.iter().map(String::as_str));
            }
        }
        seen.iter().map(|name| self.members[*name].bytes).sum()
    }
}

/// The candidates to call, given the sites of each in the unit.
pub fn choose<K: Copy + Eq + Hash>(
    costs: &Costs<K>,
    sites: &HashMap<K, u32>,
) -> Vec<K> {
    let used: Vec<(&Candidate<K>, u32)> =
        costs.candidates.iter().filter_map(|one| sites.get(&one.key).map(|&n| (one, n))).filter(|(_, n)| *n > 0).collect();
    if used.len() > EXHAUSTIVE {
        return used
            .iter()
            .filter(|(one, n)| {
                u64::from(*n) * u64::from(one.call_bytes) + u64::from(costs.linked(std::iter::once(one.routine.as_str())))
                    < u64::from(*n) * u64::from(one.inline_bytes)
            })
            .map(|(one, _)| one.key)
            .collect();
    }
    let total = |mask: u32| -> u64 {
        let mut bytes = 0u64;
        for (at, (one, n)) in used.iter().enumerate() {
            let per = if mask >> at & 1 == 1 { one.call_bytes } else { one.inline_bytes };
            bytes += u64::from(*n) * u64::from(per);
        }
        let routines = used.iter().enumerate().filter(|(at, _)| mask >> at & 1 == 1).map(|(_, (one, _))| one.routine.as_str());
        bytes + u64::from(costs.linked(routines))
    };
    // fewest bytes, then fewest calls, then the lowest mask
    let best = (0..1u32 << used.len()).min_by_key(|&mask| (total(mask), mask.count_ones(), mask)).unwrap_or(0);
    used.iter().enumerate().filter(|(at, _)| best >> at & 1 == 1).map(|(_, (one, _))| one.key).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn costs() -> Costs<&'static str> {
        let candidates = vec![
            Candidate { key: "ctz64", routine: "ctzdi2".into(), inline_bytes: 50, call_bytes: 18 },
            Candidate { key: "pop64", routine: "popcountdi2".into(), inline_bytes: 124, call_bytes: 18 },
            Candidate { key: "pop32", routine: "popcountsi2".into(), inline_bytes: 62, call_bytes: 14 },
        ];
        let members = HashMap::from([
            ("ctzdi2".to_owned(), Member { bytes: 46, needs: vec![] }),
            ("popcountdi2".to_owned(), Member { bytes: 32, needs: vec!["popcountsi2".into()] }),
            ("popcountsi2".to_owned(), Member { bytes: 70, needs: vec![] }),
        ]);
        Costs { candidates, members }
    }

    fn sites(list: &[(&'static str, u32)]) -> HashMap<&'static str, u32> {
        list.iter().copied().collect()
    }

    /// One use of a 64-bit count is inline (50 B against 18 + 46), the second flips it to a call (100 B against 36 + 46): the library is paid once.
    #[test]
    fn the_second_site_flips_a_count_to_a_call() {
        assert_eq!(choose(&costs(), &sites(&[("ctz64", 1)])), Vec::<&str>::new());
        assert_eq!(choose(&costs(), &sites(&[("ctz64", 2)])), vec!["ctz64"]);
        assert_eq!(choose(&costs(), &sites(&[("ctz64", 3)])), vec!["ctz64"]);
    }

    /// A tie is inline: 2 x 31 = 62 B against 2 x 14 + 34.
    #[test]
    fn a_tie_stays_inline() {
        let mut one = costs();
        one.members.get_mut("ctzdi2").unwrap().bytes = 34;
        one.candidates[0].inline_bytes = 31;
        assert_eq!(choose(&one, &sites(&[("ctz64", 2)])), Vec::<&str>::new());
    }

    /// The popcount routines share a member: popcount64 alone is 124 B inline against 18 + 32 + 70 (a call, by 4 B); popcount32 alone is 62 B against 14 + 70 (inline);
    /// with popcount64 called, popcount32 is 62 B against 14 more (70 is already linked): a call.
    #[test]
    fn a_member_another_choice_links_is_charged_once() {
        assert_eq!(choose(&costs(), &sites(&[("pop64", 1)])), vec!["pop64"]);
        assert_eq!(choose(&costs(), &sites(&[("pop32", 1)])), Vec::<&str>::new());
        assert_eq!(choose(&costs(), &sites(&[("pop64", 1), ("pop32", 1)])), vec!["pop64", "pop32"]);
    }

    #[test]
    fn no_sites_choose_nothing() {
        assert!(choose(&costs(), &sites(&[])).is_empty());
        assert!(choose(&costs(), &sites(&[("ctz64", 0)])).is_empty());
    }
}
