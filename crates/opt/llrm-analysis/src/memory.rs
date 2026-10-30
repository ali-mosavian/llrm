//! The memory model every alias query reads: objects, byte slices, provenance,
//! accesses (`MemRef`) and the `Unit` that carries a function's facts. It gives
//! every analysis one description of what an access touches, so none re-derives
//! it from the pointer.
//!
//! # How it works
//! An object is what LLVM calls an underlying object (`MemoryObject`, interned
//! per module as `ObjectRef`). Its `MemoryKind` is one of:
//!
//! - `Frame`: an `alloca`.
//! - `Global`: a global variable.
//! - `Parameter`: what a pointer parameter points to.
//! - `Unknown`: a pointer no fact follows.
//! - `Nonlocal`: what a callee may reach.
//! - `Absolute`: linear memory (`regions::linear`).
//! - `Stack`, `External`, `Named`, `Allocation`: kept for `classes_may_alias`;
//!   the rich MIR has no producer for them (an allocation is the `!tbaa` type
//!   "allocation" on the access).
//!
//! Besides its identity an object carries three facts that equality ignores:
//! `addressed` (some code computes its address), `captured` (that address can
//! be found from outside this activation) and `constant` (nothing writes it).
//! `objects_may_alias` asks only the `AliasClass` (kind, addressed, captured):
//!
//! ```text
//!   an unaddressed object            -> reached only by its own name
//!   Unknown vs x                     -> x.captured
//!   Nonlocal vs x                    -> x.captured and x is not Frame/Stack
//!   Parameter vs x                   -> x.captured and x is not Frame
//!   Global/External vs Global/External -> only if one is External
//!   anything else (distinct)         -> no
//! ```
//!
//! A `Slice` is bytes of one object: `[low, high)` with `stride` and `width`,
//! so a stride above one selects byte lanes, not their hull. `WHOLE_LOW..
//! WHOLE_HIGH` is "any byte". `Slice::intersects` is exact for one object (gcd
//! of the strides) and, for two distinct objects that may alias, true. A
//! `Provenance` is a set of slices plus the `restrict` roots (`noalias`
//! parameters) a pointer is based on; two with disjoint nonempty restrict sets
//! do not intersect.
//!
//! `MemRef` is an access, LLVM's `MemoryLocation`, with its pointer decomposed
//! as BasicAA's `DecomposeGEPExpression` does: a `root`, a constant `disp`
//! wrapped to the index width, and at most one variable index (`base`,
//! `scale`), plus the width, `!tbaa` type and its ancestors (`lineage`), and
//! `volatile`. `MemRef::of` builds it from a load or store, `filled` from a
//! constant-length `memset`, `reach` from provenance alone (what a call's
//! effect names). `named` says the bytes are fixed outright; `unwritable` that
//! every object it may reach is `constant`.
//!
//! `Unit` is what the analyses read: the function, its module's types,
//! constants, layout, metadata and globals, the program's target and segment
//! layout, and optional facts handed in with `with_*` (shape, registers,
//! pointers, annotated references, assumptions, counted loops, edges,
//! exposed frames, GlobalsAA, callbacks). A fact not handed in is derived on
//! demand, which is correct but repeats work; [`crate::manager`]'s
//! `Unit::within` fills them from cached analyses. `Standing` holds the
//! registers for a caller that edits the body as it goes. `constant_bits` and
//! `constant_bytes` read a `constant` global's initializer. `own_bytes` is the
//! one statement of what a load or store does to its bytes, and
//! `unmodeled_write` says whether any other instruction may write.
//!
//! # Example
//! `MemRef::of` on the two loads below, under the DOS data layout (address
//! space 1 is far pointers):
//!
//! ```mir
//! @g = global [8 x i16] zeroinitializer
//!
//! define i16 @f(i16 %sel, i16 %i) {
//! b0:
//!   %s = inttoptr i16 %sel to ptr addrspace(2)
//!   %far = addrspacecast ptr addrspace(2) %s to ptr addrspace(1)
//!   %e = getelementptr inbounds i16, ptr addrspace(1) %far, i16 %i
//!   %f = getelementptr inbounds i8, ptr addrspace(1) %e, i16 -2
//!   %x = load i16, ptr addrspace(1) %f
//!   %y = load i16, ptr getelementptr (i8, ptr @g, i16 6)
//!   %z = add i16 %x, %y
//!   ret i16 %z
//! }
//! ```
//!
//! | `MemRef` of | `%x` load | `%y` load |
//! |------------|-----------|-----------|
//! | `root`     | `%far`    | -         |
//! | `disp`     | -2        | 6         |
//! | `base`     | `%i`      | none      |
//! | `scale`    | 2         | -         |
//! | `segment`  | `%sel`    | -         |
//! | `width`    | -         | 2         |
//! | `object`   | false     | true      |
//! | `addr()`   | none      | some      |
//!
//! From unit test
//! `a_far_element_access_decomposes_to_its_segment_index_and_displacement`
//! in `memory.rs`.
//!
//! # Provides
//! The types above; `exposed_frames`, `object_of`, `global_object`, `typed`,
//! `lineage`, `is_lifetime_marker`. Consumers: nearly every analysis in this
//! crate ([`crate::alias`], [`crate::memoryssa`], [`crate::regions`],
//! [`crate::avail`], [`crate::consts`], [`crate::effects`]) and the memory
//! passes of llrm-transforms.
//!
//! # See also
//! LLVM BasicAA `DecomposeGEPExpression` and `MemoryLocation`
//! (`llvm/lib/Analysis/BasicAliasAnalysis.cpp`).

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt;

use llrm_mir::context::{ConstantExpr, ConstantKind, Context, GlobalId, signed};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{
    Function, GlobalKind, GlobalValue, InstId, MetadataNode, MetadataOperand, Module, Operand, ValueDef, ValueId,
};
use llrm_mir::opcode::{CastOp, Flags, Opcode};
use llrm_mir::program::ProgramProxy;
use llrm_mir::types::{Type, TypeId};
use llrm_support::hash::IndexMap;

use crate::alias::PointsTo;
use crate::assumptions::Assumptions;
use crate::cfg::Shape;
use crate::consts::Known;
use crate::globalsaa::Globals;

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
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// What tells two objects of one kind apart: a parameter's position, an
/// alloca's value, a global's id, names, and tuples of them.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Identity {
    Int(i64),
    Str(String),
    Global(u32),
    Value(u32),
    Tuple(Vec<Identity>),
}

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
    /// A `constant` global: writing it is undefined, so nothing changes it,
    /// wherever its address went.
    pub constant: bool,
}

impl MemoryObject {
    pub const fn new(kind: MemoryKind) -> Self {
        Self { kind, identity: None, generation: 0, extent: None, addressed: true, captured: true, constant: false }
    }

    fn key(&self) -> (MemoryKind, &Option<Identity>, i64, Option<i64>) {
        (self.kind, &self.identity, self.generation, self.extent)
    }
}

impl PartialEq for MemoryObject {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.key() == other.key()
    }
}

impl Eq for MemoryObject {}

impl std::hash::Hash for MemoryObject {
    fn hash<H: std::hash::Hasher>(
        &self,
        state: &mut H,
    ) {
        self.key().hash(state);
    }
}

impl PartialOrd for MemoryObject {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
thread_local! {
    /// Structural comparisons of objects, for the test that sets of slices make
    /// none.
    pub static OBJECT_COMPARES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
thread_local! {
    /// Reads of an access's `!tbaa` name and lineage, for the test that a held
    /// access is not asked its type again.
    pub static TYPE_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl Ord for MemoryObject {
    fn cmp(
        &self,
        other: &Self,
    ) -> std::cmp::Ordering {
        #[cfg(test)]
        OBJECT_COMPARES.with(|count| count.set(count.get() + 1));
        self.key().cmp(&other.key())
    }
}

/// A `MemoryObject` as its module's interner numbered it: a small id compared,
/// hashed and ordered as an integer, with the fields the passes ask of every
/// slice beside it, so a slice is `Copy` and asking costs nothing. The rest
/// (the identity, the generation) is `ObjectInterner::object`. Interned by the
/// object's identity (kind, identity, generation, extent) alone: the same
/// object spelled with other facts (a frame slot whose address an edit
/// stopped exposing) is the same number, so a result kept past the edit and
/// one derived after it name it alike. The facts (`addressed`, `captured`,
/// `constant`) are what the body that made the reference had of the object,
/// read through the accessors; they are not its identity. The order is the
/// order of first interning in the module, which the same module always makes
/// the same way; nothing may depend on it being `MemoryObject`'s.
#[derive(Clone, Copy, Debug)]
pub struct ObjectRef {
    id: u32,
    pub kind: MemoryKind,
    addressed: bool,
    captured: bool,
    constant: bool,
    pub extent: Option<i64>,
    /// The identity where it is a number, which is all the passes make: a name
    /// or a tuple (a test's, the linear region's) is `Other`, and
    /// `ObjectInterner::object` has it whole.
    pub key: Key,
}

/// `Identity`, small enough to be copied beside the id.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    None,
    Int(i64),
    Global(u32),
    Value(u32),
    Other,
}

impl Key {
    fn of(identity: &Option<Identity>) -> Self {
        match identity {
            None => Self::None,
            Some(Identity::Int(number)) => Self::Int(*number),
            Some(Identity::Global(number)) => Self::Global(*number),
            Some(Identity::Value(number)) => Self::Value(*number),
            Some(Identity::Str(_) | Identity::Tuple(_)) => Self::Other,
        }
    }
}

impl ObjectRef {
    /// Every interner starts with these three, so they are the same in all
    /// modules and in the statics.
    pub const UNKNOWN: Self = Self {
        id: 0,
        kind: MemoryKind::Unknown,
        addressed: true,
        captured: true,
        constant: false,
        extent: None,
        key: Key::None,
    };
    /// Memory addressed linearly, which no program object occupies (`regions`).
    pub const LINEAR: Self = Self {
        id: 2,
        kind: MemoryKind::Absolute,
        addressed: true,
        captured: true,
        constant: false,
        extent: None,
        key: Key::Other,
    };
    pub const NONLOCAL: Self = Self {
        id: 1,
        kind: MemoryKind::Nonlocal,
        addressed: true,
        captured: true,
        constant: false,
        extent: None,
        key: Key::None,
    };

    /// The number this object was given: the order it was first interned in.
    pub fn id(self) -> u32 {
        self.id
    }

    /// Some code computes its address, so a pointer of unknown origin may hold
    /// it: as the state of the body that interned this reference had it.
    pub fn addressed(self) -> bool {
        self.addressed
    }

    /// Its address can be found from outside this activation.
    pub fn captured(self) -> bool {
        self.captured
    }

    /// Writing it is undefined.
    pub fn constant(self) -> bool {
        self.constant
    }
}

impl PartialEq for ObjectRef {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.id == other.id
    }
}

impl Eq for ObjectRef {}

impl std::hash::Hash for ObjectRef {
    fn hash<H: std::hash::Hasher>(
        &self,
        state: &mut H,
    ) {
        self.id.hash(state);
    }
}

impl PartialOrd for ObjectRef {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ObjectRef {
    fn cmp(
        &self,
        other: &Self,
    ) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

struct Exact(MemoryObject);

impl PartialEq for Exact {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.0 == other.0
    }
}

impl Eq for Exact {}

impl std::hash::Hash for Exact {
    fn hash<H: std::hash::Hasher>(
        &self,
        state: &mut H,
    ) {
        self.0.hash(state);
    }
}

/// A module's memory objects, numbered densely in the order they are first
/// asked for. It lives in the module's `Context` (`ObjectInterner::of`) and is
/// dropped with it, as LLVMContext's uniqued constants are.
pub struct ObjectInterner {
    held: std::cell::RefCell<(Vec<MemoryObject>, llrm_support::hash::HashMap<Exact, u32>)>,
    lookups: std::cell::Cell<u64>,
    /// Where `LLRM_CHECK_INTERN` numbers objects by `scrambled`: the place of
    /// each number in `held`.
    places: std::cell::RefCell<llrm_support::hash::HashMap<u32, u32>>,
}

/// `LLRM_CHECK_INTERN=SEED` (any other value is seed 1): the numbers objects
/// are given are a seeded permutation of the order they were first asked for
/// in, so that a result that depends on the order analyses ran in (and
/// interned in) differs from the run without it. The three objects every
/// interner starts with keep their numbers.
fn interning_seed() -> Option<u32> {
    static SEED: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();
    if !llrm_support::env_flag!("LLRM_CHECK_INTERN") {
        return None;
    }
    *SEED.get_or_init(|| {
        let value = std::env::var("LLRM_CHECK_INTERN").ok();
        Some(value.and_then(|value| value.parse().ok()).unwrap_or(1))
    })
}

/// The number the `place`th object is given under `seed`: a bijection of the
/// places onto 31 bits (an odd multiplier and an xor, both mod 2^31), above
/// the numbers the constants hold.
fn scrambled(
    place: u32,
    seed: u32,
) -> u32 {
    const MASK: u32 = 0x7fff_ffff;
    let odd = (seed.wrapping_mul(0x9e37_79b1) | 1) & MASK;
    0x8000_0000 | ((place.wrapping_mul(odd) ^ seed.wrapping_mul(0x85eb_ca6b)) & MASK)
}

impl Default for ObjectInterner {
    fn default() -> Self {
        let interner = Self { held: Default::default(), lookups: Default::default(), places: Default::default() };
        assert_eq!(interner.intern(MemoryObject::new(MemoryKind::Unknown)), ObjectRef::UNKNOWN);
        assert_eq!(interner.intern(MemoryObject::new(MemoryKind::Nonlocal)), ObjectRef::NONLOCAL);
        assert_eq!(
            interner.intern(MemoryObject {
                identity: Some(Identity::Str("linear".to_owned())),
                ..MemoryObject::new(MemoryKind::Absolute)
            }),
            ObjectRef::LINEAR
        );
        interner
    }
}

impl ObjectInterner {
    /// The interner of `context`'s module.
    pub fn of(context: &Context) -> std::rc::Rc<Self> {
        context.extension::<Self>()
    }

