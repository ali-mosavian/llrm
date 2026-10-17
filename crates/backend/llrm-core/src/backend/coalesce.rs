//! Removes a copy whose two values can share a register.
//!
//! # How it works
//! After `phielim` and `twoaddr` the body is full of `mov v2, v1` between
//! values. If `v1` and `v2` never need different registers at once, they can
//! be one value and the move is an identity that disappears.
//!
//! ```text
//!  before                          after
//!    v1 = load ...                   v1 = load ...
//!    v2 = mov v1       (no clash)    ... v1 ...     v2 renamed v1
//!    ... v2 ...                      the mov is gone
//! ```
//!
//! Each value has a live interval (`analysis::intervals`). Copies are visited
//! block by block, the deepest loops first and, within a loop, the busiest
//! blocks first, so that of two joins that clash the one in the hotter code
//! stays. For one copy `(a, b)` the webs already joined are looked up
//! (union-find) and the join is refused when:
//!
//! - they are the same web, or are pinned to different registers;
//! - their live ranges share a point, unless the copy is of one width, begins
//!   the only run they share, and neither web is written inside it (both then
//!   hold the copied value there), and it is not a member of a parallel copy;
//! - the registers both may take (their class palettes) have none in common, or
//!   every common register is destroyed somewhere in the merged interval
//!   (`allocate::_clobbered`), or a pin is outside what is left.
//!
//! The joined web's interval is the union of the two (`_merged`) and its pin
//! is the pin of either; the pinned web is the survivor. At the end every
//! value is renamed to its web's root and identity moves are dropped.
//!
//! There is no interference graph and no Briggs or George test. Two live
//! ranges that share no point leave the live count of every point as it was,
//! so the join makes no point more crowded; what the allocator does with a
//! range of many segments is its to split and evict. Of 567 copies in
//! `d_faces` at `-O2` the graph joined 174 and this joins 518, for 15.8 M
//! of graph and 13.9 M of intervals built before the first copy was looked
//! at, 55.2 M against 18.5 M for the whole pass.
//!
//! # LLVM structure, and what is not translated
//! LLVM's `RegisterCoalescer`
//! ([`RegisterCoalescer.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp))
//! orders the copies by loop depth, then split edges, then CFG degree
//! (`copyCoalesceInMBB`, L4058), joins by `joinCopy` (L2054) over
//! `LiveIntervals` with `JoinVals` (L3154-L3222) giving values that are copies
//! of each other one number, and retries deferred copies until nothing changes
//! (L4286-L4321). This follows the order of the worklist (depth, then
//! frequency) and the overlap test with its same-value exception. It deviates,
//! each until the `liveness` module's `LiveRange` lands (`regalloc-core`'s
//! translation of `LiveInterval.cpp`): no `VNInfo`s, so the same-value rule is
//! the one case `JoinVals` finds by value number (a width-equal copy starting
//! the only shared run); no `CoalescerPair` classification of physical,
//! sub-register and virtual joins (pins and palettes stand in); no
//! `ConflictResolution` (`CR_Keep/Erase/Replace/Merge/Unresolved`), no lane
//! masks or sub-ranges; no deferral and retry (a refused copy is not tried
//! again after later joins); no `removeCopyByCommutingDef`,
//! `reMaterializeTrivialDef` or `LargeIntervalSizeThreshold`.
//!
//! # Example
//! A multiply by 5: `v3` joins `v4`, so `v3 := mov v4` goes and the add updates
//! `v4`. `v5 := mov v4` stays: `v4` and `v5` are pinned to EAX and EDX. `;
//! inserted` notes are dropped.
//!
//! Before (`TwoAddress`):
//!
//! ```lir
//! ; ...
//! 0x0001  v4 := mov v2
//! 0x0001  v4 := shl v4, 0x2
//! 0x0001  v3 := mov v4
//! 0x0001  v3 := add v3, v2
//! 0x0002  v5 := mov v3
//! 0x0002  v5 := shr v5, 0x10
//! 0x0002     ; requires v3@EAX,v5@EDX
//! ```
//!
//! After (`Coalescer`):
//!
//! ```lir
//! ; ...
//! 0x0001  v4 := mov v2
//! 0x0001  v4 := shl v4, 0x2
//! 0x0001  v4 := add v4, v2
//! 0x0002  v5 := mov v4
//! 0x0002  v5 := shr v5, 0x10
//! 0x0002     ; requires v4@EAX,v5@EDX
//! ```
//!
//! From `ISEL_DUMP=1 llrm-qb -O2 -march=i386` of `Times5& = x * 5` (function
//! `TIMES5&`).
//!
//! # Runs
//! Once per function, in the machine phases after `twoaddr` and before
//! `regalloc`, at every -O level. Both routes run it.
//!
//! # Needs
//! [`crate::backend::classes`] for the palettes, [`crate::backend::regclass`]
//! for each value's register class, `allocate::_masks` for the points that
//! destroy registers, and `analysis::intervals` for the live ranges.
//! `_interference` and `_interference_among` stay for `twoaddr.rs` and the
//! spiller's sibling pricing.

use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::analysis::intervals::{self as ranges, Interval, Segment};
use crate::backend::classes::RegisterClasses;
use crate::backend::neighbours::Neighbours;
use crate::backend::target::{self, Segments};
use crate::backend::{allocate, regclass};
use crate::model::ir::{self, Held, Loc, Operation};
use crate::model::lir::{self, Block, Insn, LirBlock, LirBody, Phi};
use crate::model::passes::LIRTransform;
use crate::support::hash::{HashMap, IndexMap};

pub struct Coalescer {
    pub pinned: IndexMap<u32, RegId>,
    pub segments: Segments,
    pub classes: std::rc::Rc<RegisterClasses>,
}

