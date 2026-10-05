//! The register class of each value: the one place that states which registers a value may be in.
//!
//! Owned here, read by the spiller (`ssaspill`) and the allocator (`allocate`), `coalesce` and `constrain`: one answer
//! (#441). The target's own sets (`target::ADDRESSING`, `target::BYTE`, `Segments::selectors`) are the vocabulary; what
//! an operand needs of them comes from the instruction.

use std::collections::BTreeSet;

use iced_x86::Register;

use crate::analysis::intervals as ranges;
use crate::backend::allocate::{_clobbered, _masks, _unread_move};
use crate::backend::target::{self, Segments};
use crate::model::ir::{Held, Loc, Operation, Space};
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::IndexMap;

pub type Classes = IndexMap<u32, BTreeSet<Register>>;

fn _restrict(out: &mut Classes, value: u32, choices: &BTreeSet<Register>) {
    let now: BTreeSet<Register> = match out.get(&value) {
        Some(had) => had.intersection(choices).copied().collect(),
        None => choices.clone(),
    };
    out.insert(value, now);
}

/// The register class each value is confined to, where it is confined.
pub fn classes(body: &LirBody, prefer_indexes: &BTreeSet<u32>, segments: &Segments) -> Classes {
    let mut out: Classes = IndexMap::default();
    let mut selecting: BTreeSet<u32> = BTreeSet::new();
    let mut numeric: BTreeSet<u32> = BTreeSet::new();
    let mut word_pairs: Vec<(u32, u32)> = Vec::new();
    let bytes: BTreeSet<Register> = BTreeSet::from([Register::AX, Register::BX, Register::CX, Register::DX]);

    for block in &body.blocks {
        for one in &block.insns {
            let Some(what) = &one.what else {
                continue;
            };
            for place in what.dests.iter().chain(&what.sources) {
                if let Loc::Mem(cell) = place {
                    if let Some(selector) = cell.selector {
                        selecting.insert(selector.value);
                    }
                    if let Some(base) = cell.base {
                        numeric.insert(base.value);
                    }
                }
                if let Loc::Held(held) = place {
                    if held.width != 2 || !_SEGMENT_OPERANDS.contains(&what.op) {
                        numeric.insert(held.value);
                    }
                }
                if let Loc::Mem(cell) = place {
                    if let (Some(base), None) = (cell.base, cell.index) {
                        if base.width == 2 {
                            let registers = if cell.addr.is_some_and(|addr| addr.space == Space::Frame) {
                                &*target::WORD_INDEXES
                            } else {
                                &*target::ADDRESSING
                            };
                            _restrict(&mut out, base.value, registers);
                        }
                    }
                    if let Some(index) = cell.index {
                        numeric.insert(index.value);
                        if index.width == 2 {
                            if cell.base.is_some_and(|base| base.width == 2) && cell.scale == 1 {
                                word_pairs.push((cell.base.expect("checked").value, index.value));
                            } else {
                                _restrict(&mut out, index.value, &target::WORD_INDEXES);
                                if let Some(base) = cell.base {
                                    _restrict(&mut out, base.value, &target::WORD_BASES);
                                }
                            }
                        }
                    }
                }
                if let Loc::Held(held) = place {
                    if held.width == 1 {
                        _restrict(&mut out, held.value, &bytes);
                    }
                }
            }
        }
    }
    let selectors: BTreeSet<Register> = segments.selectors.iter().copied().collect();
    for value in selecting.difference(&numeric) {
        _restrict(&mut out, *value, &selectors);
    }
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        if let Some(what) = &one.what {
            if target::far_load(what) {
                if let Loc::Held(held) = &what.dests[1] {
                    _restrict(&mut out, held.value, &selectors);
                }
            }
        }
    }
    _word_address_roles(&word_pairs, &mut out, body, prefer_indexes, segments);
    out
}