    pub fn intern(
        &self,
        object: MemoryObject,
    ) -> ObjectRef {
        self.lookups.set(self.lookups.get() + 1);
        let (addressed, captured, constant) = (object.addressed, object.captured, object.constant);
        let mut held = self.held.borrow_mut();
        let (objects, ids) = &mut *held;
        let key = Exact(object);
        let place = match ids.get(&key) {
            Some(&place) => place,
            None => {
                let place = objects.len() as u32;
                objects.push(key.0.clone());
                ids.insert(key, place);
                if let Some(seed) = interning_seed().filter(|_| place >= 3) {
                    self.places.borrow_mut().insert(scrambled(place, seed), place);
                }
                place
            }
        };
        let one = &objects[place as usize];
        let id = interning_seed().filter(|_| place >= 3).map_or(place, |seed| scrambled(place, seed));
        ObjectRef { id, kind: one.kind, addressed, captured, constant, extent: one.extent, key: Key::of(&one.identity) }
    }

    /// How many times `intern` was asked: a cost counter, for tests that a hot
    /// loop does not ask per step.
    pub fn lookups(&self) -> u64 {
        self.lookups.get()
    }

    /// The object `one` stands for.
    pub fn object(
        &self,
        one: ObjectRef,
    ) -> MemoryObject {
        MemoryObject {
            addressed: one.addressed,
            captured: one.captured,
            constant: one.constant,
            ..self.held.borrow().0[self.places.borrow().get(&one.id).map_or(one.id, |&place| place) as usize].clone()
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The interner `ObjectRef::from` uses in tests that have no module.
    static TEST_OBJECTS: ObjectInterner = ObjectInterner::default();
}

#[cfg(test)]
impl From<MemoryObject> for ObjectRef {
    fn from(object: MemoryObject) -> Self {
        TEST_OBJECTS.with(|interner| interner.intern(object))
    }
}

pub const WHOLE_LOW: i64 = -(1_i64 << 31);
pub const WHOLE_HIGH: i64 = 1_i64 << 31;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Slice {
    pub object: ObjectRef,
    pub low: i64,
    pub high: i64,
    pub stride: i64,
    pub width: i64,
}

/// Python raises `ValueError` for all three invalid `Slice` shapes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SliceError {
    Empty,
    NonPositiveStride,
    NonPositiveWidth,
}

impl fmt::Display for SliceError {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "an alias slice must contain at least one byte",
            Self::NonPositiveStride => "an alias stride must be positive",
            Self::NonPositiveWidth => "an alias element width must be positive",
        })
    }
}

impl std::error::Error for SliceError {}

impl Slice {
    pub fn new(
        object: impl Into<ObjectRef>,
        low: i64,
        high: i64,
        stride: i64,
        width: i64,
    ) -> Result<Self, SliceError> {
        let object = object.into();
        if high <= low {
            return Err(SliceError::Empty);
        }
        if stride <= 0 {
            return Err(SliceError::NonPositiveStride);
        }
        if width <= 0 {
            return Err(SliceError::NonPositiveWidth);
        }
        Ok(Self { object, low, high, stride, width })
    }

    pub fn whole(object: impl Into<ObjectRef>) -> Self {
        Self::new(object, WHOLE_LOW, WHOLE_HIGH, 1, 1).expect("the fixed whole-object slice is valid")
    }

    /// Every byte of `object`; none of a zero-byte one, which overlaps
    /// nothing, as in LLVM.
    pub fn every_byte(object: impl Into<ObjectRef>) -> Option<Self> {
        let object = object.into();
        match object.extent {
            Some(extent) => Self::new(object, 0, extent, 1, 1).ok(),
            None => Some(Self::whole(object)),
        }
    }

    pub fn shifted(
        &self,
        amount: i64,
    ) -> Self {
        Self::new(self.object, self.low + amount, self.high + amount, self.stride, self.width)
            .expect("shifting a valid slice retains its positive shape")
    }

    /// Whether every byte `other` names is one `self` names. A pointer at or
    /// past the end of the object names no byte of it, so it lies within any
    /// slice of the object.
    pub fn contains(
        &self,
        other: &Self,
    ) -> bool {
        if self.object != other.object {
            return false;
        }
        if self.object.extent.is_some_and(|extent| other.low >= extent) {
            return true;
        }
        let ends = (self.high - 1 + self.width, other.high - 1 + other.width);
        if self.stride == other.stride && self.width >= other.width {
            return other.low >= self.low && other.high <= self.high && (other.low - self.low) % self.stride == 0;
        }
        self.stride <= self.width && self.low <= other.low && ends.1 <= ends.0
    }

    pub fn intersects(
        &self,
        other: &Self,
    ) -> bool {
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

pub fn gcd(
    mut one: i64,
    mut other: i64,
) -> i64 {
    while other != 0 {
        (one, other) = (other, one % other);
    }
    one
}

/// A set kept as one sorted, shared array: iterates in `T`'s order as a
/// `BTreeSet` does, but a copy is a count bumped, a union is a merge, and the
/// elements sit together. Empty holds no allocation.
#[derive(Debug)]
pub struct Sorted<T>(Option<std::sync::Arc<[T]>>);

impl<T> Clone for Sorted<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Default for Sorted<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: PartialEq> PartialEq for Sorted<T> {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        match (&self.0, &other.0) {
            (Some(one), Some(two)) => std::sync::Arc::ptr_eq(one, two) || one[..] == two[..],
            _ => self.is_empty() && other.is_empty(),
        }
    }
}

impl<T: Eq> Eq for Sorted<T> {}

impl<T: std::hash::Hash> std::hash::Hash for Sorted<T> {
    fn hash<H: std::hash::Hasher>(
        &self,
        state: &mut H,
    ) {
        self.as_slice().hash(state);
    }
}

impl<T: PartialOrd> PartialOrd for Sorted<T> {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<std::cmp::Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}

impl<T: Ord> Ord for Sorted<T> {
    fn cmp(
        &self,
        other: &Self,
    ) -> std::cmp::Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl<T> Sorted<T> {
    pub const fn new() -> Self {
        Self(None)
    }

    pub fn as_slice(&self) -> &[T] {
        self.0.as_deref().unwrap_or(&[])
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.as_slice().iter()
    }

    pub fn len(&self) -> usize {
        self.as_slice().len()
    }

    pub fn is_empty(&self) -> bool {
        self.as_slice().is_empty()
    }

    pub fn first(&self) -> Option<&T> {
        self.as_slice().first()
    }
}

impl<T: Ord + Clone> Sorted<T> {
    pub fn contains(
        &self,
        one: &T,
    ) -> bool {
        self.as_slice().binary_search(one).is_ok()
    }

    /// Whether every element of `self` is in `other`.
    pub fn is_subset(
        &self,
        other: &Self,
    ) -> bool {
        let mut rest = other.as_slice();
        self.iter().all(|one| match rest.binary_search(one) {
            Ok(at) => {
                rest = &rest[at + 1..];
                true
            }
            Err(_) => false,
        })
    }

    pub fn is_disjoint(
        &self,
        other: &Self,
    ) -> bool {
        let (mut one, mut two) = (self.as_slice(), other.as_slice());
        while let (Some(a), Some(b)) = (one.first(), two.first()) {
            match a.cmp(b) {
                std::cmp::Ordering::Less => one = &one[1..],
                std::cmp::Ordering::Greater => two = &two[1..],
                std::cmp::Ordering::Equal => return false,
            }
        }
        true
    }

    /// The elements of both; the one array shared where one holds the other.
    pub fn union(
        &self,
        other: &Self,
    ) -> Self {
        if other.is_subset(self) {
            return self.clone();
        }
        if self.is_subset(other) {
            return other.clone();
        }
        let (mut one, mut two) = (self.as_slice(), other.as_slice());
        let mut made = Vec::with_capacity(one.len() + two.len());
        while let (Some(a), Some(b)) = (one.first(), two.first()) {
            match a.cmp(b) {
                std::cmp::Ordering::Less => {
                    made.push(a.clone());
                    one = &one[1..];
                }
                std::cmp::Ordering::Greater => {
                    made.push(b.clone());
                    two = &two[1..];
                }
                std::cmp::Ordering::Equal => {
                    made.push(a.clone());
                    one = &one[1..];
                    two = &two[1..];
                }
            }
        }
        made.extend_from_slice(one);
        made.extend_from_slice(two);
        Self(Some(made.into()))
    }

    pub fn to_set(&self) -> BTreeSet<T> {
        self.iter().cloned().collect()
    }
}

impl<T: Ord + Clone> FromIterator<T> for Sorted<T> {
    fn from_iter<I: IntoIterator<Item = T>>(items: I) -> Self {
        let mut made: Vec<T> = items.into_iter().collect();
        made.sort();
        made.dedup();
        if made.is_empty() { Self(None) } else { Self(Some(made.into())) }
    }
}

impl<T: Ord + Clone, const N: usize> From<[T; N]> for Sorted<T> {
    fn from(items: [T; N]) -> Self {
        items.into_iter().collect()
    }
}

impl<T: Ord + Clone> From<BTreeSet<T>> for Sorted<T> {
    fn from(items: BTreeSet<T>) -> Self {
        items.into_iter().collect()
    }
}

impl<'a, T> IntoIterator for &'a Sorted<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T: Clone> IntoIterator for Sorted<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().to_vec().into_iter()
    }
}

/// `restrict` holds the `noalias`
/// parameters a pointer is based on.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Provenance {
    pub slices: Sorted<Slice>,
    pub restrict: Sorted<Identity>,
}

impl Provenance {
    /// `Provenance.one` with the whole-object bounds and no restrict roots.
    pub fn one(object: impl Into<ObjectRef>) -> Self {
        Self { slices: Sorted::from([Slice::whole(object)]), restrict: Sorted::new() }
    }

