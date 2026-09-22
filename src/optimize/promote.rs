//! Write-through promotion: reuse a stored value across statements.
//!
//! Direct port of `qbopt/optimize/promote.py`.  Every store stays in place;
//! a store also defines a fresh variable, a load becomes a use of it, and
//! `ssa::constructed` places the phis.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use indexmap::IndexMap;
use num_bigint::BigInt;

use crate::analysis::consts::Known;
use crate::analysis::consts::early_d as consts;
use crate::analysis::ranges::Interval;
use crate::analysis::ranges::early_d::singletons;
use crate::analysis::regions::{self, RegionLayout};
use crate::analysis::{alias, effects, loops, ssa};
use crate::model::memory::{self, Identity, MemoryKind, MemoryObject, Provenance, Slice};
use crate::model::mir::{
    self, Arg, Cell, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, OrderedMap, Symbol, Value,
};
use crate::model::passes::{MIRTransform, Where};
use crate::objectfile::module::{Addr, Space};
use crate::support::pyrepr::Repr;

pub(crate) const READS: [Kind; 10] = [
    Kind::Load,
    Kind::Add,
    Kind::AddCarry,
    Kind::Sub,
    Kind::Increment,
    Kind::Decrement,
    Kind::Mul,
    Kind::And,
    Kind::Or,
    Kind::Xor,
];

pub(crate) const CELLS: [Space; 2] = [Space::Segment, Space::Frame];

/// Python's `(bounds, dgroup)` as the regions layout.  `mir.overlapping`
/// passes `dgroup` where only a `module.Group` informs, so it adds nothing.
type Bounds = IndexMap<(Space, i64), Vec<i64>>;

/// One exact scalar leaf of a canonical memory object.
///
/// Provenance is authoritative only when it names one dense, exact byte
/// range matching the access width.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct _Leaf {
    pub object: MemoryObject,
    pub low: i64,
    pub high: i64,
    pub type_class: Option<String>,
}

/// One symbolic address root plus a mathematical byte displacement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct _Affine {
    pub root: Option<Identity>,
    pub offset: i64,
}

/// What `_key` returns: Python's `_Leaf | MemRef | Addr`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Key {
    Leaf(_Leaf),
    Ref(MemRef),
    Addr(Addr),
}

fn only_slice(provenance: &Provenance) -> Option<&Slice> {
    if provenance.slices.len() == 1 {
        provenance.slices.iter().next()
    } else {
        None
    }
}

fn slice(object: MemoryObject, low: i64, high: i64) -> Slice {
    Slice::new(object, low, high, 1, 1).expect("a nonempty exact byte range")
}

fn one(object: MemoryObject, low: i64, high: i64) -> Provenance {
    Provenance::one_with_slice(object, low, high, 1, 1, BTreeSet::new())
        .expect("a nonempty exact byte range")
}

fn layout(bounds: Option<&Bounds>) -> Option<RegionLayout> {
    bounds.map(|bounds| RegionLayout {
        shared_segments: None,
        landmarks: bounds
            .iter()
            .map(|(key, marks)| (*key, marks.clone()))
            .collect(),
    })
}

/// `mir.overlapping(one, other, dgroup, bounds)`; a span Rust cannot
/// represent overlaps.
fn overlapping(one: &MemRef, other: &MemRef, layout: Option<&RegionLayout>) -> bool {
    regions::overlapping(one, other, None, None, layout).unwrap_or(true)
}

pub(crate) fn _leaf(r#ref: &MemRef) -> Option<_Leaf> {
    let span = only_slice(r#ref.provenance.as_ref()?)?;
    // Frontends spell one contiguous exact access either as the canonical
    // byte range `[low, low + width)` or as one stride-1 element whose own
    // width is the access width.  They select exactly the same bytes.
    let width = i64::from(r#ref.width);
    let contiguous = (span.width == 1 && span.high - span.low == width)
        || (span.high - span.low == 1 && span.width == width);
    if span.stride != 1 || !contiguous {
        return None;
    }
    let high = span.low + width;
    if span
        .object
        .extent
        .is_some_and(|extent| !(0 <= span.low && span.low < high && high <= extent))
    {
        return None;
    }
    Some(_Leaf {
        object: span.object.clone(),
        low: span.low,
        high,
        type_class: r#ref.typed.as_ref().map(|typed| typed.0.clone()),
    })
}

/// Objects whose accesses cannot form disjoint scalar leaves.
///
/// Equal ranges are one leaf, disjoint ranges independent leaves; a proper
/// overlap or ambiguous multi-object provenance keeps the object in memory.
pub(crate) fn _blocked_objects(refs: &[&MemRef]) -> BTreeSet<MemoryObject> {
    let mut accesses = IndexMap::<MemoryObject, Vec<_Leaf>>::new();
    let mut blocked = BTreeSet::new();
    for r#ref in refs {
        let Some(provenance) = &r#ref.provenance else {
            continue;
        };
        let objects = provenance
            .slices
            .iter()
            .map(|span| span.object.clone())
            .collect::<BTreeSet<_>>();
        if objects.len() != 1 || provenance.slices.len() != 1 {
            blocked.extend(objects);
            continue;
        }
        if let Some(leaf) = _leaf(r#ref) {
            accesses.entry(leaf.object.clone()).or_default().push(leaf);
        }
    }

    for (object, leaves) in &accesses {
        for (index, one) in leaves.iter().enumerate() {
            for other in &leaves[index + 1..] {
                let overlaps = one.low.max(other.low) < one.high.min(other.high);
                let same_range = (one.low, one.high) == (other.low, other.high);
                if overlaps && (!same_range || one.type_class != other.type_class) {
                    blocked.insert(object.clone());
                    break;
                }
            }
            if blocked.contains(object) {
                break;
            }
        }
    }
    blocked
}

pub(crate) fn _key(r#ref: &MemRef, blocked: &BTreeSet<MemoryObject>) -> Option<Key> {
    if r#ref.volatile {
        return None;
    }
    if let Some(provenance) = &r#ref.provenance {
        if provenance
            .slices
            .iter()
            .any(|span| blocked.contains(&span.object))
        {
            return None;
        }
    }
    if let Some(leaf) = _leaf(r#ref) {
        return Some(Key::Leaf(leaf));
    }
    let addr = r#ref.addr?;
    if r#ref.segment.is_some() || !CELLS.contains(&addr.space) {
        return None;
    }
    // Keep canonical object identity on the promoted cell: reducing it to
    // `Addr` lost the frontend's proof against nonlocal call effects.
    if r#ref.provenance.is_some() {
        return Some(Key::Ref(MemRef {
            width: 0,
            ..r#ref.clone()
        }));
    }
    if r#ref.base.is_none() {
        return Some(Key::Addr(addr));
    }
    if addr.space == Space::Segment && !r#ref.excludes.is_empty() {
        return Some(Key::Ref(MemRef {
            width: 0,
            excludes: vec![],
            ..r#ref.clone()
        }));
    }
    None
}

