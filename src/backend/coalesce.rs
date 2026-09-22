//! Port of `qbopt/backend/coalesce.py`: a copy whose two values can share a
//! register is not a copy.
//!
//! LLVM's `RegisterCoalescer`, conservative: Briggs and George before the
//! join, since there is no undo.

use std::collections::BTreeSet;
use std::sync::Arc;

use iced_x86::Register;
use crate::support::hash::IndexMap;

use crate::analysis::intervals::{self as ranges, Interval, Segment};
use crate::backend::{allocate, target};
use crate::model::ir::{self, Held, Loc, Operation, Semantics};
use crate::model::lir::{self, Insn, LirBlock, LirBody, Phi};
use crate::model::passes::LIRTransform;

pub struct Coalescer {
    pub pinned: IndexMap<u32, Register>,
}

impl Coalescer {
    pub const NAME: &'static str = "coalesce";

    pub fn new(pinned: Option<&IndexMap<u32, Register>>) -> Self {
        Self { pinned: pinned.cloned().unwrap_or_default() }
    }
}

impl LIRTransform for Coalescer {
    fn class_name(&self) -> &'static str {
        "Coalescer"
    }

    fn name(&self) -> &str {
        Self::NAME
    }

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        Ok(joined(&body, Some(&self.pinned)))
    }
}

type Graph = IndexMap<u32, BTreeSet<u32>>;

fn _find(parent: &mut IndexMap<u32, u32>, one: u32) -> u32 {
    let mut root = one;
    while parent.get(&root).copied().unwrap_or(root) != root {
        root = parent[&root];
    }
    let mut one = one;
    while parent.get(&one).copied().unwrap_or(one) != one {
        let next = parent[&one];
        parent.insert(one, root);
        one = next;
    }
    root
}