    pub fn one_with_slice(
        object: impl Into<ObjectRef>,
        low: i64,
        high: i64,
        stride: i64,
        width: i64,
        restrict: BTreeSet<Identity>,
    ) -> Result<Self, SliceError> {
        Ok(Self { slices: Sorted::from([Slice::new(object, low, high, stride, width)?]), restrict: restrict.into() })
    }

    pub fn shifted(
        &self,
        amount: i64,
    ) -> Self {
        let slices = self
            .slices
            .iter()
            .map(|one| {
                let whole = one.low == WHOLE_LOW && one.high == WHOLE_HIGH;
                let bounded_whole = one.object.extent.is_some_and(|extent| one.low == 0 && one.high == extent);
                if whole || bounded_whole { one.clone() } else { one.shifted(amount) }
            })
            .collect();
        Self { slices, restrict: self.restrict.clone() }
    }

    pub fn union(
        &self,
        other: &Self,
    ) -> Self {
        Self { slices: self.slices.union(&other.slices), restrict: self.restrict.union(&other.restrict) }
    }

    /// Whether every slice of `other` lies within one of `self`'s: a kept
    /// answer covers a fresh one that knows more. Restrict roots are ignored;
    /// it asks only where a pointer may point.
    pub fn covers(
        &self,
        other: &Self,
    ) -> bool {
        other.slices.iter().all(|two| self.slices.iter().any(|one| one.contains(two)))
    }

    pub fn intersects(
        &self,
        other: &Self,
    ) -> bool {
        if !self.restrict.is_empty() && !other.restrict.is_empty() && self.restrict.is_disjoint(&other.restrict) {
            return false;
        }
        self.slices.iter().any(|one| other.slices.iter().any(|two| one.intersects(two)))
    }
}

/// All `objects_may_alias` asks of
/// an object besides its identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AliasClass {
    pub addressed: bool,
    pub kind: MemoryKind,
    pub captured: bool,
}

#[cfg(test)]
thread_local! {
    /// `objects_may_alias` questions, for the test that pins how often
    /// picking a write's buckets asks them.
    pub static OBJECT_ALIASES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// What `objects_may_alias` asks of an object: a `MemoryObject` or its interned
/// `ObjectRef`.
pub trait Aliasable: PartialEq {
    fn class(&self) -> AliasClass;
}

impl Aliasable for MemoryObject {
    fn class(&self) -> AliasClass {
        AliasClass { addressed: self.addressed, kind: self.kind, captured: self.captured }
    }
}

impl Aliasable for ObjectRef {
    fn class(&self) -> AliasClass {
        AliasClass { addressed: self.addressed, kind: self.kind, captured: self.captured }
    }
}

pub fn alias_class(one: &impl Aliasable) -> AliasClass {
    one.class()
}

pub fn objects_may_alias<T: Aliasable>(
    one: &T,
    other: &T,
) -> bool {
    #[cfg(test)]
    OBJECT_ALIASES.with(|asked| asked.set(asked.get() + 1));
    if one == other {
        return true;
    }
    // classes_may_alias's first rule, asked before building either class.
    let (one, other) = (one.class(), other.class());
    one.addressed && other.addressed && classes_may_alias(one, other)
}

/// Whether two distinct objects of these classes may alias.
pub fn classes_may_alias(
    one: AliasClass,
    other: AliasClass,
) -> bool {
    // Only a reference naming an unaddressed object reaches it.
    if !(one.addressed && other.addressed) {
        return false;
    }
    for (this, that) in [(one, other), (other, one)] {
        // A pointer no fact follows, an `inttoptr` among them, reaches only
        // what escaped: LLVM's capture tracking and GlobalsAA's
        // non-address-taken globals.
        if this.kind == MemoryKind::Unknown {
            return that.captured;
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

/// What the memory analyses read: a function, and of its module the types
/// and constants, the layout, the metadata (`!tbaa`) and the globals (an
/// object's size, a callee's attributes); and of its program, where the
/// target keeps no program data and where the segments put it.
#[derive(Clone, Copy)]
pub struct Unit<'a> {
    pub program: Option<&'a ProgramProxy>,
    /// The address spaces where no program names its target.
    pub spaces: llrm_mir::spaces::Spaces,
    pub context: &'a Context,
    pub layout: &'a DataLayout,
    pub metadata: &'a [MetadataNode],
    /// The type tree of `metadata`'s `!tbaa` nodes, the module's, built once.
    pub tbaa: Option<&'a llrm_mir::tbaa::Tbaa>,
    pub globals: &'a [GlobalValue],
    pub function: &'a Function,
    /// Each access with the provenance alias found (`alias::annotated`).
    pub references: Option<&'a IndexMap<InstId, MemRef>>,
    /// What GlobalsAA proves of the module's globals; without it every
    /// global is captured.
    pub globals_aa: Option<&'a Globals>,
    /// What `GlobalsModRef` proves of the module's calls and tracked globals,
    /// the manager's `GlobalsModRefAnalysis`; without it a call says nothing
    /// of a global.
    pub globals_mr: Option<&'a crate::globalsmodref::GlobalsModRef>,
    /// The proxy `within` made the unit under, whose memo holds what depends on
    /// its module analyses alone.
    pub outer: Option<&'a llrm_mir::passes::Outer>,
    /// Dominance and loops, the manager's; without them each ask finds them.
    pub shape: Option<&'a Shape>,
    /// What consts knows without memory, the manager's `Registers`.
    pub registers: Option<&'a IndexMap<ValueId, Known>>,
    /// Every pointer's objects, the manager's `Pointers`.
    pub pointers: Option<&'a PointsTo>,
    /// Each access's reference as alias finds it, the manager's
    /// `Annotated`; unlike `references`, the unit does not read through it.
    pub annotated: Option<&'a Result<IndexMap<InstId, MemRef>, String>>,
    /// What each block assumes; without it each ask finds it.
    pub assumptions: Option<&'a Assumptions>,
    /// Each loop's counted proofs under `registers`, the manager's `Counted`;
    /// without them each ask proves them.
    pub counted: Option<&'a crate::induction::Counted>,
    /// Each loop's followers of its counters (`induction::followers`), the
    /// manager's `LoopFollowers`; without them each ask works them out of
    /// the loop.
    pub followers: Option<&'a crate::induction::LoopFollowers>,
    /// What the unavoidable branch edges bound at each block, under
    /// `registers`: the manager's `DominatedEdges`.
    pub edges: Option<&'a crate::ranger::EdgeFacts>,
    /// The allocas whose address is exposed, the manager's `ExposedFrames`;
    /// without it each ask scans the alloca's uses.
    pub exposed: Option<&'a BTreeSet<ValueId>>,
    /// The accesses of the body as `Accesses::plain` makes them under the
    /// calls' footprints, the manager's `PlainAccesses`: every client of one
    /// body state shares them, and the graph they make.
    pub plain: Option<&'a crate::memoryssa::Accesses>,
    /// The pointer value solve of this body, the manager's `PointerValues`;
    /// without it each `points_to` makes its own.
    pub point_values: Option<&'a crate::alias::PointValues>,
    /// The congruences of this body under these registers
    /// (`alias::congruences_of`), where a caller asking many times of one
    /// body made a cell for them: each proof of a loop's trips asked of the
    /// whole function, once for each loop.
    pub congruent: Option<&'a std::cell::RefCell<crate::alias::CongruenceMemo>>,
    /// The proofs of the loops' trips found so far by a call that asks many:
    /// the proof of a loop asks those of the loops around it, each of which
    /// asks those around it, 2^depth proofs of the outermost for a nest.
    pub proved: Option<&'a std::cell::RefCell<crate::induction::Counted>>,
}

impl<'a> Unit<'a> {
    /// The intrinsic `inst` calls, if it calls one.
    pub fn intrinsic(
        &self,
        inst: InstId,
    ) -> Option<llrm_mir::intrinsics::Intrinsic> {
        let callee = llrm_mir::memory::callee(self.context, self.function, inst)?;
        self.globals.get(callee.0 as usize)?.intrinsic()
    }

    /// Whether `inst` calls out: a call no intrinsic's instructions
    /// replace, as LLVM's `isLoweredToCall` says.
    pub fn calls_out(
        &self,
        inst: InstId,
    ) -> bool {
        matches!(
            self.function.instruction(inst).opcode,
            Opcode::Call(_) | Opcode::Invoke(_)
        )
            && self.intrinsic(inst).is_none()
    }

    pub fn of(
        module: &'a Module,
        layout: &'a DataLayout,
        function: &'a Function,
    ) -> Self {
        Self::over(&module.context, layout, &module.metadata, &module.globals, function)
    }

    /// [`Unit::of`] for a caller that holds the module's parts, not the module.
    pub fn over(
        context: &'a Context,
        layout: &'a DataLayout,
        metadata: &'a [MetadataNode],
        globals: &'a [GlobalValue],
        function: &'a Function,
    ) -> Self {
        Self {
            program: None,
            spaces: llrm_mir::spaces::Spaces::FLAT,
            context,
            layout,
            metadata,
            tbaa: None,
            globals,
            function,
            globals_aa: None,
            globals_mr: None,
            outer: None,
            references: None,
            shape: None,
            registers: None,
            pointers: None,
            annotated: None,
            assumptions: None,
            counted: None,
            followers: None,
            edges: None,
            exposed: None,
            plain: None,
            point_values: None,
            congruent: None,
            proved: None,
        }
    }

    pub fn with_spaces(
        self,
        spaces: llrm_mir::spaces::Spaces,
    ) -> Self {
        Self { spaces, ..self }
    }

    pub fn with_point_values(
        self,
        point_values: &'a crate::alias::PointValues,
    ) -> Self {
        Self { point_values: Some(point_values), ..self }
    }

    pub fn with_plain(
        self,
        plain: &'a crate::memoryssa::Accesses,
    ) -> Self {
        Self { plain: Some(plain), ..self }
    }

    pub fn with_exposed(
        self,
        exposed: &'a BTreeSet<ValueId>,
    ) -> Self {
        Self { exposed: Some(exposed), ..self }
    }

    pub fn with_globals_mr(
        self,
        globals_mr: &'a crate::globalsmodref::GlobalsModRef,
    ) -> Self {
        Self { globals_mr: Some(globals_mr), ..self }
    }

    pub fn with_globals_aa(
        self,
        globals_aa: &'a Globals,
    ) -> Self {
        Self { globals_aa: Some(globals_aa), ..self }
    }

    pub fn with_references(
        self,
        references: &'a IndexMap<InstId, MemRef>,
    ) -> Self {
        Self { references: Some(references), ..self }
    }

    pub fn with_shape(
        self,
        shape: &'a Shape,
    ) -> Self {
        Self { shape: Some(shape), ..self }
    }

    pub fn with_assumptions(
        self,
        assumptions: &'a Assumptions,
    ) -> Self {
        Self { assumptions: Some(assumptions), ..self }
    }

    pub fn with_edges(
        self,
        edges: &'a crate::ranger::EdgeFacts,
    ) -> Self {
        Self { edges: Some(edges), ..self }
    }

    pub fn with_counted(
        self,
        counted: &'a crate::induction::Counted,
    ) -> Self {
        Self { counted: Some(counted), ..self }
    }

    pub fn with_followers(
        self,
        followers: &'a crate::induction::LoopFollowers,
    ) -> Self {
        Self { followers: Some(followers), ..self }
    }

    pub fn with_registers(
        self,
        registers: &'a IndexMap<ValueId, Known>,
    ) -> Self {
        Self { registers: Some(registers), ..self }
    }

    pub fn with_pointers(
        self,
        pointers: &'a PointsTo,
    ) -> Self {
        Self { pointers: Some(pointers), ..self }
    }

    pub fn with_annotated(
        self,
        annotated: &'a Result<IndexMap<InstId, MemRef>, String>,
    ) -> Self {
        Self { annotated: Some(annotated), ..self }
    }

    /// Each access's reference as `alias::annotated` finds it: the
    /// manager's where the unit carries it.
    pub fn annotated(&self) -> Result<Cow<'a, IndexMap<InstId, MemRef>>, String> {
        match self.annotated {
            Some(annotated) => annotated.as_ref().map(Cow::Borrowed).map_err(String::clone),
            None => crate::alias::annotated(self).map(Cow::Owned),
        }
    }

    /// What consts knows without memory: the manager's where the unit
    /// carries it.
    ///
    /// A unit that carries none was made without the manager, and deriving them
    /// here would be a second derivation of a fact the manager holds (or a
    /// stale copy of it): asking is a bug. A caller over a body the manager has
    /// not seen states what it computes with `with_registers`.
    pub fn registers(&self) -> Cow<'a, IndexMap<ValueId, Known>> {
        match self.registers {
            Some(registers) => Cow::Borrowed(registers),
            None => panic!("a unit with no registers was asked for them: take them from the analysis manager"),
        }
    }

    /// Every pointer's objects, as `alias::pointers`: the manager's where
    /// the unit carries them.
    pub fn pointers(&self) -> Result<Cow<'a, PointsTo>, String> {
        match self.pointers {
            Some(pointers) => Ok(Cow::Borrowed(pointers)),
            None => crate::alias::points_to(self, None, None).map(Cow::Owned),
        }
    }

    /// The function's dominance and loops: the manager's where the unit
    /// carries them.
    pub fn assumptions(&self) -> Cow<'a, Assumptions> {
        match self.assumptions {
            Some(found) => Cow::Borrowed(found),
            None => Cow::Owned(Assumptions::of(self)),
        }
    }

    pub fn shape(&self) -> Cow<'a, Shape> {
        match self.shape {
            Some(shape) => {
                if llrm_support::env_flag!("LLRM_CHECK_SHAPE") {
                    assert!(
                        *shape == Shape::of(self.function),
                        "the shape a unit carries is not that of the body it stands over: stale"
                    );
                }
                Cow::Borrowed(shape)
            }
            None => panic!("a unit with no shape was asked for it: take it from the analysis manager"),
        }
    }

    /// The access `inst` makes: alias's, where the unit has its references.
    pub fn reference(
        &self,
        inst: InstId,
    ) -> Option<MemRef> {
        self.references.and_then(|all| all.get(&inst).cloned()).or_else(|| MemRef::of(self, inst))
    }

    pub fn operand_type(
        &self,
        operand: Operand,
    ) -> Option<TypeId> {
        self.function.operand_type(self.context, operand)
    }

    /// An integer operand's width in bits.
    pub fn int_bits(
        &self,
        operand: Operand,
    ) -> Option<u32> {
        self.operand_type(operand).and_then(|ty| self.context.types.int_bits(ty))
    }

    /// The base reference of the load or store `inst`: its pointer decomposed,
    /// made once and kept across the edits that do not reach it
    /// (`references`).
    pub fn base_reference(
        &self,
        inst: InstId,
    ) -> Option<std::rc::Rc<MemRef>> {
        crate::references::of(self, inst)
    }

    /// An integer constant's bits, if `operand` is one.
    pub fn int_constant(
        &self,
        operand: Operand,
    ) -> Option<u128> {
        match operand {
            Operand::Constant(id) => match self.context.get(id).kind {
                ConstantKind::Int(bits) => Some(bits),
                _ => None,
            },
            _ => None,
        }
    }

    /// The address spaces by role: the program's target's, or one flat space
    /// where none is named.
    pub fn spaces(&self) -> llrm_mir::spaces::Spaces {
        self.program.map_or(self.spaces, |program| program.spaces)
    }

    /// A pointer operand's address space.
    pub fn space(
        &self,
        operand: Operand,
    ) -> Option<u32> {
        match self.context.types.get(self.operand_type(operand)?) {
            Type::Pointer(space) => Some(*space),
            _ => None,
        }
    }

    /// The instruction defining `operand`, if a local value defines it.
    pub fn defining(
        &self,
        operand: Operand,
    ) -> Option<(InstId, &'a llrm_mir::module::Instruction)> {
        let Operand::Value(value) = operand else { return None };
        DEFINITIONS_READ.with(|read| {
            if let Some(read) = read.borrow_mut().as_mut() {
                read.push(value);
            }
        });
        match self.function.value(value).def {
            ValueDef::Instruction(inst) => Some((inst, self.function.instruction(inst))),
            ValueDef::Argument(_) => None,
        }
    }
}

