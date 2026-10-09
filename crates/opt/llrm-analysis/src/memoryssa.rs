//! LLVM's `MemorySSA`: llrm-core's `analysis/memoryssa.rs`, adapted to the
//! rich MIR. Conservative, rebuilt after a function changes.
//!
//! Stores and writing calls define a single memory state; loads and
//! reading calls use it. The clobber walker skips writes
//! `may_clobber` rules out. What each instruction touches is `Accesses`'.
//!
//! A site is an instruction. Dropped, with no rich MIR counterpart: the x87
//! writes (`floating`, `Fcheck`), each query's `dgroup` (foreign memory is
//! `Unit::machine`'s), the `operations` map (a site's instruction is the
//! function's) and `Kind::value` (Python's repr).

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use crate::graph::loops;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::memory::stated;
use llrm_mir::module::{Function, GlobalValue, InstId, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Analyses, Analysis};
use llrm_support::hash::IndexMap;

use crate::cfg;
use crate::alias::{self, Effect, Procedure, Summary};
use crate::consts::Calls;
use crate::manager::{Annotated, CallEffects};
use crate::memory::{MemRef, Unit, own_bytes, unmodeled_write};
use crate::pointerfacts::{self, Location};
use crate::ranges::Interval;
use crate::regions::{displaced_span, overlapping};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Kind {
    Live,
    Use,
    Def,
    Phi,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Access {
    pub id: usize,
    pub kind: Kind,
    pub block: Option<i64>,
    pub site: Option<InstId>,
    pub defining: Option<usize>,
    // None denotes the invocation edge into the entry block.
    pub incoming: Vec<(Option<i64>, usize)>,
}

impl Access {
    fn new(id: usize, kind: Kind) -> Self {
        Self { id, kind, block: None, site: None, defining: None, incoming: Vec::new() }
    }
}

/// What an instruction reads and what it writes; `None` for anything.
type Footprint = (Option<Rc<[MemRef]>>, Option<Rc<[MemRef]>>);

/// What each instruction reads and writes, as MemorySSA and its clients
/// ask: old `Op.loads` and `Op.stores`. A load's or store's bytes are its
/// reference; a call's are its footprint, else anything its attributes
/// allow.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Accesses {
    /// Each load's and store's reference.
    pub references: IndexMap<InstId, MemRef>,
    /// Of each instruction touching memory, what it reads and what it
    /// writes; `None` for anything.
    touched: IndexMap<InstId, Footprint>,
    /// Of each call, the bytes it writes before reading any.
    fills: IndexMap<InstId, Vec<MemRef>>,
}

impl Analysis for Accesses {
    type Result = Result<Rc<Accesses>, String>;
    const NAME: &'static str = "accesses";

    fn run(context: &Context, layout: &DataLayout, function: &Function, analyses: &mut Analyses) -> Self::Result {
        let references = analyses.get::<Annotated>(context, layout, function);
        let effects = analyses.get::<CallEffects>(context, layout, function);
        let references = Result::as_ref(&*references).map_err(String::clone)?;
        let effects = Result::as_ref(&*effects).map_err(String::clone)?;
        let shape = analyses.get::<crate::cfg::Shape>(context, layout, function);
        let exposed = analyses.get::<crate::manager::ExposedFrames>(context, layout, function);
        Ok(Rc::new(Self::of(&Unit::within(context, layout, function, analyses.outer()).with_shape(&shape).with_exposed(&exposed), references.clone(), effects)))
    }
}

impl Accesses {
    /// `unit`'s accesses as `alias` resolves them: each reference with its
    /// provenance, each call's effect instantiated from `known` callees.
    pub fn resolved(unit: &Unit, known: &IndexMap<String, Summary>) -> Result<Self, String> {
        Ok(Self::of(unit, unit.annotated()?.into_owned(), &alias::calls_annotated(&Procedure::of(*unit), known)?))
    }

    /// `function`'s accesses, the manager's: from its `Annotated` and
    /// `CallEffects`; the pipeline must require `Summaries`.
    pub fn managed(context: &Context, layout: &DataLayout, function: &Function, analyses: &mut Analyses) -> Result<Rc<Self>, String> {
        (*analyses.get::<Accesses>(context, layout, function)).clone()
    }

