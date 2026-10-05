//! Boundary splits: a value confined to a register class that a loop contests lives in a copy of its own inside the
//! loop, made on the loop's entry edge and given back on its exit edge.
//!
//! A value takes the class its reads need together. An outer pointer read as a base in the outer loop and as plain data
//! in the inner one is confined by the first, and holds a register of the class through the inner loop that the
//! inner loop's own address operands then lack. Evicting it reloads it every trip; the copy costs two moves per entry.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use iced_x86::Register;

use crate::analysis::frequency::Frequency;
use crate::analysis::loops;
use crate::backend::ssaspill::Prices;
use crate::backend::target::{self, Segments};
use crate::backend::{allocate, constrain, regclass, spiller};
use crate::model::ir::{Held, Loc};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::support::hash::IndexMap;

/// `body` with the splits that pay, or None where none do.
pub fn split_at_loops(body: &LirBody, segments: &Segments, prices: Prices, frequency: &Frequency) -> Option<LirBody> {
    let confined = regclass::classes(body, &BTreeSet::new(), segments);
    let general: BTreeSet<Register> = target::AVAILABLE.iter().map(|one| allocate::_whole(*one)).collect();
    let class_of = |value: u32| -> Option<BTreeSet<Register>> {
        let class: BTreeSet<Register> = confined.get(&value)?.iter().map(|one| allocate::_whole(*one)).filter(|one| general.contains(one)).collect();
        (!class.is_empty() && class.len() < general.len()).then_some(class)
    };
    let weight = |at: i64| if prices.by_frequency { frequency.block(at) } else { 1.0 };
    let edge_weight = |from: i64, to: i64| if prices.by_frequency { frequency.edge(from, to) } else { 1.0 };
    let (live_in, live_out) = allocate::live(body);
    let found = loops::loops(&body.blocks, Some(body.entry));
    let doms = loops::dominators(&body.blocks, Some(body.entry));
    let by_at: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut preds: IndexMap<i64, Vec<i64>> = IndexMap::default();
    for block in &body.blocks {
        for to in &block.succ {
            preds.entry(*to).or_default().push(block.at);
        }
    }
    let reads = regclass::confining_uses(body, segments);
    let mut next = spiller::_next_value(body);
    let mut at_end: IndexMap<i64, Vec<(u32, u32)>> = IndexMap::default();
    let mut at_top: IndexMap<i64, Vec<(u32, u32)>> = IndexMap::default();
    // Per block: renames of reads, and per phi operand coming from a block.
    let mut renames: IndexMap<i64, IndexMap<u32, u32>> = IndexMap::default();
    let mut taken: BTreeSet<u32> = BTreeSet::new();
    for one in &found {
        if found.iter().any(|other| other.header != one.header && one.body.contains(&other.header) && other.body.is_subset(&one.body)) {
            continue;
        }
        // The one way in and the one way out the copies can sit on.
        let outside: Vec<i64> = preds.get(&one.header).into_iter().flatten().copied().filter(|from| !one.body.contains(from)).collect();
        let [entering] = outside.as_slice() else { continue };
        if by_at[entering].succ.len() != 1 {
            continue;
        }
        let exits: Vec<(i64, i64)> = one.body.iter().flat_map(|at| by_at[at].succ.iter().map(move |to| (*at, *to))).filter(|(_, to)| !one.body.contains(to)).collect();
        let [(leaving, after)] = exits.as_slice() else { continue };
        if preds.get(after).is_none_or(|from| from.len() != 1) {
            continue;
        }
        // Where the peak demand on each class is, over the loop.
        let mut peak: BTreeMap<BTreeSet<Register>, usize> = BTreeMap::new();
        let classes: BTreeSet<BTreeSet<Register>> = live_in[&one.header].iter().chain(one.body.iter().flat_map(|at| live_out[at].iter())).filter_map(|value| class_of(*value)).collect();
        for at in &one.body {
            let mut live: BTreeSet<u32> = live_out[at].clone();
            let mut states = vec![live.clone()];
            for insn in by_at[at].insns.iter().rev() {
                for value in &insn.defines {
                    live.remove(value);
                }
                live.extend(insn.uses.iter().copied());
                states.push(live.clone());
            }
            for state in &states {
                for class in &classes {
                    let count = state.iter().filter(|value| class_of(**value).is_some_and(|mine| mine.is_subset(class))).count();
                    let entry = peak.entry(class.clone()).or_default();
                    *entry = (*entry).max(count);
                }
            }
        }
        let contested: Vec<&BTreeSet<Register>> = classes.iter().filter(|class| peak.get(*class).copied().unwrap_or(0) > class.len()).collect();
        if std::env::var_os("TRACECS").is_some() {
            eprintln!("CS {} loop {:#x} body {:?} peak {:?} contested {}", body.name, one.header, one.body, peak, contested.len());
        }
        if contested.is_empty() {
            continue;
        }
        let dominated: BTreeSet<i64> = doms.iter().filter(|(_, above)| above.contains(after)).map(|(at, _)| *at).collect();
        // What can be reached after the loop.
        let mut later: BTreeSet<i64> = BTreeSet::new();
        let mut todo = vec![*after];
        while let Some(at) = todo.pop() {
            if later.insert(at) {
                todo.extend(by_at[&at].succ.iter().copied());
            }
        }
        let mut candidates: Vec<(f64, u32, BTreeSet<Register>)> = Vec::new();
        for value in live_in[&one.header].iter().copied() {
            if taken.contains(&value) || by_at.values().any(|block| block.phis.iter().any(|phi| phi.result == value && one.body.contains(&block.at))) {
                continue;
            }
            if std::env::var_os("TRACECS").is_some() {
                eprintln!("CS   live-in {value} class {:?}", class_of(value));
            }
            let Some(own) = class_of(value) else { continue };
            if !contested.iter().any(|class| own.is_subset(class)) {
                continue;
            }
            let defined_inside = one.body.iter().any(|at| by_at[at].insns.iter().any(|insn| insn.defines.contains(&value)));
            let read: Vec<i64> = one.body.iter().copied().filter(|at| by_at[at].insns.iter().any(|insn| insn.uses.contains(&value))).collect();
            if defined_inside || read.is_empty() {
                continue;
            }
            // What its reads inside the loop need, taken alone.
            let mut inside: Option<BTreeSet<Register>> = None;
            for used in reads.iter().filter(|used| used.value == value && !used.defining && one.body.contains(&body.blocks[used.block].at)) {
                let class: BTreeSet<Register> = used.class.iter().map(|one| allocate::_whole(*one)).filter(|one| general.contains(one)).collect();
                inside = Some(match inside {
                    Some(so_far) => so_far.intersection(&class).copied().collect(),
                    None => class,
                });
            }
            if inside.as_ref().is_some_and(|class| class.is_empty() || class.is_subset(&own)) {
                continue;
            }
            // Read after the loop only where the exit dominates.
            let later_reads = later.iter().filter(|at| !one.body.contains(at)).copied().filter(|at| by_at[at].insns.iter().any(|insn| insn.uses.contains(&value)) || by_at[at].phis.iter().any(|phi| phi.incoming.iter().any(|(_, used)| *used == value)));
            if later_reads.into_iter().any(|at| !dominated.contains(&at)) {
                continue;
            }
            if std::env::var_os("TRACECS").is_some() {
                eprintln!("CS   candidate {value} own {own:?} inside {inside:?}");
            }
            let back = live_in[after].contains(&value);
            let evict = prices.load * read.iter().map(|at| weight(*at)).sum::<f64>() + prices.store * weight(one.header);
            let split = prices.copy * (edge_weight(*entering, one.header) + if back { edge_weight(*leaving, *after) } else { 0.0 });
            if split < evict {
                candidates.push((evict - split, value, own));
            }
        }
        candidates.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let mut over: BTreeMap<&BTreeSet<Register>, usize> = contested.iter().map(|class| (*class, peak[*class] - class.len())).collect();
        for (_, value, own) in candidates {
            let helps: Vec<&BTreeSet<Register>> = over.iter().filter(|(class, left)| **left > 0 && own.is_subset(class)).map(|(class, _)| *class).collect();
            if helps.is_empty() {
                continue;
            }
            for class in helps {
                if let Some(left) = over.get_mut(class) {
                    *left -= 1;
                }
            }
            let inside = next;
            let outside_name = next + 1;
            next += 2;
            taken.insert(value);
            at_end.entry(*entering).or_default().push((inside, value));
            for at in &one.body {
                renames.entry(*at).or_default().insert(value, inside);
            }
            if live_in[after].contains(&value) {
                at_top.entry(*after).or_default().push((outside_name, inside));
                for at in &dominated {
                    renames.entry(*at).or_default().insert(value, outside_name);
                }
            }
        }
    }
    if taken.is_empty() {
        return None;
    }
    let width_of = |value: u32| -> u32 {
        body.insns().iter().flat_map(|insn| insn.widths.iter()).find(|(seen, _)| *seen == value).map_or(2, |(_, width)| *width)
    };
    let mut blocks: Vec<LirBlock> = Vec::new();
    for block in &body.blocks {
        let rename = renames.get(&block.at);
        let renamed = |insn: &Arc<Insn>| -> Arc<Insn> {
            match rename {
                Some(map) if insn.uses.iter().any(|value| map.contains_key(value)) => {
                    // Reads only: a value split is made once, outside.
                    let only: IndexMap<u32, u32> = map.iter().filter(|(value, _)| !insn.defines.contains(value)).map(|(value, name)| (*value, *name)).collect();
                    spiller::_renamed(insn, &only)
                }
                _ => Arc::clone(insn),
            }
        };
        let mut insns: Vec<Arc<Insn>> = Vec::new();
        let anchor = block.insns.first().cloned();
        for (name, from) in at_top.get(&block.at).into_iter().flatten() {
            let beside = anchor.clone().unwrap_or_else(|| Arc::new(Insn::new(block.at, Some((block.at, block.at)), None, Vec::new(), Vec::new())));
            // The value given back is the original's name after the loop, so what reads it there reads this copy.
            let original = renames.get(&block.at).and_then(|map| map.iter().find(|(_, to)| *to == name).map(|(value, _)| *value));
            let width = original.map_or(2, width_of);
            insns.push(constrain::_move(&beside, Held { value: *name, width }, Loc::Held(Held { value: *from, width })));
        }
        let tail = crate::backend::splitkit::_tail(block);
        for (position, insn) in block.insns.iter().enumerate() {
            if position == tail {
                for (name, from) in at_end.get(&block.at).into_iter().flatten() {
                    let width = width_of(*from);
                    insns.push(constrain::_move(insn, Held { value: *name, width }, Loc::Held(Held { value: *from, width })));
                }
            }
            insns.push(renamed(insn));
        }
        if tail == block.insns.len() {
            if let Some(last) = block.insns.last() {
                for (name, from) in at_end.get(&block.at).into_iter().flatten() {
                    let width = width_of(*from);
                    insns.push(constrain::_move(last, Held { value: *name, width }, Loc::Held(Held { value: *from, width })));
                }
            }
        }
        let phis = block
            .phis
            .iter()
            .map(|phi| crate::model::lir::Phi {
                result: phi.result,
                incoming: phi
                    .incoming
                    .iter()
                    .map(|(from, value)| (*from, renames.get(from).and_then(|map| map.get(value)).copied().unwrap_or(*value)))
                    .collect(),
            })
            .collect();
        blocks.push(LirBlock { phis, ..block.with_insns(insns) });
    }
    Some(body.with_blocks(blocks))
}
