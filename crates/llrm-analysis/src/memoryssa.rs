//! LLVM's `MemorySSA`: llrm-core's `analysis/memoryssa.rs`, adapted to the
//! rich MIR. Conservative, rebuilt after a function changes.
//!
//! Stores, writing calls and volatile accesses define a single memory
//! state; loads and reading calls use it. The clobber walker skips writes
//! `may_clobber` rules out. What each instruction touches is `Accesses`'.
//!
//! A site is an instruction. Dropped, with no rich MIR counterpart: the x87
//! writes (`floating`, `Fcheck`), each query's `dgroup` (foreign memory is
//! `Unit::machine`'s), the `operations` map (a site's instruction is the
//! function's) and `Kind::value` (Python's repr).

use std::collections::{BTreeMap, BTreeSet};

use llrm_graph::loops;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::memory::stated;
use llrm_mir::module::{Function, GlobalValue, InstId, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::Analyses;
use llrm_support::hash::IndexMap;

use crate::cfg;
use crate::alias::{self, Effect, Procedure, Summary};
use crate::consts::Calls;
use crate::manager::{Annotated, CallEffects};
use crate::memory::{MemRef, Unit, unmodeled_write};
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
type Footprint = (Option<Vec<MemRef>>, Option<Vec<MemRef>>);

/// What each instruction reads and writes, as MemorySSA and its clients
/// ask: old `Op.loads` and `Op.stores`. A load's or store's bytes are its
/// reference; a call's are its footprint, else anything its attributes
/// allow.
#[derive(Clone, Debug, Default)]
pub struct Accesses {
    /// Each load's and store's reference.
    pub references: IndexMap<InstId, MemRef>,
    /// Of each instruction touching memory, what it reads and what it
    /// writes; `None` for anything.
    touched: IndexMap<InstId, Footprint>,
}

impl Accesses {
    /// `unit`'s accesses as `alias` resolves them: each reference with its
    /// provenance, each call's effect instantiated from `known` callees.
    pub fn resolved(unit: &Unit, known: &IndexMap<String, Summary>) -> Result<Self, String> {
        Ok(Self::of(unit, alias::annotated(unit)?, &alias::calls_annotated(&Procedure::of(*unit), known)?))
    }

    /// `function`'s accesses from the manager's `Annotated` and
    /// `CallEffects`; the pipeline must require `Summaries`.
    pub fn managed(context: &Context, layout: &DataLayout, function: &Function, analyses: &mut Analyses) -> Result<Self, String> {
        let references = analyses.get::<Annotated>(context, layout, function);
        let effects = analyses.get::<CallEffects>(context, layout, function);
        let references = Result::as_ref(&*references).map_err(String::clone)?;
        let effects = Result::as_ref(&*effects).map_err(String::clone)?;
        Ok(Self::of(&Unit::within(context, layout, function, analyses.outer()), references.clone(), effects))
    }

    /// `unit`'s accesses from `references` (`alias::annotated`'s) and each
    /// call's `effects` (`alias::calls_annotated`'s).
    pub fn of(unit: &Unit, references: IndexMap<InstId, MemRef>, effects: &IndexMap<InstId, Effect>) -> Self {
        Self::new(unit, references, |inst| effects.get(&inst).map(|effect| (Some(effect.loads.clone()), Some(effect.stores.clone()))))
    }

    /// `unit`'s accesses unresolved: each reference as the instruction
    /// spells it, each call writing its footprint in `calls`.
    pub fn plain(unit: &Unit, calls: &Calls) -> Self {
        let references = unit.function.walk().filter_map(|(_, inst)| MemRef::of(unit, inst).map(|one| (inst, one))).collect();
        Self::new(unit, references, |inst| calls.get(&inst).map(|stores| (None, Some(stores.clone()))))
    }

    /// `footprint` gives a call's reads and writes, where known.
    fn new(unit: &Unit, references: IndexMap<InstId, MemRef>, footprint: impl Fn(InstId) -> Option<Footprint>) -> Self {
        let function = unit.function;
        let mut touched = IndexMap::default();
        for (_, inst) in function.walk() {
            let reference = || references.get(&inst).cloned().into_iter().collect::<Vec<_>>();
            let found = match &function.instruction(inst).opcode {
                Opcode::Load { volatile: false, .. } => (Some(reference()), Some(Vec::new())),
                Opcode::Store { volatile: false, .. } => (Some(Vec::new()), Some(reference())),
                Opcode::Load { .. } | Opcode::Store { .. } => (None, None),
                Opcode::Call(info) | Opcode::Invoke(info) => {
                    let callee = llrm_mir::memory::callee(unit.context, function, inst).and_then(|one| unit.globals.get(one.0 as usize)).and_then(GlobalValue::function);
                    let reading = stated(&info.attrs).reads && callee.is_none_or(|one| stated(&one.attrs).reads);
                    let (reads, writes) = footprint(inst).unwrap_or((None, None));
                    (if reading { reads } else { Some(Vec::new()) }, if unmodeled_write(unit, inst) { writes } else { Some(Vec::new()) })
                }
                _ => continue,
            };
            touched.insert(inst, found);
        }
        Self { references, touched }
    }

    /// What `inst` writes; `None` where it may write anything.
    pub fn writes(&self, inst: InstId) -> Option<&[MemRef]> {
        self.touched.get(&inst).map_or(Some(&[]), |(_, writes)| writes.as_deref())
    }

    /// What `inst` reads; `None` where it may read anything.
    pub fn reads(&self, inst: InstId) -> Option<&[MemRef]> {
        self.touched.get(&inst).map_or(Some(&[]), |(reads, _)| reads.as_deref())
    }
}

fn located(reference: &MemRef) -> Option<Location> {
    reference.pointer.map(|pointer| Location { pointer, bytes: u64::from(reference.width) })
}

/// Whether writing `store` may change a byte of `cell`: `regions` leaves
/// it open and `pointerfacts` cannot place them apart.
pub fn may_clobber(unit: &Unit, known: Option<&BTreeMap<ValueId, Interval>>, cell: &MemRef, store: &MemRef) -> bool {
    let offsets = pointerfacts::offsets(unit.context, unit.layout, unit.function);
    let apart = matches!((located(cell), located(store)), (Some(one), Some(other)) if offsets.disjoint(one, other));
    // An answer Rust cannot represent is taken to overlap.
    overlapping(cell, store, known, known, unit.machine).unwrap_or(true) && !apart
}

/// Whether `outer` certainly holds every byte of `inner`: both at fixed
/// displacements in one frame (`regions`), or at constant offsets from one
/// pointer (`pointerfacts`).
pub fn covers(unit: &Unit, outer: &MemRef, inner: &MemRef) -> bool {
    if let (Some((frame, low, high)), Some((inner_frame, inner_low, inner_high))) = (displaced_span(outer), displaced_span(inner))
        && frame == inner_frame
    {
        return low <= inner_low && inner_high <= high;
    }
    let offsets = pointerfacts::offsets(unit.context, unit.layout, unit.function);
    let (Some(one), Some(other)) = (located(outer), located(inner)) else { return false };
    offsets.comparable(one, other).is_some_and(|(low, inner_low)| low <= inner_low && inner_low + other.bytes as i64 <= low + one.bytes as i64)
}

/// Whether `one` and `other` certainly name the same bytes.
pub fn same_bytes(unit: &Unit, one: &MemRef, other: &MemRef) -> bool {
    covers(unit, one, other) && covers(unit, other, one)
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

    fn frontier(&self, site: InstId, memory: &MemRef, boundary: Option<usize>, edge: Option<i64>, edge_memory: Option<&MemRef>) -> BTreeSet<usize> {
        let block = self.at(site).block;
        let mut pending = vec![self.at(site).defining];
        let mut seen = BTreeSet::new();
        let mut found = BTreeSet::new();
        while let Some(current) = pending.pop() {
            let Some(current) = current else {
                continue;
            };
            if !seen.insert(current) {
                continue;
            }
            if Some(current) == boundary {
                found.insert(current);
                continue;
            }
            let access = self.access(current);
            match access.kind {
                Kind::Live => {
                    found.insert(current);
                }
                Kind::Phi => pending.extend(
                    access
                        .incoming
                        .iter()
                        .filter(|(parent, _)| edge.is_none() || access.block != block || *parent == edge)
                        .map(|(_, value)| Some(*value)),
                ),
                Kind::Def => {
                    let queried = match edge_memory {
                        Some(edge_memory) if access.block != block => edge_memory,
                        _ => memory,
                    };
                    let written = &self.written[&access.site.expect("a def has a site")];
                    if written.as_ref().is_none_or(|stores| stores.iter().any(|store| may_clobber(&self.unit, None, queried, store))) {
                        found.insert(current);
                    } else {
                        pending.push(access.defining);
                    }
                }
                Kind::Use => pending.push(access.defining),
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
    MemorySSA { live, accesses, sites, phis, written, unit: *unit }
}

#[cfg(test)]
#[path = "memoryssa_tests.rs"]
mod tests;
