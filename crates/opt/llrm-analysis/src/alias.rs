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
//! Tests skipped: `test_outgoing_argument_stack_does_not_kill_current_frame_values`
//! (no push area), `test_pointer_fact_does_not_hide_a_conflicting_concrete_operand_object`
//! (no attached provenance), `test_a_lane_form_slice_names_every_byte_it_covers`
//! (`named_bytes`). `test_unknown_call_reaches_nonlocals_and_only_its_pointer_actual`
//! drops its Python `repr` order, and `test_interprocedural_modref_reaches_the_call_operation`
//! its `memory_complete` flag: a call's effect is a side table.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::LazyLock;

use crate::graph::loops;
use llrm_mir::context::{ConstantKind, GlobalId};
use llrm_mir::facts::Facts;
use llrm_mir::memory::Effects;
use llrm_mir::module::{InstId, Linkage, Operand, ValueId};
use llrm_mir::opcode::{Attribute, BinaryOp, CastOp, Opcode};
use llrm_mir::types::Type;
use llrm_support::bits::Bits;
use llrm_support::hash::{HashMap, HashSet, IndexMap, IndexSet};
use num_bigint::BigInt;

use crate::cellmap::{Bucket, CellMap};
use crate::cfg;
use crate::consts::Known;
use crate::globalsaa;
use crate::induction;
use crate::memory::{self, Addr, Identity, MemRef, MemoryKind, MemoryObject, Provenance, Slice, Unit, object_of, unmodeled_write, wrapped};
use crate::ranges;
use crate::regions::{self, ByteRange};

pub static UNKNOWN: LazyLock<Provenance> = LazyLock::new(|| Provenance::one(MemoryObject::new(MemoryKind::Unknown)));
pub static NONLOCAL: LazyLock<Provenance> = LazyLock::new(|| Provenance::one(MemoryObject::new(MemoryKind::Nonlocal)));
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
    Object(MemoryObject, i64, i64),
    Address(Addr, i64),
}

/// Whole objects an unknown callee can reach through pointers it owns.
fn _whole<'a>(provenances: impl IntoIterator<Item = &'a Provenance>, escaped: &BTreeSet<MemoryObject>) -> BTreeSet<Slice> {
    let mut objects = provenances.into_iter().flat_map(|provenance| provenance.slices.iter().map(|one| one.object.clone())).collect::<BTreeSet<_>>();
    objects.extend(escaped.iter().cloned());
    objects.into_iter().filter_map(Slice::every_byte).collect()
}

/// Python `qbopt.analysis.alias:PointsTo`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointsTo {
    pub values: IndexMap<ValueId, Provenance>,
    pub escaped: BTreeSet<MemoryObject>,
    /// Objects visible immediately before each call.
    pub escaped_before: EscapedBefore,
}

/// Objects escaped before each call, load and store, kept as bits over one
/// numbering and named only when asked: most solves never read them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EscapedBefore {
    objects: Rc<IndexSet<MemoryObject>>,
    at: IndexMap<InstId, Bits>,
}

impl EscapedBefore {
    pub fn get(&self, at: &InstId) -> Option<BTreeSet<MemoryObject>> {
        self.at.get(at).map(|bits| bits.iter().map(|one| self.objects[one].clone()).collect())
    }

    /// Whether the access `reference` at `at`, through a pointer no fact
    /// follows or one only to what escaped, cannot reach `cell`: every byte
    /// of it is in frame objects not escaped before `at`. LLVM's
    /// `EarliestEscapeInfo`.
    pub fn apart(&self, at: InstId, reference: &MemRef, cell: &MemRef) -> bool {
        let Some(bits) = self.at.get(&at) else { return false };
        // No provenance is a pointer no fact follows: `_lost` publishes what
        // it came from.
        let escaping = reference.provenance.as_ref().is_none_or(|one| one.slices.iter().all(|slice| matches!(slice.object.kind, MemoryKind::Unknown | MemoryKind::Nonlocal)));
        let unreached = |object: &MemoryObject| object.kind == MemoryKind::Frame && self.objects.get_index_of(object).is_none_or(|one| !bits.contains(one));
        escaping && cell.provenance.as_ref().is_some_and(|one| !one.slices.is_empty() && one.slices.iter().all(|slice| unreached(&slice.object)))
    }
}

impl PointsTo {
    /// Canonical bytes reached by an access through an analysed pointer.
    pub fn reference(&self, unit: &Unit, reference: &MemRef) -> Option<Provenance> {
        _resolved_reference(unit, reference, &self.values)
    }

    /// Whether `value` can only designate a real static or frame object.
    ///
    /// Incoming pointers remain nullable. A frame object or a global is
    /// non-null by the source language contract even though its eventual
    /// 16-bit offset is not known until link time.
    pub fn nonnull(&self, value: ValueId) -> bool {
        self.values.get(&value).is_some_and(nonnull)
    }
}

fn nonnull(provenance: &Provenance) -> bool {
    !provenance.slices.is_empty()
        && provenance.slices.iter().all(|one| matches!(one.object.kind, MemoryKind::Frame | MemoryKind::Global | MemoryKind::External | MemoryKind::Named))
}

/// `PointsTo::nonnull` of `value` where its definition alone settles it,
/// as LLVM's `isKnownNonZero` reads a pointer's underlying object instead
/// of solving every pointer; `None` where only the whole solve can say.
pub fn nonnull_by_definition(unit: &Unit, value: ValueId) -> Option<bool> {
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

fn is_pointer(unit: &Unit, operand: Operand) -> bool {
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
            let object = MemoryObject { identity: Some(Identity::Int(at as i64)), ..MemoryObject::new(MemoryKind::Parameter) };
            let restrict = if Facts::param(function, at).no_alias() { BTreeSet::from([Identity::Int(at as i64)]) } else { BTreeSet::new() };
            (*value, Provenance::one_with_slice(object, 0, 1, 1, 1, restrict).expect("one byte is a slice"))
        })
        .collect()
}

