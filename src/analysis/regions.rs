//! What a reference can reach, as a set instead of several partial facts.
//!
//! Direct port of the non-provenance portion of
//! `qbopt/analysis/regions.py`: `RegionSet`, `_under`, `_meets`,
//! `_surviving`, `_absolute`, `_region`, `_floor`, `_at`, `addressed`,
//! `_holes`, `_spans`, `regions`, `_same_typed_start`, `typed_apart`, and
//! `addresses`, and `may_alias`.  The public MIR overlapping entry point is
//! deliberately separate: this module answers only the underlying alias
//! query, exactly as Python `qbopt.analysis.regions` does.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use num_bigint::BigInt;

use crate::analysis::ranges::{Interval, covering};
use crate::analysis::cellmap::Bucket;
use crate::model::memory::{
    AliasClass, Identity, MemoryKind, MemoryObject, Provenance, Slice, SliceError, alias_class, classes_may_alias,
};
use num_traits::ToPrimitive;

use crate::support::hash::{HashMap, HashSet};
use crate::model::mir::{MemRef, Reach, Symbol, Value, symbolic_ref};
use crate::objectfile::module::{Addr, Space};

const FLOOR: i64 = -(1_i64 << 31);
const CEILING: i64 = 1_i64 << 31;
const WHOLE: (i64, i64) = (FLOOR, CEILING);

/// One component of a Python region path.
///
/// `Allocation` keeps allocation identity structural rather than turning it
/// into a display string.  The remaining variants correspond to Python's
/// `"seg:{index}"`, `"ext:{index}"`, and literal-selector path components.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum RegionPart {
    Stack,
    Dgroup,
    Linked,
    Named,
    Absolute,
    Alloc,
    Segment(i64),
    Allocation(Symbol),
    // Only the translation to provenance uses this: DGROUP less its uncaptured segments.
    Nonlocal,
}

/// A region is a path: a coarser prefix meets each child below it.
#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct Region(pub(crate) Vec<RegionPart>);

impl Region {
    fn stack() -> Self {
        Self(vec![RegionPart::Stack])
    }

    fn dgroup() -> Self {
        Self(vec![RegionPart::Dgroup])
    }

    fn linked() -> Self {
        Self(vec![RegionPart::Dgroup, RegionPart::Linked])
    }

    fn named() -> Self {
        Self(vec![RegionPart::Named])
    }

    fn nonlocal() -> Self {
        Self(vec![RegionPart::Nonlocal])
    }

    fn under(&self, other: &Self) -> bool {
        self.0.starts_with(&other.0) || other.0.starts_with(&self.0)
    }
}

/// The displacement origin for a region span.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Origin {
    Here,
    Sp,
    Bp,
    External(i64),
}

/// A half-open byte interval in one region, counted from `origin`.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct Span {
    pub(crate) region: Region,
    pub(crate) origin: Origin,
    pub(crate) low: i64,
    pub(crate) high: i64,
}

impl Span {
    fn whole(region: Region, origin: Origin) -> Self {
        Self {
            region,
            origin,
            low: WHOLE.0,
            high: WHOLE.1,
        }
    }
}

/// The bytes a reference may reach: its spans less independently-proven holes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RegionSet {
    pub(crate) spans: BTreeSet<Span>,
    pub(crate) holes: BTreeSet<Span>,
}

impl RegionSet {
    fn new(spans: BTreeSet<Span>, holes: BTreeSet<Span>) -> Self {
        Self { spans, holes }
    }

    /// Python `RegionSet.intersects`.
    ///
    /// A span is removed only when a *single* hole covers it.  Two holes
    /// whose union covers it do not, deliberately, remove the span.
    pub(crate) fn intersects(&self, other: &Self) -> bool {
        let holes = self.holes.union(&other.holes).cloned().collect();
        surviving(&self.spans, &holes).iter().any(|one| {
            surviving(&other.spans, &holes)
                .iter()
                .any(|two| meets(one, two))
        })
    }
}

/// The only source-neutral layout facts regions consumes.
///
/// A missing layout means Python's `layout=None`: every segment is linked
/// (potentially overlaid) and there are no bounds for indexed addressing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RegionLayout {
    /// Segments using COMMON combination, therefore shared with another unit.
    ///
    /// `None` means this is bounds-only context, not a statement that no
    /// segment is COMMON.  That preserves Python's separate `bounds` and
    /// `layout` inputs.
    pub(crate) shared_segments: Option<BTreeSet<i64>>,
    /// Sorted exact named displacements, keyed by `(Space, segment index)`.
    pub(crate) landmarks: BTreeMap<(Space, i64), Vec<i64>>,
}

/// Failure to express an otherwise-Python-sized span in an existing Rust
/// address endpoint.  Refusing retains conservatism; wrapping would narrow it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RegionError {
    EndpointOverflow,
    /// Python integers can describe a narrowed slice outside Rust's signed
    /// `Slice` endpoints.  Do not discard an otherwise-applicable fact.
    NarrowedSliceUnrepresentable,
    /// `Slice` rejects the same invalid interval shape that Python rejects
    /// when `memory.Slice` is constructed.
    InvalidNarrowedSlice(SliceError),
}

fn endpoint(low: i64, width: u32) -> Result<i64, RegionError> {
    low.checked_add(i64::from(width))
        .ok_or(RegionError::EndpointOverflow)
}

fn region(
    space: Option<Space>,
    index: Option<i64>,
    layout: Option<&RegionLayout>,
) -> (Region, Origin) {
    match space {
        Some(Space::Stack) => (Region::stack(), Origin::Sp),
        Some(Space::Frame) => (Region::stack(), Origin::Bp),
        Some(Space::Segment) => match index {
            None => (Region::dgroup(), Origin::Here),
            Some(index) => {
                let owned = layout
                    .and_then(|one| one.shared_segments.as_ref())
                    .is_some_and(|shared| !shared.contains(&index));
                let mut path = if owned {
                    Region::dgroup()
                } else {
                    Region::linked()
                };
                path.0.push(RegionPart::Segment(index));
                (path, Origin::Here)
            }
        },
        Some(Space::External) => (
            Region::linked(),
            index.map_or(Origin::Here, Origin::External),
        ),
        Some(Space::Far) if index.unwrap_or(0) != 0 => (Region::named(), Origin::Here),
        // Literal, Far without an index, Group, and a missing kind are root.
        _ => (Region::default(), Origin::Here),
    }
}

fn floor(address: Option<Addr>) -> BTreeSet<Span> {
    let Some(address) = address else {
        return BTreeSet::new();
    };
    if region(Some(address.space), Some(address.index), None).0 == Region::default() {
        BTreeSet::from([Span::whole(Region::stack(), Origin::Sp)])
    } else {
        BTreeSet::new()
    }
}

fn reach(
    address: Addr,
    width: u32,
    layout: &RegionLayout,
) -> Option<Result<(i64, i64), RegionError>> {
    if address.base == iced_x86::Register::None {
        return Some(endpoint(address.disp, width).map(|high| (address.disp, high)));
    }
    let landmarks = layout.landmarks.get(&(address.space, address.index))?;
    landmarks
        .iter()
        .copied()
        .find(|one| *one > address.disp)
        .map(|high| Ok((address.disp, high)))
}

