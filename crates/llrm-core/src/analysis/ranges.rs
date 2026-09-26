//! Non-wrapping integer intervals, scoped to the taken body of a counted loop.
//!
//! Port of `qbopt/analysis/ranges.py`.

use std::borrow::Cow;
use std::rc::Rc;
use std::collections::{BTreeMap, BTreeSet};

use crate::support::hash::IndexMap;
use num_bigint::BigInt;

use super::{consts, induction, loops};
use crate::model::ir::Operation;
use crate::model::mir::{Arg, Const, Kind, MemRef, MirBlock, MirBody, Op, OpCode, Value, symbolic_ref};
use crate::objectfile::module::Space;
use crate::support::pyset::PySet;

/// A non-wrapping mathematical interval at a fixed width.
///
/// Direct port of `qbopt.analysis.ranges:Interval`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Interval {
    pub low: BigInt,
    pub high: BigInt,
    pub width: u32,
}

/// A non-wrapping near indexed access as the static byte interval it can touch.
///
/// Direct port of `qbopt.analysis.ranges:covering`.
pub fn covering<'a>(reference: &'a MemRef, known: &BTreeMap<Value, Interval>) -> Cow<'a, MemRef> {
    let reference = symbolic_ref(reference);
    let Some(address) = reference.addr else {
        return reference;
    };
    if address.space != Space::Segment || reference.segment.is_some() {
        return reference;
    }
    // Python's `known.get(ref.base)` returns no interval when `base` is None.
    let interval = reference.base.and_then(|base| known.get(&base));
    let Some(interval) = interval else {
        return reference;
    };
    if interval.width != reference.base_width || reference.base_width != 2 {
        return reference;
    }

    let low = BigInt::from(address.disp) + &interval.low;
    let end = BigInt::from(address.disp) + &interval.high + BigInt::from(reference.width);
    let limit = BigInt::from(1_u8) << (8 * reference.base_width);
    if low < BigInt::from(0_u8) || low >= end || end > limit {
        return reference;
    }

    // `Addr` and `MemRef` use host-sized representations, unlike Python's
    // integers. Refuse rather than truncate if a supported Python result does
    // not fit those representations.
    let width = &end - &low;
    let (Ok(disp), Ok(width)) = (i64::try_from(&low), u32::try_from(&width)) else {
        return reference;
    };
    let mut covered = reference.into_owned();
    covered.addr = Some(crate::objectfile::module::Addr {
        disp,
        base: iced_x86::Register::None,
        ..address
    });
    covered.base = None;
    covered.width = width;
    Cow::Owned(covered)
}

/// The values that address cells whose offset every wider sum names exactly.
///
/// A 16-bit address wraps; summed through 32-bit registers it does not. The
/// two agree for a cell whose start is its object's first byte (a symbol, or
/// the far origin the frontend names, or offset 0 of its own segment) when every partial sum of the offset
/// added to that start, as the affine operations computing it would be cut
/// anywhere, is a non-negative integer below 64K: each register then holds
/// its partial sum exactly, zero extension is the identity, and the object
/// ending inside its segment keeps the total there. The language's promise
/// that the access stays inside its object (`inbounds`) is what places the
/// start. A value is exact only if every cell it addresses is.
pub fn exact_offsets(body: &Rc<MirBody>) -> Result<BTreeSet<u32>, String> {
    let scoped = scoped(body)?;
    let mut made: IndexMap<Value, (&Op, i64)> = IndexMap::default();
    let mut arrived: IndexMap<Value, i64> = IndexMap::default();
    for block in &body.blocks {
        for phi in &block.phis {
            arrived.insert(phi.result, block.at);
        }
        for op in &block.ops {
            for result in &op.results {
                if let Arg::Held(held) = result {
                    made.insert(held.value, (op, block.at));
                }
            }
        }
    }
    let mut verdict: IndexMap<u32, bool> = IndexMap::default();
    for block in &body.blocks {
        for op in &block.ops {
            for arg in op.args.iter().chain(&op.results) {
                let Arg::Cell(cell) = arg else {
                    continue;
                };
                let Some(base) = cell.r#ref.base else {
                    continue;
                };
                let exact = _exact_cell(&cell.r#ref, base, block.at, &made, &arrived, &scoped);
                *verdict.entry(base.id).or_insert(true) &= exact;
            }
        }
    }
    Ok(verdict.into_iter().filter(|(_, exact)| *exact).map(|(value, _)| value).collect())
}