/// The provenance of a pointer operand: a value's as solved, a constant's
/// from the global it addresses.
fn _operand(unit: &Unit, operand: Operand, values: &IndexMap<ValueId, Provenance>) -> Option<Provenance> {
    match operand {
        Operand::Value(value) => values.get(&value).cloned(),
        Operand::Constant(_) => {
            let access = MemRef::at(unit, operand, 1);
            let root = access.root?;
            if !matches!(root, Operand::Constant(id) if matches!(unit.context.get(id).kind, ConstantKind::Global(_))) {
                return None;
            }
            let object = object_of(unit, root)?;
            Provenance::one_with_slice(object, access.disp, access.disp + 1, 1, 1, BTreeSet::new()).ok()
        }
        Operand::Block(_) => None,
    }
}

/// Resolve an access through the current pointer-value facts.
fn _resolved_reference(unit: &Unit, reference: &MemRef, values: &IndexMap<ValueId, Provenance>) -> Option<Provenance> {
    if let Some(attached) = &reference.provenance {
        return Some(attached.clone());
    }
    let source = _operand(unit, reference.pointer?, values)?;
    // A singleton address names `width` consecutive bytes. A set of
    // indexed addresses retains its stride and widens its final lane.
    let slices = source
        .slices
        .iter()
        .map(|one| Slice::new(one.object.clone(), one.low, one.high, one.stride, i64::from(reference.width.max(1))).expect("a slice keeps its positive shape"))
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
}

impl Summary {
    pub fn instantiated(&self, arguments: &[Provenance]) -> Summary {
        let expand = |items: &BTreeSet<Slice>| {
            let mut out = BTreeSet::new();
            for item in items {
                if item.object.kind != MemoryKind::Parameter {
                    out.insert(item.clone());
                    continue;
                }
                let Some(Identity::Int(index)) = &item.object.identity else {
                    continue;
                };
                if !(0 <= *index && *index < arguments.len() as i64) {
                    continue;
                }
                for actual in &arguments[*index as usize].slices {
                    out.insert(
                        Slice::new(actual.object.clone(), actual.low + item.low, actual.high + item.high - 1, memory::gcd(actual.stride, item.stride), item.width)
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
        }
    }
}

/// Python `qbopt.analysis.alias:Procedure`: a function, its calls by
/// callee name, and each call's actual arguments.
#[derive(Clone)]
pub struct Procedure<'a> {
    pub unit: Unit<'a>,
    pub calls: IndexMap<InstId, String>,
    pub arguments: IndexMap<InstId, Vec<Actual>>,
}

impl<'a> Procedure<'a> {
    /// `unit`'s calls of named functions, and every call's actuals, one
    /// per argument.
    pub fn of(unit: Unit<'a>) -> Self {
        let function = unit.function;
        let mut calls = IndexMap::default();
        let mut arguments = IndexMap::default();
        for (_, inst) in function.walk() {
            let op = function.instruction(inst);
            let (Opcode::Call(_) | Opcode::Invoke(_)) = op.opcode else { continue };
            let callee = *op.operands.last().expect("a call names its callee");
            if let Operand::Constant(id) = callee
                && let ConstantKind::Global(global) = unit.context.get(id).kind
                && let Some(name) = unit.globals.get(global.0 as usize).and_then(|one| one.name.clone())
            {
                calls.insert(inst, name);
            }
            let count = match op.opcode {
                Opcode::Invoke(_) => op.operands.len() - 3,
                _ => op.operands.len() - 1,
            };
            let actual = op.operands[..count]
                .iter()
                .map(|&one| match one {
                    Operand::Value(value) if is_pointer(&unit, one) => Actual::Pointer(value, 0),
                    // A null pointer points to nothing.
                    Operand::Constant(id) if matches!(unit.context.get(id).kind, ConstantKind::Null | ConstantKind::Zero | ConstantKind::Poison) => Actual::Absent,
                    Operand::Constant(_) if is_pointer(&unit, one) => _operand(&unit, one, &IndexMap::default()).map_or(Actual::Provenance(UNKNOWN.clone()), Actual::Provenance),
                    _ => Actual::Absent,
                })
                .collect();
            arguments.insert(inst, actual);
        }
        Self { unit, calls, arguments }
    }
}

/// The calls of `unit`, in order.
fn call_sites(unit: &Unit) -> Vec<InstId> {
    let function = unit.function;
    function.walk().map(|(_, inst)| inst).filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::Call(_) | Opcode::Invoke(_))).collect()
}