/// `body` with every copy this can prove unnecessary removed.
pub fn joined(body: &LirBody, pinned: Option<&IndexMap<u32, Register>>) -> LirBody {
    let mut every = body.pins.clone();
    every.extend(pinned.into_iter().flatten().map(|(value, register)| (*value, *register)));
    let pinned: IndexMap<u32, Register> =
        every.into_iter().map(|(value, register)| (value, ir::root(register))).collect();

    let index = ranges::indexed(body);
    let mut live = ranges::intervals(body, Some(&index));
    let masks = allocate::_masks(body, &index);
    let mut widths = allocate::_widest(body);
    let where_of = allocate::classes(body, &BTreeSet::new());
    let everything: BTreeSet<Register> = target::AVAILABLE.into_iter().collect();
    let mut may: IndexMap<u32, BTreeSet<Register>> = live
        .keys()
        .map(|one| (*one, target::order(where_of.get(one)).into_iter().collect()))
        .collect();
    for (value, register) in &pinned {
        if !everything.contains(register) && !where_of.contains_key(value) {
            may.insert(*value, BTreeSet::from([*register]));
        }
    }
    let mut held: IndexMap<u32, Register> = pinned.clone();
    let mut near = _interference(body);
    let mut parent: IndexMap<u32, u32> = IndexMap::default();
    let empty = BTreeSet::new();

    for block in &body.blocks {
        for one in &block.insns {
            let Some(pair) = _copy(one) else {
                continue;
            };
            let (mut here, mut there) = (_find(&mut parent, pair.0), _find(&mut parent, pair.1));
            if here == there {
                continue;
            }
            // Two pinned to different registers are two registers.
            let (mine_pin, theirs_pin) = (held.get(&here).copied(), held.get(&there).copied());
            if let (Some(mine), Some(theirs)) = (mine_pin, theirs_pin) {
                if mine != theirs {
                    continue;
                }
            }
            let (Some(mine), Some(theirs)) = (live.get(&here), live.get(&there)) else {
                continue;
            };
            if near.get(&here).is_some_and(|found| found.contains(&there)) {
                continue;
            }
            let allowed: BTreeSet<Register> = may
                .get(&here)
                .unwrap_or(&everything)
                .intersection(may.get(&there).unwrap_or(&everything))
                .copied()
                .collect();
            if allowed.is_empty() {
                continue;
            }
            let merged = _merged(mine, theirs);
            let width = widths
                .get(&here)
                .copied()
                .unwrap_or(0)
                .max(widths.get(&there).copied().unwrap_or(0))
                .max(1);
            let allowed: BTreeSet<Register> = allowed
                .into_iter()
                .filter(|register| !allocate::_clobbered(&merged, *register, &masks, width))
                .collect();
            if allowed.is_empty() {
                continue;
            }
            if [mine_pin, theirs_pin].into_iter().flatten().any(|pin| !allowed.contains(&pin)) {
                continue;
            }
            let mut neighbours: BTreeSet<u32> = near
                .get(&here)
                .unwrap_or(&empty)
                .union(near.get(&there).unwrap_or(&empty))
                .copied()
                .filter(|one| *one != here && *one != there)
                .collect();
            let k = allowed.len();
            let constrained = neighbours.iter().chain([&here, &there]).any(|value| held.contains_key(value));
            let significant = neighbours
                .iter()
                .filter(|o| {
                    let palette = may.get(*o).unwrap_or(&everything);
                    palette.intersection(&allowed).next().is_some()
                        && near.get(*o).map_or(0, BTreeSet::len) >= if constrained { k } else { palette.len() }
                })
                .count();
            if significant >= k
                && (held.contains_key(&here)
                    || held.contains_key(&there)
                    || !(_george(here, there, &allowed, &near, &may, &held)
                        || _george(there, here, &allowed, &near, &may, &held)))
            {
                continue; // Briggs and George: the merged class would not be colourable
            }
            if mine_pin.is_some() && theirs_pin.is_none() {
                std::mem::swap(&mut here, &mut there);
            }
            parent.insert(here, there);
            live.insert(there, merged);
            live.shift_remove(&here);
            may.insert(there, allowed);
            may.shift_remove(&here);
            widths.insert(there, width);
            widths.shift_remove(&here);
            if mine_pin.is_some() || theirs_pin.is_some() {
                held.insert(there, mine_pin.or(theirs_pin).expect("one is pinned"));
            }
            held.shift_remove(&here);
            for other in near.shift_remove(&here).unwrap_or_default() {
                if let Some(found) = near.get_mut(&other) {
                    found.remove(&here);
                }
                if other != there {
                    near.entry(other).or_default().insert(there);
                    neighbours.insert(other);
                }
            }
            near.insert(there, neighbours);
        }
    }

    // No early return where nothing joined: `_kept` also removes what was
    // already an identity.
    let mut swap: IndexMap<u32, u32> = IndexMap::default();
    for block in &body.blocks {
        for insn in &block.insns {
            for one in insn.defines.iter().chain(&insn.uses) {
                let found = _find(&mut parent, *one);
                swap.insert(*one, found);
            }
        }
    }
    let keys: Vec<u32> = parent.keys().copied().collect();
    for one in keys {
        let found = _find(&mut parent, one);
        swap.insert(one, found);
    }
    let swap = |value: u32| swap.get(&value).copied().unwrap_or(value);
    LirBody {
        inputs: body.inputs.iter().map(|value| swap(*value)).collect(),
        blocks: body
            .blocks
            .iter()
            .map(|block| LirBlock {
                insns: _kept(block, &swap),
                phis: block
                    .phis
                    .iter()
                    .map(|phi| Phi {
                        result: swap(phi.result),
                        incoming: phi.incoming.iter().map(|(at, value)| (*at, swap(*value))).collect(),
                    })
                    .collect(),
                ..block.clone()
            })
            .collect(),
        ..body.clone()
    }
}

/// Whether `gone` can join `kept` without making `kept` harder to colour.
fn _george(
    gone: u32,
    kept: u32,
    allowed: &BTreeSet<Register>,
    near: &Graph,
    may: &IndexMap<u32, BTreeSet<Register>>,
    held: &IndexMap<u32, Register>,
) -> bool {
    let everything: BTreeSet<Register> = target::AVAILABLE.into_iter().collect();
    if allowed != may.get(&kept).unwrap_or(&everything) {
        return false;
    }
    let empty = BTreeSet::new();
    near.get(&gone).unwrap_or(&empty).iter().filter(|other| **other != gone && **other != kept).all(|other| {
        let palette = may.get(other).unwrap_or(&everything);
        near.get(&kept).is_some_and(|found| found.contains(other))
            || palette.intersection(allowed).next().is_none()
            || (!held.contains_key(other) && near.get(other).map_or(0, BTreeSet::len) < palette.len())
    })
}

