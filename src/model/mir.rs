//! Values and source-neutral MIR operands.
//!
//! Direct port of the source-neutral MIR definitions in `qbopt/model/mir.py`.
//!
//! Raising, live-out materialization, decoded source occurrences, and source
//! maps remain deliberately deferred: `tests/test_mir.py`'s corpus raise and
//! form gates, `tests/test_rule5.py`'s production exit-liveness gate, and the
//! raising-copy provenance regressions belong to the raiser/liveness/source-
//! map ports, not to this public schema slice.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::hash::{Hash, Hasher};

use num_bigint::BigInt;

use crate::analysis::loops;
use crate::model::floating::Semantics as FloatingSemantics;
use crate::model::ir::{Loc, Operation, Semantics};
use crate::model::memory::Provenance;
use crate::objectfile::module::{Addr, Space};
use crate::support::pyrepr::{self, Repr};
use iced_x86::Register;

/// The registers that become values, rooted.
pub const TRACKED: [Register; 6] =
    [Register::EAX, Register::EBX, Register::ECX, Register::EDX, Register::ESI, Register::EDI];

/// One SSA variable, deliberately with no register or historical home.
///
/// Direct port of `qbopt.model.mir:Value` and `Value.__repr__`.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Value {
    pub id: u32,
    pub at: i64,
    pub flags: bool,
    pub variable: u32,
    pub version: u32,
}

impl Value {
    pub const fn new(id: u32, at: i64) -> Self {
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

/// A frozen dataclass hashes as the tuple of its fields.
impl crate::support::pyset::PyHash for Value {
    fn py_hash(&self) -> i64 {
        use crate::support::pyset::{int_hash, tuple_hash};
        tuple_hash(&[
            int_hash(i64::from(self.id)),
            int_hash(self.at),
            i64::from(self.flags),
            int_hash(i64::from(self.variable)),
            int_hash(i64::from(self.version)),
        ])
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

/// A frontend-established, non-wrapping mathematical integer range.
///
/// Direct port of `qbopt.model.mir:IntegerRange`.  This is source semantics,
/// not a target representation: the frontend translates its own ABI facts
/// into this source-neutral range before MIR analyses consume it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegerRange {
    pub low: BigInt,
    pub high: BigInt,
    pub width: u32,
}

impl IntegerRange {
    pub fn new(low: impl Into<BigInt>, high: impl Into<BigInt>, width: u32) -> Self {
        Self {
            low: low.into(),
            high: high.into(),
            width,
        }
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
pub type Reach = (i64, BTreeSet<(i64, i64)>);

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
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Const {
    pub n: BigInt,
    pub width: u32,
}

impl Const {
    pub fn new(n: impl Into<BigInt>, width: u32) -> Self {
        Self { n: n.into(), width }
    }
}

/// Direct port of `qbopt.model.mir:Symbol`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Symbol {
    pub space: Space,
    pub index: i64,
    pub offset: i64,
    pub width: u32,
    pub addend: i64,
}

impl Symbol {
    pub const fn new(space: Space, index: i64, offset: i64, width: u32) -> Self {
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

/// The run-time selector of the current activation's frame segment.
/// Direct port of `qbopt.model.mir:FrameSelector`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FrameSelector {
    pub width: u32,
}

impl Default for FrameSelector {
    fn default() -> Self {
        Self { width: 2 }
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
    #[allow(dead_code)] // The direct lowering port will be this method's first production caller.
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
    FrameSelector(FrameSelector),
    Cell(Cell),
    Opaque(Opaque),
}

/// What one selected node computes, as MIR says it.
///
/// Direct port of `qbopt.model.mir:_kind_of`.  The mnemonic is consulted
/// here and in the closed `BY_NAME` table only.  For moves, source and result
/// operands decide copy versus load versus store exactly as the Python port
/// does.
pub(crate) fn kind_of(what: &Semantics, args: &[Arg], results: &[Arg]) -> Kind {
    let name = what.name.as_deref().unwrap_or("");
    match what.op {
        Operation::Binary | Operation::Unary => by_name(name).unwrap_or(Kind::Opaque),
        Operation::Move => {
            if results.iter().any(|one| matches!(one, Arg::Cell(_))) {
                Kind::Store
            } else if args.iter().any(|one| matches!(one, Arg::Cell(_))) {
                Kind::Load
            } else {
                Kind::Copy
            }
        }
        Operation::Multiply => Kind::Mul,
        Operation::Divide => Kind::Div,
        Operation::Compare => by_name(name).unwrap_or(Kind::Sub),
        Operation::Extend => Kind::Convert,
        Operation::Address => Kind::Address,
        Operation::Push => Kind::Arg,
        Operation::Pop => Kind::Result,
        Operation::Jump => Kind::Jump,
        Operation::Branch => Kind::Branch,
        Operation::Call => Kind::Call,
        Operation::Return => Kind::Return,
        Operation::Escape => Kind::Escape,
        Operation::Nothing => Kind::Nothing,
        Operation::Restore => Kind::Join,
        Operation::FloatLoad => Kind::Fload,
        Operation::FloatStore => Kind::Fstore,
        Operation::FloatArith | Operation::FloatArithPop | Operation::FloatUnary => {
            by_name(name).unwrap_or(Kind::Opaque)
        }
        _ => Kind::Opaque,
    }
}

/// Direct port of `qbopt.model.mir:WHOLE_FRAME`: every BP-relative frame byte.
/// A flags value's tracked variable. Direct port of `mir.FLAGS`.
pub const FLAGS: Register = Register::None;

/// Which root a restore reads and which it writes the high half into, by
/// `ir.FIXUP`'s pair numbering. Direct port of `mir.RESTORE_PAIR`.
pub fn restore_pair(pair: i64) -> Option<(Register, Register)> {
    match pair {
        0 => Some((Register::EAX, Register::EDX)),
        1 => Some((Register::ECX, Register::EBX)),
        _ => None,
    }
}

/// Each contract register at its own name. Direct port of `mir.AS_NAMED`.
pub fn as_named(one: crate::abi::runtime::Reg) -> Option<Register> {
    use crate::abi::runtime::Reg;
    match one {
        Reg::Ax => Some(Register::AX),
        Reg::Bx => Some(Register::BX),
        Reg::Cx => Some(Register::CX),
        Reg::Dx => Some(Register::DX),
        Reg::Si => Some(Register::SI),
        Reg::Di => Some(Register::DI),
        Reg::Flags => Some(FLAGS),
        _ => None,
    }
}

pub const WHOLE_FRAME: (Addr, u32) = (Addr::new(Space::Frame, -(1 << 15)), 1 << 16);

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

/// Direct port of `qbopt.model.mir:_symbolic_ref`.
///
/// Analyses that need an address-based view of a reference must use this
/// normalization rather than spelling symbolic resolution themselves.
pub(crate) fn symbolic_ref(reference: &MemRef) -> MemRef {
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
    FixedMul,
    FixedDiv,
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
    pub const ALL: [Self; 65] = [
        Self::Add,
        Self::Sub,
        Self::AddCarry,
        Self::SubBorrow,
        Self::Increment,
        Self::Decrement,
        Self::Mul,
        Self::Smulhi,
        Self::FixedMul,
        Self::FixedDiv,
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
            Self::FixedMul => "fixed_mul",
            Self::FixedDiv => "fixed_div",
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

impl Kind {
    /// The member name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Add => "ADD",
            Self::Sub => "SUB",
            Self::AddCarry => "ADD_CARRY",
            Self::SubBorrow => "SUB_BORROW",
            Self::Increment => "INCREMENT",
            Self::Decrement => "DECREMENT",
            Self::Mul => "MUL",
            Self::Smulhi => "SMULHI",
            Self::FixedMul => "FIXED_MUL",
            Self::FixedDiv => "FIXED_DIV",
            Self::Div => "DIV",
            Self::Rem => "REM",
            Self::Divmod => "DIVMOD",
            Self::Udivmod => "UDIVMOD",
            Self::And => "AND",
            Self::Or => "OR",
            Self::Xor => "XOR",
            Self::Shl => "SHL",
            Self::Shr => "SHR",
            Self::Sar => "SAR",
            Self::Neg => "NEG",
            Self::Not => "NOT",
            Self::Lt => "LT",
            Self::Le => "LE",
            Self::Gt => "GT",
            Self::Ge => "GE",
            Self::Eq => "EQ",
            Self::Ne => "NE",
            Self::Below => "BELOW",
            Self::BelowEq => "BELOW_EQ",
            Self::Above => "ABOVE",
            Self::AboveEq => "ABOVE_EQ",
            Self::Copy => "COPY",
            Self::Load => "LOAD",
            Self::Store => "STORE",
            Self::Convert => "CONVERT",
            Self::SignExtend => "SIGN_EXTEND",
            Self::ZeroExtend => "ZERO_EXTEND",
            Self::Address => "ADDRESS",
            Self::PtrOffset => "PTR_OFFSET",
            Self::Fill => "FILL",
            Self::Call => "CALL",
            Self::Branch => "BRANCH",
            Self::Switch => "SWITCH",
            Self::Jump => "JUMP",
            Self::Return => "RETURN",
            Self::Escape => "ESCAPE",
            Self::Fadd => "FADD",
            Self::Fsub => "FSUB",
            Self::Fmul => "FMUL",
            Self::Fdiv => "FDIV",
            Self::Fneg => "FNEG",
            Self::Fabs => "FABS",
            Self::Fsqrt => "FSQRT",
            Self::Fload => "FLOAD",
            Self::Fstore => "FSTORE",
            Self::Fcompare => "FCOMPARE",
            Self::Fcheck => "FCHECK",
            Self::Arg => "ARG",
            Self::Result => "RESULT",
            Self::Join => "JOIN",
            Self::Extract => "EXTRACT",
            Self::Concat => "CONCAT",
            Self::Opaque => "OPAQUE",
            Self::Nothing => "NOTHING",
        }
    }
}

impl Repr for Kind {
    fn repr(&self) -> String {
        pyrepr::str_enum("Kind", self.name(), self.as_str())
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

// Direct port of `qbopt.model.mir:_BY_NAME`.  `_kind_of` is the one place
// which reads these selected mnemonic spellings; MIR itself carries `Kind`.
const BY_NAME: [(&str, Kind); 40] = [
    ("add", Kind::Add),
    ("adc", Kind::AddCarry),
    ("inc", Kind::Increment),
    ("sub", Kind::Sub),
    ("sbb", Kind::SubBorrow),
    ("dec", Kind::Decrement),
    ("cmp", Kind::Sub),
    ("and", Kind::And),
    ("test", Kind::And),
    ("or", Kind::Or),
    ("xor", Kind::Xor),
    ("not", Kind::Not),
    ("neg", Kind::Neg),
    ("shl", Kind::Shl),
    ("sal", Kind::Shl),
    ("shr", Kind::Shr),
    ("sar", Kind::Sar),
    ("imul", Kind::Mul),
    ("mul", Kind::Mul),
    ("idiv", Kind::Div),
    ("div", Kind::Div),
    ("fadd", Kind::Fadd),
    ("faddp", Kind::Fadd),
    ("fsub", Kind::Fsub),
    ("fsubp", Kind::Fsub),
    ("fsubr", Kind::Fsub),
    ("fsubrp", Kind::Fsub),
    ("fmul", Kind::Fmul),
    ("fmulp", Kind::Fmul),
    ("fdiv", Kind::Fdiv),
    ("fdivp", Kind::Fdiv),
    ("fdivr", Kind::Fdiv),
    ("fdivrp", Kind::Fdiv),
    ("fchs", Kind::Fneg),
    ("fabs", Kind::Fabs),
    ("fsqrt", Kind::Fsqrt),
    ("fcom", Kind::Fcompare),
    ("fcomp", Kind::Fcompare),
    ("fcompp", Kind::Fcompare),
    ("ftst", Kind::Fcompare),
];

fn by_name(name: &str) -> Option<Kind> {
    BY_NAME
        .iter()
        .find_map(|(spelling, kind)| (*spelling == name).then_some(*kind))
}

/// Python's ordered `dict`, with mapping equality.
///
/// `Phi.incoming` and `Op.merges` are dictionaries: equality does not depend
/// on insertion order, but iteration does.  A `BTreeMap` would silently sort
/// phi copies before lowering, while a bare `Vec` would make equality
/// order-sensitive.  This is the smallest local representation of that
/// Python contract.  Keys are unique; assigning an existing key replaces its
/// value without moving it, as Python `dict` does.
#[derive(Clone, Debug)]
pub struct OrderedMap<K, V> {
    entries: Vec<(K, V)>,
}

impl<K, V> OrderedMap<K, V> {
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&K, &V)> {
        self.entries.iter().map(|(key, value)| (key, value))
    }

    pub fn keys(&self) -> impl ExactSizeIterator<Item = &K> {
        self.entries.iter().map(|(key, _)| key)
    }

    pub fn values(&self) -> impl ExactSizeIterator<Item = &V> {
        self.entries.iter().map(|(_, value)| value)
    }
}

impl<K: Eq, V> OrderedMap<K, V> {
    pub fn get(&self, key: &K) -> Option<&V> {
        self.entries
            .iter()
            .find_map(|(candidate, value)| (candidate == key).then_some(value))
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if let Some((_, previous)) = self
            .entries
            .iter_mut()
            .find(|(candidate, _)| *candidate == key)
        {
            return Some(std::mem::replace(previous, value));
        }
        self.entries.push((key, value));
        None
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.entries
            .iter()
            .position(|(candidate, _)| candidate == key)
            .map(|index| self.entries.remove(index).1)
    }
}

impl<K, V> Default for OrderedMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Eq, V: PartialEq> PartialEq for OrderedMap<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .all(|(key, value)| other.get(key).is_some_and(|other| other == value))
    }
}

impl<K: Eq, V: Eq> Eq for OrderedMap<K, V> {}

impl<K: Eq, V> FromIterator<(K, V)> for OrderedMap<K, V> {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let mut result = Self::new();
        for (key, value) in iter {
            result.insert(key, value);
        }
        result
    }
}

/// Python's `ir.Operation | Synth` field on `Op`.
///
/// `Operation` is the already-landed direct port of
/// `qbopt.model.ir:Operation`; this wrapper represents only Python's sum
/// type and deliberately does not invent another operation vocabulary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OpCode {
    Operation(Operation),
    Synth(Synth),
}

impl OpCode {
    /// The inert source-ownership operation used by MIR transforms.
    pub(crate) const fn nothing() -> Self {
        Self::Operation(Operation::Nothing)
    }

    /// The synthetic control transfer introduced by MIR transforms.
    pub(crate) const fn jump() -> Self {
        Self::Operation(Operation::Jump)
    }
}

/// Lowering's identity baseline while general floating allocation is
/// unfinished.  Direct port of `qbopt.model.mir:FloatingOrigin`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FloatingOrigin {
    pub block: i64,
    pub sequence: Vec<i64>,
    pub at: i64,
    pub kind: Kind,
    pub semantics: FloatingSemantics,
    pub inputs: Vec<Arg>,
    pub outputs: Vec<Arg>,
    pub machine_inputs: Vec<Arg>,
    pub machine_outputs: Vec<Arg>,
}

/// One instruction, as values in and values out.
///
/// Direct port of `qbopt.model.mir:Op`.  Public MIR carries semantic values
/// and opaque source identities only: source nodes, byte ranges, historical
/// register homes, and pins stay outside this type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Op {
    pub at: i64,
    /// The Python model annotates this as `ir.Operation | Synth`, while its
    /// own model tests use `None` for operations whose machine spelling is
    /// deliberately irrelevant.  Preserve that accepted form explicitly.
    pub op: Option<OpCode>,
    pub name: String,
    pub defines: Vec<Value>,
    pub uses: Vec<Value>,
    pub array: Option<ArrayRequest>,
    pub memory_values: Vec<(MemRef, Const)>,
    pub floating: Option<FloatingSemantics>,
    pub floating_origin: Option<FloatingOrigin>,
    pub loads: Vec<MemRef>,
    pub stores: Vec<MemRef>,
    pub source_backed: bool,
    pub kind: Kind,
    pub stack: Option<i64>,
    pub test: Option<Kind>,
    pub merges: OrderedMap<Value, Value>,
    pub args: Vec<Arg>,
    pub results: Vec<Arg>,
    pub raised: Option<(Vec<Arg>, Vec<Arg>)>,
    pub target: Option<i64>,
    pub cases: Vec<(i64, i64)>,
    pub id: Option<u32>,
    /// `None` is an unchanged operand, `Some(true)` owns a moved relocation,
    /// and `Some(false)` left that relocation on another operation.
    pub symbol: Option<bool>,
    pub args_known: bool,
    pub memory_complete: bool,
    pub reads_complete: bool,
    pub volatile: bool,
    /// `None` means every opaque resource; `Some(empty)` means none.
    pub opaque_defs: Option<BTreeSet<String>>,
    /// `None` means every opaque resource; `Some(empty)` means none.
    pub opaque_uses: Option<BTreeSet<String>>,
    pub absorbed: Vec<u32>,
    pub indirect: bool,
    pub exits: Vec<Value>,
}

impl Op {
    /// Constructs Python's five-required-field `Op` form with every later
    /// field set to its dataclass default.
    pub fn new(
        at: i64,
        op: impl Into<Option<OpCode>>,
        name: impl Into<String>,
        defines: Vec<Value>,
        uses: Vec<Value>,
    ) -> Self {
        Self {
            at,
            op: op.into(),
            name: name.into(),
            defines,
            uses,
            array: None,
            memory_values: Vec::new(),
            floating: None,
            floating_origin: None,
            loads: Vec::new(),
            stores: Vec::new(),
            source_backed: false,
            kind: Kind::Opaque,
            stack: None,
            test: None,
            merges: OrderedMap::new(),
            args: Vec::new(),
            results: Vec::new(),
            raised: None,
            target: None,
            cases: Vec::new(),
            id: None,
            symbol: None,
            args_known: true,
            memory_complete: false,
            reads_complete: false,
            volatile: false,
            opaque_defs: Some(BTreeSet::new()),
            opaque_uses: Some(BTreeSet::new()),
            absorbed: Vec::new(),
            indirect: false,
            exits: Vec::new(),
        }
    }

