//! Available loads and dead stores, as LLVM's GVN load PRE and DSE find
//! them through MemoryDependence: llrm-core's `analysis/avail.rs`, adapted
//! to the rich MIR.
//!
//! A forward dataflow whose fact is `MemRef -> Operand`: "these bytes are
//! this value". Keyed on the access, which carries the SSA values its
//! address is reached through. It intersects at joins; `loadjoins` forms a
//! phi where predecessors differ, from MemorySSA's per-edge proof. A value
//! serves a read only of its own type.
//!
//! Dropped, with no rich MIR counterpart: `Holder` (what a store wrote is
//! an operand, a constant or a global's address included), `_addressing`,
//! `_real` and `_preserved` (a load defines its whole result from its one
//! pointer), `_crosses_edges` and every stack exclusion (no push area),
//! the runtime names in `calls` (a call's footprint is `Calls`), float
//! exceptions, `sealed` and `handles_errors` (a handler in this function is
//! reached along an `invoke`'s unwind edge, which the solve walks),
//! `bounds` (the region lattice's
//! landmarks), `Forward::op` (`at` names the instruction) and the
//! `DEAD_OVERLAPS` counter.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashSet};

use llrm_graph::loops;
use llrm_mir::module::{InstId, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_support::bits::Bits;
use llrm_support::hash::{HashMap, IndexMap};

use crate::cellmap::CellMap;
use crate::cfg;
use crate::consts::Calls;
use crate::memory::{MemRef, MemoryKind, Unit};
use crate::memoryssa::{self, covers, may_clobber, reads, same_bytes, writes};
use crate::ranges::{self, Interval};
use crate::regions::{OverlapBucket, overlap_bucket, overlap_buckets};

/// What each cell holds.
pub type Holders = IndexMap<MemRef, Operand>;

/// The map on entry to and exit from each block.
#[derive(Clone, Debug)]
pub struct Held {
    pub into: IndexMap<i64, Holders>,
    pub outof: IndexMap<i64, Holders>,
    // Each value's constant, so an index is placed by its number.
    pub known: BTreeMap<ValueId, Interval>,
}

/// The cell `inst` purely loads, and the value it lands in.
pub fn loaded_into(unit: &Unit, inst: InstId) -> Option<(MemRef, ValueId)> {
    let instruction = unit.function.instruction(inst);
    match instruction.opcode {
        Opcode::Load { volatile: false, .. } => Some((unit.reference(inst)?, instruction.result?)),
        _ => None,
    }
}

/// The cell `inst` purely stores, and what it wrote there.
pub fn stored_from(unit: &Unit, inst: InstId) -> Option<(MemRef, Operand)> {
    let instruction = unit.function.instruction(inst);
    match instruction.opcode {
        Opcode::Store { volatile: false, .. } => Some((unit.reference(inst)?, instruction.operands[0])),
        _ => None,
    }
}

/// The cell `inst` purely stores to, whatever it put there.
pub fn stored_cell(unit: &Unit, inst: InstId) -> Option<MemRef> {
    stored_from(unit, inst).map(|(cell, _)| cell)
}

/// Whether `holder` can stand for `value`: it has its type.
fn serves(unit: &Unit, holder: Operand, value: ValueId) -> bool {
    unit.operand_type(holder) == Some(unit.function.value(value).ty)
}

/// The map across one instruction.
fn after(unit: &Unit, inst: InstId, mut holders: Holders, calls: &Calls, known: Option<&BTreeMap<ValueId, Interval>>) -> Holders {
    let Some(stores) = writes(unit, calls, inst) else {
        return Holders::default();
    };
    for store in &stores {
        holders.retain(|one, _| !may_clobber(unit, known, one, store));
    }
    let found = stored_from(unit, inst).or_else(|| loaded_into(unit, inst).map(|(cell, value)| (cell, Operand::Value(value))));
    if let Some((cell, value)) = found {
        holders.insert(cell, value);
    }
    holders
}

/// Only what every predecessor agrees on, value and all.
fn meet(maps: &[&Holders]) -> Holders {
    let Some(first) = maps.first() else {
        return Holders::default();
    };
    let mut out = (*first).clone();
    for other in &maps[1..] {
        out.retain(|one, who| other.get(one) == Some(who));
    }
    out
}

/// Which value each cell holds, at every block's entry and exit.
pub fn holders(unit: &Unit, calls: &Calls) -> Held {
    // Register facts only: a memory-aware solve per query costs more than it finds.
    let known: BTreeMap<ValueId, Interval> = ranges::constants(unit).into_iter().collect();
    let graph = cfg::graph(unit.function);
    let entry = unit.function.entry().map(cfg::id);
    let preds = loops::predecessors(&graph);
    let mut into: IndexMap<i64, Holders> = graph.iter().map(|block| (block.at, Holders::default())).collect();
    let mut outof = into.clone();

    // A block whose parents' exits are unchanged would recompute its own.
    let mut stale: HashSet<i64> = graph.iter().map(|block| block.at).collect();
    let mut changing = true;
    while changing {
        changing = false;
        for block in &graph {
            if !stale.remove(&block.at) {
                continue;
            }
            let arriving = if Some(block.at) == entry { Holders::default() } else { meet(&preds[&block.at].iter().map(|one| &outof[one]).collect::<Vec<_>>()) };
            let mut leaving = arriving.clone();
            for &inst in unit.function.block(cfg::block(block.at)).instructions() {
                leaving = after(unit, inst, leaving, calls, Some(&known));
            }
            if leaving != outof[&block.at] {
                stale.extend(&block.succ);
            }
            if arriving != into[&block.at] || leaving != outof[&block.at] {
                into.insert(block.at, arriving);
                outof.insert(block.at, leaving);
                changing = true;
            }
        }
    }

    Held { into, outof, known }
}

/// What holds `reference`'s bytes just before `at`.
///
/// A new use extends its lifetime; this says nothing about its allocation.
pub fn provider(unit: &Unit, calls: &Calls, at: InstId, reference: &MemRef) -> Option<Operand> {
    let found = holders(unit, calls);
    let block = unit.function.parent(at)?;
    let mut current = found.into[&cfg::id(block)].clone();
    for &inst in unit.function.block(block).instructions() {
        if inst == at {
            return current.iter().find(|(one, _)| same_bytes(unit, one, reference)).map(|(_, who)| *who);
        }
        current = after(unit, inst, current, calls, Some(&found.known));
    }
    None
}

/// A load whose bytes equal a known value.
#[derive(Clone, Debug, PartialEq)]
pub struct Forward {
    pub at: InstId,
    // Which register holds it is the allocator's answer.
    pub value: Operand,
}

/// Whether a cell is named outright rather than reached through a value.
///
/// A fixed displacement in an object names its bytes. So does canonical
/// provenance. An unresolved pointer or index reaches a private cell no
/// more than an unknown call does.
fn fixed(reference: &MemRef) -> bool {
    let canonical = reference
        .provenance
        .as_ref()
        .is_some_and(|provenance| !provenance.slices.is_empty() && provenance.slices.iter().all(|one| one.object.kind != MemoryKind::Unknown));
    canonical || (reference.object && reference.addr().is_some())
}

/// Every cell a function stores to, numbered once for its dead-store solve.
///
/// A block's state is a set of these, not a map rebuilt per block: what is
/// overwritten only ever names a cell some store here writes, as LLVM's
/// DSE numbers its MemoryLocations. `index` buckets them as `overlapping`
/// rules writes out; a write asks only the live cells of the buckets it
/// reaches.
struct Stored {
    cells: Vec<MemRef>,
    number: IndexMap<MemRef, usize>,
    index: CellMap<usize, (), OverlapBucket>,
    private: Bits,
    /// Per cell, lazily: the cells whose store writes all its bytes, and
    /// the cells naming its bytes.
    covering: RefCell<Vec<Option<Bits>>>,
    alike: RefCell<Vec<Option<Bits>>>,
}

impl Stored {
    fn new(unit: &Unit, private: Option<&dyn Fn(&MemRef) -> bool>) -> Self {
        let mut number: IndexMap<MemRef, usize> = IndexMap::default();
        for (_, inst) in unit.function.walk() {
            if let Some(cell) = stored_cell(unit, inst) {
                let next = number.len();
                number.entry(cell).or_insert(next);
            }
        }
        let cells: Vec<MemRef> = number.keys().cloned().collect();
        let index = CellMap::new((0..cells.len()).map(|at| (at, ())).collect(), |at| (overlap_bucket(&cells[*at]), None));
        let mut marked = Bits::new(cells.len());
        if let Some(private) = private {
            cells.iter().enumerate().filter(|(_, one)| private(one)).for_each(|(at, _)| marked.insert(at));
        }
        let size = cells.len();
        Self { cells, number, index, private: marked, covering: RefCell::new(vec![None; size]), alike: RefCell::new(vec![None; size]) }
    }

    fn none(&self) -> Bits {
        Bits::new(self.cells.len())
    }

    fn related(&self, memo: &RefCell<Vec<Option<Bits>>>, at: usize, related: impl Fn(&MemRef, &MemRef) -> bool) -> Bits {
        if let Some(known) = &memo.borrow()[at] {
            return known.clone();
        }
        let mut found = self.none();
        self.cells.iter().enumerate().filter(|(_, other)| related(&self.cells[at], other)).for_each(|(one, _)| found.insert(one));
        memo.borrow_mut()[at] = Some(found.clone());
        found
    }

    /// The cells a store to which writes every byte of cell `at`.
    fn covering(&self, unit: &Unit, at: usize) -> Bits {
        self.related(&self.covering, at, |one, other| covers(unit, other, one))
    }

    fn alike(&self, unit: &Unit, at: usize) -> Bits {
        self.related(&self.alike, at, |one, other| same_bytes(unit, one, other))
    }
}

/// What a dead-store solve reads of the function besides its instructions.
struct Solve<'a, 'u> {
    unit: &'a Unit<'u>,
    calls: &'a Calls,
    private: Option<&'a dyn Fn(&MemRef) -> bool>,
    stored: Stored,
}

impl Solve<'_, '_> {
    /// One block, backward, from what its successors have already overwritten.
    ///
    /// Returns the stores it found dead and what is overwritten on entry, so
    /// the caller can carry the second to the predecessors.
    fn dead_in(&self, block: i64, overwritten: &Bits) -> (Vec<InstId>, Bits) {
        let unit = self.unit;
        let stored = &self.stored;
        let mut found = Vec::new();
        let mut overwritten = overwritten.clone();
        for &inst in unit.function.block(cfg::block(block)).instructions().iter().rev() {
            let opcode = &unit.function.instruction(inst).opcode;
            let volatile = matches!(opcode, Opcode::Load { volatile: true, .. } | Opcode::Store { volatile: true, .. });
            let call = matches!(opcode, Opcode::Call(_) | Opcode::Invoke(_));
            // Nothing can read a private cell but by its name: not a call,
            // and not an address this cannot resolve.
            let shielded = self.private.is_some() && !volatile;
            let (Some(loads), Some(stores)) = (reads(unit, inst), writes(unit, self.calls, inst)) else {
                if shielded && call {
                    overwritten.intersect_with(&stored.private);
                } else {
                    overwritten = stored.none();
                }
                continue;
            };

            if let Some(cell) = stored_cell(unit, inst) {
                let at = stored.number[&cell];
                if stored.covering(unit, at).intersects(&overwritten) {
                    found.push(inst);
                }
                overwritten.insert(at);
                continue;
            }

            // Anything this reads or writes puts the cells it may touch back in doubt.
            for reference in loads.iter().chain(&stores) {
                self.clobber(&mut overwritten, reference, shielded && (call || !fixed(reference)));
            }
        }
        (found, overwritten)
    }

    /// Forget the cells an access through `reference` may touch; an
    /// `unnamed` one cannot reach a private cell.
    fn clobber(&self, overwritten: &mut Bits, reference: &MemRef, unnamed: bool) {
        let stored = &self.stored;
        let live = |at: &usize| overwritten.contains(*at) && !(unnamed && stored.private.contains(*at));
        let reached: Vec<usize> = match overlap_buckets(reference, &stored.index.parts) {
            None => stored.index.buckets.values().flat_map(IndexMap::keys).filter(|at| live(at)).copied().collect(),
            Some(buckets) => buckets.iter().filter_map(|bucket| stored.index.buckets.get(bucket)).flat_map(IndexMap::keys).filter(|at| live(at)).copied().collect(),
        };
        for at in reached {
            if may_clobber(self.unit, None, &stored.cells[at], reference) {
                overwritten.remove(at);
            }
        }
    }
}

/// Stores whose bytes are overwritten before anything reads them.
///
/// Instructions, not addresses: two stores may share an address with an
/// unrelated live load.
///
/// Backward through each block: a store to a cell that a later store
/// overwrites, with nothing in between that could have read it, computed
/// nothing. Across edges a cell has to be overwritten on *every* successor
/// path, so what a block starts from is the intersection of what its
/// successors have. The fixed point starts from every stored cell and
/// shrinks; only the last round's verdicts stand.
///
/// A block with no successor starts from nothing: the caller may read the
/// cell -- unless `private` says nothing outside the function can, in which
/// case it starts from every such cell stored here.
pub fn dead_stores(unit: &Unit, calls: &Calls, private: Option<&dyn Fn(&MemRef) -> bool>) -> Vec<InstId> {
    let solve = Solve { unit, calls, private, stored: Stored::new(unit, private) };
    let stored = &solve.stored;
    let mut every = stored.none();
    (0..stored.cells.len()).for_each(|at| every.insert(at));
    let graph = cfg::graph(unit.function);
    let mut entry: IndexMap<i64, Bits> = graph.iter().map(|block| (block.at, every.clone())).collect();
    let unread = if private.is_some() { stored.private.clone() } else { stored.none() };

    let mut found: BTreeSet<InstId> = BTreeSet::new();
    // A block's answer depends only on what is overwritten after it.
    let mut last: HashMap<i64, (Bits, Vec<InstId>, Bits)> = HashMap::default();
    let mut changing = true;
    while changing {
        changing = false;
        found = BTreeSet::new();
        for block in graph.iter().rev() {
            let mut out: Option<Bits> = None;
            for successor in &block.succ {
                let have = &entry[successor];
                out = Some(match out {
                    None => have.clone(),
                    Some(mut out) => {
                        for one in out.iter().collect::<Vec<_>>() {
                            if !stored.alike(unit, one).intersects(have) {
                                out.remove(one);
                            }
                        }
                        out
                    }
                });
            }
            // No successor at all: only the caller may read it.
            let out = out.unwrap_or_else(|| unread.clone());
            let (mine, start) = match last.get(&block.at) {
                Some((seen, mine, start)) if *seen == out => (mine.clone(), start.clone()),
                _ => {
                    let (mine, start) = solve.dead_in(block.at, &out);
                    last.insert(block.at, (out, mine.clone(), start.clone()));
                    (mine, start)
                }
            };
            found.extend(mine);
            if start.len() != entry[&block.at].len() {
                changing = true;
            }
            entry.insert(block.at, start);
        }
    }
    unit.function.walk().map(|(_, inst)| inst).filter(|inst| found.contains(inst)).collect()
}

/// Loads in `want` that a known value can serve instead of memory.
///
/// The caller replaces the load, extending the provider's lifetime.
pub fn forwardable(unit: &Unit, calls: &Calls, want: &BTreeSet<InstId>) -> Vec<Forward> {
    let held = holders(unit, calls);
    let mut found = Vec::new();
    let mut missing = Vec::new();

    for &at in unit.function.layout() {
        let mut current = held.into[&cfg::id(at)].clone();
        for &inst in unit.function.block(at).instructions() {
            if want.contains(&inst)
                && let Some((cell, result)) = loaded_into(unit, inst)
            {
                match current.iter().find(|(one, who)| same_bytes(unit, one, &cell) && serves(unit, **who, result)) {
                    Some((_, who)) => found.push(Forward { at: inst, value: *who }),
                    None => missing.push(inst),
                }
            }
            current = after(unit, inst, current, calls, Some(&held.known));
        }
    }
    if !missing.is_empty() {
        found.extend(memory_providers(unit, calls, &missing));
    }
    found
}

/// Recover dominating memory values the forward lattice lost at loops.
fn memory_providers(unit: &Unit, calls: &Calls, missing: &[InstId]) -> Vec<Forward> {
    let function = unit.function;
    let graph = memoryssa::built(unit, calls);
    let dominance = loops::dominance(&cfg::graph(function), function.entry().map(cfg::id));
    let places: HashMap<InstId, (i64, usize)> =
        function.layout().iter().flat_map(|&block| function.block(block).instructions().iter().enumerate().map(move |(index, &inst)| (inst, (cfg::id(block), index)))).collect();
    let loads: Vec<(InstId, (MemRef, ValueId))> = graph.sites.keys().filter_map(|&site| loaded_into(unit, site).map(|loaded| (site, loaded))).collect();
    let available = |source: InstId, site: InstId| -> bool {
        let ((source_block, source_index), (block, index)) = (places[&source], places[&site]);
        dominance.dominates(source_block, block) && (source_block != block || source_index < index)
    };

    let mut found = Vec::new();
    for &site in missing {
        let Some((cell, result)) = loaded_into(unit, site) else {
            continue;
        };
        let clobbers = graph.clobbers(site, &cell);
        let single = if clobbers.len() == 1 { clobbers.first().map(|id| graph.access(*id)) } else { None };
        if let Some(source) = single.filter(|access| access.kind == memoryssa::Kind::Def).and_then(|access| access.site)
            && let Some((stored, value)) = stored_from(unit, source)
            && available(source, site)
            && same_bytes(unit, &stored, &cell)
            && serves(unit, value, result)
        {
            found.push(Forward { at: site, value });
            continue;
        }
        for (source, (loaded, value)) in &loads {
            if available(*source, site) && same_bytes(unit, loaded, &cell) && serves(unit, Operand::Value(*value), result) && graph.unchanged(*source, site, loaded) {
                found.push(Forward { at: site, value: Operand::Value(*value) });
                break;
            }
        }
    }
    found
}

#[cfg(test)]
#[path = "avail_tests.rs"]
mod tests;
