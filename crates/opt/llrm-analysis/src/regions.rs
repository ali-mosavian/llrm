//! What a reference can reach, and whether two can meet: llrm-core's
//! `analysis/regions.rs`, adapted to the rich MIR's accesses.
//!
//! Kept: `may_alias` and `overlapping` (Python `mir.overlapping`), with
//! `typed_apart`, the index `narrowed`, foreign memory, the displacement
//! compare off one base, and the overlap buckets a `CellMap` indexes by.
//!
//! Skipped, with no rich MIR counterpart: the region lattice (`RegionSet`,
//! `Region`, `Span`, `regions`, `addressed`, `addresses`, `RegionLayout`'s
//! COMMON segments and landmarks, `provenance`, `_reached`, `_without`,
//! `_object`) and the holes it subtracted (`beyond`, `excludes`, `within`).
//! It placed an x86 operand in the stack, DGROUP or a linked segment where
//! no provenance named its object; a rich MIR access has a root, and
//! `alias::points_to` gives every pointer an object, `Unknown` at worst, so
//! provenance alone answers. Where an access carries none, nothing proves
//! it apart. `allocation` (a descriptor's object) is the `!tbaa` type
//! "allocation" now, which `typed_apart` reads.
//!
//! Which linear memory holds no program data is the target's fact, and
//! which address spaces hold program data the segment layout's: both the
//! program's, which the pass manager hands analyses as `Outer::program`.
//!
//! Tests skipped, the lattice's:
//! `hierarchy_coarse_dgroup_meets_linked_static_but_stack_does_not`,
//! `one_hole_covers_but_the_union_of_holes_does_not`,
//! `layout_makes_owned_segment_private_but_shared_segment_linked`,
//! `indexed_access_stays_in_its_segment_and_landmarks_bound_physical_indexing`,
//! `beyond_and_excludes_remove_only_the_named_bytes`,
//! `overlapping_reports_unrepresentable_region_endpoints`,
//! `empty_within_falls_through_to_the_address_span`,
//! `endpoint_overflow_refuses_to_narrow`, and all of `regions_alias_tests.rs`
//! (x86 addresses and an OMF object's landmarks). Skipped,
//! `overlapping_pointers_skip_covering_and_same_base_arithmetic`: no access
//! is a bare pointer with a spelled address. Skipped,
//! `narrowing_reports_invalid_and_unrepresentable_python_slices`' invalid
//! half: an interval whose low passes its high is no rich MIR fact to hand
//! it, the unrepresentable half is ported.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use llrm_mir::module::{Operand, ValueId};
use llrm_support::hash::{HashMap, HashSet};
use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::cellmap::Bucket;
use crate::memory::{
    AliasClass, MemRef, MemoryKind, ObjectRef, Provenance, Slice, SliceError, alias_class, classes_may_alias,
};
use crate::ranges::{Interval, covering};

const FLOOR: i64 = -(1_i64 << 31);

use llrm_mir::program::ProgramProxy;

/// Failure to express a narrowed slice in `Slice`'s endpoints. Refusing
/// retains conservatism; wrapping would narrow it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegionError {
    /// Python integers can describe a narrowed slice outside Rust's signed
    /// `Slice` endpoints.  Do not discard an otherwise-applicable fact.
    NarrowedSliceUnrepresentable,
    /// `Slice` rejects the same invalid interval shape that Python rejects
    /// when `memory.Slice` is constructed.
    InvalidNarrowedSlice(SliceError),
}

/// Memory addressed linearly, which no program object occupies. Its slice
/// offsets are linear addresses, so one object covers every selector.
fn linear() -> ObjectRef {
    ObjectRef::LINEAR
}

/// What an access through a fixed-address pointer names: linear memory, wholly.
/// The language says no program object lives there, and no selector range
/// has to prove it.
pub fn fixed_provenance() -> Provenance {
    Provenance::one(linear())
}

/// The range of `value`'s interval, `disp` plus `scale` times it.
fn scaled(
    interval: &Interval,
    disp: i64,
    scale: i64,
) -> (BigInt, BigInt) {
    let (one, other) = (BigInt::from(disp) + &interval.low * scale, BigInt::from(disp) + &interval.high * scale);
    if one <= other { (one, other) } else { (other, one) }
}

/// The size of a flat target's address space, in bytes: an offset is the
/// linear address and wraps modulo this. None where it does not fit an i64
/// (a 64-bit target), whose ranges are then not modelled.
fn flat_word(
    layout: &llrm_mir::datalayout::DataLayout,
    spaces: &llrm_mir::spaces::Spaces,
) -> Option<i64> {
    1i64.checked_shl(layout.offset_bits(spaces.near)).filter(|word| *word > 0)
}

/// The linear bytes `reference` reaches, when its selector's range lands it
/// wholly in memory the machine keeps no program data in.
fn foreign(
    reference: &MemRef,
    known: Option<&BTreeMap<ValueId, Interval>>,
    program: Option<&ProgramProxy>,
) -> Option<Slice> {
    let machine = &*program?.target;
    let spaces = machine.spaces();
    // A selector or offset is a word of the segment's size: none where the
    // target has no segments, unless it has one address space, where an
    // offset is the linear address, a 32-bit word, and there is no selector.
    let flat = spaces.far_is_near();
    // An address that is not a constant is not in a range the platform states.
    if flat && !reference.linear {
        return None;
    }
    let segment = match spaces.segment_bytes {
        Some(bytes) => i64::try_from(bytes).ok()?,
        None if flat => flat_word(&program?.layout, &spaces)?,
        None => return None,
    };
    // A selector or offset is an unsigned word; ranges may carry it signed,
    // and an offset wraps within its segment.
    let words = |low: BigInt, high: BigInt| -> Option<(i64, i64)> {
        let (low, high) = (low.to_i64()?, high.to_i64()?);
        let word = |one: i64| one.rem_euclid(segment);
        (high - low < segment && word(low) <= word(high)).then(|| (word(low), word(high)))
    };
    let selectors = match (reference.selector, reference.segment) {
        _ if flat => (0, 0),
        (Some(selector), _) => words(selector.into(), selector.into())?,
        (None, Some(Operand::Value(segment))) => {
            let selector = known?.get(&segment)?;
            words(selector.low.clone(), selector.high.clone())?
        }
        (None, _) => return None,
    };
    let disp = BigInt::from(reference.origin) + reference.disp;
    let offsets = match reference.base {
        None => words(disp.clone(), disp),
        Some(base) => known
            .and_then(|known| known.get(&base))
            .filter(|interval| interval.width == reference.base_width)
            .and_then(|interval| {
                let (low, high) = scaled(interval, 0, reference.scale);
                words(low + &disp, high + &disp)
            }),
    }
    .unwrap_or((0, segment - 1));
    let width = i64::from(reference.width.max(1));
    let (start, end) = machine.foreign_span(selectors, offsets, width)?;
    Some(Slice::new(linear(), start, end - width + 1, 1, width).expect("a foreign span holds one access"))
}

