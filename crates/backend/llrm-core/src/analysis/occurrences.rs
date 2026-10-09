//! Facts of a few values of a body, found from where the values occur: the one mechanism for what would be a pass over every
//! instruction of the body to ask of a handful of its values (liveness rows, widths, intervals, weights). LLVM's per-register use-def
//! chains and `LiveRangeEdit` ask the same: a register's own references, not the function's.
//!
//! The occurrences of a set of values come from the body's postings (kept as the body changes) or from one scan; every query reads
//! them and the blocks the values are live in, and `LLRM_CHECK_OCCURRENCES` compares each with the walk of the whole body.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::analysis::frequency::Frequency;
use crate::analysis::intervals::{self, Indexes, Interval, Place};
use crate::backend::allocate::{self, WebRows};
use crate::backend::postings::Postings;
use crate::model::ir::{self, Held};
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::IndexMap;

/// An instruction that names a value: where it is, whether it defines and whether it reads it, and how many times it names it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Named {
    pub place: Place,
    pub defined: bool,
    pub used: bool,
    pub mentions: u32,
}

/// The instructions that name each of a set of values, by block then position.
#[derive(Debug, Default)]
pub struct Occurrences {
    by_value: IndexMap<u32, Vec<Named>>,
    /// Instructions that `require` one of the values in a register: they name it too, for a width.
    required: BTreeSet<Place>,
}

impl Occurrences {
    /// From one pass over `body`, of the values `only` holds, in the order the body first names them.
    pub fn scan(body: &LirBody, only: &impl Fn(u32) -> bool) -> Self {
        body.facts.0.bump("occurrence-scans");
        let mut found = Self::default();
        let mut named: Vec<Named> = Vec::new();
        let mut values: Vec<u32> = Vec::new();
        for (block_index, block) in body.blocks.iter().enumerate() {
            for (position, one) in block.insns.iter().enumerate() {
                named.clear();
                values.clear();
                for (value, defined) in one.defines.iter().map(|value| (value, true)).chain(one.uses.iter().map(|value| (value, false))) {
                    if !only(*value) {
                        continue;
                    }
                    match values.iter().position(|seen| seen == value) {
                        Some(at) => {
                            named[at].defined |= defined;
                            named[at].used |= !defined;
                            named[at].mentions += 1;
                        }
                        None => {
                            values.push(*value);
                            named.push(Named { place: (block_index, position), defined, used: !defined, mentions: 1 });
                        }
                    }
                }
                for (value, made) in values.iter().zip(&named) {
                    found.by_value.entry(*value).or_default().push(*made);
                }
                if one.requires.iter().any(|(held, _)| only(held.value)) {
                    found.required.insert((block_index, position));
                }
            }
        }
        found
    }

    /// From a plan: the instructions that will name values, as (block, position, values defined, values read), in the order the body
    /// holds them. For values the body does not hold yet (the spiller's homes), whose intervals are asked of the body with the plan applied.
    pub fn planned(named: &[(usize, usize, BTreeSet<u32>, BTreeSet<u32>)]) -> Self {
        let mut found = Self::default();
        for (block, position, defined, used) in named {
            for value in defined.union(used) {
                found.by_value.entry(*value).or_default().push(Named { place: (*block, *position), defined: defined.contains(value), used: used.contains(value), mentions: 1 });
            }
        }
        found
    }

    /// The intervals of `values` (ascending) found from the occurrences, unweighed.
    pub fn ranges(&self, body: &LirBody, index: &Indexes, values: &[u32]) -> IndexMap<u32, Interval> {
        intervals::intervals_by_occurrences(body, index, values, &self.occurrences())
    }

    /// How many times this body's facts have scanned it for occurrences, for a test that a caller that has the postings does not.
    pub fn scans(body: &LirBody) -> usize {
        body.facts.0.counted("occurrence-scans")
    }

    /// From the postings of the body, of `values`.
    pub fn of(postings: &Postings, values: &BTreeSet<u32>) -> Self {
        let mut found = Self::default();
        for value in values {
            let mut named: std::collections::BTreeMap<Place, Named> = std::collections::BTreeMap::new();
            for (list, defined) in [(postings.defs(*value), true), (postings.uses(*value), false)] {
                for &(block, position) in list {
                    let place = (block as usize, position as usize);
                    let made = named.entry(place).or_insert(Named { place, defined: false, used: false, mentions: 0 });
                    made.defined |= defined;
                    made.used |= !defined;
                    made.mentions += 1;
                }
            }
            found.by_value.insert(*value, named.into_values().collect());
            found.required.extend(postings.needs(*value).iter().map(|&(block, position)| (block as usize, position as usize)));
        }
        found
    }

    /// What `LLRM_CHECK_OCCURRENCES` holds the postings' occurrences to: the scan's.
    pub fn check_against_scan(&self, body: &LirBody, only: &impl Fn(u32) -> bool) {
        let scanned = Self::scan(body, only);
        let same = |a: &Self, b: &Self| a.by_value.iter().filter(|(_, named)| !named.is_empty()).all(|(value, named)| b.named(*value) == named.as_slice());
        assert!(same(self, &scanned) && same(&scanned, self) && self.required == scanned.required, "{}: the occurrences from the postings differ from a scan", body.name);
    }

