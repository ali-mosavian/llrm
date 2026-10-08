//! Port of `qbopt/backend/arithmetic.py`: target-dependent ranking of
//! constant arithmetic, not MIR semantics.

use std::collections::BTreeMap;

use crate::backend::cpu::{self as targets, ProfileOrName};

pub fn cost<'a>(cpu: impl Into<ProfileOrName<'a>>, operation: &str) -> Result<i64, String> {
    targets::profile(cpu)?.cost(operation)
}

/// A left shift by `count`, by its cheapest form.
pub fn shift<'a>(cpu: impl Into<ProfileOrName<'a>>, count: i64) -> Result<i64, String> {
    let profile = targets::profile(cpu)?;
    profile.cost(if count == 1 { profile.doubling()? } else { "shift_ri" })
}

/// `int.bit_length()`.
fn bit_length(value: i64) -> i64 {
    i64::from(64 - value.unsigned_abs().leading_zeros())
}

/// Core clocks for audited positive imm8 on a 386 and every immediate on a 486; other forms still use the old estimate.
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
    if targets::profile(cpu)?.name == "486" {
        // Intel 240440-002, Table 10.1 note 3: 10 + max(log2|m|, n), n = 3 for +m and 5 for -m, by the immediate of the three-operand form. The data sheet does not say how the logarithm rounds: up, as `vsgcc/harness.py` counts it.
        return Ok(10 + bit_length(number).max(if number < 0 { 5 } else { 3 }));
    }
    cost(cpu, "imul_r32")
}

/// Binary and signed-digit chains, including destructive-operand copies,
/// where the target prices them below `imul`.
pub fn scale<'a>(
    number: i64,
    cpu: impl Into<ProfileOrName<'a>>,
) -> Result<Option<Vec<(&'static str, i64)>>, String> {
    let target = targets::profile(cpu)?;
    let Some((best, clocks)) = cheapest_chain(number, target)? else { return Ok(None) };
    Ok((clocks < immediate_multiply(target, number)?).then_some(best))
}