type Facts = IndexMap<i64, IndexMap<Value, Interval>>;

fn _exact_cell(
    reference: &MemRef,
    base: Value,
    at: i64,
    made: &IndexMap<Value, (&Op, i64)>,
    arrived: &IndexMap<Value, i64>,
    scoped: &Facts,
) -> bool {
    let Some(addr) = reference.addr else {
        return false;
    };
    if !reference.inbounds || reference.base_width != 2 {
        return false;
    }
    // The part of the address that is an offset into the object.
    let offset = match addr.space {
        Space::Segment | Space::External => Some(base),
        Space::Far | Space::Literal => {
            // Where the base is not `origin + offset`, its whole sum must be
            // exact: a constant origin has folded into the arithmetic.
            let whole = || {
                _exact_sum(base, at, made, arrived, scoped, 16)
                    .is_some_and(|(low, high)| low + addr.disp >= 0 && high + addr.disp < 1 << 16)
            };
            let Some(origin) = reference.origin else {
                return whole();
            };
            if origin == base {
                None
            } else {
                let Some((op, _)) = made.get(&base) else {
                    return whole();
                };
                let held: Vec<Value> = op
                    .args
                    .iter()
                    .filter_map(|arg| match arg {
                        Arg::Held(held) => Some(held.value),
                        _ => None,
                    })
                    .collect();
                match (op.kind, held.as_slice()) {
                    (Kind::Add | Kind::PtrOffset, [left, right]) if *left == origin => Some(*right),
                    (Kind::Add | Kind::PtrOffset, [left, right]) if *right == origin => Some(*left),
                    _ => return whole(),
                }
            }
        }
        _ => return false,
    };
    let (low, high) = match offset {
        Some(offset) => match _exact_sum(offset, at, made, arrived, scoped, 16) {
            Some(bounds) => bounds,
            None => return false,
        },
        None => (0, 0),
    };
    low + addr.disp >= 0 && high + addr.disp < 1 << 16
}

/// The integer range of `value`, where it and every affine partial sum
/// computing it is a non-negative 16-bit integer; leaves read at `at`.
fn _exact_sum(
    value: Value,
    at: i64,
    made: &IndexMap<Value, (&Op, i64)>,
    arrived: &IndexMap<Value, i64>,
    scoped: &Facts,
    depth: usize,
) -> Option<(i64, i64)> {
    // Past the depth, a node is unproven, not a leaf: a later pass may still
    // see through it.
    if depth == 0 {
        return None;
    }
    let inside = |bounds: (i64, i64)| (bounds.0 >= 0 && bounds.1 < 1 << 16).then_some(bounds);
    let operand = |arg: &Arg| match arg {
        Arg::Held(held) if held.width == 2 => _exact_sum(held.value, at, made, arrived, scoped, depth.checked_sub(1)?),
        Arg::Const(constant) => i64::try_from(&constant.n).ok().map(|n| (n, n)),
        _ => None,
    };
    if let Some((op, _)) = made.get(&value).filter(|(op, _)| {
        op.loads.is_empty() && !crate::model::mir::partial(op) && op.results.len() == 1
    }) {
        let affine = match (op.kind, op.args.as_slice()) {
            (Kind::Copy, [one]) => Some(operand(one)),
            (Kind::Add, [left, right]) => Some(operand(left).zip(operand(right)).map(|(l, r)| (l.0 + r.0, l.1 + r.1))),
            (Kind::Sub, [left, right @ Arg::Const(_)]) => {
                Some(operand(left).zip(operand(right)).map(|(l, r)| (l.0 - r.1, l.1 - r.0)))
            }
            (Kind::Mul, [left, right @ Arg::Const(_)]) | (Kind::Mul, [right @ Arg::Const(_), left]) => {
                Some(operand(left).zip(operand(right)).filter(|(_, r)| r.0 >= 0).map(|(l, r)| (l.0 * r.0, l.1 * r.0)))
            }
            (Kind::Shl, [left, Arg::Const(count)]) => {
                let count = i64::try_from(&count.n).ok().filter(|count| (0..16).contains(count));
                Some(operand(left).zip(count).map(|(l, count)| (l.0 << count, l.1 << count)))
            }
            _ => None,
        };
        if let Some(bounds) = affine {
            return bounds.and_then(inside);
        }
    }
    // A leaf: whatever it is, it is read in this block.
    let defined = made.get(&value).map(|(_, block)| *block).or_else(|| arrived.get(&value).copied());
    let fact = scoped
        .get(&at)
        .and_then(|known| known.get(&value))
        .or_else(|| defined.and_then(|block| scoped.get(&block)).and_then(|known| known.get(&value)))?;
    let bounds = (i64::try_from(&fact.low).ok()?, i64::try_from(&fact.high).ok()?);
    (fact.width == 2).then_some(bounds).and_then(inside)
}

