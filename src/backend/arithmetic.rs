//! Port of `qbopt/backend/arithmetic.py`: target-dependent ranking of
//! constant arithmetic, not MIR semantics.

use crate::backend::cpu::{self as targets, ProfileOrName};

pub fn cost<'a>(cpu: impl Into<ProfileOrName<'a>>, operation: &str) -> Result<i64, String> {
    targets::profile(cpu)?.cost(operation)
}

/// A left shift by `count`, by its cheapest form.
pub fn shift<'a>(cpu: impl Into<ProfileOrName<'a>>, count: i64) -> Result<i64, String> {
    let profile = targets::profile(cpu)?;
    profile.cost(if count == 1 { profile.doubling()? } else { "shift_ri" })
}

pub fn validate<'a>(cpu: impl Into<ProfileOrName<'a>>) -> Result<(), String> {
    targets::profile(cpu)?;
    Ok(())
}

/// `int.bit_length()`.
fn bit_length(value: i64) -> i64 {
    i64::from(64 - value.unsigned_abs().leading_zeros())
}

/// Core clocks for audited positive imm8; other forms still use the old estimate.
///
/// Intel 80386 Programmer's Reference Manual, IMUL: for positive m,
/// max(ceil(log2(m)), 3) + 6. Restricted to positive imm8 so the value is
/// identical in word and dword forms. See docs/measurement/timing-audit.md.
pub fn immediate_multiply<'a>(
    cpu: impl Into<ProfileOrName<'a>> + Copy,
    number: i64,
) -> Result<i64, String> {
    if targets::profile(cpu)?.name == "386" && (0..=127).contains(&number) {
        return Ok((if number != 0 {
            bit_length(number - 1)
        } else {
            0
        })
        .max(3)
            + 6);
    }
    cost(cpu, "imul_r32")
}

/// Binary and signed-digit chains, including destructive-operand copies.
pub fn scale<'a>(
    number: i64,
    cpu: impl Into<ProfileOrName<'a>>,
) -> Result<Option<Vec<(&'static str, i64)>>, String> {
    let target = targets::profile(cpu)?;
    if number <= 1 {
        return Ok(None);
    }

    let chain = |signed: bool| -> Vec<(&'static str, i64)> {
        let mut digits = Vec::new();
        let mut remaining = number;
        while remaining > 1 {
            let digit = if remaining & 1 != 0 {
                if signed {
                    2 - remaining.rem_euclid(4)
                } else {
                    1
                }
            } else {
                0
            };
            digits.push(digit);
            remaining = (remaining - digit).div_euclid(2);
        }
        let mut parts = Vec::new();
        let mut shift = 0;
        for digit in digits.into_iter().rev() {
            shift += 1;
            if digit != 0 {
                parts.extend([("shl", shift), (if digit > 0 { "add" } else { "sub" }, 0)]);
                shift = 0;
            }
        }
        if shift != 0 {
            parts.push(("shl", shift));
        }
        parts
    };

    let clocks = |parts: &[(&str, i64)]| -> Result<i64, String> {
        if ["386", "P6"].contains(&target.name.as_str())
            && parts.len() == 3
            && parts[0].0 == "shl"
            && (1..=3).contains(&parts[0].1)
            && parts[1] == ("add", 0)
            && parts[2].0 == "shl"
        {
            // peephole.addresses selects LEA + SHL for this exact shape.
            // 386: two core clocks. P6: GCC pentiumpro_cost ranks indexed
            // LEA at one unit, like a shift, versus four for multiply.
            return Ok((if target.name == "386" { 2 } else { 1 }) + cost(target, "shift_ri")?);
        }
        // One copy seeds the accumulator without destroying the source.
        let mut total = cost(target, "mov_rr")?;
        for (name, count) in parts {
            total += if *name == "shl" { shift(target, *count)? } else { cost(target, "alu_rr")? };
        }
        Ok(total)
    };

    // `min(..., key=clocks)`: the first of equal keys wins.
    let (unsigned, signed) = (chain(false), chain(true));
    let (first, second) = (clocks(&unsigned)?, clocks(&signed)?);
    let best = if second < first { signed } else { unsigned };
    Ok(if clocks(&best)? < immediate_multiply(target, number)? {
        Some(best)
    } else {
        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_chains_preserve_product() {
        for cpu in ["386", "486", "P5", "P6"] {
            for factor in [3, 7, 15, 20, 31, 45, 63, 85, 127, 1000, 32769] {
                let Some(chain) = scale(factor, cpu).unwrap() else {
                    continue;
                };
                for source in [0i64, 1, -1, -32768, 32767, -2147483648, 2147483647] {
                    let mut value = source;
                    for (name, count) in &chain {
                        match *name {
                            "shl" => value <<= count,
                            "add" => value += source,
                            "sub" => value -= source,
                            _ => {}
                        }
                    }
                    assert_eq!(value, source * factor, "{cpu} {factor}");
                }
            }
        }
    }

    #[test]
    fn test_fast_multiply_changes_break_even() {
        assert!(scale(20, "386").unwrap().is_some());
        assert!(scale(20, "486").unwrap().is_some());
        assert!(scale(20, "P5").unwrap().is_some());
        assert!(scale(20, "P6").unwrap().is_some());
        assert!(scale(85, "P6").unwrap().is_none());
        assert_eq!(scale(7, "386").unwrap(), Some(vec![("shl", 3), ("sub", 0)]));
    }

    #[test]
    fn test_unknown_cpu_is_not_silently_defaulted() {
        assert!(scale(20, "unknown").unwrap_err().contains("CPU target"));
    }

    /// The selector expanded x*85 using a 22-clock guess; its immediate multiply costs 13.
    #[test]
    fn test_386_small_immediate_multiply_is_not_costed_as_unknown_dword() {
        assert!(scale(85, "386").unwrap().is_none());
        assert!(scale(10, "386").unwrap().is_some());
    }

    #[test]
    fn test_386_positive_immediate_early_out() {
        for (number, clocks) in [
            (0, 9),
            (1, 9),
            (8, 9),
            (9, 10),
            (20, 11),
            (85, 13),
            (127, 13),
        ] {
            assert_eq!(immediate_multiply("386", number), Ok(clocks), "{number}");
        }
    }
}