    /// Python `Op.barrier`.
    pub const fn barrier(&self) -> bool {
        matches!(self.op, Some(OpCode::Operation(Operation::Barrier))) || self.volatile
    }

    /// Python `Op.inserted`: public MIR operations own no source occurrence
    /// precisely when their opaque ownership identity list is empty.
    pub fn inserted(&self) -> bool {
        self.absorbed.is_empty()
    }
}

/// A source-free MIR computation invented by a semantic transform.
///
/// Direct port of `qbopt.model.mir:computed`.  The operation names only its
/// MIR computation and operands; target selection remains below this layer.
pub(crate) fn computed(at: i64, kind: Kind, result: Value, args: Vec<Arg>, width: u32) -> Op {
    let loads = args
        .iter()
        .filter_map(|argument| match argument {
            Arg::Cell(cell) => Some(cell.r#ref.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut uses = Vec::new();
    for value in args.iter().filter_map(|argument| match argument {
        Arg::Held(held) => Some(held.value),
        _ => None,
    }) {
        if !uses.contains(&value) {
            uses.push(value);
        }
    }
    for value in loads
        .iter()
        .flat_map(|reference| [reference.base, reference.segment])
        .flatten()
    {
        if !uses.contains(&value) {
            uses.push(value);
        }
    }

    let mut operation = Op::new(at, OpCode::nothing(), "", vec![result], uses);
    operation.loads = loads;
    operation.kind = kind;
    operation.args = args;
    operation.results = vec![Arg::Held(Held {
        value: result,
        width,
    })];
    operation.symbol = Some(false);
    operation.memory_complete = true;
    operation.reads_complete = true;
    operation
}

/// Retains an occurrence's source ownership while deleting its meaning.
///
/// Direct port of `qbopt/model/mir.py:cleared`.
pub fn cleared(op: &Op) -> Op {
    let mut result = op.clone();
    result.kind = Kind::Nothing;
    result.name.clear();
    result.defines.clear();
    result.uses.clear();
    result.loads.clear();
    result.stores.clear();
    result.args.clear();
    result.results.clear();
    result.merges = OrderedMap::new();
    result.raised = None;
    result.target = None;
    result.test = None;
    result.stack = None;
    result.symbol = Some(false);
    result
}

/// Where definitions meet on CFG edges.  Direct port of `qbopt.model.mir:Phi`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Phi {
    pub result: Value,
    pub incoming: OrderedMap<i64, Value>,
}

impl Phi {
    pub fn new(result: Value) -> Self {
        Self {
            result,
            incoming: OrderedMap::new(),
        }
    }
}

/// One MIR CFG block.  Direct port of `qbopt.model.mir:MirBlock`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirBlock {
    pub at: i64,
    pub phis: Vec<Phi>,
    pub ops: Vec<Op>,
    pub succ: Vec<i64>,
}

impl MirBlock {
    pub fn new(at: i64, phis: Vec<Phi>, ops: Vec<Op>, succ: Vec<i64>) -> Self {
        Self {
            at,
            phis,
            ops,
            succ,
        }
    }
}

/// One source-neutral MIR body.  Direct port of `qbopt.model.mir:MirBody`.
///
/// Raise-time allocation history is deliberately not part of this public
/// type.  It crosses the boundary separately as [`AllocationHints`].
///
/// ```compile_fail
/// fn origin_is_private(body: &llrm::model::mir::MirBody) {
///     let _ = &body.origin;
/// }
/// ```
///
/// ```compile_fail
/// fn pins_are_private(body: &llrm::model::mir::MirBody) {
///     let _ = &body.pins;
/// }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirBody {
    pub entry: i64,
    pub blocks: Vec<MirBlock>,
    pub initial: Vec<(MemRef, Const)>,
    pub repetitions: Vec<(i64, i64)>,
    pub cloned: bool,
    pub sealed: bool,
    pub pointer_values: BTreeSet<Value>,
    pub pointer_seeds: OrderedMap<Value, Provenance>,
    pub integer_ranges: OrderedMap<Value, IntegerRange>,
    pub loop_trip_counts: Vec<(i64, i64)>,
}

impl MirBody {
    /// Constructs Python's two-required-field `MirBody` form.
    pub fn new(entry: i64, blocks: Vec<MirBlock>) -> Self {
        Self {
            entry,
            blocks,
            initial: Vec::new(),
            repetitions: Vec::new(),
            cloned: false,
            sealed: false,
            pointer_values: BTreeSet::new(),
            pointer_seeds: OrderedMap::new(),
            integer_ranges: OrderedMap::new(),
            loop_trip_counts: Vec::new(),
        }
    }

    /// Python `MirBody.block`.
    pub fn block(&self, at: i64) -> Option<&MirBlock> {
        self.blocks.iter().find(|block| block.at == at)
    }

    /// Python `MirBody.values`, preserving block/phi/op declaration order.
    pub fn values(&self) -> Vec<Value> {
        self.blocks
            .iter()
            .flat_map(|block| {
                block
                    .phis
                    .iter()
                    .map(|phi| phi.result)
                    .chain(block.ops.iter().flat_map(|op| op.defines.iter().copied()))
            })
            .collect()
    }
}

/// Private raise-time placement view, sufficient only to externalize
/// `AllocationHints`.  Rust has no struct inheritance, so `Deref` gives this
/// the same read-only body surface as Python's `_RaisedBody(MirBody)` while
/// keeping the two additional maps private to raising.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(dead_code)] // Constructed by the direct raiser port; model tests exercise it meanwhile.
pub(crate) struct RaisedBody {
    pub body: MirBody,
    pub origin: OrderedMap<Value, iced_x86::Register>,
    pub pins: OrderedMap<Value, iced_x86::Register>,
}

impl RaisedBody {
    #[allow(dead_code)] // Constructed by the direct raiser port; model tests exercise it meanwhile.
    pub(crate) fn new(body: MirBody) -> Self {
        Self {
            body,
            origin: OrderedMap::new(),
            pins: OrderedMap::new(),
        }
    }
}

impl std::ops::Deref for RaisedBody {
    type Target = MirBody;

    fn deref(&self) -> &Self::Target {
        &self.body
    }
}

/// Failures while externalizing the private allocation history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AllocationHintsError {
    ConflictingOrigin {
        variable: u32,
        previous: iced_x86::Register,
        location: iced_x86::Register,
    },
    MissingPinDefinition {
        value: Value,
    },
    ConflictingPin {
        operation: u32,
        result: usize,
        previous: iced_x86::Register,
        location: iced_x86::Register,
    },
}

impl fmt::Display for AllocationHintsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConflictingOrigin {
                variable,
                previous,
                location,
            } => write!(
                formatter,
                "variable {variable} has conflicting allocation hints: {} and {}", *previous as u32, *location as u32
            ),
            Self::MissingPinDefinition { value } => {
                write!(
                    formatter,
                    "pinned {value:?} has no source definition identity"
                )
            }
            Self::ConflictingPin {
                operation,
                result,
                previous,
                location,
            } => write!(
                formatter,
                "definition ({operation}, {result}) has conflicting allocation pins: {} and {}", *previous as u32, *location as u32
            ),
        }
    }
}

impl std::error::Error for AllocationHintsError {}

/// Backend-only placement history, outside public MIR semantics.
/// Direct port of `qbopt.model.mir:AllocationHints`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllocationHints {
    pub origins: OrderedMap<u32, iced_x86::Register>,
    pub pins: OrderedMap<(u32, usize), iced_x86::Register>,
}

impl AllocationHints {
    pub fn new() -> Self {
        Self {
            origins: OrderedMap::new(),
            pins: OrderedMap::new(),
        }
    }

    /// Python `AllocationHints.from_body`.
    #[allow(dead_code)] // Called by the direct raiser port; model tests exercise it meanwhile.
    pub(crate) fn from_body(body: &RaisedBody) -> Result<Self, AllocationHintsError> {
        let mut origins = OrderedMap::new();
        for (value, location) in body.origin.iter() {
            if let Some(previous) = origins.get(&value.variable) {
                if previous != location {
                    return Err(AllocationHintsError::ConflictingOrigin {
                        variable: value.variable,
                        previous: *previous,
                        location: *location,
                    });
                }
            } else {
                origins.insert(value.variable, *location);
            }
        }

        let definitions = body
            .blocks
            .iter()
            .flat_map(|block| block.ops.iter())
            .filter_map(|op| op.id.map(|id| (id, &op.defines)))
            .flat_map(|(id, defines)| {
                defines
                    .iter()
                    .copied()
                    .enumerate()
                    .map(move |(index, value)| (value, (id, index)))
            })
            .collect::<BTreeMap<_, _>>();

        let mut pins = OrderedMap::new();
        for (value, location) in body.pins.iter() {
            let Some(&(operation, result)) = definitions.get(value) else {
                return Err(AllocationHintsError::MissingPinDefinition { value: *value });
            };
            if let Some(previous) = pins.get(&(operation, result)) {
                if previous != location {
                    return Err(AllocationHintsError::ConflictingPin {
                        operation,
                        result,
                        previous: *previous,
                        location: *location,
                    });
                }
            } else {
                pins.insert((operation, result), *location);
            }
        }
        Ok(Self { origins, pins })
    }