    /// `unit`'s accesses from `references` (`alias::annotated`'s) and each
    /// call's `effects` (`alias::calls_annotated`'s).
    pub fn of(unit: &Unit, references: IndexMap<InstId, MemRef>, effects: &IndexMap<InstId, Effect>) -> Self {
        let fills = effects.iter().filter(|(_, effect)| !effect.fills.is_empty()).map(|(&at, effect)| (at, effect.fills.clone())).collect();
        Self { fills, ..Self::new(unit, references, |inst| effects.get(&inst).map(|effect| (Some(Rc::clone(&effect.loads)), Some(Rc::clone(&effect.stores))))) }
    }

    /// `unit`'s accesses unresolved: each reference as the unit has it
    /// (`Unit::reference`), each call writing its footprint in `calls`.
    pub fn plain(unit: &Unit, calls: &Calls) -> Self {
        let references = unit.function.walk().filter_map(|(_, inst)| unit.reference(inst).map(|one| (inst, one))).collect();
        Self::new(unit, references, |inst| calls.get(&inst).map(|stores| (None, Some(Rc::clone(stores)))))
    }

    /// `footprint` gives a call's reads and writes, where known.
    fn new(unit: &Unit, references: IndexMap<InstId, MemRef>, footprint: impl Fn(InstId) -> Option<Footprint>) -> Self {
        let function = unit.function;
        let mut touched = IndexMap::default();
        for (_, inst) in function.walk() {
            let reference = || references.get(&inst).cloned().into_iter().collect::<Vec<_>>();
            let opcode = &function.instruction(inst).opcode;
            let found = match opcode {
                // Its order against other volatile accesses is the passes', which never move one.
                _ if let Some(own) = own_bytes(opcode) => {
                    let touched = |does: bool| Some(if does { Rc::from(reference()) } else { Rc::from([]) });
                    (touched(own.reads), touched(own.writes))
                }
                Opcode::Call(info) | Opcode::Invoke(info) => {
                    let callee = llrm_mir::memory::callee(unit.context, function, inst).and_then(|one| unit.globals.get(one.0 as usize)).and_then(GlobalValue::function);
                    let reading = stated(&info.attrs).reads && callee.is_none_or(|one| stated(&one.attrs).reads);
                    let (reads, writes) = footprint(inst).unwrap_or((None, None));
                    (if reading { reads } else { Some(Rc::from([])) }, if unmodeled_write(unit, inst) { writes } else { Some(Rc::from([])) })
                }
                _ => continue,
            };
            touched.insert(inst, found);
        }
        Self { references, touched, fills: IndexMap::default() }
    }

    /// What `inst` writes; `None` where it may write anything.
    pub fn writes(&self, inst: InstId) -> Option<&[MemRef]> {
        self.touched.get(&inst).map_or(Some(&[]), |(_, writes)| writes.as_deref())
    }

    /// What the call `inst` writes before reading any: `initializes`.
    pub fn fills(&self, inst: InstId) -> &[MemRef] {
        self.fills.get(&inst).map_or(&[], Vec::as_slice)
    }

    /// What `inst` reads; `None` where it may read anything.
    pub fn reads(&self, inst: InstId) -> Option<&[MemRef]> {
        self.touched.get(&inst).map_or(Some(&[]), |(reads, _)| reads.as_deref())
    }
}

fn located(reference: &MemRef) -> Option<Location> {
    reference.pointer.map(|pointer| Location { pointer, bytes: u64::from(reference.width) })
}

/// Whether `writes`, what an instruction writes (`None`: anything), may
/// change a byte of `cell`, each write asked `clobbers`: the one answer to
/// it, so that the rules hold everywhere. Nothing changes a constant
/// object, nor what an `invariant` read reads (`memory::invariant_load`).
pub fn changes(cell: &MemRef, invariant: bool, writes: Option<&[MemRef]>, clobbers: impl Fn(&MemRef) -> bool) -> bool {
    !invariant && !cell.unwritable() && writes.is_none_or(|stores| stores.iter().any(clobbers))
}

