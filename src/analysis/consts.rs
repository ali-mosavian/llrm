//! Value-only constant facts for source-neutral MIR.
//!
//! Direct port of the pure, no-memory paths in `qbopt.analysis.consts`:
//! `Known`, `masked`, `_read`, `_operand`, `_defined`, `_result`, and
//! `_carry`, `_solved`, and `known`.  Cells, aliases, calls, and division remain
//! separate ports.  Python's `FIXED_MUL` and `FIXED_DIV` have no Rust
//! [`Kind`] counterpart; Python `_result` does not evaluate either, so this
//! slice deliberately defers them rather than inventing semantics.

use std::collections::BTreeMap;
use std::fmt;

use num_bigint::BigInt;

use crate::model::mir::{self, Arg, Kind, Op, Value};

/// The low `width` bytes of a value are `n`; nothing is known above them.
///
/// Direct port of `qbopt.analysis.consts:Known`.
#[derive(Clone, Eq, Hash, PartialEq)]
pub(crate) struct Known {
    pub n: BigInt,
    pub width: u32,
}

impl Known {
    pub(crate) fn new(n: impl Into<BigInt>, width: u32) -> Self {
        Self { n: n.into(), width }
    }
}

impl fmt::Debug for Known {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:#x}:{}", self.n, self.width)
    }
}

/// Python's `masked(n, width)`.
pub(crate) fn masked(n: &BigInt, width: u32) -> BigInt {
    n & mask(width)
}

fn mask(width: u32) -> BigInt {
    (BigInt::from(1_u8) << (width * 8)) - 1
}

/// Python's `_read`: demand no more bytes than a fact establishes.
fn _read(fact: Option<&Known>, width: u32) -> Option<Known> {
    let fact = fact?;
    (fact.width >= width).then(|| Known::new(masked(&fact.n, width), width))
}

/// Python's value-only `_operand` path.
///
/// `Arg::Cell` deliberately remains unknown here.  Its Python counterpart
/// delegates that case to the memory lattice, which is outside this port.
fn _operand(_op: &Op, argument: &Arg, known: &BTreeMap<Value, Known>) -> Option<Known> {
    match argument {
        Arg::Const(constant) => Some(Known::new(
            masked(&constant.n, constant.width),
            constant.width,
        )),
        Arg::Held(held) => _read(known.get(&held.value), held.width),
        Arg::Symbol(_) | Arg::FrameAddress(_) | Arg::Cell(_) | Arg::Opaque(_) => None,
    }
}

/// Python's `_defined`: the first non-flag result this operation defines.
pub(super) fn _defined(op: &Op) -> Option<Value> {
    let real = op
        .defines
        .iter()
        .copied()
        .filter(|value| !value.flags)
        .collect::<Vec<_>>();
    if let [value] = real.as_slice() {
        return Some(*value);
    }
    let first = op.results.iter().find_map(|result| match result {
        Arg::Held(held) => Some(held.value),
        _ => None,
    })?;
    real.contains(&first).then_some(first)
}

fn _width(argument: &Arg) -> Option<u32> {
    match argument {
        Arg::Held(held) => Some(held.width),
        Arg::Const(constant) => Some(constant.width),
        Arg::Symbol(_) | Arg::FrameAddress(_) | Arg::Cell(_) | Arg::Opaque(_) => None,
    }
}

fn _signed(number: &BigInt, width: u32) -> BigInt {
    let sign = BigInt::from(1_u8) << (width * 8 - 1);
    (number ^ &sign) - sign
}

fn _arithmetic(kind: Kind, first: &BigInt, second: &BigInt) -> Option<BigInt> {
    let count = || {
        (second & BigInt::from(31_u8))
            .to_u32_digits()
            .1
            .first()
            .copied()
            .unwrap_or(0)
    };
    match kind {
        Kind::Add => Some(first + second),
        Kind::Sub => Some(first - second),
        Kind::And => Some(first & second),
        Kind::Or => Some(first | second),
        Kind::Xor => Some(first ^ second),
        // This is Python's `ARITH[SHR]` fallback.  Well-shaped shifts take
        // the width-sensitive path in `_result` before reaching this case.
        Kind::Shl => Some(first << count()),
        Kind::Shr => Some((first & BigInt::from(0xffff_ffff_u64)) >> count()),
        Kind::Mul => Some(first * second),
        _ => None,
    }
}

fn _unary(kind: Kind, operand: &BigInt) -> Option<BigInt> {
    match kind {
        Kind::Neg => Some(-operand),
        Kind::Not => Some(!operand),
        _ => None,
    }
}

