//! Machine-independent memory objects and byte-accurate access paths.
//!
//! Direct port of `qbopt/model/memory.py`: `Kind`, `Object`, `Slice`,
//! `Provenance`, and `objects_may_alias`.  Slices are half-open; a stride
//! greater than one describes selected byte lanes, not merely their hull.

use std::collections::BTreeSet;
use std::fmt;

use crate::model::mir::{Symbol, Value};
use crate::support::pyrepr::{self, Repr};
use crate::objectfile::module::Space;

/// Python `qbopt.model.memory:Kind`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MemoryKind {
    Unknown,
    Stack,
    Frame,
    Global,
    External,
    Nonlocal,
    Allocation,
    Absolute,
    Named,
    Parameter,
}

impl MemoryKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Stack => "stack",
            Self::Frame => "frame",
            Self::Global => "global",
            Self::External => "external",
            Self::Nonlocal => "nonlocal",
            Self::Allocation => "allocation",
            Self::Absolute => "absolute",
            Self::Named => "named",
            Self::Parameter => "parameter",
        }
    }
}

impl Repr for MemoryKind {
    fn repr(&self) -> String {
        pyrepr::str_enum("Kind", &self.as_str().to_uppercase(), self.as_str())
    }
}

impl fmt::Display for MemoryKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Python's `object`, as `Object.identity` and `Provenance.restrict` hold it:
/// ints, strings, spaces, symbols, values and tuples of them.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Identity {
    Int(i64),
    Str(String),
    Space(Space),
    Symbol(Symbol),
    Value(Value),
    Tuple(Vec<Identity>),
}

impl Repr for Identity {
    fn repr(&self) -> String {
        match self {
            Identity::Int(one) => one.repr(),
            Identity::Str(one) => one.repr(),
            Identity::Space(one) => one.repr(),
            Identity::Symbol(one) => one.repr(),
            Identity::Value(one) => one.repr(),
            Identity::Tuple(items) => pyrepr::tuple(items),
        }
    }
}

/// Python `qbopt.model.memory:Object`.
///
/// `addressed` and `captured` are `field(compare=False)`: equality, hashing
/// and order ignore them.
#[derive(Clone, Debug)]
pub struct MemoryObject {
    pub kind: MemoryKind,
    pub identity: Option<Identity>,
    pub generation: i64,
    pub extent: Option<i64>,
    // Facts about the object, not its identity: two spellings of one object
    // are the same object whatever they say. LLVM's split, stated once:
    // `addressed` -- some code computes its address, so a pointer of unknown
    // origin may hold it. `captured` -- that address can be found from
    // outside this activation (memory, a return, a callee that keeps it), so
    // NONLOCAL and PARAMETER may reach it. Unaddressed implies uncaptured.
    pub addressed: bool,
    pub captured: bool,
}

impl MemoryObject {
    pub const fn new(kind: MemoryKind) -> Self {
        Self { kind, identity: None, generation: 0, extent: None, addressed: true, captured: true }
    }

    fn key(&self) -> (MemoryKind, &Option<Identity>, i64, Option<i64>) {
        (self.kind, &self.identity, self.generation, self.extent)
    }
}

impl PartialEq for MemoryObject {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl Eq for MemoryObject {}

impl std::hash::Hash for MemoryObject {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key().hash(state);
    }
}

impl PartialOrd for MemoryObject {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MemoryObject {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.key().cmp(&other.key())
    }
}

impl Repr for MemoryObject {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Object",
            &[
                ("kind", self.kind.repr()),
                ("identity", self.identity.repr()),
                ("generation", self.generation.repr()),
                ("extent", self.extent.repr()),
                ("addressed", self.addressed.repr()),
                ("captured", self.captured.repr()),
            ],
        )
    }
}

pub const WHOLE_LOW: i64 = -(1_i64 << 31);
pub const WHOLE_HIGH: i64 = 1_i64 << 31;

/// Python `qbopt.model.memory:Slice`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Slice {
    pub object: MemoryObject,
    pub low: i64,
    pub high: i64,
    pub stride: i64,
    pub width: i64,
}

/// Python raises `ValueError` for all three invalid `Slice` shapes.  Rust
/// callers receive the corresponding closed error instead.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SliceError {
    Empty,
    NonPositiveStride,
    NonPositiveWidth,
}

impl fmt::Display for SliceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "an alias slice must contain at least one byte",
            Self::NonPositiveStride => "an alias stride must be positive",
            Self::NonPositiveWidth => "an alias element width must be positive",
        })
    }
}

