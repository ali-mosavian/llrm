//! Exact finite values of floating operations: llrm-core's
//! `analysis/floatfacts.rs`, a port of `qbopt/analysis/floatfacts.py`,
//! adapted to the rich MIR. LLVM's counterpart is ValueTracking's
//! floating-point class, `computeKnownFPClass`, narrowed to one value.
//!
//! A fact is a result exact in its format, so it holds under any rounding
//! mode and any flush-to-zero: both are the environment's, which the rich
//! MIR does not state. `nsz`, the frontend's promise, lets an exact
//! cancellation's zero be +0.
//!
//! What changed with the IR:
//! - A rule is its instruction's (`rule`): the arithmetic, `fneg`, `llvm.fabs`,
//!   `llvm.sqrt`, the casts, `llvm.lrint`, and a float load or store. `fptosi`
//!   and `fptoui` truncate, as the rich MIR defines them.
//! - Memory is consts': a float store's bits reach its cells as a fact about
//!   the stored value, where the old one shadowed the body.
//! - A float phi is known where every incoming agrees, as consts' phis.
//! - A loop exit's latch ends in its `br` to the header.
//!
//! Dropped, no rich MIR analogue: `Extended80` and precision control (x87
//! state; the rich MIR's types are `float` and `double`); `checkpoint`
//! (`Fcheck`: the rich MIR observes no FP exception); caching through
//! `manager` (consts' note).
//!
//! Tests skipped: BC object corpora,
//! `test_a_loop_exit_repeats_its_stores_every_iteration`
//! and `test_fpcse_known_inputs_reach_float_computations`, rewritten as MIR;
//! `test_entry_bytes_are_killed_by_a_store`, consts' own.

use std::cmp::Ordering;
use std::ops::{Add, Div, Mul, Neg, Sub};

use llrm_mir::context::ConstantKind;
use llrm_mir::context::Context;
use llrm_mir::intrinsics::{FloatFunction, Intrinsic};
use llrm_mir::module::Function;
use llrm_mir::module::{InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Opcode};
use llrm_mir::types::{FloatKind, Type, TypeId, Types};
use llrm_support::hash::HashSet;
use llrm_support::hash::IndexMap;
use num_bigint::BigInt;

use crate::cfg;
use crate::consts::{self, _MemoryQueries, Calls, Cells, HeldCells, Known};
use crate::graph::loops;
use crate::induction;
use crate::memory::{MemRef, Unit};
use crate::regions;

/// Whether `ty` is, or holds, a floating-point number; an identified struct,
/// whose body this does not read, may.
fn floating(
    types: &Types,
    ty: TypeId,
) -> bool {
    match types.get(ty) {
        Type::Float(_) | Type::Named(_) => true,
        Type::Array { element, .. } | Type::Vector { element, .. } => floating(types, *element),
        Type::Struct { fields, .. } => fields.iter().any(|&field| floating(types, field)),
        _ => false,
    }
}

/// Whether `function` has any floating-point value or operand. Every solve of
/// floats starts from one: where there is none, no float loop, fold or fact
/// exists, and asking for the solve (a whole-function dataflow over memory) is
/// a cost for nothing.
pub fn touches(
    context: &Context,
    function: &Function,
) -> bool {
    let types = &context.types;
    function
        .walk()
        .any(
            |(_, inst)| {
                let instruction = function.instruction(inst);
                instruction.result.is_some_and(|value| floating(types, function.value(value).ty))
                    || instruction
                        .operands
                        .iter()
                        .any(|&operand| function.operand_type(context, operand).is_some_and(|ty| floating(types, ty)))
            },
        )
}

/// Python's `fractions.Fraction`: always in lowest terms, denominator positive.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Fraction {
    pub numerator: BigInt,
    pub denominator: BigInt,
}

impl Fraction {
    pub fn new(
        numerator: impl Into<BigInt>,
        denominator: impl Into<BigInt>,
    ) -> Self {
        let (mut numerator, mut denominator) = (numerator.into(), denominator.into());
        assert!(denominator != BigInt::from(0), "Fraction(_, 0)");
        if denominator < BigInt::from(0) {
            numerator = -numerator;
            denominator = -denominator;
        }
        let divisor = induction::gcd(numerator.magnitude().clone().into(), denominator.clone());
        Self { numerator: numerator / &divisor, denominator: denominator / divisor }
    }

