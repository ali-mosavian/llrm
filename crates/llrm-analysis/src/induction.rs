//! Which values are affine functions of a loop's counter, and how many
//! trips a loop makes: llrm-core's `analysis/induction.rs`, adapted to the
//! rich MIR. LLVM's ScalarEvolution add recurrences and trip counts, with
//! InductionDescriptor's view of a counter. A width is in bits.
//!
//! A compare is the `icmp` a conditional `br` reads; a step is an `add` or
//! a `sub` of a constant; a pointer offset is a `getelementptr`, scaled as
//! the layout says. A counter tested narrower is a `trunc`, not a counter.
//!
//! Dropped, with no rich-MIR counterpart: copies (`copied`, `definitions`,
//! `transparent_aliases`, a replacement's aliases and copies); flags (the
//! `or i, i` zero test, `test_only`, a step's flags read elsewhere); an
//! arithmetic operand in memory (`unwritten`: a `load` is its own
//! instruction, hoisted by LICM); the frontend's `integer_ranges` and the
//! remembered `loop_trip_counts`, which nothing states here; and the
//! occurrence accessors, an `InstId` being the identity.

use std::cmp::max;
use std::collections::{BTreeMap, BTreeSet};

use llrm_graph::loops::{self, Loop};
use llrm_mir::context::signed;
use llrm_mir::module::{BlockId, Function, InstId, Instruction, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_support::hash::{HashSet, IndexMap};
use num_bigint::BigInt;

use crate::cfg;
use crate::consts::{self, Known, masked};
use crate::memory::{MemRef, Unit};
use crate::noreturn;
use crate::occurrence::{operations, phis};

/// A recurrence's start or step: a value, or a number.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum AffineOperand {
    /// A value `width` bits wide.
    Value(ValueId, u32),
    Const(Known),
}

impl AffineOperand {
    /// `n` modulo `width` bits.
    pub fn constant(n: impl Into<BigInt>, width: u32) -> Self {
        Self::Const(Known::new(masked(&n.into(), width), width))
    }

    pub fn width(&self) -> u32 {
        match self {
            Self::Value(_, width) => *width,
            Self::Const(known) => known.width,
        }
    }
}

/// An integer operand as a term.
pub fn term(unit: &Unit, operand: Operand) -> Option<AffineOperand> {
    let width = unit.int_bits(operand)?;
    match operand {
        Operand::Value(value) => Some(AffineOperand::Value(value, width)),
        Operand::Constant(_) => Some(AffineOperand::Const(Known::new(unit.int_constant(operand)?, width))),
        Operand::Block(_) => None,
    }
}

/// `start + step * iteration`, in the loop `header` heads.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Affine {
    pub value: ValueId,
    pub start: AffineOperand,
    pub step: AffineOperand,
    pub header: i64,
}

/// A value an instruction computes as `by * of + sum(coefficient * term)`,
/// plus `pointer` where it is an address: then in bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Derived {
    pub op: InstId,
    pub of: Affine,
    pub by: AffineOperand,
    pub offsets: Vec<(AffineOperand, BigInt)>,
    pub pointer: Option<Operand>,
}

/// `scale * source + offset`, modulo `width` bits.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AffineMap {
    pub scale: BigInt,
    pub offset: BigInt,
    pub width: u32,
}

type Form = (Affine, BigInt, Vec<(AffineOperand, BigInt)>);

/// The one proof of how many trips a loop makes, shared by every pass.
///
/// ```text
/// i = start; loop { [i test bound?] body; i += step; [i test bound?] }
/// ```
///
/// `test` continues the loop, counter first; `step` is a nonzero
/// constant. A pre-tested loop tests the header value before each trip; a
/// post-tested one tests after each trip, the stepped value when
/// `stepped`.
///
/// `count` is the exact trip count when constant. `trips` places it
/// when symbolic, which needs a pre-tested unit step: the only proofs with
/// no `count`. `first` and `last` are the header's signed values on
/// the first and last trip, given only when nothing up to the exit wraps.
/// `maximum` bounds the trips when the count is unknown.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CountedLoop {
    pub counter: Affine,
    pub phi: InstId,
    pub compare: InstId,
    pub branch: InstId,
    pub start: AffineOperand,
    pub bound: AffineOperand,
    pub test: IntPredicate,
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
    pub fn inclusive(&self) -> bool {
        _inclusive(self.test)
    }

    pub fn width(&self) -> u32 {
        self.bound.width()
    }

    /// The signed values the header's counter takes on a trip, lowest first.
    pub fn span(&self) -> Option<(BigInt, BigInt)> {
        let (first, last) = (self.first.as_ref()?, self.last.as_ref()?);
        Some((first.min(last).clone(), first.max(last).clone()))
    }
}

fn _ascending(test: IntPredicate) -> bool {
    matches!(test, IntPredicate::Slt | IntPredicate::Sle | IntPredicate::Ult | IntPredicate::Ule)
}

fn _descending(test: IntPredicate) -> bool {
    matches!(test, IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Ugt | IntPredicate::Uge)
}

fn _inclusive(test: IntPredicate) -> bool {
    matches!(test, IntPredicate::Sle | IntPredicate::Ule | IntPredicate::Sge | IntPredicate::Uge)
}

fn _unsigned(test: IntPredicate) -> bool {
    matches!(test, IntPredicate::Ult | IntPredicate::Ule | IntPredicate::Ugt | IntPredicate::Uge)
}

/// Places one preheader operation and returns its result.
pub type Computed<'a> = dyn FnMut(BinaryOp, Vec<AffineOperand>) -> AffineOperand + 'a;

/// The preheader comparison, and the test on it, under which the loop runs no trips.
pub fn skipped(proof: &CountedLoop) -> Option<((AffineOperand, AffineOperand), IntPredicate)> {
    if proof.posttested {
        return None;
    }
    Some(((proof.bound.clone(), proof.start.clone()), proof.test.inverse().swapped()))
}

/// Trips on the entered path, exact modulo the compare's width, or None where not expressible.
pub fn trips(proof: &CountedLoop, computed: &mut Computed<'_>) -> Option<AffineOperand> {
    let width = proof.width();
    if proof.posttested {
        return None;
    }
    if let Some(count) = &proof.count {
        return (count < &(BigInt::from(1) << width)).then(|| AffineOperand::constant(count.clone(), width));
    }
    let (ahead, behind) = if proof.step > BigInt::from(0) { (&proof.bound, &proof.start) } else { (&proof.start, &proof.bound) };
    let count = computed(BinaryOp::Sub, vec![ahead.clone(), behind.clone()]);
    Some(computed(BinaryOp::Add, vec![count, AffineOperand::constant(u8::from(proof.inclusive()), width)]))
}

/// The header's counter as a pre-tested loop that ran a trip leaves: the first value failing its test.
pub fn exit_value(proof: &CountedLoop, computed: &mut Computed<'_>) -> Option<AffineOperand> {
    let width = proof.width();
    if proof.posttested {
        return None;
    }
    if proof.test == IntPredicate::Ne {
        return Some(proof.bound.clone());
    }
    if let (AffineOperand::Const(start), Some(count)) = (&proof.start, &proof.count) {
        return Some(AffineOperand::constant(&start.n + count * &proof.step, width));
    }
    let past = AffineOperand::constant(&proof.step * u8::from(proof.inclusive()), width);
    if let (AffineOperand::Const(bound), AffineOperand::Const(past)) = (&proof.bound, &past) {
        return Some(AffineOperand::constant(&bound.n + &past.n, width));
    }
    Some(computed(BinaryOp::Add, vec![proof.bound.clone(), past]))
}

