//! Machine-independent memory objects and byte-accurate access paths:
//! llrm-core's `model/memory.rs` (`Kind`, `Object`, `Slice`, `Provenance`,
//! `objects_may_alias`), and the access its analyses read, `MemRef`, adapted
//! to the rich MIR. Slices are half-open; a stride greater than one
//! describes selected byte lanes, not merely their hull.
//!
//! An object is what LLVM calls an underlying object. The rich MIR makes
//! these kinds: `Frame` (an alloca), `Global` (a global variable),
//! `Parameter` (what a pointer parameter points to), `Unknown`, `Nonlocal`
//! (what a callee may reach) and `Absolute` (linear memory; see
//! `regions::linear`). Kept for `classes_may_alias`, with no rich MIR
//! producer: `Stack` (the push area), `External` (a symbol the linker may
//! share: distinct rich MIR globals never overlap), `Named` (a far named
//! segment) and `Allocation` (a descriptor's heap block: the rich MIR marks
//! it on the access instead, as the `!tbaa` type "allocation").
//!
//! A global is `captured` unless the unit's GlobalsAA tracks it: the old
//! raise's private segments.
//!
//! Old `MemRef` spelled an address as an x86 operand -- a space, a segment
//! index, a displacement and a register base -- with an SSA `base` and
//! `segment` beside it. Here it is its pointer decomposed, as BasicAA's
//! `DecomposeGEPExpression` does: a root, a constant displacement, and at
//! most one variable index. Old concepts with no rich MIR counterpart are
//! not carried: `symbolic` (a second spelling of an address), `beyond`,
//! `excludes` and `within` (holes in the x86 region lattice), `allocation`
//! (the `!tbaa` type now), `pointer` (an access whose address was a bare
//! value: every access here is a decomposed pointer), `published`, `origin`.
//!
//! Skipped: the Python `repr` of each type, which ordered a call's effects;
//! they sort as the types order.

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
    /// Structural comparisons of objects, for the test that sets of slices make none.
    pub static OBJECT_COMPARES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
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

/// A `MemoryObject` as its module's interner numbered it: a small id compared, hashed and ordered as an integer, with
/// the fields the passes ask of every slice beside it, so a slice is `Copy` and asking costs nothing. The rest (the
/// identity, the generation) is `ObjectInterner::object`. Interned by every field, the facts too, so two spellings that
/// differ in a fact are two objects here and a set keeps both (none was seen to differ in 1,700 programs and QCport).
/// The order is the order of first interning in the module, which the same module always makes the same way; nothing
/// may depend on it being `MemoryObject`'s.
#[derive(Clone, Copy, Debug)]
pub struct ObjectRef {
    id: u32,
    pub kind: MemoryKind,
    pub addressed: bool,
    pub captured: bool,
    pub constant: bool,
    pub extent: Option<i64>,
    /// The identity where it is a number, which is all the passes make: a name or a tuple (a test's, the linear
    /// region's) is `Other`, and `ObjectInterner::object` has it whole.
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
    /// Every interner starts with these three, so they are the same in all modules and in the statics.
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
            && self.0.addressed == other.0.addressed
            && self.0.captured == other.0.captured
            && self.0.constant == other.0.constant
    }
}

impl Eq for Exact {}

impl std::hash::Hash for Exact {
    fn hash<H: std::hash::Hasher>(
        &self,
        state: &mut H,
    ) {
        self.0.hash(state);
        (self.0.addressed, self.0.captured, self.0.constant).hash(state);
    }
}

/// A module's memory objects, numbered densely in the order they are first asked for. It lives in the module's
/// `Context` (`ObjectInterner::of`) and is dropped with it, as LLVMContext's uniqued constants are.
pub struct ObjectInterner {
    held: std::cell::RefCell<(Vec<MemoryObject>, llrm_support::hash::HashMap<Exact, u32>)>,
    lookups: std::cell::Cell<u64>,
}

impl Default for ObjectInterner {
    fn default() -> Self {
        let interner = Self { held: Default::default(), lookups: Default::default() };
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
        let mut held = self.held.borrow_mut();
        let (objects, ids) = &mut *held;
        let key = Exact(object);
        let id = match ids.get(&key) {
            Some(&id) => id,
            None => {
                let id = objects.len() as u32;
                objects.push(key.0.clone());
                ids.insert(key, id);
                id
            }
        };
        let one = &objects[id as usize];
        ObjectRef {
            id,
            kind: one.kind,
            addressed: one.addressed,
            captured: one.captured,
            constant: one.constant,
            extent: one.extent,
            key: Key::of(&one.identity),
        }
    }

