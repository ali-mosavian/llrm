//! Which values are affine functions of a loop's counter.
//!
//! Port of `qbopt/analysis/induction.py`.  `id(op)` is an [`OpOccurrence`]
//! of the analysed body; `floor_div`, `mod_floor`, `modular_inverse` and
//! `gcd` are Python's `//`, `%`, `pow(x, -1, m)` and `math.gcd` on `BigInt`.

use std::rc::Rc;
use std::cmp::max;
use std::collections::{BTreeMap, BTreeSet};
use crate::support::hash::HashSet;

use crate::support::hash::IndexMap;

use num_bigint::BigInt;

use super::consts::{self, Known, masked};
use super::occurrence::{OpOccurrence, PhiOccurrence, operations, phis};
use super::noreturn;
use super::ranges;
use super::regions::{RegionError, RegionLayout, overlapping};
use crate::analysis::loops::{self, Loop, predecessors};
use crate::model::mir::{self, Arg, Const, Held, Kind, MemRef, MirBody, Op, OrderedMap, Value};
use crate::support::pyset::PySet;

/// Python's `mir.Held | mir.Const` affine operand union.
///
/// Direct port of the closed annotation on `induction.Affine.start` and
/// `induction.Affine.step`; no other MIR operand can be a recurrence term.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum AffineOperand {
    Held(Held),
    Const(Const),
}

impl AffineOperand {
    pub const fn width(&self) -> u32 {
        match self {
            Self::Held(held) => held.width,
            Self::Const(constant) => constant.width,
        }
    }

    pub fn as_arg(&self) -> Arg {
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
pub struct Affine {
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
/// snapshot.  It is deliberately neither source provenance (`Op.source`) nor a
/// source address nor structural operation equality.
///
/// `offsets` remains an ordered vector, rather than a map: Python retains
/// duplicate invariant terms and their declaration order exactly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Derived {
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
pub struct AffineMap {
    pub scale: BigInt,
    pub offset: BigInt,
    pub width: u32,
}

/// The one proof of how many trips a loop makes, shared by every pass.
///
///     i = start; loop { [i test bound?] body; i += step; [i test bound?] }
///
/// `test` continues the loop, counter first; `step` is a nonzero
/// constant. A pre-tested loop tests the header value before each trip; a
/// post-tested one tests after each trip, the stepped value when
/// `stepped`. `width` is the compare's: a counter read narrower is
/// counted modulo that width.
///
/// `count` is the exact trip count when constant. `trips` places it
/// when symbolic, which needs a pre-tested unit step: the only proofs with
/// no `count`. `first` and `last` are the header's signed values on
/// the first and last trip, given only when nothing up to the exit wraps.
/// `maximum` bounds the trips when the count is unknown.
///
/// Direct port of `qbopt.analysis.induction:CountedLoop`.  Python stores the
/// proven phi and operations by object identity; Rust stores snapshot-local
/// occurrence keys for the same exact body instead.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CountedLoop {
    pub counter: Affine,
    pub phi: PhiOccurrence,
    pub compare: OpOccurrence,
    pub branch: OpOccurrence,
    pub start: AffineOperand,
    pub bound: AffineOperand,
    pub test: Kind,
    pub preheader: Option<i64>,
    pub latch: i64,
    pub entered: i64,
    pub exit: i64,
    pub maximum: Option<BigInt>,
    pub step: BigInt,
    pub posttested: bool,
    pub stepped: bool,
    /// Some other exit stops the program; `count` is the trips when it goes on.
    pub stops: bool,
    pub count: Option<BigInt>,
    pub first: Option<BigInt>,
    pub last: Option<BigInt>,
}

impl CountedLoop {
    /// Python's `CountedLoop.inclusive` property.
    pub const fn inclusive(&self) -> bool {
        _INCLUSIVE(self.test)
    }

    /// Python's `CountedLoop.width` property.
    pub const fn width(&self) -> u32 {
        self.bound.width()
    }

    /// The proven phi in `body`, the snapshot this proof indexes.
    pub fn phi_in<'b>(&self, body: &'b MirBody) -> &'b mir::Phi {
        &body.blocks[self.phi.block_index()].phis[self.phi.phi_index()]
    }

    /// The proven compare in `body`, the snapshot this proof indexes.
    pub fn compare_in<'b>(&self, body: &'b MirBody) -> &'b Op {
        &body.blocks[self.compare.block_index()].ops[self.compare.operation_index()]
    }

    /// The proven branch in `body`, the snapshot this proof indexes.
    pub fn branch_in<'b>(&self, body: &'b MirBody) -> &'b Op {
        &body.blocks[self.branch.block_index()].ops[self.branch.operation_index()]
    }

    /// The signed values the header's counter takes on a trip, lowest first.
    pub fn span(&self) -> Option<(BigInt, BigInt)> {
        let (first, last) = (self.first.as_ref()?, self.last.as_ref()?);
        Some((first.min(last).clone(), first.max(last).clone()))
    }
}