thread_local! {
    /// The values whose defining instruction a decomposition is reading, while
    /// one is (`reading`).
    static DEFINITIONS_READ: std::cell::RefCell<Option<Vec<ValueId>>> = const { std::cell::RefCell::new(None) };
}

/// `work`, and every value whose defining instruction it asked of
/// `Unit::defining`: the values an edit would have to touch to change what
/// `work` made, however far from the pointer the helper that read it was (a
/// far pointer's selector is the operand of the cast two instructions back).
fn reading<T>(work: impl FnOnce() -> T) -> (T, Vec<ValueId>) {
    let outer = DEFINITIONS_READ.with(|read| read.replace(Some(Vec::new())));
    let made = work();
    let mut mine = DEFINITIONS_READ.with(|read| read.replace(outer)).unwrap_or_default();
    mine.sort_unstable_by_key(|value| value.0);
    mine.dedup();
    (made, mine)
}

/// Whether `inst` marks an object's lifetime, which names it without handing
/// out its address.
pub fn is_lifetime_marker(
    unit: &Unit,
    inst: InstId,
) -> bool {
    matches!(
        unit.intrinsic(inst),
        Some(llrm_mir::intrinsics::Intrinsic::LifetimeStart | llrm_mir::intrinsics::Intrinsic::LifetimeEnd)
    )
}

/// What is known of each value without memory (`consts::known`).
pub type Knowns = llrm_support::hash::SparseIdMap<ValueId, Known>;

/// What is known of a body without memory (`consts::known`), for a caller that
/// changes the body as it goes: the manager's where the body is as the manager
/// saw it, derived again, once for each state, once it is not. The one
/// place a unit's registers are derived outside the manager.
pub struct Standing<'h> {
    held: Option<&'h Knowns>,
    derived: Option<Knowns>,
    /// What stood before an edit, and the values the edit reached: `derived`
    /// is made from them when next asked.
    edited: Option<(Knowns, llrm_mir::dense::IdSet<ValueId>)>,
    /// The blocks, edges and loops of the body, as `held` has them or derived
    /// once: they stand while the body's blocks and edges do (`reshaped`).
    shape: Option<std::rc::Rc<crate::cfg::Shape>>,
    /// Brings the facts the manager holds up to date with the body, asked once
    /// the body changed and with whether its blocks and edges did: the
    /// manager updates them from the changes logged, where deriving them again
    /// walks the whole body.
    renew: Option<Box<Renew<'h>>>,
    renewed: Option<std::rc::Rc<Knowns>>,
    reshaped: bool,
}

/// What `Standing::renewing` is given: the context, layout and function, and
/// whether the blocks and edges changed.
pub type Renew<'h> = dyn FnMut(&Context, &DataLayout, &Function, bool) -> std::rc::Rc<Knowns> + 'h;

impl<'h> Standing<'h> {
    /// `self`, asking `renew` for the facts of the body once it has changed,
    /// where it would derive them.
    pub fn renewing(
        mut self,
        renew: impl FnMut(&Context, &DataLayout, &Function, bool) -> std::rc::Rc<Knowns> + 'h,
    ) -> Self {
        self.renew = Some(Box::new(renew));
        self
    }

    /// The body is as `registers` were found of it.
    pub fn held(registers: &'h Knowns) -> Self {
        Self {
            held: Some(registers),
            derived: None,
            edited: None,
            shape: None,
            renew: None,
            renewed: None,
            reshaped: false,
        }
    }

    /// No one has found them: derived when first asked.
    pub fn underived() -> Self {
        Self { held: None, derived: None, edited: None, shape: None, renew: None, renewed: None, reshaped: false }
    }

    /// `self`, the shape of the body as the manager found it.
    pub fn with_shape(
        mut self,
        shape: std::rc::Rc<crate::cfg::Shape>,
    ) -> Self {
        self.shape = Some(shape);
        self
    }

    /// The shape of `function`, found once while its blocks and edges stand.
    pub fn shape(
        &mut self,
        function: &Function,
    ) -> std::rc::Rc<crate::cfg::Shape> {
        std::rc::Rc::clone(self.shape.get_or_insert_with(|| std::rc::Rc::new(crate::cfg::Shape::of(function))))
    }

    /// The body's blocks or edges changed: its shape, and with it what was
    /// found of it, no longer holds.
    pub fn reshaped(&mut self) {
        self.shape = None;
        self.changed();
        self.reshaped = true;
    }

    /// The body changed: what was found of it no longer holds.
    pub fn changed(&mut self) {
        self.held = None;
        self.derived = None;
        self.edited = None;
        self.renewed = None;
    }

    /// The body changed by `changes` and not in its blocks or edges: a value's
    /// fact is of the values it is made from alone, so what stood stands for
    /// every value the edit did not reach (`manager::Registers::update`),
    /// and the rest are settled when next asked.
    pub fn edited(
        &mut self,
        function: &Function,
        changes: &[llrm_mir::module::Change],
    ) {
        use llrm_mir::module::Change;
        let kept = match (self.edited.take(), self.derived.take(), self.held.take()) {
            (Some((kept, _)), _, _) | (None, Some(kept), _) => kept,
            (None, None, Some(held)) => held.clone(),
            (None, None, None) if self.renewed.is_some() => (**self.renewed.as_ref().unwrap()).clone(),
            // Nothing stood: derived afresh when asked.
            (None, None, None) => {
                self.changed();
                return;
            }
        };
        self.renewed = None;
        let mut kept = kept;
        let mut touched = llrm_mir::dense::IdSet::<ValueId>::new();
        let mut work = Vec::<ValueId>::new();
        for change in changes {
            let inst = match *change {
                Change::Inserted { inst, .. }
                | Change::Erased { inst, .. }
                | Change::Moved { inst, .. }
                | Change::Rewritten(inst) => inst,
                Change::Cloned { to, .. } => to,
                Change::BlockCreated(_)
                | Change::BlockErased(_)
                | Change::Operand { .. }
                | Change::Operands { .. }
                | Change::Substituted { .. } => continue,
                Change::Parameters => {
                    self.changed();
                    return;
                }
            };
            if let Some(result) = function.instruction(inst).result
                && touched.insert(result)
            {
                work.push(result);
            }
        }
        // The touched, those whose fact was made from them, and the users of
        // each.
        let mut dirty = touched.clone();
        let mut seen = touched;
        while let Some(value) = work.pop() {
            kept.swap_remove(&value);
            for user in function.users(value) {
                if function.parent(user.user).is_none() {
                    continue;
                }
                let Some(result) = function.instruction(user.user).result else { continue };
                dirty.insert(result);
                if kept.contains_key(&result) && seen.insert(result) {
                    work.push(result);
                }
            }
        }
        self.edited = Some((kept, dirty));
    }

    /// What is known of `unit`'s body as it stands, which it must be the one
    /// these were asked of.
    pub fn of(
        &mut self,
        unit: &Unit,
    ) -> &Knowns {
        if let Some(held) = self.held {
            return held;
        }
        if let Some((kept, dirty)) = self.edited.take() {
            self.renewed = None;
            self.derived = Some(crate::consts::known_from(&Unit { registers: None, ..*unit }, kept, Some(&dirty), &[]));
        }
        if self.derived.is_none()
            && self.renewed.is_none()
            && let Some(renew) = self.renew.as_mut()
        {
            self.renewed = Some(renew(unit.context, unit.layout, unit.function, self.reshaped));
        }
        if let Some(renewed) = &self.renewed {
            return renewed;
        }
        self.derived.get_or_insert_with(|| crate::consts::known(&Unit { registers: None, ..*unit }))
    }
}

