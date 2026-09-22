//! Which values are affine functions of a loop's counter.
//!
//! Port of `qbopt/analysis/induction.py`.  `id(op)` is an [`OpOccurrence`]
//! of the analysed body; `floor_div`, `mod_floor`, `modular_inverse` and
//! `gcd` are Python's `//`, `%`, `pow(x, -1, m)` and `math.gcd` on `BigInt`.

use std::cmp::max;
use std::collections::{BTreeMap, BTreeSet};
use crate::support::hash::HashSet;

use crate::support::hash::IndexMap;

use num_bigint::BigInt;

use super::consts::{self, Known, masked};
use super::occurrence::{OpOccurrence, PhiOccurrence, operations, phis};
use super::ranges;
use super::regions::{RegionError, RegionLayout, overlapping};
use crate::analysis::loops::{self, Loop, predecessors};
use crate::model::mir::{Arg, Const, Held, Kind, MemRef, MirBody, Op, OrderedMap, Value};
use crate::support::pyset::PySet;

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
    pub(crate) const fn width(&self) -> u32 {
        match self {
            Self::Held(held) => held.width,
            Self::Const(constant) => constant.width,
        }
    }

    pub(crate) fn as_arg(&self) -> Arg {
        match self {
            Self::Held(held) => Arg::Held(*held),
            Self::Const(constant) => Arg::Const(constant.clone()),
        }
    }

    /// Python's `isinstance(arg, (mir.Held, mir.Const))`, as the union.
    pub(crate) fn from_arg(arg: &Arg) -> Option<Self> {
        match arg {
            Arg::Held(held) => Some(Self::Held(*held)),
            Arg::Const(constant) => Some(Self::Const(constant.clone())),
            _ => None,
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

/// Python's local `forms` map in `induction._extended`.
///
/// It is deliberately insertion-ordered, as are the preceding derived-form
/// passes.  The offset vector likewise preserves every term and its order;
/// this proof only reads it and must not normalize it into another analysis.
type ExtendedForms = OrderedMap<u32, (Affine, BigInt, Vec<(Arg, BigInt)>)>;

/// An operation that computes an affine value from another one.
///
/// Direct port of `qbopt.analysis.induction:Derived`.  Python retains the
/// exact immutable `mir.Op` object that produced the formula; an
/// [`OpOccurrence`] is the corresponding identity in one immutable Rust MIR
/// snapshot.  It is deliberately neither source provenance (`Op.id`) nor a
/// source address nor structural operation equality.
///
/// `offsets` remains an ordered vector, rather than a map: Python retains
/// duplicate invariant terms and their declaration order exactly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Derived {
    pub op: OpOccurrence,
    pub of: Affine,
    pub by: Arg,
    pub offsets: Vec<(Arg, BigInt)>,
    pub pointer: Option<Arg>,
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
    pub start: AffineOperand,
    pub bound: AffineOperand,
    /// The comparison that continues the loop.
    pub test: Kind,
    pub preheader: i64,
    pub latch: i64,
    pub entered: i64,
    pub exit: i64,
    pub maximum: Option<BigInt>,
}

impl CountedLoop {
    /// Python's `CountedLoop.inclusive` property.
    pub(crate) const fn inclusive(&self) -> bool {
        matches!(self.test, Kind::Le | Kind::BelowEq)
    }
}

/// Python's `_SKIPPED.get(test)`.
#[allow(non_snake_case)]
const fn _SKIPPED(test: Kind) -> Option<Kind> {
    match test {
        Kind::Below => Some(Kind::BelowEq),
        Kind::Lt => Some(Kind::Le),
        Kind::BelowEq => Some(Kind::Below),
        Kind::Le => Some(Kind::Lt),
        _ => None,
    }
}

/// Python's `Computed`: places one preheader operation and returns its result.
pub(crate) type Computed<'a> = dyn FnMut(Kind, Vec<Arg>) -> AffineOperand + 'a;

/// The preheader comparison, and the test on it, under which the loop runs no trips.
pub(crate) fn skipped(proof: &CountedLoop) -> ((AffineOperand, AffineOperand), Kind) {
    let mut test = _SKIPPED(proof.test).expect("KeyError");
    if test == Kind::BelowEq && proof.start == AffineOperand::Const(Const::new(0, proof.start.width())) {
        test = Kind::Eq; // nothing is below zero
    }
    ((proof.bound.clone(), proof.start.clone()), test)
}

/// Trips on the entered path, exact modulo the counter's width.
///
/// `computed(kind, args)` places one preheader operation and returns its
/// result. `counted` proved the count fits: an exclusive test cannot reach
/// the width's size, and an inclusive one is proved finite first.
pub(crate) fn trips(proof: &CountedLoop, computed: &mut Computed<'_>) -> AffineOperand {
    let width = proof.bound.width();
    if let (AffineOperand::Const(bound), AffineOperand::Const(start)) = (&proof.bound, &proof.start) {
        let count = &bound.n - &start.n + BigInt::from(u8::from(proof.inclusive()));
        return AffineOperand::Const(Const::new(masked(&count, width), width));
    }
    let count = computed(Kind::Sub, vec![proof.bound.as_arg(), proof.start.as_arg()]);
    computed(Kind::Add, vec![count.as_arg(), Arg::Const(Const::new(u8::from(proof.inclusive()), width))])
}

/// The counter as a loop that ran a trip leaves: the first value failing its test.
pub(crate) fn exit_value(proof: &CountedLoop, computed: &mut Computed<'_>) -> AffineOperand {
    let width = proof.bound.width();
    if let AffineOperand::Const(bound) = &proof.bound {
        let value = &bound.n + BigInt::from(u8::from(proof.inclusive()));
        return AffineOperand::Const(Const::new(masked(&value, width), width));
    }
    computed(Kind::Add, vec![proof.bound.as_arg(), Arg::Const(Const::new(u8::from(proof.inclusive()), width))])
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
    /// Exit-block phis reading the counter as the loop leaves: `exit_value`
    /// after a trip, `start` after none.
    pub exits: Vec<PhiOccurrence>,
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
    facts: &IndexMap<Value, Known>,
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

/// Python's `derived_map(formula, facts)`.
///
/// This deliberately proves only the constant modular map carried by one
/// already-recognized `Derived` formula.  Recognition, pointer formation, and
/// recurrence discovery belong to their Python-equivalent callers; this is
/// not a second scalar-evolution solver.
pub(crate) fn derived_map(formula: &Derived, facts: &IndexMap<Value, Known>) -> Option<AffineMap> {
    let width = formula.of.start.width();
    let scale = _signed(&formula.by, facts, width)?;
    if scale == BigInt::from(0_u8) || formula.pointer.is_some() {
        return None;
    }
    let modulus = BigInt::from(1_u8) << (width * 8);
    let mut offset = BigInt::from(0_u8);
    for (value, coefficient) in &formula.offsets {
        let constant = _constant(value, facts, width)?;
        offset = mod_floor(&(offset + constant * coefficient), &modulus);
    }
    Some(AffineMap {
        scale,
        offset,
        width,
    })
}

/// Python's `_quotients(body, loop, found)`.
///
/// Exact division of a non-wrapping recurrence is another recurrence.  The
/// operation occurrence comes from this immutable body snapshot, preserving
/// Python's exact `mir.Op` identity rather than source operation provenance.
fn _quotients(body: &MirBody, loop_: &Loop, found: &OrderedMap<u32, Affine>) -> Vec<Derived> {
    let facts = consts::known(body, None, None, None, None);
    let mut out = Vec::new();
    for (occurrence, block, operation) in operations(body) {
        if !loop_.body.contains(&block.at) || block.at == loop_.header {
            continue;
        }
        if operation.kind != Kind::Divmod
            || operation.args.len() != 2
            || operation.results.len() != 2
        {
            continue;
        }
        if !operation.loads.is_empty()
            || !operation.stores.is_empty()
            || operation.barrier()
            || !operation
                .results
                .iter()
                .all(|result| matches!(result, Arg::Held(held) if held.width == 2))
        {
            continue;
        }
        let Arg::Held(dividend) = &operation.args[0] else {
            continue;
        };
        if dividend.width != 2 {
            continue;
        }
        let Some(counter) = found.get(&dividend.value.id) else {
            continue;
        };
        let Some(start) = _signed(&counter.start.as_arg(), &facts, 2) else {
            continue;
        };
        let Some(step) = _signed(&counter.step.as_arg(), &facts, 2) else {
            continue;
        };
        let Some(denominator) = _signed(&operation.args[1], &facts, 2) else {
            continue;
        };
        if denominator == BigInt::from(0_u8)
            || mod_floor(&start, &denominator) != BigInt::from(0_u8)
            || mod_floor(&step, &denominator) != BigInt::from(0_u8)
        {
            continue;
        }
        let Some(last) = _last_counter(body, loop_, counter, &facts, 2) else {
            continue;
        };
        let quotient_start = floor_div(&start, &denominator);
        let quotient_last = floor_div(&last, &denominator);
        if quotient_start < BigInt::from(-32768_i32)
            || quotient_start > BigInt::from(32767_i32)
            || quotient_last < BigInt::from(-32768_i32)
            || quotient_last > BigInt::from(32767_i32)
        {
            continue;
        }
        out.push(Derived {
            op: occurrence,
            of: Affine {
                value: counter.value,
                start: AffineOperand::Const(Const::new(quotient_start, 2)),
                step: AffineOperand::Const(Const::new(floor_div(&step, &denominator), 2)),
                header: loop_.header,
            },
            by: Arg::Const(Const::new(1, 2)),
            offsets: Vec::new(),
            pointer: None,
        });
    }
    out
}

/// Python's `domain(body, loop, affine, facts)`.
///
/// `_signed` establishes the initial integer interpretation and
/// `_last_counter` is the sole finite-loop proof.  Keeping their exact calls
/// here prevents this consumer from becoming an independent range solver.
pub(crate) fn domain(
    body: &MirBody,
    loop_: &Loop,
    affine: &Affine,
    facts: &IndexMap<Value, Known>,
) -> Option<(BigInt, BigInt)> {
    let width = affine.start.width();
    let start = _signed(&affine.start.as_arg(), facts, width)?;
    let last = _last_counter(body, loop_, affine, facts, width)?;
    Some(if start <= last {
        (start, last)
    } else {
        (last, start)
    })
}

/// Python's `_extended(body, loop, op, forms, facts)`.
///
/// An extension carries a narrow affine recurrence into a wider one only
/// after the exact finite-loop proof establishes that the narrow value cannot
/// wrap.  This is intentionally the Python case split, not a general range
/// analysis: `_last_counter` supplies the only loop-end fact, and every
/// operand must be an exact constant at the narrow recurrence width.
fn _extended(
    body: &MirBody,
    loop_: &Loop,
    op: &Op,
    forms: &ExtendedForms,
    facts: &IndexMap<Value, Known>,
) -> Option<(Affine, BigInt, Vec<(Arg, BigInt)>)> {
    if op.args.len() != 1
        || op.results.len() != 1
        || !op.loads.is_empty()
        || !op.stores.is_empty()
        || op.barrier()
        || !op.merges.is_empty()
    {
        return None;
    }
    let (Arg::Held(source), Arg::Held(result)) = (&op.args[0], &op.results[0]) else {
        return None;
    };
    if source.width >= result.width {
        return None;
    }
    let (counter, scale, offsets) = forms.get(&source.value.id)?;
    let width = source.width;
    if counter.start.width() != width {
        return None;
    }
    let raw_start = _constant(&counter.start.as_arg(), facts, width)?;
    let raw_step = _constant(&counter.step.as_arg(), facts, width)?;
    let last = _last_counter(body, loop_, counter, facts, width)?;
    let constants = offsets
        .iter()
        .map(|(argument, coefficient)| {
            Some((_constant(argument, facts, width)?, coefficient.clone()))
        })
        .collect::<Option<Vec<_>>>()?;
    let step = _as_signed(&raw_step, width);
    if step == BigInt::from(0_u8) {
        return None;
    }
    let start = match op.kind {
        Kind::SignExtend => _as_signed(&raw_start, width),
        Kind::ZeroExtend if last >= BigInt::from(0_u8) => raw_start.clone(),
        _ => return None,
    };
    let distance = &last - &start;
    if &distance * &step < BigInt::from(0_u8) || &distance % &step != BigInt::from(0_u8) {
        return None;
    }
    let count = &distance / &step + 1_u8;
    if count <= BigInt::from(0_u8) {
        return None;
    }

    let mask = (BigInt::from(1_u8) << (width * 8)) - 1_u8;
    let sign = BigInt::from(1_u8) << (width * 8 - 1);
    let (initial, stride, low, high) = if op.kind == Kind::SignExtend {
        let signed_scale = _as_signed(&(scale & &mask), width);
        let initial = &start * &signed_scale
            + constants
                .iter()
                .fold(BigInt::from(0_u8), |sum, (value, coefficient)| {
                    sum + _as_signed(value, width) * coefficient
                });
        (initial, &step * signed_scale, -sign.clone(), sign.clone())
    } else {
        let initial = masked(
            &(raw_start.clone() * scale
                + constants
                    .iter()
                    .fold(BigInt::from(0_u8), |sum, (value, coefficient)| {
                        sum + value * coefficient
                    })),
            width,
        );
        let raw_stride = masked(&(raw_step * scale), width);
        // Half the modulus has two equally valid directions. Without another
        // semantic fact, choosing either would invent a wide recurrence.
        if raw_stride == sign && count > BigInt::from(1_u8) {
            return None;
        }
        (
            initial,
            _as_signed(&raw_stride, width),
            BigInt::from(0_u8),
            mask + 1_u8,
        )
    };
    let final_value = &initial + (&count - 1_u8) * &stride;
    if initial < low || initial >= high || final_value < low || final_value >= high {
        return None;
    }
    Some((
        Affine {
            value: result.value.id,
            start: AffineOperand::Const(Const::new(masked(&initial, result.width), result.width)),
            step: AffineOperand::Const(Const::new(masked(&stride, result.width), result.width)),
            header: loop_.header,
        },
        BigInt::from(1_u8),
        Vec::new(),
    ))
}

/// Python's `_multiplier(op, by)`.
///
/// A left shift records its count rather than its scale, so its derived
/// recurrence multiplies by `1 << count`.  Its caller establishes whether
/// that count is valid for the operation width; this helper deliberately
/// only translates the operand representation.
fn _multiplier(op: &Op, by: &Arg) -> Arg {
    if op.kind != Kind::Shl {
        return by.clone();
    }
    let Arg::Const(constant) = by else {
        return by.clone();
    };
    let shift: usize = constant
        .n
        .clone()
        .try_into()
        .expect("shift count must fit Rust address space");
    Arg::Const(Const::new(
        BigInt::from(1_u8) << shift,
        max(constant.width, 2),
    ))
}

/// Python's `_composed(body, loop, found, made, settled)`.
///
/// This is deliberately a fixed-point scan over the original immutable body
/// order.  `forms` models Python's insertion-ordered local dictionary, while
/// `out` models its `id(op)`-keyed result dictionary: recalculating one
/// operation overwrites its formula without moving that operation's output
/// position.  [`OpOccurrence`] is the corresponding snapshot-local identity.
fn _composed<F>(
    body: &MirBody,
    loop_: &Loop,
    found: &OrderedMap<u32, Affine>,
    made: &BTreeMap<u32, &Op>,
    settled: F,
) -> Result<Vec<Derived>, RegionError>
where
    F: Fn(&MemRef) -> Result<bool, RegionError>,
{
    let inside = loop_.body.clone();
    let known = consts::known(body, None, None, None, None);
    let still = invariant(body, &inside);
    let mut forms = OrderedMap::new();
    for (value, recurrence) in found.iter() {
        if matches!(
            recurrence.start,
            AffineOperand::Held(_) | AffineOperand::Const(_)
        ) && matches!(recurrence.start.width(), 2 | 4)
        {
            forms.insert(*value, (recurrence.clone(), BigInt::from(1_u8), Vec::new()));
        }
    }
    let mut out = OrderedMap::<OpOccurrence, Derived>::new();
    let mut changed = true;
    while changed {
        changed = false;
        for (occurrence, block, operation) in operations(body) {
            if !inside.contains(&block.at) {
                continue;
            }
            if block.at != loop_.header
                && matches!(operation.kind, Kind::SignExtend | Kind::ZeroExtend)
                && !operation.results.is_empty()
                && matches!(operation.results[0], Arg::Held(_))
                && !matches!(&operation.results[0], Arg::Held(result) if forms.contains_key(&result.value.id))
            {
                if let Some(extended) = _extended(body, loop_, operation, &forms, &known) {
                    let Arg::Held(result) = &operation.results[0] else {
                        unreachable!("checked above");
                    };
                    forms.insert(result.value.id, extended);
                    changed = true;
                }
            }
            if operation.kind == Kind::PtrOffset
                && operation.loads.is_empty()
                && operation.stores.is_empty()
                && !operation.barrier()
                && operation.merges.is_empty()
                && operation.args.len() == 2
                && operation.results.len() == 1
            {
                let pointer = &operation.args[0];
                let offset = &operation.args[1];
                let result = &operation.results[0];
                let form = match offset {
                    Arg::Held(offset) => forms.get(&offset.value.id),
                    _ => None,
                };
                if let (
                    Arg::Held(pointer),
                    Arg::Held(offset),
                    Arg::Held(result),
                    Some((counter, scale, offsets)),
                ) = (pointer, offset, result, form)
                {
                    if still.contains(&pointer.value.id)
                        && pointer.width == offset.width
                        && offset.width == result.width
                        && counter.start.width() == result.width
                    {
                        out.insert(
                            occurrence,
                            Derived {
                                op: occurrence,
                                of: counter.clone(),
                                by: Arg::Const(Const::new(
                                    masked(scale, result.width),
                                    result.width,
                                )),
                                offsets: offsets.clone(),
                                pointer: Some(Arg::Held(*pointer)),
                            },
                        );
                    }
                }
                // A pointer offset is never a numeric composed formula too.
                continue;
            }
            if !operation.stores.is_empty()
                || operation.barrier()
                || operation.args.len() != 2
                || operation.results.is_empty()
            {
                continue;
            }
            let loads = operation.loads.iter().collect::<HashSet<_>>();
            let argument_loads = operation
                .args
                .iter()
                .filter_map(|argument| match argument {
                    Arg::Cell(cell) => Some(&cell.r#ref),
                    _ => None,
                })
                .collect::<HashSet<_>>();
            if loads != argument_loads {
                continue;
            }
            let Arg::Held(result) = &operation.results[0] else {
                continue;
            };
            if !matches!(result.width, 2 | 4) || forms.contains_key(&result.value.id) {
                continue;
            }
            let width = result.width;
            let mut arguments = operation
                .args
                .iter()
                .map(|argument| match argument {
                    Arg::Held(held) => Arg::Held(_copied(*held, made)),
                    _ => argument.clone(),
                })
                .collect::<Vec<_>>();
            for argument in &mut arguments {
                let Arg::Held(held) = argument else {
                    continue;
                };
                let Some(fact) = known.get(&held.value) else {
                    continue;
                };
                if held.width == width && !forms.contains_key(&held.value.id) && fact.width >= width
                {
                    *argument = Arg::Const(Const::new(masked(&fact.n, width), width));
                }
            }
            let left = &arguments[0];
            let right = &arguments[1];
            let first = match left {
                Arg::Held(held) if held.width == width => forms.get(&held.value.id),
                _ => None,
            };
            let second = match right {
                Arg::Held(held) if held.width == width => forms.get(&held.value.id),
                _ => None,
            };
            if [first, second]
                .into_iter()
                .flatten()
                .any(|(counter, _, _)| counter.start.width() != width)
            {
                continue;
            }
            if matches!(operation.kind, Kind::And | Kind::Or) && left == right && first.is_some() {
                forms.insert(result.value.id, first.expect("checked above").clone());
                changed = true;
                continue;
            }
            let (base, scale, offsets) = if matches!(operation.kind, Kind::Add | Kind::Sub)
                && first.is_some()
                && second.is_some()
            {
                let (first_base, first_scale, first_offsets) = first.expect("checked above");
                let (second_base, second_scale, second_offsets) = second.expect("checked above");
                if first_base != second_base {
                    continue;
                }
                let scale = if operation.kind == Kind::Add {
                    first_scale + second_scale
                } else {
                    first_scale - second_scale
                };
                let sign = if operation.kind == Kind::Add { 1 } else { -1 };
                let offsets = first_offsets
                    .iter()
                    .cloned()
                    .chain(
                        second_offsets
                            .iter()
                            .map(|(argument, coefficient)| (argument.clone(), coefficient * sign)),
                    )
                    .collect();
                (first_base.clone(), scale, offsets)
            } else if (operation.kind == Kind::Add && (first.is_some() || second.is_some()))
                || (operation.kind == Kind::Sub && first.is_some() && second.is_none())
            {
                let (recurrence, offset) = if first.is_some() {
                    (first.expect("checked above"), right)
                } else {
                    (second.expect("add has one recurrence"), left)
                };
                match offset {
                    Arg::Cell(cell) => {
                        let reference = &cell.r#ref;
                        if reference.addr.is_none()
                            || reference.base.is_some_and(|base| !still.contains(&base.id))
                            || reference.segment.is_some()
                            || reference.width != width
                            || !settled(reference)?
                        {
                            continue;
                        }
                    }
                    Arg::Const(constant) if constant.width == width => {}
                    Arg::Held(held) if held.width == width && still.contains(&held.value.id) => {}
                    _ => continue,
                }
                let (base, scale, prior_offsets) = recurrence;
                let mut offsets = prior_offsets.clone();
                offsets.push((
                    offset.clone(),
                    BigInt::from(if operation.kind == Kind::Sub { -1 } else { 1 }),
                ));
                (base.clone(), scale.clone(), offsets)
            } else if operation.kind == Kind::Mul {
                if let (Some((base, scale, offsets)), Arg::Const(constant)) = (first, right) {
                    if constant.width != width {
                        continue;
                    }
                    (
                        base.clone(),
                        scale * &constant.n,
                        offsets
                            .iter()
                            .map(|(argument, coefficient)| {
                                (argument.clone(), coefficient * &constant.n)
                            })
                            .collect(),
                    )
                } else if let (Some((base, scale, offsets)), Arg::Const(constant)) = (second, left)
                {
                    if constant.width != width {
                        continue;
                    }
                    (
                        base.clone(),
                        scale * &constant.n,
                        offsets
                            .iter()
                            .map(|(argument, coefficient)| {
                                (argument.clone(), coefficient * &constant.n)
                            })
                            .collect(),
                    )
                } else {
                    continue;
                }
            } else if operation.kind == Kind::Shl && first.is_some() {
                let (base, scale, offsets) = first.expect("checked above");
                let Arg::Const(amount) = right else {
                    continue;
                };
                if amount.n < BigInt::from(0_u8) || amount.n >= BigInt::from(width * 8) {
                    continue;
                }
                let shift: usize = amount
                    .n
                    .clone()
                    .try_into()
                    .expect("bounded non-negative shift count");
                (
                    base.clone(),
                    scale << shift,
                    offsets
                        .iter()
                        .map(|(argument, coefficient)| (argument.clone(), coefficient << shift))
                        .collect(),
                )
            } else {
                continue;
            };
            let scale = masked(&scale, width);
            forms.insert(
                result.value.id,
                (base.clone(), scale.clone(), offsets.clone()),
            );
            out.insert(
                occurrence,
                Derived {
                    op: occurrence,
                    of: base,
                    by: Arg::Const(Const::new(scale, width)),
                    offsets,
                    pointer: None,
                },
            );
            changed = true;
        }
    }
    Ok(out.values().cloned().collect())
}

/// Whether a cell is one no store inside the loop can reach.
///
/// `mir.overlapping` hands `bounds` to regions and `dgroup` as a layout
/// that is not a `module.Group`: a landmarks-only [`RegionLayout`] is both.
fn unwritten<'a>(
    body: &'a MirBody,
    inside: &'a BTreeSet<i64>,
    dgroup: &'a BTreeSet<i64>,
    bounds: Option<&'a RegionLayout>,
) -> impl Fn(&MemRef) -> Result<bool, RegionError> + 'a {
    let wrote = body
        .blocks
        .iter()
        .filter(|block| inside.contains(&block.at))
        .flat_map(|block| block.ops.iter())
        .flat_map(|operation| operation.stores.iter())
        .collect::<Vec<_>>();
    // With constants, as hoist asks.
    let known = if wrote.is_empty() {
        BTreeMap::new()
    } else {
        ranges::constants(body, Some(dgroup), None).into_iter().collect()
    };

    move |cell| {
        for store in &wrote {
            if overlapping(cell, store, Some(&known), Some(&known), bounds)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// Every multiply inside the loop whose operand is one of its counters.
pub(crate) fn derived(
    body: &MirBody,
    loop_: &Loop,
    found: Option<&OrderedMap<u32, Affine>>,
    dgroup: &BTreeSet<i64>,
    bounds: Option<&RegionLayout>,
) -> Result<Vec<Derived>, RegionError> {
    let at_of = body
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.at, index))
        .collect::<BTreeMap<_, _>>();
    let inside = loop_
        .body
        .iter()
        .copied()
        .filter(|at| at_of.contains_key(at))
        .collect::<PySet<_>>();
    let members = inside.iter().copied().collect::<BTreeSet<_>>();
    let calculated;
    let found = match found {
        Some(found) => found,
        None => {
            calculated = basics(body, loop_);
            &calculated
        }
    };
    if found.is_empty() {
        return Ok(Vec::new());
    }
    let still = invariant(body, &members);
    let settled = unwritten(body, &members, dgroup, bounds);
    // Python's dictionary comprehension is last-definition-wins in body,
    // block, operation, and defined-value order.
    let mut made = BTreeMap::<u32, &Op>::new();
    for block in &body.blocks {
        for operation in &block.ops {
            for value in &operation.defines {
                made.insert(value.id, operation);
            }
        }
    }

    let mut direct = OrderedMap::<OpOccurrence, Derived>::new();
    for at in inside.iter() {
        let block_index = at_of[at];
        for (occurrence, block, operation) in operations(body) {
            if occurrence.block_index() != block_index || block.at != *at {
                continue;
            }
            if operation.kind == Kind::PtrOffset
                && operation.args.len() == 2
                && operation.results.len() == 1
                && matches!(&operation.results[0], Arg::Held(result) if result.width == 4)
                && operation.loads.is_empty()
                && operation.stores.is_empty()
                && !operation.barrier()
                && operation.merges.is_empty()
            {
                let (pointer, offset) = (&operation.args[0], &operation.args[1]);
                if let (Arg::Held(pointer), Arg::Held(offset)) = (pointer, offset) {
                    let recurrence = found.get(&offset.value.id);
                    if still.contains(&pointer.value.id)
                        && pointer.width == offset.width
                        && recurrence.is_some_and(|one| offset.width == one.start.width())
                        && offset.width == 4
                    {
                        direct.insert(
                            occurrence,
                            Derived {
                                op: occurrence,
                                of: recurrence.expect("checked above").clone(),
                                by: Arg::Const(Const::new(1, 4)),
                                offsets: Vec::new(),
                                pointer: Some(Arg::Held(*pointer)),
                            },
                        );
                    }
                }
                continue;
            }
            // A Cell multiplier is an allowed load.  Only stores reject the
            // direct multiply/shift recognizer, as in Python.
            if !matches!(operation.kind, Kind::Mul | Kind::Shl) || !operation.stores.is_empty() {
                continue;
            }
            let arguments = operation
                .args
                .iter()
                .map(|argument| match argument {
                    Arg::Held(held) => Arg::Held(_copied(*held, &made)),
                    _ => argument.clone(),
                })
                .collect::<Vec<_>>();
            let counters = arguments
                .iter()
                .filter(|argument| {
                    matches!(argument, Arg::Held(held) if found.contains_key(&held.value.id))
                })
                .collect::<Vec<_>>();
            let others = arguments
                .iter()
                .filter(|argument| !counters.iter().any(|counter| *counter == *argument))
                .collect::<Vec<_>>();
            if counters.len() != 1 || others.len() != 1 {
                continue;
            }
            let Arg::Held(counter) = counters[0] else {
                unreachable!("counter filter retains Held operands only");
            };
            let by = others[0];
            if operation.kind == Kind::Shl
                && (arguments[0] != *counters[0]
                    || !matches!(by, Arg::Const(constant) if constant.n >= BigInt::from(0_u8) && constant.n < BigInt::from(counter.width) * 8_u8))
            {
                continue;
            }
            if matches!(by, Arg::Held(held) if !still.contains(&held.value.id)) {
                continue;
            }
            if let Arg::Cell(cell) = by {
                if !settled(&cell.r#ref)? {
                    continue;
                }
            }
            direct.insert(
                occurrence,
                Derived {
                    op: occurrence,
                    of: found
                        .get(&counter.value.id)
                        .expect("counter filter established recurrence")
                        .clone(),
                    by: _multiplier(operation, by),
                    offsets: Vec::new(),
                    pointer: None,
                },
            );
        }
    }

    // Python creates one insertion-ordered dict from direct formulas, then
    // updates it with composed and quotient formulas.  `OrderedMap::insert`
    // overwrites in place, retaining that exact output position.
    for formula in _composed(body, loop_, found, &made, &settled)? {
        direct.insert(formula.op, formula);
    }
    for formula in _quotients(body, loop_, found) {
        direct.insert(formula.op, formula);
    }
    Ok(direct.values().cloned().collect())
}

/// Every loop in this body, with its counters and what they derive.
#[allow(clippy::type_complexity)]
pub(crate) fn of(
    body: &MirBody,
    dgroup: &BTreeSet<i64>,
    bounds: Option<&RegionLayout>,
) -> Result<Vec<(Loop, OrderedMap<u32, Affine>, Vec<Derived>)>, RegionError> {
    let mut result = Vec::new();
    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        let found = basics(body, &loop_);
        if found.is_empty() {
            continue;
        }
        let formulas = derived(body, &loop_, Some(&found), dgroup, bounds)?;
        result.push((loop_, found, formulas));
    }
    Ok(result)
}

/// Prove every canonical `start ..< bound` or `start ..= bound` unit control recurrence.
///
/// An exclusive test stops the counter before it can wrap.  An inclusive
/// one runs forever where `bound` is its type's maximum, so it is proved
/// only where that cannot happen: a constant below it, or a finite
/// `maximum`.
pub(crate) fn counted(body: &MirBody, loop_: &Loop, facts: Option<&IndexMap<Value, Known>>) -> Vec<CountedLoop> {
    let computed;
    let facts = match facts {
        Some(facts) => facts,
        None => {
            computed = consts::known(body, None, None, None, None);
            &computed
        }
    };
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
    let Some(test) = _continuing_test(branch, inside).filter(|test| _SKIPPED(*test).is_some()) else {
        return Vec::new();
    };
    let unsigned = matches!(test, Kind::Below | Kind::BelowEq);
    let inclusive = matches!(test, Kind::Le | Kind::BelowEq);
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
        // Python's `isinstance(counter.start, (Held, Const))` is `AffineOperand`'s type.
        if _signed(&counter.step.as_arg(), facts, width) != Some(BigInt::from(1_u8)) {
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
        let start = _constant(&counter.start.as_arg(), facts, width);
        let limit = _constant(&bound.as_arg(), facts, width);
        let (first, last, top) = if unsigned {
            (start.clone(), limit.clone(), (BigInt::from(1_u8) << (8 * width)) - 1)
        } else {
            (
                start.as_ref().map(|start| _as_signed(start, width)),
                limit.as_ref().map(|limit| _as_signed(limit, width)),
                (BigInt::from(1_u8) << (8 * width - 1)) - 1,
            )
        };
        let one = BigInt::from(u8::from(inclusive));
        if inclusive && last.as_ref() == Some(&top) {
            continue;
        }
        let lowest = first.or_else(|| _range(body, &counter.start, 0, &top));
        let highest = last.clone().or_else(|| _range(body, &bound, 1, &(&top - &one)));
        let mut maximum = None;
        if let (Some(lowest), Some(highest)) = (&lowest, &highest) {
            maximum = Some(max(BigInt::from(0_u8), highest - lowest + &one));
        }
        if maximum.is_none() {
            maximum = _inbounds_trips(body, loop_, shape.latch);
        }
        if inclusive && last.is_none() && maximum.is_none() {
            continue;
        }
        proven.push(CountedLoop {
            counter: counter.clone(),
            phi: phi_occurrence,
            compare: compare_occurrence,
            branch: branch_occurrence,
            start: start.map_or_else(|| counter.start.clone(), |start| AffineOperand::Const(Const::new(start, width))),
            bound: limit.map_or_else(|| bound.clone(), |limit| AffineOperand::Const(Const::new(limit, width))),
            test,
            preheader: shape.preheader,
            latch: shape.latch,
            entered: shape.entered,
            exit: shape.exit,
            maximum,
        });
    }
    proven
}

/// How far each counter and each value affine in one advances per iteration.
///
/// The bytes-per-iteration view of `basics` and `derived`; nothing here
/// re-derives which values are affine.
pub(crate) fn advances(body: &MirBody, loop_: &Loop) -> IndexMap<Value, BigInt> {
    let found = basics(body, loop_);
    let header = body.blocks.iter().find(|block| block.at == loop_.header).expect("the loop's header is a block");
    let mut out = IndexMap::default();
    for phi in &header.phis {
        if let Some(Affine { step: AffineOperand::Const(step), .. }) = found.get(&phi.result.id) {
            out.insert(phi.result, _as_signed(&step.n, step.width));
        }
    }
    // Python's `derived` cannot fail; an endpoint Rust cannot hold drops only
    // the derived entries, which leaves fewer, never wrong, advances.
    for one in derived(body, loop_, Some(&found), &BTreeSet::new(), None).unwrap_or_default() {
        let op = &body.blocks[one.op.block_index()].ops[one.op.operation_index()];
        if let (AffineOperand::Const(step), Arg::Const(by), None, [Arg::Held(result)]) =
            (&one.of.step, &one.by, &one.pointer, op.results.as_slice())
        {
            out.insert(result.value, _as_signed(&step.n, step.width) * _as_signed(&by.n, by.width));
        }
    }
    out.into_iter().filter(|(_, step)| *step != BigInt::from(0_u8)).collect()
}

/// The low (`end` 0) or high (`end` 1) of a frontend range on `arg`, if it lies in `0 ..= top`.
fn _range(body: &MirBody, arg: &AffineOperand, end: usize, top: &BigInt) -> Option<BigInt> {
    let AffineOperand::Held(held) = arg else {
        return None;
    };
    let interval = body.integer_ranges.get(&held.value)?;
    if interval.width != held.width || interval.low < BigInt::from(0_u8) || &interval.high > top {
        return None;
    }
    Some([&interval.low, &interval.high][end].clone())
}

/// The most iterations an access made every iteration allows, as LLVM's inbounds does.
///
/// Iteration i reaches `b + i*s` inside one object, and an offset `w` bytes
/// wide addresses at most 2**(8w) of them, so i*s + width <= 2**(8w).
fn _inbounds_trips(body: &MirBody, loop_: &Loop, latch: i64) -> Option<BigInt> {
    let step = advances(body, loop_);
    let dominators = loops::dominators(&body.blocks, Some(body.entry));
    let empty = BTreeSet::new();
    let every = dominators.get(&latch).unwrap_or(&empty);
    body.blocks
        .iter()
        // The header also runs the final, failing test: n + 1 times.
        .filter(|block| loop_.body.contains(&block.at) && every.contains(&block.at) && block.at != loop_.header)
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.loads.iter().chain(&op.stores))
        .filter_map(|reference| {
            let advance = step.get(&reference.base?)?;
            Some(
                ((BigInt::from(1_u8) << (8 * reference.base_width)) - reference.width) / abs(advance)
                    + 1,
            )
        })
        .min()
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
    facts: &IndexMap<Value, Known>,
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
pub(crate) fn _signed(
    argument: &Arg,
    facts: &IndexMap<Value, Known>,
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
pub(crate) fn _last_counter(
    body: &MirBody,
    loop_: &Loop,
    counter: &Affine,
    facts: &IndexMap<Value, Known>,
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
    facts: &IndexMap<Value, Known>,
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
    facts: &IndexMap<Value, Known>,
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
    facts: &IndexMap<Value, Known>,
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
    let facts = consts::known(body, None, None, None, None);
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
pub(crate) fn floor_div(numerator: &BigInt, denominator: &BigInt) -> BigInt {
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
pub(crate) fn mod_floor(value: &BigInt, modulus: &BigInt) -> BigInt {
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

pub(crate) fn gcd(mut one: BigInt, mut other: BigInt) -> BigInt {
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
/// those, canonical control, transparent copies and exit phis are every observation of
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
    let (exit_index, exit) = blocks.get(&proof.exit).copied()?;
    let expected = OrderedMap::from_iter([(header.at, phi.result)]);
    let exits = phis(body)
        .filter(|(occurrence, _, other)| occurrence.block_index() == exit_index && other.incoming == expected)
        .map(|(occurrence, _, _)| occurrence)
        .collect::<Vec<_>>();
    let exit_phis = exits.iter().map(|at| &exit.phis[at.phi_index()]).collect::<Vec<_>>();
    if phis(body).any(|(occurrence, _, other)| {
        occurrence != proof.phi
            // Python's `other not in exits` compares phis by value.
            && !exit_phis.contains(&other)
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
        exits,
    })
}

/// Prove that `candidate` can supply a counted loop's terminating flags.
pub(crate) fn zero_terminating_control<'a>(
    body: &MirBody,
    loop_: &Loop,
    proof: &'a CountedLoop,
    candidate: &Affine,
    facts: Option<&IndexMap<Value, Known>>,
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
    let computed;
    let facts = match facts {
        Some(facts) => facts,
        None => {
            computed = consts::known(body, None, None, None, None);
            &computed
        }
    };
    let step = _signed(&candidate.step.as_arg(), facts, width)?;
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
#[path = "induction_tests.rs"]
mod tests;
