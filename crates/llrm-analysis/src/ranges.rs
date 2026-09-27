//! Non-wrapping integer intervals, scoped to the branch edges and counted
//! loops that bound them: llrm-core's `analysis/ranges.rs`, a port of
//! `qbopt/analysis/ranges.py`, adapted to the rich MIR. A width is in bits.
//!
//! A comparison is an `icmp` feeding a conditional `br`, where the old MIR
//! read a flags value and a branch's test; an increment is an `add`. Old
//! `Copy` has no counterpart.
//!
//! Waiting for induction's port, the back edge of this dependency cycle:
//! `bounded` (a counted loop's counters and what they compute), `scoped`
//! (bounded, narrowed by `dominated_edges`) and `exact_offsets` (whose
//! leaves `scoped` bounds). Their tests wait with them:
//! `test_guard_refines_subscript_without_leaking_to_the_join`,
//! `test_a_value_the_header_makes_is_bounded_inside_the_loop`,
//! `test_a_compare_under_the_same_compare_is_decided_outside_any_loop` (its
//! `bounded` half; `dominated_edges` answers it here),
//! `test_a_posttested_header_knows_its_counter`.
//!
//! Skipped, BC object corpora: `test_fpdeep_one_based_index_has_a_bounded_byte_offset`,
//! `test_addrm_long_array_value_keeps_counter_bounds`,
//! `test_rngarm_writes_its_counter_only_after_the_loop`.
//! Skipped, `test_non_comparison_flags_do_not_establish_a_bound`: a branch
//! reads an `i1`, which only an `icmp` makes a comparison of; a flags value
//! another operation set has no counterpart.

use std::borrow::Cow;
use std::collections::BTreeMap;

use llrm_graph::loops;
use llrm_mir::context::signed;
use llrm_mir::module::{BlockId, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, IntPredicate, Opcode};
use llrm_support::hash::IndexMap;
use num_bigint::BigInt;

use crate::cfg;
use crate::consts::{self, Known};
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
pub fn covering<'a>(reference: &'a MemRef, known: &BTreeMap<ValueId, Interval>) -> Cow<'a, MemRef> {
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

    let (first, last) = (BigInt::from(reference.disp) + &interval.low * reference.scale, BigInt::from(reference.disp) + &interval.high * reference.scale);
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

/// The block a successor edge leaves and the conditional branch there:
/// its condition, and the block it goes to when that holds.
fn branch(unit: &Unit, block: BlockId) -> Option<(Operand, BlockId)> {
    let terminator = unit.function.instruction(unit.function.terminator(block)?);
    match (&terminator.opcode, terminator.operands.as_slice()) {
        (Opcode::Br, [condition, Operand::Block(taken), Operand::Block(_)]) => Some((*condition, *taken)),
        _ => None,
    }
}