/// Whole slices of `globals`.
fn _globals(unit: &Unit, globals: impl IntoIterator<Item = GlobalId>) -> BTreeSet<Slice> {
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

fn _allowed(unit: &Unit, at: InstId) -> Allowed {
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &unit.function.instruction(at).opcode else {
        return Allowed { arguments: Effects::ANY, other: Effects::ANY, through: Vec::new() };
    };
    let declared = llrm_mir::memory::callee(unit.context, unit.function, at).and_then(|one| unit.globals.get(one.0 as usize)).and_then(|one| one.function());
    let both = |one: Effects, other: Effects| Effects { reads: one.reads && other.reads, writes: one.writes && other.writes };
    let (mut arguments, mut other) = llrm_mir::memory::located(&info.attrs);
    if let Some(declared) = declared {
        let (on_arguments, on_other) = llrm_mir::memory::located(&declared.attrs);
        (arguments, other) = (both(arguments, on_arguments), both(other, on_other));
    }
    let through = (0..info.argument_attrs.len())
        .map(|index| {
            let parameter = declared.and_then(|one| one.parameter_attrs.get(index)).map_or(Effects::ANY, |attrs| llrm_mir::memory::through(attrs));
            both(both(arguments, llrm_mir::memory::through(&info.argument_attrs[index])), parameter)
        })
        .collect();
    Allowed { arguments, other, through }
}

/// Per argument of the call `at`, its pointer operand, where the callee may
/// read through it.
pub fn read_arguments(unit: &Unit, at: InstId) -> Vec<Operand> {
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
        .filter(|(index, one)| is_pointer(unit, **one) && allowed.through.get(*index).is_none_or(|effects| effects.reads))
        .map(|(_, one)| *one)
        .collect()
}

/// Whether the call `at` keeps no copy of its argument `index`: `nocapture`
/// at the site or on the callee's parameter.
fn _borrowed(unit: &Unit, at: InstId, index: usize) -> bool {
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &unit.function.instruction(at).opcode else { return false };
    let nocapture = |attrs: &[Attribute]| llrm_mir::memory::has(attrs, "nocapture");
    let declared = llrm_mir::memory::callee(unit.context, unit.function, at).and_then(|one| unit.globals.get(one.0 as usize)).and_then(|one| one.function());
    info.argument_attrs.get(index).is_some_and(|attrs| nocapture(attrs)) || declared.and_then(|one| one.parameter_attrs.get(index)).is_some_and(|attrs| nocapture(attrs))
}

/// What a callee nobody summarized may read and write at `at`, as far as
/// its attributes allow: through each actual, the objects it points to;
/// elsewhere, what a nonlocal reaches, what escaped before the call, and
/// of the tracked globals what GlobalsAA says it names and what
/// `callbacks` into the module do, all of them where that is unknown.
fn _unknown_visible(procedure: &Procedure, facts: &PointsTo, at: InstId, actual: &[Provenance], callbacks: Option<&Summary>) -> Result<(BTreeSet<Slice>, BTreeSet<Slice>), String> {
    let unit = &procedure.unit;
    let allowed = _allowed(unit, at);
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
    let (other_reads, other_writes) = _unknown_other(unit, facts, at, callbacks)?;
    if allowed.other.reads {
        reads.extend(other_reads);
    }
    if allowed.other.writes {
        writes.extend(other_writes);
    }
    Ok((reads, writes))
}

/// What an unsummarized callee may read and write at `at` other than
/// through its arguments.
fn _unknown_other(unit: &Unit, facts: &PointsTo, at: InstId, callbacks: Option<&Summary>) -> Result<(BTreeSet<Slice>, BTreeSet<Slice>), String> {
    let mut reads = NONLOCAL.slices.clone();
    reads.extend(_whole([], &facts.escaped_before.get(&at).unwrap_or_default()));
    let mut writes = reads.clone();
    let Some(globals) = unit.globals_aa else { return Ok((reads, writes)) };
    let callee = llrm_mir::memory::callee(unit.context, unit.function, at);
    let (read, written) = globals.unsummarized(callee);
    reads.extend(_globals(unit, read));
    writes.extend(_globals(unit, written));
    // An indirect callee may be an entry itself.
    if callee.is_none() || globalsaa::calls_back(unit, at) {
        let Some(callbacks) = callbacks else {
            reads.extend(_tracked(unit));
            writes.extend(_tracked(unit));
            return Ok((reads, writes));
        };
        reads.extend(callbacks.reads.iter().cloned());
        writes.extend(callbacks.writes.iter().cloned());
        if callbacks.unknown_read {
            reads.extend(_tracked(unit));
        }
        if callbacks.unknown_write {
            writes.extend(_tracked(unit));
        }
    }
    Ok((reads, writes))
}

/// What calling back into the module may do: the effects of the unit's
/// GlobalsAA entries, in no caller's object space; none where `known`
/// lacks one.
fn _callbacks(unit: &Unit, known: &IndexMap<String, Summary>) -> Option<Summary> {
    let mut out = Summary::default();
    for name in unit.globals_aa?.entries() {
        let one = known.get(name)?.instantiated(&[]);
        out.reads.extend(one.reads);
        out.writes.extend(one.writes);
        out.unknown_read |= one.unknown_read;
        out.unknown_write |= one.unknown_write;
    }
    Some(Summary { reads: _coalesced(&out.reads), writes: _coalesced(&out.writes), ..out })
}

fn _actuals(procedure: &Procedure, facts: &PointsTo, at: InstId) -> Vec<Provenance> {
    procedure
        .arguments
        .get(&at)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .map(|actual| match actual {
            Actual::Provenance(provenance) => provenance.clone(),
            Actual::Pointer(value, displacement) if facts.values.contains_key(value) => facts.values[value].shifted(*displacement),
            Actual::Absent => EMPTY.clone(),
            Actual::Pointer(..) => UNKNOWN.clone(),
        })
        .collect()
}

/// Whether `slice` outlives its function's activation: no frame object.
fn outlives(slice: &Slice) -> bool {
    !matches!(slice.object.kind, MemoryKind::Frame | MemoryKind::Stack)
}

pub fn _direct_summary(unit: &Unit) -> Result<Summary, String> {
    let (mut reads, mut writes) = (BTreeSet::new(), BTreeSet::new());
    let (mut unknown_read, mut unknown_write) = (false, false);
    let facts = points_to(unit, None, None)?;
    for (_, inst) in unit.function.walk() {
        let Some(reference) = MemRef::of(unit, inst) else { continue };
        let read = matches!(unit.function.instruction(inst).opcode, Opcode::Load { .. });
        let Some(provenance) = facts.reference(unit, &reference) else {
            if read {
                unknown_read = true;
            } else {
                unknown_write = true;
            }
            continue;
        };
        for one in provenance.slices {
            let unknown = one.object.kind == MemoryKind::Unknown;
            if outlives(&one) {
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
                }
            }
        }
    }
    let captures =
        facts.escaped.iter().filter(|one| one.kind == MemoryKind::Parameter && matches!(one.identity, Some(Identity::Int(_)))).map(|one| one.identity.clone()).collect();
    Ok(Summary { reads, writes, captures, unknown_read, unknown_write })
}