/// Whether running `write` leaves the bytes `read` reads as they were: it writes none of them, as `accesses` says
/// and `regions::overlapping` decides on `program`; an answer it cannot give overlaps.
pub fn spares(accesses: &Accesses, program: Option<&llrm_mir::program::ProgramProxy>, read: &MemRef, write: InstId) -> bool {
    let overlaps = |wrote: &MemRef| overlapping(read, wrote, None, None, program).unwrap_or(true);
    !changes(read, false, accesses.writes(write), overlaps)
}

/// Whether writing `store` may change a byte of `cell`: `regions` leaves
/// it open and `pointerfacts` cannot place them apart.
pub fn may_clobber(unit: &Unit, known: Option<&BTreeMap<ValueId, Interval>>, cell: &MemRef, store: &MemRef) -> bool {
    let offsets = pointerfacts::offsets(unit.context, unit.layout, unit.function);
    let apart = matches!((located(cell), located(store)), (Some(one), Some(other)) if offsets.disjoint(one, other));
    // An answer Rust cannot represent is taken to overlap.
    overlapping(cell, store, known, known, unit.program).unwrap_or(true) && !apart
}

/// Whether `outer` certainly holds every byte of `inner`: both at fixed
/// displacements in one frame (`regions`), or at constant offsets from one
/// pointer (`pointerfacts`).
pub fn covers(unit: &Unit, outer: &MemRef, inner: &MemRef) -> bool {
    covered(unit, &[outer], inner)
}

/// Whether `outers` together certainly hold every byte of `inner`, as
/// LLVM's DSE merges the intervals later stores overwrite.
pub fn covered(unit: &Unit, outers: &[&MemRef], inner: &MemRef) -> bool {
    let Some(size) = placed(unit, inner, inner).map(|(low, high)| high - low) else {
        return false;
    };
    let mut spans: Vec<(i128, i128)> = outers.iter().filter_map(|outer| placed(unit, outer, inner)).collect();
    spans.sort_unstable();
    let mut reached = 0;
    for (low, high) in spans {
        if low > reached {
            break;
        }
        reached = reached.max(high);
    }
    reached >= size
}

/// `outer`'s bytes, counted from `inner`'s first, where both are placed:
/// at fixed displacements in one frame (`regions`), or at constant offsets
/// from one pointer (`pointerfacts`).
pub fn placed(unit: &Unit, outer: &MemRef, inner: &MemRef) -> Option<(i128, i128)> {
    if let (Some((frame, low, high)), Some((inner_frame, inner_low, _))) = (displaced_span(outer), displaced_span(inner))
        && frame == inner_frame
    {
        return Some((low - inner_low, high - inner_low));
    }
    let offsets = pointerfacts::offsets(unit.context, unit.layout, unit.function);
    let (one, other) = (located(outer)?, located(inner)?);
    offsets.comparable(one, other).map(|(low, inner_low)| (i128::from(low - inner_low), i128::from(low - inner_low) + i128::from(one.bytes)))
}

/// Whether `one` and `other` certainly name the same bytes.
pub fn same_bytes(unit: &Unit, one: &MemRef, other: &MemRef) -> bool {
    covers(unit, one, other) && covers(unit, other, one)
}

thread_local! {
    static RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has asked whether a def's writes may clobber a cell, rather than been
/// answered from memory.
pub fn clobber_runs() -> usize {
    RUNS.with(std::cell::Cell::get)
}

/// `MemorySSA::span`.
fn spans(accesses: &[Access]) -> Vec<(u32, u32)> {
    let top = accesses.last().map_or(0, |last| last.id) + 1;
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); top];
    let mut roots = Vec::new();
    for access in accesses {
        match access.defining {
            Some(parent) => children[parent].push(access.id),
            None => roots.push(access.id),
        }
    }
    let mut span = vec![(0, 0); top];
    let mut clock = 0;
    for root in roots {
        let mut stack = vec![(root, 0usize)];
        span[root].0 = clock;
        clock += 1;
        while let Some((node, next)) = stack.pop() {
            if let Some(&child) = children[node].get(next) {
                stack.push((node, next + 1));
                span[child].0 = clock;
                clock += 1;
                stack.push((child, 0));
            } else {
                span[node].1 = clock;
            }
        }
    }
    span
}

fn check_jumps() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("LLRM_CHECK_JUMPS").is_some())
}

