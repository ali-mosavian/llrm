//! Port of `qbopt/backend/splitkit.py`: splitting a live range instead of
//! spilling the whole of it.
//!
//! LLVM's `SplitKit`, driven by `RegAllocGreedy`'s split ladder: `_regional`
//! (`tryRegionSplit`), `_local` (`tryLocalSplit`), `_per_block`
//! (`tryBlockSplit`).

use std::collections::BTreeSet;
use std::sync::Arc;

use iced_x86::Register;
use crate::support::hash::IndexMap;

use crate::analysis::intervals::{self as ranges, Indexes, Interval, Segment};
use crate::analysis::loops;
use crate::backend::{allocate, spiller, target};
use crate::model::ir::{self, Held, Loc, Operation, Semantics};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::model::passes::LIRTransform;

pub struct Splitter {
    pub only: Option<BTreeSet<u32>>,
}

impl Splitter {
    pub const NAME: &'static str = "split";

    pub fn new(only: Option<BTreeSet<u32>>) -> Self {
        Self { only }
    }
}

impl LIRTransform for Splitter {
    fn class_name(&self) -> &'static str {
        "Splitter"
    }

    fn name(&self) -> &str {
        Self::NAME
    }

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        Ok(split(&body, self.only.as_ref(), None, None).unwrap_or(body))
    }
}

type Form = fn(&LirBody, u32, &IndexMap<u32, Interval>, &Indexes, &IndexMap<i64, u32>) -> Option<Region>;

/// `body` with one failing value's range cut, or None where `body` is
/// returned unchanged (Python returns the same object).
pub fn split(
    body: &LirBody,
    only: Option<&BTreeSet<u32>>,
    mut done: Option<&mut BTreeSet<u32>>,
    r#where: Option<&IndexMap<u32, Register>>,
) -> Option<LirBody> {
    let index = ranges::indexed(body);
    let live = ranges::intervals(body, Some(&index));
    let wanted: Vec<u32> = match only {
        Some(only) => only.iter().copied().collect(),
        None => {
            let mut every: Vec<u32> = live.keys().copied().collect();
            every.sort_unstable();
            every
        }
    };
    if wanted.is_empty() {
        return None;
    }
    let widths = _widths(body);
    let deep = ranges::depths(body);
    let mut order: Vec<u32> = wanted.into_iter().filter(|value| live.contains_key(value)).collect();
    order.sort_by_key(|value| (-live[value].size(), *value));
    let mut cut: Option<LirBody> = None;
    let confined = if r#where.is_some() { allocate::classes(body, &BTreeSet::new()) } else { IndexMap::default() };
    let forms: [Form; 3] = [_regional, _local, _per_block];
    for value in order {
        let Some(width) = widths.get(&value).copied() else {
            continue;
        };
        if done.as_ref().is_some_and(|done| done.contains(&value)) {
            continue;
        }
        for form in forms {
            let current = cut.as_ref().unwrap_or(body);
            let plan = form(current, value, &live, &index, &deep);
            let Some(plan) = plan.filter(|plan| _fits(value, plan, &live, &index, r#where, &confined)) else {
                continue;
            };
            if let Some(carved) = _carved(current, value, _next_value(current), width, &plan) {
                cut = Some(carved);
                if let Some(done) = done.as_mut() {
                    done.insert(value);
                }
                break;
            }
        }
    }
    cut
}

/// Carve spilled address bases into the natural loops that reuse them.
pub fn loop_bases(body: &LirBody, values: &BTreeSet<u32>) -> (LirBody, BTreeSet<u32>) {
    if values.is_empty() {
        return (body.clone(), BTreeSet::new());
    }
    let mut result = body.clone();
    let mut kept: BTreeSet<u32> = BTreeSet::new();
    for found in loops::loops(&ranges::_graph(&result.blocks), Some(result.entry)) {
        let inside = &found.body;
        let references: IndexMap<u32, IndexMap<i64, Vec<usize>>> =
            values.iter().map(|value| (*value, _references(&result, *value))).collect();
        let candidates: BTreeSet<u32> = references
            .iter()
            .filter(|(value, found)| {
                found.keys().any(|at| inside.contains(at))
                    && found.keys().any(|at| !inside.contains(at))
                    && result
                        .blocks
                        .iter()
                        .filter(|block| inside.contains(&block.at))
                        .flat_map(|block| &block.insns)
                        .filter_map(|one| one.what.as_ref())
                        .flat_map(|what| what.dests.iter().chain(&what.sources))
                        .any(|place| matches!(place, Loc::Mem(cell) if cell.base.is_some_and(|base| base.value == **value)))
            })
            .map(|(value, _found)| *value)
            .collect();
        for value in candidates {
            let widths = _widths(&result);
            let Some(width) = widths.get(&value).copied() else {
                continue;
            };
            let fresh = _next_value(&result);
            let Some(carved) = _carved(&result, value, fresh, width, &Region { blocks: inside.clone(), starts_at: None })
            else {
                continue;
            };
            result = carved;
            kept.insert(fresh);
        }
    }
    (result, kept)
}

/// Whether some register is free, in the allocation that failed, for the piece `region` carves.
fn _fits(
    value: u32,
    region: &Region,
    live: &IndexMap<u32, Interval>,
    index: &Indexes,
    r#where: Option<&IndexMap<u32, Register>>,
    confined: &allocate::Classes,
) -> bool {
    let Some(r#where) = r#where else {
        return true;
    };
    let Some(mine) = live.get(&value) else {
        return true;
    };
    let spans: Vec<Segment> = region
        .blocks
        .iter()
        .filter_map(|at| index.span.get(at))
        .map(|(start, end)| Segment { start: *start, end: *end })
        .collect();
    let mut piece: Vec<Segment> = mine
        .segments
        .iter()
        .flat_map(|one| {
            spans.iter().filter(|span| one.overlaps(span)).map(|span| Segment {
                start: one.start.max(span.start),
                end: one.end.min(span.end),
            })
        })
        .collect();
    piece.sort_by_key(|one| one.start);
    if piece.is_empty() {
        return false;
    }
    let carved = Interval::new(value, piece);
    let mut occupants: IndexMap<Register, Vec<u32>> = IndexMap::default();
    for (other, register) in r#where {
        if *other != value && live.contains_key(other) {
            occupants.entry(allocate::_whole(*register)).or_default().push(*other);
        }
    }
    target::order(confined.get(&value)).into_iter().any(|register| {
        !occupants
            .get(&allocate::_whole(register))
            .into_iter()
            .flatten()
            .any(|other| live[other].overlaps(&carved))
    })
}