/// Proof that a counted loop's own recurrence may be removed: its phi is
/// read only by the compare, the step, the caller's covered instructions
/// and `exits`, the exit block's phis reading it as the loop leaves.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlReplacement<'a> {
    pub counted: &'a CountedLoop,
    pub stepping: InstId,
    pub update: ValueId,
    pub exits: Vec<InstId>,
}

/// Proof that `candidate` reaching its final value can end counted control:
/// it takes no value twice in `maximum` trips.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ZeroTerminatingControl<'a> {
    pub replacement: ControlReplacement<'a>,
    pub candidate: Affine,
    pub step: BigInt,
    pub maximum: BigInt,
    pub period: BigInt,
}

impl AffineMap {
    /// Trips before the mapped value repeats.
    pub fn period(&self) -> BigInt {
        let modulus = BigInt::from(1) << self.width;
        modulus.clone() / gcd(abs(&self.scale), modulus)
    }

    /// Whether distinct sources in `low ..= high` map to distinct values.
    pub fn injective(&self, low: &BigInt, high: &BigInt) -> bool {
        self.scale != BigInt::from(0) && high - low < self.period()
    }
}

/// `target` as a map of `source`, where their steps divide.
pub fn relation(source: &Affine, target: &Affine, facts: &IndexMap<ValueId, Known>) -> Option<AffineMap> {
    let width = source.start.width();
    if target.start.width() != width {
        return None;
    }
    let source_start = _signed(&source.start, facts, width)?;
    let source_step = _signed(&source.step, facts, width)?;
    let target_start = _signed(&target.start, facts, width)?;
    let target_step = _signed(&target.step, facts, width)?;
    if source_step == BigInt::from(0) || (&target_step % &source_step) != BigInt::from(0) {
        return None;
    }
    // Divisible, so truncating division is exact.
    let scale = &target_step / &source_step;
    if scale == BigInt::from(0) {
        return None;
    }
    let offset = masked(&(target_start - &scale * source_start), width);
    Some(AffineMap { scale, offset, width })
}

/// The constant map one numeric formula carries.
pub fn derived_map(formula: &Derived, facts: &IndexMap<ValueId, Known>) -> Option<AffineMap> {
    let width = formula.of.start.width();
    let scale = _signed(&formula.by, facts, width)?;
    if scale == BigInt::from(0) || formula.pointer.is_some() {
        return None;
    }
    let modulus = BigInt::from(1) << width;
    let mut offset = BigInt::from(0);
    for (value, coefficient) in &formula.offsets {
        let constant = _constant(value, facts, width)?;
        offset = mod_floor(&(offset + constant * coefficient), &modulus);
    }
    Some(AffineMap { scale, offset, width })
}

/// Exact signed division of a non-wrapping recurrence is another recurrence.
fn _quotients(unit: &Unit, loop_: &Loop, found: &IndexMap<ValueId, Affine>, facts: &IndexMap<ValueId, Known>) -> Vec<Derived> {
    let mut out = Vec::new();
    for (inst, block, operation) in operations(unit.function) {
        let at = cfg::id(block);
        if !loop_.body.contains(&at) || at == loop_.header || operation.opcode != Opcode::Binary(BinaryOp::SDiv) {
            continue;
        }
        let Operand::Value(dividend) = operation.operands[0] else { continue };
        let Some(counter) = found.get(&dividend) else { continue };
        let width = counter.start.width();
        let (Some(start), Some(step), Some(denominator)) = (
            _signed(&counter.start, facts, width),
            _signed(&counter.step, facts, width),
            term(unit, operation.operands[1]).and_then(|one| _signed(&one, facts, width)),
        ) else {
            continue;
        };
        if denominator == BigInt::from(0)
            || mod_floor(&start, &denominator) != BigInt::from(0)
            || mod_floor(&step, &denominator) != BigInt::from(0)
        {
            continue;
        }
        let Some((low, high)) = domain(unit, loop_, counter, facts) else { continue };
        let sign = BigInt::from(1) << (width - 1);
        if ![low, high].iter().all(|value| {
            let quotient = floor_div(value, &denominator);
            -&sign <= quotient && quotient < sign
        }) {
            continue;
        }
        out.push(Derived {
            op: inst,
            of: Affine {
                value: counter.value,
                start: AffineOperand::constant(floor_div(&start, &denominator), width),
                step: AffineOperand::constant(floor_div(&step, &denominator), width),
                header: loop_.header,
            },
            by: AffineOperand::constant(1, width),
            offsets: Vec::new(),
            pointer: None,
        });
    }
    out
}

/// The finite inclusive signed domain `affine` takes on a trip.
pub fn domain(unit: &Unit, loop_: &Loop, affine: &Affine, facts: &IndexMap<ValueId, Known>) -> Option<(BigInt, BigInt)> {
    controlling(unit, loop_, affine, facts)?.span()
}

/// A narrow recurrence extended, as a wide one: only where the counted-loop
/// proof shows the narrow value cannot wrap on any trip.
fn _extended(unit: &Unit, loop_: &Loop, inst: InstId, forms: &IndexMap<ValueId, Form>, facts: &IndexMap<ValueId, Known>) -> Option<Form> {
    let op = unit.function.instruction(inst);
    let Opcode::Cast(cast @ (CastOp::SExt | CastOp::ZExt)) = op.opcode else { return None };
    let (Operand::Value(source), Some(result)) = (op.operands[0], op.result) else { return None };
    let width = unit.int_bits(Operand::Value(source))?;
    let wide = unit.int_bits(Operand::Value(result))?;
    let (counter, scale, offsets) = forms.get(&source)?;
    if width >= wide || counter.start.width() != width {
        return None;
    }
    let raw_start = _constant(&counter.start, facts, width)?;
    let raw_step = _constant(&counter.step, facts, width)?;
    let count = controlling(unit, loop_, counter, facts)
        .filter(|proof| proof.width() == width)
        .and_then(|proof| proof.count)
        .filter(|count| *count != BigInt::from(0))?;
    let constants = offsets
        .iter()
        .map(|(argument, coefficient)| Some((_constant(argument, facts, width)?, coefficient.clone())))
        .collect::<Option<Vec<_>>>()?;
    let step = _as_signed(&raw_step, width);
    if step == BigInt::from(0) {
        return None;
    }
    let mask = (BigInt::from(1) << width) - 1;
    let sign = BigInt::from(1) << (width - 1);
    let (initial, stride, low, high) = if cast == CastOp::SExt {
        let signed_scale = _as_signed(&(scale & &mask), width);
        let initial = _as_signed(&raw_start, width) * &signed_scale
            + constants.iter().fold(BigInt::from(0), |sum, (value, coefficient)| sum + _as_signed(value, width) * coefficient);
        (initial, &step * signed_scale, -sign.clone(), sign.clone())
    } else {
        let initial = masked(&(raw_start * scale + constants.iter().fold(BigInt::from(0), |sum, (value, coefficient)| sum + value * coefficient)), width);
        let raw_stride = masked(&(raw_step * scale), width);
        // Half the modulus has two equally valid directions: choosing either
        // would invent a wide recurrence.
        if raw_stride == sign && count > BigInt::from(1) {
            return None;
        }
        (initial, _as_signed(&raw_stride, width), BigInt::from(0), mask + 1)
    };
    let final_value = &initial + (&count - 1) * &stride;
    if initial < low || initial >= high || final_value < low || final_value >= high {
        return None;
    }
    Some((
        Affine { value: result, start: AffineOperand::constant(initial, wide), step: AffineOperand::constant(stride, wide), header: loop_.header },
        BigInt::from(1),
        Vec::new(),
    ))
}