impl Coalescer {
    pub const NAME: &'static str = "coalesce";

    pub fn new(
        pinned: Option<&IndexMap<u32, RegId>>,
        segments: &Segments,
        classes: &std::rc::Rc<RegisterClasses>,
    ) -> Self {
        Self {
            pinned: pinned.cloned().unwrap_or_default(),
            segments: segments.clone(),
            classes: std::rc::Rc::clone(classes),
        }
    }
}

impl LIRTransform for Coalescer {
    fn class_name(&self) -> &'static str {
        "Coalescer"
    }

    fn name(&self) -> &str {
        Self::NAME
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(joined(&body, Some(&self.pinned), &self.segments, &self.classes))
    }
}

pub type Graph = IndexMap<u32, Neighbours>;

/// Whether two live ranges that overlap meet only where `copy` starts: they
/// overlap in one run that begins at the copy's write and neither web is
/// written inside it, so both hold the copied value throughout.
fn _same_value_where_they_meet(
    mine: &Interval,
    theirs: &Interval,
    copy: &Arc<Insn>,
    copied: u32,
    index: &ranges::Indexes,
    writes: &IndexMap<u32, Vec<(i64, u32)>>,
    mine_node: u32,
    theirs_node: u32,
) -> bool {
    // The members of a parallel copy read before any is written: they are not
    // one after the other.
    if copy.group.is_some() {
        return false;
    }
    let Some(start) = index.write_of(ranges::key(copy)) else {
        return false;
    };
    let mut meeting: Option<Segment> = None;
    for first in &mine.segments {
        for second in &theirs.segments {
            if first.overlaps(second) {
                if meeting.is_some() {
                    return false;
                }
                meeting = Some(Segment { start: first.start.max(second.start), end: first.end.min(second.end) });
            }
        }
    }
    let Some(run) = meeting else {
        return true;
    };
    run.start == start
        && [mine_node, theirs_node].iter().all(|node| {
            writes
                .get(node)
                .into_iter()
                .flatten()
                .all(|(at, value)| (*at == start && *value == copied) || *at < run.start || *at >= run.end)
        })
}