    pub fn from_integer(value: impl Into<BigInt>) -> Self {
        Self::new(value, 1)
    }

    pub fn is_zero(&self) -> bool {
        self.numerator == BigInt::from(0)
    }

    fn abs(&self) -> Self {
        Self::new(self.numerator.magnitude().clone(), self.denominator.clone())
    }

    /// Python's `int(fraction)`, truncating toward zero.
    pub fn int(&self) -> BigInt {
        &self.numerator / &self.denominator
    }
}

/// Python's `str(fraction)`: `n` over one, else `n/d`.
impl std::fmt::Display for Fraction {
    fn fmt(
        &self,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        if self.denominator == BigInt::from(1) {
            write!(formatter, "{}", self.numerator)
        } else {
            write!(formatter, "{}/{}", self.numerator, self.denominator)
        }
    }
}

impl Ord for Fraction {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        (&self.numerator * &other.denominator).cmp(&(&other.numerator * &self.denominator))
    }
}

impl PartialOrd for Fraction {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Add for &Fraction {
    type Output = Fraction;
    fn add(
        self,
        other: Self,
    ) -> Fraction {
        Fraction::new(
            &self.numerator * &other.denominator + &other.numerator * &self.denominator,
            &self.denominator * &other.denominator,
        )
    }
}

impl Sub for &Fraction {
    type Output = Fraction;
    fn sub(
        self,
        other: Self,
    ) -> Fraction {
        self + &-other
    }
}

impl Mul for &Fraction {
    type Output = Fraction;
    fn mul(
        self,
        other: Self,
    ) -> Fraction {
        Fraction::new(&self.numerator * &other.numerator, &self.denominator * &other.denominator)
    }
}

impl Div for &Fraction {
    type Output = Fraction;
    fn div(
        self,
        other: Self,
    ) -> Fraction {
        Fraction::new(&self.numerator * &other.denominator, &self.denominator * &other.numerator)
    }
}

impl Neg for &Fraction {
    type Output = Fraction;
    fn neg(self) -> Fraction {
        Fraction { numerator: -&self.numerator, denominator: self.denominator.clone() }
    }
}

/// A finite value, and for a zero its sign.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Finite {
    pub value: Fraction,
    pub negative_zero: bool,
}

impl Finite {
    pub fn new(
        value: Fraction,
        negative_zero: bool,
    ) -> Self {
        Self { value, negative_zero }
    }

    pub fn negative(&self) -> bool {
        self.value < Fraction::from_integer(0) || self.negative_zero
    }
}

/// How a number is held: old `Format`, less `Extended80`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Format {
    Binary32,
    Binary64,
    Signed(u32),
    Unsigned(u32),
}

impl Format {
    /// A binary format's precision, exponent bits and bias.
    fn binary(self) -> Option<(u32, u32, i64)> {
        match self {
            Self::Binary32 => Some((24, 8, 127)),
            Self::Binary64 => Some((53, 11, 1023)),
            _ => None,
        }
    }

    /// The format of float type `ty`.
    pub fn of(
        types: &Types,
        ty: TypeId,
    ) -> Option<Self> {
        match types.get(ty) {
            Type::Float(FloatKind::Float) => Some(Self::Binary32),
            Type::Float(FloatKind::Double) => Some(Self::Binary64),
            _ => None,
        }
    }

    /// Bits a value of this format has.
    pub fn bits(self) -> u32 {
        match self {
            Self::Binary32 => 32,
            Self::Binary64 => 64,
            Self::Signed(width) | Self::Unsigned(width) => width,
        }
    }
}

/// What a floating instruction does: old `Kind`'s floating kinds, a load
/// or store being a `Convert` to its own format.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Operation {
    Convert,
    /// Toward zero, to an integer: `fptosi`, `fptoui`.
    Truncate,
    Neg,
    Abs,
    Sqrt,
    Add,
    Sub,
    Mul,
    Div,
}

/// Old `Semantics`: an operation, its operands' formats and its result's.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rule {
    pub operation: Operation,
    pub inputs: Vec<Format>,
    pub result: Format,
    /// `nsz`: a zero's sign is insignificant.
    pub nsz: bool,
}

impl Rule {
    pub fn new(
        operation: Operation,
        inputs: &[Format],
        result: Format,
    ) -> Self {
        Self { operation, inputs: inputs.to_vec(), result, nsz: false }
    }
}