/// A left shift's count as the multiplier it is.
fn _multiplier(shift: bool, by: &AffineOperand) -> AffineOperand {
    match by {
        AffineOperand::Const(constant) if shift => {
            let count = usize::try_from(&constant.n).expect("a count below the width");
            AffineOperand::constant(BigInt::from(1) << count, constant.width)
        }
        _ => by.clone(),
    }
}

/// A `getelementptr` off an invariant pointer whose one variable index is
/// affine: its bytes as a formula.
fn _address(unit: &Unit, inst: InstId, forms: &IndexMap<ValueId, Form>, still: &Invariant) -> Option<Derived> {
    let op = unit.function.instruction(inst);
    let Opcode::GetElementPtr { source } = op.opcode else { return None };
    let pointer = op.operands[0];
    if !still.operand(pointer) {
        return None;
    }
    let indices = op.operands[1..]
        .iter()
        .map(|&one| unit.int_constant(one).map(|bits| signed(bits, unit.int_bits(one).unwrap_or(128))))
        .collect::<Vec<_>>();
    let (constant, variable) = unit.layout.collect_offset(&unit.context.types, source, &indices);
    let [(at, scale)] = variable[..] else { return None };
    let Operand::Value(index) = op.operands[1 + at] else { return None };
    let (counter, form_scale, offsets) = forms.get(&index)?;
    let width = counter.start.width();
    if unit.int_bits(Operand::Value(index)) != Some(width) || unit.layout.pointer(unit.space(pointer)?).index_bits != width {
        return None;
    }
    let scale = BigInt::from(scale);
    let mut offsets = offsets.iter().map(|(one, coefficient)| (one.clone(), coefficient * &scale)).collect::<Vec<_>>();
    if constant != 0 {
        offsets.push((AffineOperand::constant(constant, width), BigInt::from(1)));
    }
    Some(Derived { op: inst, of: counter.clone(), by: AffineOperand::constant(form_scale * &scale, width), offsets, pointer: Some(pointer) })
}

/// Sums, differences, constant multiples and shifts of recurrences, their
/// extensions, and addresses off them, to a fixed point.
fn _composed(
    unit: &Unit,
    loop_: &Loop,
    found: &IndexMap<ValueId, Affine>,
    facts: &IndexMap<ValueId, Known>,
    still: &Invariant,
) -> IndexMap<InstId, Derived> {
    let inside = &loop_.body;
    let mut forms: IndexMap<ValueId, Form> =
        found.iter().map(|(value, recurrence)| (*value, (recurrence.clone(), BigInt::from(1), Vec::new()))).collect();
    let mut out = IndexMap::<InstId, Derived>::default();
    let mut changed = true;
    while changed {
        changed = false;
        for (inst, block, operation) in operations(unit.function) {
            let at = cfg::id(block);
            let Some(result) = operation.result.filter(|result| inside.contains(&at) && !forms.contains_key(result)) else { continue };
            match operation.opcode {
                Opcode::Cast(CastOp::SExt | CastOp::ZExt) if at != loop_.header => {
                    if let Some(extended) = _extended(unit, loop_, inst, &forms, facts) {
                        forms.insert(result, extended);
                        changed = true;
                    }
                    continue;
                }
                Opcode::GetElementPtr { .. } => {
                    if let Some(address) = _address(unit, inst, &forms, still) {
                        out.insert(inst, address);
                    }
                    continue;
                }
                Opcode::Binary(BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl) => {}
                _ => continue,
            }
            let Opcode::Binary(kind) = operation.opcode else { unreachable!("matched above") };
            let Some(width) = unit.int_bits(Operand::Value(result)) else { continue };
            let (Some(mut left), Some(mut right)) = (term(unit, operation.operands[0]), term(unit, operation.operands[1])) else { continue };
            for argument in [&mut left, &mut right] {
                if let AffineOperand::Value(value, one) = argument
                    && *one == width
                    && !forms.contains_key(value)
                    && let Some(fact) = facts.get(value).filter(|fact| fact.width >= width)
                {
                    *argument = AffineOperand::constant(fact.n.clone(), width);
                }
            }
            let form = |one: &AffineOperand| match one {
                AffineOperand::Value(value, one) if *one == width => forms.get(value),
                _ => None,
            };
            let (first, second) = (form(&left), form(&right));
            if [first, second].into_iter().flatten().any(|(counter, _, _)| counter.start.width() != width) {
                continue;
            }
            let (base, scale, offsets) = match (kind, first, second) {
                (BinaryOp::Add | BinaryOp::Sub, Some((first_base, first_scale, first_offsets)), Some((second_base, second_scale, second_offsets))) => {
                    if first_base != second_base {
                        continue;
                    }
                    let sign = if kind == BinaryOp::Add { 1 } else { -1 };
                    let offsets = first_offsets
                        .iter()
                        .cloned()
                        .chain(second_offsets.iter().map(|(argument, coefficient)| (argument.clone(), coefficient * sign)))
                        .collect();
                    (first_base.clone(), first_scale + second_scale * sign, offsets)
                }
                (BinaryOp::Add, Some(recurrence), None) | (BinaryOp::Sub, Some(recurrence), None) | (BinaryOp::Add, None, Some(recurrence)) => {
                    let offset = if first.is_some() { &right } else { &left };
                    let fits = match offset {
                        AffineOperand::Const(constant) => constant.width == width,
                        AffineOperand::Value(value, one) => *one == width && still.contains(*value),
                    };
                    if !fits {
                        continue;
                    }
                    let (base, scale, prior) = recurrence;
                    let mut offsets = prior.clone();
                    offsets.push((offset.clone(), BigInt::from(if kind == BinaryOp::Sub { -1 } else { 1 })));
                    (base.clone(), scale.clone(), offsets)
                }
                (BinaryOp::Mul, Some((base, scale, offsets)), None) | (BinaryOp::Mul, None, Some((base, scale, offsets))) => {
                    let AffineOperand::Const(constant) = (if first.is_some() { &right } else { &left }) else { continue };
                    if constant.width != width {
                        continue;
                    }
                    let offsets = offsets.iter().map(|(argument, coefficient)| (argument.clone(), coefficient * &constant.n)).collect();
                    (base.clone(), scale * &constant.n, offsets)
                }
                (BinaryOp::Shl, Some((base, scale, offsets)), _) => {
                    let AffineOperand::Const(amount) = &right else { continue };
                    if amount.n >= BigInt::from(width) {
                        continue;
                    }
                    let shift = usize::try_from(&amount.n).expect("a count below the width");
                    let offsets = offsets.iter().map(|(argument, coefficient)| (argument.clone(), coefficient << shift)).collect();
                    (base.clone(), scale << shift, offsets)
                }
                _ => continue,
            };
            let scale = masked(&scale, width);
            forms.insert(result, (base.clone(), scale.clone(), offsets.clone()));
            out.insert(inst, Derived { op: inst, of: base, by: AffineOperand::Const(Known::new(scale, width)), offsets, pointer: None });
            changed = true;
        }
    }
    out
}