fn _find(
    parent: &mut IndexMap<u32, u32>,
    one: u32,
) -> u32 {
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
pub fn joined(
    body: &LirBody,
    pinned: Option<&IndexMap<u32, RegId>>,
    segments: &Segments,
    classes: &RegisterClasses,
) -> LirBody {
    let mut every = body.pins.clone();
    every.extend(pinned.into_iter().flatten().map(|(value, register)| (*value, *register)));
    let pinned: IndexMap<u32, RegId> = every.into_iter().map(|(value, register)| (value, ir::root(register))).collect();

    let index = ranges::indexed(body);
    let mut live = ranges::intervals(body, Some(&index));
    let masks = allocate::_masks(body, &index, segments);
    let mut widths = allocate::_widest(body);
    let widths_of_values = widths.clone();
    let where_of = regclass::classes(body, &BTreeSet::new(), segments, classes);
    let everything: BTreeSet<RegId> = classes.available.iter().copied().collect();
    // Where each web is written: two live ranges that meet are one value there,
    // and so no clash, where a copy starts the meeting and neither is
    // written inside it (LLVM's `JoinVals` gives such values one number).
    let mut writes: IndexMap<u32, Vec<(i64, u32)>> = IndexMap::default();
    for block in &body.blocks {
        for one in &block.insns {
            if let Some(at) = index.write_of(ranges::key(one)) {
                for value in &one.defines {
                    writes.entry(*value).or_default().push((at, *value));
                }
            }
        }
    }
    // The registers a web may take: its class's, or the one it is pinned to
    // outside every class.
    let mut palette: IndexMap<u32, BTreeSet<RegId>> = live
        .keys()
        .map(|one| (*one, target::order(where_of.get(one), segments, classes).into_iter().collect()))
        .collect();
    for (value, register) in &pinned {
        if !everything.contains(register) && !where_of.contains_key(value) {
            palette.insert(*value, BTreeSet::from([*register]));
        }
    }
    let mut held: IndexMap<u32, RegId> = pinned.clone();
    let mut parent: IndexMap<u32, u32> = IndexMap::default();
    // The web a root's values are kept as.
    let mut node_of: IndexMap<u32, u32> = IndexMap::default();

    // The deepest loops first, as LLVM's `RegisterCoalescer` orders its
    // worklist, and in a loop the blocks that run most often: of two joins
    // that clash the first stays.
    let busy = crate::analysis::frequency::Frequency::of(body);
    let mut blocks: Vec<&Block> = body.blocks.iter().collect();
    let depth = ranges::depths_shared(body);
    blocks.sort_by(|a, b| {
        let (a_depth, b_depth) = (depth.get(&a.at).copied().unwrap_or(0), depth.get(&b.at).copied().unwrap_or(0));
        b_depth
            .cmp(&a_depth)
            .then_with(
                || if a_depth == 0 { std::cmp::Ordering::Equal } else { busy.block(b.at).total_cmp(&busy.block(a.at)) },
            )
    });
    for block in blocks {
        for one in &block.insns {
            let Some(pair) = _copy(one) else {
                continue;
            };
            let (mut here, mut there) = (_find(&mut parent, pair.0), _find(&mut parent, pair.1));
            if here == there {
                continue;
            }
            let (mut mine_node, mut theirs_node) =
                (node_of.get(&here).copied().unwrap_or(here), node_of.get(&there).copied().unwrap_or(there));
            // Two pinned to different registers are two registers.
            let (mine_pin, theirs_pin) = (held.get(&mine_node).copied(), held.get(&theirs_node).copied());
            if matches!(
                (mine_pin, theirs_pin),
                (Some(mine), Some(theirs)) if mine != theirs
            ) {
                continue;
            }
            let (Some(mine), Some(theirs)) = (live.get(&mine_node), live.get(&theirs_node)) else {
                continue;
            };
            // Two live ranges that share no point leave the live count of every
            // point as it was: the join is refused only where they
            // meet. A copy of the same width is the one meeting that is no
            // clash.
            if mine.overlaps(theirs) {
                let same_width = widths_of_values.get(&pair.0) == widths_of_values.get(&pair.1);
                if !same_width
                    || !_same_value_where_they_meet(mine, theirs, one, pair.0, &index, &writes, mine_node, theirs_node)
                {
                    continue;
                }
            }
            let allowed: BTreeSet<RegId> = palette
                .get(&mine_node)
                .into_iter()
                .flatten()
                .filter(|register| palette.get(&theirs_node).is_some_and(|other| other.contains(register)))
                .copied()
                .collect();
            if allowed.is_empty() {
                continue;
            }
            let merged = _merged(mine, theirs);
            let width =
                widths.get(&mine_node).copied().unwrap_or(0).max(widths.get(&theirs_node).copied().unwrap_or(0)).max(1);
            let allowed: BTreeSet<RegId> = allowed
                .into_iter()
                .filter(|register| !allocate::_clobbered(&merged, *register, &masks, width))
                .collect();
            if allowed.is_empty() || [mine_pin, theirs_pin].into_iter().flatten().any(|pin| !allowed.contains(&pin)) {
                continue;
            }
            // The pinned web is the one that stays, so that the survivor is the
            // value the allocator was told of.
            if mine_pin.is_some() && theirs_pin.is_none() {
                std::mem::swap(&mut here, &mut there);
                std::mem::swap(&mut mine_node, &mut theirs_node);
            }
            parent.insert(here, there);
            node_of.insert(there, mine_node);
            live.insert(mine_node, merged);
            live.swap_remove(&theirs_node);
            let theirs_writes = writes.swap_remove(&theirs_node).unwrap_or_default();
            writes.entry(mine_node).or_default().extend(theirs_writes);
            widths.insert(mine_node, width);
            palette.insert(mine_node, allowed);
            palette.swap_remove(&theirs_node);
            held.swap_remove(&theirs_node);
            if let Some(register) = mine_pin.or(theirs_pin) {
                held.insert(mine_node, register);
            }
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
        ..body.with_blocks(
            body.blocks
                .iter()
                .map(|block| LirBlock {
                    phis: block
                        .phis
                        .iter()
                        .map(|phi| Phi {
                            result: swap(phi.result),
                            incoming: phi.incoming.iter().map(|(at, value)| (*at, swap(*value))).collect(),
                        })
                        .collect(),
                    ..block.with_insns(_kept(block, &swap)).into_inner()
                })
                .collect(),
        )
    }
}

thread_local! {
    static ASKED: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static NUMBERED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static VISITED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many values the liveness rows of the last interference graph of this
/// thread numbered, for a test that a graph of a few values is not made over
/// every value in the body.
pub fn last_numbered() -> usize {
    NUMBERED.with(std::cell::Cell::get)
}

/// How many values the last interference graph of this thread was asked for,
/// none for all, for a test that `siblings` asks of the webs it grows from
/// only.
pub fn last_asked() -> Option<usize> {
    ASKED.with(std::cell::Cell::get)
}

/// How many instructions the last interference graph of this thread looked at.
pub fn last_visited() -> usize {
    VISITED.with(std::cell::Cell::get)
}

pub fn _interference(body: &LirBody) -> Graph {
    _interference_among(body, None)
}

/// `_interference`, of the values in `only` alone where it is given: a caller
/// that asks of a few values pays for the pairs among them, not for every pair
/// live together. The rows `_interference_among` reads: the walk's, or those
/// found from where the values occur.
enum Rows<'a> {
    Dense(allocate::LiveRows),
    Web(&'a allocate::WebRows),
}

impl Rows<'_> {
    fn numbered(&self) -> usize {
        match self {
            Rows::Dense(rows) => rows.numbered(),
            Rows::Web(rows) => rows.numbered(),
        }
    }

    fn entering(
        &self,
        at: i64,
    ) -> Vec<u32> {
        match self {
            Rows::Dense(rows) => rows.entering(at).collect(),
            Rows::Web(rows) => rows.entering(at).collect(),
        }
    }

    fn leaving(
        &self,
        at: i64,
    ) -> Vec<u32> {
        match self {
            Rows::Dense(rows) => rows.leaving(at).collect(),
            Rows::Web(rows) => rows.leaving(at).collect(),
        }
    }
}

pub fn _interference_among(
    body: &LirBody,
    only: Option<&BTreeSet<u32>>,
) -> Graph {
    ASKED.with(|asked| asked.set(only.map(BTreeSet::len)));
    let wanted = |value: u32| only.is_none_or(|only| only.contains(&value));
    // Asked of a few values, the rows and widths come from where the values
    // occur, not from a walk of every instruction (in a body with no phis,
    // whose arguments are read in other blocks).
    let among = only.map(|only| {
        crate::backend::postings::following(body, |postings| {
            crate::analysis::occurrences::Occurrences::of(postings, only)
        })
    });
    let web = among.as_ref().and_then(|found| found.rows(body));
    let rows = match &web {
        Some(web) => Rows::Web(web),
        None => Rows::Dense(allocate::live_rows_by(body, wanted)),
    };
    NUMBERED.with(|numbered| numbered.set(rows.numbered()));
    VISITED.with(|visited| visited.set(0));
    // A copy's widths are read only where both its values are asked of (an edge
    // needs both).
    let widths = match (&among, &web) {
        (Some(found), Some(_)) => found.widths(body, &wanted),
        _ => crate::analysis::occurrences::widths_of(body.blocks.iter().flat_map(|block| &block.insns), &wanted),
    };
    // Asked of a few values, their rows are bits from the start: a group of N
    // live together is N rows given N bits at once, not N^2 pairs each
    // written into a tree.
    // A matrix of rows of bits, as long as it is no more than 64 MB; the trees
    // that turn to bits as they fill are for what does not fit.
    let highest = body
        .blocks
        .iter()
        .flat_map(|block| &block.insns)
        .flat_map(|one| one.defines.iter().chain(&one.uses))
        .max()
        .copied();
    let matrix = highest.is_some_and(|highest| rows.numbered() * (highest as usize / 64 + 1) <= 8_000_000);
    let mut graph = if only.is_some() || matrix { Building::bits() } else { Building::default() };

    let all_pairs = |graph: &mut Building, alive: &Alive| {
        let named: Vec<u32> = alive.iter().filter(|one| wanted(*one)).collect();
        graph.link_group(&named);
    };

    let targets: BTreeSet<i64> = body.blocks.iter().flat_map(|block| block.succ.iter().copied()).collect();
    let mut entries: BTreeSet<i64> = BTreeSet::from([body.entry]);
    entries.extend(body.blocks.iter().map(|block| block.at).filter(|at| !targets.contains(at)));
    // Asked of a few values, a block that names none of them changes nothing in
    // the walk: what is live in it stays so from its end to its start, so
    // it adds pairs only at the entry, at its phis, and at a parallel copy
    // where two of them are live.
    let touched: Option<BTreeSet<usize>> = only.map(|only| {
        crate::backend::postings::following(body, |postings| {
            only.iter()
                .flat_map(|value| postings.defs_at(*value).chain(postings.uses_at(*value)))
                .map(|at| at.0 as usize)
                .collect()
        })
    });
    // Asked of a few values, an instruction that names none of them and is no
    // parallel copy changes nothing in the walk (what it defines or reads is
    // never live here), so the walk is of the instructions that do.
    let naming: Option<HashMap<usize, Vec<usize>>> = only.map(|only| {
        crate::backend::postings::following(body, |postings| {
            let mut found: HashMap<usize, Vec<usize>> = HashMap::default();
            for at in only.iter().flat_map(|value| postings.defs_at(*value).chain(postings.uses_at(*value))) {
                found.entry(at.0 as usize).or_default().push(at.1 as usize);
            }
            found
        })
    });
    for (block_index, block) in body.blocks.iter().enumerate() {
        if let Some(touched) = &touched {
            // A quiet block (no value asked of in it, no entry, no phi, no
            // parallel copy with two live) adds no pair, and what
            // is live at its end is asked of the others only.
            let quiet = !touched.contains(&block_index)
                && !entries.contains(&block.at)
                && block.phis.is_empty()
                && (!block.insns.iter().any(|one| one.group.is_some())
                    || rows.leaving(block.at).into_iter().filter(|one| wanted(*one)).nth(1).is_none());
            if quiet {
                continue;
            }
        }
        let mut alive = Alive::of(only.is_some(), rows.leaving(block.at).into_iter().filter(|one| wanted(*one)));
        if entries.contains(&block.at) {
            all_pairs(
                &mut graph,
                &Alive::of(only.is_some(), rows.entering(block.at).into_iter().filter(|one| wanted(*one))),
            );
        }
        // The positions to walk, last first; every one where not asked of a
        // few.
        let sparse: Option<Vec<usize>> = naming
            .as_ref()
            .map(
                |naming| {
                    let mut at: Vec<usize> = naming.get(&block_index).cloned().unwrap_or_default();
                    at.extend(block.insns.iter().enumerate().filter(|(_, one)| one.group.is_some()).map(|(i, _)| i));
                    at.sort_unstable_by(|a, b| b.cmp(a));
                    at.dedup();
                    at
                },
            );
        let mut next = 0;
        let mut index = block.insns.len() as i64 - 1;
        loop {
            if let Some(sparse) = &sparse {
                while next < sparse.len() && sparse[next] as i64 > index {
                    next += 1;
                }
                let Some(&at) = sparse.get(next) else { break };
                index = at as i64;
            } else if index < 0 {
                break;
            }
            VISITED.with(|visited| visited.set(visited.get() + 1));
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
                        before.remove(*value);
                    }
                }
                for item in group {
                    before.extend(item.uses.iter().copied());
                }
                all_pairs(&mut graph, &before);
                for item in group {
                    for value in &item.defines {
                        if wanted(*value) {
                            graph.touch(*value);
                        }
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
                    if into.width == source.width && Some(source.width) == wide(copy.0) && wide(copy.0) == wide(copy.1)
                    {
                        equal = Some(copy.1);
                    }
                }
            }
            for value in &one.defines {
                if !wanted(*value) {
                    continue;
                }
                match &alive {
                    // The whole body: the pairs one at a time, as the tree is
                    // walked.
                    Alive::Tree(set) => {
                        for other in set {
                            if Some(*other) != equal && other != value && wanted(*other) {
                                graph.link(*value, *other);
                            }
                        }
                    }
                    Alive::Bits(_) => {
                        let near: Vec<u32> = alive
                            .iter()
                            .filter(|other| Some(*other) != equal && *other != *value && wanted(*other))
                            .collect();
                        graph.link_all(*value, &near);
                    }
                }
            }
            for value in &one.defines {
                alive.remove(*value);
            }
            alive.extend(one.uses.iter().copied().filter(|value| wanted(*value)));
            index -= 1;
        }
        for value in block.arrives() {
            if wanted(value) {
                match &alive {
                    Alive::Tree(set) => {
                        for other in set {
                            if *other != value && wanted(*other) {
                                graph.link(value, *other);
                            }
                        }
                    }
                    Alive::Bits(_) => {
                        let near: Vec<u32> = alive.iter().filter(|other| *other != value && wanted(*other)).collect();
                        graph.link_all(value, &near);
                    }
                }
            }
        }
    }
    graph.finish()
}