/// Choose BX versus SI/DI for commutative `[word+word]` graphs.
fn _word_address_roles(
    pairs: &[(u32, u32)],
    confined: &mut Classes,
    body: &LirBody,
    prefer_indexes: &BTreeSet<u32>,
    segments: &Segments,
) {
    let mut adjacent: IndexMap<u32, BTreeSet<u32>> = IndexMap::default();
    for (base, index) in pairs {
        adjacent.entry(*base).or_default().insert(*index);
        adjacent.entry(*index).or_default().insert(*base);
    }
    let mut unseen: BTreeSet<u32> = adjacent.keys().copied().collect();
    let numbered = ranges::indexed(body);
    let live = ranges::intervals(body, Some(&numbered));
    let masks = _masks(body, &numbered, segments);
    let word_base = *target::WORD_BASES.iter().next().expect("one word base");

    let base_penalty = |values: &BTreeSet<u32>| -> i64 {
        values
            .iter()
            .filter_map(|value| live.get(value))
            .map(|interval| i64::from(_clobbered(interval, word_base, &masks, 2)))
            .sum()
    };

    let allowed = |confined: &Classes, values: &BTreeSet<u32>, choices: &BTreeSet<Register>| -> bool {
        values.iter().all(|value| match confined.get(value) {
            Some(had) => had.intersection(choices).next().is_some(),
            None => !choices.is_empty(),
        })
    };

    let restrict = |confined: &mut Classes, values: &BTreeSet<u32>, choices: &BTreeSet<Register>| {
        for value in values {
            _restrict(confined, *value, choices);
        }
    };

    while let Some(&seed) = unseen.iter().next() {
        let mut colors: IndexMap<u32, u8> = IndexMap::from_iter([(seed, 0)]);
        let mut work = vec![seed];
        let mut bipartite = true;
        while let Some(value) = work.pop() {
            for other in &adjacent[&value] {
                match colors.get(other) {
                    None => {
                        let color = 1 - colors[&value];
                        colors.insert(*other, color);
                        work.push(*other);
                    }
                    Some(color) if *color == colors[&value] => bipartite = false,
                    Some(_) => {}
                }
            }
        }
        let component: BTreeSet<u32> = colors.keys().copied().collect();
        for value in &component {
            unseen.remove(value);
        }
        let source_spelling = |confined: &mut Classes| {
            for (base, index) in pairs {
                if component.contains(base) {
                    restrict(confined, &BTreeSet::from([*base]), &target::WORD_BASES);
                    restrict(confined, &BTreeSet::from([*index]), &target::WORD_INDEXES);
                }
            }
        };
        if !bipartite {
            source_spelling(confined);
            continue;
        }
        let sides: (BTreeSet<u32>, BTreeSet<u32>) = (
            colors.iter().filter(|(_value, color)| **color == 0).map(|(value, _)| *value).collect(),
            colors.iter().filter(|(_value, color)| **color != 0).map(|(value, _)| *value).collect(),
        );
        let options: Vec<(&BTreeSet<u32>, &BTreeSet<u32>)> = [(&sides.0, &sides.1), (&sides.1, &sides.0)]
            .into_iter()
            .filter(|(left, right)| {
                allowed(confined, left, &target::WORD_BASES) && allowed(confined, right, &target::WORD_INDEXES)
            })
            .collect();
        if options.is_empty() {
            source_spelling(confined);
            continue;
        }
        let key = |option: &(&BTreeSet<u32>, &BTreeSet<u32>)| {
            (
                base_penalty(option.0),
                option.0.intersection(prefer_indexes).count(),
                option.0.len(),
                option.0.iter().copied().collect::<Vec<u32>>(),
            )
        };
        // `min` keeps the first of equal keys.
        let mut best = options[0];
        for option in &options[1..] {
            if key(option) < key(&best) {
                best = *option;
            }
        }
        let (bases, indexes) = (best.0.clone(), best.1.clone());
        restrict(confined, &bases, &target::WORD_BASES);
        restrict(confined, &indexes, &target::WORD_INDEXES);
    }
}

pub(crate) const _SEGMENT_OPERANDS: [Operation; 3] = [Operation::Move, Operation::Push, Operation::Pop];


/// A point of a body at which the values live cannot all sit in registers their classes allow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Violation {
    pub block: i64,
    pub position: usize,
    pub file: &'static str,
    pub why: Why,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Why {
    /// More values live than the file has registers.
    Crowded { live: usize, registers: usize },
    /// Values an instruction acts on, which no assignment gives distinct registers of their classes.
    Unmatched(Vec<u32>),
}

