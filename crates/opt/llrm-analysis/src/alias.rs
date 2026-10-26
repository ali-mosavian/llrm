//! Strong alias analysis: what each pointer may point to, what escapes and
//! when, and what each call reads and writes.
//!
//! # How it works
//! One vocabulary serves every frontend. A pointer's fact is a `Provenance`: a
//! set of `Slice`s (a canonical object, a byte range of it, a stride and an
//! access width) plus `restrict` roots. Objects are the function's own
//! (`memory::object_of`: frame `alloca`s, globals), each pointer parameter's
//! `Parameter` object (offset 0; a `noalias` parameter is also a restrict
//! root, `seeds`) and, for a call whose callee returns `noalias`, a fresh
//! `Allocation`. Unknown stores kill spill facts; they never manufacture a
//! disjointness proof.
//!
//! Value solve (`point_values`). Pointer facts flow over the values to a fixed
//! point, by gcc's worklist over constraints (`solve_graph`,
//! [tree-ssa-structalias.cc#L2711](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-structalias.cc#L2711)):
//! an instruction is visited again only when a value it reads, or the fields of
//! an object it loads from, changed. The solve is flow-insensitive through
//! memory: one abstract location per object.
//!
//! - `alloca`, `bitcast`, `addrspacecast`, `getelementptr` give a fact from
//!   their operand (`_direct`); a constant displacement shifts the slices, a
//!   variable index widens to the whole object.
//! - A `phi` or `select` joins its arms as a union; a pointer with an arm no
//!   fact follows is `UNKNOWN`.
//! - A store of a pointer adds it to the fields of the objects it may write; a
//!   load of one reads every pointer ever stored into the object, and
//!   `UNKNOWN`, unless the slot is one only this body writes: then it is the
//!   stores that reach it (`slotstores::reaching`, the memory SSA of one
//!   object), a spilled pointer read back as what was spilled.
//! - A loop-carried fact is widened to whole objects (`_widened`) at a
//!   natural-loop header so it settles, and a fact with more than
//!   `MAX_FIELDS_FOR_FIELD_SENSITIVE` slices is also collapsed, as gcc and LLVM
//!   do.
//! - Integers carry no provenance: a `ptrtoint` publishes its pointer, as LLVM
//!   counts it a capture, and an `inttoptr` has none.
//!
//! ```text
//!  %p = alloca [8 x i16]            %p -> {frame:%p [0,16)}
//!  %q = gep %p, i16 2               %q -> {frame:%p [4,6)}
//!  store ptr %q, ptr %slot          slot's stores: {this one}  (spill slot)
//!  %r = load ptr, ptr %slot         %r -> {frame:%p [4,6)}     (its reaching store)
//!  call @unknown(ptr %q)            %q's object escapes from here on
//! ```
//!
//! Escape solve (`points_to`, `escapes`). Over the value solve, what each
//! instruction publishes is found once for the instructions that can publish
//! (a call, a return, a stored pointer, one an instruction takes and does not
//! carry on: `could_publish`), what reaches each block is a pass over the
//! components of the CFG, and what had escaped before an instruction is
//! worked out when asked and kept (`EscapedBefore`): objects handed to a call
//! that keeps them (not `nocapture`; a summary's `captures` stand where the
//! callee has one), stored somewhere other than a frame, returned, or converted
//! to an integer something reads, closed over pointers stored in what was
//! published and over the initializers of published globals. A frame object not
//! escaped before an access cannot be reached by a pointer no fact follows
//! (`EscapedBefore::apart`, LLVM's `EarliestEscapeInfo`).
//!
//! Summaries (`summaries`). Each body's `Summary` is the slices it reads and
//! writes, whether it reads or writes unknown memory, its captured parameters,
//! and the `!tbaa` types of its unplaced writes. It starts from the body's own
//! accesses (`_direct_summary`) and adds each callee's summary instantiated at
//! the call's actual arguments (`instantiated`), to a fixed point over the call
//! graph in SCC order; a cycle is solved from nothing, so a recursive function
//! that captures nothing proves so. A callee with a body that may be replaced
//! (weak or linkonce) describes no call. A call to something with no summary
//! reaches what its attributes allow (`_allowed`: `memory(...)`, `readnone`,
//! ...): through each actual the objects it points to, elsewhere the nonlocal
//! memory, what escaped before the call, and, of the globals
//! [`crate::globalsaa`] tracks, those it names; unless `nocallback` it also
//! does what the module's entry points do (`callbacks`). `summaries_updating`
//! redoes only the bodies that changed and their readers.
//!
//! Call effects (`calls_by_attributes`). Each call gets an `Effect`: the
//! `MemRef`s it loads and stores, instantiated through the actuals, and the
//! bytes it fills (`initialized`: bytes written before any is read). It is a
//! side table, because the rich MIR's call carries no memory operands.
//!
//! Access annotation (`annotated`). Each load and store gets its provenance;
//! where a range bounds its index the slice narrows to the bytes the index can
//! reach, using strides from `congruences` (a value's residue modulo a
//! modulus). Far or fixed-address accesses name linear memory.
//!
//! # Example
//! A pointer spilled to a slot, reloaded on one arm and joined with the
//! original:
//!
//! ```mir
//! define void @f(i1 %c) {
//! b0:
//!   %root = alloca [4 x i8]
//!   %slot = alloca ptr
//!   store ptr %root, ptr %slot
//!   br i1 %c, label %b10, label %b20
//!
//! b10:
//!   %loaded = load ptr, ptr %slot
//!   br label %b30
//!
//! b20:
//!   br label %b30
//!
//! b30:
//!   %joined = phi ptr [ %loaded, %b10 ], [ %root, %b20 ]
//!   ret void
//! }
//! ```
//!
//! `points_to(..).values` gives `%loaded` and `%joined` the same `Provenance`
//! as `%root`, the frame object it allocates.
//!
//! From unit test `test_points_to_flows_through_memory_and_a_phi` in
//! `alias_tests.rs`.
//!
//! # Provides
//! `pointers`, `point_values`, `points_to` and `PointsTo` (`values`, `escaped`,
//! `escaped_before`, `nonnull`), `seeds`, `summaries`, `calls_by_attributes`,
//! `annotated`, `congruences`, `initialized`, `read_arguments`,
//! `nonnull_by_definition`. Manager entries in [`crate::manager`]: `Pointers`,
//! `PointValues`, `Annotated`, `Summaries`. Consumers:
//! [`crate::memory`] (a `Unit`'s lazily built `annotated` and `points_to`),
//! [`crate::memoryssa`] (`calls_by_attributes`), [`crate::avail`]
//! (`EscapedBefore`), [`crate::induction`]
//! (`congruences`), [`crate::globalsaa`], [`crate::interprocedural`], and
//! `gvn`, `promote`, `loadjoins` and `decide` in `llrm-transforms`.
//!
//! # Needs
//! [`crate::globalsaa`] (tracked globals), [`crate::cfg::Shape`],
//! [`crate::ranges`] and [`crate::consts`] (index intervals, constants),
//! [`crate::cellmap`] (the escape solve's per-block cells),
//! [`crate::slotstores`], [`crate::induction`] (strides), `llrm_mir::memory`
//! for what attributes state.
//!
//! # See also
//! LLVM `BasicAA`, `GlobalsAA`, `EarliestEscapeInfo`, `FunctionAttrs`
//! `initializes`; GCC `tree-ssa-structalias.cc` (field sensitivity cap).

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::LazyLock;

use llrm_mir::context::{ConstantKind, GlobalId};
use llrm_mir::facts::Facts;
use llrm_mir::memory::Effects;
use llrm_mir::module::{InstId, Linkage, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{Attribute, BinaryOp, CastOp, Opcode};
use llrm_mir::types::Type;
use llrm_support::bits::Bits;
use llrm_support::hash::{HashMap, HashSet, IndexMap, IndexSet};

use crate::cfg;
use crate::consts::Known;
use crate::globalsaa;
use crate::graph::loops;
use crate::induction;
use crate::memory::{
    self, Identity, Key, MemRef, MemoryKind, MemoryObject, ObjectInterner, ObjectRef, Provenance, Slice, Unit,
    object_of, wrapped,
};
use crate::num::Num as BigInt;
use crate::ranges::{self, Interval};
use crate::regions;
use crate::sliceset::{self, Shared, SliceSet};

pub static UNKNOWN: LazyLock<Provenance> = LazyLock::new(|| Provenance::one(ObjectRef::UNKNOWN));
pub static NONLOCAL: LazyLock<Provenance> = LazyLock::new(|| Provenance::one(ObjectRef::NONLOCAL));
pub const EMPTY: Provenance =
    Provenance { slices: crate::memory::Sorted::new(), restrict: crate::memory::Sorted::new() };

/// One actual argument of a call: a provenance, a `(pointer value,
/// displacement)` pair, or nothing a pointer is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Actual {
    Provenance(Provenance),
    Pointer(ValueId, i64),
    Absent,
}

/// Whole objects an unknown callee can reach through pointers it owns.
fn _whole<'a>(
    provenances: impl IntoIterator<Item = &'a Provenance>,
    escaped: &BTreeSet<ObjectRef>,
) -> BTreeSet<Slice> {
    let mut objects = provenances
        .into_iter()
        .flat_map(|provenance| provenance.slices.iter().map(|one| one.object))
        .collect::<BTreeSet<_>>();
    objects.extend(escaped.iter().cloned());
    objects.into_iter().filter_map(Slice::every_byte).collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointsTo {
    pub values: IndexMap<ValueId, Provenance>,
    pub escaped: BTreeSet<ObjectRef>,
    /// Objects visible immediately before each call.
    pub escaped_before: EscapedBefore,
}

/// Objects escaped before each call, load and store, named only when asked: the
/// answer for an instruction is what reaches its block (`entering`), what the
/// block's publishing instructions up to it publish, and what a call passes
/// to its callee; found when asked and kept. Most solves never ask.
#[derive(Clone, Debug, Default)]
pub struct EscapedBefore {
    objects: Rc<IndexSet<ObjectRef>>,
    /// What the answers are worked out from, found with the solve.
    inputs: Rc<Inputs>,
    /// What reaches each block, and the instructions asked of, worked out from
    /// `inputs` when first asked: a solve nothing asks about builds none of it
    /// (334 solves, 40 asked, over d_faces at -O2).
    built: Rc<std::cell::OnceCell<Built>>,
    /// What a call lets its callee reach without keeping it.
    during: IndexMap<InstId, Vec<usize>>,
    found: Answers,
}

/// What `EscapedBefore` is worked out from: each block's successors, and its
/// instructions that publish or are asked of, in order.
#[derive(Debug, Default, Eq, PartialEq)]
struct Inputs {
    blocks: Vec<(i64, Vec<i64>)>,
    /// Per block: an instruction that publishes (with what) or is asked of.
    order: Vec<Vec<Step>>,
}

#[derive(Debug, Eq, PartialEq)]
struct Step {
    inst: InstId,
    /// Sorted, where it publishes something.
    publishes: Option<Vec<usize>>,
    /// A call, load or store.
    asked: bool,
}

/// `EscapedBefore`'s structure.
#[derive(Debug, Default)]
struct Built {
    /// What reaches the start of each block.
    entering: IndexMap<i64, Bits>,
    /// The instructions asked of (calls, loads and stores): their block and
    /// how many of its publishing instructions come at or before them: a
    /// place that moves with no change to what is published does not
    /// change the answer.
    sites: IndexMap<InstId, (i64, usize)>,
    /// What each block's publishing instructions publish, in order.
    published: IndexMap<i64, Vec<Vec<usize>>>,
}

thread_local! {
    static BEFORE_ANSWERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static STRUCTURES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many `EscapedBefore` structures this thread has built, for a test that a
/// solve nothing asks about builds none.
pub fn structures_built() -> usize {
    STRUCTURES.with(std::cell::Cell::get)
}

/// How many answers of `EscapedBefore` this thread has worked out, for a test
/// that a solve works out none until it is asked and none twice.
pub fn before_answers() -> usize {
    BEFORE_ANSWERS.with(std::cell::Cell::get)
}

/// What has been found of `EscapedBefore`: no part of what it says.
#[derive(Clone, Debug, Default)]
struct Answers(RefCell<HashMap<InstId, Rc<Bits>>>);

impl PartialEq for Answers {
    fn eq(
        &self,
        _: &Self,
    ) -> bool {
        true
    }
}

impl Eq for Answers {}

impl PartialEq for EscapedBefore {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.objects == other.objects && self.inputs == other.inputs && self.during == other.during
    }
}

impl Eq for EscapedBefore {}

impl Inputs {
    /// What reaches each block: the publications of every block with a path to
    /// it, a cycle's included in all of its members. The blocks in components
    /// that follow their predecessors, so that each is passed once (gcc's
    /// `ssa_propagation_engine` over reverse post order).
    fn built(
        &self,
        width: usize,
    ) -> Built {
        STRUCTURES.with(|built| built.set(built.get() + 1));
        let graph: Vec<cfg::Block> =
            self.blocks.iter().map(|(at, succ)| cfg::Block { at: *at, succ: succ.clone().into() }).collect();
        let predecessors = loops::predecessors(&graph);
        let none: &[i64] = &[];
        let mut generated: Vec<Bits> = Vec::with_capacity(graph.len());
        let mut published: IndexMap<i64, Vec<Vec<usize>>> = IndexMap::default();
        let mut sites = IndexMap::<InstId, (i64, usize)>::default();
        for (block, steps) in graph.iter().zip(&self.order) {
            let mut all = Bits::new(width);
            let mut mine: Vec<Vec<usize>> = Vec::new();
            for step in steps {
                if let Some(objects) = &step.publishes {
                    objects.iter().for_each(|one| all.insert(*one));
                    mine.push(objects.clone());
                }
                if step.asked {
                    sites.insert(step.inst, (block.at, mine.len()));
                }
            }
            generated.push(all);
            published.insert(block.at, mine);
        }
        let index: IndexMap<i64, usize> = graph.iter().enumerate().map(|(at, block)| (block.at, at)).collect();
        let parents: Vec<Vec<usize>> = graph
            .iter()
            .map(|block| {
                predecessors.get(&block.at).unwrap_or(none).iter().filter_map(|one| index.get(one).copied()).collect()
            })
            .collect();
        let (component, cyclic) = llrm_mir::callgraph::strong_components(&parents);
        let mut members: Vec<Vec<usize>> = vec![Vec::new(); cyclic.len()];
        for (at, one) in component.iter().enumerate() {
            members[*one].push(at);
        }
        let mut through: Vec<Bits> = Vec::with_capacity(cyclic.len());
        let mut reaching: Vec<Bits> = Vec::with_capacity(cyclic.len());
        for (one, blocks) in members.iter().enumerate() {
            let mut coming = Bits::new(width);
            let mut own = Bits::new(width);
            for &at in blocks {
                own.union_with(&generated[at]);
                for &parent in &parents[at] {
                    if component[parent] != one {
                        coming.union_with(&through[component[parent]]);
                    }
                }
            }
            let mut leaving = coming.clone();
            leaving.union_with(&own);
            // Every member of a cycle reaches every other, itself too.
            if cyclic[one] {
                coming = leaving.clone();
            }
            reaching.push(coming);
            through.push(leaving);
        }
        let entering: IndexMap<i64, Bits> =
            graph.iter().enumerate().map(|(at, block)| (block.at, reaching[component[at]].clone())).collect();
        Built { entering, sites, published }
    }
}

impl EscapedBefore {
    fn before(
        &self,
        at: &InstId,
    ) -> Option<Rc<Bits>> {
        if let Some(found) = self.found.0.borrow().get(at) {
            return Some(Rc::clone(found));
        }
        let built = self.built.get_or_init(|| self.inputs.built(self.objects.len()));
        let (block, upto) = *built.sites.get(at)?;
        BEFORE_ANSWERS.with(|answers| answers.set(answers.get() + 1));
        // The state after an instruction holds the state before it: what a
        // call publishes itself reaches it, as its own arguments do.
        let mut state = built.entering[&block].clone();
        for objects in built.published[&block].iter().take(upto) {
            objects.iter().for_each(|one| state.insert(*one));
        }
        if let Some(reached) = self.during.get(at) {
            reached.iter().for_each(|one| state.insert(*one));
        }
        let state = Rc::new(state);
        self.found.0.borrow_mut().insert(*at, Rc::clone(&state));
        Some(state)
    }

    pub fn get(
        &self,
        at: &InstId,
    ) -> Option<BTreeSet<ObjectRef>> {
        self.before(at).map(|bits| bits.iter().map(|one| self.objects[one].clone()).collect())
    }

    /// What escaped before `at`, unnamed: two instructions with equal bits have
    /// the same objects.
    pub fn bits(
        &self,
        at: &InstId,
    ) -> Option<Rc<Bits>> {
        self.before(at)
    }

    /// Whether the access `reference` at `at`, through a pointer no fact
    /// follows or one only to what escaped, cannot reach `cell`: every byte
    /// of it is in frame objects not escaped before `at`. LLVM's
    /// `EarliestEscapeInfo`.
    pub fn apart(
        &self,
        at: InstId,
        reference: &MemRef,
        cell: &MemRef,
    ) -> bool {
        // No provenance is a pointer no fact follows: `_lost` publishes what
        // it came from.
        let escaping = reference.provenance.as_ref().is_none_or(|one| {
            one.slices.iter().all(|slice| matches!(slice.object.kind, MemoryKind::Unknown | MemoryKind::Nonlocal))
        });
        // What escaped before `at` is asked of only an access that can reach
        // only what escaped, to a cell in frame objects.
        let frames = cell
            .provenance
            .as_ref()
            .is_some_and(
                |one| !one.slices.is_empty() && one.slices.iter().all(|slice| slice.object.kind == MemoryKind::Frame),
            );
        if !escaping || !frames {
            return false;
        }
        let Some(bits) = self.before(&at) else { return false };
        cell.provenance
            .as_ref()
            .is_some_and(
                |one| one.slices
                    .iter()
                    .all(|slice| self.objects.get_index_of(&slice.object).is_none_or(|index| !bits.contains(index))),
            )
    }
}

/// What gives an access its provenance: the pointer values' objects, whether
/// the solve went on to find what escapes (`PointsTo`) or not (`PointValues`).
pub trait Resolves {
    fn reference(
        &self,
        unit: &Unit,
        reference: &MemRef,
    ) -> Option<Provenance>;
}

impl PointValues {
    /// Canonical bytes reached by an access through an analysed pointer.
    pub fn reference(
        &self,
        unit: &Unit,
        reference: &MemRef,
    ) -> Option<Provenance> {
        _resolved_reference(unit, reference, &self.values)
    }
}

impl Resolves for PointValues {
    fn reference(
        &self,
        unit: &Unit,
        reference: &MemRef,
    ) -> Option<Provenance> {
        PointValues::reference(self, unit, reference)
    }
}

impl Resolves for PointsTo {
    fn reference(
        &self,
        unit: &Unit,
        reference: &MemRef,
    ) -> Option<Provenance> {
        PointsTo::reference(self, unit, reference)
    }
}

impl PointsTo {
    /// Canonical bytes reached by an access through an analysed pointer.
    pub fn reference(
        &self,
        unit: &Unit,
        reference: &MemRef,
    ) -> Option<Provenance> {
        _resolved_reference(unit, reference, &self.values)
    }

    /// Whether `value` can only designate a real static or frame object.
    ///
    /// Incoming pointers remain nullable. A frame object or a global is
    /// non-null by the source language contract even though its eventual
    /// 16-bit offset is not known until link time.
    pub fn nonnull(
        &self,
        value: ValueId,
    ) -> bool {
        self.values.get(&value).is_some_and(nonnull)
    }
}

fn nonnull(provenance: &Provenance) -> bool {
    !provenance.slices.is_empty()
        && provenance.slices.iter().all(|one| {
            matches!(
                one.object.kind,
                MemoryKind::Frame | MemoryKind::Global | MemoryKind::External | MemoryKind::Named
            )
        })
}

/// `PointsTo::nonnull` of `value` where its definition alone settles it,
/// as LLVM's `isKnownNonZero` reads a pointer's underlying object instead
/// of solving every pointer; `None` where only the whole solve can say.
pub fn nonnull_by_definition(
    unit: &Unit,
    value: ValueId,
) -> Option<bool> {
    // A parameter the language states non-null, as a reference is; or
    // dereferenceable in a near space, where null holds no object (DGROUP's
    // first bytes are the runtime's); a far one may be 0000:0000.
    if let ValueDef::Argument(at) = unit.function.value(value).def {
        let facts = Facts::param(unit.function, at as usize);
        let near = unit
            .operand_type(Operand::Value(value))
            .is_some_and(
                |ty| matches!(
                    unit.context.types.get(ty),
                    Type::Pointer(space) if *space == unit.spaces().near
                ),
            );
        if facts.non_null() || near && facts.dereferenceable().is_some_and(|bytes| bytes > 0) {
            return Some(true);
        }
    }
    if let Some(seed) = seeds(unit).get(&value) {
        return Some(nonnull(seed));
    }
    let (inst, _) = unit.defining(Operand::Value(value))?;
    let fixed = matches!(unit.function.instruction(inst).opcode, Opcode::Alloca { .. });
    if !fixed {
        return None;
    }
    _direct(unit, inst, &IndexMap::default()).ok().flatten().map(|provenance| nonnull(&provenance))
}

/// Every value `points_to` could give a fact: every pointer.
pub fn may_point(unit: &Unit) -> HashSet<ValueId> {
    let function = unit.function;
    let parameters = function.parameters().iter().copied();
    let results = function.walk().filter_map(|(_, inst)| function.instruction(inst).result);
    parameters.chain(results).filter(|&value| is_pointer(unit, Operand::Value(value))).collect()
}

fn is_pointer(
    unit: &Unit,
    operand: Operand,
) -> bool {
    unit.operand_type(operand).is_some_and(|ty| matches!(unit.context.types.get(ty), Type::Pointer(_)))
}

/// What each pointer parameter points to: offset 0 of a `Parameter` object,
/// so a summary's slices are relative to the actual; for a `noalias` one
/// also a restrict root.
pub fn seeds(unit: &Unit) -> IndexMap<ValueId, Provenance> {
    let function = unit.function;
    function
        .parameters()
        .iter()
        .enumerate()
        .filter(|(_, value)| is_pointer(unit, Operand::Value(**value)))
        .map(|(at, value)| {
            let object = ObjectInterner::of(unit.context).intern(MemoryObject {
                identity: Some(Identity::Int(at as i64)),
                ..MemoryObject::new(MemoryKind::Parameter)
            });
            let restrict = if Facts::param(function, at).no_alias() {
                BTreeSet::from([Identity::Int(at as i64)])
            } else {
                BTreeSet::new()
            };
            (*value, Provenance::one_with_slice(object, 0, 1, 1, 1, restrict).expect("one byte is a slice"))
        })
        .collect()
}

/// The provenance of a pointer operand: a value's as solved, a constant's
/// from the global it addresses.
fn _operand(
    unit: &Unit,
    operand: Operand,
    values: &IndexMap<ValueId, Provenance>,
) -> Option<Provenance> {
    match operand {
        Operand::Value(value) => values.get(&value).cloned(),
        Operand::Constant(_) => {
            let access = MemRef::at(unit, operand, 1);
            let root = access.root?;
            if !matches!(
                root,
                Operand::Constant(id) if matches!(unit.context.get(id).kind, ConstantKind::Global(_))
            ) {
                return None;
            }
            let object = object_of(unit, root)?;
            Provenance::one_with_slice(object, access.disp, access.disp + 1, 1, 1, BTreeSet::new()).ok()
        }
        Operand::Block(_) => None,
    }
}

/// Resolve an access through the current pointer-value facts.
fn _resolved_reference(
    unit: &Unit,
    reference: &MemRef,
    values: &IndexMap<ValueId, Provenance>,
) -> Option<Provenance> {
    if let Some(attached) = &reference.provenance {
        return Some(attached.clone());
    }
    let source = _operand(unit, reference.pointer?, values)?;
    // A singleton address names `width` consecutive bytes. A set of
    // indexed addresses retains its stride and widens its final lane.
    let slices = source
        .slices
        .iter()
        .map(|one| {
            Slice::new(one.object, one.low, one.high, one.stride, i64::from(reference.width.max(1)))
                .expect("a slice keeps its positive shape")
        })
        .collect();
    Some(Provenance { slices, restrict: source.restrict })
}

/// One procedure's transitive memory effects in its own object space.
///
/// `captures` holds PARAMETER
/// object identities.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct Summary {
    pub reads: SliceSet,
    pub writes: SliceSet,
    pub captures: BTreeSet<Option<Identity>>,
    pub unknown_read: bool,
    pub unknown_write: bool,
    /// The `!tbaa` access types of the writes `unknown_write` stands for, where
    /// every one is a pointer no fact follows and has a type: a store of one
    /// type cannot land on a load of a type apart from it, however unplaced
    /// its pointer. None: some has none.
    pub unknown_write_types: Option<BTreeSet<Access>>,
    /// The tracked globals `reads` and `writes` name are all the unknown
    /// access reaches of them (`GlobalsModRef` found them per global).
    pub globals_listed: bool,
}