/// The loads and stores of `function`: the capacity of a map of each one's
/// reference.
pub(crate) fn loads_and_stores(function: &Function) -> usize {
    function
        .walk()
        .filter(|&(_, inst)| matches!(
            function.instruction(inst).opcode,
            Opcode::Load { .. } | Opcode::Store { .. }
        ))
        .count()
}

/// The allocas of `unit`'s function whose address is exposed, in one pass: what
/// `object_of` reads of `Unit::exposed` instead of asking each alloca's uses.
/// Whether operand `at` of `inst` hands out no address that outlives it: a
/// lifetime marker's, or a call argument the callee keeps no copy of
/// ([`nocapture`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/CaptureTracking.cpp#L304)).
pub fn kept_by(
    unit: &Unit,
    inst: InstId,
    at: usize,
) -> bool {
    is_lifetime_marker(unit, inst) || crate::alias::no_capture_argument(unit, inst, at)
}

pub fn exposed_frames(unit: &Unit) -> BTreeSet<ValueId> {
    llrm_mir::counts::bump(llrm_mir::counts::Event::ExposedFrames);
    crate::frameescape::exposed_allocas(unit.function, |inst, at| kept_by(unit, inst, at))
}

/// The object `root` is the address of, where it is an object's own: an
/// alloca (`Frame`) or a global variable (`Global`).
pub fn object_of(
    unit: &Unit,
    root: Operand,
) -> Option<ObjectRef> {
    match root {
        Operand::Value(value) => {
            let (_, instruction) = unit.defining(root)?;
            let Opcode::Alloca { allocated, .. } = instruction.opcode else { return None };
            let size = unit.layout.alloc_size(&unit.context.types, allocated) as i64;
            let count = match instruction.operands.first() {
                None => Some(1),
                Some(&count) => unit.int_constant(count).map(|bits| bits as i64),
            };
            // Only a reference naming an alloca reaches it until its address
            // is exposed.
            let exposed = match unit.exposed {
                Some(found) => found.contains(&value),
                None => crate::frameescape::exposes(unit.function, value, |inst, at| kept_by(unit, inst, at)),
            };
            Some(ObjectInterner::of(unit.context).intern(MemoryObject {
                identity: Some(Identity::Value(value.0)),
                extent: count.map(|count| size * count),
                addressed: exposed,
                captured: exposed,
                ..MemoryObject::new(MemoryKind::Frame)
            }))
        }
        Operand::Constant(id) => {
            let ConstantKind::Global(global) = unit.context.get(id).kind else { return None };
            global_object(unit, global)
        }
        Operand::Block(_) => None,
    }
}

/// Whether `root` is an object's own address: `object_of(unit, root).is_some()`
/// without making the object (its extent, exposure and interning), which a
/// reference built for each access had been doing: 1.8% of a d_faces -O1
/// compile.
pub fn is_object(
    unit: &Unit,
    root: Operand,
) -> bool {
    match root {
        Operand::Value(_) => {
            unit.defining(root).is_some_and(|(_, instruction)| matches!(instruction.opcode, Opcode::Alloca { .. }))
        }
        Operand::Constant(id) => {
            matches!(
                unit.context.get(id).kind,
                ConstantKind::Global(global) if matches!(unit.globals.get(global.0 as usize).map(|one| &one.kind), Some(GlobalKind::Variable(_)))
            )
        }
        Operand::Block(_) => false,
    }
}

/// The object a global variable is.
pub fn global_object(
    unit: &Unit,
    global: GlobalId,
) -> Option<ObjectRef> {
    let extent = match &unit.globals.get(global.0 as usize)?.kind {
        GlobalKind::Variable(variable) => Some(unit.layout.alloc_size(&unit.context.types, variable.ty) as i64),
        GlobalKind::Function(_) => return None,
    };
    let captured = !unit.globals_aa.is_some_and(|aa| aa.tracked(global));
    let constant = matches!(
        &unit.globals.get(global.0 as usize)?.kind,
        GlobalKind::Variable(variable) if variable.constant
    );
    Some(ObjectInterner::of(unit.context).intern(MemoryObject {
        identity: Some(Identity::Global(global.0)),
        extent,
        captured,
        constant,
        ..MemoryObject::new(MemoryKind::Global)
    }))
}

/// An access as the alias queries read it: LLVM's `MemoryLocation`, its
/// pointer decomposed into a root, a constant displacement and at most one
/// variable index.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MemRef {
    /// The pointer accessed; none for bytes a call's effect names by
    /// object alone.
    pub pointer: Option<Operand>,
    /// What `disp` and `base` are added to: a pointer no GEP, bitcast or
    /// near-far cast computed.
    pub root: Option<Operand>,
    /// Constant bytes added to `root`, wrapped to the index width.
    pub disp: i64,
    /// The one variable index added: old `base`.
    pub base: Option<ValueId>,
    /// Bytes per unit of `base`.
    pub scale: i64,
    /// `base`'s width, in bits.
    pub base_width: u32,
    /// The selector of a far pointer made from one, as `segment:0`: old
    /// `segment`.
    pub segment: Option<Operand>,
    /// The selector's number, where it is a constant: `segment`'s, or that
    /// of a pair `root` an integer constant makes.
    pub selector: Option<i64>,
    /// `root`'s offset in its segment where an integer constant makes it,
    /// else 0: `disp` counts from it.
    pub origin: i64,
    /// `root` is an integer constant a target with one address space made a
    /// pointer of: `origin` is its linear address.
    pub linear: bool,
    /// `root` is an object's own address, so `disp` is an offset in it:
    /// old `Space::Segment` and `Space::Frame`, as against a pointer.
    pub object: bool,
    pub space: u32,
    pub index_bits: u32,
    /// Bytes accessed.
    pub width: u32,
    /// The `!tbaa` access type's name.
    pub typed: Option<std::rc::Rc<str>>,
    /// The names of that type's ancestors, nearest first: an access whose type
    /// is one of them may alias this one's (a parent type covers its children,
    /// as C's `omnipotent char` covers every scalar).
    pub lineage: std::rc::Rc<[String]>,
    /// Every GEP on the way from `root` was `inbounds`.
    pub inbounds: bool,
    pub volatile: bool,
    pub provenance: Option<Provenance>,
}

/// The accesses worked out for a version of a function, and which version:
/// of its text, the spaces and the globals the unit lists (a unit with none
/// finds no root a global variable, `object`).
struct Held {
    key: (llrm_mir::module::Mark, llrm_mir::spaces::Spaces, usize),
    seen: std::rc::Rc<
        std::cell::RefCell<llrm_support::hash::SparseIdMap<InstId, (Option<std::rc::Rc<MemRef>>, Option<u32>)>>,
    >,
}

/// The most versions kept: the analyses of one function ask in a run, and a
/// call summary asks of its callees in between.
const ACCESS_VERSIONS: usize = 8;

/// How many cells the held versions of accesses take, for a test that asking
/// of one instruction in a large body takes one: a slot for every id below its
/// own cost 56.6 M of the 5.9 G instructions of a d_faces -O2 compile.
#[cfg(test)]
pub(crate) fn held_cells() -> usize {
    ACCESSES.with(|held| held.borrow().iter().map(|one| one.seen.borrow().len()).sum())
}

thread_local! {
    static ACCESSES: std::cell::RefCell<Vec<Held>> = const { std::cell::RefCell::new(Vec::new()) };
}

impl MemRef {
    /// This reference with each value that `to` gives another operand named by
    /// it: the reference to the same bytes by the value that replaced one found
    /// equal. None where a name cannot be carried over: the replaced value is
    /// the object's own address (`object`: `disp` counts from it), keys an
    /// object of `provenance`, or is the variable index (a constant has no
    /// register to scale).
    pub fn renamed(
        &self,
        to: &dyn Fn(ValueId) -> Option<Operand>,
    ) -> Option<Self> {
        let named = |operand: Option<Operand>| match operand {
            Some(Operand::Value(value)) => to(value).map(Some),
            other => Some(other),
        };
        let replaced = |operand: Option<Operand>| matches!(operand, Some(Operand::Value(value)) if to(value).is_some());
        if self.object && replaced(self.root) {
            return None;
        }
        if let Some(provenance) = &self.provenance {
            if provenance.slices.iter().any(|slice| {
                matches!(
                    slice.object.key,
                    Key::Value(value) if to(ValueId(value)).is_some()
                )
            }) {
                return None;
            }
        }
        let base = match self.base {
            Some(value) => match to(value) {
                Some(Operand::Value(now)) => Some(now),
                Some(_) => return None,
                None => Some(value),
            },
            None => None,
        };
        Some(Self {
            pointer: named(self.pointer)?.or(self.pointer),
            root: named(self.root)?.or(self.root),
            segment: named(self.segment)?.or(self.segment),
            base,
            ..self.clone()
        })
    }

    /// Whether every object it may reach is constant, so that no write
    /// changes it.
    pub fn unwritable(&self) -> bool {
        self.provenance
            .as_ref()
            .is_some_and(|one| !one.slices.is_empty() && one.slices.iter().all(|slice| slice.object.constant))
    }

    /// `width` bytes at `pointer`.
    pub fn at(
        unit: &Unit,
        pointer: Operand,
        width: u32,
    ) -> Self {
        Self::at_tracing(unit, pointer, width, None)
    }

    /// `at`, noting in `chain` each value whose definition it read.
    fn at_tracing(
        unit: &Unit,
        pointer: Operand,
        width: u32,
        mut chain: Option<&mut Vec<ValueId>>,
    ) -> Self {
        let space = unit.space(pointer).unwrap_or(0);
        let index_bits = unit.layout.pointer(space).index_bits;
        let mut made = Self {
            pointer: Some(pointer),
            root: Some(pointer),
            disp: 0,
            base: None,
            scale: 0,
            base_width: 0,
            segment: None,
            selector: None,
            origin: 0,
            linear: false,
            object: false,
            space,
            index_bits,
            width,
            typed: None,
            lineage: no_lineage(),
            inbounds: true,
            volatile: false,
            provenance: None,
        };
        let mut disp: i128 = 0;
        let mut root = pointer;
        // A block nothing enters may define a pointer from itself; a chain
        // longer than 64 starts remembering what it has passed.
        let mut steps = 0u32;
        let mut seen = BTreeSet::new();
        loop {
            steps += 1;
            if steps > 64 {
                if let Operand::Value(value) = root
                    && !seen.insert(value)
                {
                    break;
                }
            }
            if let (Some(chain), Operand::Value(value)) = (chain.as_deref_mut(), root) {
                chain.push(value);
            }
            let Some(step) = step(unit, root) else { break };
            match step {
                Step::Through(inner) => root = inner,
                Step::Offset { pointer: inner, constant, variable, inbounds } => {
                    match (variable, made.base) {
                        (None, _) => {}
                        (Some((index, scale)), None) => {
                            made.base = Some(index);
                            made.scale = scale;
                            made.base_width = unit.int_bits(Operand::Value(index)).unwrap_or(index_bits);
                        }
                        (Some(_), Some(_)) => break,
                    }
                    disp += constant;
                    made.inbounds &= inbounds;
                    root = inner;
                }
            }
        }
        made.root = Some(root);
        made.disp = wrapped(disp, index_bits);
        made.segment = segment(unit, root);
        made.selector = made.segment.and_then(|one| unit.int_constant(one)).map(|bits| bits as i64);
        if let Some((selector, origin)) = pair_constant(unit, root) {
            (made.selector, made.origin) = (Some(selector), origin);
        } else if let Some(address) = flat_constant(unit, root) {
            (made.origin, made.linear) = (address, true);
        }
        made.object = is_object(unit, root);
        made
    }

