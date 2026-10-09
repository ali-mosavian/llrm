//! Floating operations exact over bounded, not necessarily known, integers:
//! llrm-core's `analysis/floatbounds.rs`, a port of
//! `qbopt/analysis/floatbounds.py`, adapted to the rich MIR. LLVM's
//! counterpart is ValueTracking's `computeKnownFPClass` with its integer
//! range reasoning (`isKnownExactCastIntToFP`).
//!
//! What changed with the IR:
//! - A rule is floatfacts' (`floatfacts::rule`), a load's reading memory.
//! - An indexed load's elements are its index's interval times its scale; the old one found an element's stride in a
//!   `shl` of a byte offset.
//! - A float phi is bounded by its incomings.
//! - An exact operation is its instruction, where the old one was its occurrence.
//!
//! Dropped, no rich MIR analogue: `Extended80` precision; the shadow that
//! made a barrier or an opaque call write everything (a call writes what
//! `memory::unmodeled_write` says); an x86 register base.
//!
//! Tests skipped: BC object corpora and runtime helper contracts
//! (`test_fpdeep_reuses_proven_finite_array_loads`,
//! `test_computed_runtime_integer_uses_one_conversion`,
//! `test_helper_conversion_respects_its_effect_contract`,
//! `test_runtime_integer_conversion_is_shared_in_emitted_code`);
//! `test_unknown_integer_loads_share_a_value_but_unknown_floats_do_not`
//! (gvn's, which reuses float work freely). The array tests are rewritten
//! as MIR.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::context::ConstantKind;
use llrm_mir::module::{InstId, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_support::hash::IndexMap;
use num_bigint::BigInt;

use crate::cfg;
use crate::consts::{self, Calls, Cells};
use crate::floatfacts::{self, Finite, Format, Operation, Rule};
use crate::memory::{MemRef, Unit};
use crate::ranges::{self, Interval};

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
    memory: &Cells,
    scoped: &IndexMap<ValueId, Interval>,
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
        let interval = scoped.get(&base)?;
        let known = scoped.iter().map(|(value, interval)| (*value, interval.clone())).collect::<BTreeMap<_, _>>();
        if ranges::covering(&reference, &known).base.is_some() {
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
    let mut values = Vec::new();
    for offset in offsets {
        let cell = MemRef {
            base: None,
            scale: 0,
            disp: reference.disp.checked_add(i64::try_from(offset).ok()?)?,
            ..reference.clone()
        };
        let bits = consts::_cell(memory, &cell)?;
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
    let memory = floatfacts::cells(unit, &Calls::default());
    // What the counted loops bound is the manager's: it is never solved here (no program in the corpus reached a solve
    // here).
    let scoped = unit.bounds.ok_or("float bounds without the manager's bounds of the body")?;
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
    let (empty_cells, empty_scope) = (Cells::default(), IndexMap::default());
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
                let here = memory.get(&inst).map(|here| &**here).unwrap_or(&empty_cells);
                _memory(unit, inst, rule.inputs[0], here, scoped.at(cfg::id(block)).unwrap_or(&empty_scope))
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
