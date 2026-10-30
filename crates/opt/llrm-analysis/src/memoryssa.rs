//! Memory SSA: a single memory state versioned by stores and writing calls,
//! with a walker that finds the writes a load may see.
//!
//! # How it works
//! `Accesses` records what each instruction touches: a load's or store's own
//! `MemRef`, a call's footprint (`alias::calls_by_attributes`: its attributes
//! and `GlobalsModRef`, no points-to solve), and the bytes a call fills before
//! reading any. A query of a call is the meet of the footprint and `AAResults`
//! (BasicAA, GlobalsAA). It is the manager's `accesses` analysis, built from
//! [`crate::manager`]'s `Annotated`, and kept across changes that only move
//! instructions or give a store another value (`update`).
//!
//! `built` numbers the accesses. Every instruction that writes (`None` writes
//! means anything) is a `Def`; one that only reads memory is a `Use`; one that
//! touches none is skipped. Each access names the access before it in its block
//! as `defining`. Each block starts in a memory `Phi` over its predecessors'
//! exit states, with the entry block also fed by `Live`, the state on entry to
//! the function. Phis are wired before they are resolved, so back edges need no
//! iteration over memory versions; then phis whose inputs reduce to one state
//! are replaced by it. What that makes (`Structure`) depends on the body and
//! its `Accesses` alone, so it is kept in the accesses, per body state, and
//! every graph built from them shares it: the walks' memory of what was
//! asked (and the registers they know) is each graph's own. The manager's
//! `PlainAccesses` is the one set `ThroughMemory` and `FloatFacts` share.
//!
//! Edits that make, erase or move no memory op or terminator, rewrite no
//! terminator and make or erase no block leave the structure as it was
//! (`structure_kept`): the manager's accesses carry it past them, and
//! `LLRM_CHECK_CLOBBERS` builds it afresh to compare.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   live["<b>entry: Live(0)</b>"]:::mid
//!   s1["<b>b1: Def s1</b>"]:::mid
//!   s2["<b>Def s2</b>"]:::mid
//!   phi["<b>b2: Phi</b><br/><small>[b1: s2, b3: s3]</small>"]:::mid
//!   s3["<b>b3: Def s3</b>"]:::mid
//!   use["<b>Use l1</b><br/><small>(defining = Phi)</small>"]:::mid
//!   live --> s1
//!   s1 -->|"defining"| s2
//!   s2 --> phi
//!   phi --> use
//!   phi --> s3
//!   s3 --> phi
//! ```
//!
//! *Memory SSA of a loop: b2's Phi merges b1's last Def and b3's Def*
//!
//! The walker (`clobbers`, `clobbers_at_end`, `nearest_writes`) answers, for a
//! `MemRef` read at a site, the set of nearest defs whose write may change it,
//! or `Live`. It steps up `defining` links and through every phi input,
//! skipping defs that `may_clobber` rules out: a constant or `invariant` read
//! is changed by nothing; two references at fixed displacements of one frame
//! whose bytes miss are settled by `regions::displaced_span`; otherwise
//! [`crate::pointerfacts`] disjointness is tried before `regions::overlapping`.
//! Per (cell, def) answers are remembered, and a walk jumps along chains of
//! accesses that leave the cell alone. After enough steps per access, a
//! persistent map from (object, byte) to the nearest def writing it replaces
//! stepping. A visited set closes cycles without taking a back edge as evidence
//! that memory is unchanged. An empty answer means no reachable source, not a
//! reusable value; the caller must establish value availability itself.
//! `nearest_writes` stops after `MAX_NEAREST_WRITES` (100) and the caller
//! serves nothing past that. `unchanged` and `available_on_edge` answer whether
//! an earlier access still supplies the bytes, the caller having proved
//! dominance and equal addresses.
//!
//! Predicates shared with the passes: `changes`, `spares`, `covers`/`covered`
//! (outer references certainly hold every byte of the inner), `placed`,
//! `same_bytes`, `byte_keys`, and `Writers`, which indexes writes by object.
//!
//! # Example
//! A diamond whose arms store to the same cell of `@g` and whose join loads it:
//!
//! ```mir
//! define i16 @f(i1 %c) {
//! b0:
//!   br i1 %c, label %b1, label %b2
//!
//! b1:
//!   store i16 1, ptr getelementptr (i8, ptr @g, i16 32)
//!   br label %b3
//!
//! b2:
//!   store i16 2, ptr getelementptr (i8, ptr @g, i16 32)
//!   br label %b3
//!
//! b3:
//!   %x = load i16, ptr getelementptr (i8, ptr @g, i16 32)
//!   ret i16 %x
//! }
//! ```
//!
//! | access      | kind  | incoming or defining                           |
//! |-------------|-------|------------------------------------------------|
//! | `b3` entry  | `Phi` | (`b1`, the `b1` store), (`b2`, the `b2` store) |
//! | `%x = load` | `Use` | the `b3` `Phi`                                 |
//!
//! From unit test `test_a_join_gets_one_memory_phi` in `memoryssa_tests.rs`.
//!
//! # Provides
//! `Accesses` (analysis `accesses`), `MemorySSA` via `built`, `Kind`, `Access`,
//! `Reach`, the predicates above. Consumers: [`crate::avail`],
//! [`crate::consts`], [`crate::floatfacts`], and in llrm-transforms `gvn`,
//! `dse`, `hoist`, `loadjoins`, `loopmotion`, `promote`, `spares`, `homes`,
//! `transform`.
//!
//! # Needs
//! [`crate::manager`] (`Annotated`, `PointerValues`, `ExposedFrames`),
//! [`crate::cfg`], [`crate::regions`], [`crate::pointerfacts`],
//! [`crate::memory`].
//!
//! # Keeps
//! Conservative: rebuilt after a function changes. `Accesses` survives moves.
//! `LLRM_CHECK_CLOBBERS` and `LLRM_CHECK_JUMPS` assert the fast paths against
//! the slow ones.
//!
//! # See also
//! LLVM `MemorySSA` and its `ClobberWalker`
//! (`llvm/lib/Analysis/MemorySSA.cpp`).

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::memory::stated;
use llrm_mir::module::{Change, Function, GlobalValue, InstId, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Analyses, Analysis};
use llrm_support::hash::IndexMap;