#[allow(non_snake_case)]
const fn _ASCENDING(test: Kind) -> bool {
    matches!(test, Kind::Lt | Kind::Le | Kind::Below | Kind::BelowEq)
}

#[allow(non_snake_case)]
const fn _DESCENDING(test: Kind) -> bool {
    matches!(test, Kind::Gt | Kind::Ge | Kind::Above | Kind::AboveEq)
}

#[allow(non_snake_case)]
const fn _INCLUSIVE(test: Kind) -> bool {
    matches!(test, Kind::Le | Kind::BelowEq | Kind::Ge | Kind::AboveEq)
}

#[allow(non_snake_case)]
const fn _UNSIGNED(test: Kind) -> bool {
    matches!(test, Kind::Below | Kind::BelowEq | Kind::Above | Kind::AboveEq)
}

/// The preheader test `bound SKIPPED start` under which no trip runs: `_SKIPPED[test]`.
#[allow(non_snake_case)]
fn _SKIPPED(test: Kind) -> Kind {
    assert!(_ASCENDING(test) || _DESCENDING(test) || test == Kind::Ne, "KeyError: {test:?}");
    mir::MIRRORED(mir::NEGATED(test).expect("a comparison negates")).expect("a comparison mirrors")
}

/// Python's `Computed`: places one preheader operation and returns its result.
pub type Computed<'a> = dyn FnMut(Kind, Vec<Arg>) -> AffineOperand + 'a;

/// The preheader comparison, and the test on it, under which the loop runs no trips.
pub fn skipped(proof: &CountedLoop) -> Option<((AffineOperand, AffineOperand), Kind)> {
    if proof.posttested {
        return None;
    }
    Some(((proof.bound.clone(), proof.start.clone()), _SKIPPED(proof.test)))
}

/// Trips on the entered path, exact modulo the compare's width, or None where not expressible.
///
/// `computed(kind, args)` places one preheader operation and returns its
/// result. `counted` proved the count finite.
pub fn trips(proof: &CountedLoop, computed: &mut Computed<'_>) -> Option<AffineOperand> {
    let width = proof.width();
    if proof.posttested {
        return None;
    }
    if let Some(count) = &proof.count {
        return (count < &(BigInt::from(1_u8) << (8 * width)))
            .then(|| AffineOperand::Const(Const::new(count.clone(), width)));
    }
    let (ahead, behind) =
        if proof.step > BigInt::from(0_u8) { (&proof.bound, &proof.start) } else { (&proof.start, &proof.bound) };
    let count = computed(Kind::Sub, vec![ahead.as_arg(), behind.as_arg()]);
    Some(computed(Kind::Add, vec![count.as_arg(), Arg::Const(Const::new(u8::from(proof.inclusive()), width))]))
}

/// The header's counter as a pre-tested loop that ran a trip leaves: the first value failing its test.
pub fn exit_value(proof: &CountedLoop, computed: &mut Computed<'_>) -> Option<AffineOperand> {
    let width = proof.width();
    if proof.posttested {
        return None;
    }
    if proof.test == Kind::Ne {
        return Some(proof.bound.clone());
    }
    if let (AffineOperand::Const(start), Some(count)) = (&proof.start, &proof.count) {
        return Some(AffineOperand::Const(Const::new(masked(&(&start.n + count * &proof.step), width), width)));
    }
    let past = Const::new(masked(&(&proof.step * u8::from(proof.inclusive())), width), width);
    if let AffineOperand::Const(bound) = &proof.bound {
        return Some(AffineOperand::Const(Const::new(masked(&(&bound.n + &past.n), width), width)));
    }
    Some(computed(Kind::Add, vec![proof.bound.as_arg(), Arg::Const(past)]))
}

/// Proof that a counted loop's source recurrence may be removed.
///
/// Direct port of `qbopt.analysis.induction:ControlReplacement`.  The
/// counted-loop proof is borrowed, retaining Python's `is` relationship for
/// the consumer.  Operations and phis use snapshot-local occurrences rather
/// than `Op.source` or structural equality.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlReplacement<'a> {
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
pub struct ZeroTerminatingControl<'a> {
    pub replacement: ControlReplacement<'a>,
    pub candidate: Affine,
    pub step: BigInt,
    pub maximum: BigInt,
    pub period: BigInt,
}

impl AffineMap {
    /// Python's `AffineMap.period` property.
    pub fn period(&self) -> BigInt {
        let modulus = BigInt::from(1_u8) << (self.width * 8);
        let scale = if self.scale < BigInt::from(0_u8) {
            -&self.scale
        } else {
            self.scale.clone()
        };
        modulus.clone() / gcd(scale, modulus)
    }