    /// How many times `intern` was asked: a cost counter, for tests that a hot loop does not ask per step.
    pub fn lookups(&self) -> u64 {
        self.lookups.get()
    }

    /// The object `one` stands for.
    pub fn object(
        &self,
        one: ObjectRef,
    ) -> MemoryObject {
        self.held.borrow().0[one.id as usize].clone()
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

/// Python `qbopt.model.memory:Slice`.
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

/// Python `qbopt.model.memory:Provenance`. `restrict` holds the `noalias`
/// parameters a pointer is based on.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Provenance {
    pub slices: BTreeSet<Slice>,
    pub restrict: BTreeSet<Identity>,
}

impl Provenance {
    /// `Provenance.one` with the whole-object bounds and no restrict roots.
    pub fn one(object: impl Into<ObjectRef>) -> Self {
        Self { slices: BTreeSet::from([Slice::whole(object)]), restrict: BTreeSet::new() }
    }

    pub fn one_with_slice(
        object: impl Into<ObjectRef>,
        low: i64,
        high: i64,
        stride: i64,
        width: i64,
        restrict: BTreeSet<Identity>,
    ) -> Result<Self, SliceError> {
        Ok(Self { slices: BTreeSet::from([Slice::new(object, low, high, stride, width)?]), restrict })
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
        Self {
            slices: self.slices.union(&other.slices).cloned().collect(),
            restrict: self.restrict.union(&other.restrict).cloned().collect(),
        }
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

/// Python `qbopt.model.memory:AliasClass`: all `objects_may_alias` asks of
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

/// What `objects_may_alias` asks of an object: a `MemoryObject` or its interned `ObjectRef`.
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
    /// Each loop's counted proofs under `registers`, the manager's `Counted`; without them each ask proves them.
    pub counted: Option<&'a crate::induction::Counted>,
    /// What the unavoidable branch edges bound at each block, under `registers`: the manager's `DominatedEdges`.
    pub edges: Option<&'a crate::ranges::EdgeStates>,
    /// What each counted loop bounds at each block, under `registers`: the manager's `Bounded`.
    pub bounds: Option<&'a crate::ranges::Bounds>,
    /// The allocas whose address is exposed, the manager's `ExposedFrames`; without it each ask scans
    /// the alloca's uses.
    pub exposed: Option<&'a BTreeSet<ValueId>>,
}

impl<'a> Unit<'a> {
    /// The intrinsic `inst` calls, if it calls one.
    pub fn intrinsic(
        &self,
        inst: InstId,
    ) -> Option<llrm_mir::intrinsics::Intrinsic> {
        let callee = llrm_mir::memory::callee(self.context, self.function, inst)?;
        llrm_mir::intrinsics::Intrinsic::named(self.globals.get(callee.0 as usize)?.name.as_deref()?)
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
        Self {
            program: None,
            spaces: llrm_mir::spaces::Spaces::FLAT,
            context: &module.context,
            layout,
            metadata: &module.metadata,
            tbaa: None,
            globals: &module.globals,
            function,
            globals_aa: None,
            references: None,
            shape: None,
            registers: None,
            pointers: None,
            annotated: None,
            assumptions: None,
            counted: None,
            edges: None,
            bounds: None,
            exposed: None,
        }
    }

    pub fn with_spaces(
        self,
        spaces: llrm_mir::spaces::Spaces,
    ) -> Self {
        Self { spaces, ..self }
    }