/// The floating rule of `inst`, if it has one.
pub fn rule(
    unit: &Unit,
    inst: InstId,
) -> Option<Rule> {
    let op = unit.function.instruction(inst);
    let float = |at: usize| unit.operand_type(*op.operands.get(at)?).and_then(|ty| Format::of(&unit.context.types, ty));
    let int = |at: usize, signed: bool| {
        unit.int_bits(*op.operands.get(at)?)
            .map(|width| if signed { Format::Signed(width) } else { Format::Unsigned(width) })
    };
    let returned = || Format::of(&unit.context.types, op.ty);
    let integer = |signed: bool| {
        unit.context
            .types
            .int_bits(op.ty)
            .map(|width| if signed { Format::Signed(width) } else { Format::Unsigned(width) })
    };
    let (operation, inputs, result) = match op.opcode {
        Opcode::Binary(kind @ (BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul | BinaryOp::FDiv)) => {
            let operation = match kind {
                BinaryOp::FAdd => Operation::Add,
                BinaryOp::FSub => Operation::Sub,
                BinaryOp::FMul => Operation::Mul,
                _ => Operation::Div,
            };
            (operation, vec![float(0)?, float(1)?], returned()?)
        }
        Opcode::FNeg => (Operation::Neg, vec![float(0)?], returned()?),
        Opcode::Cast(CastOp::FPExt | CastOp::FPTrunc) => (Operation::Convert, vec![float(0)?], returned()?),
        Opcode::Cast(CastOp::SIToFP) => (Operation::Convert, vec![int(0, true)?], returned()?),
        Opcode::Cast(CastOp::UIToFP) => (Operation::Convert, vec![int(0, false)?], returned()?),
        Opcode::Cast(CastOp::FPToSI) => (Operation::Truncate, vec![float(0)?], integer(true)?),
        Opcode::Cast(CastOp::FPToUI) => (Operation::Truncate, vec![float(0)?], integer(false)?),
        Opcode::Load { volatile: false, .. } => (Operation::Convert, vec![returned()?], returned()?),
        Opcode::Store { volatile: false, .. } => (Operation::Convert, vec![float(0)?], float(0)?),
        Opcode::Call(_) => match unit.intrinsic(inst)? {
            Intrinsic::Unary(FloatFunction::Fabs) => (Operation::Abs, vec![float(0)?], returned()?),
            Intrinsic::Unary(FloatFunction::Sqrt) => (Operation::Sqrt, vec![float(0)?], returned()?),
            // As the rounding mode says: only an integral input is exact under
            // every one.
            Intrinsic::LRint => (Operation::Convert, vec![float(0)?], integer(true)?),
            _ => return None,
        },
        _ => return None,
    };
    Some(Rule { operation, inputs, result, nsz: llrm_mir::facts::Facts::of_flags(op.flags).no_signed_zeros() })
}

pub fn decoded(
    bits: &BigInt,
    format: Format,
) -> Option<Finite> {
    let zero = BigInt::from(0);
    match format {
        Format::Signed(width) => {
            if !(zero <= *bits && *bits < BigInt::from(1) << width) {
                return None;
            }
            let sign = BigInt::from(1) << (width - 1);
            return Some(Finite::new(Fraction::from_integer((bits ^ &sign) - &sign), false));
        }
        Format::Unsigned(width) => {
            return (zero <= *bits && *bits < BigInt::from(1) << width)
                .then(|| Finite::new(Fraction::from_integer(bits.clone()), false));
        }
        _ => {}
    }
    let (precision, exponent_bits, bias) = format.binary()?;
    if !(zero <= *bits && *bits < BigInt::from(1) << (precision + exponent_bits)) {
        return None;
    }
    let fraction = bits & ((BigInt::from(1) << (precision - 1)) - 1);
    let exponent =
        i64::try_from((bits >> (precision - 1)) & ((BigInt::from(1) << exponent_bits) - 1)).expect("an exponent field");
    let negative = (bits >> (precision + exponent_bits - 1)) != zero;
    if exponent == (1 << exponent_bits) - 1 || (exponent == 0 && fraction != zero) {
        return None;
    }
    if exponent == 0 {
        return Some(Finite::new(Fraction::from_integer(0), negative));
    }
    let significand = (BigInt::from(1) << (precision - 1)) | fraction;
    let shift = exponent - bias - i64::from(precision) + 1;
    let value = Fraction::new(
        significand << u64::try_from(shift.max(0)).expect("non-negative"),
        BigInt::from(1) << u64::try_from((-shift).max(0)).expect("non-negative"),
    );
    Some(Finite::new(if negative { -&value } else { value }, false))
}

