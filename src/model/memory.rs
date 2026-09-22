//! Machine-independent memory objects and byte-accurate access paths.
//!
//! Direct port of `qbopt/model/memory.py`: `Kind`, `Object`, `Slice`,
//! `Provenance`, and `objects_may_alias`.  Slices are half-open; a stride
//! greater than one describes selected byte lanes, not merely their hull.

use std::collections::BTreeSet;
use std::fmt;

use crate::model::mir::{Symbol, Value};
use crate::old::object::omf::module::Space;

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

impl fmt::Display for MemoryKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The spelling values used by Python's `Space` and source-storage enums.
///
/// `External` deliberately has one identity here: both Python
/// `Space.EXTERNAL` and `Storage.EXTERNAL` spell the same string value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ObjectTag {
    Seg,
    External,
    Bp,
    Abs,
    Grp,
    Far,
    Sp,
    Local,
    Parameter,
    Static,
    Module,
    Common,
}

impl ObjectTag {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Seg => "seg",
            Self::External => "external",
            Self::Bp => "bp",
            Self::Abs => "abs",
            Self::Grp => "grp",
            Self::Far => "far",
            Self::Sp => "sp",
            Self::Local => "local",
            Self::Parameter => "parameter",
            Self::Static => "static",
            Self::Module => "module",
            Self::Common => "common",
        }
    }
}

impl fmt::Display for ObjectTag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl From<Space> for ObjectTag {
    fn from(space: Space) -> Self {
        match space {
            Space::Segment => Self::Seg,
            Space::External => Self::External,
            Space::Frame => Self::Bp,
            Space::Literal => Self::Abs,
            Space::Group => Self::Grp,
            Space::Far => Self::Far,
            Space::Stack => Self::Sp,
        }
    }
}

/// Closed identities discovered at the Python model's production sites.
///
/// This replaces Python's untyped `Object.identity`.  It is deliberately a
/// value identity only; no target register, instruction encoding, or frontend
/// implementation detail belongs to an object.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ObjectIdentity {
    Parameter {
        index: u32,
    },
    Named {
        selector: u32,
    },
    Frame {
        owner: Option<u32>,
        low: i64,
        high: i64,
    },
    TaggedIndex {
        tag: ObjectTag,
        index: u32,
    },
    CallAllocation {
        callee: String,
        site: u32,
    },
    FloatConversion {
        instruction: u32,
    },
    FloatResult {
        owner: u32,
    },
    DescriptorAllocation {
        descriptor: Symbol,
        generation: u32,
        root: Value,
    },
}

impl ObjectIdentity {
    /// The canonical identity of a hidden floating-point result, whether its
    /// owner originated as a function ID or an operation ID.
    pub const fn float_result(owner: u32) -> Self {
        Self::FloatResult { owner }
    }
}

/// Python `qbopt.model.memory:Object`.
///
/// `extent` is a nonnegative byte size.  Slice bounds remain signed because
/// they can be frame-relative; `Provenance::shifted` performs the one exact
/// `u32`-to-`i64` comparison required for a bounded whole object.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryObject {
    pub kind: MemoryKind,
    pub identity: Option<ObjectIdentity>,
    pub generation: u32,
    pub extent: Option<u32>,
}

impl MemoryObject {
    pub const fn new(kind: MemoryKind) -> Self {
        Self {
            kind,
            identity: None,
            generation: 0,
            extent: None,
        }
    }
}