/// Per block, the positions in it that name this value.
fn _references(body: &LirBody, value: u32) -> IndexMap<i64, Vec<usize>> {
    let mut out = IndexMap::default();
    for block in &body.blocks {
        let found: Vec<usize> = block
            .insns
            .iter()
            .enumerate()
            .filter(|(_position, one)| one.defines.contains(&value) || one.uses.contains(&value))
            .map(|(position, _one)| position)
            .collect();
        if !found.is_empty() {
            out.insert(block.at, found);
        }
    }
    out
}

/// `tryRegionSplit`: the deepest-nested blocks that use it, carved out.
fn _regional(
    body: &LirBody,
    value: u32,
    _live: &IndexMap<u32, Interval>,
    _index: &Indexes,
    deep: &IndexMap<i64, u32>,
) -> Option<Region> {
    let found = _references(body, value);
    if found.len() < 2 {
        return None;
    }
    let depths: IndexMap<i64, u32> = found.keys().map(|at| (*at, deep.get(at).copied().unwrap_or(0))).collect();
    let hottest = *depths.values().max().expect("two references");
    if hottest == 0 || depths.values().all(|one| *one == hottest) {
        return None;
    }
    let region = depths.iter().filter(|(_at, one)| **one == hottest).map(|(at, _)| *at).collect();
    Some(Region { blocks: region, starts_at: None })
}

/// `tryLocalSplit`: cut the widest same-block gap between references.
fn _local(
    body: &LirBody,
    value: u32,
    _live: &IndexMap<u32, Interval>,
    _index: &Indexes,
    _deep: &IndexMap<i64, u32>,
) -> Option<Region> {
    let found = _references(body, value);
    // The gap in code: a meta instruction is no distance.
    let gaps: Vec<(usize, i64, usize)> = body
        .blocks
        .iter()
        .filter_map(|block| found.get(&block.at).map(|positions| (block, positions)))
        .flat_map(|(block, positions)| {
            positions.windows(2).map(move |pair| {
                (block.insns[pair[0]..pair[1]].iter().filter(|one| !one.is_meta()).count(), block.at, pair[1])
            })
        })
        .collect();
    let (gap, at, cut) = *gaps.iter().max()?;
    if gap < 2 {
        return None;
    }
    Some(Region { blocks: BTreeSet::from([at]), starts_at: Some(cut) })
}