impl std::error::Error for SliceError {}

impl Slice {
    /// Python `Slice.__post_init__`, performed at construction in Rust.
    pub fn new(
        object: MemoryObject,
        low: i64,
        high: i64,
        stride: i64,
        width: i64,
    ) -> Result<Self, SliceError> {
        if high <= low {
            return Err(SliceError::Empty);
        }
        if stride <= 0 {
            return Err(SliceError::NonPositiveStride);
        }
        if width <= 0 {
            return Err(SliceError::NonPositiveWidth);
        }
        Ok(Self {
            object,
            low,
            high,
            stride,
            width,
        })
    }

    pub fn whole(object: MemoryObject) -> Self {
        Self::new(object, WHOLE_LOW, WHOLE_HIGH, 1, 1)
            .expect("the fixed whole-object slice is valid")
    }

    /// Direct port of `Slice.shifted`.
    pub fn shifted(&self, amount: i64) -> Self {
        Self::new(
            self.object.clone(),
            self.low + amount,
            self.high + amount,
            self.stride,
            self.width,
        )
        .expect("shifting a valid slice retains its positive shape")
    }

    /// Direct port of `Slice.intersects`.
    pub fn intersects(&self, other: &Self) -> bool {
        if !objects_may_alias(&self.object, &other.object) {
            return false;
        }

        let mine_low = i128::from(self.low);
        let mine_high = i128::from(self.high);
        let mine_width = i128::from(self.width);
        let theirs_low = i128::from(other.low);
        let theirs_high = i128::from(other.high);
        let theirs_width = i128::from(other.width);
        // Byte offsets have a common origin only for the same concrete object.
        if self.object != other.object {
            return true;
        }
        if mine_low >= theirs_high + theirs_width - 1 || theirs_low >= mine_high + mine_width - 1 {
            return false;
        }

        let divisor = gcd(self.stride, other.stride);
        for mine in 0..self.width {
            for theirs in 0..other.width {
                let mine_start = mine_low + i128::from(mine);
                let theirs_start = theirs_low + i128::from(theirs);
                if (mine_start - theirs_start) % i128::from(divisor) != 0 {
                    continue;
                }
                let low = mine_start.max(theirs_start);
                let high = (mine_high + i128::from(mine)).min(theirs_high + i128::from(theirs));
                let stride = i128::from(self.stride);
                let at = mine_start + ((low - mine_start + stride - 1) / stride) * stride;
                let limit = high.min(at + i128::from(other.stride / divisor) * stride);
                let other_stride = i128::from(other.stride);
                let mut at = at;
                while at < limit {
                    if (at - theirs_start) % other_stride == 0 {
                        return true;
                    }
                    at += stride;
                }
            }
        }
        false
    }
}

pub(crate) fn gcd(mut one: i64, mut other: i64) -> i64 {
    while other != 0 {
        (one, other) = (other, one % other);
    }
    one
}

/// Python `qbopt.model.memory:Provenance`.
///
/// `BTreeSet` supplies the deterministic iteration Python's `frozenset`
/// presentation lacks while preserving order-independent equality.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Provenance {
    pub slices: BTreeSet<Slice>,
    pub restrict: BTreeSet<Identity>,
}

impl Provenance {
    /// Direct port of `Provenance.one` with Python's default whole-object
    /// bounds and no restrict roots.
    pub fn one(object: MemoryObject) -> Self {
        Self {
            slices: BTreeSet::from([Slice::whole(object)]),
            restrict: BTreeSet::new(),
        }
    }

    /// `Provenance.one` with its explicit slice and restrict arguments.
    pub fn one_with_slice(
        object: MemoryObject,
        low: i64,
        high: i64,
        stride: i64,
        width: i64,
        restrict: BTreeSet<Identity>,
    ) -> Result<Self, SliceError> {
        Ok(Self {
            slices: BTreeSet::from([Slice::new(object, low, high, stride, width)?]),
            restrict,
        })
    }

    /// Direct port of `Provenance.shifted`.
    pub fn shifted(&self, amount: i64) -> Self {
        let slices = self
            .slices
            .iter()
            .map(|one| {
                let whole = one.low == WHOLE_LOW && one.high == WHOLE_HIGH;
                let bounded_whole = one
                    .object
                    .extent
                    .is_some_and(|extent| one.low == 0 && one.high == extent);
                if whole || bounded_whole {
                    one.clone()
                } else {
                    one.shifted(amount)
                }
            })
            .collect();
        Self {
            slices,
            restrict: self.restrict.clone(),
        }
    }

