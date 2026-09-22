//! Exact finite numeric facts; strict effects remain a separate obligation.
//!
//! Port of `qbopt/analysis/floatfacts.py`.

#![allow(dead_code)] // its consumers are not yet ported

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::ops::{Add, Div, Mul, Neg, Sub};

use crate::support::hash::IndexMap;
use num_bigint::BigInt;

use super::consts::{self, Cells, Known};
use super::{induction, loops, regions};
use crate::model::floating::{Format, Precision, Semantics};
use crate::model::mir::{self, Arg, Const, Kind, MemRef, MirBody, Op, Value};
use crate::objectfile::module::Addr;

/// Python's `fractions.Fraction`: always in lowest terms, denominator positive.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Fraction {
    pub numerator: BigInt,
    pub denominator: BigInt,
}

impl Fraction {
    pub(crate) fn new(numerator: impl Into<BigInt>, denominator: impl Into<BigInt>) -> Self {
        let (mut numerator, mut denominator) = (numerator.into(), denominator.into());
        assert!(denominator != BigInt::from(0), "Fraction(_, 0)");
        if denominator < BigInt::from(0) {
            numerator = -numerator;
            denominator = -denominator;
        }
        let divisor = induction::gcd(numerator.magnitude().clone().into(), denominator.clone());
        Self {
            numerator: numerator / &divisor,
            denominator: denominator / divisor,
        }
    }

    pub(crate) fn from_integer(value: impl Into<BigInt>) -> Self {
        Self::new(value, 1)
    }

    fn is_zero(&self) -> bool {
        self.numerator == BigInt::from(0)
    }

    fn abs(&self) -> Self {
        Self::new(self.numerator.magnitude().clone(), self.denominator.clone())
    }

    /// Python's `int(fraction)`, truncating toward zero.
    fn int(&self) -> BigInt {
        &self.numerator / &self.denominator
    }
}

impl Ord for Fraction {
    fn cmp(&self, other: &Self) -> Ordering {
        (&self.numerator * &other.denominator).cmp(&(&other.numerator * &self.denominator))
    }
}

impl PartialOrd for Fraction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Add for &Fraction {
    type Output = Fraction;
    fn add(self, other: Self) -> Fraction {
        Fraction::new(
            &self.numerator * &other.denominator + &other.numerator * &self.denominator,
            &self.denominator * &other.denominator,
        )
    }
}

impl Sub for &Fraction {
    type Output = Fraction;
    fn sub(self, other: Self) -> Fraction {
        self + &-other
    }
}

impl Mul for &Fraction {
    type Output = Fraction;
    fn mul(self, other: Self) -> Fraction {
        Fraction::new(&self.numerator * &other.numerator, &self.denominator * &other.denominator)
    }
}

impl Div for &Fraction {
    type Output = Fraction;
    fn div(self, other: Self) -> Fraction {
        Fraction::new(&self.numerator * &other.denominator, &self.denominator * &other.numerator)
    }
}

