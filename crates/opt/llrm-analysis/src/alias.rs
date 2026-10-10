//! Strong, flow-sensitive alias analysis: llrm-core's `analysis/alias.rs`,
//! a port of `qbopt/analysis/alias.py`, adapted to the rich MIR. One
//! vocabulary for every frontend: canonical objects, subobject byte
//! slices, pointer provenance and restrict roots. Pointer facts flow
//! through SSA phis and through exact pointer spill slots. Unknown stores
//! kill spill facts; they never manufacture a disjointness proof.
//!
//! The objects a function starts from are its own (`memory::object_of`)
//! and its pointer parameters' (`Parameter`, a `noalias` one also a
//! restrict root): old `pointer_seeds`. Every pointer-typed value is a
//! pointer (old `pointer_values`). A `select` joins as a phi does. Integers
//! carry no provenance, so a `ptrtoint` publishes its pointer, as LLVM
//! counts it a capture; an `inttoptr` has no provenance.
//!
//! What a call reads and writes, `calls_annotated`, is a side table here:
//! the rich MIR's call carries no memory operands. Of the globals GlobalsAA
//! tracks, a callee no summary describes reaches what `globalsaa` says. A
//! body that may be replaced, weak or linkonce, is no summary.
//!
//! Not ported: `named_bytes` (a cell's root is its object), the merge of a
//! frontend's attached provenance with the derived one in
//! `_resolved_reference` (nothing attaches one), and `annotated`'s outgoing
//! stack excludes (no push area).
//!
//! Tests skipped:
//! `test_outgoing_argument_stack_does_not_kill_current_frame_values`
//! (no push area),
//! `test_pointer_fact_does_not_hide_a_conflicting_concrete_operand_object`
//! (no attached provenance),
//! `test_a_lane_form_slice_names_every_byte_it_covers` (`named_bytes`).
//! `test_unknown_call_reaches_nonlocals_and_only_its_pointer_actual`
//! drops its Python `repr` order, and
//! `test_interprocedural_modref_reaches_the_call_operation`
//! its `memory_complete` flag: a call's effect is a side table.

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
use num_bigint::BigInt;

use crate::cellmap::{Bucket, CellMap};
use crate::cfg;
use crate::consts::Known;
use crate::globalsaa;
use crate::graph::loops;
use crate::induction;
use crate::memory::{
    self, Addr, Identity, Key, MemRef, MemoryKind, MemoryObject, ObjectInterner, ObjectRef, Provenance, Slice, Unit,
    object_of, unmodeled_write, wrapped,
};
use crate::ranges;
use crate::regions::{self, ByteRange};

pub static UNKNOWN: LazyLock<Provenance> = LazyLock::new(|| Provenance::one(ObjectRef::UNKNOWN));
pub static NONLOCAL: LazyLock<Provenance> = LazyLock::new(|| Provenance::one(ObjectRef::NONLOCAL));
pub const EMPTY: Provenance = Provenance { slices: BTreeSet::new(), restrict: BTreeSet::new() };

/// One actual argument of a call: a provenance, a `(pointer value,
/// displacement)` pair, or nothing a pointer is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Actual {
    Provenance(Provenance),
    Pointer(ValueId, i64),
    Absent,
}

/// Python's `_cell_key` tuples: `(object, low, high)`, or a fixed address
/// and width.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CellKey {
    Object(ObjectRef, i64, i64),
    Address(Addr, i64),
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

/// Python `qbopt.analysis.alias:PointsTo`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointsTo {
    pub values: IndexMap<ValueId, Provenance>,
    pub escaped: BTreeSet<ObjectRef>,
    /// Objects visible immediately before each call.
    pub escaped_before: EscapedBefore,
}

/// Objects escaped before each call, load and store, kept as bits over one
/// numbering and named only when asked: most solves never read them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EscapedBefore {
    objects: Rc<IndexSet<ObjectRef>>,
    at: IndexMap<InstId, Bits>,
}

impl EscapedBefore {
    pub fn get(
        &self,
        at: &InstId,
    ) -> Option<BTreeSet<ObjectRef>> {
        self.at.get(at).map(|bits| bits.iter().map(|one| self.objects[one].clone()).collect())
    }

    /// What escaped before `at`, unnamed: two instructions with equal bits have
    /// the same objects.
    pub fn bits(
        &self,
        at: &InstId,
    ) -> Option<&Bits> {
        self.at.get(at)
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
        let Some(bits) = self.at.get(&at) else { return false };
        // No provenance is a pointer no fact follows: `_lost` publishes what
        // it came from.
        let escaping = reference.provenance.as_ref().is_none_or(|one| {
            one.slices.iter().all(|slice| matches!(slice.object.kind, MemoryKind::Unknown | MemoryKind::Nonlocal))
        });
        let unreached = |object: &ObjectRef| {
            object.kind == MemoryKind::Frame && self.objects.get_index_of(object).is_none_or(|one| !bits.contains(one))
        };
        escaping
            && cell
                .provenance
                .as_ref()
                .is_some_and(|one| !one.slices.is_empty() && one.slices.iter().all(|slice| unreached(&slice.object)))
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
/// Python `qbopt.analysis.alias:Summary`. `captures` holds PARAMETER
/// object identities.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Summary {
    pub reads: BTreeSet<Slice>,
    pub writes: BTreeSet<Slice>,
    pub captures: BTreeSet<Option<Identity>>,
    pub unknown_read: bool,
    pub unknown_write: bool,
    /// The `!tbaa` access types of the writes `unknown_write` stands for, where
    /// every one is a pointer no fact follows and has a type: a store of one
    /// type cannot land on a load of a type apart from it, however unplaced
    /// its pointer. None: some has none.
    pub unknown_write_types: Option<BTreeSet<Access>>,
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
        let within = |old_unknown: bool, new_unknown: bool, old: &BTreeSet<Slice>, new: &BTreeSet<Slice>| {
            old_unknown
                || new_unknown
                || new
                    .iter()
                    .filter(|one| one.object.kind != MemoryKind::Unknown && !unplaced(one))
                    .all(|one| old.contains(one))
        };
        within(self.unknown_read, fresh.unknown_read, &self.reads, &fresh.reads)
            && within(self.unknown_write, fresh.unknown_write, &self.writes, &fresh.writes)
    }