fn _recursive_edges(procedures: &IndexMap<String, Procedure>) -> BTreeSet<(String, String)> {
    let graph = procedures
        .iter()
        .map(|(name, procedure)| {
            let targets = procedure.calls.values().filter(|target| procedures.contains_key(*target)).collect::<BTreeSet<_>>();
            (name, targets)
        })
        .collect::<IndexMap<_, _>>();

    let reaches = |start: &String, wanted: &String| {
        let (mut pending, mut seen) = (vec![start], BTreeSet::new());
        while let Some(at) = pending.pop() {
            if at == wanted {
                return true;
            }
            if !seen.insert(at) {
                continue;
            }
            pending.extend(graph.get(at).into_iter().flatten().copied());
        }
        false
    };

    graph
        .iter()
        .flat_map(|(caller, targets)| targets.iter().filter(|callee| reaches(callee, caller)).map(|callee| ((*caller).clone(), (*callee).clone())))
        .collect()
}

fn _widen_parameters(summary: Summary) -> Summary {
    let widened = |items: &BTreeSet<Slice>| {
        items.iter().map(|one| if one.object.kind == MemoryKind::Parameter { Slice::whole(one.object.clone()) } else { one.clone() }).collect()
    };
    Summary { reads: widened(&summary.reads), writes: widened(&summary.writes), ..summary }
}

/// Drop subranges once the same object already has a whole-object effect.
fn _coalesced(items: &BTreeSet<Slice>) -> BTreeSet<Slice> {
    let is_whole = |one: &Slice| one.low == memory::WHOLE_LOW && one.high == memory::WHOLE_HIGH && one.stride == 1 && one.width == 1;
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
fn _summary<'s>(unit: &Unit, known: &'s IndexMap<String, Summary>, name: &str) -> Option<&'s Summary> {
    let global = unit.globals.iter().find(|one| one.name.as_deref() == Some(name));
    let replaceable = global.is_some_and(|one| one.function().is_some() && !matches!(one.linkage, Linkage::External | Linkage::Internal | Linkage::Private | Linkage::ExternWeak));
    (!replaceable).then(|| known.get(name)).flatten()
}

/// Transitive per-procedure mod/ref and capture summaries to a fixed point.
///
/// `known` supplies established external semantics, such as C library
/// functions. A body in this compilation unit always takes precedence; one
/// that may be replaced describes no call.
pub fn summaries(procedures: &IndexMap<String, Procedure>, known: Option<&IndexMap<String, Summary>>) -> Result<IndexMap<String, Summary>, String> {
    // What a body captures grows from nothing: a call captures what its
    // callee's summary says, so a least fixed point, as a recursive one
    // that captures nothing proves.
    let direct = procedures
        .iter()
        .map(|(name, one)| Ok((name.clone(), Summary { captures: BTreeSet::new(), .._direct_summary(&one.unit)? })))
        .collect::<Result<Vec<_>, String>>()?;
    let mut result = known.cloned().unwrap_or_default();
    result.extend(direct);
    let recursive = _recursive_edges(procedures);
    loop {
        let mut changed = false;
        for (name, procedure) in procedures {
            let callbacks = _callbacks(&procedure.unit, &result);
            let captured_at = procedure.calls.iter().map(|(at, target)| (*at, _summary(&procedure.unit, &result, target).map(|one| one.captures.clone()))).collect::<IndexMap<_, _>>();
            let facts = points_to(&procedure.unit, Some(&procedure.arguments), Some(&captured_at))?;
            let direct = _direct_summary(&procedure.unit)?;
            let (mut reads, mut writes) = (direct.reads, direct.writes);
            let captures = facts.escaped.iter().filter(|one| one.kind == MemoryKind::Parameter && matches!(one.identity, Some(Identity::Int(_)))).map(|one| one.identity.clone()).collect();
            let (mut unknown_read, mut unknown_write) = (direct.unknown_read, direct.unknown_write);
            for at in call_sites(&procedure.unit) {
                let target = procedure.calls.get(&at);
                let callee = target.and_then(|target| _summary(&procedure.unit, &result, target));
                let actual = _actuals(procedure, &facts, at);
                let Some(callee) = callee else {
                    let (read, written) = _unknown_visible(procedure, &facts, at, &actual, callbacks.as_ref())?;
                    reads.extend(read);
                    writes.extend(written);
                    continue;
                };
                let mut effect = callee.instantiated(&actual);
                let target = target.expect("a known callee has a target");
                if recursive.contains(&(name.clone(), target.clone())) {
                    effect = _widen_parameters(effect);
                }
                reads.extend(effect.reads);
                writes.extend(effect.writes);
                unknown_read |= effect.unknown_read;
                unknown_write |= effect.unknown_write;
            }
            // A callee's frame is gone when it returns: what its calls touch
            // there, like its own accesses, is no effect of calling it.
            let (reads, writes) = (reads.into_iter().filter(outlives).collect(), writes.into_iter().filter(outlives).collect());
            let made = Summary { reads: _coalesced(&reads), writes: _coalesced(&writes), captures, unknown_read, unknown_write };
            if made != result[name] {
                result.insert(name.clone(), made);
                changed = true;
            }
        }
        if !changed {
            return Ok(result);
        }
    }
}

/// What one call reads and writes, as the bytes of the objects it reaches.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Effect {
    pub loads: Vec<MemRef>,
    pub stores: Vec<MemRef>,
    /// The bytes it writes before reading any, as `initializes` states.
    pub fills: Vec<MemRef>,
}

