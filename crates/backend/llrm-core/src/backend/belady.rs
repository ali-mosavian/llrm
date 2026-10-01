//! A pressure-first spiller: Belady's furthest next use, per block, with
//! next-use distances that see across blocks (Braun & Hack, "Register
//! Spilling and Live-Range Splitting for SSA-Form Programs", CC 2009, 4).
//!
//! It decides where each value sits in a register. The answer is a region per
//! value, which `splitkit::carved_moving` cuts out as a piece and
//! `spiller::spilled_from` then spills the rest of whole: the spiller's
//! folding, rematerialization and slots, and splitkit's edge copies, all
//! apply as they do to any other spill. Greedy then assigns the result.

use std::collections::BTreeSet;

use iced_x86::Register;

use crate::analysis::intervals as ranges;
use crate::backend::allocate::{self, Classes, _whole};
use crate::backend::splitkit::Region;
use crate::backend::{spiller, target};
use crate::model::ir::Operation;
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::support::hash::IndexMap;

const FAR: i64 = 1 << 40;
/// Leaving a loop: what is used after it is used far from inside it.
const EXIT: i64 = 1 << 20;

/// What an instruction (or a whole parallel copy) does to a value.
#[derive(Clone, Copy, PartialEq)]
enum Event {
    /// Reads the value from a register.
    Use,
    /// Reads the value from its slot, if it is spilled: no register is needed.
    Fold,
    Define,
}

/// The points of a block: a parallel copy is one, reading before it writes.
fn points(block: &LirBlock) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for (at, one) in block.insns.iter().enumerate() {
        match out.last_mut() {
            Some(last) if one.group.is_some() && block.insns[last.0].group == one.group => last.1 = at + 1,
            _ => out.push((at, at + 1)),
        }
    }
    out
}

struct Flow<'a> {
    body: &'a LirBody,
    points: IndexMap<i64, Vec<(usize, usize)>>,
    /// Each value's uses and definitions in a block, by point.
    events: IndexMap<i64, IndexMap<u32, Vec<(usize, Event)>>>,
    live_in: allocate::Live,
    live_out: allocate::Live,
    depth: IndexMap<i64, u32>,
    /// Distance from a block's entry to a value's next use of any kind, and to its next register use.
    from_top: IndexMap<i64, IndexMap<u32, i64>>,
    register_top: IndexMap<i64, IndexMap<u32, i64>>,
}

impl<'a> Flow<'a> {
    fn of(body: &'a LirBody) -> Self {
        let (live_in, live_out) = allocate::live(body);
        let mut points_of = IndexMap::default();
        let mut events: IndexMap<i64, IndexMap<u32, Vec<(usize, Event)>>> = IndexMap::default();
        for block in &body.blocks {
            let spans = points(block);
            let mut found: IndexMap<u32, Vec<(usize, Event)>> = IndexMap::default();
            for (point, (from, to)) in spans.iter().enumerate() {
                let mut here: IndexMap<u32, Event> = IndexMap::default();
                for one in &block.insns[*from..*to] {
                    for value in &one.uses {
                        let event = if spiller::folds(one, *value) { Event::Fold } else { Event::Use };
                        if here.get(value) != Some(&Event::Use) {
                            here.insert(*value, event);
                        }
                    }
                }
                for one in &block.insns[*from..*to] {
                    for value in &one.defines {
                        here.entry(*value).or_insert(Event::Define);
                    }
                }
                for (value, event) in here {
                    found.entry(value).or_default().push((point, event));
                }
            }
            points_of.insert(block.at, spans);
            events.insert(block.at, found);
        }
        let mut flow = Self {
            body,
            points: points_of,
            events,
            live_in,
            live_out,
            depth: ranges::depths(body),
            from_top: body.blocks.iter().map(|block| (block.at, IndexMap::default())).collect(),
            register_top: body.blocks.iter().map(|block| (block.at, IndexMap::default())).collect(),
        };
        flow.distances();
        flow
    }

    fn leaving(&self, from: i64, to: i64) -> i64 {
        if self.depth[&to] < self.depth[&from] { EXIT } else { 0 }
    }

    /// Distance from the end of `block` to `value`'s next use.
    fn beyond(&self, block: &LirBlock, value: u32, register: bool) -> i64 {
        let table = if register { &self.register_top } else { &self.from_top };
        block
            .succ
            .iter()
            .filter_map(|to| table.get(to).and_then(|found| found.get(&value)).map(|got| got + self.leaving(block.at, *to)))
            .min()
            .unwrap_or(FAR)
    }