pub fn _interference(body: &LirBody) -> Graph {
    let (incoming, outgoing) = allocate::live(body);
    let mut widths: IndexMap<u32, u32> = IndexMap::default();
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        let mut held: Vec<Held> = match &one.what {
            Some(what) => what.dests.iter().chain(&what.sources).flat_map(ir::values).collect(),
            None => Vec::new(),
        };
        held.extend(one.requires.iter().chain(&one.delivers).map(|(value, _)| *value));
        for value in held {
            let had = widths.get(&value.value).copied().unwrap_or(0);
            widths.insert(value.value, had.max(value.width));
        }
        for (value, width) in &one.widths {
            let had = widths.get(value).copied().unwrap_or(0);
            widths.insert(*value, had.max(*width));
        }
    }
    let mut graph: Graph = IndexMap::default();

    let edge = |graph: &mut Graph, one: u32, other: u32| {
        if one != other {
            graph.entry(one).or_default().insert(other);
            graph.entry(other).or_default().insert(one);
        }
    };
    let all_pairs = |graph: &mut Graph, alive: &BTreeSet<u32>| {
        for value in alive {
            for other in alive {
                edge(graph, *value, *other);
            }
        }
    };

    let targets: BTreeSet<i64> = body.blocks.iter().flat_map(|block| block.succ.iter().copied()).collect();
    let mut entries: BTreeSet<i64> = BTreeSet::from([body.entry]);
    entries.extend(body.blocks.iter().map(|block| block.at).filter(|at| !targets.contains(at)));
    for block in &body.blocks {
        if entries.contains(&block.at) {
            all_pairs(&mut graph, &incoming[&block.at]);
        }
        let mut alive = outgoing[&block.at].clone();
        let mut index = block.insns.len() as i64 - 1;
        while index >= 0 {
            let one = &block.insns[index as usize];
            if one.group.is_some() {
                let mut first = index as usize;
                while first > 0 && block.insns[first - 1].group == one.group {
                    first -= 1;
                }
                let group = &block.insns[first..=index as usize];
                // The values live after a parallel copy all coexist.
                all_pairs(&mut graph, &alive);
                // Every source is live before any destination is written.
                let mut before = alive.clone();
                for item in group {
                    for value in &item.defines {
                        before.remove(value);
                    }
                }
                for item in group {
                    before.extend(item.uses.iter().copied());
                }
                all_pairs(&mut graph, &before);
                for item in group {
                    for value in &item.defines {
                        graph.entry(*value).or_default();
                    }
                }
                alive = before;
                index = first as i64 - 1;
                continue;
            }
            let copy = _copy(one);
            let mut equal = None;
            if let Some(copy) = copy {
                if one.defines == [copy.0] && one.uses == [copy.1] {
                    let what = one.what.as_ref().expect("a copy has semantics");
                    let (Loc::Held(into), Loc::Held(source)) = (&what.dests[0], &what.sources[0]) else {
                        unreachable!("a copy is between two values")
                    };
                    let wide = |value: u32| widths.get(&value).copied();
                    if into.width == source.width
                        && Some(source.width) == wide(copy.0)
                        && wide(copy.0) == wide(copy.1)
                    {
                        equal = Some(copy.1);
                    }
                }
            }
            for value in &one.defines {
                for other in &alive {
                    if Some(*other) != equal {
                        edge(&mut graph, *value, *other);
                    }
                }
            }
            for value in &one.defines {
                alive.remove(value);
            }
            alive.extend(one.uses.iter().copied());
            index -= 1;
        }
        for value in block.arrives() {
            for other in &alive {
                edge(&mut graph, value, *other);
            }
        }
    }
    graph
}