/// Instantiate callee effects through actual pointer provenance: each
/// call's effect, as old `calls_annotated` wrote it into the call.
pub fn calls_annotated(procedure: &Procedure, known: &IndexMap<String, Summary>) -> Result<IndexMap<InstId, Effect>, String> {
    // Capture is part of escape flow. Unknown callees may retain every
    // pointer actual; known callees retain only the parameters their fixed
    // point summary says they capture.
    let callee = |at: &InstId| procedure.calls.get(at).and_then(|target| _summary(&procedure.unit, known, target));
    let captures = procedure.calls.keys().map(|at| (*at, callee(at).map(|one| one.captures.clone()))).collect::<IndexMap<_, _>>();
    let facts = points_to(&procedure.unit, Some(&procedure.arguments), Some(&captures))?;
    let callbacks = _callbacks(&procedure.unit, known);

    let reference = |one: &Slice| {
        MemRef::reach(u32::try_from(one.width).expect("a slice width is a memory width"), Provenance { slices: BTreeSet::from([one.clone()]), restrict: BTreeSet::new() })
    };

    let mut out = IndexMap::default();
    for at in call_sites(&procedure.unit) {
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
                let (reads, writes) = _unknown_visible(procedure, &facts, at, &actual, callbacks.as_ref())?;
                Summary { reads, writes, ..Summary::default() }
            }
        };
        if effect.unknown_read {
            let visible = _whole(&actual, &facts.escaped_before.get(&at).unwrap_or_default());
            effect.reads.extend(if visible.is_empty() { UNKNOWN.slices.clone() } else { visible });
            effect.reads.extend(_tracked(&procedure.unit));
        }
        if effect.unknown_write {
            let visible = _whole(&actual, &facts.escaped_before.get(&at).unwrap_or_default());
            effect.writes.extend(if visible.is_empty() { UNKNOWN.slices.clone() } else { visible });
            effect.writes.extend(_tracked(&procedure.unit));
        }
        let fills = _fills(&procedure.unit, &facts, at);
        out.insert(at, Effect { loads: effect.reads.iter().map(reference).collect(), stores: effect.writes.iter().map(reference).collect(), fills });
    }
    Ok(out)
}

