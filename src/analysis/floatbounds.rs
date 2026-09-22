//! Exception-free floating work over bounded, not necessarily known, integers.
//!
//! Port of `qbopt/analysis/floatbounds.py`.  Python's `id(op)` is the
//! operation's [`OpOccurrence`] in the analysed body.

#![allow(dead_code)] // its consumers are not yet ported

use std::rc::Rc;
use std::collections::{BTreeMap, BTreeSet};

use crate::support::hash::IndexMap;
use num_bigint::BigInt;

use super::consts::{self, Cells};
use super::floatfacts::{self, Finite};
use super::occurrence::{OpOccurrence, operations};
use super::ranges::{self, Interval};
use crate::model::floating::{Format, Precision, Semantics};
use crate::model::mir::{self, Arg, Held, Kind, MemRef, MirBody, Op, Value};

pub(crate) type Bounds = (BigInt, BigInt);

static _SIGNED: [(Format, u32); 3] = [(Format::Signed16, 16), (Format::Signed32, 32), (Format::Signed64, 64)];
static _UNSIGNED: [(Format, u32); 1] = [(Format::Unsigned64, 64)];

fn _get(table: &[(Format, u32)], format: Format) -> Option<u32> {
    table.iter().find(|(one, _)| *one == format).map(|(_, value)| *value)
}

pub(crate) fn evaluated(kind: Kind, rule: &Semantics, inputs: &[Bounds]) -> Option<Bounds> {
    if inputs.len() != rule.inputs.len() {
        return None;
    }
    let zero = BigInt::from(0);
    let absolute = |n: &BigInt| if *n < BigInt::from(0) { -n } else { n.clone() };
    let result = match (kind, inputs) {
        (Kind::Fload | Kind::Fstore, [value]) => value.clone(),
        (Kind::Fneg, [(low, high)]) => (-high, -low),
        (Kind::Fabs, [(low, high)]) => (
            if *low <= zero && zero <= *high {
                zero.clone()
            } else {
                absolute(low).min(absolute(high))
            },
            absolute(low).max(absolute(high)),
        ),
        (Kind::Fadd, [(low, high), (other_low, other_high)]) => (low + other_low, high + other_high),
        (Kind::Fsub, [(low, high), (other_low, other_high)]) => (low - other_high, high - other_low),
        (Kind::Fmul, [(low, high), (other_low, other_high)]) => {
            let products = [low * other_low, low * other_high, high * other_low, high * other_high];
            (
                products.iter().min().expect("four").clone(),
                products.iter().max().expect("four").clone(),
            )
        }
        _ => return None,
    };
    if let Some(width) = _get(&_SIGNED, rule.result) {
        let limit = BigInt::from(1) << (width - 1);
        return (-&limit <= result.0 && result.0 <= result.1 && result.1 < limit).then_some(result);
    }
    if let Some(width) = _get(&_UNSIGNED, rule.result) {
        let limit = BigInt::from(1) << width;
        return (zero <= result.0 && result.0 <= result.1 && result.1 < limit).then_some(result);
    }
    let precision = match rule.result {
        Format::Binary32 => 24,
        Format::Binary64 => 53,
        Format::Extended80 => {
            if rule.precision == Precision::Dynamic {
                24
            } else {
                64
            }
        }
        _ => return None,
    };
    let limit = BigInt::from(1) << precision;
    (-&limit <= result.0 && result.0 <= result.1 && result.1 <= limit).then_some(result)
}

