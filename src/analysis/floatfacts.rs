//! Exact finite numeric facts; strict effects remain a separate obligation.
//!
//! Port of `qbopt/analysis/floatfacts.py`.

// ---- early port (agent E) ----
#![allow(dead_code)]

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};

use crate::model::floating::{Format, Precision, Semantics};
use crate::model::mir::Kind;

/// Python's `Fraction`.
pub(crate) type Fraction = BigRational;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Finite {
    pub value: Fraction,
    pub negative_zero: bool,
}

impl Finite {
    pub(crate) fn new(value: Fraction) -> Self {
        Self { value, negative_zero: false }
    }

    pub(crate) fn with_zero(value: Fraction, negative_zero: bool) -> Self {
        Self { value, negative_zero }
    }

    pub(crate) fn negative(&self) -> bool {
        self.value.is_negative() || self.negative_zero
    }
}

#[allow(non_snake_case)]
fn _BINARY(format: Format) -> Option<(u64, u64, i64)> {
    match format {
        Format::Binary32 => Some((24, 8, 127)),
        Format::Binary64 => Some((53, 11, 1023)),
        _ => None,
    }
}

#[allow(non_snake_case)]
fn _INTEGER(format: Format) -> Option<u64> {
    match format {
        Format::Signed16 => Some(16),
        Format::Signed32 => Some(32),
        Format::Signed64 => Some(64),
        _ => None,
    }
}

#[allow(non_snake_case)]
fn _UNSIGNED(format: Format) -> Option<u64> {
    match format {
        Format::Unsigned64 => Some(64),
        _ => None,
    }
}

fn shl(bits: u64) -> BigInt {
    BigInt::one() << bits
}

fn integer(n: BigInt) -> Fraction {
    Fraction::from_integer(n)
}

pub(crate) fn decoded(bits: &BigInt, format: Format) -> Option<Finite> {
    if let Some(width) = _INTEGER(format) {
        if !(&BigInt::zero() <= bits && bits < &shl(width)) {
            return None;
        }
        let sign = shl(width - 1);
        return Some(Finite::new(integer((bits ^ &sign) - &sign)));
    }
    if let Some(width) = _UNSIGNED(format) {
        return (&BigInt::zero() <= bits && bits < &shl(width)).then(|| Finite::new(integer(bits.clone())));
    }
    let (precision, exponent_bits, bias) = _BINARY(format)?;
    if !(&BigInt::zero() <= bits && bits < &shl(precision + exponent_bits)) {
        return None;
    }
    let fraction: BigInt = bits & (shl(precision - 1) - 1);
    let exponent: BigInt = (bits >> (precision - 1)) & (shl(exponent_bits) - 1);
    let negative = !(bits >> (precision + exponent_bits - 1)).is_zero();
    if exponent == shl(exponent_bits) - 1 || (exponent.is_zero() && !fraction.is_zero()) {
        return None;
    }
    if exponent.is_zero() {
        return Some(Finite::with_zero(Fraction::zero(), negative));
    }
    let significand = shl(precision - 1) | fraction;
    let shift = i64::try_from(&exponent).expect("exponent fits") - bias - precision as i64 + 1;
    let value = Fraction::new(
        significand << shift.max(0) as u64,
        shl((-shift).max(0) as u64),
    );
    Some(Finite::new(if negative { -value } else { value }))
}

fn _fits(value: &Fraction, precision: u64, minimum: i64, maximum: i64) -> bool {
    if value.is_zero() {
        return true;
    }
    let (numerator, denominator) = (value.numer().abs(), value.denom().clone());
    if !(&denominator & (&denominator - BigInt::one())).is_zero() {
        return false;
    }
    let trailing = numerator.trailing_zeros().expect("nonzero numerator");
    let exponent = numerator.bits() as i64 - denominator.bits() as i64;
    numerator.bits() - trailing <= precision && minimum <= exponent && exponent <= maximum
}