    /// The access instruction `inst` makes: a load's or a store's. It is a fact
    /// of the function's text alone (the pointer's chain of casts and offsets,
    /// the access's type and width), so it is worked out once for each version
    /// of the function, whichever of the analyses that read it asks first: a
    /// quarter of the compile of d_faces was this, asked again by `escapes`,
    /// `annotated`, `slotstores`, the summaries and the memory walks.
    pub fn of(
        unit: &Unit,
        inst: InstId,
    ) -> Option<Self> {
        let key = (unit.function.mark(), unit.spaces(), unit.globals.len());
        let held = ACCESSES.with(|held| {
            let held = held.borrow();
            held.iter().rev().find(|one| one.key == key).map(|one| std::rc::Rc::clone(&one.seen))
        });
        let seen = match held {
            Some(seen) => seen,
            None => ACCESSES.with(|held| {
                let mut held = held.borrow_mut();
                if held.len() >= ACCESS_VERSIONS {
                    held.remove(0);
                }
                let seen = std::rc::Rc::new(std::cell::RefCell::new(llrm_support::hash::SparseIdMap::default()));
                held.push(Held { key, seen: std::rc::Rc::clone(&seen) });
                seen
            }),
        };
        let tag =
            unit.function.instruction(inst).metadata.iter().find(|(kind, _)| kind == "tbaa").map(|(_, tag)| tag.0);
        let found = seen.borrow().get(&inst).cloned();
        let Some((found, held_tag)) = found else {
            let made = crate::references::of(unit, inst);
            seen.borrow_mut().insert(inst, (made.clone(), tag));
            return made.map(|one| (*one).clone());
        };
        // Metadata is attached without a change logged: the type is asked
        // again where the tag is not the one it was held with.
        let mut found = found.map(|one| (*one).clone());
        if let (Some(access), true) = (&mut found, tag != held_tag) {
            access.typed = typed(unit, inst);
            access.lineage = lineage(unit, inst);
            seen.borrow_mut().insert(inst, (Some(std::rc::Rc::new(access.clone())), tag));
        }
        static CHECKED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *CHECKED.get_or_init(|| llrm_support::env_flag!("LLRM_CHECK_ACCESSES")) {
            assert!(
                found == Self::derived(unit, inst),
                "the held access of {inst:?} is not what deriving it again gives"
            );
        }
        found
    }

    fn derived(
        unit: &Unit,
        inst: InstId,
    ) -> Option<Self> {
        Self::decomposed(unit, inst, &mut Vec::new())
    }

    /// The access `inst` makes, decomposed afresh: what `references` keeps for
    /// the loads and stores, with the values its pointer chain read.
    pub(crate) fn decomposed(
        unit: &Unit,
        inst: InstId,
        chain: &mut Vec<ValueId>,
    ) -> Option<Self> {
        let instruction = unit.function.instruction(inst);
        let (pointer, ty, volatile) = match instruction.opcode {
            Opcode::Load { volatile, .. } => (instruction.operands[0], instruction.ty, volatile),
            Opcode::Store { volatile, .. } => {
                (instruction.operands[1], unit.operand_type(instruction.operands[0])?, volatile)
            }
            _ => return None,
        };
        let width = unit.layout.store_size(&unit.context.types, ty) as u32;
        let (made, read) = reading(|| Self::at_tracing(unit, pointer, width, Some(&mut *chain)));
        chain.extend(read);
        Some(Self { typed: typed(unit, inst), lineage: lineage(unit, inst), volatile, ..made })
    }

    /// Whether the access names its bytes outright rather than reaching
    /// them through a value: a fixed displacement in an object, or canonical
    /// provenance. An unresolved pointer or index does not.
    pub fn named(&self) -> bool {
        let canonical = self.provenance.as_ref().is_some_and(|provenance| {
            !provenance.slices.is_empty() && provenance.slices.iter().all(|one| one.object.kind != MemoryKind::Unknown)
        });
        canonical || (self.object && self.addr().is_some())
    }

    /// The bytes a call to `llvm.memset` of a constant length fills, as
    /// LLVM's `MemoryLocation::getForDest` names them: a write like a
    /// store's.
    pub fn filled(
        unit: &Unit,
        inst: InstId,
    ) -> Option<Self> {
        let instruction = unit.function.instruction(inst);
        let Opcode::Call(_) = instruction.opcode else { return None };
        let callee = llrm_mir::memory::callee(unit.context, unit.function, inst)?;
        let cell = match unit.globals.get(callee.0 as usize)?.intrinsic()? {
            Intrinsic::MemSet => 1,
            Intrinsic::MemSetPattern => unit.int_bits(instruction.operands[1])? / 8,
            _ => return None,
        };
        let width = u32::try_from(llrm_mir::intrinsics::length(unit.context, instruction.operands[2])?)
            .ok()?
            .checked_mul(cell)
            .filter(|&one| one > 0)?;
        let volatile = unit.int_constant(instruction.operands[3])? != 0;
        Some(Self { volatile, ..Self::at(unit, instruction.operands[0], width) })
    }

    /// `width` bytes of `provenance`'s objects that a call's effect names,
    /// at no pointer: old `MemRef(None, width)`.
    pub fn reach(
        width: u32,
        provenance: Provenance,
    ) -> Self {
        Self {
            pointer: None,
            root: None,
            disp: 0,
            base: None,
            scale: 0,
            base_width: 0,
            segment: None,
            selector: None,
            origin: 0,
            linear: false,
            object: false,
            space: 0,
            index_bits: 16,
            width,
            typed: None,
            lineage: no_lineage(),
            inbounds: false,
            volatile: false,
            provenance: Some(provenance),
        }
    }

    /// Where every byte of this access is fixed: `root` plus `disp`.
    pub fn addr(&self) -> Option<Addr> {
        let root = self.root?;
        (self.base.is_none() && self.segment.is_none()).then_some(Addr { root, disp: self.disp })
    }
}

/// The bits `reference` reads from a `constant` global's initializer,
/// little-endian: LLVM's `ConstantFoldLoadFromConstPtr`. None where the
/// address is not fixed or a byte is an address only the linker knows.
pub fn constant_bits(
    unit: &Unit,
    reference: &MemRef,
) -> Option<num_bigint::BigInt> {
    let addr = reference.addr()?;
    let Operand::Constant(root) = addr.root else { return None };
    let ConstantKind::Global(global) = unit.context.get(root).kind else { return None };
    let GlobalKind::Variable(variable) = &unit.globals.get(global.0 as usize)?.kind else { return None };
    if !variable.constant || reference.volatile {
        return None;
    }
    let bytes = constant_bytes(unit.context, unit.layout, variable.initializer?)?;
    let (low, width) = (usize::try_from(addr.disp).ok()?, reference.width as usize);
    let read = bytes.get(low..low.checked_add(width)?)?;
    Some(
        read.iter().rev().fold(num_bigint::BigInt::from(0), |bits, &byte| (bits << 8) | num_bigint::BigInt::from(byte)),
    )
}

/// The bytes `constant` lays out in memory; None where one is an address.
pub fn constant_bytes(
    context: &Context,
    layout: &DataLayout,
    constant: llrm_mir::context::ConstantId,
) -> Option<Vec<u8>> {
    let one = context.get(constant);
    let size = usize::try_from(layout.alloc_size(&context.types, one.ty)).ok()?;
    let little = |bits: u128, count: usize| {
        (0..size).map(|at| if at < count.min(16) { (bits >> (8 * at)) as u8 } else { 0 }).collect::<Vec<_>>()
    };
    match &one.kind {
        ConstantKind::Zero => Some(vec![0; size]),
        ConstantKind::Int(bits) => Some(little(*bits, size)),
        ConstantKind::Float(bits) => Some(little(u128::from(*bits), size)),
        ConstantKind::Bytes(bytes) => Some(bytes.iter().copied().chain(std::iter::repeat(0)).take(size).collect()),
        ConstantKind::Aggregate(members) => {
            let offsets = match context.types.get(one.ty) {
                Type::Array { element, .. } => {
                    let stride = layout.alloc_size(&context.types, *element);
                    (0..members.len() as u64).map(|at| at * stride).collect()
                }
                Type::Struct { .. } => layout.struct_layout(&context.types, one.ty).1,
                _ => return None,
            };
            let mut out = vec![0; size];
            for (&member, offset) in members.iter().zip(offsets) {
                let bytes = constant_bytes(context, layout, member)?;
                let at = usize::try_from(offset).ok()?;
                out.get_mut(at..at + bytes.len())?.copy_from_slice(&bytes);
            }
            Some(out)
        }
        _ => None,
    }
}

/// A fixed address: bytes past a root pointer. Old `Addr`, less its x86
/// spelling.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Addr {
    pub root: Operand,
    pub disp: i64,
}

impl Addr {
    pub fn plus(
        self,
        bytes: i64,
    ) -> Self {
        Self { disp: self.disp + bytes, ..self }
    }
}

/// `value` as a two's-complement number `bits` wide: pointer arithmetic
/// wraps at the index width.
pub fn wrapped(
    value: i128,
    bits: u32,
) -> i64 {
    signed(value as u128 & llrm_mir::context::mask(bits), bits) as i64
}

/// The bytes of the object `root` is the address of, where they are known:
/// a defined global's or a sized alloca's. A declaration's type is not
/// promised to be its size.
pub fn extent_of(
    unit: &Unit,
    root: Operand,
) -> Option<i64> {
    if let Operand::Constant(id) = root {
        let ConstantKind::Global(global) = unit.context.get(id).kind else { return None };
        let GlobalKind::Variable(variable) = &unit.globals.get(global.0 as usize)?.kind else { return None };
        variable.initializer?;
        // The size alone: making the object (its capture state, interning)
        // for it was 47 M Ir of fjoins' 991 M, asked 63,024 times.
        return Some(unit.layout.alloc_size(&unit.context.types, variable.ty) as i64);
    }
    object_of(unit, root)?.extent
}

/// A pointer as an object, the variable indexes added to it on the way, and
/// whether every GEP between was `inbounds`: `MemRef` keeps only the first
/// index, this keeps them all.
#[derive(Clone, Debug)]
pub struct Indexed {
    pub root: Operand,
    /// Each variable index and the bytes it is scaled by.
    pub indices: Vec<(ValueId, i64)>,
    pub inbounds: bool,
}

/// `pointer` walked back through its GEPs and casts to the pointer none
/// computed, with every variable index met.
pub fn indexed(
    unit: &Unit,
    pointer: Operand,
) -> Indexed {
    let mut found = Indexed { root: pointer, indices: Vec::new(), inbounds: true };
    while let Some(next) = step(unit, found.root) {
        match next {
            Step::Through(inner) => found.root = inner,
            Step::Offset { pointer: inner, variable, inbounds, .. } => {
                found.indices.extend(variable);
                found.inbounds &= inbounds;
                found.root = inner;
            }
        }
    }
    found
}

/// The pointer `pointer` is an offset or a respelling of, found without the
/// rest of what `MemRef::at` works out.
pub(crate) fn root_of(
    unit: &Unit,
    pointer: Operand,
) -> Operand {
    let mut root = pointer;
    while let Some(step) = step(unit, root) {
        match step {
            Step::Through(inner) | Step::Offset { pointer: inner, .. } => root = inner,
        }
    }
    root
}

enum Step {
    /// The same address, spelled otherwise.
    Through(Operand),
    /// `pointer` plus `constant` bytes, plus one index scaled.
    Offset { pointer: Operand, constant: i128, variable: Option<(ValueId, i64)>, inbounds: bool },
}

