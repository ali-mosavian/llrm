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

/// `quotient` times the divisor, by the shifts, adds and `lea`s of `chain` (`arithmetic::scale`): the product of the
/// step before it is shifted, or added to the quotient, or the quotient plus it scaled, as the address unit makes it.
fn chain_product(parts: &mut Vec<ir::Semantics>, chain: &[(&'static str, i64)], quotient: ir::Held, fresh: &mut dyn FnMut() -> u32) -> ir::Held {
    let width = quotient.width;
    let mut product = quotient;
    for &(name, amount) in chain {
        let into = ir::Held { value: fresh(), width };
        parts.push(if name == "lea" {
            let cell = ir::Mem { base: Some(quotient), index: Some(product), scale: amount, ..ir::Mem::new(None, width) };
            ir::Semantics { name: Some("lea".to_owned()), dests: vec![ir::Loc::Held(into)], sources: vec![ir::Loc::Mem(cell)], ..ir::Semantics::new(ir::Operation::Address) }
        } else {
            let other = if name == "shl" { ir::Loc::Imm(ir::Imm { value: amount, width: 1, address: None }) } else { ir::Loc::Held(quotient) };
            ir::Semantics { name: Some(name.to_owned()), dests: vec![ir::Loc::Held(into)], sources: vec![ir::Loc::Held(product), other], ..ir::Semantics::new(ir::Operation::Binary) }
        });
        product = into;
    }
    product
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
            for (name, count) in chain {
                total += if *name == "shl" { arithmetic::shift(cpu, *count)? } else { cost("alu_rr")? };
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
    // One clock per operand-size prefix (Intel 241430-004 section 24.3) on each dword instruction where the code's own
    // size is not a dword: every one in real mode, none when flat. Charge the reserved copies too.
    if cpu.operand_bytes != 4 {
        let extra = copies + 4 + i64::from(remainder) + i64::from(multiplier < 0) + i64::from(shift != 0);
        estimate += cpu.operations.prefix * extra;
        if remainder {
            estimate += cpu.operations.prefix * match chained {
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
        product = chain_product(&mut parts, chain, quotient, fresh);
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

/// Hacker's Delight `magicu` for 32 bits: the multiplier, whether the product needs the dividend added back
/// (a 33-bit multiplier), and the shift after it. `divisor` is 3 or more and not a power of two.
pub fn unsigned_magic(divisor: u64) -> (u64, bool, i64) {
    let two32: u64 = 1 << 32;
    let top: u64 = 1 << 31;
    let nc = two32 - 1 - (two32 - divisor) % divisor;
    let mut p: i64 = 31;
    let (mut q1, mut r1) = (top / nc, top - (top / nc) * nc);
    let (mut q2, mut r2) = ((top - 1) / divisor, (top - 1) - ((top - 1) / divisor) * divisor);
    let mut add = false;
    loop {
        p += 1;
        if r1 >= nc - r1 {
            q1 = 2 * q1 + 1;
            r1 = 2 * r1 - nc;
        } else {
            q1 *= 2;
            r1 *= 2;
        }
        if r2 + 1 >= divisor - r2 {
            if q2 >= top - 1 {
                add = true;
            }
            q2 = 2 * q2 + 1;
            r2 = 2 * r2 + 1 - divisor;
        } else {
            if q2 >= top {
                add = true;
            }
            q2 *= 2;
            r2 = 2 * r2 + 1;
        }
        let delta = divisor - 1 - r2;
        if !(p < 64 && (q1 < delta || (q1 == delta && r1 == 0))) {
            break;
        }
    }
    ((q2 + 1) & (two32 - 1), add, p - 32)
}

/// An unsigned division of a dword by a constant as a multiply by its reciprocal, where the target's own prices
/// (the audited multiply bounds, its divide, the prefix its code size carries) make that cheaper: `mul`'s high
/// half shifted, with the dividend added back where the multiplier is 33 bits. None keeps `div`.
pub fn unsigned_reciprocal<'a>(
    dividend: ir::Held,
    divisor: i64,
    results: &[ir::Held],
    fresh: &mut dyn FnMut() -> u32,
    cpu: impl Into<ProfileOrName<'a>>,
    remainder: bool,
) -> Result<Option<Vec<ir::Semantics>>, String> {
    let cpu = targets::profile(cpu)?;
    let width = dividend.width;
    if width != 4 || !(2 < divisor && divisor < 1 << 31) || (divisor & (divisor - 1)) == 0 {
        return Ok(None);
    }
    let (Some(multiply), Some(divide)) = (timing::signed_multiply(cpu, 4, true)?, timing::unsigned_divide(cpu, 4)?) else {
        return Ok(None);
    };
    let (multiplier, add, shift) = unsigned_magic(divisor as u64);
    let cost = |name: &str| arithmetic::cost(cpu, name);
    let chain = arithmetic::scale(divisor, cpu)?;
    let chained = chain.as_ref().filter(|chain| !chain.is_empty());
    let reconstruction = if !remainder {
        0
    } else {
        match chained {
            Some(chain) => {
                let mut total = 0;
                for (name, count) in chain {
                    total += if *name == "shl" { arithmetic::shift(cpu, *count)? } else { cost("alu_rr")? };
                }
                total
            }
            None => timing::signed_multiply(cpu, 4, false)?.ok_or_else(|| "no multiply bound".to_owned())?.maximum,
        }
    };
    // The multiplier, the multiply's seed, the dividend kept for the correction and for the remainder.
    let copies = if remainder { 4 } else { 3 };
    let shifts = if add { 2 } else { 1 };
    let alu = if add { 3 } else { 0 } + i64::from(remainder);
    let mut estimate = copies * cost("mov_rr")? + multiply.maximum + reconstruction + shifts * cost("shift_ri")? + alu * cost("alu_rr")?;
    if cpu.operand_bytes != 4 {
        estimate += cpu.operations.prefix * (copies + 1 + shifts + alu + i64::from(remainder) * chained.map_or(1, |chain| chain.len() as i64));
    }
    if estimate >= divide.minimum {
        return Ok(None);
    }
    let mut parts = Vec::new();
    let imm = |value: i64, width: u32| ir::Loc::Imm(ir::Imm { value, width, address: None });
    let mut emit = |parts: &mut Vec<ir::Semantics>, operation: ir::Operation, name: &str, sources: Vec<ir::Loc>, into: Option<ir::Held>, fresh: &mut dyn FnMut() -> u32| -> ir::Held {
        let into = into.unwrap_or_else(|| ir::Held { value: fresh(), width });
        parts.push(ir::Semantics { name: Some(name.to_owned()), dests: vec![ir::Loc::Held(into)], sources, ..ir::Semantics::new(operation) });
        into
    };
    let constant = emit(&mut parts, ir::Operation::Move, "mov", vec![imm(multiplier as i64, width)], None, fresh);
    let low = ir::Held { value: fresh(), width };
    let high = ir::Held { value: fresh(), width };
    parts.push(ir::Semantics {
        name: Some("mul".to_owned()),
        dests: vec![ir::Loc::Held(low), ir::Loc::Held(high)],
        sources: vec![ir::Loc::Held(dividend), ir::Loc::Held(constant)],
        ..ir::Semantics::new(ir::Operation::Multiply)
    });
    let quotient_into = results[0];
    let quotient = if add {
        // ((x - t) >> 1) + t, then shifted the rest of the way.
        let difference = emit(&mut parts, ir::Operation::Binary, "sub", vec![ir::Loc::Held(dividend), ir::Loc::Held(high)], None, fresh);
        let half = emit(&mut parts, ir::Operation::Binary, "shr", vec![ir::Loc::Held(difference), imm(1, 1)], None, fresh);
        let sum = emit(&mut parts, ir::Operation::Binary, "add", vec![ir::Loc::Held(half), ir::Loc::Held(high)], None, fresh);
        if shift > 1 {
            emit(&mut parts, ir::Operation::Binary, "shr", vec![ir::Loc::Held(sum), imm(shift - 1, 1)], Some(quotient_into), fresh)
        } else {
            emit(&mut parts, ir::Operation::Move, "mov", vec![ir::Loc::Held(sum)], Some(quotient_into), fresh)
        }
    } else if shift > 0 {
        emit(&mut parts, ir::Operation::Binary, "shr", vec![ir::Loc::Held(high), imm(shift, 1)], Some(quotient_into), fresh)
    } else {
        emit(&mut parts, ir::Operation::Move, "mov", vec![ir::Loc::Held(high)], Some(quotient_into), fresh)
    };
    if !remainder {
        return Ok(Some(parts));
    }
    let mut product = quotient;
    if let Some(chain) = chained {
        product = chain_product(&mut parts, chain, quotient, fresh);
    } else {
        product = emit(&mut parts, ir::Operation::Multiply, "imul", vec![ir::Loc::Held(quotient), imm(divisor, width)], None, fresh);
    }
    emit(&mut parts, ir::Operation::Binary, "sub", vec![ir::Loc::Held(dividend), ir::Loc::Held(product)], Some(results[1]), fresh);
    Ok(Some(parts))
}

#[cfg(test)]
mod tests {
    use crate::support::hash::HashMap;

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

    /// #789 gave the multiply chains a `lea` step; the remainder's quotient-times-divisor took it for a binary operation
    /// and named it `lea r, r2`, which no instruction is: a flat target's `x % 10` failed to assemble ("Semantics(op=BINARY,
    /// name='lea' ...)") at -O2 on a Pentium.
    #[test]
    fn test_a_remainders_product_makes_its_lea_step_as_an_address() {
        let m32 = crate::backend::cpu::tuned_for(&llrm_x86_m32::M32, "P5", false).unwrap();
        let results = [ir::Held { value: 2, width: 4 }, ir::Held { value: 3, width: 4 }];
        let mut count = 4..;
        let mut fresh = || count.next().unwrap();
        for signed in [true, false] {
            let parts = if signed { reciprocal(ir::Held { value: 1, width: 4 }, 10, &results, &mut fresh, m32, true) } else { unsigned_reciprocal(ir::Held { value: 1, width: 4 }, 10, &results, &mut fresh, m32, true) }
                .unwrap()
                .expect("a reciprocal");
            assert!(!parts.iter().any(|one| one.name.as_deref() == Some("lea") && one.op == ir::Operation::Binary), "signed {signed}: {parts:?}");
            assert!(parts.iter().any(|one| one.name.as_deref() == Some("lea") && one.op == ir::Operation::Address), "signed {signed}: {parts:?}");
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
                let mut values: HashMap<u32, i64> = HashMap::from_iter([(1, number)]);
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

    /// The multipliers Hacker's Delight prints for 32 bits.
    #[test]
    fn test_unsigned_magic_numbers_are_the_known_ones() {
        assert_eq!(unsigned_magic(10), (0xCCCC_CCCD, false, 3));
        assert_eq!(unsigned_magic(3), (0xAAAA_AAAB, false, 1));
        assert_eq!(unsigned_magic(7), (0x2492_4925, true, 3));
    }

    /// Run `parts` on `number` as the machine would, unsigned, and give each value by its number.
    fn run_unsigned(parts: &[ir::Semantics], number: u64) -> HashMap<u32, u64> {
        let mut values: HashMap<u32, u64> = HashMap::from_iter([(1, number)]);
        for part in parts {
            let args: Vec<u64> = part
                .sources
                .iter()
                .map(|arg| match arg {
                    ir::Loc::Held(held) => values[&held.value],
                    ir::Loc::Imm(imm) => imm.value as u64 & 0xffff_ffff,
                    other => panic!("{other:?}"),
                })
                .collect();
            let low = |value: u64| value & 0xffff_ffff;
            let answer = match part.name.as_deref().unwrap() {
                "mov" => args[0],
                "mul" => {
                    let value = args[0] * args[1];
                    values.insert(held(&part.dests[0]), low(value));
                    values.insert(held(&part.dests[1]), value >> 32);
                    continue;
                }
                "imul" => low(args[0] * args[1]),
                "add" => low(args[0] + args[1]),
                "sub" => low(args[0].wrapping_sub(args[1])),
                "shl" => low(args[0] << args[1]),
                "shr" => args[0] >> args[1],
                other => panic!("{other}"),
            };
            values.insert(held(&part.dests[0]), answer);
        }
        values
    }

    /// A multiply by the reciprocal gives the quotient and the remainder of every dividend, the multiplier of 33 bits
    /// (7, 19) with its add-back and the others without.
    #[test]
    fn test_unsigned_reciprocal_preserves_quotient_and_remainder() {
        for divisor in [3_i64, 5, 6, 7, 10, 19, 100, 641, 1000, 65537, 2147483647] {
            let results = [ir::Held { value: 2, width: 4 }, ir::Held { value: 3, width: 4 }];
            let mut count = 4..;
            let mut fresh = || count.next().unwrap();
            let parts = unsigned_reciprocal(ir::Held { value: 1, width: 4 }, divisor, &results, &mut fresh, "P5", true).unwrap().expect("a reciprocal on a Pentium");
            for number in [0_u64, 1, 2, divisor as u64 - 1, divisor as u64, divisor as u64 + 1, 12345, 0x7fff_ffff, 0x8000_0000, 0xffff_fffe, 0xffff_ffff] {
                let values = run_unsigned(&parts, number);
                assert_eq!(values[&2], number / divisor as u64, "{number} / {divisor}");
                assert_eq!(values[&3], number % divisor as u64, "{number} % {divisor}");
            }
        }
    }

    /// The 486's multiply by a magic number takes 13 to 42 clocks and its `div` 40: no reciprocal wins there, and one
    /// does on a Pentium (multiply 10, divide 41), whatever the instruction before said of a flat price.
    #[test]
    fn test_unsigned_reciprocal_follows_the_cpu() {
        let results = [ir::Held { value: 2, width: 4 }, ir::Held { value: 3, width: 4 }];
        let attempt = |cpu: &str| {
            let mut count = 4..;
            let mut fresh = || count.next().unwrap();
            unsigned_reciprocal(ir::Held { value: 1, width: 4 }, 10, &results, &mut fresh, cpu, false).unwrap().is_some()
        };
        assert_eq!((attempt("386"), attempt("486"), attempt("P5")), (false, false, true));
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