fn _shift_count(number: &BigInt, width: u32) -> u32 {
    let masked = number & BigInt::from(width * 8 - 1);
    masked.to_u32_digits().1.first().copied().unwrap_or(0)
}

/// Python's value-only `_result` path.
///
/// Carries are deliberately separate one-bit facts, just as Python keeps
/// them apart from complete condition-code values.
pub(super) fn _result(
    op: &Op,
    known: &BTreeMap<Value, Known>,
    carries: Option<&BTreeMap<Value, u8>>,
) -> Option<Known> {
    _defined(op)?;
    if op.kind == Kind::Extract && op.args.len() == 2 && op.results.len() == 1 {
        let fact = _operand(op, &op.args[0], known)?;
        let offset = match &op.args[1] {
            Arg::Const(constant) if constant.n >= BigInt::from(0_u8) => &constant.n,
            _ => return None,
        };
        let width = _width(&op.results[0])?;
        let required = offset + BigInt::from(width * 8);
        if BigInt::from(fact.width * 8) < required {
            return None;
        }
        let shift = offset.to_u32_digits().1.first().copied().unwrap_or(0);
        return Some(Known::new(masked(&(&fact.n >> shift), width), width));
    }
    if matches!(op.kind, Kind::Xor | Kind::Sub)
        && op.args.len() == 2
        && matches!((&op.args[0], &op.args[1]), (Arg::Held(one), Arg::Held(other)) if one == other)
    {
        return _width(&op.args[0]).map(|width| Known::new(0, width));
    }

    let parts = op
        .args
        .iter()
        .map(|argument| _operand(op, argument, known))
        .collect::<Option<Vec<_>>>()?;
    let first = parts.first()?;

    if matches!(op.kind, Kind::SignExtend | Kind::ZeroExtend)
        && parts.len() == 1
        && op.results.len() == 1
    {
        let source = &op.args[0];
        let result = match &op.results[0] {
            Arg::Held(result) => result,
            _ => return None,
        };
        let source_width = _width(source)?;
        if !(matches!(source, Arg::Held(_) | Arg::Const(_))
            && 0 < source_width
            && source_width < result.width
            && result.width <= 8
            && first.width >= source_width)
        {
            return None;
        }
        let number = masked(&first.n, source_width);
        let number = if op.kind == Kind::SignExtend {
            _signed(&number, source_width)
        } else {
            number
        };
        return Some(Known::new(masked(&number, result.width), result.width));
    }
    if op.kind == Kind::Concat && parts.len() == 2 && op.results.len() == 1 {
        let high_width = _width(&op.args[0])?;
        let low_width = _width(&op.args[1])?;
        let result_width = _width(&op.results[0])?;
        let width = high_width + low_width;
        if result_width != width || parts[0].width < high_width || parts[1].width < low_width {
            return None;
        }
        let number =
            (masked(&parts[0].n, high_width) << (low_width * 8)) | masked(&parts[1].n, low_width);
        return Some(Known::new(number, width));
    }
    if matches!(op.kind, Kind::Shl | Kind::Shr) && parts.len() == 2 && op.results.len() == 1 {
        let source = match &op.args[0] {
            Arg::Held(source) => source.width,
            Arg::Const(source) => source.width,
            _ => return None,
        };
        let result = match &op.results[0] {
            Arg::Held(result) => result,
            _ => return None,
        };
        if source != result.width || parts[0].width < source {
            return None;
        }
        let number = masked(&parts[0].n, result.width);
        let count = _shift_count(&parts[1].n, result.width);
        let shifted = if op.kind == Kind::Shl {
            number << count
        } else {
            number >> count
        };
        return Some(Known::new(masked(&shifted, result.width), result.width));
    }

    let width = parts.iter().map(|part| part.width).min()?;
    if op.kind == Kind::Smulhi && parts.len() == 2 && op.results.len() == 1 {
        let result = match &op.results[0] {
            Arg::Held(result) => result,
            _ => return None,
        };
        if !(matches!(result.width, 2 | 4)
            && op.args.iter().all(|argument| {
                matches!(argument, Arg::Held(held) if held.width == result.width)
                    || matches!(argument, Arg::Const(constant) if constant.width == result.width)
            })
            && width >= result.width)
        {
            return None;
        }
        let first = _signed(&masked(&parts[0].n, result.width), result.width);
        let second = _signed(&masked(&parts[1].n, result.width), result.width);
        return Some(Known::new(
            masked(&((first * second) >> (result.width * 8)), result.width),
            result.width,
        ));
    }
    if op.kind == Kind::AddCarry && parts.len() == 2 {
        let flags = op
            .uses
            .iter()
            .copied()
            .filter(|value| value.flags)
            .collect::<Vec<_>>();
        if let [flags] = flags.as_slice() {
            if let Some(carry) = carries.and_then(|facts| facts.get(flags)) {
                return Some(Known::new(
                    masked(&(&parts[0].n + &parts[1].n + BigInt::from(*carry)), width),
                    width,
                ));
            }
        }
        return None;
    }
    if parts.len() == 1 {
        if let Some((_, Arg::Const(step))) = mir::stepping(op) {
            return Some(Known::new(masked(&(&first.n + &step.n), width), width));
        }
    }
    if matches!(op.kind, Kind::Copy | Kind::Load) && parts.len() == 1 {
        return Some(Known::new(masked(&first.n, width), width));
    }
    if parts.len() == 2 {
        if let Some(number) = _arithmetic(op.kind, &parts[0].n, &parts[1].n) {
            return Some(Known::new(masked(&number, width), width));
        }
    }
    if parts.len() == 1 {
        if let Some(number) = _unary(op.kind, &first.n) {
            return Some(Known::new(masked(&number, width), width));
        }
    }
    None
}

