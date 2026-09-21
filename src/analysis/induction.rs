//! Control-only facts for Python MIR induction analysis.
//!
//! Direct port of `qbopt.analysis.induction` loop-shape and affine-recurrence
//! facts: `LoopShape`, `Affine`, `AffineMap`, `canonical`, `invariant`,
//! `test_only`, `basics`, `_copied`, `_stepped`, and `relation`.  This module
//! deliberately does not invent a portable-IR loop abstraction: Python MIR
//! is the current stage contract.

use std::cmp::max;
use std::collections::{BTreeMap, BTreeSet};

use num_bigint::BigInt;

use super::constants::{self, masked, Known};
use super::occurrence::{operations, phis, OpOccurrence, PhiOccurrence};
use crate::model::mir::{Arg, Const, Held, Kind, MirBody, Op, OrderedMap, Value};
use crate::model::mir_loops::{predecessors, Loop};

/// The canonical pre-tested, single-latch loop CFG.
///
/// Direct port of `qbopt.analysis.induction:LoopShape`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LoopShape {
    pub preheader: i64,
    pub latch: i64,
    pub entered: i64,
    pub exit: i64,
}

/// Python's `mir.Held | mir.Const` affine operand union.
///
/// Direct port of the closed annotation on `induction.Affine.start` and
/// `induction.Affine.step`; no other MIR operand can be a recurrence term.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum AffineOperand {
    Held(Held),
    Const(Const),
}

impl AffineOperand {
    const fn width(&self) -> u32 {
        match self {
            Self::Held(held) => held.width,
            Self::Const(constant) => constant.width,
        }
    }

    fn as_arg(&self) -> Arg {
        match self {
            Self::Held(held) => Arg::Held(*held),
            Self::Const(constant) => Arg::Const(constant.clone()),
        }
    }
}

/// `start + step * iteration`, in the loop this was asked about.
///
/// Direct port of `qbopt.analysis.induction:Affine`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Affine {
    pub value: u32,
    pub start: AffineOperand,
    pub step: AffineOperand,
    pub header: i64,
}

/// A width-limited `scale * source + offset` relation.
///
/// Direct port of `qbopt.analysis.induction:AffineMap`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct AffineMap {
    pub scale: BigInt,
    pub offset: BigInt,
    pub width: u32,
}

/// A canonical zero-or-more loop with an exact symbolic trip-count bound.
///
/// Direct port of `qbopt.analysis.induction:CountedLoop`.  Python stores the
/// proven phi and operations by object identity; Rust stores snapshot-local
/// occurrence keys for the same exact body instead.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CountedLoop {
    pub counter: Affine,
    pub phi: PhiOccurrence,
    pub compare: OpOccurrence,
    pub branch: OpOccurrence,
    pub bound: AffineOperand,
    pub preheader: i64,
    pub latch: i64,
    pub entered: i64,
    pub exit: i64,
    pub maximum: Option<BigInt>,
}

impl CountedLoop {
    /// Python's `CountedLoop.trips` property.
    pub(crate) const fn trips(&self) -> &AffineOperand {
        &self.bound
    }
}

/// Proof that a counted loop's source recurrence may be removed.
///
/// Direct port of `qbopt.analysis.induction:ControlReplacement`.  The
/// counted-loop proof is borrowed, retaining Python's `is` relationship for
/// the consumer.  Operations and phis use snapshot-local occurrences rather
/// than `Op.id` or structural equality.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ControlReplacement<'a> {
    pub counted: &'a CountedLoop,
    pub stepping: OpOccurrence,
    pub update: Value,
    pub aliases: BTreeSet<Value>,
    pub copies: BTreeSet<OpOccurrence>,
}

/// Proof that an affine recurrence's update flags end counted control.
///
/// Direct port of `qbopt.analysis.induction:ZeroTerminatingControl`.  The
/// replacement borrows the exact counted-loop proof supplied by the caller,
/// as its Python counterpart retains that object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ZeroTerminatingControl<'a> {
    pub replacement: ControlReplacement<'a>,
    pub candidate: Affine,
    pub step: BigInt,
    pub maximum: BigInt,
    pub period: BigInt,
}

impl AffineMap {
    /// Python's `AffineMap.period` property.
    pub(crate) fn period(&self) -> BigInt {
        let modulus = BigInt::from(1_u8) << (self.width * 8);
        let scale = if self.scale < BigInt::from(0_u8) {
            -&self.scale
        } else {
            self.scale.clone()
        };
        modulus.clone() / gcd(scale, modulus)
    }

    /// Python's `AffineMap.injective(low, high)`.
    pub(crate) fn injective(&self, low: &BigInt, high: &BigInt) -> bool {
        self.scale != BigInt::from(0_u8) && high - low < self.period()
    }
}

/// Python's `relation(source, target, facts)`.
pub(crate) fn relation(
    source: &Affine,
    target: &Affine,
    facts: &BTreeMap<crate::model::mir::Value, Known>,
) -> Option<AffineMap> {
    let width = source.start.width();
    if target.start.width() != width {
        return None;
    }
    let source_start = _signed(&source.start.as_arg(), facts, width)?;
    let source_step = _signed(&source.step.as_arg(), facts, width)?;
    let target_start = _signed(&target.start.as_arg(), facts, width)?;
    let target_step = _signed(&target.step.as_arg(), facts, width)?;
    if source_step == BigInt::from(0_u8) || (&target_step % &source_step) != BigInt::from(0_u8) {
        return None;
    }
    // Divisibility was established immediately above, so Rust's truncating
    // division has the same result as Python's floor division here.
    let scale = &target_step / &source_step;
    if scale == BigInt::from(0_u8) {
        return None;
    }
    let offset = masked(&(target_start - &scale * source_start), width);
    Some(AffineMap {
        scale,
        offset,
        width,
    })
}

/// Python's default `counted(body, loop)` invocation.
pub(crate) fn counted(body: &MirBody, loop_: &Loop) -> Vec<CountedLoop> {
    let facts = constants::known(body);
    counted_with_facts(body, loop_, &facts)
}

/// Python's explicit-facts `counted(body, loop, facts)` form.
///
/// Pipeline consumers which already computed the immutable body's facts use
/// this form so several induction questions share one analysis result.
pub(crate) fn counted_with_facts(
    body: &MirBody,
    loop_: &Loop,
    facts: &BTreeMap<Value, Known>,
) -> Vec<CountedLoop> {
    // Python's `{block.at: block for block in body.blocks}` keeps the last
    // duplicate address; `canonical` and this semantic phase share it.
    let blocks = body
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.at, (index, block)))
        .collect::<BTreeMap<_, _>>();
    let Some(shape) = canonical(body, loop_) else {
        return Vec::new();
    };
    let (header_index, _) = blocks[&loop_.header];
    let header_operations = operations(body)
        .filter(|(occurrence, _, _)| occurrence.block_index() == header_index)
        .map(|(occurrence, _, operation)| (occurrence, operation))
        .collect::<Vec<_>>();
    let Some((branch_occurrence, branch)) = header_operations.last().copied() else {
        return Vec::new();
    };
    let inside = &loop_.body;
    if _continuing_test(branch, inside) != Some(Kind::Below) {
        return Vec::new();
    }
    let still = invariant(body, inside);
    let mut made = BTreeMap::<u32, &Op>::new();
    for block in &body.blocks {
        for operation in &block.ops {
            for value in &operation.defines {
                made.insert(value.id, operation);
            }
        }
    }
    let header_phis = phis(body)
        .filter(|(occurrence, _, _)| occurrence.block_index() == header_index)
        .map(|(occurrence, _, phi)| (occurrence, phi))
        .collect::<Vec<_>>();

    let mut proven = Vec::new();
    for counter in basics(body, loop_).values() {
        let width = counter.start.width();
        if _signed(&counter.start.as_arg(), facts, width) != Some(BigInt::from(0_u8))
            || _signed(&counter.step.as_arg(), facts, width) != Some(BigInt::from(1_u8))
        {
            continue;
        }
        let Some((phi_occurrence, phi)) = header_phis
            .iter()
            .find(|(_, phi)| phi.result.id == counter.value)
            .copied()
        else {
            continue;
        };
        if phi.incoming.keys().copied().collect::<BTreeSet<_>>()
            != BTreeSet::from([shape.preheader, shape.latch])
        {
            continue;
        }
        let comparisons = header_operations[..header_operations.len() - 1]
            .iter()
            .filter_map(|(occurrence, operation)| {
                _counter_bound(operation, branch, counter, width, Some(&made))
                    .map(|bound| (*occurrence, bound))
            })
            .collect::<Vec<_>>();
        if comparisons.len() != 1 {
            continue;
        }
        let (compare_occurrence, bound) = comparisons[0].clone();
        let bound = match bound {
            Arg::Held(held) if held.width == width => AffineOperand::Held(held),
            Arg::Const(constant) if constant.width == width => AffineOperand::Const(constant),
            _ => continue,
        };
        if matches!(&bound, AffineOperand::Held(held) if !still.contains(&held.value.id)) {
            continue;
        }
        let Some(update) = phi.incoming.get(&shape.latch).copied() else {
            continue;
        };
        let Some(stepping) = made.get(&update.id).copied() else {
            continue;
        };
        if crate::model::mir::stepping(stepping)
            != Some((
                Arg::Held(Held {
                    value: phi.result,
                    width,
                }),
                Arg::Const(Const::new(1, width)),
            ))
            || stepping.results
                != vec![Arg::Held(Held {
                    value: update,
                    width,
                })]
            || !stepping.loads.is_empty()
            || !stepping.stores.is_empty()
            || stepping.barrier()
            || !stepping.merges.is_empty()
        {
            continue;
        }
        let maximum = match &bound {
            AffineOperand::Const(constant) => Some(masked(&constant.n, constant.width)),
            AffineOperand::Held(held) => body.integer_ranges.get(&held.value).and_then(|range| {
                (range.width == held.width && range.low >= BigInt::from(0_u8))
                    .then(|| range.high.clone())
            }),
        };
        proven.push(CountedLoop {
            counter: counter.clone(),
            phi: phi_occurrence,
            compare: compare_occurrence,
            branch: branch_occurrence,
            bound,
            preheader: shape.preheader,
            latch: shape.latch,
            entered: shape.entered,
            exit: shape.exit,
            maximum,
        });
    }
    proven
}

/// Python's `_continuing_test(branch, inside)`.
fn _continuing_test(branch: &Op, inside: &BTreeSet<i64>) -> Option<Kind> {
    if branch.target.is_some_and(|target| inside.contains(&target)) {
        return branch.test;
    }
    match branch.test {
        Some(Kind::Le) => Some(Kind::Gt),
        Some(Kind::Lt) => Some(Kind::Ge),
        Some(Kind::Ge) => Some(Kind::Lt),
        Some(Kind::Gt) => Some(Kind::Le),
        Some(Kind::Below) => Some(Kind::AboveEq),
        Some(Kind::BelowEq) => Some(Kind::Above),
        Some(Kind::Above) => Some(Kind::BelowEq),
        Some(Kind::AboveEq) => Some(Kind::Below),
        Some(Kind::Eq) => Some(Kind::Ne),
        Some(Kind::Ne) => Some(Kind::Eq),
        _ => None,
    }
}

/// Python's `_counter_bound(op, branch, counter, width, made=None)`.
///
/// This crate-visible exact spelling is shared by the trip proofs and their
/// transform consumers. It returns the original MIR operand: recognizing a
/// comparison is analysis, while choosing what to do with it remains the
/// caller's job.
pub(crate) fn _counter_bound(
    op: &Op,
    branch: &Op,
    counter: &Affine,
    width: u32,
    made: Option<&BTreeMap<u32, &Op>>,
) -> Option<Arg> {
    let first = match op.args.first() {
        Some(Arg::Held(held)) if op.args.len() == 2 && held.width == width => *held,
        _ => return None,
    };
    if !op.loads.is_empty() || !op.stores.is_empty() || op.barrier() {
        return None;
    }
    let compared = made.map_or(first, |made| _copied(first, made));
    if compared.value.id != counter.value {
        return None;
    }
    let flags = op
        .defines
        .iter()
        .filter(|value| value.flags)
        .copied()
        .collect::<Vec<_>>();
    if flags.len() != 1 || !branch.uses.contains(&flags[0]) {
        return None;
    }
    if op.kind == Kind::Sub && op.results.is_empty() && op.defines.len() == 1 {
        return Some(op.args[1].clone());
    }
    if matches!(op.kind, Kind::And | Kind::Or) && op.args[0] == op.args[1] {
        return Some(Arg::Const(Const::new(0, width)));
    }
    None
}

/// Python's `_constant(arg, facts, width)`.
fn _constant(
    argument: &Arg,
    facts: &BTreeMap<crate::model::mir::Value, Known>,
    width: u32,
) -> Option<BigInt> {
    let argument_width = match argument {
        Arg::Held(held) => held.width,
        Arg::Const(constant) => constant.width,
        _ => return None,
    };
    if argument_width != width {
        return None;
    }
    let (number, fact_width) = match argument {
        Arg::Held(held) => {
            let fact = facts.get(&held.value)?;
            (&fact.n, fact.width)
        }
        Arg::Const(constant) => (&constant.n, constant.width),
        _ => return None,
    };
    if fact_width < width {
        return None;
    }
    Some(masked(number, width))
}

/// Python's `_as_signed(value, width)`.
fn _as_signed(value: &BigInt, width: u32) -> BigInt {
    let sign = BigInt::from(1_u8) << (width * 8 - 1);
    (value ^ &sign) - sign
}