fn at(
    address: Option<Addr>,
    width: u32,
    layout: Option<&RegionLayout>,
    indexed: bool,
) -> Result<BTreeSet<Span>, RegionError> {
    let Some(address) = address else {
        return Ok(BTreeSet::from([Span::whole(
            Region::default(),
            Origin::Here,
        )]));
    };
    let width = width.max(1);
    let span = if !indexed {
        layout
            .and_then(|one| reach(address, width, one))
            .transpose()?
    } else {
        None
    };
    let (low, high) = match span {
        Some(span) => span,
        None if address.base != iced_x86::Register::None || indexed => WHOLE,
        None => (address.disp, endpoint(address.disp, width)?),
    };
    let (region, origin) = region(Some(address.space), Some(address.index), layout);
    let (low, high) =
        if region == Region::default() || region == Region::dgroup() || region == Region::named() {
            WHOLE
        } else {
            (low, high)
        };
    Ok(BTreeSet::from([Span {
        region,
        origin,
        low,
        high,
    }]))
}

/// Python `addressed`: region facts for an address absent a `MemRef`.
pub(crate) fn addressed(
    address: Option<Addr>,
    width: u32,
    layout: Option<&RegionLayout>,
) -> Result<RegionSet, RegionError> {
    Ok(RegionSet::new(
        at(address, width, layout, false)?,
        floor(address),
    ))
}

fn holes(reference: &MemRef, layout: Option<&RegionLayout>) -> Result<BTreeSet<Span>, RegionError> {
    let mut out = BTreeSet::new();
    if let Some((owner, reaches)) = &reference.beyond {
        if !reaches
            .iter()
            .any(|(segment, _)| *segment == *owner)
        {
            let (region, origin) = region(Some(Space::Segment), Some(*owner), layout);
            out.insert(Span::whole(region, origin));
        }
    }
    for (address, width) in &reference.excludes {
        let (region, origin) = region(Some(address.space), Some(address.index), layout);
        out.insert(Span {
            region,
            origin,
            low: address.disp,
            high: endpoint(address.disp, *width)?,
        });
    }
    out.extend(floor(reference.addr));
    Ok(out)
}

/// Memory addressed linearly, which no program object occupies. Its slice
/// offsets are linear addresses, so one object covers every selector.
fn linear() -> MemoryObject {
    MemoryObject { identity: Some(Identity::Str("linear".to_owned())), ..MemoryObject::new(MemoryKind::Absolute) }
}

/// The linear bytes `reference` reaches, when its selector's range lands it
/// wholly in memory the machine keeps no program data in.
fn foreign(reference: &MemRef, known: Option<&BTreeMap<Value, Interval>>) -> Option<Slice> {
    let known = known?;
    // A selector or offset is an unsigned word; ranges may carry it signed,
    // and an offset wraps within its segment.
    let words = |low: BigInt, high: BigInt| -> Option<(i64, i64)> {
        let (low, high) = (low.to_i64()?, high.to_i64()?);
        let word = |one: i64| one.rem_euclid(0x1_0000);
        (high - low < 0x1_0000 && word(low) <= word(high)).then(|| (word(low), word(high)))
    };
    let selector = known.get(&reference.segment?)?;
    let selectors = words(selector.low.clone(), selector.high.clone())?;
    let disp = BigInt::from(reference.addr?.disp);
    let offsets = match reference.base {
        None => words(disp.clone(), disp),
        Some(base) => known
            .get(&base)
            .filter(|interval| interval.width == reference.base_width)
            .and_then(|interval| words(&disp + &interval.low, &disp + &interval.high)),
    }
    .unwrap_or((0, 0xFFFF));
    let width = i64::from(reference.width.max(1));
    let (start, end) = crate::abi::machine::current().foreign_span(selectors, offsets, width)?;
    Some(Slice::new(linear(), start, end - width + 1, 1, width).expect("a foreign span holds one access"))
}

/// What `reference` may name under `facts`: its linear bytes when its
/// segment lands it in foreign memory, else its provenance narrowed.
fn refined(
    reference: &MemRef,
    facts: Option<&BTreeMap<Value, Interval>>,
) -> Result<Option<Provenance>, RegionError> {
    if let Some(slice) = foreign(reference, facts) {
        return Ok(Some(Provenance { slices: BTreeSet::from([slice]), restrict: BTreeSet::new() }));
    }
    reference.provenance.as_ref().map(|provenance| narrowed(reference, provenance, facts)).transpose()
}

fn spans(
    reference: &MemRef,
    known: Option<&BTreeMap<Value, Interval>>,
    layout: Option<&RegionLayout>,
) -> Result<BTreeSet<Span>, RegionError> {
    if let Some(within) = reference.within.as_ref().filter(|one| !one.is_empty()) {
        return Ok(within
            .iter()
            .map(|&(low, high)| Span {
                region: Region::stack(),
                origin: Origin::Bp,
                low,
                high,
            })
            .collect());
    }
    if let Some((region, origin)) = foreign(reference, known).map(|_| (Region(vec![RegionPart::Absolute]), Origin::Here)) {
        return Ok(BTreeSet::from([Span::whole(region, origin)]));
    }
    if let Some(allocation) = reference.allocation {
        return Ok(BTreeSet::from([Span::whole(
            Region(vec![RegionPart::Alloc, RegionPart::Allocation(allocation)]),
            Origin::Here,
        )]));
    }
    if reference.pointer || reference.addr.is_none() {
        let (region, origin) = region(reference.space, None, layout);
        return Ok(BTreeSet::from([Span::whole(region, origin)]));
    }
    let address = reference.addr.expect("checked above");
    // An SSA base with no physical address base is the Python indirect-index
    // case; its own segment is still the complete possible region.
    at(
        Some(address),
        reference.width,
        layout,
        reference.base.is_some() && address.base == iced_x86::Register::None,
    )
}

/// Python `regions`, excluding its later provenance-based `may_alias` path.
pub(crate) fn regions(
    reference: &MemRef,
    known: Option<&BTreeMap<Value, Interval>>,
    layout: Option<&RegionLayout>,
) -> Result<RegionSet, RegionError> {
    Ok(RegionSet::new(
        spans(reference, known, layout)?,
        holes(reference, layout)?,
    ))
}

fn same_typed_start(one: &MemRef, other: &MemRef) -> bool {
    if let (Some(one_address), Some(other_address)) = (one.addr, other.addr) {
        if one_address.space == Space::Far && one.base.is_none() && one.segment.is_none() {
            return false;
        }
        return one_address == other_address
            && one.base == other.base
            && one.segment == other.segment
            && one.base_width == other.base_width
            && one.symbolic == other.symbolic
            && one.allocation == other.allocation;
    }
    if one.pointer && other.pointer {
        return one.base.is_some()
            && one.base == other.base
            && one.segment == other.segment
            && one.base_width == other.base_width;
    }
    let (Some(one_provenance), Some(other_provenance)) = (&one.provenance, &other.provenance)
    else {
        return false;
    };
    if one_provenance.slices.len() != 1 || other_provenance.slices.len() != 1 {
        return false;
    }
    let first = one_provenance.slices.first().expect("checked above");
    let second = other_provenance.slices.first().expect("checked above");
    first.object == second.object && first.low == second.low && first.low != FLOOR
}

/// Python `typed_apart`: incompatible scalar access types cannot alias except
/// for two views explicitly computed from the same union start.
pub(crate) fn typed_apart(one: &MemRef, other: &MemRef) -> bool {
    let (Some(one_type), Some(other_type)) = (&one.typed, &other.typed) else {
        return false;
    };
    one_type.0 != other_type.0 && !same_typed_start(one, other)
}

