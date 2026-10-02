//! Copy coalescing by recolouring (Hack & Goos, PLDI 2008) over the colours
//! `ssacolour` chose in dominance order. That order commits a register before
//! the demands after it are seen; here the values an affinity joins (a phi's
//! result and arguments, a tied result and its first source, a copy, a wish
//! for one register) are moved to one register where no value live beside them
//! holds it, weighed by block frequency.

use std::collections::BTreeSet;

use iced_x86::Register;

use crate::analysis::frequency::Frequency;
use crate::backend::allocate::{self, _whole};
use crate::backend::target::{self, Segments};
use crate::backend::{coalesce, ssaassign, ssacolour, twoaddr};
use crate::model::ir::Loc;
use crate::model::lir::LirBody;
use crate::support::hash::IndexMap;

/// What an affinity joins a value to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wants {
    /// Another value's register.
    Value(u32),
    /// One register.
    Register(Register),
}

/// `value` would rather share a register with `wants`, worth `weight` executions.
#[derive(Clone, Copy, Debug)]
pub struct Affinity {
    pub value: u32,
    pub wants: Wants,
    pub weight: f64,
}

/// Which values may not share a register: those live together.
pub type Interference = IndexMap<u32, BTreeSet<u32>>;

/// For each value, the values live where it is written: SSA's interference graph.
///
/// A tied instruction writes its first source's register while its other sources
/// are still read, so its result may share the first source's and no other's.
pub fn interference(body: &LirBody) -> Interference {
    let (live_in, live_out) = allocate::live(body);
    let mut out: Interference = IndexMap::default();
    let mut link = |a: u32, b: u32| {
        if a != b {
            out.entry(a).or_default().insert(b);
            out.entry(b).or_default().insert(a);
        }
    };
    for block in &body.blocks {
        // Phi results are written together, on entry.
        for phi in &block.phis {
            for other in &block.phis {
                link(phi.result, other.result);
            }
            for value in &live_in[&block.at] {
                link(phi.result, *value);
            }
        }
        let mut live = live_out[&block.at].clone();
        for one in block.insns.iter().rev() {
            for def in &one.defines {
                for value in live.iter().filter(|value| !one.defines.contains(value)) {
                    link(*def, *value);
                }
                for other in &one.defines {
                    link(*def, *other);
                }
                // A tied result is written beside the sources it does not take over.
                if let Some(first) = twoaddr::tie_source(one, &live) {
                    for source in one.uses.iter().filter(|source| **source != first) {
                        link(*def, *source);
                    }
                }
            }
            for value in &one.defines {
                live.remove(value);
            }
            live.extend(one.uses.iter().copied());
        }
    }
    out
}

/// What the body would like in one register, weighed by how often it runs.
pub fn affinities(body: &LirBody, frequency: &Frequency) -> Vec<Affinity> {
    let (_, live_out) = allocate::live(body);
    let mut out: Vec<Affinity> = Vec::new();
    for block in &body.blocks {
        for phi in &block.phis {
            for (from, value) in &phi.incoming {
                out.push(Affinity { value: phi.result, wants: Wants::Value(*value), weight: frequency.edge(*from, block.at) });
            }
        }
        let here = frequency.block(block.at);
        // The same literal made twice in a block costs one `mov` where both sit in one register.
        let mut literals: Vec<(crate::model::ir::Imm, u32)> = Vec::new();
        for one in &block.insns {
            let Some(what) = one.what.as_ref().filter(|what| what.op == crate::model::ir::Operation::Move) else { continue };
            if let ([Loc::Held(into)], [Loc::Imm(number)]) = (what.dests.as_slice(), what.sources.as_slice()) {
                if let Some((_, earlier)) = literals.iter().find(|(seen, _)| seen == number) {
                    out.push(Affinity { value: into.value, wants: Wants::Value(*earlier), weight: here });
                }
                literals.retain(|(seen, _)| seen != number);
                literals.push((number.clone(), into.value));
            }
        }
        let mut live = live_out[&block.at].clone();
        for one in block.insns.iter().rev() {
            if let Some((into, out_of)) = coalesce::_copy(one) {
                out.push(Affinity { value: into, wants: Wants::Value(out_of), weight: here });
            } else if let (Some(first), [made]) = (twoaddr::tie_source(one, &live), &one.defines[..]) {
                // A source still read after the instruction cannot share the result's register.
                if !live.contains(&first) {
                    out.push(Affinity { value: *made, wants: Wants::Value(first), weight: here });
                }
            }
            for (held, register) in one.requires.iter().chain(&one.delivers) {
                out.push(Affinity { value: held.value, wants: Wants::Register(_whole(*register)), weight: here });
            }
            if let Some(what) = &one.what {
                for (place, register) in target::requirements(what) {
                    let side = if place.side == "dest" { &what.dests } else { &what.sources };
                    if let Some(Loc::Held(held)) = side.get(place.index) {
                        out.push(Affinity { value: held.value, wants: Wants::Register(_whole(register)), weight: here });
                    }
                }
            }
            for value in &one.defines {
                live.remove(value);
            }
            live.extend(one.uses.iter().copied());
        }
    }
    out
}

