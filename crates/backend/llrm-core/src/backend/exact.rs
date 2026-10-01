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
use crate::backend::target::{self, Segments};
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
    /// Points where more values need a register than the machine has: the
    /// members' positions in `order`, cheapest to spill first.
    crowded: Vec<Vec<usize>>,
    registers: usize,
}

impl Search<'_> {
    fn width(&self, value: u32) -> u32 {
        self.facts.widths.get(&value).copied().unwrap_or(4)
    }

    /// The least still to be spilled: at any crowded point, the values placed
    /// there plus those undecided must come down to the registers there are,
    /// and only the undecided can be spilled, cheapest first.
    fn owed(&self, at: usize) -> f64 {
        let mut most = 0.0_f64;
        for members in &self.crowded {
            let placed = members.iter().filter(|one| **one < at && self.placed.contains_key(&self.order[**one])).count();
            let undecided = members.iter().filter(|one| **one >= at);
            let excess = (placed + undecided.clone().count()).saturating_sub(self.registers);
            let weight = |one: &usize| self.facts.live[&self.order[*one]].weight;
            most = most.max(undecided.take(excess).map(weight).sum::<f64>());
        }
        most
    }

    fn visit(&mut self, at: usize, cost: f64) {
        self.nodes += 1;
        if self.nodes > BUDGET || cost >= self.best.cost || (!self.crowded.is_empty() && cost + self.owed(at) >= self.best.cost) {
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
    solved(body, pinned, unspillable, protected, profile, segments, greedy, true)
}

/// `improved`, with the pressure bound switched on or off: off is the plain
/// search the bound must agree with.
#[allow(clippy::too_many_arguments)]
pub fn solved(
    body: &LirBody,
    pinned: &IndexMap<u32, Register>,
    unspillable: &BTreeSet<u32>,
    protected: &BTreeSet<u32>,
    profile: &Profile,
    segments: &Segments,
    greedy: &Assignment,
    bounded: bool,
) -> Assignment {
    let facts = Facts::of(body, profile, segments, unspillable, protected);
    let data_free = !datagroup::names_data_segment(body, segments);
    let mut order: Vec<u32> = _values(body).into_iter().filter(|one| facts.live.contains_key(one)).collect();
    order.sort_by(|a, b| {
        let key = |v: &u32| (!pinned.contains_key(v), -facts.live[v].weight);
        key(a).partial_cmp(&key(b)).expect("weights are numbers")
    });
    // A value no register suits even with every other value gone (it lives
    // across a call that destroys them all) goes to the stack, whatever else.
    let alone = IndexMap::default();
    let doomed: BTreeSet<u32> = order
        .iter()
        .copied()
        .filter(|one| {
            let width = facts.widths.get(one).copied().unwrap_or(4);
            facts.live[one].weight.is_finite()
                && !pinned.contains_key(one)
                && !candidates(*one, &facts, pinned, data_free, segments)
                    .iter()
                    .any(|register| _free(&facts.live[one], &[*register], &alone, &facts.live, &facts.masks, width).is_some())
        })
        .collect();
    let owed: f64 = doomed.iter().map(|one| facts.live[one].weight).sum();
    order.retain(|one| !doomed.contains(one));
    let choices: IndexMap<u32, Vec<Register>> = order.iter().map(|one| (*one, candidates(*one, &facts, pinned, data_free, segments))).collect();
    let free = order.iter().copied().filter(|one| !pinned.contains_key(one) && facts.live[one].weight.is_finite()).collect();
    let general: BTreeSet<Register> = target::AVAILABLE.iter().map(|one| _whole(*one)).collect();
    // Only a value that can take nothing but a general register is counted
    // against them: one that may go elsewhere owes the bound nothing.
    let needs: Vec<usize> = (0..order.len())
        .filter(|at| {
            let all = &choices_of(&choices, order[*at]);
            !all.is_empty() && all.iter().all(|one| general.contains(&_whole(*one)))
        })
        .collect();
    let crowded = if bounded { crowded_points(&facts, &order, &needs, general.len()) } else { Vec::new() };
    let mut search = Search {
        facts: &facts,
        order,
        choices,
        free,
        best: Assignment { optimal: false, why: "greedy held".to_owned(), ..greedy.clone() },
        union: IndexMap::default(),
        placed: IndexMap::default(),
        spilled: doomed,
        nodes: 0,
        crowded,
        registers: general.len(),
    };
    search.visit(0, owed);
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

fn choices_of(choices: &IndexMap<u32, Vec<Register>>, value: u32) -> &[Register] {
    &choices[&value]
}

/// The most crowded points: where the values needing a general register
/// outnumber the registers, as positions in `order`, cheapest first.
fn crowded_points(facts: &Facts, order: &[u32], needs: &[usize], registers: usize) -> Vec<Vec<usize>> {
    let mut points: Vec<Vec<usize>> = Vec::new();
    let mut seen: BTreeSet<Vec<usize>> = BTreeSet::new();
    for slot in needs.iter().flat_map(|one| facts.live[&order[*one]].segments.iter().map(|seg| seg.start)) {
        let mut members: Vec<usize> = needs
            .iter()
            .copied()
            .filter(|one| facts.live[&order[*one]].segments.iter().any(|seg| seg.start <= slot && slot < seg.end))
            .collect();
        if members.len() > registers && seen.insert(members.clone()) {
            members.sort_by(|a, b| facts.live[&order[*a]].weight.partial_cmp(&facts.live[&order[*b]].weight).expect("numbers"));
            points.push(members);
        }
    }
    points.sort_by_key(|one| std::cmp::Reverse(one.len()));
    points.truncate(24);
    points
}