    pub fn named(&self, value: u32) -> &[Named] {
        self.by_value.get(&value).map_or(&[], Vec::as_slice)
    }

    /// The values that occur, ascending.
    pub fn values(&self) -> Vec<u32> {
        let mut values: Vec<u32> = self.by_value.iter().filter(|(_, named)| !named.is_empty()).map(|(value, _)| *value).collect();
        values.sort_unstable();
        values
    }

    /// Every instruction that names, or requires, one of the values.
    pub fn places(&self) -> BTreeSet<Place> {
        self.by_value.values().flatten().map(|made| made.place).chain(self.required.iter().copied()).collect()
    }

    fn occurrences(&self) -> IndexMap<u32, Vec<intervals::Occurrence>> {
        self.by_value.iter().map(|(value, named)| (*value, named.iter().map(|made| (made.place, made.defined, made.used)).collect())).collect()
    }

    /// What is live into and out of each block for these values: `live_rows_by`'s, in a body with no phis (None where it has them).
    pub fn rows(&self, body: &LirBody) -> Option<WebRows> {
        if body.blocks.iter().any(|block| !block.phis.is_empty()) {
            return None;
        }
        let rows = allocate::live_rows_among(body, &self.values(), &self.occurrences());
        if llrm_support::env_set("LLRM_CHECK_OCCURRENCES") {
            let wanted: BTreeSet<u32> = self.values().into_iter().collect();
            let walk = allocate::live_rows_by(body, |value| wanted.contains(&value));
            for block in &body.blocks {
                let (a, b): (Vec<u32>, Vec<u32>) = (walk.entering(block.at).collect(), rows.entering(block.at).collect());
                let (c, d): (Vec<u32>, Vec<u32>) = (walk.leaving(block.at).collect(), rows.leaving(block.at).collect());
                assert!(a == b && c == d, "{}: the rows found from the occurrences differ from the walk in block {:#x}: in {a:?} / {b:?}, out {c:?} / {d:?}", body.name, block.at);
            }
        }
        Some(rows)
    }

    /// The widest each wanted value is named by the instructions that name it.
    pub fn widths(&self, body: &LirBody, wanted: &dyn Fn(u32) -> bool) -> IndexMap<u32, u32> {
        let named: Vec<&Arc<Insn>> = self.places().into_iter().map(|(block, position)| &body.blocks[block].insns[position]).collect();
        let widths = widths_of(named.into_iter(), wanted);
        if llrm_support::env_set("LLRM_CHECK_OCCURRENCES") {
            assert!(widths == widths_of(body.blocks.iter().flat_map(|block| &block.insns), wanted), "{}: the widths found from the occurrences differ from the walk's", body.name);
        }
        widths
    }

    /// The intervals of the values, weighed: `worked_out_with_totals`'s, in a body with no phis, found from the occurrences, and the
    /// total each value's references weigh before they are divided by its size.
    pub fn intervals(&self, body: &LirBody, index: &Indexes, busy: &Frequency) -> (IndexMap<u32, Interval>, IndexMap<u32, f64>) {
        let mut totals: IndexMap<u32, f64> = IndexMap::default();
        for (value, named) in &self.by_value {
            for made in named {
                let each = busy.block(body.blocks[made.place.0].at);
                for _ in 0..made.mentions {
                    *totals.entry(*value).or_insert(0.0) += each;
                }
            }
        }
        let ranges = intervals::intervals_by_occurrences(body, index, &self.values(), &self.occurrences());
        let weight = intervals::divided(&totals, &ranges);
        let answer = ranges
            .into_iter()
            .map(|(value, one)| {
                let weight = weight.get(&value).copied().unwrap_or(0.0);
                (value, Interval { weight, ..one })
            })
            .collect();
        (answer, totals)
    }
}

/// Where each of `values` is live, as `live_rows_by` finds it, from one pass for their occurrences and the blocks they are live in: what
/// a caller that asks of a few values at a time reads (`LiveAt`).
pub fn live_among(body: &LirBody, values: &BTreeSet<u32>) -> Box<dyn allocate::LiveAt> {
    match Occurrences::scan(body, &|value| values.contains(&value)).rows(body) {
        Some(rows) => Box::new(rows),
        None => Box::new(allocate::live_rows_by(body, |value| values.contains(&value))),
    }
}

/// The widest each wanted value is named by `insns`.
pub fn widths_of<'a>(insns: impl Iterator<Item = &'a Arc<Insn>>, wanted: &dyn Fn(u32) -> bool) -> IndexMap<u32, u32> {
    let mut widths: IndexMap<u32, u32> = IndexMap::default();
    for one in insns {
        let mut held: Vec<Held> = match &one.what {
            Some(what) => what.dests.iter().chain(&what.sources).flat_map(ir::values).collect(),
            None => Vec::new(),
        };
        held.extend(one.requires.iter().chain(&one.delivers).map(|(value, _)| *value));
        for value in held {
            if wanted(value.value) {
                let had = widths.get(&value.value).copied().unwrap_or(0);
                widths.insert(value.value, had.max(value.width));
            }
        }
        for (value, width) in &one.widths {
            if wanted(*value) {
                let had = widths.get(value).copied().unwrap_or(0);
                widths.insert(*value, had.max(*width));
            }
        }
    }
    widths
}
