//! An exact allocator, as an oracle for the greedy one: the cheapest set of
//! values to spill so the rest fit, by branch and bound.
//!
//! It owns no fact of its own. The values, their spill weights, their classes
//! and what clobbers them are `allocate::Facts`; whether a register is free
//! for a value is `allocate::_free`; the greedy cost is the incumbent it must
//! beat. It prices what greedy prices: whole values to the stack, no splits.

use std::collections::BTreeSet;

use iced_x86::Register;

use crate::backend::allocate::{self, Assignment, Error, Facts, _free, _values, _whole, candidates};
use crate::backend::cpu::Profile;
use crate::backend::datagroup;
use crate::backend::target::Segments;
use crate::model::lir::LirBody;
use crate::support::hash::IndexMap;

/// Most nodes a search may visit before it gives the incumbent back unproved.
pub const BUDGET: usize = 2_000_000;

struct Search<'a> {
    facts: &'a Facts,
    order: Vec<u32>,
    choices: IndexMap<u32, Vec<Register>>,
    free: BTreeSet<u32>,
    best: Assignment,
    union: IndexMap<Register, Vec<u32>>,
    placed: IndexMap<u32, Register>,
    spilled: BTreeSet<u32>,
    nodes: usize,
}

impl Search<'_> {
    fn width(&self, value: u32) -> u32 {
        self.facts.widths.get(&value).copied().unwrap_or(4)
    }

    fn visit(&mut self, at: usize, cost: f64) {
        self.nodes += 1;
        if self.nodes > BUDGET || cost >= self.best.cost {
            return;
        }
        let Some(&value) = self.order.get(at) else {
            self.best = Assignment {
                r#where: self.placed.clone(),
                spilled: self.spilled.clone(),
                cost,
                optimal: true,
                why: "branch and bound".to_owned(),
            };
            return;
        };
        let mine = &self.facts.live[&value];
        let width = self.width(value);
        for register in self.choices[&value].clone() {
            if _free(mine, &[register], &self.union, &self.facts.live, &self.facts.masks, width).is_none() {
                continue;
            }
            self.union.entry(_whole(register)).or_default().push(value);
            self.placed.insert(value, register);
            self.visit(at + 1, cost);
            self.placed.shift_remove(&value);
            self.union.get_mut(&_whole(register)).expect("just pushed").pop();
        }
        if self.free.contains(&value) {
            self.spilled.insert(value);
            self.visit(at + 1, cost + mine.weight);
            self.spilled.remove(&value);
        }
    }
}

/// `greedy`, or something cheaper: the least total weight of spilled values
/// over every legal placement of the others. `optimal` says the search ended
/// inside `BUDGET`; if not, the answer is the best found, no more.
pub fn improved(
    body: &LirBody,
    pinned: &IndexMap<u32, Register>,
    unspillable: &BTreeSet<u32>,
    protected: &BTreeSet<u32>,
    profile: &Profile,
    segments: &Segments,
    greedy: &Assignment,
) -> Assignment {
    let facts = Facts::of(body, profile, segments, unspillable, protected);
    let data_free = !datagroup::names_data_segment(body, segments);
    let mut order: Vec<u32> = _values(body).into_iter().filter(|one| facts.live.contains_key(one)).collect();
    order.sort_by(|a, b| {
        let key = |v: &u32| (!pinned.contains_key(v), -facts.live[v].weight);
        key(a).partial_cmp(&key(b)).expect("weights are numbers")
    });
    let choices = order.iter().map(|one| (*one, candidates(*one, &facts, pinned, data_free, segments))).collect();
    let free = order.iter().copied().filter(|one| !pinned.contains_key(one) && facts.live[one].weight.is_finite()).collect();
    let mut search = Search {
        facts: &facts,
        order,
        choices,
        free,
        best: Assignment { optimal: false, why: "greedy held".to_owned(), ..greedy.clone() },
        union: IndexMap::default(),
        placed: IndexMap::default(),
        spilled: BTreeSet::new(),
        nodes: 0,
    };
    search.visit(0, 0.0);
    let proved = search.nodes <= BUDGET;
    let mut best = search.best;
    best.optimal = proved;
    if proved && best.why == "greedy held" {
        best.why = "greedy is optimal".to_owned();
    }
    best
}

/// Greedy against exact on one body, on the `exact` debug channel:
/// `name: greedy G, exact E (proved|unproved)`.
pub fn report(body: &LirBody, pinned: &IndexMap<u32, Register>, unspillable: &BTreeSet<u32>, profile: &Profile, segments: &Segments) {
    let protected = BTreeSet::new();
    let greedy = match allocate::allocate(body, Some(pinned), Some(unspillable), Some(&protected), None, (&profile.clone()).into(), segments) {
        Ok(one) => one,
        Err(why) => return llrm_support::debug!("exact", "{}: greedy refused: {why}", body.name),
    };
    let better = improved(body, pinned, unspillable, &protected, profile, segments, &greedy);
    llrm_support::debug!(
        "exact",
        "{}: values {} greedy {} exact {} {}",
        body.name,
        _values(body).len(),
        greedy.cost,
        better.cost,
        if better.optimal { "proved" } else { "unproved" }
    );
}