/// `tryBlockSplit`: the single deepest block that references it.
fn _per_block(
    body: &LirBody,
    value: u32,
    _live: &IndexMap<u32, Interval>,
    _index: &Indexes,
    deep: &IndexMap<i64, u32>,
) -> Option<Region> {
    let found = _references(body, value);
    if found.len() < 2 {
        return None;
    }
    let key = |one: i64| (deep.get(&one).copied().unwrap_or(0), -one);
    let mut at = *found.keys().next().expect("two references");
    for one in found.keys() {
        if key(*one) > key(at) {
            at = *one;
        }
    }
    if deep.get(&at).copied().unwrap_or(0) == 0 {
        return None;
    }
    Some(Region { blocks: BTreeSet::from([at]), starts_at: None })
}

/// Where a piece lives: a set of blocks, and where inside one it starts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Region {
    pub blocks: BTreeSet<i64>,
    pub starts_at: Option<usize>,
}

/// `body` with `region` reading a fresh value, joined by copies, or None
/// where `body` is returned unchanged.
fn _carved(body: &LirBody, value: u32, fresh: u32, width: u32, region: &Region) -> Option<LirBody> {
    let live_out = _live_out(body, value);
    let entered = _entries(body, region);
    let mut predecessors: IndexMap<i64, BTreeSet<i64>> = IndexMap::default();
    for block in &body.blocks {
        for place in &block.succ {
            predecessors.entry(*place).or_default().insert(block.at);
        }
    }
    let shared: BTreeSet<i64> = if region.starts_at.is_some() {
        BTreeSet::new()
    } else {
        entered
            .iter()
            .copied()
            .filter(|at| predecessors.get(at).is_some_and(|found| !found.is_disjoint(&region.blocks)))
            .collect()
    };
    let feeding: BTreeSet<i64> = shared
        .iter()
        .flat_map(|at| predecessors[at].iter().copied())
        .filter(|place| !region.blocks.contains(place))
        .collect();
    let at_of: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    if shared.iter().any(|at| *at == body.entry || predecessors[at].is_subset(&region.blocks))
        || feeding.iter().any(|place| at_of[place].insns.is_empty())
    {
        return None; // entered with no outside predecessor to hold the copy
    }
    let mut blocks: Vec<LirBlock> = Vec::new();
    let mut changed = false;
    let mut deferred: Vec<(i64, i64, Arc<Insn>)> = Vec::new();
    let rename = IndexMap::from_iter([(value, fresh)]);
    for block in &body.blocks {
        if feeding.contains(&block.at) {
            let mut insns = block.insns.clone();
            let last = Arc::clone(insns.last().expect("checked above"));
            let place = if _terminates(&last) { insns.len() - 1 } else { insns.len() };
            insns.insert(place, _copy(&last, fresh, value, width));
            blocks.push(block.with_insns(insns));
            changed = true;
            continue;
        }
        if !region.blocks.contains(&block.at) {
            blocks.push(block.clone());
            continue;
        }
        let start = region.starts_at.unwrap_or(0);
        let outside: Vec<i64> = block.succ.iter().copied().filter(|place| !region.blocks.contains(place)).collect();
        let leaves = region.starts_at.is_some() || !outside.is_empty();
        let mut insns: Vec<Arc<Insn>> = block.insns[..start.min(block.insns.len())].to_vec();
        let interior: Vec<Arc<Insn>> =
            block.insns[start.min(block.insns.len())..].iter().map(|one| _renamed(one, &rename)).collect();
        if interior.is_empty() {
            blocks.push(block.clone());
            continue;
        }
        if (entered.contains(&block.at) && !shared.contains(&block.at)) || region.starts_at.is_some() {
            insns.push(_copy(&interior[0], fresh, value, width));
            changed = true;
        }
        insns.extend(interior.iter().cloned());
        // Only where the original is wanted again.
        let edge_exit = region.starts_at.is_none() && !outside.is_empty() && outside.len() < block.succ.len();
        if leaves && (live_out.contains(&block.at) || region.starts_at.is_some()) && !edge_exit {
            let back = _copy(interior.last().expect("not empty"), value, fresh, width);
            let mut place =
                if _terminates(insns.last().expect("not empty")) { insns.len() - 1 } else { insns.len() };
            // After the last use where every way out leaves the region.
            if block.succ.iter().all(|place| !region.blocks.contains(place)) {
                place = place.min(_after_last(&insns, fresh));
            }
            insns.insert(place, back);
            changed = true;
        } else if edge_exit && live_out.contains(&block.at) {
            let beside = interior.last().expect("not empty");
            deferred.extend(outside.iter().map(|place| (block.at, *place, Arc::clone(beside))));
        }
        blocks.push(block.with_insns(insns));
    }
    if !changed && deferred.is_empty() {
        return None;
    }
    // The bridge owns an exit edge, so the original value is restored only
    // for paths that actually leave the region.
    let mut by_at: IndexMap<i64, LirBlock> = blocks.iter().map(|block| (block.at, block.clone())).collect();
    let mut bridges: Vec<LirBlock> = Vec::new();
    let mut next_at = body.blocks.iter().map(|block| block.at).max().unwrap_or(0) + 1;
    for (source, outside, beside) in deferred {
        let bridge = next_at;
        next_at += 1;
        let original = by_at[&source].clone();
        let rewritten: Vec<Arc<Insn>> = original
            .insns
            .iter()
            .map(|one| match &one.what {
                Some(what) if what.target == Some(outside) => {
                    let mut made = (**one).clone();
                    made.what = Some(Semantics { target: Some(bridge), ..what.clone() });
                    Arc::new(made)
                }
                _ => Arc::clone(one),
            })
            .collect();
        by_at.insert(
            source,
            LirBlock { succ: original.succ.iter().map(|one| if *one == outside { bridge } else { *one }).collect(), ..original.with_insns(rewritten) },
        );
        let back = _copy(&beside, value, fresh, width);
        let mut jump = Insn::new(
            beside.at,
            Some((beside.at, beside.at)),
            Some(Semantics { name: Some("jmp".to_owned()), target: Some(outside), ..Semantics::new(Operation::Jump) }),
            Vec::new(),
            Vec::new(),
        );
        jump.op = beside.op.clone();
        bridges.push(LirBlock { succ: vec![outside], ..LirBlock::new(bridge, vec![back, Arc::new(jump)]) });
    }
    let mut out: Vec<LirBlock> = blocks.iter().map(|block| by_at[&block.at].clone()).collect();
    out.extend(bridges);
    Some(body.with_blocks(out))
}

