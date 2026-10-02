//! Values and source-neutral MIR operands.
//!
//! Direct port of the source-neutral MIR definitions in `qbopt/model/mir.py`.
//!
//! Raising, live-out materialization, decoded source occurrences, and source
//! maps remain deliberately deferred: `tests/test_mir.py`'s corpus raise and
//! form gates, `tests/test_rule5.py`'s production exit-liveness gate, and the
//! raising-copy provenance regressions belong to the raiser/liveness/source-
//! map ports, not to this public schema slice.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt;
use std::hash::{Hash, Hasher};

use num_bigint::BigInt;

use crate::model::floating::Semantics as FloatingSemantics;
use crate::model::ir::{Loc, Operation, Semantics};
use crate::model::memory::Provenance;
use crate::objectfile::module::{Addr, Space};
use crate::support::pyrepr::{self, Repr};
use iced_x86::Register;

/// The registers that become values, rooted.
pub const TRACKED: [Register; 6] = llrm_x86_code16::GENERAL;

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
    /// Another agent may write the object at any time: an interrupt handler,
    /// the runtime. The access is `volatile`, ordered as one; reading it once
    /// before a loop that ends without it is the run where the write comes
    /// after the loop, so such a read may leave that loop.
    pub published: bool,
    /// The source language promises this access stays inside one object.
    /// Not part of equality or hashing, as Python's `compare=False`.
    pub inbounds: bool,
    /// The value holding the offset of the object's first byte, as the
    /// frontend promised it: the effective offset, `base` plus `addr`, is
    /// that plus a non-negative offset inside the object, and the object ends
    /// inside its segment. A weak reference: it keeps nothing alive, a pass
    /// that renames values renames it, and one that cannot drops it. Not part
    /// of equality or hashing.
    pub origin: Option<Value>,
}

impl MemRef {
    /// Whether its exclusions cover the frame bytes `[disp, disp + width)`;
    /// with no `disp`, a frame cell at no fixed place, whether they cover
    /// the whole frame.
    pub fn spares(&self, disp: Option<i64>, width: u32) -> bool {
        let (whole, size) = WHOLE_FRAME;
        let (low, high) = match disp {
            Some(disp) => (disp, disp + i64::from(width)),
            None => (whole.disp, whole.disp + i64::from(size)),
        };
        self.excludes.iter().any(|(start, size)| start.space == Space::Frame && start.disp <= low && high <= start.disp + i64::from(*size))
    }

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
            published: false,
            inbounds: false,
            origin: None,
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
            && self.published == other.published
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
        self.published.hash(state);
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
}

/// Direct port of `qbopt.model.mir:FrameAddress`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FrameAddress {
    pub offset: i64,
    pub width: u32,
    pub extent: Option<(i64, i64)>,
}

impl FrameAddress {
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
pub fn kind_of(what: &Semantics, args: &[Arg], results: &[Arg]) -> Kind {
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

pub const WHOLE_FRAME: (Addr, u32) = (Addr::new(Space::Frame, -(1 << 15)), 1 << 16);

/// Direct port of `qbopt.model.mir:_symbolic_ref`.
///
/// Analyses that need an address-based view of a reference must use this
/// normalization rather than spelling symbolic resolution themselves.
pub fn symbolic_ref(reference: &MemRef) -> Cow<'_, MemRef> {
    let Some(symbol) = reference.symbolic else {
        return Cow::Borrowed(reference);
    };
    let mut address = Addr::new(symbol.space, symbol.offset + symbol.addend);
    address.index = symbol.index;
    if reference.addr == Some(address) && reference.base.is_none() && reference.segment.is_none() {
        return Cow::Borrowed(reference);
    }
    let mut resolved = reference.clone();
    resolved.addr = Some(address);
    resolved.base = None;
    resolved.segment = None;
    Cow::Owned(resolved)
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
    // Device I/O, always volatile: c := the byte at port a; port a := byte b.
    // Memory reach is the port's device's, from `abi::ports`.
    PortIn,
    PortOut,
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
            Self::PortIn => "port_in",
            Self::PortOut => "port_out",
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
            Self::PortIn => "PORT_IN",
            Self::PortOut => "PORT_OUT",
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

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&K, &V)> {
        self.entries.iter().map(|(key, value)| (key, value))
    }

}