fn check_clobbers() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("LLRM_CHECK_CLOBBERS").is_some())
}

#[derive(Clone)]
pub struct MemorySSA<'a> {
    pub live: Access,
    pub accesses: Vec<Access>,
    pub sites: IndexMap<InstId, Access>,
    pub phis: IndexMap<i64, Access>,
    /// What each def writes, as `Accesses::writes` says.
    written: IndexMap<InstId, Option<Vec<MemRef>>>,
    unit: Unit<'a>,
    /// Whether a def's writes may clobber a cell, once for each pair: the loads of one address ask it of
    /// the same defs again and again.
    clobbers: std::cell::RefCell<llrm_support::hash::HashMap<MemRef, llrm_support::hash::HashMap<InstId, bool>>>,
    /// For a cell and whether the load is invariant: from an access that is a use or a def that leaves the cell
    /// alone, the access the walk back reaches before a clobber, a join, the live state or a boundary it was stopped at (plus one; 0: not
    /// known), by access number. A walk takes the jump instead of the steps, unless its own boundary lies between.
    jumps: std::cell::RefCell<llrm_support::hash::HashMap<(MemRef, bool), Vec<u32>>>,
    /// Each access's entry and exit in a depth-first order of the tree its `defining` links make, by access number: one is behind another
    /// when its interval holds the other's.
    span: Vec<(u32, u32)>,
}

