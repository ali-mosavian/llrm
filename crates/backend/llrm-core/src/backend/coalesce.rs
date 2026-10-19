//! Removes a copy whose two values can share a register: the phase that runs
//! LLVM's `RegisterCoalescer` ([`crate::coalescer`]), and the interference
//! graph `twoaddr` and the spiller read.
//!
//! # How it works
//! After `phielim` and `twoaddr` the body is full of `mov v2, v1` between
//! values. If the live intervals of `v1` and `v2` can be joined (the values
//! they hold agree wherever both are live), they are one value and the move
//! is an identity that disappears:
//!
//! ```text
//!  before                          after
//!    v1 = load ...                   v1 = load ...
//!    v2 = mov v1       (no clash)    ... v1 ...     v2 renamed v1
//!    ... v2 ...                      the mov is gone
//! ```
//!
//! The phase builds the intervals ([`crate::liveness::liveintervals`]) and
//! hands them to [`crate::coalescer`] with the target's answers (which values
//! may share a register: the palettes and the pins). Joining is by interval
//! and leaves colouring to the allocator, which splits where it must; the old
//! pass here (Briggs and George over an interference graph, with a call-clobber
//! check) is gone, objects unchanged on the corpora the day it went. A body
//! with phis is not coalesced.
//!
//! What stays in this file is the interference graph: `_interference` (built by
//! walking each block backwards: values live at the same point are adjacent,
//! the two ends of a copy not by that copy, and the sources of a parallel copy
//! before any of its destinations is written) and `_interference_among`, read
//! by `twoaddr.rs` and by the spiller's sibling pricing; `_merged` and
//! `_copy`.
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
//! # See also
//! LLVM's `RegisterCoalescer`:
//! <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp>

use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::analysis::intervals::{self as ranges, Interval, Segment};
use crate::backend::classes::RegisterClasses;
use crate::backend::neighbours::Neighbours;
use crate::backend::target::{self, Segments};
use crate::backend::{allocate, regclass};
use crate::model::ir::{self, Loc, Operation};
use crate::model::lir::{Insn, LirBody};
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
        Ok(match self.llvm_env(&body) {
            Some((env, depth)) => crate::coalescer::env::joined(&body, &env, &depth),
            // A body with phis is not translated: phi elimination runs first.
            None => body,
        })
    }
}

impl Coalescer {
    /// What the translated coalescer asks of the target for `body`: the
    /// registers each value may take and the ones that are fixed. None where
    /// it does not apply (a body with phis).
    fn llvm_env(
        &self,
        body: &LirBody,
    ) -> Option<(crate::coalescer::env::TargetEnv, Vec<u32>)> {
        if body.blocks.iter().any(|block| !block.phis.is_empty()) {
            return None;
        }
        let mut pins = body.pins.clone();
        pins.extend(self.pinned.iter().map(|(value, register)| (*value, *register)));
        let pins: IndexMap<u32, RegId> =
            pins.into_iter().map(|(value, register)| (value, ir::root(register))).collect();
        let where_of = regclass::classes(body, &BTreeSet::new(), &self.segments, &self.classes);
        let everything: BTreeSet<RegId> = self.classes.available.iter().copied().collect();
        let mut palettes: IndexMap<u32, BTreeSet<RegId>> = IndexMap::default();
        for block in &body.blocks {
            for one in block.insns.iter() {
                for value in one.defines.iter().chain(&one.uses) {
                    palettes
                        .entry(*value)
                        .or_insert_with(
                            || target::order(where_of.get(value), &self.segments, &self.classes).into_iter().collect(),
                        );
                }
            }
        }
        for (value, register) in &pins {
            if !everything.contains(register) && !where_of.contains_key(value) {
                palettes.insert(*value, BTreeSet::from([*register]));
            }
        }
        let depths = ranges::depths_shared(body);
        let depth = body.blocks.iter().map(|block| depths.get(&block.at).copied().unwrap_or(0)).collect();
        Some((crate::coalescer::env::TargetEnv { palettes, pins }, depth))
    }
}

pub type Graph = IndexMap<u32, Neighbours>;

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

    use super::{_interference, _merged};
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
