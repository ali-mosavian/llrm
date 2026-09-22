//! Port of `qbopt/cfront/raise_hir.py`: one procedure's trees as a MirBody.
//!
//! Every C variable is a frame cell and every tree node a fresh value, so the
//! body is in SSA by construction and promotion is left to the passes. The
//! ABI is Borland's medium model.
//!
//! Python's `eval` returns one of a dozen types; that union is `Got`.

use std::collections::{BTreeMap, BTreeSet};

use crate::support::hash::IndexMap;
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive, Zero};

use crate::abi::runtime;
use crate::analysis::alias;
use crate::cfront::hir::{self, Unsupported};
use crate::model::floating::{self, Format, Precision, Rounding};
use crate::model::ir::Operation;
use crate::model::memory::{Identity, MemoryKind, MemoryObject, Provenance};
use crate::model::mir::{
    self, AllocationHints, Arg, Cell, Const, FrameAddress, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode,
    RaisedBody, Value,
};
use crate::objectfile::module::{Addr, Space};
use crate::support::pyrepr::{self, Repr};

type R<T> = Result<T, Unsupported>;

/// `WIDTHS.get(type_)`.
pub fn widths(type_: &str) -> Option<u32> {
    Some(match type_ {
        "TY_UINT_1" | "TY_INT_1" => 1,
        "TY_UINT_2" | "TY_INT_2" => 2,
        "TY_UINT_4" | "TY_INT_4" => 4,
        "TY_UINT_8" | "TY_INT_8" => 8,
        "TY_INTEGER" | "TY_UNSIGNED" | "TY_BOOLEAN" => 2,
        "TY_NEAR_POINTER" | "TY_NEAR_CODE_PTR" => 2,
        "TY_LONG_POINTER" | "TY_HUGE_POINTER" | "TY_LONG_CODE_PTR" => 4,
        // A float moves as its bits; only arithmetic and conversion need the x87.
        "TY_SINGLE" => 4,
        "TY_DOUBLE" => 8,
        _ => return None,
    })
}

fn is_float(type_: &str) -> bool {
    matches!(type_, "TY_SINGLE" | "TY_DOUBLE")
}

fn float_arithmetic_kind(cg_op: &str) -> Option<Kind> {
    Some(match cg_op {
        "O_PLUS" => Kind::Fadd,
        "O_MINUS" => Kind::Fsub,
        "O_TIMES" => Kind::Fmul,
        "O_DIV" => Kind::Fdiv,
        _ => return None,
    })
}

fn float_unary_kind(cg_op: &str) -> Option<Kind> {
    Some(match cg_op {
        "O_UMINUS" => Kind::Fneg,
        "O_FABS" => Kind::Fabs,
        _ => return None,
    })
}

/// Operators OW's front end has a node for and Borland's library a routine.
fn library_routine(cg_op: &str) -> Option<&'static str> {
    Some(match cg_op {
        "O_SQRT" => "sqrt",
        "O_COS" => "cos",
        "O_SIN" => "sin",
        "O_TAN" => "tan",
        "O_ACOS" => "acos",
        "O_ASIN" => "asin",
        "O_ATAN" => "atan",
        "O_LOG" => "log",
        "O_LOG10" => "log10",
        "O_EXP" => "exp",
        "O_POW" => "pow",
        "O_ATAN2" => "atan2",
        "O_FMOD" => "fmod",
        _ => return None,
    })
}

/// Borland's pseudo-function laying its constant arguments down as code.
pub(crate) const EMITTED: [&str; 1] = ["__emit__"];
const EXTENDED: Format = Format::Extended80;

fn formats(width: u32) -> Format {
    match width {
        4 => Format::Binary32,
        8 => Format::Binary64,
        _ => panic!("KeyError: {width}"),
    }
}

fn integer_formats(width: u32) -> Format {
    match width {
        2 => Format::Signed16,
        4 => Format::Signed32,
        8 => Format::Signed64,
        _ => panic!("KeyError: {width}"),
    }
}

fn arith_rule() -> floating::Semantics {
    floating::Semantics::new([EXTENDED, EXTENDED], EXTENDED, Precision::Dynamic, Rounding::Dynamic)
}

fn exact_unary() -> floating::Semantics {
    floating::Semantics::new([EXTENDED], EXTENDED, Precision::Exact, Rounding::None)
}

fn _loaded(source: Format) -> floating::Semantics {
    floating::Semantics::new([source], EXTENDED, Precision::Exact, Rounding::None)
}

fn _stored(result: Format) -> floating::Semantics {
    floating::Semantics::new([EXTENDED], result, Precision::Destination, Rounding::Dynamic)
}

/// `struct.pack("<f" if width == 4 else "<d", value)`.
fn _packed(value: f64, width: u32) -> Vec<u8> {
    if width == 4 {
        let narrow = value as f32;
        if narrow.is_infinite() && value.is_finite() {
            panic!("OverflowError: float too large to pack with f format");
        }
        narrow.to_le_bytes().to_vec()
    } else {
        value.to_le_bytes().to_vec()
    }
}

/// `int.from_bytes(bits, "little", signed=True)` of four bytes.
fn signed_word(bits: &[u8]) -> BigInt {
    BigInt::from(i32::from_le_bytes(bits.try_into().expect("four bytes")))
}

fn signed(type_: &str) -> bool {
    matches!(type_, "TY_INT_1" | "TY_INT_2" | "TY_INT_4" | "TY_INT_8" | "TY_INTEGER")
}

pub(crate) fn far_pointers(type_: &str) -> bool {
    matches!(type_, "TY_LONG_POINTER" | "TY_HUGE_POINTER")
}

fn pointers(type_: &str) -> bool {
    matches!(type_, "TY_POINTER" | "TY_NEAR_POINTER" | "TY_LONG_POINTER" | "TY_HUGE_POINTER")
}

/// Standard allocation contracts: language-library semantics, not fixture recognition.
fn fresh_allocators(name: &str) -> bool {
    matches!(name, "malloc" | "_malloc" | "calloc" | "_calloc")
}

/// C's aliasing classes.
fn classes(type_: &str) -> Option<&'static str> {
    Some(match type_ {
        "TY_INT_2" | "TY_UINT_2" | "TY_INTEGER" | "TY_UNSIGNED" => "int2",
        "TY_INT_4" | "TY_UINT_4" => "int4",
        "TY_INT_8" | "TY_UINT_8" => "int8",
        "TY_SINGLE" => "float4",
        "TY_DOUBLE" => "float8",
        "TY_NEAR_POINTER" => "pointer2",
        "TY_LONG_POINTER" | "TY_HUGE_POINTER" => "pointer4",
        _ => return None,
    })
}

/// What a callee reads and writes: no byte this body can name.
fn callee_refs() -> Vec<MemRef> {
    vec![MemRef::new(None, 4)]
}

/// Literal labels share the symbol index space; float constants the raise places come after them.
pub const LITERAL: i64 = 1 << 20;
pub const POOL: i64 = 1 << 21;
/// Space.GROUP index of the selector of the segment symbol n is in; 0 is DGROUP's.
pub const SELECTOR: i64 = 1 << 22;

/// `TESTS`: (signed, unsigned).
fn tests(cg_op: &str) -> (Kind, Kind) {
    match cg_op {
        "O_EQ" => (Kind::Eq, Kind::Eq),
        "O_NE" => (Kind::Ne, Kind::Ne),
        "O_LT" => (Kind::Lt, Kind::Below),
        "O_LE" => (Kind::Le, Kind::BelowEq),
        "O_GT" => (Kind::Gt, Kind::Above),
        "O_GE" => (Kind::Ge, Kind::AboveEq),
        _ => panic!("KeyError: {cg_op:?}"),
    }
}

fn inverse(kind: Kind) -> Kind {
    match kind {
        Kind::Eq => Kind::Ne,
        Kind::Ne => Kind::Eq,
        Kind::Lt => Kind::Ge,
        Kind::Ge => Kind::Lt,
        Kind::Le => Kind::Gt,
        Kind::Gt => Kind::Le,
        Kind::Below => Kind::AboveEq,
        Kind::AboveEq => Kind::Below,
        Kind::BelowEq => Kind::Above,
        Kind::Above => Kind::BelowEq,
        _ => panic!("KeyError: {kind}"),
    }
}

/// `ARITHMETIC`: the kind, and whether it commutes.
fn arithmetic_kind(cg_op: &str) -> Option<(Kind, bool)> {
    Some(match cg_op {
        "O_PLUS" => (Kind::Add, true),
        "O_MINUS" => (Kind::Sub, false),
        "O_AND" => (Kind::And, true),
        "O_OR" => (Kind::Or, true),
        "O_XOR" => (Kind::Xor, true),
        "O_TIMES" => (Kind::Mul, true),
        _ => return None,
    })
}

// Addresses the raise holds before any of them is a value.

#[derive(Clone, Debug)]
pub struct Frame {
    pub disp: i64,
    /// the aliasing class of the scalar it names
    pub declared: Option<String>,
    pub volatile: bool,
}

#[derive(Clone, Debug)]
pub struct Global {
    pub space: Space,
    pub index: i64,
    pub disp: i64,
    /// an index into the symbol, `_arr[j]`
    pub base: Option<Value>,
    pub declared: Option<String>,
    pub volatile: bool,
}

#[derive(Clone, Debug)]
pub struct Near {
    pub base: Value,
    pub disp: i64,
    /// The source-language storage whose default selector is required.
    pub space: Space,
    pub volatile: bool,
}

#[derive(Clone, Debug)]
pub struct Far {
    pub segment: Value,
    pub offset: Value,
    pub disp: i64,
    /// the 4-byte pointer these halves were split from, at disp 0
    pub whole: Option<Value>,
    /// the selector symbol when the segment is a named object's own, else 0
    pub named: i64,
    pub declared: Option<String>,
    pub volatile: bool,
}

#[derive(Clone, Debug)]
pub enum Address {
    Frame(Frame),
    Global(Global),
    Near(Near),
    Far(Far),
}

impl Address {
    fn frame(disp: i64) -> Self {
        Address::Frame(Frame { disp, declared: None, volatile: false })
    }

    fn near(base: Value, disp: i64) -> Self {
        Address::Near(Near { base, disp, space: Space::Literal, volatile: false })
    }

    fn disp(&self) -> i64 {
        match self {
            Address::Frame(one) => one.disp,
            Address::Global(one) => one.disp,
            Address::Near(one) => one.disp,
            Address::Far(one) => one.disp,
        }
    }

    /// `replace(address, disp=...)`.
    fn at(&self, disp: i64) -> Self {
        let mut moved = self.clone();
        match &mut moved {
            Address::Frame(one) => one.disp = disp,
            Address::Global(one) => one.disp = disp,
            Address::Near(one) => one.disp = disp,
            Address::Far(one) => one.disp = disp,
        }
        moved
    }

    /// `replace(address, volatile=True)`.
    fn volatile(&self) -> Self {
        let mut marked = self.clone();
        match &mut marked {
            Address::Frame(one) => one.volatile = true,
            Address::Global(one) => one.volatile = true,
            Address::Near(one) => one.volatile = true,
            Address::Far(one) => one.volatile = true,
        }
        marked
    }

    fn is_volatile(&self) -> bool {
        match self {
            Address::Frame(one) => one.volatile,
            Address::Global(one) => one.volatile,
            Address::Near(one) => one.volatile,
            Address::Far(one) => one.volatile,
        }
    }

    /// `getattr(address, "declared", None)`: a Near has none.
    fn declared(&self) -> Option<&String> {
        match self {
            Address::Frame(one) => one.declared.as_ref(),
            Address::Global(one) => one.declared.as_ref(),
            Address::Near(_) => None,
            Address::Far(one) => one.declared.as_ref(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Aggregate {
    pub address: Address,
    pub size: i64,
}

#[derive(Clone, Debug)]
pub struct Returned {
    pub low: Value,
    pub high: Value,
}

/// The lvalue of a pointer declared restrict, before it is loaded.
#[derive(Clone, Debug)]
pub struct Restricted {
    pub value: Got,
    pub root: Identity,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub symbol: hir::Symbol,
}

/// A float literal: its bits where it is moved, the x87's where it is computed.
#[derive(Clone, Debug)]
pub struct Real {
    pub value: f64,
    pub width: u32,
}

/// A float in memory, not yet read: moved as bytes or loaded onto the x87.
#[derive(Clone, Debug)]
pub struct FloatCell {
    pub address: Address,
    pub width: u32,
}

/// `type Operand = mir.Held | mir.Const`.
#[derive(Clone, Debug)]
pub enum Operand {
    Held(Held),
    Const(Const),
}

impl Operand {
    fn width(&self) -> u32 {
        match self {
            Operand::Held(one) => one.width,
            Operand::Const(one) => one.width,
        }
    }

    fn arg(&self) -> Arg {
        match self {
            Operand::Held(one) => Arg::Held(*one),
            Operand::Const(one) => Arg::Const(one.clone()),
        }
    }

    fn got(self) -> Got {
        match self {
            Operand::Held(one) => Got::Held(one),
            Operand::Const(one) => Got::Const(one),
        }
    }
}

/// Whatever `eval` returns.
#[derive(Clone, Debug)]
pub enum Got {
    Held(Held),
    Const(Const),
    Address(Address),
    Aggregate(Box<Aggregate>),
    Returned(Returned),
    Restricted(Box<Restricted>),
    Function(Box<Function>),
    Real(Real),
    FloatCell(FloatCell),
}

fn held(value: Value, width: u32) -> Held {
    Held { value, width }
}

fn constant(n: impl Into<BigInt>, width: u32) -> Const {
    Const::new(n, width)
}

impl Repr for Frame {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Frame",
            &[("disp", self.disp.repr()), ("declared", self.declared.repr()), ("volatile", self.volatile.repr())],
        )
    }
}

impl Repr for Global {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Global",
            &[
                ("space", self.space.repr()),
                ("index", self.index.repr()),
                ("disp", self.disp.repr()),
                ("base", self.base.repr()),
                ("declared", self.declared.repr()),
                ("volatile", self.volatile.repr()),
            ],
        )
    }
}

impl Repr for Near {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Near",
            &[
                ("base", self.base.repr()),
                ("disp", self.disp.repr()),
                ("space", self.space.repr()),
                ("volatile", self.volatile.repr()),
            ],
        )
    }
}

impl Repr for Far {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Far",
            &[
                ("segment", self.segment.repr()),
                ("offset", self.offset.repr()),
                ("disp", self.disp.repr()),
                ("whole", self.whole.repr()),
                ("named", self.named.repr()),
                ("declared", self.declared.repr()),
                ("volatile", self.volatile.repr()),
            ],
        )
    }
}