/// An access type: its name and its ancestors', as `MemRef::typed` and
/// `lineage`.
pub type Access = (std::rc::Rc<str>, std::rc::Rc<[String]>);

/// The types of two writes' unplaced stores together.
fn merged_types(
    one: (bool, &Option<BTreeSet<Access>>),
    other: (bool, &Option<BTreeSet<Access>>),
) -> Option<BTreeSet<Access>> {
    match (one, other) {
        ((false, _), (_, types)) => types.clone(),
        ((_, types), (false, _)) => types.clone(),
        ((true, Some(one)), (true, Some(other))) => Some(one.union(other).cloned().collect()),
        _ => None,
    }
}

/// Whether the bytes `fresh` names lie in the bytes some slice of `kept` names:
/// the one rule of 'is the fresh footprint covered by the kept one', which a
/// summary and a call's effect (and the replay checks over both) ask. Bytes
/// past the object's end are no byte of it, and a slice stands for the bytes it
/// names, not for its shape.
pub fn slice_covered<'a>(
    fresh: &Slice,
    kept: impl IntoIterator<Item = &'a Slice>,
) -> bool {
    let clipped = fresh
        .object
        .extent
        .and_then(
            |extent| {
                let high = fresh.high.min(extent);
                let width = fresh.width.min(extent - (high - 1));
                Slice::new(fresh.object, fresh.low, high, fresh.stride, width.max(1)).ok()
            },
        );
    kept.into_iter().any(|one| one.contains(fresh) || clipped.as_ref().is_some_and(|c| one.contains(c)))
}

impl Summary {
    /// Whether `fresh` shows a read or write that `self`, held past passes that
    /// add no memory operation, does not state. A fresh summary that gives
    /// up (a flag says anything, or a slice of the unknown object or of a whole
    /// object does) refutes nothing: a rewrite such as strength reduction hides
    /// an address from the analysis without touching more memory.
    pub fn covers(
        &self,
        fresh: &Summary,
    ) -> bool {
        // A slice of a whole object, any byte of it: an address nothing placed,
        // as a pointer strength reduction made does.
        let unplaced = |one: &Slice| one.low <= -(1_i64 << 31) && one.high >= 1_i64 << 31;
        let within = |old_unknown: bool, new_unknown: bool, old: &SliceSet, new: &SliceSet| {
            old_unknown
                || new_unknown
                || new
                    .iter()
                    .filter(|one| one.object.kind != MemoryKind::Unknown && !unplaced(one))
                    .all(|one| slice_covered(one, old.iter()))
        };
        within(self.unknown_read, fresh.unknown_read, &self.reads, &fresh.reads)
            && within(self.unknown_write, fresh.unknown_write, &self.writes, &fresh.writes)
    }

    pub fn instantiated(
        &self,
        arguments: &[Provenance],
    ) -> Summary {
        let expand = |items: &SliceSet| -> SliceSet {
            // A shared run with a parameter's slice is put into the set to be
            // instantiated with it; any other passes through as it
            // is.
            let mut items = items.clone();
            if items.has_parameter_run() {
                items.materialize();
            }
            items.mapped(|item, found| {
                if item.object.kind != MemoryKind::Parameter {
                    found.push(*item);
                    return;
                }
                let Key::Int(index) = item.object.key else {
                    return;
                };
                if !(0 <= index && index < arguments.len() as i64) {
                    return;
                }
                for actual in &arguments[index as usize].slices {
                    found.push(
                        Slice::new(
                            actual.object.clone(),
                            actual.low + item.low,
                            actual.high + item.high - 1,
                            memory::gcd(actual.stride, item.stride),
                            item.width,
                        )
                        .expect("two valid slices sum to a valid slice"),
                    );
                }
            })
        };

        Summary {
            reads: expand(&self.reads),
            writes: expand(&self.writes),
            captures: self.captures.clone(),
            unknown_read: self.unknown_read,
            unknown_write: self.unknown_write,
            unknown_write_types: self.unknown_write_types.clone(),
            globals_listed: self.globals_listed,
        }
    }
}

/// What a procedure's calls are, of its body and the declarations alone: kept
/// for the next run while neither has changed.
#[derive(Debug, Default)]
pub struct CallFacts {
    pub calls: IndexMap<InstId, String>,
    pub arguments: IndexMap<InstId, Vec<Actual>>,
    /// The callees a definition elsewhere may replace: no summary describes a
    /// call to one.
    pub replaceable: std::collections::BTreeSet<String>,
    /// Every call and invoke, in order.
    pub sites: Vec<InstId>,
}

/// A function, its calls by
/// callee name, and each call's actual arguments.
#[derive(Clone)]
pub struct Procedure<'a> {
    pub unit: Unit<'a>,
    facts: Rc<CallFacts>,
}

impl std::ops::Deref for Procedure<'_> {
    type Target = CallFacts;
    fn deref(&self) -> &CallFacts {
        &self.facts
    }
}

impl<'a> Procedure<'a> {
    /// `unit`'s calls of named functions, and every call's actuals, one
    /// per argument.
    pub fn of(unit: Unit<'a>) -> Self {
        let facts = Rc::new(CallFacts::of(&unit));
        Self { unit, facts }
    }

    /// `unit`'s procedure, its calls as `facts` (of this body and these
    /// declarations) say.
    pub fn with(
        unit: Unit<'a>,
        facts: Rc<CallFacts>,
    ) -> Self {
        Self { unit, facts }
    }
}

impl CallFacts {
    pub fn of(unit: &Unit) -> Self {
        let function = unit.function;
        let mut calls = IndexMap::default();
        let mut arguments = IndexMap::default();
        let mut replaceable = std::collections::BTreeSet::new();
        for (_, inst) in function.walk() {
            let op = function.instruction(inst);
            let (Opcode::Call(_) | Opcode::Invoke(_)) = op.opcode else { continue };
            let callee = *op.operands.last().expect("a call names its callee");
            if let Operand::Constant(id) = callee
                && let ConstantKind::Global(global) = unit.context.get(id).kind
                && let Some(named) = unit.globals.get(global.0 as usize)
                && let Some(name) = named.name.owned()
            {
                if named.function().is_some()
                    && !matches!(
                        named.linkage,
                        Linkage::External | Linkage::Internal | Linkage::Private | Linkage::ExternWeak
                    )
                {
                    replaceable.insert(name.clone());
                }
                calls.insert(inst, name);
            }
            let count = match op.opcode {
                Opcode::Invoke(_) => op.operands.len() - 3,
                _ => op.operands.len() - 1,
            };
            let actual = op.operands[..count]
                .iter()
                .map(|&one| match one {
                    Operand::Value(value) if is_pointer(unit, one) => Actual::Pointer(value, 0),
                    // A null pointer points to nothing.
                    Operand::Constant(id)
                        if matches!(
                            unit.context.get(id).kind,
                            ConstantKind::Null | ConstantKind::Zero | ConstantKind::Poison | ConstantKind::Undef
                        ) =>
                    {
                        Actual::Absent
                    }
                    Operand::Constant(_) if is_pointer(unit, one) => _operand(unit, one, &IndexMap::default())
                        .map_or(Actual::Provenance(UNKNOWN.clone()), Actual::Provenance),
                    _ => Actual::Absent,
                })
                .collect();
            arguments.insert(inst, actual);
        }
        let sites = call_sites(unit);
        Self { calls, arguments, replaceable, sites }
    }
}

/// The calls of `unit`, in order.
fn call_sites(unit: &Unit) -> Vec<InstId> {
    let function = unit.function;
    function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| matches!(
            function.instruction(inst).opcode,
            Opcode::Call(_) | Opcode::Invoke(_)
        ))
        .collect()
}

/// Whole slices of `globals`.
fn _globals(
    unit: &Unit,
    globals: impl IntoIterator<Item = GlobalId>,
) -> BTreeSet<Slice> {
    globals.into_iter().filter_map(|one| memory::global_object(unit, one)).map(Slice::whole).collect()
}

/// Every global the unit's GlobalsAA tracks: what a pointer of unknown
/// origin in a callee, which may name them, reaches.
fn _tracked(unit: &Unit) -> BTreeSet<Slice> {
    _globals(unit, unit.globals_aa.into_iter().flat_map(|aa| aa.tracked_globals().iter().copied()))
}

/// What the call `at` may do, as it and its callee state it: to the
/// memory its pointer arguments point to, to any other, and through each
/// argument. LLVM's `getMemoryEffects` of a `CallBase`.
struct Allowed {
    arguments: Effects,
    other: Effects,
    through: Vec<Effects>,
}

fn _allowed(
    unit: &Unit,
    at: InstId,
) -> Allowed {
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &unit.function.instruction(at).opcode else {
        return Allowed { arguments: Effects::ANY, other: Effects::ANY, through: Vec::new() };
    };
    let declared = llrm_mir::memory::callee(unit.context, unit.function, at)
        .and_then(|one| unit.globals.get(one.0 as usize))
        .and_then(|one| one.function());
    let both =
        |one: Effects, other: Effects| Effects { reads: one.reads && other.reads, writes: one.writes && other.writes };
    let (mut arguments, mut other) = llrm_mir::memory::located(&info.attrs);
    if let Some(declared) = declared {
        let (on_arguments, on_other) = llrm_mir::memory::located(&declared.attrs);
        (arguments, other) = (both(arguments, on_arguments), both(other, on_other));
    }
    let through = (0..info.argument_attrs.len())
        .map(|index| {
            let parameter = declared
                .and_then(|one| one.parameter_attrs.get(index))
                .map_or(Effects::ANY, |attrs| llrm_mir::memory::through(attrs));
            both(both(arguments, llrm_mir::memory::through(&info.argument_attrs[index])), parameter)
        })
        .collect();
    Allowed { arguments, other, through }
}

/// Per argument of the call `at`, its pointer operand, where the callee may
/// read through it.
pub fn read_arguments(
    unit: &Unit,
    at: InstId,
) -> Vec<Operand> {
    let op = unit.function.instruction(at);
    let count = match op.opcode {
        Opcode::Call(_) => op.operands.len() - 1,
        Opcode::Invoke(_) => op.operands.len() - 3,
        _ => return Vec::new(),
    };
    let allowed = _allowed(unit, at);
    op.operands[..count]
        .iter()
        .enumerate()
        .filter(|(index, one)| {
            is_pointer(unit, **one) && allowed.through.get(*index).is_none_or(|effects| effects.reads)
        })
        .map(|(_, one)| *one)
        .collect()
}

/// Whether the call `at` keeps no copy of its argument `index`: `nocapture`
/// at the site or on the callee's parameter.
/// Whether the call `inst` writes no memory at all, through its arguments or
/// otherwise.
pub(crate) fn only_reads(
    unit: &Unit,
    inst: InstId,
) -> bool {
    let allowed = _allowed(unit, inst);
    matches!(
        unit.function.instruction(inst).opcode,
        Opcode::Call(_) | Opcode::Invoke(_)
    )
        && !allowed.arguments.writes
        && !allowed.other.writes
}