/// Signed comparison facts on one CFG edge; `None` means that edge is impossible.
pub fn on_edge(
    unit: &Unit,
    block: BlockId,
    successor: BlockId,
    known: &IndexMap<ValueId, Interval>,
    facts: Option<&IndexMap<ValueId, Known>>,
) -> Result<Option<IndexMap<ValueId, Interval>>, String> {
    let successors = unit.function.successors(block);
    if !successors.contains(&successor) {
        return Err("not a successor".to_owned());
    }
    let empty = IndexMap::default();
    let facts = facts.unwrap_or(&empty);
    let result = known.clone();
    if successors.len() != 2 {
        return Ok(Some(result));
    }
    let Some((condition, taken)) = branch(unit, block) else {
        return Ok(Some(result));
    };
    let Some((_, compare)) = unit.defining(condition) else {
        return Ok(Some(result));
    };
    let Opcode::ICmp(predicate) = compare.opcode else {
        return Ok(Some(result));
    };
    let (mut left, mut right) = (compare.operands[0], compare.operands[1]);
    let (Some(left_width), Some(right_width)) = (unit.int_bits(left), unit.int_bits(right)) else {
        return Ok(Some(result));
    };
    if !matches!(left_width, 16 | 32) || right_width != left_width {
        return Ok(Some(result));
    }
    let mut kind = predicate;
    if successor != taken {
        kind = kind.inverse();
    }
    let sign = BigInt::from(1_u8) << (left_width - 1);
    let full = Interval { low: -&sign, high: &sign - 1, width: left_width };
    let mut first = _operand(unit, left, known, facts).unwrap_or_else(|| full.clone());
    let mut second = _operand(unit, right, known, facts).unwrap_or(full);
    if matches!(kind, IntPredicate::Ugt | IntPredicate::Uge | IntPredicate::Ult | IntPredicate::Ule) {
        let (first_low, first_high) = _unsigned_span(&first);
        let (second_low, second_high) = _unsigned_span(&second);
        let possible = match kind {
            IntPredicate::Ugt => first_high > second_low,
            IntPredicate::Uge => first_high >= second_low,
            IntPredicate::Ult => first_low < second_high,
            _ => first_low <= second_high,
        };
        return Ok(possible.then_some(result));
    }
    if matches!(kind, IntPredicate::Sge | IntPredicate::Sgt) {
        (left, right) = (right, left);
        (first, second) = (second, first);
        kind = kind.swapped();
    }
    let spans = match kind {
        IntPredicate::Sle | IntPredicate::Slt => {
            let strict = BigInt::from(u8::from(kind == IntPredicate::Slt));
            [(first.low.clone(), first.high.clone().min(&second.high - &strict)), (second.low.clone().max(&first.low + &strict), second.high.clone())]
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
        _ => return Ok(Some(result)),
    };
    let mut result = result;
    for (operand, (low, high)) in [left, right].into_iter().zip(spans) {
        if low > high {
            return Ok(None);
        }
        if let Operand::Value(value) = operand {
            result.insert(value, Interval { low, high, width: left_width });
        }
    }
    Ok(Some(result))
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
pub fn _operand(unit: &Unit, operand: Operand, known: &IndexMap<ValueId, Interval>, facts: &IndexMap<ValueId, Known>) -> Option<Interval> {
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
    let fact = facts.get(&value).filter(|fact| fact.width >= width)?;
    Some(singleton(&fact.n, width))
}

/// `n`'s low `width` bits, signed, as an interval of one.
fn singleton(n: &BigInt, width: u32) -> Interval {
    let number = consts::masked(n, width);
    let sign = BigInt::from(1_u8) << (width - 1);
    let number = (number ^ &sign) - sign;
    Interval { low: number.clone(), high: number, width }
}

/// The interval `inst` computes from what is known of its operands.
pub fn _computed(unit: &Unit, inst: InstId, known: &IndexMap<ValueId, Interval>, facts: &IndexMap<ValueId, Known>) -> Option<Interval> {
    let op = unit.function.instruction(inst);
    let result = op.result?;
    let width = unit.int_bits(Operand::Value(result))?;
    if !matches!(width, 16 | 32) {
        return None;
    }
    // Every other operation answers None below, whatever its operands.
    let kind = match op.opcode {
        Opcode::Cast(CastOp::SExt) => None,
        Opcode::Binary(kind @ (BinaryOp::Shl | BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::And)) => Some(kind),
        _ => return None,
    };
    let operand = |one: Operand| _operand(unit, one, known, facts);
    if kind == Some(BinaryOp::And) {
        // A non-negative mask bounds the result whatever the other operand holds.
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
        return (args.len() == 1 && first.width < width && fits(&first.low, &first.high, first.width))
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
            _ => {
                let products = [&first.low, &first.high]
                    .into_iter()
                    .flat_map(|left| [&second.low, &second.high].into_iter().map(move |right| left * right))
                    .collect::<Vec<_>>();
                (low, high) = (products.iter().min().expect("four products").clone(), products.iter().max().expect("four products").clone());
            }
        }
    } else {
        return None;
    }
    fits(&low, &high, width).then_some(Interval { low, high, width })
}

/// Taken values and the final latch update must all fit without wrapping.
pub fn _recurrence_span(start: &BigInt, step: &BigInt, advances: &BigInt, width: u32) -> Option<Interval> {
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
    let function = unit.function;
    let Some(entry) = function.entry() else {
        return Ok(IndexMap::default());
    };
    let facts = consts::known(unit, None, None, None);
    let graph = cfg::graph(function);
    let predecessors = loops::predecessors(&graph);
    let immediate = loops::immediate_dominators(&graph, Some(cfg::id(entry)));
    let mut known: BTreeMap<i64, IndexMap<ValueId, Interval>> = BTreeMap::new();
    for at in loops::reverse_postorder(&graph, cfg::id(entry)) {
        let block = cfg::block(at);
        let sole = predecessors.get(&at).filter(|parents| parents.len() == 1).and_then(|parents| parents.first());
        let mut scoped = match sole.and_then(|parent| Some((*parent, known.get(parent)?))) {
            Some((parent, inherited)) if edges(unit, cfg::block(parent), block) == 1 => {
                on_edge(unit, cfg::block(parent), block, inherited, Some(&facts))?.unwrap_or_else(|| inherited.clone())
            }
            _ => immediate.get(&at).copied().flatten().and_then(|up| known.get(&up)).cloned().unwrap_or_default(),
        };
        for &inst in function.block(block).instructions() {
            let (Some(interval), Some(result)) = (_computed(unit, inst, &scoped, &facts), function.instruction(inst).result) else {
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
        known.insert(at, scoped);
    }
    Ok(function
        .layout()
        .iter()
        .filter_map(|&block| known.get(&cfg::id(block)).filter(|scoped| !scoped.is_empty()).map(|scoped| (cfg::id(block), scoped.clone())))
        .collect())
}

/// How many of `parent`'s terminator's targets are `block`.
fn edges(unit: &Unit, parent: BlockId, block: BlockId) -> usize {
    unit.function
        .terminator(parent)
        .map_or(0, |one| unit.function.instruction(one).operands.iter().filter(|operand| **operand == Operand::Block(block)).count())
}

/// Every value `consts` knows without solving memory, as the singleton
/// interval an alias query reads.
pub fn constants(unit: &Unit) -> IndexMap<ValueId, Interval> {
    consts::known(unit, None, None, None)
        .into_iter()
        .map(|(value, fact)| (value, Interval { low: fact.n.clone(), high: fact.n, width: fact.width }))
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
                let incoming = op.operands.iter().step_by(2).map(|&one| _operand(unit, one, &known, &none)).collect::<Vec<_>>();
                if !incoming.is_empty() && !incoming.contains(&None) && incoming.iter().collect::<llrm_support::hash::HashSet<_>>().len() == 1 {
                    let interval = incoming[0].clone().expect("known");
                    known.insert(result, interval);
                }
                continue;
            }
            if let Some(interval) = _computed(unit, inst, &known, &none).filter(|interval| interval.low == interval.high) {
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
