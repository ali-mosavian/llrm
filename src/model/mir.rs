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

use crate::codegen::machine::{Loc, Operation};
use crate::model::floating::Semantics as FloatingSemantics;
use crate::model::memory::Provenance;
use crate::object::omf::module::{Addr, Space};
use crate::support::PhysicalRegister;

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
    pub origin: OrderedMap<Value, PhysicalRegister>,
    pub pins: OrderedMap<Value, PhysicalRegister>,
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
        previous: PhysicalRegister,
        location: PhysicalRegister,
    },
    MissingPinDefinition {
        value: Value,
    },
    ConflictingPin {
        operation: u32,
        result: usize,
        previous: PhysicalRegister,
        location: PhysicalRegister,
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
                "variable {variable} has conflicting allocation hints: {previous} and {location}"
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
                "definition ({operation}, {result}) has conflicting allocation pins: {previous} and {location}"
            ),
        }
    }
}

impl std::error::Error for AllocationHintsError {}

/// Backend-only placement history, outside public MIR semantics.
/// Direct port of `qbopt.model.mir:AllocationHints`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllocationHints {
    pub origins: OrderedMap<u32, PhysicalRegister>,
    pub pins: OrderedMap<(u32, usize), PhysicalRegister>,
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
    pub fn origin_of(&self, value: Value) -> Option<PhysicalRegister> {
        self.origins.get(&value.variable).copied()
    }

    /// Python `AllocationHints.pin_of`.
    pub fn pin_of(&self, operation: &Op, result: usize) -> Option<PhysicalRegister> {
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
    for arg in op.args.iter().chain(&op.results) {
        match arg {
            Arg::Held(held) => {
                result.insert(held.value);
            }
            Arg::Cell(cell) => {
                result.extend([cell.r#ref.base, cell.r#ref.segment].into_iter().flatten());
            }
            Arg::Const(_) | Arg::Symbol(_) | Arg::FrameAddress(_) | Arg::Opaque(_) => {}
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use num_bigint::BigInt;

    use crate::codegen::machine::Operation;
    use crate::model::floating::{Format, Precision, Rounding, Semantics as FloatingSemantics};
    use crate::model::memory::{MemoryKind, MemoryObject, ObjectIdentity, ObjectTag, Provenance};
    use crate::object::omf::module::{Addr, Space};
    use crate::support::PhysicalRegister;

    use super::{
        AllocationHints, AllocationHintsError, Arg, ArrayRequest, Cell, Const, FloatingOrigin,
        Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, Opaque, OrderedMap, Phi, RaisedBody,
        Symbol, Synth, Value, consumed, exit_values, exposed, ordinary_uses, partial, rewritten,
        same_bytes, stepping, unheld,
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
        raised.origin.insert(first, PhysicalRegister::new(3));
        raised.origin.insert(second, PhysicalRegister::new(3));
        raised.pins.insert(first, PhysicalRegister::new(4));
        let hints = AllocationHints::from_body(&raised).unwrap();
        assert_eq!(hints.origin_of(second), Some(PhysicalRegister::new(3)));
        assert_eq!(hints.pin_of(&first_op, 0), Some(PhysicalRegister::new(4)));
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
            .insert((first_op.id.unwrap(), 0), PhysicalRegister::new(4));

        assert_eq!(hints.pin_of(&first_op, 0), Some(PhysicalRegister::new(4)));
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
        raised.origin.insert(first, PhysicalRegister::new(1));
        raised.origin.insert(second, PhysicalRegister::new(2));
        let error = AllocationHints::from_body(&raised).unwrap_err();
        assert_eq!(
            error,
            AllocationHintsError::ConflictingOrigin {
                variable: 7,
                previous: PhysicalRegister::new(1),
                location: PhysicalRegister::new(2),
            }
        );
        assert_eq!(
            error.to_string(),
            "variable 7 has conflicting allocation hints: 1 and 2"
        );

        let mut missing = RaisedBody::new(MirBody::new(0, vec![]));
        missing.pins.insert(first, PhysicalRegister::new(3));
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
        raised.pins.insert(first, PhysicalRegister::new(1));
        raised.pins.insert(second, PhysicalRegister::new(2));
        let error = AllocationHints::from_body(&raised).unwrap_err();
        assert_eq!(
            error,
            AllocationHintsError::ConflictingPin {
                operation: 9,
                result: 0,
                previous: PhysicalRegister::new(1),
                location: PhysicalRegister::new(2),
            }
        );
        assert_eq!(
            error.to_string(),
            "definition (9, 0) has conflicting allocation pins: 1 and 2"
        );
    }
}
