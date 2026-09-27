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
//! Skipped, BC object corpora: `test_fpdeep_one_based_index_has_a_bounded_byte_offset`,
//! `test_addrm_long_array_value_keeps_counter_bounds`,
//! `test_rngarm_writes_its_counter_only_after_the_loop`.
//! Skipped, `test_non_comparison_flags_do_not_establish_a_bound`: a branch
//! reads an `i1`, which only an `icmp` makes a comparison of; a flags value
//! another operation set has no counterpart.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use crate::graph::loops;
use llrm_mir::context::signed;
use llrm_mir::module::{BlockId, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, IntPredicate, Opcode};
use llrm_support::hash::IndexMap;
use num_bigint::BigInt;

use crate::cfg;
use crate::consts::{self, Known};
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

/// Whether every byte `reference` may touch lies inside the object its
/// root names, its index within `known`: dereferenceable, as LLVM's
/// `isDereferenceablePointer` says of a constant offset, here of a bounded
/// one.
pub fn inside_object(unit: &Unit, reference: &MemRef, known: &BTreeMap<ValueId, Interval>) -> bool {
    let covered = covering(reference, known);
    let extent = covered.root.and_then(|root| crate::memory::object_of(unit, root)).and_then(|object| object.extent);
    covered.base.is_none() && covered.object && covered.segment.is_none() && extent.is_some_and(|extent| 0 <= covered.disp && covered.disp + i64::from(covered.width) <= extent)
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
    if right_width != left_width {
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
    dominated_edges_with(unit, &unit.registers())
}

/// `dominated_edges`, given what `consts::known` finds without memory.
pub fn dominated_edges_with(unit: &Unit, facts: &IndexMap<ValueId, Known>) -> Result<IndexMap<i64, IndexMap<ValueId, Interval>>, String> {
    let function = unit.function;
    let Some(entry) = function.entry() else {
        return Ok(IndexMap::default());
    };
    let graph = cfg::graph(function);
    let predecessors = loops::predecessors(&graph);
    let immediate = unit.shape().dominance.immediate_dominators(function);
    let mut known: BTreeMap<i64, IndexMap<ValueId, Interval>> = BTreeMap::new();
    for at in loops::reverse_postorder(&graph, cfg::id(entry)) {
        let block = cfg::block(at);
        let sole = predecessors.get(&at).filter(|parents| parents.len() == 1).and_then(|parents| parents.first());
        let mut scoped = match sole.and_then(|parent| Some((*parent, known.get(parent)?))) {
            Some((parent, inherited)) if edges(unit, cfg::block(parent), block) == 1 => {
                on_edge(unit, cfg::block(parent), block, inherited, Some(facts))?.unwrap_or_else(|| inherited.clone())
            }
            _ => immediate.get(&at).copied().flatten().and_then(|up| known.get(&up)).cloned().unwrap_or_default(),
        };
        for &inst in function.block(block).instructions() {
            let (Some(interval), Some(result)) = (_computed(unit, inst, &scoped, facts), function.instruction(inst).result) else {
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

pub type Facts = IndexMap<i64, IndexMap<ValueId, Interval>>;

/// Each block's intervals from the counted loops holding it: its counters,
/// what the loop computes from them, narrowed by the branch edges that
/// dominate the block. A block in no counted loop keeps the facts of the
/// branch edges that dominate it.
pub fn bounded(unit: &Unit) -> Result<Facts, String> {
    bounded_with(unit, &unit.registers())
}

/// `bounded`, given what `consts::known` finds without memory.
pub fn bounded_with(unit: &Unit, facts: &IndexMap<ValueId, Known>) -> Result<Facts, String> {
    let function = unit.function;
    let graph = cfg::graph(function);
    let predecessors = loops::predecessors(&graph);
    let shape = unit.shape();
    let mut result = Facts::default();
    for loop_ in &shape.loops {
        let proofs = induction::counted_unless_stopped(unit, &loop_, Some(facts), false);
        // A header that tests before the trip also sees the exit value; one
        // tested after it sees only the trip's.
        let mut inside = loop_.body.clone();
        if proofs.is_empty() || !proofs.iter().all(|proof| proof.posttested) {
            inside.remove(&loop_.header);
        }
        let mut known = IndexMap::<ValueId, Interval>::default();
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
        if known.is_empty() {
            continue;
        }
        // The header's values too: seen from inside, they are the trip's,
        // though the header itself also sees the exit value.
        let operations = function
            .layout()
            .iter()
            .filter(|&&block| inside.contains(&cfg::id(block)) || cfg::id(block) == loop_.header)
            .flat_map(|&block| function.block(block).instructions().iter().copied())
            .filter(|&inst| function.instruction(inst).opcode != Opcode::Phi)
            .collect::<Vec<_>>();
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
                break;
            }
        }
        for &at in &inside {
            let mut scoped = known.clone();
            for block in &graph {
                for &successor in &block.succ {
                    let parents = predecessors.get(&successor).ok_or_else(|| successor.to_string())?;
                    if parents.len() == 1
                        && parents.contains(&block.at)
                        && shape.dominance.dominates(successor, at)
                        && let Some(narrowed) = on_edge(unit, cfg::block(block.at), cfg::block(successor), &scoped, Some(&facts))?
                    {
                        scoped = narrowed;
                    }
                }
            }
            loop {
                // What each value set in this sweep held before it.
                let mut before = IndexMap::<ValueId, Option<Interval>>::default();
                for &inst in &operations {
                    let Some(mut interval) = _computed(unit, inst, &scoped, &facts) else { continue };
                    let result = function.instruction(inst).result.expect("_computed answers a result");
                    if let Some(previous) = scoped.get(&result).filter(|previous| previous.width == interval.width) {
                        let (low, high) = (previous.low.clone().max(interval.low), previous.high.clone().min(interval.high));
                        if low > high {
                            continue;
                        }
                        interval = Interval { low, high, width: interval.width };
                    }
                    let previous = scoped.insert(result, interval);
                    before.entry(result).or_insert(previous);
                }
                if before.iter().all(|(value, was)| scoped.get(value) == was.as_ref()) {
                    break;
                }
            }
            let destination = result.entry(at).or_default();
            for (value, interval) in scoped {
                narrow(destination, value, interval);
            }
        }
    }
    for (at, known) in dominated_edges_with(unit, facts)? {
        result.entry(at).or_insert(known);
    }
    Ok(result)
}

/// `interval` for `value` in `known`, met with what it already held at that width.
fn narrow(known: &mut IndexMap<ValueId, Interval>, value: ValueId, interval: Interval) {
    match known.get(&value) {
        None => {
            known.insert(value, interval);
        }
        Some(previous) if previous.width == interval.width => {
            let (low, high) = (previous.low.clone().max(interval.low), previous.high.clone().min(interval.high));
            if low <= high {
                known.insert(value, Interval { low, high, width: interval.width });
            }
        }
        Some(_) => {}
    }
}

/// Every interval known at each block: a loop's counters and what they
/// compute, narrowed by the branch edges that dominate it.
pub fn scoped(unit: &Unit) -> Result<Facts, String> {
    let mut result = bounded(unit)?;
    for (at, edges) in dominated_edges(unit)? {
        let known = result.entry(at).or_default();
        for (value, interval) in edges {
            match known.get(&value) {
                Some(previous) if previous.width == interval.width => narrow(known, value, interval),
                _ => {
                    known.insert(value, interval);
                }
            }
        }
    }
    Ok(result)
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
    let scoped = scoped(unit)?;
    let mut verdict = IndexMap::<ValueId, bool>::default();
    for (block, inst) in unit.function.walk() {
        let Some(reference) = MemRef::of(unit, inst) else { continue };
        let Some(base) = reference.base else { continue };
        let bits = reference.index_bits;
        let exact = reference.inbounds
            && reference.object
            && reference.base_width == bits
            && _exact_sum(unit, base, cfg::id(block), &scoped, bits, 16).is_some_and(|(low, high)| {
                let (low, high) = (BigInt::from(reference.disp) + low * reference.scale, BigInt::from(reference.disp) + high * reference.scale);
                low >= BigInt::from(0) && high < BigInt::from(1) << bits
            });
        *verdict.entry(base).or_insert(true) &= exact;
    }
    Ok(verdict.into_iter().filter(|(_, exact)| *exact).map(|(value, _)| value).collect())
}

/// The integer range of `value`, where it and every affine partial sum
/// computing it is a non-negative integer below 2**bits; leaves read at `at`.
fn _exact_sum(unit: &Unit, value: ValueId, at: i64, scoped: &Facts, bits: u32, depth: usize) -> Option<(BigInt, BigInt)> {
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
            (Opcode::Binary(BinaryOp::Add), &[left, right]) => Some(operand(left).zip(operand(right)).map(|(l, r)| (l.0 + r.0, l.1 + r.1))),
            (Opcode::Binary(BinaryOp::Sub), &[left, right]) if constant(right).is_some() => {
                Some(operand(left).zip(operand(right)).map(|(l, r)| (l.0 - r.1, l.1 - r.0)))
            }
            (Opcode::Binary(BinaryOp::Mul), &[left, right]) | (Opcode::Binary(BinaryOp::Mul), &[right, left]) if constant(right).is_some() => {
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
    known.iter().map(|(value, fact)| (*value, Interval { low: fact.n.clone(), high: fact.n.clone(), width: fact.width })).collect()
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