/// One block's instructions, with a joined copy's bytes given away.
fn _kept(block: &LirBlock, swap: &dyn Fn(u32) -> u32) -> Vec<Arc<Insn>> {
    let identity = |one: &Arc<Insn>| _copy(one).is_some_and(|pair| pair.0 == pair.1);
    lir::without(&block.insns, identity, Some(|one: &Arc<Insn>| _renamed(one, swap)))
}

/// One interval covering both, which is what the joined value occupies.
pub fn _merged(one: &Interval, other: &Interval) -> Interval {
    let mut runs: Vec<Segment> = one.segments.iter().chain(&other.segments).copied().collect();
    runs.sort_by_key(|x| (x.start, x.end));
    let mut out = vec![runs[0]];
    for seg in &runs[1..] {
        let last = out.last_mut().expect("seeded with the first");
        if seg.start <= last.end {
            *last = Segment { start: last.start, end: last.end.max(seg.end) };
            continue;
        }
        out.push(*seg);
    }
    let weight = if other.weight > one.weight { other.weight } else { one.weight };
    Interval { value: one.value, segments: out, weight }
}

/// The (written, read) pair this instruction is a plain move of.
pub fn _copy(one: &Insn) -> Option<(u32, u32)> {
    let what = one.what.as_ref()?;
    if what.op != Operation::Move {
        return None;
    }
    if what.dests.len() != 1 || what.sources.len() != 1 {
        return None;
    }
    match (&what.dests[0], &what.sources[0]) {
        (Loc::Held(into), Loc::Held(out_of)) => Some((into.value, out_of.value)),
        _ => None,
    }
}

/// A requirement, naming the value that survived the join.
fn _wants(side: &[(Held, Register)], swap: &dyn Fn(u32) -> u32) -> Vec<(Held, Register)> {
    side.iter().map(|(held, r)| (Held { value: swap(held.value), width: held.width }, *r)).collect()
}

/// One instruction with every joined value naming its survivor.
fn _renamed(one: &Arc<Insn>, swap: &dyn Fn(u32) -> u32) -> Arc<Insn> {
    let mut made = (**one).clone();
    if let Some(what) = &one.what {
        made.what = Some(Semantics {
            dests: what.dests.iter().map(|x| _settled(x, swap)).collect(),
            sources: what.sources.iter().map(|x| _settled(x, swap)).collect(),
            ..what.clone()
        });
    }
    made.defines = one.defines.iter().map(|v| swap(*v)).collect();
    made.uses = one.uses.iter().map(|v| swap(*v)).collect();
    made.requires = _wants(&one.requires, swap);
    made.delivers = _wants(&one.delivers, swap);
    made.widths = one.widths.iter().map(|(v, w)| (swap(*v), *w)).collect();
    Arc::new(made)
}

fn _settled(place: &Loc, swap: &dyn Fn(u32) -> u32) -> Loc {
    ir::mapped(place, |value| Held { value: swap(value.value), width: value.width })
}

#[cfg(test)]
mod tests {
    //! Port of `tests/test_coalesce.py`.

    use std::collections::BTreeSet;
    use std::sync::Arc;

    use iced_x86::Register;
    use crate::support::hash::IndexMap;

    use super::{_interference, _merged, joined};
    use crate::analysis::intervals;
    use crate::backend::{allocate, cpu::ProfileOrName, select, target};
    use crate::model::ir::{self, Held, Imm, Loc, Mem, Operation, Reg, Semantics, St};
    use crate::model::lir::{Insn, LirBlock, LirBody};

    fn semantics(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
        Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
    }

    fn held(value: u32, width: u32) -> Loc {
        Loc::Held(Held { value, width })
    }

    fn _move(at: i64, into: u32, out_of: u32) -> Insn {
        let what = semantics(Operation::Move, "mov", vec![held(into, 2)], vec![held(out_of, 2)]);
        Insn::new(at, Some((at, at + 2)), Some(what), vec![into], vec![out_of])
    }

    fn _use(at: i64, reads: u32) -> Insn {
        let what = semantics(Operation::Push, "push", vec![], vec![held(reads, 2)]);
        Insn::new(at, Some((at, at + 1)), Some(what), vec![], vec![reads])
    }