impl Neg for &Fraction {
    type Output = Fraction;
    fn neg(self) -> Fraction {
        Fraction {
            numerator: -&self.numerator,
            denominator: self.denominator.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Finite {
    pub value: Fraction,
    pub negative_zero: bool,
}

impl Finite {
    pub(crate) fn new(value: Fraction, negative_zero: bool) -> Self {
        Self { value, negative_zero }
    }

    pub(crate) fn negative(&self) -> bool {
        self.value < Fraction::from_integer(0) || self.negative_zero
    }
}

static _BINARY: [(Format, (u32, u32, i64)); 2] = [(Format::Binary32, (24, 8, 127)), (Format::Binary64, (53, 11, 1023))];
static _INTEGER: [(Format, u32); 3] = [(Format::Signed16, 16), (Format::Signed32, 32), (Format::Signed64, 64)];
static _UNSIGNED: [(Format, u32); 1] = [(Format::Unsigned64, 64)];

fn _get<T: Copy>(table: &[(Format, T)], format: Format) -> Option<T> {
    table.iter().find(|(one, _)| *one == format).map(|(_, value)| *value)
}

pub(crate) fn decoded(bits: &BigInt, format: Format) -> Option<Finite> {
    let zero = BigInt::from(0);
    if let Some(width) = _get(&_INTEGER, format) {
        if !(zero <= *bits && *bits < BigInt::from(1) << width) {
            return None;
        }
        let sign = BigInt::from(1) << (width - 1);
        return Some(Finite::new(Fraction::from_integer((bits ^ &sign) - &sign), false));
    }
    if let Some(width) = _get(&_UNSIGNED, format) {
        return (zero <= *bits && *bits < BigInt::from(1) << width)
            .then(|| Finite::new(Fraction::from_integer(bits.clone()), false));
    }
    let (precision, exponent_bits, bias) = _get(&_BINARY, format)?;
    if !(zero <= *bits && *bits < BigInt::from(1) << (precision + exponent_bits)) {
        return None;
    }
    let fraction = bits & ((BigInt::from(1) << (precision - 1)) - 1);
    let exponent = i64::try_from((bits >> (precision - 1)) & ((BigInt::from(1) << exponent_bits) - 1)).expect("an exponent field");
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

fn _fits(value: &Fraction, precision: u64, minimum: i64, maximum: i64) -> bool {
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

pub(crate) fn evaluated(kind: Kind, rule: &Semantics, inputs: &[Finite]) -> Option<Finite> {
    if inputs.len() != rule.inputs.len() {
        return None;
    }
    let result = match (kind, inputs) {
        (Kind::Fload | Kind::Fstore, [value]) => value.clone(),
        (Kind::Fneg, [value]) => Finite::new(
            -&value.value,
            if value.value.is_zero() { !value.negative_zero } else { false },
        ),
        (Kind::Fabs, [value]) => Finite::new(value.value.abs(), false),
        (Kind::Fsqrt, [value]) => {
            if value.value < Fraction::from_integer(0) {
                return None;
            }
            let numerator = value.value.numerator.sqrt();
            let denominator = value.value.denominator.sqrt();
            if &numerator * &numerator != value.value.numerator || &denominator * &denominator != value.value.denominator {
                return None;
            }
            Finite::new(Fraction::new(numerator, denominator), value.negative_zero)
        }
        (Kind::Fadd | Kind::Fsub, [left, right]) => {
            let right_value = if kind == Kind::Fadd { right.value.clone() } else { -&right.value };
            let value = &left.value + &right_value;
            if value.is_zero() {
                let right_negative = right.negative() ^ (kind == Kind::Fsub);
                if !left.value.is_zero() || !right.value.is_zero() || left.negative() != right_negative {
                    return None; // cancellation's zero sign depends on rounding
                }
                Finite::new(value, left.negative())
            } else {
                Finite::new(value, false)
            }
        }
        (Kind::Fmul | Kind::Fdiv, [left, right]) => {
            if kind == Kind::Fdiv && right.value.is_zero() {
                return None;
            }
            let value = if kind == Kind::Fmul {
                &left.value * &right.value
            } else {
                &left.value / &right.value
            };
            let negative_zero = value.is_zero() && left.negative() != right.negative();
            Finite::new(value, negative_zero)
        }
        _ => return None,
    };
    if let Some(width) = _get(&_INTEGER, rule.result) {
        let limit = Fraction::from_integer(BigInt::from(1) << (width - 1));
        return (result.value.denominator == BigInt::from(1) && -&limit <= result.value && result.value < limit)
            .then_some(result);
    }
    if let Some(width) = _get(&_UNSIGNED, rule.result) {
        return (result.value.denominator == BigInt::from(1)
            && Fraction::from_integer(0) <= result.value
            && result.value < Fraction::from_integer(BigInt::from(1) << width))
            .then_some(result);
    }
    if let Some((precision, _, bias)) = _get(&_BINARY, rule.result) {
        return _fits(&result.value, u64::from(precision), 1 - bias, bias).then_some(result);
    }
    if rule.result == Format::Extended80 {
        let precision = if rule.precision == Precision::Dynamic { 24 } else { 64 };
        return _fits(&result.value, precision, -16382, 16383).then_some(result);
    }
    None
}

pub(crate) fn encoded(value: &Finite, format: Format) -> Option<BigInt> {
    let (precision, exponent_bits, bias) = _get(&_BINARY, format)?;
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

fn _inputs(
    op: &Op,
    integers: &IndexMap<Value, Known>,
    memory: &Cells,
    facts: &IndexMap<Value, Finite>,
) -> Option<Vec<Finite>> {
    let floating = op.floating.as_ref()?;
    if op.args.len() != floating.inputs.len() {
        return None;
    }
    let mut inputs = Vec::new();
    for (arg, format) in op.args.iter().zip(floating.inputs.iter()) {
        let fact = match arg {
            Arg::Held(held) if held.width == 10 => facts.get(&held.value).cloned(),
            _ => consts::_operand(op, arg, integers, Some(memory)).and_then(|bits| decoded(&bits.n, *format)),
        };
        inputs.push(fact?);
    }
    Some(inputs)
}

/// An explicit FP exception check with no additional value or memory effect.
pub(crate) fn checkpoint(op: &Op) -> bool {
    op.kind == Kind::Fcheck
        && !op.barrier()
        && op.floating.is_none()
        && op.defines.is_empty()
        && op.uses.is_empty()
        && op.loads.is_empty()
        && op.stores.is_empty()
        && op.merges.is_empty()
        && op.stack.is_none()
}

/// Exact memory facts after a caller-proven repetition of a straight-line body.
pub(crate) fn repeated(
    ops: &[Op],
    count: &BigInt,
    initial: &Cells,
    dgroup: &BTreeSet<i64>,
    known: Option<&IndexMap<Value, Known>>,
    mut queries: Option<&mut consts::_MemoryQueries>,
) -> Option<Cells> {
    if *count < BigInt::from(0) || count * BigInt::from(ops.len()) > BigInt::from(100_000) {
        return None;
    }
    let internal = ops.iter().flat_map(|op| op.defines.iter().copied()).collect::<BTreeSet<_>>();
    let invariant = known
        .into_iter()
        .flatten()
        .filter(|(value, _)| !internal.contains(*value))
        .map(|(value, fact)| (*value, fact.clone()))
        .collect::<IndexMap<_, _>>();
    let mut memory = initial.clone();
    let allowed = [Kind::Nothing, Kind::Copy, Kind::Add, Kind::Sub, Kind::Increment, Kind::Decrement];
    let no_calls = IndexMap::default();
    let count = u64::try_from(count).expect("a repetition count Python could iterate");
    for _ in 0..count {
        let mut integers = invariant.clone();
        let mut floating = IndexMap::<Value, Finite>::default();
        for op in ops {
            if checkpoint(op) {
                continue; // exact operations add no pending exception; the check is retained by specialization
            }
            // A narrow integer update can merge the old value's preserved
            // upper half into its machine result; the new fact keeps the
            // operation's declared width, so a later wide consumer stays unknown.
            if op.barrier() {
                return None;
            }
            let Some(rule) = &op.floating else {
                if !allowed.contains(&op.kind) || !op.loads.is_empty() || !op.stores.is_empty() || op.stack.is_some() {
                    return None;
                }
                if let Some(result) = consts::_result(op, &integers, Some(&memory), None) {
                    for value in &op.defines {
                        if !value.flags {
                            integers.insert(*value, result.clone());
                        }
                    }
                }
                continue;
            };
            let inputs = _inputs(op, &integers, &memory, &floating);
            let result = evaluated(op.kind, rule, &inputs?)?;
            if op.kind == Kind::Fstore {
                if op.stores.len() != 1 {
                    return None;
                }
                let reference = mir::symbolic_ref(&op.stores[0]);
                let bits = encoded(&result, rule.result);
                let Some(bits) = bits.filter(|_| {
                    reference.addr.is_some() && reference.base.is_none() && reference.segment.is_none()
                }) else {
                    return None;
                };
                let mut store = op.clone();
                store.kind = Kind::Store;
                store.args = vec![Arg::Const(Const::new(bits, reference.width))];
                store.stores = vec![reference];
                store.uses = Vec::new();
                memory = consts::_kills(
                    &memory,
                    &store,
                    &integers,
                    dgroup,
                    &no_calls,
                    None,
                    None,
                    false,
                    queries.as_deref_mut(),
                );
            } else {
                let Some(Arg::Held(target)) = op.results.first().filter(|_| op.stores.is_empty() && op.results.len() == 1)
                else {
                    return None;
                };
                floating.insert(target.value, result);
            }
        }
    }
    Some(memory)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LoopExit {
    pub header: i64,
    pub count: BigInt,
    pub stores: Vec<(MemRef, Known)>,
}

/// Proven numeric exits of canonical loops with storage-rounded FP state.
pub(crate) fn loop_exits(body: &MirBody, dgroup: &BTreeSet<i64>, calls: &IndexMap<i64, String>) -> Vec<LoopExit> {
    if !body.blocks.iter().any(|block| block.ops.iter().any(|op| op.kind == Kind::Fstore)) {
        return Vec::new();
    }
    let integers = consts::known(body, Some(dgroup), Some(calls), None, None);
    let memory = cells(body, dgroup, calls);
    let blocks = body.blocks.iter().map(|block| (block.at, block)).collect::<IndexMap<_, _>>();
    let predecessors = loops::predecessors(&body.blocks);
    let mut exits = Vec::new();
    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        if loop_.body.len() != 2 || loop_.latches.len() != 1 {
            continue;
        }
        let header = blocks[&loop_.header];
        let latch = blocks[loop_.latches.first().expect("one latch")];
        let outside = predecessors
            .get(&header.at)
            .into_iter()
            .flatten()
            .filter(|at| !loop_.body.contains(at))
            .copied()
            .collect::<Vec<_>>();
        if outside.len() != 1 || !latch.phis.is_empty() {
            continue;
        }
        let entry = blocks[&outside[0]];
        if entry.succ != [header.at] || entry.ops.is_empty() {
            continue;
        }
        let references = latch
            .ops
            .iter()
            .flat_map(|op| op.loads.iter().chain(op.stores.iter()))
            .collect::<Vec<_>>();
        let mut stored = Vec::<&MemRef>::new();
        for reference in latch.ops.iter().filter(|op| op.kind == Kind::Fstore).flat_map(|op| op.stores.iter()) {
            if !stored.contains(&reference) {
                stored.push(reference);
            }
        }
        // `mir.overlapping(written, read, dgroup)` is regions' `layout=None`;
        // an endpoint Rust cannot represent may overlap.
        if stored.is_empty()
            || header.ops.iter().any(|op| {
                op.barrier()
                    || !matches!(op.kind, Kind::Nothing | Kind::Copy | Kind::Store | Kind::Sub | Kind::Branch)
                    || op.floating.is_some()
                    || op.stack.is_some()
                    || op.stores.iter().any(|written| {
                        references
                            .iter()
                            .any(|read| regions::overlapping(written, read, None, None, None).unwrap_or(true))
                    })
            })
        {
            continue;
        }
        let mut counts = BTreeSet::new();
        for counter in induction::basics(body, &loop_).values() {
            let width = counter.start.width();
            let last = induction::_last_counter(body, &loop_, counter, &integers, width);
            let start = induction::_signed(&counter.start.as_arg(), &integers, width);
            let step = induction::_signed(&counter.step.as_arg(), &integers, width);
            if let (Some(last), Some(start), Some(step)) = (last, start, step) {
                if step != BigInt::from(0) {
                    counts.insert(induction::floor_div(&(last - start), &step) + 1);
                }
            }
        }
        if counts.len() != 1 {
            continue;
        }
        let count = counts.pop_first().expect("one count");
        let mut asked = consts::memory_queries(body, &integers, dgroup);
        let initial = consts::_kills(
            &memory[&(entry.at, entry.ops.len() - 1)],
            entry.ops.last().expect("entry ops"),
            &integers,
            dgroup,
            calls,
            None,
            None,
            false,
            Some(&mut asked),
        );
        let Some(last) = repeated(&latch.ops, &count, &initial, dgroup, Some(&integers), Some(&mut asked)) else {
            continue;
        };
        let facts = stored
            .iter()
            .map(|reference| consts::_cell(&last, reference).map(|fact| ((*reference).clone(), fact)))
            .collect::<Option<Vec<_>>>();
        if let Some(facts) = facts {
            exits.push(LoopExit {
                header: header.at,
                count,
                stores: facts,
            });
        }
    }
    exits
}

/// Numeric memory facts on exit edges, never on a header's backedge.
pub(crate) fn exit_cells(
    body: &MirBody,
    dgroup: &BTreeSet<i64>,
    calls: &IndexMap<i64, String>,
) -> IndexMap<(i64, i64), Cells> {
    let proofs = loop_exits(body, dgroup, calls);
    if proofs.is_empty() {
        return IndexMap::default();
    }
    let regions = loops::loops(&body.blocks, Some(body.entry))
        .into_iter()
        .map(|loop_| (loop_.header, loop_.body))
        .collect::<IndexMap<_, _>>();
    let blocks = body.blocks.iter().map(|block| (block.at, block)).collect::<IndexMap<_, _>>();
    let mut edges = IndexMap::default();
    for proof in proofs {
        let leaving = blocks[&proof.header]
            .succ
            .iter()
            .copied()
            .filter(|at| !regions[&proof.header].contains(at))
            .collect::<Vec<_>>();
        let [destination] = leaving[..] else {
            panic!("expected 1 exit from {}, got {}", proof.header, leaving.len());
        };
        let mut memory = Cells::default();
        for (reference, fact) in &proof.stores {
            for (where_, fact) in consts::_fragments(&mir::symbolic_ref(reference), fact) {
                memory.insert(where_, fact);
            }
        }
        edges.insert((proof.header, destination), memory);
    }
    edges
}

/// Numeric facts, optionally given independently established entry bytes.
pub(crate) fn known(
    body: &MirBody,
    dgroup: &BTreeSet<i64>,
    calls: &IndexMap<i64, String>,
    initial: Option<&IndexMap<Addr, BigInt>>,
) -> IndexMap<Value, Finite> {
    _analyzed(body, dgroup, calls, initial).0
}

/// Exact integer conversion results, without permission to remove FP effects.
pub(crate) fn converted(
    body: &MirBody,
    dgroup: &BTreeSet<i64>,
    calls: &IndexMap<i64, String>,
    facts: Option<&IndexMap<Value, Finite>>,
) -> IndexMap<Value, Known> {
    if !body.blocks.iter().any(|block| {
        block
            .ops
            .iter()
            .any(|op| op.kind == Kind::Fstore && !op.results.is_empty() && op.stores.is_empty())
    }) {
        return IndexMap::default();
    }
    let computed;
    let facts = match facts {
        Some(facts) => facts,
        None => {
            computed = known(body, dgroup, calls, None);
            &computed
        }
    };
    let mut results = IndexMap::default();
    for block in &body.blocks {
        for op in &block.ops {
            let Some(rule) = &op.floating else {
                continue;
            };
            let Some(width) = _get(&_INTEGER, rule.result) else {
                continue;
            };
            if op.kind != Kind::Fstore || !op.stores.is_empty() || op.args.len() != 1 || op.results.len() != 1 {
                continue;
            }
            let (Arg::Held(source), Arg::Held(target)) = (&op.args[0], &op.results[0]) else {
                continue;
            };
            let Some(fact) = facts.get(&source.value) else {
                continue;
            };
            if target.width * 8 != width {
                continue;
            }
            if let Some(value) = evaluated(op.kind, rule, std::slice::from_ref(fact)) {
                results.insert(
                    target.value,
                    Known::new(consts::masked(&value.value.int(), target.width), target.width),
                );
            }
        }
    }
    results
}

/// Memory facts including exact floating storage conversions.
pub(crate) fn cells(
    body: &MirBody,
    dgroup: &BTreeSet<i64>,
    calls: &IndexMap<i64, String>,
) -> IndexMap<(i64, usize), Cells> {
    _analyzed(body, dgroup, calls, None).1
}

fn _analyzed(
    body: &MirBody,
    dgroup: &BTreeSet<i64>,
    calls: &IndexMap<i64, String>,
    initial: Option<&IndexMap<Addr, BigInt>>,
) -> (IndexMap<Value, Finite>, IndexMap<(i64, usize), Cells>) {
    let seed = initial.map(|initial| {
        initial
            .iter()
            .map(|(addr, byte)| ((*addr, 1), Known::new(byte.clone(), 1)))
            .collect::<Cells>()
    });
    let integers = consts::known(body, Some(dgroup), Some(calls), None, seed.as_ref());
    let mut facts = IndexMap::<Value, Finite>::default();
    let mut memory = IndexMap::default();
    let mut changed = true;
    while changed {
        changed = false;
        let stored = |op: &Op| -> Op {
            let (Some(rule), [Arg::Held(source)]) = (&op.floating, op.args.as_slice()) else {
                return op.clone();
            };
            if op.kind != Kind::Fstore {
                return op.clone();
            }
            let Some(fact) = facts.get(&source.value) else {
                return op.clone();
            };
            let result = evaluated(op.kind, rule, std::slice::from_ref(fact));
            let bits = result.and_then(|result| encoded(&result, rule.result));
            let Some(bits) = bits.filter(|_| op.stores.len() == 1) else {
                return op.clone();
            };
            let mut store = op.clone();
            store.kind = Kind::Store;
            store.args = vec![Arg::Const(Const::new(bits, op.stores[0].width))];
            store.uses = Vec::new();
            store
        };
        let mut shadow = body.clone();
        for block in &mut shadow.blocks {
            block.ops = block.ops.iter().map(stored).collect();
        }
        memory = consts::cells(&shadow, dgroup, calls, Some(&integers), seed.as_ref(), None, None, None);
        let empty = Cells::default();
        for block in &body.blocks {
            for (index, op) in block.ops.iter().enumerate() {
                let Some(inputs) = _inputs(op, &integers, memory.get(&(block.at, index)).unwrap_or(&empty), &facts)
                else {
                    continue;
                };
                let Some(result) = evaluated(op.kind, op.floating.as_ref().expect("inputs need a rule"), &inputs)
                else {
                    continue;
                };
                for target in &op.results {
                    if let Arg::Held(target) = target {
                        if target.width == 10 && !facts.contains_key(&target.value) {
                            facts.insert(target.value, result.clone());
                            changed = true;
                        }
                    }
                }
            }
        }
    }
    (facts, memory)
}

#[cfg(test)]
#[path = "floatfacts_tests.rs"]
mod tests;
