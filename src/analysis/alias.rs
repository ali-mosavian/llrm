//! Strong, flow-sensitive alias analysis over MIR.
//!
//! Direct port of `qbopt/analysis/alias.py`.  The analysis has one
//! vocabulary for every frontend: canonical objects, subobject byte slices,
//! pointer provenance and C restrict roots.  Pointer facts flow through SSA
//! phis and through exact pointer spill slots.  Unknown stores kill spill
//! facts; they never manufacture a disjointness proof.

use std::rc::Rc;
use std::collections::BTreeSet;
use std::sync::LazyLock;

use crate::support::hash::{IndexMap, IndexSet};
use num_bigint::BigInt;

use super::cellmap::{Bucket, CellMap};
use super::regions::ByteRange;
use super::consts::{self, Known};
use super::{induction, loops, ranges};
use crate::model::memory::{self, Identity, MemoryKind, MemoryObject, Provenance, Slice};
use crate::model::mir::{self, Arg, Cell, FrameAddress, Held, Kind, MemRef, MirBody, Op, Symbol, Value};
use crate::objectfile::module::{Addr, Space};
use crate::support::pyrepr::Repr;

pub static UNKNOWN: LazyLock<Provenance> =
    LazyLock::new(|| Provenance::one(MemoryObject::new(MemoryKind::Unknown)));
pub static NONLOCAL: LazyLock<Provenance> =
    LazyLock::new(|| Provenance::one(MemoryObject::new(MemoryKind::Nonlocal)));
pub const EMPTY: Provenance = Provenance {
    slices: BTreeSet::new(),
    restrict: BTreeSet::new(),
};

/// One element of Python's `Procedure.arguments` tuples, which hold a
/// provenance, a `(pointer value, displacement)` pair or `None`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Actual {
    Provenance(Provenance),
    Pointer(Value, i64),
    Absent,
}

/// Python's `_cell_key` tuples: `(object, low, high)` or
/// `(space, index, disp, width)`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum CellKey {
    Object(MemoryObject, i64, i64),
    Address(Space, i64, i64, i64),
}

/// Whole objects an unknown callee can reach through pointers it owns.
fn _whole<'a>(
    provenances: impl IntoIterator<Item = &'a Provenance>,
    escaped: &BTreeSet<MemoryObject>,
) -> Result<BTreeSet<Slice>, String> {
    let mut objects = provenances
        .into_iter()
        .flat_map(|provenance| provenance.slices.iter().map(|one| one.object.clone()))
        .collect::<BTreeSet<_>>();
    objects.extend(escaped.iter().cloned());
    objects
        .into_iter()
        .map(|object| match object.extent {
            Some(extent) => Slice::new(object, 0, extent, 1, 1).map_err(|error| error.to_string()),
            None => Ok(Slice::whole(object)),
        })
        .collect()
}

/// Python `qbopt.analysis.alias:PointsTo`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointsTo {
    pub values: IndexMap<Value, Provenance>,
    pub escaped: BTreeSet<MemoryObject>,
    /// Objects visible immediately before each source operation address.
    pub escaped_before: IndexMap<i64, BTreeSet<MemoryObject>>,
}

impl PointsTo {
    /// Canonical bytes reached by a reference through an analysed value.
    pub fn reference(&self, reference: &MemRef) -> Option<Provenance> {
        _resolved_reference(reference, &self.values)
    }

    /// Whether `value` can only designate a real static or frame object.
    ///
    /// Incoming pointers and allocation results remain nullable.  A current
    /// frame object or a linked object symbol is non-null by the source
    /// language contract even though its eventual 16-bit offset is not known
    /// until link time.
    pub fn nonnull(&self, value: Value) -> bool {
        let Some(provenance) = self.values.get(&value) else {
            return false;
        };
        !provenance.slices.is_empty()
            && provenance.slices.iter().all(|one| {
                matches!(
                    one.object.kind,
                    MemoryKind::Frame | MemoryKind::Global | MemoryKind::External | MemoryKind::Named
                )
            })
    }
}

/// Resolve a memory operand through the current pointer-value facts.
fn _resolved_reference(reference: &MemRef, values: &IndexMap<Value, Provenance>) -> Option<Provenance> {
    let mut derived = None;
    // An unannotated computed address has always acquired its identity from
    // its SSA base, whether or not the source needed to mark the operand as a
    // first-class pointer.  `pointer` matters only when refining an existing
    // conservative frontend annotation: ordinary indexed references must not
    // let an unrelated arithmetic base contradict their concrete object.
    let derive = reference.provenance.is_none() || reference.pointer;
    if let Some(source) = reference.base.filter(|_| derive).and_then(|base| values.get(&base)) {
        let displacement = reference.addr.map_or(0, |addr| addr.disp);
        // A singleton address names `width` consecutive bytes. A set of
        // indexed addresses retains its stride and widens its final lane.
        let slices = source
            .slices
            .iter()
            .map(|one| {
                Slice::new(
                    one.object.clone(),
                    one.low + displacement,
                    one.high + displacement,
                    one.stride,
                    i64::from(reference.width.max(1)),
                )
                .expect("a displaced slice keeps its positive shape")
            })
            .collect();
        derived = Some(Provenance {
            slices,
            restrict: source.restrict.clone(),
        });
    }
    let Some(attached) = &reference.provenance else {
        return derived;
    };
    let Some(derived) = derived else {
        return Some(attached.clone());
    };

    // The operand annotation is allowed to be a conservative source spelling;
    // the SSA pointer is the address actually dereferenced.  Prefer a concrete
    // object solved from that value over UNKNOWN/NONLOCAL/PARAMETER placeholders.
    // If two concrete claims disagree, retain both instead of manufacturing a
    // disjointness proof from inconsistent metadata.
    let abstract_ = [MemoryKind::Unknown, MemoryKind::Nonlocal, MemoryKind::Parameter];
    let attached_objects = attached.slices.iter().map(|one| &one.object).collect::<BTreeSet<_>>();
    let derived_objects = derived.slices.iter().map(|one| &one.object).collect::<BTreeSet<_>>();
    let attached_concrete =
        !attached_objects.is_empty() && attached_objects.iter().all(|one| !abstract_.contains(&one.kind));
    let derived_concrete =
        !derived_objects.is_empty() && derived_objects.iter().all(|one| !abstract_.contains(&one.kind));
    if derived_concrete && !attached_concrete {
        return Some(derived);
    }
    if attached_concrete && !derived_concrete {
        return Some(attached.clone());
    }
    if attached_concrete && derived_concrete && attached_objects == derived_objects {
        return Some(derived);
    }
    Some(attached.union(&derived))
}

/// One procedure's transitive memory effects in its own object space.
///
/// Python `qbopt.analysis.alias:Summary`.  `captures` holds PARAMETER object
/// identities, which Python does not restrict to ints.
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
        }
    }
}

/// Python `qbopt.analysis.alias:Procedure`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Procedure {
    pub body: MirBody,
    pub calls: IndexMap<i64, String>,
    pub arguments: IndexMap<i64, Vec<Actual>>,
}

