//! Non-wrapping integer intervals, scoped to the branch edges and counted
//! loops that bound them: llrm-core's `analysis/ranges.rs`, a port of
//! `qbopt/analysis/ranges.py`, adapted to the rich MIR. A width is in bits.
//!
//! A comparison is an `icmp` feeding a conditional `br`, where the old MIR
//! read a flags value and a branch's test; an increment is an `add`. Old
//! `Copy` has no counterpart.
//!
//! `exact_offsets` is of an access into its own object's address, the
//! only root whose offset is known to start at 0; the old far origin has
//! no counterpart.
//!
//! Skipped, BC object corpora:
//! `test_fpdeep_one_based_index_has_a_bounded_byte_offset`,
//! `test_addrm_long_array_value_keeps_counter_bounds`,
//! `test_rngarm_writes_its_counter_only_after_the_loop`.
//! Skipped, `test_non_comparison_flags_do_not_establish_a_bound`: a branch
//! reads an `i1`, which only an `icmp` makes a comparison of; a flags value
//! another operation set has no counterpart.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use llrm_mir::context::ConstantKind;
use llrm_mir::context::signed;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{BlockId, InstId, MetadataOperand, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{Attribute, BinaryOp, CastOp, IntPredicate, Opcode};
use llrm_support::hash::{HashMap, IndexMap};
use num_bigint::BigInt;

use crate::cfg;
use crate::consts::{self, Known};
use crate::graph::loops;
use crate::induction;
use crate::memory::{MemRef, Unit};

/// A non-wrapping mathematical interval at a fixed width.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Interval {
    pub low: BigInt,
    pub high: BigInt,
    pub width: u32,
}

/// A non-wrapping indexed access into its own object as the static byte
/// interval it can touch.
pub fn covering<'a>(
    reference: &'a MemRef,
    known: &BTreeMap<ValueId, Interval>,
) -> Cow<'a, MemRef> {
    let Some(base) = reference.base else {
        return Cow::Borrowed(reference);
    };
    if !reference.object || reference.segment.is_some() {
        return Cow::Borrowed(reference);
    }
    let Some(interval) = known.get(&base) else {
        return Cow::Borrowed(reference);
    };
    if interval.width != reference.base_width || reference.base_width != reference.index_bits {
        return Cow::Borrowed(reference);
    }

    let (first, last) = (
        BigInt::from(reference.disp) + &interval.low * reference.scale,
        BigInt::from(reference.disp) + &interval.high * reference.scale,
    );
    let (low, high) = if first <= last { (first, last) } else { (last, first) };
    let end = high + BigInt::from(reference.width);
    let limit = BigInt::from(1_u8) << reference.index_bits;
    if low < BigInt::from(0_u8) || low >= end || end > limit {
        return Cow::Borrowed(reference);
    }

    // Refuse rather than truncate a bound past the host's integers.
    let width = &end - &low;
    let (Ok(disp), Ok(width)) = (i64::try_from(&low), u32::try_from(&width)) else {
        return Cow::Borrowed(reference);
    };
    Cow::Owned(MemRef { disp, base: None, scale: 0, base_width: 0, width, ..reference.clone() })
}

/// Whether every byte `reference` may touch lies inside the object its
/// root names, its index within `known`: dereferenceable, as LLVM's
/// `isDereferenceablePointer` says of a constant offset, here of a bounded
/// one.
pub fn inside_object(
    unit: &Unit,
    reference: &MemRef,
    known: &BTreeMap<ValueId, Interval>,
) -> bool {
    let covered = covering(reference, known);
    let extent = covered.root.and_then(|root| crate::memory::object_of(unit, root)).and_then(|object| object.extent);
    covered.base.is_none()
        && covered.object
        && covered.segment.is_none()
        && extent.is_some_and(|extent| 0 <= covered.disp && covered.disp + i64::from(covered.width) <= extent)
}

/// The block a successor edge leaves and the conditional branch there:
/// its condition, and the block it goes to when that holds.
fn branch(
    unit: &Unit,
    block: BlockId,
) -> Option<(Operand, BlockId)> {
    let terminator = unit.function.instruction(unit.function.terminator(block)?);
    match (&terminator.opcode, terminator.operands.as_slice()) {
        (Opcode::Br, [condition, Operand::Block(taken), Operand::Block(_)]) => Some((*condition, *taken)),
        _ => None,
    }
}

/// Signed comparison facts on one CFG edge; `None` means that edge is
/// impossible.
pub fn on_edge(
    unit: &Unit,
    block: BlockId,
    successor: BlockId,
    known: &IndexMap<ValueId, Interval>,
    facts: Option<&IndexMap<ValueId, Known>>,
) -> Result<Option<IndexMap<ValueId, Interval>>, String> {
    Ok(edge_delta(unit, block, successor, known, facts)?.map(|delta| applied(known, delta)))
}

/// `known` with `delta`'s intervals in place of its own.
fn applied(
    known: &IndexMap<ValueId, Interval>,
    delta: IndexMap<ValueId, Interval>,
) -> IndexMap<ValueId, Interval> {
    let mut result = known.clone();
    result.extend(delta);
    result
}

/// What `on_edge` changes of `known`, and only that: `None` where the edge is
/// impossible. A caller that holds `known` narrows it in place, without a copy
/// of every value it knows per edge.
fn edge_delta(
    unit: &Unit,
    block: BlockId,
    successor: BlockId,
    known: &IndexMap<ValueId, Interval>,
    facts: Option<&IndexMap<ValueId, Known>>,
) -> Result<Option<IndexMap<ValueId, Interval>>, String> {
    #[cfg(test)]
    EDGE_DELTAS.with(|count| count.set(count.get() + 1));
    let successors = unit.function.successors(block);
    if !successors.contains(&successor) {
        return Err("not a successor".to_owned());
    }
    let empty = IndexMap::default();
    let facts = facts.unwrap_or(&empty);
    if successors.len() != 2 {
        return Ok(Some(IndexMap::default()));
    }
    let Some((condition, taken)) = branch(unit, block) else {
        return Ok(Some(IndexMap::default()));
    };
    Ok(narrowed_delta(unit, condition, successor == taken, known, facts))
}

/// What `known` becomes where `condition`, an `icmp`, is `holds`; `None`
/// where it cannot be.
fn narrowed(
    unit: &Unit,
    condition: Operand,
    holds: bool,
    known: &IndexMap<ValueId, Interval>,
    facts: &IndexMap<ValueId, Known>,
) -> Option<IndexMap<ValueId, Interval>> {
    narrowed_delta(unit, condition, holds, known, facts).map(|delta| applied(known, delta))
}

/// What `narrowed` changes of `known`: the intervals it sets, none where it
/// sets none; `None` where the condition cannot hold. Everything it reads is of
/// `known` as it was.
fn narrowed_delta(
    unit: &Unit,
    condition: Operand,
    holds: bool,
    known: &IndexMap<ValueId, Interval>,
    facts: &IndexMap<ValueId, Known>,
) -> Option<IndexMap<ValueId, Interval>> {
    let result = IndexMap::default();
    let Some((_, compare)) = unit.defining(condition) else {
        return Some(result);
    };
    let Opcode::ICmp(predicate) = compare.opcode else {
        return Some(result);
    };
    let (mut left, mut right) = (compare.operands[0], compare.operands[1]);
    let (Some(left_width), Some(right_width)) = (unit.int_bits(left), unit.int_bits(right)) else {
        return Some(result);
    };
    if right_width != left_width {
        return Some(result);
    }
    let mut kind = predicate;
    if !holds {
        kind = kind.inverse();
    }
    let sign = BigInt::from(1_u8) << (left_width - 1);
    let full = Interval { low: -&sign, high: &sign - 1, width: left_width };
    let mut first = _operand(unit, left, known, facts).unwrap_or_else(|| full.clone());
    let mut second = _operand(unit, right, known, facts).unwrap_or(full);
    if matches!(
        kind,
        IntPredicate::Ugt | IntPredicate::Uge | IntPredicate::Ult | IntPredicate::Ule
    ) {
        let (first_low, first_high) = _unsigned_span(&first);
        let (second_low, second_high) = _unsigned_span(&second);
        let possible = match kind {
            IntPredicate::Ugt => first_high > second_low,
            IntPredicate::Uge => first_high >= second_low,
            IntPredicate::Ult => first_low < second_high,
            _ => first_low <= second_high,
        };
        return possible.then_some(result);
    }
    if matches!(kind, IntPredicate::Sge | IntPredicate::Sgt) {
        (left, right) = (right, left);
        (first, second) = (second, first);
        kind = kind.swapped();
    }
    let spans = match kind {
        IntPredicate::Sle | IntPredicate::Slt => {
            let strict = BigInt::from(u8::from(kind == IntPredicate::Slt));
            [
                (first.low.clone(), first.high.clone().min(&second.high - &strict)),
                (second.low.clone().max(&first.low + &strict), second.high.clone()),
            ]
        }
        IntPredicate::Eq => {
            let shared = (first.low.clone().max(second.low.clone()), first.high.clone().min(second.high.clone()));
            [shared.clone(), shared]
        }
        IntPredicate::Ne => {
            let excluding = |interval: &Interval, other: &Interval| {
                let (mut low, mut high) = (interval.low.clone(), interval.high.clone());
                if other.low == other.high {
                    if low == other.low {
                        low += 1;
                    }
                    if high == other.low {
                        high -= 1;
                    }
                }
                (low, high)
            };
            [excluding(&first, &second), excluding(&second, &first)]
        }
        _ => return Some(result),
    };
    let mut result = result;
    for (operand, (low, high)) in [left, right].into_iter().zip(spans) {
        if low > high {
            return None;
        }
        if let Operand::Value(value) = operand {
            let interval = Interval { low, high, width: left_width };
            result.insert(value, interval.clone());
            _refine_through(unit, value, &interval, known, facts, &mut result, 3)?;
        }
    }
    Some(result)
}

