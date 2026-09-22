//! Port of `qbopt/backend/division.py`: select signed constant division
//! without exposing machine choices to MIR.

use crate::backend::arithmetic;
use crate::backend::cpu::{self as targets, ProfileOrName};
use crate::backend::timing;
use crate::model::ir;

/// Positive-divisor form of LLVM's SignedDivisionByConstantInfo algorithm.
pub fn magic(divisor: i64, bits: i64) -> Result<(i64, i64), String> {
    if !(1 < divisor && divisor < 1 << (bits - 1)) {
        return Err("positive signed divisor greater than one required".to_owned());
    }
    let half = 1i64 << (bits - 1);
    let boundary = half - 1 - half.rem_euclid(divisor);
    let (mut first, mut first_rem) = (half.div_euclid(boundary), half.rem_euclid(boundary));
    let (mut second, mut second_rem) = (half.div_euclid(divisor), half.rem_euclid(divisor));
    let mut exponent = bits - 1;
    loop {
        exponent += 1;
        first *= 2;
        first_rem *= 2;
        if first_rem >= boundary {
            first += 1;
            first_rem -= boundary;
        }
        second *= 2;
        second_rem *= 2;
        if second_rem >= divisor {
            second += 1;
            second_rem -= divisor;
        }
        let delta = divisor - second_rem;
        if first > delta || (first == delta && first_rem != 0) {
            break;
        }
    }
    let multiplier = (second + 1 + half).rem_euclid(half * 2) - half;
    Ok((multiplier, exponent - bits))
}