/// Signed comparison facts on one CFG edge; `None` means that edge is impossible.
///
/// Direct port of `qbopt.analysis.ranges:on_edge`.
pub fn on_edge(
    block: &MirBlock,
    successor: i64,
    known: &IndexMap<Value, Interval>,
    facts: Option<&IndexMap<Value, consts::Known>>,
) -> Result<Option<IndexMap<Value, Interval>>, String> {
    if !block.succ.contains(&successor) {
        return Err("not a successor".to_owned());
    }
    let empty = IndexMap::default();
    let facts = facts.unwrap_or(&empty);
    let mut result = known.clone();
    if block.ops.is_empty() || block.succ.len() != 2 {
        return Ok(Some(result));
    }
    let branch = &block.ops[block.ops.len() - 1];
    let flags = branch.uses.iter().filter(|value| value.flags).copied().collect::<Vec<_>>();
    if branch.kind != Kind::Branch
        || !branch.target.is_some_and(|target| block.succ.contains(&target))
        || flags.len() != 1
    {
        return Ok(Some(result));
    }
    let compare = block.ops[..block.ops.len() - 1]
        .iter()
        .rev()
        .find(|op| op.defines.contains(&flags[0]));
    let Some(compare) = compare else {
        return Ok(Some(result));
    };
    if compare.op != Some(OpCode::Operation(Operation::Compare))
        || compare.args.len() != 2
        || compare.kind != Kind::Sub
        || compare.defines != [flags[0]]
        || !compare.results.is_empty()
        || !compare.loads.is_empty()
        || !compare.stores.is_empty()
        || !compare.merges.is_empty()
        || compare.barrier()
        || compare.floating.is_some()
    {
        return Ok(Some(result));
    }
    let (mut left, mut right) = (&compare.args[0], &compare.args[1]);
    let width_of = |arg: &Arg| match arg {
        Arg::Held(held) => Some(held.width),
        Arg::Const(constant) => Some(constant.width),
        _ => None,
    };
    let (Some(left_width), Some(right_width)) = (width_of(left), width_of(right)) else {
        return Ok(Some(result));
    };
    if !matches!(left_width, 2 | 4) || right_width != left_width {
        return Ok(Some(result));
    }
    let mut kind = branch.test;
    if Some(successor) != branch.target {
        kind = kind.and_then(crate::model::mir::NEGATED);
    }
    let sign = BigInt::from(1_u8) << (left_width * 8 - 1);
    let full = Interval {
        low: -&sign,
        high: &sign - 1,
        width: left_width,
    };
    let mut first = _operand(left, known, facts).unwrap_or_else(|| full.clone());
    let mut second = _operand(right, known, facts).unwrap_or(full);
    if matches!(kind, Some(Kind::Above | Kind::AboveEq | Kind::Below | Kind::BelowEq)) {
        let (first_low, first_high) = _unsigned_span(&first);
        let (second_low, second_high) = _unsigned_span(&second);
        let possible = match kind {
            Some(Kind::Above) => first_high > second_low,
            Some(Kind::AboveEq) => first_high >= second_low,
            Some(Kind::Below) => first_low < second_high,
            _ => first_low <= second_high,
        };
        return Ok(possible.then_some(result));
    }
    if matches!(kind, Some(Kind::Ge | Kind::Gt)) {
        (left, right) = (right, left);
        (first, second) = (second, first);
        kind = Some(if kind == Some(Kind::Ge) { Kind::Le } else { Kind::Lt });
    }
    let spans = match kind {
        Some(Kind::Le | Kind::Lt) => {
            let strict = BigInt::from(u8::from(kind == Some(Kind::Lt)));
            [
                (first.low.clone(), first.high.clone().min(&second.high - &strict)),
                (second.low.clone().max(&first.low + &strict), second.high.clone()),
            ]
        }
        Some(Kind::Eq) => {
            let shared = (
                first.low.clone().max(second.low.clone()),
                first.high.clone().min(second.high.clone()),
            );
            [shared.clone(), shared]
        }
        Some(Kind::Ne) => {
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
    for (arg, (low, high)) in [left, right].into_iter().zip(spans) {
        if low > high {
            return Ok(None);
        }
        if let Arg::Held(held) = arg {
            result.insert(
                held.value,
                Interval {
                    low,
                    high,
                    width: held.width,
                },
            );
        }
    }
    Ok(Some(result))
}

/// Direct port of `qbopt.analysis.ranges:_unsigned_span`.
fn _unsigned_span(interval: &Interval) -> (BigInt, BigInt) {
    let mask = (BigInt::from(1_u8) << (interval.width * 8)) - 1;
    let zero = BigInt::from(0_u8);
    if interval.low < zero && zero <= interval.high {
        return (zero, mask);
    }
    (&interval.low & &mask, &interval.high & &mask)
}

/// Direct port of `qbopt.analysis.ranges:_operand`.
pub fn _operand(
    arg: &Arg,
    known: &IndexMap<Value, Interval>,
    facts: &IndexMap<Value, consts::Known>,
) -> Option<Interval> {
    if let Arg::Const(constant) = arg {
        let number = consts::masked(&constant.n, constant.width);
        let sign = BigInt::from(1_u8) << (constant.width * 8 - 1);
        let number = (number ^ &sign) - sign;
        return Some(Interval {
            low: number.clone(),
            high: number,
            width: constant.width,
        });
    }
    let Arg::Held(held) = arg else {
        return None;
    };
    if let Some(interval) = known.get(&held.value) {
        if interval.width == held.width {
            return Some(interval.clone());
        }
    }
    if let Some(fact) = facts.get(&held.value) {
        if fact.width >= held.width {
            return _operand(
                &Arg::Const(Const::new(fact.n.clone(), held.width)),
                &IndexMap::default(),
                &IndexMap::default(),
            );
        }
    }
    None
}

/// Direct port of `qbopt.analysis.ranges:_computed`.
pub fn _computed(
    op: &Op,
    known: &IndexMap<Value, Interval>,
    facts: &IndexMap<Value, consts::Known>,
) -> Option<Interval> {
    if !op.loads.is_empty() || !op.stores.is_empty() || op.barrier() || op.results.len() != 1 {
        return None;
    }
    let Arg::Held(result) = &op.results[0] else {
        return None;
    };
    if !matches!(result.width, 2 | 4) {
        return None;
    }
    // Every other kind answers None below, whatever its operands.
    if !matches!(
        op.kind,
        Kind::SignExtend
            | Kind::Copy
            | Kind::Increment
            | Kind::Decrement
            | Kind::Shl
            | Kind::Add
            | Kind::Sub
            | Kind::Mul
            | Kind::And
    ) {
        return None;
    }
    let operand = |arg: &Arg| match arg {
        Arg::Held(held) => match known.get(&held.value) {
            Some(interval) if interval.width == held.width => Some(Cow::Borrowed(interval)),
            _ => _operand(arg, known, facts).map(Cow::Owned),
        },
        _ => _operand(arg, known, facts).map(Cow::Owned),
    };
    if op.kind == Kind::And {
        // A non-negative mask bounds the result whatever the other operand holds.
        let high = op
            .args
            .iter()
            .filter_map(operand)
            .filter(|mask| mask.width == result.width && mask.low >= BigInt::from(0_u8))
            .map(|mask| mask.high.clone())
            .min()?;
        return Some(Interval { low: BigInt::from(0_u8), high, width: result.width });
    }
    let args = op.args.iter().map(operand).collect::<Option<Vec<_>>>()?;
    if args.is_empty() {
        return None;
    }
    let first = &args[0];
    let fits = |low: &BigInt, high: &BigInt, width: u32| {
        let sign = BigInt::from(1_u8) << (width * 8 - 1);
        -&sign <= *low && low <= high && *high < sign
    };
    if op.kind == Kind::SignExtend && args.len() == 1 && 0 < first.width && first.width < result.width {
        return fits(&first.low, &first.high, first.width).then(|| Interval {
            low: first.low.clone(),
            high: first.high.clone(),
            width: result.width,
        });
    }
    if first.width != result.width {
        return None;
    }
    if op.kind == Kind::Copy && args.len() == 1 {
        return Some(first.clone().into_owned());
    }
    if matches!(op.kind, Kind::Increment | Kind::Decrement) && args.len() == 1 {
        let step = if op.kind == Kind::Increment { 1 } else { -1 };
        let (low, high) = (&first.low + step, &first.high + step);
        return fits(&low, &high, result.width).then_some(Interval {
            low,
            high,
            width: result.width,
        });
    }
    if args.len() != 2 {
        return None;
    }
    let second = &args[1];
    let (low, high);
    if op.kind == Kind::Shl {
        if second.low != second.high
            || second.low < BigInt::from(0_u8)
            || second.low >= BigInt::from(result.width * 8)
        {
            return None;
        }
        let shift = usize::try_from(&second.low).expect("a checked shift is below the operation width");
        (low, high) = (&first.low << shift, &first.high << shift);
    } else if second.width == result.width {
        match op.kind {
            Kind::Add => (low, high) = (&first.low + &second.low, &first.high + &second.high),
            Kind::Sub => (low, high) = (&first.low - &second.high, &first.high - &second.low),
            Kind::Mul => {
                let products = [&first.low, &first.high]
                    .into_iter()
                    .flat_map(|left| [&second.low, &second.high].into_iter().map(move |right| left * right))
                    .collect::<Vec<_>>();
                (low, high) = (
                    products.iter().min().expect("four products").clone(),
                    products.iter().max().expect("four products").clone(),
                );
            }
            _ => return None,
        }
    } else {
        return None;
    }
    fits(&low, &high, result.width).then_some(Interval {
        low,
        high,
        width: result.width,
    })
}

/// Taken values and the final latch update must all fit without wrapping.
///
/// Direct port of `qbopt.analysis.ranges:_recurrence_span`.
fn _recurrence_span(start: &BigInt, step: &BigInt, advances: &BigInt, width: u32) -> Option<Interval> {
    let last = start + advances * step;
    let sign = BigInt::from(1_u8) << (width * 8 - 1);
    let after = &last + step;
    let lowest = start.min(&last).min(&after);
    let highest = start.max(&last).max(&after);
    if *advances >= BigInt::from(0_u8) && -&sign <= *lowest && lowest <= highest && *highest < sign {
        return Some(Interval {
            low: start.min(&last).clone(),
            high: start.max(&last).clone(),
            width,
        });
    }
    None
}

/// Direct port of `qbopt.analysis.ranges:bounded`; a block in no counted
/// loop keeps the facts of the branch edges that dominate it.
pub fn bounded(body: &Rc<MirBody>) -> Result<IndexMap<i64, IndexMap<Value, Interval>>, String> {
    let facts = consts::known(body, None, None, None, None);
    let mut result: IndexMap<i64, IndexMap<Value, Interval>> = IndexMap::default();
    let predecessors = loops::predecessors(&body.blocks);
    let dominators = loops::dominators(&body.blocks, Some(body.entry));
    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        let proofs = induction::counted_unless_stopped(body, &loop_, Some(&facts), false);
        // A header that tests before the trip also sees the exit value; one
        // tested after it sees only the trip's.
        let mut inside = loop_.body.iter().copied().collect::<PySet<i64>>();
        if proofs.is_empty() || !proofs.iter().all(|proof| proof.posttested) {
            inside.discard(&loop_.header);
        }
        let mut known: IndexMap<Value, Interval> = IndexMap::default();
        let header = body
            .blocks
            .iter()
            .find(|block| block.at == loop_.header)
            .expect("a loop header is one of the body's blocks");
        let phis = header
            .phis
            .iter()
            .map(|phi| (phi.result.id, phi.result))
            .collect::<IndexMap<_, _>>();
        let counters = induction::basics(body, &loop_).values().cloned().collect::<Vec<_>>();
        let mut trips = BTreeSet::new();
        for proof in proofs {
            if let Some((low, high)) = proof.span() {
                known.insert(proof.phi_in(body).result, Interval { low, high, width: proof.counter.start.width() });
                trips.insert(proof.count.expect("a span has a count") - 1_u8);
            }
        }
        if trips.len() == 1 {
            let advances = trips.first().expect("one trip count").clone();
            for counter in &counters {
                let width = counter.start.width();
                let start = induction::_signed(&counter.start.as_arg(), &facts, width);
                let step = induction::_signed(&counter.step.as_arg(), &facts, width);
                let (Some(start), Some(step)) = (start, step) else {
                    continue;
                };
                if let Some(interval) = _recurrence_span(&start, &step, &advances, width) {
                    let phi = *phis.get(&counter.value).ok_or_else(|| counter.value.to_string())?;
                    known.insert(phi, interval);
                }
            }
        }
        if known.is_empty() {
            continue;
        }
        // The header's values too: seen from inside, they are the trip's,
        // though the header itself also sees the exit value.
        let operations = body
            .blocks
            .iter()
            .filter(|block| inside.contains(&block.at) || block.at == loop_.header)
            .flat_map(|block| block.ops.iter())
            .collect::<Vec<_>>();
        loop {
            let before = known.len();
            for op in &operations {
                if let Some(Arg::Held(held)) = op.results.first() {
                    if !known.contains_key(&held.value) {
                        if let Some(interval) = _computed(op, &known, &facts) {
                            known.insert(held.value, interval);
                        }
                    }
                }
            }
            if known.len() == before {
                break;
            }
        }
        for &at in inside.iter() {
            let mut scoped = known.clone();
            for block in &body.blocks {
                for &successor in &block.succ {
                    let parents = predecessors.get(&successor).ok_or_else(|| successor.to_string())?;
                    if parents.len() == 1
                        && parents.contains(&block.at)
                        && dominators.get(&at).is_some_and(|dominating| dominating.contains(&successor))
                    {
                        if let Some(narrowed) = on_edge(block, successor, &scoped, Some(&facts))? {
                            scoped = narrowed;
                        }
                    }
                }
            }
            loop {
                // What each value set in this sweep held before it, so the
                // sweep is compared with its start without copying `scoped`.
                let mut before = IndexMap::<Value, Option<Interval>>::default();
                for op in &operations {
                    let Some(mut interval) = _computed(op, &scoped, &facts) else {
                        continue;
                    };
                    let Arg::Held(held) = &op.results[0] else {
                        unreachable!("_computed answers only a held result");
                    };
                    if let Some(previous) = scoped.get(&held.value) {
                        if previous.width == interval.width {
                            let low = previous.low.clone().max(interval.low.clone());
                            let high = previous.high.clone().min(interval.high.clone());
                            if low > high {
                                continue;
                            }
                            interval = Interval {
                                low,
                                high,
                                width: interval.width,
                            };
                        }
                    }
                    let previous = scoped.insert(held.value, interval);
                    before.entry(held.value).or_insert(previous);
                }
                if before.iter().all(|(value, was)| scoped.get(value) == was.as_ref()) {
                    break;
                }
            }
            let destination = result.entry(at).or_default();
            for (value, interval) in scoped {
                match destination.get(&value) {
                    None => {
                        destination.insert(value, interval);
                    }
                    Some(previous) if previous.width == interval.width => {
                        let low = previous.low.clone().max(interval.low.clone());
                        let high = previous.high.clone().min(interval.high.clone());
                        if low <= high {
                            destination.insert(
                                value,
                                Interval {
                                    low,
                                    high,
                                    width: interval.width,
                                },
                            );
                        }
                    }
                    Some(_) => {}
                }
            }
        }
    }
    for (at, known) in dominated_edges(body)? {
        result.entry(at).or_insert(known);
    }
    Ok(result)
}

/// Facts established by unavoidable branch edges at each block, and what
/// its own operations compute from them.
///
/// An edge counts only when its destination has that one predecessor: a
/// join is a second way around the check. So a block starts from its sole
/// predecessor's facts narrowed by that edge, or else from its immediate
/// dominator's, and each edge is applied once.
pub fn dominated_edges(body: &Rc<MirBody>) -> Result<IndexMap<i64, IndexMap<Value, Interval>>, String> {
    let facts = consts::known(body, None, None, None, None);
    let predecessors = loops::predecessors(&body.blocks);
    let immediate = loops::immediate_dominators(&body.blocks, Some(body.entry));
    let blocks = body.blocks.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
    let mut known: BTreeMap<i64, IndexMap<Value, Interval>> = BTreeMap::new();
    for at in loops::reverse_postorder(&body.blocks, body.entry) {
        let block = blocks[&at];
        let sole = predecessors.get(&at).filter(|parents| parents.len() == 1).and_then(|parents| parents.first());
        let mut scoped = match sole.and_then(|parent| Some((blocks.get(parent)?, known.get(parent)?))) {
            Some((parent, inherited)) if parent.succ.iter().filter(|to| **to == at).count() == 1 => {
                on_edge(parent, at, inherited, Some(&facts))?.unwrap_or_else(|| inherited.clone())
            }
            _ => immediate.get(&at).copied().flatten().and_then(|up| known.get(&up)).cloned().unwrap_or_default(),
        };
        for op in &block.ops {
            let (Some(interval), Some(Arg::Held(held))) = (_computed(op, &scoped, &facts), op.results.first()) else {
                continue;
            };
            let interval = match scoped.get(&held.value) {
                Some(previous) if previous.width == interval.width => Interval {
                    low: previous.low.clone().max(interval.low),
                    high: previous.high.clone().min(interval.high),
                    width: interval.width,
                },
                _ => interval,
            };
            if interval.low <= interval.high {
                scoped.insert(held.value, interval);
            }
        }
        known.insert(at, scoped);
    }
    Ok(body
        .blocks
        .iter()
        .filter_map(|block| known.get(&block.at).filter(|scoped| !scoped.is_empty()).map(|scoped| (block.at, scoped.clone())))
        .collect())
}

/// Every interval known at each block: a loop's counters and what they
/// compute, narrowed by the branch edges that dominate it.
pub fn scoped(body: &Rc<MirBody>) -> Result<IndexMap<i64, IndexMap<Value, Interval>>, String> {
    let mut result = bounded(body)?;
    for (at, edges) in dominated_edges(body)? {
        let known = result.entry(at).or_default();
        for (value, interval) in edges {
            match known.get(&value) {
                Some(loop_) if loop_.width == interval.width => {
                    let low = loop_.low.clone().max(interval.low.clone());
                    let high = loop_.high.clone().min(interval.high.clone());
                    if low <= high {
                        known.insert(value, Interval { low, high, width: interval.width });
                    }
                }
                _ => {
                    known.insert(value, interval);
                }
            }
        }
    }
    Ok(result)
}

/// Every value `consts` knows without solving memory, as the singleton
/// interval an alias query reads.
pub fn constants(body: &Rc<MirBody>) -> IndexMap<Value, Interval> {
    consts::known(body, None, None, None, None)
        .into_iter()
        .map(|(value, fact)| {
            (
                value,
                Interval {
                    low: fact.n.clone(),
                    high: fact.n,
                    width: fact.width,
                },
            )
        })
        .collect()
}

/// Exact values computed without consulting memory.
#[allow(dead_code)] // transform.py's, not yet ported
pub fn singletons(body: &MirBody) -> IndexMap<Value, Interval> {
    let mut known = IndexMap::<Value, Interval>::default();
    loop {
        let before = known.len();
        for block in &body.blocks {
            for phi in &block.phis {
                if known.contains_key(&phi.result) || phi.incoming.is_empty() {
                    continue;
                }
                let incoming = phi.incoming.values().map(|value| known.get(value)).collect::<Vec<_>>();
                if !incoming.is_empty()
                    && !incoming.contains(&None)
                    && incoming.iter().collect::<crate::support::hash::HashSet<_>>().len() == 1
                {
                    let interval = incoming[0].expect("known").clone();
                    known.insert(phi.result, interval);
                }
            }
            for op in &block.ops {
                let Some(Arg::Held(result)) = op.results.first() else {
                    continue;
                };
                if known.contains_key(&result.value) {
                    continue;
                }
                if let Some(interval) = _computed(op, &known, &IndexMap::default()) {
                    if interval.low == interval.high {
                        known.insert(result.value, interval);
                    }
                }
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