fn _fits(
    value: &Fraction,
    precision: u64,
    minimum: i64,
    maximum: i64,
) -> bool {
    if value.is_zero() {
        return true;
    }
    let (numerator, denominator) = (value.numerator.magnitude(), &value.denominator);
    if denominator & (denominator - BigInt::from(1)) != BigInt::from(0) {
        return false;
    }
    let trailing = numerator.trailing_zeros().expect("a nonzero numerator");
    let exponent = numerator.bits() as i64 - denominator.bits() as i64;
    numerator.bits() - trailing <= precision && minimum <= exponent && exponent <= maximum
}

/// The exact result of `rule` on `inputs`, where there is one in its format.
pub fn evaluated(
    rule: &Rule,
    inputs: &[Finite],
) -> Option<Finite> {
    if inputs.len() != rule.inputs.len() {
        return None;
    }
    let result = match (rule.operation, inputs) {
        (Operation::Convert, [value]) => value.clone(),
        (Operation::Truncate, [value]) => Finite::new(Fraction::from_integer(value.value.int()), false),
        (Operation::Neg, [value]) => {
            Finite::new(-&value.value, if value.value.is_zero() { !value.negative_zero } else { false })
        }
        (Operation::Abs, [value]) => Finite::new(value.value.abs(), false),
        (Operation::Sqrt, [value]) => {
            if value.value < Fraction::from_integer(0) {
                return None;
            }
            let numerator = value.value.numerator.sqrt();
            let denominator = value.value.denominator.sqrt();
            if &numerator * &numerator != value.value.numerator
                || &denominator * &denominator != value.value.denominator
            {
                return None;
            }
            Finite::new(Fraction::new(numerator, denominator), value.negative_zero)
        }
        (Operation::Add | Operation::Sub, [left, right]) => {
            let subtract = rule.operation == Operation::Sub;
            let right_value = if subtract { -&right.value } else { right.value.clone() };
            let value = &left.value + &right_value;
            if value.is_zero() {
                let right_negative = right.negative() ^ subtract;
                if rule.nsz {
                    Finite::new(value, false)
                } else if !left.value.is_zero() || !right.value.is_zero() || left.negative() != right_negative {
                    return None; // cancellation's zero sign depends on rounding
                } else {
                    Finite::new(value, left.negative())
                }
            } else {
                Finite::new(value, false)
            }
        }
        (Operation::Mul | Operation::Div, [left, right]) => {
            if rule.operation == Operation::Div && right.value.is_zero() {
                return None;
            }
            let value =
                if rule.operation == Operation::Mul { &left.value * &right.value } else { &left.value / &right.value };
            let negative_zero = value.is_zero() && left.negative() != right.negative();
            Finite::new(value, negative_zero)
        }
        _ => return None,
    };
    match rule.result {
        Format::Signed(width) => {
            let limit = Fraction::from_integer(BigInt::from(1) << (width - 1));
            (result.value.denominator == BigInt::from(1) && -&limit <= result.value && result.value < limit)
                .then_some(result)
        }
        Format::Unsigned(width) => (result.value.denominator == BigInt::from(1)
            && Fraction::from_integer(0) <= result.value
            && result.value < Fraction::from_integer(BigInt::from(1) << width))
        .then_some(result),
        binary => {
            let (precision, _, bias) = binary.binary().expect("a binary format");
            _fits(&result.value, u64::from(precision), 1 - bias, bias).then_some(result)
        }
    }
}