    pub fn with_exposed(
        self,
        exposed: &'a BTreeSet<ValueId>,
    ) -> Self {
        Self { exposed: Some(exposed), ..self }
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

    pub fn with_bounds(
        self,
        bounds: &'a crate::ranges::Bounds,
    ) -> Self {
        Self { bounds: Some(bounds), ..self }
    }

    pub fn with_edges(
        self,
        edges: &'a crate::ranges::EdgeStates,
    ) -> Self {
        Self { edges: Some(edges), ..self }
    }

    pub fn with_counted(
        self,
        counted: &'a crate::induction::Counted,
    ) -> Self {
        Self { counted: Some(counted), ..self }
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
    /// A unit that carries none was made without the manager, and deriving them here would be a second derivation of
    /// a fact the manager holds (or a stale copy of it): asking is a bug. A caller over a body the manager has not
    /// seen states what it computes with `with_registers`.
    pub fn registers(&self) -> Cow<'a, IndexMap<ValueId, Known>> {
        match self.registers {
            Some(registers) => {
                if llrm_support::env_set("LLRM_CHECK_FACTS") {
                    let fresh = crate::consts::known(&Unit { registers: None, ..*self }, None, None, None);
                    assert!(
                        *registers == fresh,
                        "the registers a unit carries are not those of the body it stands over: stale"
                    );
                }
                Cow::Borrowed(registers)
            }
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
                if llrm_support::env_set("LLRM_CHECK_SHAPE") {
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

    /// The address spaces by role: the program's target's, or one flat space where none is named.
    pub fn spaces(&self) -> llrm_mir::spaces::Spaces {
        self.program.map_or(self.spaces, |program| program.target.spaces())
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
        match self.function.value(value).def {
            ValueDef::Instruction(inst) => Some((inst, self.function.instruction(inst))),
            ValueDef::Argument(_) => None,
        }
    }
}

/// Whether `inst` marks an object's lifetime, which names it without handing out its address.
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

/// What is known of a body without memory (`consts::known`), for a caller that changes the body as it goes: the
/// manager's where the body is as the manager saw it, derived again, once for each state, once it is not. The one
/// place a unit's registers are derived outside the manager.
pub struct Standing<'h> {
    held: Option<&'h Knowns>,
    /// What the counted loops bound, held for the same body as `held`, where the caller has it.
    bounds: Option<&'h crate::ranges::Bounds>,
    derived: Option<Knowns>,
}

impl<'h> Standing<'h> {
    /// The body is as `registers` were found of it.
    pub fn held(registers: &'h Knowns) -> Self {
        Self { held: Some(registers), bounds: None, derived: None }
    }

    /// `held`, and the bounds the manager found of the same body.
    pub fn held_with(
        registers: &'h Knowns,
        bounds: &'h crate::ranges::Bounds,
    ) -> Self {
        Self { held: Some(registers), bounds: Some(bounds), derived: None }
    }

    /// No one has found them: derived when first asked.
    pub fn underived() -> Self {
        Self { held: None, bounds: None, derived: None }
    }

    /// The body changed: what was found of it no longer holds.
    pub fn changed(&mut self) {
        self.held = None;
        self.bounds = None;
        self.derived = None;
    }

    /// `of`, and the manager's bounds of the body where it is still as they were found of it.
    pub fn of_with_bounds(
        &mut self,
        unit: &Unit,
    ) -> (&Knowns, Option<&'h crate::ranges::Bounds>) {
        let bounds = self.bounds;
        (self.of(unit), bounds)
    }

    /// What is known of `unit`'s body as it stands, which it must be the one these were asked of.
    pub fn of(
        &mut self,
        unit: &Unit,
    ) -> &Knowns {
        if let Some(held) = self.held {
            return held;
        }
        self.derived.get_or_insert_with(|| crate::consts::known(&Unit { registers: None, ..*unit }, None, None, None))
    }
}

/// The allocas of `unit`'s function whose address is exposed, in one pass: what `object_of` reads of
/// `Unit::exposed` instead of asking each alloca's uses.
pub fn exposed_frames(unit: &Unit) -> BTreeSet<ValueId> {
    crate::frameescape::exposed_allocas(unit.function, |inst| is_lifetime_marker(unit, inst))
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
                None => crate::frameescape::exposes(unit.function, value, |inst| is_lifetime_marker(unit, inst)),
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

impl MemRef {
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
        loop {
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
        }
        made.object = object_of(unit, root).is_some();
        made
    }

    /// The access instruction `inst` makes: a load's or a store's.
    pub fn of(
        unit: &Unit,
        inst: InstId,
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
        Some(Self {
            typed: typed(unit, inst),
            lineage: lineage(unit, inst),
            volatile,
            ..Self::at(unit, pointer, width)
        })
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
        let name = unit.globals.get(callee.0 as usize)?.name.as_deref()?;
        let cell = match Intrinsic::named(name)? {
            Intrinsic::MemSet => 1,
            Intrinsic::MemSetPattern => unit.int_bits(instruction.operands[1])? / 8,
            _ => return None,
        };
        let width = u32::try_from(unit.int_constant(instruction.operands[2])?)
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
            ConstantKind::Expr(ConstantExpr::GetElementPtr { source, inbounds, operands }) => {
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

/// The selector `root` is `segment:0` of: the integer an `inttoptr` made
/// the segment from, or the segment itself.
fn segment(
    unit: &Unit,
    root: Operand,
) -> Option<Operand> {
    let (_, instruction) = unit.defining(root)?;
    if !matches!(instruction.opcode, Opcode::Cast(CastOp::AddrSpaceCast)) {
        return None;
    }
    let from = instruction.operands[0];
    if unit.space(from) != Some(2) {
        return None;
    }
    match unit.defining(from) {
        Some((_, made)) if matches!(made.opcode, Opcode::Cast(CastOp::IntToPtr)) => Some(made.operands[0]),
        _ => Some(from),
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

/// The name of the `!tbaa` access type `inst` carries.
pub fn typed(
    unit: &Unit,
    inst: InstId,
) -> Option<std::rc::Rc<str>> {
    let (_, tag) = unit.function.instruction(inst).metadata.iter().find(|(kind, _)| kind == "tbaa")?;
    if let Some(tree) = unit.tbaa {
        // A type's name read through the tree is the type node's first operand, which is the name below.
        return tree.name_of_tag(unit.metadata, *tag);
    }
    let MetadataOperand::Node(ty) = unit.metadata.get(tag.0 as usize)?.operands.first()? else { return None };
    match unit.metadata.get(ty.0 as usize)?.operands.first()? {
        MetadataOperand::String(name) => Some(std::rc::Rc::from(name.as_str())),
        _ => None,
    }
}

/// The names of the ancestors of the `!tbaa` access type `inst` carries,
/// nearest first, the root last: the module's type tree where the unit holds
/// it, else built here from the metadata.
pub fn lineage(
    unit: &Unit,
    inst: InstId,
) -> std::rc::Rc<[String]> {
    let Some((_, tag)) = unit.function.instruction(inst).metadata.iter().find(|(kind, _)| kind == "tbaa") else {
        return no_lineage();
    };
    match unit.tbaa {
        Some(tree) => tree.shared_of_tag(unit.metadata, *tag),
        None => std::rc::Rc::from(llrm_mir::tbaa::Tbaa::chain(unit.metadata, *tag)),
    }
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

    /// Isel's unit named no program, so the spaces became one flat space: a QB program's
    /// fixed-address pokes (`DEF SEG`) were no longer apart from every object and demo-qbdemo
    /// grew 6 bytes. A unit asks the spaces it was given when no program names the target.
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

    /// A set of slices compared the objects' identities (strings, tuples) tree against tree at every step of every
    /// insert, and cloned them: `BTreeSet<Slice>` was 10% of host.c's compile, `MemoryObject::cmp` 8%, malloc 10%.
    /// An object is a number once interned: building, probing and cloning sets of slices compares none
    /// structurally.
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

    /// Two spellings of an object that differ in a fact are two objects here, and one spelling is one object however
    /// often it is interned.
    #[test]
    fn an_object_is_interned_by_every_field_once() {
        let global = MemoryObject { identity: Some(Identity::Global(9)), ..object(MemoryKind::Global) };
        let (one, again) = (super::ObjectRef::from(global.clone()), super::ObjectRef::from(global.clone()));
        assert_eq!(one, again);
        assert_eq!(one.id(), again.id());
        let private = super::ObjectRef::from(MemoryObject { captured: false, ..global });
        assert_ne!(one, private);
    }

    /// A process-wide interner numbered objects by everything interned before them in the process: a test thread, a
    /// compile before this one, the LSP server's last hour. A module's ids are the module's: the same objects asked
    /// in the same order get the same ids whichever module was done first.
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

    /// The interner lives in the module's context and dies with it: a server compiling a file again and again held
    /// every object of every compile before.
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
        // Python since 8780f59b: the push area and the frame are distinct objects.
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