/// Python's `_signed(arg, facts, width)`.
fn _signed(
    argument: &Arg,
    facts: &BTreeMap<crate::model::mir::Value, Known>,
    width: u32,
) -> Option<BigInt> {
    let argument_width = match argument {
        Arg::Held(held) => held.width,
        Arg::Const(constant) => constant.width,
        _ => return None,
    };
    if argument_width != width {
        return None;
    }
    _constant(argument, facts, width).map(|value| _as_signed(&value, width))
}

/// Python's `_last_counter(body, loop, counter, facts, width)`.
///
/// The comparison's continuation is normalized by `_continuing_test`, so the
/// arithmetic below is deliberately the Python case split rather than a
/// generalized range solver.  It proves the update after the last observed
/// value remains representable; an otherwise plausible wrapping recurrence is
/// not a finite loop proof.
fn _last_counter(
    body: &MirBody,
    loop_: &Loop,
    counter: &Affine,
    facts: &BTreeMap<Value, Known>,
    width: u32,
) -> Option<BigInt> {
    if let Some(posttested) = _posttested_last(body, loop_, counter, facts, width) {
        return Some(posttested);
    }
    let blocks = body
        .blocks
        .iter()
        .map(|block| (block.at, block))
        .collect::<BTreeMap<_, _>>();
    if loop_.latches.len() != 1 {
        return None;
    }
    let header = *blocks.get(&loop_.header)?;
    let latch = *blocks.get(loop_.latches.first()?)?;
    if latch.succ.as_slice() != [header.at] || header.ops.is_empty() || header.succ.len() != 2 {
        return None;
    }
    let branch = header.ops.last()?;
    if branch.kind != Kind::Branch
        || !branch
            .target
            .is_some_and(|target| header.succ.contains(&target))
    {
        return None;
    }
    let inside = &loop_.body;
    if inside.iter().filter(|at| **at != header.at).any(|at| {
        let block = blocks[at];
        block.succ.is_empty() || block.succ.iter().any(|to| !inside.contains(to))
    }) || header.succ.iter().filter(|to| inside.contains(to)).count() != 1
    {
        return None;
    }
    let test = _continuing_test(branch, inside)?;
    let mut made = BTreeMap::<u32, &Op>::new();
    for block in &body.blocks {
        for operation in &block.ops {
            for value in &operation.defines {
                made.insert(value.id, operation);
            }
        }
    }
    let comparisons = header.ops[..header.ops.len() - 1]
        .iter()
        .filter_map(|operation| _counter_bound(operation, branch, counter, width, Some(&made)))
        .collect::<Vec<_>>();
    if comparisons.len() != 1 {
        return None;
    }
    let raw_start = _constant(&counter.start.as_arg(), facts, width)?;
    let raw_step = _constant(&counter.step.as_arg(), facts, width)?;
    let raw_bound = _constant(&comparisons[0], facts, width)?;
    let step = _as_signed(&raw_step, width);
    if step == BigInt::from(0_u8) {
        return None;
    }
    let unsigned = matches!(
        test,
        Kind::Below | Kind::BelowEq | Kind::Above | Kind::AboveEq
    );
    let start = if unsigned {
        raw_start
    } else {
        _as_signed(&raw_start, width)
    };
    let bound = if unsigned {
        raw_bound
    } else {
        _as_signed(&raw_bound, width)
    };
    let distance = if step > BigInt::from(0_u8)
        && matches!(test, Kind::Le | Kind::Lt | Kind::BelowEq | Kind::Below)
    {
        let limit = match test {
            Kind::Lt | Kind::Below => &bound - 1_u8,
            _ => bound.clone(),
        };
        limit - &start
    } else if step < BigInt::from(0_u8)
        && matches!(test, Kind::Ge | Kind::Gt | Kind::AboveEq | Kind::Above)
    {
        let limit = match test {
            Kind::Gt | Kind::Above => &bound + 1_u8,
            _ => bound.clone(),
        };
        &start - limit
    } else if test == Kind::Ne
        && (&bound - &start) * &step > BigInt::from(0_u8)
        && (&bound - &start) % &step == BigInt::from(0_u8)
    {
        let difference = &bound - &start;
        abs(&difference) - abs(&step)
    } else {
        return None;
    };
    if distance < BigInt::from(0_u8) {
        return None;
    }
    let last = &start + (&distance / abs(&step)) * &step;
    let after = &last + &step;
    if unsigned {
        let limit = BigInt::from(1_u8) << (width * 8);
        (after >= BigInt::from(0_u8) && after < limit).then_some(last)
    } else {
        let sign = BigInt::from(1_u8) << (width * 8 - 1);
        (after >= -&sign && after < sign).then_some(last)
    }
}

/// Python's `_posttested_bound(op, branch, counter, width, made)`.
fn _posttested_bound(
    op: &Op,
    branch: &Op,
    counter: &Affine,
    width: u32,
    made: &BTreeMap<u32, &Op>,
) -> Option<(Arg, Const)> {
    let following = match op.args.first() {
        Some(Arg::Held(held)) if op.args.len() == 2 && held.width == width => *held,
        _ => return None,
    };
    if op.kind != Kind::Sub
        || !op.loads.is_empty()
        || !op.stores.is_empty()
        || op.barrier()
        || !op.results.is_empty()
        || op.defines.len() != 1
        || !op
            .defines
            .iter()
            .any(|value| value.flags && branch.uses.contains(value))
    {
        return None;
    }
    let definition = made.get(&following.value.id).copied()?;
    if !definition.loads.is_empty()
        || !definition.stores.is_empty()
        || definition.barrier()
        || !definition.merges.is_empty()
        || !definition.results.contains(&Arg::Held(following))
    {
        return None;
    }
    let (source, delta) = crate::model::mir::stepping(definition)?;
    let (Arg::Held(source), Arg::Const(delta)) = (source, delta) else {
        return None;
    };
    if source.width != width
        || delta.width != width
        || _copied(source, made).value.id != counter.value
    {
        return None;
    }
    Some((op.args[1].clone(), delta))
}

/// Python's `_posttested_last(body, loop, counter, facts, width)`.
fn _posttested_last(
    body: &MirBody,
    loop_: &Loop,
    counter: &Affine,
    facts: &BTreeMap<Value, Known>,
    width: u32,
) -> Option<BigInt> {
    let blocks = body
        .blocks
        .iter()
        .map(|block| (block.at, block))
        .collect::<BTreeMap<_, _>>();
    if loop_.latches.len() != 1 {
        return None;
    }
    let latch = *blocks.get(loop_.latches.first()?)?;
    let inside = &loop_.body;
    if latch.succ.len() != 2
        || !latch.succ.contains(&loop_.header)
        || latch.succ.iter().filter(|to| inside.contains(to)).count() != 1
        || latch.ops.is_empty()
    {
        return None;
    }
    let branch = latch.ops.last()?;
    if branch.kind != Kind::Branch
        || !branch
            .target
            .is_some_and(|target| latch.succ.contains(&target))
    {
        return None;
    }
    if inside
        .iter()
        .filter(|at| **at != latch.at)
        .any(|at| blocks[at].succ.iter().any(|to| !inside.contains(to)))
    {
        return None;
    }
    let test = _continuing_test(branch, inside)?;
    let mut made = BTreeMap::<u32, &Op>::new();
    for block in &body.blocks {
        for operation in &block.ops {
            for value in &operation.defines {
                made.insert(value.id, operation);
            }
        }
    }
    let comparisons = latch.ops[..latch.ops.len() - 1]
        .iter()
        .filter_map(|operation| _posttested_bound(operation, branch, counter, width, &made))
        .collect::<Vec<_>>();
    if comparisons.len() != 1 {
        return None;
    }
    let (bound_arg, after_step) = &comparisons[0];
    let raw_start = _constant(&counter.start.as_arg(), facts, width)?;
    let raw_step = _constant(&counter.step.as_arg(), facts, width)?;
    let raw_bound = _constant(bound_arg, facts, width)?;
    let raw_after = _constant(&Arg::Const(after_step.clone()), facts, width)?;
    let step = _as_signed(&raw_step, width);
    let after = _as_signed(&raw_after, width);
    if step == BigInt::from(0_u8) || after != step {
        return None;
    }
    let unsigned = matches!(
        test,
        Kind::Below | Kind::BelowEq | Kind::Above | Kind::AboveEq
    );
    let start = if unsigned {
        raw_start
    } else {
        _as_signed(&raw_start, width)
    };
    let bound = if unsigned {
        raw_bound
    } else {
        _as_signed(&raw_bound, width)
    };
    let first = &start + &step;
    let count = if step > BigInt::from(0_u8) && matches!(test, Kind::Lt | Kind::Below) {
        if first <= bound {
            max(BigInt::from(1_u8), (&bound - &first) / &step + 1_u8)
        } else {
            BigInt::from(1_u8)
        }
    } else if step > BigInt::from(0_u8) && matches!(test, Kind::Le | Kind::BelowEq) {
        if first <= bound {
            max(BigInt::from(1_u8), (&bound - &first) / &step + 2_u8)
        } else {
            BigInt::from(1_u8)
        }
    } else if step < BigInt::from(0_u8) && matches!(test, Kind::Gt | Kind::Above) {
        if first >= bound {
            max(BigInt::from(1_u8), (&first - &bound) / -&step + 1_u8)
        } else {
            BigInt::from(1_u8)
        }
    } else if step < BigInt::from(0_u8) && matches!(test, Kind::Ge | Kind::AboveEq) {
        if first >= bound {
            max(BigInt::from(1_u8), (&first - &bound) / -&step + 2_u8)
        } else {
            BigInt::from(1_u8)
        }
    } else if test == Kind::Ne
        && (&bound - &start) * &step > BigInt::from(0_u8)
        && (&bound - &start) % &step == BigInt::from(0_u8)
    {
        (&bound - &start) / &step
    } else {
        return None;
    };
    if count <= BigInt::from(0_u8) {
        return None;
    }
    let last = &start + (&count - 1_u8) * &step;
    let next_value = &last + &step;
    if unsigned {
        let limit = BigInt::from(1_u8) << (width * 8);
        (last >= BigInt::from(0_u8)
            && last < limit
            && next_value >= BigInt::from(0_u8)
            && next_value < limit)
            .then_some(last)
    } else {
        let sign = BigInt::from(1_u8) << (width * 8 - 1);
        (last >= -&sign && last < sign && next_value >= -&sign && next_value < sign).then_some(last)
    }
}

/// Python's `trip_count(body, loop, facts)`.
pub(crate) fn trip_count(
    body: &MirBody,
    loop_: &Loop,
    facts: &BTreeMap<Value, Known>,
) -> Option<BigInt> {
    let mut counts = BTreeSet::new();
    for counter in basics(body, loop_).values() {
        let width = counter.start.width();
        let start = _signed(&counter.start.as_arg(), facts, width);
        let step = _signed(&counter.step.as_arg(), facts, width);
        let last = _last_counter(body, loop_, counter, facts, width);
        if let (Some(start), Some(step), Some(last)) = (start, step, last) {
            if step != BigInt::from(0_u8) {
                let distance = &last - &start;
                if &distance % &step == BigInt::from(0_u8) {
                    let count = distance / step + 1_u8;
                    if count > BigInt::from(0_u8) {
                        counts.insert(count);
                    }
                }
            }
        }
        if let Some(symbolic) = _sentinel_trip_count(body, loop_, counter, facts, width) {
            counts.insert(symbolic);
        }
    }
    if counts.len() != 1 {
        if !counts.is_empty() {
            return None;
        }
        // Python's `dict(body.loop_trip_counts)` preserves the last duplicate
        // header fact; reproduce that source-order overwrite explicitly.
        return body
            .loop_trip_counts
            .iter()
            .filter_map(|(header, count)| (*header == loop_.header).then_some(BigInt::from(*count)))
            .last();
    }
    counts.into_iter().next()
}