pub(crate) fn evaluated(kind: Kind, rule: &Semantics, inputs: &[Finite]) -> Option<Finite> {
    if inputs.len() != rule.inputs.len() {
        return None;
    }
    let result = match (kind, inputs) {
        (Kind::Fload | Kind::Fstore, [value]) => value.clone(),
        (Kind::Fneg, [value]) => Finite::with_zero(
            -value.value.clone(),
            if value.value.is_zero() { !value.negative_zero } else { false },
        ),
        (Kind::Fabs, [value]) => Finite::new(value.value.abs()),
        (Kind::Fsqrt, [value]) => {
            if value.value.is_negative() {
                return None;
            }
            let numerator = value.value.numer().sqrt();
            let denominator = value.value.denom().sqrt();
            if &(&numerator * &numerator) != value.value.numer()
                || &(&denominator * &denominator) != value.value.denom()
            {
                return None;
            }
            Finite::with_zero(Fraction::new(numerator, denominator), value.negative_zero)
        }
        (Kind::Fadd | Kind::Fsub, [left, right]) => {
            let right_value = if kind == Kind::Fadd {
                right.value.clone()
            } else {
                -right.value.clone()
            };
            let value = &left.value + right_value;
            if value.is_zero() {
                let right_negative = right.negative() ^ (kind == Kind::Fsub);
                if !left.value.is_zero() || !right.value.is_zero() || left.negative() != right_negative {
                    return None; // cancellation's zero sign depends on rounding
                }
                Finite::with_zero(value, left.negative())
            } else {
                Finite::new(value)
            }
        }
        (Kind::Fmul | Kind::Fdiv, [left, right]) => {
            if kind == Kind::Fdiv && right.value.is_zero() {
                return None;
            }
            let value = if kind == Kind::Fmul {
                &left.value * &right.value
            } else {
                &left.value / &right.value
            };
            let negative_zero = value.is_zero() && left.negative() != right.negative();
            Finite::with_zero(value, negative_zero)
        }
        _ => return None,
    };
    if let Some(width) = _INTEGER(rule.result) {
        let low = integer(-shl(width - 1));
        let high = integer(shl(width - 1));
        return (result.value.is_integer() && low <= result.value && result.value < high).then_some(result);
    }
    if let Some(width) = _UNSIGNED(rule.result) {
        return (result.value.is_integer() && !result.value.is_negative() && result.value < integer(shl(width)))
            .then_some(result);
    }
    if let Some((precision, _, bias)) = _BINARY(rule.result) {
        return _fits(&result.value, precision, 1 - bias, bias).then_some(result);
    }
    if rule.result == Format::Extended80 {
        let precision = if rule.precision == Precision::Dynamic { 24 } else { 64 };
        return _fits(&result.value, precision, -16382, 16383).then_some(result);
    }
    None
}

pub(crate) fn encoded(value: &Finite, format: Format) -> Option<BigInt> {
    let (precision, exponent_bits, bias) = _BINARY(format)?;
    if !_fits(&value.value, precision, 1 - bias, bias) {
        return None;
    }
    let sign = BigInt::from(u8::from(value.negative())) << (precision + exponent_bits - 1);
    if value.value.is_zero() {
        return Some(sign);
    }
    let magnitude = value.value.abs();
    let exponent = magnitude.numer().bits() as i64 - magnitude.denom().bits() as i64;
    let shift = precision as i64 - 1 - exponent;
    let significand = magnitude * Fraction::new(shl(shift.max(0) as u64), shl((-shift).max(0) as u64));
    Some(
        sign | (BigInt::from(exponent + bias) << (precision - 1))
            | (significand.to_integer() - shl(precision - 1)),
    )
}