impl Repr for Address {
    fn repr(&self) -> String {
        match self {
            Address::Frame(one) => one.repr(),
            Address::Global(one) => one.repr(),
            Address::Near(one) => one.repr(),
            Address::Far(one) => one.repr(),
        }
    }
}

impl Repr for Got {
    fn repr(&self) -> String {
        match self {
            Got::Held(one) => one.repr(),
            Got::Const(one) => one.repr(),
            Got::Address(one) => one.repr(),
            Got::Aggregate(one) => {
                pyrepr::dataclass("Aggregate", &[("address", one.address.repr()), ("size", one.size.repr())])
            }
            Got::Returned(one) => pyrepr::dataclass("Returned", &[("low", one.low.repr()), ("high", one.high.repr())]),
            Got::Restricted(one) => {
                pyrepr::dataclass("Restricted", &[("value", one.value.repr()), ("root", one.root.repr())])
            }
            Got::Function(one) => pyrepr::dataclass("Function", &[("symbol", one.symbol.repr())]),
            Got::Real(one) => pyrepr::dataclass("Real", &[("value", one.value.repr()), ("width", one.width.repr())]),
            Got::FloatCell(one) => {
                pyrepr::dataclass("FloatCell", &[("address", one.address.repr()), ("width", one.width.repr())])
            }
        }
    }
}

impl Repr for Operand {
    fn repr(&self) -> String {
        match self {
            Operand::Held(one) => one.repr(),
            Operand::Const(one) => one.repr(),
        }
    }
}

/// An operation still in the raise: its branch targets are labels until `run` places the blocks.
#[derive(Clone, Debug)]
struct Pending {
    op: Op,
    target: Option<String>,
    cases: Vec<(i64, String)>,
}

#[derive(Clone, Debug)]
struct _Block {
    key: String,
    ops: Vec<Pending>,
    succ: Vec<String>,
    ended: bool,
}

/// What the raise returns for one procedure.
#[derive(Clone, Debug)]
pub struct Raised {
    pub name: String,
    pub symbol: hir::Symbol,
    pub body: MirBody,
    pub hints: AllocationHints,
    /// call site -> callee object name
    pub calls: IndexMap<i64, String>,
    pub callees: IndexMap<i64, hir::Symbol>,
    pub contracts: IndexMap<i64, runtime::Contract>,
    /// Source-order actual pointer provenance.
    pub arguments: IndexMap<i64, Vec<alias::Actual>>,
    /// Source-order scalar constants, kept outside MIR for module specialization.
    pub constants: IndexMap<i64, Vec<Option<Const>>>,
    /// Entry cells occupied by source parameters, in declaration order.
    pub parameters: Vec<MemRef>,
    /// A site whose callee is inline assembly: its bytes, and (kind, name, offset) where a symbol goes.
    pub inline: IndexMap<i64, Vec<InlinePart>>,
}

pub use crate::backend::masm::InlinePart;

/// What a module's procedures raise into together.
#[derive(Clone, Debug, Default)]
pub struct Shared {
    /// a constant's bits -> its number
    pub literals: IndexMap<Vec<u8>, i64>,
    /// routines no declaration names
    pub runtime: IndexMap<String, hir::Symbol>,
}

pub fn raised(unit: &hir::Unit, proc: &hir::Proc, shared: &mut Shared) -> R<Raised> {
    _Raise::new(unit, proc, shared)?.run()
}

/// Every (space, index) a raised operand can name, as its object name.
pub fn names(unit: &hir::Unit, shared: Option<&Shared>) -> IndexMap<(Space, i64), String> {
    let mut out: IndexMap<(Space, i64), String> =
        unit.symbols.values().map(|one| ((_space(one), one.id), one.object_name())).collect();
    out.insert((Space::Group, 0), "DGROUP".to_owned());
    for one in unit.symbols.values() {
        // Procedure symbols name code, never DGROUP.
        if one.proc() || !unit.grouped(one) {
            out.insert((Space::Group, SELECTOR + one.id), format!("seg {}", one.object_name()));
        }
    }
    for (back, symbol) in &unit.backs {
        if *symbol == 0 {
            out.insert((Space::Segment, LITERAL + back), format!("L_b{back}"));
        }
    }
    if let Some(shared) = shared {
        for n in shared.literals.values() {
            out.insert((Space::Segment, POOL + n), format!("L_f{n}"));
        }
    }
    out
}

fn _space(symbol: &hir::Symbol) -> Space {
    if symbol.imported() { Space::External } else { Space::Segment }
}

fn _even(n: i64) -> i64 {
    n + (n & 1)
}

/// The keyword fields `_Raise.op` passes through to `mir.Op`.
#[derive(Default)]
struct Extra {
    loads: Vec<MemRef>,
    stores: Vec<MemRef>,
    target: Option<String>,
    cases: Vec<(i64, String)>,
    test: Option<Kind>,
    floating: Option<floating::Semantics>,
    memory_complete: bool,
    reads_complete: bool,
    symbol: Option<bool>,
    indirect: bool,
}

struct _Raise<'a> {
    unit: &'a hir::Unit,
    proc: &'a hir::Proc,
    shared: &'a mut Shared,
    symbol: hir::Symbol,
    values: u32,
    at: i64,
    anonymous: i64,
    blocks: Vec<_Block>,
    current: Option<usize>,
    done: BTreeMap<i64, Got>,
    calls: IndexMap<i64, String>,
    callees: IndexMap<i64, hir::Symbol>,
    contracts: IndexMap<i64, runtime::Contract>,
    arguments: IndexMap<i64, Vec<alias::Actual>>,
    constants: IndexMap<i64, Vec<Option<Const>>>,
    inline: IndexMap<i64, Vec<InlinePart>>,
    frame: IndexMap<String, i64>,
    /// each frame object's bytes
    objects: Vec<(i64, i64)>,
    pointer_values: BTreeSet<Value>,
    pointer_seeds: mir::OrderedMap<Value, Provenance>,
    parameter_at: IndexMap<i64, i64>,
    selects: IndexMap<String, (Vec<(i64, String)>, Option<String>)>,
    down: i64,
}

impl<'a> _Raise<'a> {
    fn new(unit: &'a hir::Unit, proc: &'a hir::Proc, shared: &'a mut Shared) -> R<Self> {
        let symbol = unit.symbols[&proc.symbol].clone();
        if symbol.call_class & hir::CALLER_POPS == 0 || symbol.call_class & hir::REVERSE_PARMS != 0 {
            return Err(Unsupported(format!("{}: only cdecl procedures are defined", symbol.name)));
        }
        let mut raise = _Raise {
            unit,
            proc,
            shared,
            at: 0,
            values: 0,
            anonymous: 0,
            blocks: Vec::new(),
            current: None,
            done: BTreeMap::new(),
            calls: IndexMap::default(),
            callees: IndexMap::default(),
            contracts: IndexMap::default(),
            arguments: IndexMap::default(),
            constants: IndexMap::default(),
            inline: IndexMap::default(),
            frame: IndexMap::default(),
            objects: Vec::new(),
            pointer_values: BTreeSet::new(),
            pointer_seeds: mir::OrderedMap::new(),
            parameter_at: IndexMap::default(),
            selects: IndexMap::default(),
            down: 0,
            symbol,
        };
        let mut at = if raise.symbol.far() { 6 } else { 4 };
        for (number, (symbol, type_)) in proc.parms.iter().enumerate() {
            raise.frame.insert(format!("y{symbol}"), at);
            raise.parameter_at.insert(at, number as i64);
            let size = _even(2.max(raise.size(type_)?));
            raise.objects.push((at, at + size));
            at += size;
        }
        raise.down = 0;
        for (key, type_) in &proc.autos {
            let size = raise.size(type_)?;
            let slot = raise.slot(size);
            raise.frame.insert(key.clone(), slot);
        }
        Ok(raise)
    }

    fn name(&self) -> &str {
        &self.symbol.name
    }

    fn unsupported<T>(&self, message: impl std::fmt::Display) -> R<T> {
        Err(Unsupported(format!("{}: {message}", self.symbol.name)))
    }

    /// A new frame cell below the last.
    fn slot(&mut self, size: i64) -> i64 {
        self.down -= _even(size);
        self.objects.push((self.down, self.down + _even(size)));
        self.down
    }

    /// The frame object holding `disp`: C keeps an address into an object inside it.
    fn extent(&self, disp: i64) -> Option<(i64, i64)> {
        self.objects.iter().copied().find(|(low, high)| *low <= disp && disp < *high)
    }

    // ---- types ----

    fn width(&self, type_: &str) -> R<u32> {
        let type_ = self.unit.canonical_type(type_);
        if type_ == "TY_POINTER" {
            return Ok(if self.unit.target & hir::BIG_DATA != 0 { 4 } else { 2 });
        }
        if type_ == "TY_CODE_PTR" {
            return Ok(if self.unit.target & hir::BIG_CODE != 0 { 4 } else { 2 });
        }
        if let Some(width) = widths(&type_) {
            return Ok(width);
        }
        self.unsupported(format!("no scalar width for {type_}"))
    }

    fn size(&self, type_: &str) -> R<i64> {
        let type_ = self.unit.canonical_type(type_);
        match self.unit.types.get(&type_) {
            Some(size) => Ok(*size),
            None => Ok(i64::from(self.width(&type_)?)),
        }
    }

    fn far_pointer(&self, type_: &str) -> bool {
        let type_ = self.unit.canonical_type(type_);
        far_pointers(&type_) || (type_ == "TY_POINTER" && self.unit.target & hir::BIG_DATA != 0)
    }

    // ---- blocks and operations ----

    fn fresh(&mut self) -> Value {
        self.fresh_value(false)
    }

    fn fresh_value(&mut self, flags: bool) -> Value {
        self.values += 1;
        Value { id: self.values, at: self.at + 1, flags, variable: self.values, version: 1 }
    }

    fn start(&mut self, key: String) {
        if let Some(current) = self.current.filter(|current| !self.blocks[*current].ended) {
            self.blocks[current].succ.push(key.clone());
        }
        self.blocks.push(_Block { key, ops: Vec::new(), succ: Vec::new(), ended: false });
        self.current = Some(self.blocks.len() - 1);
        self.op(Kind::Nothing, vec![], vec![], None, None, Extra::default());
    }

    fn label(&mut self) -> String {
        self.anonymous += 1;
        format!("a{}", self.anonymous)
    }

    fn end(&mut self, succ: &[String]) {
        let current = self.current.expect("a current block");
        self.blocks[current].succ.extend(succ.iter().cloned());
        self.blocks[current].ended = true;
    }