/// Python's value-only `_carry` path for an `ADD` result.
fn _carry(op: &Op, facts: &BTreeMap<Value, Known>) -> Option<u8> {
    if op.kind != Kind::Add || op.args.len() != 2 || op.results.len() != 1 {
        return None;
    }
    let width = match &op.results[0] {
        Arg::Held(result) => result.width,
        _ => return None,
    };
    let operands = op
        .args
        .iter()
        .map(|argument| _operand(op, argument, facts))
        .collect::<Option<Vec<_>>>()?;
    if operands.iter().any(|fact| fact.width < width) {
        return None;
    }
    let sum = operands
        .iter()
        .fold(BigInt::from(0_u8), |sum, fact| sum + masked(&fact.n, width));
    Some(u8::from(sum >= (BigInt::from(1_u8) << (width * 8))))
}

/// Python's no-memory `_solved(body, None, None, ...)`.
///
/// This is deliberately the first, optimistic walk from the Python source:
/// acyclic facts and carry bits are collected before `constant_cycles`
/// resolves the remaining phi/operation graph.  No value changes after this
/// walk; the cycle solver distinguishes its pending values from values that
/// are genuinely overdefined.
fn _solved(body: &mir::MirBody) -> BTreeMap<Value, Known> {
    let mut facts = BTreeMap::<Value, Known>::new();
    let mut carries = BTreeMap::<Value, u8>::new();
    let mut changing = true;
    while changing {
        changing = false;
        for block in &body.blocks {
            for phi in &block.phis {
                if facts.contains_key(&phi.result) || phi.incoming.is_empty() {
                    continue;
                }
                let seen = phi
                    .incoming
                    .values()
                    .map(|value| facts.get(value))
                    .collect::<Vec<_>>();
                let Some(first) = seen.first().and_then(|fact| *fact) else {
                    continue;
                };
                if seen.iter().all(|fact| *fact == Some(first)) {
                    facts.insert(phi.result, first.clone());
                    changing = true;
                }
            }
            for op in &block.ops {
                if let Some(carry) = _carry(op, &facts) {
                    for value in &op.defines {
                        if value.flags && !carries.contains_key(value) {
                            carries.insert(*value, carry);
                            changing = true;
                        }
                    }
                }
                let Some(target) = _defined(op) else {
                    continue;
                };
                if facts.contains_key(&target) {
                    continue;
                }
                if let Some(found) = _result(op, &facts, Some(&carries)) {
                    facts.insert(target, found);
                    changing = true;
                }
            }
        }
    }
    super::constant_cycles::propagated(body, facts)
}