use crate::cfg;
use crate::consts::Calls;
use crate::graph::loops;
use crate::manager::Annotated;
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
    source: Option<Rc<<Annotated as Analysis>::Result>>,
    /// The calls' footprints `touched` holds for them were made from.
    footprint_source: Option<Rc<<crate::manager::CallFootprints as Analysis>::Result>>,
    /// What the manager's plain accesses were made from: the references
    /// annotated, the calls' writes, and the module facts that decide a call.
    plain_source: Option<PlainSource>,
    /// The graph's structure, made when first asked.
    graph: GraphCell,
    /// Nothing above is made up front: a reference and the lists an
    /// instruction touches are worked out of the unit when first asked
    /// (`reference_in`, `reads_in`, `writes_in`), as the pin computes a
    /// `MemoryLocation` when a walk wants one. The graph's clients ask 1.6 %
    /// of them.
    lazy: bool,
}

/// What `PlainAccesses` reads.
#[derive(Clone)]
pub struct PlainSource {
    pub annotated: Rc<<Annotated as Analysis>::Result>,
    pub writes: Rc<<crate::manager::Writes as Analysis>::Result>,
    pub outer: Rc<llrm_mir::passes::Outer>,
}

impl std::fmt::Debug for PlainSource {
    fn fmt(
        &self,
        out: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        out.write_str("PlainSource")
    }
}

impl PartialEq for PlainSource {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.annotated == other.annotated && self.writes == other.writes && Rc::ptr_eq(&self.outer, &other.outer)
    }
}

/// The `Structure` of the one body state these accesses are of, kept for
/// every graph built from them. A copy of the accesses starts without it, so
/// a copy that is changed does not carry it.
#[derive(Default)]
struct GraphCell(std::cell::RefCell<Option<(llrm_mir::module::Mark, Rc<Structure>)>>);

impl GraphCell {
    /// This cell's structure, as true of the body at `mark`.
    fn carried(
        &self,
        mark: llrm_mir::module::Mark,
    ) -> Self {
        Self(std::cell::RefCell::new(self.0.borrow().as_ref().map(|(_, kept)| (mark, Rc::clone(kept)))))
    }

    fn of(
        &self,
        function: &Function,
        make: impl FnOnce() -> Structure,
    ) -> Rc<Structure> {
        let mark = function.mark();
        if let Some((made, kept)) = &*self.0.borrow()
            && *made == mark
        {
            return Rc::clone(kept);
        }
        STRUCTURES.with(|made| made.set(made.get() + 1));
        let made = Rc::new(make());
        *self.0.borrow_mut() = Some((mark, Rc::clone(&made)));
        made
    }
}

impl Clone for GraphCell {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl std::fmt::Debug for GraphCell {
    fn fmt(
        &self,
        out: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        out.write_str("GraphCell")
    }
}

impl PartialEq for GraphCell {
    fn eq(
        &self,
        _: &Self,
    ) -> bool {
        true
    }
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
        let references = Result::as_ref(&*annotated).map_err(String::clone)?;
        let footprints_held = analyses.get::<crate::manager::CallFootprints>(context, layout, function);
        let footprints = Result::as_ref(&*footprints_held).map_err(String::clone)?;
        let shape = analyses.get::<crate::cfg::Shape>(context, layout, function);
        let exposed = analyses.get::<crate::manager::ExposedFrames>(context, layout, function);
        // The pointer value solve `Annotated` shares: the objects a call's
        // pointer arguments reach.
        let values = analyses.get::<crate::manager::PointerValues>(context, layout, function);
        let mut unit =
            Unit::within(context, layout, function, analyses.outer()).with_shape(&shape).with_exposed(&exposed);
        if let Ok(values) = &*values {
            unit = unit.with_point_values(values);
        }
        let made = Self::of(&unit, references.clone(), footprints);
        Ok(Rc::new(Self { source: Some(annotated), footprint_source: Some(footprints_held), ..made }))
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
        let was_annotated = held.source.as_ref()?;
        let inert = |inst: InstId| {
            let opcode = &function.instruction(inst).opcode;
            !llrm_mir::touched::touches_memory(opcode) && !opcode.is_terminator()
        };
        let only_moves = changes.iter().all(|change| match *change {
            // What an operand was is the record of a `Rewritten`, which
            // decides.
            Change::Moved { .. } | Change::Operand { .. } | Change::Operands { .. } | Change::Substituted { .. } => {
                true
            }
            Change::Rewritten(inst) => matches!(function.instruction(inst).opcode, Opcode::Store { .. }) || inert(inst),
            Change::Inserted { inst, .. } | Change::Erased { inst, .. } => inert(inst),
            Change::Cloned { to, .. } => inert(to),
            _ => false,
        });
        if !only_moves {
            return None;
        }
        let annotated = analyses.get::<Annotated>(context, layout, function);
        let unchanged = Rc::ptr_eq(&annotated, was_annotated) || *annotated == **was_annotated;
        if !unchanged {
            return None;
        }
        // A call's footprint names the values its arguments are made of; an
        // edit to those (a gep's index replaced) leaves the references alone.
        let footprints = analyses.get::<crate::manager::CallFootprints>(context, layout, function);
        let was_footprints = held.footprint_source.as_ref()?;
        if !(Rc::ptr_eq(&footprints, was_footprints) || *footprints == **was_footprints) {
            return None;
        }
        Some(if structure_kept(changes, function) {
            Ok(Rc::new(held.carried(function)))
        } else {
            // These accesses stay the result, shared; the structure they hold
            // is of the body before these edits, and a later edit
            // that leaves the structure alone would carry it over
            // as the body's.
            held.graph.0.borrow_mut().take();
            previous.clone()
        })
    }
}

/// Whether `changes` leave the graph's structure as it was, given that what
/// each access touches is: no memory op or terminator made, erased or moved
/// (the accesses' order and the blocks' edges are the structure), no
/// terminator rewritten, no block made or erased. LLVM's `MemorySSAUpdater`
/// leaves the same untouched.
pub(crate) fn structure_kept(
    changes: &[Change],
    function: &Function,
) -> bool {
    let inert = |inst: InstId| {
        let opcode = &function.instruction(inst).opcode;
        !llrm_mir::touched::touches_memory(opcode) && !opcode.is_terminator()
    };
    changes
        .iter()
        .all(
            |change| match *change {
                Change::Inserted { inst, .. } | Change::Erased { inst, .. } | Change::Moved { inst, .. } => inert(inst),
                Change::Cloned { to, .. } => inert(to),
                Change::Rewritten(inst) => !function.instruction(inst).opcode.is_terminator(),
                Change::Operand { .. } | Change::Operands { .. } | Change::Substituted { .. } => true,
                Change::BlockCreated(_) | Change::BlockErased(_) | Change::Parameters => false,
            },
        )
}