impl MemorySSA<'_> {
    pub fn at(&self, site: InstId) -> &Access {
        &self.sites[&site]
    }

    /// The access numbered `id`: `accesses` is sorted by id.
    pub fn access(&self, id: usize) -> &Access {
        &self.accesses[self.accesses.binary_search_by_key(&id, |access| access.id).expect("a numbered access")]
    }

    /// Possible nearest writes before a site, including live-on-entry.
    ///
    /// Walk every phi input. A visited set closes cycles without treating
    /// the backedge as evidence that memory is unchanged. An empty result
    /// means no reachable source was found, not a reusable memory value.
    /// This identifies memory states, not a dominating scalar definition;
    /// forwarding consumers must establish value availability separately.
    pub fn clobbers(&self, site: InstId, memory: &MemRef) -> BTreeSet<usize> {
        self.frontier(site, memory, None, None, None)
    }

    /// Whether a dominating earlier read's memory state still applies.
    ///
    /// The caller must establish dominance and equal addresses. Stop at
    /// the earlier memory version, rejecting any possibly aliasing write
    /// on the way, including writes carried by loop backedges.
    pub fn unchanged(&self, earlier: InstId, later: InstId, memory: &MemRef) -> bool {
        let boundary = self.at(earlier).defining;
        boundary.is_some_and(|boundary| self.frontier(later, memory, Some(boundary), None, None) == BTreeSet::from([boundary]))
    }

    /// Whether any write of the def at `site` may change a byte of `cell`, remembered.
    fn clobbered(&self, cell: &MemRef, site: InstId) -> bool {
        if let Some(&known) = self.clobbers.borrow().get(cell).and_then(|sites| sites.get(&site)) {
            if !check_clobbers() {
                return known;
            }
        }
        let stores = self.written[&site].as_deref().unwrap_or(&[]);
        let found = stores.iter().any(|store| may_clobber(&self.unit, None, cell, store));
        RUNS.with(|runs| runs.set(runs.get() + 1));
        let mut memo = self.clobbers.borrow_mut();
        let sites = memo.entry(cell.clone()).or_default();
        if let Some(&known) = sites.get(&site) {
            assert_eq!(known, found, "LLRM_CHECK_CLOBBERS: a remembered answer differs from the one worked out");
        }
        sites.insert(site, found);
        found
    }

    fn frontier(&self, site: InstId, memory: &MemRef, boundary: Option<usize>, edge: Option<i64>, edge_memory: Option<&MemRef>) -> BTreeSet<usize> {
        let found = self.walked(site, memory, boundary, edge, edge_memory, true);
        if check_jumps() {
            assert_eq!(found, self.walked(site, memory, boundary, edge, edge_memory, false), "LLRM_CHECK_JUMPS: a walk that jumps found other than a walk step by step");
        }
        found
    }

    /// Whether `ahead` is `access` or reached from it by `defining` links.
    fn behind(&self, access: usize, ahead: usize) -> bool {
        let (entered, left) = self.span[ahead];
        let (at, _) = self.span[access];
        entered <= at && at < left
    }

    /// `frontier`; `jumping` takes the remembered jumps along chains of accesses that leave the cell alone, or none.
    fn walked(&self, site: InstId, memory: &MemRef, boundary: Option<usize>, edge: Option<i64>, edge_memory: Option<&MemRef>, jumping: bool) -> BTreeSet<usize> {
        let block = self.at(site).block;
        // A load of what is written once, then never: no write changes what it reads.
        let invariant = llrm_mir::memory::invariant_load(self.unit.context, self.unit.layout, self.unit.function, site);
        let mut pending = vec![self.at(site).defining];
        let mut seen = BTreeSet::new();
        let mut found = BTreeSet::new();
        // Where no edge chooses, the chain of uses and untouching defs back from an access is the same for every load of the cell: its
        // end is found once and jumped to (LLVM's walker caches its clobber the same way).
        let jumpable = jumping && edge.is_none() && edge_memory.is_none();
        let mut jumps = self.jumps.borrow_mut();
        let none = &mut Vec::new();
        let table = if jumpable { jumps.entry((memory.clone(), invariant)).or_insert_with(|| vec![0; self.span.len()]) } else { none };
        // Accesses passed that nothing on them touches the cell; a jump to `end + 1` says the ones before `end` are clean.
        let mut chain: Vec<usize> = Vec::new();
        let ended = |table: &mut Vec<u32>, chain: &mut Vec<usize>, end: usize| {
            for &passed in chain.iter() {
                table[passed] = end as u32 + 1;
            }
            chain.clear();
        };
        while let Some(current) = pending.pop() {
            let Some(current) = current else {
                chain.clear();
                continue;
            };
            if jumpable && table[current] != 0 {
                chain.clear();
                let end = table[current] as usize - 1;
                // The boundary lies on the chain when it is behind `current` and `end` is behind it: the walk stops there.
                match boundary {
                    Some(stop) if stop != end && self.behind(current, stop) && self.behind(stop, end) => {
                        found.insert(stop);
                    }
                    _ => pending.push(Some(end)),
                }
                continue;
            }
            if !seen.insert(current) {
                chain.clear();
                continue;
            }
            if Some(current) == boundary {
                ended(table, &mut chain, current);
                found.insert(current);
                continue;
            }
            let access = self.access(current);
            match access.kind {
                Kind::Live => {
                    ended(table, &mut chain, current);
                    found.insert(current);
                }
                Kind::Phi => {
                    ended(table, &mut chain, current);
                    pending.extend(
                        access
                            .incoming
                            .iter()
                            .filter(|(parent, _)| edge.is_none() || access.block != block || *parent == edge)
                            .map(|(_, value)| Some(*value)),
                    )
                }
                Kind::Def => {
                    let queried = match edge_memory {
                        Some(edge_memory) if access.block != block => edge_memory,
                        _ => memory,
                    };
                    let written = &self.written[&access.site.expect("a def has a site")];
                    let site = access.site.expect("a def has a site");
                    if changes(queried, invariant, written.as_deref(), |_| self.clobbered(queried, site)) {
                        ended(table, &mut chain, current);
                        found.insert(current);
                    } else {
                        if jumpable {
                            chain.push(current);
                        }
                        pending.push(access.defining);
                    }
                }
                Kind::Use => {
                    if jumpable {
                        chain.push(current);
                    }
                    pending.push(access.defining);
                }
            }
        }
        found
    }

    /// An earlier load/store still supplies these bytes on one incoming edge.
    ///
    /// The caller proves equal addresses and that the scalar provider dominates
    /// the predecessor. Only the destination's memory phi is edge-selected;
    /// other intervening joins still require agreement along every path.
    /// A translated address applies outside the destination; its prefix must
    /// still be checked against the original phi-based address.
    pub fn available_on_edge(&self, earlier: InstId, later: InstId, predecessor: i64, memory: &MemRef, edge_memory: Option<&MemRef>) -> bool {
        let source = self.at(earlier);
        let boundary = if source.kind == Kind::Def { Some(source.id) } else { source.defining };
        boundary.is_some_and(|boundary| self.frontier(later, memory, Some(boundary), Some(predecessor), edge_memory) == BTreeSet::from([boundary]))
    }
}