/// Visited values one attempt may move before it gives up.
const BUDGET: usize = 64;
/// Passes over the chunks while a pass still gains.
const PASSES: usize = 3;
/// Smaller gains are no gain: floating point's noise must not look like progress.
const EPSILON: f64 = 1e-9;

struct Model<'a> {
    colour: IndexMap<u32, Register>,
    interferes: Interference,
    /// Registers a value may take, where it has no choice (byte values).
    bound: IndexMap<u32, BTreeSet<Register>>,
    /// Registers a value whose result dies where it is written may not take: what its instruction clobbers.
    forbidden: IndexMap<u32, BTreeSet<Register>>,
    /// Pinned values never move.
    fixed: &'a IndexMap<u32, Register>,
    /// Where each value lives through an instruction that takes registers: (executions, registers).
    through: IndexMap<u32, Vec<(f64, BTreeSet<Register>)>>,
    /// Where each value forms an address: (executions, registers that can).
    demands: IndexMap<u32, Vec<(f64, BTreeSet<Register>)>>,
    affinities: Vec<Affinity>,
    touching: IndexMap<u32, Vec<usize>>,
    registers: Vec<Register>,
}

/// One attempt's moves, and the values it may no longer move.
#[derive(Clone, Default)]
struct Attempt {
    moved: IndexMap<u32, Register>,
    locked: BTreeSet<u32>,
    visited: usize,
}

impl Model<'_> {
    fn at(&self, attempt: &Attempt, value: u32) -> Option<Register> {
        attempt.moved.get(&value).or_else(|| self.colour.get(&value)).copied()
    }

    fn permitted(&self, value: u32, register: Register) -> bool {
        self.registers.contains(&register)
            && self.bound.get(&value).is_none_or(|class| class.contains(&register))
            && self.forbidden.get(&value).is_none_or(|set| !set.contains(&register))
    }

    /// What a value's register costs it: a relocation pair at each instruction that takes
    /// the register while it lives through, a copy at each address it cannot form.
    fn price(&self, value: u32, register: Register) -> f64 {
        let moved: f64 = self.through.get(&value).into_iter().flatten().filter(|(_, taken)| taken.contains(&register)).map(|(weight, _)| 2.0 * weight).sum();
        let copied: f64 = self.demands.get(&value).into_iter().flatten().filter(|(_, allowed)| !allowed.contains(&register)).map(|(weight, _)| weight).sum();
        moved + copied
    }

    /// The affinities satisfied and the prices paid around `nodes`, as `lookup` colours them.
    fn score(&self, nodes: &BTreeSet<u32>, lookup: &dyn Fn(u32) -> Option<Register>) -> f64 {
        let seen: BTreeSet<usize> = nodes.iter().flat_map(|node| self.touching.get(node).into_iter().flatten().copied()).collect();
        let joined: f64 = seen
            .into_iter()
            .map(|at| {
                let one = &self.affinities[at];
                let mine = lookup(one.value);
                let satisfied = match one.wants {
                    Wants::Value(other) => mine.is_some() && mine == lookup(other),
                    Wants::Register(register) => mine == Some(register),
                };
                if satisfied { one.weight } else { 0.0 }
            })
            .sum();
        let paid: f64 = nodes.iter().filter_map(|node| lookup(*node).map(|register| self.price(*node, register))).sum();
        joined - paid
    }

    /// `value` in `register`, moving the values beside it that hold `register` to others.
    fn assign(&self, value: u32, register: Register, attempt: &mut Attempt) -> bool {
        if !self.claim(value, register, attempt) {
            return false;
        }
        self.evict(value, register, attempt)
    }

    /// `value` takes `register` and may not move again in this attempt.
    fn claim(&self, value: u32, register: Register, attempt: &mut Attempt) -> bool {
        if self.at(attempt, value) == Some(register) {
            attempt.locked.insert(value);
            return true;
        }
        if self.fixed.contains_key(&value) || !self.colour.contains_key(&value) || !self.permitted(value, register) || attempt.visited >= BUDGET {
            return false;
        }
        attempt.visited += 1;
        attempt.locked.insert(value);
        attempt.moved.insert(value, register);
        true
    }

    /// The values beside `value`, which holds `register`, other than the ones locked, in other registers.
    fn evict(&self, value: u32, register: Register, attempt: &mut Attempt) -> bool {
        let beside: Vec<u32> = self.interferes.get(&value).into_iter().flatten().copied().collect();
        for other in beside {
            if self.at(attempt, other) != Some(register) {
                continue;
            }
            if attempt.locked.contains(&other) {
                return false;
            }
            let mut elsewhere = self.partners(other, attempt);
            elsewhere.extend(self.registers.iter().copied());
            let mut done = false;
            for choice in elsewhere {
                if choice == register || !self.permitted(other, choice) {
                    continue;
                }
                let before = attempt.clone();
                if self.assign(other, choice, attempt) {
                    done = true;
                    break;
                }
                *attempt = before;
            }
            if !done {
                return false;
            }
        }
        true
    }

    /// Every member in `register`, claimed together before any value beside one is moved.
    fn assign_all(&self, members: &[u32], register: Register, attempt: &mut Attempt) -> bool {
        members.iter().all(|value| self.claim(*value, register, attempt)) && members.iter().all(|value| self.evict(*value, register, attempt))
    }

    /// The registers `value`'s affinity partners hold now.
    fn partners(&self, value: u32, attempt: &Attempt) -> Vec<Register> {
        let mut out: Vec<Register> = Vec::new();
        for at in self.touching.get(&value).into_iter().flatten() {
            let one = &self.affinities[*at];
            let found = match one.wants {
                Wants::Register(register) => Some(register),
                Wants::Value(other) => self.at(attempt, if one.value == value { other } else { one.value }),
            };
            if let Some(register) = found {
                if !out.contains(&register) {
                    out.push(register);
                }
            }
        }
        out
    }
}