impl Accesses {
    pub fn plain_source(&self) -> Option<&PlainSource> {
        self.plain_source.as_ref()
    }

    pub fn with_plain_source(
        self,
        source: PlainSource,
    ) -> Self {
        Self { plain_source: Some(source), ..self }
    }

    /// These accesses for the body as it stands, with the structure they made
    /// of the body before.
    pub(crate) fn carried(
        &self,
        function: &Function,
    ) -> Self {
        let carried = Self { graph: self.graph.carried(function.mark()), ..self.clone() };
        if llrm_support::env_flag!("LLRM_CHECK_CLOBBERS")
            && let Some((_, kept)) = &*carried.graph.0.borrow()
        {
            let fresh = structure_of(function, &carried);
            let shown = |one: &Structure| {
                format!("{:?} {:?} {:?} {:?} {:?}", one.accesses, one.sites, one.phis, one.ends, one.span)
            };
            assert_eq!(shown(kept), shown(&fresh), "the structure kept past the edits is not the one the body makes");
        }
        carried
    }

    /// `unit`'s accesses as `alias` resolves them: each reference with its
    /// provenance.
    pub fn resolved(unit: &Unit) -> Result<Self, String> {
        Ok(Self::of(unit, unit.annotated()?.into_owned(), &crate::alias::calls_by_attributes(unit)))
    }

    /// `function`'s accesses, the manager's: from its `Annotated` and
    /// the unit's `GlobalsModRef`.
    pub fn managed(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Result<Rc<Self>, String> {
        (*analyses.get::<Accesses>(context, layout, function)).clone()
    }

    /// `unit`'s accesses from `references` (`alias::annotated`'s), each call
    /// reading and writing what its footprint in `calls` says (the manager's
    /// `CallFootprints`: its attributes and `GlobalsModRef`, no points-to
    /// solve), and the bytes each call initializes.
    pub fn of(
        unit: &Unit,
        references: IndexMap<InstId, MemRef>,
        calls: &llrm_mir::dense::IdMap<InstId, crate::alias::Effect>,
    ) -> Self {
        Self {
            fills: initialized_by_calls(unit),
            ..Self::new(unit, references, |inst| {
                calls.get(&inst).map(|effect| (Some(Rc::clone(&effect.loads)), Some(Rc::clone(&effect.stores))))
            })
        }
    }

    /// `unit`'s accesses unresolved: each reference as the unit has it
    /// (`Unit::reference`), each call writing its footprint in `calls`.
    pub fn plain(
        unit: &Unit,
        calls: &Calls,
    ) -> Self {
        let mut references =
            IndexMap::with_capacity_and_hasher(crate::memory::loads_and_stores(unit.function), Default::default());
        references.extend(unit.function.walk().filter_map(|(_, inst)| unit.reference(inst).map(|one| (inst, one))));
        Self::new(unit, references, |inst| calls.get(&inst).map(|stores| (None, Some(Rc::clone(stores)))))
    }

    /// The manager's accesses where `unit` has them (`Unit::with_plain`), else
    /// what `make` makes of it.
    pub fn shared_or<'u>(
        unit: &'u Unit,
        make: impl FnOnce() -> Self,
    ) -> std::borrow::Cow<'u, Self> {
        match unit.plain {
            Some(shared) => std::borrow::Cow::Borrowed(shared),
            None => std::borrow::Cow::Owned(make()),
        }
    }

    /// What `inst` reads and writes, as `new` has it: none where it touches
    /// no memory.
    fn touched_by(
        unit: &Unit,
        inst: InstId,
        reference: &dyn Fn() -> Rc<[MemRef]>,
        footprint: &impl Fn(InstId) -> Option<Footprint>,
    ) -> Option<Footprint> {
        let function = unit.function;
        let opcode = &function.instruction(inst).opcode;
        Some(match opcode {
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
            _ => return None,
        })
    }

    /// These accesses told of `inst`, an instruction made since: its reference
    /// as the unit has it and what it touches (a call writes what its
    /// attributes allow, as `plain` has it).
    pub fn add_instruction(
        &mut self,
        unit: &Unit,
        inst: InstId,
    ) {
        if self.lazy {
            return;
        }
        if let Some(reference) = unit.reference(inst) {
            self.references.insert(inst, reference);
        }
        let own = self.references.get(&inst).cloned();
        if let Some(found) = Self::touched_by(unit, inst, &|| own.clone().into_iter().collect::<Rc<[_]>>(), &|_| None) {
            self.touched.insert(inst, found);
        }
    }

    /// `footprint` gives a call's reads and writes, where known.
    fn new(
        unit: &Unit,
        references: IndexMap<InstId, MemRef>,
        footprint: impl Fn(InstId) -> Option<Footprint>,
    ) -> Self {
        let mut touched = IndexMap::default();
        for (_, inst) in unit.function.walk() {
            if let Some(found) = Self::touched_by(
                unit,
                inst,
                &|| references.get(&inst).cloned().into_iter().collect::<Rc<[_]>>(),
                &footprint,
            ) {
                touched.insert(inst, found);
            }
        }
        Self {
            references,
            touched,
            fills: IndexMap::default(),
            source: None,
            footprint_source: None,
            plain_source: None,
            graph: GraphCell::default(),
            lazy: false,
        }
    }

    /// Accesses that make nothing up front (`lazy`): the references as the unit
    /// has them, each call writing anything its attributes allow.
    pub fn lazy() -> Self {
        Self { lazy: true, ..Self::default() }
    }

    /// Whether these accesses make nothing up front (`lazy`).
    pub fn is_lazy(&self) -> bool {
        self.lazy
    }

    /// The access `inst` makes, as the unit has it (`Unit::reference`), made
    /// when asked where these accesses are lazy.
    pub fn reference_in(
        &self,
        unit: &Unit,
        inst: InstId,
    ) -> Option<Cow<'_, MemRef>> {
        if self.lazy {
            return unit.reference(inst).map(Cow::Owned);
        }
        self.references.get(&inst).map(Cow::Borrowed)
    }

    /// What `inst` touches, made when asked where these accesses are lazy.
    fn touched_in(
        &self,
        unit: &Unit,
        inst: InstId,
    ) -> Option<Footprint> {
        Self::touched_by(unit, inst, &|| unit.reference(inst).into_iter().collect::<Rc<[_]>>(), &|_| None)
    }

    /// [`reads`](Self::reads), asked of the unit where these accesses are lazy.
    pub fn reads_in(
        &self,
        unit: &Unit,
        inst: InstId,
    ) -> Option<Cow<'_, [MemRef]>> {
        if self.lazy {
            LAZY_LISTS.with(|made| made.set(made.get() + 1));
            return match self.touched_in(unit, inst) {
                None => Some(Cow::Borrowed(&[])),
                Some((reads, _)) => reads.map(|list| Cow::Owned(list.to_vec())),
            };
        }
        self.reads(inst).map(Cow::Borrowed)
    }

    /// [`writes`](Self::writes), asked of the unit where these accesses are
    /// lazy.
    pub fn writes_in(
        &self,
        unit: &Unit,
        inst: InstId,
    ) -> Option<Cow<'_, [MemRef]>> {
        if self.lazy {
            LAZY_LISTS.with(|made| made.set(made.get() + 1));
            return match self.touched_in(unit, inst) {
                None => Some(Cow::Borrowed(&[])),
                Some((_, writes)) => writes.map(|list| Cow::Owned(list.to_vec())),
            };
        }
        self.writes(inst).map(Cow::Borrowed)
    }

    /// These accesses with the stores `absolute` names writing nothing a
    /// program object holds: a far store through a selector taken to be an
    /// absolute segment (`consts::_selector`).
    pub fn without_absolute_writes(
        mut self,
        absolute: &llrm_mir::dense::IdSet<InstId>,
    ) -> Self {
        for inst in absolute.iter() {
            if let Some((_, writes)) = self.touched.get_mut(&inst) {
                *writes = Some(none());
            }
        }
        self
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

    /// [`fills`](Self::fills), asked of the unit where these accesses are lazy.
    pub fn fills_in(
        &self,
        unit: &Unit,
        inst: InstId,
    ) -> Cow<'_, [MemRef]> {
        if self.lazy {
            return Cow::Owned(initialized_by(unit, inst));
        }
        Cow::Borrowed(self.fills(inst))
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
    // Two references of one frame at fixed displacements, their bytes apart:
    // what `regions::overlapping` settles first of everything it does, and
    // most of the writes a walk passes.
    if let (Some((frame, low, high)), Some((other_frame, other_low, other_high))) =
        (crate::regions::displaced_span(cell), crate::regions::displaced_span(store))
        && frame == other_frame
        && !(low < other_high && other_low < high)
    {
        assert!(
            !check_clobbers() || !may_clobber_slowly(unit, known, cell, store, None),
            "LLRM_CHECK_CLOBBERS: bytes apart in one frame were found to overlap"
        );
        return false;
    }
    may_clobber_slowly(unit, known, cell, store, None)
}