/// Region blocks control can reach from outside it.
fn _entries(body: &LirBody, region: &Region) -> BTreeSet<i64> {
    let inside = &region.blocks;
    let mut out: BTreeSet<i64> = body
        .blocks
        .iter()
        .filter(|block| inside.contains(&block.at) && block.at == body.entry)
        .map(|block| block.at)
        .collect();
    for block in &body.blocks {
        if inside.contains(&block.at) {
            continue;
        }
        out.extend(block.succ.iter().copied().filter(|place| inside.contains(place)));
    }
    let reached: BTreeSet<i64> = body
        .blocks
        .iter()
        .filter(|block| inside.contains(&block.at))
        .flat_map(|block| block.succ.iter().copied())
        .collect();
    out.extend(inside.iter().copied().filter(|at| !reached.contains(at)));
    out
}

/// Blocks this value is still wanted after.
fn _live_out(body: &LirBody, value: u32) -> BTreeSet<i64> {
    let (_incoming, outgoing) = allocate::live(body);
    outgoing.iter().filter(|(_at, values)| values.contains(&value)).map(|(at, _)| *at).collect()
}

/// The position after the last instruction naming `value`, and after the parallel copy holding it.
fn _after_last(insns: &[Arc<Insn>], value: u32) -> usize {
    let mut place = insns
        .iter()
        .enumerate()
        .filter(|(_position, one)| one.defines.contains(&value) || one.uses.contains(&value))
        .map(|(position, _one)| position + 1)
        .max()
        .unwrap_or(0);
    while 0 < place && place < insns.len() && insns[place].group.is_some() && insns[place].group == insns[place - 1].group
    {
        place += 1;
    }
    place
}

fn _terminates(one: &Insn) -> bool {
    one.what.as_ref().is_some_and(|what| matches!(what.op, Operation::Jump | Operation::Branch))
}

/// A move that claims none of the original bytes.
fn _copy(beside: &Insn, into: u32, out_of: u32, width: u32) -> Arc<Insn> {
    let at = beside.covers.map_or(beside.at, |covers| covers.0);
    let mut made = Insn::new(
        beside.at,
        Some((at, at)),
        Some(Semantics {
            name: Some("mov".to_owned()),
            dests: vec![Loc::Held(Held { value: into, width })],
            sources: vec![Loc::Held(Held { value: out_of, width })],
            ..Semantics::new(Operation::Move)
        }),
        vec![into],
        vec![out_of],
    );
    made.op = beside.op.clone();
    Arc::new(made)
}

