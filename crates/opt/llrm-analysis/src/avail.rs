//! Available loads and dead stores, as LLVM's GVN load PRE and DSE find
//! them through MemoryDependence: llrm-core's `analysis/avail.rs`, adapted
//! to the rich MIR.
//!
//! A load is served by the value a MemorySSA walk finds in its bytes
//! (`forwardable_walk`); `loadjoins` forms a phi where predecessors differ,
//! from MemorySSA's per-edge proof. A value serves a read only of its own
//! type. The forward availability map this once solved (`MemRef -> Operand`
//! at each block) was deleted after the walk served every load it served
//! (45216 of 45216 over the QCport -O2 compile) and cost more.
//!
//! Dropped, with no rich MIR counterpart: `Holder` (what a store wrote is
//! an operand, a constant or a global's address included), `_addressing`,
//! `_real` and `_preserved` (a load defines its whole result from its one
//! pointer), `_crosses_edges` and every stack exclusion (no push area),
//! the runtime names in `calls` (what a call touches is `Accesses`'), float
//! exceptions, `sealed` and `handles_errors` (a handler in this function is
//! reached along an `invoke`'s unwind edge, which the walk follows),
//! `bounds` (the region lattice's
//! landmarks), `Forward::op` (`at` names the instruction) and the
//! `DEAD_OVERLAPS` counter.

use std::cell::RefCell;
use std::collections::BTreeSet;