    /// Python's `AffineMap.injective(low, high)`.
    pub fn injective(&self, low: &BigInt, high: &BigInt) -> bool {
        self.scale != BigInt::from(0_u8) && high - low < self.period()
    }
}

/// Python's `relation(source, target, facts)`.
pub fn relation(
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
pub fn derived_map(formula: &Derived, facts: &IndexMap<Value, Known>) -> Option<AffineMap> {
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
fn _quotients(body: &Rc<MirBody>, loop_: &Loop, found: &OrderedMap<u32, Affine>) -> Vec<Derived> {
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
        let Some((low, high)) = domain(body, loop_, counter, &facts) else {
            continue;
        };
        if ![low, high].iter().all(|value| {
            let quotient = floor_div(value, &denominator);
            BigInt::from(-32768_i32) <= quotient && quotient <= BigInt::from(32767_i32)
        }) {
            continue;
        }
        let quotient_start = floor_div(&start, &denominator);
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

/// The finite inclusive signed domain `affine` takes on a trip.
pub fn domain(
    body: &Rc<MirBody>,
    loop_: &Loop,
    affine: &Affine,
    facts: &IndexMap<Value, Known>,
) -> Option<(BigInt, BigInt)> {
    controlling(body, loop_, affine, facts)?.span()
}

/// Python's `_extended(body, loop, op, forms, facts)`.
///
/// An extension carries a narrow affine recurrence into a wider one only
/// after the exact finite-loop proof establishes that the narrow value cannot
/// wrap.  The counted-loop proof supplies the only loop-end fact, and every
/// operand must be an exact constant at the narrow recurrence width.
fn _extended(
    body: &Rc<MirBody>,
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
    let raw_start = _constant(&counter.start.as_arg(), facts, width);
    let raw_step = _constant(&counter.step.as_arg(), facts, width);
    let count = controlling(body, loop_, counter, facts)
        .filter(|proof| proof.width() == width)
        .and_then(|proof| proof.count)
        .filter(|count| *count != BigInt::from(0_u8));
    let constants = offsets
        .iter()
        .map(|(argument, coefficient)| Some((_constant(argument, facts, width)?, coefficient.clone())))
        .collect::<Option<Vec<_>>>();
    let (Some(raw_start), Some(raw_step), Some(count), Some(constants)) = (raw_start, raw_step, count, constants) else {
        return None;
    };
    let step = _as_signed(&raw_step, width);
    if step == BigInt::from(0_u8) || !matches!(op.kind, Kind::SignExtend | Kind::ZeroExtend) {
        return None;
    }
    let start = if op.kind == Kind::SignExtend { _as_signed(&raw_start, width) } else { raw_start.clone() };

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
    body: &Rc<MirBody>,
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
    body: &'a Rc<MirBody>,
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
pub fn derived(
    body: &Rc<MirBody>,
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
pub fn of(
    body: &Rc<MirBody>,
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

/// Where a single-latch loop with one exit tests whether to go round again.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct _Control {
    block: i64,
    preheader: Option<i64>,
    entered: i64,
    exit: i64,
    posttested: bool,
    stops: bool,
}

/// The block whose final branch is the loop's only exit that goes on: its header, or its latch.
fn _control(body: &MirBody, loop_: &Loop) -> Option<_Control> {
    // Python's dict comprehension retains the last duplicate address.
    let blocks = body.blocks.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
    if loop_.latches.len() != 1 || !blocks.contains_key(&loop_.header) {
        return None;
    }
    let latch = *blocks.get(loop_.latches.first()?)?;
    let header = blocks[&loop_.header];
    let inside = &loop_.body;
    let (control, entered) = if latch.succ.as_slice() == [header.at] {
        (header, header.succ.iter().copied().filter(|at| inside.contains(at)).collect::<Vec<_>>())
    } else if latch.succ.contains(&header.at) {
        (latch, vec![header.at])
    } else {
        return None;
    };
    let exits = control.succ.iter().copied().filter(|at| !inside.contains(at)).collect::<Vec<_>>();
    if control.succ.len() != 2
        || entered.len() != 1
        || exits.len() != 1
        || control.ops.is_empty()
        || control.ops.last()?.kind != Kind::Branch
        || !control.ops.last()?.target.is_some_and(|target| control.succ.contains(&target))
        || inside.iter().any(|at| blocks[at].succ.is_empty())
    {
        return None;
    }
    // Any other way out must stop the program: the count holds whenever it goes on.
    let elsewhere = inside
        .iter()
        .filter(|at| **at != control.at)
        .flat_map(|at| blocks[at].succ.iter().copied().filter(|to| !inside.contains(to)))
        .collect::<BTreeSet<_>>();
    if !elsewhere.is_empty() && !elsewhere.is_subset(&noreturn::stranded(body, header.at)) {
        return None;
    }
    let outside = body
        .blocks
        .iter()
        .filter(|block| block.succ.contains(&header.at) && !inside.contains(&block.at))
        .map(|block| block.at)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let preheader =
        (outside.len() == 1 && blocks[&outside[0]].succ.as_slice() == [header.at]).then(|| outside[0]);
    Some(_Control {
        block: control.at,
        preheader,
        entered: entered[0],
        exit: exits[0],
        posttested: std::ptr::eq(control, latch),
        stops: !elsewhere.is_empty(),
    })
}

/// Prove every counter that alone decides when a single-exit loop leaves.
///
/// Constant start and bound give an exact `count`, and so does an
/// equality sentinel a constant distance from the start. Otherwise the
/// proof is symbolic, and only for a pre-tested unit step whose loop is
/// proved finite: an exclusive or `!=` test always is; an inclusive one
/// runs forever where `bound` is the end of its type, so needs a
/// `maximum`. Only with `inbounds` is one taken from the loop's memory
/// accesses: that reads `derived`, which asks this for counts.
pub fn counted(
    body: &Rc<MirBody>,
    loop_: &Loop,
    facts: Option<&IndexMap<Value, Known>>,
    inbounds: bool,
) -> Vec<CountedLoop> {
    counted_unless_stopped(body, loop_, facts, inbounds).into_iter().filter(|proof| !proof.stops).collect()
}

/// `counted`, also for a loop that may leave into a block that never returns.
pub fn counted_unless_stopped(
    body: &Rc<MirBody>,
    loop_: &Loop,
    facts: Option<&IndexMap<Value, Known>>,
    inbounds: bool,
) -> Vec<CountedLoop> {
    let computed;
    let facts = match facts {
        Some(facts) => facts,
        None => {
            computed = consts::known(body, None, None, None, None);
            &computed
        }
    };
    // Python's `{block.at: block for block in body.blocks}` keeps the last duplicate address.
    let blocks =
        body.blocks.iter().enumerate().map(|(index, block)| (block.at, (index, block))).collect::<BTreeMap<_, _>>();
    let Some(shape) = _control(body, loop_) else {
        return Vec::new();
    };
    let (header_index, _) = blocks[&loop_.header];
    let (control_index, _) = blocks[&shape.block];
    let control_operations = operations(body)
        .filter(|(occurrence, _, _)| occurrence.block_index() == control_index)
        .map(|(occurrence, _, operation)| (occurrence, operation))
        .collect::<Vec<_>>();
    let (branch_occurrence, branch) = *control_operations.last().expect("_control proved a branch");
    let inside = &loop_.body;
    let continuing = if branch.target.is_some_and(|target| inside.contains(&target)) {
        branch.test
    } else {
        branch.test.and_then(mir::NEGATED)
    };
    let latch = *loop_.latches.first().expect("_control proved one latch");
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
        let Some((phi_occurrence, phi)) = header_phis.iter().find(|(_, phi)| phi.result.id == counter.value).copied()
        else {
            continue;
        };
        let Some(update) = phi.incoming.get(&latch).copied() else {
            continue;
        };
        // Python's `isinstance(counter.start, (Held, Const))` is `AffineOperand`'s type.
        let mut tested = BTreeMap::from([(phi.result.id, false)]);
        if shape.posttested {
            tested.insert(update.id, true);
        }
        let comparisons = control_operations[..control_operations.len() - 1]
            .iter()
            .filter_map(|(occurrence, operation)| {
                _compared(operation, branch, &tested, &made).map(|found| (*occurrence, found))
            })
            .collect::<Vec<_>>();
        let Some(continuing) = continuing else {
            continue;
        };
        if comparisons.len() != 1 {
            continue;
        }
        let (compare_occurrence, (_, width, bound, mirrored, stepped)) = comparisons[0].clone();
        let test = if mirrored { mir::MIRRORED(continuing).expect("KeyError: a mirrored test") } else { continuing };
        let Some(step) = _signed(&counter.step.as_arg(), facts, counter.start.width()) else {
            continue;
        };
        if step == BigInt::from(0_u8) || width > counter.start.width() {
            continue;
        }
        let bound = match bound {
            Arg::Held(held) if held.width == width => AffineOperand::Held(held),
            Arg::Const(constant) if constant.width == width => AffineOperand::Const(constant),
            _ => continue,
        };
        if matches!(&bound, AffineOperand::Held(held) if !still.contains(&held.value.id)) {
            continue;
        }
        let step = _as_signed(&masked(&step, width), width);
        let zero = BigInt::from(0_u8);
        if step == zero
            || !(test == Kind::Ne || (_ASCENDING(test) && step > zero) || (_DESCENDING(test) && step < zero))
        {
            continue;
        }
        let start = match &counter.start {
            AffineOperand::Held(held) => AffineOperand::Held(Held { value: held.value, width }),
            AffineOperand::Const(constant) => AffineOperand::Const(Const::new(masked(&constant.n, width), width)),
        };
        let begin = _constant(&start.as_arg(), facts, width);
        let limit = _constant(&bound.as_arg(), facts, width);
        let difference = _difference(&bound, &start, begin.as_ref(), limit.as_ref(), &made, facts, width);
        let (mut first, mut last, maximum);
        let count = match (&difference, &begin, &limit) {
            (Some(difference), _, _) if test == Kind::Ne => {
                _equal_after(difference, &step, width, shape.posttested, stepped)
            }
            (_, Some(begin), Some(limit)) if test != Kind::Ne || difference.is_none() => {
                _ordered_after(begin, limit, &step, test, width, shape.posttested, stepped)
            }
            _ => None,
        };
        (first, last) = (None, None);
        if let Some(count) = &count {
            maximum = Some(count.clone());
            if _signed(&counter.step.as_arg(), facts, counter.start.width()).as_ref() == Some(&step) {
                (first, last) = _signed_span(&counter.start, facts, width, count, &step);
            }
        } else if shape.posttested || abs(&step) != BigInt::from(1_u8) {
            continue;
        } else {
            maximum = _unit_maximum(body, loop_, &start, &bound, begin.as_ref(), limit.as_ref(), &step, test, inbounds);
            if maximum.is_none() && _INCLUSIVE(test) {
                continue;
            }
        }
        proven.push(CountedLoop {
            counter: counter.clone(),
            phi: phi_occurrence,
            compare: compare_occurrence,
            branch: branch_occurrence,
            start: begin.map_or_else(|| start.clone(), |begin| AffineOperand::Const(Const::new(begin, width))),
            bound: limit.map_or_else(|| bound.clone(), |limit| AffineOperand::Const(Const::new(limit, width))),
            test,
            preheader: shape.preheader,
            latch,
            entered: shape.entered,
            exit: shape.exit,
            maximum,
            step,
            posttested: shape.posttested,
            stepped,
            stops: shape.stops,
            count,
            first,
            last,
        });
    }
    proven
}

/// `(op, width, bound, mirrored, stepped)` where `op` sets `branch`'s flags from a tested counter value.
pub fn _compared<'o>(
    op: &'o Op,
    branch: &Op,
    tested: &BTreeMap<u32, bool>,
    made: &BTreeMap<u32, &Op>,
) -> Option<(&'o Op, u32, Arg, bool, bool)> {
    if op.args.len() != 2 || !op.loads.is_empty() || !op.stores.is_empty() || op.barrier() {
        return None;
    }
    let flags = op.defines.iter().filter(|value| value.flags).copied().collect::<Vec<_>>();
    if flags.len() != 1 || !branch.uses.contains(&flags[0]) {
        return None;
    }
    for (index, arg) in op.args.iter().enumerate() {
        let Arg::Held(held) = arg else { continue };
        let Some(&stepped) = tested.get(&_copied(*held, made).value.id) else { continue };
        if op.kind == Kind::Sub && op.results.is_empty() && op.defines.len() == 1 {
            return Some((op, held.width, op.args[1 - index].clone(), index == 1, stepped));
        }
        if matches!(op.kind, Kind::And | Kind::Or) && op.args[0] == op.args[1] {
            return Some((op, held.width, Arg::Const(Const::new(0, held.width)), false, stepped));
        }
    }
    None
}

/// `bound - start` modulo the width, when constant: both constant, or `bound = start + c`.
fn _difference(
    bound: &AffineOperand,
    start: &AffineOperand,
    begin: Option<&BigInt>,
    limit: Option<&BigInt>,
    made: &BTreeMap<u32, &Op>,
    facts: &IndexMap<Value, Known>,
    width: u32,
) -> Option<BigInt> {
    if let (Some(begin), Some(limit)) = (begin, limit) {
        return Some(masked(&(limit - begin), width));
    }
    let (root, ahead) = anchored(&bound.as_arg(), made, width, Some(facts));
    let (other, behind) = anchored(&start.as_arg(), made, width, Some(facts));
    (root == other).then(|| masked(&(ahead - behind), width))
}

/// `arg` as a root value plus a constant, through copies and constant adds; a number has no root.
///
/// Two values with one root are a constant apart, which is how a loop from
/// `x - 32` to `x` is counted and how two counters starting 4 apart share one.
pub fn anchored(
    arg: &Arg,
    made: &BTreeMap<u32, &Op>,
    width: u32,
    facts: Option<&IndexMap<Value, Known>>,
) -> (Option<Value>, BigInt) {
    let empty = IndexMap::default();
    let facts = facts.unwrap_or(&empty);
    let mut arg = arg.clone();
    let mut offset = BigInt::from(0_u8);
    while let Arg::Held(held) = &arg {
        if held.width != width {
            break;
        }
        if let Some(known) = _constant(&arg, facts, width) {
            arg = Arg::Const(Const::new(known, width));
            break;
        }
        let Some(op) = made.get(&held.value.id).copied() else {
            break;
        };
        if !op.loads.is_empty()
            || !op.stores.is_empty()
            || op.barrier()
            || !op.merges.is_empty()
            || op.results != [arg.clone()]
        {
            break;
        }
        let constants = op.args.iter().filter(|one| matches!(one, Arg::Const(_))).count();
        if op.kind == Kind::Copy && op.args.len() == 1 && matches!(op.args[0], Arg::Held(_) | Arg::Const(_)) {
            arg = op.args[0].clone();
        } else if op.kind == Kind::Add && op.args.len() == 2 && constants == 1 {
            let (constant, other) =
                if matches!(op.args[0], Arg::Const(_)) { (&op.args[0], &op.args[1]) } else { (&op.args[1], &op.args[0]) };
            let Arg::Const(constant) = constant else { unreachable!("one constant") };
            offset += &constant.n;
            arg = other.clone();
        } else {
            break;
        }
    }
    match arg {
        Arg::Const(constant) => (None, masked(&(&constant.n + offset), width)),
        Arg::Held(held) => (Some(held.value), masked(&offset, width)),
        other => panic!("AttributeError: {other:?} has no attribute 'value'"),
    }
}

/// Trips until `start + k*step`, tested as the loop is shaped, first equals `start + difference`.
fn _equal_after(difference: &BigInt, step: &BigInt, width: u32, posttested: bool, stepped: bool) -> Option<BigInt> {
    let modulus = BigInt::from(1_u8) << (8 * width);
    let lead = u8::from(posttested && stepped);
    let divisor = gcd(mod_floor(step, &modulus), modulus.clone());
    let remaining = mod_floor(&(difference - step * lead), &modulus);
    if mod_floor(&remaining, &divisor) != BigInt::from(0_u8) {
        return None; // never equal: the loop does not end
    }
    let period = &modulus / &divisor;
    let inverse = modular_inverse(&floor_div(&mod_floor(step, &modulus), &divisor), &period)?;
    Some(BigInt::from(u8::from(posttested)) + mod_floor(&(floor_div(&remaining, &divisor) * inverse), &period))
}

/// The low and high of a test's integers: unsigned or signed at `width`.
fn _extent(unsigned: bool, width: u32) -> (BigInt, BigInt) {
    if unsigned {
        (BigInt::from(0_u8), (BigInt::from(1_u8) << (8 * width)) - 1_u8)
    } else {
        (-(BigInt::from(1_u8) << (8 * width - 1)), (BigInt::from(1_u8) << (8 * width - 1)) - 1_u8)
    }
}

/// Trips of an ordered test with constant ends, or None where a tested value would wrap first.
fn _ordered_after(
    begin: &BigInt,
    limit: &BigInt,
    step: &BigInt,
    test: Kind,
    width: u32,
    posttested: bool,
    stepped: bool,
) -> Option<BigInt> {
    let unsigned = _UNSIGNED(test);
    let (low, high) = _extent(unsigned, width);
    let first = (if unsigned { begin.clone() } else { _as_signed(begin, width) }) + step * u8::from(posttested && stepped);
    let bound = if unsigned { limit.clone() } else { _as_signed(limit, width) };
    if !(low <= first && first <= high) {
        return None;
    }
    let inclusive = u8::from(_INCLUSIVE(test));
    let zero = BigInt::from(0_u8);
    let tested = if step > &zero {
        let edge = &bound + inclusive;
        max(zero, -floor_div(&(&first - &edge), step))
    } else {
        let edge = &bound - inclusive;
        max(zero, -floor_div(&(&edge - &first), &-step))
    };
    let reached = &first + &tested * step;
    (low <= reached && reached <= high).then(|| BigInt::from(u8::from(posttested)) + tested)
}

/// The first and last signed header values over `count` trips, where none up to the exit wraps.
fn _signed_span(
    start: &AffineOperand,
    facts: &IndexMap<Value, Known>,
    width: u32,
    count: &BigInt,
    step: &BigInt,
) -> (Option<BigInt>, Option<BigInt>) {
    let Some(begin) = _signed(&start.as_arg(), facts, start.width()) else {
        return (None, None);
    };
    if count < &BigInt::from(1_u8) || _as_signed(&masked(&begin, width), width) != begin {
        return (None, None);
    }
    let last = &begin + (count - 1_u8) * step;
    let sign = BigInt::from(1_u8) << (8 * width - 1);
    let after = &last + step;
    if -&sign <= last && last < sign && -&sign <= after && after < sign {
        (Some(begin), Some(last))
    } else {
        (None, None)
    }
}

/// Most trips of a symbolic unit-step loop, where proved; None for an inclusive test that may never end.
#[allow(clippy::too_many_arguments)]
fn _unit_maximum(
    body: &Rc<MirBody>,
    loop_: &Loop,
    start: &AffineOperand,
    bound: &AffineOperand,
    begin: Option<&BigInt>,
    limit: Option<&BigInt>,
    step: &BigInt,
    test: Kind,
    inbounds: bool,
) -> Option<BigInt> {
    let width = bound.width();
    if test == Kind::Ne {
        return Some((BigInt::from(1_u8) << (8 * width)) - 1_u8);
    }
    let (unsigned, inclusive) = (_UNSIGNED(test), _INCLUSIVE(test));
    let (low, high) = _extent(unsigned, width);
    let ascending = step > &BigInt::from(0_u8);
    // Walked toward `bound`, as integers in the test's own signedness.
    let signed = |value: &BigInt| if unsigned { value.clone() } else { _as_signed(value, width) };
    let begin = begin.map(signed);
    let limit = limit.map(signed);
    let end = if ascending { &high } else { &low };
    if inclusive && limit.as_ref() == Some(end) {
        return None;
    }
    let ends = (
        _range(body, start, usize::from(!ascending), &high),
        _range(body, bound, usize::from(ascending), &high),
    );
    let origin = begin.or(ends.0);
    let mut target = limit.clone().or(ends.1);
    if target.is_some() && limit.is_none() && inclusive && target.as_ref() == Some(end) {
        target = None;
    }
    if let (Some(origin), Some(target)) = (&origin, &target) {
        if low <= *origin.min(target) && *origin.max(target) <= high {
            return Some(max(BigInt::from(0_u8), (target - origin) * step + u8::from(inclusive)));
        }
    }
    if inbounds { _inbounds_trips(body, loop_, *loop_.latches.first().expect("one latch")) } else { None }
}

/// How far each counter and each value affine in one advances per iteration.
///
/// The bytes-per-iteration view of `basics` and `derived`; nothing here
/// re-derives which values are affine.
pub fn advances(body: &Rc<MirBody>, loop_: &Loop) -> IndexMap<Value, BigInt> {
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
/// wide addresses at most 2**(8w) of them, so i*s + width <= 2**(8w). Only
/// an access its frontend marked `inbounds` is promised that.
fn _inbounds_trips(body: &Rc<MirBody>, loop_: &Loop, latch: i64) -> Option<BigInt> {
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
        .filter(|reference| reference.inbounds)
        .filter_map(|reference| {
            let advance = step.get(&reference.base?)?;
            Some(
                ((BigInt::from(1_u8) << (8 * reference.base_width)) - reference.width) / abs(advance)
                    + 1,
            )
        })
        .min()
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
pub fn _signed(
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

/// The one positive trip count every counter of a loop proves, if they prove one.
///
/// A loop may carry an integer counter, a byte address and one or more
/// derived counters at once.  They are evidence for the same trip count,
/// not alternatives from which a transform may pick the convenient one.
/// Refusing disagreement keeps cloning transforms independent of which
/// recurrence happened to be visited first.
pub fn agreed_count(proofs: &[CountedLoop]) -> Option<BigInt> {
    let zero = BigInt::from(0_u8);
    let counts = proofs.iter().filter_map(|proof| proof.count.clone()).filter(|count| *count != zero).collect::<BTreeSet<_>>();
    if counts.len() == 1 { counts.into_iter().next() } else { None }
}

/// `agreed_count`, or the count remembered at this header when nothing proves one now.
pub fn trip_count(body: &Rc<MirBody>, loop_: &Loop, facts: &IndexMap<Value, Known>) -> Option<BigInt> {
    _trips(body, loop_, &counted(body, loop_, Some(facts), false))
}

/// `trip_count` for a loop that may also stop the program: its trips whenever it does not.
pub fn trips_unless_stopped(body: &Rc<MirBody>, loop_: &Loop, facts: &IndexMap<Value, Known>) -> Option<BigInt> {
    _trips(body, loop_, &counted_unless_stopped(body, loop_, Some(facts), false))
}

fn _trips(body: &Rc<MirBody>, loop_: &Loop, proofs: &[CountedLoop]) -> Option<BigInt> {
    let zero = BigInt::from(0_u8);
    if !proofs.iter().any(|proof| proof.count.as_ref().is_some_and(|count| *count != zero)) {
        // A semantics-preserving loop transform may consume the syntactic
        // relationship which established this fact.  Nested-recurrence
        // rewind, for example, replaces `start` with an outer phi after it
        // has proved the inner loop's exact distance.  Retain that proof at
        // the same header so rotation and measurement do not fall back to a
        // guessed trip count.  A newly derived disagreement is still refused.
        // Python's `dict(body.loop_trip_counts)` keeps the last duplicate header.
        return body
            .loop_trip_counts
            .iter()
            .filter_map(|(header, count)| (*header == loop_.header).then_some(BigInt::from(*count)))
            .last();
    }
    agreed_count(proofs)
}

/// A counted loop whose first iteration and finite exit are proven.
pub fn nonempty(body: &Rc<MirBody>, loop_: &Loop) -> bool {
    trip_count(body, loop_, &consts::known(body, None, None, None, None)).is_some()
}

/// The proof in which `counter` decides when `loop` leaves.
pub fn controlling(
    body: &Rc<MirBody>,
    loop_: &Loop,
    counter: &Affine,
    facts: &IndexMap<Value, Known>,
) -> Option<CountedLoop> {
    counted(body, loop_, Some(facts), false).into_iter().find(|proof| proof.counter.value == counter.value)
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
pub fn floor_div(numerator: &BigInt, denominator: &BigInt) -> BigInt {
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
pub fn mod_floor(value: &BigInt, modulus: &BigInt) -> BigInt {
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

pub fn gcd(mut one: BigInt, mut other: BigInt) -> BigInt {
    while other != BigInt::from(0_u8) {
        let remainder = one % &other;
        one = other;
        other = remainder;
    }
    one
}

/// Python's `test_only(op)`.
pub fn test_only(op: &Op) -> bool {
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
pub fn invariant(body: &MirBody, inside: &BTreeSet<i64>) -> Invariant {
    let mut written = Invariant::default();
    let mut named = Invariant::default();
    for block in &body.blocks {
        if inside.contains(&block.at) {
            for op in &block.ops {
                for value in &op.defines {
                    written.insert(value.id);
                }
            }
            for phi in &block.phis {
                written.insert(phi.result.id);
            }
        }
        for op in &block.ops {
            for value in op.defines.iter().chain(&op.uses) {
                named.insert(value.id);
            }
        }
    }
    for (word, gone) in named.words.iter_mut().zip(&written.words) {
        *word &= !gone;
    }
    named
}

/// Value ids, as `invariant` answers them: a bit per id.
#[derive(Clone, Debug, Default)]
pub struct Invariant {
    words: Vec<u64>,
}

impl Invariant {
    fn insert(&mut self, id: u32) {
        let (word, bit) = (id as usize / 64, id % 64);
        if word >= self.words.len() {
            self.words.resize(word + 1, 0);
        }
        self.words[word] |= 1 << bit;
    }

    pub fn contains(&self, id: &u32) -> bool {
        self.words.get(*id as usize / 64).is_some_and(|word| word & (1 << (id % 64)) != 0)
    }
}

/// Python's `basics(body, loop)`.
pub fn basics(body: &MirBody, loop_: &Loop) -> OrderedMap<u32, Affine> {
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
    still: &Invariant,
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
pub fn transparent_aliases(
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
pub fn control_replacement<'a>(
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
    if proof.posttested || proof.preheader.is_none() || proof.width() != proof.counter.start.width() {
        return None;
    }
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
    let width = proof.width();
    let stepping_op = &body.blocks[stepping.block_index()].ops[stepping.operation_index()];
    let counter = Arg::Held(Held { value: phi.result, width });
    if !mir::stepping(stepping_op).is_some_and(|(one, other)| one == counter || other == counter)
        || stepping_op.results != vec![Arg::Held(Held { value: update, width })]
        || !stepping_op.loads.is_empty()
        || !stepping_op.stores.is_empty()
        || stepping_op.barrier()
        || !stepping_op.merges.is_empty()
    {
        return None;
    }
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
///
/// `covered` are the counter's reads the caller rebases; the counter itself
/// is a candidate when they are all its data reads.
pub fn zero_terminating_control<'a>(
    body: &Rc<MirBody>,
    loop_: &Loop,
    proof: &'a CountedLoop,
    candidate: &Affine,
    covered: &BTreeSet<OpOccurrence>,
    facts: Option<&IndexMap<Value, Known>>,
) -> Option<ZeroTerminatingControl<'a>> {
    let replacement = control_replacement(body, loop_, proof, covered)?;
    let maximum = proof.maximum.as_ref()?;
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

#[cfg(test)]
#[path = "counted_loops_tests.rs"]
pub mod counted_loops_tests;