fn _actuals(procedure: &Procedure, facts: &PointsTo, at: i64) -> Vec<Provenance> {
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

fn _direct_summary(body: &MirBody) -> Result<Summary, String> {
    let (mut reads, mut writes) = (BTreeSet::new(), BTreeSet::new());
    let (mut unknown_read, mut unknown_write) = (false, false);
    let facts = points_to(body, None, None)?;
    for block in &body.blocks {
        for op in &block.ops {
            if op.kind == Kind::Call {
                continue;
            }
            let references = op
                .loads
                .iter()
                .map(|reference| (reference, true))
                .chain(op.stores.iter().map(|reference| (reference, false)));
            for (reference, read) in references {
                let Some(provenance) = facts.reference(reference) else {
                    if read {
                        unknown_read = true;
                    } else {
                        unknown_write = true;
                    }
                    continue;
                };
                for one in provenance.slices {
                    let unknown = one.object.kind == MemoryKind::Unknown;
                    if !matches!(one.object.kind, MemoryKind::Frame | MemoryKind::Stack) {
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
        }
    }
    let captures = facts
        .escaped
        .iter()
        .filter(|one| one.kind == MemoryKind::Parameter && matches!(one.identity, Some(Identity::Int(_))))
        .map(|one| one.identity.clone())
        .collect();
    Ok(Summary {
        reads,
        writes,
        captures,
        unknown_read,
        unknown_write,
    })
}

fn _recursive_edges(procedures: &IndexMap<String, Procedure>) -> BTreeSet<(String, String)> {
    let graph = procedures
        .iter()
        .map(|(name, procedure)| {
            let targets = procedure
                .calls
                .values()
                .filter(|target| procedures.contains_key(*target))
                .collect::<BTreeSet<_>>();
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
        .flat_map(|(caller, targets)| {
            targets
                .iter()
                .filter(|callee| reaches(callee, caller))
                .map(|callee| ((*caller).clone(), (*callee).clone()))
        })
        .collect()
}

fn _widen_parameters(summary: Summary) -> Summary {
    let widened = |items: &BTreeSet<Slice>| {
        items
            .iter()
            .map(|one| {
                if one.object.kind == MemoryKind::Parameter {
                    Slice::whole(one.object.clone())
                } else {
                    one.clone()
                }
            })
            .collect()
    };

    Summary {
        reads: widened(&summary.reads),
        writes: widened(&summary.writes),
        ..summary
    }
}

/// Drop subranges once the same object already has a whole-object effect.
fn _coalesced(items: &BTreeSet<Slice>) -> BTreeSet<Slice> {
    let is_whole = |one: &Slice| {
        one.low == memory::WHOLE_LOW && one.high == memory::WHOLE_HIGH && one.stride == 1 && one.width == 1
    };
    let whole = items
        .iter()
        .filter(|one| is_whole(one))
        .map(|one| &one.object)
        .collect::<BTreeSet<_>>();
    items
        .iter()
        .filter(|one| !whole.contains(&one.object) || is_whole(one))
        .cloned()
        .collect()
}

/// Transitive per-procedure mod/ref and capture summaries to a fixed point.
///
/// `known` supplies established external semantics, such as C library
/// functions. A body in this compilation unit always takes precedence.
pub fn summaries(
    procedures: &IndexMap<String, Procedure>,
    known: Option<&IndexMap<String, Summary>>,
) -> Result<IndexMap<String, Summary>, String> {
    let direct = procedures
        .iter()
        .map(|(name, one)| Ok((name.clone(), _direct_summary(&one.body)?)))
        .collect::<Result<Vec<_>, String>>()?;
    let mut result = known.cloned().unwrap_or_default();
    result.extend(direct);
    let recursive = _recursive_edges(procedures);
    loop {
        let mut changed = false;
        for (name, procedure) in procedures {
            let captured_at = procedure
                .calls
                .iter()
                .map(|(at, target)| (*at, result.get(target).map(|one| one.captures.clone())))
                .collect::<IndexMap<_, _>>();
            let facts = points_to(&procedure.body, Some(&procedure.arguments), Some(&captured_at))?;
            let direct = _direct_summary(&procedure.body)?;
            let (mut reads, mut writes, mut captures) = (direct.reads, direct.writes, direct.captures);
            let (mut unknown_read, mut unknown_write) = (direct.unknown_read, direct.unknown_write);
            for block in &procedure.body.blocks {
                for op in &block.ops {
                    if op.kind != Kind::Call {
                        continue;
                    }
                    let target = procedure.calls.get(&op.at);
                    let callee = target.and_then(|target| result.get(target));
                    let actual = _actuals(procedure, &facts, op.at);
                    let Some(callee) = callee else {
                        let mut visible = NONLOCAL.slices.clone();
                        let empty = BTreeSet::new();
                        visible.extend(_whole(&actual, facts.escaped_before.get(&op.at).unwrap_or(&empty))?);
                        reads.extend(visible.iter().cloned());
                        writes.extend(visible);
                        captures.extend(
                            actual
                                .iter()
                                .flat_map(|provenance| provenance.slices.iter())
                                .filter(|slice| slice.object.kind == MemoryKind::Parameter)
                                .map(|slice| slice.object.identity.clone()),
                        );
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
                    for index in &effect.captures {
                        let index = match index {
                            Some(Identity::Int(index)) => *index,
                            // Python's `0 <= index` raises for any other identity.
                            other => {
                                let name = match other {
                                    None => "NoneType",
                                    Some(Identity::Str(_)) => "str",
                                    Some(Identity::Space(_)) => "Space",
                                    Some(Identity::Storage(_)) => "Storage",
                                    Some(Identity::Symbol(_)) => "Symbol",
                                    Some(Identity::Value(_)) => "Value",
                                    Some(Identity::Tuple(_)) => "tuple",
                                    Some(Identity::Int(_)) => unreachable!("matched above"),
                                };
                                return Err(format!("'<=' not supported between instances of 'int' and '{name}'"));
                            }
                        };
                        if 0 <= index && index < actual.len() as i64 {
                            captures.extend(
                                actual[index as usize]
                                    .slices
                                    .iter()
                                    .filter(|one| one.object.kind == MemoryKind::Parameter)
                                    .map(|one| one.object.identity.clone()),
                            );
                        }
                    }
                }
            }
            let made = Summary {
                reads: _coalesced(&reads),
                writes: _coalesced(&writes),
                captures,
                unknown_read,
                unknown_write,
            };
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

/// Instantiate callee effects through actual pointer provenance.
pub fn calls_annotated(procedure: &Procedure, known: &IndexMap<String, Summary>) -> Result<MirBody, String> {
    // Capture is part of escape flow. Unknown callees may retain every
    // pointer actual; known callees retain only the parameters their fixed
    // point summary says they capture.
    let captures = procedure
        .calls
        .iter()
        .map(|(at, target)| (*at, known.get(target).map(|one| one.captures.clone())))
        .collect::<IndexMap<_, _>>();
    let facts = points_to(&procedure.body, Some(&procedure.arguments), Some(&captures))?;

    let reference = |one: &Slice| {
        let mut made = MemRef::new(None, u32::try_from(one.width).expect("a slice width is a memory width"));
        made.provenance = Some(Provenance {
            slices: BTreeSet::from([one.clone()]),
            restrict: BTreeSet::new(),
        });
        made
    };

    let mut blocks = Vec::new();
    for block in &procedure.body.blocks {
        let mut ops = Vec::new();
        for op in &block.ops {
            if op.kind != Kind::Call {
                ops.push(op.clone());
                continue;
            }
            let actual = _actuals(procedure, &facts, op.at);
            let target = procedure.calls.get(&op.at);
            let mut effect = match target.and_then(|target| known.get(target)) {
                Some(callee) => callee.instantiated(&actual),
                None => {
                    let mut visible = NONLOCAL.slices.clone();
                    let empty = BTreeSet::new();
                    visible.extend(_whole(&actual, facts.escaped_before.get(&op.at).unwrap_or(&empty))?);
                    Summary {
                        reads: visible.clone(),
                        writes: visible,
                        ..Summary::default()
                    }
                }
            };
            let empty = BTreeSet::new();
            if effect.unknown_read {
                let visible = _whole(&actual, facts.escaped_before.get(&op.at).unwrap_or(&empty))?;
                effect.reads.extend(if visible.is_empty() { UNKNOWN.slices.clone() } else { visible });
            }
            if effect.unknown_write {
                let visible = _whole(&actual, facts.escaped_before.get(&op.at).unwrap_or(&empty))?;
                effect.writes.extend(if visible.is_empty() { UNKNOWN.slices.clone() } else { visible });
            }
            let sorted = |items: &BTreeSet<Slice>| {
                let mut items = items.iter().map(|one| (one.repr(), one)).collect::<Vec<_>>();
                items.sort_by(|left, right| left.0.cmp(&right.0));
                items.into_iter().map(|(_, one)| reference(one)).collect::<Vec<_>>()
            };
            let mut annotated = op.clone();
            annotated.loads = sorted(&effect.reads);
            annotated.stores = sorted(&effect.writes);
            annotated.memory_complete = true;
            ops.push(annotated);
        }
        blocks.push(block.with_ops(ops));
    }
    let mut body = procedure.body.clone();
    body.blocks = blocks;
    Ok(body)
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
/// every trip around the back edge manufactures another offset.  At a
/// natural-loop header, use the standard abstract-interpretation widening
/// instead.  Keeping object identity and restrict roots still proves the
/// important disjointness facts; only the changing subrange is forgotten.
fn _widened(provenance: &Provenance) -> Result<Provenance, String> {
    let slices = provenance
        .slices
        .iter()
        .map(|one| match one.object.extent {
            Some(extent) => Slice::new(one.object.clone(), 0, extent, 1, 1).map_err(|error| error.to_string()),
            None => Ok(Slice::whole(one.object.clone())),
        })
        .collect::<Result<_, _>>()?;
    Ok(Provenance {
        slices,
        restrict: provenance.restrict.clone(),
    })
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
    if let Some(addr) = reference.addr {
        if reference.base.is_none() && reference.segment.is_none() {
            return Some(CellKey::Address(addr.space, addr.index, addr.disp, i64::from(reference.width)));
        }
    }
    None
}

fn _direct(op: &Op, values: &IndexMap<Value, Provenance>) -> Result<Option<Provenance>, String> {
    if op.results.len() != 1 || !matches!(op.results[0], Arg::Held(_)) {
        return Ok(None);
    }
    let candidates = match (op.kind, op.args.as_slice()) {
        (
            Kind::Address,
            [
                Arg::FrameAddress(FrameAddress {
                    offset,
                    extent: Some((low, high)),
                    ..
                }),
            ],
        ) => {
            let object = MemoryObject {
                kind: MemoryKind::Frame,
                identity: Some(Identity::Tuple(vec![Identity::Int(*low), Identity::Int(*high)])),
                generation: 0,
                extent: Some(high - low),
                addressed: true,
                captured: true,
            };
            return Provenance::one_with_slice(object, offset - low, offset - low + 1, 1, 1, BTreeSet::new())
                .map(Some)
                .map_err(|error| error.to_string());
        }
        (Kind::Address, [Arg::Cell(Cell { r#ref })])
            if r#ref.provenance.as_ref().is_some_and(|provenance| !provenance.slices.is_empty()) =>
        {
            // Frontends with explicit object identities can spell address-of
            // as a canonical cell instead of reconstructing a FrameAddress.
            // The cell's provenance is the object being published, including
            // its complete bounded subobject rather than only the pointer-width
            // bytes used to encode the address operation.
            return Ok(r#ref.provenance.clone());
        }
        (
            Kind::Copy,
            [
                Arg::Symbol(Symbol {
                    space,
                    index,
                    offset,
                    addend,
                    ..
                }),
            ],
        ) => {
            let kind = if *space == Space::External {
                MemoryKind::External
            } else {
                MemoryKind::Global
            };
            let object = MemoryObject {
                kind,
                identity: Some(Identity::Tuple(vec![Identity::Space(*space), Identity::Int(*index)])),
                generation: 0,
                extent: None,
                addressed: true,
                captured: true,
            };
            return Provenance::one_with_slice(object, offset + addend, offset + addend + 1, 1, 1, BTreeSet::new())
                .map(Some)
                .map_err(|error| error.to_string());
        }
        (Kind::Copy, [Arg::Held(Held { value: source, .. })]) => return Ok(values.get(source).cloned()),
        (Kind::Extract, [Arg::Held(Held { value: source, .. }), Arg::Const(constant)])
            if constant.n == BigInt::from(0_u8) =>
        {
            // The low half of a far pointer is still its object-relative
            // offset.  Retain that identity while target lowering adjusts the
            // offset and later rejoins it with the unchanged selector.
            return Ok(values.get(source).cloned());
        }
        (Kind::Concat, [_, Arg::Held(Held { value: offset, .. })]) => {
            // Reconstituting selector:offset does not change the object named
            // by an offset whose provenance is already known.
            return Ok(values.get(offset).cloned());
        }
        (Kind::Add | Kind::PtrOffset, [left, right]) => vec![(left, right), (right, left)],
        (Kind::Sub, [left, right]) => vec![(left, right)],
        _ => return Ok(None),
    };
    for (pointer, amount) in candidates {
        let Arg::Held(pointer) = pointer else {
            continue;
        };
        let Some(fact) = values.get(&pointer.value) else {
            continue;
        };
        if let Arg::Const(amount) = amount {
            // Pointer displacements are ptrdiff values represented in the
            // operation's fixed-width integer.  Folding -16 into a 16-bit
            // ADD produces 65520; treating that spelling as a positive byte
            // offset loses exact provenance for cancellation chains such as
            // `base + 16 - 16`.
            if amount.width == 0 {
                return Err("negative shift count".to_owned());
            }
            let sign = BigInt::from(1_u8) << (amount.width * 8 - 1);
            let mut delta: BigInt = ((&amount.n & ((&sign << 1_u8) - 1)) ^ &sign) - &sign;
            if op.kind == Kind::Sub {
                delta = -delta;
            }
            let delta = i64::try_from(&delta)
                .map_err(|_| format!("pointer displacement {delta} exceeds the i64 slice model"))?;
            return Ok(Some(fact.shifted(delta)));
        }
        // Arithmetic by an unknown integer remains within each known object,
        // but no longer has a byte offset precise enough to compare.
        let slices = fact
            .slices
            .iter()
            .map(|one| match one.object.extent {
                Some(high) => Slice::new(one.object.clone(), 0, high, 1, 1).map_err(|error| error.to_string()),
                None => Ok(Slice::whole(one.object.clone())),
            })
            .collect::<Result<_, _>>()?;
        return Ok(Some(Provenance {
            slices,
            restrict: fact.restrict.clone(),
        }));
    }
    Ok(None)
}

/// Flow pointer objects through values, exact spill slots and CFG joins.
pub fn points_to(
    body: &MirBody,
    arguments: Option<&IndexMap<i64, Vec<Actual>>>,
    captures: Option<&IndexMap<i64, Option<BTreeSet<Option<Identity>>>>>,
) -> Result<PointsTo, String> {
    let mut values = body
        .pointer_seeds
        .iter()
        .map(|(value, provenance)| (*value, provenance.clone()))
        .collect::<IndexMap<_, _>>();
    let mut pointer_values = body
        .pointer_values
        .iter()
        .chain(values.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    let mut predecessors = body
        .blocks
        .iter()
        .map(|block| (block.at, BTreeSet::new()))
        .collect::<IndexMap<_, _>>();
    for block in &body.blocks {
        for successor in &block.succ {
            predecessors.entry(*successor).or_default().insert(block.at);
        }
    }
    // A pointer value is otherwise an exact byte slice.  Natural-loop joins
    // are the one place those exact facts can grow without a program bound.
    let dominators = loops::dominators(&body.blocks, Some(body.entry));
    let back_edges = body
        .blocks
        .iter()
        .flat_map(|block| block.succ.iter().map(move |successor| (block.at, *successor)))
        .filter(|(at, successor)| dominators.get(at).is_some_and(|dominating| dominating.contains(successor)))
        .collect::<BTreeSet<_>>();
    let mut incoming = body
        .blocks
        .iter()
        .map(|block| (block.at, IndexMap::<CellKey, Provenance>::default()))
        .collect::<IndexMap<_, _>>();
    let mut outgoing = incoming.clone();
    let none = BTreeSet::new();

    loop {
        let before_values = values.clone();
        let before_outgoing = outgoing.clone();
        for block in &body.blocks {
            let parents_at = predecessors.get(&block.at).unwrap_or(&none);
            let has_back_edge = parents_at.iter().any(|parent| back_edges.contains(&(*parent, block.at)));
            let mut state = IndexMap::default();
            {
                let parents = parents_at.iter().map(|one| &outgoing[one]).collect::<Vec<_>>();
                let previous_incoming = &incoming[&block.at];
                if !parents.is_empty() {
                    let keys = parents
                        .iter()
                        .flat_map(|one| one.keys())
                        .cloned()
                        .collect::<IndexSet<_>>();
                    for key in keys {
                        // A missing fact on one incoming edge is unknown, not an
                        // invitation to retain the other edge's pointer.
                        if parents.iter().all(|one| one.contains_key(&key)) {
                            let mut fact = _union(parents.iter().map(|one| one.get(&key)))
                                .expect("every parent holds this key");
                            if has_back_edge {
                                if let Some(previous) = previous_incoming.get(&key) {
                                    if fact != *previous {
                                        fact = _widened(&previous.union(&fact))?;
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
            for phi in &block.phis {
                let parts = phi
                    .incoming
                    .values()
                    .map(|one| values.get(one).cloned())
                    .collect::<Vec<_>>();
                if pointer_values.contains(&phi.result) || (!parts.is_empty() && parts.iter().all(Option::is_some)) {
                    let mut fact = if parts.iter().any(Option::is_none) {
                        Some(UNKNOWN.clone())
                    } else {
                        _union(parts.iter().map(Option::as_ref))
                    };
                    if let Some(current) = &fact {
                        if phi.incoming.keys().any(|parent| back_edges.contains(&(*parent, block.at))) {
                            if let Some(previous) = values.get(&phi.result) {
                                if current != previous {
                                    fact = Some(_widened(&previous.union(current))?);
                                }
                            }
                        }
                    }
                    if let Some(fact) = fact {
                        values.insert(phi.result, fact);
                        pointer_values.insert(phi.result);
                    }
                }
            }
            for op in &block.ops {
                let direct = _direct(op, &values)?;
                for result in &op.defines {
                    // ADDRESS and arithmetic derived from an already-known
                    // pointer prove their own pointer nature. Requiring the
                    // frontend side table to redundantly list every COPY/ADD
                    // result loses facts as soon as a MIR pass synthesizes or
                    // reparents one of those otherwise ordinary values.
                    if let Some(direct) = direct.as_ref().filter(|_| !body.pointer_seeds.contains_key(result)) {
                        values.insert(*result, direct.clone());
                        pointer_values.insert(*result);
                    }
                }
                if !op.loads.is_empty() && op.defines.len() == 1 && pointer_values.contains(&op.defines[0]) {
                    let loaded = _union(
                        op.loads
                            .iter()
                            .map(|reference| _cell_key(reference).and_then(|key| state.get(&key))),
                    );
                    if let Some(loaded) = loaded {
                        values.insert(op.defines[0], loaded);
                    }
                }
                if !op.stores.is_empty() {
                    let source = _union(op.args.iter().filter_map(|arg| match arg {
                        Arg::Held(held) => Some(values.get(&held.value)),
                        _ => None,
                    }));
                    for reference in &op.stores {
                        let mut keyed = reference.clone();
                        keyed.provenance = _resolved_reference(reference, &values);
                        let key = _cell_key(&keyed);
                        // Any possibly overlapping write invalidates prior cell
                        // contents; an exact pointer store then defines it.
                        _kill(&mut state, key.as_ref());
                        if let (Some(key), Some(source)) = (key, &source) {
                            state.insert(key, source.clone(), _key_place);
                        }
                    }
                }
            }
            outgoing.insert(block.at, state.into_items());
        }
        if values == before_values && outgoing == before_outgoing {
            break;
        }
    }

    // Escape is flow-sensitive separately from pointer contents. A pointer
    // published after a call must not make the earlier call reach its frame.
    let mut escape_in = body
        .blocks
        .iter()
        .map(|block| (block.at, BTreeSet::<MemoryObject>::new()))
        .collect::<IndexMap<_, _>>();
    let mut escape_out = escape_in.clone();
    let mut escaped_before: IndexMap<i64, BTreeSet<MemoryObject>> = IndexMap::default();
    let mut pointer_fields: IndexMap<MemoryObject, BTreeSet<MemoryObject>> = IndexMap::default();
    for block in &body.blocks {
        for op in &block.ops {
            if op.stores.is_empty() {
                continue;
            }
            let Some(source) = _union(op.args.iter().filter_map(|arg| match arg {
                Arg::Held(held) => Some(values.get(&held.value)),
                _ => None,
            })) else {
                continue;
            };
            let targets = op
                .stores
                .iter()
                .filter_map(|reference| _resolved_reference(reference, &values))
                .flat_map(|provenance| provenance.slices.into_iter().map(|one| one.object))
                .collect::<BTreeSet<_>>();
            for target in targets {
                pointer_fields
                    .entry(target)
                    .or_default()
                    .extend(source.slices.iter().map(|one| one.object.clone()));
            }
        }
    }

    // Close publication through pointer-valued fields of known objects.
    let pointees = |objects: BTreeSet<MemoryObject>, cells: &IndexMap<CellKey, Provenance>| {
        let mut reached = objects;
        loop {
            let before = reached.len();
            for (key, provenance) in cells {
                if let CellKey::Object(object, _, _) = key {
                    if reached.contains(object) {
                        reached.extend(provenance.slices.iter().map(|one| one.object.clone()));
                    }
                }
            }
            for object in reached.iter().cloned().collect::<Vec<_>>() {
                if let Some(fields) = pointer_fields.get(&object) {
                    reached.extend(fields.iter().cloned());
                }
            }
            if reached.len() == before {
                return reached;
            }
        }
    };

    loop {
        let before = escape_out.clone();
        for block in &body.blocks {
            let mut state = predecessors
                .get(&block.at)
                .unwrap_or(&none)
                .iter()
                .flat_map(|one| escape_out[one].iter().cloned())
                .collect::<BTreeSet<_>>();
            escape_in.insert(block.at, state.clone());
            let mut cells = CellMap::new(incoming[&block.at].clone(), _key_place);
            for op in &block.ops {
                let mut visible = escaped_before.get(&op.at).cloned().unwrap_or_default();
                visible.extend(state.iter().cloned());
                escaped_before.insert(op.at, visible);
                let mut newly = BTreeSet::new();
                if let Some(arguments) = arguments.filter(|_| op.kind == Kind::Call) {
                    let actual = _resolved_actuals(arguments.get(&op.at).map_or(&[][..], Vec::as_slice), &values);
                    match captures.and_then(|captures| captures.get(&op.at)).and_then(Option::as_ref) {
                        None => {
                            for one in &actual {
                                newly.extend(one.slices.iter().map(|one| one.object.clone()));
                            }
                        }
                        Some(selected) => {
                            for index in selected {
                                let index = match index {
                                    Some(Identity::Int(index)) => *index,
                                    // Python's `0 <= index` raises for any other identity.
                                    other => {
                                        let name = match other {
                                            None => "NoneType",
                                            Some(Identity::Str(_)) => "str",
                                            Some(Identity::Space(_)) => "Space",
                                            Some(Identity::Storage(_)) => "Storage",
                                            Some(Identity::Symbol(_)) => "Symbol",
                                            Some(Identity::Value(_)) => "Value",
                                            Some(Identity::Tuple(_)) => "tuple",
                                            Some(Identity::Int(_)) => unreachable!("matched above"),
                                        };
                                        return Err(format!(
                                            "'<=' not supported between instances of 'int' and '{name}'"
                                        ));
                                    }
                                };
                                if 0 <= index && index < actual.len() as i64 {
                                    newly.extend(actual[index as usize].slices.iter().map(|one| one.object.clone()));
                                }
                            }
                        }
                    }
                }
                if op.kind == Kind::Call {
                    for arg in &op.args {
                        if let Arg::Held(held) = arg {
                            if let Some(provenance) = values.get(&held.value) {
                                newly.extend(provenance.slices.iter().map(|one| one.object.clone()));
                            }
                        }
                    }
                }
                if matches!(op.kind, Kind::Return | Kind::Escape) {
                    for value in &op.uses {
                        if let Some(provenance) = values.get(value) {
                            newly.extend(provenance.slices.iter().map(|one| one.object.clone()));
                        }
                    }
                }
                if !op.stores.is_empty() {
                    let destinations = op
                        .stores
                        .iter()
                        .filter_map(|reference| _resolved_reference(reference, &values))
                        .collect::<Vec<_>>();
                    let outside = destinations.is_empty()
                        || destinations
                            .iter()
                            .flat_map(|provenance| provenance.slices.iter())
                            .any(|one| one.object.kind != MemoryKind::Frame);
                    if outside {
                        for arg in &op.args {
                            if let Arg::Held(held) = arg {
                                if let Some(provenance) = values.get(&held.value) {
                                    newly.extend(provenance.slices.iter().map(|one| one.object.clone()));
                                }
                            }
                        }
                    }
                    let source = _union(op.args.iter().filter_map(|arg| match arg {
                        Arg::Held(held) => Some(values.get(&held.value)),
                        _ => None,
                    }));
                    for reference in &op.stores {
                        let mut keyed = reference.clone();
                        keyed.provenance = _resolved_reference(reference, &values);
                        let key = _cell_key(&keyed);
                        _kill(&mut cells, key.as_ref());
                        if let (Some(key), Some(source)) = (key, &source) {
                            cells.insert(key, source.clone(), _key_place);
                        }
                    }
                }
                state.extend(pointees(newly, &cells));
                let mut visible = escaped_before.get(&op.at).cloned().unwrap_or_default();
                visible.extend(state.iter().cloned());
                escaped_before.insert(op.at, visible);
            }
            escape_out.insert(block.at, state);
        }
        if escape_out == before {
            break;
        }
    }
    let escaped = escape_out.values().flatten().cloned().collect();
    Ok(PointsTo {
        values,
        escaped,
        escaped_before,
    })
}

fn _resolved_actuals(actuals: &[Actual], values: &IndexMap<Value, Provenance>) -> Vec<Provenance> {
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
pub(crate) enum KeyBucket {
    Object(MemoryObject),
    Address(Space, i64),
}

impl Bucket for KeyBucket {
    // Nothing looks a key bucket up by a component.
    type Parts = ();

    fn held(&self, _: &mut ()) {}

    fn released(&self, _: &mut ()) {}
}

pub(crate) fn _key_bucket(key: &CellKey) -> KeyBucket {
    match key {
        CellKey::Object(object, _, _) => KeyBucket::Object(object.clone()),
        CellKey::Address(space, index, _, _) => KeyBucket::Address(*space, *index),
    }
}

/// A key's bucket, and no span: Python's map has no `span_of`.
pub(crate) fn _key_place(key: &CellKey) -> (KeyBucket, Option<ByteRange>) {
    (_key_bucket(key), None)
}

/// Drop the cells a store to `key` may overwrite, keeping `key` itself.
pub(crate) fn _kill<V>(cells: &mut CellMap<CellKey, V, KeyBucket>, key: Option<&CellKey>) {
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
    pub(crate) static ASKED: std::cell::RefCell<Vec<CellKey>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub(crate) fn _keys_overlap(one: Option<&CellKey>, other: Option<&CellKey>) -> bool {
    let (Some(one), Some(other)) = (one, other) else {
        return true;
    };
    match (one, other) {
        (CellKey::Address(space, index, disp, width), CellKey::Address(other_space, other_index, other_disp, other_width))
            if (space, index) == (other_space, other_index) =>
        {
            disp < &(other_disp + other_width) && other_disp < &(disp + width)
        }
        (CellKey::Object(object, low, high), CellKey::Object(other_object, other_low, other_high))
            if object == other_object =>
        {
            low < other_high && other_low < high
        }
        _ => one == other,
    }
}

/// Known `value == residue (mod modulus)` facts; modulus zero is exact.
pub fn congruences(body: &Rc<MirBody>) -> IndexMap<Value, (BigInt, BigInt)> {
    let constants = consts::known(body, None, None, None, None);
    let values = body.values().into_iter().map(|value| (value.id, value)).collect::<IndexMap<_, _>>();
    let mut result: IndexMap<Value, (BigInt, BigInt)> = IndexMap::default();
    let zero = BigInt::from(0_u8);

    fn number(arg: &Arg, constants: &crate::support::hash::IndexMap<Value, Known>) -> Option<BigInt> {
        match arg {
            Arg::Const(constant) => Some(constant.n.clone()),
            Arg::Held(held) => constants.get(&held.value).map(|fact| fact.n.clone()),
            _ => None,
        }
    }

    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        for affine in induction::basics(body, &loop_).values() {
            let (start, step) = (
                number(&affine.start.as_arg(), &constants),
                number(&affine.step.as_arg(), &constants),
            );
            let value = values.get(&affine.value);
            if let (Some(value), Some(start), Some(step)) = (value, start, step) {
                if step != zero {
                    let modulus = if step < zero { -step } else { step };
                    let residue = induction::mod_floor(&start, &modulus);
                    result.insert(*value, (modulus, residue));
                }
            }
        }
    }

    fn computed(
        op: &Op,
        result: &IndexMap<Value, (BigInt, BigInt)>,
        constants: &crate::support::hash::IndexMap<Value, Known>,
    ) -> Option<(BigInt, BigInt)> {
        let zero = BigInt::from(0_u8);
        let Some(Arg::Held(held)) = op.results.first().filter(|_| op.results.len() == 1) else {
            return None;
        };
        if !op.loads.is_empty() || !op.stores.is_empty() || op.barrier() {
            return None;
        }
        let args = &op.args;
        if op.kind == Kind::Copy && args.len() == 1 {
            if let Some(n) = number(&args[0], constants) {
                return Some((zero, n));
            }
            return match &args[0] {
                Arg::Held(source) => result.get(&source.value).cloned(),
                _ => None,
            };
        }
        if args.len() != 2 {
            return None;
        }
        let fact = |arg: &Arg| match arg {
            Arg::Held(source) => result.get(&source.value).cloned(),
            Arg::Const(constant) => Some((BigInt::from(0_u8), constant.n.clone())),
            _ => None,
        };
        let (Some(mut a), Some(mut b)) = (fact(&args[0]), fact(&args[1])) else {
            return None;
        };
        if matches!(op.kind, Kind::Add | Kind::Sub) {
            let modulus = induction::gcd(a.0.clone(), b.0.clone());
            let residue = if op.kind == Kind::Add { a.1 + b.1 } else { a.1 - b.1 };
            if modulus == zero {
                return Some((modulus, residue));
            }
            let residue = induction::mod_floor(&residue, &modulus);
            return Some((modulus, residue));
        }
        if op.kind == Kind::Mul {
            if a.0 == zero {
                (a, b) = (b, a);
            }
            if b.0 == zero {
                let factor = b.1;
                let product = &a.0 * &factor;
                let modulus = if product < zero { -product } else { product };
                let residue = a.1 * factor;
                if modulus == zero {
                    return Some((modulus, residue));
                }
                let residue = induction::mod_floor(&residue, &modulus);
                return Some((modulus, residue));
            }
        }
        if op.kind == Kind::Shl && b.0 == zero && zero <= b.1 && b.1 < BigInt::from(held.width * 8) {
            let shift = usize::try_from(&b.1).expect("a checked shift is below the result width");
            let factor = BigInt::from(1_u8) << shift;
            let modulus = &a.0 * &factor;
            let residue = a.1 * factor;
            if modulus != zero {
                let residue = induction::mod_floor(&residue, &modulus);
                return Some((modulus, residue));
            }
            return Some((modulus, residue));
        }
        None
    }

    loop {
        let mut changed = false;
        for block in &body.blocks {
            for op in &block.ops {
                let Some(fact) = computed(op, &result, &constants) else {
                    continue;
                };
                for value in &op.defines {
                    if !result.contains_key(value) {
                        result.insert(*value, fact.clone());
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            return result;
        }
    }
}

/// Python `named_bytes`' dict, keyed by address and by `(space, index)`:
/// the two key shapes it mixes, as two maps.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NamedBytes {
    pub at: IndexMap<Addr, (MemoryObject, i64)>,
    pub spaces: IndexMap<(Space, i64), MemoryObject>,
}

/// The object and offset each directly addressed byte is, as the body's own references name it.
///
/// Keyed by address, and by (space, index) for a space that is one object
/// at its own displacements. A cell known only by its address takes its
/// object from here, so it carries the same object facts every other
/// reference to it does.
pub fn named_bytes(body: &MirBody) -> NamedBytes {
    let mut out: IndexMap<Addr, Option<(MemoryObject, i64)>> = IndexMap::default();
    let mut refs: Vec<&MemRef> = body.initial.iter().map(|(reference, _)| reference).collect();
    for block in &body.blocks {
        for op in &block.ops {
            let cells = op.args.iter().chain(&op.results).filter_map(|arg| match arg {
                Arg::Cell(cell) => Some(&cell.r#ref),
                _ => None,
            });
            refs.extend(op.loads.iter().chain(&op.stores).chain(cells).chain(op.memory_values.iter().map(|(reference, _)| reference)));
        }
    }
    for reference in refs.into_iter().map(mir::symbolic_ref) {
        let (Some(provenance), None, None, Some(addr)) =
            (&reference.provenance, reference.base, reference.segment, reference.addr)
        else {
            continue;
        };
        if addr.base != iced_x86::Register::None || provenance.slices.len() != 1 {
            continue;
        }
        let one = provenance.slices.first().expect("one slice");
        if one.stride != 1 || one.high + one.width - 1 - one.low != i64::from(reference.width) {
            continue;
        }
        for byte in 0..i64::from(reference.width) {
            let (at, named) = (addr.plus(byte), (one.object.clone(), one.low + byte));
            let same = out.get(&at).is_none_or(|previous| previous.as_ref() == Some(&named));
            out.insert(at, same.then_some(named));
        }
    }
    let named: IndexMap<Addr, (MemoryObject, i64)> =
        out.into_iter().filter_map(|(at, one)| one.map(|one| (at, one))).collect();
    // A space whose every named byte is one object at its own displacement
    // is that object throughout: BC's segments and frame are.
    let mut spaces: IndexMap<(Space, i64), Option<MemoryObject>> = IndexMap::default();
    for (at, (object, offset)) in &named {
        let key = (at.space, at.index);
        let agrees = spaces.get(&key).is_none_or(|previous| previous.as_ref() == Some(object));
        spaces.insert(key, (*offset == at.disp && agrees).then(|| object.clone()));
    }
    NamedBytes {
        at: named,
        spaces: spaces.into_iter().filter_map(|(key, one)| one.map(|one| (key, one))).collect(),
    }
}

/// Attach solved provenance to every indirect reference in a body.
pub fn annotated(body: &Rc<MirBody>) -> Result<MirBody, String> {
    let facts = points_to(body, None, None)?;
    let bounded = ranges::bounded(body)?;
    let constants = ranges::constants(body, None, None);
    let strides = congruences(body);

    let tag = |reference: &MemRef, at: i64, outgoing: bool| -> Result<MemRef, String> {
        let mut got = facts.reference(reference);
        let interval = reference.base.and_then(|base| {
            bounded
                .get(&at)
                .and_then(|known| known.get(&base))
                .or_else(|| constants.get(&base))
        });
        if let (Some(current), Some(_), Some(base), Some(addr), Some(interval)) =
            (&got, &reference.provenance, reference.base, reference.addr, interval)
        {
            if interval.width == reference.base_width && current.slices.len() == 1 {
                let source = current.slices.first().expect("one slice");
                let (modulus, residue) = strides
                    .get(&base)
                    .cloned()
                    .unwrap_or((BigInt::from(1_u8), BigInt::from(0_u8)));
                let stride = if modulus > BigInt::from(1_u8) {
                    modulus
                } else {
                    BigInt::from(1_u8)
                };
                let first = &interval.low + induction::mod_floor(&(residue - &interval.low), &stride);
                let width = i64::from(reference.width.max(1));
                let low = BigInt::from(addr.disp) + first;
                let high = BigInt::from(addr.disp) + &interval.high + 1;
                let end = &high + width - 1;
                let zero = BigInt::from(0_u8);
                if low < high
                    && source
                        .object
                        .extent
                        .is_none_or(|extent| zero <= low && low < high && end <= BigInt::from(extent))
                {
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
        // A near pointer does not encode its selector.  Once it has travelled
        // through SSA, a phi, or an exact pointer spill, the canonical object
        // proof is the only reliable source of that selector.  All current-
        // activation frame objects live in SS; any mixed or unknown set must
        // retain the ordinary near-data interpretation instead.
        let space = if got.as_ref().is_some_and(|got| {
            !got.slices.is_empty() && got.slices.iter().all(|one| one.object.kind == MemoryKind::Frame)
        }) {
            Some(Space::Frame)
        } else {
            reference.space
        };
        let mut excludes = reference.excludes.clone();
        if outgoing && reference.space == Some(Space::Stack) && !excludes.contains(&mir::WHOLE_FRAME) {
            // ARG and CALL implicit stack traffic is below the current stack
            // pointer.  It cannot overwrite this activation's BP-relative
            // frame without stack overflow, independently of SS == DS.  Keep
            // arbitrary SP-relative references conservative; the operation's
            // semantic role is the proof, not the address spelling.
            excludes.push(mir::WHOLE_FRAME);
        }
        if outgoing && reference.space == Some(Space::Stack) {
            if let Some(current) = got {
                got = Some(Provenance {
                    slices: current.slices.into_iter().filter(|one| one.object.kind != MemoryKind::Frame).collect(),
                    restrict: current.restrict,
                });
            }
        }
        if got != reference.provenance || space != reference.space || excludes != reference.excludes {
            let mut tagged = reference.clone();
            tagged.provenance = got;
            tagged.space = space;
            tagged.excludes = excludes;
            return Ok(tagged);
        }
        Ok(reference.clone())
    };

    let operand = |arg: &Arg, at: i64| -> Result<Arg, String> {
        Ok(match arg {
            Arg::Cell(cell) => Arg::Cell(Cell {
                r#ref: tag(&cell.r#ref, at, false)?,
            }),
            _ => arg.clone(),
        })
    };

    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut ops = Vec::new();
        for op in &block.ops {
            let mut tagged = op.clone();
            tagged.loads = op
                .loads
                .iter()
                .map(|reference| tag(reference, block.at, op.kind == Kind::Call))
                .collect::<Result<_, _>>()?;
            tagged.stores = op
                .stores
                .iter()
                .map(|reference| tag(reference, block.at, matches!(op.kind, Kind::Arg | Kind::Call)))
                .collect::<Result<_, _>>()?;
            tagged.args = op.args.iter().map(|arg| operand(arg, block.at)).collect::<Result<_, _>>()?;
            tagged.results = op.results.iter().map(|arg| operand(arg, block.at)).collect::<Result<_, _>>()?;
            ops.push(tagged);
        }
        blocks.push(block.with_ops(ops));
    }
    let mut annotated = MirBody::clone(body);
    annotated.blocks = blocks;
    Ok(annotated)
}

#[cfg(test)]
mod tests {
    //! Ports of `tests/test_mir_alias.py`.
    use std::rc::Rc;

    use std::collections::BTreeSet;

    use crate::support::hash::IndexMap;

    use super::{
        _direct_summary, Actual, Procedure, Summary, UNKNOWN, annotated, calls_annotated, congruences, named_bytes,
        points_to, summaries,
    };
    use crate::analysis::ranges::tests::guarded_loop;
    use crate::analysis::regions::overlapping;
    use crate::model::ir::Operation;
    use crate::model::memory::{self, Identity, MemoryKind, MemoryObject, Provenance, Slice};
    use crate::model::mir::{
        Arg, Cell, Const, FrameAddress, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, OrderedMap, Phi, Value,
        WHOLE_FRAME,
    };
    use crate::objectfile::module::{Addr, Space};
    use crate::support::pyrepr::Repr;

    fn value(n: u32) -> Value {
        Value {
            variable: n,
            version: 1,
            ..Value::new(n, i64::from(n))
        }
    }

    fn object(kind: MemoryKind, identity: Identity, extent: Option<i64>) -> MemoryObject {
        MemoryObject {
            identity: Some(identity),
            extent,
            ..MemoryObject::new(kind)
        }
    }

    fn named(name: &str, offset: i64) -> Identity {
        Identity::Tuple(vec![Identity::Str(name.to_owned()), Identity::Int(offset)])
    }

    fn segment(index: i64) -> Identity {
        Identity::Tuple(vec![Identity::Space(Space::Segment), Identity::Int(index)])
    }

    fn one(object: &MemoryObject, low: i64, high: i64) -> Provenance {
        Provenance::one_with_slice(object.clone(), low, high, 1, 1, BTreeSet::new()).unwrap()
    }

    fn held(value: Value, width: u32) -> Arg {
        Arg::Held(Held { value, width })
    }

    fn cell(reference: &MemRef) -> Arg {
        Arg::Cell(Cell {
            r#ref: reference.clone(),
        })
    }

    fn op(at: i64, operation: Operation, kind: Kind, defines: Vec<Value>, uses: Vec<Value>) -> Op {
        let mut made = Op::new(at, OpCode::Operation(operation), "", defines, uses);
        made.kind = kind;
        made
    }

    fn phi(result: Value, incoming: &[(i64, Value)]) -> Phi {
        Phi {
            result,
            incoming: incoming.iter().copied().collect::<OrderedMap<_, _>>(),
        }
    }

    fn body(blocks: Vec<MirBlock>, pointers: &[Value], seeds: &[(Value, Provenance)]) -> MirBody {
        let mut made = MirBody::new(0, blocks);
        made.pointer_values = pointers.iter().copied().collect();
        made.pointer_seeds = seeds.iter().cloned().collect();
        made
    }

    fn with_provenance(width: u32, provenance: Provenance) -> MemRef {
        let mut made = MemRef::new(None, width);
        made.provenance = Some(provenance);
        made
    }

    fn overlaps(one: &MemRef, other: &MemRef) -> bool {
        overlapping(one, other, None, None, None).unwrap()
    }

    fn procedure(body: MirBody, calls: &[(i64, &str)], arguments: Vec<(i64, Vec<Actual>)>) -> Procedure {
        Procedure {
            body,
            calls: calls.iter().map(|(at, name)| (*at, (*name).to_owned())).collect(),
            arguments: arguments.into_iter().collect(),
        }
    }

    fn call(at: i64) -> Op {
        op(at, Operation::Call, Kind::Call, vec![], vec![])
    }

    fn catch_all_call(at: i64) -> Op {
        let mut made = call(at);
        made.loads = vec![MemRef::new(None, 4)];
        made.stores = vec![MemRef::new(None, 4)];
        made
    }

    fn frame(offset: i64) -> MemRef {
        let mut made = MemRef::new(Some(Addr::new(Space::Frame, offset)), 2);
        made.space = Some(Space::Frame);
        made
    }

    fn spill(at: i64, root: Value, slot: &MemRef) -> Op {
        let mut made = op(at, Operation::Move, Kind::Store, vec![], vec![root]);
        made.args = vec![held(root, 2)];
        made.results = vec![cell(slot)];
        made.stores = vec![slot.clone()];
        made
    }

    fn reload(at: i64, loaded: Value, slot: &MemRef) -> Op {
        let mut made = op(at, Operation::Move, Kind::Load, vec![loaded], vec![]);
        made.args = vec![cell(slot)];
        made.results = vec![held(loaded, 2)];
        made.loads = vec![slot.clone()];
        made
    }

    fn constant_store(at: i64, number: i64, destination: &MemRef, uses: Vec<Value>) -> Op {
        let mut made = op(at, Operation::Move, Kind::Store, vec![], uses);
        made.args = vec![Arg::Const(Const::new(number, 2))];
        made.results = vec![cell(destination)];
        made.stores = vec![destination.clone()];
        made
    }

    #[test]
    fn test_outgoing_argument_stack_does_not_kill_current_frame_values() {
        // qbsp kept reloading nodenr and emitted IMUL after pushing call args.
        let mut stack = MemRef::new(Some(Addr::new(Space::Stack, -2)), 2);
        stack.space = Some(Space::Stack);
        let mut push = op(1, Operation::Push, Kind::Arg, vec![], vec![]);
        push.name = "push".to_owned();
        push.args = vec![Arg::Const(Const::new(1, 2))];
        push.results = vec![cell(&stack)];
        push.stores = vec![stack];
        let mut made = MirBody::new(1, vec![MirBlock::new(1, vec![], vec![push], vec![])]);
        made.sealed = true;

        let written = annotated(&Rc::new(MirBody::clone(&made))).unwrap().blocks[0].ops[0].stores[0].clone();
        let local = frame(-22);

        assert!(written.excludes.contains(&WHOLE_FRAME));
        assert!(!overlaps(&local, &written));
    }

    #[test]
    fn test_points_to_flows_through_memory_and_a_phi() {
        // A pointer spilled on one arm and joined with a copy retains its object set.
        let (root, loaded, joined) = (value(1), value(2), value(3));
        let slot = frame(-2);
        let mut address = op(1, Operation::Address, Kind::Address, vec![root], vec![]);
        address.args = vec![Arg::FrameAddress(FrameAddress {
            extent: Some((-8, -4)),
            ..FrameAddress::new(-8, 2)
        })];
        address.results = vec![held(root, 2)];
        let made = body(
            vec![
                MirBlock::new(0, vec![], vec![address, spill(2, root, &slot)], vec![10, 20]),
                MirBlock::new(10, vec![], vec![reload(3, loaded, &slot)], vec![30]),
                MirBlock::new(20, vec![], vec![], vec![30]),
                MirBlock::new(30, vec![phi(joined, &[(10, loaded), (20, root)])], vec![], vec![]),
            ],
            &[root, loaded, joined],
            &[],
        );

        let facts = points_to(&made, None, None).unwrap();
        assert_eq!(facts.values[&loaded], facts.values[&root]);
        assert_eq!(facts.values[&joined], facts.values[&root]);
    }

    #[test]
    fn test_unannotated_computed_address_keeps_ssa_provenance() {
        // C nbody lost every frame-array object and stopped unrolling its hot loop.
        let (address, result) = (value(1), value(2));
        let mut reference = MemRef::new(None, 8);
        reference.base = Some(address);
        reference.space = Some(Space::Frame);
        let mut make_address = op(1, Operation::Address, Kind::Address, vec![address], vec![]);
        make_address.name = "address".to_owned();
        make_address.args = vec![Arg::FrameAddress(FrameAddress {
            extent: Some((-32, 0)),
            ..FrameAddress::new(-32, 2)
        })];
        make_address.results = vec![held(address, 2)];
        let mut load = op(2, Operation::Move, Kind::Load, vec![result], vec![address]);
        load.name = "mov".to_owned();
        load.args = vec![cell(&reference)];
        load.results = vec![held(result, 8)];
        load.loads = vec![reference];
        let mut made = body(vec![MirBlock::new(0, vec![], vec![make_address, load], vec![])], &[], &[]);
        made.sealed = true;

        let tagged = annotated(&Rc::new(MirBody::clone(&made))).unwrap().blocks[0].ops[1].loads[0].clone();

        let provenance = tagged.provenance.expect("a derived provenance");
        let kinds = provenance.slices.iter().map(|one| one.object.kind).collect::<BTreeSet<_>>();
        assert_eq!(kinds, BTreeSet::from([MemoryKind::Frame]));
    }

    #[test]
    fn test_store_through_parameter_keeps_disjoint_frame_pointer_spill() {
        // ls_switch lost its saved parameter after storing field 0, so fields 2/4/6 became unknown writes.
        let (root, first, second) = (value(1), value(2), value(3));
        let parameter = object(MemoryKind::Parameter, Identity::Int(0), None);
        let frame_object = object(MemoryKind::Frame, named("ls_switch", -4), Some(2));
        let mut slot = frame(-4);
        slot.provenance = Some(one(&frame_object, 0, 2));
        let mut field0 = MemRef::new(Some(Addr::new(Space::Literal, 0)), 2);
        field0.base = Some(first);
        let mut field2 = MemRef::new(Some(Addr::new(Space::Literal, 2)), 2);
        field2.base = Some(second);
        let made = body(
            vec![MirBlock::new(
                0,
                vec![],
                vec![
                    spill(1, root, &slot),
                    reload(2, first, &slot),
                    constant_store(3, 1, &field0, vec![]),
                    reload(4, second, &slot),
                    constant_store(5, 2, &field2, vec![]),
                ],
                vec![],
            )],
            &[root, first, second],
            &[(root, Provenance::one(parameter.clone()))],
        );

        let facts = points_to(&made, None, None).unwrap();
        let summary = _direct_summary(&made).unwrap();

        assert_eq!(facts.values[&second], facts.values[&root]);
        assert!(!summary.unknown_write);
        let written = summary.writes.iter().map(|one| one.object.clone()).collect::<BTreeSet<_>>();
        assert_eq!(written, BTreeSet::from([parameter]));
    }

    #[test]
    fn test_pointer_phi_with_an_unknown_arm_is_unknown() {
        // A known arm must not erase the other arm: that falsely made the joined pointer disjoint from real objects.
        let (known, unknown, joined) = (value(1), value(2), value(3));
        let frame_object = object(MemoryKind::Frame, named("f", -4), Some(4));
        let made = body(
            vec![
                MirBlock::new(0, vec![], vec![], vec![10, 20]),
                MirBlock::new(10, vec![], vec![], vec![30]),
                MirBlock::new(20, vec![], vec![], vec![30]),
                MirBlock::new(30, vec![phi(joined, &[(10, known), (20, unknown)])], vec![], vec![]),
            ],
            &[known, unknown, joined],
            &[(known, one(&frame_object, 0, 1))],
        );

        assert_eq!(points_to(&made, None, None).unwrap().values[&joined], *UNKNOWN);
    }

    #[test]
    fn test_pointer_recurrence_at_a_loop_header_widens_and_terminates() {
        // qcport con_print advanced its text pointer one byte per solver round instead of compiling.
        let (root, current, advanced) = (value(1), value(2), value(3));
        let parameter = object(MemoryKind::Parameter, Identity::Int(0), None);
        let mut increment = op(2, Operation::Binary, Kind::Add, vec![advanced], vec![current]);
        increment.args = vec![held(current, 2), Arg::Const(Const::new(1, 2))];
        increment.results = vec![held(advanced, 2)];
        let made = body(
            vec![
                MirBlock::new(0, vec![], vec![], vec![10]),
                MirBlock::new(10, vec![phi(current, &[(0, root), (20, advanced)])], vec![], vec![20]),
                MirBlock::new(20, vec![], vec![increment], vec![10]),
            ],
            &[root, current, advanced],
            &[(root, one(&parameter, 0, 1))],
        );

        let facts = points_to(&made, None, None).unwrap();

        let expected = Provenance {
            slices: UNKNOWN.slices.union(&Provenance::one(parameter).slices).cloned().collect(),
            restrict: BTreeSet::new(),
        };
        assert_eq!(facts.values[&current], expected);
        assert_eq!(facts.values[&advanced], expected);
    }

    #[test]
    fn test_parameter_modref_is_instantiated_at_a_call_site() {
        // A callee writing parameter zero clobbers its actual object and no neighbour.
        let param = object(MemoryKind::Parameter, Identity::Int(0), None);
        let summary = Summary {
            writes: BTreeSet::from([Slice::new(param, 2, 4, 1, 1).unwrap()]),
            ..Summary::default()
        };
        let actual = object(MemoryKind::Global, segment(7), Some(16));
        let effect = summary.instantiated(&[one(&actual, 4, 5)]).writes;

        assert_eq!(effect, BTreeSet::from([Slice::new(actual, 6, 8, 1, 1).unwrap()]));
    }

    #[test]
    fn test_interprocedural_modref_reaches_the_call_operation() {
        // A known callee replaces CALL's catch-all effect with its actual object.
        let parameter = object(MemoryKind::Parameter, Identity::Int(0), None);
        let actual = object(MemoryKind::Global, segment(9), Some(16));
        let write = with_provenance(2, one(&parameter, 2, 4));
        let mut stored = op(1, Operation::Move, Kind::Store, vec![], vec![]);
        stored.stores = vec![write];
        let callee = procedure(body(vec![MirBlock::new(0, vec![], vec![stored], vec![])], &[], &[]), &[], vec![]);
        let caller = procedure(
            body(vec![MirBlock::new(0, vec![], vec![catch_all_call(2)], vec![])], &[], &[]),
            &[(2, "callee")],
            vec![(2, vec![Actual::Provenance(one(&actual, 4, 5))])],
        );

        let procedures = IndexMap::from_iter([("caller".to_owned(), caller.clone()), ("callee".to_owned(), callee)]);
        let known = summaries(&procedures, None).unwrap();
        let annotated_body = calls_annotated(&caller, &known).unwrap();
        let effect = &annotated_body.blocks[0].ops[0];

        assert!(effect.memory_complete && effect.loads.is_empty());
        assert_eq!(effect.stores[0].provenance, Some(one(&actual, 6, 8)));
    }

    #[test]
    fn test_recursive_pointer_offset_summary_widens_and_terminates() {
        // A recursive f(p + 1) grew one byte per summary round; the SCC effect is the whole formal object.
        let pointer = value(1);
        let parameter = object(MemoryKind::Parameter, Identity::Int(0), None);
        let write = with_provenance(1, one(&parameter, 0, 1));
        let mut store = op(1, Operation::Move, Kind::Store, vec![], vec![]);
        store.stores = vec![write];
        let made = body(
            vec![MirBlock::new(0, vec![], vec![store, call(2)], vec![])],
            &[pointer],
            &[(pointer, one(&parameter, 0, 1))],
        );
        let recursive = procedure(made, &[(2, "recursive")], vec![(2, vec![Actual::Pointer(pointer, 1)])]);

        let summary = summaries(&IndexMap::from_iter([("recursive".to_owned(), recursive)]), None).unwrap()["recursive"]
            .clone();

        assert_eq!(summary.writes, BTreeSet::from([Slice::whole(parameter)]));
    }

    #[test]
    fn test_unknown_call_reaches_nonlocals_and_only_its_pointer_actual() {
        // An unknown C call does not clobber every local, but may use a local whose address it receives.
        let passed = object(MemoryKind::Frame, named("caller", -8), Some(4));
        let private = object(MemoryKind::Frame, named("caller", -12), Some(4));
        let unknown = procedure(
            body(vec![MirBlock::new(0, vec![], vec![catch_all_call(4)], vec![])], &[], &[]),
            &[(4, "external")],
            vec![(4, vec![Actual::Provenance(one(&passed, 0, 1)), Actual::Absent])],
        );

        let annotated_body = calls_annotated(&unknown, &IndexMap::default()).unwrap();
        let effect = &annotated_body.blocks[0].ops[0];
        let passed_ref = with_provenance(4, one(&passed, 0, 4));
        let passed_tail = with_provenance(1, one(&passed, 3, 4));
        let private_ref = with_provenance(4, one(&private, 0, 4));

        assert!(effect.memory_complete);
        assert!(effect.stores.iter().any(|one| overlaps(&passed_ref, one)));
        assert!(effect.stores.iter().any(|one| overlaps(&passed_tail, one)));
        assert!(!effect.stores.iter().any(|one| overlaps(&private_ref, one)));
        // Python orders the effect by `repr`, which puts FRAME before NONLOCAL.
        let order = effect
            .stores
            .iter()
            .map(|one| one.provenance.as_ref().unwrap().slices.first().unwrap().repr())
            .collect::<Vec<_>>();
        assert_eq!(
            order,
            [
                "Slice(object=Object(kind=<Kind.FRAME: 'frame'>, identity=('caller', -8), generation=0, extent=4, addressed=True, captured=True), \
                 low=0, high=4, stride=1, width=1)",
                "Slice(object=Object(kind=<Kind.NONLOCAL: 'nonlocal'>, identity=None, generation=0, extent=None, addressed=True, captured=True), \
                 low=-2147483648, high=2147483648, stride=1, width=1)",
            ]
        );
    }

    #[test]
    fn test_unknown_call_reaches_a_frame_pointer_escaped_before_the_call() {
        // Storing &local outside the frame exposes that object to a later unknown call, not its neighbours.
        let pointer = value(1);
        let escaped = object(MemoryKind::Frame, named("caller", -8), Some(4));
        let private = object(MemoryKind::Frame, named("caller", -12), Some(4));
        let global = object(MemoryKind::Global, segment(9), None);
        let destination = with_provenance(2, one(&global, 0, 2));
        let mut publish = op(2, Operation::Move, Kind::Store, vec![], vec![pointer]);
        publish.args = vec![held(pointer, 2)];
        publish.stores = vec![destination];
        let made = body(
            vec![MirBlock::new(0, vec![], vec![publish, call(4)], vec![])],
            &[pointer],
            &[(pointer, one(&escaped, 0, 1))],
        );
        let unknown = procedure(made, &[(4, "external")], vec![(4, vec![])]);

        let annotated_body = calls_annotated(&unknown, &IndexMap::default()).unwrap();
        let effect = annotated_body.blocks[0].ops.last().unwrap();
        let escaped_ref = with_provenance(4, one(&escaped, 0, 4));
        let private_ref = with_provenance(4, one(&private, 0, 4));

        assert!(effect.stores.iter().any(|one| overlaps(&escaped_ref, one)));
        assert!(!effect.stores.iter().any(|one| overlaps(&private_ref, one)));
    }

    #[test]
    fn test_known_capture_summary_controls_later_external_reach() {
        // A borrowed pointer remains private; a retained pointer exposes its object to subsequent unknown calls.
        let pointer = value(1);
        let local = object(MemoryKind::Frame, named("caller", -8), Some(4));
        let made = body(
            vec![MirBlock::new(0, vec![], vec![call(2), call(3)], vec![])],
            &[pointer],
            &[(pointer, one(&local, 0, 1))],
        );
        let caller = procedure(
            made,
            &[(2, "known"), (3, "external")],
            vec![(2, vec![Actual::Pointer(pointer, 0)]), (3, vec![])],
        );
        let reference = with_provenance(4, one(&local, 0, 4));

        let borrowed_body =
            calls_annotated(&caller, &IndexMap::from_iter([("known".to_owned(), Summary::default())])).unwrap();
        let captured = Summary {
            captures: BTreeSet::from([Some(Identity::Int(0))]),
            ..Summary::default()
        };
        let captured_body = calls_annotated(&caller, &IndexMap::from_iter([("known".to_owned(), captured)])).unwrap();
        let borrowed = borrowed_body.blocks[0].ops.last().unwrap();
        let captured = captured_body.blocks[0].ops.last().unwrap();

        assert!(!borrowed.stores.iter().any(|one| overlaps(&reference, one)));
        assert!(captured.stores.iter().any(|one| overlaps(&reference, one)));
    }

    #[test]
    fn test_pointer_fact_does_not_hide_a_conflicting_concrete_operand_object() {
        // Inconsistent concrete metadata must remain may-alias, never become a false proof.
        let pointer = value(1);
        let allocation = object(MemoryKind::Allocation, Identity::Str("derived".to_owned()), Some(8));
        let attached = object(MemoryKind::Global, segment(9), Some(8));
        let mut reference = with_provenance(2, one(&attached, 0, 2));
        reference.base = Some(pointer);
        reference.pointer = true;
        let mut store = constant_store(1, 0, &reference, vec![pointer]);
        store.name = "mov".to_owned();
        let mut made = body(
            vec![MirBlock::new(0, vec![], vec![store], vec![])],
            &[pointer],
            &[(pointer, one(&allocation, 0, 1))],
        );
        made.sealed = true;

        let tagged = annotated(&Rc::new(MirBody::clone(&made))).unwrap().blocks[0].ops[0].stores[0].clone();

        let provenance = tagged.provenance.expect("a merged provenance");
        let objects = provenance.slices.iter().map(|one| one.object.clone()).collect::<BTreeSet<_>>();
        assert_eq!(objects, BTreeSet::from([allocation, attached]));
    }

    #[test]
    fn annotated_narrows_a_loop_index_to_its_strided_interval() {
        // Expected from Python: `tests/test_edge_ranges.py:guarded_loop` plus a
        // word load at seg:3+4 indexed by the counter scaled by two.
        let mut made = guarded_loop();
        let offset = Value::new(6, 30);
        let mut address = Addr::new(Space::Segment, 4);
        address.index = 3;
        let mut reference = MemRef::new(Some(address), 2);
        reference.base = Some(offset);
        reference.base_width = 2;
        reference.provenance = Some(Provenance::one(object(MemoryKind::Global, segment(3), Some(64))));
        let loaded = Value::new(7, 31);
        let mut load = op(31, Operation::Move, Kind::Load, vec![loaded], vec![offset]);
        load.name = "mov".to_owned();
        load.args = vec![cell(&reference)];
        load.results = vec![held(loaded, 2)];
        load.loads = vec![reference];
        made.blocks[3].ops.push(load);

        let strides = congruences(&Rc::new(MirBody::clone(&made)))
            .into_iter()
            .map(|(value, (modulus, residue))| format!("{value}: ({modulus}, {residue})"))
            .collect::<Vec<_>>();
        let tagged = annotated(&Rc::new(MirBody::clone(&made))).unwrap().blocks[3].ops[1].loads[0].repr();

        assert_eq!(strides, ["v2: (1, 0)", "v1: (0, 0)", "v6: (2, 0)"]);
        assert_eq!(
            tagged,
            "MemRef(addr=[seg:3+0x4], width=2, base=v6, segment=None, space=None, beyond=None, symbolic=None, \
             allocation=None, base_width=2, pointer=False, excludes=(), typed=None, within=None, \
             provenance=Provenance(slices=frozenset({Slice(object=Object(kind=<Kind.GLOBAL: 'global'>, \
             identity=(<Space.SEGMENT: 'seg'>, 3), generation=0, extent=64, addressed=True, captured=True), low=4, high=11, stride=2, width=2)}), \
             restrict=frozenset()), volatile=False, inbounds=False)"
        );
    }

    #[test]
    fn test_offsets_in_different_objects_are_never_compared() {
        // A bp slot and an sp push were called disjoint by comparing -0x16 with -2.
        let r#static = one(&object(MemoryKind::Global, Identity::Str("table".to_owned()), None), 0x16, 0x18);
        let extern_ = one(&object(MemoryKind::External, Identity::Str("shared".to_owned()), None), 2, 4);

        assert!(r#static.intersects(&extern_));
    }

    #[test]
    fn test_capture_decides_what_nonlocal_reaches() {
        // A call's NONLOCAL reach met every global, so no call left a private static in a register.
        let private = MemoryObject {
            captured: false,
            ..object(MemoryKind::Global, Identity::Str("counter".to_owned()), None)
        };
        let unaddressed = MemoryObject {
            addressed: false,
            captured: false,
            ..object(MemoryKind::Global, Identity::Str("total".to_owned()), None)
        };
        let nonlocal = MemoryObject::new(MemoryKind::Nonlocal);
        let unknown = MemoryObject::new(MemoryKind::Unknown);

        assert!(!memory::objects_may_alias(&nonlocal, &private));
        assert!(memory::objects_may_alias(&unknown, &private));
        assert!(!memory::objects_may_alias(&unknown, &unaddressed));
        assert!(memory::objects_may_alias(&unaddressed, &unaddressed));
    }

    #[test]
    fn test_one_base_value_settles_provenance_references_by_displacement() {
        // Two fields off one pointer, each whole-object provenance, were called overlapping.
        let base = Value::new(1, 2);
        let whole = Provenance::one(MemoryObject::new(MemoryKind::Unknown));
        let mut first = MemRef::new(Some(Addr::new(Space::Literal, 0)), 2);
        first.base = Some(base);
        first.provenance = Some(whole);
        let mut second = first.clone();
        second.addr = Some(Addr::new(Space::Literal, 2));
        let mut third = first.clone();
        third.addr = Some(Addr::new(Space::Literal, 1));

        assert_eq!(overlapping(&first, &second, None, None, None), Ok(false));
        assert_eq!(overlapping(&first, &third, None, None, None), Ok(true));
    }

    #[test]
    fn test_a_lane_form_slice_names_every_byte_it_covers() {
        // A narrowed word slice [6, 7) of width 2 covers bytes 6 and 7; reading its end as 7 named neither.
        let r#static = object(MemoryKind::Global, segment(5), None);
        let mut reference = MemRef::new(Some(Addr { index: 5, ..Addr::new(Space::Segment, 6) }), 2);
        reference.provenance = Some(Provenance {
            slices: BTreeSet::from([Slice::new(r#static.clone(), 6, 7, 1, 2).unwrap()]),
            restrict: BTreeSet::new(),
        });
        let mut store = op(0, Operation::Move, Kind::Store, vec![], vec![]);
        store.name = "mov".to_owned();
        store.args = vec![Arg::Const(Const::new(7, 2))];
        store.stores = vec![reference];
        let body = MirBody::new(0, vec![MirBlock::new(0, vec![], vec![store], vec![])]);

        let named = named_bytes(&body);

        assert_eq!(named.at[&Addr { index: 5, ..Addr::new(Space::Segment, 7) }], (r#static, 7));
    }
}