/// Every value inside the loop affine in one of its counters.
pub fn derived(unit: &Unit, loop_: &Loop, found: Option<&IndexMap<ValueId, Affine>>) -> Vec<Derived> {
    let calculated;
    let found = match found {
        Some(found) => found,
        None => {
            calculated = basics(unit, loop_);
            &calculated
        }
    };
    if found.is_empty() {
        return Vec::new();
    }
    let facts = consts::known(unit, None, None, None);
    let still = invariant(unit.function, &loop_.body);

    let mut direct = IndexMap::<InstId, Derived>::default();
    for (inst, block, operation) in operations(unit.function) {
        let (Opcode::Binary(kind @ (BinaryOp::Mul | BinaryOp::Shl)), true) = (&operation.opcode, loop_.body.contains(&cfg::id(block))) else {
            continue;
        };
        let Some(arguments) = operation.operands.iter().map(|&one| term(unit, one)).collect::<Option<Vec<_>>>() else { continue };
        let is_counter = |one: &AffineOperand| matches!(one, AffineOperand::Value(value, _) if found.contains_key(value));
        let counters = arguments.iter().filter(|one| is_counter(one)).collect::<Vec<_>>();
        let others = arguments.iter().filter(|one| !is_counter(one)).collect::<Vec<_>>();
        let ([AffineOperand::Value(counter, width)], [by]) = (&counters[..], &others[..]) else { continue };
        let shift = *kind == BinaryOp::Shl;
        if shift && (!is_counter(&arguments[0]) || !matches!(by, AffineOperand::Const(constant) if constant.n < BigInt::from(*width))) {
            continue;
        }
        if matches!(by, AffineOperand::Value(value, _) if !still.contains(*value)) {
            continue;
        }
        direct.insert(inst, Derived { op: inst, of: found[counter].clone(), by: _multiplier(shift, by), offsets: Vec::new(), pointer: None });
    }
    for (inst, formula) in _composed(unit, loop_, found, &facts, &still) {
        direct.insert(inst, formula);
    }
    for formula in _quotients(unit, loop_, found, &facts) {
        direct.insert(formula.op, formula);
    }
    direct.into_values().collect()
}

