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
/// ints, strings, spaces, HIR storage classes, symbols, values and tuples
/// of them.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Identity {
    Int(i64),
    Str(String),
    Space(Space),
    Storage(crate::hir::model::Storage),
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
            Identity::Storage(one) => one.repr(),
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

pub fn gcd(mut one: i64, mut other: i64) -> i64 {
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

/// Python `qbopt.model.memory:AliasClass`: all `objects_may_alias` asks of
/// an object besides its identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AliasClass {
    pub addressed: bool,
    pub kind: MemoryKind,
    pub captured: bool,
}

#[cfg(any(test, feature = "testing"))]
thread_local! {
    /// `objects_may_alias` questions, for the test that pins how often
    /// picking a write's buckets asks them.
    pub static OBJECT_ALIASES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub fn alias_class(one: &MemoryObject) -> AliasClass {
    AliasClass { addressed: one.addressed, kind: one.kind, captured: one.captured }
}

/// Direct port of `qbopt.model.memory:objects_may_alias`.
pub fn objects_may_alias(one: &MemoryObject, other: &MemoryObject) -> bool {
    #[cfg(any(test, feature = "testing"))]
    OBJECT_ALIASES.with(|asked| asked.set(asked.get() + 1));
    if one == other {
        return true;
    }
    // classes_may_alias's first rule, asked before building either class.
    one.addressed && other.addressed && classes_may_alias(alias_class(one), alias_class(other))
}

/// Whether two distinct objects of these classes may alias.
pub fn classes_may_alias(one: AliasClass, other: AliasClass) -> bool {
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
