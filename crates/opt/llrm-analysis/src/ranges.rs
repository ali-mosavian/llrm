//! Bounds each integer value in each block by a non-wrapping interval, from the
//! branch edges that dominate the block. A bounds check or overlap test that
//! the interval decides can then go.
//!
//! # How it works
//! An `Interval` is `[low, high]` over mathematical integers at a bit width,
//! signed. A value's interval is the intersection of what several sources give.
//!
//! Operands. `_operand` reads a constant's, an entry in the scope's
//! `Intervals`, a parameter's `range` attribute or a `!range` on a load or call
//! (`declared`), or a single known value from [`crate::consts`]. `_computed`
//! gives what an instruction computes from its operands' intervals (`add`,
//! `sub`, `mul`, `shl`, `and`, `sext`, `zext`, fixed-point multiply), and no
//! interval where the result could wrap.
//!
//! Edges. `dominated_edges` walks the dominator tree. A conditional `br` on an
//! `icmp` narrows the compared values on each edge (`on_edge`): signed
//! predicates move bounds, `eq` intersects, `ne` trims an end when the other
//! side is a point, unsigned ones only decide whether the edge is possible, and
//! `assume` calls add facts. `_refine_through` pushes a bound back through the
//! `add` or `sub` that made the value. An edge counts only when its destination
//! has that one predecessor, because a join is another way around the check; a
//! block starts from its sole predecessor's facts narrowed by the edge, or else
//! its immediate dominator's. An edge no execution takes makes the block
//! unreachable. `EdgeStates` keeps each block's state so a change re-solves
//! only the blocks it reaches (`edges_solved`).
//!
//! ```text
//!   b1:     %c = icmp slt %i, 10         b2 (taken):    %i in [lo, 9]
//!           br %c, b2, b3                b3 (not taken): %i in [10, hi]
//! ```
//!
//! Queries. `covering` widens an indexed access into its own object to the byte
//! interval it can touch; `inside_object` says that interval lies inside the
//! object.
//!
//! # Example
//! `if x < 4 { if x >= 0 { y = x * 2 } }`:
//!
//! ```mir
//! define void @f(i16 %x) {
//! b0:
//!   %c = icmp slt i16 %x, 4
//!   br i1 %c, label %b1, label %b2
//!
//! b1:
//!   %lo = icmp sge i16 %x, 0
//!   br i1 %lo, label %b3, label %b2
//!
//! b3:
//!   %y = mul i16 %x, 2
//!   ret void
//!
//! b2:
//!   ret void
//! }
//! ```
//!
//! `dominated_edges` gives `%x` in [0, 3] and `%y` in [0, 6] at `b3`, and
//! nothing at the join `b2`.
//!
//! From unit test
//! `a_guarded_block_bounds_what_it_computes_from_the_guarded_value`
//! in `ranges_tests.rs`.
//!
//! # Provides
//! The interval facts above. In the manager they are `DominatedEdges`
//! (incremental) and `Ranges`, which bundles them with registers for a pass.
//! Consumers: [`crate::alias`], [`crate::regions`], [`crate::guards`],
//! [`crate::memoryssa`], [`crate::avail`], and in llrm-transforms
//! `fixednarrow`.
//!
//! # Needs
//! [`crate::consts`] (`Known`, the registers), [`crate::assumptions`],
//! [`crate::cfg`] (shape and dominance).
//!
//! # See also
//! [`crate::ranger`], the edges' facts of one value at one block on request,
//! checked against `edges_solved` under `LLRM_CHECK_REPLAY`.
//! GCC's ranger (`gimple-range-cache.cc`, `ranger_cache::range_on_entry`);
//! LLVM's LazyValueInfo.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{BuildHasher, Hash, Hasher};
use std::rc::Rc;