/// How wide each value is, from the widest operand naming it.
fn _widths(body: &LirBody) -> IndexMap<u32, u32> {
    let mut out: IndexMap<u32, u32> = IndexMap::default();
    let mut widen = |value: u32, width: u32| {
        let had = out.get(&value).copied().unwrap_or(0);
        out.insert(value, had.max(width));
    };
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        if let Some(what) = &one.what {
            for operand in what.dests.iter().chain(&what.sources) {
                for held in ir::values(operand) {
                    widen(held.value, held.width);
                }
            }
        }
        for (held, _register) in one.requires.iter().chain(&one.delivers) {
            widen(held.value, held.width);
        }
        for (named, wide) in &one.widths {
            widen(*named, *wide);
        }
    }
    out
}

fn _renamed(one: &Arc<Insn>, rename: &IndexMap<u32, u32>) -> Arc<Insn> {
    if rename.is_empty() { Arc::clone(one) } else { spiller::_renamed(one, rename) }
}

fn _next_value(body: &LirBody) -> u32 {
    let mut seen: BTreeSet<u32> = BTreeSet::from([0]);
    for block in &body.blocks {
        seen.extend(block.arrives());
        for one in &block.insns {
            seen.extend(one.defines.iter().copied());
            seen.extend(one.uses.iter().copied());
        }
    }
    seen.last().copied().expect("seeded with zero") + 1
}

#[cfg(test)]
mod tests {
    //! Port of `tests/test_splitkit.py`.

    use std::collections::BTreeSet;
    use crate::support::hash::HashMap;
    use std::sync::Arc;

    use iced_x86::Register;
    use crate::support::hash::IndexMap;

    use super::{_carved, _local, loop_bases, split, Region};
    use crate::analysis::intervals::{self, Indexes};
    use crate::model::ir::{Addr, Held, Imm, Loc, Mem, Operation, Semantics, Space};
    use crate::model::lir::{Insn, LirBlock, LirBody};

    fn held(value: u32, width: u32) -> Loc {
        Loc::Held(Held { value, width })
    }

    fn imm(value: i64) -> Loc {
        Loc::Imm(Imm { value, width: 2, address: None })
    }

    fn sem(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>, target: Option<i64>) -> Semantics {
        Semantics { name: Some(name.to_owned()), dests, sources, target, ..Semantics::new(op) }
    }

    fn _insn(at: i64, what: Semantics, defines: &[u32], uses: &[u32]) -> Arc<Insn> {
        Arc::new(Insn::new(at, Some((at, at + 2)), Some(what), defines.to_vec(), uses.to_vec()))
    }

    fn jump(at: i64, to: i64) -> Arc<Insn> {
        _insn(at, sem(Operation::Jump, "jmp", vec![], vec![], Some(to)), &[], &[])
    }

    fn move_imm(at: i64, into: u32, value: i64) -> Arc<Insn> {
        _insn(at, sem(Operation::Move, "mov", vec![held(into, 2)], vec![imm(value)], None), &[into], &[])
    }

    fn push(at: i64, value: u32) -> Arc<Insn> {
        _insn(at, sem(Operation::Push, "push", vec![], vec![held(value, 2)], None), &[], &[value])
    }

    fn add(at: i64, value: u32) -> Arc<Insn> {
        let what = sem(Operation::Binary, "add", vec![held(value, 2)], vec![held(value, 2), imm(1)], None);
        _insn(at, what, &[value], &[value])
    }

    fn block(at: i64, insns: Vec<Arc<Insn>>, succ: &[i64]) -> LirBlock {
        LirBlock { succ: succ.to_vec(), ..LirBlock::new(at, insns) }
    }

    fn body(name: &str, blocks: Vec<LirBlock>) -> LirBody {
        LirBody::new(name, 0, blocks, IndexMap::default(), IndexMap::default())
    }

    fn where_() -> Addr {
        Addr { base: Register::SI, ..Addr::new(Space::Segment, 0x10) }
    }

    /// v3 is made before the loop, read in it and after it, as a cell's base.
    fn _pointer_across_a_loop() -> LirBody {
        let cell = Loc::Mem(Mem {
            through: Register::None,
            offset: 0,
            disp_width: 2,
            base: Some(Held { value: 3, width: 2 }),
            ..Mem::new(Some(where_()), 2)
        });
        body(
            "one",
            vec![
                block(0, vec![move_imm(0, 3, 0x40), move_imm(2, 4, 1), jump(4, 0x10)], &[0x10]),
                block(
                    0x10,
                    vec![
                        add(0x10, 4),
                        _insn(0x11, sem(Operation::Move, "mov", vec![held(6, 2)], vec![cell.clone()], None), &[6], &[3]),
                        _insn(0x12, sem(Operation::Branch, "jne", vec![], vec![], Some(0x10)), &[], &[]),
                    ],
                    &[0x10, 0x20],
                ),
                block(
                    0x20,
                    vec![
                        _insn(0x20, sem(Operation::Move, "mov", vec![held(5, 2)], vec![cell], None), &[5], &[3]),
                        _insn(0x22, sem(Operation::Return, "ret", vec![], vec![], None), &[], &[5]),
                    ],
                    &[],
                ),
            ],
        )
    }

