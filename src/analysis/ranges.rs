//! Non-wrapping integer intervals, scoped to the taken body of a counted loop.
//!
//! Port of `qbopt/analysis/ranges.py`.

use std::rc::Rc;
use std::collections::{BTreeMap, BTreeSet};

use indexmap::IndexMap;
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
pub(crate) struct Interval {
    pub low: BigInt,
    pub high: BigInt,
    pub width: u32,
}

/// A non-wrapping near indexed access as the static byte interval it can touch.
///
/// Direct port of `qbopt.analysis.ranges:covering`.
pub(crate) fn covering(reference: &MemRef, known: &BTreeMap<Value, Interval>) -> MemRef {
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
    let mut covered = reference.clone();
    covered.addr = Some(crate::objectfile::module::Addr {
        disp,
        base: iced_x86::Register::None,
        ..address
    });
    covered.base = None;
    covered.width = width;
    covered
}

/// Signed comparison facts on one CFG edge; `None` means that edge is impossible.
///
/// Direct port of `qbopt.analysis.ranges:on_edge`.
pub(crate) fn on_edge(
    block: &MirBlock,
    successor: i64,
    known: &IndexMap<Value, Interval>,
    facts: Option<&IndexMap<Value, consts::Known>>,
) -> Result<Option<IndexMap<Value, Interval>>, String> {
    if !block.succ.contains(&successor) {
        return Err("not a successor".to_owned());
    }
    let empty = IndexMap::new();
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
        kind = match kind {
            Some(Kind::Le) => Some(Kind::Gt),
            Some(Kind::Lt) => Some(Kind::Ge),
            Some(Kind::Ge) => Some(Kind::Lt),
            Some(Kind::Gt) => Some(Kind::Le),
            Some(Kind::Eq) => Some(Kind::Ne),
            Some(Kind::Ne) => Some(Kind::Eq),
            Some(Kind::Above) => Some(Kind::BelowEq),
            Some(Kind::AboveEq) => Some(Kind::Below),
            Some(Kind::Below) => Some(Kind::AboveEq),
            Some(Kind::BelowEq) => Some(Kind::Above),
            _ => None,
        };
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
pub(crate) fn _operand(
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
                &IndexMap::new(),
                &IndexMap::new(),
            );
        }
    }
    None
}

/// Direct port of `qbopt.analysis.ranges:_computed`.
pub(crate) fn _computed(
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
    let args = op
        .args
        .iter()
        .map(|arg| _operand(arg, known, facts))
        .collect::<Vec<_>>();
    if args.is_empty() || args.iter().any(Option::is_none) {
        return None;
    }
    let args = args.into_iter().flatten().collect::<Vec<_>>();
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
        return Some(first.clone());
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

/// Direct port of `qbopt.analysis.ranges:bounded`.
pub(crate) fn bounded(body: &Rc<MirBody>) -> Result<IndexMap<i64, IndexMap<Value, Interval>>, String> {
    let facts = consts::known(body, None, None, None, None);
    let mut result: IndexMap<i64, IndexMap<Value, Interval>> = IndexMap::new();
    let predecessors = loops::predecessors(&body.blocks);
    let dominators = loops::dominators(&body.blocks, Some(body.entry));
    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        let mut inside = loop_.body.iter().copied().collect::<PySet<i64>>();
        inside.discard(&loop_.header);
        let mut known: IndexMap<Value, Interval> = IndexMap::new();
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
        for counter in &counters {
            let width = counter.start.width();
            let last = induction::_last_counter(body, &loop_, counter, &facts, width);
            let start = induction::_signed(&counter.start.as_arg(), &facts, width);
            if let (Some(last), Some(start)) = (last, start) {
                let phi = *phis.get(&counter.value).ok_or_else(|| counter.value.to_string())?;
                known.insert(
                    phi,
                    Interval {
                        low: start.clone().min(last.clone()),
                        high: start.clone().max(last.clone()),
                        width,
                    },
                );
                let Some(step) = induction::_signed(&counter.step.as_arg(), &facts, width) else {
                    return Err("unsupported operand type(s) for //: 'int' and 'NoneType'".to_owned());
                };
                if step == BigInt::from(0_u8) {
                    return Err("integer division or modulo by zero".to_owned());
                }
                trips.insert(induction::floor_div(&(last - start), &step));
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
        let operations = body
            .blocks
            .iter()
            .filter(|block| inside.contains(&block.at))
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
                let before = scoped.clone();
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
                    scoped.insert(held.value, interval);
                }
                if scoped == before {
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
    Ok(result)
}

/// Facts established by unavoidable branch edges at each block.
///
/// Direct port of `qbopt.analysis.ranges:dominated_edges`.  An edge counts
/// only when its destination has that one predecessor and dominates the
/// queried block: a join is a second way around the check.
pub(crate) fn dominated_edges(body: &Rc<MirBody>) -> Result<IndexMap<i64, IndexMap<Value, Interval>>, String> {
    let facts = consts::known(body, None, None, None, None);
    let predecessors = loops::predecessors(&body.blocks);
    let dominators = loops::dominators(&body.blocks, Some(body.entry));
    let mut edges = body
        .blocks
        .iter()
        .flat_map(|block| block.succ.iter().map(move |&successor| (block, successor)))
        .filter(|(block, successor)| {
            predecessors
                .get(successor)
                .is_some_and(|parents| parents.len() == 1 && parents.contains(&block.at))
        })
        .collect::<Vec<_>>();
    edges.sort_by_key(|(_, successor)| dominators.get(successor).map_or(0, BTreeSet::len));
    let mut result: IndexMap<i64, IndexMap<Value, Interval>> = IndexMap::new();
    for block in &body.blocks {
        let mut known: IndexMap<Value, Interval> = IndexMap::new();
        // Apply the path from outermost to innermost dominator once. Repeating
        // a relational `a < b` constraint would falsely walk both open
        // intervals inward rather than intersecting with one original fact.
        for &(parent, successor) in &edges {
            if !dominators.get(&block.at).is_some_and(|dominating| dominating.contains(&successor)) {
                continue;
            }
            if let Some(narrowed) = on_edge(parent, successor, &known, Some(&facts))? {
                known = narrowed;
            }
        }
        if !known.is_empty() {
            result.insert(block.at, known);
        }
    }
    Ok(result)
}

/// Every value `consts` knows, as the singleton interval an alias query reads.
pub(crate) fn constants(
    body: &Rc<MirBody>,
    dgroup: Option<&BTreeSet<i64>>,
    calls: Option<&IndexMap<i64, String>>,
) -> IndexMap<Value, Interval> {
    consts::known(body, dgroup, calls, None, None)
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
pub(crate) fn singletons(body: &MirBody) -> IndexMap<Value, Interval> {
    let mut known = IndexMap::<Value, Interval>::new();
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
                    && incoming.iter().collect::<std::collections::HashSet<_>>().len() == 1
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
                if let Some(interval) = _computed(op, &known, &IndexMap::new()) {
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
pub(crate) mod tests;