    fn _define(at: i64, makes: u32) -> Insn {
        let what = semantics(
            Operation::Move,
            "mov",
            vec![held(makes, 2)],
            vec![Loc::Imm(Imm { value: i64::from(makes), width: 2, address: None })],
        );
        Insn::new(at, Some((at, at + 3)), Some(what), vec![makes], vec![])
    }

    fn _jump(at: i64, to: i64) -> Insn {
        let what = Semantics { name: Some("jmp".to_owned()), target: Some(to), ..Semantics::new(Operation::Jump) };
        Insn::new(at, Some((at, at + 2)), Some(what), vec![], vec![])
    }

    fn body(name: &str, insns: Vec<Insn>, pins: &[(u32, Register)]) -> LirBody {
        LirBody::new(
            name,
            0,
            vec![LirBlock::new(0, insns.into_iter().map(Arc::new).collect())],
            IndexMap::default(),
            pins.iter().copied().collect(),
        )
    }

    fn grouped(one: Insn, group: i64) -> Insn {
        Insn { group: Some(group), ..one }
    }

    fn allocated(body: &LirBody, pins: &IndexMap<u32, Register>) -> allocate::Assignment {
        allocate::allocate(body, Some(pins), None, None, None, ProfileOrName::Name("386")).expect("allocates")
    }

    /// HARR's hoisted selector copy became unencodable mov es,es across a coverage gap.
    #[test]
    fn test_retained_resource_identity_has_a_legal_encoding() {
        let body = body("resource-copy", vec![_move(3, 1, 1)], &[(1, Register::ES)]);
        let result = allocate::applied(&body, &allocated(&body, &body.pins)).expect("applies");
        let insns = result.insns();
        assert_eq!(insns.len(), 1);
        assert_eq!(insns[0].covers, body.insns()[0].covers);
        let emitted = select::emit(insns[0].what.as_ref().expect("semantics"), 3, None, false, false, None);
        assert!(emitted.is_some());
        assert!(emitted.unwrap().code.is_empty());
    }

    // ---------------------------------------------------- tests/test_flow.py

    /// lngmix joins v3 with v9 and then, through the rename, v9 with v20.
    #[test]
    fn test_the_coalescer_joins_the_intervals_it_merges() {
        let segment = |start, end| intervals::Segment { start, end };
        let one = intervals::Interval::new(1, vec![segment(15, 16), segment(59, 60)]);
        let other = intervals::Interval::new(2, vec![segment(0, 15)]);
        assert!(!one.overlaps(&other), "these abut and must not read as overlapping");

        let both = _merged(&one, &other);
        assert_eq!(both.segments, [segment(0, 16), segment(59, 60)]);
        // And a third value inside the union is now correctly refused.
        let third = intervals::Interval::new(3, vec![segment(4, 9)]);
        assert!(!one.overlaps(&third), "the original said nothing about this range");
        assert!(both.overlaps(&third), "the merged interval must cover what it swallowed");
    }