/// What `value` lying in `interval` says of the operands of the instruction
/// that made it, put in `result`; `None` where they cannot be. Only an
/// instruction that, from `known`, computes an interval does so without
/// wrapping, so only then do its operands' bounds follow from the result's.
fn _refine_through(
    unit: &Unit,
    value: ValueId,
    interval: &Interval,
    known: &IndexMap<ValueId, Interval>,
    facts: &IndexMap<ValueId, Known>,
    result: &mut IndexMap<ValueId, Interval>,
    depth: usize,
) -> Option<()> {
    let Some((inst, op)) = unit.defining(Operand::Value(value)) else { return Some(()) };
    if depth == 0 || _computed(unit, inst, known, facts).is_none() {
        return Some(());
    }
    let width = interval.width;
    let operand = |one: Operand| _operand(unit, one, known, facts).filter(|found| found.width == width);
    let mut bounds: Vec<(Operand, BigInt, BigInt)> = Vec::new();
    match (&op.opcode, unit.intrinsic(inst)) {
        (Opcode::Binary(BinaryOp::Add), _) => {
            let (Some(a), Some(b)) = (operand(op.operands[0]), operand(op.operands[1])) else { return Some(()) };
            bounds.push((op.operands[0], &interval.low - &b.high, &interval.high - &b.low));
            bounds.push((op.operands[1], &interval.low - &a.high, &interval.high - &a.low));
        }
        (Opcode::Binary(BinaryOp::Sub), _) => {
            let (Some(a), Some(b)) = (operand(op.operands[0]), operand(op.operands[1])) else { return Some(()) };
            bounds.push((op.operands[0], &interval.low + &b.low, &interval.high + &b.high));
            bounds.push((op.operands[1], &a.low - &interval.high, &a.high - &interval.low));
        }
        (_, Some(Intrinsic::Fixed { divide: false })) if op.operands[0] == op.operands[1] => {
            let Some(scale) = unit.int_constant(op.operands[2]).and_then(|scale| usize::try_from(scale).ok()) else {
                return Some(());
            };
            if interval.high < BigInt::from(0_u8) {
                return None;
            }
            // The least square past `high`, floored by the scale, is `high + 1`
            // scaled.
            let limit: BigInt = ((&interval.high + 1) << scale) - 1;
            let root = limit.sqrt();
            bounds.push((op.operands[0], -root.clone(), root));
        }
        _ => {}
    }
    for (one, low, high) in bounds {
        let Operand::Value(inner) = one else { continue };
        let Some(current) = operand(one) else { continue };
        let (low, high) = (current.low.clone().max(low), current.high.clone().min(high));
        if low > high {
            return None;
        }
        let narrower = Interval { low, high, width };
        if narrower != current {
            result.insert(inner, narrower.clone());
            _refine_through(unit, inner, &narrower, known, facts, result, depth - 1)?;
        }
    }
    Some(())
}

fn _unsigned_span(interval: &Interval) -> (BigInt, BigInt) {
    let mask = (BigInt::from(1_u8) << interval.width) - 1;
    let zero = BigInt::from(0_u8);
    if interval.low < zero && zero <= interval.high {
        return (zero, mask);
    }
    (&interval.low & &mask, &interval.high & &mask)
}

/// An integer operand's interval: a constant's, or what `known` or `facts`
/// say of a value.
pub fn _operand(
    unit: &Unit,
    operand: Operand,
    known: &IndexMap<ValueId, Interval>,
    facts: &IndexMap<ValueId, Known>,
) -> Option<Interval> {
    let width = unit.int_bits(operand)?;
    if let Some(bits) = unit.int_constant(operand) {
        let number = BigInt::from(signed(bits, width));
        return Some(Interval { low: number.clone(), high: number, width });
    }
    let Operand::Value(value) = operand else {
        return None;
    };
    if let Some(interval) = known.get(&value).filter(|interval| interval.width == width) {
        return Some(interval.clone());
    }
    if let Some(interval) = declared_argument(unit, value).filter(|interval| interval.width == width) {
        return Some(interval);
    }
    let fact = facts.get(&value).filter(|fact| fact.width >= width)?;
    Some(singleton(&fact.n, width))
}

/// What the program states of `value` alone: a parameter's `range`, or the
/// `!range` or `range` of the load or call that makes it.
pub fn declared(
    unit: &Unit,
    value: ValueId,
) -> Option<Interval> {
    match unit.function.value(value).def {
        ValueDef::Argument(_) => declared_argument(unit, value),
        ValueDef::Instruction(inst) => _computed(unit, inst, &IndexMap::default(), &IndexMap::default()).filter(|_| {
            let op = unit.function.instruction(inst);
            matches!(
                op.opcode,
                Opcode::Load { .. } | Opcode::Call(_) | Opcode::Invoke(_)
            )
        }),
    }
}

/// The interval a `range` attribute states, if it is one that does not wrap
/// past the signed bounds: `[lower, upper)` at the type's width.
fn stated(
    unit: &Unit,
    attribute: &Attribute,
) -> Option<Interval> {
    let Attribute::Range { ty, lower, upper } = attribute else { return None };
    let width = unit.context.types.int_bits(*ty)?;
    let (low, high) = (singleton(&BigInt::from(*lower), width), singleton(&(BigInt::from(*upper) - 1), width));
    (lower != upper && low.low <= high.low).then_some(Interval { low: low.low, high: high.low, width })
}

/// What `!range` on `inst` says of its result: `[lo, hi)` at the result's
/// width, as LLVM's `computeConstantRange` reads it of a load or a call, the
/// way a frontend states what a value may hold of an instruction.
fn declared_metadata(
    unit: &Unit,
    inst: InstId,
) -> Option<Interval> {
    let op = unit.function.instruction(inst);
    let (_, node) = op.metadata.iter().find(|(kind, _)| kind == "range")?;
    let [MetadataOperand::Constant(lower), MetadataOperand::Constant(upper)] =
        unit.metadata.get(node.0 as usize)?.operands[..]
    else {
        return None;
    };
    let ty = op.result.map(|result| unit.function.value(result).ty)?;
    let bits = |one| match unit.context.get(one).kind {
        ConstantKind::Int(bits) => Some(bits),
        ConstantKind::Zero => Some(0),
        _ => None,
    };
    stated(unit, &Attribute::Range { ty, lower: bits(lower)?, upper: bits(upper)? })
}

/// Every parameter's stated range.
fn declared_arguments(unit: &Unit) -> IndexMap<ValueId, Interval> {
    unit.function.parameters().iter().filter_map(|&value| Some((value, declared_argument(unit, value)?))).collect()
}

/// What a `range` on parameter `value` says of it.
fn declared_argument(
    unit: &Unit,
    value: ValueId,
) -> Option<Interval> {
    let ValueDef::Argument(index) = unit.function.value(value).def else { return None };
    unit.function.parameter_attrs.get(index as usize)?.iter().find_map(|one| stated(unit, one))
}

/// What a `range` on call `inst`'s result says of it, at the call or on the
/// callee: the contract of a routine whose result is bounded, `LEN`'s
/// 0 to 32767.
fn declared_result(
    unit: &Unit,
    inst: InstId,
) -> Option<Interval> {
    let op = unit.function.instruction(inst);
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &op.opcode else { return None };
    let callee = llrm_mir::memory::callee(unit.context, unit.function, inst)
        .and_then(|global| unit.globals.get(global.0 as usize))
        .and_then(|global| global.function());
    let attributes = info.return_attrs.iter().chain(callee.iter().flat_map(|function| function.return_attrs.iter()));
    let mut stated = attributes.filter_map(|one| stated(unit, one));
    let first = stated.next()?;
    stated.try_fold(first, |one, other| {
        (one.width == other.width)
            .then(|| Interval { low: one.low.max(other.low), high: one.high.min(other.high), width: one.width })
            .filter(|met| met.low <= met.high)
    })
}

/// `n`'s low `width` bits, signed, as an interval of one.
fn singleton(
    n: &BigInt,
    width: u32,
) -> Interval {
    let number = consts::masked(n, width);
    let sign = BigInt::from(1_u8) << (width - 1);
    let number = (number ^ &sign) - sign;
    Interval { low: number.clone(), high: number, width }
}

/// Whether `_computed` can answer for `inst` whatever is known: it declares a
/// range, or is one of the operations that compute one. The rest answer None
/// whatever their operands, and a loop's sweeps need not ask them again.
fn computes(
    unit: &Unit,
    inst: InstId,
) -> bool {
    let op = unit.function.instruction(inst);
    let Some(result) = op.result else { return false };
    let Some(width) = unit.int_bits(Operand::Value(result)) else { return false };
    let declared = [declared_result(unit, inst), declared_metadata(unit, inst)]
        .into_iter()
        .flatten()
        .filter(|interval| interval.width == width);
    declared
        .reduce(|one, other| Interval { low: one.low.max(other.low), high: one.high.min(other.high), width })
        .is_some_and(|interval| interval.low <= interval.high)
        || matches!(unit.intrinsic(inst), Some(Intrinsic::Fixed { divide: false }))
        || matches!(
            op.opcode,
            Opcode::Cast(CastOp::SExt | CastOp::ZExt)
                | Opcode::Binary(BinaryOp::Shl | BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::And)
        )
}