/// Whether every value of `wanted` can take a distinct register of its own set: a bipartite matching.
fn matched(wanted: &[(u32, BTreeSet<Register>)]) -> bool {
    fn place(at: usize, wanted: &[(u32, BTreeSet<Register>)], taken: &mut IndexMap<Register, usize>, seen: &mut BTreeSet<Register>) -> bool {
        for register in &wanted[at].1 {
            if !seen.insert(*register) {
                continue;
            }
            let holder = taken.get(register).copied();
            if holder.is_none_or(|other| place(other, wanted, taken, seen)) {
                taken.insert(*register, at);
                return true;
            }
        }
        false
    }
    let mut taken: IndexMap<Register, usize> = IndexMap::default();
    (0..wanted.len()).all(|at| place(at, wanted, &mut taken, &mut BTreeSet::new()))
}

/// The points of `body` at which the values live do not fit the registers their classes allow, per register file:
/// the general registers and the segment registers. A body with none can be coloured, with copies where a value
/// waits in another class's register; one with some cannot, whatever the allocator does. `skip` names the values
/// another pass places (x87, pinned, inputs).
pub fn violations(body: &LirBody, segments: &Segments, skip: &BTreeSet<u32>) -> Vec<Violation> {
    let confined = classes(body, &BTreeSet::new(), segments);
    let (_, live_out) = crate::backend::allocate::live(body);
    let general: BTreeSet<Register> = target::AVAILABLE.iter().map(|one| crate::backend::allocate::_whole(*one)).collect();
    let selectors: BTreeSet<Register> = segments.selectors.iter().copied().collect();
    let files: [(&'static str, &BTreeSet<Register>); 2] = [("general", &general), ("selector", &selectors)];
    let in_file = |value: u32, file: &BTreeSet<Register>, general_file: bool| match confined.get(&value) {
        Some(class) => class.iter().any(|one| file.contains(&crate::backend::allocate::_whole(*one))),
        None => general_file,
    };
    let class_in = |value: u32, file: &BTreeSet<Register>| -> BTreeSet<Register> {
        match confined.get(&value) {
            Some(class) => class.iter().map(|one| crate::backend::allocate::_whole(*one)).filter(|one| file.contains(one)).collect(),
            None => file.clone(),
        }
    };
    let mut out = Vec::new();
    for block in &body.blocks {
        let mut live: BTreeSet<u32> = live_out[&block.at].iter().copied().filter(|value| !skip.contains(value)).collect();
        for (position, one) in block.insns.iter().enumerate().rev() {
            // Two states per instruction: after it (what is live, its results among them) and before it (what it reads
            // is live, its results not yet): a result takes the register of an operand that dies.
            // The copies of one phi edge are one parallel copy: a point at its ends, not between its copies.
            let grouped = |other: Option<&std::sync::Arc<crate::model::lir::Insn>>| one.group.is_some() && other.is_some_and(|other| other.group == one.group);
            let (inside_after, inside_before) = (grouped(block.insns.get(position + 1)), grouped(position.checked_sub(1).and_then(|at| block.insns.get(at))));
            let after = live.clone();
            let mut before = live.clone();
            for value in &one.defines {
                before.remove(value);
            }
            before.extend(one.uses.iter().copied().filter(|value| !skip.contains(value)));
            for (state, acting_values, inside) in [(&after, &one.defines, inside_after), (&before, &one.uses, inside_before)] {
                if inside {
                    continue;
                }
                for (name, file) in files {
                    let members: Vec<u32> = state.iter().copied().filter(|value| in_file(*value, file, name == "general")).collect();
                    if members.len() > file.len() {
                        out.push(Violation { block: block.at, position, file: name, why: Why::Crowded { live: members.len(), registers: file.len() } });
                        continue;
                    }
                    let acting: Vec<(u32, BTreeSet<Register>)> = acting_values
                        .iter()
                        .copied()
                        .filter(|value| members.contains(value))
                        .collect::<BTreeSet<u32>>()
                        .into_iter()
                        .map(|value| (value, class_in(value, file)))
                        .collect();
                    if !matched(&acting) {
                        out.push(Violation { block: block.at, position, file: name, why: Why::Unmatched(acting.iter().map(|(value, _)| *value).collect()) });
                    }
                }
            }
            live = before;
        }
    }
    out
}