/// Every loop of the function, with its counters and what they derive.
pub fn of(unit: &Unit) -> Vec<(Loop, IndexMap<ValueId, Affine>, Vec<Derived>)> {
    let mut result = Vec::new();
    for loop_ in loops::loops(&cfg::graph(unit.function), None) {
        let found = basics(unit, &loop_);
        if found.is_empty() {
            continue;
        }
        let formulas = derived(unit, &loop_, Some(&found));
        result.push((loop_, found, formulas));
    }
    result
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

/// The block whose conditional branch is the loop's only exit that goes on: its header, or its latch.
fn _control(function: &Function, loop_: &Loop) -> Option<_Control> {
    let graph = cfg::graph(function);
    let blocks = graph.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
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
    let conditional = function
        .terminator(cfg::block(control.at))
        .is_some_and(|last| matches!(function.instruction(last), Instruction { opcode: Opcode::Br, operands, .. } if operands.len() == 3));
    if control.succ.len() != 2 || entered.len() != 1 || exits.len() != 1 || !conditional || inside.iter().any(|at| blocks[at].succ.is_empty()) {
        return None;
    }
    // Any other way out must stop the program: the count holds whenever it goes on.
    let elsewhere = inside
        .iter()
        .filter(|at| **at != control.at)
        .flat_map(|at| blocks[at].succ.iter().copied().filter(|to| !inside.contains(to)))
        .collect::<BTreeSet<_>>();
    if !elsewhere.is_empty() && !elsewhere.is_subset(&noreturn::stranded(function, header.at)) {
        return None;
    }
    let outside =
        graph.iter().filter(|block| block.succ.contains(&header.at) && !inside.contains(&block.at)).map(|block| block.at).collect::<BTreeSet<_>>();
    let preheader = match outside.first() {
        Some(&one) if outside.len() == 1 && blocks[&one].succ.as_slice() == [header.at] => Some(one),
        _ => None,
    };
    Some(_Control {
        block: control.at,
        preheader,
        entered: entered[0],
        exit: exits[0],
        posttested: control.at == latch.at,
        stops: !elsewhere.is_empty(),
    })
}

/// The operand phi `inst` takes from `from`.
fn incoming(function: &Function, inst: InstId, from: BlockId) -> Option<Operand> {
    function.instruction(inst).operands.chunks(2).find(|pair| pair[1] == Operand::Block(from)).map(|pair| pair[0])
}

/// The instruction defining `value`.
fn defining(function: &Function, value: ValueId) -> Option<InstId> {
    match function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

/// Prove every counter that alone decides when a single-exit loop leaves.
///
/// Constant start and bound give an exact `count`, and so does an
/// equality sentinel a constant distance from the start. Otherwise the
/// proof is symbolic, and only for a pre-tested unit step whose loop is
/// proved finite: an exclusive or `!=` test always is; an inclusive one
/// runs forever where `bound` is the end of its type, so needs a
/// `maximum`: a step promised not to wrap, or with `inbounds` the loop's
/// memory accesses. That reads `derived`, which asks this for counts.
pub fn counted(unit: &Unit, loop_: &Loop, facts: Option<&IndexMap<ValueId, Known>>, inbounds: bool) -> Vec<CountedLoop> {
    counted_unless_stopped(unit, loop_, facts, inbounds).into_iter().filter(|proof| !proof.stops).collect()
}

/// `counted`, also for a loop that may leave into a block that never returns.
pub fn counted_unless_stopped(unit: &Unit, loop_: &Loop, facts: Option<&IndexMap<ValueId, Known>>, inbounds: bool) -> Vec<CountedLoop> {
    let function = unit.function;
    let computed;
    let facts = match facts {
        Some(facts) => facts,
        None => {
            computed = consts::known(unit, None, None, None);
            &computed
        }
    };
    let Some(shape) = _control(function, loop_) else { return Vec::new() };
    let branch = function.terminator(cfg::block(shape.block)).expect("_control proved a branch");
    let [condition, Operand::Block(taken), Operand::Block(_)] = function.instruction(branch).operands[..] else { return Vec::new() };
    let Some((compare, icmp)) = unit.defining(condition) else { return Vec::new() };
    let Opcode::ICmp(predicate) = icmp.opcode else { return Vec::new() };
    let inside = &loop_.body;
    let continuing = if inside.contains(&cfg::id(taken)) { predicate } else { predicate.inverse() };
    let latch = *loop_.latches.first().expect("_control proved one latch");
    let still = invariant(function, inside);

    let mut proven = Vec::new();
    for counter in basics(unit, loop_).values() {
        let Some(phi) = defining(function, counter.value) else { continue };
        let Some(Operand::Value(update)) = incoming(function, phi, cfg::block(latch)) else { continue };
        let mut tested = BTreeMap::from([(counter.value, false)]);
        if shape.posttested {
            tested.insert(update, true);
        }
        let Some((width, bound, mirrored, stepped)) = _compared(unit, icmp, &tested) else { continue };
        let test = if mirrored { continuing.swapped() } else { continuing };
        let Some(step) = _signed(&counter.step, facts, counter.start.width()) else { continue };
        if step == BigInt::from(0) || width != counter.start.width() {
            continue;
        }
        let Some(bound) = term(unit, bound).filter(|bound| bound.width() == width) else { continue };
        if matches!(&bound, AffineOperand::Value(value, _) if !still.contains(*value)) {
            continue;
        }
        let zero = BigInt::from(0);
        if !(test == IntPredicate::Ne || (_ascending(test) && step > zero) || (_descending(test) && step < zero)) {
            continue;
        }
        let start = counter.start.clone();
        let begin = _constant(&start, facts, width);
        let limit = _constant(&bound, facts, width);
        let difference = _difference(unit, &bound, &start, begin.as_ref(), limit.as_ref(), facts, width);
        let count = match (&difference, &begin, &limit) {
            (Some(difference), _, _) if test == IntPredicate::Ne => _equal_after(difference, &step, width, shape.posttested, stepped),
            (_, Some(begin), Some(limit)) if test != IntPredicate::Ne || difference.is_none() => {
                _ordered_after(begin, limit, &step, test, width, shape.posttested, stepped)
            }
            _ => None,
        };
        let (mut first, mut last) = (None, None);
        let maximum = if let Some(count) = &count {
            (first, last) = _signed_span(&start, facts, width, count, &step);
            Some(count.clone())
        } else if shape.posttested || abs(&step) != BigInt::from(1) {
            continue;
        } else {
            let promised = _promised(function, update, &step, _unsigned(test));
            let found = _unit_maximum(unit, loop_, width, begin.as_ref(), limit.as_ref(), &step, test, inbounds, promised);
            if found.is_none() && _inclusive(test) {
                continue;
            }
            found
        };
        proven.push(CountedLoop {
            counter: counter.clone(),
            phi,
            compare,
            branch,
            start: begin.map_or_else(|| start.clone(), |begin| AffineOperand::constant(begin, width)),
            bound: limit.map_or_else(|| bound.clone(), |limit| AffineOperand::constant(limit, width)),
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

/// `(width, bound, mirrored, stepped)` where `icmp` tests a counter value in `tested`.
fn _compared(unit: &Unit, icmp: &Instruction, tested: &BTreeMap<ValueId, bool>) -> Option<(u32, Operand, bool, bool)> {
    for (index, operand) in icmp.operands.iter().enumerate().take(2) {
        let Operand::Value(value) = operand else { continue };
        let Some(&stepped) = tested.get(value) else { continue };
        return Some((unit.int_bits(*operand)?, icmp.operands[1 - index], index == 1, stepped));
    }
    None
}

/// Whether the step `update` makes is promised not to wrap as `unsigned` or signed integers.
fn _promised(function: &Function, update: ValueId, step: &BigInt, unsigned: bool) -> bool {
    let Some(inst) = defining(function, update) else { return false };
    let op = function.instruction(inst);
    let upward = step > &BigInt::from(0);
    match op.opcode {
        Opcode::Binary(BinaryOp::Add) if unsigned => upward && op.flags.contains(Flags::NUW),
        Opcode::Binary(BinaryOp::Sub) if unsigned => !upward && op.flags.contains(Flags::NUW),
        Opcode::Binary(BinaryOp::Add | BinaryOp::Sub) => op.flags.contains(Flags::NSW),
        _ => false,
    }
}

/// `bound - start` modulo the width, when constant: both constant, or `bound = start + c`.
fn _difference(
    unit: &Unit,
    bound: &AffineOperand,
    start: &AffineOperand,
    begin: Option<&BigInt>,
    limit: Option<&BigInt>,
    facts: &IndexMap<ValueId, Known>,
    width: u32,
) -> Option<BigInt> {
    if let (Some(begin), Some(limit)) = (begin, limit) {
        return Some(masked(&(limit - begin), width));
    }
    let (root, ahead) = anchored(unit, bound, width, Some(facts));
    let (other, behind) = anchored(unit, start, width, Some(facts));
    (root == other).then(|| masked(&(ahead - behind), width))
}

/// `term` as terms times coefficients, through adds and subtracts. A
/// constant is a term; `headers` stay terms. A value no rule expands is a
/// term too when `opaque`, else None.
pub fn linear(
    unit: &Unit,
    term_: &AffineOperand,
    headers: &BTreeSet<ValueId>,
    width: u32,
    opaque: bool,
    visiting: &BTreeSet<ValueId>,
    cached: &mut IndexMap<AffineOperand, IndexMap<AffineOperand, BigInt>>,
) -> Option<IndexMap<AffineOperand, BigInt>> {
    let value = match term_ {
        AffineOperand::Const(constant) if constant.width == width => None,
        AffineOperand::Value(value, one) if *one == width => Some(*value),
        _ => return None,
    };
    let leaf = || Some(IndexMap::from_iter([(term_.clone(), BigInt::from(1))]));
    let made = value
        .filter(|value| !headers.contains(value))
        .and_then(|value| unit.defining(Operand::Value(value)))
        .filter(|(_, op)| op.opcode != Opcode::Phi);
    let (Some(value), Some((_, op))) = (value, made) else { return leaf() };
    if visiting.contains(&value) {
        return None;
    }
    if let Some(found) = cached.get(term_) {
        return Some(found.clone());
    }
    let parts = match op.opcode {
        Opcode::Binary(kind @ (BinaryOp::Add | BinaryOp::Sub)) => {
            let (Some(left), Some(right)) = (term(unit, op.operands[0]), term(unit, op.operands[1])) else { return if opaque { leaf() } else { None } };
            vec![(left, BigInt::from(1)), (right, BigInt::from(if kind == BinaryOp::Sub { -1 } else { 1 }))]
        }
        _ => return if opaque { leaf() } else { None },
    };
    let mut result = IndexMap::<AffineOperand, BigInt>::default();
    let mut deeper = visiting.clone();
    deeper.insert(value);
    for (source, coefficient) in parts {
        for (one, factor) in linear(unit, &source, headers, width, opaque, &deeper, cached)? {
            *result.entry(one).or_insert_with(|| BigInt::from(0)) += &coefficient * factor;
        }
    }
    let kept = result.into_iter().filter(|(_, factor)| *factor != BigInt::from(0)).collect::<IndexMap<_, _>>();
    cached.insert(term_.clone(), kept.clone());
    Some(kept)
}

/// How far `one` lies above `other`, where their terms other than constants agree.
///
/// Strength reduction starts `a[i].x` at `n + (m + 600)` and `a[i].y` at
/// `n + (m + 606)`: no single root, but 6 apart.
pub fn distance(unit: &Unit, one: &AffineOperand, other: &AffineOperand, width: u32) -> Option<BigInt> {
    let terms_of = |of: &AffineOperand| linear(unit, of, &BTreeSet::new(), width, true, &BTreeSet::new(), &mut IndexMap::default());
    let mut terms = terms_of(one)?;
    for (one, factor) in terms_of(other)? {
        *terms.entry(one).or_insert_with(|| BigInt::from(0)) -= factor;
    }
    let mut apart = BigInt::from(0);
    for (one, factor) in terms {
        match one {
            AffineOperand::Const(constant) => apart += &constant.n * factor,
            _ if factor == BigInt::from(0) => {}
            _ => return None,
        }
    }
    Some(masked(&apart, width))
}

/// `term` as a root value plus a constant, through constant adds; a number has no root.
///
/// Two values with one root are a constant apart, which is how a loop from
/// `x - 32` to `x` is counted and how two counters starting 4 apart share one.
pub fn anchored(unit: &Unit, term_: &AffineOperand, width: u32, facts: Option<&IndexMap<ValueId, Known>>) -> (Option<ValueId>, BigInt) {
    let empty = IndexMap::default();
    let facts = facts.unwrap_or(&empty);
    let mut term_ = term_.clone();
    let mut offset = BigInt::from(0);
    while let AffineOperand::Value(value, one) = term_ {
        if one != width {
            break;
        }
        if let Some(known) = _constant(&term_, facts, width) {
            term_ = AffineOperand::constant(known, width);
            break;
        }
        let Some((_, op)) = unit.defining(Operand::Value(value)) else { break };
        if op.opcode != Opcode::Binary(BinaryOp::Add) {
            break;
        }
        let (Some(left), Some(right)) = (term(unit, op.operands[0]), term(unit, op.operands[1])) else { break };
        let ((AffineOperand::Const(constant), other) | (other, AffineOperand::Const(constant))) = (left, right) else { break };
        offset += &constant.n;
        term_ = other;
    }
    match term_ {
        AffineOperand::Const(constant) => (None, masked(&(&constant.n + offset), width)),
        AffineOperand::Value(value, _) => (Some(value), masked(&offset, width)),
    }
}

/// Trips until `start + k*step`, tested as the loop is shaped, first equals `start + difference`.
fn _equal_after(difference: &BigInt, step: &BigInt, width: u32, posttested: bool, stepped: bool) -> Option<BigInt> {
    let modulus = BigInt::from(1) << width;
    let lead = u8::from(posttested && stepped);
    let divisor = gcd(mod_floor(step, &modulus), modulus.clone());
    let remaining = mod_floor(&(difference - step * lead), &modulus);
    if mod_floor(&remaining, &divisor) != BigInt::from(0) {
        return None; // never equal: the loop does not end
    }
    let period = &modulus / &divisor;
    let inverse = modular_inverse(&floor_div(&mod_floor(step, &modulus), &divisor), &period)?;
    Some(BigInt::from(u8::from(posttested)) + mod_floor(&(floor_div(&remaining, &divisor) * inverse), &period))
}

/// The low and high of a test's integers: unsigned or signed at `width`.
fn _extent(unsigned: bool, width: u32) -> (BigInt, BigInt) {
    if unsigned {
        (BigInt::from(0), (BigInt::from(1) << width) - 1)
    } else {
        (-(BigInt::from(1) << (width - 1)), (BigInt::from(1) << (width - 1)) - 1)
    }
}

/// Trips of an ordered test with constant ends, or None where a tested value would wrap first.
fn _ordered_after(begin: &BigInt, limit: &BigInt, step: &BigInt, test: IntPredicate, width: u32, posttested: bool, stepped: bool) -> Option<BigInt> {
    let unsigned = _unsigned(test);
    let (low, high) = _extent(unsigned, width);
    let first = (if unsigned { begin.clone() } else { _as_signed(begin, width) }) + step * u8::from(posttested && stepped);
    let bound = if unsigned { limit.clone() } else { _as_signed(limit, width) };
    if !(low <= first && first <= high) {
        return None;
    }
    let inclusive = u8::from(_inclusive(test));
    let zero = BigInt::from(0);
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
fn _signed_span(start: &AffineOperand, facts: &IndexMap<ValueId, Known>, width: u32, count: &BigInt, step: &BigInt) -> (Option<BigInt>, Option<BigInt>) {
    let Some(begin) = _signed(start, facts, width) else { return (None, None) };
    if count < &BigInt::from(1) {
        return (None, None);
    }
    let last = &begin + (count - 1) * step;
    let sign = BigInt::from(1) << (width - 1);
    let after = &last + step;
    if -&sign <= last && last < sign && -&sign <= after && after < sign { (Some(begin), Some(last)) } else { (None, None) }
}

/// Most trips of a symbolic unit-step loop, where proved; None for an inclusive test that may never end.
#[allow(clippy::too_many_arguments)]
fn _unit_maximum(
    unit: &Unit,
    loop_: &Loop,
    width: u32,
    begin: Option<&BigInt>,
    limit: Option<&BigInt>,
    step: &BigInt,
    test: IntPredicate,
    inbounds: bool,
    promised: bool,
) -> Option<BigInt> {
    if test == IntPredicate::Ne {
        return Some((BigInt::from(1) << width) - 1);
    }
    let (unsigned, inclusive) = (_unsigned(test), _inclusive(test));
    let (low, high) = _extent(unsigned, width);
    let ascending = step > &BigInt::from(0);
    // Walked toward `bound`, as integers in the test's own signedness.
    let signed = |value: &BigInt| if unsigned { value.clone() } else { _as_signed(value, width) };
    let mut origin = begin.map(signed);
    let mut target = limit.map(signed);
    let end = if ascending { &high } else { &low };
    // Stepping past the width's end wraps back inside an inclusive bound,
    // unless the step promised it never wraps: then the program stops first.
    let endless = inclusive && !promised;
    if endless && target.as_ref() == Some(end) {
        return None;
    }
    if !endless && inclusive {
        // Promised: the counter runs between the width's ends whatever it is given.
        target = target.or_else(|| Some(end.clone()));
        origin = origin.or_else(|| Some(if ascending { low.clone() } else { high.clone() }));
    }
    if !inclusive {
        // An exclusive bound lies inside the width: the counter stops by its end.
        target = target.or_else(|| Some(end.clone()));
    }
    let ranged = match (&origin, &target) {
        (Some(origin), Some(target)) if low <= *origin.min(target) && *origin.max(target) <= high => {
            Some(max(BigInt::from(0), (target - origin) * step + u8::from(inclusive)))
        }
        _ => None,
    };
    // Both bound the trips; keep the tighter.
    let bounded = if inbounds { _inbounds_trips(unit, loop_, *loop_.latches.first().expect("one latch")) } else { None };
    ranged.into_iter().chain(bounded).min()
}

/// How far each counter and each value affine in one advances per iteration.
///
/// The per-iteration view of `basics` and `derived`; nothing here
/// re-derives which values are affine.
pub fn advances(unit: &Unit, loop_: &Loop) -> IndexMap<ValueId, BigInt> {
    let found = basics(unit, loop_);
    let mut out = IndexMap::default();
    for (value, affine) in &found {
        if let AffineOperand::Const(step) = &affine.step {
            out.insert(*value, _as_signed(&step.n, step.width));
        }
    }
    for one in derived(unit, loop_, Some(&found)) {
        if let (AffineOperand::Const(step), AffineOperand::Const(by), None, Some(result)) =
            (&one.of.step, &one.by, &one.pointer, unit.function.instruction(one.op).result)
        {
            out.insert(result, _as_signed(&step.n, step.width) * _as_signed(&by.n, by.width));
        }
    }
    out.into_iter().filter(|(_, step)| *step != BigInt::from(0)).collect()
}

/// The most iterations an access made every iteration allows, as LLVM's inbounds does.
///
/// Iteration i reaches `b + i*s` inside one object, and an index `w` bits
/// wide addresses at most 2**w bytes of it, so i*s + width <= 2**w. Only
/// an access through `inbounds` GEPs is promised that.
fn _inbounds_trips(unit: &Unit, loop_: &Loop, latch: i64) -> Option<BigInt> {
    let step = advances(unit, loop_);
    let dominators = loops::dominators(&cfg::graph(unit.function), None);
    let empty = BTreeSet::new();
    let every = dominators.get(&latch).unwrap_or(&empty);
    loop_
        .body
        .iter()
        // The header also runs the final, failing test: n + 1 times.
        .filter(|at| every.contains(at) && **at != loop_.header)
        .flat_map(|&at| unit.function.block(cfg::block(at)).instructions())
        .filter_map(|&inst| MemRef::of(unit, inst))
        .filter(|reference| reference.inbounds)
        .filter_map(|reference| {
            let advance = step.get(&reference.base?)? * reference.scale;
            (advance != BigInt::from(0))
                .then(|| ((BigInt::from(1) << reference.index_bits) - reference.width) / abs(&advance) + 1)
        })
        .min()
}

/// `term`'s number at `width`, where known.
fn _constant(term_: &AffineOperand, facts: &IndexMap<ValueId, Known>, width: u32) -> Option<BigInt> {
    if term_.width() != width {
        return None;
    }
    match term_ {
        AffineOperand::Value(value, _) => {
            let fact = facts.get(value)?;
            (fact.width >= width).then(|| masked(&fact.n, width))
        }
        AffineOperand::Const(constant) => Some(masked(&constant.n, width)),
    }
}

fn _as_signed(value: &BigInt, width: u32) -> BigInt {
    let sign = BigInt::from(1) << (width - 1);
    (value ^ &sign) - sign
}

/// `term`'s signed number at `width`, where known.
pub fn _signed(term_: &AffineOperand, facts: &IndexMap<ValueId, Known>, width: u32) -> Option<BigInt> {
    _constant(term_, facts, width).map(|value| _as_signed(&value, width))
}

/// The one positive trip count every counter of a loop proves, if they prove one.
///
/// A loop may carry several counters at once. They are evidence for the
/// same trip count, not alternatives a transform may pick from; refusing
/// disagreement keeps transforms independent of visiting order.
pub fn agreed_count(proofs: &[CountedLoop]) -> Option<BigInt> {
    let zero = BigInt::from(0);
    let counts = proofs.iter().filter_map(|proof| proof.count.clone()).filter(|count| *count != zero).collect::<BTreeSet<_>>();
    if counts.len() == 1 { counts.into_iter().next() } else { None }
}

/// The loop's trips, where its counters agree on a positive count.
pub fn trip_count(unit: &Unit, loop_: &Loop, facts: &IndexMap<ValueId, Known>) -> Option<BigInt> {
    agreed_count(&counted(unit, loop_, Some(facts), false))
}

/// `trip_count` for a loop that may also stop the program: its trips whenever it does not.
pub fn trips_unless_stopped(unit: &Unit, loop_: &Loop, facts: &IndexMap<ValueId, Known>) -> Option<BigInt> {
    agreed_count(&counted_unless_stopped(unit, loop_, Some(facts), false))
}

/// A counted loop whose first iteration and finite exit are proven.
pub fn nonempty(unit: &Unit, loop_: &Loop) -> bool {
    trip_count(unit, loop_, &consts::known(unit, None, None, None)).is_some()
}

/// The proof in which `counter` decides when `loop` leaves.
pub fn controlling(unit: &Unit, loop_: &Loop, counter: &Affine, facts: &IndexMap<ValueId, Known>) -> Option<CountedLoop> {
    counted(unit, loop_, Some(facts), false).into_iter().find(|proof| proof.counter.value == counter.value)
}

fn abs(value: &BigInt) -> BigInt {
    if value < &BigInt::from(0) { -value } else { value.clone() }
}

/// Floor division for a nonzero divisor; `BigInt` truncates.
pub fn floor_div(numerator: &BigInt, denominator: &BigInt) -> BigInt {
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    if remainder != BigInt::from(0) && ((remainder < BigInt::from(0)) != (denominator < &BigInt::from(0))) { quotient - 1 } else { quotient }
}

/// The remainder with the positive modulus's sign.
pub fn mod_floor(value: &BigInt, modulus: &BigInt) -> BigInt {
    let remainder = value % modulus;
    if remainder < BigInt::from(0) { remainder + modulus } else { remainder }
}

/// `value`'s inverse modulo `modulus`, if it has one.
fn modular_inverse(value: &BigInt, modulus: &BigInt) -> Option<BigInt> {
    let (mut old_r, mut r) = (mod_floor(value, modulus), modulus.clone());
    let (mut old_s, mut s) = (BigInt::from(1), BigInt::from(0));
    while r != BigInt::from(0) {
        let quotient = &old_r / &r;
        let next_r = &old_r - &quotient * &r;
        old_r = std::mem::replace(&mut r, next_r);
        let next_s = &old_s - &quotient * &s;
        old_s = std::mem::replace(&mut s, next_s);
    }
    (old_r == BigInt::from(1)).then(|| mod_floor(&old_s, modulus))
}

pub fn gcd(mut one: BigInt, mut other: BigInt) -> BigInt {
    while other != BigInt::from(0) {
        let remainder = one % &other;
        one = std::mem::replace(&mut other, remainder);
    }
    one
}

/// The values a loop does not define.
#[derive(Clone, Debug, Default)]
pub struct Invariant {
    defined: HashSet<ValueId>,
}

impl Invariant {
    pub fn contains(&self, value: ValueId) -> bool {
        !self.defined.contains(&value)
    }

    /// A constant, or a value the loop does not define.
    pub fn operand(&self, operand: Operand) -> bool {
        match operand {
            Operand::Value(value) => self.contains(value),
            Operand::Constant(_) => true,
            Operand::Block(_) => false,
        }
    }
}

/// What the blocks `inside` do not define.
pub fn invariant(function: &Function, inside: &BTreeSet<i64>) -> Invariant {
    let defined = inside
        .iter()
        .flat_map(|&at| function.block(cfg::block(at)).instructions())
        .filter_map(|&inst| function.instruction(inst).result)
        .collect();
    Invariant { defined }
}

/// The header phis that one invariant or constant step advances on every
/// way round, from one start on every way in.
pub fn basics(unit: &Unit, loop_: &Loop) -> IndexMap<ValueId, Affine> {
    let function = unit.function;
    let header = cfg::block(loop_.header);
    let mut out = IndexMap::default();
    if !function.layout().contains(&header) {
        return out;
    }
    let inside = &loop_.body;
    let still = invariant(function, inside);
    let within = |inst: InstId| function.parent(inst).is_some_and(|block| inside.contains(&cfg::id(block)));
    for &inst in function.block(header).instructions() {
        let phi = function.instruction(inst);
        if phi.opcode != Opcode::Phi {
            break;
        }
        let Some(result) = phi.result.filter(|&result| unit.int_bits(Operand::Value(result)).is_some()) else { continue };
        let pairs = phi.operands.chunks(2).filter_map(|pair| match pair[1] {
            Operand::Block(from) => Some((pair[0], inside.contains(&cfg::id(from)))),
            _ => None,
        });
        let (around, into): (Vec<_>, Vec<_>) = pairs.partition(|(_, back)| *back);
        let Some(&(start, _)) = into.first() else { continue };
        if into.iter().any(|(one, _)| *one != start) {
            continue;
        }
        let steps = around
            .iter()
            .map(|(one, _)| match one {
                Operand::Value(value) => defining(function, *value).filter(|&made| within(made)).and_then(|made| _stepped(unit, made, result, &still)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let (Some(Some(step)), Some(start)) = (steps.first(), term(unit, start)) else { continue };
        if steps.iter().all(|one| one.as_ref() == Some(step)) {
            out.insert(result, Affine { value: result, start, step: step.clone(), header: loop_.header });
        }
    }
    out
}

/// `(stepped, step)` where `op` adds `step` to `stepped`: an `add`, or a `sub` of a constant.
pub fn stepping(unit: &Unit, op: &Instruction) -> Option<(AffineOperand, AffineOperand)> {
    let Opcode::Binary(kind) = op.opcode else { return None };
    let (left, right) = (term(unit, op.operands[0])?, term(unit, op.operands[1])?);
    match (kind, right) {
        (BinaryOp::Add, right) => Some((left, right)),
        (BinaryOp::Sub, AffineOperand::Const(constant)) => Some((left, AffineOperand::constant(-&constant.n, constant.width))),
        _ => None,
    }
}

/// The invariant or constant step `inst` adds to `value`.
fn _stepped(unit: &Unit, inst: InstId, value: ValueId, still: &Invariant) -> Option<AffineOperand> {
    let (mut stepped, mut step) = stepping(unit, unit.function.instruction(inst))?;
    let is_value = |one: &AffineOperand| matches!(one, AffineOperand::Value(found, _) if *found == value);
    if !is_value(&stepped) {
        if !is_value(&step) {
            return None;
        }
        std::mem::swap(&mut stepped, &mut step);
    }
    match step {
        AffineOperand::Const(_) => Some(step),
        AffineOperand::Value(one, _) if still.contains(one) => Some(step),
        AffineOperand::Value(..) => None,
    }
}

/// Whether the counted loop's own recurrence is read by nothing but its
/// control, its step, `covered` and the exit's phis: then it may go.
///
/// A proof, not a transform: the caller names the instructions it will
/// replace.
pub fn control_replacement<'a>(unit: &Unit, loop_: &Loop, proof: &'a CountedLoop, covered: &BTreeSet<InstId>) -> Option<ControlReplacement<'a>> {
    let function = unit.function;
    if proof.posttested || proof.preheader.is_none() || proof.width() != proof.counter.start.width() {
        return None;
    }
    let (header, latch) = (cfg::block(loop_.header), cfg::block(proof.latch));
    let first = function.block(latch).instructions().first().map(|&one| &function.instruction(one).opcode);
    if loop_.body.len() != 2 || proof.entered != proof.latch || first == Some(&Opcode::Phi) || function.predecessors(latch) != [header] {
        return None;
    }
    let tests_only = function.block(header).instructions().iter().all(|&inst| {
        inst == proof.compare || inst == proof.branch || matches!(function.instruction(inst).opcode, Opcode::Phi | Opcode::ICmp(_))
    });
    if !tests_only {
        return None;
    }
    let counter = proof.counter.value;
    let Some(Operand::Value(update)) = incoming(function, proof.phi, latch) else { return None };
    let stepping_inst = defining(function, update)?;
    let is_counter = |one: &AffineOperand| matches!(one, AffineOperand::Value(value, _) if *value == counter);
    if !stepping(unit, function.instruction(stepping_inst)).is_some_and(|(one, other)| is_counter(&one) || is_counter(&other)) {
        return None;
    }
    let is_phi = |inst: InstId| function.instruction(inst).opcode == Opcode::Phi;
    let allowed = |inst: InstId| covered.contains(&inst) || inst == proof.compare || inst == stepping_inst;
    if function.users(counter).iter().any(|one| !is_phi(one.user) && !allowed(one.user))
        || function.users(update).iter().any(|one| !is_phi(one.user))
    {
        return None;
    }
    let exit = cfg::block(proof.exit);
    let expected = [Operand::Value(counter), Operand::Block(header)];
    let exits = phis(function)
        .filter(|(_, block, phi)| *block == exit && phi.operands == expected)
        .map(|(inst, _, _)| inst)
        .collect::<Vec<_>>();
    let reads = |phi: &Instruction| phi.operands.iter().any(|one| *one == Operand::Value(counter) || *one == Operand::Value(update));
    if phis(function).any(|(inst, _, phi)| inst != proof.phi && !exits.contains(&inst) && reads(phi)) {
        return None;
    }
    let result = function.instruction(proof.compare).result?;
    if function.users(result).iter().any(|one| one.user != proof.branch) {
        return None;
    }
    Some(ControlReplacement { counted: proof, stepping: stepping_inst, update, exits })
}

/// Prove that `candidate` can supply a counted loop's terminating test.
///
/// `covered` are the counter's reads the caller rebases; the counter itself
/// is a candidate when they are all its data reads.
pub fn zero_terminating_control<'a>(
    unit: &Unit,
    loop_: &Loop,
    proof: &'a CountedLoop,
    candidate: &Affine,
    covered: &BTreeSet<InstId>,
    facts: Option<&IndexMap<ValueId, Known>>,
) -> Option<ZeroTerminatingControl<'a>> {
    let replacement = control_replacement(unit, loop_, proof, covered)?;
    let maximum = proof.maximum.as_ref()?;
    let width = proof.counter.start.width();
    if candidate.start.width() != width || candidate.step.width() != width || maximum < &BigInt::from(0) {
        return None;
    }
    let computed;
    let facts = match facts {
        Some(facts) => facts,
        None => {
            computed = consts::known(unit, None, None, None);
            &computed
        }
    };
    let step = _signed(&candidate.step, facts, width)?;
    if step == BigInt::from(0) {
        return None;
    }
    let period = AffineMap { scale: step.clone(), offset: BigInt::from(0), width }.period();
    if maximum > &period {
        return None;
    }
    Some(ZeroTerminatingControl { replacement, candidate: candidate.clone(), step, maximum: maximum.clone(), period })
}

#[cfg(test)]
#[path = "induction_tests.rs"]
pub(crate) mod tests;