/// What computed `pointer` from another, if anything did.
fn step(
    unit: &Unit,
    pointer: Operand,
) -> Option<Step> {
    let types = &unit.context.types;
    let through = |from: Operand| {
        // A segment's cast to a far pointer is `segment:0`, a new root.
        (unit.space(from).is_some() && !unit.spaces().is_segment(unit.space(from))).then_some(Step::Through(from))
    };
    match pointer {
        Operand::Value(_) => {
            let (_, instruction) = unit.defining(pointer)?;
            match &instruction.opcode {
                Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) => through(instruction.operands[0]),
                Opcode::GetElementPtr { source } => {
                    let indices = instruction.operands[1..]
                        .iter()
                        .map(|&one| unit.int_constant(one).map(|bits| bits_signed(unit, one, bits)))
                        .collect::<Vec<_>>();
                    let (constant, variable) = unit.layout.collect_offset(types, *source, &indices);
                    let variable = match variable.as_slice() {
                        [] => None,
                        [(at, scale)] => match instruction.operands[1 + at] {
                            Operand::Value(index) => Some((index, *scale as i64)),
                            _ => return None,
                        },
                        _ => return None,
                    };
                    Some(Step::Offset {
                        pointer: instruction.operands[0],
                        constant,
                        variable,
                        inbounds: instruction.flags.contains(Flags::INBOUNDS),
                    })
                }
                _ => None,
            }
        }
        Operand::Constant(id) => match &unit.context.get(id).kind {
            ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::BitCast | CastOp::AddrSpaceCast, value }) => {
                through(Operand::Constant(*value))
            }
            ConstantKind::Expr(ConstantExpr::GetElementPtr { source, inbounds, operands, .. }) => {
                let indices = operands[1..]
                    .iter()
                    .map(|&one| {
                        unit.int_constant(Operand::Constant(one))
                            .map(|bits| bits_signed(unit, Operand::Constant(one), bits))
                    })
                    .collect::<Vec<_>>();
                if indices.iter().any(Option::is_none) {
                    return None;
                }
                let (constant, _) = unit.layout.collect_offset(types, *source, &indices);
                Some(Step::Offset {
                    pointer: Operand::Constant(operands[0]),
                    constant,
                    variable: None,
                    inbounds: *inbounds,
                })
            }
            _ => None,
        },
        Operand::Block(_) => None,
    }
}

fn bits_signed(
    unit: &Unit,
    operand: Operand,
    bits: u128,
) -> i128 {
    signed(bits, unit.int_bits(operand).unwrap_or(128))
}

/// The linear address of a flat pointer `root` an integer constant makes: a
/// target with one address space has no selector, and its pointer's integer is
/// the address.
pub(crate) fn flat_constant(
    unit: &Unit,
    root: Operand,
) -> Option<i64> {
    let spaces = unit.spaces();
    if !spaces.far_is_near() || spaces.segment_bytes.is_some() {
        return None;
    }
    let integer = match root {
        Operand::Value(_) => match unit.defining(root)? {
            (_, made) if matches!(made.opcode, Opcode::Cast(CastOp::IntToPtr)) => made.operands[0],
            _ => return None,
        },
        Operand::Constant(id) => match &unit.context.get(id).kind {
            ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::IntToPtr, value }) => Operand::Constant(*value),
            _ => return None,
        },
        Operand::Block(_) => return None,
    };
    unit.int_constant(integer).and_then(|bits| i64::try_from(bits).ok())
}

/// The selector `root` is `segment:0` of: the integer an `inttoptr` made
/// the segment from, or the segment itself.
fn segment(
    unit: &Unit,
    root: Operand,
) -> Option<Operand> {
    let from = match root {
        Operand::Constant(id) => match &unit.context.get(id).kind {
            ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::AddrSpaceCast, value }) => Operand::Constant(*value),
            _ => return None,
        },
        _ => {
            let (_, instruction) = unit.defining(root)?;
            if !matches!(instruction.opcode, Opcode::Cast(CastOp::AddrSpaceCast)) {
                return None;
            }
            instruction.operands[0]
        }
    };
    if unit.space(from) != Some(2) {
        return None;
    }
    match from {
        Operand::Constant(id) => match &unit.context.get(id).kind {
            ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::IntToPtr, value }) => Some(Operand::Constant(*value)),
            _ => Some(from),
        },
        _ => match unit.defining(from) {
            Some((_, made)) if matches!(made.opcode, Opcode::Cast(CastOp::IntToPtr)) => Some(made.operands[0]),
            _ => Some(from),
        },
    }
}

/// The selector and offset of a pair pointer `root` an integer constant
/// makes: a pair's integer form is its selector word, then its offset word.
fn pair_constant(
    unit: &Unit,
    root: Operand,
) -> Option<(i64, i64)> {
    let space = unit.space(root)?;
    if !unit.layout.is_pair(space) {
        return None;
    }
    let integer = match root {
        Operand::Value(_) => match unit.defining(root)? {
            (_, made) if matches!(made.opcode, Opcode::Cast(CastOp::IntToPtr)) => made.operands[0],
            _ => return None,
        },
        Operand::Constant(id) => match &unit.context.get(id).kind {
            ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::IntToPtr, value }) => Operand::Constant(*value),
            _ => return None,
        },
        Operand::Block(_) => return None,
    };
    let bits = unit.int_constant(integer)?;
    let offset = unit.layout.offset_bits(space);
    let word = |shift: u32| ((bits >> shift) & ((1 << offset) - 1)) as i64;
    Some((word(offset), word(0)))
}

/// The names of `!tbaa` type nodes read off metadata without the module's tree,
/// once for each node: a reference was given the name and every ancestor's as
/// new strings (7% of the compile's allocations, QCport). Per module, in its
/// context, and keyed by the metadata it was read from.
#[derive(Default)]
struct TypeNames {
    lineages: std::cell::RefCell<llrm_support::hash::HashMap<(usize, usize, u32), std::rc::Rc<[String]>>>,
    names: std::cell::RefCell<llrm_support::hash::HashMap<(usize, usize, u32), Option<std::rc::Rc<str>>>>,
}

/// The name of the `!tbaa` access type `inst` carries.
pub fn typed(
    unit: &Unit,
    inst: InstId,
) -> Option<std::rc::Rc<str>> {
    #[cfg(test)]
    TYPE_READS.with(|count| count.set(count.get() + 1));
    let (_, tag) = unit.function.instruction(inst).metadata.iter().find(|(kind, _)| kind == "tbaa")?;
    if let Some(tree) = unit.tbaa {
        // A type's name read through the tree is the type node's first operand,
        // which is the name below.
        return tree.name_of_tag(unit.metadata, *tag);
    }
    let key = (unit.metadata.as_ptr() as usize, unit.metadata.len(), tag.0);
    let held = unit.context.extension::<TypeNames>();
    if let Some(found) = held.names.borrow().get(&key) {
        return found.clone();
    }
    let name = match unit.metadata.get(tag.0 as usize)?.operands.first()? {
        MetadataOperand::Node(ty) => match unit.metadata.get(ty.0 as usize)?.operands.first()? {
            MetadataOperand::String(name) => Some(std::rc::Rc::from(name.as_str())),
            _ => None,
        },
        _ => None,
    };
    held.names.borrow_mut().insert(key, name.clone());
    name
}

/// The names of the ancestors of the `!tbaa` access type `inst` carries,
/// nearest first, the root last: the module's type tree where the unit holds
/// it, else built here from the metadata.
pub fn lineage(
    unit: &Unit,
    inst: InstId,
) -> std::rc::Rc<[String]> {
    #[cfg(test)]
    TYPE_READS.with(|count| count.set(count.get() + 1));
    let Some((_, tag)) = unit.function.instruction(inst).metadata.iter().find(|(kind, _)| kind == "tbaa") else {
        return no_lineage();
    };
    if let Some(tree) = unit.tbaa {
        return tree.shared_of_tag(unit.metadata, *tag);
    }
    let key = (unit.metadata.as_ptr() as usize, unit.metadata.len(), tag.0);
    let held = unit.context.extension::<TypeNames>();
    if let Some(found) = held.lineages.borrow().get(&key) {
        return std::rc::Rc::clone(found);
    }
    let made: std::rc::Rc<[String]> = std::rc::Rc::from(llrm_mir::tbaa::Tbaa::chain(unit.metadata, *tag));
    held.lineages.borrow_mut().insert(key, std::rc::Rc::clone(&made));
    made
}

/// A lineage of no names, shared: an access with no type is the common one.
pub fn no_lineage() -> std::rc::Rc<[String]> {
    thread_local! {
        static NONE: std::rc::Rc<[String]> = std::rc::Rc::from(Vec::new());
    }
    NONE.with(std::rc::Rc::clone)
}

/// What a load or store does to the bytes it addresses, and it touches no
/// others: volatile or not, as LLVM's. None for any other instruction.
/// The one statement of it: `Accesses`, `unmodeled_write` and
/// `effects::unmodeled` all ask here (#257 regressed where one did not).
pub fn own_bytes(opcode: &Opcode) -> Option<llrm_mir::memory::Effects> {
    match opcode {
        Opcode::Load { .. } => Some(llrm_mir::memory::Effects { reads: true, writes: false }),
        Opcode::Store { .. } => Some(llrm_mir::memory::Effects { reads: false, writes: true }),
        _ => None,
    }
}