pub fn encoded(
    value: &Finite,
    format: Format,
) -> Option<BigInt> {
    let (precision, exponent_bits, bias) = format.binary()?;
    if !_fits(&value.value, u64::from(precision), 1 - bias, bias) {
        return None;
    }
    let sign = BigInt::from(u8::from(value.negative())) << (precision + exponent_bits - 1);
    if value.value.is_zero() {
        return Some(sign);
    }
    let magnitude = value.value.abs();
    let exponent = magnitude.numerator.bits() as i64 - magnitude.denominator.bits() as i64;
    let shift = i64::from(precision) - 1 - exponent;
    let significand = &magnitude
        * &Fraction::new(
            BigInt::from(1) << u64::try_from(shift.max(0)).expect("non-negative"),
            BigInt::from(1) << u64::try_from((-shift).max(0)).expect("non-negative"),
        );
    let biased = u64::try_from(exponent + bias).expect("a normal exponent");
    Some(sign | (BigInt::from(biased) << (precision - 1)) | (significand.int() - (BigInt::from(1) << (precision - 1))))
}

/// One operand's exact value, read as `format`.
pub fn _operand(
    unit: &Unit,
    operand: Operand,
    format: Format,
    integers: &IndexMap<ValueId, Known>,
    facts: &IndexMap<ValueId, Finite>,
) -> Option<Finite> {
    if let Format::Signed(_) | Format::Unsigned(_) = format {
        return decoded(&consts::_operand(unit, operand, integers, None)?.n, format);
    }
    match operand {
        Operand::Value(value) => facts.get(&value).cloned(),
        Operand::Constant(id) => match unit.context.get(id).kind {
            ConstantKind::Float(bits) => decoded(&BigInt::from(bits), format),
            _ => None,
        },
        Operand::Block(_) => None,
    }
}

/// `inst`'s inputs, a load's from `here`.
fn _inputs(
    unit: &Unit,
    inst: InstId,
    rule: &Rule,
    integers: &IndexMap<ValueId, Known>,
    here: Option<&Cells>,
    queries: &mut _MemoryQueries,
    facts: &IndexMap<ValueId, Finite>,
) -> Option<Vec<Finite>> {
    let op = unit.function.instruction(inst);
    if let Opcode::Load { .. } = op.opcode {
        let reference = queries.resolve(&MemRef::of(unit, inst)?);
        if let Some(bits) = crate::memory::constant_bits(unit, &reference) {
            return Some(vec![decoded(&bits, rule.inputs[0])?]);
        }
        let bits = consts::_cell(here?, &reference)?;
        return Some(vec![decoded(&bits.n, rule.inputs[0])?]);
    }
    rule.inputs
        .iter()
        .zip(&op.operands)
        .map(|(&format, &operand)| _operand(unit, operand, format, integers, facts))
        .collect()
}

/// A result's fact as consts holds a number: the bits of its format.
fn _bits(
    fact: &Finite,
    format: Format,
) -> Option<Known> {
    match format {
        Format::Signed(width) | Format::Unsigned(width) => {
            Some(Known::new(consts::masked(&fact.value.int(), width), width))
        }
        binary => Some(Known::new(encoded(fact, binary)?, binary.bits())),
    }
}