/// `colour` with affinity chunks moved to one register where that gains, still a proper colouring.
pub fn recoloured(body: &LirBody, colour: &IndexMap<u32, Register>, fixed: &IndexMap<u32, Register>, segments: &Segments) -> IndexMap<u32, Register> {
    let frequency = Frequency::of(body);
    let general_roots: BTreeSet<Register> = target::AVAILABLE.iter().map(|one| _whole(*one)).collect();
    let mut registers: Vec<Register> = Vec::new();
    for register in target::AVAILABLE.iter().map(|one| _whole(*one)) {
        if !registers.contains(&register) {
            registers.push(register);
        }
    }
    let (_, live_out) = allocate::live(body);
    let mut through: IndexMap<u32, Vec<(f64, BTreeSet<Register>)>> = IndexMap::default();
    let mut forbidden: IndexMap<u32, BTreeSet<Register>> = IndexMap::default();
    for block in &body.blocks {
        let here = frequency.block(block.at);
        let mut live = live_out[&block.at].clone();
        for one in block.insns.iter().rev() {
            let taken = ssaassign::takes(one, &general_roots);
            if !taken.is_empty() {
                for value in live.iter().filter(|value| !one.defines.contains(value)) {
                    through.entry(*value).or_default().push((here, taken.clone()));
                }
            }
            let clobbered: BTreeSet<Register> = one.clobbers.iter().map(|register| _whole(*register)).collect();
            if !clobbered.is_empty() {
                for value in one.defines.iter().filter(|value| !live.contains(value)) {
                    forbidden.entry(*value).or_default().extend(clobbered.iter().copied());
                }
            }
            for value in &one.defines {
                live.remove(value);
            }
            live.extend(one.uses.iter().copied());
        }
    }
    let mut demands: IndexMap<u32, Vec<(f64, BTreeSet<Register>)>> = IndexMap::default();
    for (value, allowed, at) in ssacolour::address_demands(body) {
        demands.entry(value).or_default().push((frequency.block(at), allowed));
    }
    let affinities: Vec<Affinity> = affinities(body, &frequency).into_iter().filter(|one| colour.contains_key(&one.value)).collect();
    let mut touching: IndexMap<u32, Vec<usize>> = IndexMap::default();
    for (at, one) in affinities.iter().enumerate() {
        touching.entry(one.value).or_default().push(at);
        if let Wants::Value(other) = one.wants {
            touching.entry(other).or_default().push(at);
        }
    }
    let mut model = Model {
        colour: colour.clone(),
        interferes: interference(body),
        bound: ssacolour::byte_classes(body, segments),
        forbidden,
        fixed,
        through,
        demands,
        affinities,
        touching,
        registers,
    };
    let everyone: BTreeSet<u32> = model.colour.keys().copied().collect();
    let before = model.score(&everyone, &|value| model.colour.get(&value).copied());
    for _ in 0..PASSES {
        if !pass(&mut model) {
            break;
        }
    }
    let after = model.score(&everyone, &|value| model.colour.get(&value).copied());
    let moved = model.colour.iter().filter(|(value, register)| colour.get(*value) != Some(*register)).count();
    llrm_support::debug!("ssarecolour", "{}: {} affinities, score {before:.1} -> {after:.1}, {moved} values moved", body.name, model.affinities.len());
    if let Some(why) = ssaassign::improper(body, &model.colour) {
        llrm_support::debug!("ssaassign", "{}: recolouring made no assignment ({why}); kept the dominance-order colours", body.name);
        return colour.clone();
    }
    model.colour
}