pub(crate) fn borrowed(
    unit: &Unit,
    at: InstId,
    index: usize,
) -> bool {
    let (Opcode::Call(_) | Opcode::Invoke(_)) = &unit.function.instruction(at).opcode else { return false };
    // What these return is the operand's bytes, followed as a value: the call
    // keeps nothing (`captures(ret: provenance)` in LLVM).
    if matches!(
        unit.intrinsic(at),
        Some(llrm_mir::intrinsics::Intrinsic::Window | llrm_mir::intrinsics::Intrinsic::Restrict)
    ) {
        return true;
    }
    no_capture_argument(unit, at, index)
}

/// Whether the call `at` is stated to keep no copy of its argument `index`,
/// at the site or on the callee's parameter (`nocapture`).
pub(crate) fn no_capture_argument(
    unit: &Unit,
    at: InstId,
    index: usize,
) -> bool {
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &unit.function.instruction(at).opcode else { return false };
    let nocapture = |attrs: &[Attribute]| llrm_mir::facts::Facts::of(attrs).no_capture();
    let declared = llrm_mir::memory::callee(unit.context, unit.function, at)
        .and_then(|one| unit.globals.get(one.0 as usize))
        .and_then(|one| one.function());
    info.argument_attrs.get(index).is_some_and(|attrs| nocapture(attrs))
        || declared.and_then(|one| one.parameter_attrs.get(index)).is_some_and(|attrs| nocapture(attrs))
}

/// What a callee nobody summarized may read and write through each of `actual`,
/// as `allowed` says.
fn _through_arguments(
    allowed: &Allowed,
    actual: &[Provenance],
) -> (BTreeSet<Slice>, BTreeSet<Slice>) {
    let (mut reads, mut writes) = (BTreeSet::new(), BTreeSet::new());
    for (index, one) in actual.iter().enumerate() {
        let through = allowed.through.get(index).copied().unwrap_or(allowed.arguments);
        let objects = _whole([one], &BTreeSet::new());
        if through.reads {
            reads.extend(objects.iter().cloned());
        }
        if through.writes {
            writes.extend(objects);
        }
    }
    (reads, writes)
}

/// What an unsummarized callee may read and write at `at` other than through
/// its arguments, less what a call back into the module adds, which depends
/// on the callbacks and nothing else a revisit changes; and whether the call
/// may call back.
fn _unknown_base(
    unit: &Unit,
    facts: &PointsTo,
    at: InstId,
) -> Result<(BTreeSet<Slice>, BTreeSet<Slice>, bool), String> {
    OTHER_RUNS.with(|runs| runs.set(runs.get() + 1));
    let mut reads = NONLOCAL.slices.to_set();
    reads.extend(_whole([], &facts.escaped_before.get(&at).unwrap_or_default()));
    // A port `ports` left reaching memory reaches it as its device does, by
    // address: every object, whether the program ever took its address.
    if matches!(
        unit.intrinsic(at),
        Some(llrm_mir::intrinsics::Intrinsic::PortIn | llrm_mir::intrinsics::Intrinsic::PortOut)
    ) {
        reads.extend(_globals(unit, (0..unit.globals.len()).map(|one| GlobalId(one as u32))));
        let frames = unit
            .function
            .walk()
            .filter(|&(_, inst)| matches!(unit.function.instruction(inst).opcode, Opcode::Alloca { .. }));
        reads.extend(
            frames
                .filter_map(|(_, inst)| {
                    memory::object_of(unit, Operand::Value(unit.function.instruction(inst).result?))
                })
                .map(Slice::whole),
        );
    }
    let mut writes = reads.clone();
    let Some(globals) = unit.globals_aa else { return Ok((reads, writes, false)) };
    let callee = llrm_mir::memory::callee(unit.context, unit.function, at);
    let (read, written) = globals.unsummarized(callee);
    reads.extend(_globals(unit, read));
    writes.extend(_globals(unit, written));
    // An indirect callee may be an entry itself.
    Ok((reads, writes, callee.is_none() || globalsaa::calls_back(unit, at)))
}

thread_local! {
    static CALLBACK_SLICES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many slices this thread has moved between entries' owners for callbacks,
/// for a test that a change costs what it changed.
pub fn callback_slices_moved() -> usize {
    CALLBACK_SLICES.with(std::cell::Cell::get)
}

thread_local! {
    static CALLBACK_ENTRIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many entries' summaries this thread has instantiated for callbacks, for
/// a test that the work does not grow with the entries times the bodies.
pub fn callback_entries() -> usize {
    CALLBACK_ENTRIES.with(std::cell::Cell::get)
}

/// The slices of `before` that `after` lacks, and the ones it has that `before`
/// lacks: one walk of both, in order.
fn sorted_difference<'a>(
    before: &'a SliceSet,
    after: &'a SliceSet,
    gone: &mut Vec<&'a Slice>,
    came: &mut Vec<&'a Slice>,
) {
    let (mut one, mut other) = (before.iter().peekable(), after.iter().peekable());
    loop {
        match (one.peek(), other.peek()) {
            (Some(a), Some(b)) => match a.cmp(b) {
                std::cmp::Ordering::Equal => {
                    one.next();
                    other.next();
                }
                std::cmp::Ordering::Less => gone.push(one.next().expect("peeked")),
                std::cmp::Ordering::Greater => came.push(other.next().expect("peeked")),
            },
            (Some(_), None) => gone.push(one.next().expect("peeked")),
            (None, Some(_)) => came.push(other.next().expect("peeked")),
            (None, None) => break,
        }
    }
}

/// The callbacks as the entries' summaries add up. Each slice is owned by the
/// entries that hold it, counted: an entry whose summary changed moves only the
/// slices it gained and lost, and the sum is made again only when a slice
/// gained its first owner or lost its last (every change used to add all the
/// entries up again when one shrank: 549 of GORILLA's 1,175 changes, 545 Minstr
/// at -O1).
struct CalledBack {
    parts: IndexMap<String, Summary>,
    reads: BTreeMap<Slice, usize>,
    writes: BTreeMap<Slice, usize>,
    /// How many entries may read / write anything.
    unknown: (usize, usize),
    /// What the owners add up to, once made and until a slice comes or goes.
    built: std::cell::RefCell<Option<Summary>>,
}

impl CalledBack {
    fn of(
        globals: &globalsaa::Globals,
        known: &IndexMap<String, Summary>,
    ) -> Option<Self> {
        let mut parts = IndexMap::default();
        for name in globals.entries() {
            CALLBACK_ENTRIES.with(|entries| entries.set(entries.get() + 1));
            parts.insert(name.clone(), known.get(name)?.instantiated(&[]));
        }
        Some(Self::from_parts(parts))
    }

    fn from_parts(parts: IndexMap<String, Summary>) -> Self {
        let mut this =
            Self { parts, reads: BTreeMap::new(), writes: BTreeMap::new(), unknown: (0, 0), built: Default::default() };
        let Self { parts, reads, writes, unknown, .. } = &mut this;
        for one in parts.values() {
            one.reads.iter().for_each(|item| *reads.entry(*item).or_default() += 1);
            one.writes.iter().for_each(|item| *writes.entry(*item).or_default() += 1);
            unknown.0 += usize::from(one.unknown_read);
            unknown.1 += usize::from(one.unknown_write);
        }
        this
    }

    fn summary(&self) -> Option<Summary> {
        if let Some(built) = self.built.borrow().as_ref() {
            return Some(built.clone());
        }
        // The run every summary that calls back holds, made once.
        // Coalesced as `SliceSet::coalesced` has it: an object with every byte
        // in the set loses its subranges.
        let run = |owned: &BTreeMap<Slice, usize>| {
            let wholes: BTreeSet<ObjectRef> =
                owned.keys().filter(|one| sliceset::whole(one)).map(|one| one.object).collect();
            let kept =
                owned.keys().filter(|one| !wholes.contains(&one.object) || sliceset::whole(one)).copied().collect();
            SliceSet::shared(&Shared::of(kept))
        };
        let made = Summary {
            reads: run(&self.reads),
            writes: run(&self.writes),
            unknown_read: self.unknown.0 > 0,
            unknown_write: self.unknown.1 > 0,
            ..Summary::default()
        };
        *self.built.borrow_mut() = Some(made.clone());
        Some(made)
    }

    /// Entry `name` is now `summary`: the slices it gained and lost change
    /// their owners' counts.
    fn changed(
        &mut self,
        name: &str,
        summary: &Summary,
    ) {
        CALLBACK_ENTRIES.with(|entries| entries.set(entries.get() + 1));
        let now = summary.instantiated(&[]);
        let Self { parts, reads, writes, unknown, built } = self;
        let Some(then) = parts.get_mut(name) else { return };
        let mut moved = false;
        let mut move_owners = |owned: &mut BTreeMap<Slice, usize>, before: &SliceSet, after: &SliceSet| {
            let (mut gone, mut came) = (Vec::new(), Vec::new());
            sorted_difference(before, after, &mut gone, &mut came);
            CALLBACK_SLICES.with(|moved| moved.set(moved.get() + gone.len() + came.len()));
            for item in gone {
                let count = owned.get_mut(item).expect("an owner of what it held");
                *count -= 1;
                if *count == 0 {
                    owned.remove(item);
                    moved = true;
                }
            }
            for item in came {
                let count = owned.entry(*item).or_default();
                moved |= *count == 0;
                *count += 1;
            }
        };
        move_owners(reads, &then.reads, &now.reads);
        move_owners(writes, &then.writes, &now.writes);
        for (count, before, after) in [
            (&mut unknown.0, then.unknown_read, now.unknown_read),
            (&mut unknown.1, then.unknown_write, now.unknown_write),
        ] {
            let was = *count > 0;
            *count = *count + usize::from(after) - usize::from(before);
            moved |= was != (*count > 0);
        }
        *then = now;
        if moved {
            *built.borrow_mut() = None;
        }
    }
}

fn _actuals(
    procedure: &Procedure,
    facts: &PointsTo,
    at: InstId,
) -> Vec<Provenance> {
    procedure
        .arguments
        .get(&at)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .map(|actual| match actual {
            Actual::Provenance(provenance) => provenance.clone(),
            Actual::Pointer(value, displacement) if facts.values.contains_key(value) => {
                facts.values[value].shifted(*displacement)
            }
            Actual::Absent => EMPTY.clone(),
            Actual::Pointer(..) => UNKNOWN.clone(),
        })
        .collect()
}

/// Whether `slice` outlives its function's activation: no frame object.
fn outlives(slice: &Slice) -> bool {
    !matches!(slice.object.kind, MemoryKind::Frame | MemoryKind::Stack)
}

/// Whether the address `inst` accesses is an integer made a pointer, moved by
/// GEPs and casts: LLVM's `inttoptr`, which `_lost` publishes the escape of.
fn from_integer(
    unit: &Unit,
    inst: InstId,
) -> bool {
    let op = unit.function.instruction(inst);
    let address = if matches!(op.opcode, Opcode::Store { .. }) { op.operands.get(1) } else { op.operands.first() };
    let mut at = address.copied();
    while let Some((_, def)) = at.and_then(|one| unit.defining(one)) {
        match &def.opcode {
            Opcode::Cast(CastOp::IntToPtr) => return true,
            Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) | Opcode::GetElementPtr { .. } => {
                at = def.operands.first().copied()
            }
            _ => return false,
        }
    }
    false
}

thread_local! {
    static OTHER_RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static DIRECT_RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static WHOLE_RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has worked out what an unknown call does other
/// than through its arguments, for a test that calls alike are worked out once.
pub fn other_runs() -> usize {
    OTHER_RUNS.with(std::cell::Cell::get)
}

/// How many direct summaries this thread has made, for a test that a
/// procedure's is made once.
pub fn direct_runs() -> usize {
    DIRECT_RUNS.with(std::cell::Cell::get)
}

/// How many runs of `summaries_updating` on this thread were worked out from
/// nothing, for a test that a run in which every body is dirty is one.
pub fn whole_runs() -> usize {
    WHOLE_RUNS.with(std::cell::Cell::get)
}

/// How many times this thread has summarized a body against its callees'
/// summaries, for a test that a body in no cycle of calls is visited once.
pub fn visits() -> usize {
    VISITS.with(std::cell::Cell::get)
}

pub fn _direct_summary(unit: &Unit) -> Result<Summary, String> {
    _direct_summary_with(unit).map(|(summary, _)| summary)
}

/// `_direct_summary`, and the points-to facts it was made from.
fn _direct_summary_with(unit: &Unit) -> Result<(Summary, Rc<PointsTo>), String> {
    DIRECT_RUNS.with(|runs| runs.set(runs.get() + 1));
    let (mut reads, mut writes) = (BTreeSet::new(), BTreeSet::new());
    let (mut unknown_read, mut unknown_write) = (false, false);
    let mut types = Some(BTreeSet::new());
    let facts = Rc::new(points_to(unit, None, None)?);
    for (_, inst) in unit.function.walk() {
        let Some(reference) = MemRef::of(unit, inst) else { continue };
        let read = matches!(unit.function.instruction(inst).opcode, Opcode::Load { .. });
        // A write nothing places is its type's, not a slice of the unknown
        // object.
        let unplaced = |types: &mut Option<BTreeSet<Access>>| {
            let access = reference.typed.clone().map(|typed| (typed, reference.lineage.clone()));
            match (types.as_mut(), access) {
                (Some(types), Some(access)) => {
                    types.insert(access);
                }
                _ => *types = None,
            }
        };
        let Some(provenance) = facts.reference(unit, &reference) else {
            if read {
                unknown_read = true;
            } else {
                unknown_write = true;
                // An address built from an integer reaches only what escaped,
                // as `Unknown` does; any other the analysis
                // lost may be a tracked global's too.
                if from_integer(unit, inst) {
                    unplaced(&mut types);
                } else {
                    types = None;
                }
            }
            continue;
        };
        for one in provenance.slices {
            let unknown = one.object.kind == MemoryKind::Unknown;
            if outlives(&one) && !(unknown && !read) {
                if read {
                    reads.insert(one);
                } else {
                    writes.insert(one);
                }
            }
            if unknown {
                if read {
                    unknown_read = true;
                } else {
                    unknown_write = true;
                    unplaced(&mut types);
                }
            }
        }
    }
    let captures = facts
        .escaped
        .iter()
        .filter(|one| one.kind == MemoryKind::Parameter)
        .filter_map(|one| match one.key {
            Key::Int(number) => Some(Some(Identity::Int(number))),
            _ => None,
        })
        .collect();
    let summary = Summary {
        reads: reads.into(),
        writes: writes.into(),
        captures,
        unknown_read,
        unknown_write,
        unknown_write_types: types,
        globals_listed: false,
    };
    Ok((summary, facts))
}

fn _widen_parameters(summary: Summary) -> Summary {
    let widened = |items: &SliceSet| {
        let mut items = items.clone();
        if items.has_parameter_run() {
            items.materialize();
        }
        items.mapped(|one, found| {
            found.push(if one.object.kind == MemoryKind::Parameter { Slice::whole(one.object) } else { *one });
        })
    };
    Summary { reads: widened(&summary.reads), writes: widened(&summary.writes), ..summary }
}

/// Drop subranges once the same object already has a whole-object effect.
fn _coalesced(items: &BTreeSet<Slice>) -> BTreeSet<Slice> {
    let is_whole = |one: &Slice| {
        one.low == memory::WHOLE_LOW && one.high == memory::WHOLE_HIGH && one.stride == 1 && one.width == 1
    };
    let whole = items.iter().filter(|one| is_whole(one)).map(|one| &one.object).collect::<BTreeSet<_>>();
    items.iter().filter(|one| !whole.contains(&one.object) || is_whole(one)).cloned().collect()
}

/// A parameter identity's index, as Python's `0 <= index` asks it.
fn _index(index: &Option<Identity>) -> Result<i64, String> {
    match index {
        Some(Identity::Int(index)) => Ok(*index),
        other => Err(format!("'<=' not supported between instances of 'int' and '{other:?}'")),
    }
}

/// The summary `known` holds of `name`, where the body it describes is the
/// one that runs: LLVM's `hasExactDefinition`. A weak or linkonce body may
/// be replaced by another.
fn _summary<'s>(
    procedure: &CallFacts,
    known: &'s IndexMap<String, Summary>,
    name: &str,
) -> Option<&'s Summary> {
    (!procedure.replaceable.contains(name)).then(|| known.get(name)).flatten()
}

/// What a call back into the module adds to what a call reads and writes: the
/// callbacks' effects, and of the tracked globals all that the callbacks do not
/// state, or where none are known.
fn _calling_back(
    unit: &Unit,
    callbacks: Option<&Summary>,
    reading: bool,
    writing: bool,
    reads: &mut SliceSet,
    writes: &mut SliceSet,
) {
    let Some(callbacks) = callbacks else {
        if reading {
            reads.extend(_tracked(unit));
        }
        if writing {
            writes.extend(_tracked(unit));
        }
        return;
    };
    if reading {
        reads.absorb(&callbacks.reads);
        if callbacks.unknown_read {
            reads.extend(_tracked(unit));
        }
    }
    if writing {
        writes.absorb(&callbacks.writes);
        if callbacks.unknown_write {
            writes.extend(_tracked(unit));
        }
    }
}

/// What calling back into the module may do: the effects of the unit's
/// GlobalsAA entries, in no caller's object space; none where `known`
/// lacks one.
fn _callbacks(
    unit: &Unit,
    known: &IndexMap<String, Summary>,
) -> Option<Summary> {
    callbacks_over(unit.globals_aa?, known)
}

/// `_callbacks` of GlobalsAA's `globals` and the summaries `known`: all that
/// the answer depends on.
pub fn callbacks_over(
    globals: &globalsaa::Globals,
    known: &IndexMap<String, Summary>,
) -> Option<Summary> {
    CalledBack::of(globals, known)?.summary()
}

