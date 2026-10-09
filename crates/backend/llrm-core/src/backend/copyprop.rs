//! Port of `qbopt/backend/copyprop.py`: forward and remove physical register
//! copies proven on every CFG path.
//!
//! LLVM's MachineCopyPropagation tracks physical register units after
//! allocation; here a unit is one byte lane.  Undirected lane equality removes
//! redundant copies, while a second must-analysis retains the reaching copy's
//! direction for operand substitution.  Selection and decoded effects are both
//! checked again before a source is renamed.

use std::collections::{BTreeMap, BTreeSet};
use crate::support::hash::{HashMap, HashSet};
use std::sync::Arc;

use iced_x86::Register;
use crate::support::hash::IndexMap;

use crate::analysis::loops;
use crate::backend::peephole::{Lane, Lanes, _lanes, _register_effects, id};
use crate::backend::{select, target};
use crate::model::ir::{self, Loc, Operation, Reg, Semantics};
use crate::model::lir::{self, Insn, LirBlock, LirBody};

/// Python's `frozenset[tuple[Lane, Lane]]`: the copies that reach a point, by direction.
pub type Relations = BTreeSet<(Lane, Lane)>;

/// Which byte lanes hold the same value on every path to a point. Equality is an equivalence, so it is the partition of the
/// lanes, not the set of its pairs (800 for 40 lanes, copied, filtered and intersected for every instruction of every pass
/// of a fixed point): `class[i]` is the least lane of `i`'s class, so equal partitions compare equal. This is LLVM's
/// MachineCopyPropagation keeping a map per register unit; an instruction touches the units it writes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Equal {
    class: Vec<u16>,
}

impl Equal {
    /// Every lane equal to every other: the must-analysis top.
    pub fn top(lanes: usize) -> Self {
        Self { class: vec![0; lanes] }
    }

    /// No two lanes known equal.
    pub fn bottom(lanes: usize) -> Self {
        Self { class: (0..lanes as u16).collect() }
    }

    pub fn same(&self, left: usize, right: usize) -> bool {
        self.class[left] == self.class[right]
    }

    /// These lanes written: each is equal to no other.
    pub fn without(&self, written: &[usize]) -> Self {
        let mut out = self.clone();
        for &lane in written {
            let was = out.class[lane] as usize;
            if was == lane {
                // The class loses its least lane: the next is its name.
                if let Some(next) = (0..out.class.len()).find(|&other| other != lane && out.class[other] as usize == lane) {
                    for other in 0..out.class.len() {
                        if other != lane && out.class[other] as usize == lane {
                            out.class[other] = next as u16;
                        }
                    }
                }
            }
            out.class[lane] = lane as u16;
        }
        out
    }

    /// The lanes equal where their `source`s were: after a copy, a lane is what its source was.
    pub fn pulled_back(&self, source: &dyn Fn(usize) -> Source) -> Self {
        let mut names: Vec<(Source, u16)> = Vec::new();
        let class = (0..self.class.len())
            .map(|lane| {
                let key = match source(lane) {
                    Source::Lane(other) => Source::Class(self.class[other]),
                    other => other,
                };
                match names.iter().find(|(name, _)| *name == key) {
                    Some((_, least)) => *least,
                    None => {
                        names.push((key, lane as u16));
                        lane as u16
                    }
                }
            })
            .collect();
        Self { class }
    }

    /// Equal on both: the pairs in the intersection of the two sets.
    pub fn met(&self, other: &Self) -> Self {
        let mut names: Vec<((u16, u16), u16)> = Vec::new();
        let class = (0..self.class.len())
            .map(|lane| {
                let key = (self.class[lane], other.class[lane]);
                match names.iter().find(|(name, _)| *name == key) {
                    Some((_, least)) => *least,
                    None => {
                        names.push((key, lane as u16));
                        lane as u16
                    }
                }
            })
            .collect();
        Self { class }
    }
}

/// What a lane is after a copy, by where it was: a lane of the partition, a class of it already, or a lane it has no name for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Source {
    Lane(usize),
    Class(u16),
    Outside(Lane),
}