use llrm_mir::context::ConstantKind;
use llrm_mir::context::signed;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{BlockId, InstId, MetadataOperand, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{Attribute, BinaryOp, CastOp, IntPredicate, Opcode};
use llrm_support::hash::IndexMap;

use crate::cfg;
use crate::consts::{self, Known};
use crate::graph::loops;
use crate::memory::{MemRef, Unit};
use crate::num::Num as BigInt;

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
    known: &dyn Lookup,
) -> Cow<'a, MemRef> {
    let Some(base) = reference.base else {
        return Cow::Borrowed(reference);
    };
    if !reference.object || reference.segment.is_some() {
        return Cow::Borrowed(reference);
    }
    let Some(interval) = known.lookup(&base) else {
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
    known: &dyn Lookup,
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
    known: &Intervals,
    facts: Option<&IndexMap<ValueId, Known>>,
) -> Result<Option<Intervals>, String> {
    Ok(edge_delta(unit, block, successor, known, facts)?.map(|delta| applied(known, delta)))
}

/// `known` with `delta`'s intervals in place of its own.
fn applied(
    known: &Intervals,
    delta: Intervals,
) -> Intervals {
    let mut result = known.clone();
    result.extend(delta);
    result
}

/// What `on_edge` changes of `known`, and only that: `None` where the edge is
/// impossible. A caller that holds `known` narrows it in place, without a copy
/// of every value it knows per edge.
pub(crate) fn edge_delta(
    unit: &Unit,
    block: BlockId,
    successor: BlockId,
    known: &dyn Lookup,
    facts: Option<&IndexMap<ValueId, Known>>,
) -> Result<Option<Intervals>, String> {
    if !unit.function.successors(block).contains(&successor) {
        return Err("not a successor".to_owned());
    }
    edge_delta_of(unit, block, successor, known, facts)
}

/// `edge_delta` of an edge known to be one: a block's way in is asked of its
/// predecessor, which names it, and a switch of N cases listed its N successors
/// again for each.
pub(crate) fn edge_delta_of(
    unit: &Unit,
    block: BlockId,
    successor: BlockId,
    known: &dyn Lookup,
    facts: Option<&IndexMap<ValueId, Known>>,
) -> Result<Option<Intervals>, String> {
    let empty = IndexMap::default();
    let facts = facts.unwrap_or(&empty);
    // Two distinct targets, or the branch decides nothing.
    let Some((condition, taken)) = branch(unit, block) else {
        return Ok(Some(Intervals::default()));
    };
    let other = match unit.function.instruction(unit.function.terminator(block).expect("a branch")).operands[2] {
        Operand::Block(other) => other,
        _ => unreachable!("branch() found two blocks"),
    };
    if taken == other {
        return Ok(Some(Intervals::default()));
    }
    Ok(narrowed_delta(unit, condition, successor == taken, known, facts))
}