/// The interval `inst` computes from what is known of its operands.
pub fn _computed(
    unit: &Unit,
    inst: InstId,
    known: &IndexMap<ValueId, Interval>,
    facts: &IndexMap<ValueId, Known>,
) -> Option<Interval> {
    let op = unit.function.instruction(inst);
    let result = op.result?;
    let width = unit.int_bits(Operand::Value(result))?;
    let declared = [declared_result(unit, inst), declared_metadata(unit, inst)]
        .into_iter()
        .flatten()
        .filter(|interval| interval.width == width);
    if let Some(interval) = declared
        .reduce(|one, other| Interval { low: one.low.max(other.low), high: one.high.min(other.high), width })
        .filter(|interval| interval.low <= interval.high)
    {
        return Some(interval);
    }
    if let Some(Intrinsic::Fixed { divide: false }) = unit.intrinsic(inst) {
        return _fixed_product(unit, inst, width, known, facts);
    }
    // Every other operation answers None below, whatever its operands.
    let kind = match op.opcode {
        Opcode::Cast(CastOp::SExt | CastOp::ZExt) => None,
        Opcode::Binary(kind @ (BinaryOp::Shl | BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::And)) => {
            Some(kind)
        }
        _ => return None,
    };
    let operand = |one: Operand| _operand(unit, one, known, facts);
    if kind == Some(BinaryOp::And) {
        // A non-negative mask bounds the result whatever the other operand
        // holds.
        let high = op
            .operands
            .iter()
            .filter_map(|&one| operand(one))
            .filter(|mask| mask.width == width && mask.low >= BigInt::from(0_u8))
            .map(|mask| mask.high)
            .min()?;
        return Some(Interval { low: BigInt::from(0_u8), high, width });
    }
    let args = op.operands.iter().map(|&one| operand(one)).collect::<Option<Vec<_>>>()?;
    let first = args.first()?;
    let fits = |low: &BigInt, high: &BigInt, width: u32| {
        let sign = BigInt::from(1_u8) << (width - 1);
        -&sign <= *low && low <= high && *high < sign
    };
    if kind.is_none() {
        // A zero extension keeps the numbers only of a non-negative interval.
        let zero = matches!(op.opcode, Opcode::Cast(CastOp::ZExt));
        return (args.len() == 1
            && first.width < width
            && fits(&first.low, &first.high, first.width)
            && (!zero || first.low >= BigInt::from(0_u8)))
        .then(|| Interval { low: first.low.clone(), high: first.high.clone(), width });
    }
    if first.width != width || args.len() != 2 {
        return None;
    }
    let second = &args[1];
    let (low, high);
    if kind == Some(BinaryOp::Shl) {
        if second.low != second.high || second.low < BigInt::from(0_u8) || second.low >= BigInt::from(width) {
            return None;
        }
        let shift = usize::try_from(&second.low).expect("a checked shift is below the operation width");
        (low, high) = (&first.low << shift, &first.high << shift);
    } else if second.width == width {
        match kind {
            Some(BinaryOp::Add) => (low, high) = (&first.low + &second.low, &first.high + &second.high),
            Some(BinaryOp::Sub) => (low, high) = (&first.low - &second.high, &first.high - &second.low),
            _ => (low, high) = product(first, second, op.operands[0] == op.operands[1]),
        }
    } else {
        return None;
    }
    if fits(&low, &high, width) {
        return Some(Interval { low, high, width });
    }
    // A sum, product or shift that does not wrap (`nsw`) of values never
    // negative is never negative, and stays below the signed maximum
    // whatever its operands' corners say: poison otherwise. Where it may be
    // negative the corners decide, as before.
    if op.flags.contains(llrm_mir::opcode::Flags::NSW) && low >= BigInt::from(0_u8) {
        return Some(Interval { low, high: (BigInt::from(1_u8) << (width - 1)) - 1u8, width });
    }
    None
}

/// The values `a * b` takes, before any wrap. `same` is that both are one
/// value, a square: never negative, though its corners' products are.
pub fn product(
    a: &Interval,
    b: &Interval,
    same: bool,
) -> (BigInt, BigInt) {
    if same {
        let zero = BigInt::from(0_u8);
        let (low, high) = (&a.low * &a.low, &a.high * &a.high);
        let spans_zero = a.low <= zero && zero <= a.high;
        return (if spans_zero { zero } else { low.clone().min(high.clone()) }, low.max(high));
    }
    let corners = [&a.low, &a.high]
        .into_iter()
        .flat_map(|left| [&b.low, &b.high].into_iter().map(move |right| left * right))
        .collect::<Vec<_>>();
    (corners.iter().min().expect("four products").clone(), corners.iter().max().expect("four products").clone())
}

/// `llvm.smul.fix` of two integers: the wide product floored by the scale,
/// where it fits the width; one that wraps when stored has no interval.
fn _fixed_product(
    unit: &Unit,
    inst: InstId,
    width: u32,
    known: &IndexMap<ValueId, Interval>,
    facts: &IndexMap<ValueId, Known>,
) -> Option<Interval> {
    let operands = &unit.function.instruction(inst).operands;
    let scale =
        usize::try_from(unit.int_constant(operands[2])?).ok().filter(|&scale| (0..width as usize).contains(&scale))?;
    let operand = |one: Operand| _operand(unit, one, known, facts).filter(|interval| interval.width == width);
    let (a, b) = (operand(operands[0])?, operand(operands[1])?);
    let (low, high) = product(&a, &b, operands[0] == operands[1]);
    // `>>` on a BigInt floors, as the arithmetic shift of the wide product
    // does.
    let (low, high) = (low >> scale, high >> scale);
    let sign = BigInt::from(1_u8) << (width - 1);
    (-&sign <= low && high < sign).then_some(Interval { low, high, width })
}

/// Taken values and the final latch update must all fit without wrapping.
pub fn _recurrence_span(
    start: &BigInt,
    step: &BigInt,
    advances: &BigInt,
    width: u32,
) -> Option<Interval> {
    let last = start + advances * step;
    let sign = BigInt::from(1_u8) << (width - 1);
    let after = &last + step;
    let lowest = start.min(&last).min(&after);
    let highest = start.max(&last).max(&after);
    if *advances >= BigInt::from(0_u8) && -&sign <= *lowest && lowest <= highest && *highest < sign {
        return Some(Interval { low: start.min(&last).clone(), high: start.max(&last).clone(), width });
    }
    None
}

/// Facts established by unavoidable branch edges at each block, and what
/// its own operations compute from them.
///
/// An edge counts only when its destination has that one predecessor: a
/// join is a second way around the check. So a block starts from its sole
/// predecessor's facts narrowed by that edge, or else from its immediate
/// dominator's, and each edge is applied once.
pub fn dominated_edges(unit: &Unit) -> Result<IndexMap<i64, IndexMap<ValueId, Interval>>, String> {
    dominated_edges_with(unit, &unit.registers())
}

/// `dominated_edges`, given what `consts::known` finds without memory.
pub fn dominated_edges_with(
    unit: &Unit,
    facts: &IndexMap<ValueId, Known>,
) -> Result<IndexMap<i64, IndexMap<ValueId, Interval>>, String> {
    Ok(edges_solved(unit, facts, None)?.blocks(unit.function))
}

/// What is known of each value at a point: an interval of it.
pub type Intervals = llrm_support::hash::SparseIdMap<ValueId, Interval>;

type Scope = Rc<Intervals>;

/// `dominated_edges`' state at each block, as the solve left it: what the
/// block's operations leave (`own`) and what holds below it once its assumes
/// are taken (`below`), which is what its successors start from. A block that
/// did not change shares its maps with the solve before.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeStates {
    order: Vec<i64>,
    own: BTreeMap<i64, Scope>,
    below: BTreeMap<i64, Scope>,
}

impl EdgeStates {
    /// What holds on entering `at`, shared; none where nothing is known.
    pub fn scope_of(
        &self,
        at: i64,
    ) -> Option<&Scope> {
        self.own.get(&at).filter(|scoped| !scoped.is_empty())
    }

    /// `blocks`, the maps shared.
    pub fn shared(
        &self,
        function: &llrm_mir::module::Function,
    ) -> IndexMap<i64, Scope> {
        function
            .layout()
            .iter()
            .filter_map(|&block| {
                self.own
                    .get(&cfg::id(block))
                    .filter(|scoped| !scoped.is_empty())
                    .map(|scoped| (cfg::id(block), Rc::clone(scoped)))
            })
            .collect()
    }

    /// The blocks with something known, in layout order.
    pub fn blocks(
        &self,
        function: &llrm_mir::module::Function,
    ) -> IndexMap<i64, IndexMap<ValueId, Interval>> {
        function
            .layout()
            .iter()
            .filter_map(|&block| {
                self.own
                    .get(&cfg::id(block))
                    .filter(|scoped| !scoped.is_empty())
                    .map(|scoped| (cfg::id(block), (**scoped).clone()))
            })
            .collect()
    }
}