    /// Direct port of `Provenance.union`.
    pub fn union(&self, other: &Self) -> Self {
        Self {
            slices: self.slices.union(&other.slices).cloned().collect(),
            restrict: self.restrict.union(&other.restrict).cloned().collect(),
        }
    }

    /// Direct port of `Provenance.intersects`.
    pub fn intersects(&self, other: &Self) -> bool {
        if !self.restrict.is_empty()
            && !other.restrict.is_empty()
            && self.restrict.is_disjoint(&other.restrict)
        {
            return false;
        }
        self.slices
            .iter()
            .any(|one| other.slices.iter().any(|two| one.intersects(two)))
    }
}

impl Repr for Slice {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Slice",
            &[
                ("object", self.object.repr()),
                ("low", self.low.repr()),
                ("high", self.high.repr()),
                ("stride", self.stride.repr()),
                ("width", self.width.repr()),
            ],
        )
    }
}

impl Repr for Provenance {
    fn repr(&self) -> String {
        let slices: Vec<&Slice> = self.slices.iter().collect();
        let restrict: Vec<&Identity> = self.restrict.iter().collect();
        pyrepr::dataclass(
            "Provenance",
            &[("slices", pyrepr::frozenset(&slices)), ("restrict", pyrepr::frozenset(&restrict))],
        )
    }
}