/// Exact memory facts after a caller-proven repetition of a straight-line
/// block, `insts`, ending in its `br`.
pub fn repeated<'a>(
    unit: &Unit<'a>,
    insts: &[InstId],
    count: &BigInt,
    initial: &Cells,
    known: Option<&IndexMap<ValueId, Known>>,
    queries: Option<&mut _MemoryQueries<'a>>,
) -> Option<Cells> {
    if *count < BigInt::from(0) || count * BigInt::from(insts.len()) > BigInt::from(100_000) {
        return None;
    }
    let function = unit.function;
    let internal = insts.iter().filter_map(|&inst| function.instruction(inst).result).collect::<HashSet<_>>();
    let invariant = known
        .into_iter()
        .flatten()
        .filter(|(value, _)| !internal.contains(*value))
        .map(|(value, fact)| (*value, fact.clone()))
        .collect::<IndexMap<_, _>>();
    let mut own;
    let queries = match queries {
        Some(queries) => queries,
        None => {
            own = consts::memory_queries(*unit, &invariant);
            &mut own
        }
    };
    let mut memory = initial.clone();
    let no_calls = Calls::default();
    let count = u64::try_from(count).expect("a repetition count Python could iterate");
    for _ in 0..count {
        let mut integers = invariant.clone();
        let mut floating = IndexMap::<ValueId, Finite>::default();
        for &inst in insts {
            let op = function.instruction(inst);
            let Some(rule) = rule(unit, inst) else {
                match op.opcode {
                    Opcode::Br if op.operands.len() == 1 => continue,
                    Opcode::Binary(BinaryOp::Add | BinaryOp::Sub) => {}
                    _ => return None,
                }
                if let (Some(result), Some(fact)) = (op.result, consts::_result(unit, inst, &integers, Some(&memory))) {
                    integers.insert(result, fact);
                }
                continue;
            };
            let inputs = _inputs(unit, inst, &rule, &integers, Some(&memory), queries, &floating)?;
            let result = evaluated(&rule, &inputs)?;
            if let Opcode::Store { .. } = op.opcode {
                let reference = queries.resolve(&MemRef::of(unit, inst)?);
                let bits = encoded(&result, rule.result)?;
                if reference.addr().is_none() || !reference.object {
                    return None;
                }
                memory = consts::_kills(memory, inst, &integers, &no_calls, None, None, false, queries);
                memory.extend(consts::_fragments(&reference, &Known::new(bits, rule.result.bits())));
                continue;
            }
            let target = op.result?;
            match rule.result {
                Format::Signed(_) | Format::Unsigned(_) => {
                    integers.insert(target, _bits(&result, rule.result)?);
                }
                _ => {
                    floating.insert(target, result);
                }
            }
        }
    }
    Some(memory)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopExit {
    pub header: i64,
    pub count: BigInt,
    pub stores: Vec<(MemRef, Known)>,
}

/// Proven numeric exits of two-block counted loops whose float state is in
/// memory.
pub fn loop_exits(
    unit: &Unit,
    calls: &Calls,
) -> Vec<LoopExit> {
    // Solved only once a loop has the shape asked for: most bodies have none.
    let solved = std::cell::OnceCell::new();
    _exits(unit, calls, || solved.get_or_init(|| solved_with(unit, calls, None)))
}

/// `loop_exits`, given `solved_with(unit, calls, None)`.
pub fn exits(
    unit: &Unit,
    calls: &Calls,
    solved: &Solved,
) -> Vec<LoopExit> {
    _exits(unit, calls, || solved)
}

fn _exits<'s>(
    unit: &Unit,
    calls: &Calls,
    solve: impl Fn() -> &'s Solved,
) -> Vec<LoopExit> {
    let function = unit.function;
    let stores =
        |inst: InstId| matches!(function.instruction(inst).opcode, Opcode::Store { .. }) && rule(unit, inst).is_some();
    if !function.walk().any(|(_, inst)| stores(inst)) {
        return Vec::new();
    }
    if function.entry().is_none() {
        return Vec::new();
    }
    let graph = cfg::graph(function);
    let successors = graph.iter().map(|block| (block.at, &block.succ)).collect::<IndexMap<_, _>>();
    let predecessors = loops::predecessors(&graph);
    let mut exits = Vec::new();
    for loop_ in &unit.shape().loops {
        if loop_.body.len() != 2 || loop_.latches.len() != 1 {
            continue;
        }
        let latch = *loop_.latches.first().expect("one latch");
        let outside = predecessors
            .get(&loop_.header)
            .into_iter()
            .flatten()
            .filter(|at| !loop_.body.contains(at))
            .copied()
            .collect::<Vec<_>>();
        let [preheader] = outside[..] else {
            continue;
        };
        if *successors[&preheader] != [loop_.header] {
            continue;
        }
        let instructions = |at: i64| function.block(cfg::block(at)).instructions();
        if instructions(latch).iter().any(|&inst| function.instruction(inst).opcode == Opcode::Phi) {
            continue;
        }
        let references = instructions(latch).iter().filter_map(|&inst| MemRef::of(unit, inst)).collect::<Vec<_>>();
        let mut stored = Vec::<MemRef>::new();
        for reference in
            instructions(latch).iter().filter(|&&inst| stores(inst)).filter_map(|&inst| MemRef::of(unit, inst))
        {
            if !stored.contains(&reference) {
                stored.push(reference);
            }
        }
        // An overlap Rust cannot represent may overlap.
        if stored.is_empty()
            || instructions(loop_.header).iter().any(|&inst| {
                let op = function.instruction(inst);
                !matches!(
                    op.opcode,
                    Opcode::Phi
                        | Opcode::ICmp(_)
                        | Opcode::Br
                        | Opcode::Store { volatile: false, .. }
                        | Opcode::Binary(BinaryOp::Sub)
                ) || rule(unit, inst).is_some()
                    || Format::of(&unit.context.types, op.ty).is_some()
                    || MemRef::of(unit, inst).is_some_and(|written| {
                        references
                            .iter()
                            .any(|read| regions::overlapping(&written, read, None, None, unit.program).unwrap_or(true))
                    })
            })
        {
            continue;
        }
        let Solved { integers, cells: memory, .. } = solve();
        let Some(count) = induction::agreed_count(&induction::counted(unit, &loop_, Some(integers), false)) else {
            continue;
        };
        let mut asked = consts::memory_queries(*unit, integers);
        let last = function.terminator(cfg::block(preheader)).expect("a terminated block");
        let initial = consts::_kills((*memory[&last]).clone(), last, integers, calls, None, None, false, &mut asked);
        let Some(after) = repeated(unit, instructions(latch), &count, &initial, Some(integers), Some(&mut asked))
        else {
            continue;
        };
        let facts = stored
            .iter()
            .map(|reference| consts::_cell(&after, &asked.resolve(reference)).map(|fact| (reference.clone(), fact)))
            .collect::<Option<Vec<_>>>();
        if let Some(facts) = facts {
            exits.push(LoopExit { header: loop_.header, count, stores: facts });
        }
    }
    exits
}

