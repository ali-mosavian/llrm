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

use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::memory::stated;
use llrm_mir::module::{Change, Function, GlobalValue, InstId, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Analyses, Analysis};
use llrm_support::hash::IndexMap;

use crate::alias::{self, Effect, Procedure, Summary};
use crate::cfg;
use crate::consts::Calls;
use crate::graph::loops;
use crate::manager::{Annotated, CallEffects};
use crate::memory::{MemRef, ObjectRef, Unit, own_bytes, unmodeled_write};
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
    fn new(
        id: usize,
        kind: Kind,
    ) -> Self {
        Self { id, kind, block: None, site: None, defining: None, incoming: Vec::new() }
    }
}

/// What an instruction reads and what it writes; `None` for anything.
/// The list of no references, shared: an empty `Rc<[_]>` still allocates its
/// counts.
fn none() -> Rc<[MemRef]> {
    thread_local! {
        static NONE: Rc<[MemRef]> = Rc::from([]);
    }
    NONE.with(Rc::clone)
}

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
    /// What the manager's accesses were made from: the references annotated
    /// and the calls' effects.
    source: Option<(Rc<<Annotated as Analysis>::Result>, Rc<<CallEffects as Analysis>::Result>)>,
}

/// The objects an instruction's writes can reach, for ruling a cell out before
/// any alias reasoning: a cell none of whose objects may alias any of them is
/// not written. `unknown` when a write names no object, or may write anything.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reach {
    unknown: bool,
    objects: Vec<ObjectRef>,
}

impl Reach {
    fn of(writes: &Option<Rc<[MemRef]>>) -> Self {
        let Some(writes) = writes else { return Self { unknown: true, objects: Vec::new() } };
        let mut reach = Self::default();
        for write in writes.iter() {
            match write.provenance.as_ref().filter(|one| !one.slices.is_empty()) {
                None => reach.unknown = true,
                Some(provenance) => {
                    for slice in &provenance.slices {
                        if !reach.objects.contains(&slice.object) {
                            reach.objects.push(slice.object);
                        }
                    }
                }
            }
        }
        reach
    }

    /// Whether the writes certainly leave `cell` alone, by the objects each
    /// names alone.
    pub fn misses(
        &self,
        cell: &MemRef,
    ) -> bool {
        !self.unknown
            && cell.provenance.as_ref().is_some_and(|one| {
                !one.slices.is_empty()
                    && one.slices.iter().all(|slice| {
                        self.objects.iter().all(|written| !crate::memory::objects_may_alias(&slice.object, written))
                    })
            })
    }
}

impl Analysis for Accesses {
    type Result = Result<Rc<Accesses>, String>;
    const NAME: &'static str = "accesses";
    const INCREMENTAL: bool = true;

    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        let annotated = analyses.get::<Annotated>(context, layout, function);
        let calls = analyses.get::<CallEffects>(context, layout, function);
        let references = Result::as_ref(&*annotated).map_err(String::clone)?;
        let effects = Result::as_ref(&*calls).map_err(String::clone)?;
        let shape = analyses.get::<crate::cfg::Shape>(context, layout, function);
        let exposed = analyses.get::<crate::manager::ExposedFrames>(context, layout, function);
        let made = Self::of(
            &Unit::within(context, layout, function, analyses.outer()).with_shape(&shape).with_exposed(&exposed),
            references.clone(),
            effects,
        );
        Ok(Rc::new(Self { source: Some((annotated, calls)), ..made }))
    }

    /// What an instruction touches is its own: a store or call moved to
    /// another place touches what it did, so these are the ones they were made
    /// from, as long as the references annotated and the calls' effects are
    /// (the same results, or equal ones). Any change that is not a move or a
    /// store given another value derives them afresh.
    fn update(
        previous: &Self::Result,
        changes: &[Change],
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Option<Self::Result> {
        let held = previous.as_ref().ok()?;
        let (was_annotated, was_calls) = held.source.as_ref()?;
        let only_moves = changes
            .iter()
            .all(
                |change| match *change {
                    Change::Moved { .. } => true,
                    Change::Rewritten(inst) => matches!(function.instruction(inst).opcode, Opcode::Store { .. }),
                    _ => false,
                },
            );
        if !only_moves {
            return None;
        }
        let annotated = analyses.get::<Annotated>(context, layout, function);
        let calls = analyses.get::<CallEffects>(context, layout, function);
        let unchanged = (Rc::ptr_eq(&annotated, was_annotated) || *annotated == **was_annotated)
            && (Rc::ptr_eq(&calls, was_calls) || *calls == **was_calls);
        unchanged.then(|| previous.clone())
    }
}

