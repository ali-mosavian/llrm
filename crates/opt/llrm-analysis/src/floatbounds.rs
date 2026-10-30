//! Floating operations that are exact over bounded, not necessarily known,
//! integers.
//!
//! # How it works
//! `exact(unit, constants)` returns the float instructions proven numerically
//! exact: every value they can produce is an integer of magnitude at most 2^24
//! (`binary32`) or 2^53 (`binary64`), so conversion and the arithmetic stay
//! exact. It is interval arithmetic over the same `Rule`s as
//! [`crate::floatfacts`] (`evaluated` on `(low, high)` pairs: `Neg` flips,
//! `Abs` splits at zero, `Add`, `Sub` and `Mul` combine the endpoints, `Div`
//! and `Sqrt` have no rule) and fails closed where the result leaves the
//! format's exact range.
//!
//! Operands. An integer input is its whole type range. A float is bounded where
//! floatfacts knows it as an integer constant, or where it was bounded in an
//! earlier round. A float phi is bounded by the union of its incoming bounds.
//! Memory: for a load, `_memory` bounds every element it may read, from the
//! index's interval times its scale (at most 64 elements) and the cell facts at
//! the load (`floatfacts::cells_before`).
//!
//! The pass iterates to a fixed point, each round bounding phis and then trying
//! every pending instruction, so a result proven in one round feeds the next.
//!
//! ```text
//!    %i  in [0, 100]             int input: its interval
//!    %f = sitofp i16 %i          Convert      -> [0, 100]
//!    %g = fmul double %f, %f     Mul          -> [0, 10000]  < 2^53: exact
//!    %h = fadd double %g, 0.5    0.5 not an integer: not exact
//! ```
//!
//! # Example
//! Floats converted from 16-bit integers (`%a`, `%b`) are bounded integers, so
//! sums stay exact:
//!
//! ```mir
//! define void @f(i16 %a, i16 %b, i1 %c) {
//! b0:
//!   %x = sitofp i16 %a to float
//!   %y = sitofp i16 %b to float
//!   %sum = fadd float %x, %y
//!   %product = fmul float %x, %y
//!   ; ...
//!   %half = fmul float %x, 0.5
//!   ; ...
//! }
//! ```
//!
//! | Value | `exact` | Why |
//! | --- | --- | --- |
//! | `%x` | yes | in `[-32768, 32767]` |
//! | `%sum` | yes | in `[-65536, 65534]` |
//! | `%product` | no | up to 2^30, past 2^24 |
//! | `%half` | no | 0.5 is not an integer |
//!
//! From unit test `test_integer_inputs_bound_their_float_work` in
//! `floatbounds_tests.rs`.
//!
//! # Provides
//! `exact`, `evaluated`, `_memory`. No pass consumes them: only this module's
//! tests call them.
//!
//! # Needs
//! [`crate::floatfacts`] (`rule`, `Format`, facts and `cells_before`),
//! [`crate::consts`], [`crate::ranges`] (the edges' intervals at a load).
//!
//! # See also
//! LLVM ValueTracking's `computeKnownFPClass` with `isKnownExactCastIntToFP`.

use std::collections::BTreeSet;