pub(crate) fn _reference(key: &Key, width: u32) -> MemRef {
    match key {
        Key::Leaf(leaf) => MemRef {
            provenance: Some(one(leaf.object.clone(), leaf.low, leaf.high)),
            ..MemRef::new(None, width)
        },
        Key::Ref(r#ref) => MemRef {
            width,
            ..r#ref.clone()
        },
        Key::Addr(addr) => MemRef::new(Some(*addr), width),
    }
}

/// Python's sort key; a cell's is `(0, "", "", index, disp, base)`.
pub(crate) fn _order(key: &Key) -> (u8, String, String, i64, i64, i64, String) {
    if let Key::Leaf(leaf) = key {
        return (
            1,
            leaf.object.kind.to_string(),
            leaf.object.identity.repr(),
            leaf.object.generation,
            leaf.low,
            leaf.high,
            leaf.type_class.clone().unwrap_or_default(),
        );
    }
    let r#ref = _reference(key, 0);
    let addr = r#ref.addr.expect("a cell key has an address");
    (
        0,
        String::new(),
        String::new(),
        addr.index,
        addr.disp,
        r#ref.base.map_or(-1, |base| i64::from(base.id)),
        String::new(),
    )
}

/// Objects known to contain more than the scalar leaf being accessed.
pub(crate) fn _aggregate_objects<'a>(
    leaves: impl IntoIterator<Item = &'a Key>,
) -> BTreeSet<MemoryObject> {
    let mut ranges = IndexMap::<MemoryObject, BTreeSet<(i64, i64)>>::new();
    for leaf in leaves {
        if let Key::Leaf(leaf) = leaf {
            ranges
                .entry(leaf.object.clone())
                .or_default()
                .insert((leaf.low, leaf.high));
        }
    }
    ranges
        .into_iter()
        .filter(|(object, parts)| {
            parts.len() > 1
                || object
                    .extent
                    .is_some_and(|extent| parts.iter().any(|(low, high)| extent > high - low))
        })
        .map(|(object, _)| object)
        .collect()
}

#[allow(dead_code)] // Wired by the transform port.
pub(crate) struct Promote {
    pub r#where: Where,
}

impl Promote {
    #[allow(dead_code)] // Wired by the transform port.
    pub(crate) fn new(r#where: Where) -> Self {
        Self { r#where }
    }
}

impl MIRTransform for Promote {
    fn class_name(&self) -> &'static str {
        "Promote"
    }

    fn name(&self) -> &str {
        "promote"
    }

    fn transform(&mut self, body: MirBody) -> Result<MirBody, String> {
        promoted(
            &body,
            &self.r#where.dgroup,
            self.r#where.bounds.as_ref(),
            false,
            true,
            false,
        )
    }
}

/// Scalarize proven aggregate leaves before scalar simplification.
#[allow(dead_code)] // Wired by the transform port.
pub(crate) struct Sroa {
    pub r#where: Where,
}

impl Sroa {
    #[allow(dead_code)] // Wired by the transform port.
    pub(crate) fn new(r#where: Where) -> Self {
        Self { r#where }
    }
}

impl MIRTransform for Sroa {
    fn class_name(&self) -> &'static str {
        "Sroa"
    }

    fn name(&self) -> &str {
        "sroa"
    }

    fn transform(&mut self, body: MirBody) -> Result<MirBody, String> {
        let body = _allocation_leaves(&body);
        let body = _bounded_leaves(&body)?;
        let body = _canonical_leaf_types(&body);
        let body = _split_copies(&body);
        promoted(
            &body,
            &self.r#where.dgroup,
            self.r#where.bounds.as_ref(),
            false,
            true,
            true,
        )
    }
}

pub(crate) fn _signed(number: &BigInt, width: u32) -> BigInt {
    let sign = BigInt::from(1) << (width * 8 - 1);
    ((number & ((&sign << 1) - 1)) ^ &sign) - sign
}