use llrm_mir::dense::IdSet;
use llrm_mir::module::{InstId, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_support::bits::Bits;
use llrm_support::hash::{HashMap, IndexMap};

use crate::alias::EscapedBefore;
use crate::cellmap::CellMap;
use crate::cfg;
use crate::memory::{MemRef, Unit};
use crate::memoryssa::{self, Accesses, covered, covers, may_clobber, placed, same_bytes};
use crate::ranges;
use crate::regions::{OverlapBucket, OverlapBuckets, overlap_bucket, overlap_buckets};

/// The cell `inst` purely loads, and the value it lands in.
pub fn loaded_into(
    unit: &Unit,
    accesses: &Accesses,
    inst: InstId,
) -> Option<(MemRef, ValueId)> {
    let instruction = unit.function.instruction(inst);
    match instruction.opcode {
        Opcode::Load { volatile: false, .. } => Some((accesses.references.get(&inst)?.clone(), instruction.result?)),
        _ => None,
    }
}

/// The cell `inst` purely stores, and what it wrote there.
pub fn stored_from(
    unit: &Unit,
    accesses: &Accesses,
    inst: InstId,
) -> Option<(MemRef, Operand)> {
    let instruction = unit.function.instruction(inst);
    match instruction.opcode {
        Opcode::Store { volatile: false, .. } => {
            Some((accesses.references.get(&inst)?.clone(), instruction.operands[0]))
        }
        _ => None,
    }
}

/// The cell `inst` purely stores to, whatever it put there: a store's, or
/// the bytes a `memset` fills.
pub fn stored_cell(
    unit: &Unit,
    accesses: &Accesses,
    inst: InstId,
) -> Option<MemRef> {
    stored_from(unit, accesses, inst).map(|(cell, _)| cell).or_else(|| {
        MemRef::filled(unit, inst).filter(|one| !one.volatile).and_then(|_| accesses.references.get(&inst).cloned())
    })
}

/// Loads in `want` that a known value can serve, by the MemorySSA walk.
pub fn forwardable_walk(
    unit: &Unit,
    accesses: &Accesses,
    want: &IdSet<InstId>,
) -> Vec<Forward> {
    WALKED.with(|walked| walked.set(walked.get() + 1));
    let missing: Vec<InstId> = unit
        .function
        .layout()
        .iter()
        .flat_map(|&block| unit.function.block(block).instructions().to_vec())
        .filter(|inst| want.contains(inst) && loaded_into(unit, accesses, *inst).is_some())
        .collect();
    if missing.is_empty() { Vec::new() } else { memory_providers(unit, accesses, &missing) }
}

thread_local! {
    static WALKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has walked for every load, for a test that a
/// function numbered twice walks once.
pub fn walked() -> usize {
    WALKED.with(std::cell::Cell::get)
}

/// A load whose bytes equal a known value.
#[derive(Clone, Debug, PartialEq)]
pub struct Forward {
    pub at: InstId,
    // Which register holds it is the allocator's answer.
    pub value: Operand,
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
    /// Per cell, lazily: the cells whose store writes all its bytes, the
    /// cells naming its bytes, and those placed to write some of them.
    covering: RefCell<Vec<Option<Bits>>>,
    alike: RefCell<Vec<Option<Bits>>>,
    pieces: RefCell<Vec<Option<Bits>>>,
}

impl Stored {
    fn new(
        unit: &Unit,
        accesses: &Accesses,
        private: Option<&dyn Fn(&MemRef) -> bool>,
    ) -> Self {
        let mut number: IndexMap<MemRef, usize> = IndexMap::default();
        for (_, inst) in unit.function.walk() {
            for cell in stored_cell(unit, accesses, inst).into_iter().chain(accesses.fills(inst).iter().cloned()) {
                let next = number.len();
                number.entry(cell).or_insert(next);
            }
        }
        let cells: Vec<MemRef> = number.keys().cloned().collect();
        let mut buckets = OverlapBuckets::default();
        let index = CellMap::new((0..cells.len()).map(|at| (at, ())).collect(), |at| {
            (overlap_bucket(&mut buckets, &cells[*at]), None)
        });
        let mut marked = Bits::new(cells.len());
        if let Some(private) = private {
            cells.iter().enumerate().filter(|(_, one)| private(one)).for_each(|(at, _)| marked.insert(at));
        }
        let size = cells.len();
        Self {
            cells,
            number,
            index,
            private: marked,
            covering: RefCell::new(vec![None; size]),
            alike: RefCell::new(vec![None; size]),
            pieces: RefCell::new(vec![None; size]),
        }
    }

    fn none(&self) -> Bits {
        Bits::new(self.cells.len())
    }

    fn related(
        &self,
        memo: &RefCell<Vec<Option<Bits>>>,
        at: usize,
        related: impl Fn(&MemRef, &MemRef) -> bool,
    ) -> Bits {
        if let Some(known) = &memo.borrow()[at] {
            return known.clone();
        }
        let mut found = self.none();
        self.cells
            .iter()
            .enumerate()
            .filter(|(_, other)| related(&self.cells[at], other))
            .for_each(|(one, _)| found.insert(one));
        memo.borrow_mut()[at] = Some(found.clone());
        found
    }

    /// The cells a store to which writes every byte of cell `at`.
    fn covering(
        &self,
        unit: &Unit,
        at: usize,
    ) -> Bits {
        self.related(&self.covering, at, |one, other| covers(unit, other, one))
    }

    fn alike(
        &self,
        unit: &Unit,
        at: usize,
    ) -> Bits {
        self.related(&self.alike, at, |one, other| same_bytes(unit, one, other))
    }

    /// Whether the cells in `overwritten` together write every byte of
    /// cell `at`, no one of them all.
    fn pieced(
        &self,
        unit: &Unit,
        at: usize,
        overwritten: &Bits,
    ) -> bool {
        let pieces = self.related(&self.pieces, at, |one, other| {
            placed(unit, other, one)
                .zip(placed(unit, one, one))
                .is_some_and(|((low, high), (_, size))| low < size && high > 0)
        });
        let written: Vec<&MemRef> =
            pieces.iter().filter(|piece| overwritten.contains(*piece)).map(|piece| &self.cells[piece]).collect();
        written.len() > 1 && covered(unit, &written, &self.cells[at])
    }
}

/// What a dead-store solve reads of the function besides its instructions.
struct Solve<'a, 'u> {
    unit: &'a Unit<'u>,
    accesses: &'a Accesses,
    private: Option<&'a dyn Fn(&MemRef) -> bool>,
    escaped: Option<&'a EscapedBefore>,
    stored: Stored,
}

impl Solve<'_, '_> {
    /// One block, backward, from what its successors have already overwritten.
    ///
    /// Returns the stores it found dead and what is overwritten on entry, so
    /// the caller can carry the second to the predecessors.
    fn dead_in(
        &self,
        block: i64,
        overwritten: &Bits,
    ) -> (Vec<InstId>, Bits) {
        let unit = self.unit;
        let stored = &self.stored;
        let mut found = Vec::new();
        let mut overwritten = overwritten.clone();
        for &inst in unit.function.block(cfg::block(block)).instructions().iter().rev() {
            let opcode = &unit.function.instruction(inst).opcode;
            let call = matches!(opcode, Opcode::Call(_) | Opcode::Invoke(_));
            // Nothing can read a private cell but by its name: not a call,
            // and not an address this cannot resolve.
            let shielded = self.private.is_some();
            let (Some(loads), Some(stores)) = (self.accesses.reads(inst), self.accesses.writes(inst)) else {
                if shielded && call {
                    overwritten.intersect_with(&stored.private);
                } else {
                    overwritten = stored.none();
                }
                continue;
            };

            if let Some(cell) = stored_cell(unit, self.accesses, inst) {
                let at = stored.number[&cell];
                if stored.covering(unit, at).intersects(&overwritten) || stored.pieced(unit, at, &overwritten) {
                    found.push(inst);
                }
                overwritten.insert(at);
                continue;
            }

            // Anything this reads or writes puts the cells it may touch back in
            // doubt.
            for reference in loads.iter().chain(stores) {
                self.clobber(&mut overwritten, inst, reference, shielded && (call || !reference.named()));
            }
            // What a call fills it writes before reading, unless it may read it
            // otherwise.
            for fill in self.accesses.fills(inst) {
                let at = stored.number[fill];
                if !loads.iter().any(|one| self.reaches(inst, one, at, shielded && (call || !one.named()))) {
                    overwritten.insert(at);
                }
            }
        }
        (found, overwritten)
    }

    /// Whether an access through `reference` at `inst` may touch cell `at`,
    /// for all the index knows; an `unnamed` one cannot reach a private cell.
    fn within(
        &self,
        inst: InstId,
        reference: &MemRef,
        at: usize,
        unnamed: bool,
    ) -> bool {
        let stored = &self.stored;
        !(unnamed && stored.private.contains(at))
            && !self.escaped.is_some_and(|escaped| escaped.apart(inst, reference, &stored.cells[at]))
    }

    /// Whether an access through `reference` at `inst` may touch cell `at`.
    fn reaches(
        &self,
        inst: InstId,
        reference: &MemRef,
        at: usize,
        unnamed: bool,
    ) -> bool {
        self.within(inst, reference, at, unnamed) && may_clobber(self.unit, None, &self.stored.cells[at], reference)
    }

    /// Forget the cells an access through `reference` at `inst` may touch;
    /// an `unnamed` one cannot reach a private cell.
    fn clobber(
        &self,
        overwritten: &mut Bits,
        inst: InstId,
        reference: &MemRef,
        unnamed: bool,
    ) {
        // Nothing overwritten, nothing to forget: no need to find the cells it
        // could reach.
        if overwritten.is_empty() {
            return;
        }
        let stored = &self.stored;
        let live = |at: &usize| overwritten.contains(*at) && self.within(inst, reference, *at, unnamed);
        let reached: Vec<usize> = match overlap_buckets(reference, &stored.index.parts) {
            None => stored.index.buckets.values().flat_map(IndexMap::keys).filter(|at| live(at)).copied().collect(),
            Some(buckets) => buckets
                .iter()
                .filter_map(|bucket| stored.index.buckets.get(bucket))
                .flat_map(IndexMap::keys)
                .filter(|at| live(at))
                .copied()
                .collect(),
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
/// case it starts from every such cell stored here. One ending in
/// `unreachable` starts from every cell.
pub fn dead_stores(
    unit: &Unit,
    accesses: &Accesses,
    private: Option<&dyn Fn(&MemRef) -> bool>,
) -> Vec<InstId> {
    dead_stores_escaping(unit, accesses, private, None)
}

/// `dead_stores`, an access through a pointer no fact follows sparing a
/// frame cell whose object had not escaped by then, as `escaped` says.
pub fn dead_stores_escaping(
    unit: &Unit,
    accesses: &Accesses,
    private: Option<&dyn Fn(&MemRef) -> bool>,
    escaped: Option<&EscapedBefore>,
) -> Vec<InstId> {
    let solve = Solve { unit, accesses, private, escaped, stored: Stored::new(unit, accesses, private) };
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
            // No successor at all: only the caller may read it -- and after an
            // `unreachable` nothing does, as LLVM's DSE skips such exits; the
            // noreturn call before it reads what escaped by then.
            let out = out.unwrap_or_else(|| if aborts(unit, block.at) { every.clone() } else { unread.clone() });
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

/// Whether block `at` ends in `unreachable`.
fn aborts(
    unit: &Unit,
    at: i64,
) -> bool {
    unit.function
        .terminator(cfg::block(at))
        .is_some_and(|last| unit.function.instruction(last).opcode == Opcode::Unreachable)
}

thread_local! {
    static SAMES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has compared a load's bytes with a missing one's
/// in `memory_providers`.
pub fn same_runs() -> usize {
    SAMES.with(std::cell::Cell::get)
}

/// Whether `holder` can stand for `value`: it has its type.
fn serves(
    unit: &Unit,
    holder: Operand,
    value: ValueId,
) -> bool {
    unit.operand_type(holder) == Some(unit.function.value(value).ty)
}

/// Recover dominating memory values the forward lattice lost at loops.
fn memory_providers(
    unit: &Unit,
    accesses: &Accesses,
    missing: &[InstId],
) -> Vec<Forward> {
    let function = unit.function;
    // Register facts only, as the availability map reads them: a memory-aware
    // solve per query costs more than it finds.
    let graph = memoryssa::built(unit, accesses).with_known(ranges::constants(unit).into_iter().collect());
    let shape = unit.shape();
    let dominance = &shape.dominance;
    let places: HashMap<InstId, (i64, usize)> = function
        .layout()
        .iter()
        .flat_map(|&block| {
            function
                .block(block)
                .instructions()
                .iter()
                .enumerate()
                .map(move |(index, &inst)| (inst, (cfg::id(block), index)))
        })
        .collect();
    let loads: Vec<(InstId, (MemRef, ValueId))> =
        graph.sites.keys().filter_map(|&site| loaded_into(unit, accesses, site).map(|loaded| (site, loaded))).collect();
    let available = |source: InstId, site: InstId| -> bool {
        let ((source_block, source_index), (block, index)) = (places[&source], places[&site]);
        dominance.dominates(source_block, block) && (source_block != block || source_index < index)
    };

    // Whether `value` is defined where `site` can read it.
    let available_value = |value: Operand, site: InstId| -> bool {
        match value {
            Operand::Value(one) => match function.value(one).def {
                llrm_mir::module::ValueDef::Instruction(def) => available(def, site),
                _ => true,
            },
            _ => true,
        }
    };

    let mut groups: Vec<(&MemRef, Vec<usize>)> = Vec::new();
    let mut group_of: HashMap<&MemRef, usize> = HashMap::default();
    // The groups by what a group naming the same bytes shares, so a load
    // compares itself with those and not with all.
    let mut group_keys: Vec<Vec<memoryssa::ByteKey>> = Vec::new();
    let mut load_group: Vec<usize> = Vec::with_capacity(loads.len());
    for (at, (_, (loaded, _))) in loads.iter().enumerate() {
        let group = *group_of
            .entry(loaded)
            .or_insert_with(
                || {
                    groups.push((loaded, Vec::new()));
                    group_keys.push(memoryssa::byte_keys(unit, loaded).list());
                    groups.len() - 1
                },
            );
        groups[group].1.push(at);
        load_group.push(group);
    }
    // The loads by the bytes they name, in the blocks that hold them: those
    // that dominate a site are found from its block upward, nearest first,
    // where every load of the same bytes was taken and sorted for each site
    // (9.5 M candidates for 7,000 sites at N=1024).
    let tree = shape.dominance.tree();
    let nearest = crate::nearest::Nearest::of(
        tree,
        loads
            .iter()
            .enumerate()
            .filter_map(|(at, (site, _))| function.parent(*site).map(|block| (at, block)))
            .flat_map(|(at, block)| group_keys[load_group[at]].iter().cloned().map(move |key| (key, block, at))),
    );
    let mut found = Vec::new();
    for &site in missing {
        let Some((cell, result)) = loaded_into(unit, accesses, site) else {
            continue;
        };
        // A load that is invariant is clobbered by nothing; the store before it
        // that initialised what it reads is still the value it has.
        let clobbers = graph.clobbers_ignoring_invariance(site, &cell);
        let single = if clobbers.len() == 1 { clobbers.first().map(|id| graph.access(*id)) } else { None };
        if let Some(source) = single.filter(|access| access.kind == memoryssa::Kind::Def).and_then(|access| access.site)
            && let Some((stored, value)) = stored_from(unit, accesses, source)
            && available(source, site)
            && same_bytes(unit, &stored, &cell)
            && serves(unit, value, result)
        {
            found.push(Forward { at: site, value });
            continue;
        }
        // Several clobbers, each an exact store of the one value, and that
        // value defined where it reaches the load: the arms of a join
        // that stored the same thing.
        if clobbers.len() > 1
            && let Some(value) = joined_store(unit, accesses, &graph, &clobbers, &cell, site, &available_value)
            && serves(unit, value, result)
        {
            found.push(Forward { at: site, value });
            continue;
        }
        // Loads of one address are one group, whose bytes are compared with
        // `cell` once; the candidates are then taken in load order, as
        // when each was compared.
        let keys = memoryssa::byte_keys(unit, &cell).list();
        // Whether the loads of a group name the bytes of `cell`, asked once of
        // each.
        let mut same: HashMap<usize, bool> = HashMap::default();
        let mut names = |group: usize| -> bool {
            *same
                .entry(group)
                .or_insert_with(
                    || {
                        SAMES.with(|runs| runs.set(runs.get() + 1));
                        same_bytes(unit, groups[group].0, &cell)
                    },
                )
        };
        // The loads that dominate the site and name its bytes, in load order.
        // None for a site the entry does not reach: nothing is `available`
        // there.
        let reached = function.parent(site).filter(|&block| tree.is_reachable(block));
        let mut candidates: Vec<usize> = reached
            .map(|block| {
                nearest
                    .dominating(tree, &keys.iter().collect::<Vec<_>>(), block)
                    .map(|(_, at)| at)
                    .filter(|&at| names(load_group[at]))
                    .collect()
            })
            .unwrap_or_default();
        candidates.sort_unstable();
        if llrm_support::env_set("LLRM_CHECK_GROUPS") {
            let every: Vec<usize> = groups
                .iter()
                .filter(|(loaded, _)| same_bytes(unit, loaded, &cell))
                .flat_map(|(_, members)| members.iter().copied())
                .filter(|&at| available(loads[at].0, site))
                .collect();
            let mut every = every;
            every.sort_unstable();
            let kept: Vec<usize> = candidates.iter().copied().filter(|&at| available(loads[at].0, site)).collect();
            assert_eq!(
                kept, every,
                "LLRM_CHECK_GROUPS: the loads that dominate a site, found from its block up, are not those a scan of every group finds"
            );
        }
        // Nearest first: a walk from the load stops at the first value that
        // holds, and the loads before it, each a walk to fail, are not tried.
        // Every candidate holds the same bytes, so the one a chain of served
        // loads ends in is the same. A candidate whose walk finds a write
        // fails every load that dominates it too: the write lies on a path
        // from the farther one through the nearer, so those are not walked.
        let mut failed: Vec<InstId> = Vec::new();
        for at in candidates.into_iter().rev() {
            let (source, (loaded, value)) = &loads[at];
            if !available(*source, site)
                || !serves(unit, Operand::Value(*value), result)
                || failed.iter().any(|&nearer| available(*source, nearer))
            {
                continue;
            }
            if graph.unchanged(*source, site, loaded) {
                found.push(Forward { at: site, value: Operand::Value(*value) });
                break;
            }
            failed.push(*source);
        }
    }
    found
}

/// The one value every clobber of a load stores into its exact bytes, when each
/// clobber is such a store and the value is defined where `site` reads it.
fn joined_store(
    unit: &Unit,
    accesses: &Accesses,
    graph: &memoryssa::MemorySSA,
    clobbers: &BTreeSet<usize>,
    cell: &MemRef,
    site: InstId,
    available_value: &dyn Fn(Operand, InstId) -> bool,
) -> Option<Operand> {
    let mut one = None;
    for id in clobbers {
        let access = graph.access(*id);
        let source = access.site.filter(|_| access.kind == memoryssa::Kind::Def)?;
        let (stored, value) = stored_from(unit, accesses, source)?;
        if !same_bytes(unit, &stored, cell) || one.is_some_and(|before| before != value) {
            return None;
        }
        one = Some(value);
    }
    one.filter(|&value| available_value(value, site))
}

#[cfg(test)]
#[path = "avail_tests.rs"]
mod tests;
