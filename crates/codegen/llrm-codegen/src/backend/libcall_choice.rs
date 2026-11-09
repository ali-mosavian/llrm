//! Which operations a size-built module calls a runtime routine for, and which
//! it expands inline, by the bytes of the whole linked program.
//!
//! # How it works
//! A target describes each operation that can go either way as a
//! [`Candidate`]: the key and the runtime routine it calls. A routine is a
//! [`Member`]: its bytes, and the members it calls. The unit measures what
//! calling would save: per candidate, the bytes of its inline sites less the
//! bytes of the same sites as calls (a site's inline code varies with what is
//! known of its operands, so no table column prices it). [`choose`] takes
//! those savings and returns the candidates to call: the set whose savings,
//! less every member they link once, are the greatest and positive. A tie is
//! inline.
//!
//! DEVIATE M68K-SIZE-LIBCALL: the pin's `Expand` against `LibCall` is a fact of
//! the operation's legality (`setOperationAction`) with no accounting of the
//! library, so it picks one for every function. Reason: best of gcc and clang
//! is the goal at `-Os`, and the bytes of a call are the bytes of the routine
//! too. A member is charged in full to this translation unit's sites: a unit
//! cannot know the uses of another (conservative). Sizes come from the target
//! description. Owner m68k. Review: 2026-12-20.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

/// An operation with an inline expansion and a runtime routine.
#[derive(Clone, Debug)]
pub struct Candidate<K> {
    pub key: K,
    /// The member it calls.
    pub routine: String,
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

/// The most candidates with savings that are searched exhaustively; past it
/// each candidate is judged alone with its members charged in full.
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

/// The candidates to call, given the bytes calling each would save in the unit
/// (inline sites less call sites, members not counted; may be negative).
pub fn choose<K: Copy + Eq + Hash>(
    costs: &Costs<K>,
    saved: &HashMap<K, i64>,
) -> Vec<K> {
    let used: Vec<(&Candidate<K>, i64)> =
        costs.candidates.iter().filter_map(|one| saved.get(&one.key).map(|&bytes| (one, bytes))).collect();
    if used.len() > EXHAUSTIVE {
        return used
            .iter()
            .filter(|(one, bytes)| *bytes > i64::from(costs.linked(std::iter::once(one.routine.as_str()))))
            .map(|(one, _)| one.key)
            .collect();
    }
    let benefit = |mask: u32| -> i64 {
        let kept: i64 =
            used.iter().enumerate().filter(|(at, _)| mask >> at & 1 == 1).map(|(_, (_, bytes))| bytes).sum();
        let routines =
            used.iter().enumerate().filter(|(at, _)| mask >> at & 1 == 1).map(|(_, (one, _))| one.routine.as_str());
        kept - i64::from(costs.linked(routines))
    };
    // most bytes saved, then fewest calls, then the lowest mask
    let best = (0..1u32 << used.len()).min_by_key(|&mask| (-benefit(mask), mask.count_ones(), mask)).unwrap_or(0);
    used.iter().enumerate().filter(|(at, _)| best >> at & 1 == 1).map(|(_, (one, _))| one.key).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn costs() -> Costs<&'static str> {
        let candidates = vec![
            Candidate { key: "ctz64", routine: "ctzdi2".into() },
            Candidate { key: "pop64", routine: "popcountdi2".into() },
            Candidate { key: "pop32", routine: "popcountsi2".into() },
        ];
        let members = HashMap::from([
            ("ctzdi2".to_owned(), Member { bytes: 46, needs: vec![] }),
            ("popcountdi2".to_owned(), Member { bytes: 32, needs: vec!["popcountsi2".into()] }),
            ("popcountsi2".to_owned(), Member { bytes: 70, needs: vec![] }),
        ]);
        Costs { candidates, members }
    }

    fn saved(list: &[(&'static str, i64)]) -> HashMap<&'static str, i64> {
        list.iter().copied().collect()
    }

    /// Calling saves 32 B a site (50 inline, 18 call): one site does not pay
    /// the 46 B member, the second does.
    #[test]
    fn the_second_site_flips_a_count_to_a_call() {
        assert_eq!(choose(&costs(), &saved(&[("ctz64", 32)])), Vec::<&str>::new());
        assert_eq!(choose(&costs(), &saved(&[("ctz64", 64)])), vec!["ctz64"]);
        assert_eq!(choose(&costs(), &saved(&[("ctz64", 96)])), vec!["ctz64"]);
    }

    /// A saving that only equals the member is a tie: inline.
    #[test]
    fn a_tie_stays_inline() {
        assert_eq!(choose(&costs(), &saved(&[("ctz64", 46)])), Vec::<&str>::new());
        assert_eq!(choose(&costs(), &saved(&[("ctz64", 47)])), vec!["ctz64"]);
    }

    /// A call that is larger than the inline site never wins, however many.
    #[test]
    fn a_negative_saving_is_never_called() {
        assert!(choose(&costs(), &saved(&[("ctz64", -100)])).is_empty());
    }

    /// The popcount routines share a member: popcount64 alone saves 106 B
    /// against 32 + 70 (a call, by 4 B); popcount32 alone saves 48 B against
    /// 70 (inline); with popcount64 called, popcount32's 48 B pay nothing
    /// more: a call.
    #[test]
    fn a_member_another_choice_links_is_charged_once() {
        assert_eq!(choose(&costs(), &saved(&[("pop64", 106)])), vec!["pop64"]);
        assert_eq!(choose(&costs(), &saved(&[("pop32", 48)])), Vec::<&str>::new());
        assert_eq!(choose(&costs(), &saved(&[("pop64", 106), ("pop32", 48)])), vec!["pop64", "pop32"]);
    }

    #[test]
    fn no_savings_choose_nothing() {
        assert!(choose(&costs(), &saved(&[])).is_empty());
    }
}