    /// Python `AllocationHints.origin_of`.
    pub fn origin_of(&self, value: Value) -> Option<iced_x86::Register> {
        self.origins.get(&value.variable).copied()
    }

    /// Python `AllocationHints.pin_of`.
    pub fn pin_of(&self, operation: &Op, result: usize) -> Option<iced_x86::Register> {
        operation
            .id
            .and_then(|id| self.pins.get(&(id, result)).copied())
    }
}

impl Default for AllocationHints {
    fn default() -> Self {
        Self::new()
    }
}

/// Python `consumed`: all values actually read by one operation.
pub fn consumed(op: &Op) -> BTreeSet<Value> {
    let mut result = op
        .uses
        .iter()
        .copied()
        .filter(|value| !op.merges.contains_key(value))
        .collect::<BTreeSet<_>>();
    result.extend(op.args.iter().filter_map(|arg| match arg {
        Arg::Held(held) => Some(held.value),
        _ => None,
    }));
    for arg in op.args.iter().chain(&op.results) {
        if let Arg::Cell(cell) = arg {
            result.extend([cell.r#ref.base, cell.r#ref.segment].into_iter().flatten());
        }
    }
    result
}

/// Python `rewritten`.
pub fn rewritten(op: &Op) -> bool {
    op.raised
        .as_ref()
        .is_some_and(|raised| (&op.args, &op.results) != (&raised.0, &raised.1))
}

/// Python `partial`: a result retains bits from its old place.
pub fn partial(op: &Op) -> bool {
    let words = op
        .results
        .iter()
        .filter_map(|result| match result {
            Arg::Held(held) if held.width == 2 => Some(held.value),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    op.merges.values().any(|value| !words.contains(value))
}

fn arg_width(arg: &Arg) -> u32 {
    match arg {
        Arg::Held(value) => value.width,
        Arg::Const(value) => value.width,
        Arg::Symbol(value) => value.width,
        Arg::FrameAddress(value) => value.width,
        Arg::FrameSelector(value) => value.width,
        Arg::Cell(_) | Arg::Opaque(_) => 2,
    }
}

/// Python `stepping`: `(what it steps, by how much)`, or no affine step.
pub fn stepping(op: &Op) -> Option<(Arg, Arg)> {
    if !op.loads.is_empty() || !op.stores.is_empty() {
        return None;
    }
    match op.kind {
        Kind::Increment if op.args.len() == 1 => Some((
            op.args[0].clone(),
            Arg::Const(Const::new(1, arg_width(&op.args[0]))),
        )),
        Kind::Decrement if op.args.len() == 1 => Some((
            op.args[0].clone(),
            Arg::Const(Const::new(-1, arg_width(&op.args[0]))),
        )),
        Kind::Add if op.args.len() == 2 => Some((op.args[0].clone(), op.args[1].clone())),
        Kind::Sub if op.args.len() == 2 => match &op.args[1] {
            Arg::Const(value) => Some((
                op.args[0].clone(),
                Arg::Const(Const::new(-&value.n, value.width)),
            )),
            _ => None,
        },
        _ => None,
    }
}

/// Python `exposed`: values observable after control leaves a body.
pub fn exposed(body: &MirBody) -> BTreeSet<Value> {
    body.blocks
        .iter()
        .filter(|block| block.succ.is_empty())
        .filter_map(|block| block.ops.last())
        .flat_map(|op| exit_values(op).iter().copied())
        .collect()
}

/// Python `ordinary_uses`: uses encoded or consumed by an operation itself.
pub fn ordinary_uses(op: &Op) -> &[Value] {
    &op.uses
}

/// Python `exit_values`: values observable without being operation operands.
pub fn exit_values(op: &Op) -> &[Value] {
    &op.exits
}

/// Python `unheld`: opaque resource writes and reads; `None` means every
/// resource.
pub fn unheld(op: &Op) -> (&Option<BTreeSet<String>>, &Option<BTreeSet<String>>) {
    (&op.opaque_defs, &op.opaque_uses)
}

/// Direct port of Python `qbopt.model.mir:_Renamer`.
struct Renamer {
    next: u32,
    stack: BTreeMap<u32, Vec<Value>>,
    versions: BTreeMap<u32, u32>,
}

impl Renamer {
    fn fresh(&mut self, variable: u32, at: i64, flags: bool) -> Value {
        self.next += 1;
        let version = self.versions.entry(variable).or_default();
        *version += 1;
        Value {
            id: self.next,
            at,
            flags,
            variable,
            version: *version,
        }
    }

    fn current_of(&mut self, variable: u32, at: i64) -> Value {
        if self.stack.get(&variable).is_none_or(Vec::is_empty) {
            let value = self.fresh(variable, at, false);
            self.stack.entry(variable).or_default().push(value);
        }
        *self.stack[&variable]
            .last()
            .expect("a Python renamer current value was just established")
    }

    fn current(&mut self, value: Value, at: i64) -> Value {
        if self.stack.get(&value.variable).is_none_or(Vec::is_empty) {
            let current = self.fresh(value.variable, at, value.flags);
            self.stack.entry(value.variable).or_default().push(current);
        }
        *self.stack[&value.variable]
            .last()
            .expect("a Python renamer current value was just established")
    }
}

fn remember(
    renamed: &mut BTreeMap<Value, BTreeSet<Value>>,
    old: Option<Value>,
    new: Option<Value>,
) {
    if let (Some(old), Some(new)) = (old, new) {
        renamed.entry(old).or_default().insert(new);
    }
}

/// Direct port of Python `qbopt.model.mir:_renamed_arg`.
fn renamed_arg(argument: &Arg, swap: &BTreeMap<u32, Value>, refs: &[(MemRef, MemRef)]) -> Arg {
    match argument {
        Arg::Held(held) => swap
            .get(&held.value.variable)
            .map(|value| {
                Arg::Held(Held {
                    value: *value,
                    width: held.width,
                })
            })
            .unwrap_or_else(|| argument.clone()),
        Arg::Cell(cell) => refs
            .iter()
            // Python `dict(zip(...))` keeps the last equal key's value.
            .rev()
            .find_map(|(old, new)| (old == &cell.r#ref).then(|| new.clone()))
            .map(|r#ref| Arg::Cell(Cell { r#ref }))
            .unwrap_or_else(|| argument.clone()),
        Arg::Const(_) | Arg::Symbol(_) | Arg::FrameAddress(_) | Arg::FrameSelector(_) | Arg::Opaque(_) => argument.clone(),
    }
}

/// Python `_IDS = itertools.count(1)`.
static IDS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

/// `next(_IDS)`.
pub fn next_id() -> u32 {
    IDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// Python `_live_outs`: the source values observable at each machine exit.
fn live_outs(body: &RaisedBody) -> BTreeMap<i64, BTreeSet<Value>> {
    use crate::analysis::liveness as alive_at;
    use crate::model::ir::root;

    type Registers = BTreeMap<iced_x86::Register, BTreeSet<Value>>;
    let predecessors = loops::predecessors(&body.blocks);
    let mut arriving = Registers::new();
    for value in alive_at::entry_values(body) {
        if value.flags {
            continue;
        }
        if let Some(register) = body.origin.get(&value) {
            arriving.entry(root(*register)).or_default().insert(value);
        }
    }

    let mut outof: BTreeMap<i64, Registers> = body.blocks.iter().map(|block| (block.at, Registers::new())).collect();
    let mut changing = true;
    while changing {
        changing = false;
        for block in &body.blocks {
            let mut here = Registers::new();
            let mut incoming: Vec<&Registers> =
                predecessors.get(&block.at).into_iter().flatten().map(|one| &outof[one]).collect();
            if block.at == body.entry {
                incoming.push(&arriving);
            }
            for state in incoming {
                for (register, values) in state {
                    here.entry(*register).or_default().extend(values.iter().copied());
                }
            }
            for phi in &block.phis {
                if phi.result.flags {
                    continue;
                }
                if let Some(register) = body.origin.get(&phi.result) {
                    here.insert(root(*register), BTreeSet::from([phi.result]));
                }
            }
            for op in &block.ops {
                for value in &op.defines {
                    if value.flags {
                        continue;
                    }
                    if let Some(register) = body.origin.get(value) {
                        here.insert(root(*register), BTreeSet::from([*value]));
                    }
                }
            }
            if here != outof[&block.at] {
                outof.insert(block.at, here);
                changing = true;
            }
        }
    }

    let mut result = BTreeMap::new();
    for block in &body.blocks {
        if !block.succ.is_empty() {
            continue;
        }
        let leaving = match block.ops.last() {
            Some(last) if last.kind == Kind::Return => {
                consumed(last).into_iter().filter(|value| !value.flags).collect()
            }
            _ => outof[&block.at].values().flatten().copied().collect(),
        };
        result.insert(block.at, leaving);
    }
    result
}

/// Python `_with_live_outs`: exit visibility as machine-free MIR uses.
pub(crate) fn with_live_outs(body: RaisedBody) -> RaisedBody {
    let live = live_outs(&body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let leaving = live.get(&block.at).cloned().unwrap_or_default();
        if leaving.is_empty() {
            blocks.push(block.clone());
            continue;
        }
        let mut ordered: Vec<Value> = leaving.into_iter().collect();
        ordered.sort_by_key(|value| (value.variable, value.version, value.id));
        let mut block = block.clone();
        if let Some(last) = block.ops.last_mut() {
            last.exits = ordered;
        } else {
            let mut marker = Op::new(block.at, OpCode::nothing(), "", Vec::new(), Vec::new());
            marker.kind = Kind::Nothing;
            marker.source_backed = false;
            marker.id = Some(next_id());
            marker.exits = ordered;
            block.ops = vec![marker];
        }
        blocks.push(block);
    }
    let mut body = body;
    body.body.blocks = blocks;
    body
}

/// Python `_public`: drop the raise-only machine view.
pub(crate) fn public(body: RaisedBody) -> MirBody {
    body.body
}

/// Python `_outside`: the frame's bytes no range in `reach` covers, as exclusions.
fn outside(reach: &BTreeSet<(i64, i64)>) -> Vec<(Addr, u32)> {
    let (start, size) = WHOLE_FRAME;
    let (low, high) = (start.disp, start.disp + i64::from(size));
    let mut out = Vec::new();
    let mut at = low;
    for &(start, end) in reach {
        if start > at {
            out.push((Addr::new(Space::Frame, at), (start - at) as u32));
        }
        at = at.max(end);
    }
    if at < high {
        out.push((Addr::new(Space::Frame, at), (high - at) as u32));
    }
    out
}

/// Python `_through_frame`.
fn through_frame(body: MirBody, framed: &BTreeMap<Value, BTreeSet<(i64, i64)>>) -> MirBody {
    if framed.is_empty() {
        return body;
    }
    let reaches = |one: &MemRef| one.base.is_some_and(|base| framed.contains_key(&base));
    let tag = |one: &MemRef| {
        match one.base.and_then(|base| framed.get(&base)) {
            Some(extents)
                if one.segment.is_none()
                    && !one.pointer
                    && one.addr.is_some_and(|addr| addr.space == Space::Literal) =>
            {
                let mut tagged = one.clone();
                tagged.within = Some(extents.iter().copied().collect());
                tagged
            }
            _ => one.clone(),
        }
    };
    let operand = |one: &Arg| match one {
        Arg::Cell(cell) => Arg::Cell(Cell { r#ref: tag(&cell.r#ref) }),
        other => other.clone(),
    };
    let mut body = body;
    for block in &mut body.blocks {
        for op in &mut block.ops {
            if op.loads.iter().chain(&op.stores).any(reaches) {
                op.loads = op.loads.iter().map(tag).collect();
                op.stores = op.stores.iter().map(tag).collect();
                op.args = op.args.iter().map(operand).collect();
                op.results = op.results.iter().map(operand).collect();
            }
        }
    }
    body
}

/// Python `_frame_bounded`: exclude this body's frame slots from every bounded effect.
pub(crate) fn frame_bounded(body: MirBody, pointers: bool) -> MirBody {
    use crate::analysis::frameescape;

    let body = if pointers {
        let framed = frameescape::framed(&body);
        through_frame(body, &framed)
    } else {
        body
    };
    let escapes = frameescape::analysed(&body);
    let Some(reach) = escapes.reach.as_ref().filter(|_| escapes.opaque_addresses.is_empty()) else {
        return body;
    };
    let holes = outside(reach);
    if holes.is_empty() {
        return body;
    }
    let bounded = |one: &MemRef| {
        if one.excludes.contains(&holes[0]) {
            return false;
        }
        if one.beyond.is_some() {
            return true;
        }
        let reached = one.base.is_some() || one.segment.is_some() || one.pointer || one.addr.is_none();
        pointers
            && reached
            && !matches!(one.where_(), Some(Space::Frame | Space::Segment | Space::External | Space::Stack))
    };
    let bound = |one: &MemRef| {
        if bounded(one) {
            let mut bound = one.clone();
            bound.excludes.extend(holes.iter().copied());
            bound
        } else {
            one.clone()
        }
    };
    let mut body = body;
    for block in &mut body.blocks {
        for op in &mut block.ops {
            if op.loads.iter().chain(&op.stores).any(|one| bounded(one)) {
                op.loads = op.loads.iter().map(bound).collect();
                op.stores = op.stores.iter().map(bound).collect();
            }
        }
    }
    body
}

/// Direct port of Python `qbopt.model.mir:_rehomed`.
fn rehomed(reference: &MemRef, namer: &mut Renamer, at: i64) -> MemRef {
    let base = reference.base.map(|value| namer.current(value, at));
    let segment = reference.segment.map(|value| namer.current(value, at));
    if base == reference.base && segment == reference.segment {
        reference.clone()
    } else {
        let mut renamed = reference.clone();
        renamed.base = base;
        renamed.segment = segment;
        renamed
    }
}

fn resolved_rename(
    at: i64,
    start: i64,
    blocks: &BTreeMap<i64, &MirBlock>,
    children: &BTreeMap<i64, Vec<i64>>,
    namer: &mut Renamer,
    phis: &mut BTreeMap<i64, Vec<(u32, Phi)>>,
    out: &mut BTreeMap<i64, Vec<Op>>,
    renamed: &mut BTreeMap<Value, BTreeSet<Value>>,
) {
    let block = blocks[&at];
    let mut pushed = Vec::new();
    for (variable, phi) in &phis[&at] {
        namer.stack.entry(*variable).or_default().push(phi.result);
        pushed.push(*variable);
    }

    for operation in &block.ops {
        let used = operation
            .uses
            .iter()
            .copied()
            .map(|value| namer.current(value, start))
            .collect::<Vec<_>>();
        let exits = operation
            .exits
            .iter()
            .copied()
            .map(|value| namer.current(value, start))
            .collect::<Vec<_>>();
        for (old, new) in operation.uses.iter().zip(&used) {
            remember(renamed, Some(*old), Some(*new));
        }
        for (old, new) in operation.exits.iter().zip(&exits) {
            remember(renamed, Some(*old), Some(*new));
        }
        let swap = operation
            .uses
            .iter()
            .map(|value| value.variable)
            .zip(used.iter().copied())
            .collect::<BTreeMap<_, _>>();
        let loads = operation
            .loads
            .iter()
            .map(|reference| rehomed(reference, namer, start))
            .collect::<Vec<_>>();
        let stores = operation
            .stores
            .iter()
            .map(|reference| rehomed(reference, namer, start))
            .collect::<Vec<_>>();
        for (old, new) in operation
            .loads
            .iter()
            .chain(&operation.stores)
            .zip(loads.iter().chain(&stores))
        {
            remember(renamed, old.base, new.base);
            remember(renamed, old.segment, new.segment);
        }
        let refs = operation
            .loads
            .iter()
            .chain(&operation.stores)
            .cloned()
            .zip(loads.iter().chain(&stores).cloned())
            .collect::<Vec<_>>();
        let mut fresh = Vec::new();
        for value in &operation.defines {
            let now = namer.fresh(value.variable, operation.at, value.flags);
            namer.stack.entry(value.variable).or_default().push(now);
            pushed.push(value.variable);
            fresh.push(now);
            remember(renamed, Some(*value), Some(now));
        }
        let made = operation
            .defines
            .iter()
            .map(|value| value.variable)
            .zip(fresh.iter().copied())
            .collect::<BTreeMap<_, _>>();
        let mut merges = OrderedMap::new();
        for (before, after) in operation.merges.iter() {
            merges.insert(
                swap.get(&before.variable).copied().unwrap_or(*before),
                made.get(&after.variable).copied().unwrap_or(*after),
            );
        }
        let mut made_operation = operation.clone();
        made_operation.defines = fresh;
        made_operation.uses = used;
        made_operation.exits = exits;
        made_operation.loads = loads;
        made_operation.stores = stores;
        made_operation.args = operation
            .args
            .iter()
            .map(|argument| renamed_arg(argument, &swap, &refs))
            .collect();
        made_operation.results = operation
            .results
            .iter()
            .map(|argument| renamed_arg(argument, &made, &refs))
            .collect();
        made_operation.raised = operation.raised.as_ref().map(|(args, results)| {
            (
                args.iter()
                    .map(|argument| renamed_arg(argument, &swap, &refs))
                    .collect(),
                results
                    .iter()
                    .map(|argument| renamed_arg(argument, &made, &refs))
                    .collect(),
            )
        });
        made_operation.merges = merges;
        out.get_mut(&at)
            .expect("every Python block receives an output operation list")
            .push(made_operation);
    }

    for successor in &block.succ {
        if let Some(successor_phis) = phis.get_mut(successor) {
            for (variable, phi) in successor_phis {
                phi.incoming.insert(at, namer.current_of(*variable, start));
            }
        }
    }

    let mut descendants = children[&at].clone();
    descendants.sort_unstable();
    for child in descendants {
        resolved_rename(child, start, blocks, children, namer, phis, out, renamed);
    }
    for variable in pushed.into_iter().rev() {
        namer
            .stack
            .get_mut(&variable)
            .expect("every pushed Python variable has a stack")
            .pop();
    }
}

/// Direct port of `qbopt.model.mir:resolved`.
///
/// The optional `calls` argument is deliberately retained even though Python
/// currently does not read it; callers supply it at the same port boundary.
pub fn resolved(body: &MirBody, _calls: Option<&BTreeMap<i64, String>>) -> Result<MirBody, String> {
    if body.blocks.is_empty() {
        return Err("no blocks to resolve".to_owned());
    }
    let start = body.entry;
    let everything = body
        .blocks
        .iter()
        .map(|block| (block.at, block))
        .collect::<BTreeMap<_, _>>();
    if !everything.contains_key(&start) {
        return Err(format!(
            "the entry {} is not one of these blocks",
            python_padded_hex(start)
        ));
    }

    let mut reachable = BTreeSet::from([start]);
    let mut pending = vec![start];
    while let Some(at) = pending.pop() {
        for successor in &everything[&at].succ {
            if everything.contains_key(successor) && reachable.insert(*successor) {
                pending.push(*successor);
            }
        }
    }
    let blocks = body
        .blocks
        .iter()
        .filter(|block| reachable.contains(&block.at))
        .cloned()
        .collect::<Vec<_>>();
    if !loops::irreducible(&blocks, Some(start)).is_empty() {
        return Err(
            "the body's control flow is irreducible, so it has no dominator tree".to_owned(),
        );
    }

    let by_at = blocks
        .iter()
        .map(|block| (block.at, block))
        .collect::<BTreeMap<_, _>>();
    let immediate = loops::immediate_dominators(&blocks, Some(start));
    let mut children = blocks
        .iter()
        .map(|block| (block.at, Vec::new()))
        .collect::<BTreeMap<_, Vec<i64>>>();
    for block in &blocks {
        if let Some(parent) = immediate[&block.at] {
            children
                .get_mut(&parent)
                .expect("an immediate dominator is a supplied block")
                .push(block.at);
        }
    }
    let frontier = loops::frontiers(&blocks, Some(start));
    let mut where_defined = BTreeMap::<u32, BTreeSet<i64>>::new();
    for block in &blocks {
        for operation in &block.ops {
            for value in &operation.defines {
                where_defined
                    .entry(value.variable)
                    .or_default()
                    .insert(block.at);
            }
        }
    }
    let mut needed = blocks
        .iter()
        .map(|block| (block.at, BTreeSet::new()))
        .collect::<BTreeMap<i64, BTreeSet<u32>>>();
    for (variable, defined) in &where_defined {
        let mut pending = defined.iter().copied().collect::<Vec<_>>();
        let mut seen = BTreeSet::new();
        while let Some(at) = pending.pop() {
            for join in &frontier[&at] {
                if seen.insert(*join) {
                    needed
                        .get_mut(join)
                        .expect("a frontier only names supplied blocks")
                        .insert(*variable);
                    pending.push(*join);
                }
            }
        }
    }

    let mut namer = Renamer {
        next: 0,
        stack: BTreeMap::new(),
        versions: BTreeMap::new(),
    };
    let mut phis = blocks
        .iter()
        .map(|block| (block.at, Vec::<(u32, Phi)>::new()))
        .collect::<BTreeMap<_, _>>();
    let mut out = blocks
        .iter()
        .map(|block| (block.at, Vec::new()))
        .collect::<BTreeMap<_, Vec<Op>>>();
    let mut renamed = BTreeMap::<Value, BTreeSet<Value>>::new();
    let flagged = blocks
        .iter()
        .flat_map(|block| block.ops.iter())
        .flat_map(|operation| operation.defines.iter())
        .filter(|value| value.flags)
        .map(|value| value.variable)
        .collect::<BTreeSet<_>>();
    for block in &blocks {
        let mut variables = needed[&block.at].iter().copied().collect::<Vec<_>>();
        variables.sort_by_key(|variable| (!flagged.contains(variable), *variable));
        for variable in variables {
            let result = namer.fresh(variable, block.at, flagged.contains(&variable));
            phis.get_mut(&block.at)
                .expect("every supplied block has a phi list")
                .push((variable, Phi::new(result)));
        }
        for old in &block.phis {
            if let Some((_, made)) = phis[&block.at]
                .iter()
                .find(|(variable, _)| *variable == old.result.variable)
            {
                remember(&mut renamed, Some(old.result), Some(made.result));
            }
        }
    }
    resolved_rename(
        start,
        start,
        &by_at,
        &children,
        &mut namer,
        &mut phis,
        &mut out,
        &mut renamed,
    );

    let resolved_blocks = blocks
        .iter()
        .map(|block| MirBlock {
            at: block.at,
            phis: phis[&block.at].iter().map(|(_, phi)| phi.clone()).collect(),
            ops: out[&block.at].clone(),
            succ: block
                .succ
                .iter()
                .copied()
                .filter(|successor| reachable.contains(successor))
                .collect(),
        })
        .collect();
    let mut pointer_values = body
        .pointer_values
        .iter()
        .flat_map(|old| renamed.get(old).into_iter().flatten().copied())
        .collect::<BTreeSet<_>>();
    let mut pointer_seeds = OrderedMap::new();
    let mut conflicting = BTreeSet::new();
    for (old, provenance) in body.pointer_seeds.iter() {
        for new in renamed.get(old).into_iter().flatten() {
            if pointer_seeds
                .get(new)
                .is_some_and(|previous| previous != provenance)
            {
                conflicting.insert(*new);
            } else {
                pointer_seeds.insert(*new, provenance.clone());
            }
        }
    }
    for value in conflicting {
        pointer_seeds.remove(&value);
    }
    let mut integer_ranges = OrderedMap::new();
    let mut range_conflicts = BTreeSet::new();
    for (old, interval) in body.integer_ranges.iter() {
        for new in renamed.get(old).into_iter().flatten() {
            if integer_ranges
                .get(new)
                .is_some_and(|previous| previous != interval)
            {
                range_conflicts.insert(*new);
            } else {
                integer_ranges.insert(*new, interval.clone());
            }
        }
    }
    for value in range_conflicts {
        integer_ranges.remove(&value);
    }
    pointer_values.extend(pointer_seeds.keys().copied());
    Ok(MirBody {
        entry: start,
        blocks: resolved_blocks,
        initial: body.initial.clone(),
        repetitions: body.repetitions.clone(),
        cloned: body.cloned,
        sealed: body.sealed,
        pointer_values,
        pointer_seeds,
        integer_ranges,
        loop_trip_counts: body.loop_trip_counts.clone(),
    })
}

/// Direct port of `qbopt.model.mir:verify`.
///
/// The returned diagnostics establish only Python MIR's three SSA promises:
/// one definition per value, definitions that dominate uses, and one phi
/// incoming value per predecessor.  This is intentionally not a structural
/// or type verifier.
pub fn verify(body: &MirBody) -> Vec<String> {
    let dominators = loops::dominators(&body.blocks, Some(body.entry));
    let mut problems = Vec::new();

    let mut defined_at = BTreeMap::<Value, i64>::new();
    for block in &body.blocks {
        for phi in &block.phis {
            if defined_at.contains_key(&phi.result) {
                problems.push(format!("{} defined twice", phi.result));
            }
            defined_at.insert(phi.result, block.at);
        }
        for op in &block.ops {
            for value in &op.defines {
                if defined_at.contains_key(value) {
                    problems.push(format!(
                        "{} defined twice, at {}",
                        value,
                        python_padded_hex(op.at)
                    ));
                }
                defined_at.insert(*value, block.at);
            }
        }
    }

    let predecessors = loops::predecessors(&body.blocks);
    for block in &body.blocks {
        for phi in &block.phis {
            let want = predecessors
                .get(&block.at)
                .expect("every supplied block has a predecessor entry")
                .iter()
                .filter(|predecessor| body.block(**predecessor).is_some())
                .copied()
                .collect::<BTreeSet<_>>();
            let have = phi.incoming.keys().copied().collect::<BTreeSet<_>>();
            if have != want {
                problems.push(format!(
                    "{} at {} has {}, its predecessors are {}",
                    phi.result,
                    python_padded_hex(block.at),
                    python_hex_list(&have),
                    python_hex_list(&want)
                ));
            }
            for (came_from, value) in phi.incoming.iter() {
                if let Some(where_) = defined_at.get(value) {
                    if !dominators
                        .get(came_from)
                        .is_some_and(|dominators| dominators.contains(where_))
                    {
                        problems.push(format!(
                            "{} takes {} from {}, which it does not reach",
                            phi.result,
                            value,
                            python_padded_hex(*came_from)
                        ));
                    }
                }
            }
        }

        let mut pending = block
            .ops
            .iter()
            .flat_map(|op| op.defines.iter().copied())
            .collect::<BTreeSet<_>>();
        for op in &block.ops {
            for value in &op.uses {
                let Some(where_) = defined_at.get(value) else {
                    continue;
                };
                if pending.contains(value) {
                    problems.push(format!(
                        "{} uses {} before its definition in {}",
                        python_padded_hex(op.at),
                        value,
                        python_padded_hex(block.at)
                    ));
                } else if !dominators
                    .get(&block.at)
                    .is_some_and(|dominators| dominators.contains(where_))
                {
                    problems.push(format!(
                        "{} uses {}, defined in {}, which does not dominate it",
                        python_padded_hex(op.at),
                        value,
                        python_padded_hex(*where_)
                    ));
                }
            }
            for value in &op.defines {
                pending.remove(value);
            }
            for value in &op.exits {
                if let Some(where_) = defined_at.get(value) {
                    if !dominators
                        .get(&block.at)
                        .is_some_and(|dominators| dominators.contains(where_))
                    {
                        problems.push(format!(
                            "{} exposes {}, defined in {}, which does not dominate it",
                            python_padded_hex(op.at),
                            value,
                            python_padded_hex(*where_)
                        ));
                    }
                }
            }
        }
    }
    problems
}

fn python_hex_list(values: &BTreeSet<i64>) -> String {
    let mut values = values
        .iter()
        .map(|value| python_hex(*value))
        .collect::<Vec<_>>();
    values.sort();
    let values = values
        .iter()
        .map(|value| format!("'{value}'"))
        .collect::<Vec<_>>();
    format!("[{}]", values.join(", "))
}

fn python_hex(value: i64) -> String {
    if value < 0 {
        format!("-0x{:x}", value.unsigned_abs())
    } else {
        format!("0x{value:x}")
    }
}

fn python_padded_hex(value: i64) -> String {
    if value < 0 {
        format!("-0x{:03x}", value.unsigned_abs())
    } else {
        format!("0x{value:04x}")
    }
}

impl Repr for Value {
    fn repr(&self) -> String {
        self.to_string()
    }
}

impl Repr for BigInt {
    fn repr(&self) -> String {
        self.to_string()
    }
}

impl Repr for IntegerRange {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "IntegerRange",
            &[("low", self.low.repr()), ("high", self.high.repr()), ("width", self.width.repr())],
        )
    }
}

impl Repr for MemRef {
    fn repr(&self) -> String {
        let beyond = self.beyond.as_ref().map_or_else(
            || "None".to_owned(),
            |(segment, cells)| format!("({}, {})", segment, pyrepr::frozenset(&cells.iter().collect::<Vec<_>>())),
        );
        pyrepr::dataclass(
            "MemRef",
            &[
                ("addr", self.addr.repr()),
                ("width", self.width.repr()),
                ("base", self.base.repr()),
                ("segment", self.segment.repr()),
                ("space", self.space.repr()),
                ("beyond", beyond),
                ("symbolic", self.symbolic.repr()),
                ("allocation", self.allocation.repr()),
                ("base_width", self.base_width.repr()),
                ("pointer", self.pointer.repr()),
                ("excludes", pyrepr::tuple(&self.excludes)),
                ("typed", self.typed.repr()),
                ("within", self.within.as_ref().map_or_else(|| "None".to_owned(), |one| pyrepr::tuple(one))),
                ("provenance", self.provenance.repr()),
                ("volatile", self.volatile.repr()),
            ],
        )
    }
}

impl Repr for Held {
    fn repr(&self) -> String {
        pyrepr::dataclass("Held", &[("value", self.value.repr()), ("width", self.width.repr())])
    }
}

impl Repr for Const {
    fn repr(&self) -> String {
        pyrepr::dataclass("Const", &[("n", self.n.repr()), ("width", self.width.repr())])
    }
}

impl Repr for Symbol {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Symbol",
            &[
                ("space", self.space.repr()),
                ("index", self.index.repr()),
                ("offset", self.offset.repr()),
                ("width", self.width.repr()),
                ("addend", self.addend.repr()),
            ],
        )
    }
}

impl Repr for FrameAddress {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "FrameAddress",
            &[("offset", self.offset.repr()), ("width", self.width.repr()), ("extent", self.extent.repr())],
        )
    }
}

impl Repr for FrameSelector {
    fn repr(&self) -> String {
        pyrepr::dataclass("FrameSelector", &[("width", self.width.repr())])
    }
}

impl Repr for ArrayRequest {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "ArrayRequest",
            &[
                ("descriptor", self.descriptor.repr()),
                ("element_width", self.element_width.repr()),
                ("bounds", pyrepr::tuple(&self.bounds)),
                ("replaces", self.replaces.repr()),
            ],
        )
    }
}

impl Repr for Cell {
    fn repr(&self) -> String {
        pyrepr::dataclass("Cell", &[("ref", self.r#ref.repr())])
    }
}

impl Repr for Opaque {
    fn repr(&self) -> String {
        pyrepr::dataclass("Opaque", &[("what", self.what.repr()), ("name", self.name.repr())])
    }
}

impl Repr for Arg {
    fn repr(&self) -> String {
        match self {
            Arg::Held(one) => one.repr(),
            Arg::Const(one) => one.repr(),
            Arg::Symbol(one) => one.repr(),
            Arg::FrameAddress(one) => one.repr(),
            Arg::FrameSelector(one) => one.repr(),
            Arg::Cell(one) => one.repr(),
            Arg::Opaque(one) => one.repr(),
        }
    }
}

impl Repr for Phi {
    fn repr(&self) -> String {
        let incoming: Vec<String> =
            self.incoming.iter().map(|(pred, value)| format!("{}: {}", pred, value.repr())).collect();
        pyrepr::dataclass("Phi", &[("result", self.result.repr()), ("incoming", format!("{{{}}}", incoming.join(", ")))])
    }
}

#[cfg(test)]
mod tests {
    /// The draft lacked FIXED_MUL and FIXED_DIV; `Kind` must be Python's, member for member.
    #[test]
    fn kind_members_match_python() {
        let got: Vec<(&str, &str)> = Kind::ALL.iter().map(|kind| (kind.name(), kind.as_str())).collect();
        assert_eq!(got, [("ADD", "add"), ("SUB", "sub"), ("ADD_CARRY", "addcarry"), ("SUB_BORROW", "subborrow"), ("INCREMENT", "increment"), ("DECREMENT", "decrement"), ("MUL", "mul"), ("SMULHI", "smulhi"), ("FIXED_MUL", "fixed_mul"), ("FIXED_DIV", "fixed_div"), ("DIV", "div"), ("REM", "rem"), ("DIVMOD", "divmod"), ("UDIVMOD", "udivmod"), ("AND", "and"), ("OR", "or"), ("XOR", "xor"), ("SHL", "shl"), ("SHR", "shr"), ("SAR", "sar"), ("NEG", "neg"), ("NOT", "not"), ("LT", "lt"), ("LE", "le"), ("GT", "gt"), ("GE", "ge"), ("EQ", "eq"), ("NE", "ne"), ("BELOW", "below"), ("BELOW_EQ", "beloweq"), ("ABOVE", "above"), ("ABOVE_EQ", "aboveeq"), ("COPY", "copy"), ("LOAD", "load"), ("STORE", "store"), ("CONVERT", "convert"), ("SIGN_EXTEND", "sign_extend"), ("ZERO_EXTEND", "zero_extend"), ("ADDRESS", "address"), ("PTR_OFFSET", "ptr_offset"), ("FILL", "fill"), ("CALL", "call"), ("BRANCH", "branch"), ("SWITCH", "switch"), ("JUMP", "jump"), ("RETURN", "return"), ("ESCAPE", "escape"), ("FADD", "fadd"), ("FSUB", "fsub"), ("FMUL", "fmul"), ("FDIV", "fdiv"), ("FNEG", "fneg"), ("FABS", "fabs"), ("FSQRT", "fsqrt"), ("FLOAD", "fload"), ("FSTORE", "fstore"), ("FCOMPARE", "fcompare"), ("FCHECK", "fcheck"), ("ARG", "arg"), ("RESULT", "result"), ("JOIN", "join"), ("EXTRACT", "extract"), ("CONCAT", "concat"), ("OPAQUE", "opaque"), ("NOTHING", "nothing")]);
    }

    /// The order CPython 3.13 iterates a set of Values built, thinned and
    /// grown this way; `lower_int64` numbers fresh values in that order.
    #[test]
    fn value_sets_iterate_in_cpython_order() {
        use crate::support::pyset::PySet;
        let make = |i: u32| Value {
            id: i * 7 % 113,
            at: i64::from(i * 37),
            flags: i % 3 == 0,
            variable: i,
            version: i % 4,
        };
        let mut set = PySet::new();
        for i in 1..120 {
            set.add(make(i));
        }
        for i in (1..120).step_by(5) {
            set.discard(&make(i));
        }
        for i in 200..260_u32 {
            set.add(Value { id: i, at: -i64::from(i), flags: false, variable: 0, version: 0 });
        }
        let got: Vec<i64> = set
            .iter()
            .map(|one| if one.version != 0 { i64::from(one.variable) } else { -i64::from(one.id) })
            .collect();
        assert_eq!(got, [-52, 30, -207, 37, -246, -247, 63, -109, 102, 113, -217, -208, -256, -82, -51, -200, -248, 43, 67, 50, 109, 58, -78, 25, 110, -236, -206, -221, 70, 118, 114, -213, 38, 45, 77, 47, -253, 7, 55, 59, 74, 34, -106, 83, -210, 99, -250, -28, -230, 82, -203, 97, -205, -108, -257, -204, -227, 105, -231, 107, -214, -233, 65, 35, -238, 9, -224, -225, 42, 90, -202, -27, 115, 78, 15, -239, -252, 18, -245, 27, 119, -243, -234, -223, -84, -216, -212, 17, -54, -56, -55, 3, -240, -209, -258, -81, -232, 10, 62, -219, 54, -237, 13, -111, -229, -226, 89, 117, -50, 33, 19, -222, -254, -235, -201, 95, -228, 5, 53, -241, 87, -211, -79, -22, 57, 85, 69, -244, -23, 103, -220, -259, -110, 98, -251, 2, -218, 29, 49, -242, -24, -215, -255, 39, -83, 73, 94, -25, 79, 75, 93, 22, 14, -249, 23]);
    }

    use std::collections::BTreeSet;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use num_bigint::BigInt;

    use crate::model::floating::{Format, Precision, Rounding, Semantics as FloatingSemantics};
    use crate::model::ir::{Operation, Semantics};
    use crate::model::memory::{Identity, MemoryKind, MemoryObject, Provenance};
    use crate::objectfile::module::{Addr, Space};
    use iced_x86::Register;

    use super::{
        AllocationHints, AllocationHintsError, Arg, ArrayRequest, Cell, Const, FloatingOrigin,
        Held, IntegerRange, Kind, MemRef, MirBlock, MirBody, Op, OpCode, Opaque, OrderedMap, Phi,
        RaisedBody, Symbol, Synth, Value, cleared, computed, consumed, exit_values, exposed,
        kind_of, ordinary_uses, partial, python_padded_hex, resolved, rewritten, same_bytes,
        stepping, unheld, verify,
    };

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
    }

    fn semantics(operation: Operation, name: &str) -> Semantics {
        let mut semantics = Semantics::new(operation);
        semantics.name = Some(name.to_owned());
        semantics
    }

    fn cell() -> Arg {
        Arg::Cell(Cell {
            r#ref: MemRef::new(None, 2),
        })
    }

    #[test]
    fn kind_of_reads_every_python_by_name_entry_for_binary_and_unary() {
        // Direct transcription of qbopt/model/mir.py::_BY_NAME.  The
        // Python classifier permits this table for both BINARY and UNARY;
        // the table itself, rather than an opcode-specific filter, decides.
        for (name, expected) in [
            ("add", Kind::Add),
            ("adc", Kind::AddCarry),
            ("inc", Kind::Increment),
            ("sub", Kind::Sub),
            ("sbb", Kind::SubBorrow),
            ("dec", Kind::Decrement),
            ("cmp", Kind::Sub),
            ("and", Kind::And),
            ("test", Kind::And),
            ("or", Kind::Or),
            ("xor", Kind::Xor),
            ("not", Kind::Not),
            ("neg", Kind::Neg),
            ("shl", Kind::Shl),
            ("sal", Kind::Shl),
            ("shr", Kind::Shr),
            ("sar", Kind::Sar),
            ("imul", Kind::Mul),
            ("mul", Kind::Mul),
            ("idiv", Kind::Div),
            ("div", Kind::Div),
            ("fadd", Kind::Fadd),
            ("faddp", Kind::Fadd),
            ("fsub", Kind::Fsub),
            ("fsubp", Kind::Fsub),
            ("fsubr", Kind::Fsub),
            ("fsubrp", Kind::Fsub),
            ("fmul", Kind::Fmul),
            ("fmulp", Kind::Fmul),
            ("fdiv", Kind::Fdiv),
            ("fdivp", Kind::Fdiv),
            ("fdivr", Kind::Fdiv),
            ("fdivrp", Kind::Fdiv),
            ("fchs", Kind::Fneg),
            ("fabs", Kind::Fabs),
            ("fsqrt", Kind::Fsqrt),
            ("fcom", Kind::Fcompare),
            ("fcomp", Kind::Fcompare),
            ("fcompp", Kind::Fcompare),
            ("ftst", Kind::Fcompare),
        ] {
            for operation in [Operation::Binary, Operation::Unary] {
                assert_eq!(
                    kind_of(&semantics(operation, name), &[], &[]),
                    expected,
                    "{name}"
                );
            }
        }
        assert_eq!(
            kind_of(&semantics(Operation::Binary, "unknown"), &[], &[]),
            Kind::Opaque
        );
        assert_eq!(
            kind_of(&semantics(Operation::Unary, "ADD"), &[], &[]),
            Kind::Opaque
        );
        assert_eq!(
            kind_of(&Semantics::new(Operation::Binary), &[], &[]),
            Kind::Opaque
        );
    }

    #[test]
    fn kind_of_uses_operands_for_move_and_preserves_python_result_priority() {
        let moved = semantics(Operation::Move, "mov");
        assert_eq!(kind_of(&moved, &[], &[]), Kind::Copy);
        assert_eq!(
            kind_of(&moved, &[Arg::Opaque(Opaque::new(None))], &[]),
            Kind::Copy
        );
        assert_eq!(kind_of(&moved, &[cell()], &[]), Kind::Load);
        assert_eq!(kind_of(&moved, &[], &[cell()]), Kind::Store);
        assert_eq!(kind_of(&moved, &[cell()], &[cell()]), Kind::Store);
    }

    #[test]
    fn kind_of_covers_each_python_semantics_arm_and_fallback() {
        for (operation, expected) in [
            (Operation::Multiply, Kind::Mul),
            (Operation::Divide, Kind::Div),
            (Operation::Extend, Kind::Convert),
            (Operation::Address, Kind::Address),
            (Operation::Push, Kind::Arg),
            (Operation::Pop, Kind::Result),
            (Operation::Jump, Kind::Jump),
            (Operation::Branch, Kind::Branch),
            (Operation::Call, Kind::Call),
            (Operation::Return, Kind::Return),
            (Operation::Escape, Kind::Escape),
            (Operation::Nothing, Kind::Nothing),
            (Operation::Restore, Kind::Join),
            (Operation::FloatLoad, Kind::Fload),
            (Operation::FloatStore, Kind::Fstore),
        ] {
            assert_eq!(
                kind_of(&semantics(operation, "ignored"), &[], &[]),
                expected
            );
        }

        assert_eq!(
            kind_of(&semantics(Operation::Compare, "cmp"), &[], &[]),
            Kind::Sub
        );
        assert_eq!(
            kind_of(&semantics(Operation::Compare, "add"), &[], &[]),
            Kind::Add
        );
        assert_eq!(
            kind_of(&semantics(Operation::Compare, "test"), &[], &[]),
            Kind::And
        );
        assert_eq!(
            kind_of(&semantics(Operation::Compare, "unknown"), &[], &[]),
            Kind::Sub
        );
        assert_eq!(
            kind_of(&semantics(Operation::Compare, ""), &[], &[]),
            Kind::Sub
        );
        assert_eq!(
            kind_of(&Semantics::new(Operation::Compare), &[], &[]),
            Kind::Sub
        );
        assert_eq!(
            kind_of(&semantics(Operation::Branch, "jl"), &[], &[]),
            Kind::Branch
        );
        assert_eq!(
            kind_of(&semantics(Operation::Branch, "je"), &[], &[]),
            Kind::Branch
        );
        for operation in [
            Operation::FloatArith,
            Operation::FloatArithPop,
            Operation::FloatUnary,
        ] {
            assert_eq!(kind_of(&semantics(operation, "fadd"), &[], &[]), Kind::Fadd);
            assert_eq!(
                kind_of(&semantics(operation, "unknown"), &[], &[]),
                Kind::Opaque
            );
        }
        assert_eq!(
            kind_of(&Semantics::new(Operation::FloatArith), &[], &[]),
            Kind::Opaque
        );
        assert_eq!(
            kind_of(&semantics(Operation::FloatUnary, ""), &[], &[]),
            Kind::Opaque
        );
        assert_ne!(
            kind_of(&semantics(Operation::FloatUnary, "fchs"), &[], &[]),
            kind_of(&semantics(Operation::FloatUnary, "fabs"), &[], &[])
        );
        assert_eq!(
            kind_of(&semantics(Operation::FloatArithPop, "fsubrp"), &[], &[]),
            Kind::Fsub
        );
        for operation in [
            Operation::Exchange,
            Operation::Funnel,
            Operation::Leave,
            Operation::Fill,
            Operation::Data,
            Operation::Barrier,
        ] {
            assert_eq!(
                kind_of(&semantics(operation, "ignored"), &[], &[]),
                Kind::Opaque
            );
        }
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
            identity: Some(Identity::Tuple(vec![Identity::Space(Space::Segment), Identity::Int(index)])),
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

    #[test]
    fn ordered_map_keeps_python_dict_iteration_and_equality() {
        let left = [(20_i64, Value::new(2, 0)), (10, Value::new(1, 0))]
            .into_iter()
            .collect::<OrderedMap<_, _>>();
        let right = [(10_i64, Value::new(1, 0)), (20, Value::new(2, 0))]
            .into_iter()
            .collect::<OrderedMap<_, _>>();
        assert_eq!(left, right, "Python dictionaries compare as mappings");
        assert_eq!(left.keys().copied().collect::<Vec<_>>(), [20, 10]);

        let mut replaced = left.clone();
        assert_eq!(
            replaced.insert(20, Value::new(3, 0)),
            Some(Value::new(2, 0))
        );
        assert_eq!(replaced.keys().copied().collect::<Vec<_>>(), [20, 10]);
    }

    #[test]
    fn direct_mir_resolved_places_phis_in_python_dominator_child_order() {
        // Direct port of tests/test_loops.py::test_a_frontier_is_where_two_definitions_could_meet,
        // through qbopt/model/mir.py::resolved.  The entry lists its children
        // backwards; Python renames sorted dominator children, so the phi's
        // insertion-ordered incoming dictionary is 1 then 2.
        let variable = 5;
        let first = Value {
            id: 71,
            at: 1,
            flags: false,
            variable,
            version: 8,
        };
        let second = Value {
            id: 72,
            at: 2,
            flags: false,
            variable,
            version: 9,
        };
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], vec![], vec![2, 1]),
                MirBlock::new(
                    1,
                    vec![],
                    vec![Op::new(1, None::<OpCode>, "", vec![first], vec![])],
                    vec![3],
                ),
                MirBlock::new(
                    2,
                    vec![],
                    vec![Op::new(2, None::<OpCode>, "", vec![second], vec![])],
                    vec![3],
                ),
                MirBlock::new(3, vec![], vec![], vec![]),
            ],
        );

        let rebuilt = resolved(&body, None).expect("the diamond is reducible");
        let phi = &rebuilt.block(3).expect("join block").phis[0];
        assert_eq!(
            phi.result,
            Value {
                id: 1,
                at: 3,
                flags: false,
                variable,
                version: 1
            }
        );
        assert_eq!(phi.incoming.keys().copied().collect::<Vec<_>>(), [1, 2]);
        assert_eq!(
            phi.incoming.values().copied().collect::<Vec<_>>(),
            [
                Value {
                    id: 2,
                    at: 1,
                    flags: false,
                    variable,
                    version: 2
                },
                Value {
                    id: 3,
                    at: 2,
                    flags: false,
                    variable,
                    version: 3
                },
            ]
        );
        assert!(verify(&rebuilt).is_empty());
    }

    #[test]
    fn direct_mir_resolved_rehomes_cells_raised_values_and_pointer_metadata() {
        // Direct port of tests/test_mir.py::{test_resolving_segld_renames_memory_operands_with_their_accesses,
        // test_resolving_renames_pointer_metadata_with_its_values}.  `raised`
        // must receive the same names as ordinary args/results, otherwise an
        // unchanged operation is falsely seen as rewritten after SSA repair.
        let pointer = Value {
            id: 99,
            at: 0,
            flags: false,
            variable: 7,
            version: 4,
        };
        let mut reference = MemRef::new(None, 2);
        reference.base = Some(pointer);
        let mut operation = Op::new(10, None::<OpCode>, "", vec![pointer], vec![pointer]);
        operation.exits = vec![pointer];
        operation.loads = vec![reference.clone()];
        operation.stores = vec![reference.clone()];
        operation.args = vec![
            Arg::Held(Held {
                value: pointer,
                width: 2,
            }),
            Arg::Cell(Cell {
                r#ref: reference.clone(),
            }),
        ];
        operation.results = vec![
            Arg::Held(Held {
                value: pointer,
                width: 2,
            }),
            Arg::Cell(Cell {
                r#ref: reference.clone(),
            }),
        ];
        operation.raised = Some((operation.args.clone(), operation.results.clone()));
        operation.merges.insert(pointer, pointer);

        let provenance = Provenance {
            slices: BTreeSet::new(),
            restrict: BTreeSet::new(),
        };
        let interval = IntegerRange::new(0, 31, 2);
        let mut body = MirBody::new(0, vec![MirBlock::new(0, vec![], vec![operation], vec![])]);
        body.pointer_values.insert(pointer);
        body.pointer_seeds.insert(pointer, provenance.clone());
        body.integer_ranges.insert(pointer, interval.clone());
        body.loop_trip_counts = vec![(0, 3)];

        let rebuilt = resolved(&body, None).expect("one block is reducible");
        let operation = &rebuilt.blocks[0].ops[0];
        let used = Value {
            id: 1,
            at: 0,
            flags: false,
            variable: 7,
            version: 1,
        };
        let defined = Value {
            id: 2,
            at: 10,
            flags: false,
            variable: 7,
            version: 2,
        };
        assert_eq!(operation.uses, [used]);
        assert_eq!(operation.exits, [used]);
        assert_eq!(operation.defines, [defined]);
        assert_eq!(operation.loads[0].base, Some(used));
        assert_eq!(operation.stores[0].base, Some(used));
        assert!(matches!(&operation.args[0], Arg::Held(Held { value, .. }) if *value == used));
        assert!(
            matches!(&operation.results[0], Arg::Held(Held { value, .. }) if *value == defined)
        );
        assert_eq!(
            operation.raised,
            Some((operation.args.clone(), operation.results.clone()))
        );
        assert_eq!(
            operation.merges.iter().collect::<Vec<_>>(),
            vec![(&used, &defined)]
        );
        assert_eq!(rebuilt.pointer_values, BTreeSet::from([used, defined]));
        assert_eq!(
            rebuilt.pointer_seeds.iter().collect::<Vec<_>>(),
            vec![(&used, &provenance), (&defined, &provenance)]
        );
        assert!(
            rebuilt
                .integer_ranges
                .iter()
                .all(|(_, range)| range == &interval),
            "every renamed value retains the Python range fact"
        );
        assert_eq!(
            rebuilt
                .integer_ranges
                .keys()
                .copied()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([used, defined])
        );
        assert_eq!(rebuilt.loop_trip_counts, [(0, 3)]);
    }

    #[test]
    fn direct_mir_resolved_drops_conflicting_metadata_just_as_python_does() {
        let first = Value {
            id: 40,
            at: 0,
            flags: false,
            variable: 7,
            version: 1,
        };
        let second = Value {
            id: 41,
            at: 0,
            flags: false,
            variable: 7,
            version: 2,
        };
        let mut operation = Op::new(0, None::<OpCode>, "", vec![], vec![first, second]);
        operation.args = vec![
            Arg::Held(Held {
                value: first,
                width: 2,
            }),
            Arg::Held(Held {
                value: second,
                width: 2,
            }),
        ];
        let mut body = MirBody::new(0, vec![MirBlock::new(0, vec![], vec![operation], vec![])]);
        body.pointer_values.extend([first, second]);
        body.pointer_seeds
            .insert(first, Provenance::one(MemoryObject::new(MemoryKind::Frame)));
        body.pointer_seeds.insert(
            second,
            Provenance::one(MemoryObject::new(MemoryKind::Global)),
        );
        body.integer_ranges
            .insert(first, IntegerRange::new(0, 31, 2));
        body.integer_ranges
            .insert(second, IntegerRange::new(0, 63, 2));

        let rebuilt = resolved(&body, None).expect("one block is reducible");
        assert_eq!(rebuilt.pointer_values.len(), 1);
        assert!(rebuilt.pointer_seeds.is_empty());
        assert!(rebuilt.integer_ranges.is_empty());
    }

    #[test]
    fn direct_mir_resolved_refuses_python_error_shapes_and_drops_unreachable_blocks() {
        // Direct ports of qbopt/model/mir.py::resolved's three refusal paths
        // and its reachable-body filter.
        assert_eq!(
            resolved(&MirBody::new(0, vec![]), None),
            Err("no blocks to resolve".to_owned())
        );
        assert_eq!(
            resolved(
                &MirBody::new(-1, vec![MirBlock::new(0, vec![], vec![], vec![])]),
                None
            ),
            Err("the entry -0x001 is not one of these blocks".to_owned())
        );
        let irreducible = MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], vec![], vec![1, 2]),
                MirBlock::new(1, vec![], vec![], vec![2]),
                MirBlock::new(2, vec![], vec![], vec![1]),
            ],
        );
        assert_eq!(
            resolved(&irreducible, None),
            Err("the body's control flow is irreducible, so it has no dominator tree".to_owned())
        );
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], vec![], vec![1, 99]),
                MirBlock::new(1, vec![], vec![], vec![]),
                MirBlock::new(8, vec![], vec![], vec![]),
            ],
        );
        let rebuilt = resolved(&body, None).expect("unknown edges are ignored");
        assert_eq!(
            rebuilt
                .blocks
                .iter()
                .map(|block| block.at)
                .collect::<Vec<_>>(),
            [0, 1]
        );
        assert_eq!(rebuilt.blocks[0].succ, [1]);
    }

    #[test]
    fn op_keeps_all_python_defaults_and_tri_states() {
        let op = Op::new(0, OpCode::Operation(Operation::Move), "mov", vec![], vec![]);
        assert_eq!(op.array, None);
        assert!(op.memory_values.is_empty());
        assert_eq!(op.floating, None);
        assert_eq!(op.floating_origin, None);
        assert!(op.loads.is_empty() && op.stores.is_empty());
        assert!(!op.source_backed);
        assert_eq!(op.kind, Kind::Opaque);
        assert_eq!(op.stack, None);
        assert_eq!(op.test, None);
        assert!(op.merges.is_empty() && op.args.is_empty() && op.results.is_empty());
        assert_eq!(op.raised, None);
        assert_eq!(op.target, None);
        assert!(op.cases.is_empty());
        assert_eq!(op.id, None);
        assert_eq!(op.symbol, None);
        assert!(op.args_known);
        assert!(!op.memory_complete && !op.reads_complete && !op.volatile);
        assert_eq!(op.opaque_defs, Some(BTreeSet::new()));
        assert_eq!(op.opaque_uses, Some(BTreeSet::new()));
        assert!(op.absorbed.is_empty() && !op.indirect && op.exits.is_empty());

        let mut all_resources = op.clone();
        all_resources.opaque_defs = None;
        all_resources.opaque_uses = None;
        all_resources.symbol = Some(true);
        assert_ne!(op, all_resources);
        all_resources.symbol = Some(false);
        assert_ne!(op, all_resources);
        assert_eq!(all_resources.symbol, Some(false));
        assert_eq!(all_resources.opaque_defs, None);
    }

    #[test]
    fn computed_builds_a_source_free_mir_operation_with_ordered_unique_uses() {
        let first = Value::new(1, 7);
        let base = Value::new(2, 7);
        let segment = Value::new(3, 7);
        let result = Value::new(4, 7);
        let mut reference = MemRef::new(None, 2);
        reference.base = Some(base);
        reference.segment = Some(segment);
        let arguments = vec![
            Arg::Held(Held {
                value: first,
                width: 2,
            }),
            Arg::Held(Held {
                value: first,
                width: 2,
            }),
            Arg::Cell(Cell {
                r#ref: reference.clone(),
            }),
        ];

        let operation = computed(7, Kind::Add, result, arguments.clone(), 4);

        assert_eq!(operation.op, Some(OpCode::nothing()));
        assert_eq!(operation.name, "");
        assert_eq!(operation.defines, vec![result]);
        assert_eq!(operation.uses, vec![first, base, segment]);
        assert_eq!(operation.loads, vec![reference]);
        assert!(!operation.source_backed);
        assert_eq!(operation.kind, Kind::Add);
        assert_eq!(operation.args, arguments);
        assert_eq!(
            operation.results,
            vec![Arg::Held(Held {
                value: result,
                width: 4,
            })]
        );
        assert_eq!(operation.id, None);
        assert_eq!(operation.symbol, Some(false));
        assert!(operation.memory_complete);
        assert!(operation.reads_complete);
    }

    #[test]
    fn cleared_deletes_only_the_meaning_fields_from_python_mir_cleared() {
        // Direct port of qbopt/model/mir.py:cleared.  `replace()` retains
        // source occurrence ownership and every field not named there.
        let defined = Value::new(1, 7);
        let used = Value::new(2, 7);
        let memory = MemRef::new(None, 2);
        let held = Arg::Held(Held {
            value: used,
            width: 2,
        });
        let semantics = FloatingSemantics::new(
            vec![Format::Binary32],
            Format::Binary32,
            Precision::Exact,
            Rounding::None,
        );
        let floating_origin = FloatingOrigin {
            block: 3,
            sequence: vec![3, 7],
            at: 7,
            kind: Kind::Fadd,
            semantics: semantics.clone(),
            inputs: vec![held.clone()],
            outputs: vec![Arg::Held(Held {
                value: defined,
                width: 2,
            })],
            machine_inputs: vec![Arg::Const(Const::new(3, 2))],
            machine_outputs: vec![Arg::Const(Const::new(4, 2))],
        };
        let symbol = Symbol::new(Space::Segment, 2, 9, 2);
        let mut op = Op::new(
            7,
            OpCode::Synth(Synth::ConcatLow),
            "source spelling",
            vec![defined],
            vec![used],
        );
        op.array = Some(ArrayRequest::new(symbol, 2, vec![(0, 3)]));
        op.memory_values = vec![(memory.clone(), Const::new(5, 2))];
        op.floating = Some(semantics);
        op.floating_origin = Some(floating_origin);
        op.loads = vec![memory.clone()];
        op.stores = vec![memory.clone()];
        op.source_backed = true;
        op.kind = Kind::Add;
        op.stack = Some(-1);
        op.test = Some(Kind::Le);
        op.merges.insert(used, defined);
        op.args = vec![held.clone()];
        op.results = vec![Arg::Held(Held {
            value: defined,
            width: 2,
        })];
        op.raised = Some((op.args.clone(), op.results.clone()));
        op.target = Some(11);
        op.cases = vec![(1, 12)];
        op.id = Some(13);
        op.symbol = Some(true);
        op.args_known = false;
        op.memory_complete = true;
        op.reads_complete = true;
        op.volatile = true;
        op.opaque_defs = Some(BTreeSet::from(["es".to_owned()]));
        op.opaque_uses = None;
        op.absorbed = vec![14];
        op.indirect = true;
        op.exits = vec![defined];

        let result = cleared(&op);

        assert_eq!(result.kind, Kind::Nothing);
        assert!(result.name.is_empty());
        assert!(result.defines.is_empty());
        assert!(result.uses.is_empty());
        assert!(result.loads.is_empty());
        assert!(result.stores.is_empty());
        assert!(result.args.is_empty());
        assert!(result.results.is_empty());
        assert!(result.merges.is_empty());
        assert_eq!(result.raised, None);
        assert_eq!(result.target, None);
        assert_eq!(result.test, None);
        assert_eq!(result.stack, None);
        assert_eq!(result.symbol, Some(false));

        assert_eq!(result.at, op.at);
        assert_eq!(result.op, op.op);
        assert_eq!(result.array, op.array);
        assert_eq!(result.memory_values, op.memory_values);
        assert_eq!(result.floating, op.floating);
        assert_eq!(result.floating_origin, op.floating_origin);
        assert_eq!(result.source_backed, op.source_backed);
        assert_eq!(result.cases, op.cases);
        assert_eq!(result.id, op.id);
        assert_eq!(result.args_known, op.args_known);
        assert_eq!(result.memory_complete, op.memory_complete);
        assert_eq!(result.reads_complete, op.reads_complete);
        assert_eq!(result.volatile, op.volatile);
        assert_eq!(result.opaque_defs, op.opaque_defs);
        assert_eq!(result.opaque_uses, op.opaque_uses);
        assert_eq!(result.absorbed, op.absorbed);
        assert_eq!(result.indirect, op.indirect);
        assert_eq!(result.exits, op.exits);
    }

    #[test]
    fn floating_origin_preserves_the_python_identity_baseline() {
        let semantics = FloatingSemantics::new(
            vec![Format::Binary32],
            Format::Binary32,
            Precision::Exact,
            Rounding::None,
        );
        let input = Arg::Const(Const::new(1, 4));
        let output = Arg::Const(Const::new(2, 10));
        let machine_input = Arg::Const(Const::new(3, 4));
        let machine_output = Arg::Const(Const::new(4, 10));
        let origin = FloatingOrigin {
            block: -1,
            sequence: vec![-1, 4],
            at: 4,
            kind: Kind::Fadd,
            semantics: semantics.clone(),
            inputs: vec![input.clone()],
            outputs: vec![output.clone()],
            machine_inputs: vec![machine_input.clone()],
            machine_outputs: vec![machine_output.clone()],
        };
        assert_eq!(origin.semantics, semantics);
        assert_eq!(origin.inputs, [input]);
        assert_eq!(origin.outputs, [output]);
        assert_eq!(origin.machine_inputs, [machine_input]);
        assert_eq!(origin.machine_outputs, [machine_output]);
        assert_eq!(origin, origin.clone());
    }

    #[test]
    fn op_barrier_and_inserted_follow_python_properties() {
        let barrier = Op::new(0, OpCode::Operation(Operation::Barrier), "", vec![], vec![]);
        assert!(barrier.barrier());
        assert!(barrier.inserted());

        let mut volatile = Op::new(0, OpCode::Synth(Synth::ConcatLow), "", vec![], vec![]);
        volatile.volatile = true;
        volatile.absorbed = vec![11];
        assert!(volatile.barrier());
        assert!(!volatile.inserted());
    }

    #[test]
    fn stepping_is_the_single_affine_operation_question() {
        // Direct port of tests/test_mir.py::test_what_an_operation_steps_by_is_asked_in_one_place.
        let one = Arg::Held(Held {
            value: Value {
                id: 1,
                at: 0,
                flags: false,
                variable: 1,
                version: 1,
            },
            width: 2,
        });
        let other = Arg::Held(Held {
            value: Value {
                id: 2,
                at: 0,
                flags: false,
                variable: 2,
                version: 1,
            },
            width: 2,
        });
        let made = |kind: Kind, args: Vec<Arg>| {
            let mut op = Op::new(0, None::<OpCode>, kind.as_str(), vec![], vec![]);
            op.kind = kind;
            op.args = args;
            op
        };
        assert_eq!(
            stepping(&made(Kind::Increment, vec![one.clone()])),
            Some((one.clone(), Arg::Const(Const::new(1, 2))))
        );
        assert_eq!(
            stepping(&made(Kind::Decrement, vec![one.clone()])),
            Some((one.clone(), Arg::Const(Const::new(-1, 2))))
        );
        assert_eq!(
            stepping(&made(
                Kind::Sub,
                vec![one.clone(), Arg::Const(Const::new(4, 2))]
            )),
            Some((one.clone(), Arg::Const(Const::new(-4, 2))))
        );
        assert_eq!(
            stepping(&made(
                Kind::Sub,
                vec![one.clone(), Arg::Const(Const::new(i64::MIN, 8)),],
            )),
            Some((
                one.clone(),
                Arg::Const(Const::new(BigInt::from(1_u8) << 63, 8)),
            ))
        );
        assert_eq!(
            stepping(&made(Kind::Add, vec![one.clone(), other.clone()])),
            Some((one.clone(), other.clone()))
        );
        assert_eq!(stepping(&made(Kind::Sub, vec![one, other])), None);
        let mut memory = made(Kind::Increment, vec![Arg::Const(Const::new(0, 2))]);
        memory.loads = vec![MemRef::new(None, 2)];
        assert_eq!(stepping(&memory), None);
    }

    #[test]
    fn consumed_uses_operands_and_cell_address_values_but_not_exit_values() {
        let old = Value::new(1, 0);
        let result = Value::new(2, 0);
        let explicit = Value::new(3, 0);
        let base = Value::new(4, 0);
        let segment = Value::new(5, 0);
        let exposed_only = Value::new(6, 0);
        let mut cell = MemRef::new(None, 2);
        cell.base = Some(base);
        cell.segment = Some(segment);
        let mut op = Op::new(
            0,
            OpCode::Operation(Operation::Move),
            "mov",
            vec![],
            vec![old, result],
        );
        op.merges.insert(old, result);
        op.args = vec![
            Arg::Held(Held {
                value: old,
                width: 2,
            }),
            Arg::Held(Held {
                value: explicit,
                width: 2,
            }),
            Arg::Cell(Cell {
                r#ref: cell.clone(),
            }),
        ];
        op.results = vec![Arg::Cell(Cell { r#ref: cell })];
        op.exits = vec![exposed_only];
        assert_eq!(
            consumed(&op),
            BTreeSet::from([old, result, explicit, base, segment])
        );
    }

    /// A held result is written, not read; counting it kept dead pure calls
    /// and refused inlining a call whose result was otherwise unused.
    #[test]
    fn consumed_does_not_read_a_held_result() {
        let written = Value::new(1, 0);
        let mut op = Op::new(0, OpCode::Operation(Operation::Move), "mov", vec![], vec![]);
        op.results = vec![Arg::Held(Held { value: written, width: 2 })];
        assert!(consumed(&op).is_empty());
    }

    #[test]
    fn rewritten_requires_a_raised_snapshot_and_changed_operands() {
        let value = Value::new(1, 0);
        let mut op = Op::new(0, OpCode::Operation(Operation::Move), "mov", vec![], vec![]);
        op.args = vec![Arg::Held(Held { value, width: 2 })];
        op.raised = Some((op.args.clone(), op.results.clone()));
        assert!(!rewritten(&op));
        op.results = vec![Arg::Const(Const::new(7, 2))];
        assert!(rewritten(&op));
        op.raised = None;
        assert!(!rewritten(&op));
    }

    #[test]
    fn partial_distinguishes_a_word_result_from_a_partial_result() {
        let before = Value::new(1, 0);
        let after = Value::new(2, 0);
        let mut op = Op::new(0, OpCode::Operation(Operation::Move), "mov", vec![], vec![]);
        op.merges.insert(before, after);
        op.results = vec![Arg::Held(Held {
            value: after,
            width: 2,
        })];
        assert!(!partial(&op));
        op.results = vec![Arg::Held(Held {
            value: after,
            width: 4,
        })];
        assert!(partial(&op));
        op.results.clear();
        assert!(partial(&op), "an unnamed merge result is partial");
    }

    #[test]
    fn mir_body_values_and_exit_accessors_follow_the_python_model() {
        let phi_value = Value::new(1, -1);
        let defined = Value::new(2, 0);
        let used = Value::new(3, 0);
        let exiting = Value::new(4, 0);
        let later_phi = Value::new(5, 8);
        let later_defined = Value::new(6, 8);
        let nonterminal_exit = Value::new(7, 0);
        let mut predecessor = Op::new(
            -1,
            OpCode::Operation(Operation::Move),
            "mov",
            vec![defined],
            vec![],
        );
        predecessor.exits = vec![nonterminal_exit];
        let mut terminal = Op::new(
            7,
            OpCode::Operation(Operation::Return),
            "ret",
            vec![later_defined],
            vec![used],
        );
        terminal.exits = vec![exiting];
        let body = MirBody::new(
            -1,
            vec![
                MirBlock::new(-1, vec![Phi::new(phi_value)], vec![predecessor], vec![8]),
                MirBlock::new(8, vec![Phi::new(later_phi)], vec![terminal.clone()], vec![]),
            ],
        );
        assert_eq!(body.block(-1).map(|block| block.at), Some(-1));
        assert_eq!(body.block(99), None);
        assert_eq!(
            body.values(),
            vec![phi_value, defined, later_phi, later_defined]
        );
        assert_eq!(ordinary_uses(&terminal), [used]);
        assert_eq!(exit_values(&terminal), [exiting]);
        assert!(
            ordinary_uses(&terminal)
                .iter()
                .all(|value| !exit_values(&terminal).contains(value))
        );
        assert_eq!(exposed(&body), BTreeSet::from([exiting]));
        assert!(!exposed(&body).contains(&nonterminal_exit));
    }

    #[test]
    fn unheld_preserves_the_none_means_every_resource_tri_state() {
        let mut op = Op::new(0, OpCode::Operation(Operation::Nothing), "", vec![], vec![]);
        assert_eq!(
            unheld(&op),
            (&Some(BTreeSet::new()), &Some(BTreeSet::new()))
        );
        op.opaque_defs = None;
        op.opaque_uses = Some(BTreeSet::from(["st0".to_owned()]));
        let (written, read) = unheld(&op);
        assert_eq!(written, &None);
        assert_eq!(read, &Some(BTreeSet::from(["st0".to_owned()])));

        op.opaque_defs = Some(BTreeSet::new());
        op.opaque_uses = None;
        let (written, read) = unheld(&op);
        assert_eq!(written, &Some(BTreeSet::new()));
        assert_eq!(read, &None);
    }

    #[test]
    fn allocation_hints_from_body_follow_variable_and_definition_identity() {
        // Direct port of qbopt/model/mir.py:AllocationHints.from_body.  The
        // production raise/lower regression remains deferred until those
        // pipeline stages consume this Rust model.
        let first = Value {
            id: 1,
            at: 10,
            flags: false,
            variable: 7,
            version: 1,
        };
        let second = Value {
            id: 2,
            at: 20,
            flags: false,
            variable: 7,
            version: 2,
        };
        let mut first_op = Op::new(
            10,
            OpCode::Synth(Synth::ConcatLow),
            "first",
            vec![first],
            vec![],
        );
        first_op.id = Some(100);
        let mut second_op = Op::new(
            20,
            OpCode::Synth(Synth::ConcatLow),
            "second",
            vec![second],
            vec![],
        );
        second_op.id = Some(200);
        let mut raised = RaisedBody::new(MirBody::new(
            10,
            vec![MirBlock::new(
                10,
                vec![],
                vec![first_op.clone(), second_op.clone()],
                vec![],
            )],
        ));
        raised.origin.insert(first, iced_x86::Register::DL);
        raised.origin.insert(second, iced_x86::Register::DL);
        raised.pins.insert(first, iced_x86::Register::BL);
        let hints = AllocationHints::from_body(&raised).unwrap();
        assert_eq!(hints.origin_of(second), Some(iced_x86::Register::DL));
        assert_eq!(hints.pin_of(&first_op, 0), Some(iced_x86::Register::BL));
        assert_eq!(hints.pin_of(&second_op, 0), None);
        assert!(raised.pointer_values.is_empty());
    }

    #[test]
    fn a_pin_belongs_to_one_definition_not_every_ssa_version() {
        // Direct model-level port of
        // tests/test_rule5.py::test_a_pin_belongs_to_one_definition_not_every_ssa_version.
        let first = Value {
            id: 1,
            at: 10,
            flags: false,
            variable: 7,
            version: 1,
        };
        let second = Value {
            id: 2,
            at: 20,
            flags: false,
            variable: 7,
            version: 2,
        };
        let mut first_op = Op::new(
            10,
            OpCode::Synth(Synth::ConcatLow),
            "first",
            vec![first],
            vec![],
        );
        first_op.id = Some(100);
        let mut second_op = Op::new(
            20,
            OpCode::Synth(Synth::ConcatLow),
            "second",
            vec![second],
            vec![],
        );
        second_op.id = Some(200);
        let mut hints = AllocationHints::new();
        hints
            .pins
            .insert((first_op.id.unwrap(), 0), iced_x86::Register::BL);

        assert_eq!(hints.pin_of(&first_op, 0), Some(iced_x86::Register::BL));
        assert_eq!(hints.pin_of(&second_op, 0), None);
    }

    #[test]
    fn allocation_hints_refuse_conflicting_origins_and_missing_pins() {
        let first = Value {
            id: 1,
            at: 0,
            flags: false,
            variable: 7,
            version: 1,
        };
        let second = Value {
            id: 2,
            at: 1,
            flags: false,
            variable: 7,
            version: 2,
        };
        let mut raised = RaisedBody::new(MirBody::new(0, vec![]));
        raised.origin.insert(first, iced_x86::Register::AL);
        raised.origin.insert(second, iced_x86::Register::CL);
        let error = AllocationHints::from_body(&raised).unwrap_err();
        assert_eq!(
            error,
            AllocationHintsError::ConflictingOrigin {
                variable: 7,
                previous: iced_x86::Register::AL,
                location: iced_x86::Register::CL,
            }
        );
        assert_eq!(
            error.to_string(),
            "variable 7 has conflicting allocation hints: 1 and 2"
        );

        let mut missing = RaisedBody::new(MirBody::new(0, vec![]));
        missing.pins.insert(first, iced_x86::Register::DL);
        let error = AllocationHints::from_body(&missing).unwrap_err();
        assert_eq!(
            error,
            AllocationHintsError::MissingPinDefinition { value: first }
        );
        assert_eq!(
            error.to_string(),
            "pinned v7_1 has no source definition identity"
        );
    }

    #[test]
    fn allocation_hints_refuse_two_pins_for_one_source_definition_identity() {
        let first = Value::new(1, 0);
        let second = Value::new(2, 1);
        let mut first_op = Op::new(0, OpCode::Synth(Synth::ConcatLow), "", vec![first], vec![]);
        first_op.id = Some(9);
        let mut second_op = Op::new(1, OpCode::Synth(Synth::ConcatLow), "", vec![second], vec![]);
        second_op.id = Some(9);
        let mut raised = RaisedBody::new(MirBody::new(
            0,
            vec![MirBlock::new(0, vec![], vec![first_op, second_op], vec![])],
        ));
        raised.pins.insert(first, iced_x86::Register::AL);
        raised.pins.insert(second, iced_x86::Register::CL);
        let error = AllocationHints::from_body(&raised).unwrap_err();
        assert_eq!(
            error,
            AllocationHintsError::ConflictingPin {
                operation: 9,
                result: 0,
                previous: iced_x86::Register::AL,
                location: iced_x86::Register::CL,
            }
        );
        assert_eq!(
            error.to_string(),
            "definition (9, 0) has conflicting allocation pins: 1 and 2"
        );
    }

    fn direct_mir_verify_op(at: i64, defines: Vec<Value>, uses: Vec<Value>) -> Op {
        Op::new(at, None::<OpCode>, "", defines, uses)
    }

    #[test]
    fn direct_mir_verify_accepts_complete_phi_arguments_from_every_predecessor() {
        // Port of tests/test_mir.py::test_a_phi_argument_comes_from_every_predecessor.
        // Caller-defined inputs have no in-body definition and are in scope on
        // either predecessor, exactly as Python verify permits.
        let result = Value::new(3, 3);
        let mut phi = Phi::new(result);
        phi.incoming.insert(1, Value::new(1, 0));
        phi.incoming.insert(2, Value::new(2, 0));
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], vec![], vec![1, 2]),
                MirBlock::new(1, vec![], vec![], vec![3]),
                MirBlock::new(2, vec![], vec![], vec![3]),
                MirBlock::new(3, vec![phi], vec![], vec![]),
            ],
        );

        assert_eq!(verify(&body), Vec::<String>::new());
    }

    #[test]
    fn direct_mir_verify_reports_duplicate_phi_and_operation_definitions_in_source_order() {
        // Port of tests/test_mir.py::test_a_value_is_defined_exactly_once.
        let phi_value = Value::new(1, 0);
        let op_value = Value::new(2, 0);
        let body = MirBody::new(
            0,
            vec![MirBlock::new(
                0,
                vec![Phi::new(phi_value), Phi::new(phi_value)],
                vec![
                    direct_mir_verify_op(0, vec![op_value], vec![]),
                    direct_mir_verify_op(1, vec![op_value], vec![]),
                ],
                vec![],
            )],
        );

        assert_eq!(
            verify(&body),
            vec![
                "v1 defined twice".to_owned(),
                "v2 defined twice, at 0x0001".to_owned(),
            ]
        );
    }

    #[test]
    fn direct_mir_verify_keeps_the_later_definition_for_following_dominance_checks() {
        let repeated = Value::new(1, 0);
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(
                    0,
                    vec![],
                    vec![direct_mir_verify_op(0, vec![repeated], vec![])],
                    vec![1, 2],
                ),
                MirBlock::new(
                    1,
                    vec![],
                    vec![direct_mir_verify_op(1, vec![repeated], vec![])],
                    vec![],
                ),
                MirBlock::new(
                    2,
                    vec![],
                    vec![direct_mir_verify_op(2, vec![], vec![repeated])],
                    vec![],
                ),
            ],
        );

        assert_eq!(
            verify(&body),
            vec![
                "v1 defined twice, at 0x0001".to_owned(),
                "0x0002 uses v1, defined in 0x0001, which does not dominate it".to_owned(),
            ]
        );
    }

    #[test]
    fn direct_mir_verify_reports_missing_and_excess_phi_predecessors_with_python_lists() {
        let result = Value::new(3, 2);
        let caller = Value::new(4, 0);
        let mut missing = Phi::new(result);
        missing.incoming.insert(0, caller);
        let mut excess = Phi::new(Value::new(5, 2));
        excess.incoming.insert(0, caller);
        excess.incoming.insert(1, caller);
        excess.incoming.insert(3, caller);
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], vec![], vec![2]),
                MirBlock::new(1, vec![], vec![], vec![2]),
                MirBlock::new(2, vec![missing, excess], vec![], vec![]),
            ],
        );

        assert_eq!(
            verify(&body),
            vec![
                "v3 at 0x0002 has ['0x0'], its predecessors are ['0x0', '0x1']".to_owned(),
                "v5 at 0x0002 has ['0x0', '0x1', '0x3'], its predecessors are ['0x0', '0x1']"
                    .to_owned(),
            ]
        );
    }

    #[test]
    fn direct_mir_verify_reports_a_phi_value_that_does_not_reach_its_edge() {
        let defined = Value::new(1, 1);
        let mut phi = Phi::new(Value::new(2, 3));
        phi.incoming.insert(1, defined);
        phi.incoming.insert(2, defined);
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], vec![], vec![1, 2]),
                MirBlock::new(
                    1,
                    vec![],
                    vec![direct_mir_verify_op(1, vec![defined], vec![])],
                    vec![3],
                ),
                MirBlock::new(2, vec![], vec![], vec![3]),
                MirBlock::new(3, vec![phi], vec![], vec![]),
            ],
        );

        assert_eq!(
            verify(&body),
            vec!["v2 takes v1 from 0x0002, which it does not reach".to_owned()]
        );
    }

    #[test]
    fn direct_mir_verify_reports_a_same_block_use_before_its_definition() {
        // Port of tests/test_cfront.py::test_verify_reports_a_use_before_its_definition_in_one_block.
        let start = Value::new(1, 1);
        let limit = Value::new(2, 1);
        let body = MirBody::new(
            1,
            vec![MirBlock::new(
                1,
                vec![],
                vec![
                    direct_mir_verify_op(1, vec![limit], vec![start]),
                    direct_mir_verify_op(2, vec![start], vec![]),
                ],
                vec![],
            )],
        );

        assert_eq!(
            verify(&body),
            vec!["0x0001 uses v1 before its definition in 0x0001".to_owned()]
        );
    }

    #[test]
    fn direct_mir_verify_formats_negative_addresses_like_python() {
        assert_eq!(python_padded_hex(-1), "-0x001");
        assert_eq!(python_padded_hex(-16), "-0x010");
        assert_eq!(python_padded_hex(1), "0x0001");

        let start = Value::new(1, -1);
        let body = MirBody::new(
            -1,
            vec![MirBlock::new(
                -1,
                vec![],
                vec![
                    direct_mir_verify_op(-1, vec![Value::new(2, -1)], vec![start]),
                    direct_mir_verify_op(-16, vec![start], vec![]),
                ],
                vec![],
            )],
        );

        assert_eq!(
            verify(&body),
            vec!["-0x001 uses v1 before its definition in -0x001".to_owned()]
        );
    }

    #[test]
    fn direct_mir_verify_sorts_phi_address_diagnostics_by_python_hex_spelling() {
        let caller = Value::new(1, 0);
        let mut phi = Phi::new(Value::new(2, 16));
        phi.incoming.insert(2, caller);
        phi.incoming.insert(16, caller);
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], vec![], vec![2, 16]),
                MirBlock::new(2, vec![], vec![], vec![]),
                MirBlock::new(16, vec![phi], vec![], vec![]),
            ],
        );

        assert_eq!(
            verify(&body),
            vec!["v2 at 0x0010 has ['0x10', '0x2'], its predecessors are ['0x0']".to_owned()]
        );
    }

    #[test]
    fn direct_mir_verify_reports_non_dominating_use_and_exit() {
        let defined = Value::new(1, 1);
        let mut exit = direct_mir_verify_op(2, vec![], vec![defined]);
        exit.exits = vec![defined];
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], vec![], vec![1, 2]),
                MirBlock::new(
                    1,
                    vec![],
                    vec![direct_mir_verify_op(1, vec![defined], vec![])],
                    vec![],
                ),
                MirBlock::new(2, vec![], vec![exit], vec![]),
            ],
        );

        assert_eq!(
            verify(&body),
            vec![
                "0x0002 uses v1, defined in 0x0001, which does not dominate it".to_owned(),
                "0x0002 exposes v1, defined in 0x0001, which does not dominate it".to_owned(),
            ]
        );
    }
}
