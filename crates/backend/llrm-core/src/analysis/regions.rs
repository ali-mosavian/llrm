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
    AliasClass, Identity, MemoryKind, MemoryObject, Provenance, Slice, SliceError, alias_class,
};
use num_traits::ToPrimitive;

use crate::support::hash::{HashMap, HashSet};
use crate::model::mir::{MemRef, Symbol, Value, symbolic_ref};
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
pub enum RegionPart {
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
pub struct Region(pub Vec<RegionPart>);

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

    fn under(&self, other: &Self) -> bool {
        self.0.starts_with(&other.0) || other.0.starts_with(&self.0)
    }
}

/// The displacement origin for a region span.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Origin {
    Here,
    Sp,
    Bp,
    External(i64),
}

/// A half-open byte interval in one region, counted from `origin`.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Span {
    pub region: Region,
    pub origin: Origin,
    pub low: i64,
    pub high: i64,
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
pub struct RegionSet {
    pub spans: BTreeSet<Span>,
    pub holes: BTreeSet<Span>,
}

impl RegionSet {
    fn new(spans: BTreeSet<Span>, holes: BTreeSet<Span>) -> Self {
        Self { spans, holes }
    }

    /// Python `RegionSet.intersects`.
    ///
    /// A span is removed only when a *single* hole covers it.  Two holes
    /// whose union covers it do not, deliberately, remove the span.
    pub fn intersects(&self, other: &Self) -> bool {
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
pub struct RegionLayout {
    /// Segments using COMMON combination, therefore shared with another unit.
    ///
    /// `None` means this is bounds-only context, not a statement that no
    /// segment is COMMON.  That preserves Python's separate `bounds` and
    /// `layout` inputs.
    pub shared_segments: Option<BTreeSet<i64>>,
    /// Sorted exact named displacements, keyed by `(Space, segment index)`.
    pub landmarks: BTreeMap<(Space, i64), Vec<i64>>,
}

/// Failure to express an otherwise-Python-sized span in an existing Rust
/// address endpoint.  Refusing retains conservatism; wrapping would narrow it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegionError {
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
pub fn addressed(
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
    let (start, end) = crate::abi::machine::BUILT_IN.foreign_span(selectors, offsets, width)?;
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
pub fn regions(
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
pub fn typed_apart(one: &MemRef, other: &MemRef) -> bool {
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
/// Whether one reference lands in foreign memory and the other in the data
/// group. An access with no selector of its own goes through the data or
/// stack segment, both the data group; it holds program data, and foreign
/// memory holds none.
fn foreign_apart(
    one: &MemRef,
    other: &MemRef,
    known: Option<&BTreeMap<Value, Interval>>,
    other_known: Option<&BTreeMap<Value, Interval>>,
) -> bool {
    let grouped = |reference: &MemRef| reference.segment.is_none() && reference.where_() != Some(Space::Far);
    (grouped(one) && foreign(other, other_known).is_some()) || (grouped(other) && foreign(one, known).is_some())
}

pub fn may_alias(
    one: &MemRef,
    other: &MemRef,
    known: Option<&BTreeMap<Value, Interval>>,
    other_known: Option<&BTreeMap<Value, Interval>>,
    layout: Option<&RegionLayout>,
) -> Result<bool, RegionError> {
    if typed_apart(one, other) || foreign_apart(one, other, known, other_known) {
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
pub fn overlapping(
    one: &MemRef,
    other: &MemRef,
    known: Option<&BTreeMap<Value, Interval>>,
    other_known: Option<&BTreeMap<Value, Interval>>,
    layout: Option<&RegionLayout>,
) -> Result<bool, RegionError> {
    if typed_apart(one, other) || foreign_apart(one, other, known, other_known) {
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
pub struct OverlapShape {
    pub object: Option<MemoryObject>,
    pub frame: Option<Frame>,
    pub class: Option<AliasClass>,
}

/// An interned `OverlapShape`: equal shapes share one, so a cell map hashes
/// and copies a word rather than an object's identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OverlapBucket(u32);

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
pub type Frame = (Option<Value>, Option<Value>, Space, i64);

/// The buckets held, by object, frame and alias class: Python's `parts`.
#[derive(Clone, Default)]
pub struct OverlapParts {
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
pub fn overlap_bucket(reference: &MemRef) -> OverlapBucket {
    let one = reference
        .provenance
        .as_ref()
        .filter(|provenance| provenance.slices.len() == 1)
        .map(|provenance| provenance.slices.first().expect("one slice").object.clone());
    object_bucket(one, _frame(reference))
}

/// Python `mir.object_bucket`.
pub fn object_bucket(one: Option<MemoryObject>, frame: Option<Frame>) -> OverlapBucket {
    let class = one.as_ref().map(alias_class);
    OverlapBucket::interned(OverlapShape { object: one, frame, class })
}

#[cfg(any(test, feature = "testing"))]
thread_local! {
    /// Writes whose buckets were picked, and the alias classes scanned doing
    /// it: Python's test counts the calls a pick makes.
    pub static PICKED: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
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
pub type ByteRange = (i128, i128);

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
pub fn displaced_span(reference: &MemRef) -> Option<(Frame, i128, i128)> {
    if reference.pointer {
        return None;
    }
    _span(&symbolic_ref(reference)).filter(|(frame, _, _)| frame.0.is_none())
}

/// Python `mir.overlap_span`: `reference`'s bytes as `displaced_span`
/// names them, for a `CellMap`'s spans.
pub fn overlap_span(reference: &MemRef) -> Option<ByteRange> {
    displaced_span(reference).map(|(_, low, high)| (low, high))
}

/// Python `mir.displaced_buckets`: the buckets held (`parts`) in the frame
/// of `reference`'s `displaced_span`, and its bytes: a cell there that the
/// bytes miss is one a write through `reference` cannot reach.
pub fn displaced_buckets(
    reference: &MemRef,
    parts: &OverlapParts,
) -> Option<(HashSet<OverlapBucket>, ByteRange)> {
    let (frame, low, high) = displaced_span(reference)?;
    Some((parts.frames.get(&Some(frame))?.clone(), (low, high)))
}

/// Python `addresses`: byte-region intersection for two naked addresses.
pub fn addresses(
    one: Option<Addr>,
    one_width: u32,
    other: Option<Addr>,
    other_width: u32,
    layout: Option<&RegionLayout>,
) -> Result<bool, RegionError> {
    Ok(addressed(one, one_width, layout)?.intersects(&addressed(other, other_width, layout)?))
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