/// The full pair test. `remembered`, where given, answers what a pointer is a
/// constant byte offset from: a walk asks of one cell against many writes, and
/// the chain of a pointer is the same each time.
fn may_clobber_slowly(
    unit: &Unit,
    known: Option<&BTreeMap<ValueId, Interval>>,
    cell: &MemRef,
    store: &MemRef,
    remembered: Option<&dyn Fn(Operand) -> Option<(Operand, i64)>>,
) -> bool {
    let relative = |pointer| match remembered {
        Some(remembered) => remembered(pointer),
        None => pointerfacts::offsets(unit.context, unit.layout, unit.function).relative(pointer),
    };
    #[cfg(test)]
    SLOW_CLOBBERS.with(|asked| asked.set(asked.get() + 1));
    let apart = match (located(cell), located(store)) {
        (Some(one), Some(other)) if one.bytes != 0 && other.bytes != 0 => {
            match (relative(one.pointer), relative(other.pointer)) {
                (Some(left), Some(right)) if left.0 == right.0 => {
                    left.1 + one.bytes as i64 <= right.1 || right.1 + other.bytes as i64 <= left.1
                }
                _ => false,
            }
        }
        _ => false,
    };
    // An answer Rust cannot represent is taken to overlap.
    !apart && overlapping(cell, store, known, known, unit.program).unwrap_or(true)
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

/// One of a reference's `ByteKeys`, to hash.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ByteKey {
    Frame(crate::regions::Frame, i128, i128),
    Pointer(llrm_mir::module::Operand, i64, u64),
}

impl ByteKeys {
    /// The keys it has: none, one or two.
    pub fn list(self) -> Vec<ByteKey> {
        let frame = self.frame.map(|(frame, low, high)| ByteKey::Frame(frame, low, high));
        let pointer = self.pointer.map(|(base, offset, bytes)| ByteKey::Pointer(base, offset, bytes));
        frame.into_iter().chain(pointer).collect()
    }
}

/// Whether `one` and `other` certainly name the same bytes.
pub fn same_bytes(
    unit: &Unit,
    one: &MemRef,
    other: &MemRef,
) -> bool {
    // One pointer and one width are the same bytes whatever else is known of
    // it: the scalar a pass reads and writes through the same address, which
    // every load of N joins asks of every store that reaches it.
    if one.pointer.is_some() && one.pointer == other.pointer && one.width == other.width && one.disp == other.disp {
        if llrm_support::env_flag!("LLRM_CHECK_SAMEBYTES") {
            assert!(
                covers(unit, one, other) && covers(unit, other, one),
                "same_bytes: one pointer, width and displacement are not the same bytes: {one:?} {other:?}"
            );
        }
        return true;
    }
    covers(unit, one, other) && covers(unit, other, one)
}