/// Normalize address arithmetic without giving physical locations meaning.
///
/// A direct cell is an opaque symbolic root.  Addition, subtraction and
/// width conversion retain that root and combine only byte displacements;
/// two roots, products, phis and unknown operations are refused.
pub(crate) fn _affine_values(body: &MirBody) -> IndexMap<Value, _Affine> {
    let mut definitions = IndexMap::<Value, &Op>::new();
    for op in body.blocks.iter().flat_map(|block| &block.ops) {
        for result in &op.results {
            if let Arg::Held(result) = result {
                definitions.insert(result.value, op);
            }
        }
    }
    let mut cache = HashMap::<Value, Option<_Affine>>::new();
    let mut active = HashSet::<Value>::new();

    fn operand(
        arg: &Arg,
        definitions: &IndexMap<Value, &Op>,
        cache: &mut HashMap<Value, Option<_Affine>>,
        active: &mut HashSet<Value>,
    ) -> Option<_Affine> {
        match arg {
            Arg::Const(constant) => Some(_Affine {
                root: None,
                offset: i64::try_from(_signed(&constant.n, constant.width)).ok()?,
            }),
            Arg::Held(held) => value(held.value, definitions, cache, active),
            Arg::Cell(cell) if !cell.r#ref.volatile => {
                let r#ref = mir::symbolic_ref(&cell.r#ref);
                let addr = r#ref.addr?;
                if r#ref.base.is_some() || r#ref.segment.is_some() {
                    return None;
                }
                // Python's `("cell", addr, width, space)`; `Identity` holds
                // neither `Addr` nor `None`, so they are spelled as their repr.
                let root = Identity::Tuple(vec![
                    Identity::Str("cell".to_owned()),
                    Identity::Str(addr.repr()),
                    Identity::Int(i64::from(r#ref.width)),
                    r#ref
                        .space
                        .map_or_else(|| Identity::Str("None".to_owned()), Identity::Space),
                ]);
                Some(_Affine {
                    root: Some(root),
                    offset: 0,
                })
            }
            _ => None,
        }
    }

    fn combined(kind: Kind, left: &_Affine, right: &_Affine) -> Option<_Affine> {
        if kind == Kind::Add {
            if left.root.is_some() && right.root.is_some() {
                return None;
            }
            return Some(_Affine {
                root: if left.root.is_some() {
                    left.root.clone()
                } else {
                    right.root.clone()
                },
                offset: left.offset + right.offset,
            });
        }
        if kind == Kind::Sub && right.root.is_none() {
            return Some(_Affine {
                root: left.root.clone(),
                offset: left.offset - right.offset,
            });
        }
        None
    }

    fn value(
        one: Value,
        definitions: &IndexMap<Value, &Op>,
        cache: &mut HashMap<Value, Option<_Affine>>,
        active: &mut HashSet<Value>,
    ) -> Option<_Affine> {
        if let Some(found) = cache.get(&one) {
            return found.clone();
        }
        if active.contains(&one) {
            return None;
        }
        active.insert(one);
        let mut result = None;
        if let Some(op) = definitions.get(&one) {
            if !op.barrier() && !op.volatile {
                let args = op
                    .args
                    .iter()
                    .map(|arg| operand(arg, definitions, cache, active))
                    .collect::<Vec<_>>();
                if args.iter().all(Option::is_some) {
                    let known = args.into_iter().flatten().collect::<Vec<_>>();
                    result = match op.kind {
                        Kind::Copy | Kind::Load | Kind::ZeroExtend => {
                            if known.len() == 1 {
                                Some(known[0].clone())
                            } else {
                                None
                            }
                        }
                        Kind::Add | Kind::Sub if known.len() == 2 => {
                            combined(op.kind, &known[0], &known[1])
                        }
                        Kind::Increment if known.len() == 1 => Some(_Affine {
                            offset: known[0].offset + 1,
                            ..known[0].clone()
                        }),
                        Kind::Decrement if known.len() == 1 => Some(_Affine {
                            offset: known[0].offset - 1,
                            ..known[0].clone()
                        }),
                        _ => None,
                    };
                }
            }
        }
        active.remove(&one);
        cache.insert(one, result.clone());
        result
    }

    let values = definitions.keys().copied().collect::<Vec<_>>();
    values
        .into_iter()
        .filter_map(|one| value(one, &definitions, &mut cache, &mut active).map(|fact| (one, fact)))
        .collect()
}