/// Numeric memory facts on exit edges, never on a header's backedge.
pub fn exit_cells(
    unit: &Unit,
    calls: &Calls,
) -> IndexMap<(i64, i64), Cells> {
    let proofs = loop_exits(unit, calls);
    if proofs.is_empty() {
        return IndexMap::default();
    }
    let function = unit.function;
    let graph = cfg::graph(function);
    let regions = unit.shape().loops.iter().map(|loop_| (loop_.header, loop_.body.clone())).collect::<IndexMap<_, _>>();
    let successors = graph.iter().map(|block| (block.at, &block.succ)).collect::<IndexMap<_, _>>();
    let mut queries = consts::memory_queries(*unit, &IndexMap::default());
    let mut edges = IndexMap::default();
    for proof in proofs {
        let leaving = successors[&proof.header]
            .iter()
            .copied()
            .filter(|at| !regions[&proof.header].contains(at))
            .collect::<Vec<_>>();
        // A header leaving by two edges is not the shape proven.
        let [destination] = leaving[..] else {
            continue;
        };
        let mut memory = Cells::default();
        for (reference, fact) in &proof.stores {
            memory.extend(consts::_fragments(&queries.resolve(reference), fact));
        }
        edges.insert((proof.header, destination), memory);
    }
    edges
}

/// Numeric facts, optionally given independently established entry cells.
pub fn known(
    unit: &Unit,
    calls: &Calls,
    initial: Option<&Cells>,
) -> IndexMap<ValueId, Finite> {
    solved_with(unit, calls, initial).facts
}

/// Exact integer conversion results.
pub fn converted(
    unit: &Unit,
    calls: &Calls,
    facts: Option<&IndexMap<ValueId, Finite>>,
) -> IndexMap<ValueId, Known> {
    let function = unit.function;
    let conversions = function
        .walk()
        .filter_map(|(_, inst)| rule(unit, inst).map(|rule| (inst, rule)))
        .filter(|(_, rule)| matches!(rule.result, Format::Signed(_) | Format::Unsigned(_)))
        .collect::<Vec<_>>();
    if conversions.is_empty() {
        return IndexMap::default();
    }
    let computed;
    let facts = match facts {
        Some(facts) => facts,
        None => {
            computed = known(unit, calls, None);
            &computed
        }
    };
    let none = IndexMap::default();
    let mut results = IndexMap::default();
    for (inst, rule) in conversions {
        let op = function.instruction(inst);
        let Some(input) = _operand(unit, op.operands[0], rule.inputs[0], &none, facts) else {
            continue;
        };
        if let (Some(target), Some(value)) = (op.result, evaluated(&rule, std::slice::from_ref(&input))) {
            results.insert(target, _bits(&value, rule.result).expect("an integer"));
        }
    }
    results
}

/// Memory facts including exact floating stores.
pub fn cells(
    unit: &Unit,
    calls: &Calls,
) -> HeldCells {
    solved_with(unit, calls, None).cells
}