/// Chunks by affinity weight, each tried in the registers its members hold and wish for. Whether any moved.
fn pass(model: &mut Model<'_>) -> bool {
    let mut edges: Vec<(f64, u32, u32)> = model
        .affinities
        .iter()
        .filter_map(|one| match one.wants {
            Wants::Value(other) if model.colour.contains_key(&other) && other != one.value => Some((one.weight, one.value, other)),
            _ => None,
        })
        .collect();
    edges.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut chunk_of: IndexMap<u32, usize> = IndexMap::default();
    let mut chunks: Vec<Vec<u32>> = Vec::new();
    for (_, a, b) in edges {
        for value in [a, b] {
            if !chunk_of.contains_key(&value) {
                chunk_of.insert(value, chunks.len());
                chunks.push(vec![value]);
            }
        }
        let (ca, cb) = (chunk_of[&a], chunk_of[&b]);
        if ca == cb {
            continue;
        }
        let beside = |x: &u32, y: &u32| model.interferes.get(x).is_some_and(|set| set.contains(y));
        let joined: Vec<u32> = chunks[ca].iter().chain(&chunks[cb]).copied().collect();
        let pinned: BTreeSet<Register> = joined.iter().filter_map(|value| model.fixed.get(value)).map(|register| _whole(*register)).collect();
        let room = joined.iter().filter_map(|value| model.bound.get(value)).fold(None::<BTreeSet<Register>>, |all, class| match all {
            None => Some(class.clone()),
            Some(all) => Some(all.intersection(class).copied().collect()),
        });
        let compatible = pinned.len() <= 1 && room.is_none_or(|room| !room.is_empty() && pinned.iter().all(|register| room.contains(register)));
        if !compatible || chunks[ca].iter().any(|x| chunks[cb].iter().any(|y| beside(x, y))) {
            continue;
        }
        let moving = std::mem::take(&mut chunks[cb]);
        for value in &moving {
            chunk_of.insert(*value, ca);
        }
        chunks[ca].extend(moving);
    }
    let mut order: Vec<(f64, usize)> = chunks
        .iter()
        .enumerate()
        .filter(|(_, members)| members.len() > 1)
        .map(|(at, members)| {
            let inside: BTreeSet<u32> = members.iter().copied().collect();
            let weight: f64 = model
                .affinities
                .iter()
                .filter(|one| inside.contains(&one.value) && matches!(one.wants, Wants::Value(other) if inside.contains(&other)))
                .map(|one| one.weight)
                .sum();
            (weight, at)
        })
        .collect();
    order.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut changed = false;
    for (_, at) in order {
        let members = chunks[at].clone();
        let mut wanted: Vec<Register> = Vec::new();
        for value in &members {
            let held = model.colour.get(value).copied();
            let wished = model.touching.get(value).into_iter().flatten().filter_map(|at| match model.affinities[*at].wants {
                Wants::Register(register) if model.affinities[*at].value == *value => Some(register),
                _ => None,
            });
            for register in held.into_iter().chain(wished) {
                if !wanted.contains(&register) {
                    wanted.push(register);
                }
            }
        }
        for register in model.registers.clone() {
            if !wanted.contains(&register) {
                wanted.push(register);
            }
        }
        let mut best: Option<(f64, Attempt)> = None;
        for register in wanted {
            let mut attempt = Attempt::default();
            let ok = model.assign_all(&members, register, &mut attempt);
            if !ok {
                continue;
            }
            let nodes: BTreeSet<u32> = attempt.moved.keys().copied().collect();
            if nodes.is_empty() {
                continue;
            }
            let after = model.score(&nodes, &|value| model.at(&attempt, value));
            let before = model.score(&nodes, &|value| model.colour.get(&value).copied());
            if after - before > EPSILON && best.as_ref().is_none_or(|(gain, _)| after - before > *gain) {
                best = Some((after - before, attempt));
            }
        }
        if let Some((_, attempt)) = best {
            model.colour.extend(attempt.moved);
            changed = true;
        }
    }
    changed
}