/// Python `may_alias`'s private `narrowed` helper.
///
/// An indexed reference with one concrete source object and a matching range
/// fact can name fewer bytes than its coarse provenance.  The range counts
/// starts, so the access width is included exactly once when calculating the
/// final byte.  BigInt retains Python arithmetic until the new `Slice` must
/// be represented by Rust's bounded endpoints.
fn narrowed(
    reference: &MemRef,
    provenance: &Provenance,
    facts: Option<&BTreeMap<Value, Interval>>,
) -> Result<Provenance, RegionError> {
    let Some(base) = reference.base else {
        return Ok(provenance.clone());
    };
    let Some(address) = reference.addr else {
        return Ok(provenance.clone());
    };
    let Some(interval) = facts.and_then(|facts| facts.get(&base)) else {
        return Ok(provenance.clone());
    };
    if interval.width != reference.base_width || provenance.slices.len() != 1 {
        return Ok(provenance.clone());
    }
    let source = provenance
        .slices
        .first()
        .expect("one source slice was checked above");
    let width = i64::from(reference.width.max(1));
    let low = BigInt::from(address.disp) + &interval.low;
    let high = BigInt::from(address.disp) + &interval.high + 1_u8;
    let end = &high + width - 1_i64;

    if let Some(extent) = source.object.extent {
        let extent = BigInt::from(extent);
        if low < BigInt::from(0_u8) || low >= high || end > extent {
            return Ok(provenance.clone());
        }
    }

    let (Ok(low), Ok(high)) = (i64::try_from(&low), i64::try_from(&high)) else {
        return Err(RegionError::NarrowedSliceUnrepresentable);
    };
    let slice = Slice::new(source.object.clone(), low, high, 1, width)
        .map_err(RegionError::InvalidNarrowedSlice)?;
    Ok(Provenance {
        slices: BTreeSet::from([slice]),
        restrict: provenance.restrict.clone(),
    })
}

/// Python `qbopt.analysis.regions:may_alias`.
///
/// Each reference gets its own interval facts.  If both have provenance,
/// their concrete object paths decide; otherwise the source-neutral region
/// lattice supplies the conservative answer.
pub(crate) fn may_alias(
    one: &MemRef,
    other: &MemRef,
    known: Option<&BTreeMap<Value, Interval>>,
    other_known: Option<&BTreeMap<Value, Interval>>,
    layout: Option<&RegionLayout>,
) -> Result<bool, RegionError> {
    if typed_apart(one, other) {
        return Ok(false);
    }
    if let (Some(one), Some(other)) = (refined(one, known)?, refined(other, other_known)?) {
        return Ok(one.intersects(&other));
    }
    Ok(regions(one, known, layout)?.intersects(&regions(other, other_known, layout)?))
}

/// Whether a write through `other` could land on `one`.
///
/// Direct port of `qbopt.model.mir:overlapping`.  Provenance and regions
/// answer the symbolic part; when two ordinary references retain the same
/// base value, their byte intervals answer the remaining displacement
/// question.  `Result` makes a region endpoint Python can express but Rust
/// cannot retain an explicit refusal instead of a conservative guess.
pub(crate) fn overlapping(
    one: &MemRef,
    other: &MemRef,
    known: Option<&BTreeMap<Value, Interval>>,
    other_known: Option<&BTreeMap<Value, Interval>>,
    layout: Option<&RegionLayout>,
) -> Result<bool, RegionError> {
    if typed_apart(one, other) {
        return Ok(false);
    }
    // Canonical references carry their own object identity. Keep their base
    // value available to the canonical range query; the legacy covering
    // rewrite below erases it after widening the address to a byte hull.
    if one.provenance.is_some() && other.provenance.is_some() {
        let apart = if one.pointer || other.pointer {
            None
        } else {
            _displaced(&symbolic_ref(one), &symbolic_ref(other))
        };
        return match apart {
            Some(apart) => Ok(!apart),
            None => may_alias(one, other, known, other_known, layout),
        };
    }
    if (_unescaped(one) && _through_pointer(other)) || (_unescaped(other) && _through_pointer(one)) {
        return Ok(false);
    }
    if !one.pointer && !other.pointer {
        let have_facts = known.is_some_and(|facts| !facts.is_empty())
            || other_known.is_some_and(|facts| !facts.is_empty());
        let empty = BTreeMap::new();
        let (one, other) = if have_facts {
            (covering(one, known.unwrap_or(&empty)), covering(other, other_known.unwrap_or(&empty)))
        } else {
            (Cow::Borrowed(one), Cow::Borrowed(other))
        };
        if let Some(apart) = _displaced(&symbolic_ref(&one), &symbolic_ref(&other)) {
            return Ok(!apart);
        }
    }
    may_alias(one, other, known, other_known, layout)
}

/// Python `mir._unescaped`: whether only a reference naming its objects can
/// reach what `ref` names.
fn _unescaped(reference: &MemRef) -> bool {
    reference.provenance.as_ref().is_some_and(|provenance| {
        !provenance.slices.is_empty()
            && !provenance
                .slices
                .iter()
                .any(|one| one.object.addressed || one.object.kind == MemoryKind::Absolute)
    })
}

/// Python `mir._through_pointer`: whether `ref`'s address is a value rather
/// than a named object plus an index.
fn _through_pointer(reference: &MemRef) -> bool {
    (reference.base.is_some() || reference.segment.is_some())
        && reference
            .addr
            .is_none_or(|addr| matches!(addr.space, Space::Literal | Space::Far))
}

/// Python `mir.overlap_bucket`'s tuple: what `overlapping` needs of a cell
/// to rule a write out unseen -- its one object (None if it has no single
/// one), the frame `_displaced` compares displacements in (None for a
/// pointer), and the object's alias class.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct OverlapShape {
    pub object: Option<MemoryObject>,
    pub frame: Option<Frame>,
    pub class: Option<AliasClass>,
}

/// An interned `OverlapShape`: equal shapes share one, so a cell map hashes
/// and copies a word rather than an object's identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct OverlapBucket(u32);

thread_local! {
    static SHAPES: std::cell::RefCell<(HashMap<OverlapShape, OverlapBucket>, Vec<OverlapShape>)> =
        std::cell::RefCell::new((HashMap::default(), Vec::new()));
}

impl OverlapBucket {
    fn interned(shape: OverlapShape) -> Self {
        SHAPES.with(|shapes| {
            let (named, all) = &mut *shapes.borrow_mut();
            *named.entry(shape).or_insert_with_key(|shape| {
                all.push(shape.clone());
                OverlapBucket(u32::try_from(all.len() - 1).expect("fewer than 2^32 buckets"))
            })
        })
    }

    fn shape<T>(self, read: impl FnOnce(&OverlapShape) -> T) -> T {
        SHAPES.with(|shapes| read(&shapes.borrow().1[self.0 as usize]))
    }
}

/// Python `mir._frame`'s tuple: base, segment, space and index.
pub(crate) type Frame = (Option<Value>, Option<Value>, Space, i64);

/// The buckets held, by object, frame and alias class: Python's `parts`.
#[derive(Clone, Default)]
pub(crate) struct OverlapParts {
    pub objects: HashMap<MemoryObject, HashSet<OverlapBucket>>,
    pub objectless: HashSet<OverlapBucket>,
    pub frames: HashMap<Option<Frame>, HashSet<OverlapBucket>>,
    pub classes: HashMap<Option<AliasClass>, HashSet<OverlapBucket>>,
}