/// The linear bytes `reference` names where its selector's range under
/// `known` lands it wholly in memory the machine keeps no program data in.
pub fn foreign_provenance(
    reference: &MemRef,
    known: &BTreeMap<ValueId, Interval>,
    program: Option<&ProgramProxy>,
) -> Option<Provenance> {
    let slice = foreign(reference, Some(known), program)?;
    Some(Provenance { slices: BTreeSet::from([slice]), restrict: BTreeSet::new() })
}

/// What `reference` may name under `facts`: its linear bytes when its
/// segment lands it in foreign memory, else its provenance narrowed. Its own
/// provenance, borrowed, where nothing narrows it: the common answer, and a
/// copy of two sets of slices for each pair asked.
fn refined<'a>(
    reference: &'a MemRef,
    facts: Option<&BTreeMap<ValueId, Interval>>,
    program: Option<&ProgramProxy>,
) -> Result<Option<Cow<'a, Provenance>>, RegionError> {
    if let Some(slice) = foreign(reference, facts, program) {
        return Ok(Some(Cow::Owned(Provenance { slices: BTreeSet::from([slice]), restrict: BTreeSet::new() })));
    }
    reference.provenance.as_ref().map(|provenance| narrowed(reference, provenance, facts)).transpose()
}

/// Two accesses at one start, as their roots or their objects say.
fn same_typed_start(
    one: &MemRef,
    other: &MemRef,
) -> bool {
    if one.root.is_some() && one.root == other.root {
        return one.disp == other.disp
            && one.base == other.base
            && one.scale == other.scale
            && one.segment == other.segment;
    }
    let (Some(one_provenance), Some(other_provenance)) = (&one.provenance, &other.provenance) else {
        return false;
    };
    if one_provenance.slices.len() != 1 || other_provenance.slices.len() != 1 {
        return false;
    }
    let first = one_provenance.slices.first().expect("checked above");
    let second = other_provenance.slices.first().expect("checked above");
    first.object == second.object && first.low == second.low && first.low != FLOOR
}

/// Whether the address analysis has the two accesses in one object at bytes
/// that overlap: each names one start in a single object, and their widths
/// reach one another. Type-based alias analysis only tells accesses apart that
/// the address analysis cannot (as LLVM asks it of MayAlias alone); it does not
/// unsay an overlap that is known.
fn provably_overlap(
    one: &MemRef,
    other: &MemRef,
) -> bool {
    let (Some(one_provenance), Some(other_provenance)) = (&one.provenance, &other.provenance) else {
        return false;
    };
    let (Some(first), Some(second)) = (
        one_provenance.slices.iter().next().filter(|_| one_provenance.slices.len() == 1),
        other_provenance.slices.iter().next().filter(|_| other_provenance.slices.len() == 1),
    ) else {
        return false;
    };
    let exact = |slice: &Slice| slice.stride == 1 && slice.high == slice.low + 1 && slice.low != FLOOR;
    let (one_width, other_width) = (i64::from(one.width.max(1)), i64::from(other.width.max(1)));
    first.object == second.object
        && exact(first)
        && exact(second)
        && first.low < second.low + other_width
        && second.low < first.low + one_width
}

/// Python `typed_apart`: accesses of different `!tbaa` types cannot alias
/// unless one's type is an ancestor of the other's, or for two views
/// explicitly computed from the same union start.
pub fn typed_apart(
    one: &MemRef,
    other: &MemRef,
) -> bool {
    let (Some(one_type), Some(other_type)) = (&one.typed, &other.typed) else {
        return false;
    };
    if provably_overlap(one, other) {
        return false;
    }
    // As LLVM's TypeBasedAA: types of one root, neither covering the other.
    // A type with no root, or of another root, says nothing: may alias.
    let (Some(one_root), Some(other_root)) = (one.lineage.last(), other.lineage.last()) else {
        return false;
    };
    let related = one.lineage.iter().any(|name| name.as_str() == &**other_type)
        || other.lineage.iter().any(|name| name.as_str() == &**one_type);
    one_root == other_root && one_type != other_type && !related && !same_typed_start(one, other)
}

/// Python `may_alias`'s private `narrowed` helper.
///
/// An indexed access into its own object with a matching range fact can
/// name fewer bytes than its coarse provenance. The range counts starts,
/// so the access width is included exactly once when calculating the final
/// byte. BigInt retains Python arithmetic until the new `Slice` must be
/// represented by Rust's bounded endpoints.
fn narrowed<'a>(
    reference: &MemRef,
    provenance: &'a Provenance,
    facts: Option<&BTreeMap<ValueId, Interval>>,
) -> Result<Cow<'a, Provenance>, RegionError> {
    let Some(base) = reference.base.filter(|_| reference.object) else {
        return Ok(Cow::Borrowed(provenance));
    };
    let Some(interval) = facts.and_then(|facts| facts.get(&base)) else {
        return Ok(Cow::Borrowed(provenance));
    };
    if interval.width != reference.base_width || provenance.slices.len() != 1 {
        return Ok(Cow::Borrowed(provenance));
    }
    let source = provenance.slices.first().expect("one source slice was checked above");
    let width = i64::from(reference.width.max(1));
    let (low, high) = scaled(interval, reference.disp, reference.scale);
    let high = high + 1_u8;
    let end = &high + width - 1_i64;

    if let Some(extent) = source.object.extent {
        let extent = BigInt::from(extent);
        if low < BigInt::from(0_u8) || low >= high || end > extent {
            return Ok(Cow::Borrowed(provenance));
        }
    }

    let (Ok(low), Ok(high)) = (i64::try_from(&low), i64::try_from(&high)) else {
        return Err(RegionError::NarrowedSliceUnrepresentable);
    };
    let slice = Slice::new(source.object.clone(), low, high, 1, width).map_err(RegionError::InvalidNarrowedSlice)?;
    Ok(Cow::Owned(Provenance { slices: BTreeSet::from([slice]), restrict: provenance.restrict.clone() }))
}