/// Direct port of `qbopt.model.memory:objects_may_alias`.
pub fn objects_may_alias(one: &MemoryObject, other: &MemoryObject) -> bool {
    if one == other {
        return true;
    }
    // Only a reference naming an unaddressed object reaches it.
    if !(one.addressed && other.addressed) {
        return false;
    }
    for (this, _that) in [(one, other), (other, one)] {
        if this.kind == MemoryKind::Unknown {
            return true;
        }
    }
    for (this, that) in [(one, other), (other, one)] {
        if this.kind == MemoryKind::Nonlocal {
            return that.captured && !matches!(that.kind, MemoryKind::Frame | MemoryKind::Stack);
        }
    }
    for (this, that) in [(one, other), (other, one)] {
        if this.kind == MemoryKind::Parameter {
            // An incoming pointer predates this activation and cannot designate
            // one of its frame objects. At a call site the parameter object is
            // replaced by the actual provenance before caller-side queries.
            return that.captured && that.kind != MemoryKind::Frame;
        }
    }
    if matches!(one.kind, MemoryKind::Global | MemoryKind::External)
        && matches!(other.kind, MemoryKind::Global | MemoryKind::External)
    {
        return one.kind == MemoryKind::External || other.kind == MemoryKind::External;
    }
    false
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        Identity, MemoryKind, MemoryObject, Provenance, Slice, SliceError, Space,
    };

    fn object(kind: MemoryKind) -> MemoryObject {
        MemoryObject::new(kind)
    }

    #[test]
    fn object_kind_rules_keep_stack_and_global_separate() {
        assert!(!super::objects_may_alias(
            &object(MemoryKind::Stack),
            &object(MemoryKind::Global)
        ));
        // Python since 8780f59b: the push area and the frame are distinct objects.
        assert!(!super::objects_may_alias(
            &object(MemoryKind::Stack),
            &object(MemoryKind::Frame)
        ));
    }

    #[test]
    fn provenance_subobjects_use_object_identity_and_byte_ranges() {
        let first = MemoryObject {
            kind: MemoryKind::Frame,
            identity: Some(Identity::Tuple(vec![Identity::Int(0), Identity::Int(-8), Identity::Int(0)])),
            generation: 0,
            extent: Some(8),
            addressed: true,
            captured: true,
        };
        let second = MemoryObject {
            identity: Some(Identity::Tuple(vec![Identity::Int(0), Identity::Int(-16), Identity::Int(-8)])),
            ..first.clone()
        };
        let a = Provenance::one_with_slice(first.clone(), 0, 4, 1, 1, BTreeSet::new()).unwrap();
        let b = Provenance::one_with_slice(first, 4, 8, 1, 1, BTreeSet::new()).unwrap();
        let c = Provenance::one_with_slice(second, 0, 4, 1, 1, BTreeSet::new()).unwrap();

        assert!(!a.intersects(&b));
        assert!(!a.intersects(&c));
        assert!(
            a.intersects(
                &Provenance::one_with_slice(
                    a.slices.first().unwrap().object.clone(),
                    0,
                    2,
                    1,
                    1,
                    BTreeSet::new(),
                )
                .unwrap()
            )
        );
    }

    #[test]
    fn provenance_strided_ranges_prove_interleaved_arrays_disjoint() {
        let object = MemoryObject {
            kind: MemoryKind::Global,
            identity: Some(Identity::Tuple(vec![Identity::Space(Space::Segment), Identity::Int(4)])),
            generation: 0,
            extent: Some(64),
            addressed: true,
            captured: true,
        };
        let even =
            Provenance::one_with_slice(object.clone(), 0, 64, 2, 1, BTreeSet::new()).unwrap();
        let odd = Provenance::one_with_slice(object, 1, 64, 2, 1, BTreeSet::new()).unwrap();

        assert!(!even.intersects(&odd));
    }

    #[test]
    fn strided_slice_intersection_matches_the_bytes_it_describes() {
        // Direct port of
        // tests/test_mir_alias.py::test_strided_slice_intersection_matches_the_bytes_it_describes.
        let object = MemoryObject {
            kind: MemoryKind::Allocation,
            identity: Some(Identity::Tuple(vec![Identity::Str("allocation".to_owned()), Identity::Int(1)])),
            generation: 0,
            extent: Some(12),
            addressed: true,
            captured: true,
        };
        let mut slices = Vec::new();
        for low in 0..4 {
            for high in (low + 1)..7 {
                for stride in 1..5 {
                    for width in 1..4 {
                        slices.push(Slice::new(object.clone(), low, high, stride, width).unwrap());
                    }
                }
            }
        }

        let bytes_of = |one: &Slice| {
            (one.low..one.high)
                .step_by(one.stride as usize)
                .flat_map(|start| (0..one.width).map(move |lane| start + lane))
                .collect::<BTreeSet<_>>()
        };
        for one in &slices {
            let one_bytes = bytes_of(one);
            for other in &slices {
                let expected = !one_bytes.is_disjoint(&bytes_of(other));
                assert_eq!(one.intersects(other), expected, "{one:?}, {other:?}");
            }
        }
    }

    #[test]
    fn provenance_restrict_roots_prove_disjoint() {
        let unknown = object(MemoryKind::Unknown);
        let left = Provenance::one_with_slice(
            unknown.clone(),
            super::WHOLE_LOW,
            super::WHOLE_HIGH,
            1,
            1,
            BTreeSet::from([Identity::Int(1)]),
        )
        .unwrap();
        let right = Provenance::one_with_slice(
            unknown,
            super::WHOLE_LOW,
            super::WHOLE_HIGH,
            1,
            1,
            BTreeSet::from([Identity::Int(2)]),
        )
        .unwrap();

        assert!(!left.intersects(&right));
    }

    #[test]
    fn invalid_slice_bounds_stride_and_width_match_python_value_errors() {
        let object = object(MemoryKind::Global);
        let empty = Slice::new(object.clone(), 1, 1, 1, 1).unwrap_err();
        let stride = Slice::new(object.clone(), 0, 1, 0, 1).unwrap_err();
        let width = Slice::new(object, 0, 1, 1, 0).unwrap_err();
        assert_eq!(empty, SliceError::Empty);
        assert_eq!(stride, SliceError::NonPositiveStride);
        assert_eq!(width, SliceError::NonPositiveWidth);
        assert_eq!(
            empty.to_string(),
            "an alias slice must contain at least one byte"
        );
        assert_eq!(stride.to_string(), "an alias stride must be positive");
        assert_eq!(width.to_string(), "an alias element width must be positive");
    }

    #[test]
    fn whole_and_bounded_whole_provenance_are_shift_fixed_points() {
        let whole = Provenance::one(object(MemoryKind::Global));
        assert_eq!(whole.shifted(8), whole);

        let bounded_object = MemoryObject {
            extent: Some(4),
            ..object(MemoryKind::Frame)
        };
        let bounded =
            Provenance::one_with_slice(bounded_object, 0, 4, 1, 1, BTreeSet::new()).unwrap();
        assert_eq!(bounded.shifted(8), bounded);
    }

    #[test]
    fn union_is_order_independent_and_iterates_canonically() {
        let left =
            Provenance::one_with_slice(object(MemoryKind::Global), 0, 1, 1, 1, BTreeSet::new())
                .unwrap();
        let right =
            Provenance::one_with_slice(object(MemoryKind::External), 4, 5, 1, 1, BTreeSet::new())
                .unwrap();

        let forward = left.union(&right);
        let reverse = right.union(&left);
        assert_eq!(forward, reverse);
        assert_eq!(
            forward.slices.iter().collect::<Vec<_>>(),
            reverse.slices.iter().collect::<Vec<_>>(),
        );
    }
}