/// The bytes the call `at` writes through an argument before it reads
/// any: `initializes` at the site or on the callee's parameter.
fn _fills(unit: &Unit, facts: &PointsTo, at: InstId) -> Vec<MemRef> {
    let op = unit.function.instruction(at);
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &op.opcode else { return Vec::new() };
    let declared = llrm_mir::memory::callee(unit.context, unit.function, at).and_then(|one| unit.globals.get(one.0 as usize)).and_then(|one| one.function());
    let mut out = Vec::new();
    for (argument, &operand) in op.operands.iter().enumerate().take(info.argument_attrs.len()) {
        let declared = declared.and_then(|one| one.parameter_attrs.get(argument)).map_or(&[][..], |attrs| llrm_mir::memory::initializes(attrs));
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

fn _with(ranges: &Ranges, low: i64, high: i64) -> Ranges {
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

fn _without(ranges: &Ranges, low: i64, high: i64) -> Ranges {
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

fn _common(one: &Ranges, other: &Ranges) -> Ranges {
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
    match (&one.object.kind, &one.object.identity) {
        (MemoryKind::Parameter, Some(Identity::Int(index))) => usize::try_from(*index).ok(),
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
pub fn initialized(procedure: &Procedure, known: &IndexMap<String, Summary>) -> Result<Vec<Ranges>, String> {
    let unit = &procedure.unit;
    let function = unit.function;
    let count = function.parameters().len();
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
    let read = |state: &mut State, one: &Slice| {
        for (index, ranges) in state.iter_mut().enumerate() {
            let parameter = MemoryObject { identity: Some(Identity::Int(index as i64)), ..MemoryObject::new(MemoryKind::Parameter) };
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
                for one in effect.loads.iter().filter_map(|one| one.provenance.as_ref()).flat_map(|one| one.slices.iter()) {
                    read(&mut state, one);
                }
                let (Opcode::Call(info) | Opcode::Invoke(info)) = &op.opcode else { continue };
                let declared = llrm_mir::memory::callee(unit.context, function, inst).and_then(|one| unit.globals.get(one.0 as usize)).and_then(|one| one.function());
                for (argument, operand) in op.operands.iter().enumerate().take(info.argument_attrs.len()) {
                    let mut ranges = llrm_mir::memory::initializes(&info.argument_attrs[argument]).to_vec();
                    ranges.extend(declared.and_then(|one| one.parameter_attrs.get(argument)).map_or(&[][..], |attrs| llrm_mir::memory::initializes(attrs)));
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
        match function.instruction(*function.block(cfg::block(at)).instructions().last().expect("a block ends")).opcode {
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
    Ok(first.into_iter().map(|one| one.unwrap_or_default().into_iter().filter(bounded).collect()).collect())
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
    let slices = provenance.slices.iter().filter_map(|one| Slice::every_byte(one.object.clone())).collect();
    Provenance { slices, restrict: provenance.restrict.clone() }
}

fn _cell_key(reference: &MemRef) -> Option<CellKey> {
    if let Some(provenance) = &reference.provenance {
        if provenance.slices.len() == 1 {
            let one = provenance.slices.first().expect("one slice");
            if one.stride == 1 {
                return Some(CellKey::Object(one.object.clone(), one.low, one.high));
            }
        }
    }
    reference.addr().map(|addr| CellKey::Address(addr, i64::from(reference.width)))
}

/// What `inst` computes as a pointer from what it is given: an object's
/// own address, or a known pointer moved, cast or joined.
fn _direct(unit: &Unit, inst: InstId, values: &IndexMap<ValueId, Provenance>) -> Result<Option<Provenance>, String> {
    let op = unit.function.instruction(inst);
    let Some(result) = op.result.filter(|&result| is_pointer(unit, Operand::Value(result))) else {
        return Ok(None);
    };
    match &op.opcode {
        Opcode::Alloca { .. } => {
            let object = object_of(unit, Operand::Value(result)).expect("an alloca is an object");
            Provenance::one_with_slice(object, 0, 1, 1, 1, BTreeSet::new()).map(Some).map_err(|error| error.to_string())
        }
        // A segment is no pointer to a program object: `segment:0` is a
        // new root.
        Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) if unit.space(op.operands[0]) != Some(2) => Ok(_operand(unit, op.operands[0], values)),
        Opcode::GetElementPtr { source } => {
            let Some(fact) = _operand(unit, op.operands[0], values) else {
                return Ok(None);
            };
            let indices = op.operands[1..].iter().map(|&one| unit.int_constant(one).map(|bits| llrm_mir::context::signed(bits, unit.int_bits(one).unwrap_or(128)))).collect::<Vec<_>>();
            let (constant, variable) = unit.layout.collect_offset(&unit.context.types, *source, &indices);
            if variable.is_empty() {
                // A displacement is an index-width integer: -16 is never 65520.
                let space = unit.space(op.operands[0]).unwrap_or(0);
                return Ok(Some(fact.shifted(wrapped(constant, unit.layout.pointer(space).index_bits))));
            }
            // Arithmetic by an unknown integer remains within each known
            // object, but no longer has a byte offset precise enough to compare.
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
fn read_values(unit: &Unit, inst: InstId) -> impl Iterator<Item = ValueId> {
    let op = unit.function.instruction(inst);
    let mut read = op.operands.iter().filter_map(|one| if let Operand::Value(value) = one { Some(*value) } else { None }).collect::<Vec<_>>();
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

/// Flow pointer objects through values, exact spill slots and CFG joins.
pub fn points_to(
    unit: &Unit,
    arguments: Option<&IndexMap<InstId, Vec<Actual>>>,
    captures: Option<&IndexMap<InstId, Option<BTreeSet<Option<Identity>>>>>,
) -> Result<PointsTo, String> {
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
    let mut incoming = graph.iter().map(|block| (block.at, IndexMap::<CellKey, Provenance>::default())).collect::<IndexMap<_, _>>();
    let mut outgoing = incoming.clone();
    let instructions = |at: i64| function.block(cfg::block(at)).instructions();

    // A block reads its parents' cells and these values. With neither
    // changed since its last visit it would compute what it already holds,
    // widening included, as widening an object's whole slice gives the same slice.
    let reads = graph.iter().map(|block| instructions(block.at).iter().flat_map(|&inst| read_values(unit, inst)).collect::<HashSet<_>>()).collect::<Vec<_>>();
    let tick = std::cell::Cell::new(0_u64);
    let touched = RefCell::new(HashMap::<ValueId, u64>::default());
    // Every pointer stored anywhere in each object: what a cell of it may
    // hold when its exact contents are not known.
    let mut fields = HashMap::<MemoryObject, Provenance>::default();
    // Objects a call or an unknown value may have written: no such bound.
    let mut unbounded = HashSet::<MemoryObject>::default();
    let mut sent = HashMap::<i64, u64>::default();
    let mut visited = vec![None::<u64>; graph.len()];
    loop {
        let changed = std::cell::Cell::new(false);
        let learn = |values: &mut IndexMap<ValueId, Provenance>, value: ValueId, fact: Provenance| {
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
                if !parents_at.iter().any(|parent| since(sent.get(parent))) && !reads[index].iter().any(|value| since(touched.get(value))) {
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
                        // A missing fact on one incoming edge is unknown, not an
                        // invitation to retain the other edge's pointer.
                        if parents.iter().all(|one| one.contains_key(&key)) {
                            let mut fact = _union(parents.iter().map(|one| one.get(&key))).expect("every parent holds this key");
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
                        let mut fact = if parts.iter().any(Option::is_none) { Some(UNKNOWN.clone()) } else { _union(parts.iter().map(Option::as_ref)) };
                        if let Some(current) = &fact {
                            let carried = op.opcode == Opcode::Phi
                                && op.operands.iter().skip(1).step_by(2).any(|parent| matches!(parent, Operand::Block(parent) if back_edges.contains(&(cfg::id(*parent), block.at))));
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
                    (Opcode::Load { .. }, Some(reference)) if op.result.is_some_and(|result| pointer_values.contains(&result)) => {
                        let resolved = MemRef { provenance: _resolved_reference(unit, &reference, &values), ..reference.clone() };
                        let loaded = _cell_key(&resolved).and_then(|key| state.get(&key).cloned());
                        let loaded = loaded.or_else(|| {
                            let stored = resolved
                                .provenance
                                .iter()
                                .flat_map(|provenance| provenance.slices.iter())
                                .map(|one| (!unbounded.contains(&one.object)).then(|| fields.get(&one.object)).flatten())
                                .collect::<Option<Vec<_>>>()?;
                            (!stored.is_empty()).then(|| _union(stored.into_iter().map(Some).chain([Some(&*UNKNOWN)])))?
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
                                if unbounded.insert(one.object.clone()) {
                                    changed.set(true);
                                }
                            }
                        }
                        if let (Some(source), Some(targets)) = (&source, &keyed.provenance) {
                            for one in &targets.slices {
                                // Whole objects: stored offsets may shift each trip around a loop.
                                let grown = _widened(&fields.get(&one.object).map_or_else(|| source.clone(), |held| held.union(source)));
                                if fields.get(&one.object) != Some(&grown) {
                                    fields.insert(one.object.clone(), grown);
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

    // Escape is flow-sensitive separately from pointer contents. A pointer
    // published after a call must not make the earlier call reach its frame.
    let mut pointer_fields: IndexMap<MemoryObject, BTreeSet<MemoryObject>> = IndexMap::default();
    for (_, inst) in function.walk() {
        let op = function.instruction(inst);
        let (Opcode::Store { .. }, Some(reference)) = (&op.opcode, MemRef::of(unit, inst)) else { continue };
        let Some(source) = _operand(unit, op.operands[0], &values) else { continue };
        let targets = _resolved_reference(unit, &reference, &values).into_iter().flat_map(|provenance| provenance.slices.into_iter().map(|one| one.object)).collect::<BTreeSet<_>>();
        for target in targets {
            pointer_fields.entry(target).or_default().extend(source.slices.iter().map(|one| one.object.clone()));
        }
    }

    // Objects numbered once, so closures and unions compare indices instead
    // of identity trees.
    let objects = RefCell::new(IndexSet::<MemoryObject>::default());
    let number = |object: &MemoryObject| {
        let mut objects = objects.borrow_mut();
        match objects.get_index_of(object) {
            Some(index) => index,
            None => objects.insert_full(object.clone()).0,
        }
    };
    let pointer_fields = pointer_fields.iter().map(|(target, sources)| (number(target), sources.iter().map(number).collect::<Vec<_>>())).collect::<HashMap<_, _>>();
    // Close publication through pointer-valued fields of known objects.
    let pointees = |objects: BTreeSet<MemoryObject>, cells: &IndexMap<CellKey, Provenance>| {
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

    // What each instruction publishes does not depend on what reached it,
    // so it is found once; only the unions along edges iterate: the
    // gen/kill form of a forward dataflow.
    let calls = call_sites(unit).into_iter().collect::<BTreeSet<_>>();
    let provenances = |operands: &[Operand], values: &IndexMap<ValueId, Provenance>| {
        operands.iter().filter_map(|&one| _operand(unit, one, values)).flat_map(|one| one.slices.into_iter().map(|slice| slice.object)).collect::<Vec<_>>()
    };
    let mut publishes: IndexMap<i64, Vec<Vec<usize>>> = IndexMap::default();
    for block in &graph {
        let mut cells = CellMap::new(incoming[&block.at].clone(), _key_place);
        let mut mine = Vec::new();
        for &inst in instructions(block.at) {
            let op = function.instruction(inst);
            let mut newly = BTreeSet::new();
            // What a call reads a pointer out of, it may keep.
            let mut lent = BTreeSet::new();
            if calls.contains(&inst) {
                if let Some(arguments) = arguments {
                    let actual = _resolved_actuals(arguments.get(&inst).map_or(&[][..], Vec::as_slice), &values);
                    let kept = |index: usize| -> Result<bool, String> {
                        match captures.and_then(|captures| captures.get(&inst)).and_then(Option::as_ref) {
                            None => Ok(!_borrowed(unit, inst, index)),
                            Some(selected) => Ok(selected.iter().map(_index).collect::<Result<Vec<_>, _>>()?.contains(&(index as i64))),
                        }
                    };
                    // Nor can it read a pointer out of what it may only write.
                    let through = _allowed(unit, inst).through;
                    let reads = |index: usize| through.get(index).is_none_or(|one| one.reads);
                    for (index, one) in actual.iter().enumerate() {
                        let objects = one.slices.iter().map(|one| one.object.clone());
                        if kept(index)? {
                            newly.extend(objects)
                        } else if reads(index) {
                            lent.extend(objects)
                        }
                    }
                } else {
                    // The callee's own address is no argument.
                    let count = op.operands.len() - if matches!(op.opcode, Opcode::Invoke(_)) { 3 } else { 1 };
                    let kept = (0..count).filter(|&index| !_borrowed(unit, inst, index)).map(|index| op.operands[index]).collect::<Vec<_>>();
                    newly.extend(provenances(&kept, &values));
                }
            }
            newly.extend(_lost(unit, inst, &values));
            // Returned, or turned into an integer: found from outside.
            if matches!(op.opcode, Opcode::Ret | Opcode::Cast(CastOp::PtrToInt)) {
                newly.extend(provenances(&op.operands, &values));
            }
            match (&op.opcode, MemRef::of(unit, inst)) {
                (Opcode::Store { .. }, Some(reference)) => {
                    let destination = _resolved_reference(unit, &reference, &values);
                    let outside = destination.as_ref().is_none_or(|provenance| provenance.slices.iter().any(|one| one.object.kind != MemoryKind::Frame));
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
                // A call may leave a cell as it was: what it held stays reachable.
                _ if unmodeled_write(unit, inst) => {}
                _ => {}
            }
            let lent_numbers = lent.iter().map(number).collect::<HashSet<_>>();
            let mut published = pointees(newly, &cells);
            published.extend(pointees(lent, &cells).into_iter().filter(|one| !lent_numbers.contains(one)));
            mine.push(published);
        }
        publishes.insert(block.at, mine);
    }
    let objects = objects.into_inner();
    let bits_of = |escapes: &Vec<usize>| {
        let mut bits = Bits::new(objects.len());
        escapes.iter().for_each(|one| bits.insert(*one));
        bits
    };
    let generated = publishes
        .iter()
        .map(|(at, mine)| {
            let mut all = Bits::new(objects.len());
            mine.iter().for_each(|escapes| all.union_with(&bits_of(escapes)));
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
            state.union_with(&bits_of(escapes));
            if calls.contains(&inst) || matches!(function.instruction(inst).opcode, Opcode::Load { .. } | Opcode::Store { .. }) {
                before.insert(inst, state.clone());
            }
        }
    }
    let mut every = Bits::new(objects.len());
    out.values().for_each(|one| every.union_with(one));
    let escaped = named(&every);
    let escaped_before = EscapedBefore { objects: Rc::new(objects), at: before };
    Ok(PointsTo { values, escaped, escaped_before })
}

/// Objects whose address `inst` turns into what no pointer fact follows,
/// which LLVM's capture tracking counts a capture: a pointer operand whose
/// provenance neither the result nor the access carries, and a global in
/// a constant that is no pointer. A comparison captures nothing; what a
/// call, a return, a store and a `ptrtoint` publish, their own rules say.
fn _lost(unit: &Unit, inst: InstId, values: &IndexMap<ValueId, Provenance>) -> Vec<MemoryObject> {
    let op = unit.function.instruction(inst);
    let carried = match &op.opcode {
        Opcode::ICmp(_) => return Vec::new(),
        Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) => unit.space(op.operands[0]) != Some(2),
        Opcode::Load { .. } | Opcode::Store { .. } | Opcode::GetElementPtr { .. } | Opcode::Phi | Opcode::Select => true,
        Opcode::Call(_) | Opcode::Invoke(_) | Opcode::Ret | Opcode::Cast(CastOp::PtrToInt) => true,
        _ => false,
    };
    let mut out = Vec::new();
    for &operand in &op.operands {
        if is_pointer(unit, operand) {
            if !carried {
                out.extend(_operand(unit, operand, values).into_iter().flat_map(|one| one.slices.into_iter().map(|slice| slice.object)));
            }
        } else if let Operand::Constant(id) = operand {
            let mut held = BTreeSet::new();
            globalsaa::embedded(unit.context, id, &mut held);
            out.extend(held.into_iter().filter_map(|one| memory::global_object(unit, one)));
        }
    }
    out
}

fn _resolved_actuals(actuals: &[Actual], values: &IndexMap<ValueId, Provenance>) -> Vec<Provenance> {
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
    Object(MemoryObject),
    Address(Operand),
}

impl Bucket for KeyBucket {
    // Nothing looks a key bucket up by a component.
    type Parts = ();

    fn held(&self, _: &mut ()) {}

    fn released(&self, _: &mut ()) {}
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
pub fn _kill<V>(cells: &mut CellMap<CellKey, V, KeyBucket>, key: Option<&CellKey>) {
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

pub fn _keys_overlap(one: Option<&CellKey>, other: Option<&CellKey>) -> bool {
    let (Some(one), Some(other)) = (one, other) else {
        return true;
    };
    match (one, other) {
        (CellKey::Address(addr, width), CellKey::Address(other_addr, other_width)) if addr.root == other_addr.root => {
            addr.disp < other_addr.disp + other_width && other_addr.disp < addr.disp + width
        }
        (CellKey::Object(object, low, high), CellKey::Object(other_object, other_low, other_high)) if object == other_object => low < other_high && other_low < high,
        _ => one == other,
    }
}

/// `value` modulo `modulus`, never negative for a positive modulus.
fn mod_floor(value: &BigInt, modulus: &BigInt) -> BigInt {
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
pub fn congruences_with(unit: &Unit, constants: &IndexMap<ValueId, Known>) -> IndexMap<ValueId, (BigInt, BigInt)> {
    let function = unit.function;
    let mut result = IndexMap::<ValueId, (BigInt, BigInt)>::default();
    let zero = BigInt::from(0);
    for loop_ in &unit.shape().loops {
        for affine in induction::basics(unit, loop_).values() {
            let width = affine.start.width();
            let (Some(start), Some(step)) = (induction::_signed(&affine.start, &constants, width), induction::_signed(&affine.step, &constants, width)) else {
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
            let (Opcode::Binary(kind), [left, right], Some(width)) = (&op.opcode, op.operands.as_slice(), unit.int_bits(Operand::Value(value))) else { continue };
            let fact = |one: Operand| match one {
                Operand::Value(source) => result.get(&source).cloned().or_else(|| constants.get(&source).map(|known| (BigInt::from(0), known.n.clone()))),
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
fn reduced(modulus: BigInt, residue: BigInt, width: u32) -> (BigInt, BigInt) {
    let whole = BigInt::from(1) << width;
    let modulus = induction::gcd(if modulus < BigInt::from(0) { -modulus } else { modulus }, whole.clone());
    if modulus == whole { (BigInt::from(0), mod_floor(&residue, &whole)) } else { (modulus.clone(), mod_floor(&residue, &modulus)) }
}

/// Attach solved provenance to every access of the function: each load's
/// and store's, narrowed where a range bounds its index.
pub fn annotated(unit: &Unit) -> Result<IndexMap<InstId, MemRef>, String> {
    annotated_with(unit, &*unit.pointers()?, &unit.registers())
}

/// `annotated`, given the points-to facts and what `consts::known` finds
/// without memory.
pub fn annotated_with(unit: &Unit, facts: &PointsTo, known: &IndexMap<ValueId, Known>) -> Result<IndexMap<InstId, MemRef>, String> {
    let bounded = ranges::bounded_with(unit, known)?;
    let strides = congruences_with(unit, known);
    let constants = ranges::intervals(known);

    let tag = |reference: &MemRef, at: i64| -> Result<MemRef, String> {
        let mut got = facts.reference(unit, reference);
        let interval = reference.base.and_then(|base| bounded.get(&at).and_then(|known| known.get(&base)).or_else(|| constants.get(&base)));
        if let (Some(current), true, Some(base), Some(interval)) = (&got, reference.object, reference.base, interval) {
            if interval.width == reference.base_width && current.slices.len() == 1 && reference.scale > 0 {
                let source = current.slices.first().expect("one slice");
                let (modulus, residue) = strides.get(&base).cloned().unwrap_or((BigInt::from(1_u8), BigInt::from(0_u8)));
                let modulus = if modulus > BigInt::from(1_u8) { modulus } else { BigInt::from(1_u8) };
                let first = &interval.low + mod_floor(&(residue - &interval.low), &modulus);
                let width = i64::from(reference.width.max(1));
                let low = BigInt::from(reference.disp) + first * reference.scale;
                let high = BigInt::from(reference.disp) + &interval.high * reference.scale + 1;
                let end = &high + width - 1;
                let stride = modulus * reference.scale;
                let zero = BigInt::from(0_u8);
                if low < high && source.object.extent.is_none_or(|extent| zero <= low && end <= BigInt::from(extent)) {
                    let model = |number: &BigInt| i64::try_from(number).map_err(|_| format!("slice bound {number} exceeds the i64 slice model"));
                    got = Some(Provenance {
                        slices: BTreeSet::from([Slice::new(source.object.clone(), model(&low)?, model(&high)?, model(&stride)?, width)
                            .expect("a nonempty positive-stride slice is valid")]),
                        restrict: current.restrict.clone(),
                    });
                }
            }
        }
        // A far access whose selector's range lands it in foreign memory
        // names those linear bytes, whatever its pointer's provenance.
        if let (None, Some(Operand::Value(segment))) = (reference.selector, reference.segment) {
            let lookup = |value: ValueId| bounded.get(&at).and_then(|known| known.get(&value)).or_else(|| constants.get(&value)).map(|one| (value, one.clone()));
            let known = [Some(segment), reference.base].into_iter().flatten().filter_map(lookup).collect();
            if let Some(foreign) = regions::foreign_provenance(reference, &known, unit.program) {
                got = Some(foreign);
            }
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