/// Every value this body computes that is a number, to a fixed point.
///
/// Direct port of the default, value-only invocation of
/// `qbopt.analysis.consts:known`.  Memory, call, edge, and initial-cell
/// arguments are intentionally outside this slice.
pub(crate) fn known(body: &mir::MirBody) -> BTreeMap<Value, Known> {
    _solved(body)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use num_bigint::BigInt;

    use super::{_carry, _defined, _result, Known, masked};
    use crate::model::mir::{Arg, Const, Held, Kind, Op, Value};

    fn value(id: u32) -> Value {
        Value::new(id, 0)
    }

    fn flags(id: u32) -> Value {
        let mut value = value(id);
        value.flags = true;
        value
    }

    fn operation(
        kind: Kind,
        defines: Vec<Value>,
        uses: Vec<Value>,
        args: Vec<Arg>,
        results: Vec<Arg>,
    ) -> Op {
        let mut op = Op::new(0, None, "", defines, uses);
        op.kind = kind;
        op.args = args;
        op.results = results;
        op
    }

    #[test]
    fn facts_are_masked_to_their_own_width() {
        assert_eq!(masked(&BigInt::from(-1), 2), BigInt::from(0xffff));
        assert_eq!(masked(&BigInt::from(0x1_ffff), 2), BigInt::from(0xffff));
    }

    #[test]
    fn flags_do_not_stop_an_operation_being_folded() {
        let result = value(1);
        assert_eq!(
            _defined(&operation(
                Kind::Decrement,
                vec![flags(2), result],
                vec![],
                vec![],
                vec![]
            )),
            Some(result)
        );
        assert_eq!(
            _defined(&operation(
                Kind::Decrement,
                vec![flags(2)],
                vec![],
                vec![],
                vec![]
            )),
            None
        );
    }

    #[test]
    fn self_cancellation_needs_no_input_fact() {
        for kind in [Kind::Xor, Kind::Sub] {
            for width in [1, 2, 4] {
                let source = value(1);
                let result = value(2);
                let held = Arg::Held(Held {
                    value: source,
                    width,
                });
                let op = operation(
                    kind,
                    vec![result],
                    vec![source],
                    vec![held.clone(), held],
                    vec![Arg::Held(Held {
                        value: result,
                        width,
                    })],
                );
                assert_eq!(
                    _result(&op, &BTreeMap::new(), None),
                    Some(Known::new(0, width))
                );
            }
        }
    }

    #[test]
    fn sign_and_zero_extension_demand_the_whole_source() {
        let source = value(1);
        let result = value(2);
        let facts = BTreeMap::from([(source, Known::new(0x8001, 2))]);
        for (kind, expected) in [
            (Kind::SignExtend, 0xffff_ffff_ffff_8001_u64),
            (Kind::ZeroExtend, 0x8001),
        ] {
            let op = operation(
                kind,
                vec![result],
                vec![source],
                vec![Arg::Held(Held {
                    value: source,
                    width: 2,
                })],
                vec![Arg::Held(Held {
                    value: result,
                    width: 8,
                })],
            );
            assert_eq!(_result(&op, &facts, None), Some(Known::new(expected, 8)));
        }
        let narrow = BTreeMap::from([(source, Known::new(0x8001, 1))]);
        let op = operation(
            Kind::SignExtend,
            vec![result],
            vec![source],
            vec![Arg::Held(Held {
                value: source,
                width: 2,
            })],
            vec![Arg::Held(Held {
                value: result,
                width: 4,
            })],
        );
        assert_eq!(_result(&op, &narrow, None), None);
    }

    #[test]
    fn concat_requires_both_operand_widths() {
        let high = value(1);
        let low = value(2);
        let result = value(3);
        let op = operation(
            Kind::Concat,
            vec![result],
            vec![high, low],
            vec![
                Arg::Held(Held {
                    value: high,
                    width: 2,
                }),
                Arg::Held(Held {
                    value: low,
                    width: 2,
                }),
            ],
            vec![Arg::Held(Held {
                value: result,
                width: 4,
            })],
        );
        let facts = BTreeMap::from([(high, Known::new(4, 2)), (low, Known::new(0, 2))]);
        assert_eq!(_result(&op, &facts, None), Some(Known::new(262_144, 4)));
        let narrow = BTreeMap::from([(high, Known::new(4, 1)), (low, Known::new(0, 2))]);
        assert_eq!(_result(&op, &narrow, None), None);
    }

    #[test]
    fn shifts_mask_the_operand_before_shifting_and_use_the_full_width_count() {
        let source = value(1);
        let result = value(2);
        let word = operation(
            Kind::Shr,
            vec![result],
            vec![source],
            vec![
                Arg::Held(Held {
                    value: source,
                    width: 2,
                }),
                Arg::Const(Const::new(1, 2)),
            ],
            vec![Arg::Held(Held {
                value: result,
                width: 2,
            })],
        );
        assert_eq!(
            _result(
                &word,
                &BTreeMap::from([(source, Known::new(0x1235_8000_u32, 4))]),
                None
            ),
            Some(Known::new(0x4000, 2))
        );
        let wide = operation(
            Kind::Shr,
            vec![result],
            vec![source],
            vec![
                Arg::Held(Held {
                    value: source,
                    width: 8,
                }),
                Arg::Const(Const::new(36, 1)),
            ],
            vec![Arg::Held(Held {
                value: result,
                width: 8,
            })],
        );
        let number = 0xfedc_ba98_7654_3210_u64;
        assert_eq!(
            _result(
                &wide,
                &BTreeMap::from([(source, Known::new(number, 8))]),
                None
            ),
            Some(Known::new(number >> 36, 8))
        );
    }

    #[test]
    fn stepping_and_subtraction_preserve_direction_and_width() {
        let result = value(1);
        for (kind, number, answer) in [(Kind::Increment, -1, 0), (Kind::Decrement, 0, -1)] {
            let op = operation(
                kind,
                vec![result],
                vec![],
                vec![Arg::Const(Const::new(number, 2))],
                vec![Arg::Held(Held {
                    value: result,
                    width: 2,
                })],
            );
            assert_eq!(
                _result(&op, &BTreeMap::new(), None),
                Some(Known::new(masked(&BigInt::from(answer), 2), 2))
            );
        }
        let op = operation(
            Kind::Sub,
            vec![result],
            vec![],
            vec![Arg::Const(Const::new(3, 2)), Arg::Const(Const::new(5, 2))],
            vec![Arg::Held(Held {
                value: result,
                width: 2,
            })],
        );
        assert_eq!(
            _result(&op, &BTreeMap::new(), None),
            Some(Known::new(0xfffe, 2))
        );
    }

    #[test]
    fn extract_demands_all_requested_bits() {
        let source = value(1);
        let result = value(2);
        let op = operation(
            Kind::Extract,
            vec![result],
            vec![source],
            vec![
                Arg::Held(Held {
                    value: source,
                    width: 4,
                }),
                Arg::Const(Const::new(16, 4)),
            ],
            vec![Arg::Held(Held {
                value: result,
                width: 2,
            })],
        );
        assert_eq!(
            _result(
                &op,
                &BTreeMap::from([(source, Known::new(0x1234_5678_u32, 4))]),
                None
            ),
            Some(Known::new(0x1234, 2))
        );
        assert_eq!(
            _result(
                &op,
                &BTreeMap::from([(source, Known::new(0x1234_5678_u32, 2))]),
                None
            ),
            None
        );
    }

    #[test]
    fn add_carry_is_a_separate_one_bit_fact() {
        let low = value(1);
        let condition = flags(2);
        let high = value(3);
        let add = operation(
            Kind::Add,
            vec![low],
            vec![],
            vec![
                Arg::Const(Const::new(0xffff, 2)),
                Arg::Const(Const::new(1, 2)),
            ],
            vec![Arg::Held(Held {
                value: low,
                width: 2,
            })],
        );
        assert_eq!(_carry(&add, &BTreeMap::new()), Some(1));
        let adc = operation(
            Kind::AddCarry,
            vec![high],
            vec![condition],
            vec![Arg::Const(Const::new(2, 2)), Arg::Const(Const::new(3, 2))],
            vec![Arg::Held(Held {
                value: high,
                width: 2,
            })],
        );
        assert_eq!(
            _result(
                &adc,
                &BTreeMap::new(),
                Some(&BTreeMap::from([(condition, 1)]))
            ),
            Some(Known::new(6, 2))
        );
        assert_eq!(_result(&adc, &BTreeMap::new(), None), None);
    }

    #[test]
    fn signed_high_product_is_signed_and_rejects_mixed_widths() {
        let result = value(1);
        for width in [2, 4] {
            let op = operation(
                Kind::Smulhi,
                vec![result],
                vec![],
                vec![
                    Arg::Const(Const::new(-7, width)),
                    Arg::Const(Const::new(11, width)),
                ],
                vec![Arg::Held(Held {
                    value: result,
                    width,
                })],
            );
            let bits = width * 8;
            let product = BigInt::from((-7_i64) * 11);
            let expected = masked(&(&product >> bits), width);
            assert_eq!(
                _result(&op, &BTreeMap::new(), None),
                Some(Known::new(expected, width))
            );
        }
        let mixed = operation(
            Kind::Smulhi,
            vec![result],
            vec![],
            vec![Arg::Const(Const::new(-7, 2)), Arg::Const(Const::new(11, 4))],
            vec![Arg::Held(Held {
                value: result,
                width: 4,
            })],
        );
        assert_eq!(_result(&mixed, &BTreeMap::new(), None), None);
    }
}