/// Transitive per-procedure mod/ref and capture summaries to a fixed point.
///
/// `known` supplies established external semantics, such as C library
/// functions. A body in this compilation unit always takes precedence; one
/// that may be replaced describes no call.
pub fn summaries<B: Bodies + ?Sized>(
    procedures: &B,
    known: Option<&IndexMap<String, Summary>>,
) -> Result<IndexMap<String, Summary>, String> {
    summaries_updating(procedures, known, &mut SummaryMemo::default(), None)
}

/// The bodies a run of `summaries` is over, in the order their numbers give:
/// what each calls is known without its `Procedure`, which is made for the
/// bodies visited.
pub trait Bodies {
    fn len(&self) -> usize;
    fn name(
        &self,
        at: usize,
    ) -> &str;
    fn position(
        &self,
        name: &str,
    ) -> Option<usize>;
    fn facts(
        &self,
        at: usize,
    ) -> &Rc<CallFacts>;
    fn procedure(
        &self,
        at: usize,
    ) -> Procedure<'_>;
    /// The globals' facts the bodies were made under.
    fn globals(&self) -> Option<&crate::globalsaa::Globals>;

    /// A token for these bodies' names and order, and for their facts: the
    /// same token as the run before says the names are the same, and the
    /// facts of a body outside the dirty set are the very ones it had. Zero
    /// where the bodies say nothing, and every run compares them.
    fn layout(&self) -> u64 {
        0
    }
}

impl Bodies for IndexMap<String, Procedure<'_>> {
    fn len(&self) -> usize {
        IndexMap::len(self)
    }

    fn name(
        &self,
        at: usize,
    ) -> &str {
        self.get_index(at).expect("a member of the graph").0
    }

    fn position(
        &self,
        name: &str,
    ) -> Option<usize> {
        self.get_index_of(name)
    }

    fn facts(
        &self,
        at: usize,
    ) -> &Rc<CallFacts> {
        &self.get_index(at).expect("a member of the graph").1.facts
    }

    fn procedure(
        &self,
        at: usize,
    ) -> Procedure<'_> {
        self.get_index(at).expect("a member of the graph").1.clone()
    }

    fn globals(&self) -> Option<&crate::globalsaa::Globals> {
        self.values().next().and_then(|one| one.unit.globals_aa)
    }
}

/// What a run of `summaries` leaves for the next: the summaries, the bodies'
/// own (`direct`) and their last visits, and what they were made from.
#[derive(Default)]
pub struct SummaryMemo {
    result: IndexMap<String, Summary>,
    direct: IndexMap<String, Summary>,
    /// The facts a call-free body's direct summary was made from: its visit,
    /// with no call to give them other captures, finds the same.
    direct_facts: IndexMap<String, Rc<PointsTo>>,
    /// The last visit of each body, in the order of the run, with its name.
    found: Vec<(String, Option<Visit>)>,
    known: Option<IndexMap<String, Summary>>,
    topology: Option<Rc<Topology>>,
    /// The bodies the topology was made of, in order, and their calls.
    made_of: Vec<(String, Rc<CallFacts>)>,
    /// The `layout` of the bodies the last run was over.
    layout: u64,
    /// Where the next run's versions start: past every version a kept visit
    /// holds, so that none of them is current and none is reset.
    epoch: usize,
    /// The version each body was last visited with, the run each was last
    /// started from nothing in, and the run each is queued in.
    seen: Vec<usize>,
    started: Vec<usize>,
    queued: Vec<usize>,
}

/// `summaries`, where only the bodies in `dirty` differ from the run `memo`
/// holds, and nothing else it was made from does (the caller's to know: the
/// globals' facts, the declarations, `known`).
///
/// A body's summary reads its own, its callees' and, where it calls something
/// unknown, the entries' (`callbacks`). So what an edit can change is the dirty
/// bodies and every body that reads them through a chain of calls. That closure
/// starts again from nothing, as a whole run does, and the rest is as it was:
/// nothing outside the closure reads anything in it. (The callbacks are an
/// entry's summaries read by every body that calls something unknown, and an
/// entry among those feeds them: a closure with an entry takes all of them.)
/// `None` for `dirty`, or a memo of other bodies, is a whole run.
pub fn summaries_updating<B: Bodies + ?Sized>(
    procedures: &B,
    known: Option<&IndexMap<String, Summary>>,
    memo: &mut SummaryMemo,
    dirty: Option<&BTreeSet<String>>,
) -> Result<IndexMap<String, Summary>, String> {
    let layout = procedures.layout();
    let same_layout = layout != 0 && memo.layout == layout && memo.found.len() == procedures.len();
    let same_bodies = same_layout || {
        let names: Vec<&str> = (0..procedures.len()).map(|at| procedures.name(at)).collect();
        memo.result.len() >= procedures.len()
            && names.iter().all(|name| memo.direct.contains_key(*name) && memo.result.contains_key(*name))
            && memo.direct.len() == procedures.len()
    };
    // Where every body is dirty nothing is kept, and what the update does to
    // keep it (waking readers, comparing what changed) costs more than the
    // run from nothing it comes to.
    let every = dirty.is_some_and(|dirty| (0..procedures.len()).all(|at| dirty.contains(procedures.name(at))));
    let whole = dirty.is_none() || every || !same_bodies || memo.known.as_ref() != known;
    if whole {
        WHOLE_RUNS.with(|runs| runs.set(runs.get() + 1));
    }
    if whole {
        *memo = SummaryMemo::default();
    }
    let same_layout = same_layout && !whole;
    let dirty_names: BTreeSet<&str> = match dirty {
        Some(dirty) if !whole => dirty.iter().map(String::as_str).collect(),
        _ => (0..procedures.len()).map(|at| procedures.name(at)).collect(),
    };
    // What a body captures grows from nothing: a call captures what its
    // callee's summary says, so a least fixed point, as a recursive one
    // that captures nothing proves.
    // What a body does on its own does not change from round to round: made
    // once, and again only for a body that was edited.
    llrm_support::debug::timed("summaries direct", || {
        // In the bodies' order: the objects a summary names are numbered as
        // they are first met.
        let mut dirty_at: Vec<usize> = dirty_names.iter().filter_map(|name| procedures.position(name)).collect();
        dirty_at.sort_unstable();
        for at in dirty_at {
            let procedure = procedures.procedure(at);
            let (summary, facts) = _direct_summary_with(&procedure.unit)?;
            let name = procedures.name(at).to_owned();
            if procedure.calls.is_empty() && procedure.arguments.is_empty() {
                memo.direct_facts.insert(name.clone(), facts);
            } else {
                memo.direct_facts.shift_remove(&name);
            }
            memo.direct.insert(name, summary);
        }
        Ok::<(), String>(())
    })?;
    let direct = std::mem::take(&mut memo.direct);
    let mut result = if whole { known.cloned().unwrap_or_default() } else { std::mem::take(&mut memo.result) };
    if whole {
        result.extend(
            direct.iter().map(|(name, one)| (name.clone(), Summary { captures: BTreeSet::new(), ..one.clone() })),
        );
    }
    let topology =
        llrm_support::debug::timed("summaries topology", || _topology(procedures, &result, memo, whole, &dirty_names));
    let Topology { component, order, rank, readers, callers_of_unknown, external, .. } = &*topology;
    let entries: Vec<usize> = procedures
        .globals()
        .map(|found| found.entries().iter().filter_map(|name| procedures.position(name)).collect())
        .unwrap_or_default();
    // The bodies that read a call back into the module, and their callers
    // among them: when an entry is one of those, an entry's summary feeds
    // what it is made of, so they stand or fall together and are worked out
    // from nothing as one, after everything else has settled.
    let (coupled, in_coupled) = topology
        .coupled
        .get_or_init(
            || {
                let mut up: BTreeSet<usize> = callers_of_unknown.clone();
                let mut todo: Vec<usize> = up.iter().copied().collect();
                while let Some(at) = todo.pop() {
                    todo.extend(readers[at].iter().copied().filter(|reader| up.insert(*reader)));
                }
                let list: Vec<usize> =
                    if entries.iter().any(|one| up.contains(one)) { up.into_iter().collect() } else { Vec::new() };
                let mut flags = vec![false; procedures.len()];
                list.iter().for_each(|at| flags[*at] = true);
                (list, flags)
            },
        );
    // Whether the coupled bodies wait (the others are being settled).
    let mut waiting_coupled = !whole && !coupled.is_empty();
    let mut deferred = false;
    let run = memo.epoch;
    let mut started = std::mem::take(&mut memo.started);
    started.resize(procedures.len(), usize::MAX);
    let mut queued = std::mem::take(&mut memo.queued);
    queued.resize(procedures.len(), usize::MAX);
    // A body is worked out again from what it reads now, and its readers
    // when it came out other than it was: gcc's summaries stop where a
    // function's does not change. A cycle of calls is worked out from nothing,
    // as its least fixed point is.
    let mut work: std::collections::BinaryHeap<std::cmp::Reverse<usize>> = Default::default();
    macro_rules! wake {
        ($at:expr) => {{
            let at: usize = $at;
            if waiting_coupled && in_coupled[at] {
                deferred = true;
            } else if queued[at] != run {
                queued[at] = run;
                work.push(std::cmp::Reverse(rank[at]));
            }
        }};
    }
    if whole {
        for at in 0..procedures.len() {
            wake!(at);
        }
    } else {
        for name in &dirty_names {
            if let Some(at) = procedures.position(name) {
                wake!(at);
            }
        }
        // A callee defined elsewhere whose declaration was restated: its
        // callers read it.
        for name in &dirty_names {
            for &at in external.get(*name).into_iter().flatten() {
                wake!(at);
            }
        }
    }
    let start = |at: usize, result: &mut IndexMap<String, Summary>| {
        let name = procedures.name(at);
        result.insert(name.to_owned(), Summary { captures: BTreeSet::new(), ..direct[name].clone() });
    };
    let mut called_back = procedures.globals().and_then(|globals| CalledBack::of(globals, &result));
    let mut callbacks = called_back.as_ref().and_then(CalledBack::summary);
    // What each body's points-to facts were found from: they change only with
    // the callees' captures, not with the effects a revisit is for. A visit
    // of an earlier run says what its calls to something unknown did
    // for the callbacks it had: that is worked out again.
    // A body that was edited has other facts than its last visit found.
    let mut last = std::mem::take(&mut memo.found);
    if same_layout {
        for name in &dirty_names {
            if let Some(at) = procedures.position(name) {
                last[at].1 = None;
            }
        }
    } else {
        last = (0..procedures.len())
            .map(|at| {
                let name = procedures.name(at);
                let visit = last
                    .get_mut(at)
                    .filter(|(then, _)| then == name)
                    .and_then(|(_, visit)| visit.take())
                    .filter(|_| !dirty_names.contains(name));
                (name.to_owned(), visit)
            })
            .collect();
    }
    let mut version = memo.epoch;
    // The callbacks' version each body was last visited with: a change to the
    // callbacks wakes the bodies that read them once the queue has drained,
    // the ones still in it being visited with the new callbacks anyway.
    let mut seen = std::mem::take(&mut memo.seen);
    seen.resize(procedures.len(), usize::MAX);
    let mut stale = false;
    loop {
        let Some(std::cmp::Reverse(first)) = work.pop() else {
            if stale {
                stale = false;
                for reader in callers_of_unknown.iter().copied().filter(|reader| seen[*reader] != version) {
                    wake!(reader);
                }
                continue;
            }
            if !(waiting_coupled && deferred) {
                break;
            }
            // Everything else is settled: the coupled bodies from nothing.
            waiting_coupled = false;
            stale = false;
            for at in coupled {
                start(*at, &mut result);
                started[*at] = run;
            }
            called_back = procedures.globals().and_then(|globals| CalledBack::of(globals, &result));
            callbacks = called_back.as_ref().and_then(CalledBack::summary);
            version += 1;
            for at in coupled {
                wake!(*at);
            }
            continue;
        };
        let at = order[first];
        queued[at] = usize::MAX;
        if !(whole || started[at] == run) && component[at].1 {
            // The first of a cycle to be worked out: all of it, from nothing.
            let id = component[at];
            for member in (0..procedures.len()).filter(|member| component[*member] == id) {
                start(member, &mut result);
                started[member] = run;
                wake!(member);
            }
            continue;
        }
        llrm_support::debug::counted("summaries rounds", true);
        seen[at] = version;
        let name = procedures.name(at);
        let procedure = procedures.procedure(at);
        let made = llrm_support::debug::timed("summaries visit", || {
            _summarized(
                at,
                &procedure,
                &direct[name],
                memo.direct_facts.get(name),
                &result,
                (callbacks.as_ref(), version),
                &mut last[at].1,
                component,
                procedures,
            )
        })?;
        if made != result[name] {
            result.insert(name.to_owned(), made);
            let woken: Vec<usize> = readers[at].iter().copied().collect();
            if entries.contains(&at) {
                // The callbacks may not move while an entry that is among the
                // coupled bodies' own makers does: they could hold each other
                // up.
                deferred |= waiting_coupled;
                let now = llrm_support::debug::timed("summaries callbacks", || {
                    match called_back.as_mut() {
                        Some(held) => held.changed(name, &result[name]),
                        None => {
                            called_back = procedures.globals().and_then(|globals| CalledBack::of(globals, &result));
                        }
                    }
                    called_back.as_ref().and_then(CalledBack::summary)
                });
                if llrm_support::env_flag!("LLRM_CHECK_CALLBACKS") {
                    let fresh = procedures.globals().and_then(|globals| callbacks_over(globals, &result));
                    assert!(now == fresh, "the callbacks folded in are not what adding the entries up again gives");
                }
                if now != callbacks {
                    callbacks = now;
                    version += 1;
                    stale = true;
                }
            }
            for reader in woken {
                wake!(reader);
            }
        }
    }
    memo.found = last;
    memo.layout = layout;
    memo.epoch = version + 1;
    memo.seen = seen;
    memo.started = started;
    memo.queued = queued;
    memo.direct = direct;
    memo.known = known.cloned();
    memo.result = result.clone();
    Ok(result)
}

/// Who calls whom, and the order to visit in: of the calls alone, so kept while
/// no body's calls moved.
struct Topology {
    /// Each body's component of the calls among them, and whether it is a
    /// cycle.
    component: Vec<(usize, bool)>,
    order: Vec<usize>,
    rank: Vec<usize>,
    readers: Vec<BTreeSet<usize>>,
    /// The bodies that call something unknown read the entries' summaries
    /// together, as one: what a call back into the module may do.
    callers_of_unknown: BTreeSet<usize>,
    /// The bodies worked out together from nothing when a call back into the
    /// module feeds an entry: none when it cannot.
    coupled: std::cell::OnceCell<(Vec<usize>, Vec<bool>)>,
    /// The bodies that call each function defined elsewhere, by its name.
    external: llrm_support::hash::HashMap<String, Vec<usize>>,
}

/// `procedures`' topology: the last run's where only the dirty bodies' calls
/// are as they were (the rest are the same facts), else made afresh.
fn _topology<B: Bodies + ?Sized>(
    procedures: &B,
    result: &IndexMap<String, Summary>,
    memo: &mut SummaryMemo,
    whole: bool,
    dirty_names: &BTreeSet<&str>,
) -> Rc<Topology> {
    if let Some(mut kept) = memo.topology.take().filter(|_| !whole) {
        let layout = procedures.layout();
        let same_layout = layout != 0 && memo.layout == layout;
        let same_bodies = memo.made_of.len() == procedures.len()
            && (same_layout || memo.made_of.iter().enumerate().all(|(at, (then, _))| procedures.name(at) == then));
        // The bodies whose calls are not the facts last held: only the dirty
        // ones may be (the layout says so where it is the same).
        let moved: Vec<usize> = if same_layout {
            dirty_names
                .iter()
                .filter_map(|name| procedures.position(name))
                .filter(|at| !Rc::ptr_eq(procedures.facts(*at), &memo.made_of[*at].1))
                .collect()
        } else if same_bodies {
            (0..procedures.len()).filter(|at| !Rc::ptr_eq(procedures.facts(*at), &memo.made_of[*at].1)).collect()
        } else {
            Vec::new()
        };
        if same_bodies && moved.iter().all(|at| dirty_names.contains(memo.made_of[*at].0.as_str())) {
            let changed: Vec<usize> = moved
                .iter()
                .copied()
                .filter(|at| {
                    let (now, then) = (procedures.facts(*at), &memo.made_of[*at].1);
                    now.calls != then.calls || now.sites != then.sites || now.replaceable != then.replaceable
                })
                .collect();
            if changed.is_empty() || _rewired(&mut kept, procedures, result, memo, &changed) {
                for at in moved {
                    memo.made_of[at].1 = Rc::clone(procedures.facts(at));
                }
                memo.topology = Some(Rc::clone(&kept));
                return kept;
            }
        }
    }
    let out: Vec<Vec<usize>> = (0..procedures.len())
        .map(|at| {
            let mut to: Vec<usize> =
                procedures.facts(at).calls.values().filter_map(|target| procedures.position(target)).collect();
            to.sort_unstable();
            to.dedup();
            to
        })
        .collect();
    let (of, cyclic) = llrm_mir::callgraph::strong_components(&out);
    // Callees before their callers, a component's members in order.
    let mut order: Vec<usize> = (0..procedures.len()).collect();
    order.sort_unstable_by_key(|one| (of[*one], *one));
    let component = of.iter().map(|one| (*one, cyclic[*one])).collect();
    let mut rank = vec![0; procedures.len()];
    for (at, one) in order.iter().enumerate() {
        rank[*one] = at;
    }
    let mut readers: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); procedures.len()];
    let mut callers_of_unknown: BTreeSet<usize> = BTreeSet::new();
    for at in 0..procedures.len() {
        let procedure = &**procedures.facts(at);
        for target in procedure.calls.values().filter_map(|target| procedures.position(target)) {
            readers[target].insert(at);
        }
        // Asked against the summaries as the bodies' own start from them:
        // whether a callee has one does not change.
        if procedure
            .sites
            .iter()
            .any(|site| procedure.calls.get(site).and_then(|target| _summary(procedure, result, target)).is_none())
        {
            callers_of_unknown.insert(at);
        }
    }
    memo.made_of =
        (0..procedures.len()).map(|at| (procedures.name(at).to_owned(), Rc::clone(procedures.facts(at)))).collect();
    let mut external: llrm_support::hash::HashMap<String, Vec<usize>> = Default::default();
    for at in 0..procedures.len() {
        for target in procedures.facts(at).calls.values().filter(|target| procedures.position(target).is_none()) {
            let callers = external.entry(target.clone()).or_default();
            if callers.last() != Some(&at) {
                callers.push(at);
            }
        }
    }
    let made = Rc::new(Topology {
        component,
        order,
        rank,
        readers,
        callers_of_unknown,
        coupled: Default::default(),
        external,
    });
    memo.topology = Some(Rc::clone(&made));
    made
}