fn _inputs(
    op: &crate::model::mir::Op,
    integers: &std::collections::BTreeMap<crate::model::mir::Value, crate::analysis::consts::Known>,
    memory: &crate::analysis::consts::Cells,
    facts: &indexmap::IndexMap<crate::model::mir::Value, Finite>,
) -> Option<Vec<Finite>> {
    use crate::analysis::consts;
    use crate::model::mir::Arg;

    let floating = op.floating.as_ref()?;
    if op.args.len() != floating.inputs.len() {
        return None;
    }
    let mut inputs = Vec::new();
    for (arg, format) in op.args.iter().zip(floating.inputs.iter()) {
        let fact = match arg {
            Arg::Held(held) if held.width == 10 => facts.get(&held.value).cloned(),
            _ => {
                // Python's `consts._operand(op, arg, integers, memory)`, whose cell arm reads memory.
                let bits = match arg {
                    Arg::Cell(cell) => consts::_cell(memory, &consts::_addressed(&cell.r#ref, integers)),
                    _ => consts::_operand(op, arg, integers),
                };
                bits.and_then(|bits| decoded(&bits.n, *format))
            }
        };
        inputs.push(fact?);
    }
    Some(inputs)
}

/// Numeric facts, optionally given independently established entry bytes.
pub(crate) fn known(
    body: &crate::model::mir::MirBody,
    dgroup: &std::collections::BTreeSet<i64>,
    calls: &indexmap::IndexMap<i64, String>,
    initial: Option<&indexmap::IndexMap<crate::objectfile::module::Addr, BigInt>>,
) -> Result<indexmap::IndexMap<crate::model::mir::Value, Finite>, String> {
    Ok(_analyzed(body, dgroup, calls, initial)?.0)
}

#[allow(clippy::type_complexity)]
fn _analyzed(
    body: &crate::model::mir::MirBody,
    dgroup: &std::collections::BTreeSet<i64>,
    calls: &indexmap::IndexMap<i64, String>,
    initial: Option<&indexmap::IndexMap<crate::objectfile::module::Addr, BigInt>>,
) -> Result<
    (
        indexmap::IndexMap<crate::model::mir::Value, Finite>,
        indexmap::IndexMap<(i64, usize), crate::analysis::consts::Cells>,
    ),
    String,
> {
    use crate::analysis::consts::{self, Cells, Known};
    use crate::model::mir::{Arg, Const, Op};

    let seed = initial.map(|initial| {
        initial
            .iter()
            .map(|(addr, byte)| ((*addr, 1), Known::new(byte.clone(), 1)))
            .collect::<Cells>()
    });
    // Python passes `dgroup, calls, initial=seed`; the Rust `consts::known`
    // is still the value-only slice.
    let integers = consts::known(body);
    let mut facts = indexmap::IndexMap::<crate::model::mir::Value, Finite>::new();
    let mut memory = indexmap::IndexMap::new();
    let mut changed = true;
    while changed {
        changed = false;
        let stored = |op: &Op| -> Op {
            let (Kind::Fstore, Some(floating), [Arg::Held(held)]) =
                (op.kind, op.floating.as_ref(), op.args.as_slice())
            else {
                return op.clone();
            };
            let Some(fact) = facts.get(&held.value) else {
                return op.clone();
            };
            let result = evaluated(op.kind, floating, &[fact.clone()]);
            let bits = result.as_ref().and_then(|result| encoded(result, floating.result));
            let Some(bits) = bits.filter(|_| op.stores.len() == 1) else {
                return op.clone();
            };
            let mut store = op.clone();
            store.kind = Kind::Store;
            store.args = vec![Arg::Const(Const::new(bits, op.stores[0].width))];
            store.uses = Vec::new();
            store
        };
        let mut shadow = body.clone();
        for block in &mut shadow.blocks {
            block.ops = block.ops.iter().map(stored).collect();
        }
        memory = consts::cells(&shadow, dgroup, calls, Some(&integers), seed.as_ref(), None, None, None)
            .map_err(|error| format!("{error:?}"))?;
        let empty = Cells::new();
        for block in &body.blocks {
            for (index, op) in block.ops.iter().enumerate() {
                let inputs = _inputs(op, &integers, memory.get(&(block.at, index)).unwrap_or(&empty), &facts);
                if let Some(inputs) = inputs {
                    let floating = op.floating.as_ref().expect("inputs need floating");
                    if let Some(result) = evaluated(op.kind, floating, &inputs) {
                        for target in &op.results {
                            if let Arg::Held(target) = target {
                                if target.width == 10 && !facts.contains_key(&target.value) {
                                    facts.insert(target.value, result.clone());
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok((facts, memory))
}