    /// Next-use distances from every block's entry, to a fixed point.
    fn distances(&mut self) {
        for register in [false, true] {
            for _ in 0..64 {
                let mut changed = false;
                for block in self.body.blocks.iter().rev() {
                    let length = self.points[&block.at].len() as i64;
                    for value in self.live_in[&block.at].clone() {
                        let first = self.events[&block.at]
                            .get(&value)
                            .and_then(|list| list.iter().copied().find(|(_, event)| *event != Event::Fold || !register));
                        let here = match first {
                            Some((point, Event::Use | Event::Fold)) => point as i64,
                            Some((_, Event::Define)) => FAR,
                            None => self.beyond(block, value, register).saturating_add(length).min(FAR),
                        };
                        let table = if register { &mut self.register_top } else { &mut self.from_top };
                        if table[&block.at].get(&value) != Some(&here) {
                            table.get_mut(&block.at).expect("a block").insert(value, here);
                            changed = true;
                        }
                    }
                }
                if !changed {
                    break;
                }
            }
        }
    }

    /// How far after `point` of `block` `value` is next used, by a register use only or by any.
    fn next_use(&self, block: &LirBlock, point: usize, value: u32, register: bool) -> i64 {
        if let Some(list) = self.events[&block.at].get(&value) {
            if let Some((at, event)) = list.iter().find(|(at, event)| *at > point && (*event != Event::Fold || !register)) {
                return if *event == Event::Define { FAR } else { (*at - point) as i64 };
            }
        }
        if !self.live_out[&block.at].contains(&value) {
            return FAR;
        }
        ((self.points[&block.at].len() - point) as i64).saturating_add(self.beyond(block, value, register)).min(FAR)
    }
}

/// How many registers a point leaves values of the body: the allocatable
/// ones less the fixed registers some live value is pinned to there.
fn capacity(pinned: usize) -> usize {
    target::AVAILABLE.len().saturating_sub(pinned)
}