    /// The operation made, and returns its `at`.
    fn op(
        &mut self,
        kind: Kind,
        results: Vec<Arg>,
        args: Vec<Arg>,
        defines: Option<Vec<Value>>,
        uses: Option<Vec<Value>>,
        mut extra: Extra,
    ) -> i64 {
        if self.current.is_none_or(|current| self.blocks[current].ended) {
            let label = self.label();
            self.start(label);
        }
        self.at += 1;
        let defines = defines.unwrap_or_else(|| {
            results
                .iter()
                .filter_map(|one| match one {
                    Arg::Held(one) => Some(one.value),
                    _ => None,
                })
                .collect()
        });
        let uses = uses.unwrap_or_else(|| {
            let mut read: Vec<Value> = args
                .iter()
                .filter_map(|one| match one {
                    Arg::Held(one) => Some(one.value),
                    _ => None,
                })
                .collect();
            for one in args.iter().chain(&results) {
                if let Arg::Cell(cell) = one {
                    read.extend([cell.r#ref.base, cell.r#ref.segment].into_iter().flatten());
                }
            }
            let mut unique = Vec::new();
            for one in read {
                if !unique.contains(&one) {
                    unique.push(one);
                }
            }
            unique
        });
        let observable = extra.loads.iter().chain(&extra.stores).any(|one| one.volatile);
        let mut made = Op::new(self.at, OpCode::Operation(Operation::Nothing), "", defines, uses);
        if observable {
            // The explicit references are the complete footprint.
            extra.memory_complete = true;
            extra.reads_complete = true;
            made.volatile = true;
        }
        made.kind = kind;
        made.args = args;
        made.results = results;
        made.id = Some(mir::next_id());
        made.loads = extra.loads;
        made.stores = extra.stores;
        made.test = extra.test;
        made.floating = extra.floating;
        made.memory_complete = extra.memory_complete;
        made.reads_complete = extra.reads_complete;
        made.symbol = extra.symbol;
        made.indirect = extra.indirect;
        let at = made.at;
        let current = self.current.expect("a current block");
        self.blocks[current].ops.push(Pending { op: made, target: extra.target, cases: extra.cases });
        at
    }

    fn run(mut self) -> R<Raised> {
        self.start("entry".to_owned());
        for one in &self.proc.body {
            self.statement(one)?;
        }
        if !self.blocks[self.current.expect("a current block")].ended {
            return self.unsupported("control reaches the end with no return");
        }
        let mut reached: BTreeSet<String> = BTreeSet::new();
        let mut queue = vec!["entry".to_owned()];
        let by_key: IndexMap<&str, &_Block> = self.blocks.iter().map(|block| (block.key.as_str(), block)).collect();
        while let Some(key) = queue.pop() {
            if reached.insert(key.clone()) {
                queue.extend(by_key[key.as_str()].succ.iter().cloned());
            }
        }
        let kept: Vec<&_Block> = self.blocks.iter().filter(|block| reached.contains(&block.key)).collect();
        let at: IndexMap<&str, i64> = kept.iter().map(|block| (block.key.as_str(), block.ops[0].op.at)).collect();
        let blocks: Vec<MirBlock> = kept
            .iter()
            .map(|block| {
                let ops = block
                    .ops
                    .iter()
                    .map(|pending| {
                        let mut op = pending.op.clone();
                        if let Some(target) = &pending.target {
                            op.target = Some(at[target.as_str()]);
                            op.cases = pending.cases.iter().map(|(n, label)| (*n, at[label.as_str()])).collect();
                        }
                        op
                    })
                    .collect();
                MirBlock::new(at[block.key.as_str()], vec![], ops, block.succ.iter().map(|key| at[key.as_str()]).collect())
            })
            .collect();
        let mut body = MirBody::new(blocks[0].at, blocks);
        body.sealed = true;
        body.pointer_values = self.pointer_values.clone();
        body.pointer_seeds = self.pointer_seeds.clone();
        let body = RaisedBody::new(mir::frame_bounded(body, true));
        let body = RaisedBody { body: alias::annotated(&body.body).map_err(Unsupported)?, ..body };
        // Even an unknown C callee has a precise language-level boundary.
        let procedure =
            alias::Procedure { body: body.body.clone(), calls: self.calls.clone(), arguments: self.arguments.clone() };
        let body = RaisedBody { body: alias::calls_annotated(&procedure, &IndexMap::default()).map_err(Unsupported)?, ..body };
        let body = mir::with_live_outs(body);
        let problems = mir::verify(&body);
        if !problems.is_empty() {
            let first: Vec<String> = problems.into_iter().take(3).collect();
            return self.unsupported(format!("raised MIR is not SSA: {}", pyrepr::list(&first)));
        }
        let hints = AllocationHints::from_body(&body).unwrap_or_else(|error| panic!("ValueError: {error}"));
        let mut parameters = Vec::new();
        for (symbol, type_) in &self.proc.parms {
            let size = self.size(type_)?;
            parameters.push(self.placed(&Address::frame(self.frame[&format!("y{symbol}")]), size as u32));
        }
        Ok(Raised {
            name: self.symbol.object_name(),
            symbol: self.symbol.clone(),
            body: mir::public(body),
            hints,
            calls: self.calls,
            callees: self.callees,
            contracts: self.contracts,
            arguments: self.arguments,
            constants: self.constants,
            parameters,
            inline: self.inline,
        })
    }

    // ---- statements ----

    fn statement(&mut self, one: &hir::Statement) -> R<()> {
        let args: Vec<&str> = one.args.iter().map(String::as_str).collect();
        match (one.call.as_str(), &args[..]) {
            ("CGDone" | "CGTrash", [node]) => {
                self.eval(node)?;
            }
            ("CGControl", ["O_LABEL", _, label]) => self.start((*label).to_owned()),
            ("CGControl", ["O_GOTO", _, label]) => {
                let extra = Extra { target: Some((*label).to_owned()), ..Extra::default() };
                self.op(Kind::Jump, vec![], vec![], None, None, extra);
                self.end(&[(*label).to_owned()]);
            }
            ("CGControl", [test @ ("O_IF_TRUE" | "O_IF_FALSE"), node, label]) => {
                self.branch(node, label, *test == "O_IF_TRUE")?;
            }
            ("CGReturn", [node, type_]) => self.ret(node, type_)?,
            ("CGSelInit", [select]) => {
                self.selects.insert((*select).to_owned(), (Vec::new(), None));
            }
            ("CGSelCase", [select, label, value]) => {
                let cases = &mut self.selects.get_mut(*select).expect("KeyError").0;
                cases.push((hir::int(value), (*label).to_owned()));
            }
            ("CGSelOther", [select, label]) => {
                let cases = self.selects[*select].0.clone();
                self.selects.insert((*select).to_owned(), (cases, Some((*label).to_owned())));
            }
            ("CGSelect", [select, node]) if self.selects[*select].1.is_some() => {
                let (cases, other) = self.selects.shift_remove(*select).expect("KeyError");
                let other = other.expect("a default label");
                let type_ = self.type_of(node);
                let got = self.eval(node)?;
                let operand = self.operand(got, &type_)?;
                let width = 2.max(self.width(&type_)?);
                let value = self.narrowed(operand, width)?;
                let extra = Extra { target: Some(other.clone()), cases: cases.clone(), ..Extra::default() };
                self.op(Kind::Switch, vec![], vec![value.arg()], None, None, extra);
                let mut succ: Vec<String> = Vec::new();
                for label in std::iter::once(&other).chain(cases.iter().map(|(_, label)| label)) {
                    if !succ.contains(label) {
                        succ.push(label.clone());
                    }
                }
                self.end(&succ);
            }
            _ => {
                return self.unsupported_line(one);
            }
        }
        Ok(())
    }

    fn unsupported_line<T>(&self, one: &hir::Statement) -> R<T> {
        Err(Unsupported(format!("{} line {}: {} {}", self.name(), one.line, one.call, one.args.join(" "))))
    }

    fn ret(&mut self, node: &str, type_: &str) -> R<()> {
        if node != "n0" && is_float(type_) {
            let got = self.eval(node)?;
            let value = self.floating(got)?;
            let extra = Extra { reads_complete: true, ..Extra::default() };
            self.op(Kind::Return, vec![], vec![Arg::Held(value)], None, Some(vec![value.value]), extra);
            self.end(&[]);
            return Ok(());
        }
        if node != "n0" && self.width(type_)? == 8 {
            let got = self.eval(node)?;
            let mut value = self.operand(got, type_)?;
            if let Operand::Const(_) = value {
                value = Operand::Held(held(self.copy(&value), 8));
            }
            let Operand::Held(value) = self.narrowed(value, 8)? else { unreachable!("a held value") };
            let extra = Extra { reads_complete: true, ..Extra::default() };
            self.op(Kind::Return, vec![], vec![Arg::Held(value)], None, Some(vec![value.value]), extra);
            self.end(&[]);
            return Ok(());
        }
        let last = self.current.and_then(|current| self.blocks[current].ops.last()).map(|one| one.op.clone());
        let mut node = Some(node);
        let mut got = None;
        if let Some(last) = last.filter(|last| {
            node == Some("n0") && widths(type_).is_some() && last.kind == Kind::Call && last.results.len() == 2
        }) {
            // A value-less return from a function that has one: the value is
            // what the call before it left, as inline assembly means it to be.
            let value = |one: &Arg| match one {
                Arg::Held(one) => one.value,
                _ => panic!("AttributeError: {} has no attribute 'value'", one.repr()),
            };
            got = Some(Got::Returned(Returned { low: value(&last.results[0]), high: value(&last.results[1]) }));
            node = None;
        }
        let mut returned: Vec<Value> = Vec::new();
        if node != Some("n0") {
            let got = match node {
                Some(node) => self.eval(node)?,
                None => got.expect("the call's result"),
            };
            if let Got::Address(Address::Far(far)) = &got {
                let offset = self.near(&Address::near(far.offset, far.disp))?;
                returned = vec![
                    self.copy(&Operand::Held(held(offset, 2))),
                    self.copy(&Operand::Held(held(far.segment, 2))),
                ];
            } else if self.width(type_)? == 4 {
                let whole = self.operand(got, type_)?;
                let (low, high) = match &whole {
                    Operand::Const(whole) => (
                        Operand::Const(constant(&whole.n & BigInt::from(0xFFFF), 2)),
                        Operand::Const(constant((&whole.n >> 16u32) & BigInt::from(0xFFFF), 2)),
                    ),
                    Operand::Held(whole) => {
                        let shifted = self.fresh();
                        self.op(
                            Kind::Shr,
                            vec![Arg::Held(held(shifted, 4))],
                            vec![Arg::Held(*whole), Arg::Const(constant(16, 1))],
                            None,
                            None,
                            Extra::default(),
                        );
                        (Operand::Held(held(whole.value, 2)), Operand::Held(held(shifted, 2)))
                    }
                };
                returned = vec![self.copy(&low), self.copy(&high)];
            } else {
                let operand = self.operand(got, type_)?;
                let narrowed = self.narrowed(operand, 2)?;
                returned = vec![self.copy(&narrowed)];
            }
        }
        let args = returned.iter().map(|one| Arg::Held(held(*one, 2))).collect();
        let extra = Extra { reads_complete: true, ..Extra::default() };
        self.op(Kind::Return, vec![], args, None, Some(returned), extra);
        self.end(&[]);
        Ok(())
    }

    /// Go to `label` when `node` is `when`; fall through otherwise.
    fn branch(&mut self, node: &str, label: &str, when: bool) -> R<()> {
        let tree = &self.unit.nodes[&hir::handle(node)];
        let args: Vec<&str> = tree.args.iter().map(String::as_str).collect();
        match (tree.call.as_str(), &args[..]) {
            ("CGCompare", _) => {
                let (flags, test) = self.compare(tree)?;
                self.jump_if(flags, if when { test } else { inverse(test) }, label);
            }
            ("CGFlow", ["O_FLOW_NOT", inner, _]) => self.branch(inner, label, !when)?,
            ("CGFlow", [flow @ ("O_FLOW_AND" | "O_FLOW_OR"), left, right]) => {
                if (*flow == "O_FLOW_OR") == when {
                    self.branch(left, label, when)?;
                    self.branch(right, label, when)?;
                } else {
                    let skip = self.label();
                    self.branch(left, &skip, !when)?;
                    self.branch(right, label, when)?;
                    self.start(skip);
                }
            }
            _ => {
                let got = self.eval(node)?;
                let value = self.operand(got, "TY_INTEGER")?;
                let flags = self.fresh_value(true);
                let width = match &value {
                    Operand::Held(one) => one.width,
                    Operand::Const(_) => 2,
                };
                self.op(
                    Kind::Sub,
                    vec![],
                    vec![value.arg(), Arg::Const(constant(0, width))],
                    Some(vec![flags]),
                    None,
                    Extra::default(),
                );
                self.jump_if(flags, if when { Kind::Ne } else { Kind::Eq }, label);
            }
        }
        Ok(())
    }

    fn compare(&mut self, tree: &hir::Node) -> R<(Value, Kind)> {
        let [cg_op, left, right, type_] = &tree.args[..] else {
            panic!("ValueError: not enough values to unpack");
        };
        if is_float(type_) {
            let got = self.eval(left)?;
            let converted = self.convert(got, &self.type_of(left), type_)?;
            let x = self.floating(converted)?;
            let got = self.eval(right)?;
            let converted = self.convert(got, &self.type_of(right), type_)?;
            let y = self.floating(converted)?;
            let flags = self.fresh_value(true);
            self.op(Kind::Fcompare, vec![], vec![Arg::Held(x), Arg::Held(y)], Some(vec![flags]), None, Extra::default());
            return Ok((flags, tests(cg_op).0));
        }
        let width = 2.max(self.width(type_)?);
        let got = self.eval(left)?;
        let coerced = self.coerced(got, left, type_)?;
        let a = self.narrowed(coerced, width)?;
        let got = self.eval(right)?;
        let coerced = self.coerced(got, right, type_)?;
        let b = self.narrowed(coerced, width)?;
        let test = if signed(type_) { tests(cg_op).0 } else { tests(cg_op).1 };
        let flags = self.fresh_value(true);
        self.op(Kind::Sub, vec![], vec![a.arg(), b.arg()], Some(vec![flags]), None, Extra::default());
        Ok((flags, test))
    }

    fn jump_if(&mut self, flags: Value, test: Kind, label: &str) {
        let extra = Extra { test: Some(test), target: Some(label.to_owned()), ..Extra::default() };
        self.op(Kind::Branch, vec![], vec![], None, Some(vec![flags]), extra);
        let fall = self.label();
        self.end(&[fall.clone(), label.to_owned()]);
        self.start(fall);
    }

    // ---- expressions ----

    fn eval(&mut self, node: &str) -> R<Got> {
        let key = hir::handle(node);
        if let Some(done) = self.done.get(&key) {
            return Ok(done.clone());
        }
        let got = self.expression(&self.unit.nodes[&key], node)?;
        self.done.insert(key, got.clone());
        Ok(got)
    }

    fn expression(&mut self, tree: &hir::Node, node: &str) -> R<Got> {
        let args: Vec<&str> = tree.args.iter().map(String::as_str).collect();
        let call = tree.call.as_str();
        match (call, &args[..]) {
            ("CGInteger", [value, type_]) => {
                return Ok(Got::Const(constant(big(value), 2.max(self.width(type_)?))));
            }
            ("CGInt64", [value, type_]) => return Ok(Got::Const(constant(self.wrapped(&big(value), type_)?, 8))),
            ("CGFloat", [text, type_]) if is_float(type_) => {
                return Ok(Got::Real(self.real(float(text), self.width(type_)?)));
            }
            ("CGFEName", [symbol, type_]) => return self.named(symbol, Some(type_)),
            ("CGTempName", [temp, _]) => return Ok(Got::Address(Address::frame(self.frame[*temp]))),
            ("CGBackName", [back, _]) => {
                let symbol = self.unit.backs[&hir::handle(back)];
                if symbol != 0 {
                    return self.named(&format!("y{symbol}"), None);
                }
                return Ok(Got::Address(Address::Global(Global {
                    space: Space::Segment,
                    index: LITERAL + hir::handle(back),
                    disp: 0,
                    base: None,
                    declared: None,
                    volatile: false,
                })));
            }
            ("CGUnary", ["O_POINTS", inner, type_]) => {
                let got = self.eval(inner)?;
                return self.points(got, type_);
            }
            ("CGUnary", ["O_CONVERT", inner, type_]) => {
                let got = self.eval(inner)?;
                return self.convert(got, &self.type_of(inner), type_);
            }
            ("CGUnary", [cg_op @ ("O_UMINUS" | "O_COMPLEMENT" | "O_FABS"), inner, type_]) => {
                let got = self.eval(inner)?;
                return self.unary(cg_op, got, type_);
            }
            ("CGUnary", [cg_op, inner, type_]) if library_routine(cg_op).is_some() => {
                let got = self.eval(inner)?;
                return self.library(library_routine(cg_op).unwrap(), vec![((*inner).to_owned(), got)], type_);
            }
            ("CGBinary", ["O_COMMA", left, right, _]) => {
                self.eval(left)?;
                return self.eval(right);
            }
            ("CGBinary", [cg_op, left, right, type_]) if library_routine(cg_op).is_some() => {
                // The runtime takes them last first, as every call's parms are listed.
                let second = self.eval(right)?;
                let first = self.eval(left)?;
                let arguments = vec![((*right).to_owned(), second), ((*left).to_owned(), first)];
                return self.library(library_routine(cg_op).unwrap(), arguments, type_);
            }
            ("CGBinary", [cg_op, left, right, type_]) => return self.binary(cg_op, left, right, type_),
            ("CGAssign", [target, source, type_]) => return self.assign(target, source, type_),
            ("CGLVAssign", [target, source, _]) => {
                let target = self.eval(target)?;
                let source = self.eval(source)?;
                return self.aggregate(target, source);
            }
            ("CGPostGets" | "CGPreGets", [cg_op, target, source, type_]) if is_float(type_) => {
                let got = self.eval(target)?;
                let address = self.address(got)?;
                let width = self.width(type_)?;
                let old = self.floating(Got::FloatCell(FloatCell { address: address.clone(), width }))?;
                let got = self.eval(source)?;
                let converted = self.convert(got, &self.type_of(source), type_)?;
                let source = self.floating(converted)?;
                let new = self.float_arithmetic(cg_op, old, source)?;
                self.put_float(&address, width, Got::Held(new))?;
                return Ok(if call == "CGPostGets" {
                    Got::Held(old)
                } else {
                    Got::FloatCell(FloatCell { address, width })
                });
            }
            ("CGPostGets" | "CGPreGets", [cg_op @ ("O_PLUS" | "O_MINUS"), target, source, type_])
                if self.far_pointer(type_) =>
            {
                let got = self.eval(target)?;
                let address = self.address(got)?;
                let old = self.far_loaded(&address, type_)?;
                let got = self.eval(source)?;
                let delta = self.operand(got, &self.type_of(source))?;
                let new = self.offset(Address::Far(old.clone()), delta, *cg_op == "O_MINUS")?;
                let Address::Far(new) = new else { unreachable!("a far address stays far") };
                self.put_pointer(&address, &new, type_)?;
                return Ok(Got::Address(Address::Far(if call == "CGPostGets" { old } else { new })));
            }
            ("CGPostGets" | "CGPreGets", [cg_op, target, source, type_]) => {
                let got = self.eval(target)?;
                let address = self.address(got)?;
                let width = self.width(type_)?;
                let cell = self.cell(&address, width, Some(type_));
                let old = self.load(cell, type_);
                let got = self.eval(source)?;
                let coerced = self.coerced(got, source, type_)?;
                let new = self.arithmetic(cg_op, Operand::Held(old), coerced, type_)?;
                let cell = self.cell(&address, width, Some(type_));
                self.store(cell, new.clone())?;
                return Ok(if call == "CGPostGets" { Got::Held(old) } else { new.got() });
            }
            ("CGCall", [call]) => {
                let call = &self.unit.calls[&hir::handle(call)];
                return self.call(call);
            }
            ("CGChoose", [test, yes, no, type_]) => return self.choose(test, yes, no, type_),
            ("CGCompare" | "CGFlow", _) => return self.truth(node),
            ("CGEval", [inner]) => return self.eval(inner),
            ("CGVolatile", [inner]) => {
                let got = self.eval(inner)?;
                return match got {
                    Got::Address(address) => Ok(Got::Address(address.volatile())),
                    Got::Restricted(restricted) if matches!(restricted.value, Got::Address(_)) => {
                        let Got::Address(address) = &restricted.value else { unreachable!() };
                        let value = Got::Address(address.volatile());
                        Ok(Got::Restricted(Box::new(Restricted { value, root: restricted.root.clone() })))
                    }
                    other => self.unsupported(format!("volatile access through {}", other.repr())),
                };
            }
            ("CGAttr", [inner, "3"]) => {
                let got = self.eval(inner)?;
                let root = |kind: &str, n: i64| Identity::Tuple(vec![Identity::Str(kind.to_owned()), Identity::Int(n)]);
                let root = match &got {
                    Got::Address(Address::Frame(frame)) => root("frame", frame.disp),
                    Got::Address(Address::Global(global)) => root("global", global.index),
                    Got::Address(Address::Far(far)) if far.named != 0 => root("far", far.named),
                    _ => root("node", hir::handle(inner)),
                };
                return Ok(Got::Restricted(Box::new(Restricted { value: got, root })));
            }
            ("CGAttr", [inner, _]) => return self.eval(inner),
            _ => {}
        }
        self.unsupported(format!("{} {}", tree.call, tree.args.join(" ")))
    }

    fn choose(&mut self, test: &str, yes: &str, no: &str, type_: &str) -> R<Got> {
        let floats = is_float(type_);
        let arm = move |raise: &mut Self, node: &str| -> R<Got> {
            let got = raise.eval(node)?;
            if floats {
                raise.convert(got, &raise.type_of(node), type_)
            } else {
                Ok(raise.coerced(got, node, type_)?.got())
            }
        };
        self.joined(test, &|raise| arm(raise, yes), &|raise| arm(raise, no), type_)
    }

    /// A compare or flow as a value: 1 or 0.
    fn truth(&mut self, test: &str) -> R<Got> {
        self.joined(
            test,
            &|_| Ok(Got::Const(constant(1, 2))),
            &|_| Ok(Got::Const(constant(0, 2))),
            "TY_INTEGER",
        )
    }

    /// `test ? yes() : no()`: each arm stores into one frame cell, read after the join.
    #[allow(clippy::type_complexity)]
    fn joined(
        &mut self,
        test: &str,
        yes: &dyn Fn(&mut Self) -> R<Got>,
        no: &dyn Fn(&mut Self) -> R<Got>,
        type_: &str,
    ) -> R<Got> {
        let floats = is_float(type_);
        let width = if floats { self.width(type_)? } else { 2.max(self.width(type_)?) };
        let slot = self.slot(i64::from(width));
        let joined = Address::frame(slot);

        let put = |raise: &mut Self, value: Got| -> R<()> {
            if floats {
                raise.put_float(&joined, width, value)?;
            } else {
                let cell = raise.cell(&joined, width, None);
                let operand = raise.as_operand(value)?;
                let narrowed = raise.narrowed(operand, width)?;
                raise.store(cell, narrowed)?;
            }
            Ok(())
        };

        let otherwise = self.label();
        let join = self.label();
        self.branch(test, &otherwise, false)?;
        let value = yes(self)?;
        put(self, value)?;
        let extra = Extra { target: Some(join.clone()), ..Extra::default() };
        self.op(Kind::Jump, vec![], vec![], None, None, extra);
        self.end(&[join.clone()]);
        self.start(otherwise);
        let value = no(self)?;
        put(self, value)?;
        self.start(join);
        if floats {
            return Ok(Got::FloatCell(FloatCell { address: joined, width }));
        }
        if self.far_pointer(type_) {
            return Ok(Got::Address(Address::Far(self.far_loaded(&joined, type_)?)));
        }
        let cell = self.cell(&joined, width, None);
        Ok(Got::Held(self.load(cell, type_)))
    }

    /// `narrowed` takes an Operand; a `put` of anything else fails there as Python's attribute access does.
    fn as_operand(&self, value: Got) -> R<Operand> {
        match value {
            Got::Held(one) => Ok(Operand::Held(one)),
            Got::Const(one) => Ok(Operand::Const(one)),
            other => panic!("AttributeError: {} has no attribute 'width'", other.repr()),
        }
    }

    fn type_of(&self, node: &str) -> String {
        let tree = &self.unit.nodes[&hir::handle(node)];
        match tree.call.as_str() {
            "CGCall" => return self.unit.calls[&hir::handle(&tree.args[0])].type_.clone(),
            "CGEval" | "CGVolatile" | "CGAttr" => return self.type_of(&tree.args[0]),
            "CGFlow" | "CGCompare" => return "TY_BOOLEAN".to_owned(),
            _ => {}
        }
        tree.args.last().expect("IndexError: list index out of range").clone()
    }

    /// An operand at the operation's type: the code generator converts operands implicitly.
    fn coerced(&mut self, got: Got, node: &str, type_: &str) -> R<Operand> {
        let converted = self.convert(got, &self.type_of(node), type_)?;
        self.operand(converted, type_)
    }

    /// Python `name`.
    fn named(&mut self, token: &str, type_: Option<&str>) -> R<Got> {
        let symbol = self.unit.symbols[&hir::handle(token)].clone();
        if symbol.proc() {
            return Ok(Got::Function(Box::new(Function { symbol })));
        }
        let declared = self.aliasing(type_)?;
        if let Some(disp) = self.frame.get(token) {
            return Ok(Got::Address(Address::Frame(Frame { disp: *disp, declared, volatile: false })));
        }
        if !self.unit.grouped(&symbol) {
            let segment = self.fresh();
            let offset = self.fresh();
            self.op(
                Kind::Copy,
                vec![Arg::Held(held(segment, 2))],
                vec![Arg::Symbol(mir::Symbol::new(Space::Group, SELECTOR + symbol.id, 0, 2))],
                None,
                None,
                Extra::default(),
            );
            self.op(
                Kind::Copy,
                vec![Arg::Held(held(offset, 2))],
                vec![Arg::Symbol(mir::Symbol::new(_space(&symbol), symbol.id, 0, 2))],
                None,
                None,
                Extra::default(),
            );
            return Ok(Got::Address(Address::Far(Far {
                segment,
                offset,
                disp: 0,
                whole: None,
                named: SELECTOR + symbol.id,
                declared,
                volatile: false,
            })));
        }
        Ok(Got::Address(Address::Global(Global {
            space: _space(&symbol),
            index: symbol.id,
            disp: 0,
            base: None,
            declared,
            volatile: false,
        })))
    }

    /// A scalar type's aliasing class; None for a character, an aggregate or no type, which reach anything.
    fn aliasing(&self, type_: Option<&str>) -> R<Option<String>> {
        let type_ = type_.map(|one| self.unit.canonical_type(one));
        if type_.as_deref() == Some("TY_POINTER") {
            return Ok(Some(format!("pointer{}", self.width("TY_POINTER")?)));
        }
        Ok(type_.and_then(|one| classes(&one)).map(str::to_owned))
    }

    fn points(&mut self, got: Got, type_: &str) -> R<Got> {
        let (restricted, got) = match got {
            Got::Restricted(one) => (Some(one.root), one.value),
            got => (None, got),
        };
        if matches!(&got, Got::Held(one) if one.width == 10) {
            return Ok(got); // a float call's result, already a value
        }
        if let Some(size) = self.unit.types.get(type_) {
            if let Got::Aggregate(aggregate) = &got {
                if aggregate.size != *size {
                    return self.unsupported(format!("{}-byte aggregate used as {} bytes", aggregate.size, size));
                }
                return Ok(got);
            }
            let address = self.address(got)?;
            return Ok(Got::Aggregate(Box::new(Aggregate { address, size: *size })));
        }
        if let Got::Held(one) = &got {
            if one.width == self.width(type_)? && one.width == 8 {
                return Ok(got); // an int64 call's result is likewise already a whole MIR value
            }
        }
        if let Got::Returned(returned) = &got {
            if self.far_pointer(type_) {
                return Ok(Got::Address(Address::Far(Far {
                    segment: returned.high,
                    offset: returned.low,
                    disp: 0,
                    whole: None,
                    named: 0,
                    declared: None,
                    volatile: false,
                })));
            }
            if self.width(type_)? == 4 {
                let whole = self.fresh();
                self.op(
                    Kind::Concat,
                    vec![Arg::Held(held(whole, 4))],
                    vec![Arg::Held(held(returned.high, 2)), Arg::Held(held(returned.low, 2))],
                    None,
                    None,
                    Extra::default(),
                );
                self.pointer_values.insert(whole);
                return Ok(Got::Held(held(whole, 4)));
            }
            if self.width(type_)? == 8 {
                return self.unsupported("a DX:AX result cannot provide an 8-byte value");
            }
            return Ok(Got::Held(self.extended(held(returned.low, 2), type_)?));
        }
        let address = self.address(got)?;
        if is_float(type_) {
            return Ok(Got::FloatCell(FloatCell { address, width: self.width(type_)? }));
        }
        if self.far_pointer(type_) {
            return Ok(Got::Address(Address::Far(self.far_loaded(&address, type_)?)));
        }
        let width = self.width(type_)?;
        let cell = self.cell(&address, width, Some(type_));
        let loaded = self.load(cell, type_);
        if let Some(restricted) = restricted {
            let fact = self
                .pointer_seeds
                .get(&loaded.value)
                .cloned()
                .unwrap_or_else(|| Provenance::one(MemoryObject::new(MemoryKind::Unknown)));
            self.pointer_seeds
                .insert(loaded.value, Provenance { slices: fact.slices, restrict: BTreeSet::from([restricted]) });
            self.pointer_values.insert(loaded.value);
        }
        Ok(Got::Held(loaded))
    }

    fn convert(&mut self, got: Got, source: &str, type_: &str) -> R<Got> {
        // An address's node is typed by what it addresses: `(void far *) &a_float`.
        if let Got::Function(function) = &got {
            // Once C decays a function to a pointer, materialize the relocatable code address.
            let offset = self.fresh();
            let symbol = &function.symbol;
            self.op(
                Kind::Copy,
                vec![Arg::Held(held(offset, 2))],
                vec![Arg::Symbol(mir::Symbol::new(_space(symbol), symbol.id, 0, 2))],
                None,
                None,
                Extra::default(),
            );
            if symbol.far() {
                let segment = self.fresh();
                self.op(
                    Kind::Copy,
                    vec![Arg::Held(held(segment, 2))],
                    vec![Arg::Symbol(mir::Symbol::new(Space::Group, SELECTOR + symbol.id, 0, 2))],
                    None,
                    None,
                    Extra::default(),
                );
                return Ok(Got::Address(Address::Far(Far {
                    segment,
                    offset,
                    disp: 0,
                    whole: None,
                    named: SELECTOR + symbol.id,
                    declared: None,
                    volatile: false,
                })));
            }
            return Ok(Got::Held(held(offset, 2)));
        }
        if let Got::Address(address @ (Address::Frame(_) | Address::Global(_) | Address::Near(_))) = &got {
            if !self.far_pointer(type_) {
                return Ok(got);
            }
            let segment = self.dgroup();
            let offset = self.near(address)?;
            return Ok(Got::Address(Address::Far(Far {
                segment,
                offset,
                disp: 0,
                whole: None,
                named: 0,
                declared: None,
                volatile: false,
            })));
        }
        if let Got::Address(_) = got {
            return Ok(got);
        }
        if is_float(source) || is_float(type_) {
            if is_float(source) && is_float(type_) {
                return Ok(match got {
                    Got::Real(real) => Got::Real(self.real(real.value, self.width(type_)?)),
                    got => got,
                });
            }
            if is_float(source) {
                let value = self.floating(got)?;
                return Ok(self.truncated(value, type_)?.got());
            }
            if let Got::Const(one) = &got {
                let wrapped = self.wrapped(&one.n, source)?;
                return Ok(Got::Real(self.real(to_float(&wrapped), self.width(type_)?)));
            }
            let whole = self.operand(got, source)?;
            if source == "TY_UINT_8" {
                let result = self.fresh();
                let extra = Extra { floating: Some(_loaded(Format::Unsigned64)), ..Extra::default() };
                self.op(Kind::Fload, vec![Arg::Held(held(result, 10))], vec![whole.arg()], None, None, extra);
                return Ok(Got::Held(held(result, 10)));
            }
            if !signed(source) && self.width(source)? == 4 {
                // No 32-bit integer load reads it unsigned; its zero extension as a quad does.
                let quad = Address::frame(self.slot(8));
                let cell = self.cell(&quad, 4, None);
                self.store(cell, whole)?;
                let cell = self.cell(&quad.at(quad.disp() + 4), 4, None);
                self.store(cell, Operand::Const(constant(0, 4)))?;
                let reference = self.cell(&quad, 8, None);
                let result = self.fresh();
                let extra = Extra {
                    loads: vec![reference.clone()],
                    floating: Some(_loaded(Format::Signed64)),
                    ..Extra::default()
                };
                self.op(
                    Kind::Fload,
                    vec![Arg::Held(held(result, 10))],
                    vec![Arg::Cell(Cell { r#ref: reference })],
                    None,
                    None,
                    extra,
                );
                return Ok(Got::Held(held(result, 10)));
            }
            let mut whole = whole;
            if self.width(source)? == 1 || !signed(source) {
                let converted = self.convert(whole.got(), source, "TY_INT_4")?;
                whole = self.as_operand(converted)?;
            }
            let result = self.fresh();
            let extra = Extra { floating: Some(_loaded(integer_formats(whole.width()))), ..Extra::default() };
            self.op(Kind::Fload, vec![Arg::Held(held(result, 10))], vec![whole.arg()], None, None, extra);
            return Ok(Got::Held(held(result, 10)));
        }
        let mut got = got;
        if let Got::Returned(_) = got {
            got = self.points(got, source)?;
            if !matches!(got, Got::Held(_) | Got::Const(_)) {
                return Ok(got);
            }
        }
        let to = self.width(type_)?;
        if let Some(one) = match &got {
            Got::Held(one) if one.width == 2 && self.far_pointer(type_) => Some(*one),
            _ => None,
        } {
            let segment = self.dgroup();
            return Ok(Got::Address(Address::Far(Far {
                segment,
                offset: one.value,
                disp: 0,
                whole: None,
                named: 0,
                declared: None,
                volatile: false,
            })));
        }
        let got = match got {
            Got::Const(one) => return Ok(Got::Const(constant(self.wrapped(&one.n, type_)?, 2.max(to)))),
            Got::Held(one) => one,
            other => panic!("AttributeError: {} has no attribute 'width'", other.repr()),
        };
        if to > got.width {
            let wide = self.fresh();
            let kind = if signed(source) { Kind::SignExtend } else { Kind::ZeroExtend };
            self.op(kind, vec![Arg::Held(held(wide, to))], vec![Arg::Held(got)], None, None, Extra::default());
            return Ok(Got::Held(held(wide, to)));
        }
        if got.width == 8 && to < 8 {
            let narrow = self.fresh();
            let width = 2.max(to);
            self.op(
                Kind::Extract,
                vec![Arg::Held(held(narrow, width))],
                vec![Arg::Held(got), Arg::Const(constant(0, 1))],
                None,
                None,
                Extra::default(),
            );
            return Ok(Got::Held(self.extended(held(narrow, width), type_)?));
        }
        if to == 1 {
            return Ok(Got::Held(self.extended(held(got.value, 2), type_)?));
        }
        if to == 2 && got.width == 4 {
            return Ok(Got::Held(held(got.value, 2)));
        }
        Ok(Got::Held(got))
    }

    fn unary(&mut self, cg_op: &str, got: Got, type_: &str) -> R<Got> {
        if is_float(type_) {
            let Some(kind) = float_unary_kind(cg_op) else {
                return self.unsupported(format!("float {cg_op}"));
            };
            let result = self.fresh();
            let value = self.floating(got)?;
            let extra = Extra { floating: Some(exact_unary()), ..Extra::default() };
            self.op(kind, vec![Arg::Held(held(result, 10))], vec![Arg::Held(value)], None, None, extra);
            return Ok(Got::Held(held(result, 10)));
        }
        if cg_op == "O_FABS" {
            return self.unsupported(format!("O_FABS of {type_}"));
        }
        let value = self.operand(got, type_)?;
        let width = 2.max(self.width(type_)?);
        if let Operand::Const(value) = &value {
            let n = if cg_op == "O_UMINUS" { -&value.n } else { !&value.n };
            return Ok(Got::Const(constant(self.wrapped(&n, type_)?, width)));
        }
        let result = self.fresh();
        let kind = if cg_op == "O_UMINUS" { Kind::Neg } else { Kind::Not };
        let narrowed = self.narrowed(value, width)?;
        self.op(kind, vec![Arg::Held(held(result, width))], vec![narrowed.arg()], None, None, Extra::default());
        Ok(Got::Held(self.extended(held(result, width), type_)?))
    }

    fn binary(&mut self, cg_op: &str, left: &str, right: &str, type_: &str) -> R<Got> {
        let mut a = self.eval(left)?;
        let mut b = self.eval(right)?;
        if is_float(type_) {
            let converted = self.convert(a, &self.type_of(left), type_)?;
            let x = self.floating(converted)?;
            let converted = self.convert(b, &self.type_of(right), type_)?;
            let y = self.floating(converted)?;
            return Ok(Got::Held(self.float_arithmetic(cg_op, x, y)?));
        }
        if matches!(cg_op, "O_PLUS" | "O_MINUS")
            && matches!(type_, "TY_POINTER" | "TY_NEAR_POINTER")
            && !self.far_pointer(type_)
        {
            // A loaded near pointer is an address too, so its constant steps fold into cells.
            a = self.loaded(a, left);
            b = self.loaded(b, right);
        }
        if matches!(cg_op, "O_PLUS" | "O_MINUS") && matches!(b, Got::Address(_)) && cg_op == "O_PLUS" {
            (a, b) = (b, a);
        }
        if let Got::Held(one) = &a {
            if self.far_pointer(&self.type_of(left)) {
                a = Got::Address(Address::Far(self.split(*one)));
            }
        }
        if let Got::Address(address) = &a {
            if matches!(cg_op, "O_PLUS" | "O_MINUS") {
                let by = self.operand(b, &self.type_of(right))?;
                return Ok(Got::Address(self.offset(address.clone(), by, cg_op == "O_MINUS")?));
            }
        }
        if matches!(cg_op, "O_LSHIFT" | "O_RSHIFT") {
            let a = self.coerced(a, left, type_)?;
            let b = self.operand(b, &self.type_of(right))?;
            return Ok(self.arithmetic(cg_op, a, b, type_)?.got());
        }
        let a = self.coerced(a, left, type_)?;
        let b = self.coerced(b, right, type_)?;
        Ok(self.arithmetic(cg_op, a, b, type_)?.got())
    }

    fn offset(&mut self, address: Address, by: Operand, subtract: bool) -> R<Address> {
        // Arithmetic leaves the declared object: what it reaches is an access.
        let mut address = address;
        match &mut address {
            Address::Frame(one) => one.declared = None,
            Address::Global(one) => one.declared = None,
            Address::Far(one) => one.declared = None,
            Address::Near(_) => {}
        }
        if let Operand::Const(by) = &by {
            let n = if subtract { -&by.n } else { by.n.clone() };
            let disp = BigInt::from(address.disp()) + n;
            return Ok(address.at(disp.to_i64().expect("an address displacement fits")));
        }
        let mut index = self.narrowed(by, 2)?;
        if subtract {
            let negated = self.fresh();
            self.op(Kind::Neg, vec![Arg::Held(held(negated, 2))], vec![index.arg()], None, None, Extra::default());
            index = Operand::Held(held(negated, 2));
        }
        match address {
            Address::Far(far) => {
                let moved = self.add(held(far.offset, 2), &index);
                self.pointer_values.insert(moved);
                Ok(Address::Far(Far {
                    segment: far.segment,
                    offset: moved,
                    disp: far.disp,
                    whole: None,
                    named: far.named,
                    declared: None,
                    volatile: false,
                }))
            }
            Address::Global(global) => {
                // The symbol stays named, so its cells alias only the symbol's own.
                let moved = match global.base {
                    None => match &index {
                        Operand::Held(one) => one.value,
                        Operand::Const(one) => panic!("AttributeError: {} has no attribute 'value'", one.repr()),
                    },
                    Some(base) => self.add(held(base, 2), &index),
                };
                self.pointer_values.insert(moved);
                Ok(Address::Global(Global { base: Some(moved), ..global }))
            }
            Address::Near(near) => {
                let moved = self.add(held(near.base, 2), &index);
                self.pointer_values.insert(moved);
                Ok(Address::Near(Near { base: moved, disp: near.disp, space: near.space, volatile: false }))
            }
            Address::Frame(frame) => {
                // From the object's first byte, so the address says which object it is in.
                let start = self.extent(frame.disp).map_or(0, |extent| extent.0);
                let base = self.near(&Address::Frame(Frame { disp: start, ..frame.clone() }))?;
                let moved = self.add(held(base, 2), &index);
                self.pointer_values.insert(moved);
                Ok(Address::Near(Near { base: moved, disp: frame.disp - start, space: Space::Frame, volatile: false }))
            }
        }
    }

    fn float_arithmetic(&mut self, cg_op: &str, x: Held, y: Held) -> R<Held> {
        let Some(kind) = float_arithmetic_kind(cg_op) else {
            return self.unsupported(format!("float {cg_op}"));
        };
        let result = self.fresh();
        let extra = Extra { floating: Some(arith_rule()), ..Extra::default() };
        self.op(kind, vec![Arg::Held(held(result, 10))], vec![Arg::Held(x), Arg::Held(y)], None, None, extra);
        Ok(held(result, 10))
    }

    fn add(&mut self, a: Held, b: &Operand) -> Value {
        let result = self.fresh();
        self.op(
            Kind::Add,
            vec![Arg::Held(held(result, a.width))],
            vec![Arg::Held(a), b.arg()],
            None,
            None,
            Extra::default(),
        );
        result
    }

    fn arithmetic(&mut self, cg_op: &str, a: Operand, b: Operand, type_: &str) -> R<Operand> {
        if is_float(type_) {
            return self.unsupported(format!("float {cg_op}"));
        }
        let width = 2.max(self.width(type_)?);
        let shift = matches!(cg_op, "O_LSHIFT" | "O_RSHIFT");
        let mut a = self.narrowed(a, width)?;
        let mut b = if shift { b } else { self.narrowed(b, width)? };
        let is_signed = signed(type_);
        if let (Operand::Const(x), Operand::Const(y)) = (&a, &b) {
            let folded = _fold(cg_op, &x.n, &y.n, is_signed)?;
            return Ok(Operand::Const(constant(self.wrapped(&folded, type_)?, width)));
        }
        let result = self.fresh();
        if matches!(cg_op, "O_DIV" | "O_MOD") {
            if let Operand::Const(_) = a {
                a = Operand::Held(held(self.copy(&a), width));
            }
            let remainder = self.fresh();
            let kind = if is_signed { Kind::Divmod } else { Kind::Udivmod };
            self.op(
                kind,
                vec![Arg::Held(held(result, width)), Arg::Held(held(remainder, width))],
                vec![a.arg(), b.arg()],
                None,
                None,
                Extra::default(),
            );
            return Ok(Operand::Held(held(if cg_op == "O_DIV" { result } else { remainder }, width)));
        }
        if shift {
            if let Operand::Const(_) = a {
                a = Operand::Held(held(self.copy(&a), width));
            }
            let count = match &b {
                Operand::Const(one) => Operand::Const(constant(one.n.clone(), 1)),
                Operand::Held(one) => Operand::Held(held(one.value, 1)),
            };
            let kind = if cg_op == "O_LSHIFT" {
                Kind::Shl
            } else if is_signed {
                Kind::Sar
            } else {
                Kind::Shr
            };
            self.op(kind, vec![Arg::Held(held(result, width))], vec![a.arg(), count.arg()], None, None, Extra::default());
            return Ok(Operand::Held(self.extended(held(result, width), type_)?));
        }
        let Some((kind, commutes)) = arithmetic_kind(cg_op) else {
            return self.unsupported(cg_op);
        };
        if matches!(a, Operand::Const(_)) && commutes {
            (a, b) = (b, a);
        }
        if let Operand::Const(_) = a {
            a = Operand::Held(held(self.copy(&a), width));
        }
        self.op(kind, vec![Arg::Held(held(result, width))], vec![a.arg(), b.arg()], None, None, Extra::default());
        Ok(Operand::Held(self.extended(held(result, width), type_)?))
    }

    fn assign(&mut self, target: &str, source: &str, type_: &str) -> R<Got> {
        let value = self.eval(source)?;
        let got = self.eval(target)?;
        let address = self.address(got)?;
        let width = self.width(type_)?;
        if let Got::Address(Address::Far(far)) = &value {
            self.put_pointer(&address, far, type_)?;
            return Ok(value);
        }
        if is_float(type_) {
            let converted = self.convert(value, &self.type_of(source), type_)?;
            return self.put_float(&address, width, converted);
        }
        let operand = self.coerced(value, source, type_)?;
        let cell = self.cell(&address, width, Some(type_));
        self.store(cell, operand.clone())?;
        Ok(operand.got())
    }

    /// Store a far pointer without turning its selector and offset into integer arithmetic.
    fn put_pointer(&mut self, address: &Address, value: &Far, type_: &str) -> R<()> {
        if let Some(whole) = value.whole.filter(|_| value.disp == 0) {
            let cell = self.cell(address, 4, Some(type_));
            return self.store(cell, Operand::Held(held(whole, 4)));
        }
        let cell = self.cell(address, 2, Some(type_));
        let offset = self.near(&Address::near(value.offset, value.disp))?;
        self.store(cell, Operand::Held(held(offset, 2)))?;
        let cell = self.cell(&address.at(address.disp() + 2), 2, Some(type_));
        self.store(cell, Operand::Held(held(value.segment, 2)))
    }

    /// A float into a cell of `width` bytes, and the value the assignment is.
    fn put_float(&mut self, address: &Address, width: u32, got: Got) -> R<Got> {
        match &got {
            Got::Real(real) => {
                let packed = _packed(real.value, width);
                for at in (0..width as usize).step_by(4) {
                    let bits = signed_word(&packed[at..at + 4]);
                    let cell = self.cell(&address.at(address.disp() + at as i64), 4, None);
                    self.store(cell, Operand::Const(constant(bits, 4)))?;
                }
                return Ok(Got::Real(Real { value: real.value, width }));
            }
            Got::FloatCell(cell) if cell.width == width => {
                let source = cell.address.clone();
                for at in (0..i64::from(width)).step_by(4) {
                    let cell = self.cell(&source.at(source.disp() + at), 4, None);
                    let moved = self.load(cell, "TY_UINT_4");
                    let cell = self.cell(&address.at(address.disp() + at), 4, None);
                    self.store(cell, Operand::Held(moved))?;
                }
                return Ok(Got::FloatCell(FloatCell { address: address.clone(), width }));
            }
            _ => {}
        }
        // fstp pops what it stores, so the assignment's own value is the cell.
        let reference = self.cell(address, width, None);
        let value = self.floating(got)?;
        let extra = Extra { stores: vec![reference.clone()], floating: Some(_stored(formats(width))), ..Extra::default() };
        self.op(Kind::Fstore, vec![Arg::Cell(Cell { r#ref: reference })], vec![Arg::Held(value)], None, None, extra);
        Ok(Got::FloatCell(FloatCell { address: address.clone(), width }))
    }

    fn aggregate(&mut self, target: Got, source: Got) -> R<Got> {
        let Got::Aggregate(source) = source else {
            return self.unsupported(format!("aggregate assignment from {}", source.repr()));
        };
        let into = self.address(target)?;
        let mut done = 0;
        while done < source.size {
            let width = if source.size - done >= 4 {
                4
            } else if source.size - done >= 2 {
                2
            } else {
                1
            };
            let moved = self.fresh();
            let got = self.cell(&source.address.at(source.address.disp() + done), width, None);
            let extra = Extra { loads: vec![got.clone()], ..Extra::default() };
            self.op(Kind::Load, vec![Arg::Held(held(moved, width))], vec![Arg::Cell(Cell { r#ref: got })], None, None, extra);
            let put = self.cell(&into.at(into.disp() + done), width, None);
            let extra = Extra { stores: vec![put.clone()], ..Extra::default() };
            self.op(Kind::Store, vec![Arg::Cell(Cell { r#ref: put })], vec![Arg::Held(held(moved, width))], None, None, extra);
            done += i64::from(width);
        }
        Ok(Got::Aggregate(Box::new(Aggregate { address: into, size: source.size })))
    }

    fn call(&mut self, call: &hir::Call) -> R<Got> {
        let target = self.eval(&call.target)?;
        if let Some(function) = match &target {
            Got::Function(function) if EMITTED.contains(&function.symbol.name.as_str()) => Some(function),
            _ => None,
        } {
            let mut values = Vec::new();
            for (node, _) in call.parms.iter().rev() {
                values.push(self.eval(node)?);
            }
            let byte = |one: &Got| match one {
                Got::Const(one) if !one.n.is_negative() && one.n < BigInt::from(256) => one.n.to_u8(),
                _ => None,
            };
            let Some(bytes) = values.iter().map(byte).collect::<Option<Vec<u8>>>() else {
                return self.unsupported(format!("{} of anything but constant bytes", function.symbol.name));
            };
            let symbol = hir::Symbol { code: Some(hir::Code { data: bytes, fixups: Vec::new() }), ..function.symbol.clone() };
            return self.inline_code(&symbol);
        }
        let (callee, indirect) = match target {
            Got::Function(function) => (function.symbol, None),
            target => {
                let callee = self.unit.symbols[&call.symbol].clone();
                let wanted = if callee.far() { 4 } else { 2 };
                match target {
                    Got::Held(one) if one.width == wanted => (callee, Some(one)),
                    _ => {
                        return self.unsupported(format!(
                            "indirect {} call through anything but a {wanted}-byte code pointer",
                            if callee.far() { "far" } else { "near" }
                        ));
                    }
                }
            }
        };
        let mut arguments = Vec::new();
        for (node, type_) in &call.parms {
            arguments.push((self.eval(node)?, type_.clone()));
        }
        self.invoke(&callee, arguments, &call.type_, indirect)
    }

    /// A C runtime routine taking and returning doubles, for an operator.
    fn library(&mut self, name: &str, arguments: Vec<(String, Got)>, type_: &str) -> R<Got> {
        let callee = match self.shared.runtime.get(name) {
            Some(callee) => callee.clone(),
            None => {
                let callee = hir::Symbol {
                    id: -1 - self.shared.runtime.len() as i64,
                    name: name.to_owned(),
                    base: name.to_owned(),
                    pattern: "_*".to_owned(),
                    attr: hir::FE_PROC | hir::FE_IMPORT,
                    call_class: hir::CALLER_POPS,
                    call_target: hir::FAR_CALL,
                    register_parms: false,
                    code: None,
                    segment: 0,
                };
                self.shared.runtime.insert(name.to_owned(), callee.clone());
                callee
            }
        };
        let mut doubles = Vec::new();
        for (node, value) in arguments {
            doubles.push((self.convert(value, &self.type_of(&node), "TY_DOUBLE")?, "TY_DOUBLE".to_owned()));
        }
        let got = self.invoke(&callee, doubles, "TY_DOUBLE", None)?;
        self.convert(got, "TY_DOUBLE", type_)
    }

    /// A call, with `arguments` last first; a float result arrives on the x87.
    fn invoke(&mut self, callee: &hir::Symbol, arguments: Vec<(Got, String)>, type_: &str, indirect: Option<Held>) -> R<Got> {
        if callee.code.is_some() {
            if !arguments.is_empty() || is_float(type_) {
                return self.unsupported("inline code taking arguments or giving a float");
            }
            return self.inline_code(callee);
        }
        // Stack arguments only: cdecl (caller pops) or pascal (reversed, callee pops).
        let convention = callee.call_class & (hir::CALLER_POPS | hir::REVERSE_PARMS);
        let stacked = convention == hir::CALLER_POPS || convention == hir::REVERSE_PARMS;
        if callee.register_parms || !stacked {
            return self.unsupported(format!("{} has a register calling convention", callee.object_name()));
        }
        let source_arguments: Vec<(Got, String)> = arguments.iter().rev().cloned().collect();
        let mut arguments = arguments;
        if callee.call_class & hir::REVERSE_PARMS != 0 {
            arguments.reverse();
        }
        let mut pushed = 0;
        for (value, type_) in arguments {
            pushed += self.push(value, &type_)?;
        }
        let call_args: Vec<Arg> = indirect.iter().map(|one| Arg::Held(*one)).collect();
        let call_uses: Vec<Value> = indirect.iter().map(|one| one.value).collect();
        let call_extra = || Extra {
            loads: callee_refs(),
            stores: callee_refs(),
            memory_complete: true,
            reads_complete: true,
            symbol: indirect.map(|_| false),
            indirect: indirect.is_some(),
            ..Extra::default()
        };
        let (site, returned) = if is_float(type_) || self.width(type_)? == 8 {
            let width = if is_float(type_) { 10 } else { 8 };
            let result = self.fresh();
            let site = self.op(
                Kind::Call,
                vec![Arg::Held(held(result, width))],
                call_args,
                Some(vec![result]),
                Some(call_uses),
                call_extra(),
            );
            (site, Got::Held(held(result, width)))
        } else {
            let low = self.fresh();
            let high = self.fresh();
            let site = self.op(
                Kind::Call,
                vec![Arg::Held(held(low, 2)), Arg::Held(held(high, 2))],
                call_args,
                Some(vec![low, high]),
                Some(call_uses),
                call_extra(),
            );
            (site, Got::Returned(Returned { low, high }))
        };
        let caller_pops = callee.call_class & hir::CALLER_POPS != 0;
        self.calls.insert(site, callee.object_name());
        if indirect.is_none() {
            self.callees.insert(site, callee.clone());
        }
        let mut actuals = Vec::new();
        let mut constants = Vec::new();
        for (value, arg_type) in &source_arguments {
            actuals.push(self._call_actual(value, arg_type)?);
        }
        for (value, arg_type) in &source_arguments {
            constants.push(self._call_constant(value, arg_type)?);
        }
        self.arguments.insert(site, actuals);
        self.constants.insert(site, constants);
        let canonical = self.unit.canonical_type(type_);
        if let Some(returned) = match &returned {
            Got::Returned(returned) if pointers(&canonical) => Some(returned),
            _ => None,
        } {
            self.pointer_values.insert(returned.low);
            let name = callee.object_name();
            if fresh_allocators(&name) {
                let extent = Self::_allocation_extent(&name, &source_arguments);
                let object = MemoryObject {
                    kind: MemoryKind::Allocation,
                    identity: Some(Identity::Tuple(vec![Identity::Str(name.clone()), Identity::Int(site)])),
                    generation: site,
                    extent,
                    addressed: true,
                    captured: true,
                };
                self.pointer_seeds.insert(returned.low, one_slice(object, 0, 1));
            } else {
                self.pointer_seeds.insert(returned.low, Provenance::one(MemoryObject::new(MemoryKind::Unknown)));
            }
        }
        self.contracts.insert(
            site,
            runtime::Contract {
                name: callee.object_name(),
                cleanup: Some(if caller_pops { 0 } else { pushed }),
                control: runtime::Control::Returns,
                enters_user_code: false,
                raises_error: false,
                error_handling: false,
                writes: runtime::Memory::Any,
                reads: runtime::Memory::Any,
                clobbers: BTreeSet::from([
                    runtime::Reg::Ax,
                    runtime::Reg::Bx,
                    runtime::Reg::Cx,
                    runtime::Reg::Dx,
                    runtime::Reg::Es,
                    runtime::Reg::Flags,
                ]),
                established: true,
                evidence: "Borland medium model: stack arguments, result in AX or DX:AX; \
                           SI, DI, BP and DS kept as 16-bit registers"
                    .to_owned(),
                documented: None,
                inputs: Some(BTreeSet::new()),
                direct_inputs: None,
                clobbers_reached: false,
                caller_cleanup: if caller_pops { pushed } else { 0 },
                i386: true,
                direct_writes: None,
                direct_reads: None,
            },
        );
        Ok(returned)
    }

    /// A scalar actual known without inspecting the callee's ABI.
    fn _call_constant(&self, value: &Got, type_: &str) -> R<Option<Const>> {
        let type_ = self.unit.canonical_type(type_);
        let Got::Const(value) = value else {
            return Ok(None);
        };
        if pointers(&type_) || is_float(&type_) {
            return Ok(None);
        }
        match self.narrowed(Operand::Const(value.clone()), self.width(&type_)?)? {
            Operand::Const(one) => Ok(Some(one)),
            Operand::Held(_) => unreachable!("a constant narrows to a constant"),
        }
    }

    /// A pointer actual without emitting another address computation.
    fn _call_actual(&self, value: &Got, type_: &str) -> R<alias::Actual> {
        let type_ = self.unit.canonical_type(type_);
        if !pointers(&type_) {
            return Ok(alias::Actual::Absent);
        }
        Ok(match value {
            Got::Address(address @ (Address::Frame(_) | Address::Global(_))) => {
                match self.placed(address, 1).provenance {
                    Some(provenance) => alias::Actual::Provenance(provenance),
                    None => alias::Actual::Absent,
                }
            }
            Got::Address(Address::Near(near)) => alias::Actual::Pointer(near.base, near.disp),
            Got::Address(Address::Far(far)) if far.whole.is_some() => {
                alias::Actual::Pointer(far.whole.unwrap(), far.disp)
            }
            Got::Address(Address::Far(far)) if far.named != 0 => alias::Actual::Provenance(one_slice(
                MemoryObject { identity: Some(Identity::Int(far.named)), ..MemoryObject::new(MemoryKind::Named) },
                far.disp,
                far.disp + 1,
            )),
            Got::Held(one) => alias::Actual::Pointer(one.value, 0),
            _ => alias::Actual::Provenance(Provenance::one(MemoryObject::new(MemoryKind::Unknown))),
        })
    }

    fn _allocation_extent(name: &str, arguments: &[(Got, String)]) -> Option<i64> {
        let numbers: Vec<&BigInt> = arguments
            .iter()
            .filter_map(|(value, _)| match value {
                Got::Const(one) => Some(&one.n),
                _ => None,
            })
            .collect();
        if matches!(name, "malloc" | "_malloc") && !numbers.is_empty() {
            return if numbers[0].is_positive() { numbers[0].to_i64() } else { None };
        }
        if matches!(name, "calloc" | "_calloc") && numbers.len() >= 2 {
            let extent = numbers[0] * numbers[1];
            return if extent.is_positive() { extent.to_i64() } else { None };
        }
        None
    }

    /// Inline assembly, as a call whose callee is laid down at the site.
    fn inline_code(&mut self, callee: &hir::Symbol) -> R<Got> {
        let code = callee.code.as_ref().expect("inline code");
        let mut data = code.data.clone();
        let mut parts = Vec::new();
        let mut start = 0usize;
        let mut named = Vec::new();
        for fixup in &code.fixups {
            let target = &self.unit.symbols[&fixup.symbol];
            let key = format!("y{}", fixup.symbol);
            let at = fixup.at as usize;
            if fixup.kind == "offset" && self.frame.contains_key(&key) {
                let word = ((self.frame[&key] + fixup.offset) & 0xFFFF) as u16;
                data[at..at + 2].copy_from_slice(&word.to_le_bytes());
                named.push(self.frame[&key]);
            } else if matches!(fixup.kind.as_str(), "offset" | "segment") && !target.proc() {
                parts.push(InlinePart::Bytes(data[start.min(at)..at].to_vec()));
                parts.push(InlinePart::Fixup(fixup.kind.clone(), target.object_name(), fixup.offset));
                start = at + 2;
            } else {
                return self.unsupported(format!("inline code's {} of {}", fixup.kind, target.name));
            }
        }
        parts.push(InlinePart::Bytes(data[start.min(data.len())..].to_vec()));
        let low = self.fresh();
        let high = self.fresh();
        let mut unique = Vec::new();
        for disp in named {
            if !unique.contains(&disp) {
                unique.push(disp);
            }
        }
        let addresses = unique
            .into_iter()
            .map(|disp| Arg::FrameAddress(FrameAddress { offset: disp, width: 2, extent: self.extent(disp) }))
            .collect();
        let extra = Extra { loads: callee_refs(), stores: callee_refs(), ..Extra::default() };
        let site = self.op(
            Kind::Call,
            vec![Arg::Held(held(low, 2)), Arg::Held(held(high, 2))],
            addresses,
            Some(vec![low, high]),
            Some(vec![]),
            extra,
        );
        self.inline.insert(site, parts);
        self.calls.insert(site, callee.object_name());
        self.callees.insert(site, callee.clone());
        self.contracts.insert(
            site,
            runtime::Contract {
                name: callee.object_name(),
                cleanup: Some(0),
                control: runtime::Control::Returns,
                enters_user_code: false,
                raises_error: false,
                error_handling: false,
                writes: runtime::Memory::Any,
                reads: runtime::Memory::Any,
                clobbers: runtime::EVERY.clone(),
                established: true,
                evidence: "inline assembly: every register assumed clobbered, the result left in AX or DX:AX"
                    .to_owned(),
                documented: None,
                inputs: Some(BTreeSet::new()),
                direct_inputs: None,
                clobbers_reached: false,
                caller_cleanup: 0,
                i386: false,
                direct_writes: None,
                direct_reads: None,
            },
        );
        Ok(Got::Returned(Returned { low, high }))
    }

    fn push(&mut self, value: Got, type_: &str) -> R<i64> {
        if is_float(type_) {
            let width = self.width(type_)?;
            let mut value = self.convert(value, type_, type_)?;
            let fits = match &value {
                Got::Real(one) => one.width == width,
                Got::FloatCell(one) => one.width == width,
                _ => false,
            };
            if !fits {
                let temporary = Address::frame(self.slot(i64::from(width)));
                self.put_float(&temporary, width, value)?;
                value = Got::FloatCell(FloatCell { address: temporary, width });
            }
            // The high doubleword first, so the low one is at the lower address.
            for at in (0..width as usize).step_by(4).rev() {
                match &value {
                    Got::Real(real) => {
                        let bits = signed_word(&_packed(real.value, width)[at..at + 4]);
                        self.op(Kind::Arg, vec![], vec![Arg::Const(constant(bits, 4))], None, None, Extra::default());
                    }
                    Got::FloatCell(cell) => {
                        let reference = self.cell(&cell.address.at(cell.address.disp() + at as i64), 4, None);
                        let loaded = self.load(reference, "TY_UINT_4");
                        self.op(Kind::Arg, vec![], vec![Arg::Held(loaded)], None, None, Extra::default());
                    }
                    _ => unreachable!("a float argument is a literal or a cell"),
                }
            }
            return Ok(i64::from(width));
        }
        if let Got::Aggregate(aggregate) = &value {
            let size = self.size(type_)?;
            if aggregate.size != size {
                return self.unsupported(format!("{}-byte aggregate used as {size} bytes", aggregate.size));
            }
            let stacked = _even(size);
            let mut at = stacked;
            while at != 0 {
                let width = if at > size || at < 4 { 2 } else { 4 };
                at -= width;
                let loaded = width.min(size - at);
                let reference =
                    self.cell(&aggregate.address.at(aggregate.address.disp() + at), loaded as u32, None);
                let got = self.load(reference, &format!("TY_UINT_{loaded}"));
                self.op(Kind::Arg, vec![], vec![Arg::Held(got)], None, None, Extra::default());
            }
            return Ok(stacked);
        }
        if let Got::Address(Address::Far(far)) = &value {
            if let Some(whole) = far.whole.filter(|_| far.disp == 0) {
                self.op(Kind::Arg, vec![], vec![Arg::Held(held(whole, 4))], None, None, Extra::default());
                return Ok(4);
            }
            self.op(Kind::Arg, vec![], vec![Arg::Held(held(far.segment, 2))], None, None, Extra::default());
            let offset = self.near(&Address::near(far.offset, far.disp))?;
            self.op(Kind::Arg, vec![], vec![Arg::Held(held(offset, 2))], None, None, Extra::default());
            return Ok(4);
        }
        let width = 2.max(self.width(type_)?);
        let operand = self.operand(value, type_)?;
        let operand = self.narrowed(operand, width)?;
        self.op(Kind::Arg, vec![], vec![operand.arg()], None, None, Extra::default());
        Ok(i64::from(width))
    }

    // ---- values and cells ----

    fn operand(&mut self, got: Got, type_: &str) -> R<Operand> {
        match got {
            Got::Function(_) => {
                // C function designators decay to pointers in every scalar context but a direct call.
                let converted = self.convert(got, type_, type_)?;
                self.operand(converted, type_)
            }
            Got::Held(one) => Ok(Operand::Held(one)),
            Got::Const(one) => Ok(Operand::Const(one)),
            Got::Address(Address::Far(far)) if far.whole.is_some() && far.disp == 0 => {
                Ok(Operand::Held(held(far.whole.unwrap(), 4)))
            }
            Got::Address(Address::Far(far)) => {
                let whole = self.fresh();
                let offset = self.near(&Address::near(far.offset, far.disp))?;
                self.op(
                    Kind::Concat,
                    vec![Arg::Held(held(whole, 4))],
                    vec![Arg::Held(held(far.segment, 2)), Arg::Held(held(offset, 2))],
                    None,
                    None,
                    Extra::default(),
                );
                self.pointer_values.insert(whole);
                Ok(Operand::Held(held(whole, 4)))
            }
            Got::Address(address) => Ok(Operand::Held(held(self.near(&address)?, 2))),
            Got::Returned(_) => {
                let pointed = self.points(got, type_)?;
                self.operand(pointed, type_)
            }
            Got::Real(real) if real.width == 4 => {
                Ok(Operand::Const(constant(signed_word(&_packed(real.value, 4)), 4)))
            }
            Got::FloatCell(cell) if cell.width == 4 => {
                let reference = self.cell(&cell.address, 4, None);
                Ok(Operand::Held(self.load(reference, "TY_UINT_4")))
            }
            got => self.unsupported(format!("{} used as a value", got.repr())),
        }
    }

    /// A literal at a float type's precision.
    fn real(&self, value: f64, width: u32) -> Real {
        let value = if width == 4 { f64::from(f32::from_le_bytes(_packed(value, 4).try_into().unwrap())) } else { value };
        Real { value, width }
    }

    /// A float on the x87.
    fn floating(&mut self, got: Got) -> R<Held> {
        match got {
            Got::Held(one) if one.width == 10 => Ok(one),
            Got::FloatCell(cell) => {
                let reference = self.cell(&cell.address, cell.width, None);
                let result = self.fresh();
                let extra = Extra {
                    loads: vec![reference.clone()],
                    floating: Some(_loaded(formats(cell.width))),
                    ..Extra::default()
                };
                self.op(
                    Kind::Fload,
                    vec![Arg::Held(held(result, 10))],
                    vec![Arg::Cell(Cell { r#ref: reference })],
                    None,
                    None,
                    extra,
                );
                Ok(held(result, 10))
            }
            // fldz and fld1 load these with no operand.
            Got::Real(real) if (real.value == 0.0 || real.value == 1.0) && real.value.is_sign_positive() => {
                let result = self.fresh();
                let extra = Extra { floating: Some(_loaded(integer_formats(2))), ..Extra::default() };
                self.op(
                    Kind::Fload,
                    vec![Arg::Held(held(result, 10))],
                    vec![Arg::Const(constant(real.value as i64, 2))],
                    None,
                    None,
                    extra,
                );
                Ok(held(result, 10))
            }
            Got::Real(real) => {
                // A constant in memory, at the narrowest width that holds it exactly.
                let narrow = real.value as f32;
                let width = if !(narrow.is_infinite() && real.value.is_finite()) && f64::from(narrow) == real.value {
                    4
                } else {
                    8
                };
                let packed = _packed(real.value, width);
                let next = self.shared.literals.len() as i64;
                let number = *self.shared.literals.entry(packed).or_insert(next);
                let address = Address::Global(Global {
                    space: Space::Segment,
                    index: POOL + number,
                    disp: 0,
                    base: None,
                    declared: None,
                    volatile: false,
                });
                self.floating(Got::FloatCell(FloatCell { address, width }))
            }
            got => self.unsupported(format!("{} computed as a float", got.repr())),
        }
    }

    /// C's float-to-integer cast rounds toward zero, whatever the environment says.
    fn truncated(&mut self, value: Held, type_: &str) -> R<Operand> {
        let width = 2.max(self.width(type_)?);
        let result = self.fresh();
        let format = if type_ == "TY_UINT_8" { Format::Unsigned64 } else { integer_formats(width) };
        let rule = floating::Semantics::new([Format::Extended80], format, Precision::Destination, Rounding::TowardZero);
        let extra = Extra { floating: Some(rule), ..Extra::default() };
        self.op(Kind::Fstore, vec![Arg::Held(held(result, width))], vec![Arg::Held(value)], None, None, extra);
        if width == 8 {
            return Ok(Operand::Held(held(result, width)));
        }
        let converted =
            self.convert(Got::Held(held(result, width)), if width == 4 { "TY_INT_4" } else { "TY_INT_2" }, type_)?;
        self.as_operand(converted)
    }

    fn address(&mut self, got: Got) -> R<Address> {
        match got {
            Got::Address(address) => Ok(address),
            Got::Held(one) if one.width == 2 => Ok(Address::near(one.value, 0)),
            Got::Held(one) if one.width == 4 => Ok(Address::Far(self.split(one))),
            Got::Const(one) if one.width == 4 => {
                let segment = self.copy(&Operand::Const(constant((&one.n >> 16u32) & BigInt::from(0xFFFF), 2)));
                let offset = self.copy(&Operand::Const(constant(&one.n & BigInt::from(0xFFFF), 2)));
                Ok(Address::Far(Far {
                    segment,
                    offset,
                    disp: 0,
                    whole: None,
                    named: 0,
                    declared: None,
                    volatile: false,
                }))
            }
            Got::Const(one) if one.width == 2 => {
                let base = self.copy(&Operand::Const(constant(&one.n & BigInt::from(0xFFFF), 2)));
                Ok(Address::near(base, 0))
            }
            got => self.unsupported(format!("{} used as an address", got.repr())),
        }
    }

    fn loaded(&self, got: Got, node: &str) -> Got {
        let type_ = self.type_of(node);
        let near = type_ == "TY_NEAR_POINTER" || (type_ == "TY_POINTER" && !self.far_pointer(&type_));
        match got {
            Got::Held(one) if near && one.width == 2 => Got::Address(Address::near(one.value, 0)),
            got => got,
        }
    }

    /// A far pointer in memory, as its offset word, its segment word and the whole.
    fn far_loaded(&mut self, address: &Address, type_: &str) -> R<Far> {
        let cell = self.cell(address, 4, Some(type_));
        let whole = self.load(cell, type_);
        let cell = self.cell(address, 2, Some(type_));
        let offset = self.load(cell, "TY_UINT_2");
        let cell = self.cell(&address.at(address.disp() + 2), 2, Some(type_));
        let segment = self.load(cell, "TY_UINT_2");
        Ok(Far {
            segment: segment.value,
            offset: offset.value,
            disp: 0,
            whole: Some(whole.value),
            named: 0,
            declared: None,
            volatile: false,
        })
    }

    fn split(&mut self, pointer: Held) -> Far {
        let segment = self.fresh();
        self.op(
            Kind::Shr,
            vec![Arg::Held(held(segment, 4))],
            vec![Arg::Held(pointer), Arg::Const(constant(16, 1))],
            None,
            None,
            Extra::default(),
        );
        Far {
            segment,
            offset: pointer.value,
            disp: 0,
            whole: Some(pointer.value),
            named: 0,
            declared: None,
            volatile: false,
        }
    }

    fn near(&mut self, address: &Address) -> R<Value> {
        match address {
            Address::Frame(frame) => {
                let disp = frame.disp;
                let result = self.fresh();
                self.op(
                    Kind::Address,
                    vec![Arg::Held(held(result, 2))],
                    vec![Arg::FrameAddress(FrameAddress { offset: disp, width: 2, extent: self.extent(disp) })],
                    None,
                    None,
                    Extra::default(),
                );
                self.pointer_values.insert(result);
                if let Some((low, high)) = self.extent(disp) {
                    let object = self.frame_object(low, high);
                    self.pointer_seeds.insert(result, one_slice(object, disp - low, disp - low + 1));
                }
                Ok(result)
            }
            Address::Global(global) if global.base.is_none() => {
                let result = self.fresh();
                self.op(
                    Kind::Copy,
                    vec![Arg::Held(held(result, 2))],
                    vec![Arg::Symbol(mir::Symbol::new(global.space, global.index, global.disp, 2))],
                    None,
                    None,
                    Extra::default(),
                );
                self.pointer_values.insert(result);
                let object = global_object(global.space, global.index);
                self.pointer_seeds.insert(result, one_slice(object, global.disp, global.disp + 1));
                Ok(result)
            }
            Address::Global(global) => {
                let unindexed = Address::Global(Global {
                    space: global.space,
                    index: global.index,
                    disp: global.disp,
                    base: None,
                    declared: None,
                    volatile: false,
                });
                let base = self.near(&unindexed)?;
                Ok(self.add(held(base, 2), &Operand::Held(held(global.base.unwrap(), 2))))
            }
            Address::Near(near) if near.disp == 0 => Ok(near.base),
            Address::Near(near) => Ok(self.add(held(near.base, 2), &Operand::Const(constant(near.disp, 2)))),
            Address::Far(_) => self.unsupported(format!("{} has no near form", address.repr())),
        }
    }

    /// DGROUP's selector, which is SS's in this model: a near address's far form.
    fn dgroup(&mut self) -> Value {
        let result = self.fresh();
        self.op(
            Kind::Copy,
            vec![Arg::Held(held(result, 2))],
            vec![Arg::Symbol(mir::Symbol::new(Space::Group, 0, 0, 2))],
            None,
            None,
            Extra::default(),
        );
        result
    }

    fn frame_object(&self, low: i64, high: i64) -> MemoryObject {
        MemoryObject {
            kind: MemoryKind::Frame,
            identity: Some(Identity::Tuple(vec![
                Identity::Int(self.symbol.id),
                Identity::Int(low),
                Identity::Int(high),
            ])),
            generation: 0,
            extent: Some(high - low),
            addressed: true,
            captured: true,
        }
    }

    /// The reference, typed by the object it names where declared, else by the lvalue's type.
    fn cell(&self, address: &Address, width: u32, type_: Option<&str>) -> MemRef {
        let declared = address.declared().cloned();
        let access = self.aliasing(type_).unwrap_or_else(|error| panic!("{}", error.0));
        let mut reference = self.placed(address, width);
        if address.is_volatile() {
            reference.volatile = true;
        }
        if let Some(declared) = declared {
            reference.typed = Some((declared, true));
            return reference;
        }
        if let Some(access) = access {
            reference.typed = Some((access, false));
        }
        reference
    }

    fn placed(&self, address: &Address, width: u32) -> MemRef {
        let width_i = i64::from(width);
        match address {
            Address::Frame(frame) => {
                let disp = frame.disp;
                let provenance = self.extent(disp).map(|(low, high)| {
                    one_slice(self.frame_object(low, high), disp - low, disp - low + width_i)
                });
                MemRef {
                    space: Some(Space::Frame),
                    provenance,
                    ..MemRef::new(Some(Addr::new(Space::Frame, disp)), width)
                }
            }
            Address::Global(global) => {
                let addr = Addr { index: global.index, ..Addr::new(global.space, global.disp) };
                let object = global_object(global.space, global.index);
                match global.base {
                    None => MemRef {
                        space: Some(global.space),
                        provenance: Some(one_slice(object, global.disp, global.disp + width_i)),
                        ..MemRef::new(Some(addr), width)
                    },
                    Some(base) => MemRef {
                        base: Some(base),
                        space: Some(global.space),
                        base_width: 2,
                        provenance: Some(Provenance::one(object)),
                        ..MemRef::new(Some(addr), width)
                    },
                }
            }
            Address::Near(near) => MemRef {
                base: Some(near.base),
                space: Some(near.space),
                base_width: 2,
                ..MemRef::new(Some(Addr::new(Space::Literal, near.disp)), width)
            },
            Address::Far(far) => {
                let provenance = (far.named != 0).then(|| {
                    Provenance::one(MemoryObject {
                        identity: Some(Identity::Int(far.named)),
                        ..MemoryObject::new(MemoryKind::Named)
                    })
                });
                MemRef {
                    base: Some(far.offset),
                    segment: Some(far.segment),
                    space: Some(Space::Far),
                    base_width: 2,
                    provenance,
                    ..MemRef::new(Some(Addr { index: far.named, ..Addr::new(Space::Far, far.disp) }), width)
                }
            }
        }
    }

    fn load(&mut self, reference: MemRef, type_: &str) -> Held {
        let result = self.fresh();
        let extra = Extra { loads: vec![reference.clone()], ..Extra::default() };
        let got = if reference.width == 1 {
            let kind = if signed(type_) { Kind::SignExtend } else { Kind::ZeroExtend };
            self.op(kind, vec![Arg::Held(held(result, 2))], vec![Arg::Cell(Cell { r#ref: reference.clone() })], None, None, extra);
            held(result, 2)
        } else {
            let width = reference.width;
            self.op(
                Kind::Load,
                vec![Arg::Held(held(result, width))],
                vec![Arg::Cell(Cell { r#ref: reference.clone() })],
                None,
                None,
                extra,
            );
            held(result, width)
        };
        self._pointer_loaded(got, &reference, type_);
        got
    }

    fn _pointer_loaded(&mut self, got: Held, reference: &MemRef, type_: &str) {
        let type_ = self.unit.canonical_type(type_);
        if !pointers(&type_) {
            return;
        }
        self.pointer_values.insert(got.value);
        let number = reference
            .addr
            .filter(|addr| addr.space == Space::Frame)
            .and_then(|addr| self.parameter_at.get(&addr.disp));
        if let Some(number) = number {
            let object = MemoryObject { identity: Some(Identity::Int(*number)), ..MemoryObject::new(MemoryKind::Parameter) };
            self.pointer_seeds.insert(got.value, one_slice(object, 0, 1));
        }
    }

    fn store(&mut self, reference: MemRef, value: Operand) -> R<()> {
        let value = self.narrowed(value, reference.width)?;
        let extra = Extra { stores: vec![reference.clone()], ..Extra::default() };
        self.op(Kind::Store, vec![Arg::Cell(Cell { r#ref: reference })], vec![value.arg()], None, None, extra);
        Ok(())
    }

    fn copy(&mut self, value: &Operand) -> Value {
        let result = self.fresh();
        self.op(
            Kind::Copy,
            vec![Arg::Held(held(result, value.width()))],
            vec![value.arg()],
            None,
            None,
            Extra::default(),
        );
        result
    }

    fn narrowed(&self, value: Operand, width: u32) -> R<Operand> {
        match value {
            Operand::Const(one) => {
                let n = if one.n.is_negative() { one.n } else { one.n & ((BigInt::from(1) << (8 * width)) - 1) };
                Ok(Operand::Const(constant(n, width)))
            }
            Operand::Held(one) if one.width > width => Ok(Operand::Held(held(one.value, width))),
            Operand::Held(one) if one.width < width => {
                self.unsupported(format!("{} used at width {width}", one.repr()))
            }
            held => Ok(held),
        }
    }

    /// A one-byte type is carried as a word, extended the way the type says.
    fn extended(&mut self, value: Held, type_: &str) -> R<Held> {
        if self.width(type_)? != 1 {
            return Ok(value);
        }
        let result = self.fresh();
        let kind = if signed(type_) { Kind::SignExtend } else { Kind::ZeroExtend };
        self.op(
            kind,
            vec![Arg::Held(held(result, 2))],
            vec![Arg::Held(held(value.value, 1))],
            None,
            None,
            Extra::default(),
        );
        Ok(held(result, 2))
    }

    fn wrapped(&self, n: &BigInt, type_: &str) -> R<BigInt> {
        let bits = 8 * self.width(type_)?;
        let n: BigInt = n & ((BigInt::from(1) << bits) - 1);
        Ok(if signed(type_) && !(&n >> (bits - 1)).is_zero() { n - (BigInt::from(1) << bits) } else { n })
    }
}

fn global_object(space: Space, index: i64) -> MemoryObject {
    let kind = if space == Space::External { MemoryKind::External } else { MemoryKind::Global };
    MemoryObject {
        identity: Some(Identity::Tuple(vec![Identity::Space(space), Identity::Int(index)])),
        ..MemoryObject::new(kind)
    }
}

/// `memory.Provenance.one(object_, low, high)`.
fn one_slice(object: MemoryObject, low: i64, high: i64) -> Provenance {
    Provenance::one_with_slice(object, low, high, 1, 1, BTreeSet::new())
        .unwrap_or_else(|error| panic!("ValueError: {error:?}"))
}

/// `int(text)`, unbounded.
fn big(text: &str) -> BigInt {
    text.trim().parse().unwrap_or_else(|_| panic!("ValueError: invalid literal for int() with base 10: {text:?}"))
}

/// `float(text)`.
fn float(text: &str) -> f64 {
    text.trim().parse().unwrap_or_else(|_| panic!("ValueError: could not convert string to float: {text:?}"))
}

/// `float(int)`, correctly rounded.
fn to_float(n: &BigInt) -> f64 {
    n.to_f64().unwrap_or_else(|| panic!("OverflowError: int too large to convert to float"))
}

fn _fold(cg_op: &str, a: &BigInt, b: &BigInt, signed: bool) -> R<BigInt> {
    let quotient = || {
        // C truncates signed division toward zero.
        let magnitude = a.abs() / b.abs();
        if signed && (a.is_negative() != b.is_negative()) { -magnitude } else { magnitude }
    };
    let count = || b.to_usize().unwrap_or_else(|| panic!("ValueError: negative shift count"));
    Ok(match cg_op {
        "O_PLUS" => a + b,
        "O_MINUS" => a - b,
        "O_TIMES" => a * b,
        "O_AND" => a & b,
        "O_OR" => a | b,
        "O_XOR" => a ^ b,
        "O_LSHIFT" => a << count(),
        "O_RSHIFT" => a >> count(),
        "O_DIV" if !b.is_zero() => quotient(),
        "O_MOD" if !b.is_zero() => a - quotient() * b,
        _ => return Err(Unsupported(format!("constant {cg_op}"))),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use iced_x86::Register;

    use super::{Shared, raised};
    use crate::analysis::regions;
    use crate::backend::lower_int64;
    use crate::cfront::{hir, stream};
    use crate::model::ir::Operation;
    use crate::model::memory::MemoryKind;
    use crate::model::mir::{self, Arg, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, Value};
    use crate::objectfile::module::{Addr, Space};

    fn fixture(path: &str) -> String {
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/c").join(path)).unwrap()
    }

    fn unit(text: &str) -> hir::Unit {
        hir::unit(&stream::parse(text)).unwrap()
    }

    fn named<'a>(unit: &'a hir::Unit, name: &str) -> &'a hir::Proc {
        unit.procs.iter().find(|one| unit.symbols[&one.symbol].name == name).unwrap()
    }

    fn body(unit: &hir::Unit, proc: &hir::Proc) -> MirBody {
        raised(unit, proc, &mut Shared::default()).unwrap().body
    }

    fn ops(body: &MirBody) -> impl Iterator<Item = &Op> {
        body.blocks.iter().flat_map(|block| &block.ops)
    }

    fn width(arg: &Arg) -> u32 {
        match arg {
            Arg::Held(one) => one.width,
            Arg::Const(one) => one.width,
            Arg::Symbol(one) => one.width,
            other => panic!("no width: {other:?}"),
        }
    }

    /// `ls_animate(&ls, 0.05f)` pushes the single's four bytes; CGFloat was refused.
    #[test]
    fn test_float_moves_as_its_bits() {
        let unit = unit(&fixture("ls.cgs"));
        let body = body(&unit, named(&unit, "ls_selftest"));
        let pushed: Vec<&Arg> = ops(&body).filter(|op| op.kind == Kind::Arg).map(|op| &op.args[0]).collect();
        assert!(pushed.contains(&&Arg::Const(Const::new(0x3D4C_CCCD, 4))));
    }

    /// MIR says what each operation computes; lowering picks the instruction.
    /// The raise wrote `mov`, `lea` and `fistp` into every op it made, `call`
    /// and `retf` as machine semantics, `add sp` naming SP, and ES and BX into
    /// every far cell.
    #[test]
    fn test_raised_mir_names_no_instruction() {
        for module in ["pal", "qglsurf", "choose", "ls"] {
            let unit = unit(&fixture(&format!("{module}.cgs")));
            for proc in &unit.procs {
                for op in ops(&body(&unit, proc)) {
                    assert_eq!(
                        (op.op, op.name.as_str()),
                        (Some(OpCode::Operation(Operation::Nothing)), ""),
                        "{module} {op:?}"
                    );
                    for one in op.loads.iter().chain(&op.stores) {
                        if let Some(addr) = one.addr.filter(|addr| addr.space == Space::Far) {
                            assert_eq!((addr.base, addr.segment), (Register::None, Register::None), "{module} {one:?}");
                        }
                    }
                }
            }
        }
    }

    /// `raw` ends in inline code and `return;`-less: its MIR returned nothing,
    /// and DX:AX reached the caller only because nothing was emitted after.
    #[test]
    fn test_value_less_return_returns_what_the_code_left() {
        let unit = unit(&fixture("inline.cgs"));
        let body = body(&unit, named(&unit, "raw"));
        let returned = ops(&body).find(|op| op.kind == Kind::Return).unwrap();
        assert_eq!(returned.args.len(), 2);
    }

    /// euclid64's remainder lived in EBX:ECX by mutating `MirBody.origin`.
    /// Int64 legalization's fixed helper result is a backend allocation hint
    /// keyed by the new scalar variables; public MIR stays unchanged.
    #[test]
    fn test_int64_helper_result_placement_is_an_external_hint() {
        let unit = unit(&fixture("mir/euclid64.cgs"));
        let proc = unit.procs.iter().find(|one| unit.symbols[&one.symbol].object_name() == "_gcd64").unwrap();
        let raised = raised(&unit, proc, &mut Shared::default()).unwrap();
        let legalized =
            lower_int64::expanded(&raised.body, Some(&raised.calls), Some(&raised.contracts), Some(&raised.hints))
                .unwrap();

        let added: Vec<(&u32, &Register)> = legalized
            .hints
            .origins
            .iter()
            .filter(|(variable, _)| !raised.hints.origins.contains_key(variable))
            .collect();
        let registers: BTreeSet<Register> = added.iter().map(|(_, register)| **register).collect();
        assert_eq!(registers, BTreeSet::from([Register::EBX, Register::ECX]));
        assert_eq!(added.len(), 2);
    }

    /// An index value with no register in the address read as element zero
    /// alone, so a store to `sy[j]` did not reach `sy[2]`.
    #[test]
    fn test_indexed_cell_reaches_its_whole_symbol() {
        let j = Value::new(1, 10);
        let element = MemRef {
            base: Some(j),
            space: Some(Space::Segment),
            base_width: 2,
            ..MemRef::new(Some(Addr { index: 5, ..Addr::new(Space::Segment, 0) }), 2)
        };
        let fixed = MemRef {
            space: Some(Space::Segment),
            ..MemRef::new(Some(Addr { index: 5, ..Addr::new(Space::Segment, 4) }), 2)
        };
        assert!(regions::overlapping(&fixed, &element, None, None, None).unwrap());
    }

    /// verify asked only whether the defining block dominates, so the loop limit
    /// read before its start, in the same block, passed as SSA.
    #[test]
    fn test_verify_reports_a_use_before_its_definition_in_one_block() {
        let (start, limit) = (Value::new(1, 1), Value::new(2, 1));
        let add = Op {
            kind: Kind::Add,
            args: vec![Arg::Held(Held { value: start, width: 2 }), Arg::Const(Const::new(100, 2))],
            results: vec![Arg::Held(Held { value: limit, width: 2 })],
            ..Op::new(1, OpCode::Operation(Operation::Nothing), "", vec![limit], vec![start])
        };
        let copy = Op {
            kind: Kind::Copy,
            args: vec![Arg::Const(Const::new(0, 2))],
            results: vec![Arg::Held(Held { value: start, width: 2 })],
            ..Op::new(1, OpCode::Operation(Operation::Nothing), "", vec![start], vec![])
        };
        let body = MirBody::new(1, vec![MirBlock::new(1, vec![], vec![add, copy], vec![])]);
        assert!(mir::verify(&body).iter().any(|problem| problem.contains("before its definition")));
    }

    /// qcport's combat_brush_points stopped at `no scalar width for T51`
    /// while compiling `*target = *center`.
    #[test]
    fn test_aggregate_copy_through_pointers_reaches_mir() {
        let unit = unit(&fixture("tests/test_aggregate_copy_through_pointers_reaches_mir.cgs"));
        let body = body(&unit, &unit.procs[0]);
        let loads: u32 = ops(&body)
            .filter(|op| op.kind == Kind::Load)
            .flat_map(|op| &op.loads)
            .filter(|one| one.width == 4)
            .map(|one| one.width)
            .sum();
        let stores: u32 =
            ops(&body).filter(|op| op.kind == Kind::Store).flat_map(|op| &op.stores).map(|one| one.width).sum();
        assert_eq!(loads, 24);
        assert_eq!(stores, 24);
    }

    /// qcport's combat_radius passes a BspVec3 by value; the frontend stopped
    /// at `no scalar width for T51` instead of laying its 12 bytes on the stack.
    #[test]
    fn test_aggregate_argument_is_pushed_by_value() {
        let unit = unit(&fixture("tests/test_aggregate_argument_is_pushed_by_value.cgs"));
        let body = body(&unit, &unit.procs[0]);
        let widths: Vec<u32> = ops(&body).filter(|op| op.kind == Kind::Arg).map(|op| width(&op.args[0])).collect();
        assert_eq!(widths, [4, 4, 4]);
    }

    /// OW parsed restrict but discarded it before CG; the shim now records it.
    #[test]
    fn test_restrict_reaches_mir_as_distinct_noalias_roots() {
        let text = fixture("tests/test_restrict_reaches_mir_as_distinct_noalias_roots.cgs");
        assert_eq!(text.matches(" CGAttr ").count(), 3);

        let unit = unit(&text);
        let body = body(&unit, &unit.procs[0]);
        let roots: BTreeSet<_> = ops(&body)
            .flat_map(|op| op.loads.iter().chain(&op.stores))
            .filter_map(|one| one.provenance.as_ref())
            .filter_map(|provenance| provenance.restrict.iter().next())
            .collect();
        assert_eq!(roots.len(), 3);
    }

    /// A pointer returned by malloc is an allocation-site object, not an unknown pointer.
    #[test]
    fn test_standard_allocator_return_has_fresh_object_identity() {
        let unit = unit(&fixture("tests/test_standard_allocator_return_has_fresh_object_identity.cgs"));
        let body = body(&unit, &unit.procs[0]);
        let allocations: BTreeSet<_> = body
            .pointer_seeds
            .values()
            .flat_map(|provenance| &provenance.slices)
            .map(|slice| &slice.object)
            .filter(|object| object.kind == MemoryKind::Allocation)
            .collect();

        assert_eq!(allocations.len(), 1);
        assert_eq!(allocations.first().unwrap().extent, Some(8));
    }
}