/// Wire block entries first, then eliminate identity memory phis.
///
/// Preallocating entries handles backedges without iterative guesses about
/// memory versions. A block no edge enters keeps an invocation input.
pub fn built<'a>(unit: &Unit<'a>, accesses: &Accesses) -> MemorySSA<'a> {
    let function = unit.function;
    let graph = cfg::graph(function);
    let entry = function.entry().map(cfg::id);
    let live = Access::new(0, Kind::Live);
    let entries: IndexMap<i64, usize> = graph.iter().enumerate().map(|(index, block)| (block.at, index + 1)).collect();
    let mut sites: IndexMap<InstId, Access> = IndexMap::default();
    let mut written: IndexMap<InstId, Option<Vec<MemRef>>> = IndexMap::default();
    let mut outgoing: BTreeMap<i64, usize> = BTreeMap::new();
    let mut next_id = entries.len() + 1;
    for block in &graph {
        let mut current = entries[&block.at];
        for &inst in function.block(cfg::block(block.at)).instructions() {
            let writes = accesses.writes(inst).map(<[MemRef]>::to_vec);
            let defines = writes.as_ref().is_none_or(|stores| !stores.is_empty());
            if !defines && accesses.reads(inst).is_some_and(<[MemRef]>::is_empty) {
                continue;
            }
            let kind = if defines { Kind::Def } else { Kind::Use };
            sites.insert(inst, Access { id: next_id, kind, block: Some(block.at), site: Some(inst), defining: Some(current), incoming: Vec::new() });
            if defines {
                written.insert(inst, writes);
                current = next_id;
            }
            next_id += 1;
        }
        outgoing.insert(block.at, current);
    }

    let predecessors = loops::predecessors(&graph);
    let incoming: BTreeMap<i64, Vec<(Option<i64>, usize)>> = graph
        .iter()
        .map(|block| {
            let parents = &predecessors[&block.at];
            let mut edges: Vec<(Option<i64>, usize)> = parents.iter().map(|pred| (Some(*pred), outgoing[pred])).collect();
            if Some(block.at) == entry || parents.is_empty() {
                edges.push((None, live.id));
            }
            (block.at, edges)
        })
        .collect();
    let mut replacements: BTreeMap<usize, usize> = BTreeMap::new();

    let resolved = |replacements: &BTreeMap<usize, usize>, mut value: usize| -> usize {
        while let Some(next) = replacements.get(&value) {
            value = *next;
        }
        value
    };

    let mut changed = true;
    while changed {
        changed = false;
        for (block, entry) in &entries {
            if replacements.contains_key(entry) {
                continue;
            }
            let mut values: BTreeSet<usize> = incoming[block].iter().map(|(_, value)| resolved(&replacements, *value)).collect();
            values.remove(entry);
            if values.len() <= 1 {
                replacements.insert(*entry, values.iter().next().copied().unwrap_or(live.id));
                changed = true;
            }
        }
    }

    let phis: IndexMap<i64, Access> = entries
        .iter()
        .filter(|(_, entry)| !replacements.contains_key(entry))
        .map(|(block, entry)| {
            let mut access = Access::new(*entry, Kind::Phi);
            access.block = Some(*block);
            access.incoming = incoming[block].iter().map(|(pred, value)| (*pred, resolved(&replacements, *value))).collect();
            (*block, access)
        })
        .collect();
    let sites: IndexMap<InstId, Access> = sites
        .into_iter()
        .map(|(site, access)| {
            let defining = access.defining.map(|value| resolved(&replacements, value));
            (site, Access { defining, ..access })
        })
        .collect();
    let mut accesses: Vec<Access> = std::iter::once(live.clone()).chain(phis.values().cloned()).chain(sites.values().cloned()).collect();
    accesses.sort_by_key(|access| access.id);
    let span = spans(&accesses);
    MemorySSA { live, accesses, sites, phis, written, unit: *unit, clobbers: Default::default(), jumps: Default::default(), span }
}

#[cfg(test)]
#[path = "memoryssa_tests.rs"]
mod tests;
