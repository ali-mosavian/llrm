//! Exact value intervals for alias queries.
//!
//! Direct port of `qbopt.analysis.ranges:Interval` and `constants`, limited to
//! the `calls=None` path.  That Python invocation does not consume `dgroup`,
//! memory, or call facts: it converts each pure `consts.known` fact to the
//! singleton interval representing the same unsigned bits.

use std::collections::{BTreeMap, BTreeSet};

use indexmap::IndexMap;
use num_bigint::BigInt;

use super::{consts, induction, loops};
use crate::model::ir::Operation;
use crate::model::mir::{Arg, Const, Kind, MemRef, MirBlock, MirBody, Op, OpCode, Value, symbolic_ref};
use crate::objectfile::module::Space;

/// A non-wrapping mathematical interval at a fixed width.
///
/// Direct port of `qbopt.analysis.ranges:Interval`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Interval {
    pub low: BigInt,
    pub high: BigInt,
    pub width: u32,
}

/// Every value `constants` knows, as the singleton interval an alias query reads.
///
/// Direct port of `qbopt.analysis.ranges:constants(body, dgroup, calls=None)`.
/// The existing value-only `consts::known` is invoked exactly once; like the
/// Python `calls=None` path, this function adds no memory or call facts.
pub(crate) fn constants(body: &MirBody) -> BTreeMap<Value, Interval> {
    consts::known(body)
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
    facts: Option<&BTreeMap<Value, consts::Known>>,
) -> Result<Option<IndexMap<Value, Interval>>, String> {
    if !block.succ.contains(&successor) {
        return Err("not a successor".to_owned());
    }
    let empty = BTreeMap::new();
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
fn _operand(
    arg: &Arg,
    known: &IndexMap<Value, Interval>,
    facts: &BTreeMap<Value, consts::Known>,
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
                &BTreeMap::new(),
            );
        }
    }
    None
}