/// What a condition changes of `known`: the intervals it sets, none where it
/// sets none; `None` where the condition cannot hold. Everything it reads is of
/// `known` as it was.
pub(crate) fn narrowed_delta(
    unit: &Unit,
    condition: Operand,
    holds: bool,
    known: &dyn Lookup,
    facts: &IndexMap<ValueId, Known>,
) -> Option<Intervals> {
    let result = Intervals::default();
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
    known: &dyn Lookup,
    facts: &IndexMap<ValueId, Known>,
    result: &mut Intervals,
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
    known: &dyn Lookup,
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
    if let Some(interval) = known.lookup(&value).filter(|interval| interval.width == width) {
        return Some(interval.into_owned());
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
        ValueDef::Instruction(inst) => {
            _computed(unit, inst, &Intervals::default(), &IndexMap::default()).filter(|_| {
                let op = unit.function.instruction(inst);
                matches!(
                    op.opcode,
                    Opcode::Load { .. } | Opcode::Call(_) | Opcode::Invoke(_)
                )
            })
        }
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
fn declared_arguments(unit: &Unit) -> Intervals {
    unit.function.parameters().iter().filter_map(|&value| Some((value, declared_argument(unit, value)?))).collect()
}

/// What a `range` on parameter `value` says of it.
pub(crate) fn declared_argument(
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

/// The interval `inst` computes from what is known of its operands.
pub fn _computed(
    unit: &Unit,
    inst: InstId,
    known: &dyn Lookup,
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
    known: &dyn Lookup,
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
pub fn dominated_edges(unit: &Unit) -> Result<IndexMap<i64, Intervals>, String> {
    dominated_edges_with(unit, &unit.registers())
}

/// `dominated_edges`, given what `consts::known` finds without memory.
pub fn dominated_edges_with(
    unit: &Unit,
    facts: &IndexMap<ValueId, Known>,
) -> Result<IndexMap<i64, Intervals>, String> {
    Ok(edges_solved(unit, facts, None)?.blocks(unit.function))
}

/// What is known of each value at a point: an interval of it.
/// What is known of each value: a sorted run of shared intervals while few are
/// known, a tree of shared nodes (`ShareMap`) past `FEW`. A scope of a few
/// values is copied by a few pointers, which a tree of nodes costs more to
/// write; a block's scope in a function of many blocks is not copied at all.
#[derive(Clone, Debug)]
pub enum Intervals {
    Few(Vec<(ValueId, Rc<Interval>)>),
    Many(llrm_mir::dense::ShareMap<ValueId, Interval>),
}

/// Intervals held in a run before they go to a tree.
const FEW: usize = 64;

impl Default for Intervals {
    fn default() -> Self {
        Self::Few(Vec::new())
    }
}

impl PartialEq for Intervals {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        match (self, other) {
            (Self::Many(one), Self::Many(two)) => one == two,
            _ => self.len() == other.len() && self.iter().zip(other.iter()).all(|(one, two)| one == two),
        }
    }
}

pub enum IntervalIter<'a> {
    Few(std::slice::Iter<'a, (ValueId, Rc<Interval>)>),
    Many(std::vec::IntoIter<(ValueId, &'a Interval)>),
}

impl<'a> Iterator for IntervalIter<'a> {
    type Item = (ValueId, &'a Interval);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Few(run) => run.next().map(|(value, interval)| (*value, &**interval)),
            Self::Many(tree) => tree.next(),
        }
    }
}

impl Intervals {
    pub fn len(&self) -> usize {
        match self {
            Self::Few(run) => run.len(),
            Self::Many(tree) => tree.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn place(
        run: &[(ValueId, Rc<Interval>)],
        value: &ValueId,
    ) -> Result<usize, usize> {
        run.binary_search_by(|(at, _)| llrm_mir::dense::Dense::index(*at).cmp(&llrm_mir::dense::Dense::index(*value)))
    }

    pub fn get(
        &self,
        value: &ValueId,
    ) -> Option<&Interval> {
        match self {
            Self::Few(run) => Self::place(run, value).ok().map(|at| &*run[at].1),
            Self::Many(tree) => tree.get(value),
        }
    }

    pub fn contains_key(
        &self,
        value: &ValueId,
    ) -> bool {
        self.get(value).is_some()
    }

    pub fn get_mut(
        &mut self,
        value: &ValueId,
    ) -> Option<&mut Interval> {
        match self {
            Self::Few(run) => Self::place(run, value).ok().map(|at| Rc::make_mut(&mut run[at].1)),
            Self::Many(tree) => tree.get_mut(value),
        }
    }

    /// The interval `value` had, as it was held.
    pub fn replace(
        &mut self,
        value: ValueId,
        interval: Interval,
    ) -> Option<Rc<Interval>> {
        match self {
            Self::Few(run) => match Self::place(run, &value) {
                Ok(at) => Some(std::mem::replace(&mut run[at].1, Rc::new(interval))),
                Err(at) => {
                    if run.len() < FEW {
                        run.insert(at, (value, Rc::new(interval)));
                    } else {
                        let mut tree: llrm_mir::dense::ShareMap<ValueId, Interval> =
                            run.iter().map(|(value, interval)| (*value, (**interval).clone())).collect();
                        tree.insert(value, interval);
                        *self = Self::Many(tree);
                    }
                    None
                }
            },
            Self::Many(tree) => tree.insert(value, interval).map(Rc::new),
        }
    }

    pub fn insert(
        &mut self,
        value: ValueId,
        interval: Interval,
    ) -> Option<Interval> {
        self.replace(value, interval).map(|old| Rc::try_unwrap(old).unwrap_or_else(|shared| (*shared).clone()))
    }

    pub fn remove(
        &mut self,
        value: &ValueId,
    ) -> Option<Interval> {
        match self {
            Self::Few(run) => Self::place(run, value)
                .ok()
                .map(|at| Rc::try_unwrap(run.remove(at).1).unwrap_or_else(|shared| (*shared).clone())),
            Self::Many(tree) => tree.remove(value),
        }
    }

    /// The pairs, ascending by value.
    pub fn iter(&self) -> IntervalIter<'_> {
        match self {
            Self::Few(run) => IntervalIter::Few(run.iter()),
            Self::Many(tree) => IntervalIter::Many(tree.iter().collect::<Vec<_>>().into_iter()),
        }
    }
}

impl FromIterator<(ValueId, Interval)> for Intervals {
    fn from_iter<I: IntoIterator<Item = (ValueId, Interval)>>(pairs: I) -> Self {
        let mut found = Self::default();
        found.extend(pairs);
        found
    }
}

impl Extend<(ValueId, Interval)> for Intervals {
    fn extend<I: IntoIterator<Item = (ValueId, Interval)>>(
        &mut self,
        pairs: I,
    ) {
        for (value, interval) in pairs {
            self.replace(value, interval);
        }
    }
}

impl<'a> IntoIterator for &'a Intervals {
    type Item = (ValueId, &'a Interval);
    type IntoIter = IntervalIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl IntoIterator for Intervals {
    type Item = (ValueId, Interval);
    type IntoIter = std::vec::IntoIter<(ValueId, Interval)>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter().map(|(value, interval)| (value, interval.clone())).collect::<Vec<_>>().into_iter()
    }
}

impl std::ops::Index<&ValueId> for Intervals {
    type Output = Interval;

    fn index(
        &self,
        value: &ValueId,
    ) -> &Interval {
        self.get(value).expect("an interval known of the value")
    }
}

type Scope = Rc<Intervals>;

/// What is known of a value where a bound is asked: a table's entry, or an
/// answer worked out on request (`ranger`).
pub trait Lookup {
    fn lookup(
        &self,
        value: &ValueId,
    ) -> Option<Cow<'_, Interval>>;
}

impl Lookup for Intervals {
    fn lookup(
        &self,
        value: &ValueId,
    ) -> Option<Cow<'_, Interval>> {
        self.get(value).map(Cow::Borrowed)
    }
}

impl Lookup for BTreeMap<ValueId, Interval> {
    fn lookup(
        &self,
        value: &ValueId,
    ) -> Option<Cow<'_, Interval>> {
        self.get(value).map(Cow::Borrowed)
    }
}

/// `delta`'s facts over those of `base`: what holds past a branch's edge.
pub struct Over<'a> {
    pub delta: Intervals,
    pub base: &'a dyn Lookup,
}

impl Lookup for Over<'_> {
    fn lookup(
        &self,
        value: &ValueId,
    ) -> Option<Cow<'_, Interval>> {
        match self.delta.get(value) {
            Some(found) => Some(Cow::Borrowed(found)),
            None => self.base.lookup(value),
        }
    }
}

/// `on_edge`, without a copy of what is known: the edge's facts over `known`.
pub fn on_edge_over<'a>(
    unit: &Unit,
    block: BlockId,
    successor: BlockId,
    known: &'a dyn Lookup,
    facts: Option<&IndexMap<ValueId, Known>>,
) -> Result<Option<Over<'a>>, String> {
    Ok(edge_delta(unit, block, successor, known, facts)?.map(|delta| Over { delta, base: known }))
}