use llrm_mir::context::ConstantKind;
use llrm_mir::module::{InstId, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_support::hash::IndexMap;

use crate::cfg;
use crate::consts::{self, Calls, Cells};
use crate::floatfacts::{self, Finite, Format, Operation, Rule};
use crate::memory::{MemRef, Unit};
use crate::num::Num as BigInt;
use crate::ranges;

pub type Bounds = (BigInt, BigInt);

/// The bounds of `rule` on `inputs`, where every value between is exact.
pub fn evaluated(
    rule: &Rule,
    inputs: &[Bounds],
) -> Option<Bounds> {
    if inputs.len() != rule.inputs.len() {
        return None;
    }
    let zero = BigInt::from(0);
    let absolute = |n: &BigInt| if *n < BigInt::from(0) { -n } else { n.clone() };
    let result = match (rule.operation, inputs) {
        (Operation::Convert | Operation::Truncate, [value]) => value.clone(),
        (Operation::Neg, [(low, high)]) => (-high, -low),
        (Operation::Abs, [(low, high)]) => (
            if *low <= zero && zero <= *high { zero.clone() } else { absolute(low).min(absolute(high)) },
            absolute(low).max(absolute(high)),
        ),
        (Operation::Add, [(low, high), (other_low, other_high)]) => (low + other_low, high + other_high),
        (Operation::Sub, [(low, high), (other_low, other_high)]) => (low - other_high, high - other_low),
        (Operation::Mul, [(low, high), (other_low, other_high)]) => {
            let products = [low * other_low, low * other_high, high * other_low, high * other_high];
            (products.iter().min().expect("four").clone(), products.iter().max().expect("four").clone())
        }
        _ => return None,
    };
    let (low, high) = (&result.0, &result.1);
    let fits = match rule.result {
        Format::Signed(width) => {
            let limit = BigInt::from(1) << (width - 1);
            -&limit <= *low && low <= high && *high < limit
        }
        Format::Unsigned(width) => zero <= *low && low <= high && *high < BigInt::from(1) << width,
        binary => {
            let limit = BigInt::from(1) << if binary == Format::Binary32 { 24 } else { 53 };
            -&limit <= *low && low <= high && *high <= limit
        }
    };
    fits.then_some(result)
}

/// Bound every element the load `inst` may read, from the bytes proved
/// there.
pub fn _memory(
    unit: &Unit,
    inst: InstId,
    format: Format,
    read: &mut dyn FnMut(&[MemRef]) -> Cells,
    scoped: &dyn crate::ranges::Lookup,
) -> Option<Bounds> {
    let reference = MemRef::of(unit, inst)?;
    let width = match format {
        Format::Binary32 => 4,
        Format::Binary64 => 8,
        _ => return None,
    };
    if reference.width != width || reference.segment.is_some() {
        return None;
    }
    let mut offsets = vec![BigInt::from(0)];
    if let Some(base) = reference.base {
        let interval = scoped.lookup(&base)?;
        if ranges::covering(&reference, scoped).base.is_some() {
            return None;
        }
        offsets = Vec::new();
        let mut index = interval.low.clone();
        while index <= interval.high {
            offsets.push(&index * reference.scale);
            if offsets.len() > 64 {
                return None;
            }
            index += 1;
        }
    }
    let cells = offsets
        .into_iter()
        .map(|offset| {
            Some(MemRef {
                base: None,
                scale: 0,
                disp: reference.disp.checked_add(i64::try_from(offset).ok()?)?,
                ..reference.clone()
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let memory = read(&cells);
    let mut values = Vec::new();
    for cell in cells {
        let bits = consts::_cell(&memory, &cell)?;
        let value = floatfacts::decoded(&bits.n, format).filter(|value| value.value.denominator == BigInt::from(1))?;
        values.push(value.value.numerator);
    }
    Some((values.iter().min()?.clone(), values.iter().max()?.clone()))
}

/// One operand's bounds: an integer's whole range, or a float's where it
/// is known or bounded.
fn _operand(
    unit: &Unit,
    operand: Operand,
    format: Format,
    values: &IndexMap<ValueId, Bounds>,
    constants: &IndexMap<ValueId, Finite>,
) -> Option<Bounds> {
    let integral = |fact: Finite| {
        (fact.value.denominator == BigInt::from(1)).then(|| (fact.value.numerator.clone(), fact.value.numerator))
    };
    match format {
        Format::Signed(width) => {
            let limit = BigInt::from(1) << (width - 1);
            Some((-&limit, limit - 1))
        }
        Format::Unsigned(width) => Some((BigInt::from(0), (BigInt::from(1) << width) - 1)),
        _ => match operand {
            Operand::Value(value) => {
                constants.get(&value).cloned().and_then(integral).or_else(|| values.get(&value).cloned())
            }
            Operand::Constant(id) => match unit.context.get(id).kind {
                ConstantKind::Float(bits) => integral(floatfacts::decoded(&BigInt::from(bits), format)?),
                _ => None,
            },
            Operand::Block(_) => None,
        },
    }
}

/// The instructions proven numerically exact.
pub fn exact(
    unit: &Unit,
    constants: &IndexMap<ValueId, Finite>,
) -> Result<BTreeSet<InstId>, String> {
    let function = unit.function;
    let mut pending = function
        .walk()
        .filter_map(|(block, inst)| floatfacts::rule(unit, inst).map(|rule| (block, inst, rule)))
        .collect::<Vec<_>>();
    if pending.is_empty() {
        return Ok(BTreeSet::new());
    }
    let integers = consts::known_walked(unit, &Calls::default());
    let mut asked = consts::memory_queries(*unit, &integers);
    let scoped = ranges::dominated_edges(unit)?;
    let empty = ranges::Intervals::default();
    let phis = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| {
            function.instruction(inst).opcode == Opcode::Phi
                && Format::of(&unit.context.types, function.instruction(inst).ty).is_some()
        })
        .collect::<Vec<_>>();
    let mut safe = BTreeSet::new();
    let mut values = IndexMap::<ValueId, Bounds>::default();
    let mut changed = true;
    while changed {
        changed = false;
        for &phi in &phis {
            let op = function.instruction(phi);
            let result = op.result.expect("a phi's value");
            if values.contains_key(&result) {
                continue;
            }
            let format = Format::of(&unit.context.types, op.ty).expect("a float phi");
            let Some(bounds) = op
                .operands
                .iter()
                .step_by(2)
                .map(|&one| _operand(unit, one, format, &values, constants))
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            let low = bounds.iter().map(|(low, _)| low).min().expect("incoming").clone();
            let high = bounds.iter().map(|(_, high)| high).max().expect("incoming").clone();
            values.insert(result, (low, high));
            changed = true;
        }
        let mut remaining = Vec::new();
        for (block, inst, rule) in pending {
            let op = function.instruction(inst);
            let inputs = if let Opcode::Load { .. } = op.opcode {
                let mut read = |references: &[MemRef]| {
                    floatfacts::cells_before(
                        unit,
                        &Calls::default(),
                        &integers,
                        constants,
                        consts::ReadAt::Before(inst),
                        references,
                        &mut asked,
                    )
                };
                _memory(unit, inst, rule.inputs[0], &mut read, scoped.get(&cfg::id(block)).unwrap_or(&empty))
                    .map(|one| vec![one])
            } else {
                rule.inputs
                    .iter()
                    .zip(&op.operands)
                    .map(|(&format, &operand)| _operand(unit, operand, format, &values, constants))
                    .collect()
            };
            let Some(result) = inputs.and_then(|inputs| evaluated(&rule, &inputs)) else {
                remaining.push((block, inst, rule));
                continue;
            };
            safe.insert(inst);
            if let Some(value) = op.result {
                values.insert(value, result);
            }
            changed = true;
        }
        pending = remaining;
    }
    Ok(safe)
}

#[cfg(test)]
#[path = "floatbounds_tests.rs"]
mod tests;