/// Direct port of `qbopt.analysis.ranges:_computed`.
fn _computed(
    op: &Op,
    known: &IndexMap<Value, Interval>,
    facts: &BTreeMap<Value, consts::Known>,
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
pub(crate) fn bounded(body: &MirBody) -> Result<IndexMap<i64, IndexMap<Value, Interval>>, String> {
    let facts = consts::known(body);
    let mut result: IndexMap<i64, IndexMap<Value, Interval>> = IndexMap::new();
    let predecessors = loops::predecessors(&body.blocks);
    let dominators = loops::dominators(&body.blocks, body.entry);
    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        let mut inside = loop_.body.clone();
        inside.remove(&loop_.header);
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
        for &at in &inside {
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
pub(crate) fn dominated_edges(body: &MirBody) -> Result<IndexMap<i64, IndexMap<Value, Interval>>, String> {
    let facts = consts::known(body);
    let predecessors = loops::predecessors(&body.blocks);
    let dominators = loops::dominators(&body.blocks, body.entry);
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

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::BTreeMap;

    use num_bigint::BigInt;

    use indexmap::IndexMap;

    use super::{Interval, _computed, _recurrence_span, bounded, constants, covering, on_edge};
    use crate::model::ir::Operation;
    use crate::model::mir::{OpCode, OrderedMap};
    use crate::model::mir::{
        Arg, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, Phi, Symbol, Value,
    };
    use crate::objectfile::module::{Addr, Space};
    use iced_x86::Register;

    fn value(id: u32, at: i64) -> Value {
        Value::new(id, at)
    }

    fn copy(at: i64, result: Value, constant: impl Into<BigInt>, width: u32) -> Op {
        let mut op = Op::new(at, None, "", vec![result], vec![]);
        op.kind = Kind::Copy;
        op.args = vec![Arg::Const(Const::new(constant, width))];
        op.results = vec![Arg::Held(Held {
            value: result,
            width,
        })];
        op
    }

    fn indexed(base: Value) -> MemRef {
        let mut address = Addr::new(Space::Segment, 4);
        address.index = 5;
        address.base = iced_x86::Register::AL;
        address.segment = iced_x86::Register::CL;
        let mut reference = MemRef::new(Some(address), 2);
        reference.base = Some(base);
        reference.base_width = 2;
        reference
    }

    fn known(
        base: Value,
        low: impl Into<BigInt>,
        high: impl Into<BigInt>,
        width: u32,
    ) -> BTreeMap<Value, Interval> {
        [(
            base,
            Interval {
                low: low.into(),
                high: high.into(),
                width,
            },
        )]
        .into_iter()
        .collect()
    }

    #[test]
    fn direct_ranges_covering_makes_a_static_byte_hull() {
        // `tests/test_ranges.py::test_range_alias_checks_cover_width_and_wrap`:
        // an index in 0..20 at displacement 4, reading two bytes, touches 4..26.
        let base = value(1, 0);
        let covered = covering(&indexed(base), &known(base, 0, 20, 2));

        assert_eq!(covered.addr.unwrap().disp, 4);
        assert_eq!(covered.addr.unwrap().space, Space::Segment);
        assert_eq!(covered.addr.unwrap().index, 5);
        assert_eq!(covered.addr.unwrap().base, iced_x86::Register::None);
        assert_eq!(covered.addr.unwrap().segment, iced_x86::Register::CL);
        assert_eq!(covered.base, None);
        assert_eq!(covered.width, 22);
    }

    #[test]
    fn direct_ranges_covering_refuses_negative_and_wrapping_hulls() {
        // The same Python matrix refuses an interval whose low byte is below
        // zero and one whose high byte plus access width wraps the word.
        let base = value(1, 0);
        let reference = indexed(base);

        assert_eq!(covering(&reference, &known(base, -8, 20, 2)), reference);
        assert_eq!(covering(&reference, &known(base, 0, 65_535, 2)), reference);
    }

    #[test]
    fn direct_ranges_covering_refuses_missing_wrong_and_mismatched_intervals() {
        let base = value(1, 0);
        let other = value(2, 0);
        let reference = indexed(base);

        assert_eq!(covering(&reference, &BTreeMap::new()), reference);
        assert_eq!(covering(&reference, &known(other, 0, 20, 2)), reference);
        assert_eq!(covering(&reference, &known(base, 0, 20, 4)), reference);

        let mut wide_base = reference.clone();
        wide_base.base_width = 4;
        assert_eq!(covering(&wide_base, &known(base, 0, 20, 4)), wide_base);
    }

    #[test]
    fn direct_ranges_covering_refuses_explicit_segments_and_wrong_spaces() {
        let base = value(1, 0);
        let reference = indexed(base);
        let interval = known(base, 0, 20, 2);

        let mut segmented = reference.clone();
        segmented.segment = Some(value(2, 0));
        assert_eq!(covering(&segmented, &interval), segmented);

        let mut framed = reference.clone();
        framed.addr.as_mut().unwrap().space = Space::Frame;
        assert_eq!(covering(&framed, &interval), framed);
    }

    #[test]
    fn direct_ranges_covering_normalizes_symbolic_references_first() {
        let base = value(1, 0);
        let segment = value(2, 0);
        let mut reference = indexed(base);
        reference.segment = Some(segment);
        reference.symbolic = Some(Symbol {
            space: Space::Segment,
            index: 7,
            offset: 8,
            width: 2,
            addend: 3,
        });

        let covered = covering(&reference, &known(base, 0, 20, 2));

        // Symbolic normalization clears `base` and the explicit MemRef
        // segment before the interval lookup, so the absent base refuses.
        assert_eq!(covered.addr.unwrap().space, Space::Segment);
        assert_eq!(covered.addr.unwrap().disp, 11);
        assert_eq!(covered.addr.unwrap().index, 7);
        assert_eq!(covered.base, None);
        assert_eq!(covered.segment, None);
        assert_eq!(covered.width, reference.width);
    }

    #[test]
    fn direct_ranges_constants_preserves_masked_unsigned_values_and_widths() {
        let word = value(1, 0);
        let dword = value(2, 0);
        let body = MirBody::new(
            0,
            vec![MirBlock::new(
                0,
                vec![],
                vec![copy(0, word, -1, 2), copy(0, dword, 0x1_0000_0001_u64, 4)],
                vec![],
            )],
        );

        let facts = constants(&body);

        assert_eq!(
            facts.get(&word),
            Some(&Interval {
                low: BigInt::from(0xffff_u32),
                high: BigInt::from(0xffff_u32),
                width: 2,
            })
        );
        assert_eq!(
            facts.get(&dword),
            Some(&Interval {
                low: BigInt::from(1_u8),
                high: BigInt::from(1_u8),
                width: 4,
            })
        );
    }

    #[test]
    fn direct_ranges_constants_carries_constant_cycle_results() {
        let (seed, joined, carried) = (value(1, 0), value(2, 10), value(3, 10));
        let mut update = Op::new(10, None, "", vec![carried], vec![joined]);
        update.kind = Kind::Add;
        update.args = vec![
            Arg::Held(Held {
                value: joined,
                width: 4,
            }),
            Arg::Const(Const::new(0, 4)),
        ];
        update.results = vec![Arg::Held(Held {
            value: carried,
            width: 4,
        })];
        let mut incoming = crate::model::mir::OrderedMap::new();
        incoming.insert(0, seed);
        incoming.insert(10, carried);
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], vec![copy(0, seed, 7, 4)], vec![10]),
                MirBlock::new(
                    10,
                    vec![Phi {
                        result: joined,
                        incoming,
                    }],
                    vec![update],
                    vec![10],
                ),
            ],
        );

        let facts = constants(&body);
        let expected = Interval {
            low: BigInt::from(7_u8),
            high: BigInt::from(7_u8),
            width: 4,
        };
        assert_eq!(facts.get(&joined), Some(&expected));
        assert_eq!(facts.get(&carried), Some(&expected));
    }

    fn interval(low: i64, high: i64, width: u32) -> Interval {
        Interval {
            low: low.into(),
            high: high.into(),
            width,
        }
    }

    fn operation(at: i64, operation: Operation, kind: Kind, defines: Vec<Value>, uses: Vec<Value>) -> Op {
        let mut made = Op::new(at, OpCode::Operation(operation), "", defines, uses);
        made.kind = kind;
        made
    }

    fn word(value: Value) -> Arg {
        Arg::Held(Held { value, width: 2 })
    }

    /// `tests/test_edge_ranges.py:guarded_loop`.
    pub(crate) fn guarded_loop() -> MirBody {
        let (start, counter, advanced, offset) = (value(1, 0), value(2, 10), value(3, 40), value(6, 30));
        let compare = |at: i64, bound: i64, yes: i64, no: i64| {
            let flags = Value {
                flags: true,
                ..value(u32::try_from(at + 10).unwrap(), at)
            };
            let mut test = operation(at, Operation::Compare, Kind::Sub, vec![flags], vec![counter]);
            test.args = vec![word(counter), Arg::Const(Const::new(bound, 2))];
            let mut branch = operation(at + 1, Operation::Branch, Kind::Branch, vec![], vec![flags]);
            branch.test = Some(Kind::Lt);
            branch.target = Some(yes);
            MirBlock::new(at, vec![], vec![test, branch], vec![yes, no])
        };
        let mut initial = operation(0, Operation::Move, Kind::Copy, vec![start], vec![]);
        initial.args = vec![Arg::Const(Const::new(0, 2))];
        initial.results = vec![word(start)];
        let mut header = compare(10, 10, 20, 50);
        let mut incoming = OrderedMap::new();
        incoming.insert(0, start);
        incoming.insert(40, advanced);
        header.phis = vec![Phi {
            result: counter,
            incoming,
        }];
        let mut scaled = operation(30, Operation::Binary, Kind::Mul, vec![offset], vec![counter]);
        scaled.args = vec![word(counter), Arg::Const(Const::new(2, 2))];
        scaled.results = vec![word(offset)];
        let mut step = operation(40, Operation::Unary, Kind::Increment, vec![advanced], vec![counter]);
        step.args = vec![word(counter)];
        step.results = vec![word(advanced)];
        MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], vec![initial], vec![10]),
                header,
                compare(20, 4, 30, 40),
                MirBlock::new(30, vec![], vec![scaled], vec![40]),
                MirBlock::new(40, vec![], vec![step], vec![10]),
                MirBlock::new(50, vec![], vec![], vec![]),
            ],
        )
    }

    #[test]
    fn test_guard_refines_subscript_without_leaking_to_the_join() {
        // `tests/test_edge_ranges.py`: i<4 bounds a word-array offset to 0..6, not the whole loop's 0..18.
        let body = guarded_loop();
        let counter = body.blocks[1].phis[0].result;
        let offset = value(6, 30);
        let known = bounded(&body).unwrap();

        assert_eq!(known[&30][&counter], interval(0, 3, 2));
        assert_eq!(known[&30][&offset], interval(0, 6, 2));
        assert_eq!(known[&40][&counter], interval(0, 9, 2));
        assert!(!known.get(&50).is_some_and(|facts| facts.contains_key(&counter)));
    }

    #[test]
    fn test_signed_comparison_edges() {
        // `tests/test_edge_ranges.py`, every parametrized case.
        for (test, successor, (low, high)) in [
            (Kind::Lt, 30, (0, 3)),
            (Kind::Lt, 40, (4, 9)),
            (Kind::Le, 30, (0, 4)),
            (Kind::Gt, 30, (5, 9)),
            (Kind::Ge, 30, (4, 9)),
            (Kind::Eq, 30, (4, 4)),
            (Kind::Ne, 40, (4, 4)),
            (Kind::Ne, 30, (0, 9)),
        ] {
            let mut block = guarded_loop().blocks[2].clone();
            let Arg::Held(held) = &block.ops[0].args[0] else {
                panic!("the comparison reads the counter");
            };
            let counter = held.value;
            block.ops[1].test = Some(test);
            let known = IndexMap::from([(counter, interval(0, 9, 2))]);

            let result = on_edge(&block, successor, &known, None).unwrap().unwrap();

            assert_eq!(result[&counter], interval(low, high, 2), "{test:?} to {successor}");
        }
    }

    #[test]
    fn test_non_comparison_flags_do_not_establish_a_bound() {
        // `tests/test_edge_ranges.py`, both parametrized cases.
        for (operation, kind) in [(Operation::Binary, Kind::Sub), (Operation::Compare, Kind::And)] {
            let mut block = guarded_loop().blocks[2].clone();
            let Arg::Held(held) = &block.ops[0].args[0] else {
                panic!("the comparison reads the counter");
            };
            let known = IndexMap::from([(held.value, interval(0, 9, 2))]);
            block.ops[0].op = Some(OpCode::Operation(operation));
            block.ops[0].kind = kind;

            assert_eq!(on_edge(&block, 30, &known, None).unwrap(), Some(known));
        }
    }

    fn unary(kind: Kind, operation_: Operation, result_width: u32, extra: Option<Arg>) -> (Op, Value) {
        let (source, result) = (value(1, 0), value(2, 0));
        let mut made = operation(0, operation_, kind, vec![result], vec![source]);
        made.args = [Some(word(source)), extra].into_iter().flatten().collect();
        made.results = vec![Arg::Held(Held {
            value: result,
            width: result_width,
        })];
        (made, source)
    }

    #[test]
    fn test_unit_steps_require_nonwrapping_intervals() {
        // `tests/test_ranges.py`, every parametrized case.
        for (kind, low, high, expected) in [
            (Kind::Decrement, 1, 3, Some(interval(0, 2, 2))),
            (Kind::Increment, -3, -1, Some(interval(-2, 0, 2))),
            (Kind::Decrement, -32768, 0, None),
            (Kind::Increment, 0, 32767, None),
        ] {
            let (made, source) = unary(kind, Operation::Unary, 2, None);
            let known = IndexMap::from([(source, interval(low, high, 2))]);
            assert_eq!(_computed(&made, &known, &BTreeMap::new()), expected);
        }
    }

    #[test]
    fn test_signed_widening_keeps_the_numeric_range() {
        // `tests/test_ranges.py`: ADDRM's bounded 1..20 counter lost its interval when converted to a long.
        for (low, high) in [(1, 20), (-32768, -1), (-10, 10)] {
            let (made, source) = unary(Kind::SignExtend, Operation::Extend, 4, None);
            let known = IndexMap::from([(source, interval(low, high, 2))]);
            assert_eq!(_computed(&made, &known, &BTreeMap::new()), Some(interval(low, high, 4)));
        }
    }

    #[test]
    fn test_secondary_recurrence_bounds_reject_wrap() {
        // `tests/test_ranges.py`, every parametrized case.
        for (start, step, advances, expected) in [
            (0, 4, 5, Some(interval(0, 20, 2))),
            (20, -4, 5, Some(interval(0, 20, 2))),
            (7, 0, 5, Some(interval(7, 7, 2))),
            (32760, 4, 1, None),
            (-32760, -4, 2, None),
            (0, 16384, 4, None),
            (0, 4, -1, None),
        ] {
            let found = _recurrence_span(&BigInt::from(start), &BigInt::from(step), &BigInt::from(advances), 2);
            assert_eq!(found, expected, "{start} {step} {advances}");
        }
    }

    #[test]
    fn test_shift_ranges_refuse_wraparound() {
        // `tests/test_ranges.py`, every parametrized case.
        for (low, high, count, expected) in [
            (0, 5, 2, Some(interval(0, 20, 2))),
            (-5, -1, 2, Some(interval(-20, -4, 2))),
            (0, 16384, 1, None),
            (-32768, -1, 1, None),
            (0, 5, 32, None),
        ] {
            let (made, source) = unary(Kind::Shl, Operation::Binary, 2, Some(Arg::Const(Const::new(count, 1))));
            let known = IndexMap::from([(source, interval(low, high, 2))]);
            assert_eq!(_computed(&made, &known, &BTreeMap::new()), expected, "{low} {high} {count}");
        }
    }
}