/// Whether one reference lands in foreign memory and the other in program
/// data, as the segment layout places its address space.
fn foreign_apart(
    one: &MemRef,
    other: &MemRef,
    known: Option<&BTreeMap<ValueId, Interval>>,
    other_known: Option<&BTreeMap<ValueId, Interval>>,
    program: Option<&ProgramProxy>,
) -> bool {
    let Some(program) = program else { return false };
    let grouped = |reference: &MemRef| program.segments.program_data(reference.space);
    (grouped(one) && foreign(other, other_known, Some(program)).is_some())
        || (grouped(other) && foreign(one, known, Some(program)).is_some())
}

/// Python `qbopt.analysis.regions:may_alias`.
///
/// Each reference gets its own interval facts. If both have provenance,
/// their concrete objects decide.
pub fn may_alias(
    one: &MemRef,
    other: &MemRef,
    known: Option<&BTreeMap<ValueId, Interval>>,
    other_known: Option<&BTreeMap<ValueId, Interval>>,
    program: Option<&ProgramProxy>,
) -> Result<bool, RegionError> {
    if typed_apart(one, other) || foreign_apart(one, other, known, other_known, program) {
        return Ok(false);
    }
    match (refined(one, known, program)?, refined(other, other_known, program)?) {
        (Some(one), Some(other)) => Ok(one.intersects(&other)),
        _ => Ok(true),
    }
}

/// Whether a write through `other` could land on `one`.
///
/// Python `qbopt.model.mir:overlapping`. Provenance answers the symbolic
/// part; when two references keep one root and index, their byte intervals
/// answer the remaining displacement question.
pub fn overlapping(
    one: &MemRef,
    other: &MemRef,
    known: Option<&BTreeMap<ValueId, Interval>>,
    other_known: Option<&BTreeMap<ValueId, Interval>>,
    program: Option<&ProgramProxy>,
) -> Result<bool, RegionError> {
    if typed_apart(one, other) || foreign_apart(one, other, known, other_known, program) {
        return Ok(false);
    }
    // Canonical references carry their own object identity. Keep their base
    // value available to the canonical range query; the covering rewrite
    // below erases it after widening the address to a byte hull.
    if one.provenance.is_some() && other.provenance.is_some() {
        return match _displaced(one, other) {
            Some(apart) => Ok(!apart),
            None => may_alias(one, other, known, other_known, program),
        };
    }
    if (_unescaped(one) && _through_pointer(other)) || (_unescaped(other) && _through_pointer(one)) {
        return Ok(false);
    }
    let have_facts = known.is_some_and(|facts| !facts.is_empty()) || other_known.is_some_and(|facts| !facts.is_empty());
    let empty = BTreeMap::new();
    let (covered, other_covered) = if have_facts {
        (covering(one, known.unwrap_or(&empty)), covering(other, other_known.unwrap_or(&empty)))
    } else {
        (Cow::Borrowed(one), Cow::Borrowed(other))
    };
    if let Some(apart) = _displaced(&covered, &other_covered) {
        return Ok(!apart);
    }
    may_alias(one, other, known, other_known, program)
}

/// Python `mir._unescaped`: whether only a reference naming its objects can
/// reach what `ref` names. An uncaptured object is one too: a pointer no
/// fact follows reaches only what escaped, as `alias::_lost` publishes.
fn _unescaped(reference: &MemRef) -> bool {
    reference.provenance.as_ref().is_some_and(|provenance| {
        !provenance.slices.is_empty()
            && !provenance
                .slices
                .iter()
                .any(|one| (one.object.addressed && one.object.captured) || one.object.kind == MemoryKind::Absolute)
    })
}

/// Python `mir._through_pointer`: whether `ref`'s address is a value
/// rather than a named object plus an index.
fn _through_pointer(reference: &MemRef) -> bool {
    !reference.object
}

/// Python `mir.overlap_bucket`'s tuple: what `overlapping` needs of a cell
/// to rule a write out unseen -- its one object (None if it has no single
/// one), the frame `_displaced` compares displacements in, and the
/// object's alias class.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct OverlapShape {
    pub object: Option<ObjectRef>,
    pub frame: Option<Frame>,
    pub class: Option<AliasClass>,
}

/// An interned `OverlapShape`: equal shapes share one id, so a cell map
/// hashes and compares a word rather than an object's identity. Ids compare
/// only between buckets of one `OverlapBuckets`.
#[derive(Clone, Debug)]
pub struct OverlapBucket {
    id: u32,
    shape: Rc<OverlapShape>,
}

impl PartialEq for OverlapBucket {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.id == other.id
    }
}

impl Eq for OverlapBucket {}

impl std::hash::Hash for OverlapBucket {
    fn hash<H: std::hash::Hasher>(
        &self,
        state: &mut H,
    ) {
        self.id.hash(state);
    }
}

impl PartialOrd for OverlapBucket {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OverlapBucket {
    fn cmp(
        &self,
        other: &Self,
    ) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

/// The buckets one analysis run names: every bucket of one cell map must
/// come from one of these.
#[derive(Default)]
pub struct OverlapBuckets {
    named: HashMap<OverlapShape, OverlapBucket>,
}

impl OverlapBuckets {
    fn interned(
        &mut self,
        shape: OverlapShape,
    ) -> OverlapBucket {
        let next = u32::try_from(self.named.len()).expect("fewer than 2^32 buckets");
        self.named
            .entry(shape)
            .or_insert_with_key(|shape| OverlapBucket { id: next, shape: Rc::new(shape.clone()) })
            .clone()
    }
}

/// Python `mir._frame`'s tuple: what displacements count from, a root and
/// its one scaled index.
pub type Frame = (Operand, Option<(ValueId, i64)>);

/// The buckets held, by object, frame and alias class: Python's `parts`.
#[derive(Clone, Default)]
pub struct OverlapParts {
    pub objects: HashMap<ObjectRef, HashSet<OverlapBucket>>,
    pub objectless: HashSet<OverlapBucket>,
    pub frames: HashMap<Option<Frame>, HashSet<OverlapBucket>>,
    pub classes: HashMap<Option<AliasClass>, HashSet<OverlapBucket>>,
}

impl Bucket for OverlapBucket {
    type Parts = OverlapParts;