    fn region(blocks: &[i64], starts_at: Option<usize>) -> Region {
        Region { blocks: blocks.iter().copied().collect(), starts_at }
    }

    #[test]
    fn test_a_piece_leaves_its_region_after_its_last_use() {
        let body = body(
            "one",
            vec![
                block(0, vec![move_imm(0, 3, 1), jump(2, 0x10)], &[0x10]),
                block(0x10, vec![add(0x10, 3), move_imm(0x12, 7, 2), jump(0x14, 0x20)], &[0x20]),
                block(0x20, vec![push(0x20, 3), push(0x21, 7)], &[]),
            ],
        );
        let cut = _carved(&body, 3, 9, 2, &region(&[0x10], None)).expect("cut");
        let live = intervals::intervals(&cut, None);
        assert!(!live[&9].overlaps(&live[&7]));
    }

    fn read(values: &HashMap<u32, i64>, operand: &Loc) -> i64 {
        match operand {
            Loc::Imm(one) => one.value,
            Loc::Held(one) => values[&one.value],
            other => panic!("{other:?}"),
        }
    }

    fn into(what: &Semantics) -> u32 {
        match &what.dests[0] {
            Loc::Held(one) => one.value,
            other => panic!("{other:?}"),
        }
    }

    /// What the pushes along `path` write, running moves, adds and pushes.
    fn _pushed(body: &LirBody, path: &[i64]) -> Vec<i64> {
        let mut values: HashMap<u32, i64> = HashMap::default();
        let mut pushed = Vec::new();
        let blocks: HashMap<i64, &LirBlock> = body.blocks.iter().map(|one| (one.at, one)).collect();
        for at in path {
            for one in &blocks[at].insns {
                let what = one.what.as_ref().expect("semantics");
                match what.op {
                    Operation::Move => {
                        let value = read(&values, &what.sources[0]);
                        values.insert(into(what), value);
                    }
                    Operation::Binary => {
                        let value = read(&values, &what.sources[0]) + read(&values, &what.sources[1]);
                        values.insert(into(what), value);
                    }
                    Operation::Push => pushed.push(read(&values, &what.sources[0])),
                    _ => {}
                }
            }
        }
        pushed
    }

    #[test]
    fn test_a_region_block_also_reached_from_inside_it_keeps_the_inside_value() {
        let branch = _insn(2, sem(Operation::Branch, "jne", vec![], vec![], Some(0x20)), &[], &[]);
        let body = body(
            "one",
            vec![
                block(0, vec![move_imm(0, 3, 1), branch], &[0x10, 0x20]),
                block(0x10, vec![add(0x10, 3), jump(0x12, 0x20)], &[0x20]),
                block(0x20, vec![push(0x20, 3)], &[]),
            ],
        );
        let cut = _carved(&body, 3, 9, 2, &region(&[0x10, 0x20], None)).expect("cut");
        assert_eq!(_pushed(&cut, &[0, 0x10, 0x20]), vec![2]);
        assert_eq!(_pushed(&cut, &[0, 0x20]), vec![1]);
    }

    #[test]
    fn test_a_cut_range_renames_what_an_instruction_requires_and_delivers() {
        let what = sem(Operation::Call, "call", vec![], vec![], None);
        let call = Insn {
            delivers: vec![(Held { value: 3, width: 2 }, Register::DI)],
            widths: vec![(3, 2)],
            ..Insn::new(0x10, Some((0x10, 0x12)), Some(what), vec![3], vec![])
        };
        let out = Insn {
            requires: vec![(Held { value: 3, width: 1 }, Register::AL)],
            widths: vec![(3, 1)],
            ..Insn::new(0x12, Some((0x12, 0x13)), None, vec![], vec![3])
        };
        let body = body(
            "one",
            vec![
                block(0, vec![jump(0, 0x10)], &[0x10]),
                block(0x10, vec![Arc::new(call), Arc::new(out), jump(0x14, 0x20)], &[0x20]),
                block(0x20, vec![push(0x20, 3)], &[]),
            ],
        );
        let cut = _carved(&body, 3, 9, 2, &region(&[0x10], None)).expect("cut");
        for one in cut.insns() {
            let named: BTreeSet<u32> = one.uses.iter().chain(&one.defines).copied().collect();
            assert!(one.requires.iter().all(|(held, _)| one.uses.contains(&held.value)), "{one:?}");
            assert!(one.delivers.iter().all(|(held, _)| one.defines.contains(&held.value)), "{one:?}");
            assert!(one.widths.iter().all(|(value, _)| named.contains(value)), "{one:?}");
        }
    }