thread_local! {
    static SOLVED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many blocks this thread has worked the edges' facts of, for a test that
/// a change reworks the blocks it reaches.
pub fn blocks_solved() -> usize {
    SOLVED.with(std::cell::Cell::get)
}

/// `dominated_edges_with`'s solve, over `unit`'s function: all of it, or, given
/// the states of the function before and the blocks a change reached
/// (`reached`), only the blocks that change can alter. A block is worked again
/// when it was reached, when it follows a reached block along the one edge
/// whose branch condition it reads, or when the state it starts from changed;
/// where its own comes out as it was, what follows it is not asked.
pub fn edges_solved(
    unit: &Unit,
    facts: &IndexMap<ValueId, Known>,
    before: Option<(&EdgeStates, &BTreeSet<BlockId>)>,
) -> Result<EdgeStates, String> {
    let function = unit.function;
    let Some(entry) = function.entry() else {
        return Ok(EdgeStates { order: Vec::new(), own: BTreeMap::new(), below: BTreeMap::new() });
    };
    let graph = cfg::graph(function);
    let predecessors = loops::predecessors(&graph);
    let immediate = unit.shape().dominance.immediate_dominators(function);
    let order = loops::reverse_postorder(&graph, cfg::id(entry));
    // The blocks as they were: none to reuse where the order of blocks is not
    // the same.
    let before = before.filter(|(states, _)| states.order == order);
    let mut known: BTreeMap<i64, Scope> = BTreeMap::new();
    let mut own: BTreeMap<i64, Scope> = BTreeMap::new();
    let mut moved: BTreeSet<i64> = BTreeSet::new();
    let assumed = unit.assumptions();
    for &at in &order {
        let block = cfg::block(at);
        let sole = predecessors.get(&at).filter(|parents| parents.len() == 1).and_then(|parents| parents.first());
        let from_parent =
            sole.is_some_and(|parent| known.contains_key(parent) && edges(unit, cfg::block(*parent), block) == 1);
        let source = if from_parent { sole.copied() } else { immediate.get(&at).copied().flatten() };
        if let Some((states, reached)) = before {
            let stale = reached.contains(&block)
                || (from_parent && sole.is_some_and(|parent| reached.contains(&cfg::block(*parent))))
                || source.is_some_and(|up| moved.contains(&up));
            if !stale && let (Some(kept_own), Some(kept_below)) = (states.own.get(&at), states.below.get(&at)) {
                own.insert(at, Rc::clone(kept_own));
                known.insert(at, Rc::clone(kept_below));
                continue;
            }
        }
        SOLVED.with(|count| count.set(count.get() + 1));
        let seeded = if at == cfg::id(entry) { declared_arguments(unit) } else { IndexMap::default() };
        let mut scoped = match sole.and_then(|parent| Some((*parent, known.get(parent)?))) {
            Some((parent, inherited)) if edges(unit, cfg::block(parent), block) == 1 => {
                on_edge(unit, cfg::block(parent), block, inherited, Some(facts))?
                    .unwrap_or_else(|| (**inherited).clone())
            }
            _ => immediate
                .get(&at)
                .copied()
                .flatten()
                .and_then(|up| known.get(&up))
                .map(|up| (**up).clone())
                .unwrap_or(seeded),
        };
        for &inst in function.block(block).instructions() {
            let (Some(interval), Some(result)) =
                (_computed(unit, inst, &scoped, facts), function.instruction(inst).result)
            else {
                continue;
            };
            let interval = match scoped.get(&result) {
                Some(previous) if previous.width == interval.width => Interval {
                    low: previous.low.clone().max(interval.low),
                    high: previous.high.clone().min(interval.high),
                    width: interval.width,
                },
                _ => interval,
            };
            if interval.low <= interval.high {
                scoped.insert(result, interval);
            }
        }
        // What the block assumes holds below it, not in it: the code before the
        // assume is not covered.
        let scoped = Rc::new(scoped);
        let mut below = Rc::clone(&scoped);
        for &condition in assumed.here(at) {
            if let Some(narrower) = narrowed(unit, condition, true, &below, facts) {
                below = Rc::new(narrower);
            }
        }
        if before.is_some_and(|(states, _)| states.below.get(&at) != Some(&below)) {
            moved.insert(at);
        }
        own.insert(at, scoped);
        known.insert(at, below);
    }
    Ok(EdgeStates { order, own, below: known })
}

/// How many of `parent`'s terminator's targets are `block`.
fn edges(
    unit: &Unit,
    parent: BlockId,
    block: BlockId,
) -> usize {
    unit.function.terminator(parent).map_or(0, |one| {
        unit.function.instruction(one).operands.iter().filter(|operand| **operand == Operand::Block(block)).count()
    })
}

pub type Facts = IndexMap<i64, IndexMap<ValueId, Interval>>;

/// Each block's intervals from the counted loops holding it: its counters,
/// what the loop computes from them, narrowed by the branch edges that
/// dominate the block. A block in no counted loop keeps the facts of the
/// branch edges that dominate it.
pub fn bounded(unit: &Unit) -> Result<Facts, String> {
    bounded_with(unit, &unit.registers())
}

/// `bounded`'s facts at each block without copying them: the manager's where
/// the unit carries its `Bounded` (and works under the registers it was asked
/// of), else worked out here. A caller that asks of the same body again and
/// again asks the manager, whose result is kept and brought up to date, not a
/// solve of its own.
pub fn bounds<'u>(unit: &Unit<'u>) -> Result<std::borrow::Cow<'u, Bounds>, String> {
    let registers = unit.registers();
    match (unit.bounds, unit.registers) {
        (Some(held), Some(carried)) if std::ptr::eq(&*registers, carried) => {
            if unit.loop_intervals && llrm_support::env_set("LLRM_CHECK_REPLAY") {
                assert!(
                    held.facts() == bounded_with(&Unit { bounds: None, ..*unit }, &registers)?,
                    "the bounds a unit carries are not those of the body it stands over: stale"
                );
            }
            Ok(std::borrow::Cow::Borrowed(held))
        }
        _ => Ok(std::borrow::Cow::Owned(bounded_solved(unit, &registers, None)?)),
    }
}

/// `bounded`, given what `consts::known` finds without memory.
pub fn bounded_with(
    unit: &Unit,
    facts: &IndexMap<ValueId, Known>,
) -> Result<Facts, String> {
    Ok(bounded_solved(unit, facts, None)?.facts())
}

thread_local! {
    static KNOWNS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// A name for a `known` a loop's blocks are worked from, so that what is kept
/// of it is not compared with it block by block.
fn fresh_known() -> u64 {
    KNOWNS.with(|count| {
        count.set(count.get() + 1);
        count.get()
    })
}

/// The edges inside a loop, one after another, each node the state `known` is
/// narrowed to by the path of edges to it.
struct Prefix {
    state: Rc<Intervals>,
    next: HashMap<(usize, usize, i64), usize>,
    /// The node of the prefix one edge shorter, and the values this edge
    /// changed of its state.
    parent: Option<usize>,
    delta: Vec<ValueId>,
    /// `state` swept: found from the parent's, and the values this edge
    /// changed.
    settled: Option<Rc<Intervals>>,
}

/// The states after every prefix of the edge chains met so far, for the `known`
/// they were worked from.
#[derive(Default)]
struct Prefixes {
    /// Which `known` they were worked from (`fresh_known`).
    known: u64,
    roots: HashMap<Vec<(usize, usize, i64)>, usize>,
    nodes: Vec<Prefix>,
}

/// `bounded`'s facts at each block, as the solve left them: those the counted
/// loops give (`within`), and those with the edges' facts where a block is in
/// none (`blocks`). A block that did not change shares its map with the solve
/// before.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bounds {
    headers: Vec<i64>,
    /// The edges' facts it was worked out under: a loop reads them where it
    /// starts from.
    edges: IndexMap<i64, Scope>,
    within: IndexMap<i64, Scope>,
    blocks: IndexMap<i64, Scope>,
    /// The blocks two loops' facts contradict each other at: none executes
    /// them.
    dead: BTreeSet<i64>,
}

impl Bounds {
    /// Whether the facts of the loops holding `at` contradict each other there:
    /// the block is unreachable, and a call in it passes nothing to its
    /// callee (`parameter_ranges` rounds, which assume a range before they
    /// prove it).
    pub fn unreachable(
        &self,
        at: i64,
    ) -> bool {
        self.dead.contains(&at)
    }

    /// What is known at `at`.
    pub fn at(
        &self,
        at: i64,
    ) -> Option<&IndexMap<ValueId, Interval>> {
        self.blocks.get(&at).map(|scope| &**scope)
    }

    /// As `bounded` gives them.
    pub fn facts(&self) -> Facts {
        self.blocks.iter().map(|(at, scope)| (*at, (**scope).clone())).collect()
    }
}