/// Whether `inst` may write memory beyond what its own access says:
/// effects.rs's `unmodeled_write`. A call writes what its callee may, as
/// its attributes and the callee's state it; a load or store only what
/// it addresses (`own_bytes`).
pub fn unmodeled_write(
    unit: &Unit,
    inst: InstId,
) -> bool {
    let instruction = unit.function.instruction(inst);
    if own_bytes(&instruction.opcode).is_some() {
        return false;
    }
    match &instruction.opcode {
        Opcode::Call(info) | Opcode::Invoke(info) => {
            let callee = instruction.operands.last().and_then(|&one| match one {
                Operand::Constant(id) => match unit.context.get(id).kind {
                    ConstantKind::Global(global) => unit.globals.get(global.0 as usize).and_then(GlobalValue::function),
                    _ => None,
                },
                _ => None,
            });
            llrm_mir::memory::stated(&info.attrs).writes
                && callee.is_none_or(|callee| llrm_mir::memory::stated(&callee.attrs).writes)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use llrm_mir::module::Operand;

    use super::{Identity, MemRef, MemoryKind, MemoryObject, Provenance, Slice, SliceError, Unit};
    use crate::testing::{DOS, function, layout, parsed, value};

    fn object(kind: MemoryKind) -> MemoryObject {
        MemoryObject::new(kind)
    }

    /// A loop nothing enters (a block group that only reaches itself) can
    /// define a pointer as `gep %p, 1` of itself, as a function with
    /// JumpThreading did before `decide` removed it:
    /// the chain walk never ended and held a value per step, 50 GB in 100 s
    /// on QB's datetime.c at -Os -m16. A chain that comes back to a value ends
    /// there, as `getUnderlyingObject` ends on a value it has seen.
    #[test]
    fn a_pointer_chain_that_comes_back_to_itself_ends() {
        let module = parsed(&format!(
            "{DOS}define i8 @f(ptr %a) {{\nb0:\n  ret i8 0\nb1:\n  %p = getelementptr inbounds i8, ptr %p, i16 1\n  %v = load i8, ptr %p\n  br label %b1\n}}\n"
        ));
        let layout = layout(&module);
        let (_, _, f) = module.functions().next().unwrap();
        let unit = Unit::of(&module, &layout, f);
        let (_, load) = f
            .walk()
            .find(|&(_, at)| matches!(
                f.instruction(at).opcode,
                llrm_mir::opcode::Opcode::Load { .. }
            ))
            .unwrap();
        assert!(MemRef::of(&unit, load).is_some());
    }

    /// Isel's unit named no program, so the spaces became one flat space: a QB
    /// program's fixed-address pokes (`DEF SEG`) were no longer apart from
    /// every object and demo-qbdemo grew 6 bytes. A unit asks the spaces it
    /// was given when no program names the target.
    #[test]
    fn a_unit_with_no_program_asks_the_spaces_it_was_given() {
        let module = parsed(&format!("{DOS}define void @f() {{\nb0:\n  ret void\n}}\n"));
        let layout = layout(&module);
        let (_, _, f) = module.functions().next().unwrap();
        let dos = llrm_x86_m16::spaces();
        assert_eq!(Unit::of(&module, &layout, f).spaces(), llrm_mir::spaces::Spaces::FLAT);
        assert_eq!(Unit::of(&module, &layout, f).with_spaces(dos).spaces(), dos);
        assert!(Unit::of(&module, &layout, f).with_spaces(dos).spaces().is_fixed(4));
    }

    /// An edit stopped exposing a frame slot and its object was a second
    /// number: a result kept past the edit named it 15 (addressed), one
    /// derived after it 17 (not), and a query between the two found two
    /// objects that cannot alias (`i64_param_home.c` at -O2 -m32 lost the home
    /// store of its i64 parameter). An object is one number whatever its
    /// facts, each reference keeping the facts it was made with.
    #[test]
    fn an_object_is_one_number_whatever_facts_its_references_carry() {
        let interner = super::ObjectInterner::default();
        let slot = |exposed: bool| {
            interner.intern(MemoryObject {
                identity: Some(Identity::Value(1)),
                extent: Some(8),
                addressed: exposed,
                captured: exposed,
                ..MemoryObject::new(MemoryKind::Frame)
            })
        };
        let (kept, fresh) = (slot(true), slot(false));
        assert_eq!(kept, fresh);
        assert_eq!(kept.id(), fresh.id());
        assert!(kept.addressed() && kept.captured(), "each reference keeps the facts it was made with");
        assert!(!fresh.addressed() && !fresh.captured());
        assert!(super::objects_may_alias(&kept, &fresh), "the same object");
        assert!(!interner.object(fresh).addressed, "and gives them back");
        assert_eq!(Provenance::one(kept).union(&Provenance::one(fresh)).slices.len(), 1, "a set holds it once");
    }

    /// A set of slices compared the objects' identities (strings, tuples) tree
    /// against tree at every step of every insert, and cloned them:
    /// `BTreeSet<Slice>` was 10% of host.c's compile, `MemoryObject::cmp` 8%,
    /// malloc 10%. An object is a number once interned: building, probing
    /// and cloning sets of slices compares none structurally.
    #[test]
    fn a_set_of_slices_compares_no_object_structurally() {
        let slices: Vec<Slice> = (0..200)
            .map(|index| {
                Slice::whole(MemoryObject {
                    identity: Some(Identity::Tuple(vec![
                        Identity::Str(format!("object{index}")),
                        Identity::Int(index),
                    ])),
                    ..object(MemoryKind::Global)
                })
            })
            .collect();
        let before = super::OBJECT_COMPARES.with(std::cell::Cell::get);
        let set: BTreeSet<Slice> = slices.iter().copied().collect();
        assert_eq!(set.len(), 200);
        assert!(slices.iter().all(|one| set.contains(one)));
        let copy = set.clone();
        assert_eq!(copy, set);
        assert_eq!(super::OBJECT_COMPARES.with(std::cell::Cell::get) - before, 0, "objects compared tree against tree");
    }

    /// A provenance of shared sorted arrays iterates, merges and compares as
    /// the `BTreeSet` it replaced did: the output of a compile follows its
    /// order.
    #[test]
    fn a_provenance_unions_and_covers_as_a_set_does() {
        let some = |at: &[i64]| -> Provenance {
            let global = |n: i64| MemoryObject { identity: Some(Identity::Int(n)), ..object(MemoryKind::Global) };
            Provenance { slices: at.iter().map(|&n| Slice::whole(global(n))).collect(), restrict: Default::default() }
        };
        let (one, two) = (some(&[3, 1, 1]), some(&[2, 3]));
        let both = one.union(&two);
        let expect: BTreeSet<Slice> = one.slices.iter().chain(two.slices.iter()).copied().collect();
        assert_eq!(both.slices.iter().collect::<Vec<_>>(), expect.iter().collect::<Vec<_>>());
        assert!(both.covers(&one) && both.covers(&two) && !one.covers(&two));
        assert!(one.covers(&one) && one.covers(&some(&[])) && !some(&[]).covers(&one));
        // A kept [-2^31, 2^31) covers a fresh [0, 1); a pointer one past the
        // end of a 32-byte object lies within a slice of it.
        let global = MemoryObject { identity: Some(Identity::Int(9)), extent: Some(32), ..object(MemoryKind::Global) };
        let held = |low, high| Provenance::one_with_slice(global.clone(), low, high, 1, 1, BTreeSet::new()).unwrap();
        assert!(held(-(1 << 31), 1 << 31).covers(&held(0, 1)) && !held(0, 1).covers(&held(-(1 << 31), 1 << 31)));
        assert!(held(0, 32).covers(&held(32, 33)));
        assert_eq!(some(&[1, 3]), one);
    }

    /// One object is one object however often it is interned; another extent
    /// is another object.
    #[test]
    fn an_object_is_interned_by_its_identity_once() {
        let global = MemoryObject { identity: Some(Identity::Global(9)), ..object(MemoryKind::Global) };
        let (one, again) = (super::ObjectRef::from(global.clone()), super::ObjectRef::from(global.clone()));
        assert_eq!(one, again);
        assert_eq!(one.id(), again.id());
        let longer = super::ObjectRef::from(MemoryObject { extent: Some(4), ..global });
        assert_ne!(one, longer);
    }

    /// A process-wide interner numbered objects by everything interned before
    /// them in the process: a test thread, a compile before this one, the
    /// LSP server's last hour. A module's ids are the module's: the same
    /// objects asked in the same order get the same ids whichever module
    /// was done first.
    #[test]
    fn the_ids_of_a_module_do_not_depend_on_another_module_interned_before() {
        use llrm_mir::context::Context;
        let global = |number| MemoryObject { identity: Some(Identity::Global(number)), ..object(MemoryKind::Global) };
        let ids = |context: &Context, numbers: &[u32]| -> Vec<u32> {
            numbers.iter().map(|&number| super::ObjectInterner::of(context).intern(global(number)).id()).collect()
        };
        let (first, second) = (Context::new(), Context::new());
        let (a, b) = (ids(&first, &[7, 8]), ids(&second, &[8, 7]));
        let (other_second, other_first) = (Context::new(), Context::new());
        let (b_again, a_again) = (ids(&other_second, &[8, 7]), ids(&other_first, &[7, 8]));
        assert_eq!((a, b), (a_again, b_again));
        assert_eq!(ids(&Context::new(), &[7]), [3], "dense from the objects every module starts with");
    }

    /// The interner lives in the module's context and dies with it: a server
    /// compiling a file again and again held every object of every compile
    /// before.
    #[test]
    fn the_interner_is_dropped_with_its_module() {
        use llrm_mir::context::Context;
        let context = Context::new();
        let interner = super::ObjectInterner::of(&context);
        let weak = std::rc::Rc::downgrade(&interner);
        drop(interner);
        assert!(weak.upgrade().is_some(), "held by the context");
        drop(context);
        assert!(weak.upgrade().is_none(), "kept after its module");
    }

    #[test]
    fn object_kind_rules_keep_stack_and_global_separate() {
        assert!(!super::objects_may_alias(&object(MemoryKind::Stack), &object(MemoryKind::Global)));
        // Python since 8780f59b: the push area and the frame are distinct
        // objects.
        assert!(!super::objects_may_alias(&object(MemoryKind::Stack), &object(MemoryKind::Frame)));
    }

    #[test]
    fn provenance_subobjects_use_object_identity_and_byte_ranges() {
        let first = MemoryObject {
            kind: MemoryKind::Frame,
            identity: Some(Identity::Value(1)),
            generation: 0,
            extent: Some(8),
            addressed: true,
            captured: true,
            constant: false,
        };
        let second = MemoryObject { identity: Some(Identity::Value(2)), ..first.clone() };
        let a = Provenance::one_with_slice(first.clone(), 0, 4, 1, 1, BTreeSet::new()).unwrap();
        let b = Provenance::one_with_slice(first, 4, 8, 1, 1, BTreeSet::new()).unwrap();
        let c = Provenance::one_with_slice(second, 0, 4, 1, 1, BTreeSet::new()).unwrap();

        assert!(!a.intersects(&b));
        assert!(!a.intersects(&c));
        let same =
            Provenance::one_with_slice(a.slices.first().unwrap().object.clone(), 0, 2, 1, 1, BTreeSet::new()).unwrap();
        assert!(a.intersects(&same));
    }

    #[test]
    fn provenance_strided_ranges_prove_interleaved_arrays_disjoint() {
        let object =
            MemoryObject { identity: Some(Identity::Global(4)), extent: Some(64), ..object(MemoryKind::Global) };
        let even = Provenance::one_with_slice(object.clone(), 0, 64, 2, 1, BTreeSet::new()).unwrap();
        let odd = Provenance::one_with_slice(object, 1, 64, 2, 1, BTreeSet::new()).unwrap();

        assert!(!even.intersects(&odd));
    }

    #[test]
    fn strided_slice_intersection_matches_the_bytes_it_describes() {
        let object = MemoryObject {
            identity: Some(Identity::Tuple(vec![Identity::Str("allocation".to_owned()), Identity::Int(1)])),
            extent: Some(12),
            ..object(MemoryKind::Allocation)
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
        assert_eq!(empty.to_string(), "an alias slice must contain at least one byte");
        assert_eq!(stride.to_string(), "an alias stride must be positive");
        assert_eq!(width.to_string(), "an alias element width must be positive");
    }

    #[test]
    fn whole_and_bounded_whole_provenance_are_shift_fixed_points() {
        let whole = Provenance::one(object(MemoryKind::Global));
        assert_eq!(whole.shifted(8), whole);

        let bounded_object = MemoryObject { extent: Some(4), ..object(MemoryKind::Frame) };
        let bounded = Provenance::one_with_slice(bounded_object, 0, 4, 1, 1, BTreeSet::new()).unwrap();
        assert_eq!(bounded.shifted(8), bounded);
    }

    #[test]
    fn union_is_order_independent_and_iterates_canonically() {
        let left = Provenance::one_with_slice(object(MemoryKind::Global), 0, 1, 1, 1, BTreeSet::new()).unwrap();
        let right = Provenance::one_with_slice(object(MemoryKind::External), 4, 5, 1, 1, BTreeSet::new()).unwrap();

        let forward = left.union(&right);
        let reverse = right.union(&left);
        assert_eq!(forward, reverse);
        assert_eq!(forward.slices.iter().collect::<Vec<_>>(), reverse.slices.iter().collect::<Vec<_>>());
    }

    /// The access a load of an array element through a far segment makes:
    /// its root, displacement, index and selector.
    #[test]
    fn a_far_element_access_decomposes_to_its_segment_index_and_displacement() {
        let module = parsed(&format!(
            "{DOS}@g = global [8 x i16] zeroinitializer

define i16 @f(i16 %sel, i16 %i) {{
b0:
  %s = inttoptr i16 %sel to ptr addrspace(2)
  %far = addrspacecast ptr addrspace(2) %s to ptr addrspace(1)
  %e = getelementptr inbounds i16, ptr addrspace(1) %far, i16 %i
  %f = getelementptr inbounds i8, ptr addrspace(1) %e, i16 -2
  %x = load i16, ptr addrspace(1) %f
  %y = load i16, ptr getelementptr (i8, ptr @g, i16 6)
  %z = add i16 %x, %y
  ret i16 %z
}}
"
        ));
        let layout = layout(&module);
        let f = function(&module, "f");
        let unit = Unit::of(&module, &layout, f).with_spaces(llrm_x86_m16::spaces());
        let loads = f.walk().map(|(_, inst)| inst).filter_map(|inst| MemRef::of(&unit, inst)).collect::<Vec<_>>();

        let far = &loads[0];
        assert_eq!(
            (far.root, far.disp, far.base, far.scale),
            (Some(Operand::Value(value(f, "far"))), -2, Some(value(f, "i")), 2)
        );
        assert_eq!(far.segment, Some(Operand::Value(value(f, "sel"))));
        assert!(!far.object && far.inbounds && far.addr().is_none());
        let fixed = &loads[1];
        assert_eq!((fixed.disp, fixed.base, fixed.width), (6, None, 2));
        assert!(fixed.object && fixed.addr().is_some());
    }
}