/// An interference graph as it is built: a row of bits for each value, indexed
/// by its number, so a pair costs two bit writes and no hash lookup, and the
/// graph's nodes in the order the first pair named them.
#[derive(Default)]
struct Building {
    rows: Vec<Neighbours>,
    seen: Vec<bool>,
    order: Vec<u32>,
    /// Rows are bits from the start (a few values asked of), not trees that
    /// turn to bits.
    bits: bool,
}

impl Building {
    fn bits() -> Self {
        Self { bits: true, ..Self::default() }
    }

    /// `value` next to each of `others`, in their order.
    fn link_all(
        &mut self,
        value: u32,
        others: &[u32],
    ) {
        if others.is_empty() {
            return;
        }
        if !self.bits {
            for other in others {
                self.link(value, *other);
            }
            return;
        }
        self.touch(value);
        let most = others.iter().copied().max().expect("not empty") as usize;
        let mut mask = vec![0u64; most / 64 + 1];
        for other in others {
            self.touch(*other);
            self.rows[*other as usize].insert(value);
            mask[*other as usize / 64] |= 1u64 << (*other % 64);
        }
        self.rows[value as usize].join(&mask, value);
    }

    /// Every value of `group` next to every other.
    fn link_group(
        &mut self,
        group: &[u32],
    ) {
        // A value alone is next to nothing, and is no node of the graph for it.
        if group.len() < 2 {
            return;
        }
        if !self.bits {
            for value in group {
                for other in group {
                    if value != other {
                        self.link(*value, *other);
                    }
                }
            }
            return;
        }
        let most = *group.iter().max().expect("not empty") as usize;
        let mut mask = vec![0u64; most / 64 + 1];
        for value in group {
            mask[*value as usize / 64] |= 1u64 << (*value % 64);
        }
        for value in group {
            self.touch(*value);
            self.rows[*value as usize].join(&mask, *value);
        }
    }