/// A restricted pointer root.  Unlike Python's historical `object` field,
/// every production root has one closed, structural representation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RestrictRoot {
    Frame { displacement: i64 },
    Global { symbol: u32 },
    Far { selector: u32 },
    Node { node: u32 },
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
        if mine_low >= theirs_high + theirs_width - 1 || theirs_low >= mine_high + mine_width - 1 {
            return false;
        }
        // Byte offsets have a common origin only for the same concrete object.
        if self.object != other.object {
            return true;
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

fn gcd(mut one: i64, mut other: i64) -> i64 {
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
    pub restrict: BTreeSet<RestrictRoot>,
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
        restrict: BTreeSet<RestrictRoot>,
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
                    .is_some_and(|extent| one.low == 0 && one.high == i64::from(extent));
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

/// Direct port of `qbopt.model.memory:objects_may_alias`.
pub fn objects_may_alias(one: &MemoryObject, other: &MemoryObject) -> bool {
    if one.kind == MemoryKind::Unknown || other.kind == MemoryKind::Unknown {
        return true;
    }
    if one == other {
        return true;
    }
    if one.kind == MemoryKind::Nonlocal || other.kind == MemoryKind::Nonlocal {
        return one.kind != MemoryKind::Frame
            && other.kind != MemoryKind::Frame
            && one.kind != MemoryKind::Stack
            && other.kind != MemoryKind::Stack;
    }
    if one.kind == MemoryKind::Parameter || other.kind == MemoryKind::Parameter {
        return one.kind != MemoryKind::Frame && other.kind != MemoryKind::Frame;
    }
    if matches!(one.kind, MemoryKind::Stack | MemoryKind::Frame)
        && matches!(other.kind, MemoryKind::Stack | MemoryKind::Frame)
    {
        return one.kind == MemoryKind::Stack || other.kind == MemoryKind::Stack;
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
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use super::{
        MemoryKind, MemoryObject, ObjectIdentity, ObjectTag, Provenance, RestrictRoot, Slice,
        SliceError, Space,
    };

    fn object(kind: MemoryKind) -> MemoryObject {
        MemoryObject::new(kind)
    }

    #[test]
    fn memory_kind_and_space_tags_keep_their_python_spellings() {
        assert_eq!(MemoryKind::Nonlocal.to_string(), "nonlocal");
        assert_eq!(
            [
                Space::Segment,
                Space::External,
                Space::Frame,
                Space::Literal,
                Space::Group,
                Space::Far,
                Space::Stack,
            ]
            .map(ObjectTag::from),
            [
                ObjectTag::Seg,
                ObjectTag::External,
                ObjectTag::Bp,
                ObjectTag::Abs,
                ObjectTag::Grp,
                ObjectTag::Far,
                ObjectTag::Sp,
            ],
        );
    }

    #[test]
    fn object_kind_rules_keep_stack_and_global_separate() {
        assert!(!super::objects_may_alias(
            &object(MemoryKind::Stack),
            &object(MemoryKind::Global)
        ));
        assert!(super::objects_may_alias(
            &object(MemoryKind::Stack),
            &object(MemoryKind::Frame)
        ));
    }

    #[test]
    fn provenance_subobjects_use_object_identity_and_byte_ranges() {
        let first = MemoryObject {
            kind: MemoryKind::Frame,
            identity: Some(ObjectIdentity::Frame {
                owner: Some(0),
                low: -8,
                high: 0,
            }),
            generation: 0,
            extent: Some(8),
        };
        let second = MemoryObject {
            identity: Some(ObjectIdentity::Frame {
                owner: Some(0),
                low: -16,
                high: -8,
            }),
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
            identity: Some(ObjectIdentity::TaggedIndex {
                tag: ObjectTag::Seg,
                index: 4,
            }),
            generation: 0,
            extent: Some(64),
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
            identity: Some(ObjectIdentity::CallAllocation {
                callee: "allocation".to_owned(),
                site: 1,
            }),
            generation: 0,
            extent: Some(12),
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
            BTreeSet::from([RestrictRoot::Node { node: 1 }]),
        )
        .unwrap();
        let right = Provenance::one_with_slice(
            unknown,
            super::WHOLE_LOW,
            super::WHOLE_HIGH,
            1,
            1,
            BTreeSet::from([RestrictRoot::Node { node: 2 }]),
        )
        .unwrap();

        assert!(!left.intersects(&right));
    }

    #[test]
    fn external_tag_and_float_result_identity_are_shared() {
        let from_space = ObjectIdentity::TaggedIndex {
            tag: ObjectTag::from(Space::External),
            index: 3,
        };
        // The future HIR Storage conversion must map its "external" spelling
        // to this same tag; no second storage enum belongs in this layer.
        let from_storage_spelling = ObjectIdentity::TaggedIndex {
            tag: ObjectTag::External,
            index: 3,
        };
        assert_eq!(from_space, from_storage_spelling);
        let mut left = DefaultHasher::new();
        let mut right = DefaultHasher::new();
        from_space.hash(&mut left);
        from_storage_spelling.hash(&mut right);
        assert_eq!(left.finish(), right.finish());

        let function_result = ObjectIdentity::float_result(9);
        let operation_result = ObjectIdentity::FloatResult { owner: 9 };
        assert_eq!(function_result, operation_result);
        let mut function_hash = DefaultHasher::new();
        let mut operation_hash = DefaultHasher::new();
        function_result.hash(&mut function_hash);
        operation_result.hash(&mut operation_hash);
        assert_eq!(function_hash.finish(), operation_hash.finish());
    }

    #[test]
    fn production_identity_shapes_are_closed_and_structurally_distinct() {
        // Every shape used by the 28 production `memory.Object(...)` sites.
        let identities = BTreeSet::from([
            ObjectIdentity::Parameter { index: 1 },
            ObjectIdentity::Named { selector: 2 },
            ObjectIdentity::Frame {
                owner: None,
                low: -8,
                high: -4,
            },
            ObjectIdentity::Frame {
                owner: Some(3),
                low: -8,
                high: -4,
            },
            ObjectIdentity::TaggedIndex {
                tag: ObjectTag::Module,
                index: 4,
            },
            ObjectIdentity::CallAllocation {
                callee: "malloc".to_owned(),
                site: 5,
            },
            ObjectIdentity::FloatConversion { instruction: 6 },
            ObjectIdentity::FloatResult { owner: 7 },
            ObjectIdentity::DescriptorAllocation {
                descriptor: super::Symbol::new(Space::Segment, 8, 10, 2),
                generation: 9,
                root: super::Value::new(10, 11),
            },
        ]);
        assert_eq!(identities.len(), 9);

        assert_eq!(
            BTreeSet::from([
                RestrictRoot::Frame { displacement: -2 },
                RestrictRoot::Global { symbol: 1 },
                RestrictRoot::Far { selector: 2 },
                RestrictRoot::Node { node: 3 },
            ])
            .len(),
            4
        );
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