/// The functions `facts` calls that are not among `procedures`.
fn _outside<'f, B: Bodies + ?Sized>(
    procedures: &B,
    facts: &'f CallFacts,
) -> BTreeSet<&'f String> {
    facts.calls.values().filter(|target| procedures.position(target).is_none()).collect()
}

/// `kept` brought up to date for the bodies `changed` (whose calls are not
/// those it was made of), where that leaves its order a valid one: each new
/// call is of a body of an earlier component, and none of the bodies is in a
/// cycle (taking a call out of one may split it). False where it does not, and
/// the topology is made afresh.
fn _rewired<B: Bodies + ?Sized>(
    kept: &mut Rc<Topology>,
    procedures: &B,
    result: &IndexMap<String, Summary>,
    memo: &SummaryMemo,
    changed: &[usize],
) -> bool {
    let Some(held) = Rc::get_mut(kept) else { return false };
    let targets = |procedure: &CallFacts| -> BTreeSet<usize> {
        procedure.calls.values().filter_map(|target| procedures.position(target)).collect()
    };
    for &at in changed {
        if held.component[at].1 {
            return false;
        }
        if targets(procedures.facts(at))
            .iter()
            .any(|to| held.component[*to] == held.component[at] || held.rank[*to] >= held.rank[at])
        {
            return false;
        }
    }
    for &at in changed {
        let before: BTreeSet<usize> =
            memo.made_of[at].1.calls.values().filter_map(|target| procedures.position(target)).collect();
        let now = targets(procedures.facts(at));
        let (was_outside, is_outside) =
            (_outside(procedures, &memo.made_of[at].1), _outside(procedures, procedures.facts(at)));
        for gone in was_outside.difference(&is_outside) {
            if let Some(callers) = held.external.get_mut(*gone) {
                callers.retain(|caller| *caller != at);
            }
        }
        for added in is_outside.difference(&was_outside) {
            let callers = held.external.entry((*added).clone()).or_default();
            if !callers.contains(&at) {
                callers.push(at);
            }
        }
        for gone in before.difference(&now) {
            held.readers[*gone].remove(&at);
        }
        for added in now.difference(&before) {
            held.readers[*added].insert(at);
        }
        let procedure = &**procedures.facts(at);
        let unknown = procedure
            .sites
            .iter()
            .any(|site| procedure.calls.get(site).and_then(|target| _summary(procedure, result, target)).is_none());
        if unknown {
            held.callers_of_unknown.insert(at);
        } else {
            held.callers_of_unknown.remove(&at);
        }
    }
    held.coupled = Default::default();
    true
}

/// What a body's last visit was made from, and found: its points-to facts, from
/// its callees' captures, and what its calls to something unknown may do, from
/// those facts and the callbacks of one `version`.
#[derive(Clone)]
struct Visit {
    captured: IndexMap<InstId, Option<BTreeSet<Option<Identity>>>>,
    facts: Rc<PointsTo>,
    version: Option<usize>,
    unknown: (SliceSet, SliceSet),
    /// What the calls to something unknown may do besides calling back, with
    /// whether the reads and the writes of one that does are wanted: found
    /// once from the facts, where `unknown` is found again for each callbacks.
    base: Option<(BTreeSet<Slice>, BTreeSet<Slice>, bool, bool)>,
}

/// `procedure`'s summary given `result`, the summaries of what it calls so far:
/// what its body does on its own (`direct`) and what each call does,
/// instantiated at its actuals.
fn _summarized(
    me: usize,
    procedure: &Procedure,
    direct: &Summary,
    direct_facts: Option<&Rc<PointsTo>>,
    result: &IndexMap<String, Summary>,
    (callbacks, version): (Option<&Summary>, usize),
    memo: &mut Option<Visit>,
    component: &[(usize, bool)],
    procedures: &(impl Bodies + ?Sized),
) -> Result<Summary, String> {
    VISITS.with(|visits| visits.set(visits.get() + 1));
    let captured_at = procedure
        .calls
        .iter()
        .map(|(at, target)| (*at, _summary(procedure, result, target).map(|one| one.captures.clone())))
        .collect::<IndexMap<_, _>>();
    if memo.as_ref().is_none_or(|one| one.captured != captured_at) {
        let facts = match direct_facts {
            Some(held) if captured_at.is_empty() && procedure.arguments.is_empty() => Rc::clone(held),
            _ => Rc::new(llrm_support::debug::timed("summaries points-to", || {
                points_to(&procedure.unit, Some(&procedure.arguments), Some(&captured_at))
            })?),
        };
        *memo = Some(Visit { captured: captured_at, facts, version: None, unknown: Default::default(), base: None });
    }
    let visit = memo.as_mut().expect("made above");
    let facts = Rc::clone(&visit.facts);
    // What the calls to something unknown may do, all of them together, depends
    // on the facts and on what a call back into the module may do, and on
    // nothing a revisit changes.
    if visit.base.is_none() {
        let unit = &procedure.unit;
        let (mut reads, mut writes) = (BTreeSet::new(), BTreeSet::new());
        // What a call may do other than through its arguments depends on its
        // callee and on what escaped before it, and a body calls the
        // same few routines again and again: each pair is worked out
        // once, and added to the whole once.
        #[allow(clippy::type_complexity)]
        let mut others: Vec<(
            Option<GlobalId>,
            Option<Rc<Bits>>,
            bool,
            bool,
            (BTreeSet<Slice>, BTreeSet<Slice>, bool),
        )> = Vec::new();
        for at in procedure.sites.iter().copied() {
            if procedure.calls.get(&at).and_then(|target| _summary(procedure, result, target)).is_some() {
                continue;
            }
            let allowed = _allowed(unit, at);
            let (read, written) = _through_arguments(&allowed, &_actuals(procedure, &facts, at));
            reads.extend(read);
            writes.extend(written);
            if !(allowed.other.reads || allowed.other.writes) {
                continue;
            }
            let (callee, escaped) =
                (llrm_mir::memory::callee(unit.context, unit.function, at), facts.escaped_before.bits(&at));
            match others.iter_mut().find(|(one, bits, ..)| *one == callee && *bits == escaped) {
                Some((_, _, wants_reads, wants_writes, _)) => {
                    *wants_reads |= allowed.other.reads;
                    *wants_writes |= allowed.other.writes;
                }
                None => others.push((
                    callee,
                    escaped,
                    allowed.other.reads,
                    allowed.other.writes,
                    _unknown_base(unit, &facts, at)?,
                )),
            }
        }
        let (mut back_reads, mut back_writes) = (false, false);
        for (_, _, wants_reads, wants_writes, (other_reads, other_writes, back)) in others {
            if wants_reads {
                reads.extend(other_reads);
                back_reads |= back;
            }
            if wants_writes {
                writes.extend(other_writes);
                back_writes |= back;
            }
        }
        visit.base = Some((reads, writes, back_reads, back_writes));
        visit.version = None;
    }
    if visit.version != Some(version) {
        let (reads, writes, back_reads, back_writes) = visit.base.clone().expect("found above");
        let (mut reads, mut writes) = (SliceSet::from(reads), SliceSet::from(writes));
        _calling_back(&procedure.unit, callbacks, back_reads, back_writes, &mut reads, &mut writes);
        visit.unknown = (reads, writes);
        visit.version = Some(version);
    }
    let (mut reads, mut writes) = (direct.reads.clone(), direct.writes.clone());
    reads.absorb(&visit.unknown.0);
    writes.absorb(&visit.unknown.1);
    let captures = facts
        .escaped
        .iter()
        .filter(|one| one.kind == MemoryKind::Parameter)
        .filter_map(|one| match one.key {
            Key::Int(number) => Some(Some(Identity::Int(number))),
            _ => None,
        })
        .collect();
    let (mut unknown_read, mut unknown_write) = (direct.unknown_read, direct.unknown_write);
    let mut types = direct.unknown_write_types.clone();
    for at in procedure.sites.iter().copied() {
        let target = procedure.calls.get(&at);
        let callee = target.and_then(|target| _summary(procedure, result, target));
        let Some(callee) = callee else { continue };
        let actual = _actuals(procedure, &facts, at);
        let mut effect = callee.instantiated(&actual);
        let target = target.expect("a known callee has a target");
        if procedures.position(target).is_some_and(|callee| component[me].1 && component[me] == component[callee]) {
            effect = _widen_parameters(effect);
        }
        reads.absorb(&effect.reads);
        writes.absorb(&effect.writes);
        unknown_read |= effect.unknown_read;
        types = merged_types((unknown_write, &types), (effect.unknown_write, &effect.unknown_write_types));
        unknown_write |= effect.unknown_write;
    }
    // A callee's frame is gone when it returns: what its calls touch
    // there, like its own accesses, is no effect of calling it.
    reads.retain_outliving();
    writes.retain_outliving();
    Ok(Summary {
        reads: reads.coalesced(),
        writes: writes.coalesced(),
        captures,
        unknown_read,
        unknown_write,
        unknown_write_types: types,
        globals_listed: false,
    })
}

/// What one call reads and writes, as the bytes of the objects it reaches.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Effect {
    /// Shared by the calls whose effect is the same: most are the unit's
    /// tracked globals and what escaped.
    pub loads: Rc<[MemRef]>,
    pub stores: Rc<[MemRef]>,
    /// The bytes it writes before reading any, as `initializes` states.
    pub fills: Vec<MemRef>,
}

impl Effect {
    /// Whether `self`, held past edits that keep every value's meaning, still
    /// states what `fresh` finds. `loads` and `stores` are may-sets: `self`
    /// covers `fresh` where every reference of `fresh` is one of `self` (a
    /// value replaced by another found equal leaves a reference the pointer
    /// analysis had of the first: a global beside the unknown that a fresh run
    /// of the one value states alone). The unknown object reaches only what
    /// escaped: a kept `{global}` does not cover a fresh `{unknown}`. `fills`
    /// are must-writes, a kept one that `fresh` lacks claims bytes the body
    /// no longer shows: equal.
    pub fn covers(
        &self,
        fresh: &Effect,
    ) -> bool {
        // A reference that names no value is its provenance alone: covered
        // where each of its slices is within a kept slice by the bytes
        // it names, or within the unknown object's whole range where
        // its object escaped (the unknown reaches what escaped). One
        // that names a value is a decomposition, and stays exact.
        let names_nothing =
            |one: &MemRef| one.pointer.is_none() && one.root.is_none() && one.base.is_none() && one.segment.is_none();
        let within = |old: &[MemRef], new: &[MemRef]| {
            let kept_slices = || old.iter().filter_map(|one| one.provenance.as_ref()).flat_map(|one| one.slices.iter());
            let whole = |kind: MemoryKind| {
                kept_slices().any(|one| one.object.kind == kind && one.low <= -(1_i64 << 31) && one.high >= 1_i64 << 31)
            };
            // `objects_may_alias` asks both objects addressed first, then the
            // other one captured.
            let reached = |slice: &Slice| slice.object.addressed() && slice.object.captured();
            let unknown_whole = whole(MemoryKind::Unknown);
            // What a callee may reach (`Nonlocal`) meets what escaped, a frame
            // object aside: `objects_may_alias`' own rule.
            let nonlocal_whole = whole(MemoryKind::Nonlocal);
            new.iter().all(|one| {
                old.contains(one)
                    || (names_nothing(one)
                        && one.provenance.as_ref().is_some_and(|provenance| {
                            provenance
                                .slices
                                .iter()
                                .all(
                                    |slice| slice_covered(slice, kept_slices())
                                        || (unknown_whole && reached(slice))
                                        || (nonlocal_whole
                                            && reached(slice)
                                            && !matches!(slice.object.kind, MemoryKind::Frame | MemoryKind::Stack)),
                                )
                        }))
            })
        };
        within(&self.loads, &fresh.loads) && within(&self.stores, &fresh.stores) && self.fills == fresh.fills
    }
}

/// What each call of `unit` reads and writes as its attributes and the unit's
/// `GlobalsModRef` say, with no points-to solve (`getModRefInfo(Call, Loc)`
/// as the pin's BasicAA and GlobalsAA read it): through each pointer argument
/// the call may read or write, as a reference to the whole object it points
/// into; elsewhere (`other`) unknown and non-local memory and the tracked
/// globals the callee may touch (all of them where `GlobalsModRef` has nothing
/// of the callee).
pub fn calls_by_attributes(unit: &Unit) -> llrm_mir::dense::IdMap<InstId, Effect> {
    FOOTPRINTS.with(|made| made.set(made.get() + 1));
    let tracked: Vec<GlobalId> =
        unit.globals_aa.into_iter().flat_map(|aa| aa.tracked_globals().iter().copied()).collect();
    let reference = |one: &Slice| {
        MemRef::reach(
            u32::MAX >> 2,
            Provenance { slices: crate::memory::Sorted::from([one.clone()]), restrict: crate::memory::Sorted::new() },
        )
    };
    // What a call reaches elsewhere depends on its callee alone, and on the
    // module analyses the unit reads: kept with the proxy that holds them,
    // where the unit is made under it.
    let memo = unit.outer.filter(|outer| {
        let same = |held: Option<*const ()>, now: Option<*const ()>| held == now;
        same(
            unit.globals_aa.map(|one| std::ptr::from_ref(one).cast()),
            outer
                .cached_ref::<crate::manager::GlobalsAA>()
                .and_then(|one| one.as_ref().ok())
                .map(|one| std::ptr::from_ref(one).cast()),
        ) && same(
            unit.globals_mr.map(|one| std::ptr::from_ref(one).cast()),
            outer.cached_ref::<crate::manager::GlobalsModRefAnalysis>().map(|one| std::ptr::from_ref(one).cast()),
        )
    });
    let reaches = |callee: Option<GlobalId>, nocallback: bool| -> (Rc<[MemRef]>, Rc<[MemRef]>) {
        REACHES.with(|made| made.set(made.get() + 1));
        // A callee that cannot call back writes of the tracked globals only
        // what the runtime module promises of it (`!llrm.writes`).
        let promised = if nocallback { unit.globals_aa.map(|aa| aa.unsummarized(callee)) } else { None };
        let (mut reads, mut writes) = (SliceSet::new(), SliceSet::new());
        for object in [ObjectRef::UNKNOWN, ObjectRef::NONLOCAL] {
            reads.insert(Slice::whole(object));
            writes.insert(Slice::whole(object));
        }
        for &global in &tracked {
            let Some(object) = memory::global_object(unit, global) else { continue };
            // What reaches the global through an argument is the argument's own
            // reference.
            let touched = unit.globals_mr.map_or(crate::aa::ModRefInfo::ModRef, |all| {
                all.get_mod_ref_info_for_global(unit.globals, callee, global, || crate::aa::ModRefInfo::NoModRef)
            });
            // Only a named global is promised anything: the runtime holds its
            // address by name alone. Any other is reached through arguments
            // too.
            let (reads_it, writes_it) = match &promised {
                Some((named, written)) if named.contains(&global) => (true, written.contains(&global)),
                _ => (true, true),
            };
            if touched.is_ref_set() && reads_it {
                reads.insert(Slice::whole(object));
            }
            if touched.is_mod_set() && writes_it {
                writes.insert(Slice::whole(object));
            }
        }
        (reads.iter().map(&reference).collect(), writes.iter().map(&reference).collect())
    };
    let remembered = |key: u64, make: &dyn Fn() -> (Rc<[MemRef]>, Rc<[MemRef]>)| match memo {
        Some(outer) => (*outer.memo(key, make)).clone(),
        None => make(),
    };
    let mut elsewhere: llrm_mir::dense::IdMap<u32, (Rc<[MemRef]>, Rc<[MemRef]>)> = llrm_mir::dense::IdMap::default();
    let mut elsewhere_unknown = None;
    let mut out = llrm_mir::dense::IdMap::default();
    // Calls reaching the same references share the one list.
    // Hashed by Fx: the default SipHash of these lists was 27 M Ir on d_faces
    // -O2.
    let mut shared: llrm_support::hash::HashMap<Vec<MemRef>, Rc<[MemRef]>> = llrm_support::hash::HashMap::default();
    let mut share = |list: Vec<MemRef>| -> Rc<[MemRef]> {
        if let Some(one) = shared.get(&list) {
            return Rc::clone(one);
        }
        let one: Rc<[MemRef]> = list.as_slice().into();
        shared.insert(list, Rc::clone(&one));
        one
    };
    for (_, at) in unit.function.walk() {
        let op = unit.function.instruction(at);
        let (Opcode::Call(_) | Opcode::Invoke(_)) = op.opcode else { continue };
        let count = match op.opcode {
            Opcode::Call(_) => op.operands.len() - 1,
            _ => op.operands.len() - 3,
        };
        let allowed = _allowed(unit, at);
        let (mut loads, mut stores): (Vec<MemRef>, Vec<MemRef>) = (Vec::new(), Vec::new());
        for (index, &argument) in op.operands[..count].iter().enumerate() {
            if !is_pointer(unit, argument) {
                continue;
            }
            let through = allowed.through.get(index).copied().unwrap_or(Effects::ANY);
            let width = match crate::aa::MemoryLocation::get_for_argument(unit, at, index as u32).map(|one| one.size) {
                Some(crate::aa::LocationSize::Precise(bytes)) => u32::try_from(bytes).unwrap_or(u32::MAX >> 2),
                _ => u32::MAX >> 2,
            };
            let mut place = MemRef::at(unit, argument, width);
            place.provenance = unit.point_values.and_then(|values| values.reference(unit, &place));
            // A pointer of no known object (a far pointer built from a segment
            // word) reaches any object that may be reached: the unknown one.
            if place.provenance.is_none() {
                place = MemRef::reach(
                    width,
                    Provenance {
                        slices: crate::memory::Sorted::from([Slice::whole(ObjectRef::UNKNOWN)]),
                        restrict: crate::memory::Sorted::new(),
                    },
                );
            }
            if through.writes && !_by_value(unit, at, index) {
                stores.push(place.clone());
            }
            if through.reads {
                loads.push(place);
            }
        }
        let callee = llrm_mir::memory::callee(unit.context, unit.function, at);
        let nocallback = !crate::globalsaa::calls_back(unit, at);
        let (other_loads, other_stores) = match callee {
            Some(id) => {
                let key = id.0 * 2 + u32::from(nocallback);
                if !elsewhere.contains_key(&key) {
                    elsewhere.insert(key, remembered(u64::from(key), &|| reaches(callee, nocallback)));
                }
                elsewhere.get(&key).expect("inserted above")
            }
            None => elsewhere_unknown.get_or_insert_with(|| remembered(u64::MAX, &|| reaches(None, false))),
        };
        if allowed.other.reads {
            loads.extend(other_loads.iter().cloned());
        }
        if allowed.other.writes {
            stores.extend(other_stores.iter().cloned());
        }
        out.insert(at, Effect { loads: share(loads), stores: share(stores), fills: Vec::new() });
    }
    out
}