impl Accesses {
    /// `unit`'s accesses as `alias` resolves them: each reference with its
    /// provenance, each call's effect instantiated from `known` callees.
    pub fn resolved(
        unit: &Unit,
        known: &IndexMap<String, Summary>,
    ) -> Result<Self, String> {
        Ok(Self::of(unit, unit.annotated()?.into_owned(), &alias::calls_annotated(&Procedure::of(*unit), known)?))
    }

    /// `function`'s accesses, the manager's: from its `Annotated` and
    /// `CallEffects`; the pipeline must require `Summaries`.
    pub fn managed(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Result<Rc<Self>, String> {
        (*analyses.get::<Accesses>(context, layout, function)).clone()
    }

    /// `unit`'s accesses from `references` (`alias::annotated`'s) and each
    /// call's `effects` (`alias::calls_annotated`'s).
    pub fn of(
        unit: &Unit,
        references: IndexMap<InstId, MemRef>,
        effects: &IndexMap<InstId, Effect>,
    ) -> Self {
        let fills = effects
            .iter()
            .filter(|(_, effect)| !effect.fills.is_empty())
            .map(|(&at, effect)| (at, effect.fills.clone()))
            .collect();
        Self {
            fills,
            ..Self::new(unit, references, |inst| {
                effects.get(&inst).map(|effect| (Some(Rc::clone(&effect.loads)), Some(Rc::clone(&effect.stores))))
            })
        }
    }

    /// `unit`'s accesses unresolved: each reference as the unit has it
    /// (`Unit::reference`), each call writing its footprint in `calls`.
    pub fn plain(
        unit: &Unit,
        calls: &Calls,
    ) -> Self {
        let references =
            unit.function.walk().filter_map(|(_, inst)| unit.reference(inst).map(|one| (inst, one))).collect();
        Self::new(unit, references, |inst| calls.get(&inst).map(|stores| (None, Some(Rc::clone(stores)))))
    }

    /// `footprint` gives a call's reads and writes, where known.
    fn new(
        unit: &Unit,
        references: IndexMap<InstId, MemRef>,
        footprint: impl Fn(InstId) -> Option<Footprint>,
    ) -> Self {
        let function = unit.function;
        let mut touched = IndexMap::default();
        for (_, inst) in function.walk() {
            let reference = || references.get(&inst).cloned().into_iter().collect::<Rc<[_]>>();
            let opcode = &function.instruction(inst).opcode;
            let found = match opcode {
                // Its order against other volatile accesses is the passes',
                // which never move one.
                _ if let Some(own) = own_bytes(opcode) => {
                    let touched = |does: bool| Some(if does { reference() } else { none() });
                    (touched(own.reads), touched(own.writes))
                }
                Opcode::Call(info) | Opcode::Invoke(info) => {
                    let callee = llrm_mir::memory::callee(unit.context, function, inst)
                        .and_then(|one| unit.globals.get(one.0 as usize))
                        .and_then(GlobalValue::function);
                    let reading = stated(&info.attrs).reads && callee.is_none_or(|one| stated(&one.attrs).reads);
                    let (reads, writes) = footprint(inst).unwrap_or((None, None));
                    (
                        if reading { reads } else { Some(none()) },
                        if unmodeled_write(unit, inst) { writes } else { Some(none()) },
                    )
                }
                _ => continue,
            };
            touched.insert(inst, found);
        }
        Self { references, touched, fills: IndexMap::default(), source: None }
    }

    /// What `inst` writes; `None` where it may write anything.
    pub fn writes(
        &self,
        inst: InstId,
    ) -> Option<&[MemRef]> {
        self.touched.get(&inst).map_or(Some(&[]), |(_, writes)| writes.as_deref())
    }

    /// `writes`, shared rather than copied; `nothing` for an instruction that
    /// touches no memory.
    fn shared_writes(
        &self,
        inst: InstId,
        nothing: &Rc<[MemRef]>,
    ) -> Option<Rc<[MemRef]>> {
        self.touched.get(&inst).map_or_else(|| Some(Rc::clone(nothing)), |(_, writes)| writes.clone())
    }

    /// What the call `inst` writes before reading any: `initializes`.
    pub fn fills(
        &self,
        inst: InstId,
    ) -> &[MemRef] {
        self.fills.get(&inst).map_or(&[], Vec::as_slice)
    }

    /// What `inst` reads; `None` where it may read anything.
    pub fn reads(
        &self,
        inst: InstId,
    ) -> Option<&[MemRef]> {
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
pub fn changes(
    cell: &MemRef,
    invariant: bool,
    writes: Option<&[MemRef]>,
    clobbers: impl Fn(&MemRef) -> bool,
) -> bool {
    !invariant && !cell.unwritable() && writes.is_none_or(|stores| stores.iter().any(clobbers))
}

/// Whether running `write` leaves the bytes `read` reads as they were: it
/// writes none of them, as `accesses` says and `regions::overlapping` decides
/// on `program`; an answer it cannot give overlaps.
pub fn spares(
    accesses: &Accesses,
    program: Option<&llrm_mir::program::ProgramProxy>,
    read: &MemRef,
    write: InstId,
) -> bool {
    let overlaps = |wrote: &MemRef| overlapping(read, wrote, None, None, program).unwrap_or(true);
    !changes(read, false, accesses.writes(write), overlaps)
}

/// Whether writing `store` may change a byte of `cell`: `regions` leaves
/// it open and `pointerfacts` cannot place them apart.
pub fn may_clobber(
    unit: &Unit,
    known: Option<&BTreeMap<ValueId, Interval>>,
    cell: &MemRef,
    store: &MemRef,
) -> bool {
    let offsets = pointerfacts::offsets(unit.context, unit.layout, unit.function);
    let apart = matches!(
        (located(cell), located(store)),
        (Some(one), Some(other)) if offsets.disjoint(one, other)
    );
    // An answer Rust cannot represent is taken to overlap.
    overlapping(cell, store, known, known, unit.program).unwrap_or(true) && !apart
}

/// Whether `outer` certainly holds every byte of `inner`: both at fixed
/// displacements in one frame (`regions`), or at constant offsets from one
/// pointer (`pointerfacts`).
pub fn covers(
    unit: &Unit,
    outer: &MemRef,
    inner: &MemRef,
) -> bool {
    covered(unit, &[outer], inner)
}

/// Whether `outers` together certainly hold every byte of `inner`, as
/// LLVM's DSE merges the intervals later stores overwrite.
pub fn covered(
    unit: &Unit,
    outers: &[&MemRef],
    inner: &MemRef,
) -> bool {
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
pub fn placed(
    unit: &Unit,
    outer: &MemRef,
    inner: &MemRef,
) -> Option<(i128, i128)> {
    if let (Some((frame, low, high)), Some((inner_frame, inner_low, _))) =
        (displaced_span(outer), displaced_span(inner))
        && frame == inner_frame
    {
        return Some((low - inner_low, high - inner_low));
    }
    let offsets = pointerfacts::offsets(unit.context, unit.layout, unit.function);
    let (one, other) = (located(outer)?, located(inner)?);
    offsets
        .comparable(one, other)
        .map(|(low, inner_low)| (i128::from(low - inner_low), i128::from(low - inner_low) + i128::from(one.bytes)))
}

/// What references naming the same bytes (`same_bytes`) share, to find them
/// without comparing each pair: the frame and displaced span, or the base
/// pointer, constant offset and width. Two references `same_bytes` accepts have
/// at least one key in common; a common key is not proof, `same_bytes` decides.
pub struct ByteKeys {
    pub frame: Option<(crate::regions::Frame, i128, i128)>,
    pub pointer: Option<(llrm_mir::module::Operand, i64, u64)>,
}

pub fn byte_keys(
    unit: &Unit,
    reference: &MemRef,
) -> ByteKeys {
    let pointer = located(reference).and_then(|at| {
        let (base, offset) = pointerfacts::offsets(unit.context, unit.layout, unit.function).relative(at.pointer)?;
        Some((base, offset, at.bytes))
    });
    ByteKeys { frame: displaced_span(reference), pointer }
}

/// Whether `one` and `other` certainly name the same bytes.
pub fn same_bytes(
    unit: &Unit,
    one: &MemRef,
    other: &MemRef,
) -> bool {
    covers(unit, one, other) && covers(unit, other, one)
}

thread_local! {
    static RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has asked whether a def's writes may clobber a
/// cell, rather than been answered from memory.
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
    *ON.get_or_init(|| llrm_support::env_set("LLRM_CHECK_JUMPS"))
}

fn check_clobbers() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| llrm_support::env_set("LLRM_CHECK_CLOBBERS"))
}

#[cfg(test)]
thread_local! {
    /// Writes asked of `may_clobber` by the walks: the alias reasoning the
    /// object summary spares.
    pub(crate) static MAY_CLOBBERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
thread_local! {
    /// Times a cell was looked up by value (hashed whole) rather than by
    /// number, for a test that a walk does it once.
    pub(crate) static CELL_PROBES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[derive(Clone)]
pub struct MemorySSA<'a> {
    pub live: Access,
    pub accesses: Vec<Access>,
    pub sites: IndexMap<InstId, Access>,
    pub phis: IndexMap<i64, Access>,
    /// What each def writes, as `Accesses::writes` says.
    written: llrm_mir::dense::IdMap<InstId, Option<Rc<[MemRef]>>>,
    /// What each def's writes can reach, worked out when a walk first asks of
    /// the def.
    reaches: std::cell::RefCell<llrm_mir::dense::IdMap<InstId, Rc<Reach>>>,
    unit: Unit<'a>,
    /// The values that are constants, for placing an index away from a cell
    /// (`with_known`).
    known: Option<BTreeMap<ValueId, Interval>>,
    /// Whether a def's writes may clobber a cell, once for each pair: the loads
    /// of one address ask it of the same defs again and again.
    /// Keyed by the cell's slot (`slot`) and the def's access number: 0 not
    /// asked, 1 no, 2 yes.
    clobbers: std::cell::RefCell<Vec<Vec<u8>>>,
    /// Each cell asked about, numbered in the order it was first asked: a walk
    /// hashes the cell once, not once a step.
    slots: std::cell::RefCell<llrm_support::hash::HashMap<MemRef, usize>>,
    /// For a cell and whether the load is invariant: from an access that is a
    /// use or a def that leaves the cell alone, the access the walk back
    /// reaches before a clobber, a join, the live state or a boundary it was
    /// stopped at (plus one; 0: not known), by access number. A walk takes
    /// the jump instead of the steps, unless its own boundary lies between.
    jumps: std::cell::RefCell<Vec<Option<Vec<u32>>>>,
    /// Each access's entry and exit in a depth-first order of the tree its
    /// `defining` links make, by access number: one is behind another when
    /// its interval holds the other's.
    span: Vec<(u32, u32)>,
}

impl MemorySSA<'_> {
    /// This graph with the register constants `known` tells, as the
    /// availability map had them: an index a constant places away
    /// from a cell is not a clobber of it.
    pub fn with_known(
        mut self,
        known: BTreeMap<ValueId, Interval>,
    ) -> Self {
        self.known = Some(known);
        self
    }

    pub fn at(
        &self,
        site: InstId,
    ) -> &Access {
        &self.sites[&site]
    }

    /// The access numbered `id`: `accesses` is sorted by id.
    pub fn access(
        &self,
        id: usize,
    ) -> &Access {
        &self.accesses[self.accesses.binary_search_by_key(&id, |access| access.id).expect("a numbered access")]
    }

    /// Possible nearest writes before a site, including live-on-entry.
    ///
    /// Walk every phi input. A visited set closes cycles without treating
    /// the backedge as evidence that memory is unchanged. An empty result
    /// means no reachable source was found, not a reusable memory value.
    /// This identifies memory states, not a dominating scalar definition;
    /// forwarding consumers must establish value availability separately.
    pub fn clobbers(
        &self,
        site: InstId,
        memory: &MemRef,
    ) -> BTreeSet<usize> {
        self.frontier(site, memory, None, None, None)
    }

    /// `clobbers` for a load that is invariant, as if it were not: the store
    /// that initialises what it reads is found, so its value serves the
    /// load.
    pub fn clobbers_ignoring_invariance(
        &self,
        site: InstId,
        memory: &MemRef,
    ) -> BTreeSet<usize> {
        self.frontier_for(site, memory, None, None, None, false)
    }

    /// Whether a dominating earlier read's memory state still applies.
    ///
    /// The caller must establish dominance and equal addresses. Stop at
    /// the earlier memory version, rejecting any possibly aliasing write
    /// on the way, including writes carried by loop backedges.
    pub fn unchanged(
        &self,
        earlier: InstId,
        later: InstId,
        memory: &MemRef,
    ) -> bool {
        let boundary = self.at(earlier).defining;
        boundary.is_some_and(|boundary| {
            self.frontier(later, memory, Some(boundary), None, None) == BTreeSet::from([boundary])
        })
    }

    /// Whether any write of the def at `site` may change a byte of `cell`,
    /// remembered.
    fn clobbered(
        &self,
        slot: usize,
        cell: &MemRef,
        access: usize,
        site: InstId,
    ) -> bool {
        let remembered = self.clobbers.borrow().get(slot).and_then(|asked| asked.get(access)).copied().unwrap_or(0);
        if remembered != 0 && !check_clobbers() {
            return remembered == 2;
        }
        let stores = self.written[&site].as_deref().unwrap_or(&[]);
        // Objects that cannot meet rule a def out before any alias reasoning.
        let reach =
            Rc::clone(self.reaches.borrow_mut().get_or_insert_with(site, || Rc::new(Reach::of(&self.written[&site]))));
        let missed = reach.misses(cell);
        let found = if missed && !check_clobbers() {
            false
        } else {
            #[cfg(test)]
            MAY_CLOBBERS.with(|asked| asked.set(asked.get() + stores.len()));
            let found = stores.iter().any(|store| may_clobber(&self.unit, self.known.as_ref(), cell, store));
            assert!(!(missed && found), "LLRM_CHECK_CLOBBERS: objects that cannot meet were found to clobber");
            found
        };
        RUNS.with(|runs| runs.set(runs.get() + 1));
        let mut memo = self.clobbers.borrow_mut();
        if memo.len() <= slot {
            memo.resize(slot + 1, Vec::new());
        }
        let asked = &mut memo[slot];
        if asked.len() <= access {
            asked.resize(self.span.len().max(access + 1), 0);
        }
        if asked[access] != 0 {
            assert_eq!(
                asked[access] == 2,
                found,
                "LLRM_CHECK_CLOBBERS: a remembered answer differs from the one worked out"
            );
        }
        asked[access] = 1 + u8::from(found);
        found
    }

    /// `cell`'s number among the cells asked about; the cell is copied only the
    /// first time.
    fn slot(
        &self,
        cell: &MemRef,
    ) -> usize {
        #[cfg(test)]
        CELL_PROBES.with(|probes| probes.set(probes.get() + 1));
        let mut slots = self.slots.borrow_mut();
        if let Some(&slot) = slots.get(cell) {
            return slot;
        }
        let next = slots.len();
        slots.insert(cell.clone(), next);
        next
    }

    fn frontier(
        &self,
        site: InstId,
        memory: &MemRef,
        boundary: Option<usize>,
        edge: Option<i64>,
        edge_memory: Option<&MemRef>,
    ) -> BTreeSet<usize> {
        self.frontier_for(site, memory, boundary, edge, edge_memory, true)
    }

    /// `frontier`; `honor` whether a load that is invariant is taken to be
    /// clobbered by nothing.
    fn frontier_for(
        &self,
        site: InstId,
        memory: &MemRef,
        boundary: Option<usize>,
        edge: Option<i64>,
        edge_memory: Option<&MemRef>,
        honor: bool,
    ) -> BTreeSet<usize> {
        let found = self.walked(site, memory, boundary, edge, edge_memory, true, honor);
        if check_jumps() {
            assert_eq!(
                found,
                self.walked(site, memory, boundary, edge, edge_memory, false, honor),
                "LLRM_CHECK_JUMPS: a walk that jumps found other than a walk step by step"
            );
        }
        found
    }

    /// Whether `ahead` is `access` or reached from it by `defining` links.
    fn behind(
        &self,
        access: usize,
        ahead: usize,
    ) -> bool {
        let (entered, left) = self.span[ahead];
        let (at, _) = self.span[access];
        entered <= at && at < left
    }

    /// `frontier`; `jumping` takes the remembered jumps along chains of
    /// accesses that leave the cell alone, or none.
    fn walked(
        &self,
        site: InstId,
        memory: &MemRef,
        boundary: Option<usize>,
        edge: Option<i64>,
        edge_memory: Option<&MemRef>,
        jumping: bool,
        honor: bool,
    ) -> BTreeSet<usize> {
        let block = self.at(site).block;
        // A load of what is written once, then never: no write changes what it
        // reads.
        let invariant =
            honor && llrm_mir::memory::invariant_load(self.unit.context, self.unit.layout, self.unit.function, site);
        let mut pending = vec![self.at(site).defining];
        let mut seen = BTreeSet::new();
        let mut found = BTreeSet::new();
        // Where no edge chooses, the chain of uses and untouching defs back
        // from an access is the same for every load of the cell: its
        // end is found once and jumped to (LLVM's walker caches its clobber the
        // same way).
        let jumpable = jumping && edge.is_none() && edge_memory.is_none();
        let (slot, edge_slot) = (self.slot(memory), edge_memory.map(|one| self.slot(one)));
        let mut jumps = self.jumps.borrow_mut();
        let none = &mut Vec::new();
        let table = if jumpable {
            let at = slot * 2 + usize::from(invariant);
            if jumps.len() <= at {
                jumps.resize(at + 1, None);
            }
            jumps[at].get_or_insert_with(|| vec![0; self.span.len()])
        } else {
            none
        };
        // Accesses passed that nothing on them touches the cell; a jump to `end
        // + 1` says the ones before `end` are clean.
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
                // The boundary lies on the chain when it is behind `current`
                // and `end` is behind it: the walk stops there.
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
                    let (queried, queried_slot) = match (edge_memory, edge_slot) {
                        (Some(edge_memory), Some(edge_slot)) if access.block != block => (edge_memory, edge_slot),
                        _ => (memory, slot),
                    };
                    let written = &self.written[&access.site.expect("a def has a site")];
                    let site = access.site.expect("a def has a site");
                    if changes(queried, invariant, written.as_deref(), |_| {
                        self.clobbered(queried_slot, queried, current, site)
                    }) {
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
    pub fn available_on_edge(
        &self,
        earlier: InstId,
        later: InstId,
        predecessor: i64,
        memory: &MemRef,
        edge_memory: Option<&MemRef>,
    ) -> bool {
        let source = self.at(earlier);
        let boundary = if source.kind == Kind::Def { Some(source.id) } else { source.defining };
        boundary.is_some_and(|boundary| {
            self.frontier(later, memory, Some(boundary), Some(predecessor), edge_memory) == BTreeSet::from([boundary])
        })
    }
}

/// Wire block entries first, then eliminate identity memory phis.
///
/// Preallocating entries handles backedges without iterative guesses about
/// memory versions. A block no edge enters keeps an invocation input.
pub fn built<'a>(
    unit: &Unit<'a>,
    accesses: &Accesses,
) -> MemorySSA<'a> {
    let function = unit.function;
    let graph = cfg::graph(function);
    let entry = function.entry().map(cfg::id);
    let live = Access::new(0, Kind::Live);
    let entries: IndexMap<i64, usize> = graph.iter().enumerate().map(|(index, block)| (block.at, index + 1)).collect();
    let mut sites: IndexMap<InstId, Access> = IndexMap::default();
    let mut written: llrm_mir::dense::IdMap<InstId, Option<Rc<[MemRef]>>> = Default::default();
    let nothing: Rc<[MemRef]> = Rc::from(Vec::new());
    let mut outgoing: BTreeMap<i64, usize> = BTreeMap::new();
    let mut next_id = entries.len() + 1;
    for block in &graph {
        let mut current = entries[&block.at];
        for &inst in function.block(cfg::block(block.at)).instructions() {
            let writes = accesses.shared_writes(inst, &nothing);
            let defines = writes.as_ref().is_none_or(|stores| !stores.is_empty());
            if !defines && accesses.reads(inst).is_some_and(<[MemRef]>::is_empty) {
                continue;
            }
            let kind = if defines { Kind::Def } else { Kind::Use };
            sites.insert(
                inst,
                Access {
                    id: next_id,
                    kind,
                    block: Some(block.at),
                    site: Some(inst),
                    defining: Some(current),
                    incoming: Vec::new(),
                },
            );
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
            let mut edges: Vec<(Option<i64>, usize)> =
                parents.iter().map(|pred| (Some(*pred), outgoing[pred])).collect();
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
            let mut values: BTreeSet<usize> =
                incoming[block].iter().map(|(_, value)| resolved(&replacements, *value)).collect();
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
            access.incoming =
                incoming[block].iter().map(|(pred, value)| (*pred, resolved(&replacements, *value))).collect();
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
    let mut accesses: Vec<Access> =
        std::iter::once(live.clone()).chain(phis.values().cloned()).chain(sites.values().cloned()).collect();
    accesses.sort_by_key(|access| access.id);
    let span = spans(&accesses);
    MemorySSA {
        live,
        accesses,
        sites,
        phis,
        written,
        reaches: Default::default(),
        unit: *unit,
        known: None,
        clobbers: Default::default(),
        slots: Default::default(),
        jumps: Default::default(),
        span,
    }
}

#[cfg(test)]
#[path = "memoryssa_tests.rs"]
mod tests;