fn _rewritten_refs(
    body: &MirBody,
    reference: impl Fn(&MemRef) -> MemRef,
    skip: impl Fn(&Op) -> bool,
) -> MirBody {
    let operand = |arg: &Arg| match arg {
        Arg::Cell(cell) => Arg::Cell(Cell {
            r#ref: reference(&cell.r#ref),
        }),
        other => other.clone(),
    };
    MirBody {
        blocks: body
            .blocks
            .iter()
            .map(|block| MirBlock {
                ops: block
                    .ops
                    .iter()
                    .map(|op| {
                        if skip(op) {
                            return op.clone();
                        }
                        Op {
                            loads: op.loads.iter().map(&reference).collect(),
                            stores: op.stores.iter().map(&reference).collect(),
                            args: op.args.iter().map(operand).collect(),
                            results: op.results.iter().map(operand).collect(),
                            memory_values: op
                                .memory_values
                                .iter()
                                .map(|(r#ref, value)| (reference(r#ref), value.clone()))
                                .collect(),
                            ..op.clone()
                        }
                    })
                    .collect(),
                ..block.clone()
            })
            .collect(),
        ..body.clone()
    }
}

/// Attach exact relative leaves to affine accesses of one allocation.
///
/// `MemRef.allocation` proves an access is inside the owning allocation.
/// Grouping by opaque root and normalizing constant differences makes those
/// byte ranges canonical provenance; the minimum offset is only an origin.
pub(crate) fn _allocation_leaves(body: &MirBody) -> MirBody {
    let mut requests = IndexMap::<Symbol, Vec<(i64, i64)>>::new();
    for op in body.blocks.iter().flat_map(|block| &block.ops) {
        let Some(request) = &op.array else { continue };
        if request.replaces || request.element_width == 0 {
            continue;
        }
        let count = request
            .bounds
            .iter()
            .fold(BigInt::from(1), |product, (low, high)| {
                product * BigInt::from(high - low + 1)
            });
        let extent = BigInt::from(request.element_width) * &count;
        if count > BigInt::from(0) && BigInt::from(0) < extent && extent < BigInt::from(1_i64 << 31)
        {
            let generation = op.id.map_or(op.at, i64::from);
            requests
                .entry(request.descriptor)
                .or_default()
                .push((generation, i64::try_from(extent).expect("below 2**31")));
        }
    }
    let unique = requests
        .into_iter()
        .filter(|(_, found)| found.len() == 1)
        .map(|(descriptor, found)| (descriptor, found[0]))
        .collect::<IndexMap<_, _>>();
    if unique.is_empty() {
        return body.clone();
    }

    let affine = _affine_values(body);
    let mut grouped = IndexMap::<(Symbol, i64, i64, Identity), Vec<(MemRef, i64)>>::new();
    for op in body.blocks.iter().flat_map(|block| &block.ops) {
        for r#ref in op.loads.iter().chain(&op.stores) {
            let request = r#ref
                .allocation
                .and_then(|allocation| unique.get(&allocation));
            let fact = r#ref.base.and_then(|base| affine.get(&base));
            let (Some(&(generation, extent)), Some(fact)) = (request, fact) else {
                continue;
            };
            let (Some(root), Some(addr)) = (&fact.root, r#ref.addr) else {
                continue;
            };
            if r#ref.width == 0 {
                continue;
            }
            grouped
                .entry((
                    r#ref.allocation.expect("requested"),
                    generation,
                    extent,
                    root.clone(),
                ))
                .or_default()
                .push((r#ref.clone(), fact.offset + addr.disp));
        }
    }

    let mut exact = IndexMap::<MemRef, Provenance>::new();
    for ((descriptor, generation, extent, root), accesses) in grouped {
        let origin = accesses
            .iter()
            .map(|(_, offset)| *offset)
            .min()
            .expect("grouped");
        if accesses.iter().any(|(r#ref, offset)| {
            !(0 <= offset - origin && offset - origin <= extent - i64::from(r#ref.width))
        }) {
            continue;
        }
        let object = MemoryObject {
            kind: MemoryKind::Allocation,
            identity: Some(Identity::Tuple(vec![
                Identity::Symbol(descriptor),
                Identity::Int(generation),
                root,
            ])),
            generation: 0,
            extent: Some(extent),
        };
        for (r#ref, offset) in accesses {
            let low = offset - origin;
            let width = i64::from(r#ref.width);
            exact.insert(r#ref, one(object.clone(), low, low + width));
        }
    }
    if exact.is_empty() {
        return body.clone();
    }

    let reference = |r#ref: &MemRef| match exact.get(r#ref) {
        Some(provenance) => MemRef {
            provenance: Some(provenance.clone()),
            ..r#ref.clone()
        },
        None => r#ref.clone(),
    };
    _rewritten_refs(body, reference, |_| false)
}

/// Give one singleton indexed access its exact object slice.
///
/// Whole-object provenance establishes identity, the range the byte offset,
/// and the object's extent that the addition stays inside it.
pub(crate) fn _bounded_ref(r#ref: &MemRef, known: &IndexMap<Value, Interval>) -> MemRef {
    let (Some(base), Some(addr), Some(provenance)) = (r#ref.base, r#ref.addr, &r#ref.provenance)
    else {
        return r#ref.clone();
    };
    let Some(source) = only_slice(provenance) else {
        return r#ref.clone();
    };
    let Some(interval) = known.get(&base) else {
        return r#ref.clone();
    };
    if interval.width != r#ref.base_width || interval.low != interval.high {
        return r#ref.clone();
    }
    let Some(extent) = source.object.extent else {
        return r#ref.clone();
    };
    // Only whole-object provenance can be narrowed this way.
    if source.low > 0 || source.high < extent {
        return r#ref.clone();
    }
    let Ok(low) = i64::try_from(BigInt::from(addr.disp) + &interval.low) else {
        return r#ref.clone();
    };
    let high = low + i64::from(r#ref.width);
    if !(0 <= low && low < high && high <= extent) {
        return r#ref.clone();
    }
    let provenance = Provenance {
        slices: BTreeSet::from([slice(source.object.clone(), low, high)]),
        restrict: provenance.restrict.clone(),
    };
    MemRef {
        provenance: Some(provenance),
        ..r#ref.clone()
    }
}

/// Give an exact object-relative pointer access its scalar leaf.
///
/// Used only when the pointer fact denotes one exact byte inside the same
/// bounded object the reference's provenance names, and that provenance's
/// lanes cover every byte of the access.
pub(crate) fn _pointed_ref(r#ref: &MemRef, pointers: &IndexMap<Value, Provenance>) -> MemRef {
    let (Some(base), Some(provenance)) = (r#ref.base, &r#ref.provenance) else {
        return r#ref.clone();
    };
    let Some(source) = only_slice(provenance) else {
        return r#ref.clone();
    };
    let Some(address) = pointers.get(&base).and_then(only_slice) else {
        return r#ref.clone();
    };
    if address.object != source.object
        || address.stride != 1
        || address.width != 1
        || address.high - address.low != 1
    {
        return r#ref.clone();
    }
    let low = address.low + r#ref.addr.map_or(0, |addr| addr.disp);
    let high = low + i64::from(r#ref.width);
    let Some(extent) = address.object.extent else {
        return r#ref.clone();
    };
    if !(0 <= low && low < high && high <= extent) {
        return r#ref.clone();
    }
    // The annotation may be the conservative lane of the original indexed
    // expression; an exact same-object pointer refines it only when that
    // lane covers every byte of the access.
    let covered = |byte: i64| {
        (0..source.width).any(|lane| {
            source.low <= byte - lane
                && byte - lane < source.high
                && (byte - lane - source.low).rem_euclid(source.stride) == 0
        })
    };
    if !(low..high).all(covered) {
        return r#ref.clone();
    }
    let exact = Provenance {
        slices: BTreeSet::from([slice(source.object.clone(), low, high)]),
        restrict: provenance.restrict.clone(),
    };
    MemRef {
        provenance: Some(exact),
        ..r#ref.clone()
    }
}

/// Materialize exact singleton proofs on every indexed-ref occurrence.
///
/// Constant propagation already computes every singleton expression; the
/// heavier loop-range analysis added compile time and no leaf.
pub(crate) fn _bounded_leaves(body: &MirBody) -> Result<MirBody, String> {
    let constants = singletons(body);
    let pointers = alias::points_to(body, None, None)?.values;
    let reference = |r#ref: &MemRef| _pointed_ref(&_bounded_ref(r#ref, &constants), &pointers);
    Ok(_rewritten_refs(body, reference, |_| false))
}

/// Attach an established scalar type to an otherwise untyped same-size leaf.
///
/// A C aggregate move is byte-typed at the raise while a later field access
/// carries the field's type; they are the same bytes.  Two explicit,
/// distinct type classes retain the union/type-pun rejection.
pub(crate) fn _canonical_leaf_types(body: &MirBody) -> MirBody {
    let mut types = IndexMap::<(MemoryObject, i64, i64), BTreeSet<String>>::new();
    for op in body.blocks.iter().flat_map(|block| &block.ops) {
        if op.source_backed || !op.absorbed.is_empty() || op.id.is_none() {
            continue;
        }
        for r#ref in op.loads.iter().chain(&op.stores) {
            if let Some(leaf) = _leaf(r#ref) {
                if let Some(type_class) = leaf.type_class {
                    types
                        .entry((leaf.object, leaf.low, leaf.high))
                        .or_default()
                        .insert(type_class);
                }
            }
        }
    }

    let known = types
        .into_iter()
        .filter(|(_, classes)| classes.len() == 1)
        .map(|(key, classes)| (key, classes.into_iter().next().expect("one")))
        .collect::<IndexMap<_, _>>();
    if known.is_empty() {
        return body.clone();
    }

    let reference = |r#ref: &MemRef| {
        let Some(leaf) = _leaf(r#ref) else {
            return r#ref.clone();
        };
        if r#ref.typed.is_some() {
            return r#ref.clone();
        }
        match known.get(&(leaf.object, leaf.low, leaf.high)) {
            Some(type_class) => MemRef {
                typed: Some((type_class.clone(), false)),
                ..r#ref.clone()
            },
            None => r#ref.clone(),
        }
    };
    _rewritten_refs(body, reference, |op| {
        op.source_backed || !op.absorbed.is_empty() || op.id.is_none()
    })
}

/// The exact scalar partition of a whole-object move destination, if any.
pub(crate) fn _copy_partition(r#ref: &MemRef, leaves: &[_Leaf]) -> Option<Vec<_Leaf>> {
    let whole = _leaf(r#ref)?;
    let mut pieces = Vec::<_Leaf>::new();
    for leaf in leaves {
        if leaf.object == whole.object
            && whole.low <= leaf.low
            && leaf.low < leaf.high
            && leaf.high <= whole.high
            && (leaf.low, leaf.high) != (whole.low, whole.high)
            && !pieces.contains(leaf)
        {
            pieces.push(leaf.clone());
        }
    }
    pieces.sort_by_key(|leaf| (leaf.low, leaf.high));
    if pieces.len() < 2 {
        return None;
    }
    let mut at = whole.low;
    for piece in &pieces {
        if piece.low != at {
            return None;
        }
        at = piece.high;
    }
    (at == whole.high).then_some(pieces)
}

/// One direct scalar byte range of a proven whole-object access.
pub(crate) fn _copy_piece(r#ref: &MemRef, whole: &_Leaf, piece: &_Leaf, low: i64) -> MemRef {
    let addr = r#ref.addr.expect("a direct copy");
    MemRef {
        addr: Some(addr.plus(low - whole.low)),
        width: u32::try_from(piece.high - piece.low).expect("a scalar leaf"),
        typed: piece
            .type_class
            .clone()
            .map(|type_class| (type_class, false)),
        provenance: Some(one(whole.object.clone(), low, low + piece.high - piece.low)),
        ..r#ref.clone()
    }
}

/// Expand a proven exact aggregate copy into its existing scalar leaves.
///
/// Both sides must be exact, disjoint references and the destination's
/// byte range must already have a contiguous scalar partition.  Far,
/// indexed, volatile, overlapping or incompletely partitioned copies stay
/// whole, as do source-backed instructions.
pub(crate) fn _split_copies(body: &MirBody) -> MirBody {
    let leaves = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.loads.iter().chain(&op.stores))
        .filter_map(_leaf)
        .collect::<Vec<_>>();
    let mut fresh = _next(body);
    let mut variable = ssa::values(body).map(|one| one.variable).max().unwrap_or(0) + 1;
    let mut changed = false;
    let mut blocks = Vec::new();

    for block in &body.blocks {
        let mut ops = Vec::new();
        let mut index = 0;
        while index < block.ops.len() {
            let load = &block.ops[index];
            let store = block.ops.get(index + 1);
            let Some((source, destination, source_leaf, destination_leaf, pieces)) =
                _copy_candidate(load, store, &leaves)
            else {
                ops.push(load.clone());
                index += 1;
                continue;
            };
            let store = store.expect("a candidate has a store");
            for piece in &pieces {
                let source_piece = _copy_piece(
                    &source,
                    &source_leaf,
                    piece,
                    source_leaf.low + piece.low - destination_leaf.low,
                );
                let destination_piece =
                    _copy_piece(&destination, &destination_leaf, piece, piece.low);
                let width = u32::try_from(piece.high - piece.low).expect("a scalar leaf");
                let value = Value {
                    variable,
                    version: 1,
                    ..Value::new(fresh, load.at)
                };
                fresh += 1;
                variable += 1;
                let held = Held { value, width };
                ops.push(Op {
                    defines: vec![value],
                    uses: [source_piece.base, source_piece.segment]
                        .into_iter()
                        .flatten()
                        .collect(),
                    loads: vec![source_piece.clone()],
                    stores: vec![],
                    args: vec![Arg::Cell(Cell {
                        r#ref: source_piece,
                    })],
                    results: vec![Arg::Held(held)],
                    source_backed: false,
                    raised: None,
                    id: None,
                    absorbed: vec![],
                    symbol: None,
                    ..load.clone()
                });
                ops.push(Op {
                    defines: vec![],
                    uses: [
                        Some(value),
                        destination_piece.base,
                        destination_piece.segment,
                    ]
                    .into_iter()
                    .flatten()
                    .collect(),
                    loads: vec![],
                    stores: vec![destination_piece.clone()],
                    args: vec![Arg::Held(held)],
                    results: vec![Arg::Cell(Cell {
                        r#ref: destination_piece,
                    })],
                    source_backed: false,
                    raised: None,
                    id: None,
                    absorbed: vec![],
                    symbol: None,
                    ..store.clone()
                });
            }
            changed = true;
            index += 2;
        }
        blocks.push(MirBlock {
            ops,
            ..block.clone()
        });
    }
    if changed {
        MirBody {
            blocks,
            ..body.clone()
        }
    } else {
        body.clone()
    }
}

/// The proof for one adjacent exact C aggregate copy, otherwise `None`.
#[allow(clippy::type_complexity)]
pub(crate) fn _copy_candidate(
    load: &Op,
    store: Option<&Op>,
    leaves: &[_Leaf],
) -> Option<(MemRef, MemRef, _Leaf, _Leaf, Vec<_Leaf>)> {
    let store = store?;
    if load.source_backed
        || store.source_backed
        || !load.absorbed.is_empty()
        || !store.absorbed.is_empty()
        || load.id.is_none()
        || store.id.is_none()
        || load.volatile
        || store.volatile
    {
        return None;
    }
    let same = matches!((load.results.first(), store.args.first()), (Some(Arg::Held(one)), Some(Arg::Held(other))) if one.value == other.value);
    if load.kind != Kind::Load
        || store.kind != Kind::Store
        || load.loads.len() != 1
        || !load.stores.is_empty()
        || store.stores.len() != 1
        || !store.loads.is_empty()
        || load.results.len() != 1
        || store.args.len() != 1
        || !same
        || !matches!(load.results[0], Arg::Held(result) if load.defines == [result.value])
        || !store.defines.is_empty()
    {
        return None;
    }
    let (source, destination) = (&load.loads[0], &store.stores[0]);
    let (Some(source_leaf), Some(destination_leaf)) = (_leaf(source), _leaf(destination)) else {
        return None;
    };
    let (Some(source_addr), Some(destination_addr)) = (source.addr, destination.addr) else {
        return None;
    };
    if source.width != destination.width
        || source.volatile
        || destination.volatile
        || source.pointer
        || destination.pointer
        || destination.segment.is_some()
        || !source_addr.direct()
        || !destination_addr.direct()
        || memory::objects_may_alias(&source_leaf.object, &destination_leaf.object)
    {
        return None;
    }
    let source_inputs = [source.base, source.segment]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let Arg::Held(stored) = store.args[0] else {
        return None;
    };
    let destination_inputs = [Some(stored.value), destination.base, destination.segment]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if load.uses != source_inputs || store.uses != destination_inputs {
        return None;
    }
    let pieces = _copy_partition(destination, leaves)?;
    Some((
        source.clone(),
        destination.clone(),
        source_leaf,
        destination_leaf,
        pieces,
    ))
}

/// Cells whose reads can use a known stored value, by address.
///
/// Touched more than once, supported reads agreeing on one width, and
/// available along every path.
pub(crate) fn promotable(
    body: &MirBody,
    dgroup: &BTreeSet<i64>,
    bounds: Option<&Bounds>,
    aggregate_only: bool,
) -> IndexMap<Key, u32> {
    let every = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.loads.iter().chain(&op.stores))
        .collect::<Vec<_>>();
    let blocked = _blocked_objects(&every);
    let mut seen = IndexMap::<Key, usize>::new();
    let mut widths = HashMap::<Key, BTreeSet<u32>>::new();
    for one in &every {
        let Some(key) = _key(one, &blocked) else {
            continue;
        };
        *seen.entry(key).or_default() += 1;
    }
    for op in body.blocks.iter().flat_map(|block| &block.ops) {
        if op.loads.is_empty() {
            continue;
        }
        if let Some(r#ref) = _cell(op) {
            if let Some(key) = _key(&r#ref, &blocked) {
                widths.entry(key).or_default().insert(r#ref.width);
            }
        }
    }

    let mut candidates = seen
        .into_iter()
        .filter(|(addr, times)| {
            *times > 1 && widths.get(addr).is_some_and(|found| found.len() == 1)
        })
        .map(|(addr, _)| {
            let width = *widths[&addr].iter().next().expect("one width");
            (addr, width)
        })
        .collect::<IndexMap<_, _>>();
    if aggregate_only {
        let aggregates = _aggregate_objects(candidates.keys());
        candidates
            .retain(|addr, _| matches!(addr, Key::Leaf(leaf) if aggregates.contains(&leaf.object)));
    }
    let usable = _available(body, &candidates, dgroup, bounds);
    let mut used = HashSet::new();
    for (block_index, block) in body.blocks.iter().enumerate() {
        for (op_index, op) in block.ops.iter().enumerate() {
            if usable.contains(&(block_index, op_index)) {
                used.extend(op.loads.iter().map(|r#ref| _key(r#ref, &blocked)));
            }
        }
    }
    candidates
        .into_iter()
        .filter(|(addr, _)| used.contains(&Some(addr.clone())))
        .collect()
}

/// The whole access this pass can replace, never part of an operation.
pub(crate) fn _cell(op: &Op) -> Option<MemRef> {
    let empty = BTreeSet::new();
    match (op.loads.as_slice(), op.stores.as_slice()) {
        ([r#ref], []) if READS.contains(&op.kind) => _key(r#ref, &empty).map(|_| r#ref.clone()),
        ([], [r#ref]) if op.kind == Kind::Store => _key(r#ref, &empty).map(|_| r#ref.clone()),
        _ => None,
    }
}

/// Complete scalar constants established by intact, possibly split stores.
pub(crate) fn _initializers(
    body: &MirBody,
    cells: &IndexMap<Key, u32>,
    dgroup: &BTreeSet<i64>,
    bounds: Option<&Bounds>,
) -> HashMap<(usize, usize), IndexMap<Key, Known>> {
    let layout = layout(bounds);
    let calls = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|op| op.barrier() || op.kind == Kind::Call)
        .map(|op| (op.at, String::new()))
        .collect::<IndexMap<_, _>>();
    let memory = consts::cells(body, dgroup, &calls, None, None, None, None, None);
    let nothing = BTreeMap::new();
    let mut initialized = HashMap::new();
    for (block_index, block) in body.blocks.iter().enumerate() {
        for (index, op) in block.ops.iter().enumerate() {
            let cell = _cell(op);
            let Some(cell) = cell.filter(|cell| {
                op.kind == Kind::Store
                    && !op.barrier()
                    && cell.base.is_none()
                    && cell.segment.is_none()
                    && cell.addr.is_some_and(|addr| CELLS.contains(&addr.space))
            }) else {
                continue;
            };
            let before = memory.get(&(block.at, index)).cloned().unwrap_or_default();
            let after = consts::_kills(
                &before, op, &nothing, dgroup, &calls, None, None, false, None,
            );
            let mut facts = IndexMap::new();
            for (addr, width) in cells {
                let Key::Addr(addr) = addr else { continue };
                if (Some(*addr), *width) == (cell.addr, cell.width) {
                    continue;
                }
                let whole = MemRef::new(Some(*addr), *width);
                if !overlapping(&whole, &cell, layout.as_ref()) {
                    continue;
                }
                if let Some(fact) = consts::_cell(&after, &whole) {
                    facts.insert(Key::Addr(*addr), fact);
                }
            }
            initialized.insert((block_index, index), facts);
        }
    }
    initialized
}

/// Reads reached by a stored value on every path, without an intervening aliasing write.
pub(crate) fn _available(
    body: &MirBody,
    cells: &IndexMap<Key, u32>,
    dgroup: &BTreeSet<i64>,
    bounds: Option<&Bounds>,
) -> HashSet<(usize, usize)> {
    let layout = layout(bounds);
    let reachable = loops::dominators(&body.blocks, body.entry)
        .into_iter()
        .filter(|(_, doms)| !doms.is_empty())
        .map(|(at, _)| at)
        .collect::<BTreeSet<_>>();
    let predecessors = loops::predecessors(&body.blocks);
    let mut leaving = reachable
        .iter()
        .map(|at| (*at, cells.keys().cloned().collect::<HashSet<_>>()))
        .collect::<BTreeMap<_, _>>();
    let refs = cells
        .iter()
        .map(|(addr, width)| (addr.clone(), _reference(addr, *width)))
        .collect::<IndexMap<_, _>>();
    let initializers = _initializers(body, cells, dgroup, bounds);

    let entering = |at: i64, leaving: &BTreeMap<i64, HashSet<Key>>| -> HashSet<Key> {
        let parents = predecessors
            .get(&at)
            .into_iter()
            .flatten()
            .filter(|parent| reachable.contains(parent))
            .collect::<Vec<_>>();
        if parents.is_empty() || at == body.entry {
            return HashSet::new();
        }
        let mut result = leaving[parents[0]].clone();
        for parent in &parents[1..] {
            result.retain(|key| leaving[*parent].contains(key));
        }
        result
    };

    let through = |block_index: usize,
                   mut available: HashSet<Key>,
                   mut reads: Option<&mut HashSet<(usize, usize)>>| {
        let block = &body.blocks[block_index];
        let redefined = |available: &mut HashSet<Key>, values: &HashSet<Value>| {
            available.retain(|key| {
                let r#ref = &refs[key];
                !(r#ref.base.is_some_and(|base| values.contains(&base))
                    || r#ref
                        .segment
                        .is_some_and(|segment| values.contains(&segment)))
            });
        };

        redefined(
            &mut available,
            &block.phis.iter().map(|phi| phi.result).collect(),
        );
        for (op_index, op) in block.ops.iter().enumerate() {
            if effects::unmodeled_write(op) {
                available.clear();
                continue;
            }
            let cell = _cell(op);
            let key = cell.as_ref().and_then(|cell| _key(cell, &BTreeSet::new()));
            if let (Some(reads), Some(cell), Some(key)) = (reads.as_deref_mut(), &cell, &key) {
                if !op.loads.is_empty() && available.contains(key) && cell.width == cells[key] {
                    reads.insert((block_index, op_index));
                }
            }
            redefined(&mut available, &op.defines.iter().copied().collect());
            available.retain(|addr| {
                !op.stores
                    .iter()
                    .any(|written| overlapping(&refs[addr], written, layout.as_ref()))
            });
            if let (Some(cell), Some(key)) = (&cell, &key) {
                if op.kind == Kind::Store
                    && cells.get(key).is_some_and(|width| cell.width == *width)
                {
                    available.insert(key.clone());
                }
            }
            if let Some(initialized) = initializers.get(&(block_index, op_index)) {
                available.extend(initialized.keys().cloned());
            }
        }
        available
    };

    loop {
        let mut changed = false;
        for (block_index, block) in body.blocks.iter().enumerate() {
            if !reachable.contains(&block.at) {
                continue;
            }
            let result = through(block_index, entering(block.at, &leaving), None);
            if leaving.get(&block.at) != Some(&result) {
                leaving.insert(block.at, result);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut reads = HashSet::new();
    for (block_index, block) in body.blocks.iter().enumerate() {
        if reachable.contains(&block.at) {
            through(block_index, entering(block.at, &leaving), Some(&mut reads));
        }
    }
    reads
}

/// Reuse eligible stored values without removing observable writes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn promoted(
    body: &MirBody,
    dgroup: &BTreeSet<i64>,
    bounds: Option<&Bounds>,
    loop_only: bool,
    split_updates: bool,
    aggregate_only: bool,
) -> Result<MirBody, String> {
    let original = body;
    let mut aggregate_objects = None;
    if aggregate_only {
        let refs = body
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .flat_map(|op| op.loads.iter().chain(&op.stores))
            .collect::<Vec<_>>();
        let blocked = _blocked_objects(&refs);
        let keys = refs
            .iter()
            .filter_map(|r#ref| _key(r#ref, &blocked))
            .collect::<Vec<_>>();
        let objects = _aggregate_objects(&keys);
        if objects.is_empty() {
            return Ok(original.clone());
        }
        aggregate_objects = Some(objects);
    }
    let separated;
    let body = if split_updates {
        separated = _separated(body, aggregate_objects.as_ref());
        &separated
    } else {
        body
    };
    let mut found = promotable(body, dgroup, bounds, aggregate_only);
    if loop_only {
        let hot = loops::loops(&body.blocks, Some(body.entry))
            .into_iter()
            .flat_map(|loop_| loop_.body)
            .collect::<BTreeSet<_>>();
        let read = body
            .blocks
            .iter()
            .filter(|block| hot.contains(&block.at))
            .flat_map(|block| &block.ops)
            .flat_map(|op| &op.loads)
            .map(|r#ref| _key(r#ref, &BTreeSet::new()))
            .collect::<HashSet<_>>();
        found.retain(|addr, _| read.contains(&Some(addr.clone())));
    }
    if found.is_empty() {
        return Ok(original.clone());
    }
    let usable = _available(body, &found, dgroup, bounds);
    let updates = original
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|op| !op.loads.is_empty() && !op.stores.is_empty())
        .map(|op| op.id)
        .collect::<HashSet<_>>();
    if split_updates
        && body.blocks.iter().enumerate().any(|(block_index, block)| {
            block.ops.iter().enumerate().any(|(op_index, op)| {
                updates.contains(&op.id)
                    && !op.loads.is_empty()
                    && op.stores.is_empty()
                    && !usable.contains(&(block_index, op_index))
            })
        })
    {
        return promoted(original, dgroup, bounds, loop_only, false, aggregate_only);
    }

    let taken = ssa::values(body).map(|one| one.variable).max().unwrap_or(0);
    let mut fresh = _next(body);
    let mut sorted = found.keys().cloned().collect::<Vec<_>>();
    sorted.sort_by_cached_key(_order);
    let mut holds = IndexMap::<Key, u32>::new();
    for (number, addr) in (1..).zip(sorted) {
        holds.insert(addr, taken + number);
    }

    let mut changed = false;
    let initializers = _initializers(body, &found, dgroup, bounds);
    let mut blocks = Vec::new();
    for (block_index, block) in body.blocks.iter().enumerate() {
        let mut ops = Vec::new();
        for (op_index, op) in block.ops.iter().enumerate() {
            let here = (block_index, op_index);
            let initialized = initializers
                .get(&here)
                .filter(|initialized| !initialized.is_empty());
            if let Some(initialized) = initialized {
                ops.push(op.clone());
                if let Some(exact) = _instead(op, &holds, &found, fresh) {
                    fresh += 1;
                    ops.push(Op {
                        source_backed: false,
                        id: None,
                        absorbed: vec![],
                        symbol: Some(false),
                        ..exact
                    });
                }
                for (addr, fact) in initialized {
                    let value = Value {
                        variable: holds[addr],
                        version: 1,
                        ..Value::new(fresh, op.at)
                    };
                    fresh += 1;
                    ops.push(Op {
                        kind: Kind::Copy,
                        name: "mov".to_owned(),
                        defines: vec![value],
                        uses: vec![],
                        loads: vec![],
                        stores: vec![],
                        merges: OrderedMap::new(),
                        args: vec![Arg::Const(Const::new(fact.n.clone(), fact.width))],
                        results: vec![Arg::Held(Held {
                            value,
                            width: fact.width,
                        })],
                        source_backed: false,
                        raised: None,
                        id: None,
                        absorbed: vec![],
                        symbol: Some(false),
                        ..op.clone()
                    });
                }
                changed = true;
                continue;
            }
            if !op.loads.is_empty() && !usable.contains(&here) {
                ops.push(op.clone());
                continue;
            }
            let Some(mut made) = _instead(op, &holds, &found, fresh) else {
                ops.push(op.clone());
                continue;
            };
            fresh += 1;
            changed = true;
            if !op.stores.is_empty() {
                ops.push(op.clone());
                made = Op {
                    source_backed: false,
                    id: None,
                    absorbed: vec![],
                    symbol: Some(false),
                    ..made
                };
            }
            ops.push(made);
        }
        blocks.push(MirBlock {
            ops,
            ..block.clone()
        });
    }
    if !changed {
        return Ok(body.clone());
    }
    ssa::constructed(
        &MirBody {
            blocks,
            ..body.clone()
        },
        &holds.values().copied().collect(),
    )
    .map_err(|error| error.to_string())
}

/// Expose a memory update as a value computation and an observable store.
pub(crate) fn _separated(body: &MirBody, objects: Option<&BTreeSet<MemoryObject>>) -> MirBody {
    let mut fresh = _next(body);
    let mut variable = ssa::values(body).map(|one| one.variable).max().unwrap_or(0) + 1;
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut ops = Vec::new();
        for op in &block.ops {
            let leaf = if op.loads.len() == 1 {
                _leaf(&op.loads[0])
            } else {
                None
            };
            if op.kind == Kind::Load
                || !READS.contains(&op.kind)
                || op.loads.len() != 1
                || op.loads != op.stores
                || op.results
                    != [Arg::Cell(Cell {
                        r#ref: op.loads[0].clone(),
                    })]
                || op.defines.iter().any(|value| !value.flags)
                || (objects.is_none()
                    && (op.loads[0].base.is_some() || op.loads[0].segment.is_some()))
                || objects.is_some_and(|objects| {
                    leaf.as_ref()
                        .is_none_or(|leaf| !objects.contains(&leaf.object))
                })
            {
                ops.push(op.clone());
                continue;
            }
            let result = Value {
                variable,
                version: 1,
                ..Value::new(fresh, op.at)
            };
            fresh += 1;
            variable += 1;
            let held = Held {
                value: result,
                width: op.loads[0].width,
            };
            ops.push(Op {
                results: vec![Arg::Held(held)],
                stores: vec![],
                defines: std::iter::once(result)
                    .chain(op.defines.iter().copied())
                    .collect(),
                symbol: Some(false),
                ..op.clone()
            });
            ops.push(Op {
                kind: Kind::Store,
                name: "mov".to_owned(),
                args: vec![Arg::Held(held)],
                loads: vec![],
                defines: vec![],
                uses: [Some(result), op.loads[0].base, op.loads[0].segment]
                    .into_iter()
                    .flatten()
                    .collect(),
                source_backed: false,
                raised: None,
                absorbed: vec![],
                merges: OrderedMap::new(),
                symbol: Some(true),
                ..op.clone()
            });
        }
        blocks.push(MirBlock {
            ops,
            ..block.clone()
        });
    }
    MirBody {
        blocks,
        ..body.clone()
    }
}

/// The value operation paired with a store, or replacing a memory read.
///
/// Only where the cell is the operation's whole memory traffic: a half
/// rewritten operation reads stale memory.
pub(crate) fn _instead(
    op: &Op,
    holds: &IndexMap<Key, u32>,
    found: &IndexMap<Key, u32>,
    fresh: u32,
) -> Option<Op> {
    let cell = _cell(op)?;
    let addr = _key(&cell, &BTreeSet::new())?;
    let &variable = holds.get(&addr)?;
    let width = found[&addr];
    if cell.width != width {
        return None;
    }

    if !op.stores.is_empty() && op.loads.is_empty() {
        // `mov [x],ax` is `x := ax`, and x is a variable now.
        let into = Value {
            variable,
            version: 1,
            ..Value::new(fresh, op.at)
        };
        return Some(Op {
            kind: Kind::Copy,
            name: "mov".to_owned(),
            defines: std::iter::once(into)
                .chain(op.defines.iter().filter(|one| one.flags).copied())
                .collect(),
            stores: vec![],
            results: vec![Arg::Held(Held { value: into, width })],
            args: op
                .args
                .iter()
                .filter(|one| !matches!(one, Arg::Cell(_)))
                .cloned()
                .collect(),
            uses: op
                .args
                .iter()
                .filter_map(|one| match one {
                    Arg::Held(held) => Some(held.value),
                    _ => None,
                })
                .collect(),
            ..op.clone()
        });
    }

    if !op.loads.is_empty() {
        // `mov ax,[x]` is `ax := x`.  The version is a placeholder:
        // `ssa::constructed` renames per variable.
        let holding = Value {
            variable,
            version: 1,
            ..Value::new(fresh, op.at)
        };
        let mut uses = Vec::new();
        for one in op
            .args
            .iter()
            .filter_map(|one| match one {
                Arg::Held(held) => Some(held.value),
                _ => None,
            })
            .chain(op.uses.iter().filter(|one| one.flags).copied())
            .chain([holding])
        {
            if !uses.contains(&one) {
                uses.push(one);
            }
        }
        return Some(Op {
            kind: if op.kind == Kind::Load {
                Kind::Copy
            } else {
                op.kind
            },
            uses,
            loads: vec![],
            args: op
                .args
                .iter()
                .map(|one| match one {
                    Arg::Cell(_) => Arg::Held(Held {
                        value: holding,
                        width,
                    }),
                    other => other.clone(),
                })
                .collect(),
            ..op.clone()
        });
    }
    None
}

/// An id nothing in this body uses.
pub(crate) fn _next(body: &MirBody) -> u32 {
    ssa::values(body).map(|one| one.id).max().unwrap_or(0) + 1
}

#[cfg(test)]
#[path = "promote_tests.rs"]
mod tests;