    fn held(
        &self,
        parts: &mut OverlapParts,
    ) {
        let shape = &*self.shape;
        match &shape.object {
            Some(object) => parts.objects.entry(*object).or_default().insert(self.clone()),
            None => parts.objectless.insert(self.clone()),
        };
        parts.frames.entry(shape.frame).or_default().insert(self.clone());
        parts.classes.entry(shape.class).or_default().insert(self.clone());
    }

    fn released(
        &self,
        parts: &mut OverlapParts,
    ) {
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
        let shape = &*self.shape;
        match &shape.object {
            Some(object) => drop_from(&mut parts.objects, object, self),
            None => {
                parts.objectless.remove(self);
            }
        }
        drop_from(&mut parts.frames, &shape.frame, self);
        drop_from(&mut parts.classes, &shape.class, self);
    }
}

/// Python `mir.overlap_bucket`.
pub fn overlap_bucket(
    buckets: &mut OverlapBuckets,
    reference: &MemRef,
) -> OverlapBucket {
    let one = reference
        .provenance
        .as_ref()
        .filter(|provenance| provenance.slices.len() == 1)
        .map(|provenance| provenance.slices.first().expect("one slice").object.clone());
    object_bucket(buckets, one, _frame(reference))
}

/// Python `mir.object_bucket`.
pub fn object_bucket(
    buckets: &mut OverlapBuckets,
    one: Option<ObjectRef>,
    frame: Option<Frame>,
) -> OverlapBucket {
    let class = one.as_ref().map(|one| alias_class(one));
    buckets.interned(OverlapShape { object: one, frame, class })
}

/// Python `mir.overlap_buckets`: the buckets held (`parts`, of a map keyed
/// by `object_bucket`) a write through `reference` may reach; None for all
/// of them.
///
/// Only these can hold a cell `overlapping` does not rule out: one whose
/// object is unknown, one in the write's `_displaced` frame, one in the
/// write's own object, and one whose alias class may alias the write's.
///
/// Sorted and without repeats: a set per write, rehashed as it grew, was dearer
/// than the kill.
pub fn overlap_buckets(
    reference: &MemRef,
    parts: &OverlapParts,
) -> Option<Vec<OverlapBucket>> {
    #[cfg(test)]
    PICKED.with(|picked| picked.set((picked.get().0 + 1, picked.get().1 + parts.classes.len())));
    let provenance = reference.provenance.as_ref()?;
    let mut reached = Vec::with_capacity(16);
    reached.extend(parts.objectless.iter().cloned());
    if let Some(frame) = _frame(reference) {
        if let Some(buckets) = parts.frames.get(&Some(frame)) {
            reached.extend(buckets.iter().cloned());
        }
    }
    // A write names one or two objects, and a set of slices keeps one object's
    // together (it orders by object first): the classes seen sit in a few
    // inline slots, and a spill only past them.
    let mut few = [None::<AliasClass>; 6];
    let mut spill = Vec::new();
    let mut last = None;
    for one in &provenance.slices {
        if last == Some(one.object) {
            continue;
        }
        last = Some(one.object);
        if let Some(buckets) = parts.objects.get(&one.object) {
            reached.extend(buckets.iter().cloned());
        }
        let kind = alias_class(&one.object);
        if !few.contains(&Some(kind)) && !spill.contains(&kind) {
            match few.iter_mut().find(|slot| slot.is_none()) {
                Some(slot) => *slot = Some(kind),
                None => spill.push(kind),
            }
        }
    }
    for (kind, buckets) in &parts.classes {
        if kind.is_some_and(|kind| few.iter().flatten().chain(&spill).any(|one| classes_may_alias(*one, kind))) {
            reached.extend(buckets.iter().cloned());
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
    pub static PICKED: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
}

/// Python `mir._frame`: none for bytes named by object alone.
fn _frame(reference: &MemRef) -> Option<Frame> {
    Some((reference.root?, reference.base.map(|base| (base, reference.scale))))
}

/// Python `mir._displaced`: whether two references off one root and index
/// are disjoint by displacement; `None` if not one frame.
///
/// LLVM's constant-offset GEP compare: a fact about values, so it holds
/// whatever object either reference names.
fn _displaced(
    one: &MemRef,
    other: &MemRef,
) -> Option<bool> {
    let (one, other) = (_span(one)?, _span(other)?);
    if one.0 != other.0 {
        return None;
    }
    Some(!(one.1 < other.2 && other.1 < one.2))
}

/// A reference's bytes [low, high) in its frame.
pub type ByteRange = (i128, i128);

/// Python `mir._span`: the frame `_displaced` measures `reference` in and
/// the bytes it covers there.
fn _span(reference: &MemRef) -> Option<(Frame, i128, i128)> {
    let low = i128::from(reference.disp);
    Some((_frame(reference)?, low, low + i128::from(reference.width)))
}

/// Python `mir.displaced_span`: the frame and bytes where `overlapping`
/// settles `reference` against every reference of that frame by
/// displacement alone: one whose bytes miss these cannot overlap it.
///
/// None for an address off an index, which `covering` may widen before
/// the displacements are compared.
pub fn displaced_span(reference: &MemRef) -> Option<(Frame, i128, i128)> {
    _span(reference).filter(|(frame, _, _)| frame.1.is_none())
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

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use llrm_mir::datalayout::DataLayout;
    use llrm_mir::module::{Module, Operand};
    use llrm_mir::spaces::Spaces;
    pub use llrm_x86_m16::Dos;

    use super::{RegionError, may_alias, overlapping, typed_apart};
    use crate::memory::{Identity, MemRef, MemoryKind, MemoryObject, Provenance, Unit};
    use crate::ranges::Interval;
    use crate::testing::{DOS, function, layout, parsed, value};

    /// `module` alone, a program for real-mode DOS.
    pub fn dos(module: &Module) -> std::rc::Rc<llrm_mir::program::ProgramProxy> {
        llrm_mir::program::ProgramProxy::of(module, std::rc::Rc::new(Dos::default()))
    }

    /// Every access `@f` of `text` makes, in order.
    fn accesses(
        module: &Module,
        layout: &DataLayout,
    ) -> Vec<MemRef> {
        let f = function(module, "f");
        let unit = crate::testing::with_registers(Unit::of(module, layout, f)).with_spaces(llrm_x86_m16::spaces());
        f.walk().filter_map(|(_, inst)| MemRef::of(&unit, inst)).collect()
    }

    fn module(body: &str) -> Module {
        parsed(&format!("{DOS}{body}"))
    }

    fn global(
        index: u32,
        extent: Option<i64>,
    ) -> MemoryObject {
        MemoryObject { identity: Some(Identity::Global(index)), extent, ..MemoryObject::new(MemoryKind::Global) }
    }

    fn with(
        reference: &MemRef,
        provenance: Provenance,
    ) -> MemRef {
        MemRef { provenance: Some(provenance), ..reference.clone() }
    }

    fn one(
        object: &MemoryObject,
        low: i64,
        high: i64,
    ) -> Provenance {
        Provenance::one_with_slice(object.clone(), low, high, 1, 1, BTreeSet::new()).unwrap()
    }

    fn interval(
        low: i64,
        high: i64,
        width: u32,
    ) -> Interval {
        Interval { low: low.into(), high: high.into(), width }
    }

    const TAGS: &str = "!0 = !{!\"qb\"}
!1 = !{!\"place\", !0, i64 0}
!2 = !{!\"allocation\", !0, i64 0}
!3 = !{!1, !1, i64 0}
!4 = !{!2, !2, i64 0}
";

    #[test]
    fn incompatible_typed_scalars_are_apart_except_for_same_union_start() {
        let module = module(&format!(
            "define void @f(ptr %p, ptr %q) {{
b0:
  store i16 1, ptr %p, !tbaa !3
  store i32 2, ptr %p, !tbaa !4
  %r = getelementptr i8, ptr %p, i16 4
  store i32 3, ptr %r, !tbaa !4
  store i32 4, ptr %q, !tbaa !4
  ret void
}}

{TAGS}"
        ));
        let dl = layout(&module);
        let [short, union_view, unrelated, elsewhere] = &accesses(&module, &dl)[..] else { panic!() };

        assert!(!typed_apart(short, union_view));
        assert!(typed_apart(short, unrelated));
        assert!(typed_apart(short, elsewhere));
        // Two roots one object starts: the union view by provenance.
        let object = global(1, Some(8));
        assert!(!typed_apart(&with(short, one(&object, 0, 1)), &with(elsewhere, one(&object, 0, 1))));
    }

    /// Types tell apart what the address analysis cannot place; two accesses it
    /// places in one object at overlapping bytes are not told apart by
    /// their types (`long long` stored, `int` read at 4 of it, #677), and ones
    /// whose bytes do not meet still are.
    #[test]
    fn a_known_overlap_outranks_the_types() {
        let module = module(&format!(
            "define void @f(ptr %p, ptr %q) {{
b0:
  store i16 1, ptr %p, !tbaa !3
  store i32 4, ptr %q, !tbaa !4
  ret void
}}

{TAGS}"
        ));
        let dl = layout(&module);
        let [short, wide] = &accesses(&module, &dl)[..] else { panic!() };
        let object = global(1, Some(8));
        // The short is bytes 0-1; the wide, at 1, is bytes 1-4 and at 2, bytes
        // 2-5.
        assert!(!typed_apart(&with(short, one(&object, 0, 1)), &with(wide, one(&object, 1, 2))));
        assert!(typed_apart(&with(short, one(&object, 0, 1)), &with(wide, one(&object, 2, 3))));
    }

    const C_TAGS: &str = "!0 = !{!\"Simple C/C++ TBAA\"}
!1 = !{!\"omnipotent char\", !0, i64 0}
!2 = !{!\"int2\", !1, i64 0}
!3 = !{!\"int4\", !1, i64 0}
!4 = !{!1, !1, i64 0}
!5 = !{!2, !2, i64 0}
!6 = !{!3, !3, i64 0}
!7 = !{!\"llrm hir\"}
!8 = !{!\"place\", !7, i64 0}
!9 = !{!8, !8, i64 0}
";

    /// `*q = 1` through a `char *` between two loads of `*p` was no write of
    /// `*p`: the types differed by name, and `omnipotent char`, the parent of
    /// every scalar type, was not read as one that covers them. C lets a
    /// char access any object (llrm-c at -O2 added `*p` to itself).
    #[test]
    fn a_type_covers_its_descendants_and_roots_are_not_compared() {
        let module = module(&format!(
            "define void @f(ptr %p, ptr %q, ptr %r, ptr %s) {{
b0:
  store i8 1, ptr %q, !tbaa !4
  store i16 2, ptr %p, !tbaa !5
  store i32 3, ptr %r, !tbaa !6
  store i16 4, ptr %s, !tbaa !9
  ret void
}}

{C_TAGS}"
        ));
        let dl = layout(&module);
        let [character, short, long, place] = &accesses(&module, &dl)[..] else { panic!() };
        // The parent covers a child; siblings are apart; another root says
        // nothing.
        assert!(!typed_apart(character, short));
        assert!(!typed_apart(long, character));
        assert!(typed_apart(short, long));
        assert!(!typed_apart(short, place));
    }

    #[test]
    fn provenance_alias_uses_canonical_subobjects_and_byte_ranges() {
        // Fields of one object, and equal offsets in distinct objects, are
        // disjoint.
        let module = module(
            "define void @f(ptr %p) {
b0:
  store i32 0, ptr %p
  ret void
}
",
        );
        let dl = layout(&module);
        let access = &accesses(&module, &dl)[0];
        let (first, second) = (global(1, Some(8)), global(2, Some(8)));
        let a = with(access, one(&first, 0, 4));
        let b = with(access, one(&first, 4, 8));
        let c = with(access, one(&second, 0, 4));

        assert!(!may_alias(&a, &b, None, None, None).unwrap());
        assert!(!may_alias(&a, &c, None, None, None).unwrap());
        assert!(may_alias(&a, &a, None, None, None).unwrap());
    }

    #[test]
    fn provenance_alias_respects_strides_restrict_roots_and_typed_union_starts() {
        let module = module(&format!(
            "define void @f(ptr %p, ptr %q) {{
b0:
  store i32 0, ptr %p, !tbaa !3
  store i32 0, ptr %q, !tbaa !4
  store i32 0, ptr %q, !tbaa !3
  ret void
}}

{TAGS}"
        ));
        let dl = layout(&module);
        let [left, right, right_place] = &accesses(&module, &dl)[..] else { panic!() };
        let lanes = global(4, Some(64));
        let even = with(left, Provenance::one_with_slice(lanes.clone(), 0, 64, 2, 1, BTreeSet::new()).unwrap());
        let odd = with(right_place, Provenance::one_with_slice(lanes, 1, 64, 2, 1, BTreeSet::new()).unwrap());
        assert!(!may_alias(&MemRef { width: 1, ..even }, &MemRef { width: 1, ..odd }, None, None, None).unwrap());

        let unknown = MemoryObject::new(MemoryKind::Unknown);
        let rooted = |root: i64| {
            Provenance::one_with_slice(
                unknown.clone(),
                super::FLOOR,
                1 << 31,
                1,
                1,
                BTreeSet::from([Identity::Int(root)]),
            )
            .unwrap()
        };
        let left = with(left, rooted(1));
        let right = with(right, rooted(2));
        assert!(!may_alias(&left, &right, None, None, None).unwrap());

        // With matching types, only the disjoint restrict roots make this
        // pair disjoint, so the provenance path is exercised directly.
        let restrict_only = with(right_place, rooted(2));
        assert!(!may_alias(&left, &restrict_only, None, None, None).unwrap());

        // Removing the roots leaves incompatible types apart.
        let typed_only = with(&right, Provenance::one(unknown));
        assert!(!may_alias(&left, &typed_only, None, None, None).unwrap());

        // One union start: different types, one address.
        let union = module_union();
        let union_dl = layout(&union);
        let [int, float] = &accesses(&union, &union_dl)[..] else { panic!() };
        assert!(may_alias(int, float, None, None, None).unwrap());
    }

    fn module_union() -> Module {
        module(&format!(
            "define void @f() {{
b0:
  %u = alloca [4 x i8]
  store i32 0, ptr %u, !tbaa !3
  store float 0.0, ptr %u, !tbaa !4
  ret void
}}

{TAGS}"
        ))
    }