/// The headers of the loops whose bounds a change can alter: the loops holding
/// a block it reached, the loops under a branch (or an assume, or any call)
/// whose condition it reached, the loops that start from a block whose edges'
/// facts changed (those `bounds` was worked under against `held`, now), and
/// then every loop that starts from what one of those leaves: the block its
/// header's dominator is in, or a block that enters it. (A loop around a dirty
/// one is dirty by the first two: they name its blocks, and its header
/// dominates theirs.)
pub fn loops_reached(
    function: &llrm_mir::module::Function,
    shape: &cfg::Shape,
    blocks: &BTreeSet<BlockId>,
    conditions: &BTreeSet<BlockId>,
    bounds: &Bounds,
    held: &EdgeStates,
) -> BTreeSet<i64> {
    let loops = &shape.loops;
    let reached: BTreeSet<i64> = blocks.iter().map(|block| cfg::id(*block)).collect();
    let judged: Vec<i64> = conditions.iter().map(|block| cfg::id(*block)).collect();
    let graph = cfg::graph(function);
    let entering = loops::predecessors(&graph);
    let now = held.shared(function);
    // Where each loop starts from: the block its header's dominator is, and the
    // blocks that enter it.
    let starts = |one: &loops::Loop| -> Vec<i64> {
        shape
            .dominance
            .immediate(one.header)
            .into_iter()
            .chain(entering.get(&one.header).into_iter().flatten().copied().filter(|at| !one.body.contains(at)))
            .collect()
    };
    let moved = |at: &i64| match (bounds.edges.get(at), now.get(at)) {
        (Some(was), Some(is)) => !Rc::ptr_eq(was, is) && was != is,
        (None, None) => false,
        _ => true,
    };
    let mut dirty: BTreeSet<i64> = loops
        .iter()
        .filter(|one| {
            one.body.iter().any(|at| reached.contains(at))
                || judged.iter().any(|above| shape.dominance.dominates(*above, one.header))
                || starts(one).iter().any(moved)
        })
        .map(|one| one.header)
        .collect();
    // What each loop starts from, held by other loops.
    let hosts: Vec<(i64, Vec<i64>)> = loops
        .iter()
        .map(|one| {
            let held: BTreeSet<i64> = starts(one)
                .into_iter()
                .flat_map(|at| loops.iter().filter(move |other| other.body.contains(&at)).map(|other| other.header))
                .filter(|header| *header != one.header)
                .collect();
            (one.header, held.into_iter().collect())
        })
        .collect();
    loop {
        let before = dirty.len();
        for one in loops {
            let hosted =
                hosts.iter().any(|(header, held)| *header == one.header && held.iter().any(|at| dirty.contains(at)));
            if hosted {
                dirty.insert(one.header);
            }
        }
        if dirty.len() == before {
            return dirty;
        }
    }
}