thread_local! {
    static RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static LAZY_LISTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many read or write lists this thread's lazy accesses have derived for
/// an instruction (`reads_in`, `writes_in`), for a test that a client asking
/// only whether a load or store writes derives none.
pub fn lazy_lists() -> usize {
    LAZY_LISTS.with(std::cell::Cell::get)
}

thread_local! {
    static STRUCTURES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many graph structures this thread has made (`built` of accesses and a
/// body state it has not seen).
pub fn structures_made() -> usize {
    STRUCTURES.with(std::cell::Cell::get)
}

/// How many times this thread has asked whether a def's writes may clobber a
/// cell, rather than been answered from memory.
pub fn clobber_runs() -> usize {
    RUNS.with(std::cell::Cell::get)
}

/// The index is made once walks have stepped over this many accesses for each
/// access there is: a body that is walked a little pays for the walks, not for
/// the index.
const STEPS_PER_ACCESS: usize = 2;
/// The bytes of an object the index keeps.
const OBJECT_BYTES: i128 = 1 << 24;
/// The widest write or read an index of bytes holds.
const WIDEST: i128 = 64;

/// The nearest write to each byte of every exact cell, above each access: a
/// persistent map from (object, byte) to def, a version of it per access (a def
/// that writes one cell exactly makes a new one by changing its bytes;
/// everything else keeps its parent's), as LLVM's `OptimizeUses` keeps a stack
/// of the last def of each location down its walk of the tree. A load of one of
/// N cells then reads its bytes and not the N stores before it. GCC's
/// `walk_aliased_vdefs` and LLVM's `memssa-check-limit` bound the walk instead;
/// this does not give an answer up. Made once walks have stepped over enough
/// accesses to pay for it.
#[derive(Default)]
struct FrameIndex {
    /// Per access, the nearest def writing each (object, byte) above it and
    /// itself.
    at: Vec<Option<llrm_mir::dense::ShareMap<usize, usize>>>,
    /// Per access, the nearest def above it and itself that is no exact cell:
    /// one the walk asks of any read.
    wild: Vec<Option<usize>>,
    /// Per access, the nearest def above it and itself that writes an exact
    /// cell of an object that may be addressed from elsewhere: one the walk
    /// asks of a read of another addressed object.
    escaping: Vec<Option<usize>>,
    /// Per access, the same strictly above it.
    up_wild: Vec<Option<usize>>,
    up_escaping: Vec<Option<usize>>,
    /// Per access, the phi or entry its chain ends in.
    root: Vec<usize>,
    /// The addressed objects some def writes a cell of, and for an object read
    /// whether another of them may alias it (asked once per object).
    addressed: Vec<ObjectRef>,
    aliased: std::cell::RefCell<llrm_support::hash::HashMap<u32, bool>>,
}

/// The key of a byte of an object in the index.
fn key(
    object: u32,
    byte: i128,
) -> usize {
    ((object as usize) << 24) | byte as usize
}

/// The one object a reference certainly points into, and the bytes it covers
/// there, where its provenance names a single offset (`Slice::low` with `high`
/// one past): the same cell is the same object and bytes however its pointer
/// was made.
fn exact_object(reference: &MemRef) -> Option<(ObjectRef, i128, i128)> {
    let provenance = reference.provenance.as_ref()?;
    let mut slices = provenance.slices.iter();
    let (Some(slice), None) = (slices.next(), slices.next()) else { return None };
    (slice.high == slice.low + 1 && reference.width > 0)
        .then(|| (slice.object, i128::from(slice.low), i128::from(slice.low) + i128::from(reference.width)))
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
    *ON.get_or_init(|| llrm_support::env_flag!("LLRM_CHECK_JUMPS"))
}

fn check_clobbers() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| llrm_support::env_flag!("LLRM_CHECK_CLOBBERS"))
}

#[cfg(test)]
thread_local! {
    /// Writes asked of `may_clobber` by the walks: the alias reasoning the
    /// object summary spares.
    pub(crate) static MAY_CLOBBERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(crate) static SLOW_CLOBBERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(crate) static UNCHANGED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Accesses the walks stepped over.
    pub(crate) static STEPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
thread_local! {
    /// Times a cell was looked up by value (hashed whole) rather than by
    /// number, for a test that a walk does it once.
    pub(crate) static CELL_PROBES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// What the graph is made of, from the function and its `Accesses` alone: the
/// accesses, the phis, what each def writes and the order of the tree. The
/// registers a client knows change what a walk answers, not any of this, so
/// every client of one body state shares it (`Accesses::graph`).
pub struct Structure {
    pub live: Access,
    pub accesses: Vec<Access>,
    pub sites: IndexMap<InstId, Access>,
    pub phis: IndexMap<i64, Access>,
    /// What each def writes, as `Accesses::writes` says.
    written: llrm_mir::dense::IdMap<InstId, Option<Rc<[MemRef]>>>,
    /// What each def's writes can reach, worked out when a walk first asks of
    /// the def.
    reaches: std::cell::RefCell<llrm_mir::dense::IdMap<InstId, Rc<Reach>>>,
    /// The state each block leaves.
    ends: IndexMap<i64, Option<usize>>,
    /// Each access's entry and exit in a depth-first order of the tree its
    /// `defining` links make, by access number: one is behind another when
    /// its interval holds the other's.
    span: Vec<(u32, u32)>,
    /// Of each def that writes one cell at a fixed displacement, the frame and
    /// bytes (`regions::displaced_span`): a cell at a fixed displacement of the
    /// same frame whose bytes miss them is not written, settled without asking
    /// `may_clobber` or remembering the answer.
    fixed: llrm_mir::dense::IdMap<InstId, (crate::regions::Frame, i128, i128)>,
    /// Of each def that writes one cell its pointer's provenance places
    /// exactly, the object and bytes: a read placed exactly elsewhere in
    /// the same object, or in one it cannot alias, is not written.
    exacts: llrm_mir::dense::IdMap<InstId, (ObjectRef, i128, i128)>,
}

#[derive(Clone)]
pub struct MemorySSA<'a> {
    structure: Rc<Structure>,
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
    /// Per access, the walk that last visited it, and the walks made.
    stamps: std::cell::RefCell<(Vec<u32>, u32)>,
    /// The last-write index, once made, and the accesses walks have stepped
    /// over.
    index: std::cell::RefCell<Option<Rc<FrameIndex>>>,
    taken: std::cell::Cell<usize>,
    /// Whether a walk stopped at `MAX_WALK_STEPS`.
    stopped: std::cell::Cell<bool>,
    /// What each pointer asked about is a constant byte offset from.
    offsets: std::cell::RefCell<llrm_mir::dense::IdMap<ValueId, Option<(Operand, i64)>>>,
    /// While a walk is bounded (`nearest_writes`): the most writes it collects
    /// before it stops.
    limit: std::cell::Cell<Option<usize>>,
}

/// The nearest writes a walk collects for a caller that serves a load from
/// them: past this it stops and the caller serves nothing. A scalar written in
/// each arm of N joins is reached by N of them, N^2 over the body (`joins` 256:
/// 10% of the compile). LLVM's walker stops at `memssa-check-limit` (100)
/// checks, gcc's value numbering at `sccvn-max-alias-queries-per-access`
/// (1000).
pub const MAX_NEAREST_WRITES: usize = 100;

/// Accesses one walk steps over before it answers with the access it stopped
/// at, as a clobber (LLVM's walker stops at `memssa-check-limit` checks). The
/// corpus' longest walk is 584 steps; 500 changes no object, 100 changes two.
pub const MAX_WALK_STEPS: usize = 1000;

impl std::ops::Deref for MemorySSA<'_> {
    type Target = Structure;

    fn deref(&self) -> &Structure {
        &self.structure
    }
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

    /// The possible nearest writes at the end of `block`, as a load placed
    /// there would find them.
    pub fn clobbers_at_end(
        &self,
        block: i64,
        memory: &MemRef,
    ) -> BTreeSet<usize> {
        self.walked_from(Some(block), self.ends[&block], false, memory, None, None, None, true)
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

    /// `clobbers_ignoring_invariance`, stopped once more than
    /// `MAX_NEAREST_WRITES` are found: a set longer than that is incomplete,
    /// and its length is all a caller reads of it.
    pub fn nearest_writes(
        &self,
        site: InstId,
        memory: &MemRef,
    ) -> BTreeSet<usize> {
        self.limit.set(Some(MAX_NEAREST_WRITES));
        let found = self.frontier_for(site, memory, None, None, None, false);
        self.limit.set(None);
        found
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
        #[cfg(test)]
        UNCHANGED.with(|asked| asked.set(asked.get() + 1));
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
            let found = match self.call_clobbers(site, cell) {
                Some(asked)
                    if stores.is_empty()
                        || !matches!(
                            self.unit.function.instruction(site).opcode,
                            Opcode::Store { .. }
                        ) =>
                {
                    asked
                }
                // The stack knows no far segments or byte ranges of a store:
                // either answer apart rules it out.
                Some(asked) => asked && stores.iter().any(|store| self.may_clobber_cached(cell, store)),
                None => stores.iter().any(|store| self.may_clobber_cached(cell, store)),
            };
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

    /// `changes` for the def at `site`: a call with a pointer-bearing cell is
    /// the AA stack's to rule out, even where its footprint says anything;
    /// the writes of the footprint decide the rest.
    fn site_changes(
        &self,
        cell: &MemRef,
        invariant: bool,
        site: InstId,
        slot: usize,
        access: usize,
    ) -> bool {
        if cell.pointer.is_some()
            && !invariant
            && !cell.unwritable()
            && matches!(
                self.unit.function.instruction(site).opcode,
                Opcode::Call(_) | Opcode::Invoke(_)
            )
        {
            return self.clobbered(slot, cell, access, site);
        }
        changes(cell, invariant, self.written[&site].as_deref(), |_| self.clobbered(slot, cell, access, site))
    }

    /// Whether the call or store `site` may write `cell`, as the pin's AA
    /// stack answers `getModRefInfo(Inst, Loc)` (BasicAA, TBAA, GlobalsAA;
    /// MemorySSA.cpp `instructionClobbersQuery` L320); none for another
    /// instruction or a cell with no pointer, which the footprint answers.
    /// The cell carries no TBAA tag, so the store's own tag rules nothing out.
    fn call_clobbers(
        &self,
        site: InstId,
        cell: &MemRef,
    ) -> Option<bool> {
        use crate::aa::{AAQueryInfo, AAResults, LocationSize, MemoryLocation};
        if !matches!(
            self.unit.function.instruction(site).opcode,
            Opcode::Call(_) | Opcode::Invoke(_) | Opcode::Store { .. }
        ) {
            return None;
        }
        let pointer = cell.pointer?;
        // A cell cut from its pointer (another offset or width) names bytes
        // the pointer does not; the footprint answers for it.
        if matches!(
            self.unit.function.instruction(site).opcode,
            Opcode::Store { .. }
        )
            && MemRef::at(&self.unit, pointer, cell.width) != *cell
        {
            return None;
        }
        let location = MemoryLocation::new(pointer, LocationSize::Precise(u64::from(cell.width)));
        let mut aggregate = crate::aggregate::AggregateAA::new();
        aggregate.add_aa_result(crate::basicaa::BasicAA::new(&self.unit));
        aggregate.add_aa_result(crate::typebasedaa::TypeBasedAA::new(&self.unit));
        if let Some(info) = self.unit.globals_mr {
            aggregate.add_aa_result(crate::globalsmodref::GlobalsAAResult::new(&self.unit, info));
        }
        Some(aggregate.get_mod_ref_info(site, Some(&location), &mut AAQueryInfo::default()).is_mod_set())
    }

    /// `may_clobber`, with the offsets of the pointers remembered.
    fn may_clobber_cached(
        &self,
        cell: &MemRef,
        store: &MemRef,
    ) -> bool {
        // Two references of one frame at fixed displacements, their bytes
        // apart.
        if let (Some((frame, low, high)), Some((other_frame, other_low, other_high))) =
            (crate::regions::displaced_span(cell), crate::regions::displaced_span(store))
            && frame == other_frame
            && !(low < other_high && other_low < high)
        {
            assert!(
                !check_clobbers() || !may_clobber_slowly(&self.unit, self.known.as_ref(), cell, store, None),
                "LLRM_CHECK_CLOBBERS: bytes apart in one frame were found to overlap"
            );
            return false;
        }
        let found =
            may_clobber_slowly(&self.unit, self.known.as_ref(), cell, store, Some(&|pointer| self.relative(pointer)));
        assert!(
            !check_clobbers() || found == may_clobber_slowly(&self.unit, self.known.as_ref(), cell, store, None),
            "LLRM_CHECK_CLOBBERS: the offsets remembered differ from the ones worked out"
        );
        found
    }

    /// `pointerfacts::Offsets::relative` of `pointer`, once for each.
    fn relative(
        &self,
        pointer: Operand,
    ) -> Option<(Operand, i64)> {
        let Operand::Value(value) = pointer else {
            return pointerfacts::offsets(self.unit.context, self.unit.layout, self.unit.function).relative(pointer);
        };
        if let Some(found) = self.offsets.borrow().get(&value) {
            return *found;
        }
        let found = pointerfacts::offsets(self.unit.context, self.unit.layout, self.unit.function).relative(pointer);
        self.offsets.borrow_mut().insert(value, found);
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

    /// The index, made once walks have stepped over enough accesses.
    fn index_ready(&self) -> Option<Rc<FrameIndex>> {
        if let Some(found) = self.index.borrow().as_ref() {
            return Some(Rc::clone(found));
        }
        if self.taken.get() < STEPS_PER_ACCESS * self.span.len() {
            return None;
        }
        let made = Rc::new(self.built_index());
        *self.index.borrow_mut() = Some(Rc::clone(&made));
        Some(made)
    }

    fn built_index(&self) -> FrameIndex {
        let top = self.span.len();
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); top];
        let mut roots = Vec::new();
        for access in &self.accesses {
            match access.defining {
                Some(parent) => children[parent].push(access.id),
                None => roots.push(access.id),
            }
        }
        let mut index = FrameIndex {
            at: vec![None; top],
            wild: vec![None; top],
            escaping: vec![None; top],
            up_wild: vec![None; top],
            up_escaping: vec![None; top],
            root: vec![0; top],
            addressed: Vec::new(),
            aliased: Default::default(),
        };
        let mut seen_addressed = std::collections::BTreeSet::new();
        let mut stack: Vec<usize> = Vec::new();
        for &root in &roots {
            index.at[root] = Some(Default::default());
            index.root[root] = root;
            stack.push(root);
            while let Some(node) = stack.pop() {
                for &child in &children[node] {
                    let access = self.access(child);
                    let mut version = index.at[node].clone();
                    let (mut wild, mut escaping) = (index.wild[node], index.escaping[node]);
                    if access.kind == Kind::Def
                        && let Some(site) = access.site
                    {
                        match self.exacts.get(&site) {
                            Some((object, first, last))
                                if last - first <= WIDEST && *first >= 0 && *last <= OBJECT_BYTES =>
                            {
                                let map = version.as_mut().expect("a version");
                                for byte in *first..*last {
                                    map.insert(key(object.id(), byte), child);
                                }
                                if object.addressed() {
                                    escaping = Some(child);
                                    if seen_addressed.insert(object.id()) {
                                        index.addressed.push(*object);
                                    }
                                }
                            }
                            _ => wild = Some(child),
                        }
                    }
                    index.up_wild[child] = index.wild[node];
                    index.up_escaping[child] = index.escaping[node];
                    index.at[child] = version;
                    index.wild[child] = wild;
                    index.escaping[child] = escaping;
                    index.root[child] = index.root[node];
                    stack.push(child);
                }
            }
        }
        index
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
        self.stopped.set(false);
        let found = self.walked(site, memory, boundary, edge, edge_memory, true, honor);
        if check_jumps() {
            let stepped = self.walked(site, memory, boundary, edge, edge_memory, false, honor);
            let cut = self.limit.get().is_some_and(|limit| found.len() > limit && stepped.len() > limit);
            // A jump skips steps, so the two walks reach the cap at different
            // places.
            assert!(
                found == stepped || cut || self.stopped.get(),
                "LLRM_CHECK_JUMPS: a walk that jumps found other than a walk step by step"
            );
        }
        found
    }

    /// Whether `ahead` is `access` or reached from it by `defining` links:
    /// MemorySSA's `dominates(ahead, access)`.
    pub fn dominates(
        &self,
        ahead: usize,
        access: usize,
    ) -> bool {
        self.behind(access, ahead)
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
        // A load of what is written once, then never: no write changes what it
        // reads.
        let invariant =
            honor && llrm_mir::memory::invariant_load(self.unit.context, self.unit.layout, self.unit.function, site);
        self.walked_from(
            self.at(site).block,
            self.at(site).defining,
            invariant,
            memory,
            boundary,
            edge,
            edge_memory,
            jumping,
        )
    }

    /// `walked` from the memory state `defining`, which a site or the end of a
    /// block is in.
    fn walked_from(
        &self,
        block: Option<i64>,
        defining: Option<usize>,
        invariant: bool,
        memory: &MemRef,
        boundary: Option<usize>,
        edge: Option<i64>,
        edge_memory: Option<&MemRef>,
        jumping: bool,
    ) -> BTreeSet<usize> {
        let memory_span = displaced_span(memory);
        let edge_span = edge_memory.and_then(displaced_span);
        let memory_exact = exact_object(memory);
        let edge_exact = edge_memory.and_then(exact_object);
        let mut pending = vec![defining];
        // A fixed cell of a frame with many fixed writes: the nearest write of
        // it is the index's, past the writes `fixed` rules apart.
        if jumping
            && boundary.is_none()
            && edge.is_none()
            && edge_memory.is_none()
            && !invariant
            && let Some((object, low, high)) = memory_exact
            && high - low <= WIDEST
            && low >= 0
            && high <= OBJECT_BYTES
            && let Some(index) = self.index_ready()
            && let Some(above) = defining
            && let Some(version) = index.at[above].as_ref()
        {
            let nearest = (low..high)
                .filter_map(|byte| version.get(&key(object.id(), byte)).copied())
                .max_by_key(|found| self.span[*found].0);
            let root = index.root[above];
            let reaches_others = object.addressed() && {
                let known = index.aliased.borrow().get(&object.id()).copied();
                known.unwrap_or_else(|| {
                    let found = index
                        .addressed
                        .iter()
                        .any(|other| other.id() != object.id() && crate::memory::objects_may_alias(other, &object));
                    index.aliased.borrow_mut().insert(object.id(), found);
                    found
                })
            };
            let (mut wild, mut escaping) =
                (index.wild[above], if reaches_others { index.escaping[above] } else { None });
            let mut hit = None;
            // The writes nearer than the exact one that may reach the cell,
            // nearest first: the walk asks each, as it asks any def.
            loop {
                let (candidate, is_wild) = match (wild, escaping) {
                    (Some(w), Some(e)) => {
                        if self.behind(w, e) {
                            (w, true)
                        } else {
                            (e, false)
                        }
                    }
                    (Some(w), None) => (w, true),
                    (None, Some(e)) => (e, false),
                    (None, None) => break,
                };
                if nearest.is_some_and(|nearest| !self.behind(candidate, nearest)) {
                    break;
                }
                if is_wild {
                    wild = index.up_wild[candidate];
                } else {
                    escaping = index.up_escaping[candidate];
                }
                let at = self.access(candidate).site.expect("a def has a site");
                if !is_wild
                    && self.exacts.get(&at).is_none_or(|(written, _, _)| {
                        written.id() == object.id() || !crate::memory::objects_may_alias(written, &object)
                    })
                {
                    continue;
                }
                let slot = self.slot(memory);
                if self.site_changes(memory, invariant, at, slot, candidate) {
                    hit = Some(candidate);
                    break;
                }
            }
            // The exact one is a write of those bytes; whether it clobbers the
            // read is the alias rules' (a type may apart them):
            // where it does not, the walk goes on.
            let confirmed = hit.is_some()
                || nearest.is_none_or(|found| {
                    let at = self.access(found).site.expect("a def has a site");
                    let slot = self.slot(memory);
                    self.site_changes(memory, invariant, at, slot, found)
                });
            if confirmed {
                if let Some(found) = hit.or(nearest) {
                    return BTreeSet::from([found]);
                }
                pending = vec![Some(root)];
            }
        }
        // Visited accesses by stamp: a set built per walk was most of its cost.
        let mut stamps = self.stamps.borrow_mut();
        if stamps.0.len() < self.span.len() {
            stamps.0.resize(self.span.len(), 0);
        }
        stamps.1 += 1;
        let epoch = stamps.1;
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
        let started = self.taken.get();
        while let Some(current) = pending.pop() {
            if self.limit.get().is_some_and(|limit| found.len() > limit) {
                break;
            }
            self.taken.set(self.taken.get() + 1);
            #[cfg(test)]
            STEPS.with(|steps| steps.set(steps.get() + 1));
            let Some(current) = current else {
                chain.clear();
                continue;
            };
            if self.taken.get() - started > MAX_WALK_STEPS {
                self.stopped.set(true);
                found.insert(current);
                break;
            }
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
            if std::mem::replace(&mut stamps.0[current], epoch) == epoch {
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
                    let site = access.site.expect("a def has a site");
                    let fixed_apart = match (
                        self.fixed.get(&site),
                        if std::ptr::eq(queried, memory) { &memory_span } else { &edge_span },
                    ) {
                        (Some((frame, low, high)), Some((cell_frame, cell_low, cell_high))) => {
                            frame == cell_frame && !(cell_low < high && low < cell_high)
                        }
                        _ => false,
                    };
                    // A write placed exactly: apart from a read placed exactly
                    // in the same object by its bytes, from
                    // one in another object that cannot alias it by the
                    // objects.
                    let apart = fixed_apart
                        || match (
                            self.exacts.get(&site),
                            if std::ptr::eq(queried, memory) { &memory_exact } else { &edge_exact },
                        ) {
                            (Some((written, low, high)), Some((read, cell_low, cell_high))) => {
                                if written.id() == read.id() {
                                    !(cell_low < high && low < cell_high)
                                } else {
                                    !crate::memory::objects_may_alias(written, read)
                                }
                            }
                            _ => false,
                        };
                    if !apart && self.site_changes(queried, invariant, site, queried_slot, current) {
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
    let structure = accesses
        .graph
        .of(
            unit.function,
            || llrm_support::debug::timed("memoryssa structure", || structure_of(unit.function, accesses)),
        );
    MemorySSA {
        structure,
        unit: *unit,
        known: None,
        clobbers: Default::default(),
        slots: Default::default(),
        jumps: Default::default(),
        stamps: Default::default(),
        index: Default::default(),
        taken: Default::default(),
        stopped: Default::default(),
        offsets: Default::default(),
        limit: Default::default(),
    }
}

fn structure_of(
    function: &Function,
    accesses: &Accesses,
) -> Structure {
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
        for inst in function.iter_block(cfg::block(block.at)) {
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
    let fixed = written
        .iter()
        .filter_map(|(site, writes)| match writes.as_deref() {
            Some([only]) => displaced_span(only).map(|span| (site, span)),
            _ => None,
        })
        .collect();
    let exacts = written
        .iter()
        .filter_map(|(site, writes)| match writes.as_deref() {
            Some([only]) => exact_object(only).map(|found| (site, found)),
            _ => None,
        })
        .collect();
    let ends = outgoing.iter().map(|(block, state)| (*block, Some(resolved(&replacements, *state)))).collect();
    Structure { ends, live, accesses, sites, phis, written, reaches: Default::default(), span, fixed, exacts }
}

#[cfg(test)]
#[path = "memoryssa_tests.rs"]
mod tests;

/// What some instructions write, indexed by object: whether they spare a
/// read is asked of the writers that may reach it (`regions::Index`), not of
/// every one.
pub struct Writers<'a> {
    index: crate::regions::Index<'a, InstId>,
    /// One of them writes anything.
    anywhere: bool,
}

impl<'a> Writers<'a> {
    pub fn of(
        accesses: &'a Accesses,
        insts: &[InstId],
    ) -> Self {
        let mut index = crate::regions::Index::default();
        let mut anywhere = false;
        for &inst in insts {
            match accesses.writes(inst) {
                None => anywhere = true,
                Some(writes) => writes.iter().for_each(|write| index.push(inst, write)),
            }
        }
        Self { index, anywhere }
    }

    /// `spares` of every one of them.
    pub fn spare(
        &self,
        accesses: &Accesses,
        program: Option<&llrm_mir::program::ProgramProxy>,
        read: &MemRef,
    ) -> bool {
        read.unwritable()
            || (!self.anywhere && self.index.all_near(read, |writer, _| spares(accesses, program, read, writer)))
    }
}

/// The bytes each call of `unit` writes through an argument before it reads
/// any: `initializes` at the site or on the callee's parameter.
fn initialized_by_calls(unit: &Unit) -> IndexMap<InstId, Vec<MemRef>> {
    let mut out = IndexMap::default();
    for (_, inst) in unit.function.walk() {
        let fills = initialized_by(unit, inst);
        if !fills.is_empty() {
            out.insert(inst, fills);
        }
    }
    out
}

/// The bytes the call `inst` writes through an argument before it reads any
/// (`initializes`); nothing for another instruction.
fn initialized_by(
    unit: &Unit,
    inst: InstId,
) -> Vec<MemRef> {
    let op = unit.function.instruction(inst);
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &op.opcode else { return Vec::new() };
    let callee = llrm_mir::memory::callee(unit.context, unit.function, inst)
        .and_then(|one| unit.globals.get(one.0 as usize))
        .and_then(GlobalValue::function);
    let mut fills = Vec::new();
    for (argument, &operand) in op.operands.iter().enumerate().take(info.argument_attrs.len()) {
        let declared = callee.and_then(|one| one.parameter_attrs.get(argument)).map_or(&[][..], Vec::as_slice);
        let byval = |attrs: &[llrm_mir::opcode::Attribute]| {
            attrs
                .iter()
                .any(
                    |one| matches!(
                        one,
                        llrm_mir::opcode::Attribute::Flag(name) | llrm_mir::opcode::Attribute::Type(name, _) if name == "byval"
                    ),
                )
        };
        if byval(&info.argument_attrs[argument]) || byval(declared) {
            continue;
        }
        for &(low, high) in llrm_mir::memory::initializes(&info.argument_attrs[argument])
            .iter()
            .chain(llrm_mir::memory::initializes(declared))
        {
            let Ok(width) = u32::try_from(high - low) else { continue };
            let mut one = MemRef::at(unit, operand, width);
            one.disp += low;
            fills.push(one);
        }
    }
    fills
}