    fn touch(
        &mut self,
        value: u32,
    ) {
        let at = value as usize;
        if at >= self.rows.len() {
            if self.bits {
                self.rows.resize_with(at + 1, || Neighbours::Dense { words: Vec::new(), len: 0 });
            } else {
                self.rows.resize_with(at + 1, Neighbours::default);
            }
            self.seen.resize(at + 1, false);
        }
        if !self.seen[at] {
            self.seen[at] = true;
            self.order.push(value);
        }
    }

    fn link(
        &mut self,
        one: u32,
        other: u32,
    ) {
        self.touch(one);
        self.touch(other);
        self.rows[one as usize].insert(other);
        self.rows[other as usize].insert(one);
    }

    fn finish(mut self) -> Graph {
        self.order.iter().map(|value| (*value, std::mem::take(&mut self.rows[*value as usize]))).collect()
    }
}

/// One block's instructions, with a joined copy's bytes given away.
fn _kept(
    block: &LirBlock,
    swap: &dyn Fn(u32) -> u32,
) -> Vec<Arc<Insn>> {
    let identity = |one: &Arc<Insn>| _copy(one).is_some_and(|pair| pair.0 == pair.1);
    lir::without(&block.insns, identity, Some(|one: &Arc<Insn>| _renamed(one, swap)))
}