    #[test]
    fn test_equal_resource_values_coalesce_without_consuming_a_gpr() {
        let mut load = _define(0, 1);
        load.what = Some(semantics(
            Operation::Move,
            "mov",
            vec![held(1, 2)],
            vec![Loc::Mem(Mem { through: Register::BP, offset: 0, disp_width: 2, ..Mem::new(None, 2) })],
        ));
        let mut insns = vec![load];
        insns.extend((0..6).map(|index| _define(3 + index * 3, 10 + index as u32)));
        insns.extend([_move(21, 2, 1), _use(26, 1), _use(28, 2)]);
        insns.extend((0..6).map(|index| _use(30 + index * 3, 10 + index as u32)));
        let body = body("resources", insns, &[(1, Register::ES), (2, Register::ES)]);
        let done = joined(&body, None);
        assert_eq!(done.insns().len(), body.insns().len() - 1);
        let result = allocated(&done, &done.pins);
        assert!(result.spilled.is_empty());
        assert!(result.r#where.values().any(|register| *register == Register::ES));
        let wholes: BTreeSet<Register> = result.r#where.values().map(|register| allocate::_whole(*register)).collect();
        assert!(target::AVAILABLE.iter().all(|register| wholes.contains(register)));
    }

    #[test]
    fn test_resource_constraints_survive_coalescing() {
        for other in ["different_resource", "clobber"] {
            let mut insns = vec![_define(0, 1), _move(3, 2, 1), _use(6, 2)];
            let pins = [(1, Register::ES), (2, if other == "different_resource" { Register::FS } else { Register::ES })];
            if other == "clobber" {
                let mut call =
                    Insn::new(5, Some((5, 5)), Some(semantics(Operation::Call, "call", vec![], vec![])), vec![], vec![]);
                call.clobbers = BTreeSet::from([Register::ES]);
                insns.insert(2, call);
            }
            let body = body("resource-safety", insns, &pins);
            let done = joined(&body, None);
            assert_eq!(done.insns().len(), body.insns().len(), "{other}");
        }
    }

    #[test]
    fn test_a_copy_can_share_a_register_while_its_equal_source_is_still_read() {
        let body = body("equal", vec![_define(0, 1), _move(3, 2, 1), _use(5, 1), _use(6, 2)], &[]);
        let done = joined(&body, None);
        let insns = done.insns();
        assert_eq!(insns.len(), 3);
        assert_eq!(insns[2].uses, insns[1].uses);
    }

    #[test]
    fn test_a_source_redefined_while_its_copy_is_live_cannot_share() {
        let insns = vec![_define(0, 1), _move(3, 2, 1), _define(5, 1), _use(8, 1), _use(9, 2)];
        let count = insns.len();
        let done = joined(&body("different", insns, &[]), None);
        let insns = done.insns();
        assert_eq!(insns.len(), count);
        assert_ne!(insns[count - 1].uses, insns[count - 2].uses);
    }

    #[test]
    fn test_parallel_copy_sources_interfere_before_any_destination_is_written() {
        let insns = vec![
            _define(0, 1),
            _define(1, 3),
            _move(3, 2, 1),
            _define(5, 2),
            grouped(_move(8, 1, 2), 1),
            grouped(_move(8, 3, 1), 1),
            _use(9, 1),
            _use(10, 3),
        ];
        let body = body("parallel-sources", insns, &[]);
        assert!(_interference(&body)[&1].contains(&2));
        let live = intervals::intervals(&body, None);
        assert!(live[&1].overlaps(&live[&2]));
    }

    #[test]
    fn test_parallel_copy_destinations_interfere_after_all_are_written() {
        let insns =
            vec![_define(0, 1), grouped(_move(1, 2, 1), 1), grouped(_move(1, 3, 1), 1), _use(4, 2), _use(5, 3)];
        let body = body("parallel-destinations", insns, &[]);
        assert!(_interference(&body)[&2].contains(&3));
        let done = joined(&body, None);
        let insns = done.insns();
        assert_ne!(insns[insns.len() - 2].uses, insns[insns.len() - 1].uses);
    }

    #[test]
    fn test_different_entry_values_cannot_share_even_if_copied_later() {
        let insns = vec![_use(0, 1), _use(1, 2), _move(2, 2, 1), _use(4, 2)];
        let count = insns.len();
        assert_eq!(joined(&body("inputs", insns, &[]), None).insns().len(), count);
    }

    #[test]
    fn test_a_narrow_copy_is_not_equality_of_a_wide_source() {
        let mut wide = _use(5, 1);
        wide.what = Some(semantics(Operation::Push, "push", vec![], vec![held(1, 4)]));
        let insns = vec![_define(0, 1), _move(3, 2, 1), wide, _use(6, 2)];
        let count = insns.len();
        assert_eq!(joined(&body("partial", insns, &[]), None).insns().len(), count);
    }

    #[test]
    fn test_a_coalesced_address_keeps_its_memory_operand_defined() {
        let memory = Mem { base: Some(Held { value: 2, width: 2 }), ..Mem::new(None, 2) };
        let load = Insn::new(
            5,
            Some((5, 7)),
            Some(semantics(Operation::Move, "mov", vec![held(3, 2)], vec![Loc::Mem(memory)])),
            vec![3],
            vec![2],
        );
        let insns = vec![_define(0, 1), _move(3, 2, 1), load, _use(7, 1), _use(8, 3)];
        let done = joined(&body("address", insns, &[]), None);
        let made: BTreeSet<u32> = done.insns().iter().flat_map(|one| one.defines.clone()).collect();
        for one in done.insns() {
            let what = one.what.as_ref().expect("semantics");
            for operand in what.dests.iter().chain(&what.sources) {
                assert!(ir::values(operand).iter().all(|value| made.contains(&value.value)));
            }
        }
    }

    #[test]
    fn test_pinned_return_cannot_absorb_an_incompatible_address_class() {
        let load = Insn::new(
            5,
            Some((5, 7)),
            Some(semantics(
                Operation::FloatLoad,
                "fld",
                vec![Loc::St(St { index: 0 })],
                vec![Loc::Mem(Mem { base: Some(Held { value: 2, width: 2 }), ..Mem::new(None, 8) })],
            )),
            vec![],
            vec![2],
        );
        let body = body("pointer", vec![_define(0, 1), _move(3, 2, 1), load], &[]);
        let pins: IndexMap<u32, Register> = IndexMap::from_iter([(1, Register::EAX)]);
        assert_eq!(joined(&body, Some(&pins)).insns().len(), 3);
        let pinned = LirBody { pins: pins.clone(), ..body };
        assert_eq!(joined(&pinned, None).insns().len(), 3);
    }

    #[test]
    fn test_coalescing_keeps_the_pinned_return_as_representative() {
        let body = body("return", vec![_define(0, 1), _move(3, 2, 1), _use(5, 2)], &[]);
        let done = joined(&body, Some(&IndexMap::from_iter([(2, Register::EAX)])));
        let insns = done.insns();
        assert_eq!(insns[0].defines, vec![2]);
        assert_eq!(insns[insns.len() - 1].uses, vec![2]);
        for register in [Register::EAX, Register::EBX, Register::ECX, Register::EDX] {
            let pins = IndexMap::from_iter([(2, register)]);
            let joined = joined(&body, Some(&pins));
            let emitted = allocate::applied(&joined, &allocated(&joined, &pins)).expect("applies");
            let insns = emitted.insns();
            assert_eq!(
                insns[insns.len() - 1].what.as_ref().expect("semantics").sources,
                vec![Loc::Reg(Reg { register: target::named(register, 2), width: 2 })]
            );
        }
    }

    #[test]
    fn test_two_copies_into_one_value_leave_every_read_defined() {
        let arm = |at: i64, value: u32| {
            let mut block = LirBlock::new(
                at,
                vec![_define(at, value), _move(at + 3, 2, value), _jump(at + 5, 0x20)].into_iter().map(Arc::new).collect(),
            );
            block.succ = vec![0x20];
            block
        };
        let last = LirBlock::new(0x20, vec![Arc::new(_use(0x20, 2))]);
        let body = LirBody::new("two arms", 0, vec![arm(0, 61), arm(0x10, 63), last], IndexMap::default(), IndexMap::default());
        let done = joined(&body, None);
        let made: BTreeSet<u32> = done.insns().iter().flat_map(|one| one.defines.clone()).collect();
        let read: BTreeSet<u32> = done.insns().iter().flat_map(|one| one.uses.clone()).collect();
        let missing: Vec<u32> = read.difference(&made).copied().collect();
        assert!(missing.is_empty(), "read with nothing defining it: {missing:?}");
    }

    #[test]
    fn test_a_pinned_neighbour_does_not_stop_georges_join() {
        for pinned in [false, true] {
            let long_lived: Vec<u32> = (10..16).collect();
            let mut insns: Vec<Insn> = long_lived.iter().enumerate().map(|(at, one)| _define(at as i64, *one)).collect();
            insns.extend([_define(0x10, 50), _define(0x13, 1), _move(0x16, 2, 1), _use(0x18, 2)]);
            insns.extend(long_lived.iter().chain([&50]).enumerate().map(|(at, one)| _use(0x20 + at as i64, *one)));
            let count = insns.len();
            let pins: Vec<(u32, Register)> = if pinned { vec![(50, Register::BX)] } else { vec![] };
            let body = body("counter", insns, &pins);
            assert_eq!(joined(&body, None).insns().len(), count - 1, "pinned={pinned}");
        }
    }
}
