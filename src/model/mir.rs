//! Values and source-neutral MIR operands.
//!
//! Direct port of the primitive definitions in `qbopt/model/mir.py`, from
//! `Value` through `Kind`, stopping before `FloatingOrigin` and `Op`.

use std::collections::BTreeSet;
use std::fmt;
use std::hash::{Hash, Hasher};

use crate::codegen::machine::Loc;
use crate::model::memory::Provenance;
use crate::object::omf::module::{Addr, Space};

/// One SSA variable, deliberately with no register or historical home.
///
/// Direct port of `qbopt.model.mir:Value` and `Value.__repr__`.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Value {
    pub id: u32,
    pub at: u32,
    pub flags: bool,
    pub variable: u32,
    pub version: u32,
}

impl Value {
    pub const fn new(id: u32, at: u32) -> Self {
        Self {
            id,
            at,
            flags: false,
            variable: 0,
            version: 0,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = if self.flags { 'f' } else { 'v' };
        if self.version == 0 {
            write!(formatter, "{kind}{}", self.id)
        } else {
            write!(formatter, "{kind}{}_{}", self.variable, self.version)
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

/// Operations no single machine instruction computes.
///
/// Direct port of `qbopt.model.mir:Synth`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Synth {
    HalfToLow,
    ConcatLow,
}

impl Synth {
    pub const ALL: [Self; 2] = [Self::HalfToLow, Self::ConcatLow];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HalfToLow => "half.tolow",
            Self::ConcatLow => "concat.low",
        }
    }
}

impl fmt::Display for Synth {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A call's bounded reach: the escaped segment and the exact cells handed
/// out in it.  Direct port of `raising_call_memory:Reach`.
pub type Reach = (u32, BTreeSet<(i64, i64)>);

/// A memory operand and the values its address depends on.
///
/// Direct port of `qbopt.model.mir:MemRef`.  `typed` and `within` are
/// intentionally excluded from equality and hashing, matching Python's
/// `field(compare=False)` exactly; every other field participates.
#[derive(Clone, Debug)]
pub struct MemRef {
    pub addr: Option<Addr>,
    pub width: u32,
    pub base: Option<Value>,
    pub segment: Option<Value>,
    pub space: Option<Space>,
    pub beyond: Option<Reach>,
    pub symbolic: Option<Symbol>,
    pub allocation: Option<Symbol>,
    pub base_width: u32,
    pub pointer: bool,
    pub excludes: Vec<(Addr, u32)>,
    pub typed: Option<(String, bool)>,
    pub within: Option<Vec<(i64, i64)>>,
    pub provenance: Option<Provenance>,
    pub volatile: bool,
}

impl MemRef {
    pub fn new(addr: Option<Addr>, width: u32) -> Self {
        Self {
            addr,
            width,
            base: None,
            segment: None,
            space: None,
            beyond: None,
            symbolic: None,
            allocation: None,
            base_width: 4,
            pointer: false,
            excludes: Vec::new(),
            typed: None,
            within: None,
            provenance: None,
            volatile: false,
        }
    }

    /// Direct port of `MemRef.where`.  `where` is reserved in Rust.
    pub const fn where_(&self) -> Option<Space> {
        match self.addr {
            Some(addr) => Some(addr.space),
            None => self.space,
        }
    }
}

impl PartialEq for MemRef {
    fn eq(&self, other: &Self) -> bool {
        self.addr == other.addr
            && self.width == other.width
            && self.base == other.base
            && self.segment == other.segment
            && self.space == other.space
            && self.beyond == other.beyond
            && self.symbolic == other.symbolic
            && self.allocation == other.allocation
            && self.base_width == other.base_width
            && self.pointer == other.pointer
            && self.excludes == other.excludes
            && self.provenance == other.provenance
            && self.volatile == other.volatile
    }
}

impl Eq for MemRef {}

impl Hash for MemRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.addr.hash(state);
        self.width.hash(state);
        self.base.hash(state);
        self.segment.hash(state);
        self.space.hash(state);
        self.beyond.hash(state);
        self.symbolic.hash(state);
        self.allocation.hash(state);
        self.base_width.hash(state);
        self.pointer.hash(state);
        self.excludes.hash(state);
        self.provenance.hash(state);
        self.volatile.hash(state);
    }
}