impl Bucket for OverlapBucket {
    type Parts = OverlapParts;

    fn held(&self, parts: &mut OverlapParts) {
        self.shape(|shape| {
            match &shape.object {
                Some(object) => parts.objects.entry(object.clone()).or_default().insert(*self),
                None => parts.objectless.insert(*self),
            };
            parts.frames.entry(shape.frame).or_default().insert(*self);
            parts.classes.entry(shape.class).or_default().insert(*self);
        });
    }

    fn released(&self, parts: &mut OverlapParts) {
        fn drop_from<P: Eq + std::hash::Hash>(
            part: &mut HashMap<P, HashSet<OverlapBucket>>,
            key: &P,
            bucket: &OverlapBucket,
        ) {
            let held = part.get_mut(key).expect("a held bucket is indexed");
            held.remove(bucket);
            if held.is_empty() {
                part.remove(key);
            }
        }
        self.shape(|shape| {
            match &shape.object {
                Some(object) => drop_from(&mut parts.objects, object, self),
                None => {
                    parts.objectless.remove(self);
                }
            }
            drop_from(&mut parts.frames, &shape.frame, self);
            drop_from(&mut parts.classes, &shape.class, self);
        });
    }
}

/// Python `mir.overlap_bucket`.
pub(crate) fn overlap_bucket(reference: &MemRef) -> OverlapBucket {
    let one = reference
        .provenance
        .as_ref()
        .filter(|provenance| provenance.slices.len() == 1)
        .map(|provenance| provenance.slices.first().expect("one slice").object.clone());
    object_bucket(one, _frame(reference))
}

/// Python `mir.object_bucket`.
pub(crate) fn object_bucket(one: Option<MemoryObject>, frame: Option<Frame>) -> OverlapBucket {
    let class = one.as_ref().map(alias_class);
    OverlapBucket::interned(OverlapShape { object: one, frame, class })
}

/// Python `mir.overlap_buckets`: the buckets held (`parts`, of a map keyed
/// by `object_bucket`) a write through `reference` may reach; None for all
/// of them.
///
/// Only these can hold a cell `overlapping` does not rule out: one whose
/// object is unknown, one in the write's `_displaced` frame, one in the
/// write's own object, and one whose alias class may alias the write's.
///
/// Sorted and without repeats: a set per write, rehashed as it grew, was dearer than the kill.
pub(crate) fn overlap_buckets(reference: &MemRef, parts: &OverlapParts) -> Option<Vec<OverlapBucket>> {
    let provenance = reference.provenance.as_ref()?;
    #[cfg(test)]
    PICKED.with(|picked| picked.set((picked.get().0 + 1, picked.get().1 + parts.classes.len())));
    let mut reached = parts.objectless.iter().copied().collect::<Vec<_>>();
    if let Some(frame) = _frame(reference) {
        if let Some(buckets) = parts.frames.get(&Some(frame)) {
            reached.extend(buckets.iter().copied());
        }
    }
    // A write names one or two objects: a list is cheaper than a set.
    let mut kinds = Vec::with_capacity(provenance.slices.len());
    let mut written = Vec::<&MemoryObject>::with_capacity(provenance.slices.len());
    for one in &provenance.slices {
        if written.contains(&&one.object) {
            continue;
        }
        written.push(&one.object);
        if let Some(buckets) = parts.objects.get(&one.object) {
            reached.extend(buckets.iter().copied());
        }
        let kind = alias_class(&one.object);
        if !kinds.contains(&kind) {
            kinds.push(kind);
        }
    }
    for (kind, buckets) in &parts.classes {
        if kind.is_some_and(|kind| kinds.iter().any(|one| classes_may_alias(*one, kind))) {
            reached.extend(buckets.iter().copied());
        }
    }
    reached.sort_unstable();
    reached.dedup();
    Some(reached)
}

#[cfg(test)]
thread_local! {
    /// Writes whose buckets were picked, and the alias classes scanned doing
    /// it: Python's test counts the calls a pick makes.
    pub(crate) static PICKED: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
}

/// Python `mir._frame`.
fn _frame(reference: &MemRef) -> Option<Frame> {
    if reference.pointer {
        return None;
    }
    if let Some(symbol) = reference.symbolic {
        return Some((None, None, symbol.space, symbol.index));
    }
    let addr = reference.addr?;
    Some((reference.base, reference.segment, addr.space, addr.index))
}

/// Python `mir._displaced`: whether two references off one base value are
/// disjoint by displacement; `None` if not one base.
///
/// LLVM's constant-offset GEP compare: a fact about values, so it holds
/// whatever object either reference names.
fn _displaced(one: &MemRef, other: &MemRef) -> Option<bool> {
    let (one, other) = (_span(one)?, _span(other)?);
    if one.0 != other.0 {
        return None;
    }
    Some(!(one.1 < other.2 && other.1 < one.2))
}

/// A reference's bytes [low, high) in its frame.
pub(crate) type ByteRange = (i128, i128);

/// Python `mir._span`: the frame `_displaced` measures `reference` in and
/// the bytes it covers there; `reference` has no symbol.
fn _span(reference: &MemRef) -> Option<(Frame, i128, i128)> {
    let addr = reference.addr?;
    if addr.space == Space::Far && reference.segment.is_none() {
        return None;
    }
    let low = i128::from(addr.disp);
    Some(((reference.base, reference.segment, addr.space, addr.index), low, low + i128::from(reference.width)))
}

/// Python `mir.displaced_span`: the frame and bytes where `overlapping`
/// settles `reference` against every reference of that frame by
/// displacement alone: one whose bytes miss these cannot overlap it.
///
/// None for a pointer, and for an address off a base value, which
/// `covering` may widen before the displacements are compared.
pub(crate) fn displaced_span(reference: &MemRef) -> Option<(Frame, i128, i128)> {
    if reference.pointer {
        return None;
    }
    _span(&symbolic_ref(reference)).filter(|(frame, _, _)| frame.0.is_none())
}

/// Python `mir.overlap_span`: `reference`'s bytes as `displaced_span`
/// names them, for a `CellMap`'s spans.
pub(crate) fn overlap_span(reference: &MemRef) -> Option<ByteRange> {
    displaced_span(reference).map(|(_, low, high)| (low, high))
}

/// Python `mir.displaced_buckets`: the buckets held (`parts`) in the frame
/// of `reference`'s `displaced_span`, and its bytes: a cell there that the
/// bytes miss is one a write through `reference` cannot reach.
pub(crate) fn displaced_buckets(
    reference: &MemRef,
    parts: &OverlapParts,
) -> Option<(HashSet<OverlapBucket>, ByteRange)> {
    let (frame, low, high) = displaced_span(reference)?;
    Some((parts.frames.get(&Some(frame))?.clone(), (low, high)))
}

/// Python `addresses`: byte-region intersection for two naked addresses.
pub(crate) fn addresses(
    one: Option<Addr>,
    one_width: u32,
    other: Option<Addr>,
    other_width: u32,
    layout: Option<&RegionLayout>,
) -> Result<bool, RegionError> {
    Ok(addressed(one, one_width, layout)?.intersects(&addressed(other, other_width, layout)?))
}