/// Whether argument `argument` of the call `at` is passed byval: the callee's
/// own copy, which its writes do not carry back.
fn _by_value(
    unit: &Unit,
    at: InstId,
    argument: usize,
) -> bool {
    let named = |attrs: &[llrm_mir::opcode::Attribute]| {
        attrs.iter().any(|one| {
            matches!(
                one,
                llrm_mir::opcode::Attribute::Flag(name) | llrm_mir::opcode::Attribute::Type(name, _) if name == "byval"
            )
        })
    };
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &unit.function.instruction(at).opcode else { return false };
    info.argument_attrs.get(argument).is_some_and(|attrs| named(attrs))
        || llrm_mir::memory::callee(unit.context, unit.function, at)
            .and_then(|one| unit.globals.get(one.0 as usize))
            .and_then(|one| one.function())
            .and_then(|one| one.parameter_attrs.get(argument))
            .is_some_and(|attrs| named(attrs))
}

/// The bytes `slice` may cover, or `None` for all of them.
fn _bytes(one: &Slice) -> Option<(i64, i64)> {
    (one.low != memory::WHOLE_LOW && one.high != memory::WHOLE_HIGH).then(|| (one.low, one.high - 1 + one.width))
}

fn _union<'a>(parts: impl IntoIterator<Item = Option<&'a Provenance>>) -> Option<Provenance> {
    let values = parts.into_iter().flatten().collect::<Vec<_>>();
    let (first, rest) = values.split_first()?;
    let mut result = (*first).clone();
    for one in rest {
        result = result.union(one);
    }
    Some(result)
}

/// The whole object after a loop-carried pointer fact changes.
///
/// A finite set of exact offsets is not a finite lattice for `p = p + n`:
/// every trip around the back edge manufactures another offset. At a
/// natural-loop header, use the standard abstract-interpretation widening
/// instead. Keeping object identity and restrict roots still proves the
/// important disjointness facts; only the changing subrange is forgotten.
fn _widened(provenance: &Provenance) -> Provenance {
    let slices = provenance.slices.iter().filter_map(|one| Slice::every_byte(one.object)).collect();
    Provenance { slices, restrict: provenance.restrict.clone() }
}

/// Whether the call `inst` returns what its callee states `noalias`: a
/// pointer to an object no other pointer reaches, as a constructor's.
fn returns_unique(
    unit: &Unit,
    inst: InstId,
) -> bool {
    llrm_mir::memory::callee(unit.context, unit.function, inst)
        .and_then(|callee| unit.globals.get(callee.0 as usize)?.function())
        .is_some_and(|callee| Facts::of(&callee.return_attrs).no_alias())
}

/// What `inst` computes as a pointer from what it is given: an object's
/// own address, or a known pointer moved, cast or joined.
fn _direct(
    unit: &Unit,
    inst: InstId,
    values: &IndexMap<ValueId, Provenance>,
) -> Result<Option<Provenance>, String> {
    let op = unit.function.instruction(inst);
    let Some(result) = op.result.filter(|&result| is_pointer(unit, Operand::Value(result))) else {
        return Ok(None);
    };
    match &op.opcode {
        Opcode::Alloca { .. } => {
            let object = object_of(unit, Operand::Value(result)).expect("an alloca is an object");
            Provenance::one_with_slice(object, 0, 1, 1, 1, BTreeSet::new()).map(Some).map_err(|error| error.to_string())
        }
        // The same bytes as its operand, said restrict: what it and what is
        // based on it reach is reached by nothing that is not.
        // The window of a huge pointer: the same bytes, said by another
        // pointer.
        Opcode::Call(_) if unit.intrinsic(inst) == Some(llrm_mir::intrinsics::Intrinsic::Window) => {
            Ok(_operand(unit, op.operands[0], values))
        }
        Opcode::Call(_) if unit.intrinsic(inst) == Some(llrm_mir::intrinsics::Intrinsic::Restrict) => {
            let Some(source) = _operand(unit, op.operands[0], values) else { return Ok(None) };
            let restrict = source.restrict.union(&crate::memory::Sorted::from([Identity::Value(result.0)]));
            Ok(Some(Provenance { slices: source.slices, restrict }))
        }
        // A callee whose result is `noalias` returns a pointer to an object
        // nothing else points to: its own, apart from every other.
        Opcode::Call(_) | Opcode::Invoke(_) if returns_unique(unit, inst) => {
            let object = ObjectInterner::of(unit.context).intern(MemoryObject {
                identity: Some(Identity::Value(result.0)),
                addressed: true,
                captured: true,
                ..MemoryObject::new(MemoryKind::Allocation)
            });
            Provenance::one_with_slice(object, 0, 1, 1, 1, BTreeSet::new()).map(Some).map_err(|error| error.to_string())
        }
        // A segment is no pointer to a program object: `segment:0` is a
        // new root.
        Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast)
            if !unit.spaces().is_segment(unit.space(op.operands[0])) =>
        {
            Ok(_operand(unit, op.operands[0], values))
        }
        Opcode::GetElementPtr { source } => {
            let Some(fact) = _operand(unit, op.operands[0], values) else {
                return Ok(None);
            };
            let indices = op.operands[1..]
                .iter()
                .map(|&one| {
                    unit.int_constant(one)
                        .map(|bits| llrm_mir::context::signed(bits, unit.int_bits(one).unwrap_or(128)))
                })
                .collect::<Vec<_>>();
            let (constant, variable) = unit.layout.collect_offset(&unit.context.types, *source, &indices);
            if variable.is_empty() {
                // A displacement is an index-width integer: -16 is never 65520.
                let space = unit.space(op.operands[0]).unwrap_or(0);
                return Ok(Some(fact.shifted(wrapped(constant, unit.layout.pointer(space).index_bits))));
            }
            // Arithmetic by an unknown integer remains within each known
            // object, but no longer has a byte offset precise enough to
            // compare.
            Ok(Some(_widened(&fact)))
        }
        _ => Ok(None),
    }
}

/// `points_to` of a whole function with no caller context.
pub fn pointers(unit: &Unit) -> Result<Rc<PointsTo>, String> {
    points_to(unit, None, None).map(Rc::new)
}

/// The values `inst` reads, its access's index and selector among them.
fn read_values(
    unit: &Unit,
    inst: InstId,
) -> impl Iterator<Item = ValueId> {
    let op = unit.function.instruction(inst);
    let mut read = op
        .operands
        .iter()
        .filter_map(|one| if let Operand::Value(value) = one { Some(*value) } else { None })
        .collect::<Vec<_>>();
    if let Some(reference) = MemRef::of(unit, inst) {
        read.extend(reference.base);
        read.extend(reference.root.and_then(|root| if let Operand::Value(value) = root { Some(value) } else { None }));
    }
    read.into_iter()
}

/// A phi's or a select's arms, as operands.
fn joined(op: &llrm_mir::module::Instruction) -> Option<Vec<Operand>> {
    match op.opcode {
        Opcode::Phi => Some(op.operands.iter().step_by(2).copied().collect()),
        Opcode::Select => Some(op.operands[1..].to_vec()),
        _ => None,
    }
}

/// What the pointer solve finds of a body before it asks what escapes: each
/// pointer value's provenance and the pointer cells held on entry to each
/// block. The call arguments and captures never enter it, so one holds for
/// every configuration of `points_to` over the same body, declarations and
/// context.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PointValues {
    pub values: llrm_support::hash::SparseIdMap<ValueId, Provenance>,
}