/// A value held at the width an operation uses it.
///
/// Direct port of `qbopt.model.mir:Held`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Held {
    pub value: Value,
    pub width: u32,
}

/// A literal signed as an operation means it.
///
/// Direct port of `qbopt.model.mir:Const`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Const {
    pub n: i64,
    pub width: u32,
}

/// Direct port of `qbopt.model.mir:Symbol`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Symbol {
    pub space: Space,
    pub index: u32,
    pub offset: i64,
    pub width: u32,
    pub addend: i64,
}

impl Symbol {
    pub const fn new(space: Space, index: u32, offset: i64, width: u32) -> Self {
        Self {
            space,
            index,
            offset,
            width,
            addend: 0,
        }
    }
}

/// Direct port of `qbopt.model.mir:FrameAddress`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FrameAddress {
    pub offset: i64,
    pub width: u32,
    pub extent: Option<(i64, i64)>,
}

impl FrameAddress {
    pub const fn new(offset: i64, width: u32) -> Self {
        Self {
            offset,
            width,
            extent: None,
        }
    }
}

/// Direct port of `qbopt.model.mir:ArrayRequest`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArrayRequest {
    pub descriptor: Symbol,
    pub element_width: u32,
    pub bounds: Vec<(i64, i64)>,
    pub replaces: bool,
}

impl ArrayRequest {
    /// Python's three-argument construction, whose `replaces` default is
    /// `False`.
    pub fn new(descriptor: Symbol, element_width: u32, bounds: Vec<(i64, i64)>) -> Self {
        Self {
            descriptor,
            element_width,
            bounds,
            replaces: false,
        }
    }
}

/// A memory cell -- the same `MemRef` an operation's loads and stores name.
/// Direct port of `qbopt.model.mir:Cell`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Cell {
    pub r#ref: MemRef,
}

/// A named primitive resource which MIR has no SSA value for.
///
/// `what` is the direct, closed port of Python's `ir.Loc | None`, carried
/// only to the sanctioned lowering boundary. Passes may inspect `name`, not
/// `what`.
/// Direct port of `qbopt.model.mir:Opaque`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Opaque {
    what: Option<Loc>,
    pub name: String,
}

impl Opaque {
    /// Python's one-argument construction, whose `name` default is empty.
    pub fn new(what: Option<Loc>) -> Self {
        Self {
            what,
            name: String::new(),
        }
    }

    pub fn named(what: Option<Loc>, name: impl Into<String>) -> Self {
        Self {
            what,
            name: name.into(),
        }
    }

    /// Historical machine payload for the lowering boundary only.
    pub(crate) const fn machine_payload(&self) -> Option<&Loc> {
        self.what.as_ref()
    }
}

/// Direct port of `qbopt.model.mir:Arg`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Arg {
    Held(Held),
    Const(Const),
    Symbol(Symbol),
    FrameAddress(FrameAddress),
    Cell(Cell),
    Opaque(Opaque),
}

/// Direct port of `qbopt.model.mir:same_bytes`.
///
/// This is the forwarding question: both references must certainly name the
/// same bytes. Its negation is not a disjointness proof.
pub fn same_bytes(one: &MemRef, other: &MemRef) -> bool {
    if matches!(
        (&one.provenance, &other.provenance),
        (Some(one), Some(other)) if one != other
    ) {
        return false;
    }
    if one.pointer || other.pointer {
        return one.pointer
            && other.pointer
            && one.base.is_some()
            && one.base == other.base
            && one.width == other.width
            && one.addr.is_none()
            && other.addr.is_none()
            && one.segment.is_none()
            && other.segment.is_none()
            && one.base_width == other.base_width;
    }

    let one = symbolic_ref(one);
    let other = symbolic_ref(other);
    let (Some(one_addr), Some(other_addr)) = (one.addr, other.addr) else {
        return false;
    };
    if one_addr.space == Space::Far
        && one.segment.is_none()
        && !(one.allocation.is_some() && one.allocation == other.allocation)
    {
        return false;
    }
    if one.width != other.width || one.base != other.base || one.segment != other.segment {
        return false;
    }
    one_addr == other_addr
}

fn symbolic_ref(reference: &MemRef) -> MemRef {
    let Some(symbol) = reference.symbolic else {
        return reference.clone();
    };
    let mut resolved = reference.clone();
    let mut address = Addr::new(symbol.space, symbol.offset + symbol.addend);
    address.index = symbol.index;
    resolved.addr = Some(address);
    resolved.base = None;
    resolved.segment = None;
    resolved
}