type Recipe = Option<(Lanes, Vec<(Lane, Lane)>)>;

pub fn forwarded(body: &LirBody) -> LirBody {
    if body.blocks.iter().any(|block| !block.phis.is_empty()) {
        return body.clone();
    }
    let lanes: Vec<Lane> = llrm_x86::registers::ROOTS
    .into_iter()
    .flat_map(_lanes)
    .collect::<Lanes>()
    .into_iter()
    .collect();
    let count = lanes.len();
    let slot: HashMap<Lane, usize> = lanes.iter().enumerate().map(|(index, lane)| (*lane, index)).collect();
    let mut register_for: IndexMap<Vec<Lane>, Register> = IndexMap::default();
    for register in target::WIDTHS.keys() {
        if !_lanes(*register).is_empty() && ir::root(*register) != Register::ESP {
            register_for.insert(_lanes(*register).into_iter().collect(), *register);
        }
    }
    let mut recipes: HashMap<usize, Recipe> = HashMap::default();
    for one in body.insns() {
        let what = one.what.as_ref();
        if what.is_some_and(|what| {
            [Operation::Branch, Operation::Jump].contains(&what.op)
                && what.target.is_some()
                && what.sources.is_empty()
                && what.dests.is_empty()
        }) && one.clobbers.is_empty()
            && one.requires.is_empty()
            && one.delivers.is_empty()
        {
            recipes.insert(id(&one), Some((Lanes::new(), Vec::new())));
            continue;
        }
        let Some(effects) = _register_effects(body.bits, &one, true, false) else {
            recipes.insert(id(&one), None);
            continue;
        };
        let mut copies = Vec::new();
        if let Some(what) = what {
            if let (Operation::Move, Some("mov"), [Loc::Reg(dest)], [Loc::Reg(source)]) =
                (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice())
            {
                let (destinations, sources): (Vec<Lane>, Vec<Lane>) =
                    (_lanes(dest.register).into_iter().collect(), _lanes(source.register).into_iter().collect());
                if destinations.len() == dest.width as usize
                    && dest.width == source.width
                    && source.width as usize == sources.len()
                {
                    copies = destinations.into_iter().zip(sources).collect();
                }
            }
        }
        recipes.insert(id(&one), Some((effects.1, copies)));
    }

    let equal = |facts: &Equal, left: Lane, right: Lane| -> bool {
        left == right || matches!((slot.get(&left), slot.get(&right)), (Some(left), Some(right)) if facts.same(*left, *right))
    };

    let after = |facts: &Equal, one: &Arc<Insn>| -> Equal {
        let Some((writes, copies)) = &recipes[&id(one)] else {
            return Equal::bottom(count);
        };
        if !copies.is_empty() {
            let sources: HashMap<Lane, Lane> = copies.iter().copied().collect();
            return facts.pulled_back(&|lane| {
                let source = sources.get(&lanes[lane]).copied().unwrap_or(lanes[lane]);
                match slot.get(&source) {
                    Some(index) => Source::Lane(*index),
                    None => Source::Outside(source),
                }
            });
        }
        if writes.is_empty() {
            return facts.clone();
        }
        let written: Vec<usize> = writes.iter().filter_map(|lane| slot.get(&lane).copied()).collect();
        facts.without(&written)
    };

    // Available copy sources, invalidated like LLVM's register units.
    //
    // Equality is undirected, but forwarding has a useful direction: for
    // `bx = ax`, AX is the older value and using it can make the copy
    // dead.  Keep that direction only while both byte lanes survive.  At a
    // join, the ordinary dataflow intersection below requires every path to
    // name the same source.
    let directed_after = |directed: &Relations, one: &Arc<Insn>| -> Relations {
        let Some((writes, copies)) = &recipes[&id(one)] else {
            return Relations::new();
        };
        let before: HashMap<Lane, Lane> = directed.iter().copied().collect();

        let oldest = |lane: Lane| -> Lane {
            let mut lane = lane;
            let mut seen: HashSet<Lane> = HashSet::default();
            while before.contains_key(&lane) && !seen.contains(&lane) {
                seen.insert(lane);
                lane = before[&lane];
            }
            lane
        };

        // Resolve sources before killing the destination: a reverse copy such
        // as AX=BX; BX=AX still reads AX's old value even though BX is written.
        let sources: HashMap<Lane, Lane> = copies.iter().map(|(dest, source)| (*dest, oldest(*source))).collect();
        let mut out: BTreeMap<Lane, Lane> = before
            .iter()
            .filter(|(dest, source)| !writes.contains(dest) && !writes.contains(source))
            .map(|(dest, source)| (*dest, *source))
            .collect();
        for (dest, source) in copies {
            let mut origin = sources[dest];
            if writes.contains(&origin) {
                origin = *source;
            }
            if *dest != origin {
                out.insert(*dest, origin);
            }
        }
        out.into_iter().collect()
    };

    // Use the reaching copy's source in explicit, independently encoded operands.
    let forward_use = |one: &Arc<Insn>, directed: &Relations, facts: &Equal| -> Arc<Insn> {
        let Some(what) = &one.what else {
            return Arc::clone(one);
        };
        if !one.clobbers.is_empty()
            || !one.clobbers_high.is_empty()
            || !one.requires.is_empty()
            || !one.delivers.is_empty()
            || !one.spread.is_empty()
            || one.group.is_some()
            || one.symbol == Some(true)
        {
            return Arc::clone(one);
        }
        let mapping: HashMap<Lane, Lane> = directed.iter().copied().collect();
        let mut changed: Semantics = what.clone();
        // `Mem` equality leaves the registers out (a cell is its address), so a rewritten one is told by this.
        let mut touched = false;
        // One register read, named `register` at `width` bytes: the older copy's, where that encodes and reads
        // the same lanes. `put` makes the semantics that reads `replacement` there.
        let substitute = |changed: &Semantics, register: Register, width: i64, put: &dyn Fn(Register) -> Semantics| -> Option<Semantics> {
            let source_lanes: Vec<Lane> = _lanes(register).into_iter().collect();
            let candidate_lanes: Vec<Lane> = source_lanes.iter().map(|lane| mapping.get(lane).copied().unwrap_or(*lane)).collect();
            let mut sorted_lanes = candidate_lanes.clone();
            sorted_lanes.sort();
            let candidate = register_for.get(&sorted_lanes).copied()?;
            if candidate == register || width != 0 && target::width_of(candidate) != Some(width) {
                return None;
            }
            if !source_lanes.iter().zip(&candidate_lanes).all(|(left, right)| equal(facts, *left, *right)) {
                return None;
            }
            let before_effects = _register_effects(body.bits, &Insn { what: Some(changed.clone()), ..(**one).clone() }, true, false)?;
            if candidate_lanes.iter().any(|lane| before_effects.1.contains(lane)) {
                return None;
            }
            let proposed = put(candidate);
            select::priced_in(body.bits, &proposed, 0, None, false, false, None)?;
            let after_effects = _register_effects(body.bits, &Insn { what: Some(proposed.clone()), ..(**one).clone() }, true, false)?;
            let expected_reads: Lanes =
                before_effects.0.iter().filter(|lane| !source_lanes.contains(lane)).copied().chain(candidate_lanes.iter().copied()).collect();
            (after_effects.1 == before_effects.1 && after_effects.0 == expected_reads).then_some(proposed)
        };
        for index in 0..changed.sources.len() {
            let Loc::Reg(source) = changed.sources[index] else {
                continue;
            };
            let current = changed.clone();
            let put = |replacement: Register| {
                let mut sources = current.sources.clone();
                sources[index] = Loc::Reg(Reg { register: replacement, width: source.width });
                Semantics { sources, ..current.clone() }
            };
            if let Some(proposed) = substitute(&current, source.register, 0, &put) {
                touched = true;
                changed = proposed;
            }
        }
        // The registers a cell or an address is made of: a copy's older register addresses the same.
        for (side, count) in [(true, changed.dests.len()), (false, changed.sources.len())] {
            for index in 0..count {
                for through in [true, false] {
                    let current = changed.clone();
                    let place = if side { &current.dests[index] } else { &current.sources[index] };
                    let register = match (place, through) {
                        (Loc::Mem(cell), true) => cell.through,
                        (Loc::Mem(cell), false) => cell.index_through,
                        (Loc::Address(address), true) => address.through,
                        (Loc::Address(address), false) => address.index,
                        _ => continue,
                    };
                    if register == Register::None {
                        continue;
                    }
                    let put = |replacement: Register| {
                        let renamed = match place {
                            Loc::Mem(cell) if through => Loc::Mem(ir::Mem { through: replacement, ..cell.clone() }),
                            Loc::Mem(cell) => Loc::Mem(ir::Mem { index_through: replacement, ..cell.clone() }),
                            Loc::Address(address) if through => Loc::Address(ir::Address { through: replacement, ..address.clone() }),
                            Loc::Address(address) => Loc::Address(ir::Address { index: replacement, ..address.clone() }),
                            other => other.clone(),
                        };
                        let (mut dests, mut sources) = (current.dests.clone(), current.sources.clone());
                        if side { dests[index] = renamed } else { sources[index] = renamed }
                        Semantics { dests, sources, ..current.clone() }
                    };
                    if let Some(proposed) = substitute(&current, register, target::width_of(register).unwrap_or(0), &put) {
                        touched = true;
                        changed = proposed;
                    }
                }
            }
        }
        if !touched { Arc::clone(one) } else { Arc::new(Insn { what: Some(changed), ..(**one).clone() }) }
    };

    let blocks: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut reachable: HashSet<i64> = HashSet::default();
    let mut pending = vec![body.entry];
    while let Some(at) = pending.pop() {
        if reachable.contains(&at) || !blocks.contains_key(&at) {
            continue;
        }
        reachable.insert(at);
        pending.extend(blocks[&at].succ.iter().copied());
    }
    // `loops.predecessors` reads only `at` and `succ`.
    let graph = &body.blocks;
    let predecessors = loops::predecessors(&graph);
    // Start at the must-analysis top, then intersect paths to a fixed point.
    // Entry contributes no equality, so a backedge cannot invent its own proof.
    let mut entries: HashMap<i64, Equal> = reachable.iter().map(|at| (*at, Equal::top(count))).collect();
    let mut exits = entries.clone();
    let mut directed_entries: HashMap<i64, Relations> = reachable.iter().map(|at| (*at, Relations::new())).collect();
    let mut directed_exits = directed_entries.clone();
    let mut changed = true;
    while changed {
        changed = false;
        for block in &body.blocks {
            if !reachable.contains(&block.at) {
                continue;
            }
            let parents: Vec<i64> =
                predecessors[&block.at].iter().copied().filter(|at| reachable.contains(at)).collect();
            let none = parents.is_empty() || block.at == body.entry;
            let incoming = if none {
                Equal::bottom(count)
            } else {
                parents[1..].iter().fold(exits[&parents[0]].clone(), |met, at| met.met(&exits[at]))
            };
            entries.insert(block.at, incoming.clone());
            let directed_incoming = if none {
                Relations::new()
            } else {
                let mut met = directed_exits[&parents[0]].clone();
                for at in &parents[1..] {
                    met = met.intersection(&directed_exits[at]).copied().collect();
                }
                met
            };
            directed_entries.insert(block.at, directed_incoming.clone());
            let mut facts = incoming;
            let mut directed = directed_incoming;
            for one in &block.insns {
                facts = after(&facts, one);
                directed = directed_after(&directed, one);
            }
            if exits[&block.at] != facts || directed_exits[&block.at] != directed {
                exits.insert(block.at, facts);
                directed_exits.insert(block.at, directed);
                changed = true;
            }
        }
    }

    let mut result = Vec::new();
    for block in &body.blocks {
        let mut facts = entries.get(&block.at).cloned().unwrap_or_else(|| Equal::bottom(count));
        let mut redundant: HashSet<usize> = HashSet::default();
        let mut directed = directed_entries.get(&block.at).cloned().unwrap_or_default();
        let mut rewritten = Vec::new();
        for one in &block.insns {
            let recipe = &recipes[&id(one)];
            if let Some((_, copies)) = recipe {
                if !copies.is_empty()
                    && reachable.contains(&block.at)
                    && one.requires.is_empty()
                    && one.delivers.is_empty()
                    && one.spread.is_empty()
                    && one.group.is_none()
                    && one.symbol != Some(true)
                    && copies.iter().all(|(left, right)| equal(&facts, *left, *right))
                {
                    redundant.insert(id(one));
                }
            }
            rewritten.push(forward_use(one, &directed, &facts));
            facts = after(&facts, one);
            directed = directed_after(&directed, one);
        }
        let insns = block
            .insns
            .iter()
            .zip(rewritten)
            .map(|(original, replacement)| {
                if redundant.contains(&id(original)) { lir::anchor(Arc::clone(original)) } else { replacement }
            })
            .collect();
        result.push(block.with_insns(insns));
    }
    body.with_blocks(result)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{Equal, Source};

    const LANES: usize = 12;

    /// What the sets of pairs did: the pairs of lanes known equal.
    fn pairs(equal: &Equal) -> BTreeSet<(usize, usize)> {
        (0..LANES).flat_map(|left| (left + 1..LANES).map(move |right| (left, right))).filter(|(left, right)| equal.same(*left, *right)).collect()
    }

    fn all() -> BTreeSet<(usize, usize)> {
        (0..LANES).flat_map(|left| (left + 1..LANES).map(move |right| (left, right))).collect()
    }

    fn holds(set: &BTreeSet<(usize, usize)>, left: usize, right: usize) -> bool {
        left == right || set.contains(&(left.min(right), left.max(right)))
    }

    /// Equal lanes were kept as the set of their pairs: 800 of them for the 40 lanes of a body, copied, filtered and
    /// intersected for every instruction of every round of a fixed point (x_switch -O2: 6% of its compile once `[r+r*s]`
    /// stopped flushing them). The partition does what the sets did, in the lanes an instruction writes.
    #[test]
    fn test_the_partition_of_equal_lanes_does_what_the_set_of_pairs_did() {
        let mut seed = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = move |limit: usize| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 33) as usize) % limit
        };
        for _round in 0..300 {
            let (mut equal, mut set) = (Equal::top(LANES), all());
            for _step in 0..12 {
                match next(3) {
                    0 => {
                        let written: Vec<usize> = (0..next(4)).map(|_| next(LANES)).collect();
                        equal = equal.without(&written);
                        set = set.into_iter().filter(|(left, right)| !written.contains(left) && !written.contains(right)).collect();
                    }
                    1 => {
                        // A copy: each lane named is what another was, the rest what they were.
                        let moved: Vec<(usize, usize)> = (0..1 + next(3)).map(|_| (next(LANES), next(LANES))).collect();
                        let mut map: Vec<usize> = (0..LANES).collect();
                        for (dest, source) in &moved {
                            map[*dest] = *source;
                        }
                        equal = equal.pulled_back(&|lane| Source::Lane(map[lane]));
                        set = all().into_iter().filter(|(left, right)| holds(&set, map[*left], map[*right])).collect();
                    }
                    _ => {
                        let other_written: Vec<usize> = (0..next(5)).map(|_| next(LANES)).collect();
                        let other = Equal::top(LANES).without(&other_written);
                        let other_set: BTreeSet<(usize, usize)> = all().into_iter().filter(|(left, right)| !other_written.contains(left) && !other_written.contains(right)).collect();
                        equal = equal.met(&other);
                        set = set.intersection(&other_set).copied().collect();
                    }
                }
                assert_eq!(pairs(&equal), set);
            }
        }
    }
}