    #[test]
    fn index_interval_narrows_once_and_refuses_out_of_extent_accesses() {
        // One dword index start touches bytes 0..4, not 0..8.
        let module = module(
            "@g = global [16 x i8] zeroinitializer
@h = global [4 x i8] zeroinitializer

define void @f(i16 %i) {
b0:
  %e = getelementptr i8, ptr @g, i16 %i
  store i32 0, ptr %e
  store i32 0, ptr getelementptr (i8, ptr @g, i16 4)
  %d = getelementptr i8, ptr @h, i16 %i
  store i32 0, ptr %d
  store i8 0, ptr getelementptr (i8, ptr @h, i16 3)
  ret void
}
",
        );
        let dl = layout(&module);
        let [indexed, next_field, bounded, last] = &accesses(&module, &dl)[..] else { panic!() };
        let index = value(function(&module, "f"), "i");
        let (object, small) = (global(0, Some(16)), global(1, Some(4)));
        let indexed = with(indexed, Provenance::one(object.clone()));
        let next_field = with(next_field, one(&object, 4, 8));
        let known = BTreeMap::from([(index, interval(0, 0, 16))]);
        assert!(!may_alias(&indexed, &next_field, Some(&known), None, None).unwrap());

        // A fact that would reach outside a bounded object is not used to
        // narrow it. The original whole-object provenance remains conservative.
        let bounded = with(bounded, Provenance::one(small.clone()));
        let last = with(last, one(&small, 3, 4));
        let outside = BTreeMap::from([(index, interval(0, 1, 16))]);
        assert!(may_alias(&bounded, &last, Some(&outside), None, None).unwrap());
    }