pub fn reciprocal<'a>(
    dividend: ir::Held,
    divisor: i64,
    results: &[ir::Held],
    fresh: &mut dyn FnMut() -> u32,
    cpu: impl Into<ProfileOrName<'a>>,
    remainder: bool,
) -> Result<Option<Vec<ir::Semantics>>, String> {
    let cpu = targets::profile(cpu)?;
    let width = dividend.width;
    if width != 4 || !(1 < divisor && divisor < 1 << 31) {
        return Ok(None);
    }
    let multiply_cost = timing::signed_multiply(cpu, i64::from(width), true)?;
    let divide_cost = timing::signed_divide(cpu, i64::from(width))?;
    let (Some(multiply_cost), Some(divide_cost)) = (multiply_cost, divide_cost) else {
        return Ok(None);
    };
    let (multiplier, shift) = magic(divisor, 32)?;
    let chain = arithmetic::scale(divisor, cpu)?;
    let cost = |name: &str| arithmetic::cost(cpu, name);
    // `if chain`: None and the empty tuple are both false.
    let chained = chain.as_ref().filter(|chain| !chain.is_empty());
    let mut reconstruction = match chained {
        Some(chain) => {
            let mut total = 0;
            for (name, _) in chain {
                total += cost(if *name == "shl" { "shift_ri" } else { "alu_rr" })?;
            }
            total
        }
        None => {
            timing::signed_multiply(cpu, i64::from(width), false)?
                .ok_or_else(|| "'NoneType' object has no attribute 'maximum'".to_owned())?
                .maximum
        }
    };
    if !remainder {
        reconstruction = 0;
    }
    // Materialize magic, seed multiply, preserve dividend and correction,
    // and seed reconstruction. Allocation may eliminate some of these moves.
    let copies = if remainder { 5 } else { 4 };
    let mut estimate = copies * cost("mov_rr")?
        + multiply_cost.maximum
        + reconstruction
        + (1 + i64::from(shift != 0)) * cost("shift_ri")?
        + (1 + i64::from(remainder) + i64::from(multiplier < 0)) * cost("alu_rr")?;
    if cpu.name == "P5" {
        // Intel 241430-004 section 24.3: one clock per prefix. Every dword
        // operation needs 66h in this 16-bit code segment. Charge the
        // reserved copies too; do not assume prefix decoding overlaps.
        estimate +=
            copies + 4 + i64::from(remainder) + i64::from(multiplier < 0) + i64::from(shift != 0);
        if remainder {
            estimate += match chained {
                Some(chain) => chain.len() as i64,
                None => 1,
            };
        }
    }
    let direct = divide_cost.minimum;
    if estimate >= direct {
        return Ok(None);
    }
    let mut parts = Vec::new();

    let emit = |parts: &mut Vec<ir::Semantics>,
                fresh: &mut dyn FnMut() -> u32,
                operation: ir::Operation,
                name: &str,
                sources: Vec<ir::Loc>,
                into: Option<ir::Held>|
     -> ir::Held {
        let into = into.unwrap_or_else(|| ir::Held {
            value: fresh(),
            width,
        });
        parts.push(ir::Semantics {
            name: Some(name.to_owned()),
            dests: vec![ir::Loc::Held(into)],
            sources,
            ..ir::Semantics::new(operation)
        });
        into
    };
    let imm = |value: i64, width: u32| {
        ir::Loc::Imm(ir::Imm {
            value,
            width,
            address: None,
        })
    };

    let constant = emit(
        &mut parts,
        fresh,
        ir::Operation::Move,
        "mov",
        vec![imm(multiplier, width)],
        None,
    );
    let low = ir::Held {
        value: fresh(),
        width,
    };
    let mut high = ir::Held {
        value: fresh(),
        width,
    };
    parts.push(ir::Semantics {
        name: Some("imul".to_owned()),
        dests: vec![ir::Loc::Held(low), ir::Loc::Held(high)],
        sources: vec![ir::Loc::Held(dividend), ir::Loc::Held(constant)],
        ..ir::Semantics::new(ir::Operation::Multiply)
    });
    if multiplier < 0 {
        high = emit(
            &mut parts,
            fresh,
            ir::Operation::Binary,
            "add",
            vec![ir::Loc::Held(high), ir::Loc::Held(dividend)],
            None,
        );
    }
    let sign = emit(
        &mut parts,
        fresh,
        ir::Operation::Binary,
        "shr",
        vec![ir::Loc::Held(high), imm(31, 1)],
        None,
    );
    if shift != 0 {
        high = emit(
            &mut parts,
            fresh,
            ir::Operation::Binary,
            "sar",
            vec![ir::Loc::Held(high), imm(shift, 1)],
            None,
        );
    }
    let quotient = emit(
        &mut parts,
        fresh,
        ir::Operation::Binary,
        "add",
        vec![ir::Loc::Held(high), ir::Loc::Held(sign)],
        Some(results[0]),
    );
    if !remainder {
        return Ok(Some(parts));
    }
    let mut product = quotient;
    if let Some(chain) = chained {
        for (name, amount) in chain {
            let other = if *name == "shl" {
                imm(*amount, 1)
            } else {
                ir::Loc::Held(quotient)
            };
            product = emit(
                &mut parts,
                fresh,
                ir::Operation::Binary,
                name,
                vec![ir::Loc::Held(product), other],
                None,
            );
        }
    } else {
        product = emit(
            &mut parts,
            fresh,
            ir::Operation::Multiply,
            "imul",
            vec![ir::Loc::Held(quotient), imm(divisor, width)],
            None,
        );
    }
    emit(
        &mut parts,
        fresh,
        ir::Operation::Binary,
        "sub",
        vec![ir::Loc::Held(dividend), ir::Loc::Held(product)],
        Some(results[1]),
    );
    Ok(Some(parts))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn signed(value: i64) -> i64 {
        ((value & 0xffffffff) ^ 0x80000000) - 0x80000000
    }

    fn held(one: &ir::Loc) -> u32 {
        match one {
            ir::Loc::Held(held) => held.value,
            other => panic!("{other:?}"),
        }
    }

    /// LNGMXX's q+r needs both answers, including negative truncation and INT_MIN.
    #[test]
    fn test_reciprocal_preserves_signed_quotient_and_remainder() {
        for divisor in [3, 7, 10, 31, 1000, 2147483647] {
            let source = ir::Held { value: 1, width: 4 };
            let results = [
                ir::Held { value: 2, width: 4 },
                ir::Held { value: 3, width: 4 },
            ];
            let mut count = 4..;
            let mut fresh = || count.next().unwrap();
            let parts = reciprocal(source, divisor, &results, &mut fresh, "P5", true)
                .unwrap()
                .expect("a reciprocal");
            for number in [
                -2147483648,
                -divisor,
                -divisor + 1,
                -1,
                0,
                1,
                divisor - 1,
                divisor,
                2147483647,
            ] {
                let mut values: HashMap<u32, i64> = HashMap::from([(1, number)]);
                for part in &parts {
                    let args: Vec<i64> = part
                        .sources
                        .iter()
                        .map(|arg| match arg {
                            ir::Loc::Held(held) => values[&held.value],
                            ir::Loc::Imm(imm) => imm.value,
                            other => panic!("{other:?}"),
                        })
                        .collect();
                    let answer = match part.name.as_deref().unwrap() {
                        "mov" => args[0],
                        "imul" if part.dests.len() == 2 => {
                            let value = args[0] * args[1];
                            values.insert(held(&part.dests[0]), signed(value));
                            values.insert(held(&part.dests[1]), signed(value >> 32));
                            continue;
                        }
                        "imul" => args[0] * args[1],
                        "add" => args[0] + args[1],
                        "sub" => args[0] - args[1],
                        "shl" => args[0] << args[1],
                        "shr" => (args[0] & 0xffffffff) >> args[1],
                        "sar" => args[0] >> args[1],
                        other => panic!("{other}"),
                    };
                    values.insert(held(&part.dests[0]), signed(answer));
                }
                let quotient = number.abs() / divisor * if number < 0 { -1 } else { 1 };
                assert_eq!(values[&2], quotient, "{number} / {divisor}");
                assert_eq!(
                    values[&3],
                    number - quotient * divisor,
                    "{number} % {divisor}"
                );
            }
        }
    }

    #[test]
    fn test_slow_multiply_keeps_division() {
        let mut count = 4..;
        let mut fresh = || count.next().unwrap();
        let results = [
            ir::Held { value: 2, width: 4 },
            ir::Held { value: 3, width: 4 },
        ];
        assert_eq!(
            reciprocal(
                ir::Held { value: 1, width: 4 },
                7,
                &results,
                &mut fresh,
                "386",
                true
            ),
            Ok(None)
        );
    }
}