/// One interval covering both, which is what the joined value occupies.
pub fn _merged(
    one: &Interval,
    other: &Interval,
) -> Interval {
    // The shorter one's segments go into the longer, found by a search, and
    // the stretches between them are copied: a web that takes one value at a
    // time, N values, sorted N segments N times (`cases` 1024: 10.5 M segments
    // through the sort).
    let (big, small) = if one.segments.len() >= other.segments.len() {
        (&one.segments, &other.segments)
    } else {
        (&other.segments, &one.segments)
    };
    let mut out: Vec<Segment> = Vec::with_capacity(big.len() + small.len());
    let push = |out: &mut Vec<Segment>, seg: Segment| match out.last_mut() {
        Some(last) if seg.start <= last.end => *last = Segment { start: last.start, end: last.end.max(seg.end) },
        _ => out.push(seg),
    };
    // A stretch of the longer one, the segments of which are apart as a rule:
    // copied whole unless two touch.
    let run = |out: &mut Vec<Segment>, stretch: &[Segment]| {
        if stretch.windows(2).any(|pair| pair[1].start <= pair[0].end) {
            stretch.iter().for_each(|kept| push(out, *kept));
        } else if let Some((first, rest)) = stretch.split_first() {
            push(out, *first);
            // What the segment put in before reaches still joins the ones after
            // it; past that, they are apart and are copied.
            let mut joined = 0;
            while joined < rest.len() && out.last().is_some_and(|last| rest[joined].start <= last.end) {
                push(out, rest[joined]);
                joined += 1;
            }
            out.extend_from_slice(&rest[joined..]);
        }
    };
    let mut at = 0;
    for seg in small {
        let to = at + big[at..].partition_point(|b| (b.start, b.end) < (seg.start, seg.end));
        run(&mut out, &big[at..to]);
        push(&mut out, *seg);
        at = to;
    }
    run(&mut out, &big[at..]);
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
fn _wants(
    side: &[(Held, RegId)],
    swap: &dyn Fn(u32) -> u32,
) -> Vec<(Held, RegId)> {
    side.iter().map(|(held, r)| (Held { value: swap(held.value), width: held.width }, *r)).collect()
}

/// One instruction with every joined value naming its survivor.
pub(crate) fn _renamed(
    one: &Arc<Insn>,
    swap: &dyn Fn(u32) -> u32,
) -> Arc<Insn> {
    one.renamed(swap)
}

#[cfg(test)]
mod tests {
    //! Port of `tests/test_coalesce.py`.

    use std::collections::BTreeSet;
    use std::sync::Arc;

    use llrm_lir::registers::RegId;

    use super::{_interference, _merged, joined};
    use crate::analysis::intervals;
    use crate::backend::{allocate, cpu::ProfileOrName, select, target};
    use crate::model::ir::{self, Held, Imm, Loc, Mem, Operation, Reg, Semantics};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn semantics(
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
    ) -> Semantics {
        Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
    }

    fn held(
        value: u32,
        width: u32,
    ) -> Loc {
        Loc::Held(Held { value, width })
    }

    fn _move(
        at: i64,
        into: u32,
        out_of: u32,
    ) -> Insn {
        let what = semantics(Operation::Move, "mov", vec![held(into, 2)], vec![held(out_of, 2)]);
        Insn::new(at, Some((at, at + 2)), Some(what), vec![into], vec![out_of])
    }

    fn _use(
        at: i64,
        reads: u32,
    ) -> Insn {
        let what = semantics(Operation::Push, "push", vec![], vec![held(reads, 2)]);
        Insn::new(at, Some((at, at + 1)), Some(what), vec![], vec![reads])
    }

    fn _define(
        at: i64,
        makes: u32,
    ) -> Insn {
        let what = semantics(
            Operation::Move,
            "mov",
            vec![held(makes, 2)],
            vec![Loc::Imm(Imm { value: i64::from(makes), width: 2, address: None })],
        );
        Insn::new(at, Some((at, at + 3)), Some(what), vec![makes], vec![])
    }

    fn _jump(
        at: i64,
        to: i64,
    ) -> Insn {
        let what = Semantics { name: Some("jmp".to_owned()), target: Some(to), ..Semantics::new(Operation::Jump) };
        Insn::new(at, Some((at, at + 2)), Some(what), vec![], vec![])
    }

    fn body(
        name: &str,
        insns: Vec<Insn>,
        pins: &[(u32, RegId)],
    ) -> LirBody {
        LirBody::new(
            name,
            0,
            vec![LirBlock::new(0, insns.into_iter().map(Arc::new).collect()).into()],
            IndexMap::default(),
            pins.iter().copied().collect(),
        )
    }

    fn grouped(
        one: Insn,
        group: i64,
    ) -> Insn {
        Insn { group: Some(group), ..one }
    }

    fn allocated(
        body: &LirBody,
        pins: &IndexMap<u32, RegId>,
    ) -> allocate::Assignment {
        allocate::allocate(
            body,
            Some(pins),
            None,
            None,
            None,
            ProfileOrName::Name("386"),
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        )
        .expect("allocates")
    }

    /// HARR's hoisted selector copy became unencodable mov es,es across a
    /// coverage gap.
    #[test]
    fn test_retained_resource_identity_has_a_legal_encoding() {
        let body = body("resource-copy", vec![_move(3, 1, 1)], &[(1, RegId::ES)]);
        let result =
            allocate::applied(&body, &allocated(&body, &body.pins), &crate::backend::classes::RegisterClasses::m16())
                .expect("applies");
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

    /// `_merged` put the shorter interval's segments into the longer by search
    /// (a web taking one value at a time sorted all its segments for each, N^2
    /// at `cases` 1024). It must give what sorting and joining all the segments
    /// gives, whatever overlaps or touches: a wide segment put in swallows the
    /// segments after it that it reaches, touching ones included.
    #[test]
    fn test_merging_by_search_gives_what_sorting_the_segments_gives() {
        let reference = |one: &intervals::Interval, other: &intervals::Interval| {
            let mut runs: Vec<intervals::Segment> = one.segments.iter().chain(&other.segments).copied().collect();
            runs.sort_by_key(|x| (x.start, x.end));
            let mut out = vec![runs[0]];
            for seg in &runs[1..] {
                let last = out.last_mut().unwrap();
                if seg.start <= last.end {
                    *last = intervals::Segment { start: last.start, end: last.end.max(seg.end) };
                } else {
                    out.push(*seg);
                }
            }
            out
        };
        // Sorted segments that may touch each other, from a small generator.
        fn draw(
            seed: &mut u64,
            limit: u64,
        ) -> u64 {
            *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (*seed >> 33) % limit
        }
        fn made(
            seed: &mut u64,
            count: u64,
            reach: u64,
        ) -> Vec<intervals::Segment> {
            let mut at = draw(seed, 5) as i64;
            (0..count)
                .map(|_| {
                    let start = at + draw(seed, 4) as i64;
                    let end = start + 1 + draw(seed, reach) as i64;
                    at = end + draw(seed, 3) as i64;
                    intervals::Segment { start, end }
                })
                .collect()
        }
        let mut seed = 0x9E37_79B9_7F4A_7C15_u64;
        for round in 0..400 {
            let (long, short, reach) = (1 + draw(&mut seed, 12), 1 + draw(&mut seed, 4), 1 + draw(&mut seed, 30));
            let (a, b) = (made(&mut seed, long, 3), made(&mut seed, short, reach));
            let (one, other) = (intervals::Interval::new(1, a), intervals::Interval::new(2, b));
            assert_eq!(_merged(&one, &other).segments, reference(&one, &other), "round {round}");
            assert_eq!(_merged(&other, &one).segments, reference(&other, &one), "round {round} swapped");
        }
    }

    #[test]
    fn test_equal_resource_values_coalesce_without_consuming_a_gpr() {
        let mut load = _define(0, 1);
        load.what = Some(semantics(
            Operation::Move,
            "mov",
            vec![held(1, 2)],
            vec![Loc::Mem(Mem { through: RegId::BP, offset: 0, disp_width: 2, ..Mem::new(None, 2) })],
        ));
        let mut insns = vec![load];
        insns.extend((0..6).map(|index| _define(3 + index * 3, 10 + index as u32)));
        insns.extend([_move(21, 2, 1), _use(26, 1), _use(28, 2)]);
        insns.extend((0..6).map(|index| _use(30 + index * 3, 10 + index as u32)));
        let body = body("resources", insns, &[(1, RegId::ES), (2, RegId::ES)]);
        let done = joined(&body, None, &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16());
        assert_eq!(done.insns().len(), body.insns().len() - 1);
        let result = allocated(&done, &done.pins);
        assert!(result.spilled.is_empty());
        assert!(result.r#where.values().any(|register| *register == RegId::ES));
        let wholes: BTreeSet<RegId> = result.r#where.values().map(|register| allocate::_whole(*register)).collect();
        assert!(
            crate::backend::classes::RegisterClasses::m16().available.iter().all(|register| wholes.contains(register))
        );
    }

    #[test]
    fn test_resource_constraints_survive_coalescing() {
        for other in ["different_resource", "clobber"] {
            let mut insns = vec![_define(0, 1), _move(3, 2, 1), _use(6, 2)];
            let pins = [(1, RegId::ES), (2, if other == "different_resource" { RegId::FS } else { RegId::ES })];
            if other == "clobber" {
                let mut call = Insn::new(
                    5,
                    Some((5, 5)),
                    Some(semantics(Operation::Call, "call", vec![], vec![])),
                    vec![],
                    vec![],
                );
                call.clobbers = BTreeSet::from([RegId::ES]);
                insns.insert(2, call);
            }
            let body = body("resource-safety", insns, &pins);
            let done = joined(&body, None, &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16());
            assert_eq!(done.insns().len(), body.insns().len(), "{other}");
        }
    }

    #[test]
    fn test_a_copy_can_share_a_register_while_its_equal_source_is_still_read() {
        let body = body("equal", vec![_define(0, 1), _move(3, 2, 1), _use(5, 1), _use(6, 2)], &[]);
        let done = joined(&body, None, &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16());
        let insns = done.insns();
        assert_eq!(insns.len(), 3);
        assert_eq!(insns[2].uses, insns[1].uses);
    }

    #[test]
    fn test_a_source_redefined_while_its_copy_is_live_cannot_share() {
        let insns = vec![_define(0, 1), _move(3, 2, 1), _define(5, 1), _use(8, 1), _use(9, 2)];
        let count = insns.len();
        let done = joined(
            &body("different", insns, &[]),
            None,
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        );
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
        let insns = vec![_define(0, 1), grouped(_move(1, 2, 1), 1), grouped(_move(1, 3, 1), 1), _use(4, 2), _use(5, 3)];
        let body = body("parallel-destinations", insns, &[]);
        assert!(_interference(&body)[&2].contains(&3));
        let done = joined(&body, None, &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16());
        let insns = done.insns();
        assert_ne!(insns[insns.len() - 2].uses, insns[insns.len() - 1].uses);
    }

    /// Briggs's test refused a copy of a temporary that dies in it whenever
    /// more values were live than the class has registers (14 live of 4 in
    /// the case that found it, `d_faces`): the move stayed. Two live ranges
    /// that share no point leave the live count of every point as it was,
    /// so the join cannot make the body harder to colour.
    #[test]
    fn test_a_temporary_dying_in_a_copy_joins_though_more_values_are_live_than_registers() {
        // Eight values live while the temporary is, other eight while its copy
        // is: no neighbour of one is one of the other, so neither
        // George's test nor Briggs's accepted the join.
        let mut at = 0;
        let mut next = || {
            at += 10;
            at
        };
        let mut insns: Vec<Insn> = (10..18).map(|value| _define(next(), value)).collect();
        insns.push(_define(next(), 1));
        insns.extend((10..18).map(|value| _use(next(), value)));
        insns.push(_move(next(), 2, 1));
        insns.extend((20..28).map(|value| _define(next(), value)));
        insns.push(_use(next(), 2));
        insns.extend((20..28).map(|value| _use(next(), value)));
        let done = joined(
            &body("crowded", insns, &[]),
            None,
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        );
        let copies = done.insns().iter().filter(|one| super::_copy(one).is_some()).count();
        assert_eq!(copies, 0, "the copy of the dying temporary stays");
    }

    #[test]
    fn test_different_entry_values_cannot_share_even_if_copied_later() {
        let insns = vec![_use(0, 1), _use(1, 2), _move(2, 2, 1), _use(4, 2)];
        let count = insns.len();
        assert_eq!(
            joined(
                &body("inputs", insns, &[]),
                None,
                &target::BUILT_IN,
                &crate::backend::classes::RegisterClasses::m16()
            )
            .insns()
            .len(),
            count
        );
    }

    #[test]
    fn test_a_narrow_copy_is_not_equality_of_a_wide_source() {
        let mut wide = _use(5, 1);
        wide.what = Some(semantics(Operation::Push, "push", vec![], vec![held(1, 4)]));
        let insns = vec![_define(0, 1), _move(3, 2, 1), wide, _use(6, 2)];
        let count = insns.len();
        assert_eq!(
            joined(
                &body("partial", insns, &[]),
                None,
                &target::BUILT_IN,
                &crate::backend::classes::RegisterClasses::m16()
            )
            .insns()
            .len(),
            count
        );
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
        let done = joined(
            &body("address", insns, &[]),
            None,
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        );
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
                vec![Loc::st(0)],
                vec![Loc::Mem(Mem { base: Some(Held { value: 2, width: 2 }), ..Mem::new(None, 8) })],
            )),
            vec![],
            vec![2],
        );
        let body = body("pointer", vec![_define(0, 1), _move(3, 2, 1), load], &[]);
        let pins: IndexMap<u32, RegId> = IndexMap::from_iter([(1, RegId::EAX)]);
        assert_eq!(
            joined(&body, Some(&pins), &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16())
                .insns()
                .len(),
            3
        );
        let pinned = LirBody { pins: pins.clone(), ..body };
        assert_eq!(
            joined(&pinned, None, &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16()).insns().len(),
            3
        );
    }

    #[test]
    fn test_coalescing_keeps_the_pinned_return_as_representative() {
        let regs = crate::backend::registerinfo::test_regs();
        let body = body("return", vec![_define(0, 1), _move(3, 2, 1), _use(5, 2)], &[]);
        let done = joined(
            &body,
            Some(&IndexMap::from_iter([(2, RegId::EAX)])),
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        );
        let insns = done.insns();
        assert_eq!(insns[0].defines, vec![2]);
        assert_eq!(insns[insns.len() - 1].uses, vec![2]);
        for register in [RegId::EAX, RegId::EBX, RegId::ECX, RegId::EDX] {
            let pins = IndexMap::from_iter([(2, register)]);
            let joined =
                joined(&body, Some(&pins), &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16());
            let emitted = allocate::applied(
                &joined,
                &allocated(&joined, &pins),
                &crate::backend::classes::RegisterClasses::m16(),
            )
            .expect("applies");
            let insns = emitted.insns();
            assert_eq!(
                insns[insns.len() - 1].what.as_ref().expect("semantics").sources,
                vec![Loc::Reg(Reg { register: regs.named(register, 2), width: 2 })]
            );
        }
    }

    #[test]
    fn test_two_copies_into_one_value_leave_every_read_defined() {
        let arm = |at: i64, value: u32| {
            let mut block = LirBlock::new(
                at,
                vec![_define(at, value), _move(at + 3, 2, value), _jump(at + 5, 0x20)]
                    .into_iter()
                    .map(Arc::new)
                    .collect(),
            );
            block.succ = vec![0x20].into();
            block
        };
        let last = LirBlock::new(0x20, vec![Arc::new(_use(0x20, 2))]);
        let body = LirBody::new(
            "two arms",
            0,
            vec![arm(0, 61).into(), arm(0x10, 63).into(), last.into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let done = joined(&body, None, &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16());
        let made: BTreeSet<u32> = done.insns().iter().flat_map(|one| one.defines.clone()).collect();
        let read: BTreeSet<u32> = done.insns().iter().flat_map(|one| one.uses.clone()).collect();
        let missing: Vec<u32> = read.difference(&made).copied().collect();
        assert!(missing.is_empty(), "read with nothing defining it: {missing:?}");
    }

    #[test]
    fn test_a_pinned_neighbour_does_not_stop_georges_join() {
        for pinned in [false, true] {
            let long_lived: Vec<u32> = (10..18).collect();
            let mut insns: Vec<Insn> =
                long_lived.iter().enumerate().map(|(at, one)| _define(at as i64, *one)).collect();
            insns.extend([_define(0x10, 50), _define(0x13, 1), _move(0x16, 2, 1), _use(0x18, 2)]);
            insns.extend(long_lived.iter().chain([&50]).enumerate().map(|(at, one)| _use(0x20 + at as i64, *one)));
            let count = insns.len();
            let pins: Vec<(u32, RegId)> = if pinned { vec![(50, RegId::BX)] } else { vec![] };
            let body = body("counter", insns, &pins);
            assert_eq!(
                joined(&body, None, &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16()).insns().len(),
                count - 1,
                "pinned={pinned}"
            );
        }
    }
}

/// The values live at a point: bits while a few values are asked of (a web), a
/// tree for the whole body, where the values are many and a bit set is walked
/// word by word.
#[derive(Clone)]
enum Alive {
    Bits(Vec<u64>),
    Tree(BTreeSet<u32>),
}

impl Alive {
    fn of(
        bits: bool,
        values: impl IntoIterator<Item = u32>,
    ) -> Self {
        let mut alive = if bits { Self::Bits(Vec::new()) } else { Self::Tree(BTreeSet::new()) };
        alive.extend(values);
        alive
    }

    fn insert(
        &mut self,
        value: u32,
    ) {
        match self {
            Self::Bits(words) => {
                let at = value as usize / 64;
                if at >= words.len() {
                    words.resize(at + 1, 0);
                }
                words[at] |= 1u64 << (value % 64);
            }
            Self::Tree(set) => {
                set.insert(value);
            }
        }
    }

    fn remove(
        &mut self,
        value: u32,
    ) {
        match self {
            Self::Bits(words) => {
                if let Some(word) = words.get_mut(value as usize / 64) {
                    *word &= !(1u64 << (value % 64));
                }
            }
            Self::Tree(set) => {
                set.remove(&value);
            }
        }
    }

    /// The members, ascending.
    fn iter(&self) -> Box<dyn Iterator<Item = u32> + '_> {
        match self {
            Self::Bits(words) => Box::new(words.iter().enumerate().flat_map(|(at, word)| {
                let mut rest = *word;
                std::iter::from_fn(move || {
                    if rest == 0 {
                        return None;
                    }
                    let bit = rest.trailing_zeros();
                    rest &= rest - 1;
                    Some(at as u32 * 64 + bit)
                })
            })),
            Self::Tree(set) => Box::new(set.iter().copied()),
        }
    }

    fn extend(
        &mut self,
        values: impl IntoIterator<Item = u32>,
    ) {
        values.into_iter().for_each(|value| self.insert(value));
    }
}