/// One solve: consts' integers, the float facts, and memory with both.
#[derive(Clone, Debug, PartialEq)]
pub struct Solved {
    pub integers: IndexMap<ValueId, Known>,
    pub facts: IndexMap<ValueId, Finite>,
    pub cells: HeldCells,
}

/// `integers`, and the bits of each stored value `facts` knows.
fn _with_stored(
    unit: &Unit,
    integers: &IndexMap<ValueId, Known>,
    facts: &IndexMap<ValueId, Finite>,
    sources: &HashSet<ValueId>,
) -> IndexMap<ValueId, Known> {
    let mut known = integers.clone();
    for value in sources {
        let format = Format::of(&unit.context.types, unit.function.value(*value).ty);
        if let Some(bits) = facts.get(value).zip(format).and_then(|(fact, format)| _bits(fact, format)) {
            known.insert(*value, bits);
        }
    }
    known
}

/// `known` and `cells` in one solve, and the integers under them.
pub fn solved_with(
    unit: &Unit,
    calls: &Calls,
    initial: Option<&Cells>,
) -> Solved {
    solved_over(unit, calls, initial, &consts::known(unit, Some(calls), None, initial))
}

/// `solved_with`, given the integers under it: `consts::known(unit,
/// Some(calls), None, initial)`, which a caller that asks it of the same body
/// for itself (the manager's `ThroughMemory`, for `initial` none) need not
/// derive again.
pub fn solved_over(
    unit: &Unit,
    calls: &Calls,
    initial: Option<&Cells>,
    integers: &IndexMap<ValueId, Known>,
) -> Solved {
    let function = unit.function;
    let rules = function.walk().filter_map(|(_, inst)| rule(unit, inst).map(|rule| (inst, rule))).collect::<Vec<_>>();
    let phis = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| {
            function.instruction(inst).opcode == Opcode::Phi
                && Format::of(&unit.context.types, function.instruction(inst).ty).is_some()
        })
        .collect::<Vec<_>>();
    // Memory changes only where a store's source was just learned.
    let sources = rules
        .iter()
        .filter(|(inst, _)| matches!(function.instruction(*inst).opcode, Opcode::Store { .. }))
        .filter_map(|(inst, _)| match function.instruction(*inst).operands[0] {
            Operand::Value(value) => Some(value),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let mut queries = consts::memory_queries(*unit, &integers);
    let mut facts = IndexMap::<ValueId, Finite>::default();
    let mut memory = HeldCells::default();
    let mut reshadow = true;
    let mut changed = true;
    while changed {
        changed = false;
        if reshadow {
            memory = consts::cells_solved(
                unit,
                calls,
                Some(&_with_stored(unit, &integers, &facts, &sources)),
                initial,
                None,
                None,
                None,
                None,
            )
            .held;
            reshadow = false;
        }
        let mut learned = |value: ValueId, fact: Finite, facts: &mut IndexMap<ValueId, Finite>| {
            facts.insert(value, fact);
            changed = true;
            reshadow |= sources.contains(&value);
        };
        for &phi in &phis {
            let op = function.instruction(phi);
            let result = op.result.expect("a phi's value");
            if facts.contains_key(&result) {
                continue;
            }
            let format = Format::of(&unit.context.types, op.ty).expect("a float phi");
            let seen = op
                .operands
                .iter()
                .step_by(2)
                .map(|&one| _operand(unit, one, format, &integers, &facts))
                .collect::<Option<Vec<_>>>();
            if let Some(seen) = seen.filter(|seen| !seen.is_empty() && seen.iter().all(|one| *one == seen[0])) {
                learned(result, seen[0].clone(), &mut facts);
            }
        }
        for (inst, rule) in &rules {
            let op = function.instruction(*inst);
            let Some(result) = op.result.filter(|result| !facts.contains_key(result)) else {
                continue;
            };
            if let Format::Signed(_) | Format::Unsigned(_) = rule.result {
                continue; // `converted`'s
            }
            let Some(inputs) =
                _inputs(unit, *inst, rule, &integers, memory.get(inst).map(|here| &**here), &mut queries, &facts)
            else {
                continue;
            };
            if let Some(fact) = evaluated(rule, &inputs) {
                learned(result, fact, &mut facts);
            }
        }
    }
    Solved { integers: integers.clone(), facts, cells: memory }
}

#[cfg(test)]
#[path = "floatfacts_tests.rs"]
mod tests;