/// Python `provenance`: the region set as objects, for migrating references
/// onto provenance alone.
///
/// `private` are the segments no pointer reaches unless handed out: their
/// objects are uncaptured, and a call reaches one only where `beyond` names
/// it. `spared` are segments some call is proven to miss part of: named by
/// every reference that reaches them, as GCC's ipa-reference does, so that
/// the exclusion has an object to be taken from. Holes are subtracted from
/// the objects they fall in. An exclusion from a coarse region has no object
/// form, so it widens; see `tools/provdiff.py`.
#[allow(dead_code)] // Called by the `mir.bodies` raise, not yet ported.
pub(crate) fn provenance(
    reference: &MemRef,
    known: Option<&BTreeMap<Value, Interval>>,
    layout: Option<&RegionLayout>,
    private: &BTreeSet<i64>,
    spared: &BTreeSet<i64>,
) -> Result<Provenance, RegionError> {
    let reference = symbolic_ref(reference);
    let mut spans = spans(&reference, known, layout)?;
    let root = Span::whole(Region::default(), Origin::Here);
    if spans.contains(&root) && reference.beyond.is_some() {
        spans.remove(&root);
        spans.extend(_reached(reference.beyond.as_ref().expect("checked above"), layout));
    } else if spans.contains(&root) && floor(reference.addr).is_empty() {
        // The push area is unaddressed, so a reference that can reach it names it.
        spans.insert(Span::whole(Region::stack(), Origin::Sp));
    }
    let framed = reference.excludes.iter().any(|(address, _)| address.space == Space::Frame);
    if spans.iter().any(|one| one.region == Region::stack() && one.origin == Origin::Sp) && !framed {
        // sp and bp displacements are not comparable: a push is anywhere in
        // the frame, unless proven clear of the locals, which puts it in the
        // push area alone.
        spans.insert(Span::whole(Region::stack(), Origin::Bp));
    }
    let uncaptured: BTreeSet<i64> = private.union(spared).copied().collect();
    let mut slices = BTreeSet::new();
    for Span { region, origin, low, high } in &spans {
        if *region == Region::dgroup() {
            // Some segment of the group: the private ones are named, being uncaptured.
            for one in &uncaptured {
                let segment = Region(vec![RegionPart::Dgroup, RegionPart::Segment(*one)]);
                slices.insert(Slice::whole(_object(&segment, &Origin::Here, &uncaptured)));
            }
        }
        if *region == Region::nonlocal() {
            for one in spared.difference(private) {
                let segment = Region(vec![RegionPart::Dgroup, RegionPart::Segment(*one)]);
                slices.insert(Slice::whole(_object(&segment, &Origin::Here, &uncaptured)));
            }
        }
        let object = _object(region, origin, &uncaptured);
        if (*low, *high) == WHOLE || matches!(object.kind, MemoryKind::Unknown | MemoryKind::Nonlocal) {
            slices.insert(Slice::whole(object));
        } else {
            slices.insert(Slice::new(object, *low, *high, 1, 1).map_err(RegionError::InvalidNarrowedSlice)?);
        }
    }
    for (address, width) in &reference.excludes {
        let (region, origin) = region(Some(address.space), Some(address.index), layout);
        let hole = _object(&region, &origin, &uncaptured);
        let high = endpoint(address.disp, *width)?;
        slices = slices
            .iter()
            .flat_map(|one| _without(one, &hole, address.disp, high))
            .collect();
    }
    Ok(Provenance { slices, restrict: BTreeSet::new() })
}

/// Python `_reached`: a bounded call's reach, positively: everything but the
/// program's own segment, and that where handed out.
fn _reached(beyond: &Reach, layout: Option<&RegionLayout>) -> BTreeSet<Span> {
    let (owner, reaches) = beyond;
    let mut out = BTreeSet::from([
        Span::whole(Region::nonlocal(), Origin::Here),
        Span::whole(Region::stack(), Origin::Sp),
    ]);
    if reaches.iter().any(|(segment, _)| segment == owner) {
        let (region, origin) = region(Some(Space::Segment), Some(*owner), layout);
        out.insert(Span::whole(region, origin));
    }
    out
}

/// Python `_without`.
fn _without(one: &Slice, hole: &MemoryObject, low: i64, high: i64) -> Vec<Slice> {
    if one.object != *hole || one.stride != 1 || high <= one.low || one.high <= low {
        return vec![one.clone()];
    }
    [(one.low, low), (high, one.high)]
        .into_iter()
        .filter(|(start, end)| start < end)
        .map(|(start, end)| Slice::new(one.object.clone(), start, end, 1, 1).expect("start < end"))
        .collect()
}

/// Python `_object`.
fn _object(region: &Region, origin: &Origin, private: &BTreeSet<i64>) -> MemoryObject {
    let origin_name = match origin {
        Origin::Here => String::new(),
        Origin::Sp => "sp".to_owned(),
        Origin::Bp => "bp".to_owned(),
        Origin::External(index) => format!("ext:{index}"),
    };
    if *region == Region::stack() {
        if *origin == Origin::Sp {
            return MemoryObject {
                identity: Some(Identity::Str(origin_name)),
                addressed: false,
                captured: false,
                ..MemoryObject::new(MemoryKind::Stack)
            };
        }
        return MemoryObject { identity: Some(Identity::Str(origin_name)), ..MemoryObject::new(MemoryKind::Frame) };
    }
    match region.0.as_slice() {
        [RegionPart::Alloc, RegionPart::Allocation(symbol)] => {
            return MemoryObject {
                identity: Some(Identity::Tuple(vec![Identity::Str(crate::support::pyrepr::Repr::repr(symbol))])),
                ..MemoryObject::new(MemoryKind::Allocation)
            };
        }
        [RegionPart::Absolute] => return linear(),
        _ => {}
    }
    if *region == Region::named() {
        return MemoryObject::new(MemoryKind::Named);
    }
    if let Some(RegionPart::Segment(index)) = region.0.last() {
        return MemoryObject {
            identity: Some(Identity::Tuple(vec![Identity::Space(Space::Segment), Identity::Int(*index)])),
            captured: !private.contains(index),
            ..MemoryObject::new(MemoryKind::Global)
        };
    }
    if *region == Region::linked() {
        return MemoryObject {
            identity: (!origin_name.is_empty()).then_some(Identity::Str(origin_name)),
            ..MemoryObject::new(MemoryKind::External)
        };
    }
    if *region == Region::dgroup() || *region == Region::nonlocal() {
        return MemoryObject::new(MemoryKind::Nonlocal);
    }
    MemoryObject::new(MemoryKind::Unknown)
}

fn meets(one: &Span, other: &Span) -> bool {
    if !one.region.under(&other.region) {
        return false;
    }
    if one.region != other.region || one.origin != other.origin {
        return true;
    }
    one.low < other.high && other.low < one.high
}