/// What an operation computes in MIR terms.
///
/// Direct port of `qbopt.model.mir:Kind`, stopping before operation-bearing
/// structures.  This vocabulary names no instruction, register, or target.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Kind {
    Add,
    Sub,
    AddCarry,
    SubBorrow,
    Increment,
    Decrement,
    Mul,
    Smulhi,
    Div,
    Rem,
    Divmod,
    Udivmod,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Sar,
    Neg,
    Not,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    Below,
    BelowEq,
    Above,
    AboveEq,
    Copy,
    Load,
    Store,
    Convert,
    SignExtend,
    ZeroExtend,
    Address,
    PtrOffset,
    Fill,
    Call,
    Branch,
    Switch,
    Jump,
    Return,
    Escape,
    Fadd,
    Fsub,
    Fmul,
    Fdiv,
    Fneg,
    Fabs,
    Fsqrt,
    Fload,
    Fstore,
    Fcompare,
    Fcheck,
    Arg,
    Result,
    Join,
    Extract,
    Concat,
    Opaque,
    Nothing,
}

impl Kind {
    pub const ALL: [Self; 63] = [
        Self::Add,
        Self::Sub,
        Self::AddCarry,
        Self::SubBorrow,
        Self::Increment,
        Self::Decrement,
        Self::Mul,
        Self::Smulhi,
        Self::Div,
        Self::Rem,
        Self::Divmod,
        Self::Udivmod,
        Self::And,
        Self::Or,
        Self::Xor,
        Self::Shl,
        Self::Shr,
        Self::Sar,
        Self::Neg,
        Self::Not,
        Self::Lt,
        Self::Le,
        Self::Gt,
        Self::Ge,
        Self::Eq,
        Self::Ne,
        Self::Below,
        Self::BelowEq,
        Self::Above,
        Self::AboveEq,
        Self::Copy,
        Self::Load,
        Self::Store,
        Self::Convert,
        Self::SignExtend,
        Self::ZeroExtend,
        Self::Address,
        Self::PtrOffset,
        Self::Fill,
        Self::Call,
        Self::Branch,
        Self::Switch,
        Self::Jump,
        Self::Return,
        Self::Escape,
        Self::Fadd,
        Self::Fsub,
        Self::Fmul,
        Self::Fdiv,
        Self::Fneg,
        Self::Fabs,
        Self::Fsqrt,
        Self::Fload,
        Self::Fstore,
        Self::Fcompare,
        Self::Fcheck,
        Self::Arg,
        Self::Result,
        Self::Join,
        Self::Extract,
        Self::Concat,
        Self::Opaque,
        Self::Nothing,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Sub => "sub",
            Self::AddCarry => "addcarry",
            Self::SubBorrow => "subborrow",
            Self::Increment => "increment",
            Self::Decrement => "decrement",
            Self::Mul => "mul",
            Self::Smulhi => "smulhi",
            Self::Div => "div",
            Self::Rem => "rem",
            Self::Divmod => "divmod",
            Self::Udivmod => "udivmod",
            Self::And => "and",
            Self::Or => "or",
            Self::Xor => "xor",
            Self::Shl => "shl",
            Self::Shr => "shr",
            Self::Sar => "sar",
            Self::Neg => "neg",
            Self::Not => "not",
            Self::Lt => "lt",
            Self::Le => "le",
            Self::Gt => "gt",
            Self::Ge => "ge",
            Self::Eq => "eq",
            Self::Ne => "ne",
            Self::Below => "below",
            Self::BelowEq => "beloweq",
            Self::Above => "above",
            Self::AboveEq => "aboveeq",
            Self::Copy => "copy",
            Self::Load => "load",
            Self::Store => "store",
            Self::Convert => "convert",
            Self::SignExtend => "sign_extend",
            Self::ZeroExtend => "zero_extend",
            Self::Address => "address",
            Self::PtrOffset => "ptr_offset",
            Self::Fill => "fill",
            Self::Call => "call",
            Self::Branch => "branch",
            Self::Switch => "switch",
            Self::Jump => "jump",
            Self::Return => "return",
            Self::Escape => "escape",
            Self::Fadd => "fadd",
            Self::Fsub => "fsub",
            Self::Fmul => "fmul",
            Self::Fdiv => "fdiv",
            Self::Fneg => "fneg",
            Self::Fabs => "fabs",
            Self::Fsqrt => "fsqrt",
            Self::Fload => "fload",
            Self::Fstore => "fstore",
            Self::Fcompare => "fcompare",
            Self::Fcheck => "fcheck",
            Self::Arg => "arg",
            Self::Result => "result",
            Self::Join => "join",
            Self::Extract => "extract",
            Self::Concat => "concat",
            Self::Opaque => "opaque",
            Self::Nothing => "nothing",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use crate::model::memory::{MemoryKind, MemoryObject, ObjectIdentity, ObjectTag, Provenance};
    use crate::object::omf::module::{Addr, Space};

    use super::{ArrayRequest, Cell, Kind, MemRef, Opaque, Symbol, Synth, Value, same_bytes};

    #[test]
    fn a_value_carries_no_register() {
        let made = Value {
            id: 1,
            at: 0,
            flags: false,
            variable: 3,
            version: 7,
        };
        assert_eq!(format!("{made:?}"), "v3_7");
        assert_eq!(format!("{:?}", Value::new(4, 0)), "v4");
        assert_eq!(
            format!(
                "{:?}",
                Value {
                    flags: true,
                    ..Value::new(4, 0)
                }
            ),
            "f4"
        );
    }

    #[test]
    fn mir_kind_and_synth_keep_their_python_spellings() {
        assert_eq!(Synth::ALL.map(Synth::as_str), ["half.tolow", "concat.low"]);
        assert_eq!(
            Kind::ALL.map(Kind::as_str),
            [
                "add",
                "sub",
                "addcarry",
                "subborrow",
                "increment",
                "decrement",
                "mul",
                "smulhi",
                "div",
                "rem",
                "divmod",
                "udivmod",
                "and",
                "or",
                "xor",
                "shl",
                "shr",
                "sar",
                "neg",
                "not",
                "lt",
                "le",
                "gt",
                "ge",
                "eq",
                "ne",
                "below",
                "beloweq",
                "above",
                "aboveeq",
                "copy",
                "load",
                "store",
                "convert",
                "sign_extend",
                "zero_extend",
                "address",
                "ptr_offset",
                "fill",
                "call",
                "branch",
                "switch",
                "jump",
                "return",
                "escape",
                "fadd",
                "fsub",
                "fmul",
                "fdiv",
                "fneg",
                "fabs",
                "fsqrt",
                "fload",
                "fstore",
                "fcompare",
                "fcheck",
                "arg",
                "result",
                "join",
                "extract",
                "concat",
                "opaque",
                "nothing",
            ]
        );
    }

    #[test]
    fn python_record_defaults_remain_available() {
        let descriptor = Symbol::new(Space::Segment, 5, 6, 2);
        assert!(!ArrayRequest::new(descriptor, 2, vec![(-3, 2)]).replaces);

        let opaque = Opaque::new(None);
        assert_eq!(opaque.name, "");
        assert_eq!(opaque.machine_payload(), None);
        let named = Opaque::named(None, "st0");
        assert_eq!(named.name, "st0");
        assert_eq!(named.machine_payload(), None);

        let cell = Cell {
            r#ref: MemRef::new(None, 2),
        };
        assert_eq!(cell.r#ref.width, 2);
    }

    #[test]
    fn memref_without_an_address_has_no_known_space() {
        let unknown = MemRef::new(None, 2);
        assert_eq!(unknown.where_(), None);
        assert_eq!(unknown.addr, None);
        assert!(!same_bytes(&unknown, &unknown));
    }

    #[test]
    fn rewritten_address_value_is_not_the_same_bytes() {
        // Direct port of
        // tests/test_mir.py::test_the_same_address_through_a_rewritten_register_is_not_the_same_bytes.
        let address = Addr::new(Space::Far, 0);
        let mut here = MemRef::new(Some(address), 1);
        here.base = Some(Value::new(41, 0x461));
        here.segment = Some(Value::new(50, 0));
        let mut there = here.clone();
        there.base = Some(Value::new(47, 0x476));

        assert_eq!(here.addr, there.addr);
        assert!(!same_bytes(&here, &there));
        assert!(same_bytes(&here, &here));
    }

    #[test]
    fn whole_pointer_same_bytes_requires_the_same_complete_identity() {
        // The `same_bytes` half of
        // tests/test_pointer_memory.py::test_pointer_identity_is_not_a_disjointness_proof.
        let mut reference = MemRef::new(None, 2);
        reference.base = Some(Value::new(1, 0));
        reference.pointer = true;
        assert!(same_bytes(&reference, &reference));

        let mut other = reference.clone();
        other.base = Some(Value::new(3, 0));
        assert!(!same_bytes(&reference, &other));
        other = reference.clone();
        other.pointer = false;
        assert!(!same_bytes(&reference, &other));
    }

    #[test]
    fn far_same_bytes_requires_a_segment_or_matching_allocation() {
        // Direct primitive port of tests/test_far_memory_identity.py.
        let mut unknown = MemRef::new(Some(Addr::new(Space::Far, 0)), 2);
        unknown.base = Some(Value::new(1, 0));
        assert!(!same_bytes(&unknown, &unknown));

        unknown.segment = Some(Value::new(3, 0));
        assert!(same_bytes(&unknown, &unknown));
        let mut changed = unknown.clone();
        changed.segment = Some(Value::new(3, 1));
        assert!(!same_bytes(&unknown, &changed));

        let allocation = Symbol::new(Space::Segment, 5, 6, 2);
        let mut proven = MemRef::new(Some(Addr::new(Space::Far, 0)), 2);
        proven.base = Some(Value::new(1, 0));
        proven.allocation = Some(allocation);
        assert!(same_bytes(&proven, &proven));
        let mut no_allocation = proven.clone();
        no_allocation.allocation = None;
        assert!(!same_bytes(&proven, &no_allocation));
        let mut other_allocation = proven.clone();
        other_allocation.allocation = Some(Symbol {
            offset: 32,
            ..allocation
        });
        assert!(!same_bytes(&proven, &other_allocation));
    }

    #[test]
    fn symbolic_reference_matches_its_direct_address() {
        // The primitive identity assertion from
        // tests/test_raising_arrays.py::test_descriptor_fields_have_proven_addresses_without_new_relocations.
        let symbol = Symbol::new(Space::Segment, 5, 8, 2);
        let mut symbolic = MemRef::new(Some(Addr::new(Space::Literal, 2)), 2);
        symbolic.base = Some(Value::new(1, 0));
        symbolic.symbolic = Some(symbol);
        let mut direct_address = Addr::new(Space::Segment, 8);
        direct_address.index = 5;
        let direct = MemRef::new(Some(direct_address), 2);

        assert!(same_bytes(&symbolic, &direct));
    }

    #[test]
    fn unequal_provenance_is_not_the_same_bytes() {
        let object = |index| MemoryObject {
            kind: MemoryKind::Global,
            identity: Some(ObjectIdentity::TaggedIndex {
                tag: ObjectTag::Seg,
                index,
            }),
            generation: 0,
            extent: Some(2),
        };
        let mut one = MemRef::new(Some(Addr::new(Space::Segment, 0)), 2);
        one.provenance = Some(Provenance::one(object(1)));
        let mut other = one.clone();
        other.provenance = Some(Provenance::one(object(2)));

        assert!(!same_bytes(&one, &other));
    }

    #[test]
    fn memref_where_prefers_the_address_space_and_excludes_typed_and_within_from_identity() {
        let mut left = MemRef::new(Some(Addr::new(Space::Segment, 8)), 2);
        left.space = Some(Space::Stack);
        left.typed = Some(("int4".to_owned(), false));
        left.within = Some(vec![(-8, -4)]);
        let mut right = left.clone();
        right.typed = Some(("float4".to_owned(), false));
        right.within = Some(vec![(-16, -8)]);

        assert_eq!(left.where_(), Some(Space::Segment));
        assert_eq!(left, right);
        let mut left_hash = DefaultHasher::new();
        let mut right_hash = DefaultHasher::new();
        left.hash(&mut left_hash);
        right.hash(&mut right_hash);
        assert_eq!(left_hash.finish(), right_hash.finish());

        right.volatile = true;
        assert_ne!(left, right);
        let mut changed_hash = DefaultHasher::new();
        right.hash(&mut changed_hash);
        assert_ne!(left_hash.finish(), changed_hash.finish());
    }
}