/// Python's `_sentinel_trip_count(body, loop, counter, facts, width)`.
fn _sentinel_trip_count(
    body: &MirBody,
    loop_: &Loop,
    counter: &Affine,
    facts: &BTreeMap<Value, Known>,
    width: u32,
) -> Option<BigInt> {
    let blocks = body
        .blocks
        .iter()
        .map(|block| (block.at, block))
        .collect::<BTreeMap<_, _>>();
    if loop_.latches.len() != 1 {
        return None;
    }
    let header = *blocks.get(&loop_.header)?;
    let latch = *blocks.get(loop_.latches.first()?)?;
    let inside = &loop_.body;
    let (control, posttested) = if latch.succ.as_slice() == [header.at] && header.succ.len() == 2 {
        (header, false)
    } else if latch.succ.len() == 2 && latch.succ.contains(&header.at) {
        (latch, true)
    } else {
        return None;
    };
    if control.ops.is_empty() || control.succ.iter().filter(|to| inside.contains(to)).count() != 1 {
        return None;
    }
    let branch = control.ops.last()?;
    if branch.kind != Kind::Branch
        || !branch
            .target
            .is_some_and(|target| control.succ.contains(&target))
    {
        return None;
    }
    if inside.iter().filter(|at| **at != control.at).any(|at| {
        let block = blocks[at];
        block.succ.is_empty() || block.succ.iter().any(|to| !inside.contains(to))
    }) {
        return None;
    }
    if _continuing_test(branch, inside) != Some(Kind::Ne) {
        return None;
    }
    let mut made = BTreeMap::<u32, &Op>::new();
    let mut owners = BTreeMap::<u32, i64>::new();
    for block in &body.blocks {
        for operation in &block.ops {
            for value in &operation.defines {
                made.insert(value.id, operation);
                owners.insert(value.id, block.at);
            }
        }
    }
    let (bound, raw_after) = if posttested {
        let compared = control.ops[..control.ops.len() - 1]
            .iter()
            .filter_map(|operation| _posttested_bound(operation, branch, counter, width, &made))
            .collect::<Vec<_>>();
        if compared.len() != 1 {
            return None;
        }
        let (bound, after) = compared.into_iter().next()?;
        (bound, _constant(&Arg::Const(after), facts, width))
    } else {
        let compared = control.ops[..control.ops.len() - 1]
            .iter()
            .filter_map(|operation| _counter_bound(operation, branch, counter, width, Some(&made)))
            .collect::<Vec<_>>();
        if compared.len() != 1 {
            return None;
        }
        (compared.into_iter().next()?, None)
    };
    let Arg::Held(bound) = bound else {
        return None;
    };
    let definition = made.get(&bound.value.id).copied()?;
    if owners
        .get(&bound.value.id)
        .is_some_and(|owner| inside.contains(owner))
        || definition.kind != Kind::Add
        || !definition.loads.is_empty()
        || !definition.stores.is_empty()
        || definition.barrier()
        || !definition.merges.is_empty()
        || definition.args.len() != 2
        || definition.results.len() != 1
        || definition.results[0] != Arg::Held(bound)
    {
        return None;
    }
    let starts = definition
        .args
        .iter()
        .filter(|argument| **argument == counter.start.as_arg())
        .collect::<Vec<_>>();
    let offsets = definition
        .args
        .iter()
        .filter(|argument| **argument != counter.start.as_arg())
        .collect::<Vec<_>>();
    if starts.len() != 1 || offsets.len() != 1 {
        return None;
    }
    let raw_step = _constant(&counter.step.as_arg(), facts, width)?;
    let delta = _constant(offsets[0], facts, width)?;
    if raw_step == BigInt::from(0_u8) || (posttested && raw_after != Some(raw_step.clone())) {
        return None;
    }
    let modulus = BigInt::from(1_u8) << (8 * width);
    let divisor = gcd(raw_step.clone(), modulus.clone());
    if &delta % &divisor != BigInt::from(0_u8) {
        return None;
    }
    let period = &modulus / &divisor;
    let count = mod_floor(
        &(floor_div(&delta, &divisor) * modular_inverse(&floor_div(&raw_step, &divisor), &period)?),
        &period,
    );
    (count != BigInt::from(0_u8)).then_some(count)
}

/// Python's `nonempty(body, loop)`.
pub(crate) fn nonempty(body: &MirBody, loop_: &Loop) -> bool {
    let facts = constants::known(body);
    trip_count(body, loop_, &facts).is_some()
}

fn abs(value: &BigInt) -> BigInt {
    if value < &BigInt::from(0_u8) {
        -value
    } else {
        value.clone()
    }
}

/// Python floor division for a nonzero divisor.  `BigInt` truncates toward
/// zero, while the sentinel modular proof uses Python's `//` for negatives.
fn floor_div(numerator: &BigInt, denominator: &BigInt) -> BigInt {
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    if remainder != BigInt::from(0_u8)
        && ((remainder < BigInt::from(0_u8)) != (denominator < &BigInt::from(0_u8)))
    {
        quotient - 1_u8
    } else {
        quotient
    }
}

/// Python's modulo sign rule, used with the positive modular period.
fn mod_floor(value: &BigInt, modulus: &BigInt) -> BigInt {
    let remainder = value % modulus;
    if remainder < BigInt::from(0_u8) {
        remainder + modulus
    } else {
        remainder
    }
}

/// The exact local equivalent of Python `pow(value, -1, modulus)`.
fn modular_inverse(value: &BigInt, modulus: &BigInt) -> Option<BigInt> {
    let mut old_r = mod_floor(value, modulus);
    let mut r = modulus.clone();
    let mut old_s = BigInt::from(1_u8);
    let mut s = BigInt::from(0_u8);
    while r != BigInt::from(0_u8) {
        let quotient = &old_r / &r;
        let next_r = &old_r - &quotient * &r;
        old_r = r;
        r = next_r;
        let next_s = &old_s - &quotient * &s;
        old_s = s;
        s = next_s;
    }
    (old_r == BigInt::from(1_u8)).then(|| mod_floor(&old_s, modulus))
}

fn gcd(mut one: BigInt, mut other: BigInt) -> BigInt {
    while other != BigInt::from(0_u8) {
        let remainder = one % &other;
        one = other;
        other = remainder;
    }
    one
}

/// Python's `canonical(body, loop)`.
///
/// This keeps every structural refusal in the Python order.  In particular,
/// Python's final `blocks[at]` lookup is deliberately not softened: an
/// unknown non-header loop-body address is an invalid hand-built shape and
/// raises rather than becoming an ordinary structural refusal.
pub(crate) fn canonical(body: &MirBody, loop_: &Loop) -> Option<LoopShape> {
    // Python's dict comprehension retains the last duplicate address.
    let blocks = body
        .blocks
        .iter()
        .map(|block| (block.at, block))
        .collect::<BTreeMap<_, _>>();
    if loop_.latches.len() != 1 || !blocks.contains_key(&loop_.header) {
        return None;
    }
    let latch_at = *loop_.latches.first()?;
    let latch = blocks.get(&latch_at);
    let header = blocks.get(&loop_.header)?;
    let inside = &loop_.body;
    let predecessors = predecessors(&body.blocks);
    let outside = predecessors
        .get(&header.at)
        .into_iter()
        .flatten()
        .copied()
        .filter(|at| !inside.contains(at))
        .collect::<Vec<_>>();
    let entered = header
        .succ
        .iter()
        .copied()
        .filter(|at| inside.contains(at) && *at != header.at)
        .collect::<Vec<_>>();
    let exits = header
        .succ
        .iter()
        .copied()
        .filter(|at| !inside.contains(at))
        .collect::<Vec<_>>();

    let latch = latch?;
    if outside.len() != 1
        || blocks.get(&outside[0])?.succ.as_slice() != [header.at]
        || latch.succ.as_slice() != [header.at]
        || entered.len() != 1
        || exits.len() != 1
        || header.ops.is_empty()
        || header.ops.last()?.kind != Kind::Branch
        || inside
            .iter()
            .filter(|at| **at != header.at)
            .any(|at| blocks[at].succ.iter().any(|to| !inside.contains(to)))
    {
        return None;
    }
    Some(LoopShape {
        preheader: outside[0],
        latch: latch_at,
        entered: entered[0],
        exit: exits[0],
    })
}

/// Python's `test_only(op)`.
pub(crate) fn test_only(op: &Op) -> bool {
    if op.kind == Kind::Nothing
        && op.name.is_empty()
        && op.defines.is_empty()
        && op.uses.is_empty()
        && op.args.is_empty()
        && op.results.is_empty()
        && op.loads.is_empty()
        && op.stores.is_empty()
        && op.merges.is_empty()
        && !op.barrier()
        && op.floating.is_none()
        && op.stack.is_none()
        && op.floating_origin.is_none()
    {
        return true;
    }
    matches!(op.kind, Kind::Sub | Kind::And | Kind::Or)
        && op.results.is_empty()
        && op.loads.is_empty()
        && op.stores.is_empty()
        && op.merges.is_empty()
        && !op.barrier()
        && op.floating.is_none()
        && op.stack.is_none()
        && op.floating_origin.is_none()
        && !op.defines.is_empty()
        && op.defines.iter().all(|value| value.flags)
}

/// Python's `invariant(body, inside)`.
pub(crate) fn invariant(body: &MirBody, inside: &BTreeSet<i64>) -> BTreeSet<u32> {
    let mut written = BTreeSet::new();
    for block in &body.blocks {
        if !inside.contains(&block.at) {
            continue;
        }
        written.extend(
            block
                .ops
                .iter()
                .flat_map(|op| op.defines.iter().map(|value| value.id)),
        );
        written.extend(block.phis.iter().map(|phi| phi.result.id));
    }
    body.blocks
        .iter()
        .flat_map(|block| block.ops.iter())
        .flat_map(|op| {
            op.defines
                .iter()
                .chain(op.uses.iter())
                .map(|value| value.id)
        })
        .filter(|value| !written.contains(value))
        .collect()
}

/// Python's `basics(body, loop)`.
pub(crate) fn basics(body: &MirBody, loop_: &Loop) -> OrderedMap<u32, Affine> {
    // Python's `{block.at: block for block in body.blocks}` retains the last
    // duplicate address.  All later `at_of` reads use that exact view.
    let at_of = body
        .blocks
        .iter()
        .map(|block| (block.at, block))
        .collect::<BTreeMap<_, _>>();
    let inside = loop_
        .body
        .iter()
        .copied()
        .filter(|at| at_of.contains_key(at))
        .collect::<BTreeSet<_>>();
    let Some(header) = at_of.get(&loop_.header).copied() else {
        return OrderedMap::new();
    };
    let still = invariant(body, &inside);
    let mut made = BTreeMap::<u32, &Op>::new();
    for at in &inside {
        for operation in &at_of[at].ops {
            for value in &operation.defines {
                made.insert(value.id, operation);
            }
        }
    }

    let mut out = OrderedMap::new();
    for phi in &header.phis {
        let starts = phi
            .incoming
            .iter()
            .filter_map(|(where_, value)| (!inside.contains(where_)).then_some(*value))
            .collect::<Vec<_>>();
        if starts.is_empty() || starts.iter().any(|value| *value != starts[0]) {
            continue;
        }
        let mut steps = Vec::<Option<AffineOperand>>::new();
        let mut widths = BTreeSet::new();
        for (where_, value) in phi.incoming.iter() {
            if !inside.contains(where_) {
                continue;
            }
            let results = made
                .get(&value.id)
                .into_iter()
                .flat_map(|definition| definition.results.iter())
                .filter_map(|argument| match argument {
                    Arg::Held(held) if held.value == *value => Some(*held),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if results.len() != 1 {
                steps.push(None);
                continue;
            }
            let width = results[0].width;
            widths.insert(width);
            let root = _copied(results[0], &made);
            let step = _stepped(
                made.get(&root.value.id).copied(),
                phi.result.id,
                &still,
                &made,
            );
            steps.push(step.filter(|step| step.width() == width));
        }
        if widths.len() == 1
            && !steps.is_empty()
            && steps[0].is_some()
            && steps.iter().all(|step| step == &steps[0])
        {
            let width = *widths.first().expect("one Python recurrence width");
            out.insert(
                phi.result.id,
                Affine {
                    value: phi.result.id,
                    start: AffineOperand::Held(Held {
                        value: starts[0],
                        width,
                    }),
                    step: steps[0].clone().expect("checked above"),
                    header: loop_.header,
                },
            );
        }
    }
    out
}

/// Python's `_copied(operand, made)`.
fn _copied(mut operand: Held, made: &BTreeMap<u32, &Op>) -> Held {
    let mut seen = BTreeSet::new();
    while !seen.contains(&operand.value.id) {
        seen.insert(operand.value.id);
        let Some(op) = made.get(&operand.value.id).copied() else {
            break;
        };
        if op.kind != Kind::Copy || !op.loads.is_empty() || !op.stores.is_empty() {
            break;
        }
        if op.args.len() != 1 || op.results.len() != 1 {
            break;
        }
        let (Arg::Held(source), Arg::Held(result)) = (&op.args[0], &op.results[0]) else {
            break;
        };
        if source.width != operand.width || result.width != operand.width {
            break;
        }
        operand = *source;
    }
    operand
}

/// Python's `_stepped(op, value, still, made)`.
fn _stepped(
    op: Option<&Op>,
    value: u32,
    still: &BTreeSet<u32>,
    made: &BTreeMap<u32, &Op>,
) -> Option<AffineOperand> {
    let op = op?;
    let (mut stepped, mut step) = crate::model::mir::stepping(op)?;
    if let Arg::Held(held) = stepped {
        stepped = Arg::Held(_copied(held, made));
    }
    if let Arg::Held(held) = step {
        step = Arg::Held(_copied(held, made));
    }
    if !matches!(&stepped, Arg::Held(held) if held.value.id == value) {
        if matches!(&step, Arg::Held(held) if held.value.id == value) {
            std::mem::swap(&mut stepped, &mut step);
        } else {
            return None;
        }
    }
    match step {
        Arg::Const(constant) => Some(AffineOperand::Const(constant)),
        Arg::Held(held) if still.contains(&held.value.id) => Some(AffineOperand::Held(held)),
        _ => None,
    }
}

/// Python's `transparent_aliases(body, loop, source)`.
///
/// The returned occurrence keys are valid only for this exact immutable body
/// snapshot.  A transform constructs a new body after consuming them and
/// must rerun this analysis before asking about that successor body.
pub(crate) fn transparent_aliases(
    body: &MirBody,
    loop_: &Loop,
    source: Value,
) -> (BTreeSet<Value>, BTreeSet<OpOccurrence>) {
    let mut aliases = BTreeSet::from([source]);
    let mut copies = BTreeSet::new();
    let mut changed = true;
    while changed {
        changed = false;
        for (occurrence, block, operation) in operations(body) {
            if !loop_.body.contains(&block.at)
                || operation.kind != Kind::Copy
                || operation.args.len() != 1
                || operation.results.len() != 1
            {
                continue;
            }
            let (Arg::Held(argument), Arg::Held(result)) =
                (&operation.args[0], &operation.results[0])
            else {
                continue;
            };
            if !aliases.contains(&argument.value) || result.width != argument.width {
                continue;
            }
            copies.insert(occurrence);
            changed |= aliases.insert(result.value);
        }
    }
    (aliases, copies)
}

/// Python's `control_replacement(body, loop, proof, covered=frozenset())`.
///
/// This is a proof, not a transform: the caller supplies precisely the
/// operation occurrences it will replace, and this analysis establishes that
/// those, canonical control, and transparent copies are every observation of
/// the control recurrence.  Occurrences belong to `body`'s immutable
/// snapshot, just as Python's `id(op)` values belong to its object graph.
pub(crate) fn control_replacement<'a>(
    body: &MirBody,
    loop_: &Loop,
    proof: &'a CountedLoop,
    covered: &BTreeSet<OpOccurrence>,
) -> Option<ControlReplacement<'a>> {
    // Python's address map retains its last duplicate.  Occurrence keys made
    // by `counted` identify that same snapshot occurrence.
    let blocks = body
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.at, (index, block)))
        .collect::<BTreeMap<_, _>>();
    let predecessors = predecessors(&body.blocks);
    let (header_index, header) = blocks.get(&loop_.header).copied()?;
    let (_, latch) = blocks.get(&proof.latch).copied()?;
    if loop_.body.len() != 2
        || proof.entered != proof.latch
        || !latch.phis.is_empty()
        || predecessors.get(&latch.at) != Some(&BTreeSet::from([header.at]))
        || operations(body).any(|(occurrence, _, operation)| {
            occurrence.block_index() == header_index
                && occurrence != proof.compare
                && occurrence != proof.branch
                && !test_only(operation)
        })
    {
        return None;
    }

    let mut made = BTreeMap::<u32, OpOccurrence>::new();
    for (occurrence, _, operation) in operations(body) {
        for value in &operation.defines {
            made.insert(value.id, occurrence);
        }
    }
    let phi =
        phis(body).find_map(|(occurrence, _, phi)| (occurrence == proof.phi).then_some(phi))?;
    let update = *phi.incoming.get(&proof.latch)?;
    let stepping = *made.get(&update.id)?;
    let (aliases, copies) = transparent_aliases(body, loop_, phi.result);
    let mut allowed = covered.clone();
    allowed.extend(copies.iter().copied());
    allowed.insert(proof.compare);
    allowed.insert(stepping);
    if operations(body).any(|(occurrence, _, operation)| {
        (operation.uses.iter().any(|value| aliases.contains(value))
            && !allowed.contains(&occurrence))
            || operation.uses.contains(&update)
    }) {
        return None;
    }
    if phis(body).any(|(occurrence, _, other)| {
        occurrence != proof.phi
            && other
                .incoming
                .values()
                .any(|value| aliases.contains(value) || *value == update)
    }) {
        return None;
    }

    let compare_flags = operations(body)
        .find_map(|(occurrence, _, operation)| (occurrence == proof.compare).then_some(operation))?
        .defines
        .iter()
        .copied()
        .filter(|value| value.flags)
        .collect::<BTreeSet<_>>();
    let step_flags = operations(body)
        .find_map(|(occurrence, _, operation)| (occurrence == stepping).then_some(operation))?
        .defines
        .iter()
        .copied()
        .filter(|value| value.flags)
        .collect::<BTreeSet<_>>();
    if operations(body).any(|(occurrence, _, operation)| {
        (operation
            .uses
            .iter()
            .any(|value| compare_flags.contains(value))
            && occurrence != proof.branch)
            || operation
                .uses
                .iter()
                .any(|value| step_flags.contains(value))
    }) {
        return None;
    }
    Some(ControlReplacement {
        counted: proof,
        stepping,
        update,
        aliases,
        copies,
    })
}