impl<T: Lookup + ?Sized> Lookup for Rc<T> {
    fn lookup(
        &self,
        value: &ValueId,
    ) -> Option<Cow<'_, Interval>> {
        (**self).lookup(value)
    }
}

/// `dominated_edges`' state at each block, as the solve left it: what the
/// block's operations leave (`own`) and what holds below it once its assumes
/// are taken (`below`), which is what its successors start from. A block that
/// did not change shares its maps with the solve before.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeStates {
    order: Vec<i64>,
    own: IdMap<i64, Scope>,
    below: IdMap<i64, Scope>,
    /// What `below` was made of, block by block: its source's, and what the
    /// block put over it. A block made of the same is the same, so whether a
    /// block's facts changed is asked of these, not of the maps, which are
    /// deep.
    made: IdMap<i64, u64>,
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
    ) -> IndexMap<i64, Intervals> {
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
        return Ok(EdgeStates { order: Vec::new(), own: IdMap::new(), below: IdMap::new(), made: IdMap::new() });
    };
    let shape = unit.shape();
    // A change that left the CFG as it was (`Touched::reach`) leaves the order
    // of the blocks as it was.
    let order = match before {
        Some((states, _)) if !llrm_support::env_flag!("LLRM_CHECK_REPLAY") => states.order.clone(),
        _ => {
            ORDERS_FOUND.with(|count| count.set(count.get() + 1));
            loops::reverse_postorder(&cfg::graph(function), cfg::id(entry))
        }
    };
    // The blocks as they were: none to reuse where the order of blocks is not
    // the same.
    let before = before.filter(|(states, _)| states.order == order);
    let mut known: IdMap<i64, Scope> = IdMap::new();
    let mut own: IdMap<i64, Scope> = IdMap::new();
    let mut moved: IdSet<i64> = IdSet::new();
    let mut made: IdMap<i64, u64> = IdMap::new();
    let assumed = unit.assumptions();
    // A block's way in counts only where its one predecessor names it once.
    let once = |parent: i64, block: i64| !names_twice(unit, parent, block);
    for &at in &order {
        let block = cfg::block(at);
        let parents = function.predecessors(block);
        let sole = (parents.len() == 1).then(|| cfg::id(parents[0]));
        let from_parent = sole.is_some_and(|parent| known.contains_key(&parent) && once(parent, at));
        let source = if from_parent { sole } else { shape.dominance.immediate(unit.function, at) };
        if let Some((states, reached)) = before {
            let stale = reached.contains(&block)
                || (from_parent && sole.is_some_and(|parent| reached.contains(&cfg::block(parent))))
                || source.is_some_and(|up| moved.contains(&up));
            if !stale
                && let (Some(kept_own), Some(kept_below), Some(kept_made)) =
                    (states.own.get(&at), states.below.get(&at), states.made.get(&at))
            {
                own.insert(at, Rc::clone(kept_own));
                known.insert(at, Rc::clone(kept_below));
                made.insert(at, *kept_made);
                continue;
            }
        }
        SOLVED.with(|count| count.set(count.get() + 1));
        let seeded = if at == cfg::id(entry) { declared_arguments(unit) } else { Intervals::default() };
        let mut made_of = llrm_support::hash::FxBuild::default().build_hasher();
        let mut scoped = match sole.and_then(|parent| Some((parent, known.get(&parent)?))) {
            Some((parent, inherited)) if once(parent, at) => {
                made.get(&parent).hash(&mut made_of);
                match edge_delta_of(unit, cfg::block(parent), block, inherited, Some(facts))? {
                    Some(delta) => {
                        delta.iter().for_each(|(value, interval)| (value, interval).hash(&mut made_of));
                        applied(inherited, delta)
                    }
                    None => (**inherited).clone(),
                }
            }
            _ => {
                let up = shape.dominance.immediate(unit.function, at);
                up.and_then(|up| made.get(&up)).hash(&mut made_of);
                up.and_then(|up| known.get(&up)).map(|up| (**up).clone()).unwrap_or(seeded)
            }
        };
        for inst in function.iter_block(block) {
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
                (result, &interval).hash(&mut made_of);
                scoped.insert(result, interval);
            }
        }
        // What the block assumes holds below it, not in it: the code before the
        // assume is not covered.
        let scoped = Rc::new(scoped);
        let mut below = Rc::clone(&scoped);
        for &condition in assumed.here(at) {
            if let Some(delta) = narrowed_delta(unit, condition, true, &below, facts) {
                delta.iter().for_each(|(value, interval)| (value, interval).hash(&mut made_of));
                below = Rc::new(applied(&below, delta));
            }
        }
        let digest = made_of.finish();
        if before.is_some_and(|(states, _)| states.made.get(&at) != Some(&digest)) {
            moved.insert(at);
        }
        made.insert(at, digest);
        own.insert(at, scoped);
        known.insert(at, below);
    }
    Ok(EdgeStates { order, own, below: known, made })
}