fn surviving(spans: &BTreeSet<Span>, holes: &BTreeSet<Span>) -> Vec<Span> {
    spans
        .iter()
        .filter(|one| {
            !holes.iter().any(|hole| {
                one.region.0.starts_with(&hole.region.0)
                    && hole.origin == one.origin
                    && hole.low <= one.low
                    && one.high <= hole.high
            })
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::{
        Origin, Region, RegionError, RegionLayout, RegionPart, RegionSet, Span, addresses,
        may_alias, overlapping, regions, typed_apart,
    };
    use crate::analysis::ranges::Interval;
    use crate::model::memory::{
        Identity, MemoryKind, MemoryObject, Provenance, SliceError,
    };
    use crate::model::mir::{MemRef, Symbol, Value};
    use crate::objectfile::module::{Addr, Space};
    use iced_x86::Register;

    fn address(space: Space, disp: i64, index: i64) -> Addr {
        Addr {
            space,
            disp,
            index,
            base: iced_x86::Register::None,
            segment: iced_x86::Register::None,
        }
    }

    fn reference(address: Addr, width: u32) -> MemRef {
        MemRef::new(Some(address), width)
    }

    fn object(index: i64, extent: Option<i64>) -> MemoryObject {
        MemoryObject {
            kind: MemoryKind::Global,
            identity: Some(Identity::Tuple(vec![Identity::Space(Space::Segment), Identity::Int(index)])),
            generation: 0,
            extent,
            addressed: true,
            captured: true,
        }
    }

    #[test]
    fn hierarchy_coarse_dgroup_meets_linked_static_but_stack_does_not() {
        // `tests/test_regions.py::test_an_external_cell_is_a_named_static...`:
        // an EXTDEF overlaps a static but neither kind is stack storage.
        let cell = reference(address(Space::External, 0, 3), 2);
        let static_ = reference(address(Space::Segment, 0x10, 5), 2);
        let frame = reference(address(Space::Frame, -4, 0), 2);

        assert!(
            regions(&cell, None, None)
                .unwrap()
                .intersects(&regions(&static_, None, None).unwrap())
        );
        assert!(
            !regions(&cell, None, None)
                .unwrap()
                .intersects(&regions(&frame, None, None).unwrap())
        );
        assert_eq!(
            regions(&cell, None, None)
                .unwrap()
                .spans
                .iter()
                .next()
                .unwrap()
                .region,
            Region(vec![RegionPart::Dgroup, RegionPart::Linked])
        );
    }

    #[test]
    fn one_hole_covers_but_the_union_of_holes_does_not() {
        let span = Span {
            region: Region::stack(),
            origin: Origin::Sp,
            low: 0,
            high: 8,
        };
        let whole_hole = Span {
            region: Region::stack(),
            origin: Origin::Sp,
            low: 0,
            high: 8,
        };
        let left = Span {
            region: Region::stack(),
            origin: Origin::Sp,
            low: 0,
            high: 4,
        };
        let right = Span {
            region: Region::stack(),
            origin: Origin::Sp,
            low: 4,
            high: 8,
        };
        let target = RegionSet::new(BTreeSet::from([span.clone()]), BTreeSet::new());
        let covered = RegionSet::new(BTreeSet::from([span.clone()]), BTreeSet::from([whole_hole]));
        let split = RegionSet::new(BTreeSet::from([span]), BTreeSet::from([left, right]));

        assert!(!target.intersects(&covered));
        assert!(target.intersects(&split));
    }

    #[test]
    fn stack_is_disjoint_from_dgroup_and_absolute_selector() {
        // `tests/test_regions.py::test_an_absolute_selector_is_not_dgroup`.
        let stack = reference(address(Space::Stack, 0, 0), 2);
        let static_ = reference(address(Space::Segment, 0x10, 5), 2);
        let selector = Value::new(99, 0x100);
        let mut absolute = reference(address(Space::Far, 0, 0), 1);
        absolute.segment = Some(selector);
        let known = BTreeMap::from([(
            selector,
            Interval {
                low: 0xa000.into(),
                high: 0xa000.into(),
                width: 2,
            },
        )]);

        assert!(
            !regions(&stack, None, None)
                .unwrap()
                .intersects(&regions(&static_, None, None).unwrap())
        );
        assert!(
            !regions(&absolute, Some(&known), None)
                .unwrap()
                .intersects(&regions(&static_, None, None).unwrap())
        );
        assert!(
            regions(&absolute, Some(&known), None)
                .unwrap()
                .intersects(&regions(&absolute, Some(&known), None).unwrap())
        );
    }

    #[test]
    fn layout_makes_owned_segment_private_but_shared_segment_linked() {
        let layout = RegionLayout {
            shared_segments: Some(BTreeSet::from([7])),
            landmarks: BTreeMap::new(),
        };
        let bounds_only = RegionLayout {
            shared_segments: None,
            landmarks: BTreeMap::new(),
        };
        let owned = reference(address(Space::Segment, 0, 5), 1);
        let shared = reference(address(Space::Segment, 0, 7), 1);
        let external = reference(address(Space::External, 0, 1), 1);

        assert!(
            regions(&owned, None, Some(&bounds_only))
                .unwrap()
                .intersects(&regions(&external, None, Some(&bounds_only)).unwrap())
        );
        assert!(
            !regions(&owned, None, Some(&layout))
                .unwrap()
                .intersects(&regions(&external, None, Some(&layout)).unwrap())
        );
        assert!(
            regions(&shared, None, Some(&layout))
                .unwrap()
                .intersects(&regions(&external, None, Some(&layout)).unwrap())
        );

        // `may_alias` has no provenance path here, and must therefore retain
        // the same owned/shared layout answer from `regions`.
        assert!(!may_alias(&owned, &external, None, None, Some(&layout)).unwrap());
        assert!(may_alias(&shared, &external, None, None, Some(&layout)).unwrap());
    }

    #[test]
    fn indexed_access_stays_in_its_segment_and_landmarks_bound_physical_indexing() {
        // `tests/test_mir_alias.py::test_an_index_stays_inside_its_own_segment`.
        let base = iced_x86::Register::DL;
        let indexed = Addr {
            base,
            ..address(Space::Segment, 0x20, 1)
        };
        let same_segment = address(Space::Segment, 0x2f, 1);
        let other_segment = address(Space::Segment, 0x10, 2);
        let layout = RegionLayout {
            shared_segments: None,
            landmarks: BTreeMap::from([((Space::Segment, 1), vec![0x20, 0x30])]),
        };

        assert!(addresses(Some(indexed), 2, Some(same_segment), 2, Some(&layout)).unwrap());
        assert!(!addresses(Some(indexed), 2, Some(other_segment), 2, Some(&layout)).unwrap());
        assert!(
            addresses(
                Some(indexed),
                2,
                Some(address(Space::Segment, 0x999, 1)),
                2,
                None
            )
            .unwrap()
        );
        let raw = super::addressed(Some(indexed), 2, Some(&layout)).unwrap();
        assert_eq!(raw.spans.iter().next().unwrap().low, 0x20);
        assert_eq!(raw.spans.iter().next().unwrap().high, 0x30);
    }

    #[test]
    fn beyond_and_excludes_remove_only_the_named_bytes() {
        // `tests/test_regions.py::test_an_exclusion_bounds_a_reference...`.
        let mut array = reference(address(Space::Segment, 6, 5), 2);
        array.addr.as_mut().unwrap().base = iced_x86::Register::DL;
        array.excludes.push((address(Space::Segment, 0x266, 5), 2));
        let field = reference(address(Space::Segment, 0x266, 5), 1);
        let neighbour = reference(address(Space::Segment, 0x300, 5), 1);

        let array_regions = regions(&array, None, None).unwrap();
        assert!(!array_regions.intersects(&regions(&field, None, None).unwrap()));
        assert!(array_regions.intersects(&regions(&neighbour, None, None).unwrap()));
        assert!(
            array_regions
                .holes
                .iter()
                .any(|one| one.low == 0x266 && one.high == 0x268)
        );

        let mut bounded_call = MemRef::new(None, 0);
        bounded_call.beyond = Some((5, BTreeSet::new()));
        let bounded = regions(&bounded_call, None, None).unwrap();
        assert!(
            bounded
                .holes
                .iter()
                .any(|one| one.region.0.ends_with(&[RegionPart::Segment(5)]))
        );
    }

    #[test]
    fn incompatible_typed_scalars_are_apart_except_for_same_union_start() {
        let union_start = address(Space::Segment, 0x20, 5);
        let mut left = reference(union_start, 2);
        let mut union_view = reference(union_start, 4);
        let mut unrelated = reference(address(Space::Segment, 0x20, 5), 4);
        left.typed = Some(("short".into(), false));
        union_view.typed = Some(("long".into(), false));
        unrelated.typed = Some(("long".into(), false));
        unrelated.symbolic = Some(Symbol::new(Space::Segment, 5, 0x20, 4));

        assert!(!typed_apart(&left, &union_view));
        assert!(typed_apart(&left, &unrelated));

        // The pointer union-start branch is considered after a non-both-
        // address shape, including a pointer retaining one address spelling.
        let base = Value::new(6, 0);
        let mut pointer_with_address = reference(union_start, 2);
        pointer_with_address.pointer = true;
        pointer_with_address.base = Some(base);
        pointer_with_address.typed = Some(("short".into(), false));
        let mut pointer_without_address = MemRef::new(None, 2);
        pointer_without_address.pointer = true;
        pointer_without_address.base = Some(base);
        pointer_without_address.typed = Some(("long".into(), false));
        assert!(!typed_apart(
            &pointer_with_address,
            &pointer_without_address
        ));
    }

    #[test]
    fn provenance_alias_uses_canonical_subobjects_and_byte_ranges() {
        // `tests/test_mir_alias.py::test_canonical_subobjects_use_object_identity_and_byte_ranges`:
        // fields of one object, and equal offsets in distinct objects, are disjoint.
        let first = object(1, Some(8));
        let second = object(2, Some(8));
        let mut a = MemRef::new(None, 4);
        a.provenance =
            Some(Provenance::one_with_slice(first.clone(), 0, 4, 1, 1, BTreeSet::new()).unwrap());
        let mut b = MemRef::new(None, 4);
        b.provenance =
            Some(Provenance::one_with_slice(first, 4, 8, 1, 1, BTreeSet::new()).unwrap());
        let mut c = MemRef::new(None, 4);
        c.provenance =
            Some(Provenance::one_with_slice(second, 0, 4, 1, 1, BTreeSet::new()).unwrap());

        assert!(!may_alias(&a, &b, None, None, None).unwrap());
        assert!(!may_alias(&a, &c, None, None, None).unwrap());
        assert!(may_alias(&a, &a, None, None, None).unwrap());
    }

    #[test]
    fn provenance_alias_respects_strides_restrict_roots_and_typed_union_starts() {
        // Directly ports `test_strided_ranges_prove_interleaved_arrays_disjoint`
        // and `test_restrict_roots_and_tbaa_share_the_alias_query`.
        let lanes = object(4, Some(64));
        let mut even = MemRef::new(None, 1);
        even.provenance =
            Some(Provenance::one_with_slice(lanes.clone(), 0, 64, 2, 1, BTreeSet::new()).unwrap());
        let mut odd = MemRef::new(None, 1);
        odd.provenance =
            Some(Provenance::one_with_slice(lanes, 1, 64, 2, 1, BTreeSet::new()).unwrap());
        assert!(!may_alias(&even, &odd, None, None, None).unwrap());

        let unknown = MemoryObject::new(MemoryKind::Unknown);
        let mut left = MemRef::new(None, 4);
        left.typed = Some(("int4".into(), false));
        left.provenance = Some(
            Provenance::one_with_slice(
                unknown.clone(),
                super::FLOOR,
                super::CEILING,
                1,
                1,
                BTreeSet::from([Identity::Int(1)]),
            )
            .unwrap(),
        );
        let mut right = MemRef::new(None, 4);
        right.typed = Some(("float4".into(), false));
        right.provenance = Some(
            Provenance::one_with_slice(
                unknown.clone(),
                super::FLOOR,
                super::CEILING,
                1,
                1,
                BTreeSet::from([Identity::Int(2)]),
            )
            .unwrap(),
        );
        assert!(!may_alias(&left, &right, None, None, None).unwrap());

        // With matching scalar types, only the disjoint restrict roots make
        // this pair disjoint, so the provenance path is exercised directly.
        let mut restrict_only = right.clone();
        restrict_only.typed = Some(("int4".into(), false));
        assert!(!may_alias(&left, &restrict_only, None, None, None).unwrap());

        // Removing the roots leaves incompatible indirect scalar views apart.
        let mut typed_only = right.clone();
        typed_only.provenance = Some(Provenance::one(unknown));
        assert!(!may_alias(&left, &typed_only, None, None, None).unwrap());

        let union_start = address(Space::Frame, -8, 0);
        let mut union_int = reference(union_start, 4);
        union_int.typed = Some(("int4".into(), false));
        let mut union_float = reference(union_start, 4);
        union_float.typed = Some(("float4".into(), false));
        assert!(may_alias(&union_int, &union_float, None, None, None).unwrap());
    }

    #[test]
    fn index_interval_narrows_once_and_refuses_out_of_extent_accesses() {
        // `tests/test_mir_alias.py::test_index_interval_counts_an_access_width_once`:
        // one dword index start touches bytes 0..4, not 0..8.
        let index = Value::new(1, 1);
        let indexed_object = object(3, Some(16));
        let mut indexed = reference(address(Space::Segment, 0, 3), 4);
        indexed.base = Some(index);
        indexed.base_width = 2;
        indexed.provenance = Some(Provenance::one(indexed_object.clone()));
        let mut next_field = MemRef::new(None, 4);
        next_field.provenance = Some(
            Provenance::one_with_slice(indexed_object.clone(), 4, 8, 1, 1, BTreeSet::new())
                .unwrap(),
        );
        let known = BTreeMap::from([(
            index,
            Interval {
                low: 0.into(),
                high: 0.into(),
                width: 2,
            },
        )]);
        assert!(!may_alias(&indexed, &next_field, Some(&known), None, None).unwrap());

        // A fact that would reach outside a bounded object is not used to
        // narrow it.  The original whole-object provenance remains conservative.
        let bounded = object(5, Some(4));
        indexed.provenance = Some(Provenance::one(bounded.clone()));
        next_field.provenance =
            Some(Provenance::one_with_slice(bounded, 3, 4, 1, 1, BTreeSet::new()).unwrap());
        let outside = BTreeMap::from([(
            index,
            Interval {
                low: 0.into(),
                high: 1.into(),
                width: 2,
            },
        )]);
        assert!(may_alias(&indexed, &next_field, Some(&outside), None, None).unwrap());
    }

    #[test]
    fn narrowing_reports_invalid_and_unrepresentable_python_slices() {
        let index = Value::new(1, 1);
        let mut indexed = reference(address(Space::Segment, 0, 3), 1);
        indexed.base = Some(index);
        indexed.base_width = 2;
        indexed.provenance = Some(Provenance::one(object(3, None)));
        let other = indexed.clone();

        let invalid = BTreeMap::from([(
            index,
            Interval {
                low: 2.into(),
                high: 1.into(),
                width: 2,
            },
        )]);
        assert_eq!(
            may_alias(&indexed, &other, Some(&invalid), None, None),
            Err(RegionError::InvalidNarrowedSlice(SliceError::Empty))
        );

        let too_large = BTreeMap::from([(
            index,
            Interval {
                low: i64::MAX.into(),
                high: i64::MAX.into(),
                width: 2,
            },
        )]);
        assert_eq!(
            may_alias(&indexed, &other, Some(&too_large), None, None),
            Err(RegionError::NarrowedSliceUnrepresentable)
        );
    }

    #[test]
    fn overlapping_far_segments_need_a_segment_identity_before_offsets_decide() {
        // Direct port of tests/test_mir_alias.py::{test_equal_offsets_do_not_prove_far_segments_disjoint,
        // test_unknown_far_segments_cannot_use_offset_disjointness}.  Far
        // offsets alone do not identify a byte: 1000:0020 and 1001:0010
        // can name the same address.
        let base = Value::new(1, 0);
        let mut one_address = address(Space::Far, 0x20, 0);
        one_address.base = iced_x86::Register::DL;
        let mut one = reference(one_address, 2);
        one.base = Some(base);
        one.segment = Some(Value::new(2, 0));

        let mut other = one.clone();
        other.addr.as_mut().expect("address").disp = 0x10;
        other.segment = Some(Value::new(3, 0));
        assert_eq!(overlapping(&one, &other, None, None, None), Ok(true));
        assert_eq!(overlapping(&other, &one, None, None, None), Ok(true));

        other.segment = one.segment;
        assert_eq!(overlapping(&one, &other, None, None, None), Ok(false));

        one.segment = None;
        other = one.clone();
        other.addr.as_mut().expect("address").disp += 16;
        assert_eq!(overlapping(&one, &other, None, None, None), Ok(true));
    }

    #[test]
    fn overlapping_keeps_indexed_segments_disjoint() {
        // Direct port of tests/test_mir_alias.py::test_an_index_stays_inside_its_own_segment.
        let base = Value::new(1, 0);
        let mut one_address = address(Space::Segment, 0x20, 1);
        one_address.base = iced_x86::Register::DL;
        let mut one = reference(one_address, 2);
        one.base = Some(base);
        let mut other = one.clone();
        other.addr.as_mut().expect("address").index = 2;
        other.addr.as_mut().expect("address").disp = 0x10;

        assert_eq!(overlapping(&one, &other, None, None, None), Ok(false));
    }

    #[test]
    fn overlapping_same_base_statics_use_half_open_byte_ranges() {
        let one = reference(address(Space::Segment, 10, 5), 4);
        let mut overlaps = one.clone();
        overlaps.addr.as_mut().expect("address").disp = 13;
        let mut adjacent = one.clone();
        adjacent.addr.as_mut().expect("address").disp = 14;

        assert_eq!(overlapping(&one, &overlaps, None, None, None), Ok(true));
        assert_eq!(overlapping(&one, &adjacent, None, None, None), Ok(false));
    }

    #[test]
    fn overlapping_range_covering_respects_width_wrap_and_each_fact_map() {
        // Direct port of tests/test_ranges.py::test_range_alias_checks_cover_width_and_wrap.
        let base = Value::new(1, 0);
        let mut indexed = reference(address(Space::Segment, 4, 5), 2);
        indexed.base = Some(base);
        indexed.base_width = 2;

        for (low, high, interval_width, offset, width, overlaps) in [
            (0_i64, 20_i64, 2_u32, 26_i64, 2_u32, false),
            (0, 20, 2, 25, 2, true),
            (0, 20, 2, 100, 4, false),
            (-8, 20, 2, 100, 4, true),
            (0, 65_535, 2, 100, 4, true),
            (0, 20, 4, 100, 4, true),
        ] {
            let known = BTreeMap::from([(
                base,
                Interval {
                    low: low.into(),
                    high: high.into(),
                    width: interval_width,
                },
            )]);
            let static_ = reference(address(Space::Segment, offset, 5), width);

            assert_eq!(
                overlapping(&indexed, &static_, Some(&known), None, None),
                Ok(overlaps),
                "left facts: {low}..{high}, width {interval_width}, static {offset}/{width}",
            );
            assert_eq!(
                overlapping(&static_, &indexed, None, Some(&known), None),
                Ok(overlaps),
                "right facts: {low}..{high}, width {interval_width}, static {offset}/{width}",
            );
        }
    }

    #[test]
    fn overlapping_incompatible_scalars_are_apart_except_at_a_union_start() {
        // Direct port of tests/test_mir_alias.py::test_restrict_roots_and_tbaa_share_the_alias_query.
        let address = address(Space::Frame, -8, 0);
        let mut short = reference(address, 2);
        short.typed = Some(("short".to_owned(), false));
        let mut long = reference(address, 4);
        long.typed = Some(("long".to_owned(), false));
        let mut distinct = long.clone();
        distinct.addr.as_mut().expect("address").disp = -4;

        assert_eq!(overlapping(&short, &long, None, None, None), Ok(true));
        assert_eq!(overlapping(&short, &distinct, None, None, None), Ok(false));
    }

    #[test]
    fn overlapping_settles_one_base_by_displacement_before_provenance() {
        // One base value (none) and one displacement settle it before the
        // objects do: Python `mir._displaced`, whatever object either names.
        let mut one = reference(address(Space::Segment, 0, 1), 2);
        one.provenance = Some(Provenance::one(object(1, Some(4))));
        let mut other = one.clone();
        other.provenance = Some(Provenance::one(object(2, Some(4))));

        assert_eq!(overlapping(&one, &other, None, None, None), Ok(true));
    }

    #[test]
    fn overlapping_pointers_skip_covering_and_same_base_arithmetic() {
        // Direct port of tests/test_pointer_memory.py::test_pointer_identity_is_not_a_disjointness_proof.
        let base = Value::new(1, 0);
        let mut one = reference(address(Space::Segment, 0, 0), 2);
        one.base = Some(base);
        one.base_width = 2;
        one.pointer = true;
        let mut other = one.clone();
        other.addr.as_mut().expect("address").disp = 8;
        let known = BTreeMap::from([(
            base,
            Interval {
                low: 0.into(),
                high: 0.into(),
                width: 2,
            },
        )]);

        assert_eq!(
            overlapping(&one, &other, Some(&known), None, None),
            Ok(true)
        );
    }

    #[test]
    fn overlapping_reports_unrepresentable_region_endpoints() {
        let overflowing = reference(address(Space::Segment, i64::MAX, 5), 1);
        let frame = reference(address(Space::Frame, 0, 0), 1);

        assert_eq!(
            overlapping(&overflowing, &frame, None, None, None),
            Err(RegionError::EndpointOverflow)
        );
    }

    #[test]
    fn empty_within_falls_through_to_the_address_span() {
        let mut reference = reference(address(Space::Segment, 0x20, 5), 2);
        reference.within = Some(Vec::new());

        let result = regions(&reference, None, None).unwrap();
        assert_eq!(result.spans.len(), 1);
        let span = result.spans.first().unwrap();
        assert_eq!((span.low, span.high), (0x20, 0x22));
    }

    #[test]
    fn endpoint_overflow_refuses_to_narrow() {
        let overflowing = reference(address(Space::Segment, i64::MAX, 5), 1);
        assert_eq!(
            regions(&overflowing, None, None),
            Err(RegionError::EndpointOverflow)
        );
    }
}