    #[test]
    fn test_a_cut_range_renames_the_cell_it_is_the_base_of() {
        let body = split(&_pointer_across_a_loop(), Some(&BTreeSet::from([3])), None, None).expect("cut");
        let loads: Vec<Arc<Insn>> = body
            .insns()
            .into_iter()
            .filter(|one| one.what.as_ref().is_some_and(|what| what.sources.iter().any(|x| matches!(x, Loc::Mem(_)))))
            .collect();
        assert!(loads.iter().any(|one| one.uses != [3]), "nothing was cut; the fixture does not reach the rename");
        let mut last = None;
        for load in &loads {
            let Loc::Mem(cell) = &load.what.as_ref().expect("semantics").sources[0] else { panic!("not a cell") };
            let base = cell.base.expect("base");
            assert_eq!(load.uses, [base.value], "uses {:?}, cell on {base:?}", load.uses);
            assert_eq!(cell.through, Register::None, "the rename placed it");
            last = Some(cell.clone());
        }
        let cell = last.expect("a load");
        assert_eq!((cell.addr, cell.width, cell.offset, cell.disp_width), (Some(where_()), 2, 0, 2));
    }

    #[test]
    fn test_local_split_can_cut_a_gap_in_one_block_of_a_cross_block_range() {
        let body = _pointer_across_a_loop();
        let first = _insn(0x0F, sem(Operation::Move, "mov", vec![held(8, 2)], vec![held(3, 2)], None), &[8], &[3]);
        // v3 is now read twice in this block with the increment/load work
        // between them, and remains read by the exit block.
        let blocks = body
            .blocks
            .iter()
            .map(|one| {
                let mut insns = one.insns.clone();
                if one.at == 0x10 {
                    insns.insert(0, Arc::clone(&first));
                }
                one.with_insns(insns)
            })
            .collect();
        let body = LirBody { blocks, ..body };
        let empty = Indexes { at: IndexMap::default(), span: IndexMap::default(), order: Vec::new() };
        let plan = _local(&body, 3, &IndexMap::default(), &empty, &IndexMap::default());
        assert_eq!(plan, Some(region(&[0x10], Some(2))));
    }

    /// A meta instruction is no distance, as LLVM's SlotIndexes skip debug
    /// instructions. Counted, markers moved splits and changed register
    /// choices: addrm took SI where BX was free.
    #[test]
    fn test_meta_instructions_neither_lengthen_a_range_nor_move_a_split() {
        let marker = |at: i64| Arc::new(Insn::new(at, Some((at, at + 1)), Some(crate::model::lir::inert()), Vec::new(), Vec::new()));
        let with = |markers: usize| {
            let mut insns = vec![move_imm(0, 3, 0x40)];
            insns.extend((0..markers as i64).map(|at| marker(2 + at)));
            insns.extend([push(0x10, 3), move_imm(0x12, 4, 1), move_imm(0x14, 5, 2), push(0x16, 3), push(0x18, 4), push(0x1a, 5)]);
            body("marked", vec![block(0, insns, &[])])
        };
        let (bare, marked) = (with(0), with(3));
        let size = |body: &LirBody| intervals::intervals(body, None)[&3].size();
        assert_eq!(size(&marked), size(&bare));
        let empty = Indexes { at: IndexMap::default(), span: IndexMap::default(), order: Vec::new() };
        let cut = |body: &LirBody| _local(body, 3, &IndexMap::default(), &empty, &IndexMap::default());
        assert_eq!(cut(&bare), Some(region(&[0], Some(4))));
        assert_eq!(cut(&marked), Some(region(&[0], Some(7))), "the same push, three markers later");
    }