/// Whether `parent`'s terminator names `block` more than once: a conditional
/// branch to one block twice, a switch with two cases to one block.
fn names_twice(
    unit: &Unit,
    parent: i64,
    block: i64,
) -> bool {
    let function = unit.function;
    let Some(last) = function.terminator(cfg::block(parent)) else { return false };
    let targets = &function.instruction(last).operands;
    // One or two targets (every `br`) are compared; more are counted.
    if targets.len() <= 3 {
        return matches!(
            targets[..],
            [_, Operand::Block(one), Operand::Block(two)] if one == two && cfg::id(one) == block
        );
    }
    targets.iter().filter(|operand| **operand == Operand::Block(cfg::block(block))).count() > 1
}

thread_local! {
    static ORDERS_FOUND: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has found the order of a body's blocks to solve
/// its edges' facts over, for a test that an update reads the one it had.
pub fn orders_found() -> usize {
    ORDERS_FOUND.with(std::cell::Cell::get)
}

/// What ValueTracking bounds `operand` to at `at`: `computeConstantRange`
/// intersected with `fromKnownBits(Known, IsSigned)`, as a signed interval;
/// none where it is the whole type.
pub fn value_tracking_at(
    unit: &Unit,
    tree: Option<&llrm_mir::domtree::DomTree>,
    assumptions: Option<&llrm_mir::valuetracking::AssumptionCache>,
    at: InstId,
    operand: Operand,
) -> Option<Interval> {
    let q = llrm_mir::valuetracking::Query {
        context: unit.context,
        globals: unit.globals,
        metadata: unit.metadata,
        function: unit.function,
        layout: unit.layout,
        dt: tree,
        assumptions,
        dom_conditions: None,
        cond_context: None,
        cxt: Some(at),
    };
    let known = llrm_mir::valuetracking::known::compute_known_bits(&q, operand, 0);
    let range = llrm_mir::valuetracking::range::compute_constant_range(&q, operand, false, 0).intersect_with(
        &llrm_mir::constantrange::ConstantRange::from_known_bits(&known, true),
        llrm_mir::constantrange::PreferredRangeType::Signed,
    );
    let width = range.width;
    if range.is_full_set() || range.is_empty_set() {
        return None;
    }
    let signed = |bits: u128| {
        let bits = BigInt::from(bits);
        if bits >= BigInt::from(1_u8) << (width - 1) { bits - (BigInt::from(1_u8) << width) } else { bits }
    };
    Some(Interval { low: signed(range.get_signed_min()), high: signed(range.get_signed_max()), width })
}

/// Every value `consts` knows without solving memory, as the singleton
/// interval an alias query reads.
pub fn constants(unit: &Unit) -> Intervals {
    intervals(&unit.registers())
}

/// Each known value as the interval of it alone.
pub fn intervals(known: &IndexMap<ValueId, Known>) -> Intervals {
    known
        .iter()
        .map(|(value, fact)| (*value, Interval { low: fact.n.clone(), high: fact.n.clone(), width: fact.width }))
        .collect()
}

/// Exact values computed without consulting memory.
pub fn singletons(unit: &Unit) -> Intervals {
    let function = unit.function;
    let mut known = Intervals::default();
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