/// The region of each value a register holds, by Belady's rule.
pub fn regions(body: &LirBody, fixed: &IndexMap<u32, Register>, confined: &Classes) -> IndexMap<u32, Region> {
    let flow = Flow::of(body);
    // What spilling each value costs: the order among values no register use soon wants.
    let weight: IndexMap<u32, i64> = ranges::intervals(body, None).into_iter().map(|(value, one)| (value, (one.weight.min(1e15) * 1000.0) as i64)).collect();
    let general: BTreeSet<Register> = target::AVAILABLE.iter().map(|one| _whole(*one)).collect();
    let wants_register = |value: u32| -> bool {
        !fixed.contains_key(&value) && confined.get(&value).is_none_or(|class| class.iter().any(|one| general.contains(&_whole(*one))))
    };
    let mut out: IndexMap<u32, Region> = IndexMap::default();
    let mut ending: IndexMap<i64, BTreeSet<u32>> = IndexMap::default();
    let preds = {
        let mut found: IndexMap<i64, Vec<i64>> = IndexMap::default();
        for block in &body.blocks {
            for to in &block.succ {
                found.entry(*to).or_default().push(block.at);
            }
        }
        found
    };
    if std::env::var_os("BELADY_DUMP").is_some() {
        eprintln!("{}", crate::tools::stages::lir_stage("belady-input", &[(body.name.clone(), body.clone())]));
    }
    for block in &body.blocks {
        let spans = &flow.points[&block.at];
        let length = block.insns.len();
        let pinned_live = pinned_live(block, &flow, fixed);
        // The values in registers on entry: those every processed predecessor ends
        // with, then those some does, then the rest, each nearest first.
        let before: Vec<&BTreeSet<u32>> = preds.get(&block.at).into_iter().flatten().filter_map(|from| ending.get(from)).collect();
        let mut ranked: Vec<(usize, i64, std::cmp::Reverse<i64>, u32)> = flow.live_in[&block.at]
            .iter()
            .filter(|value| wants_register(**value))
            .map(|value| {
                let ends = before.iter().filter(|set| set.contains(value)).count();
                let tier = if before.is_empty() { 0 } else if ends == before.len() { 0 } else if ends > 0 { 1 } else { 2 };
                (tier, flow.register_top[&block.at].get(value).copied().unwrap_or(FAR), std::cmp::Reverse(weight.get(value).copied().unwrap_or(0)), *value)
            })
            .collect();
        ranked.sort_unstable();
        let mut held: BTreeSet<u32> = ranked.iter().take(capacity(pinned_live.first().copied().unwrap_or(0))).map(|(_, _, _, value)| *value).collect();
        let mut since: IndexMap<u32, usize> = held.iter().map(|value| (*value, 0)).collect();
        let mut close = |value: u32, at: usize, since: &mut IndexMap<u32, usize>| {
            if let Some(from) = since.shift_remove(&value) {
                if from < at {
                    let region = out.entry(value).or_insert_with(|| Region { stored_at_definition: true, ..Region::default() });
                    region.add(block.at, from, at);
                }
            }
        };
        for (point, (first, last)) in spans.iter().copied().enumerate() {
            let group = &block.insns[first..last];
            let room = capacity(pinned_live[point]);
            let mut used: BTreeSet<u32> = BTreeSet::new();
            for one in group {
                for value in one.uses.iter().chain(&one.defines) {
                    if wants_register(*value) && !group.iter().all(|each| !each.uses.contains(value) && !each.defines.contains(value) || spiller::folds(each, *value)) {
                        used.insert(*value);
                    }
                }
            }
            let evict = |held: &mut BTreeSet<u32>, since: &mut IndexMap<u32, usize>, close: &mut dyn FnMut(u32, usize, &mut IndexMap<u32, usize>), keep: &BTreeSet<u32>, limit: usize| {
                while held.len() > limit {
                    let victim = held.iter().filter(|value| !keep.contains(*value)).max_by_key(|value| (flow.next_use(block, point, **value, true), std::cmp::Reverse(weight.get(*value).copied().unwrap_or(0)), **value)).copied();
                    let Some(victim) = victim else { break };
                    held.remove(&victim);
                    close(victim, first, since);
                }
            };
            // What a call leaves alone is all that may stay in registers across it.
            let clobbered: BTreeSet<Register> = group.iter().filter(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Call)).flat_map(|one| one.clobbers.iter().map(|register| _whole(*register))).collect();
            if !clobbered.is_empty() {
                let preserved = general.iter().filter(|register| !clobbered.contains(*register)).count();
                let across: BTreeSet<u32> = held.iter().copied().filter(|value| !used.contains(value)).collect();
                let limit = held.len().saturating_sub(across.len()) + preserved.min(across.len());
                evict(&mut held, &mut since, &mut close, &used, limit);
            }
            for value in &used {
                if !held.contains(value) {
                    held.insert(*value);
                    since.insert(*value, first);
                }
            }
            evict(&mut held, &mut since, &mut close, &used, room);
            // What this point reads or writes in its slot rides in a register when there is room, the costliest first.
            let mut weak: Vec<u32> = group
                .iter()
                .flat_map(|one| one.uses.iter().chain(&one.defines).copied())
                .filter(|value| wants_register(*value) && !held.contains(value))
                .collect::<BTreeSet<u32>>()
                .into_iter()
                .collect();
            weak.sort_by_key(|value| std::cmp::Reverse(weight.get(value).copied().unwrap_or(0)));
            for value in weak {
                if held.len() < room {
                    held.insert(value);
                    since.insert(value, first);
                }
            }
            // Dead after this point: leaves, and its register is free for the next definition.
            for value in held.clone() {
                if flow.next_use(block, point, value, false) >= FAR && !group.iter().any(|one| one.defines.contains(&value) && flow.live_out[&block.at].contains(&value)) {
                    held.remove(&value);
                    close(value, last, &mut since);
                }
            }
        }
        for value in &held {
            close(*value, length, &mut since);
        }
        ending.insert(block.at, held);
    }
    if std::env::var_os("BELADY_DUMP").is_some() {
        for (value, region) in &out {
            eprintln!("REGION v{value} {:?}", region.spans);
        }
    }
    out
}

/// At each point of `block`, how many pinned values are live.
fn pinned_live(block: &LirBlock, flow: &Flow<'_>, fixed: &IndexMap<u32, Register>) -> Vec<usize> {
    let spans = &flow.points[&block.at];
    let mut live: BTreeSet<u32> = flow.live_out[&block.at].iter().copied().filter(|value| fixed.contains_key(value)).collect();
    let mut out = vec![0; spans.len()];
    for (point, (first, last)) in spans.iter().copied().enumerate().rev() {
        let group = &block.insns[first..last];
        let mut here = live.clone();
        for one in group {
            here.extend(one.uses.iter().chain(&one.defines).copied().filter(|value| fixed.contains_key(value)));
        }
        out[point] = here.len();
        for one in group {
            for value in &one.defines {
                live.remove(value);
            }
        }
        for one in group {
            live.extend(one.uses.iter().copied().filter(|value| fixed.contains_key(value)));
        }
    }
    out
}

/// Whether `one` is a call or another instruction the register file does not survive.
pub fn clobbers(one: &Insn) -> bool {
    one.what.as_ref().is_some_and(|what| what.op == Operation::Call)
}