/// Python's default `zero_terminating_control(body, loop, proof, candidate)`.
pub(crate) fn zero_terminating_control<'a>(
    body: &MirBody,
    loop_: &Loop,
    proof: &'a CountedLoop,
    candidate: &Affine,
) -> Option<ZeroTerminatingControl<'a>> {
    let facts = constants::known(body);
    zero_terminating_control_with_facts(body, loop_, proof, candidate, &facts)
}

/// Python's explicit-facts form, for consumers sharing one fact analysis.
pub(crate) fn zero_terminating_control_with_facts<'a>(
    body: &MirBody,
    loop_: &Loop,
    proof: &'a CountedLoop,
    candidate: &Affine,
    facts: &BTreeMap<Value, Known>,
) -> Option<ZeroTerminatingControl<'a>> {
    let replacement = control_replacement(body, loop_, proof, &BTreeSet::new())?;
    let maximum = proof.maximum.as_ref()?;
    if candidate == &proof.counter {
        return None;
    }
    let width = proof.counter.start.width();
    if candidate.start.width() != width
        || candidate.step.width() != width
        || maximum < &BigInt::from(0_u8)
    {
        return None;
    }
    let start = _signed(&candidate.start.as_arg(), facts, width);
    let step = _signed(&candidate.step.as_arg(), facts, width);
    if start != Some(BigInt::from(0_u8)) {
        return None;
    }
    let step = step?;
    if step == BigInt::from(0_u8) {
        return None;
    }
    let period = AffineMap {
        scale: step.clone(),
        offset: BigInt::from(0_u8),
        width,
    }
    .period();
    if maximum > &period {
        return None;
    }
    Some(ZeroTerminatingControl {
        replacement,
        candidate: candidate.clone(),
        step,
        maximum: maximum.clone(),
        period,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use num_bigint::BigInt;

    use crate::analysis::constants::{masked, Known};
    use crate::codegen::machine::Operation;
    use crate::model::floating::{Format, Precision, Rounding, Semantics as FloatingSemantics};
    use crate::model::mir::{
        Arg, Cell, Const, FloatingOrigin, Held, IntegerRange, Kind, MemRef, MirBlock, MirBody, Op,
        OpCode, OrderedMap, Phi, Value,
    };
    use crate::model::mir_loops::Loop;

    use crate::analysis::constants;
    use crate::analysis::occurrence::operations;

    use super::{
        Affine, AffineMap, AffineOperand, LoopShape, _as_signed, _constant, _copied,
        _counter_bound, _signed, basics, canonical, control_replacement, counted,
        counted_with_facts, invariant, nonempty, relation, test_only, transparent_aliases,
        trip_count, zero_terminating_control,
    };

    fn value(id: u32, at: i64) -> Value {
        Value::new(id, at)
    }

    fn op(at: i64, kind: Kind, defines: Vec<Value>, uses: Vec<Value>) -> Op {
        let mut op = Op::new(at, None, "", defines, uses);
        op.kind = kind;
        op
    }

    fn copy(at: i64, source: Value, result: Value, source_width: u32, result_width: u32) -> Op {
        let mut operation = op(at, Kind::Copy, vec![result], vec![source]);
        operation.args = vec![Arg::Held(Held {
            value: source,
            width: source_width,
        })];
        operation.results = vec![Arg::Held(Held {
            value: result,
            width: result_width,
        })];
        operation
    }

    fn floating() -> FloatingSemantics {
        FloatingSemantics::new([], Format::Binary32, Precision::Exact, Rounding::None)
    }

    fn floating_origin() -> FloatingOrigin {
        FloatingOrigin {
            block: 0,
            sequence: vec![],
            at: 0,
            kind: Kind::Fadd,
            semantics: floating(),
            inputs: vec![],
            outputs: vec![],
            machine_inputs: vec![],
            machine_outputs: vec![],
        }
    }

    /// Direct Rust fixture for the source body used by
    /// `tests/test_induction_identity.py:body`.
    fn identity_body() -> (MirBody, Loop) {
        let start = Value {
            variable: 7,
            ..value(10, 0)
        };
        let counter = Value {
            variable: 7,
            ..value(11, 1)
        };
        let following = Value {
            variable: 7,
            ..value(12, 1)
        };
        let unrelated = Value {
            variable: 7,
            ..value(13, 1)
        };
        let answer = Value {
            variable: 8,
            ..value(14, 1)
        };
        let mut increment = op(1, Kind::Increment, vec![following], vec![counter]);
        increment.args = vec![Arg::Held(Held {
            value: counter,
            width: 2,
        })];
        increment.results = vec![Arg::Held(Held {
            value: following,
            width: 2,
        })];
        let mut multiply = op(2, Kind::Mul, vec![answer], vec![unrelated]);
        multiply.args = vec![
            Arg::Held(Held {
                value: unrelated,
                width: 2,
            }),
            Arg::Const(Const::new(2, 2)),
        ];
        multiply.results = vec![Arg::Held(Held {
            value: answer,
            width: 2,
        })];
        let mut incoming = crate::model::mir::OrderedMap::new();
        incoming.insert(0, start);
        incoming.insert(1, following);
        (
            MirBody::new(
                0,
                vec![
                    MirBlock::new(0, vec![], vec![], vec![1]),
                    MirBlock::new(
                        1,
                        vec![Phi {
                            result: counter,
                            incoming,
                        }],
                        vec![increment, multiply],
                        vec![1, 2],
                    ),
                    MirBlock::new(2, vec![], vec![], vec![]),
                ],
            ),
            Loop {
                header: 1,
                latches: BTreeSet::from([1]),
                body: BTreeSet::from([1]),
            },
        )
    }

    /// Rust form of `tests/test_indvars.py:_symbolic_control_body`: its
    /// preheader, pre-tested header, one latch, and one exit are the exact
    /// shape induction proofs consume.
    fn symbolic_control_body() -> (MirBody, Loop) {
        let bound = value(1, 0);
        let control = value(4, 1);
        let control_next = value(7, 2);
        let flags = Value {
            flags: true,
            ..value(6, 1)
        };
        let mut branch = op(1, Kind::Branch, vec![], vec![flags]);
        branch.target = Some(3);
        let mut incoming = crate::model::mir::OrderedMap::new();
        incoming.insert(0, value(2, 0));
        incoming.insert(2, control_next);
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(
                    0,
                    vec![],
                    vec![op(0, Kind::Load, vec![bound], vec![])],
                    vec![1],
                ),
                MirBlock::new(
                    1,
                    vec![Phi {
                        result: control,
                        incoming,
                    }],
                    vec![op(1, Kind::Sub, vec![flags], vec![control, bound]), branch],
                    vec![2, 3],
                ),
                MirBlock::new(
                    2,
                    vec![],
                    vec![
                        op(2, Kind::Add, vec![control_next], vec![control]),
                        op(2, Kind::Jump, vec![], vec![]),
                    ],
                    vec![1],
                ),
                MirBlock::new(3, vec![], vec![op(3, Kind::Return, vec![], vec![])], vec![]),
            ],
        );
        let loop_ = Loop {
            header: 1,
            latches: BTreeSet::from([2]),
            body: BTreeSet::from([1, 2]),
        };
        (body, loop_)
    }

    /// Direct Rust form of `tests/test_indvars.py:_symbolic_control_body`.
    fn symbolic_counted_body(candidate_start: i64) -> (MirBody, Loop, Value, Value) {
        let bound = Value {
            variable: 1,
            version: 1,
            ..value(1, 0)
        };
        let control_seed = Value {
            variable: 2,
            version: 1,
            ..value(2, 0)
        };
        let candidate_seed = Value {
            variable: 3,
            version: 1,
            ..value(3, 0)
        };
        let control = Value {
            variable: 2,
            version: 2,
            ..value(4, 1)
        };
        let candidate = Value {
            variable: 3,
            version: 2,
            ..value(5, 1)
        };
        let flags = Value {
            flags: true,
            variable: 4,
            version: 1,
            ..value(6, 1)
        };
        let control_next = Value {
            variable: 2,
            version: 3,
            ..value(7, 2)
        };
        let candidate_next = Value {
            variable: 3,
            version: 3,
            ..value(8, 2)
        };
        let offset = Value {
            variable: 5,
            version: 1,
            ..value(9, 2)
        };
        let mut source = MemRef::new(None, 2);
        source.space = Some(crate::object::omf::module::Space::Frame);

        let constant_copy = |at, result, number| {
            let mut operation = op(at, Kind::Copy, vec![result], vec![]);
            operation.op = Some(OpCode::Operation(Operation::Nothing));
            operation.args = vec![Arg::Const(Const::new(number, 2))];
            operation.results = vec![Arg::Held(Held {
                value: result,
                width: 2,
            })];
            operation
        };
        let add = |at, result, left, right| {
            let mut operation = op(at, Kind::Add, vec![result], vec![left]);
            operation.op = Some(OpCode::Operation(Operation::Nothing));
            operation.args = vec![
                Arg::Held(Held {
                    value: left,
                    width: 2,
                }),
                Arg::Const(Const::new(right, 2)),
            ];
            operation.results = vec![Arg::Held(Held {
                value: result,
                width: 2,
            })];
            operation
        };

        let mut load = op(0, Kind::Load, vec![bound], vec![]);
        load.op = Some(OpCode::Operation(Operation::Move));
        load.loads = vec![source.clone()];
        load.args = vec![Arg::Cell(Cell {
            r#ref: source.clone(),
        })];
        load.results = vec![Arg::Held(Held {
            value: bound,
            width: 2,
        })];
        let mut compare = op(1, Kind::Sub, vec![flags], vec![control, bound]);
        compare.op = Some(OpCode::Operation(Operation::Compare));
        compare.name = "cmp".to_owned();
        compare.args = vec![
            Arg::Held(Held {
                value: control,
                width: 2,
            }),
            Arg::Held(Held {
                value: bound,
                width: 2,
            }),
        ];
        let mut branch = op(1, Kind::Branch, vec![], vec![flags]);
        branch.op = Some(OpCode::Operation(Operation::Branch));
        branch.test = Some(Kind::AboveEq);
        branch.target = Some(3);
        let mut jump = op(2, Kind::Jump, vec![], vec![]);
        jump.op = Some(OpCode::Operation(Operation::Jump));
        jump.target = Some(1);
        let mut returned = op(3, Kind::Return, vec![], vec![]);
        returned.op = Some(OpCode::Operation(Operation::Return));
        let mut control_incoming = crate::model::mir::OrderedMap::new();
        control_incoming.insert(0, control_seed);
        control_incoming.insert(2, control_next);
        let mut candidate_incoming = crate::model::mir::OrderedMap::new();
        candidate_incoming.insert(0, candidate_seed);
        candidate_incoming.insert(2, candidate_next);
        let mut body = MirBody::new(
            0,
            vec![
                MirBlock::new(
                    0,
                    vec![],
                    vec![
                        load,
                        constant_copy(0, control_seed, 0),
                        constant_copy(0, candidate_seed, candidate_start),
                    ],
                    vec![1],
                ),
                MirBlock::new(
                    1,
                    vec![
                        Phi {
                            result: control,
                            incoming: control_incoming,
                        },
                        Phi {
                            result: candidate,
                            incoming: candidate_incoming,
                        },
                    ],
                    vec![compare, branch],
                    vec![2, 3],
                ),
                MirBlock::new(
                    2,
                    vec![],
                    vec![
                        add(2, offset, candidate, 100),
                        add(2, control_next, control, 1),
                        add(2, candidate_next, candidate, 1),
                        jump,
                    ],
                    vec![1],
                ),
                MirBlock::new(3, vec![], vec![returned], vec![]),
            ],
        );
        body.integer_ranges
            .insert(bound, IntegerRange::new(0, 7, 2));
        (
            body,
            Loop {
                header: 1,
                latches: BTreeSet::from([2]),
                body: BTreeSet::from([1, 2]),
            },
            bound,
            control_seed,
        )
    }

    #[test]
    fn direct_induction_canonical_accepts_symbolic_control_shape() {
        let (body, loop_) = symbolic_control_body();
        assert_eq!(
            canonical(&body, &loop_),
            Some(LoopShape {
                preheader: 0,
                latch: 2,
                entered: 2,
                exit: 3
            })
        );
    }

    #[test]
    fn direct_induction_canonical_refuses_each_python_structural_exception() {
        let (body, loop_) = symbolic_control_body();
        let cases = [
            (
                Loop {
                    latches: BTreeSet::from([1, 2]),
                    ..loop_.clone()
                },
                body.clone(),
            ),
            (
                Loop {
                    header: 99,
                    ..loop_.clone()
                },
                body.clone(),
            ),
            (
                Loop {
                    latches: BTreeSet::from([99]),
                    ..loop_.clone()
                },
                body.clone(),
            ),
        ];
        for (loop_, body) in cases {
            assert_eq!(canonical(&body, &loop_), None);
        }

        let mut outside = body.clone();
        outside.blocks[0].succ = vec![1, 3];
        assert_eq!(canonical(&outside, &loop_), None);
        let mut two_preheaders = body.clone();
        two_preheaders
            .blocks
            .push(MirBlock::new(4, vec![], vec![], vec![1]));
        assert_eq!(canonical(&two_preheaders, &loop_), None);
        let mut latch = body.clone();
        latch.blocks[2].succ = vec![1, 3];
        assert_eq!(canonical(&latch, &loop_), None);
        let mut entry = body.clone();
        entry.blocks[1].succ = vec![2, 2, 3];
        assert_eq!(canonical(&entry, &loop_), None);
        let mut no_entry = body.clone();
        no_entry.blocks[1].succ = vec![3];
        assert_eq!(canonical(&no_entry, &loop_), None);
        let mut exits = body.clone();
        exits.blocks[1].succ = vec![2, 3, 4];
        assert_eq!(canonical(&exits, &loop_), None);
        let mut no_exit = body.clone();
        no_exit.blocks[1].succ = vec![2];
        assert_eq!(canonical(&no_exit, &loop_), None);
        let mut no_operations = body.clone();
        no_operations.blocks[1].ops.clear();
        assert_eq!(canonical(&no_operations, &loop_), None);
        let mut no_branch = body.clone();
        no_branch.blocks[1].ops.last_mut().unwrap().kind = Kind::Jump;
        assert_eq!(canonical(&no_branch, &loop_), None);
        let mut side_exit = body.clone();
        side_exit.blocks[1].succ = vec![2, 3];
        side_exit.blocks[2].succ = vec![4, 3];
        side_exit
            .blocks
            .push(MirBlock::new(4, vec![], vec![], vec![1]));
        let side_loop = Loop {
            header: 1,
            latches: BTreeSet::from([4]),
            body: BTreeSet::from([1, 2, 4]),
        };
        assert_eq!(canonical(&side_exit, &side_loop), None);
    }

    #[test]
    #[should_panic]
    fn direct_induction_canonical_preserves_python_unknown_body_keyerror() {
        let (body, loop_) = symbolic_control_body();
        let unknown_inside = Loop {
            body: BTreeSet::from([1, 2, 99]),
            ..loop_
        };
        let _ = canonical(&body, &unknown_inside);
    }

    #[test]
    fn direct_induction_test_only_matches_empty_and_flag_test_forms() {
        let empty = op(0, Kind::Nothing, vec![], vec![]);
        assert!(test_only(&empty));

        let flags = Value {
            flags: true,
            ..value(1, 0)
        };
        for kind in [Kind::Sub, Kind::And, Kind::Or] {
            let mut one = op(0, kind, vec![flags], vec![value(2, 0)]);
            one.args.push(Arg::Const(Const::new(7, 2)));
            assert!(test_only(&one));
        }
        assert!(!test_only(&op(0, Kind::Add, vec![flags], vec![])));
        assert!(!test_only(&op(0, Kind::Sub, vec![value(2, 0)], vec![])));
    }

    #[test]
    fn direct_induction_test_only_refuses_every_python_observable_field() {
        let empty = op(0, Kind::Nothing, vec![], vec![]);
        let flags = op(
            0,
            Kind::Sub,
            vec![Value {
                flags: true,
                ..value(1, 0)
            }],
            vec![],
        );
        let mut cases = Vec::new();

        let mut one = empty.clone();
        one.name = "data".into();
        cases.push(one);
        let mut one = empty.clone();
        one.defines.push(value(2, 0));
        cases.push(one);
        let mut one = empty.clone();
        one.uses.push(value(2, 0));
        cases.push(one);
        let mut one = empty.clone();
        one.args.push(Arg::Const(Const::new(1, 2)));
        cases.push(one);
        let mut one = empty.clone();
        one.results.push(Arg::Held(Held {
            value: value(2, 0),
            width: 2,
        }));
        cases.push(one);
        let mut one = empty.clone();
        one.loads.push(MemRef::new(None, 2));
        cases.push(one);
        let mut one = empty.clone();
        one.stores.push(MemRef::new(None, 2));
        cases.push(one);
        let mut one = empty.clone();
        one.merges.insert(value(2, 0), value(3, 0));
        cases.push(one);
        let mut one = empty.clone();
        one.op = Some(OpCode::Operation(Operation::Barrier));
        cases.push(one);
        let mut one = empty.clone();
        one.floating = Some(floating());
        cases.push(one);
        let mut one = empty.clone();
        one.stack = Some(0);
        cases.push(one);
        let mut one = empty;
        one.floating_origin = Some(floating_origin());
        cases.push(one);
        assert!(cases.iter().all(|one| !test_only(one)));

        let mut cases = Vec::new();
        let mut one = flags.clone();
        one.kind = Kind::Add;
        cases.push(one);
        let mut one = flags.clone();
        one.results.push(Arg::Held(Held {
            value: value(2, 0),
            width: 2,
        }));
        cases.push(one);
        let mut one = flags.clone();
        one.loads.push(MemRef::new(None, 2));
        cases.push(one);
        let mut one = flags.clone();
        one.stores.push(MemRef::new(None, 2));
        cases.push(one);
        let mut one = flags.clone();
        one.merges.insert(value(2, 0), value(3, 0));
        cases.push(one);
        let mut one = flags.clone();
        one.op = Some(OpCode::Operation(Operation::Barrier));
        cases.push(one);
        let mut one = flags.clone();
        one.floating = Some(floating());
        cases.push(one);
        let mut one = flags.clone();
        one.stack = Some(0);
        cases.push(one);
        let mut one = flags.clone();
        one.floating_origin = Some(floating_origin());
        cases.push(one);
        cases.push(op(0, Kind::Sub, vec![], vec![]));
        cases.push(op(0, Kind::Sub, vec![value(2, 0)], vec![]));
        assert!(cases.iter().all(|one| !test_only(one)));
    }

    #[test]
    fn direct_induction_invariant_returns_only_ids_not_written_inside() {
        let outside = value(1, 0);
        let inside_op = value(2, 1);
        let inside_phi = value(3, 1);
        let used_outside = value(4, 2);
        let mut incoming = crate::model::mir::OrderedMap::new();
        incoming.insert(0, outside);
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(
                    0,
                    vec![],
                    vec![op(0, Kind::Copy, vec![outside], vec![])],
                    vec![1],
                ),
                MirBlock::new(
                    1,
                    vec![Phi {
                        result: inside_phi,
                        incoming,
                    }],
                    vec![op(1, Kind::Add, vec![inside_op], vec![outside])],
                    vec![2],
                ),
                MirBlock::new(
                    2,
                    vec![],
                    vec![op(
                        2,
                        Kind::Copy,
                        vec![],
                        vec![outside, inside_op, inside_phi, used_outside],
                    )],
                    vec![],
                ),
            ],
        );
        assert_eq!(
            invariant(&body, &BTreeSet::from([1])),
            BTreeSet::from([1, 4])
        );
    }

    #[test]
    fn direct_induction_basics_backedge_steps_the_exact_phi_value() {
        // Direct port of
        // tests/test_induction_identity.py:test_the_backedge_must_step_the_exact_phi_value.
        let (mut body, loop_) = identity_body();
        assert!(!basics(&body, &loop_).is_empty());
        let unrelated = match &body.blocks[1].ops[1].args[0] {
            Arg::Held(held) => *held,
            _ => unreachable!("identity fixture has a held multiply input"),
        };
        body.blocks[1].ops[0].args = vec![Arg::Held(unrelated)];
        body.blocks[1].ops[0].uses = vec![unrelated.value];
        assert!(basics(&body, &loop_).is_empty());
    }

    #[test]
    fn direct_induction_basics_accepts_counter_as_second_add_operand() {
        // Direct branch coverage for `_stepped`: Python permits the counter
        // as either add operand, then swaps the recurrence pair.
        let (mut body, loop_) = identity_body();
        let counter = body.blocks[1].phis[0].result;
        let following = body.blocks[1].ops[0].defines[0];
        body.blocks[1].ops[0].kind = Kind::Add;
        body.blocks[1].ops[0].args = vec![
            Arg::Const(Const::new(2, 2)),
            Arg::Held(Held {
                value: counter,
                width: 2,
            }),
        ];
        body.blocks[1].ops[0].results = vec![Arg::Held(Held {
            value: following,
            width: 2,
        })];
        let found = basics(&body, &loop_);
        assert_eq!(
            found.get(&counter.id).map(|one| one.step.clone()),
            Some(AffineOperand::Const(Const::new(2, 2)))
        );
    }

    #[test]
    fn direct_induction_basics_accepts_an_invariant_held_step() {
        // Direct branch coverage for `_stepped`'s `step.value.id in still`.
        let (mut body, loop_) = identity_body();
        let counter = body.blocks[1].phis[0].result;
        let following = body.blocks[1].ops[0].defines[0];
        let increment = Value {
            variable: 12,
            ..value(40, 0)
        };
        body.blocks[0]
            .ops
            .push(op(0, Kind::Copy, vec![increment], vec![]));
        body.blocks[1].ops[0].kind = Kind::Add;
        body.blocks[1].ops[0].args = vec![
            Arg::Held(Held {
                value: counter,
                width: 2,
            }),
            Arg::Held(Held {
                value: increment,
                width: 2,
            }),
        ];
        body.blocks[1].ops[0].results = vec![Arg::Held(Held {
            value: following,
            width: 2,
        })];
        let found = basics(&body, &loop_);
        assert_eq!(
            found.get(&counter.id).map(|one| one.step.clone()),
            Some(AffineOperand::Held(Held {
                value: increment,
                width: 2
            }))
        );
    }

    #[test]
    fn direct_induction_basics_refuses_a_loop_written_held_step() {
        // Direct branch coverage for `_stepped`: an otherwise matching held
        // step is not invariant when an in-loop operation defines it.
        let (mut body, loop_) = identity_body();
        let counter = body.blocks[1].phis[0].result;
        let following = body.blocks[1].ops[0].defines[0];
        let written = body.blocks[1].ops[1].defines[0];
        body.blocks[1].ops[0].kind = Kind::Add;
        body.blocks[1].ops[0].args = vec![
            Arg::Held(Held {
                value: counter,
                width: 2,
            }),
            Arg::Held(Held {
                value: written,
                width: 2,
            }),
        ];
        body.blocks[1].ops[0].results = vec![Arg::Held(Held {
            value: following,
            width: 2,
        })];
        assert!(basics(&body, &loop_).is_empty());
    }

    #[test]
    fn direct_induction_basics_keeps_header_phi_insertion_order() {
        let (mut body, loop_) = identity_body();
        let start = Value {
            variable: 9,
            ..value(31, 0)
        };
        let counter = Value {
            variable: 9,
            ..value(32, 1)
        };
        let following = Value {
            variable: 9,
            ..value(33, 1)
        };
        let mut incoming = crate::model::mir::OrderedMap::new();
        incoming.insert(0, start);
        incoming.insert(1, following);
        let mut increment = op(1, Kind::Increment, vec![following], vec![counter]);
        increment.args = vec![Arg::Held(Held {
            value: counter,
            width: 2,
        })];
        increment.results = vec![Arg::Held(Held {
            value: following,
            width: 2,
        })];
        body.blocks[1].phis.insert(
            0,
            Phi {
                result: counter,
                incoming,
            },
        );
        body.blocks[1].ops.insert(0, increment);
        assert_eq!(
            basics(&body, &loop_).keys().copied().collect::<Vec<_>>(),
            vec![counter.id, body.blocks[1].phis[1].result.id]
        );
    }

    #[test]
    fn direct_induction_basics_long_recurrence_keeps_its_width() {
        // Direct port of
        // tests/test_induction_identity.py:test_long_recurrence_keeps_its_width,
        // for both `copied` parameter values.
        for copied in [false, true] {
            let (mut body, loop_) = identity_body();
            // Python rebuilds this header as `ops = (update,)` before it
            // optionally makes `(update, copy)`.
            body.blocks[1].ops.truncate(1);
            let counter = body.blocks[1].phis[0].result;
            let following = body.blocks[1].ops[0].defines[0];
            body.blocks[1].ops[0].args = vec![Arg::Held(Held {
                value: counter,
                width: 4,
            })];
            body.blocks[1].ops[0].results = vec![Arg::Held(Held {
                value: following,
                width: 4,
            })];
            if copied {
                let temporary = Value {
                    variable: 7,
                    ..value(100, 1)
                };
                body.blocks[1].ops[0].defines = vec![temporary];
                body.blocks[1].ops[0].results = vec![Arg::Held(Held {
                    value: temporary,
                    width: 4,
                })];
                let mut copy = op(3, Kind::Copy, vec![following], vec![temporary]);
                copy.args = vec![Arg::Held(Held {
                    value: temporary,
                    width: 4,
                })];
                copy.results = vec![Arg::Held(Held {
                    value: following,
                    width: 4,
                })];
                body.blocks[1].ops.insert(1, copy);
            }
            let found = basics(&body, &loop_);
            let recurrence = found
                .get(&counter.id)
                .expect("the width-preserving recurrence remains affine");
            assert_eq!(
                recurrence.start,
                AffineOperand::Held(Held {
                    value: body.blocks[1].phis[0].incoming.get(&0).copied().unwrap(),
                    width: 4
                })
            );
            assert_eq!(recurrence.step, AffineOperand::Const(Const::new(1, 4)));
        }
    }

    #[test]
    fn direct_induction_basics_every_incoming_path_agrees_on_the_recurrence() {
        // Direct port of
        // tests/test_induction_identity.py:test_every_incoming_path_agrees_on_the_recurrence.
        for mismatch in ["start", "step", "unchanged", "none"] {
            let (mut body, mut loop_) = identity_body();
            let counter = body.blocks[1].phis[0].result;
            let start = *body.blocks[1].phis[0]
                .incoming
                .get(&0)
                .expect("identity phi has its preheader input");
            let following = Value {
                variable: 7,
                ..value(20, 3)
            };
            let mut step = body.blocks[1].ops[0].clone();
            step.at = 3;
            step.defines = vec![following];
            step.results = vec![Arg::Held(Held {
                value: following,
                width: 2,
            })];
            if mismatch == "step" {
                step.kind = Kind::Decrement;
            }
            let phi = &mut body.blocks[1].phis[0];
            phi.incoming.insert(
                4,
                if mismatch == "start" {
                    Value {
                        variable: 7,
                        ..value(21, 4)
                    }
                } else {
                    start
                },
            );
            phi.incoming.insert(
                3,
                if mismatch == "unchanged" {
                    counter
                } else {
                    following
                },
            );
            body.blocks[1].succ = vec![1, 2, 3];
            body.blocks
                .push(MirBlock::new(3, vec![], vec![step], vec![1]));
            body.blocks.push(MirBlock::new(4, vec![], vec![], vec![1]));
            loop_.body = BTreeSet::from([1, 3]);
            loop_.latches = BTreeSet::from([1, 3]);
            assert_eq!(!basics(&body, &loop_).is_empty(), mismatch == "none");
        }
    }

    #[test]
    fn direct_induction_basics_copied_requires_width_preservation() {
        // Direct helper port of the `_copied` premise in
        // tests/test_induction_identity.py:test_only_width_preserving_copies_carry_the_recurrence.
        for (width, expected_source) in [(2, true), (4, false)] {
            let (body, _) = identity_body();
            let counter = body.blocks[1].phis[0].result;
            let copied = match &body.blocks[1].ops[1].args[0] {
                Arg::Held(held) => held.value,
                _ => unreachable!("identity fixture has a held multiply input"),
            };
            let mut copy = op(1, Kind::Copy, vec![copied], vec![counter]);
            copy.args = vec![Arg::Held(Held {
                value: counter,
                width,
            })];
            copy.results = vec![Arg::Held(Held {
                value: copied,
                width: 2,
            })];
            let made = BTreeMap::from([(copied.id, &copy)]);
            let root = _copied(
                Held {
                    value: copied,
                    width: 2,
                },
                &made,
            );
            assert_eq!(root.value == counter, expected_source);
            assert_eq!(root.width, 2);
        }
    }

    #[test]
    fn direct_induction_basics_copied_stops_at_every_python_boundary() {
        // Focused coverage of every refusal branch in the direct port of
        // qbopt.analysis.induction:_copied.
        let source = value(1, 0);
        let result = value(2, 1);
        let operand = Held {
            value: result,
            width: 2,
        };
        let mut copy = op(1, Kind::Copy, vec![result], vec![source]);
        copy.args = vec![Arg::Held(Held {
            value: source,
            width: 2,
        })];
        copy.results = vec![Arg::Held(operand)];
        let follows = |copy: &Op| {
            let made = BTreeMap::from([(result.id, copy)]);
            _copied(operand, &made)
        };
        assert_eq!(follows(&copy).value, source);

        let mut non_copy = copy.clone();
        non_copy.kind = Kind::Add;
        assert_eq!(follows(&non_copy), operand);
        let mut memory = copy.clone();
        memory.loads.push(MemRef::new(None, 2));
        assert_eq!(follows(&memory), operand);
        let mut arity = copy.clone();
        arity.args.push(Arg::Const(Const::new(1, 2)));
        assert_eq!(follows(&arity), operand);
        let mut type_mismatch = copy.clone();
        type_mismatch.args = vec![Arg::Const(Const::new(1, 2))];
        assert_eq!(follows(&type_mismatch), operand);
        let mut source_width = copy.clone();
        source_width.args = vec![Arg::Held(Held {
            value: source,
            width: 4,
        })];
        assert_eq!(follows(&source_width), operand);
        let mut result_width = copy.clone();
        result_width.results = vec![Arg::Held(Held {
            value: result,
            width: 4,
        })];
        assert_eq!(follows(&result_width), operand);

        let mut cycle = copy;
        cycle.args = vec![Arg::Held(operand)];
        assert_eq!(follows(&cycle), operand);
    }

    #[test]
    fn direct_induction_transparent_aliases_follows_width_preserving_copy_closure() {
        // Direct port of `induction.transparent_aliases`: the second COPY
        // becomes visible only after the first has extended `aliases`.
        let source = value(1, 0);
        let first = value(2, 1);
        let second = value(3, 1);
        let body = MirBody::new(
            0,
            vec![MirBlock::new(
                1,
                vec![],
                vec![copy(1, source, first, 2, 2), copy(1, first, second, 2, 2)],
                vec![],
            )],
        );
        let loop_ = Loop {
            header: 1,
            latches: BTreeSet::new(),
            body: BTreeSet::from([1]),
        };

        let (aliases, copies) = transparent_aliases(&body, &loop_, source);

        assert_eq!(aliases, BTreeSet::from([source, first, second]));
        assert_eq!(copies.len(), 2);
    }

    #[test]
    fn direct_induction_transparent_aliases_refuses_every_python_copy_boundary() {
        let source = value(1, 0);
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(
                    1,
                    vec![],
                    vec![
                        copy(1, source, value(2, 1), 2, 4),
                        {
                            let mut operation = copy(1, source, value(3, 1), 2, 2);
                            operation.args = vec![Arg::Const(Const::new(0, 2))];
                            operation
                        },
                        {
                            let mut operation = copy(1, source, value(4, 1), 2, 2);
                            operation.kind = Kind::Add;
                            operation
                        },
                        {
                            let mut operation = copy(1, source, value(5, 1), 2, 2);
                            operation.args = vec![];
                            operation
                        },
                        {
                            let mut operation = copy(1, source, value(6, 1), 2, 2);
                            operation.results = vec![];
                            operation
                        },
                        {
                            let mut operation = copy(1, source, value(7, 1), 2, 2);
                            operation.results = vec![Arg::Const(Const::new(0, 2))];
                            operation
                        },
                        copy(1, value(8, 0), value(9, 1), 2, 2),
                    ],
                    vec![],
                ),
                MirBlock::new(2, vec![], vec![copy(2, source, value(10, 2), 2, 2)], vec![]),
            ],
        );
        let loop_ = Loop {
            header: 1,
            latches: BTreeSet::new(),
            body: BTreeSet::from([1]),
        };

        let (aliases, copies) = transparent_aliases(&body, &loop_, source);

        assert_eq!(aliases, BTreeSet::from([source]));
        assert!(copies.is_empty());
    }

    #[test]
    fn direct_induction_transparent_aliases_keeps_equal_operations_distinct() {
        // Python's `id(op)`, not dataclass equality, records both copies.
        let source = value(1, 0);
        let result = value(2, 1);
        let operation = copy(1, source, result, 2, 2);
        let body = MirBody::new(
            0,
            vec![MirBlock::new(
                1,
                vec![],
                vec![operation.clone(), operation],
                vec![],
            )],
        );
        assert_eq!(body.blocks[0].ops[0], body.blocks[0].ops[1]);
        let loop_ = Loop {
            header: 1,
            latches: BTreeSet::new(),
            body: BTreeSet::from([1]),
        };

        let (_aliases, copies) = transparent_aliases(&body, &loop_, source);

        assert_eq!(copies.len(), 2);
    }

    #[test]
    fn direct_induction_transparent_aliases_does_not_use_source_operation_id() {
        // `Op.id` is source provenance and may collide; Python `id(op)` does
        // not.  Two equal source IDs must therefore remain two occurrences.
        let source = value(1, 0);
        let result = value(2, 1);
        let mut first = copy(1, source, result, 2, 2);
        first.id = Some(9);
        let mut second = first.clone();
        second.id = Some(9);
        let body = MirBody::new(
            0,
            vec![MirBlock::new(1, vec![], vec![first, second], vec![])],
        );
        let loop_ = Loop {
            header: 1,
            latches: BTreeSet::new(),
            body: BTreeSet::from([1]),
        };

        let (_aliases, copies) = transparent_aliases(&body, &loop_, source);

        assert_eq!(copies.len(), 2);
    }

    fn constant(value: i64, width: u32) -> AffineOperand {
        AffineOperand::Const(Const::new(value, width))
    }

    fn affine(value: u32, start: AffineOperand, step: AffineOperand, header: i64) -> Affine {
        Affine {
            value,
            start,
            step,
            header,
        }
    }

    #[test]
    fn direct_induction_affine_map_carries_the_modular_injectivity_proof() {
        // Direct port of
        // tests/test_induction_identity.py:test_affine_map_carries_the_modular_injectivity_proof.
        let source = affine(1, constant(0, 2), constant(1, 2), 1);
        let byte_offset = affine(2, constant(0, 2), constant(16, 2), 1);

        let mapping = relation(&source, &byte_offset, &BTreeMap::new())
            .expect("the scaled constant recurrence has a relation");

        assert_eq!(
            mapping,
            AffineMap {
                scale: BigInt::from(16),
                offset: BigInt::from(0),
                width: 2,
            }
        );
        assert!(mapping.injective(&BigInt::from(0), &BigInt::from(5)));
        assert!(!mapping.injective(&BigInt::from(0), &BigInt::from(4096)));
    }

    #[test]
    fn direct_induction_affine_map_preserves_python_period_and_boundaries() {
        // Direct branch coverage for `AffineMap.period` and `injective`.
        let negative = AffineMap {
            scale: BigInt::from(-16),
            offset: BigInt::from(0),
            width: 2,
        };
        assert_eq!(negative.period(), BigInt::from(4096));
        assert!(negative.injective(&BigInt::from(4), &BigInt::from(0)));
        let zero = AffineMap {
            scale: BigInt::from(0),
            offset: BigInt::from(0),
            width: 2,
        };
        assert_eq!(zero.period(), BigInt::from(1));
        assert!(!zero.injective(&BigInt::from(0), &BigInt::from(0)));
    }

    #[test]
    fn direct_induction_affine_map_constant_refusals_and_masking() {
        // Direct port of every refusal in `induction._constant`.
        let facts = BTreeMap::new();
        assert_eq!(masked(&BigInt::from(-1), 2), BigInt::from(0xffff));
        assert_eq!(
            _constant(&constant(-1, 2).as_arg(), &facts, 2),
            Some(BigInt::from(0xffff))
        );
        assert_eq!(_constant(&constant(1, 4).as_arg(), &facts, 2), None);

        let held = Value::new(90, 0);
        let operand = AffineOperand::Held(Held {
            value: held,
            width: 2,
        });
        assert_eq!(_constant(&operand.as_arg(), &facts, 2), None);
        let narrow = BTreeMap::from([(held, Known::new(1, 1))]);
        assert_eq!(_constant(&operand.as_arg(), &narrow, 2), None);
        let wide = BTreeMap::from([(held, Known::new(0x1_2345, 4))]);
        assert_eq!(
            _constant(&operand.as_arg(), &wide, 2),
            Some(BigInt::from(0x2345))
        );
        let cell = Arg::Cell(Cell {
            r#ref: MemRef::new(None, 2),
        });
        assert_eq!(_constant(&cell, &facts, 2), None);
    }

    #[test]
    fn direct_induction_affine_map_signed_reinterpretation_and_refusals() {
        // Direct port of `induction._as_signed` and every refusal in
        // `induction._signed`.
        assert_eq!(_as_signed(&BigInt::from(0x7fff), 2), BigInt::from(32767));
        assert_eq!(_as_signed(&BigInt::from(0x8000), 2), BigInt::from(-32768));
        assert_eq!(_as_signed(&BigInt::from(0xffff), 2), BigInt::from(-1));
        let facts = BTreeMap::new();
        assert_eq!(
            _signed(&constant(-1, 2).as_arg(), &facts, 2),
            Some(BigInt::from(-1))
        );
        assert_eq!(
            _signed(&constant(0x8000, 2).as_arg(), &facts, 2),
            Some(BigInt::from(-32768))
        );
        assert_eq!(_signed(&constant(1, 4).as_arg(), &facts, 2), None);
        let held = Value::new(91, 0);
        let operand = AffineOperand::Held(Held {
            value: held,
            width: 2,
        });
        assert_eq!(_signed(&operand.as_arg(), &facts, 2), None);
        let narrow = BTreeMap::from([(held, Known::new(1, 1))]);
        assert_eq!(_signed(&operand.as_arg(), &narrow, 2), None);
        let cell = Arg::Cell(Cell {
            r#ref: MemRef::new(None, 2),
        });
        assert_eq!(_signed(&cell, &facts, 2), None);
    }

    #[test]
    fn direct_induction_affine_map_relation_refuses_each_python_case() {
        // Direct port of every `None` branch in `induction.relation`.
        let source = affine(1, constant(0, 2), constant(1, 2), 1);
        let target = affine(2, constant(0, 2), constant(16, 2), 1);
        let facts = BTreeMap::new();

        let mismatched_start = affine(2, constant(0, 4), constant(16, 2), 1);
        assert_eq!(relation(&source, &mismatched_start, &facts), None);
        let zero_source_step = affine(1, constant(0, 2), constant(0, 2), 1);
        assert_eq!(relation(&zero_source_step, &target, &facts), None);
        let source_step_two = affine(1, constant(0, 2), constant(2, 2), 1);
        let non_integral_scale = affine(2, constant(0, 2), constant(3, 2), 1);
        assert_eq!(
            relation(&source_step_two, &non_integral_scale, &facts),
            None
        );
        let zero_scale = affine(2, constant(0, 2), constant(0, 2), 1);
        assert_eq!(relation(&source, &zero_scale, &facts), None);

        for position in 0..4 {
            let unknown = AffineOperand::Held(Held {
                value: Value::new(100 + position, 0),
                width: 2,
            });
            let (mut source, mut target) = (source.clone(), target.clone());
            match position {
                0 => source.start = unknown,
                1 => source.step = unknown,
                2 => target.start = unknown,
                3 => target.step = unknown,
                _ => unreachable!("four Python signed inputs"),
            }
            assert_eq!(relation(&source, &target, &facts), None);
        }
        let target_step_width = affine(2, constant(0, 2), constant(16, 4), 1);
        assert_eq!(relation(&source, &target_step_width, &facts), None);
    }

    #[test]
    fn direct_induction_counted_counter_zero_test_requires_an_unchanged_counter() {
        // Direct port of
        // tests/test_induction_identity.py:test_counter_zero_test_requires_an_unchanged_counter.
        for (kind, same, accepted) in [
            (Kind::Or, true, true),
            (Kind::And, true, true),
            (Kind::Xor, true, false),
            (Kind::Or, false, false),
            (Kind::And, false, false),
        ] {
            let value = Value::new(900, 0);
            let result = Value::new(901, 0);
            let flags = Value {
                flags: true,
                ..Value::new(902, 0)
            };
            let source = Arg::Held(Held { value, width: 2 });
            let mut compare = op(0, kind, vec![result, flags], vec![value]);
            compare.args = vec![
                source.clone(),
                if same {
                    source.clone()
                } else {
                    Arg::Const(Const::new(1, 2))
                },
            ];
            compare.results = vec![Arg::Held(Held {
                value: result,
                width: 2,
            })];
            let branch = op(1, Kind::Branch, vec![], vec![flags]);
            let counter = Affine {
                value: value.id,
                start: AffineOperand::Const(Const::new(-1, 2)),
                step: AffineOperand::Const(Const::new(1, 2)),
                header: 0,
            };
            assert_eq!(
                _counter_bound(&compare, &branch, &counter, 2, None),
                accepted.then_some(Arg::Const(Const::new(0, 2)))
            );
        }
    }

    #[test]
    fn direct_induction_counted_counter_zero_test_keeps_partial_result_flags() {
        // Direct port of
        // tests/test_induction_identity.py:test_counter_zero_test_keeps_its_flags_across_a_partial_result.
        let value = Value::new(910, 0);
        let result = Value::new(911, 0);
        let flags = Value {
            flags: true,
            ..Value::new(912, 0)
        };
        let source = Arg::Held(Held { value, width: 2 });
        let mut compare = op(0, Kind::Or, vec![result, flags], vec![value]);
        compare.merges.insert(value, result);
        compare.args = vec![source.clone(), source];
        compare.results = vec![Arg::Held(Held {
            value: result,
            width: 2,
        })];
        let branch = op(1, Kind::Branch, vec![], vec![flags]);
        let counter = Affine {
            value: value.id,
            start: AffineOperand::Const(Const::new(-1, 2)),
            step: AffineOperand::Const(Const::new(1, 2)),
            header: 0,
        };
        assert_eq!(
            _counter_bound(&compare, &branch, &counter, 2, None),
            Some(Arg::Const(Const::new(0, 2)))
        );
    }

    #[test]
    fn direct_induction_counted_symbolic_control_proves_bound_and_maximum() {
        // Direct Rust fixture of tests/test_indvars.py:_symbolic_control_body.
        let (body, loop_, bound, _) = symbolic_counted_body(0);
        let proven = counted(&body, &loop_);
        assert_eq!(basics(&body, &loop_).len(), 2);
        assert_eq!(proven.len(), 1);
        assert_eq!(
            proven[0].bound,
            AffineOperand::Held(Held {
                value: bound,
                width: 2
            })
        );
        assert_eq!(
            proven[0].trips(),
            &AffineOperand::Held(Held {
                value: bound,
                width: 2
            })
        );
        assert_eq!(proven[0].maximum, Some(BigInt::from(7)));
        assert_eq!(
            (
                proven[0].preheader,
                proven[0].latch,
                proven[0].entered,
                proven[0].exit
            ),
            (0, 2, 2, 3)
        );
    }

    #[test]
    fn direct_induction_counted_refuses_nonzero_or_nonunit_control() {
        let (body, loop_, _, seed) = symbolic_counted_body(0);
        let mut nonzero = constants::known(&body);
        nonzero.insert(seed, Known::new(1, 2));
        assert!(counted_with_facts(&body, &loop_, &nonzero).is_empty());

        let mut nonunit = body;
        nonunit.blocks[2].ops[1].args[1] = Arg::Const(Const::new(2, 2));
        assert!(counted(&nonunit, &loop_).is_empty());
    }

    #[test]
    fn direct_induction_counted_refuses_missing_or_noninvariant_bound() {
        let (body, loop_, bound, _) = symbolic_counted_body(0);
        let mut missing = body.clone();
        missing.blocks[1].ops[0].args[1] = Arg::Symbol(crate::model::mir::Symbol::new(
            crate::object::omf::module::Space::Segment,
            0,
            0,
            2,
        ));
        assert!(counted(&missing, &loop_).is_empty());

        let mut noninvariant = body;
        noninvariant.blocks[2]
            .ops
            .push(op(2, Kind::Copy, vec![bound], vec![]));
        assert!(counted(&noninvariant, &loop_).is_empty());
    }

    #[test]
    fn direct_induction_counted_refuses_wrong_branch_direction_and_impure_update() {
        let (body, loop_, _, _) = symbolic_counted_body(0);
        let mut wrong_direction = body.clone();
        wrong_direction.blocks[1].ops[1].test = Some(Kind::Below);
        assert!(counted(&wrong_direction, &loop_).is_empty());

        let mut impure = body;
        impure.blocks[2].ops[1].stores.push(MemRef::new(None, 2));
        assert!(counted(&impure, &loop_).is_empty());
    }

    #[test]
    fn direct_induction_control_replacement_proves_only_canonical_control() {
        // Direct port of `induction.control_replacement`: the source
        // recurrence is removable only when the two-block counted control is
        // its sole observer.  The returned proof retains the exact counted
        // proof supplied by its caller, as Python stores that object itself.
        let (body, loop_, _, _) = symbolic_counted_body(0);
        let proofs = counted(&body, &loop_);
        let proof = &proofs[0];

        let replacement = control_replacement(&body, &loop_, proof, &BTreeSet::new())
            .expect("the symbolic loop has no source-counter observer besides control");

        assert!(std::ptr::eq(replacement.counted, proof));
        assert_eq!(replacement.update, body.blocks[2].ops[1].defines[0]);
        assert_eq!(
            replacement.aliases,
            BTreeSet::from([body.blocks[1].phis[0].result])
        );
        assert!(replacement.copies.is_empty());
    }

    #[test]
    fn direct_induction_zero_terminating_control_refuses_nonzero_terminal_recurrence() {
        // Direct Rust regression for
        // tests/test_indvars.py:test_symbolic_control_refuses_a_nonzero_terminal_recurrence.
        let (body, loop_, _, _) = symbolic_counted_body(5);
        let proofs = counted(&body, &loop_);
        let proof = &proofs[0];
        let candidates = basics(&body, &loop_);
        let candidate = candidates
            .values()
            .find(|one| *one != &proof.counter)
            .expect("the fixture has a non-control affine recurrence");

        assert!(zero_terminating_control(&body, &loop_, proof, candidate).is_none());
    }

    #[test]
    fn direct_induction_zero_terminating_control_proves_zero_terminal_recurrence() {
        // Direct Rust regression for
        // tests/test_indvars.py:test_symbolic_control_proves_a_zero_terminal_recurrence.
        let (body, loop_, _, _) = symbolic_counted_body(0);
        let proofs = counted(&body, &loop_);
        let proof = &proofs[0];
        let candidates = basics(&body, &loop_);
        let candidate = candidates
            .values()
            .find(|one| *one != &proof.counter)
            .expect("the fixture has a non-control affine recurrence");

        let proven = zero_terminating_control(&body, &loop_, proof, candidate)
            .expect("the zero-terminal recurrence supplies the terminating flags");

        assert!(std::ptr::eq(proven.replacement.counted, proof));
        assert_eq!(proven.candidate, *candidate);
        assert_eq!(proven.step, BigInt::from(1_u8));
        assert_eq!(proven.maximum, BigInt::from(7_u8));
        assert_eq!(proven.period, BigInt::from(65_536_u32));
    }

    #[test]
    fn direct_induction_control_replacement_refuses_each_structural_exception() {
        // Direct port of the first compound refusal in
        // `induction.control_replacement`: each case is a different failure
        // of the normalized two-block, one-predecessor control shape.
        let (body, loop_, _, _) = symbolic_counted_body(0);
        let proofs = counted(&body, &loop_);
        let proof = &proofs[0];

        let one_block = Loop {
            body: BTreeSet::from([1]),
            ..loop_.clone()
        };
        assert_eq!(
            control_replacement(&body, &one_block, proof, &BTreeSet::new()),
            None
        );

        let mut wrong_entry = proof.clone();
        wrong_entry.entered = proof.entered + 1;
        assert_eq!(
            control_replacement(&body, &loop_, &wrong_entry, &BTreeSet::new()),
            None
        );

        let mut latch_phi = body.clone();
        latch_phi.blocks[2].phis.push(Phi::new(value(200, 2)));
        assert_eq!(
            control_replacement(&latch_phi, &loop_, proof, &BTreeSet::new()),
            None
        );

        let mut extra_predecessor = body.clone();
        extra_predecessor
            .blocks
            .push(MirBlock::new(4, vec![], vec![], vec![2]));
        assert_eq!(
            control_replacement(&extra_predecessor, &loop_, proof, &BTreeSet::new()),
            None
        );

        let mut header_work = body;
        header_work.blocks[1]
            .ops
            .push(op(1, Kind::Add, vec![], vec![]));
        assert_eq!(
            control_replacement(&header_work, &loop_, proof, &BTreeSet::new()),
            None
        );
    }

    #[test]
    fn direct_induction_control_replacement_tracks_exact_occurrences_and_uses() {
        // `covered` is Python's `frozenset(id(op))`, not structural equality.
        // Two equal source observers prove that covering one cannot authorize
        // the other.  Likewise, an equal phi distinct from the proven phi is
        // a forbidden incoming use.
        let (body, loop_, _, _) = symbolic_counted_body(0);
        let proofs = counted(&body, &loop_);
        let proof = &proofs[0];
        let counter = body.blocks[1].phis[0].result;
        let observer = op(2, Kind::Nothing, vec![], vec![counter]);

        let mut covered_once = body.clone();
        covered_once.blocks[2].ops.push(observer.clone());
        let covered_occurrence = operations(&covered_once)
            .find_map(|(occurrence, _, operation)| (operation == &observer).then_some(occurrence))
            .expect("the covered observer belongs to this body snapshot");
        assert!(control_replacement(
            &covered_once,
            &loop_,
            proof,
            &BTreeSet::from([covered_occurrence])
        )
        .is_some());

        let mut equal_observers = covered_once;
        equal_observers.blocks[2].ops.push(observer);
        assert_eq!(
            control_replacement(
                &equal_observers,
                &loop_,
                proof,
                &BTreeSet::from([covered_occurrence])
            ),
            None
        );

        let mut equal_phi = body.clone();
        let duplicate = equal_phi.blocks[1].phis[0].clone();
        equal_phi.blocks[1].phis.push(duplicate);
        assert_eq!(
            control_replacement(&equal_phi, &loop_, proof, &BTreeSet::new()),
            None
        );

        let mut update_user = body.clone();
        update_user.blocks[2].ops.push(op(
            2,
            Kind::Nothing,
            vec![],
            vec![body.blocks[2].ops[1].defines[0]],
        ));
        assert_eq!(
            control_replacement(&update_user, &loop_, proof, &BTreeSet::new()),
            None
        );
    }

    #[test]
    fn direct_induction_control_replacement_refuses_alias_phi_and_flag_observers() {
        // Direct port of the remaining use checks: transparent copies are
        // allowed themselves, but no alias, phi edge, compare flag, or step
        // flag may introduce another observer of removed control.
        let (body, loop_, _, _) = symbolic_counted_body(0);
        let proofs = counted(&body, &loop_);
        let proof = &proofs[0];
        let counter = body.blocks[1].phis[0].result;

        let alias = value(201, 2);
        let mut alias_user = body.clone();
        alias_user.blocks[2].ops.push(copy(2, counter, alias, 2, 2));
        alias_user.blocks[2]
            .ops
            .push(op(2, Kind::Nothing, vec![], vec![alias]));
        assert_eq!(
            control_replacement(&alias_user, &loop_, proof, &BTreeSet::new()),
            None
        );

        let mut phi_user = body.clone();
        let mut incoming = crate::model::mir::OrderedMap::new();
        incoming.insert(0, counter);
        phi_user.blocks[3].phis.push(Phi {
            result: value(202, 3),
            incoming,
        });
        assert_eq!(
            control_replacement(&phi_user, &loop_, proof, &BTreeSet::new()),
            None
        );

        let mut compare_flags = body.clone();
        compare_flags.blocks[2].ops.push(op(
            2,
            Kind::Nothing,
            vec![],
            vec![body.blocks[1].ops[0].defines[0]],
        ));
        assert_eq!(
            control_replacement(&compare_flags, &loop_, proof, &BTreeSet::new()),
            None
        );

        let mut step_flags = body;
        let flags = Value {
            flags: true,
            ..value(203, 2)
        };
        step_flags.blocks[2].ops[1].defines.push(flags);
        step_flags.blocks[2]
            .ops
            .push(op(2, Kind::Nothing, vec![], vec![flags]));
        assert_eq!(
            control_replacement(&step_flags, &loop_, proof, &BTreeSet::new()),
            None
        );
    }

    /// Direct Rust form of
    /// `test_induction_identity:test_posttested_counter_has_an_exact_fixed_trip_count`.
    fn posttested_counter_body(
        start_number: i64,
        bound_number: i64,
        test: Kind,
    ) -> (MirBody, Loop) {
        let start = Value {
            variable: 1,
            ..value(920, 0)
        };
        let counter = Value {
            variable: 1,
            ..value(921, 1)
        };
        let following = Value {
            variable: 1,
            ..value(922, 1)
        };
        let flags = Value {
            flags: true,
            ..value(923, 2)
        };
        let mut initial = op(0, Kind::Copy, vec![start], vec![]);
        initial.args = vec![Arg::Const(Const::new(start_number, 2))];
        initial.results = vec![Arg::Held(Held {
            value: start,
            width: 2,
        })];
        let mut increment = op(1, Kind::Increment, vec![following], vec![counter]);
        increment.args = vec![Arg::Held(Held {
            value: counter,
            width: 2,
        })];
        increment.results = vec![Arg::Held(Held {
            value: following,
            width: 2,
        })];
        let mut compare = op(2, Kind::Sub, vec![flags], vec![following]);
        compare.args = vec![
            Arg::Held(Held {
                value: following,
                width: 2,
            }),
            Arg::Const(Const::new(bound_number, 2)),
        ];
        let mut branch = op(2, Kind::Branch, vec![], vec![flags]);
        branch.test = Some(test);
        branch.target = Some(3);
        let mut incoming = OrderedMap::new();
        incoming.insert(0, start);
        incoming.insert(2, following);
        (
            MirBody::new(
                0,
                vec![
                    MirBlock::new(0, vec![], vec![initial], vec![1]),
                    MirBlock::new(
                        1,
                        vec![Phi {
                            result: counter,
                            incoming,
                        }],
                        vec![increment],
                        vec![2],
                    ),
                    MirBlock::new(2, vec![], vec![compare, branch], vec![1, 3]),
                    MirBlock::new(3, vec![], vec![], vec![]),
                ],
            ),
            Loop {
                header: 1,
                latches: BTreeSet::from([2]),
                body: BTreeSet::from([1, 2]),
            },
        )
    }

    #[test]
    fn direct_induction_posttested_counter_keeps_c_nbody_exact_count() {
        // C nbody's rotated `i < 4` loop was priced as ten trips after GCC
        // unrolled it. Python proves the immediate latch update has four.
        let (body, loop_) = posttested_counter_body(0, 4, Kind::AboveEq);
        assert_eq!(
            trip_count(&body, &loop_, &constants::known(&body)),
            Some(BigInt::from(4))
        );
    }

    #[test]
    fn direct_induction_pretested_counter_proves_trip_count_and_nonempty() {
        // Direct pre-tested counterpart of Python's finite recurrence proof:
        // seed=0; while i < 4: i += 1.  `rotated` consumes this `_last_counter`
        // path before it has changed the loop into a post-tested shape.
        let start = Value {
            variable: 5,
            ..value(924, 0)
        };
        let counter = Value {
            variable: 5,
            ..value(925, 1)
        };
        let following = Value {
            variable: 5,
            ..value(926, 2)
        };
        let flags = Value {
            flags: true,
            ..value(927, 1)
        };
        let mut initial = op(0, Kind::Copy, vec![start], vec![]);
        initial.args = vec![Arg::Const(Const::new(0, 2))];
        initial.results = vec![Arg::Held(Held {
            value: start,
            width: 2,
        })];
        let mut compare = op(1, Kind::Sub, vec![flags], vec![counter]);
        compare.args = vec![
            Arg::Held(Held {
                value: counter,
                width: 2,
            }),
            Arg::Const(Const::new(4, 2)),
        ];
        let mut branch = op(1, Kind::Branch, vec![], vec![flags]);
        branch.test = Some(Kind::AboveEq);
        branch.target = Some(3);
        let mut increment = op(2, Kind::Increment, vec![following], vec![counter]);
        increment.args = vec![Arg::Held(Held {
            value: counter,
            width: 2,
        })];
        increment.results = vec![Arg::Held(Held {
            value: following,
            width: 2,
        })];
        let mut incoming = OrderedMap::new();
        incoming.insert(0, start);
        incoming.insert(2, following);
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], vec![initial], vec![1]),
                MirBlock::new(
                    1,
                    vec![Phi {
                        result: counter,
                        incoming,
                    }],
                    vec![compare, branch],
                    vec![2, 3],
                ),
                MirBlock::new(2, vec![], vec![increment], vec![1]),
                MirBlock::new(3, vec![], vec![], vec![]),
            ],
        );
        let loop_ = Loop {
            header: 1,
            latches: BTreeSet::from([2]),
            body: BTreeSet::from([1, 2]),
        };
        assert_eq!(
            trip_count(&body, &loop_, &constants::known(&body)),
            Some(BigInt::from(4))
        );
        assert!(nonempty(&body, &loop_));
    }

    #[test]
    fn direct_induction_posttested_symbolic_sentinel_keeps_mandelbrot_count() {
        // Python regression `test_posttested_symbolic_sentinel_keeps_its_exact_trip_count`:
        // Mandelbrot's 24-byte coordinate recurrence reaches +768 in 32 trips.
        let start = Value {
            variable: 1,
            ..value(930, 0)
        };
        let end = Value {
            variable: 2,
            ..value(931, 0)
        };
        let counter = Value {
            variable: 3,
            ..value(932, 1)
        };
        let following = Value {
            variable: 3,
            ..value(933, 2)
        };
        let flags = Value {
            flags: true,
            ..value(934, 2)
        };
        let mut endpoint = op(0, Kind::Add, vec![end], vec![start]);
        endpoint.args = vec![
            Arg::Held(Held {
                value: start,
                width: 4,
            }),
            Arg::Const(Const::new(768, 4)),
        ];
        endpoint.results = vec![Arg::Held(Held {
            value: end,
            width: 4,
        })];
        let mut increment = op(2, Kind::Add, vec![following], vec![counter]);
        increment.args = vec![
            Arg::Held(Held {
                value: counter,
                width: 4,
            }),
            Arg::Const(Const::new(24, 4)),
        ];
        increment.results = vec![Arg::Held(Held {
            value: following,
            width: 4,
        })];
        let mut compare = op(2, Kind::Sub, vec![flags], vec![following, end]);
        compare.args = vec![
            Arg::Held(Held {
                value: following,
                width: 4,
            }),
            Arg::Held(Held {
                value: end,
                width: 4,
            }),
        ];
        let mut branch = op(2, Kind::Branch, vec![], vec![flags]);
        branch.test = Some(Kind::Ne);
        branch.target = Some(1);
        let mut incoming = OrderedMap::new();
        incoming.insert(0, start);
        incoming.insert(2, following);
        let body = MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], vec![endpoint], vec![1]),
                MirBlock::new(
                    1,
                    vec![Phi {
                        result: counter,
                        incoming,
                    }],
                    vec![],
                    vec![2],
                ),
                MirBlock::new(2, vec![], vec![increment, compare, branch], vec![1, 3]),
                MirBlock::new(3, vec![], vec![], vec![]),
            ],
        );
        let loop_ = Loop {
            header: 1,
            latches: BTreeSet::from([2]),
            body: BTreeSet::from([1, 2]),
        };
        assert_eq!(
            trip_count(&body, &loop_, &constants::known(&body)),
            Some(BigInt::from(32))
        );
    }

    #[test]
    fn direct_induction_trip_count_uses_only_empty_new_proof_fallback() {
        let (mut body, loop_) = posttested_counter_body(0, 4, Kind::AboveEq);
        body.loop_trip_counts = vec![(1, 7), (1, 8)];
        // A fresh exact proof wins; stored facts only preserve a relationship a prior transform consumed.
        assert_eq!(
            trip_count(&body, &loop_, &constants::known(&body)),
            Some(BigInt::from(4))
        );
        body.blocks[1].phis.clear();
        // Python's dict construction retains the last duplicate header entry.
        assert_eq!(
            trip_count(&body, &loop_, &constants::known(&body)),
            Some(BigInt::from(8))
        );
    }

    #[test]
    fn direct_induction_trip_count_refuses_conflicting_new_recurrences() {
        // A stored header fact may repair only the absence of a newly derived
        // count. Two current recurrences that disagree must still be refused.
        let (mut body, loop_) = posttested_counter_body(0, 4, Kind::AboveEq);
        body.loop_trip_counts = vec![(1, 99)];
        let start = Value {
            variable: 4,
            ..value(940, 0)
        };
        let counter = Value {
            variable: 4,
            ..value(941, 1)
        };
        let following = Value {
            variable: 4,
            ..value(942, 1)
        };
        let flags = Value {
            flags: true,
            ..value(943, 2)
        };
        let mut initial = op(0, Kind::Copy, vec![start], vec![]);
        initial.args = vec![Arg::Const(Const::new(0, 2))];
        initial.results = vec![Arg::Held(Held {
            value: start,
            width: 2,
        })];
        body.blocks[0].ops.push(initial);
        let mut incoming = OrderedMap::new();
        incoming.insert(0, start);
        incoming.insert(2, following);
        body.blocks[1].phis.push(Phi {
            result: counter,
            incoming,
        });
        let mut increment = op(1, Kind::Increment, vec![following], vec![counter]);
        increment.args = vec![Arg::Held(Held {
            value: counter,
            width: 2,
        })];
        increment.results = vec![Arg::Held(Held {
            value: following,
            width: 2,
        })];
        body.blocks[1].ops.push(increment);
        let mut compare = op(2, Kind::Sub, vec![flags], vec![following]);
        compare.args = vec![
            Arg::Held(Held {
                value: following,
                width: 2,
            }),
            Arg::Const(Const::new(2, 2)),
        ];
        body.blocks[2]
            .ops
            .last_mut()
            .expect("fixture branch")
            .uses
            .push(flags);
        body.blocks[2].ops.insert(1, compare);
        assert_eq!(trip_count(&body, &loop_, &constants::known(&body)), None);
    }
}