    pub fn instantiated(
        &self,
        arguments: &[Provenance],
    ) -> Summary {
        let expand = |items: &BTreeSet<Slice>| {
            let mut out = BTreeSet::new();
            for item in items {
                if item.object.kind != MemoryKind::Parameter {
                    out.insert(item.clone());
                    continue;
                }
                let Key::Int(index) = item.object.key else {
                    continue;
                };
                if !(0 <= index && index < arguments.len() as i64) {
                    continue;
                }
                for actual in &arguments[index as usize].slices {
                    out.insert(
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
            }
            out
        };

        Summary {
            reads: expand(&self.reads),
            writes: expand(&self.writes),
            captures: self.captures.clone(),
            unknown_read: self.unknown_read,
            unknown_write: self.unknown_write,
            unknown_write_types: self.unknown_write_types.clone(),
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

/// Python `qbopt.analysis.alias:Procedure`: a function, its calls by
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
                && let Some(name) = named.name.clone()
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
                            ConstantKind::Null | ConstantKind::Zero | ConstantKind::Poison
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
fn _borrowed(
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

/// What a callee nobody summarized may read and write at `at`, as far as
/// its attributes allow: through each actual, the objects it points to;
/// elsewhere, what a nonlocal reaches, what escaped before the call, and
/// of the tracked globals what GlobalsAA says it names and what
/// `callbacks` into the module do, all of them where that is unknown.
fn _unknown_visible(
    procedure: &Procedure,
    facts: &PointsTo,
    at: InstId,
    actual: &[Provenance],
    callbacks: Option<&Summary>,
) -> Result<(BTreeSet<Slice>, BTreeSet<Slice>), String> {
    let unit = &procedure.unit;
    let allowed = _allowed(unit, at);
    let (mut reads, mut writes) = _through_arguments(&allowed, actual);
    let (other_reads, other_writes) = _unknown_other(unit, facts, at, callbacks)?;
    if allowed.other.reads {
        reads.extend(other_reads);
    }
    if allowed.other.writes {
        writes.extend(other_writes);
    }
    Ok((reads, writes))
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

/// What an unsummarized callee may read and write at `at` other than
/// through its arguments.
fn _unknown_other(
    unit: &Unit,
    facts: &PointsTo,
    at: InstId,
    callbacks: Option<&Summary>,
) -> Result<(BTreeSet<Slice>, BTreeSet<Slice>), String> {
    let (mut reads, mut writes, back) = _unknown_base(unit, facts, at)?;
    if back {
        _calling_back(unit, callbacks, true, true, &mut reads, &mut writes);
    }
    Ok((reads, writes))
}

/// `_unknown_other` less what a call back into the module adds, which depends
/// on the callbacks and nothing else a revisit changes; and whether the call
/// may call back.
fn _unknown_base(
    unit: &Unit,
    facts: &PointsTo,
    at: InstId,
) -> Result<(BTreeSet<Slice>, BTreeSet<Slice>, bool), String> {
    OTHER_RUNS.with(|runs| runs.set(runs.get() + 1));
    let mut reads = NONLOCAL.slices.clone();
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

/// What a call back into the module adds to what a call reads and writes: the
/// callbacks' effects, and of the tracked globals all that the callbacks do not
/// state, or where none are known.
fn _calling_back(
    unit: &Unit,
    callbacks: Option<&Summary>,
    reading: bool,
    writing: bool,
    reads: &mut BTreeSet<Slice>,
    writes: &mut BTreeSet<Slice>,
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
        reads.extend(callbacks.reads.iter().cloned());
        if callbacks.unknown_read {
            reads.extend(_tracked(unit));
        }
    }
    if writing {
        writes.extend(callbacks.writes.iter().cloned());
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
/// the answer depends on, so the module holds it once for every body that asks
/// (`manager::Callbacks`).
pub fn callbacks_over(
    globals: &globalsaa::Globals,
    known: &IndexMap<String, Summary>,
) -> Option<Summary> {
    CalledBack::of(globals, known)?.summary()
}

thread_local! {
    static CALLBACK_ENTRIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many entries' summaries this thread has instantiated for callbacks, for
/// a test that the work does not grow with the entries times the bodies.
pub fn callback_entries() -> usize {
    CALLBACK_ENTRIES.with(std::cell::Cell::get)
}

/// The callbacks as the entries' summaries add up, kept apart so that an entry
/// whose summary changed is folded in alone: every change used to add the
/// whole of them up again, entries times changes.
struct CalledBack {
    parts: IndexMap<String, Summary>,
    out: Summary,
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
        let mut this = Self { parts, out: Summary::default() };
        this.add_up();
        Some(this)
    }

    fn add_up(&mut self) {
        let mut out = Summary::default();
        for one in self.parts.values() {
            out.reads.extend(one.reads.iter().cloned());
            out.writes.extend(one.writes.iter().cloned());
            out.unknown_read |= one.unknown_read;
            out.unknown_write |= one.unknown_write;
        }
        self.out = out;
    }

    fn summary(&self) -> Option<Summary> {
        Some(Summary { reads: _coalesced(&self.out.reads), writes: _coalesced(&self.out.writes), ..self.out.clone() })
    }

    /// Entry `name` is now `summary`: added in when it only grew, else all
    /// added up again.
    fn changed(
        &mut self,
        name: &str,
        summary: &Summary,
    ) {
        CALLBACK_ENTRIES.with(|entries| entries.set(entries.get() + 1));
        let now = summary.instantiated(&[]);
        let Some(then) = self.parts.get_mut(name) else { return };
        let grew = then.reads.is_subset(&now.reads)
            && then.writes.is_subset(&now.writes)
            && (!then.unknown_read || now.unknown_read)
            && (!then.unknown_write || now.unknown_write);
        *then = now;
        if grew {
            let then = &self.parts[name];
            self.out.reads.extend(then.reads.iter().cloned());
            self.out.writes.extend(then.writes.iter().cloned());
            self.out.unknown_read |= then.unknown_read;
            self.out.unknown_write |= then.unknown_write;
        } else {
            self.add_up();
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

/// How many times this thread has summarized a body against its callees'
/// summaries, for a test that a body in no cycle of calls is visited once.
pub fn visits() -> usize {
    VISITS.with(std::cell::Cell::get)
}

pub fn _direct_summary(unit: &Unit) -> Result<Summary, String> {
    DIRECT_RUNS.with(|runs| runs.set(runs.get() + 1));
    let (mut reads, mut writes) = (BTreeSet::new(), BTreeSet::new());
    let (mut unknown_read, mut unknown_write) = (false, false);
    let mut types = Some(BTreeSet::new());
    let facts = points_to(unit, None, None)?;
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
    Ok(Summary { reads, writes, captures, unknown_read, unknown_write, unknown_write_types: types })
}

fn _widen_parameters(summary: Summary) -> Summary {
    let widened = |items: &BTreeSet<Slice>| {
        items
            .iter()
            .map(|one| if one.object.kind == MemoryKind::Parameter { Slice::whole(one.object) } else { one.clone() })
            .collect()
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
    procedure: &Procedure,
    known: &'s IndexMap<String, Summary>,
    name: &str,
) -> Option<&'s Summary> {
    (!procedure.replaceable.contains(name)).then(|| known.get(name)).flatten()
}

/// Transitive per-procedure mod/ref and capture summaries to a fixed point.
///
/// `known` supplies established external semantics, such as C library
/// functions. A body in this compilation unit always takes precedence; one
/// that may be replaced describes no call.
pub fn summaries(
    procedures: &IndexMap<String, Procedure>,
    known: Option<&IndexMap<String, Summary>>,
) -> Result<IndexMap<String, Summary>, String> {
    summaries_updating(procedures, known, &mut SummaryMemo::default(), None)
}

/// What a run of `summaries` leaves for the next: the summaries, the bodies'
/// own (`direct`) and their last visits, and what they were made from.
#[derive(Default)]
pub struct SummaryMemo {
    result: IndexMap<String, Summary>,
    direct: IndexMap<String, Summary>,
    found: IndexMap<String, Visit>,
    known: Option<IndexMap<String, Summary>>,
    topology: Option<Rc<Topology>>,
    /// The bodies the topology was made of, in order, and their calls.
    made_of: Vec<(String, Rc<CallFacts>)>,
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
pub fn summaries_updating(
    procedures: &IndexMap<String, Procedure>,
    known: Option<&IndexMap<String, Summary>>,
    memo: &mut SummaryMemo,
    dirty: Option<&BTreeSet<String>>,
) -> Result<IndexMap<String, Summary>, String> {
    let same_bodies = memo.result.len() >= procedures.len()
        && procedures.keys().all(|name| memo.direct.contains_key(name) && memo.result.contains_key(name))
        && memo.direct.len() == procedures.len();
    let whole = dirty.is_none() || !same_bodies || memo.known.as_ref() != known;
    if whole {
        *memo = SummaryMemo::default();
    }
    let dirty_names: BTreeSet<&String> = match dirty {
        Some(dirty) if !whole => dirty.iter().collect(),
        _ => procedures.keys().collect(),
    };
    // What a body captures grows from nothing: a call captures what its
    // callee's summary says, so a least fixed point, as a recursive one
    // that captures nothing proves.
    // What a body does on its own does not change from round to round: made
    // once, and again only for a body that was edited.
    llrm_support::debug::timed("summaries direct", || {
        for (name, one) in procedures {
            if whole || dirty_names.contains(name) {
                memo.direct.insert(name.clone(), _direct_summary(&one.unit)?);
            }
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
    let Topology { component, order, rank, readers, callers_of_unknown, .. } = &*topology;
    let entries: Vec<usize> = procedures
        .values()
        .next()
        .and_then(|one| one.unit.globals_aa)
        .map(|found| found.entries().iter().filter_map(|name| procedures.get_index_of(name)).collect())
        .unwrap_or_default();
    // The bodies that read a call back into the module, and their callers
    // among them: when an entry is one of those, an entry's summary feeds
    // what it is made of, so they stand or fall together and are worked out
    // from nothing as one, after everything else has settled.
    let coupled = topology
        .coupled
        .get_or_init(
            || {
                let mut up: BTreeSet<usize> = callers_of_unknown.clone();
                let mut todo: Vec<usize> = up.iter().copied().collect();
                while let Some(at) = todo.pop() {
                    todo.extend(readers[at].iter().copied().filter(|reader| up.insert(*reader)));
                }
                if entries.iter().any(|one| up.contains(one)) { up.into_iter().collect() } else { Vec::new() }
            },
        );
    let mut in_coupled = vec![false; procedures.len()];
    for at in coupled {
        in_coupled[*at] = true;
    }
    // Whether the coupled bodies wait (the others are being settled).
    let mut waiting_coupled = !whole && !coupled.is_empty();
    let mut deferred = false;
    let mut reset = vec![whole; procedures.len()];
    // A body is worked out again from what it reads now, and its readers
    // when it came out other than it was: gcc's summaries stop where a
    // function's does not change. A cycle of calls is worked out from nothing,
    // as its least fixed point is.
    let mut queued = vec![false; procedures.len()];
    let mut work: std::collections::BinaryHeap<std::cmp::Reverse<usize>> = Default::default();
    macro_rules! wake {
        ($at:expr) => {{
            let at: usize = $at;
            if waiting_coupled && in_coupled[at] {
                deferred = true;
            } else if !queued[at] {
                queued[at] = true;
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
            if let Some(at) = procedures.get_index_of(*name) {
                wake!(at);
            }
        }
        // A callee defined elsewhere whose declaration was restated: its
        // callers read it.
        for (at, (_, one)) in procedures.iter().enumerate() {
            if one.calls.values().any(|target| dirty_names.contains(target) && !procedures.contains_key(target)) {
                wake!(at);
            }
        }
    }
    let start = |at: usize, result: &mut IndexMap<String, Summary>| {
        let (name, _) = procedures.get_index(at).expect("a member of the graph");
        result.insert(name.clone(), Summary { captures: BTreeSet::new(), ..direct[name].clone() });
    };
    let mut called_back = procedures
        .values()
        .next()
        .and_then(|one| one.unit.globals_aa)
        .and_then(|globals| CalledBack::of(globals, &result));
    let mut callbacks = called_back.as_ref().and_then(CalledBack::summary);
    // What each body's points-to facts were found from: they change only with
    // the callees' captures, not with the effects a revisit is for. A visit
    // of an earlier run says what its calls to something unknown did
    // for the callbacks it had: that is worked out again.
    // A body that was edited has other facts than its last visit found.
    // (Taken out of the memo one by one without moving the rest: it is made
    // again below.)
    let mut last = std::mem::take(&mut memo.found);
    let mut found: Vec<Option<Visit>> = procedures
        .keys()
        .map(|name| {
            last.swap_remove(name).filter(|_| !dirty_names.contains(name)).map(|visit| Visit { version: None, ..visit })
        })
        .collect();
    let mut version = 0;
    loop {
        let Some(std::cmp::Reverse(first)) = work.pop() else {
            if !(waiting_coupled && deferred) {
                break;
            }
            // Everything else is settled: the coupled bodies from nothing.
            waiting_coupled = false;
            for at in coupled {
                start(*at, &mut result);
                reset[*at] = true;
            }
            called_back = procedures
                .values()
                .next()
                .and_then(|one| one.unit.globals_aa)
                .and_then(|globals| CalledBack::of(globals, &result));
            callbacks = called_back.as_ref().and_then(CalledBack::summary);
            version += 1;
            for at in coupled {
                wake!(*at);
            }
            continue;
        };
        let at = order[first];
        queued[at] = false;
        if !reset[at] && component[at].1 {
            // The first of a cycle to be worked out: all of it, from nothing.
            let id = component[at];
            for member in (0..procedures.len()).filter(|member| component[*member] == id) {
                start(member, &mut result);
                reset[member] = true;
                wake!(member);
            }
            continue;
        }
        llrm_support::debug::counted("summaries rounds", true);
        let (name, procedure) = procedures.get_index(at).expect("a member of the graph");
        let made = llrm_support::debug::timed("summaries visit", || {
            _summarized(
                at,
                procedure,
                &direct[name],
                &result,
                (callbacks.as_ref(), version),
                &mut found[at],
                component,
                procedures,
            )
        })?;
        if made != result[name] {
            result.insert(name.clone(), made);
            let mut woken: Vec<usize> = readers[at].iter().copied().collect();
            if entries.contains(&at) {
                let now = llrm_support::debug::timed("summaries callbacks", || {
                    match called_back.as_mut() {
                        Some(held) => held.changed(name, &result[name]),
                        None => {
                            called_back = procedures
                                .values()
                                .next()
                                .and_then(|one| one.unit.globals_aa)
                                .and_then(|globals| CalledBack::of(globals, &result));
                        }
                    }
                    called_back.as_ref().and_then(CalledBack::summary)
                });
                if llrm_support::env_set("LLRM_CHECK_CALLBACKS") {
                    let fresh = procedures.values().next().and_then(|one| _callbacks(&one.unit, &result));
                    assert!(now == fresh, "the callbacks folded in are not what adding the entries up again gives");
                }
                if now != callbacks {
                    callbacks = now;
                    version += 1;
                    woken.extend(callers_of_unknown.iter().copied());
                }
            }
            for reader in woken {
                wake!(reader);
            }
        }
    }
    memo.found = procedures.keys().cloned().zip(found).filter_map(|(name, visit)| Some((name, visit?))).collect();
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
    coupled: std::cell::OnceCell<Vec<usize>>,
}

/// `procedures`' topology: the last run's where only the dirty bodies' calls
/// are as they were (the rest are the same facts), else made afresh.
fn _topology(
    procedures: &IndexMap<String, Procedure>,
    result: &IndexMap<String, Summary>,
    memo: &mut SummaryMemo,
    whole: bool,
    dirty_names: &BTreeSet<&String>,
) -> Rc<Topology> {
    if let Some(kept) = memo.topology.as_ref().filter(|_| !whole) {
        let same = memo.made_of.len() == procedures.len()
            && procedures.iter().zip(&memo.made_of).all(|((name, one), (then, calls))| {
                name == then
                    && (Rc::ptr_eq(&one.facts, calls)
                        || dirty_names.contains(name)
                            && one.facts.calls == calls.calls
                            && one.facts.sites == calls.sites
                            && one.facts.replaceable == calls.replaceable)
            });
        if same {
            let kept = Rc::clone(kept);
            // The dirty bodies' facts are the ones now held.
            for (one, (_, calls)) in procedures.values().zip(memo.made_of.iter_mut()) {
                if !Rc::ptr_eq(&one.facts, calls) {
                    *calls = Rc::clone(&one.facts);
                }
            }
            return kept;
        }
    }
    let out: Vec<Vec<usize>> = procedures
        .values()
        .map(|procedure| {
            let mut to: Vec<usize> =
                procedure.calls.values().filter_map(|target| procedures.get_index_of(target)).collect();
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
    for (at, (_, procedure)) in procedures.iter().enumerate() {
        for target in procedure.calls.values().filter_map(|target| procedures.get_index_of(target)) {
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
    memo.made_of = procedures.iter().map(|(name, one)| (name.clone(), Rc::clone(&one.facts))).collect();
    let made = Rc::new(Topology { component, order, rank, readers, callers_of_unknown, coupled: Default::default() });
    memo.topology = Some(Rc::clone(&made));
    made
}

/// What a body's last visit was made from, and found: its points-to facts, from
/// its callees' captures, and what its calls to something unknown may do, from
/// those facts and the callbacks of one `version`.
#[derive(Clone)]
struct Visit {
    captured: IndexMap<InstId, Option<BTreeSet<Option<Identity>>>>,
    facts: Rc<PointsTo>,
    version: Option<usize>,
    unknown: (BTreeSet<Slice>, BTreeSet<Slice>),
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
    result: &IndexMap<String, Summary>,
    (callbacks, version): (Option<&Summary>, usize),
    memo: &mut Option<Visit>,
    component: &[(usize, bool)],
    procedures: &IndexMap<String, Procedure>,
) -> Result<Summary, String> {
    VISITS.with(|visits| visits.set(visits.get() + 1));
    let captured_at = procedure
        .calls
        .iter()
        .map(|(at, target)| (*at, _summary(procedure, result, target).map(|one| one.captures.clone())))
        .collect::<IndexMap<_, _>>();
    if memo.as_ref().is_none_or(|one| one.captured != captured_at) {
        let facts = Rc::new(llrm_support::debug::timed("summaries points-to", || {
            points_to(&procedure.unit, Some(&procedure.arguments), Some(&captured_at))
        })?);
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
            Option<&Bits>,
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
        let (mut reads, mut writes, back_reads, back_writes) = visit.base.clone().expect("found above");
        _calling_back(&procedure.unit, callbacks, back_reads, back_writes, &mut reads, &mut writes);
        visit.unknown = (reads, writes);
        visit.version = Some(version);
    }
    let (mut reads, mut writes) = (direct.reads.clone(), direct.writes.clone());
    reads.extend(visit.unknown.0.iter().cloned());
    writes.extend(visit.unknown.1.iter().cloned());
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
        if procedures.get_index_of(target).is_some_and(|callee| component[me].1 && component[me] == component[callee]) {
            effect = _widen_parameters(effect);
        }
        reads.extend(effect.reads);
        writes.extend(effect.writes);
        unknown_read |= effect.unknown_read;
        types = merged_types((unknown_write, &types), (effect.unknown_write, &effect.unknown_write_types));
        unknown_write |= effect.unknown_write;
    }
    // A callee's frame is gone when it returns: what its calls touch
    // there, like its own accesses, is no effect of calling it.
    let (reads, writes) = (reads.into_iter().filter(outlives).collect(), writes.into_iter().filter(outlives).collect());
    Ok(Summary {
        reads: _coalesced(&reads),
        writes: _coalesced(&writes),
        captures,
        unknown_read,
        unknown_write,
        unknown_write_types: types,
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

/// Instantiate callee effects through actual pointer provenance: each
/// call's effect, as old `calls_annotated` wrote it into the call.
pub fn calls_annotated(
    procedure: &Procedure,
    known: &IndexMap<String, Summary>,
) -> Result<IndexMap<InstId, Effect>, String> {
    // Capture is part of escape flow. Unknown callees may retain every
    // pointer actual; known callees retain only the parameters their fixed
    // point summary says they capture.
    let callee = |at: &InstId| procedure.calls.get(at).and_then(|target| _summary(procedure, known, target));
    let captures =
        procedure.calls.keys().map(|at| (*at, callee(at).map(|one| one.captures.clone()))).collect::<IndexMap<_, _>>();
    let facts = points_to(&procedure.unit, Some(&procedure.arguments), Some(&captures))?;
    let computed;
    let callbacks = match procedure.unit.callbacks {
        Some(held) => {
            if llrm_support::env_set("LLRM_CHECK_CALLBACKS") {
                assert!(
                    *held == _callbacks(&procedure.unit, known),
                    "the held callbacks are not what adding the entries up gives"
                );
            }
            held.as_ref()
        }
        None => {
            computed = _callbacks(&procedure.unit, known);
            computed.as_ref()
        }
    };

    let reference = |one: &Slice| {
        MemRef::reach(
            u32::try_from(one.width).expect("a slice width is a memory width"),
            Provenance { slices: BTreeSet::from([one.clone()]), restrict: BTreeSet::new() },
        )
    };

    let mut out = IndexMap::default();
    // Calls reaching the same slices share one list of references.
    let mut made: HashMap<BTreeSet<Slice>, Rc<[MemRef]>> = HashMap::default();
    for at in procedure.sites.iter().copied() {
        let actual = _actuals(procedure, &facts, at);
        let mut effect = match callee(&at) {
            Some(callee) => {
                // A body does no more than its call states.
                let allowed = _allowed(&procedure.unit, at);
                let mut effect = callee.instantiated(&actual);
                if !(allowed.arguments.reads || allowed.other.reads) {
                    (effect.reads, effect.unknown_read) = (BTreeSet::new(), false);
                }
                if !(allowed.arguments.writes || allowed.other.writes) {
                    (effect.writes, effect.unknown_write) = (BTreeSet::new(), false);
                }
                effect
            }
            None => {
                let (reads, writes) = _unknown_visible(procedure, &facts, at, &actual, callbacks)?;
                Summary { reads, writes, ..Summary::default() }
            }
        };
        if effect.unknown_read {
            let visible = _whole(&actual, &facts.escaped_before.get(&at).unwrap_or_default());
            effect.reads.extend(if visible.is_empty() { UNKNOWN.slices.clone() } else { visible });
            effect.reads.extend(NONLOCAL.slices.clone());
            effect.reads.extend(_tracked(&procedure.unit));
        }
        // What an unplaced write may reach: each of its types' stores, not an
        // untyped one.
        let mut typed = Vec::new();
        if effect.unknown_write {
            let visible = _whole(&actual, &facts.escaped_before.get(&at).unwrap_or_default());
            let mut reached = if visible.is_empty() { UNKNOWN.slices.clone() } else { visible };
            reached.extend(NONLOCAL.slices.clone());
            match &effect.unknown_write_types {
                // A pointer no fact follows reaches only what escaped, never a
                // tracked global.
                Some(types) if !types.is_empty() => {
                    for (name, lineage) in types {
                        typed.extend(reached.iter().map(|one| MemRef {
                            typed: Some(name.clone()),
                            lineage: lineage.clone(),
                            ..reference(one)
                        }));
                    }
                }
                _ => {
                    effect.writes.extend(reached);
                    effect.writes.extend(_tracked(&procedure.unit));
                }
            }
        }
        let fills = _fills(&procedure.unit, &facts, at);
        let mut shared = |slices: BTreeSet<Slice>, typed: Vec<MemRef>| -> Rc<[MemRef]> {
            if !typed.is_empty() {
                return slices.iter().map(reference).chain(typed).collect();
            }
            Rc::clone(made.entry(slices).or_insert_with_key(|slices| slices.iter().map(reference).collect()))
        };
        let (loads, stores) = (shared(effect.reads, Vec::new()), shared(effect.writes, typed));
        out.insert(at, Effect { loads, stores, fills });
    }
    Ok(out)
}

/// The bytes the call `at` writes through an argument before it reads
/// any: `initializes` at the site or on the callee's parameter.
fn _fills(
    unit: &Unit,
    facts: &PointsTo,
    at: InstId,
) -> Vec<MemRef> {
    let op = unit.function.instruction(at);
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &op.opcode else { return Vec::new() };
    let declared = llrm_mir::memory::callee(unit.context, unit.function, at)
        .and_then(|one| unit.globals.get(one.0 as usize))
        .and_then(|one| one.function());
    let mut out = Vec::new();
    for (argument, &operand) in op.operands.iter().enumerate().take(info.argument_attrs.len()) {
        let declared = declared
            .and_then(|one| one.parameter_attrs.get(argument))
            .map_or(&[][..], |attrs| llrm_mir::memory::initializes(attrs));
        for &(low, high) in llrm_mir::memory::initializes(&info.argument_attrs[argument]).iter().chain(declared) {
            let Ok(width) = u32::try_from(high - low) else { continue };
            let mut one = MemRef::at(unit, operand, width);
            one.disp += low;
            let provenance = facts.reference(unit, &one);
            out.push(MemRef { provenance, ..one });
        }
    }
    out
}

/// Byte ranges, sorted, apart and not touching.
type Ranges = Vec<(i64, i64)>;

fn _with(
    ranges: &Ranges,
    low: i64,
    high: i64,
) -> Ranges {
    let (mut low, mut high, mut out) = (low, high, Vec::new());
    for &(one, other) in ranges {
        if other < low || high < one {
            out.push((one, other));
        } else {
            (low, high) = (low.min(one), high.max(other));
        }
    }
    out.push((low, high));
    out.sort_unstable();
    out
}

fn _without(
    ranges: &Ranges,
    low: i64,
    high: i64,
) -> Ranges {
    let mut out = Vec::new();
    for &(one, other) in ranges {
        if one < low.min(other) {
            out.push((one, low.min(other)));
        }
        if high.max(one) < other {
            out.push((high.max(one), other));
        }
    }
    out
}

fn _common(
    one: &Ranges,
    other: &Ranges,
) -> Ranges {
    let mut out = Vec::new();
    for &(low, high) in one {
        for &(from, to) in other {
            if low.max(from) < high.min(to) {
                out.push((low.max(from), high.min(to)));
            }
        }
    }
    out.sort_unstable();
    out
}

/// The parameter a slice lies in.
fn _parameter(one: &Slice) -> Option<usize> {
    match (one.object.kind, one.object.key) {
        (MemoryKind::Parameter, Key::Int(index)) => usize::try_from(index).ok(),
        _ => None,
    }
}

/// The bytes `slice` may cover, or `None` for all of them.
fn _bytes(one: &Slice) -> Option<(i64, i64)> {
    (one.low != memory::WHOLE_LOW && one.high != memory::WHOLE_HIGH).then(|| (one.low, one.high - 1 + one.width))
}

/// Each pointer parameter's bytes the body writes before anything reads
/// them, on every path that returns: LLVM's `initializes`, as its
/// FunctionAttrs infers it, backwards from each `ret`. A path that
/// unwinds or cannot go on asks nothing; a call reads what its effect
/// loads and writes what its `initializes` names.
pub fn initialized(
    procedure: &Procedure,
    known: &IndexMap<String, Summary>,
) -> Result<Vec<Ranges>, String> {
    let unit = &procedure.unit;
    let function = unit.function;
    let count = function.parameters().len();
    // Only a pointer parameter is an object a body can write before it reads.
    // Two points-to solves and the effects of every call were made for each
    // body of the module (45% of `mir interprocedural` over QCport at -O1),
    // most of them for bodies whose parameters are integers.
    // LLRM_CHECK_INITIALIZED works it out anyway and asserts nothing is
    // initialized. And only a store, or a call that initializes its
    // argument, writes one.
    let pointers = function
        .parameters()
        .iter()
        .any(|&one| matches!(
            unit.context.types.get(function.value(one).ty),
            llrm_mir::types::Type::Pointer(_)
        ))
        && function.walk().any(|(_, inst)| {
            matches!(
                function.instruction(inst).opcode,
                Opcode::Store { volatile: false, .. } | Opcode::Call(_) | Opcode::Invoke(_)
            )
        });
    if !pointers && !llrm_support::env_set("LLRM_CHECK_INITIALIZED") {
        return Ok(vec![Vec::new(); count]);
    }
    // The three solves below differ in the call arguments and captures they
    // ask of the escape phase alone: the value solve is made once.
    let values = match unit.point_values {
        Some(_) => None,
        None => Some(point_values(unit)?),
    };
    let held =
        Procedure { unit: values.as_ref().map_or(*unit, |values| unit.with_point_values(values)), ..procedure.clone() };
    let (procedure, unit) = (&held, &held.unit);
    let facts = points_to(unit, None, None)?;
    let effects = calls_annotated(procedure, known)?;
    let actuals = points_to(unit, Some(&procedure.arguments), None)?;
    // `None` is every byte: nothing asked yet.
    type State = Vec<Option<Ranges>>;
    let meet = |one: &State, other: &State| -> State {
        one.iter()
            .zip(other)
            .map(|(one, other)| match (one, other) {
                (None, any) | (any, None) => any.clone(),
                (Some(one), Some(other)) => Some(_common(one, other)),
            })
            .collect()
    };
    // A read of another object that may be the parameter's reads all of it.
    let interner = ObjectInterner::of(unit.context);
    let parameter_objects = (0..count)
        .map(|index| {
            interner.intern(MemoryObject {
                identity: Some(Identity::Int(index as i64)),
                ..MemoryObject::new(MemoryKind::Parameter)
            })
        })
        .collect::<Vec<_>>();
    let read = |state: &mut State, one: &Slice| {
        for (index, ranges) in state.iter_mut().enumerate() {
            let parameter = parameter_objects[index];
            let bytes = if one.object == parameter {
                _bytes(one)
            } else if memory::objects_may_alias(&one.object, &parameter) {
                None
            } else {
                continue;
            };
            let held = ranges.clone().unwrap_or_else(|| vec![(i64::MIN, i64::MAX)]);
            *ranges = Some(bytes.map_or_else(Vec::new, |(low, high)| _without(&held, low, high)));
        }
    };
    let written = |state: &mut State, index: usize, low: i64, high: i64| {
        if let Some(Some(held)) = state.get(index) {
            state[index] = Some(_with(held, low, high));
        }
    };
    let transfer = |at: i64, mut state: State| -> State {
        for &inst in function.block(cfg::block(at)).instructions().iter().rev() {
            let op = function.instruction(inst);
            if let Some(effect) = effects.get(&inst) {
                for one in
                    effect.loads.iter().filter_map(|one| one.provenance.as_ref()).flat_map(|one| one.slices.iter())
                {
                    read(&mut state, one);
                }
                let (Opcode::Call(info) | Opcode::Invoke(info)) = &op.opcode else { continue };
                let declared = llrm_mir::memory::callee(unit.context, function, inst)
                    .and_then(|one| unit.globals.get(one.0 as usize))
                    .and_then(|one| one.function());
                for (argument, operand) in op.operands.iter().enumerate().take(info.argument_attrs.len()) {
                    let mut ranges = llrm_mir::memory::initializes(&info.argument_attrs[argument]).to_vec();
                    ranges.extend(
                        declared
                            .and_then(|one| one.parameter_attrs.get(argument))
                            .map_or(&[][..], |attrs| llrm_mir::memory::initializes(attrs)),
                    );
                    let Some(pointer) = _operand(unit, *operand, &actuals.values) else { continue };
                    let [one] = pointer.slices.iter().collect::<Vec<_>>()[..] else { continue };
                    let (Some(index), true) = (_parameter(one), one.high == one.low + 1) else { continue };
                    for (low, high) in ranges {
                        written(&mut state, index, one.low + low, one.low + high);
                    }
                }
                continue;
            }
            let Some(reference) = MemRef::of(unit, inst) else { continue };
            let slices = facts.reference(unit, &reference).map_or_else(|| UNKNOWN.slices.clone(), |one| one.slices);
            match op.opcode {
                Opcode::Store { volatile: false, .. } => {
                    if let [one] = slices.iter().collect::<Vec<_>>()[..]
                        && let (Some(index), true) = (_parameter(one), one.high == one.low + 1)
                    {
                        written(&mut state, index, one.low, one.low + one.width);
                    }
                }
                Opcode::Store { .. } => {}
                _ => slices.iter().for_each(|one| read(&mut state, one)),
            }
        }
        state
    };
    let graph = cfg::graph(function);
    let exit = |at: i64| -> State {
        match function.instruction(*function.block(cfg::block(at)).instructions().last().expect("a block ends")).opcode
        {
            Opcode::Ret => vec![Some(Vec::new()); count],
            _ => vec![None; count],
        }
    };
    let mut entry = graph.iter().map(|block| (block.at, vec![None; count])).collect::<IndexMap<i64, State>>();
    loop {
        let mut changed = false;
        for block in graph.iter().rev() {
            let out = block.succ.iter().fold(exit(block.at), |state, successor| meet(&state, &entry[successor]));
            let made = transfer(block.at, out);
            if made != entry[&block.at] {
                entry.insert(block.at, made);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let first = graph.first().map_or_else(|| vec![None; count], |block| entry[&block.at].clone());
    // A range reaching the sentinels was found only on paths that never return.
    let bounded = |(low, high): &(i64, i64)| i64::MIN < *low && *high < i64::MAX && low < high;
    let made: Vec<Ranges> =
        first.into_iter().map(|one| one.unwrap_or_default().into_iter().filter(bounded).collect()).collect();
    assert!(
        pointers || made.iter().all(Vec::is_empty),
        "initialized: a body with no pointer parameter written initializes bytes"
    );
    Ok(made)
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

fn _cell_key(reference: &MemRef) -> Option<CellKey> {
    if let Some(provenance) = &reference.provenance {
        if provenance.slices.len() == 1 {
            let one = provenance.slices.first().expect("one slice");
            if one.stride == 1 {
                return Some(CellKey::Object(one.object, one.low, one.high));
            }
        }
    }
    reference.addr().map(|addr| CellKey::Address(addr, i64::from(reference.width)))
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
    incoming: IndexMap<i64, IndexMap<CellKey, Provenance>>,
}

thread_local! {
    static VALUE_SOLVES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static VALUE_ROUNDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static LIMIT_COLLAPSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
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

/// How many value solves this thread has made, for a test that two analyses of
/// one body ask one.
pub fn value_solves() -> usize {
    VALUE_SOLVES.with(std::cell::Cell::get)
}

/// The value solve of `points_to`.
pub fn point_values(unit: &Unit) -> Result<PointValues, String> {
    VALUE_SOLVES.with(|solves| solves.set(solves.get() + 1));
    let function = unit.function;
    let seeds = seeds(unit);
    let mut values = seeds.clone();
    let pointer_values = may_point(unit);
    let graph = cfg::graph(function);
    let predecessors = loops::predecessors(&graph);
    let none = BTreeSet::new();
    // A pointer value is otherwise an exact byte slice. Natural-loop joins
    // are the one place those exact facts can grow without a program bound.
    let shape = unit.shape();
    let dominance = &shape.dominance;
    let back_edges = graph
        .iter()
        .flat_map(|block| block.succ.iter().map(move |successor| (block.at, *successor)))
        .filter(|(at, successor)| dominance.dominates(*successor, *at))
        .collect::<BTreeSet<_>>();
    let mut incoming =
        graph.iter().map(|block| (block.at, IndexMap::<CellKey, Provenance>::default())).collect::<IndexMap<_, _>>();
    let mut outgoing = incoming.clone();
    let instructions = |at: i64| function.block(cfg::block(at)).instructions();

    // A block reads its parents' cells and these values. With neither
    // changed since its last visit it would compute what it already holds,
    // widening included, as widening an object's whole slice gives the same
    // slice.
    let reads = graph
        .iter()
        .map(|block| instructions(block.at).iter().flat_map(|&inst| read_values(unit, inst)).collect::<HashSet<_>>())
        .collect::<Vec<_>>();
    let tick = std::cell::Cell::new(0_u64);
    let touched = RefCell::new(HashMap::<ValueId, u64>::default());
    // Every pointer stored anywhere in each object: what a cell of it may
    // hold when its exact contents are not known.
    let mut fields = HashMap::<ObjectRef, Provenance>::default();
    // Objects a call or an unknown value may have written: no such bound.
    let mut unbounded = HashSet::<ObjectRef>::default();
    let mut sent = HashMap::<i64, u64>::default();
    let mut visited = vec![None::<u64>; graph.len()];
    let limited = RefCell::new(HashSet::<ValueId>::default());
    let any_limited = std::cell::Cell::new(false);
    loop {
        let changed = std::cell::Cell::new(false);
        VALUE_ROUNDS.with(|rounds| rounds.set(rounds.get() + 1));
        let learn = |values: &mut IndexMap<ValueId, Provenance>, value: ValueId, fact: Provenance| {
            // Past the limit a value stays whole: its objects whole joined with
            // another slice of them are not one slice, and a value
            // taken whole once would grow again by the slices its
            // loop makes.
            let fact = if fact.slices.len() > MAX_FIELDS_FOR_FIELD_SENSITIVE
                || (any_limited.get() && limited.borrow().contains(&value))
            {
                any_limited.set(true);
                if limited.borrow_mut().insert(value) {
                    LIMIT_COLLAPSES.with(|collapsed| collapsed.set(collapsed.get() + 1));
                }
                _widened(&fact)
            } else {
                fact
            };
            if values.get(&value) != Some(&fact) {
                values.insert(value, fact);
                touched.borrow_mut().insert(value, tick.get());
                changed.set(true);
            }
        };
        for (index, block) in graph.iter().enumerate() {
            let parents_at = predecessors.get(&block.at).unwrap_or(&none);
            if let Some(last) = visited[index] {
                let since = |stamp: Option<&u64>| stamp.is_some_and(|stamp| *stamp >= last);
                let touched = touched.borrow();
                if !parents_at.iter().any(|parent| since(sent.get(parent)))
                    && !reads[index].iter().any(|value| since(touched.get(value)))
                {
                    continue;
                }
            }
            tick.set(tick.get() + 1);
            visited[index] = Some(tick.get());
            let has_back_edge = parents_at.iter().any(|parent| back_edges.contains(&(*parent, block.at)));
            let mut state = IndexMap::default();
            {
                let parents = parents_at.iter().map(|one| &outgoing[one]).collect::<Vec<_>>();
                let previous_incoming = &incoming[&block.at];
                if !parents.is_empty() {
                    let keys = parents.iter().flat_map(|one| one.keys()).cloned().collect::<IndexSet<_>>();
                    for key in keys {
                        // A missing fact on one incoming edge is unknown, not
                        // an invitation to retain the
                        // other edge's pointer.
                        if parents.iter().all(|one| one.contains_key(&key)) {
                            let mut fact =
                                _union(parents.iter().map(|one| one.get(&key))).expect("every parent holds this key");
                            if has_back_edge {
                                if let Some(previous) = previous_incoming.get(&key) {
                                    if fact != *previous {
                                        fact = _widened(&previous.union(&fact));
                                    }
                                }
                            }
                            state.insert(key, fact);
                        }
                    }
                }
            }
            incoming.insert(block.at, state.clone());
            let mut state = CellMap::new(state, _key_place);
            for &inst in instructions(block.at) {
                let op = function.instruction(inst);
                if let (Some(arms), Some(result)) = (joined(op), op.result) {
                    let parts = arms.iter().map(|&one| _operand(unit, one, &values)).collect::<Vec<_>>();
                    if pointer_values.contains(&result) || (!parts.is_empty() && parts.iter().all(Option::is_some)) {
                        let mut fact = if parts.iter().any(Option::is_none) {
                            Some(UNKNOWN.clone())
                        } else {
                            _union(parts.iter().map(Option::as_ref))
                        };
                        if let Some(current) = &fact {
                            let carried = op.opcode == Opcode::Phi
                                && op.operands.iter().skip(1).step_by(2).any(|parent| {
                                    matches!(
                                        parent,
                                        Operand::Block(parent) if back_edges.contains(&(cfg::id(*parent), block.at))
                                    )
                                });
                            if carried {
                                if let Some(previous) = values.get(&result) {
                                    if current != previous {
                                        fact = Some(_widened(&previous.union(current)));
                                    }
                                }
                            }
                        }
                        if let Some(fact) = fact {
                            learn(&mut values, result, fact);
                        }
                    }
                    continue;
                }
                // Arithmetic derived from an already-known pointer proves its
                // own pointer nature.
                if let (Some(direct), Some(result)) = (_direct(unit, inst, &values)?, op.result) {
                    if !seeds.contains_key(&result) {
                        learn(&mut values, result, direct);
                    }
                }
                let reference = MemRef::of(unit, inst);
                match (&op.opcode, reference) {
                    (Opcode::Load { .. }, Some(reference))
                        if op.result.is_some_and(|result| pointer_values.contains(&result)) =>
                    {
                        let resolved =
                            MemRef { provenance: _resolved_reference(unit, &reference, &values), ..reference.clone() };
                        let loaded = _cell_key(&resolved).and_then(|key| state.get(&key).cloned());
                        let loaded = loaded.or_else(|| {
                            let stored = resolved
                                .provenance
                                .iter()
                                .flat_map(|provenance| provenance.slices.iter())
                                .map(|one| {
                                    (!unbounded.contains(&one.object)).then(|| fields.get(&one.object)).flatten()
                                })
                                .collect::<Option<Vec<_>>>()?;
                            (!stored.is_empty())
                                .then(|| _union(stored.into_iter().map(Some).chain([Some(&*UNKNOWN)])))?
                        });
                        if let Some(loaded) = loaded {
                            learn(&mut values, op.result.expect("a load's result"), loaded);
                        }
                    }
                    (Opcode::Store { .. }, Some(reference)) => {
                        let source = _operand(unit, op.operands[0], &values);
                        let keyed = MemRef { provenance: _resolved_reference(unit, &reference, &values), ..reference };
                        let pointer_stored = is_pointer(unit, op.operands[0]);
                        let bounded = source.is_some() || !pointer_stored;
                        if let (false, Some(targets)) = (bounded, &keyed.provenance) {
                            for one in &targets.slices {
                                if unbounded.insert(one.object) {
                                    changed.set(true);
                                }
                            }
                        }
                        if let (Some(source), Some(targets)) = (&source, &keyed.provenance) {
                            for one in &targets.slices {
                                // Whole objects: stored offsets may shift each
                                // trip around a loop.
                                let grown = _widened(
                                    &fields.get(&one.object).map_or_else(|| source.clone(), |held| held.union(source)),
                                );
                                if fields.get(&one.object) != Some(&grown) {
                                    fields.insert(one.object, grown);
                                    changed.set(true);
                                }
                            }
                        }
                        let key = _cell_key(&keyed);
                        // Any possibly overlapping write invalidates prior cell
                        // contents; an exact pointer store then defines it.
                        _kill(&mut state, key.as_ref());
                        if let (Some(key), Some(source)) = (key, &source) {
                            state.insert(key, source.clone(), _key_place);
                        }
                    }
                    // A call writes whatever its callee does, so a cell it
                    // may write holds an unknown pointer after it.
                    _ if unmodeled_write(unit, inst) => _kill(&mut state, None),
                    _ => {}
                }
            }
            let state = state.into_items();
            if outgoing[&block.at] != state {
                outgoing.insert(block.at, state);
                sent.insert(block.at, tick.get());
                changed.set(true);
            }
        }
        if !changed.get() {
            break;
        }
    }

    Ok(PointValues { values, incoming })
}

/// Flow pointer objects through values, exact spill slots and CFG joins.
pub fn points_to(
    unit: &Unit,
    arguments: Option<&IndexMap<InstId, Vec<Actual>>>,
    captures: Option<&IndexMap<InstId, Option<BTreeSet<Option<Identity>>>>>,
) -> Result<PointsTo, String> {
    let solved = match unit.point_values {
        Some(held) => {
            if llrm_support::env_set("LLRM_CHECK_POINTVALUES") {
                assert!(*held == point_values(unit)?, "the held point values are not what solving again gives");
            }
            std::borrow::Cow::Borrowed(held)
        }
        None => std::borrow::Cow::Owned(point_values(unit)?),
    };
    escapes(unit, &solved, arguments, captures)
}

/// What `points_to` adds to the value solve: what escapes, and when.
fn escapes(
    unit: &Unit,
    solved: &PointValues,
    arguments: Option<&llrm_support::hash::SparseIdMap<InstId, Vec<Actual>>>,
    captures: Option<&llrm_support::hash::SparseIdMap<InstId, Option<BTreeSet<Option<Identity>>>>>,
) -> Result<PointsTo, String> {
    let function = unit.function;
    let graph = cfg::graph(function);
    let predecessors = loops::predecessors(&graph);
    let none = BTreeSet::new();
    let (values, incoming) = (&solved.values, &solved.incoming);
    let instructions = |at: i64| function.block(cfg::block(at)).instructions();
    // Escape is flow-sensitive separately from pointer contents. A pointer
    // published after a call must not make the earlier call reach its frame.
    let mut pointer_fields: IndexMap<ObjectRef, BTreeSet<ObjectRef>> = IndexMap::default();
    for (_, inst) in function.walk() {
        let op = function.instruction(inst);
        let (Opcode::Store { .. }, Some(reference)) = (&op.opcode, MemRef::of(unit, inst)) else { continue };
        let Some(source) = _operand(unit, op.operands[0], &values) else { continue };
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
    // Close publication through pointer-valued fields of known objects.
    let pointees = |objects: BTreeSet<ObjectRef>, cells: &IndexMap<CellKey, Provenance>| {
        let mut reached = objects.iter().map(number).collect::<HashSet<_>>();
        if reached.is_empty() {
            return Vec::new();
        }
        loop {
            let before = reached.len();
            for (key, provenance) in cells {
                if let CellKey::Object(object, _, _) = key {
                    if reached.contains(&number(object)) {
                        reached.extend(provenance.slices.iter().map(|one| number(&one.object)));
                    }
                }
            }
            for object in reached.iter().copied().collect::<Vec<_>>() {
                if let Some(fields) = pointer_fields.get(&object) {
                    reached.extend(fields.iter().copied());
                }
            }
            if reached.len() == before {
                return reached.into_iter().collect();
            }
        }
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
    let mut publishes: IndexMap<i64, Vec<Vec<usize>>> = IndexMap::default();
    let mut during: IndexMap<InstId, Vec<usize>> = IndexMap::default();
    for block in &graph {
        let mut cells = CellMap::new(incoming[&block.at].clone(), _key_place);
        let mut mine = Vec::new();
        for &inst in instructions(block.at) {
            let op = function.instruction(inst);
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
                            None => Ok(!_borrowed(unit, inst, index)),
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
                        .filter(|&index| !_borrowed(unit, inst, index))
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
            match (&op.opcode, MemRef::of(unit, inst)) {
                (Opcode::Store { .. }, Some(reference)) => {
                    let destination = _resolved_reference(unit, &reference, &values);
                    let outside = destination
                        .as_ref()
                        .is_none_or(
                            |provenance| provenance.slices.iter().any(|one| one.object.kind != MemoryKind::Frame),
                        );
                    if outside {
                        newly.extend(provenances(&op.operands[..1], &values));
                    }
                    let source = _operand(unit, op.operands[0], &values);
                    let keyed = MemRef { provenance: destination, ..reference };
                    let key = _cell_key(&keyed);
                    _kill(&mut cells, key.as_ref());
                    if let (Some(key), Some(source)) = (key, source) {
                        cells.insert(key, source, _key_place);
                    }
                }
                // A call may leave a cell as it was: what it held stays
                // reachable.
                _ if unmodeled_write(unit, inst) => {}
                _ => {}
            }
            let lent_numbers = lent.iter().map(number).collect::<HashSet<_>>();
            let mut published = pointees(newly, &cells);
            published.extend(pointees(lent, &cells).into_iter().filter(|one| !lent_numbers.contains(one)));
            mine.push(published);
            during.insert(inst, initialized(pointees(passing, &cells)));
        }
        publishes.insert(block.at, mine);
    }
    let objects = objects.into_inner();
    let generated = publishes
        .iter()
        .map(|(at, mine)| {
            let mut all = Bits::new(objects.len());
            mine.iter().flatten().for_each(|one| all.insert(*one));
            (*at, all)
        })
        .collect::<IndexMap<_, _>>();
    let mut out = graph.iter().map(|block| (block.at, Bits::new(objects.len()))).collect::<IndexMap<_, _>>();
    let entering = |at: i64, out: &IndexMap<i64, Bits>| {
        let mut state = Bits::new(objects.len());
        predecessors.get(&at).unwrap_or(&none).iter().for_each(|one| state.union_with(&out[one]));
        state
    };
    let mut changing = true;
    while changing {
        changing = false;
        for block in &graph {
            let mut state = entering(block.at, &out);
            state.union_with(&generated[&block.at]);
            if state != out[&block.at] {
                out.insert(block.at, state);
                changing = true;
            }
        }
    }
    let named = |bits: &Bits| bits.iter().map(|at| objects[at].clone()).collect::<BTreeSet<_>>();
    // The state after an instruction holds the state before it: what a
    // call publishes itself reaches it, as its own arguments do.
    let mut before = IndexMap::<InstId, Bits>::default();
    for block in &graph {
        let mut state = entering(block.at, &out);
        for (&inst, escapes) in instructions(block.at).iter().zip(&publishes[&block.at]) {
            escapes.iter().for_each(|one| state.insert(*one));
            if calls.contains(&inst)
                || matches!(
                    function.instruction(inst).opcode,
                    Opcode::Load { .. } | Opcode::Store { .. }
                )
            {
                let mut visible = state.clone();
                if let Some(reached) = during.get(&inst) {
                    reached.iter().for_each(|one| visible.insert(*one));
                }
                before.insert(inst, visible);
            }
        }
    }
    let mut every = Bits::new(objects.len());
    out.values().for_each(|one| every.union_with(one));
    let escaped = named(&every);
    let escaped_before = EscapedBefore { objects: Rc::new(objects), at: before };
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
    let carried = match &op.opcode {
        Opcode::ICmp(_) => return Vec::new(),
        Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) => !unit.spaces().is_segment(unit.space(op.operands[0])),
        Opcode::Load { .. } | Opcode::Store { .. } | Opcode::GetElementPtr { .. } | Opcode::Phi | Opcode::Select => {
            true
        }
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

/// The object a cell key lies in: only keys sharing it can overlap.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum KeyBucket {
    Object(ObjectRef),
    Address(Operand),
}

impl Bucket for KeyBucket {
    // Nothing looks a key bucket up by a component.
    type Parts = ();

    fn held(
        &self,
        _: &mut (),
    ) {
    }

    fn released(
        &self,
        _: &mut (),
    ) {
    }
}

pub fn _key_bucket(key: &CellKey) -> KeyBucket {
    match key {
        CellKey::Object(object, _, _) => KeyBucket::Object(object.clone()),
        CellKey::Address(addr, _) => KeyBucket::Address(addr.root),
    }
}

/// A key's bucket, and no span: Python's map has no `span_of`.
pub fn _key_place(key: &CellKey) -> (KeyBucket, Option<ByteRange>) {
    (_key_bucket(key), None)
}

/// Drop the cells a store to `key` may overwrite, keeping `key` itself.
pub fn _kill<V>(
    cells: &mut CellMap<CellKey, V, KeyBucket>,
    key: Option<&CellKey>,
) {
    let reached = key.map(|key| std::iter::once(_key_bucket(key)).collect());
    cells.kill(
        reached,
        |old| {
            #[cfg(test)]
            ASKED.with(|asked| asked.borrow_mut().push(old.clone()));
            Some(old) != key && _keys_overlap(Some(old), key)
        },
        None,
    );
}

#[cfg(test)]
thread_local! {
    /// The cells `_kill` asked the exact test of.
    pub static ASKED: RefCell<Vec<CellKey>> = const { RefCell::new(Vec::new()) };
}

pub fn _keys_overlap(
    one: Option<&CellKey>,
    other: Option<&CellKey>,
) -> bool {
    let (Some(one), Some(other)) = (one, other) else {
        return true;
    };
    match (one, other) {
        (CellKey::Address(addr, width), CellKey::Address(other_addr, other_width)) if addr.root == other_addr.root => {
            addr.disp < other_addr.disp + other_width && other_addr.disp < addr.disp + width
        }
        (CellKey::Object(object, low, high), CellKey::Object(other_object, other_low, other_high))
            if object == other_object =>
        {
            low < other_high && other_low < high
        }
        _ => one == other,
    }
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
pub fn congruences(unit: &Unit) -> IndexMap<ValueId, (BigInt, BigInt)> {
    congruences_with(unit, &unit.registers())
}

/// `congruences`, given what `consts::known` finds without memory.
pub fn congruences_with(
    unit: &Unit,
    constants: &IndexMap<ValueId, Known>,
) -> IndexMap<ValueId, (BigInt, BigInt)> {
    let function = unit.function;
    let mut result = IndexMap::<ValueId, (BigInt, BigInt)>::default();
    let zero = BigInt::from(0);
    for loop_ in &unit.shape().loops {
        for affine in induction::basics(unit, loop_).values() {
            let width = affine.start.width();
            let (Some(start), Some(step)) = (
                induction::_signed(&affine.start, &constants, width),
                induction::_signed(&affine.step, &constants, width),
            ) else {
                continue;
            };
            if step != zero {
                result.insert(affine.value, reduced(step, start, width));
            }
        }
    }
    loop {
        let mut changed = false;
        for (_, inst) in function.walk() {
            let op = function.instruction(inst);
            let Some(value) = op.result.filter(|value| !result.contains_key(value)) else { continue };
            let (Opcode::Binary(kind), [left, right], Some(width)) =
                (&op.opcode, op.operands.as_slice(), unit.int_bits(Operand::Value(value)))
            else {
                continue;
            };
            // A value nothing is known of is a multiple of 1: `x << 1` is a
            // multiple of 2 all the same.
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
            result.insert(value, found);
            changed = true;
        }
        if !changed {
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
/// `ranges::bounded`'s facts, the manager's where the unit carries them for the
/// registers asked of, else worked out.
enum Bounded<'a> {
    Held(&'a ranges::Bounds),
    Worked(ranges::Facts),
}

impl Bounded<'_> {
    fn at(
        &self,
        at: i64,
    ) -> Option<&IndexMap<ValueId, ranges::Interval>> {
        match self {
            Bounded::Held(held) => held.at(at),
            Bounded::Worked(worked) => worked.get(&at),
        }
    }
}

pub fn annotated_with(
    unit: &Unit,
    facts: &PointsTo,
    known: &IndexMap<ValueId, Known>,
) -> Result<IndexMap<InstId, MemRef>, String> {
    let bounded = match (unit.bounds, unit.registers) {
        (Some(held), Some(registers)) if std::ptr::eq(known, registers) => {
            if llrm_support::env_set("LLRM_CHECK_REPLAY") {
                assert!(
                    held.facts() == ranges::bounded_with(unit, known)?,
                    "the bounds a unit carries are not those of the body it stands over: stale"
                );
            }
            Bounded::Held(held)
        }
        _ => Bounded::Worked(ranges::bounded_with(unit, known)?),
    };
    let strides = congruences_with(unit, known);
    let constants = ranges::intervals(known);

    let tag = |reference: &MemRef, at: i64| -> Result<MemRef, String> {
        let mut got = facts.reference(unit, reference);
        let interval = reference
            .base
            .and_then(|base| bounded.at(at).and_then(|known| known.get(&base)).or_else(|| constants.get(&base)));
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
                    let model = |number: &BigInt| {
                        i64::try_from(number).map_err(|_| format!("slice bound {number} exceeds the i64 slice model"))
                    };
                    got = Some(Provenance {
                        slices: BTreeSet::from([Slice::new(
                            source.object.clone(),
                            model(&low)?,
                            model(&high)?,
                            model(&stride)?,
                            width,
                        )
                        .expect("a nonempty positive-stride slice is valid")]),
                        restrict: current.restrict.clone(),
                    });
                }
            }
        }
        // A far access whose selector's range lands it in foreign memory
        // names those linear bytes, whatever its pointer's provenance.
        if let (None, Some(Operand::Value(segment))) = (reference.selector, reference.segment) {
            let lookup = |value: ValueId| {
                bounded
                    .at(at)
                    .and_then(|known| known.get(&value))
                    .or_else(|| constants.get(&value))
                    .map(|one| (value, one.clone()))
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

    let mut out = IndexMap::default();
    for (block, inst) in unit.function.walk() {
        if let Some(reference) = MemRef::of(unit, inst).or_else(|| MemRef::filled(unit, inst)) {
            out.insert(inst, tag(&reference, cfg::id(block))?);
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