    #[test]
    fn test_a_loop_scoped_base_piece_enters_once_and_leaves_after_the_loop() {
        let (cut, kept) = loop_bases(&_pointer_across_a_loop(), &BTreeSet::from([3]));
        assert_eq!(kept.len(), 1);
        let fresh = *kept.iter().next().expect("one");
        let blocks: HashMap<i64, &LirBlock> = cut.blocks.iter().map(|one| (one.at, one)).collect();
        assert!(blocks[&0].insns.iter().any(|one| one.defines == [fresh] && one.uses == [3]));
        assert!(
            cut.blocks
                .iter()
                .filter(|one| ![0, 0x10, 0x20].contains(&one.at))
                .flat_map(|one| &one.insns)
                .any(|one| one.defines == [3] && one.uses == [fresh])
        );
        let loop_cells: Vec<&Mem> = blocks[&0x10]
            .insns
            .iter()
            .filter_map(|one| one.what.as_ref())
            .flat_map(|what| what.dests.iter().chain(&what.sources))
            .filter_map(|place| if let Loc::Mem(cell) = place { Some(cell) } else { None })
            .collect();
        let bases: BTreeSet<u32> = loop_cells.iter().map(|one| one.base.expect("base").value).collect();
        assert!(!loop_cells.is_empty() && bases == BTreeSet::from([fresh]));
    }

    #[test]
    fn test_a_loop_piece_restores_only_on_its_exiting_edge() {
        let cut = _carved(&_pointer_across_a_loop(), 3, 9, 2, &region(&[0x10], None)).expect("cut");
        let found = cut.blocks.iter().find(|one| one.at == 0x10).expect("loop");
        assert!(!found.insns.iter().any(|one| one.defines == [3] && one.uses == [9]));
        let bridge = cut.blocks.iter().find(|one| ![0, 0x10, 0x20].contains(&one.at)).expect("bridge");
        assert_eq!(bridge.succ, [0x20]);
        assert!(bridge.insns[0].defines == [3] && bridge.insns[0].uses == [9]);
    }

    /// v4 counts to 10: set before the loop, tested in its header, bumped in its latch, read after.
    fn _counting_loop() -> LirBody {
        let compare = sem(Operation::Compare, "cmp", vec![], vec![held(4, 2), imm(10)], None);
        body(
            "count",
            vec![
                block(0, vec![move_imm(0, 4, 0), jump(2, 0x10)], &[0x10]),
                block(
                    0x10,
                    vec![
                        _insn(0x10, compare, &[], &[4]),
                        _insn(0x12, sem(Operation::Branch, "jge", vec![], vec![], Some(0x30)), &[], &[]),
                    ],
                    &[0x20, 0x30],
                ),
                block(0x20, vec![add(0x20, 4), jump(0x22, 0x10)], &[0x10]),
                block(
                    0x30,
                    vec![
                        _insn(0x30, sem(Operation::Move, "mov", vec![held(5, 2)], vec![held(4, 2)], None), &[5], &[4]),
                        _insn(0x32, sem(Operation::Return, "ret", vec![], vec![], None), &[], &[5]),
                    ],
                    &[],
                ),
            ],
        )
    }

    /// Values after executing the body: moves, add, cmp, jge, jmp and ret.
    fn _run(body: &LirBody) -> HashMap<u32, i64> {
        let blocks: HashMap<i64, &LirBlock> = body.blocks.iter().map(|one| (one.at, one)).collect();
        let mut values: HashMap<u32, i64> = HashMap::default();
        let (mut at, mut flags) = (body.entry, 0);
        for _step in 0..1000 {
            let (block, mut following) = (blocks[&at], None);
            for one in &block.insns {
                let what = one.what.as_ref().expect("semantics");
                match what.op {
                    Operation::Move => {
                        let value = read(&values, &what.sources[0]);
                        values.insert(into(what), value);
                    }
                    Operation::Binary => {
                        let value = read(&values, &what.sources[0]) + read(&values, &what.sources[1]);
                        values.insert(into(what), value);
                    }
                    Operation::Compare => flags = read(&values, &what.sources[0]) - read(&values, &what.sources[1]),
                    Operation::Branch => {
                        following = if flags >= 0 {
                            what.target
                        } else {
                            block.succ.iter().copied().find(|one| Some(*one) != what.target)
                        };
                    }
                    Operation::Jump => following = what.target,
                    Operation::Return => return values,
                    _ => {}
                }
            }
            at = following.unwrap_or(block.succ[0]);
        }
        panic!("the loop never ended");
    }

    #[test]
    fn test_a_cut_loop_counter_keeps_its_latch_value() {
        assert_eq!(_run(&_counting_loop())[&5], 10);
        let body = split(&_counting_loop(), Some(&BTreeSet::from([4])), None, None).expect("cut");
        assert!(
            body.insns()
                .iter()
                .any(|one| [0x10, 0x20].contains(&one.at) && !one.uses.is_empty() && !one.uses.contains(&4)),
            "nothing was cut"
        );
        assert_eq!(_run(&body)[&5], 10);
    }
}