thread_local! {
    static LOOPS_SOLVED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many loops this thread has worked the bounds of, for a test that a
/// change reworks the loops it reaches.
pub fn loops_solved() -> usize {
    LOOPS_SOLVED.with(std::cell::Cell::get)
}

/// `bounded_with`'s solve: all of it, or, given the bounds of the function
/// before and the headers of the loops a change can alter (`dirty`), only those
/// loops; the blocks of the rest keep what they had.
pub fn bounded_solved(
    unit: &Unit,
    facts: &IndexMap<ValueId, Known>,
    prior: Option<(&Bounds, &BTreeSet<i64>)>,
) -> Result<Bounds, String> {
    let function = unit.function;
    let graph = cfg::graph(function);
    let predecessors = loops::predecessors(&graph);
    let positions: BTreeMap<i64, usize> = graph.iter().enumerate().map(|(at, block)| (block.at, at)).collect();
    let shape = unit.shape();
    let assumed = unit.assumptions();
    // The manager's, where the facts asked of are the unit's own registers.
    let edges_above = match (unit.edges, unit.registers) {
        (Some(states), Some(registers)) if std::ptr::eq(facts, registers) => {
            let held = states.shared(function);
            if llrm_support::env_set("LLRM_CHECK_REPLAY") {
                assert!(
                    held.iter().map(|(at, scope)| (*at, (**scope).clone())).collect::<IndexMap<_, _>>()
                        == dominated_edges_with(unit, facts)?,
                    "the edges a unit carries are not those of the body it stands over: stale"
                );
            }
            held
        }
        _ => dominated_edges_with(unit, facts)?.into_iter().map(|(at, scope)| (at, Rc::new(scope))).collect(),
    };
    let headers: Vec<i64> = shape.loops.iter().map(|one| one.header).collect();
    // An enclosing loop's facts are in `result` before an inner loop reads
    // them.
    let mut nest: Vec<_> = shape.loops.iter().collect();
    nest.sort_by_key(|loop_| std::cmp::Reverse(loop_.body.len()));
    let mut result: IndexMap<i64, Scope> = match prior.filter(|(held, _)| held.headers == headers) {
        // The blocks of a loop to be worked again start from nothing: what its
        // loops narrow is put in anew.
        Some((held, dirty)) => {
            let redone: BTreeSet<i64> = nest
                .iter()
                .filter(|one| dirty.contains(&one.header))
                .flat_map(|one| one.body.iter().copied())
                .collect();
            held.within
                .iter()
                .filter(|(at, _)| !redone.contains(at))
                .map(|(at, scope)| (*at, Rc::clone(scope)))
                .collect()
        }
        None => IndexMap::default(),
    };
    let prior = prior.filter(|(held, _)| held.headers == headers);
    let mut dead: BTreeSet<i64> = match prior {
        Some((held, dirty)) => {
            let redone: BTreeSet<i64> = nest
                .iter()
                .filter(|one| dirty.contains(&one.header))
                .flat_map(|one| one.body.iter().copied())
                .collect();
            held.dead.difference(&redone).copied().collect()
        }
        None => BTreeSet::new(),
    };
    for loop_ in nest {
        if prior.is_some_and(|(_, dirty)| !dirty.contains(&loop_.header)) {
            continue;
        }
        LOOPS_SOLVED.with(|count| count.set(count.get() + 1));
        let proofs = induction::counted_unless_stopped(unit, &loop_, Some(facts), false);
        // A header that tests before the trip also sees the exit value; one
        // tested after it sees only the trip's.
        let mut inside = loop_.body.clone();
        if proofs.is_empty() || !proofs.iter().all(|proof| proof.posttested) {
            inside.remove(&loop_.header);
        }
        let mut known = declared_arguments(unit);
        let mut trips = BTreeSet::new();
        for proof in &proofs {
            if let (Some((low, high)), Some(count)) = (proof.span(), &proof.count) {
                known.insert(proof.counter.value, Interval { low, high, width: proof.counter.start.width() });
                trips.insert(count - 1);
            }
        }
        if let (1, Some(advances)) = (trips.len(), trips.first()) {
            for counter in induction::basics(unit, &loop_).values() {
                let width = counter.start.width();
                let start = induction::_signed(&counter.start, facts, width);
                let step = induction::_signed(&counter.step, facts, width);
                if let (Some(start), Some(step)) = (start, step)
                    && let Some(interval) = _recurrence_span(&start, &step, advances, width)
                {
                    known.insert(counter.value, interval);
                }
            }
        }
        // A counter that steps by a constant from a known start and never wraps
        // (its update is `nsw`) does not go back past its start,
        // however many trips: SCEV's range of `{start,+,step}<nsw>`, and enough
        // for what asks a counter's sign.
        for counter in induction::basics(unit, &loop_).values() {
            if known.contains_key(&counter.value) {
                continue;
            }
            let width = counter.start.width();
            let (Some(start), Some(step)) =
                (induction::_signed(&counter.start, facts, width), induction::_signed(&counter.step, facts, width))
            else {
                continue;
            };
            let ValueDef::Instruction(phi) = function.value(counter.value).def else { continue };
            let wraps = function.instruction(phi).operands.chunks(2).filter(|pair| matches!(
                pair[1],
                Operand::Block(from) if loop_.body.contains(&cfg::id(from))
            )).any(|pair| {
                !matches!(
                    pair[0],
                    Operand::Value(next) if matches!(function.value(next).def, ValueDef::Instruction(update) if function.instruction(update).flags.contains(llrm_mir::opcode::Flags::NSW))
                )
            });
            let sign = BigInt::from(1_u8) << (width - 1);
            if wraps || step == BigInt::from(0_u8) || start < -sign.clone() || start >= sign {
                continue;
            }
            let interval = if step > BigInt::from(0_u8) {
                Interval { low: start, high: &sign - 1u8, width }
            } else {
                Interval { low: -sign, high: start, width }
            };
            known.insert(counter.value, interval);
        }
        let counted = !known.is_empty();
        // What is known above the loop of the values made outside it.
        let at_entry = |at: i64| {
            let mut found = edges_above.get(&at).map(|scope| (**scope).clone()).unwrap_or_default();
            for (value, interval) in result.get(&at).into_iter().flat_map(|scope| scope.iter()) {
                narrow(&mut found, *value, interval.clone());
            }
            found
        };
        let outside = |value: ValueId| match function.value(value).def {
            ValueDef::Argument(_) => true,
            ValueDef::Instruction(inst) => {
                function.parent(inst).is_some_and(|block| !loop_.body.contains(&cfg::id(block)))
            }
        };
        let header_scope = shape.dominance.immediate(loop_.header).map(at_entry).unwrap_or_default();
        for (value, interval) in header_scope {
            if outside(value) && !known.contains_key(&value) {
                known.insert(value, interval);
            }
        }
        let operations = function
            .layout()
            .iter()
            .filter(|&&block| inside.contains(&cfg::id(block)) || cfg::id(block) == loop_.header)
            .flat_map(|&block| function.block(block).instructions().iter().copied())
            .filter(|&inst| function.instruction(inst).opcode != Opcode::Phi)
            .collect::<Vec<_>>();
        if check_scopes() {
            let known = known.clone();
            assert!(
                operations
                    .iter()
                    .filter(|&&inst| !computes(unit, inst))
                    .all(|&inst| _computed(unit, inst, &known, facts).is_none()),
                "an operation that does not compute answered"
            );
        }
        let operations: Vec<InstId> = operations.into_iter().filter(|&inst| computes(unit, inst)).collect();
        // What each operation computes, from what `known` holds.
        let closed = |mut known: IndexMap<ValueId, Interval>| {
            loop {
                let before = known.len();
                for &inst in &operations {
                    if let Some(result) = function.instruction(inst).result.filter(|result| !known.contains_key(result))
                        && let Some(interval) = _computed(unit, inst, &known, facts)
                    {
                        known.insert(result, interval);
                    }
                }
                if known.len() == before {
                    return known;
                }
            }
        };
        // Each operation of the loop in `scoped`'s terms, once: it narrows its
        // result by what its operands give, until none changes.
        let apply = |scoped: &mut Intervals, inst: InstId| -> Option<(ValueId, Option<Interval>)> {
            #[cfg(test)]
            OPS_APPLIED.with(|count| count.set(count.get() + 1));
            let mut interval = _computed(unit, inst, scoped, facts)?;
            let result = function.instruction(inst).result.expect("_computed answers a result");
            if let Some(previous) = scoped.get(&result).filter(|previous| previous.width == interval.width) {
                let (low, high) = (previous.low.clone().max(interval.low), previous.high.clone().min(interval.high));
                if low > high {
                    return None;
                }
                interval = Interval { low, high, width: interval.width };
            }
            Some((result, scoped.insert(result, interval)))
        };
        // Every operation, swept in order until a sweep changes nothing.
        let sweep = |mut scoped: Intervals| {
            loop {
                // What each value set in this sweep held before it.
                let mut before = IndexMap::<ValueId, Option<Interval>>::default();
                for &inst in &operations {
                    if let Some((result, previous)) = apply(&mut scoped, inst) {
                        before.entry(result).or_insert(previous);
                    }
                }
                if before.iter().all(|(value, was)| scoped.get(value) == was.as_ref()) {
                    return scoped;
                }
            }
        };
        // Which operations read or define each value, by place in `operations`.
        let touching: std::cell::OnceCell<llrm_mir::dense::IdMap<ValueId, Vec<usize>>> = std::cell::OnceCell::new();
        let touching_of = || {
            touching.get_or_init(|| {
                let mut touching = llrm_mir::dense::IdMap::<ValueId, Vec<usize>>::new();
                for (place, &inst) in operations.iter().enumerate() {
                    let op = function.instruction(inst);
                    for value in op
                        .operands
                        .iter()
                        .filter_map(|one| if let Operand::Value(value) = one { Some(*value) } else { None })
                        .chain(op.result)
                    {
                        let places = touching.get_or_insert_with(value, Vec::new);
                        if places.last() != Some(&place) {
                            places.push(place);
                        }
                    }
                }
                touching
            })
        };
        // The operations `changed` values reach, and what those reach in turn,
        // worked again in `scoped`.
        let propagate = |scoped: &mut Intervals, changed: &mut dyn Iterator<Item = ValueId>| {
            let touching = touching_of();
            let mut queue: BTreeSet<usize> = changed
                .flat_map(|value| touching.get(&value).into_iter().flatten().copied().collect::<Vec<_>>())
                .collect();
            while let Some(place) = queue.pop_first() {
                if let Some((result, previous)) = apply(scoped, operations[place])
                    && previous.as_ref() != scoped.get(&result)
                {
                    queue.extend(touching.get(&result).into_iter().flatten().copied());
                }
            }
        };
        // `known` swept: what holds of the loop before any block's edges narrow
        // it. Every block asks of the same one.
        let swept: RefCell<Option<(u64, Rc<Intervals>)>> = RefCell::new(None);
        // `scoped` (`known` narrowed by a block's edges) swept, found from
        // `known` swept: a sweep leaves an operation whose operands and
        // result are as they were in `known` as it found it there, so only
        // those the narrowing reaches, and what they reach in turn, are
        // worked again from `known swept` with the narrowed values put
        // over it.
        let settle = |mut scoped: Intervals, known: &Intervals, id: u64| -> Intervals {
            // The cheaper of the two by the work each counts: a sweep evaluates
            // every operation and then once more to see nothing
            // change (2 x operations); settling finds what the block's edges
            // narrowed and puts the swept facts over the rest (one
            // pass over `scoped`'s facts), then evaluates only what that
            // reaches.
            if 2 * operations.len() <= scoped.len() {
                return sweep(scoped);
            }
            let base = {
                let held = swept.borrow().as_ref().filter(|(was, _)| *was == id).map(|(_, base)| Rc::clone(base));
                held.unwrap_or_else(|| {
                    let base = Rc::new(sweep(known.clone()));
                    *swept.borrow_mut() = Some((id, Rc::clone(&base)));
                    base
                })
            };
            let narrowed: llrm_mir::dense::IdSet<ValueId> = scoped
                .iter()
                .filter(|(value, interval)| known.get(*value) != Some(*interval))
                .map(|(value, _)| *value)
                .collect();
            for (value, interval) in base.iter() {
                if !narrowed.contains(value) {
                    scoped.insert(*value, interval.clone());
                }
            }
            propagate(&mut scoped, &mut narrowed.iter());
            scoped
        };
        let prefixes: RefCell<Prefixes> = RefCell::new(Prefixes::default());
        // How many blocks of the loop each block dominates (itself too): the
        // blocks a prefix ending at it is shared by.
        let led = std::cell::OnceCell::<HashMap<i64, usize>>::new();
        let led_to = |block: i64| -> usize {
            led.get_or_init(|| {
                let mut counts = HashMap::<i64, usize>::default();
                for &member in &loop_.body {
                    let mut above = Some(member);
                    while let Some(one) = above.filter(|one| loop_.body.contains(one)) {
                        *counts.entry(one).or_default() += 1;
                        above = shape.dominance.immediate(one);
                    }
                }
                counts
            })
            .get(&block)
            .copied()
            .unwrap_or(0)
        };
        // Everything the branch edges above `at` and the assumes narrow `known`
        // to there.
        let scope_at = |at: i64, known: &Intervals, id: u64, every_block: bool| -> Result<Rc<Intervals>, String> {
            let mut scoped = known.clone();
            // The edges into a block with no other way in, from a block that
            // dominates `at`: those of the dominator chain of `at`,
            // in the order of the blocks' layout, each narrowing in place.
            let mut chain = Vec::new();
            let mut above = Some(at);
            while let Some(one) = above {
                if let Some(parents) = predecessors.get(&one).filter(|parents| parents.len() == 1)
                    && let Some(&from) = parents.iter().next().and_then(|parent| positions.get(parent))
                {
                    for (index, &successor) in graph[from].succ.iter().enumerate() {
                        if successor == one {
                            chain.push((from, index, successor));
                        }
                    }
                }
                above = shape.dominance.immediate(one);
            }
            chain.sort_unstable();
            // The edges from outside the loop read only values made outside it,
            // which `known` holds as it did the last time: what
            // they narrow is kept, and only the edges inside the loop are
            // worked out again.
            let outer = chain.iter().take_while(|(from, _, _)| !loop_.body.contains(&graph[*from].at)).count();
            let separable = chain[outer..].iter().all(|(from, _, _)| loop_.body.contains(&graph[*from].at));
            let narrow_by =
                |edges: &[(usize, usize, i64)], scoped: &mut IndexMap<ValueId, Interval>| -> Result<(), String> {
                    for &(from, _, successor) in edges {
                        if let Some(delta) =
                            edge_delta(unit, cfg::block(graph[from].at), cfg::block(successor), scoped, Some(&facts))?
                        {
                            scoped.extend(delta);
                        }
                    }
                    Ok(())
                };
            // A prefix of the edges inside the loop is kept where the blocks it
            // leads to are more than the passes over the state it
            // costs to keep: a copy of the parent's, the edge, a diff against
            // it for what the edge changed, and a copy of the
            // parent's swept state to put that over. Fewer, and applying
            // the edges one after another is the cheaper.
            const TRIE_PASSES: usize = 4;
            // Only a `known` every block of the loop is asked of in turn has
            // prefixes that blocks share: the few latches a round
            // of boxes asks of would pay for a trie and use none of it.
            if separable && every_block {
                let mut held = prefixes.borrow_mut();
                if held.known != id {
                    *held = Prefixes { known: id, ..Prefixes::default() };
                }
                let root = match held.roots.get(&chain[..outer]).copied() {
                    Some(root) => root,
                    None => {
                        let mut narrowed = known.clone();
                        narrow_by(&chain[..outer], &mut narrowed)?;
                        held.nodes.push(Prefix {
                            state: Rc::new(narrowed),
                            next: HashMap::default(),
                            parent: None,
                            delta: Vec::new(),
                            settled: None,
                        });
                        let root = held.nodes.len() - 1;
                        held.roots.insert(chain[..outer].to_vec(), root);
                        root
                    }
                };
                let mut node = root;
                let mut kept = 0;
                for edge in &chain[outer..] {
                    node = match held.nodes[node].next.get(edge).copied() {
                        Some(next) => next,
                        None => {
                            if led_to(edge.2) < TRIE_PASSES {
                                break;
                            }
                            let mut state = (*held.nodes[node].state).clone();
                            narrow_by(std::slice::from_ref(edge), &mut state)?;
                            let delta = state
                                .iter()
                                .filter(|(value, interval)| held.nodes[node].state.get(*value) != Some(*interval))
                                .map(|(value, _)| *value)
                                .collect();
                            held.nodes.push(Prefix {
                                state: Rc::new(state),
                                next: HashMap::default(),
                                parent: Some(node),
                                delta,
                                settled: None,
                            });
                            let next = held.nodes.len() - 1;
                            held.nodes[node].next.insert(*edge, next);
                            next
                        }
                    };
                    kept += 1;
                }
                scoped = (*held.nodes[node].state).clone();
                let rest = &chain[outer + kept..];
                narrow_by(rest, &mut scoped)?;
                // A block no assume narrows, all of whose edges are kept, is
                // settled from its parent prefix's, with the
                // one edge's values put over it: sweeping the narrowed state is
                // that state's fixpoint, and the fixpoint of a
                // state between this one and its fixpoint is the same.
                if rest.is_empty()
                    && kept > 0
                    && 2 * operations.len() > scoped.len()
                    && assumed.above(&shape, at).is_empty()
                {
                    let mut path = Vec::new();
                    let mut at_node = Some(node);
                    while let Some(one) = at_node.filter(|one| held.nodes[*one].settled.is_none()) {
                        path.push(one);
                        at_node = held.nodes[one].parent;
                    }
                    for one in path.into_iter().rev() {
                        let settled = match held.nodes[one].parent {
                            None => settle(held.nodes[one].state.as_ref().clone(), known, id),
                            Some(parent) => {
                                let mut y = held.nodes[parent]
                                    .settled
                                    .as_ref()
                                    .expect("the parent is settled first")
                                    .as_ref()
                                    .clone();
                                for value in &held.nodes[one].delta {
                                    narrow(&mut y, *value, held.nodes[one].state[value].clone());
                                }
                                propagate(&mut y, &mut held.nodes[one].delta.iter().copied());
                                y
                            }
                        };
                        held.nodes[one].settled = Some(Rc::new(settled));
                    }
                    let settled = Rc::clone(held.nodes[node].settled.as_ref().expect("settled"));
                    if check_scopes() {
                        let whole = sweep(scoped.clone());
                        if whole != *settled {
                            let diff: Vec<String> = whole
                                .iter()
                                .filter(|(v, i)| settled.get(*v) != Some(*i))
                                .map(|(v, i)| {
                                    format!(
                                        "{:?}: sweep [{},{}] vs incremental {:?}",
                                        v,
                                        i.low,
                                        i.high,
                                        settled.get(v).map(|x| (x.low.to_string(), x.high.to_string()))
                                    )
                                })
                                .chain(
                                    settled
                                        .iter()
                                        .filter(|(v, _)| !whole.contains_key(*v))
                                        .map(|(v, i)| format!("{:?}: only incremental [{},{}]", v, i.low, i.high)),
                                )
                                .collect();
                            panic!(
                                "a block settled from its parent prefix's is not what sweeping its edges' state gives: {} (known {}, state {}, ops {})",
                                diff.join("; "),
                                known.len(),
                                scoped.len(),
                                operations.len()
                            );
                        }
                    }
                    return Ok(settled);
                }
            } else {
                narrow_by(&chain, &mut scoped)?;
            }
            if check_scopes() {
                let mut every = known.clone();
                narrow_by(&chain, &mut every)?;
                assert!(every.iter().eq(scoped.iter()), "the edges above a loop narrow `known` as they did before");
            }
            // What the blocks above it assume.
            for condition in assumed.above(&shape, at) {
                if let Some(delta) = narrowed_delta(unit, condition, true, &scoped, facts) {
                    scoped.extend(delta);
                }
            }
            if check_scopes() {
                let whole = sweep(scoped.clone());
                let quick = settle(scoped, known, id);
                assert!(
                    whole.iter().eq(quick.iter()),
                    "the operations a block's edges reach, worked again alone, give what sweeping them all does"
                );
                return Ok(Rc::new(quick));
            }
            Ok(Rc::new(settle(scoped, known, id)))
        };
        let boxes = inductive_boxes(unit, loop_, facts, &known, &at_entry, &closed, &scope_at)?;
        if !counted && boxes.is_empty() {
            continue;
        }
        known.extend(boxes);
        // The header's values too: seen from inside, they are the trip's,
        // though the header itself also sees the exit value.
        let known = closed(known);
        let known_id = fresh_known();
        for &at in &inside {
            let scoped = scope_at(at, &known, known_id, true)?;
            let destination = Rc::make_mut(result.entry(at).or_default());
            for (value, interval) in scoped.iter() {
                if narrow_to(destination, *value, interval) {
                    dead.insert(at);
                }
            }
        }
    }
    let within = result.clone();
    for (at, known) in &edges_above {
        result.entry(*at).or_insert_with(|| Rc::clone(known));
    }
    Ok(Bounds { headers, edges: edges_above, within, blocks: result, dead })
}

#[cfg(test)]
thread_local! {
    static OPS_APPLIED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has worked an operation of a loop out, for a test
/// that a block does not sweep them all.
#[cfg(test)]
pub(crate) fn operations_applied() -> usize {
    OPS_APPLIED.with(std::cell::Cell::get)
}

#[cfg(test)]
thread_local! {
    static EDGE_DELTAS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many edges this thread has narrowed `known` by, for a test that a loop
/// does not work out again the edges above it.
#[cfg(test)]
pub(crate) fn edge_deltas() -> usize {
    EDGE_DELTAS.with(std::cell::Cell::get)
}

fn check_scopes() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| llrm_support::env_set("LLRM_CHECK_SCOPES"))
}

/// Header phis, made by no counted proof, that every entry and every trip round
/// the loop keeps inside one box `[-2^k, 2^k)`: a value in the box at the
/// header is in it again at each latch, from the facts the edges and assumes in
/// the loop give, and it starts there. The box is grown by powers of two, the
/// phis assumed together, until the latches stay in it; a phi any latch gives
/// no interval for is dropped.
fn inductive_boxes(
    unit: &Unit,
    loop_: &loops::Loop,
    facts: &IndexMap<ValueId, Known>,
    known: &IndexMap<ValueId, Interval>,
    at_entry: &dyn Fn(i64) -> IndexMap<ValueId, Interval>,
    closed: &dyn Fn(IndexMap<ValueId, Interval>) -> IndexMap<ValueId, Interval>,
    scope_at: &dyn Fn(i64, &Intervals, u64, bool) -> Result<Rc<Intervals>, String>,
) -> Result<IndexMap<ValueId, Interval>, String> {
    const PHIS: usize = 8;
    const ROUNDS: usize = 8;
    const SIZE: usize = 512;
    let function = unit.function;
    let header = cfg::block(loop_.header);
    let size: usize = loop_.body.iter().map(|&at| function.block(cfg::block(at)).instructions().len()).sum();
    if size > SIZE {
        return Ok(IndexMap::default());
    }
    // (phi, width, the entries' hull, the (latch, value) pairs coming round)
    let mut candidates = Vec::new();
    for &inst in function.block(header).instructions() {
        let op = function.instruction(inst);
        let (Opcode::Phi, Some(phi)) = (&op.opcode, op.result) else { continue };
        let Some(width) = unit.int_bits(Operand::Value(phi)).filter(|&width| width > 1) else { continue };
        if known.contains_key(&phi) {
            continue;
        }
        let (mut entries, mut latches) = (Vec::new(), Vec::new());
        for pair in op.operands.chunks(2) {
            let &[value, Operand::Block(from)] = pair else { continue };
            if loop_.body.contains(&cfg::id(from)) {
                latches.push((cfg::id(from), value));
            } else {
                entries.push(
                    _operand(unit, value, &at_entry(cfg::id(from)), facts).filter(|interval| interval.width == width),
                );
            }
        }
        let hull = entries
            .into_iter()
            .try_fold(
                None::<Interval>,
                |hull, one| {
                    let one = one?;
                    Some(Some(match hull {
                        None => one,
                        Some(hull) => Interval { low: hull.low.min(one.low), high: hull.high.max(one.high), width },
                    }))
                },
            );
        if let Some(Some(hull)) = hull
            && !latches.is_empty()
        {
            candidates.push((phi, width, hull, latches));
        }
    }
    candidates.truncate(PHIS);
    let mut boxes: IndexMap<ValueId, Interval> =
        candidates.iter().map(|(phi, _, hull, _)| (*phi, hull.clone())).collect();
    for _ in 0..ROUNDS {
        if boxes.is_empty() {
            break;
        }
        let mut assumed = known.clone();
        assumed.extend(boxes.iter().map(|(phi, interval)| (*phi, interval.clone())));
        let assumed = closed(assumed);
        let assumed_id = fresh_known();
        let mut scopes = BTreeMap::new();
        let mut grown = false;
        let mut dropped = Vec::new();
        for (phi, width, _, latches) in &candidates {
            let Some(current) = boxes.get(phi).cloned() else { continue };
            let mut wanted = current.clone();
            let mut known_all = true;
            for (latch, value) in latches {
                if !scopes.contains_key(latch) {
                    scopes.insert(*latch, scope_at(*latch, &assumed, assumed_id, false)?);
                }
                match _operand(unit, *value, &scopes[latch], facts).filter(|interval| interval.width == *width) {
                    Some(interval) => {
                        wanted.low = wanted.low.min(interval.low);
                        wanted.high = wanted.high.max(interval.high);
                    }
                    None => known_all = false,
                }
            }
            if !known_all {
                dropped.push(*phi);
            } else if wanted != current {
                match power_box(&wanted, *width) {
                    Some(wider) => {
                        boxes.insert(*phi, wider);
                        grown = true;
                    }
                    None => dropped.push(*phi),
                }
            }
        }
        for phi in &dropped {
            boxes.swap_remove(phi);
        }
        if !grown && dropped.is_empty() {
            return Ok(boxes);
        }
    }
    Ok(IndexMap::default())
}

/// The least box `[-2^k, 2^k)` holding `interval`, none where that is the whole
/// width.
fn power_box(
    interval: &Interval,
    width: u32,
) -> Option<Interval> {
    (0..width - 1)
        .map(|bits| BigInt::from(1_u8) << bits)
        .find(|limit| -limit <= interval.low && interval.high < *limit)
        .map(|limit| Interval { low: -limit.clone(), high: limit - 1, width })
}

/// `interval` for `value` in `known`, met with what it already held at that
/// width; whether the two have nothing in common, which no execution reaching
/// the block can show (the block is unreachable under the facts that gave
/// them), and `known` keeps the first.
fn narrow(
    known: &mut Intervals,
    value: ValueId,
    interval: Interval,
) -> bool {
    narrow_to(known, value, &interval)
}

/// `narrow`, of an interval kept by its owner: an end is copied only where it
/// is the tighter.
fn narrow_to(
    known: &mut Intervals,
    value: ValueId,
    interval: &Interval,
) -> bool {
    match known.get_mut(&value) {
        None => {
            known.insert(value, interval.clone());
        }
        Some(previous) if previous.width == interval.width => {
            // Compared in place: most asks give what is already known, and a
            // copy of each end of it was most of the cost.
            let low_wins = interval.low > previous.low;
            let high_wins = interval.high < previous.high;
            let (low, high) = (
                if low_wins { &interval.low } else { &previous.low },
                if high_wins { &interval.high } else { &previous.high },
            );
            if low > high {
                return true;
            }
            if low_wins {
                previous.low = interval.low.clone();
            }
            if high_wins {
                previous.high = interval.high.clone();
            }
        }
        Some(_) => {}
    }
    false
}

/// Every interval known at each block: a loop's counters and what they
/// compute, narrowed by the branch edges that dominate it.
pub fn scoped(unit: &Unit) -> Result<Facts, String> {
    let mut result = bounded(unit)?;
    for (at, edges) in dominated_edges(unit)? {
        let known = result.entry(at).or_default();
        for (value, interval) in edges {
            match known.get(&value) {
                Some(previous) if previous.width == interval.width => {
                    narrow(known, value, interval);
                }
                _ => {
                    known.insert(value, interval);
                }
            }
        }
    }
    Ok(result)
}

/// What is known of `operand` on entering block `at`, as `scoped` has it there
/// (the bounds of the counted loops holding the block, narrowed by the edges
/// that dominate it), without the intervals of the other values (which `scoped`
/// copies). Over the manager's bounds and edges where the unit carries them.
pub fn operand_at(
    unit: &Unit,
    operand: Operand,
    at: i64,
) -> Result<Option<Interval>, String> {
    let facts = unit.registers();
    let Operand::Value(value) = operand else { return Ok(_operand(unit, operand, &Intervals::default(), &facts)) };
    let held = bounds(unit)?;
    // The manager's where the unit carries its edges (one block's, not a map of
    // them all), else worked out here.
    let worked;
    let edges: Option<&Intervals> = match unit.edges {
        Some(states) => states.scope_of(at).map(|scope| &**scope),
        None => {
            worked = dominated_edges(unit)?.shift_remove(&at);
            worked.as_ref()
        }
    };
    let mut known = Intervals::default();
    if let Some(interval) = held.at(at).and_then(|scope| scope.get(&value)) {
        known.insert(value, interval.clone());
    }
    if let Some(interval) = edges.and_then(|scope| scope.get(&value)) {
        match known.get(&value) {
            Some(previous) if previous.width == interval.width => {
                narrow(&mut known, value, interval.clone());
            }
            _ => {
                known.insert(value, interval.clone());
            }
        }
    }
    Ok(_operand(unit, operand, &known, &facts))
}

/// The values that index accesses whose offset every wider sum names exactly.
///
/// An index added to its object's own address is summed at the pointer's
/// index width, which wraps. The sum names the same bytes at any wider
/// width when every partial sum of the offset, as the affine operations
/// computing it would be cut anywhere, is a non-negative integer below
/// 2**index_bits: zero extension is then the identity. `inbounds` is the
/// promise that places the start. A value is exact only if every access it
/// indexes is.
pub fn exact_offsets(unit: &Unit) -> Result<BTreeSet<ValueId>, String> {
    exact_offsets_given(unit, &scoped(unit)?)
}

/// `exact_offsets`, given the `scoped` facts of the unit: a caller that needs
/// them too solves once.
pub fn exact_offsets_given(
    unit: &Unit,
    scoped: &Facts,
) -> Result<BTreeSet<ValueId>, String> {
    let mut verdict = IndexMap::<ValueId, bool>::default();
    for (block, inst) in unit.function.walk() {
        let Some(reference) = MemRef::of(unit, inst) else { continue };
        let Some(base) = reference.base else { continue };
        let bits = reference.index_bits;
        // A far pointer made from a selector starts at offset 0 as well.
        let placed = (reference.inbounds && reference.object) || reference.segment.is_some();
        let exact = placed
            && reference.base_width == bits
            && _exact_sum(unit, base, cfg::id(block), scoped, bits, 16).is_some_and(|(low, high)| {
                let (low, high) = (
                    BigInt::from(reference.disp) + low * reference.scale,
                    BigInt::from(reference.disp) + high * reference.scale,
                );
                low >= BigInt::from(0) && high < BigInt::from(1) << bits
            });
        *verdict.entry(base).or_insert(true) &= exact;
    }
    Ok(verdict.into_iter().filter(|(_, exact)| *exact).map(|(value, _)| value).collect())
}

/// The integer range of `value`, where it and every affine partial sum
/// computing it is a non-negative integer below 2**bits; leaves read at `at`.
fn _exact_sum(
    unit: &Unit,
    value: ValueId,
    at: i64,
    scoped: &Facts,
    bits: u32,
    depth: usize,
) -> Option<(BigInt, BigInt)> {
    // Past the depth, a node is unproven, not a leaf.
    if depth == 0 {
        return None;
    }
    let limit = BigInt::from(1) << bits;
    let inside = |(low, high): (BigInt, BigInt)| (low >= BigInt::from(0) && high < limit).then_some((low, high));
    let operand = |one: Operand| -> Option<(BigInt, BigInt)> {
        if unit.int_bits(one) != Some(bits) {
            return None;
        }
        match one {
            Operand::Value(value) => _exact_sum(unit, value, at, scoped, bits, depth - 1),
            _ => unit.int_constant(one).map(|n| (BigInt::from(n), BigInt::from(n))),
        }
    };
    let constant = |one: Operand| unit.int_constant(one).map(BigInt::from);
    let made = unit.defining(Operand::Value(value));
    if let Some((_, op)) = made {
        let affine = match (&op.opcode, op.operands.as_slice()) {
            (Opcode::Binary(BinaryOp::Add), &[left, right]) => {
                Some(operand(left).zip(operand(right)).map(|(l, r)| (l.0 + r.0, l.1 + r.1)))
            }
            (Opcode::Binary(BinaryOp::Sub), &[left, right]) if constant(right).is_some() => {
                Some(operand(left).zip(operand(right)).map(|(l, r)| (l.0 - r.1, l.1 - r.0)))
            }
            (Opcode::Binary(BinaryOp::Mul), &[left, right]) | (Opcode::Binary(BinaryOp::Mul), &[right, left])
                if constant(right).is_some() =>
            {
                Some(operand(left).zip(operand(right)).map(|(l, r)| (l.0 * &r.0, l.1 * r.0)))
            }
            (Opcode::Binary(BinaryOp::Shl), &[left, right]) => {
                let count = constant(right).and_then(|count| u32::try_from(count).ok()).filter(|count| *count < bits);
                Some(operand(left).zip(count).map(|(l, count)| (l.0 << count, l.1 << count)))
            }
            _ => None,
        };
        if let Some(bounds) = affine {
            return bounds.and_then(inside);
        }
    }
    // A leaf: whatever it is, it is read in this block.
    let defined = made.and_then(|(inst, _)| unit.function.parent(inst)).map(cfg::id);
    let fact = scoped
        .get(&at)
        .and_then(|known| known.get(&value))
        .or_else(|| defined.and_then(|block| scoped.get(&block)).and_then(|known| known.get(&value)))?;
    (fact.width == bits).then(|| (fact.low.clone(), fact.high.clone())).and_then(inside)
}

/// Every value `consts` knows without solving memory, as the singleton
/// interval an alias query reads.
pub fn constants(unit: &Unit) -> IndexMap<ValueId, Interval> {
    intervals(&unit.registers())
}

/// Each known value as the interval of it alone.
pub fn intervals(known: &IndexMap<ValueId, Known>) -> IndexMap<ValueId, Interval> {
    known
        .iter()
        .map(|(value, fact)| (*value, Interval { low: fact.n.clone(), high: fact.n.clone(), width: fact.width }))
        .collect()
}

/// Exact values computed without consulting memory.
pub fn singletons(unit: &Unit) -> IndexMap<ValueId, Interval> {
    let function = unit.function;
    let mut known = IndexMap::<ValueId, Interval>::default();
    let none = IndexMap::default();
    loop {
        let before = known.len();
        for (_, inst) in function.walk() {
            let op = function.instruction(inst);
            let Some(result) = op.result.filter(|result| !known.contains_key(result)) else {
                continue;
            };
            if op.opcode == Opcode::Phi {
                let incoming =
                    op.operands.iter().step_by(2).map(|&one| _operand(unit, one, &known, &none)).collect::<Vec<_>>();
                if !incoming.is_empty()
                    && !incoming.contains(&None)
                    && incoming.iter().collect::<llrm_support::hash::HashSet<_>>().len() == 1
                {
                    let interval = incoming[0].clone().expect("known");
                    known.insert(result, interval);
                }
                continue;
            }
            if let Some(interval) =
                _computed(unit, inst, &known, &none).filter(|interval| interval.low == interval.high)
            {
                known.insert(result, interval);
            }
        }
        if known.len() == before {
            return known;
        }
    }
}

#[cfg(test)]
#[path = "ranges_tests.rs"]
pub mod tests;