/// Bound every possible aligned element using bytes proved at this read.
pub(crate) fn _memory(
    arg: &Arg,
    format: Format,
    memory: &Cells,
    scoped: &IndexMap<Value, Interval>,
    definitions: &IndexMap<Value, &Op>,
) -> Option<Bounds> {
    let Arg::Cell(cell) = arg else {
        return None;
    };
    let reference = mir::symbolic_ref(&cell.r#ref);
    let width = match format {
        Format::Binary32 => Some(4),
        Format::Binary64 => Some(8),
        _ => None,
    };
    if Some(reference.width) != width {
        return None;
    }
    let mut offsets = vec![BigInt::from(0)];
    if let Some(base) = reference.base {
        let interval = scoped.get(&base);
        let covered = ranges::covering(&reference, &scoped.iter().map(|(v, i)| (*v, i.clone())).collect::<BTreeMap<_, _>>());
        let Some(interval) = interval.filter(|_| covered.base.is_none()) else {
            return None;
        };
        let mut stride = BigInt::from(1);
        let producer = definitions.get(&base);
        if let Some(producer) = producer {
            if producer.kind == Kind::Shl
                && producer.results == [Arg::Held(Held { value: base, width: reference.base_width })]
                && producer.args.len() == 2
            {
                if let Arg::Const(amount) = &producer.args[1] {
                    if BigInt::from(0) <= amount.n && amount.n < BigInt::from(reference.base_width * 8) {
                        stride = BigInt::from(1) << u32::try_from(&amount.n).expect("a checked shift");
                    }
                }
            }
        }
        let start = -super::induction::floor_div(&-&interval.low, &stride) * &stride;
        offsets = Vec::new();
        let mut offset = start;
        while offset < &interval.high + 1 {
            offsets.push(offset.clone());
            if offsets.len() > 64 {
                return None;
            }
            offset += &stride;
        }
        if offsets.is_empty() {
            return None;
        }
    }
    let addr = reference.addr?;
    if reference.segment.is_some() {
        return None;
    }
    if reference.base.is_none() && addr.base != iced_x86::Register::None {
        return None;
    }
    let mut values = Vec::new();
    for offset in offsets {
        let mut cell = reference.clone().into_owned();
        let mut address = addr;
        address.disp += i64::try_from(offset).ok()?;
        address.base = iced_x86::Register::None;
        cell.addr = Some(address);
        cell.base = None;
        let bits = consts::_cell(memory, &cell);
        let value = bits.and_then(|bits| floatfacts::decoded(&bits.n, format));
        let value = value.filter(|value| value.value.denominator == BigInt::from(1))?;
        values.push(value.value.numerator);
    }
    Some((
        values.iter().min().expect("offsets").clone(),
        values.iter().max().expect("offsets").clone(),
    ))
}

/// Operation identities proven numerically exact; pending checks remain separate.
pub(crate) fn exact(
    body: &Rc<MirBody>,
    constants: &IndexMap<Value, Finite>,
    dgroup: &BTreeSet<i64>,
) -> Result<BTreeSet<OpOccurrence>, String> {
    if !body.blocks.iter().any(|block| block.ops.iter().any(|op| op.floating.is_some())) {
        return Ok(BTreeSet::new());
    }
    let mut safe = BTreeSet::new();
    let mut shadow = MirBody::clone(body);
    for block in &mut shadow.blocks {
        for op in &mut block.ops {
            if op.barrier() || op.kind == Kind::Opaque || (op.kind == Kind::Call && op.stores.is_empty()) {
                op.stores = vec![MemRef::new(None, 0)];
            }
        }
    }
    let shadow = Rc::new(shadow);
    let memory = floatfacts::cells(&shadow, dgroup, &IndexMap::default());
    let scoped = ranges::bounded(body)?;
    let definitions = body
        .blocks
        .iter()
        .flat_map(|block| block.ops.iter())
        .flat_map(|op| op.defines.iter().map(move |value| (*value, op)))
        .collect::<IndexMap<_, _>>();
    let mut values = IndexMap::<Value, Bounds>::default();
    let mut pending = operations(body)
        .filter(|(_, _, op)| op.floating.is_some() && !op.barrier() && !matches!(op.kind, Kind::Call | Kind::Opaque))
        .map(|(occurrence, block, op)| (block.at, occurrence.operation_index(), op, occurrence))
        .collect::<Vec<_>>();
    let phis = body
        .blocks
        .iter()
        .flat_map(|block| block.phis.iter())
        .filter(|phi| !phi.result.flags)
        .collect::<Vec<_>>();
    let empty_cells = Cells::default();
    let empty_scope = IndexMap::default();
    let mut changed = true;
    while changed {
        changed = false;
        for phi in &phis {
            if values.contains_key(&phi.result)
                || phi.incoming.is_empty()
                || !phi.incoming.values().all(|value| values.contains_key(value))
            {
                continue;
            }
            let bounds = phi.incoming.values().map(|value| values[value].clone()).collect::<Vec<_>>();
            let low = bounds.iter().map(|(low, _)| low).min().expect("incoming").clone();
            let high = bounds.iter().map(|(_, high)| high).max().expect("incoming").clone();
            values.insert(phi.result, (low, high));
            changed = true;
        }
        let mut remaining = Vec::new();
        for (at, index, op, occurrence) in pending {
            let rule = op.floating.as_ref().expect("pending operations have a rule");
            let mut inputs = Vec::new();
            for (arg, format) in op.args.iter().zip(rule.inputs.iter()) {
                let bounds = if let Some(width) = _get(&_SIGNED, *format) {
                    let limit = BigInt::from(1) << (width - 1);
                    Some((-&limit, limit - 1))
                } else if let Arg::Held(held) = arg {
                    let mut bounds = values.get(&held.value).cloned();
                    if let Some(fact) = constants.get(&held.value) {
                        if fact.value.denominator == BigInt::from(1) {
                            bounds = Some((fact.value.numerator.clone(), fact.value.numerator.clone()));
                        }
                    }
                    bounds
                } else {
                    _memory(
                        arg,
                        *format,
                        memory.get(&(at, index)).unwrap_or(&empty_cells),
                        scoped.get(&at).unwrap_or(&empty_scope),
                        &definitions,
                    )
                };
                let Some(bounds) = bounds else {
                    break;
                };
                inputs.push(bounds);
            }
            let Some(result) = evaluated(op.kind, rule, &inputs) else {
                remaining.push((at, index, op, occurrence));
                continue;
            };
            safe.insert(occurrence);
            for arg in &op.results {
                if let Arg::Held(held) = arg {
                    values.insert(held.value, result.clone());
                }
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