/// The cheaper of the binary and signed-digit chains a multiply by
/// `number` is, and its clocks.
pub fn cheapest_chain<'a>(
    number: i64,
    cpu: impl Into<ProfileOrName<'a>>,
) -> Result<Option<(Vec<(&'static str, i64)>, i64)>, String> {
    chains(number, cpu, true)
}

/// `cheapest_chain` for a value narrower than the dword a `lea` makes: shifts, adds and subtracts only.
pub fn cheapest_narrow_chain<'a>(
    number: i64,
    cpu: impl Into<ProfileOrName<'a>>,
) -> Result<Option<(Vec<(&'static str, i64)>, i64)>, String> {
    chains(number, cpu, false)
}

fn chains<'a>(number: i64, cpu: impl Into<ProfileOrName<'a>>, with_lea: bool) -> Result<Option<(Vec<(&'static str, i64)>, i64)>, String> {
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

    // `lea r,[a+cur*scale]` where the target has the form: the shift and add it replaces in one non-destructive instruction.
    // Only a form with no address-size prefix: behind one a dword `lea` carries two prefixes the shift and add do not.
    let lea = |scale: i64| {
        let native = target.address_forms.iter().any(|form| form.index_width == 4 && form.scales.contains(&scale) && !form.secondary);
        (with_lea && native).then(|| llrm_mir::target::three_operand(&target.operations, &target.address_forms, 4, i64::from(target.operand_bytes), scale, false)).flatten()
    };
    let fused = |parts: Vec<(&'static str, i64)>| -> Vec<(&'static str, i64)> {
        let mut out = Vec::new();
        let mut index = 0;
        while index < parts.len() {
            match (parts[index], parts.get(index + 1)) {
                (("shl", count), Some(&("add", 0))) if (1..=3).contains(&count) && lea(1 << count).is_some() => {
                    out.push(("lea", 1 << count));
                    index += 2;
                }
                (("fadd", count), _) if (1..=3).contains(&count) && lea(1 << count).is_some() => {
                    out.push(("flea", 1 << count));
                    index += 1;
                }
                (part, _) => {
                    out.push(part);
                    index += 1;
                }
            }
        }
        out
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
        // One copy seeds the accumulator without destroying the source, unless a `lea` makes the first step's result.
        let mut total = if parts.first().is_some_and(|part| part.0 == "lea") { 0 } else { cost(target, "mov_rr")? };
        for (name, count) in parts {
            total += match *name {
                "shl" => shift(target, *count)?,
                "lea" => lea(*count).expect("a lea part has its form"),
                // `cur + cur*scale` is one `lea`; `cur<<count` +- `cur` is a shifted copy and an add.
                "flea" => lea(*count).expect("a lea part has its form"),
                "fadd" | "fsub" => cost(target, "mov_rr")? + shift(target, *count)? + cost(target, "alu_rr")?,
                _ => cost(target, "alu_rr")?,
            };
        }
        Ok(total)
    };

    // `min(..., key=clocks)`: the first of equal keys wins.
    let mut best: Option<(Vec<(&'static str, i64)>, i64)> = None;
    let synthesized = synth(number, &mut BTreeMap::new(), &|parts| clocks(parts), &fused);
    let synthesized = synthesized?.map(|(parts, _)| parts);
    for parts in [chain(false), chain(true)].into_iter().flat_map(|parts| [parts.clone(), fused(parts)]).chain(synthesized) {
        let price = clocks(&parts)?;
        if best.as_ref().is_none_or(|(_, kept)| price < *kept) {
            best = Some((parts, price));
        }
    }
    Ok(best)
}

type Chain = Vec<(&'static str, i64)>;

/// GCC's `synth_mult` (expmed.cc): the chain of `t`, by the cheaper of what the lower bits allow. Even `t` is a shift of `t >> m`; odd `t` is
/// `t - 1` or `t + 1` (the one a run of ones points to) plus or minus the source, a shift and a source added to `(t - 1) >> m` or
/// `(t + 1) >> m`, or `q * (2^m +- 1)` for a factor of that form: `q`'s chain, then `cur + cur<<m` or `cur<<m - cur`. `fused` turns a shift
/// and an add into a `lea`; `price` is the clocks of a chain (with its seed copy), memoised by `t`.
fn synth(t: i64, memo: &mut BTreeMap<i64, Option<(Chain, i64)>>, price: &dyn Fn(&[(&'static str, i64)]) -> Result<i64, String>, fused: &dyn Fn(Chain) -> Chain) -> Result<Option<(Chain, i64)>, String> {
    if t == 1 {
        return Ok(Some((Vec::new(), 0)));
    }
    if t < 1 {
        return Ok(None);
    }
    if let Some(known) = memo.get(&t) {
        return Ok(known.clone());
    }
    let mut options: Vec<(i64, Chain)> = Vec::new();
    if t & 1 == 0 {
        let m = i64::from(t.trailing_zeros());
        options.push((t >> m, vec![("shl", m)]));
    } else {
        // A run of ones at the bottom (but not 3) is `(t + 1) - 1`; otherwise `(t - 1) + 1`.
        let w = (t + 1) & !t;
        if w > 2 && t != 3 {
            options.push((t + 1, vec![("sub", 0)]));
        } else {
            options.push((t - 1, vec![("add", 0)]));
        }
        for m in (2..=(63 - i64::from((t - 1).leading_zeros()))).rev() {
            let up = (1i64 << m) + 1;
            let down = (1i64 << m) - 1;
            if t % up == 0 && t > up {
                options.push((t / up, vec![("fadd", m)]));
                break;
            }
            if t % down == 0 && t > down {
                options.push((t / down, vec![("fsub", m)]));
                break;
            }
        }
        for (q, op) in [(t - 1, "add"), (t + 1, "sub")] {
            let m = i64::from(q.trailing_zeros());
            if m > 0 && q >> m > 1 {
                options.push((q >> m, vec![("shl", m), (op, 0)]));
            }
        }
    }
    let mut best: Option<(Chain, i64)> = None;
    for (q, tail) in options {
        if q >= t {
            // `t + 1` shifts down below `t` only when it is even, as it is for odd `t`; a larger `q` is no progress.
            let reduced = q >> q.trailing_zeros();
            if reduced >= t {
                continue;
            }
        }
        let Some((mut parts, _)) = synth(q, memo, price, fused)? else { continue };
        parts.extend(tail);
        let parts = fused(parts);
        let clocks = price(&parts)?;
        if best.as_ref().is_none_or(|(_, kept)| clocks < *kept) {
            best = Some((parts, clocks));
        }
    }
    memo.insert(t, best.clone());
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// x3, x5 and x9 were `shl`, `add` and a copy (or an `imul` at -Os) where the address unit makes them in one
    /// instruction: the chain of a factor `base + base*scale` is that one `lea`.
    #[test]
    fn test_a_factor_of_one_more_than_a_scale_is_one_lea() {
        let m32 = targets::tuned_for(&llrm_x86_m32::M32, "486", false).unwrap();
        for factor in [3, 5, 9] {
            let (chain, _) = cheapest_chain(factor, m32).unwrap().unwrap();
            assert_eq!(chain, [("lea", factor - 1)], "x{factor}");
        }
        assert!(cheapest_narrow_chain(3, m32).unwrap().unwrap().0.iter().all(|part| part.0 != "lea"), "a word has no lea");
    }

    /// An `imul` by a 31-bit constant took 41 clocks on a 486 (Table 10.1 note 3: 10 + log2 of the multiplier) and was priced as the flat
    /// 26, so the 29-clock chain lost to it: x_switch's `r * 1103515245` cost 20 clocks a trip over gcc's shifts and adds.
    #[test]
    fn test_a_multiply_by_a_wide_constant_is_priced_by_its_multiplier_on_a_486() {
        let m32 = targets::tuned_for(&llrm_x86_m32::M32, "486", false).unwrap();
        assert_eq!(immediate_multiply(m32, 1103515245).unwrap(), 41);
        assert_eq!(immediate_multiply(m32, 10).unwrap(), 14);
        assert_eq!(immediate_multiply(m32, -3).unwrap(), 15);
        assert!(scale(1103515245, m32).unwrap().is_some(), "the chain is below the imul");
    }

    /// GCC's `synth_mult` factors `q * (2^m +- 1)` and tries `t - 1` and `t + 1`; the chains here were Horner chains only, 17 operations
    /// and 29 clocks for x_switch's `r * 1103515245` against gcc's 13. Every chain is the product, in 32 and in 16 bits.
    #[test]
    fn test_synthesized_chains_are_the_product_and_shorter_than_horner() {
        let m32 = targets::tuned_for(&llrm_x86_m32::M32, "486", false).unwrap();
        let eval = |chain: &[(&str, i64)], source: i128| {
            let mut value = source;
            for &(name, count) in chain {
                match name {
                    "lea" => value = source + value * i128::from(count),
                    "flea" => value += value * i128::from(count),
                    "fadd" => value += value << count,
                    "fsub" => value = (value << count) - value,
                    "shl" => value <<= count,
                    "add" => value += source,
                    "sub" => value -= source,
                    other => panic!("{other}"),
                }
            }
            value
        };
        let mut factors: Vec<i64> = (2..2000).collect();
        factors.extend([1103515245, 214013, 69069, 1664525, 22695477, 1000003, 40503, 2654435761, 0x7fff_ffff, 0xffff_fffe]);
        for factor in factors {
            for (name, chain) in [("dword", cheapest_chain(factor, m32).unwrap()), ("word", cheapest_narrow_chain(factor, m32).unwrap())] {
                let Some((chain, _)) = chain else { continue };
                for source in [0i128, 1, -1, 7, -32768, 32767, -2147483648, 2147483647] {
                    assert_eq!(eval(&chain, source), source * i128::from(factor), "{name} x{factor}: {chain:?}");
                }
            }
        }
        let (chain, clocks) = cheapest_chain(1103515245, m32).unwrap().unwrap();
        assert!(chain.len() <= 12 && clocks < 29, "{chain:?} {clocks}");
    }

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
                            "lea" => value = source + value * count,
                            "flea" => value += value * count,
                            "fadd" => value += value << count,
                            "fsub" => value = (value << count) - value,
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