    #[test]
    fn narrowing_reports_unrepresentable_python_slices() {
        let module = module(
            "@g = global [16 x i8] zeroinitializer

define void @f(i16 %i) {
b0:
  %e = getelementptr i8, ptr @g, i16 %i
  store i8 0, ptr %e
  ret void
}
",
        );
        let dl = layout(&module);
        let indexed = with(&accesses(&module, &dl)[0], Provenance::one(global(0, None)));
        let index = value(function(&module, "f"), "i");
        let too_large = BTreeMap::from([(index, interval(i64::MAX, i64::MAX, 16))]);
        assert_eq!(
            may_alias(&indexed, &indexed, Some(&too_large), None, None),
            Err(RegionError::NarrowedSliceUnrepresentable)
        );
    }

    #[test]
    fn overlapping_far_segments_need_one_root_before_offsets_decide() {
        // Far offsets alone do not identify a byte: 1000:0020 and 1001:0010
        // can name the same address.
        let module = module(
            "define void @f(i16 %a, i16 %b, i16 %o) {
b0:
  %sa = inttoptr i16 %a to ptr addrspace(2)
  %fa = addrspacecast ptr addrspace(2) %sa to ptr addrspace(1)
  %sb = inttoptr i16 %b to ptr addrspace(2)
  %fb = addrspacecast ptr addrspace(2) %sb to ptr addrspace(1)
  %pa = getelementptr i8, ptr addrspace(1) %fa, i16 %o
  %one = getelementptr i8, ptr addrspace(1) %pa, i16 32
  %pb = getelementptr i8, ptr addrspace(1) %fb, i16 %o
  %other = getelementptr i8, ptr addrspace(1) %pb, i16 16
  %same = getelementptr i8, ptr addrspace(1) %pa, i16 16
  store i16 0, ptr addrspace(1) %one
  store i16 0, ptr addrspace(1) %other
  store i16 0, ptr addrspace(1) %same
  ret void
}
",
        );
        let dl = layout(&module);
        let [one, other, same] = &accesses(&module, &dl)[..] else { panic!() };
        assert_eq!(overlapping(one, other, None, None, None), Ok(true));
        assert_eq!(overlapping(other, one, None, None, None), Ok(true));
        assert_eq!(overlapping(one, same, None, None, None), Ok(false));
    }

    #[test]
    fn overlapping_keeps_indexed_objects_disjoint() {
        let module = module(
            "@g = global [64 x i8] zeroinitializer
@h = global [64 x i8] zeroinitializer

define void @f(i16 %i) {
b0:
  %a = getelementptr i8, ptr @g, i16 %i
  %b = getelementptr i8, ptr @h, i16 %i
  store i16 0, ptr %a
  store i16 0, ptr %b
  ret void
}
",
        );
        let dl = layout(&module);
        let [one, other] = &accesses(&module, &dl)[..] else { panic!() };
        let one = with(one, Provenance::one(global(0, Some(64))));
        let other = with(other, Provenance::one(global(1, Some(64))));
        assert_eq!(overlapping(&one, &other, None, None, None), Ok(false));
    }

    #[test]
    fn overlapping_same_base_statics_use_half_open_byte_ranges() {
        let module = module(
            "@g = global [64 x i8] zeroinitializer

define void @f() {
b0:
  store i32 0, ptr getelementptr (i8, ptr @g, i16 10)
  store i32 0, ptr getelementptr (i8, ptr @g, i16 13)
  store i32 0, ptr getelementptr (i8, ptr @g, i16 14)
  ret void
}
",
        );
        let dl = layout(&module);
        let [one, overlaps, adjacent] = &accesses(&module, &dl)[..] else { panic!() };

        assert_eq!(overlapping(one, overlaps, None, None, None), Ok(true));
        assert_eq!(overlapping(one, adjacent, None, None, None), Ok(false));
    }

    #[test]
    fn overlapping_range_covering_respects_width_wrap_and_each_fact_map() {
        // tests/test_ranges.py::test_range_alias_checks_cover_width_and_wrap.
        for (low, high, interval_width, offset, width, overlaps) in [
            (0_i64, 20_i64, 16_u32, 26_i64, 2_u32, false),
            (0, 20, 16, 25, 2, true),
            (0, 20, 16, 100, 4, false),
            (-8, 20, 16, 100, 4, true),
            (0, 65_535, 16, 100, 4, true),
            (0, 20, 32, 100, 4, true),
        ] {
            let module = module(&format!(
                "@g = global [256 x i8] zeroinitializer

define void @f(i16 %i) {{
b0:
  %e = getelementptr i8, ptr @g, i16 %i
  %a = getelementptr i8, ptr %e, i16 4
  store i16 0, ptr %a
  store i{} 0, ptr getelementptr (i8, ptr @g, i16 {offset})
  ret void
}}
",
                width * 8
            ));
            let dl = layout(&module);
            let [indexed, fixed] = &accesses(&module, &dl)[..] else { panic!() };
            let base = value(function(&module, "f"), "i");
            let known = BTreeMap::from([(base, interval(low, high, interval_width))]);

            assert_eq!(
                overlapping(indexed, fixed, Some(&known), None, None),
                Ok(overlaps),
                "left facts: {low}..{high}, static {offset}/{width}"
            );
            assert_eq!(
                overlapping(fixed, indexed, None, Some(&known), None),
                Ok(overlaps),
                "right facts: {low}..{high}, static {offset}/{width}"
            );
            assert!(overlapping(indexed, fixed, None, None, None).unwrap());
        }
    }

    #[test]
    fn overlapping_incompatible_scalars_are_apart_except_at_a_union_start() {
        let module = module(&format!(
            "define void @f() {{
b0:
  %u = alloca [8 x i8]
  store i16 0, ptr %u, !tbaa !3
  store i32 0, ptr %u, !tbaa !4
  %d = getelementptr i8, ptr %u, i16 4
  store i32 0, ptr %d, !tbaa !4
  ret void
}}

{TAGS}"
        ));
        let dl = layout(&module);
        let [short, long, distinct] = &accesses(&module, &dl)[..] else { panic!() };

        assert_eq!(overlapping(short, long, None, None, None), Ok(true));
        assert_eq!(overlapping(short, distinct, None, None, None), Ok(false));
    }

    #[test]
    fn overlapping_settles_one_base_by_displacement_before_provenance() {
        // One root and one displacement settle it before the objects do:
        // Python `mir._displaced`, whatever object either names.
        let module = module(
            "define void @f(ptr %p) {
b0:
  store i16 0, ptr %p
  ret void
}
",
        );
        let dl = layout(&module);
        let access = &accesses(&module, &dl)[0];
        let one = with(access, Provenance::one(global(1, Some(4))));
        let other = with(access, Provenance::one(global(2, Some(4))));

        assert_eq!(overlapping(&one, &other, None, None, None), Ok(true));
    }

    /// A POKE to text memory at an unknown offset was taken to hit a BYREF
    /// argument: every counted loop reloaded its parameters.
    #[test]
    fn test_a_data_group_access_misses_foreign_memory() {
        let module = module(
            "define void @f(ptr %p, i16 %sel, i16 %o) {
b0:
  store i16 0, ptr %p
  %s = inttoptr i16 %sel to ptr addrspace(2)
  %far = addrspacecast ptr addrspace(2) %s to ptr addrspace(1)
  %t = getelementptr i8, ptr addrspace(1) %far, i16 %o
  store i8 0, ptr addrspace(1) %t
  ret void
}
",
        );
        let dl = layout(&module);
        let [near, text] = &accesses(&module, &dl)[..] else { panic!() };
        let selector = value(function(&module, "f"), "sel");
        let foreign = BTreeMap::from([(selector, interval(0xB800, 0xB800, 16))]);
        let ordinary = BTreeMap::from([(selector, interval(0x1234, 0x1234, 16))]);

        let dos = dos(&module);
        assert!(!overlapping(near, text, None, Some(&foreign), Some(&dos)).unwrap());
        assert!(!may_alias(text, near, Some(&foreign), None, Some(&dos)).unwrap());
        assert!(overlapping(near, text, None, Some(&ordinary), Some(&dos)).unwrap());
        assert!(overlapping(near, text, None, Some(&foreign), None).unwrap());
    }

    /// A store through a pointer the language puts at a fixed address
    /// (address space 4) cannot reach a program global, however its pointer
    /// was made; through a far pointer it can, a far pointer reaching any
    /// object. Every POKE to a stated device kept the reloads of the
    /// program's own data behind it.
    #[test]
    fn test_an_access_at_a_fixed_address_misses_program_objects() {
        for (space, apart) in [(4, true), (1, false)] {
            let module = module(&format!(
                "@g = global i16 0

define void @f(ptr addrspace({space}) %p) {{
b0:
  %a = load i16, ptr @g
  store i8 0, ptr addrspace({space}) %p
  ret void
}}
"
            ));
            let dl = layout(&module);
            let f = function(&module, "f");
            let unit = crate::testing::with_registers(Unit::of(&module, &dl, f)).with_spaces(llrm_x86_m16::spaces());
            let found = crate::alias::annotated(&unit).unwrap();
            let [load, store] = &found.values().cloned().collect::<Vec<_>>()[..] else { panic!("two accesses") };
            assert_eq!(!may_alias(load, store, None, None, None).unwrap(), apart, "space {space}");
        }
    }

    /// The language's word that an address is a device's is checked against
    /// the target where the address is a constant: DOS loads a program
    /// anywhere in conventional memory, so a constant segment there (0x1234)
    /// may be the program's own data and is no fixed address; B800 is video.
    #[test]
    fn test_a_constant_segment_in_conventional_memory_is_no_fixed_address() {
        for (segment, apart) in [(0xB800, true), (0x1234, false)] {
            let module = module(&format!(
                "@g = global i16 0

define void @f() {{
b0:
  %a = load i16, ptr @g
  %s = inttoptr i16 {segment} to ptr addrspace(2)
  %far = addrspacecast ptr addrspace(2) %s to ptr addrspace(1)
  %dev = addrspacecast ptr addrspace(1) %far to ptr addrspace(4)
  store i8 0, ptr addrspace(4) %dev
  ret void
}}
"
            ));
            let dl = layout(&module);
            let f = function(&module, "f");
            let dos = dos(&module);
            let mut unit =
                crate::testing::with_registers(Unit::of(&module, &dl, f)).with_spaces(llrm_x86_m16::spaces());
            unit.program = Some(&dos);
            let found = crate::alias::annotated(&unit).unwrap();
            let [load, store] = &found.values().cloned().collect::<Vec<_>>()[..] else { panic!("two accesses") };
            assert_eq!(!may_alias(load, store, None, None, Some(&dos)).unwrap(), apart, "segment {segment:#x}");
        }
    }

    /// A far pointer an integer constant makes is its selector and offset,
    /// the integer's high and low words: C's `(char far *)0xB8000010L` is
    /// video memory, apart from the program's global. It was no known
    /// segment, so a loop storing there read its other variables again. At
    /// 0x1234 it may be the program's data.
    #[test]
    fn test_a_far_pointer_made_from_a_constant_is_placed_by_its_words() {
        for (integer, apart) in [(0xB800_0010_u32, true), (0x1234_0010, false)] {
            let module = module(&format!(
                "@g = global i16 0

define void @f() {{
b0:
  %a = load i16, ptr @g
  %far = inttoptr i32 {} to ptr addrspace(1)
  %at = getelementptr i8, ptr addrspace(1) %far, i16 2
  store volatile i8 0, ptr addrspace(1) %at
  ret void
}}
",
                integer as i32
            ));
            let dl = layout(&module);
            let f = function(&module, "f");
            let dos = dos(&module);
            let mut unit =
                crate::testing::with_registers(Unit::of(&module, &dl, f)).with_spaces(llrm_x86_m16::spaces());
            unit.program = Some(&dos);
            let found = crate::alias::annotated(&unit).unwrap();
            let [load, store] = &found.values().cloned().collect::<Vec<_>>()[..] else { panic!("two accesses") };
            assert_eq!((store.selector, store.origin, store.disp), (Some(i64::from(integer >> 16)), 0x10, 2));
            assert_eq!(!may_alias(load, store, None, None, Some(&dos)).unwrap(), apart, "{integer:#x}");
        }
    }

    /// Only an address space the segment layout places program data in is
    /// apart from foreign memory: address space 0 was taken to be, whatever
    /// the layout.
    #[test]
    fn test_a_space_holding_no_program_data_may_meet_foreign_memory() {
        let module = module(
            "define void @f(ptr %p, i16 %sel, i16 %o) {
b0:
  store i16 0, ptr %p
  %s = inttoptr i16 %sel to ptr addrspace(2)
  %far = addrspacecast ptr addrspace(2) %s to ptr addrspace(1)
  %t = getelementptr i8, ptr addrspace(1) %far, i16 %o
  store i8 0, ptr addrspace(1) %t
  ret void
}
",
        );
        let dl = layout(&module);
        let [near, text] = &accesses(&module, &dl)[..] else { panic!() };
        let selector = value(function(&module, "f"), "sel");
        let foreign = BTreeMap::from([(selector, interval(0xB800, 0xB800, 16))]);
        let mut elsewhere = std::rc::Rc::unwrap_or_clone(dos(&module));
        (elsewhere.segments.data_space, elsewhere.segments.stack_space) = (3, 3);
        assert!(overlapping(near, text, None, Some(&foreign), Some(&elsewhere)).unwrap());
    }

    #[test]
    fn a_constant_selector_lands_in_foreign_memory_without_facts() {
        let module = module(
            "define void @f(ptr %p) {
b0:
  store i16 0, ptr %p
  %s = inttoptr i16 -18432 to ptr addrspace(2)
  %far = addrspacecast ptr addrspace(2) %s to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %far
  ret void
}
",
        );
        let dl = layout(&module);
        let [near, text] = &accesses(&module, &dl)[..] else { panic!() };
        assert_eq!(text.selector, Some(0xB800));
        assert_eq!(text.root, Some(Operand::Value(value(function(&module, "f"), "far"))));
        assert!(!overlapping(near, text, None, None, Some(&dos(&module))).unwrap());
    }

    /// A flat address wrapped modulo 2^32 whatever the target's pointer: on a
    /// 64-bit one the video buffer's range was taken for 0x1_000B_8000, which
    /// a program's data may occupy. The word is the pointer's.
    #[test]
    fn a_flat_words_size_is_the_pointer_not_32_bits() {
        let word = |layout: &str| super::flat_word(&DataLayout::parse(layout).unwrap(), &Spaces::FLAT);
        assert_eq!(word("e-p:16:16-n8:16"), Some(1 << 16));
        assert_eq!(word("e-p:32:32-n8:16:32"), Some(1 << 32));
        assert_eq!(word("e-p:64:64-n8:16:32:64"), None);
    }
}
