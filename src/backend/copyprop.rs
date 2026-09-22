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
use crate::model::mir::MirBlock;

/// Python's `frozenset[tuple[Lane, Lane]]`.
pub type Relations = BTreeSet<(Lane, Lane)>;

type Recipe = Option<(Lanes, Vec<(Lane, Lane)>)>;

pub fn forwarded(body: &LirBody) -> LirBody {
    if body.blocks.iter().any(|block| !block.phis.is_empty()) {
        return body.clone();
    }
    let lanes: Vec<Lane> = [
        Register::EAX,
        Register::EBX,
        Register::ECX,
        Register::EDX,
        Register::ESI,
        Register::EDI,
        Register::EBP,
    ]
    .into_iter()
    .flat_map(_lanes)
    .collect::<Lanes>()
    .into_iter()
    .collect();
    // `frozenset(combinations(lanes, 2))`.
    let mut universe = Relations::new();
    for (index, left) in lanes.iter().enumerate() {
        for right in &lanes[index + 1..] {
            universe.insert((*left, *right));
        }
    }
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
        let Some(effects) = _register_effects(&one, true, false) else {
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

    let equal = |facts: &Relations, left: Lane, right: Lane| -> bool {
        left == right || facts.contains(&if left <= right { (left, right) } else { (right, left) })
    };

    let after = |facts: &Relations, one: &Arc<Insn>| -> Relations {
        let Some((writes, copies)) = &recipes[&id(one)] else {
            return Relations::new();
        };
        if !copies.is_empty() {
            let sources: HashMap<Lane, Lane> = copies.iter().copied().collect();
            return universe
                .iter()
                .filter(|(left, right)| {
                    equal(
                        facts,
                        sources.get(left).copied().unwrap_or(*left),
                        sources.get(right).copied().unwrap_or(*right),
                    )
                })
                .copied()
                .collect();
        }
        if writes.is_empty() {
            return facts.clone();
        }
        facts.iter().filter(|(left, right)| !writes.contains(left) && !writes.contains(right)).copied().collect()
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
    let forward_use = |one: &Arc<Insn>, directed: &Relations, facts: &Relations| -> Arc<Insn> {
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
        for index in 0..changed.sources.len() {
            let Loc::Reg(source) = changed.sources[index] else {
                continue;
            };
            let source_lanes: Vec<Lane> = _lanes(source.register).into_iter().collect();
            let candidate_lanes: Vec<Lane> =
                source_lanes.iter().map(|lane| mapping.get(lane).copied().unwrap_or(*lane)).collect();
            let mut sorted_lanes = candidate_lanes.clone();
            sorted_lanes.sort();
            let Some(candidate) = register_for.get(&sorted_lanes).copied() else {
                continue;
            };
            if candidate == source.register {
                continue;
            }
            let replacement = Reg { register: candidate, width: source.width };
            if !source_lanes.iter().zip(&candidate_lanes).all(|(left, right)| equal(facts, *left, *right)) {
                continue;
            }
            let before_effects = _register_effects(&Insn { what: Some(changed.clone()), ..(**one).clone() }, true, false);
            let Some(before_effects) = before_effects else {
                continue;
            };
            if candidate_lanes.iter().any(|lane| before_effects.1.contains(lane)) {
                continue;
            }
            let mut sources = changed.sources.clone();
            sources[index] = Loc::Reg(replacement);
            let proposed = Semantics { sources, ..changed.clone() };
            if select::emit(&proposed, 0, None, false, false, None).is_none() {
                continue;
            }
            let after_effects =
                _register_effects(&Insn { what: Some(proposed.clone()), ..(**one).clone() }, true, false);
            let expected_reads: Lanes = before_effects
                .0
                .iter()
                .filter(|lane| !source_lanes.contains(lane))
                .copied()
                .chain(candidate_lanes.iter().copied())
                .collect();
            match after_effects {
                Some(after_effects) if after_effects.1 == before_effects.1 && after_effects.0 == expected_reads => {}
                _ => continue,
            }
            changed = proposed;
        }
        if changed == *what { Arc::clone(one) } else { Arc::new(Insn { what: Some(changed), ..(**one).clone() }) }
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
    let graph: Vec<MirBlock> =
        body.blocks.iter().map(|block| MirBlock::new(block.at, Vec::new(), Vec::new(), block.succ.clone())).collect();
    let predecessors = loops::predecessors(&graph);
    // Start at the must-analysis top, then intersect paths to a fixed point.
    // Entry contributes no equality, so a backedge cannot invent its own proof.
    let mut entries: HashMap<i64, Relations> = reachable.iter().map(|at| (*at, universe.clone())).collect();
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
            let meet = |map: &HashMap<i64, Relations>| -> Relations {
                if parents.is_empty() || block.at == body.entry {
                    return Relations::new();
                }
                let mut met = map[&parents[0]].clone();
                for at in &parents[1..] {
                    met = met.intersection(&map[at]).copied().collect();
                }
                met
            };
            let incoming = meet(&exits);
            entries.insert(block.at, incoming.clone());
            let directed_incoming = meet(&directed_exits);
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
        let mut facts = entries.get(&block.at).cloned().unwrap_or_default();
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
        result.push(LirBlock { insns, ..block.clone() });
    }
    LirBody { blocks: result, ..body.clone() }
}