impl<K: Eq, V> OrderedMap<K, V> {
    pub fn get(&self, key: &K) -> Option<&V> {
        self.entries
            .iter()
            .find_map(|(candidate, value)| (candidate == key).then_some(value))
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
    pub const fn nothing() -> Self {
        Self::Operation(Operation::Nothing)
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

/// An operation's own identity: unique in its body and kept across passes,
/// so what a pass did reads as which operations it kept, copied and deleted.
/// `0` is not yet assigned. It is not content: operations differing only here
/// are equal, as Python's `field(compare=False)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct OpId(pub u32);

impl PartialEq for OpId {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for OpId {}

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
    /// The raise-time operation whose relocations, decoded node and folded
    /// site this one re-emits: a copy shares it.
    pub source: Option<u32>,
    pub id: OpId,
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
    /// The language promises the signed result fits its width: QBasic's FOR
    /// raises Overflow rather than wrap its counter. A pass that changes what
    /// the operation computes clears it.
    pub nowrap: bool,
    /// Python's `_RaisedOp` subclass: `Some` is an occurrence still inside the
    /// raise, carrying its decoded node and byte ranges.  `_externalized`
    /// clears it, so no completed body has one.
    pub raising: Option<Box<Raising>>,
}

/// The fields Python's private `_RaisedOp(Op)` adds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Raising {
    pub node: Option<std::sync::Arc<crate::model::ir::nodes::Node>>,
    pub covers: Option<(i64, i64)>,
    pub extra_covers: Vec<(i64, i64)>,
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
            source: None,
            id: OpId(0),
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
            nowrap: false,
            raising: None,
        }
    }

    /// Python `Op.barrier`.
    pub const fn barrier(&self) -> bool {
        matches!(self.op, Some(OpCode::Operation(Operation::Barrier))) || self.volatile
    }
}

/// Where definitions meet on CFG edges.  Direct port of `qbopt.model.mir:Phi`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Phi {
    pub result: Value,
    pub incoming: OrderedMap<i64, Value>,
}

impl Phi {
}

/// One MIR CFG block.  Direct port of `qbopt.model.mir:MirBlock`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirBlock {
    pub at: i64,
    pub phis: Vec<Phi>,
    pub ops: Vec<Op>,
    pub succ: Vec<i64>,
    // Reached only on a path the frontend expects never to run, such as
    // raising an error. Layout places it after the hot code.
    pub cold: bool,
}

impl MirBlock {
    pub fn new(at: i64, phis: Vec<Phi>, ops: Vec<Op>, succ: Vec<i64>) -> Self {
        Self {
            at,
            phis,
            ops,
            succ,
            cold: false,
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
    // SS == DS: the stack is in the data group, so DS reaches a frame object
    // through a near pointer. The raise's to say -- BC runs so, Watcom C not.
    pub stack_in_data: bool,
    pub pointer_values: BTreeSet<Value>,
    pub pointer_seeds: OrderedMap<Value, Provenance>,
    pub integer_ranges: OrderedMap<Value, IntegerRange>,
    pub loop_trip_counts: Vec<(i64, i64)>,
}

impl MirBody {

}

// Proof caches key on a body's identity, so a shared body must not change:
// no Cell, RefCell or Rc may hide inside one.  All three are !Sync.
const _: () = {
    const fn frozen<T: Sync>() {}
    frozen::<MirBody>();
};

/// Python `rewritten`.
pub fn rewritten(op: &Op) -> bool {
    op.raised
        .as_ref()
        .is_some_and(|raised| (&op.args, &op.results) != (&raised.0, &raised.1))
}

/// Python `_outside`: the frame's bytes no range in `reach` covers, as exclusions.
pub fn outside(reach: &BTreeSet<(i64, i64)>) -> Vec<(Addr, u32)> {
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

impl Repr for Value {
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
                ("inbounds", self.inbounds.repr()),
            ]
            .into_iter()
            // Python's MemRef has no origin: spelled only where there is one.
            .chain(self.origin.map(|origin| ("origin", origin.repr())))
            .collect::<Vec<_>>()
            .as_slice(),
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

// ---- early port (agent D) ----