thread_local! {
    static VALUE_SOLVES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static ESCAPE_SOLVES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static VALUE_ROUNDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static LIMIT_COLLAPSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static FOOTPRINTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static REACHES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has worked out the footprints of a body's calls
/// (`calls_by_attributes`), for a test that `Writes` and `Accesses` share one.
pub fn footprints_made() -> usize {
    FOOTPRINTS.with(std::cell::Cell::get)
}

/// How many times this thread has worked out what a callee reaches elsewhere
/// (the `reaches` of `calls_by_attributes`), for a test that a proxy keeps
/// them.
pub fn reaches_made() -> usize {
    REACHES.with(std::cell::Cell::get)
}

/// How many rounds over the blocks the value solves of this thread have made,
/// for a test that a solve finishes within a bound.
pub fn value_rounds() -> usize {
    VALUE_ROUNDS.with(std::cell::Cell::get)
}

/// How many facts this thread has collapsed to whole objects for having more
/// slices than `MAX_FIELDS_FOR_FIELD_SENSITIVE`.
pub fn limit_collapses() -> usize {
    LIMIT_COLLAPSES.with(std::cell::Cell::get)
}

/// The most slices a pointer may be known by before its objects are taken
/// whole: gcc stops treating a structure field by field past
/// `max-fields-for-field-sensitive` (100 from -O2, tree-ssa-structalias.cc
/// via params.opt), and LLVM's BasicAA caps its depth the same way. A fact
/// grows by one slice a round where nothing widens it (a loop the shape does
/// not know of) and never settles; past the limit it settles at the objects'
/// whole extents.
pub const MAX_FIELDS_FOR_FIELD_SENSITIVE: usize = 100;

/// How many escape solves (`points_to`'s second phase) this thread has made,
/// for a test that what reads only the pointer values does not ask one.
pub fn escape_solves() -> usize {
    ESCAPE_SOLVES.with(std::cell::Cell::get)
}

/// How many value solves this thread has made, for a test that two analyses of
/// one body ask one.
pub fn value_solves() -> usize {
    VALUE_SOLVES.with(std::cell::Cell::get)
}

/// One instruction of the value solve: what it learns of a pointer value, and
/// what it adds to the cells of the objects it stores through.
struct Solve {
    values: IndexMap<ValueId, Provenance>,
    /// Every pointer stored anywhere in each object: what a load from any cell
    /// of it may get.
    fields: HashMap<ObjectRef, Provenance>,
    /// Objects a store of an unknown pointer, or a call, may have written.
    unbounded: HashSet<ObjectRef>,
    limited: HashSet<ValueId>,
    any_limited: bool,
}

impl Solve {
    /// `fact` as `value`'s, widened past the field limit; whether it was new.
    fn learn(
        &mut self,
        value: ValueId,
        fact: Provenance,
    ) -> bool {
        let fact = if fact.slices.len() > MAX_FIELDS_FOR_FIELD_SENSITIVE
            || (self.any_limited && self.limited.contains(&value))
        {
            self.any_limited = true;
            if self.limited.insert(value) {
                LIMIT_COLLAPSES.with(|collapsed| collapsed.set(collapsed.get() + 1));
            }
            _widened(&fact)
        } else {
            fact
        };
        if self.values.get(&value) == Some(&fact) {
            return false;
        }
        self.values.insert(value, fact);
        true
    }
}

/// The value solve of `points_to`: gcc's `solve_graph` over SSA values
/// ([tree-ssa-structalias.cc#L2711](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-structalias.cc#L2711)),
/// one constraint per instruction that makes, moves, joins, loads or stores a
/// pointer, visited again only when a value or an object's fields it reads
/// changed. The contents of a cell are one abstract location per object (every
/// pointer stored into it anywhere, or unknown): what an exact cell holds at a
/// point is the memory SSA's to say per query.
pub fn point_values(unit: &Unit) -> Result<PointValues, String> {
    VALUE_SOLVES.with(|solves| solves.set(solves.get() + 1));
    let function = unit.function;
    let seeds = seeds(unit);
    let pointer_values = may_point(unit);
    let graph = cfg::graph(function);
    let shape = unit.shape();
    let dominance = &shape.dominance;
    let tree = dominance.tree(function);
    let back_edges = graph
        .iter()
        .flat_map(|block| block.succ.iter().map(move |successor| (block.at, *successor)))
        .filter(|(at, successor)| dominance.dominates(unit.function, *successor, *at))
        .collect::<BTreeSet<_>>();
    // The instructions that can give a pointer a fact, in program order, and
    // the ones that read each value they read.
    let mut order: Vec<InstId> = Vec::new();
    for block in &graph {
        for inst in function.iter_block(cfg::block(block.at)) {
            let op = function.instruction(inst);
            let pointer_result = op.result.is_some_and(|result| pointer_values.contains(&result));
            let pointer_store = matches!(op.opcode, Opcode::Store { .. }) && is_pointer(unit, op.operands[0]);
            if pointer_result || pointer_store {
                order.push(inst);
            }
        }
    }
    // What a load of a slot nothing else writes reads: the stores that reach
    // it.
    let pointer_loads: llrm_mir::dense::IdSet<InstId> = order
        .iter()
        .copied()
        .filter(|&inst| {
            let op = function.instruction(inst);
            matches!(op.opcode, Opcode::Load { .. }) && op.result.is_some_and(|result| pointer_values.contains(&result))
        })
        .collect();
    let exact = crate::slotstores::reaching(unit, &pointer_loads);
    // A load that a store after it in a cycle reaches carries a fact around the
    // cycle: widened, as a loop-carried phi is, or each trip adds a slice.
    let carried: llrm_mir::dense::IdSet<InstId> = {
        let place: llrm_mir::dense::IdMap<InstId, usize> = function
            .layout()
            .iter()
            .flat_map(|&block| function.iter_block(block).enumerate().map(|(at, inst)| (inst, at)))
            .collect();
        pointer_loads
            .iter()
            .filter(|load| {
                let Some(stores) = exact.get(load) else { return false };
                let Some(here) = function.parent(*load) else { return false };
                stores
                    .iter()
                    .any(
                        |&store| {
                            let Some(there) = function.parent(store) else { return false };
                            if there == here {
                                place.get(&store) > place.get(load)
                            } else {
                                crate::reachability::is_potentially_reachable_blocks(function, here, there, Some(tree))
                            }
                        },
                    )
            })
            .collect()
    };
    let mut readers: HashMap<ValueId, Vec<usize>> = HashMap::default();
    for (slot, &inst) in order.iter().enumerate() {
        for value in read_values(unit, inst).chain(exact.get(&inst).into_iter().flatten().flat_map(|&store| {
            match function.instruction(store).operands[0] {
                Operand::Value(source) => Some(source),
                _ => None,
            }
        })) {
            let list = readers.entry(value).or_default();
            if list.last() != Some(&slot) {
                list.push(slot);
            }
        }
    }
    let mut solve = Solve {
        values: seeds.clone(),
        fields: HashMap::default(),
        unbounded: HashSet::default(),
        limited: HashSet::default(),
        any_limited: false,
    };
    // The loads that read each object's cells.
    let mut watchers: HashMap<ObjectRef, HashSet<usize>> = HashMap::default();
    let mut queued = vec![true; order.len()];
    let mut work: std::collections::BinaryHeap<std::cmp::Reverse<usize>> =
        (0..order.len()).map(std::cmp::Reverse).collect();
    let wake_readers =
        |value: ValueId, work: &mut std::collections::BinaryHeap<std::cmp::Reverse<usize>>, queued: &mut Vec<bool>| {
            for &reader in readers.get(&value).into_iter().flatten() {
                if !queued[reader] {
                    queued[reader] = true;
                    work.push(std::cmp::Reverse(reader));
                }
            }
        };
    let wake_watchers = |object: &ObjectRef,
                         watchers: &HashMap<ObjectRef, HashSet<usize>>,
                         work: &mut std::collections::BinaryHeap<std::cmp::Reverse<usize>>,
                         queued: &mut Vec<bool>| {
        for &reader in watchers.get(object).into_iter().flatten() {
            if !queued[reader] {
                queued[reader] = true;
                work.push(std::cmp::Reverse(reader));
            }
        }
    };
    while let Some(std::cmp::Reverse(slot)) = work.pop() {
        queued[slot] = false;
        VALUE_ROUNDS.with(|rounds| rounds.set(rounds.get() + 1));
        let inst = order[slot];
        let op = function.instruction(inst);
        if let (Some(arms), Some(result)) = (joined(op), op.result) {
            let parts = arms.iter().map(|&one| _operand(unit, one, &solve.values)).collect::<Vec<_>>();
            if pointer_values.contains(&result) || (!parts.is_empty() && parts.iter().all(Option::is_some)) {
                let mut fact = if parts.iter().any(Option::is_none) {
                    Some(UNKNOWN.clone())
                } else {
                    _union(parts.iter().map(Option::as_ref))
                };
                if let Some(current) = &fact {
                    let block = function.parent(inst).map(cfg::id);
                    let carried = op.opcode == Opcode::Phi
                        && op.operands.iter().skip(1).step_by(2).any(|parent| {
                            matches!(
                                parent,
                                Operand::Block(parent)
                                    if block.is_some_and(|block| back_edges.contains(&(cfg::id(*parent), block)))
                            )
                        });
                    if carried {
                        if let Some(previous) = solve.values.get(&result) {
                            if current != previous {
                                fact = Some(_widened(&previous.union(current)));
                            }
                        }
                    }
                }
                if let Some(fact) = fact {
                    if solve.learn(result, fact) {
                        wake_readers(result, &mut work, &mut queued);
                    }
                }
            }
            continue;
        }
        // Arithmetic derived from an already-known pointer proves its own
        // pointer nature.
        if let (Some(direct), Some(result)) = (_direct(unit, inst, &solve.values)?, op.result) {
            if !seeds.contains_key(&result) && solve.learn(result, direct) {
                wake_readers(result, &mut work, &mut queued);
            }
        }
        let reference = MemRef::of(unit, inst);
        match (&op.opcode, reference) {
            (Opcode::Load { .. }, Some(reference))
                if op.result.is_some_and(|result| pointer_values.contains(&result)) =>
            {
                // Every store that reaches a slot only this body writes: what
                // was stored, and nothing else.
                if let Some(stores) = exact.get(&inst) {
                    let sources: Vec<Option<Provenance>> = stores
                        .iter()
                        .map(|&store| _operand(unit, function.instruction(store).operands[0], &solve.values))
                        .collect();
                    if sources.iter().all(Option::is_some) {
                        if let Some(mut loaded) = _union(sources.iter().map(Option::as_ref)) {
                            let result = op.result.expect("a load's result");
                            if carried.contains(&inst) {
                                if let Some(previous) = solve.values.get(&result) {
                                    if loaded != *previous {
                                        loaded = _widened(&previous.union(&loaded));
                                    }
                                }
                            }
                            if solve.learn(result, loaded) {
                                wake_readers(result, &mut work, &mut queued);
                            }
                        }
                        continue;
                    }
                }
                let resolved =
                    MemRef { provenance: _resolved_reference(unit, &reference, &solve.values), ..reference.clone() };
                let mut stored = Vec::new();
                let mut known = true;
                for one in resolved.provenance.iter().flat_map(|provenance| provenance.slices.iter()) {
                    watchers.entry(one.object).or_default().insert(slot);
                    match (!solve.unbounded.contains(&one.object)).then(|| solve.fields.get(&one.object)).flatten() {
                        Some(held) => stored.push(held),
                        None => known = false,
                    }
                }
                if known && !stored.is_empty() {
                    let loaded = _union(stored.into_iter().map(Some).chain([Some(&*UNKNOWN)]));
                    if let Some(loaded) = loaded {
                        let result = op.result.expect("a load's result");
                        if solve.learn(result, loaded) {
                            wake_readers(result, &mut work, &mut queued);
                        }
                    }
                }
            }
            (Opcode::Store { .. }, Some(reference)) => {
                let source = _operand(unit, op.operands[0], &solve.values);
                let keyed = MemRef { provenance: _resolved_reference(unit, &reference, &solve.values), ..reference };
                let pointer_stored = is_pointer(unit, op.operands[0]);
                let bounded = source.is_some() || !pointer_stored;
                if let (false, Some(targets)) = (bounded, &keyed.provenance) {
                    for one in &targets.slices {
                        if solve.unbounded.insert(one.object) {
                            wake_watchers(&one.object, &watchers, &mut work, &mut queued);
                        }
                    }
                }
                if let (Some(source), Some(targets)) = (&source, &keyed.provenance) {
                    for one in &targets.slices {
                        // Whole objects: stored offsets may shift each trip
                        // around a loop.
                        let grown = _widened(
                            &solve.fields.get(&one.object).map_or_else(|| source.clone(), |held| held.union(source)),
                        );
                        if solve.fields.get(&one.object) != Some(&grown) {
                            solve.fields.insert(one.object, grown);
                            wake_watchers(&one.object, &watchers, &mut work, &mut queued);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(PointValues { values: solve.values })
}

/// Flow pointer objects through values, spill slots and CFG joins.
pub fn points_to(
    unit: &Unit,
    arguments: Option<&IndexMap<InstId, Vec<Actual>>>,
    captures: Option<&IndexMap<InstId, Option<BTreeSet<Option<Identity>>>>>,
) -> Result<PointsTo, String> {
    let solved = match unit.point_values {
        Some(held) => {
            if llrm_support::env_flag!("LLRM_CHECK_POINTVALUES") {
                assert!(*held == point_values(unit)?, "the held point values are not what solving again gives");
            }
            std::borrow::Cow::Borrowed(held)
        }
        None => std::borrow::Cow::Owned(point_values(unit)?),
    };
    let found = escapes(unit, &solved, arguments, captures)?;
    if llrm_support::env_flag!("LLRM_CHECK_CAPTURES") {
        crate::capturecheck::check(unit, &found);
    }
    Ok(found)
}

/// What `points_to` adds to the value solve: what escapes, and when.
fn escapes(
    unit: &Unit,
    solved: &PointValues,
    arguments: Option<&llrm_support::hash::SparseIdMap<InstId, Vec<Actual>>>,
    captures: Option<&llrm_support::hash::SparseIdMap<InstId, Option<BTreeSet<Option<Identity>>>>>,
) -> Result<PointsTo, String> {
    ESCAPE_SOLVES.with(|solves| solves.set(solves.get() + 1));
    let function = unit.function;
    let graph = cfg::graph(function);
    let values = &solved.values;
    let instructions = |at: i64| function.iter_block(cfg::block(at));
    // Escape is flow-sensitive separately from pointer contents. A pointer
    // published after a call must not make the earlier call reach its frame.
    let mut pointer_fields: IndexMap<ObjectRef, BTreeSet<ObjectRef>> = IndexMap::default();
    for (_, inst) in function.walk() {
        let op = function.instruction(inst);
        if !matches!(op.opcode, Opcode::Store { .. }) {
            continue;
        }
        let Some(source) = _operand(unit, op.operands[0], &values) else { continue };
        let Some(reference) = MemRef::of(unit, inst) else { continue };
        let targets = _resolved_reference(unit, &reference, &values)
            .into_iter()
            .flat_map(|provenance| provenance.slices.into_iter().map(|one| one.object))
            .collect::<BTreeSet<_>>();
        for target in targets {
            pointer_fields.entry(target).or_default().extend(source.slices.iter().map(|one| one.object));
        }
    }

    // Objects numbered once, so closures and unions compare indices instead
    // of identity trees.
    let objects = RefCell::new(IndexSet::<ObjectRef>::default());
    let number = |object: &ObjectRef| {
        let mut objects = objects.borrow_mut();
        match objects.get_index_of(object) {
            Some(index) => index,
            None => objects.insert_full(*object).0,
        }
    };
    let pointer_fields = pointer_fields
        .iter()
        .map(|(target, sources)| (number(target), sources.iter().map(number).collect::<Vec<_>>()))
        .collect::<HashMap<_, _>>();
    // Close publication through pointer-valued fields of known objects: what
    // an object reaches is found once, from the fields of every object a store
    // put a pointer into.
    let closures: RefCell<HashMap<usize, Rc<Vec<usize>>>> = RefCell::new(HashMap::default());
    let closure = |object: usize| -> Rc<Vec<usize>> {
        if let Some(found) = closures.borrow().get(&object) {
            return Rc::clone(found);
        }
        let mut reached = HashSet::<usize>::default();
        let mut work = vec![object];
        while let Some(at) = work.pop() {
            if reached.insert(at) {
                work.extend(pointer_fields.get(&at).into_iter().flatten().copied());
            }
        }
        let found = Rc::new(reached.into_iter().collect::<Vec<_>>());
        closures.borrow_mut().insert(object, Rc::clone(&found));
        found
    };
    let pointees = |objects: BTreeSet<ObjectRef>| {
        let mut reached = HashSet::<usize>::default();
        for object in objects.iter().map(number) {
            reached.extend(closure(object).iter().copied());
        }
        reached.into_iter().collect::<Vec<_>>()
    };

    // Objects named by the initializers of the globals among `reached`, closed:
    // memory a callee reads a pointer out of holds them without any store.
    let initialized = |mut reached: Vec<usize>| {
        let mut at = 0;
        while at < reached.len() {
            let object = objects.borrow()[reached[at]].clone();
            at += 1;
            let (MemoryKind::Global, Key::Global(global)) = (object.kind, object.key) else { continue };
            let Some(llrm_mir::module::GlobalKind::Variable(variable)) =
                unit.globals.get(global as usize).map(|one| &one.kind)
            else {
                continue;
            };
            let mut held = BTreeSet::new();
            variable.initializer.iter().for_each(|&one| globalsaa::embedded(unit.context, one, &mut held));
            for one in held.into_iter().filter_map(|one| memory::global_object(unit, one)) {
                let found = number(&one);
                if !reached.contains(&found) {
                    reached.push(found);
                }
            }
        }
        reached
    };

    // What each instruction publishes does not depend on what reached it,
    // so it is found once; only the unions along edges iterate: the
    // gen/kill form of a forward dataflow.
    let calls = call_sites(unit).into_iter().collect::<BTreeSet<_>>();
    let provenances = |operands: &[Operand], values: &IndexMap<ValueId, Provenance>| {
        operands
            .iter()
            .filter_map(|&one| _operand(unit, one, values))
            .flat_map(|one| one.slices.into_iter().map(|slice| slice.object))
            .collect::<Vec<_>>()
    };
    let mut publishes: IndexMap<i64, HashMap<InstId, Vec<usize>>> = IndexMap::default();
    let mut during: IndexMap<InstId, Vec<usize>> = IndexMap::default();
    for block in &graph {
        // What a block holds of its cells on entry is not carried between
        // blocks: what a cell held anywhere is `pointer_fields`'.
        let mut mine: HashMap<InstId, Vec<usize>> = HashMap::default();
        for inst in instructions(block.at) {
            let op = function.instruction(inst);
            // What publishes an object: a call, a return, a store of a pointer,
            // an integer made of a pointer, a pointer an
            // instruction takes and does not carry on, or a
            // constant that names a global. The rest holds the
            // state it had (`_lost` says which carry: loads, moves and joins).
            let publishes_something = calls.contains(&inst) || could_publish(unit, op);
            if !publishes_something {
                continue;
            }
            let mut newly = BTreeSet::new();
            // What a call reads a pointer out of, it may keep.
            let mut lent = BTreeSet::new();
            // What it reads a pointer out of but keeps none of: reachable
            // during the call, so it may read and write it, but
            // escapes no further.
            let mut passing = BTreeSet::new();
            if calls.contains(&inst) {
                if let Some(arguments) = arguments {
                    let actual = _resolved_actuals(arguments.get(&inst).map_or(&[][..], Vec::as_slice), &values);
                    let kept = |index: usize| -> Result<bool, String> {
                        match captures.and_then(|captures| captures.get(&inst)).and_then(Option::as_ref) {
                            None => Ok(!borrowed(unit, inst, index)),
                            Some(selected) => Ok(selected
                                .iter()
                                .map(_index)
                                .collect::<Result<Vec<_>, _>>()?
                                .contains(&(index as i64))),
                        }
                    };
                    // Nor can it read a pointer out of what it may only write.
                    let through = _allowed(unit, inst).through;
                    let reads = |index: usize| through.get(index).is_none_or(|one| one.reads);
                    for (index, one) in actual.iter().enumerate() {
                        let objects = one.slices.iter().map(|one| one.object);
                        if kept(index)? {
                            newly.extend(objects)
                        } else if reads(index) {
                            if llrm_mir::memory::noretain(unit.context, unit.globals, unit.function, inst, index) {
                                passing.extend(objects)
                            } else {
                                lent.extend(objects)
                            }
                        }
                    }
                } else {
                    // The callee's own address is no argument.
                    let count = op.operands.len() - if matches!(op.opcode, Opcode::Invoke(_)) { 3 } else { 1 };
                    let kept = (0..count)
                        .filter(|&index| !borrowed(unit, inst, index))
                        .map(|index| op.operands[index])
                        .collect::<Vec<_>>();
                    newly.extend(provenances(&kept, &values));
                }
            }
            newly.extend(_lost(unit, inst, &values));
            // Returned, or turned into an integer something reads: found from
            // outside.
            if op.opcode == Opcode::Ret
                || (op.opcode == Opcode::Cast(CastOp::PtrToInt)
                    && op.result.is_none_or(|result| _read(function, result, &mut BTreeSet::new())))
            {
                newly.extend(provenances(&op.operands, &values));
            }
            // A store publishes the pointer it stores, where it stores outside
            // the frame: one of no pointer fact asks nothing of
            // where it goes.
            if matches!(op.opcode, Opcode::Store { .. }) && _operand(unit, op.operands[0], &values).is_some() {
                if let Some(reference) = MemRef::of(unit, inst) {
                    let destination = _resolved_reference(unit, &reference, &values);
                    let outside = destination
                        .as_ref()
                        .is_none_or(
                            |provenance| provenance.slices.iter().any(|one| one.object.kind != MemoryKind::Frame),
                        );
                    if outside {
                        newly.extend(provenances(&op.operands[..1], &values));
                    }
                }
            }
            let lent_numbers = lent.iter().map(number).collect::<HashSet<_>>();
            let mut published = pointees(newly);
            published.extend(pointees(lent).into_iter().filter(|one| !lent_numbers.contains(one)));
            if !published.is_empty() {
                mine.insert(inst, published);
            }
            if calls.contains(&inst) {
                during.insert(inst, initialized(pointees(passing)));
            }
        }
        publishes.insert(block.at, mine);
    }
    let objects = objects.into_inner();
    let width = objects.len();
    // What escapes anywhere is the union of what any instruction publishes. The
    // flow-sensitive part (what reaches each block, and the instructions asked
    // of) is found when first asked.
    let mut every = Bits::new(width);
    let mut order: Vec<Vec<Step>> = Vec::with_capacity(graph.len());
    for block in &graph {
        let mut steps = Vec::new();
        for inst in instructions(block.at) {
            let publishes = publishes[&block.at]
                .get(&inst)
                .map(
                    |objects| {
                        objects.iter().for_each(|one| every.insert(*one));
                        let mut sorted = objects.clone();
                        sorted.sort_unstable();
                        sorted
                    },
                );
            let asked = calls.contains(&inst)
                || matches!(
                    function.instruction(inst).opcode,
                    Opcode::Load { .. } | Opcode::Store { .. }
                );
            if publishes.is_some() || asked {
                steps.push(Step { inst, publishes, asked });
            }
        }
        order.push(steps);
    }
    let named = |bits: &Bits| bits.iter().map(|at| objects[at].clone()).collect::<BTreeSet<_>>();
    let escaped = named(&every);
    let inputs = Inputs { blocks: graph.iter().map(|block| (block.at, block.succ.to_vec())).collect(), order };
    let escaped_before = EscapedBefore {
        objects: Rc::new(objects),
        inputs: Rc::new(inputs),
        built: Rc::default(),
        during,
        found: Answers::default(),
    };
    Ok(PointsTo { values: values.clone(), escaped, escaped_before })
}

/// Whether anything that stays reads `value`: a use that is no pure operation,
/// or a pure one whose own result is read. The dead-code fact, without the
/// callee summaries a call would need, so a call reads.
fn _read(
    function: &llrm_mir::module::Function,
    value: ValueId,
    seen: &mut BTreeSet<ValueId>,
) -> bool {
    if !seen.insert(value) {
        return false;
    }
    function
        .users(value)
        .iter()
        .any(
            |one| {
                let user = function.instruction(one.user);
                !llrm_mir::memory::pure_operation(&user.opcode)
                    || user.result.is_some_and(|result| _read(function, result, seen))
            },
        )
}

/// Whether `op` may publish an object: what `_lost` and the cases of `escapes`
/// can find in it, told without asking for facts.
fn could_publish(
    unit: &Unit,
    op: &llrm_mir::module::Instruction,
) -> bool {
    let carried = match &op.opcode {
        Opcode::ICmp(_) => return false,
        Opcode::Ret | Opcode::Cast(CastOp::PtrToInt) => return true,
        Opcode::Store { .. } => return is_pointer(unit, op.operands[0]) || names_a_global(unit, op),
        Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) => !unit.spaces().is_segment(unit.space(op.operands[0])),
        Opcode::Phi | Opcode::Select => false,
        Opcode::Load { .. } | Opcode::GetElementPtr { .. } | Opcode::Call(_) | Opcode::Invoke(_) => true,
        _ => false,
    };
    (!carried && op.operands.iter().any(|&operand| is_pointer(unit, operand))) || names_a_global(unit, op)
}

/// Whether a constant operand of `op` that is no pointer holds a global's
/// address.
fn names_a_global(
    unit: &Unit,
    op: &llrm_mir::module::Instruction,
) -> bool {
    op.operands
        .iter()
        .any(
            |&operand| match operand {
                Operand::Constant(id) => {
                    !is_pointer(unit, operand)
                        && !matches!(
                            unit.context.get(id).kind,
                            ConstantKind::Int(_) | ConstantKind::Float(_)
                        )
                }
                _ => false,
            },
        )
}

/// Objects whose address `inst` turns into what no pointer fact follows,
/// which LLVM's capture tracking counts a capture: a pointer operand whose
/// provenance neither the result nor the access carries, and a global in
/// a constant that is no pointer. A comparison captures nothing; what a
/// call, a return, a store and a `ptrtoint` publish, their own rules say.
fn _lost(
    unit: &Unit,
    inst: InstId,
    values: &IndexMap<ValueId, Provenance>,
) -> Vec<ObjectRef> {
    let op = unit.function.instruction(inst);
    // A join with an arm no fact follows is itself no fact (`UNKNOWN`): the
    // objects of its other arms are no longer followed (a load through
    // `phi [%slot, %b0], [%q, %b1]` reads the slot).
    let joins_unknown = matches!(op.opcode, Opcode::Phi | Opcode::Select)
        && joined(op)
            .is_some_and(|arms| arms.iter().any(|&arm| is_pointer(unit, arm) && _operand(unit, arm, values).is_none()));
    let carried = match &op.opcode {
        Opcode::ICmp(_) => return Vec::new(),
        Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) => !unit.spaces().is_segment(unit.space(op.operands[0])),
        Opcode::Phi | Opcode::Select => !joins_unknown,
        Opcode::Load { .. } | Opcode::Store { .. } | Opcode::GetElementPtr { .. } => true,
        Opcode::Call(_) | Opcode::Invoke(_) | Opcode::Ret | Opcode::Cast(CastOp::PtrToInt) => true,
        _ => false,
    };
    let mut out = Vec::new();
    for &operand in &op.operands {
        if is_pointer(unit, operand) {
            if !carried {
                out.extend(
                    _operand(unit, operand, values)
                        .into_iter()
                        .flat_map(|one| one.slices.into_iter().map(|slice| slice.object)),
                );
            }
        } else if let Operand::Constant(id) = operand {
            let mut held = BTreeSet::new();
            globalsaa::embedded(unit.context, id, &mut held);
            out.extend(held.into_iter().filter_map(|one| memory::global_object(unit, one)));
        }
    }
    out
}

fn _resolved_actuals(
    actuals: &[Actual],
    values: &IndexMap<ValueId, Provenance>,
) -> Vec<Provenance> {
    actuals
        .iter()
        .map(|actual| match actual {
            Actual::Provenance(provenance) => provenance.clone(),
            Actual::Pointer(value, displacement) => values.get(value).unwrap_or(&UNKNOWN).shifted(*displacement),
            Actual::Absent => EMPTY.clone(),
        })
        .collect()
}

/// `value` modulo `modulus`, never negative for a positive modulus.
fn mod_floor(
    value: &BigInt,
    modulus: &BigInt,
) -> BigInt {
    ((value % modulus) + modulus) % modulus
}

/// Known `value == residue (mod modulus)` facts; modulus zero is exact.
///
/// A value wraps at its width, so a modulus holds only where it divides
/// the width's: each is cut to that divisor, and an exact residue masked.
pub fn congruences(unit: &Unit) -> Congruences {
    congruences_of(unit, &unit.registers(), None, None)
}

/// Each value's (residue, modulus): what it is congruent to.
pub type Congruences = llrm_mir::dense::IdMap<ValueId, (BigInt, BigInt)>;

thread_local! {
    static CONGRUENCES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has found a body's congruences, for a test that
/// the proofs of one body share them.
pub fn congruences_found() -> usize {
    CONGRUENCES.with(std::cell::Cell::get)
}

/// What the congruences of one body asked of by value have found: the answers,
/// the body's order and each loop's counters, kept for the asks of one version
/// of the body under one set of facts (an ask under others starts it again).
#[derive(Default)]
pub struct CongruenceMemo {
    order: Option<llrm_mir::dense::IdMap<InstId, usize>>,
    counters: Vec<Option<Rc<Vec<induction::Affine>>>>,
    answers: llrm_mir::dense::IdMap<ValueId, Option<(BigInt, BigInt)>>,
    scratch: Congruences,
    inserted: Vec<ValueId>,
}

impl CongruenceMemo {
    /// The map `congruences_of` made of the last ask by value, given back: it
    /// is the size of the body's last value, and one made for each ask costs as
    /// much as the body.
    pub fn recycle(
        &mut self,
        mut found: Congruences,
    ) {
        for value in self.inserted.drain(..) {
            found.remove(&value);
        }
        self.scratch = found;
    }
}

/// Known `value == residue (mod modulus)` facts of the values `wanted` alone,
/// or of every value where none, given what `consts::known` finds without
/// memory: what they are congruent to is what the additions, subtractions,
/// products and shifts they are computed from, and the loops' counters among
/// them, give. The values nothing else reaches are not worked out (the whole
/// body's, once for each version of a body of 128 loops, was 7.2 M and 3.9% of
/// the compile).
///
/// The answer for each value asked is the one asked of every value gives: the
/// values a value is computed from are visited in the body's order, as it
/// does, so that one visited before its operand is given the operand's
/// weaker answer in both.
pub fn congruences_of(
    unit: &Unit,
    constants: &IndexMap<ValueId, Known>,
    wanted: Option<&[ValueId]>,
    memo: Option<&std::cell::RefCell<CongruenceMemo>>,
) -> Congruences {
    let mut inserted: Vec<ValueId> = Vec::new();
    let mut result = match memo.filter(|_| wanted.is_some()) {
        Some(memo) => std::mem::take(&mut memo.borrow_mut().scratch),
        None => Congruences::default(),
    };
    if wanted.is_none() {
        CONGRUENCES.with(|count| count.set(count.get() + 1));
    }
    let function = unit.function;
    let qualifies = |value: ValueId, op: &llrm_mir::module::Instruction| {
        matches!(
            op.opcode,
            Opcode::Binary(BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl)
        )
            && op.operands.len() == 2
            && unit.int_bits(Operand::Value(value)).is_some()
    };
    let memo = memo.filter(|_| wanted.is_some());
    // Held answers are for the unit's own registers alone: a map made for one
    // ask and dropped can be followed at its address by another. The body's
    // order and its loops' counters hold under any facts.
    let answers = memo.filter(|_| unit.registers.is_some_and(|own| std::ptr::eq(constants, own)));
    // The values reached by the operations that make a congruence, and the ones
    // they stop at: for the whole body, every operation, in the body's order.
    let mut reached: llrm_mir::dense::IdSet<ValueId> = llrm_mir::dense::IdSet::new();
    let mut operations: Vec<InstId> = Vec::new();
    match wanted {
        None => {
            operations.extend(function.walk().map(|(_, inst)| inst).filter(|&inst| {
                let op = function.instruction(inst);
                op.result.is_some_and(|value| qualifies(value, op))
            }));
        }
        Some(wanted) => {
            let mut work: Vec<ValueId> = wanted.to_vec();
            while let Some(value) = work.pop() {
                if !reached.insert(value) {
                    continue;
                }
                // What is found of a value is found of what it is made of
                // alone.
                if let Some(held) = answers.and_then(|memo| memo.borrow().answers.get(&value).cloned()) {
                    if let Some(fact) = held {
                        inserted.push(value);
                        result.insert(value, fact);
                    }
                    continue;
                }
                let llrm_mir::module::ValueDef::Instruction(inst) = function.value(value).def else { continue };
                let op = function.instruction(inst);
                if qualifies(value, op) {
                    operations.push(inst);
                    work.extend(
                        op.operands
                            .iter()
                            .filter_map(|one| if let Operand::Value(source) = one { Some(*source) } else { None }),
                    );
                }
            }
            // In the body's order, round after round, as the whole body's goes.
            let position = |inst: &InstId| -> usize {
                let order = || -> llrm_mir::dense::IdMap<InstId, usize> {
                    function.walk().enumerate().map(|(at, (_, inst))| (inst, at)).collect()
                };
                match memo {
                    Some(memo) => {
                        let mut held = memo.borrow_mut();
                        held.order.get_or_insert_with(order)[inst]
                    }
                    None => order()[inst],
                }
            };
            if memo.is_some() {
                operations.sort_by_key(position);
            } else {
                let order: llrm_mir::dense::IdMap<InstId, usize> =
                    function.walk().enumerate().map(|(at, (_, inst))| (inst, at)).collect();
                operations.sort_by_key(|inst| order[inst]);
            }
        }
    }
    let zero = BigInt::from(0);
    // The loops' counters among the values reached.
    let shape = unit.shape();
    let block_of = |value: ValueId| match function.value(value).def {
        llrm_mir::module::ValueDef::Instruction(inst) => function.parent(inst).map(cfg::id),
        _ => None,
    };
    for (at, loop_) in shape.loops.iter().enumerate() {
        if wanted.is_some()
            && !reached.iter().any(|value| block_of(value).is_some_and(|block| loop_.body.contains(&block)))
        {
            continue;
        }
        let counters = match memo {
            Some(memo) => {
                let held = memo.borrow().counters.get(at).cloned().flatten();
                held.unwrap_or_else(|| {
                    let made = Rc::new(induction::basics(unit, loop_).into_values().collect::<Vec<_>>());
                    let mut held = memo.borrow_mut();
                    if held.counters.len() <= at {
                        held.counters.resize(at + 1, None);
                    }
                    held.counters[at] = Some(Rc::clone(&made));
                    made
                })
            }
            None => Rc::new(induction::basics(unit, loop_).into_values().collect::<Vec<_>>()),
        };
        for affine in counters.iter() {
            if wanted.is_some() && !reached.contains(&affine.value) {
                continue;
            }
            // A held answer is the last word.
            if answers.is_some_and(|memo| memo.borrow().answers.contains_key(&affine.value)) {
                continue;
            }
            let width = affine.start.width();
            let (Some(start), Some(step)) = (
                induction::_signed(&affine.start, constants, width),
                induction::_signed(&affine.step, constants, width),
            ) else {
                continue;
            };
            if step != zero {
                inserted.push(affine.value);
                result.insert(affine.value, reduced(step, start, width));
            }
        }
    }
    loop {
        let mut changed = false;
        for &inst in &operations {
            let op = function.instruction(inst);
            let Some(value) = op.result.filter(|value| !result.contains_key(value)) else { continue };
            let (Opcode::Binary(kind), [left, right], Some(width)) =
                (&op.opcode, op.operands.as_slice(), unit.int_bits(Operand::Value(value)))
            else {
                continue;
            };
            let fact = |one: Operand| match one {
                Operand::Value(source) => Some(
                    result
                        .get(&source)
                        .cloned()
                        .or_else(|| constants.get(&source).map(|known| (BigInt::from(0), known.n.clone())))
                        .unwrap_or_else(|| (BigInt::from(1), BigInt::from(0))),
                ),
                _ => unit.int_constant(one).map(|n| (BigInt::from(0), BigInt::from(n))),
            };
            let (Some(mut a), Some(mut b)) = (fact(*left), fact(*right)) else { continue };
            let found = match kind {
                BinaryOp::Add | BinaryOp::Sub => {
                    let residue = if *kind == BinaryOp::Add { a.1 + b.1 } else { a.1 - b.1 };
                    reduced(induction::gcd(a.0, b.0), residue, width)
                }
                BinaryOp::Mul => {
                    if a.0 == zero {
                        (a, b) = (b, a);
                    }
                    if b.0 != zero {
                        continue;
                    }
                    reduced(&a.0 * &b.1, a.1 * b.1, width)
                }
                BinaryOp::Shl if b.0 == zero && zero <= b.1 && b.1 < BigInt::from(width) => {
                    let factor = BigInt::from(1) << usize::try_from(&b.1).expect("a count below the width");
                    reduced(&a.0 * &factor, a.1 * factor, width)
                }
                _ => continue,
            };
            if found.0 == BigInt::from(1) {
                continue;
            }
            inserted.push(value);
            result.insert(value, found);
            changed = true;
        }
        if !changed {
            if let Some(memo) = answers {
                let mut held = memo.borrow_mut();
                for value in reached.iter() {
                    held.answers.insert(value, result.get(&value).cloned());
                }
            }
            if let Some(memo) = memo.filter(|_| wanted.is_some()) {
                memo.borrow_mut().inserted = inserted;
            }
            return result;
        }
    }
}

/// `residue (mod modulus)` of a `width`-bit value: the modulus cut to its
/// greatest divisor of 2**width, 0 where that is the whole width's.
fn reduced(
    modulus: BigInt,
    residue: BigInt,
    width: u32,
) -> (BigInt, BigInt) {
    let whole = BigInt::from(1) << width;
    let modulus = induction::gcd(if modulus < BigInt::from(0) { -modulus } else { modulus }, whole.clone());
    if modulus == whole {
        (BigInt::from(0), mod_floor(&residue, &whole))
    } else {
        (modulus.clone(), mod_floor(&residue, &modulus))
    }
}

/// Attach solved provenance to every access of the function: each load's
/// and store's, narrowed where a range bounds its index.
pub fn annotated(unit: &Unit) -> Result<IndexMap<InstId, MemRef>, String> {
    annotated_with(unit, &*unit.pointers()?, &unit.registers())
}

/// `annotated`, given the points-to facts and what `consts::known` finds
/// without memory.
///
/// DEVIATE AN1 (owner funcattrs-port, review 2026-11-13): Annotated keeps the
/// MemRef path (Accesses/PlainAccesses consume its slices); exit: they ask
/// BasicAA per query and Annotated is deleted.
///
/// DEVIATE AN2 (owner funcattrs-port, review 2026-11-13): an index's range is
/// `computeConstantRange` at the access, with the dominator tree and no
/// assumption cache; BasicAA's `analyzeVariableOffsets` passes both. It asks no
/// SCEV and no LVI range of an index, so a loop counter's interval is its
/// ValueTracking range, as at the pin; a counted loop's span or a dominating
/// guard is not the pin's answer. Exit: the
/// assumption cache is passed (MemorySSA S3, where BasicAA asks per query).
/// The range `analyzeVariableOffsets` finds of a variable index:
/// `computeConstantRange` at `at` intersected with `fromKnownBits(Known,
/// IsSigned)`, read as the signed interval of `value`; none where it is the
/// whole type.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BasicAliasAnalysis.cpp#L1973>
fn signed_range_at(
    unit: &Unit,
    tree: Option<&llrm_mir::domtree::DomTree>,
    at: InstId,
    value: ValueId,
) -> Option<Interval> {
    ranges::value_tracking_at(unit, tree, None, at, Operand::Value(value))
}

pub fn annotated_with(
    unit: &Unit,
    facts: &impl Resolves,
    known: &IndexMap<ValueId, Known>,
) -> Result<IndexMap<InstId, MemRef>, String> {
    // Only an access with a variable index asks: 4,185 lookups in 123 solves.
    let strides = std::cell::LazyCell::new(|| congruences_of(unit, known, None, None));
    let constants = ranges::intervals(known);
    let tree = unit.shape.map(|shape| shape.dominance.tree(unit.function));

    let tag = |reference: &MemRef, at: InstId| -> Result<MemRef, String> {
        let mut got = facts.reference(unit, reference);
        let interval = reference
            .base
            .and_then(|base| signed_range_at(unit, tree, at, base).or_else(|| constants.get(&base).cloned()));
        if let (Some(current), true, Some(base), Some(interval)) = (&got, reference.object, reference.base, interval) {
            if interval.width == reference.base_width && current.slices.len() == 1 && reference.scale > 0 {
                let source = current.slices.first().expect("one slice");
                let (modulus, residue) =
                    strides.get(&base).cloned().unwrap_or((BigInt::from(1_u8), BigInt::from(0_u8)));
                let modulus = if modulus > BigInt::from(1_u8) { modulus } else { BigInt::from(1_u8) };
                let first = &interval.low + mod_floor(&(residue - &interval.low), &modulus);
                let width = i64::from(reference.width.max(1));
                let low = BigInt::from(reference.disp) + first * reference.scale;
                let high = BigInt::from(reference.disp) + &interval.high * reference.scale + 1;
                let end = &high + width - 1;
                let stride = modulus * reference.scale;
                let zero = BigInt::from(0_u8);
                if low < high && source.object.extent.is_none_or(|extent| zero <= low && end <= BigInt::from(extent)) {
                    // A bound past the i64 slice model (an index spanning 2^63
                    // elements of a run-time sized object)
                    // is left as the object's whole extent.
                    if let (Ok(low), Ok(high), Ok(stride)) =
                        (i64::try_from(&low), i64::try_from(&high), i64::try_from(&stride))
                    {
                        got = Some(Provenance {
                            slices: crate::memory::Sorted::from([Slice::new(
                                source.object.clone(),
                                low,
                                high,
                                stride,
                                width,
                            )
                            .expect("a nonempty positive-stride slice is valid")]),
                            restrict: current.restrict.clone(),
                        });
                    }
                }
            }
        }
        // A far access whose selector's range lands it in foreign memory
        // names those linear bytes, whatever its pointer's provenance.
        if let (None, Some(Operand::Value(segment))) = (reference.selector, reference.segment) {
            let lookup = |value: ValueId| {
                signed_range_at(unit, tree, at, value)
                    .or_else(|| constants.get(&value).cloned())
                    .map(|one| (value, one))
            };
            let known = [Some(segment), reference.base].into_iter().flatten().filter_map(lookup).collect();
            if let Some(foreign) = regions::foreign_provenance(reference, &known, unit.program) {
                got = Some(foreign);
            }
        }
        // A flat access to a constant address the platform says is outside the
        // program names those linear bytes.
        if reference.linear {
            if let Some(foreign) = regions::foreign_provenance(reference, &BTreeMap::new(), unit.program) {
                got = Some(foreign);
            }
        }
        // An access the language says is at a fixed address names linear
        // memory, whatever its pointer was made from.
        // The frontend states it only where the target says the address is
        // outside the program; where the selector is a constant here,
        // the target is asked again.
        if unit.spaces().is_fixed(reference.space)
            && (reference.selector.is_none()
                || regions::foreign_provenance(reference, &BTreeMap::new(), unit.program).is_some())
        {
            got = Some(regions::fixed_provenance());
        }
        Ok(MemRef { provenance: got, ..reference.clone() })
    };

    let mut out =
        IndexMap::with_capacity_and_hasher(crate::memory::loads_and_stores(unit.function), Default::default());
    for (_, inst) in unit.function.walk() {
        if !matches!(
            unit.function.instruction(inst).opcode,
            Opcode::Load { .. } | Opcode::Store { .. } | Opcode::Call(_) | Opcode::Invoke(_)
        ) {
            continue;
        }
        if let Some(reference) = MemRef::of(unit, inst).or_else(|| MemRef::filled(unit, inst)) {
            out.insert(inst, tag(&reference, inst)?);
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "alias_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "callmemory_tests.rs"]
mod callmemory_tests;
