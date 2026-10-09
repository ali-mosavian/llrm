//! Port of `qbopt/backend/arithmetic.py`: target-dependent ranking of
//! constant arithmetic, not MIR semantics.

use std::collections::BTreeMap;

use crate::backend::cpu::{self as targets, ProfileOrName};
use crate::backend::timing;

pub fn cost<'a>(
    cpu: impl Into<ProfileOrName<'a>>,
    operation: &str,
) -> Result<i64, String> {
    targets::profile(cpu)?.cost(operation)
}

/// A left shift by `count`, by its cheapest form.
pub fn shift<'a>(
    cpu: impl Into<ProfileOrName<'a>>,
    count: i64,
) -> Result<i64, String> {
    let profile = targets::profile(cpu)?;
    profile.cost(if count == 1 { profile.doubling()? } else { "shift_ri" })
}

/// `int.bit_length()`.
fn bit_length(value: i64) -> i64 {
    i64::from(64 - value.unsigned_abs().leading_zeros())
}

/// Core clocks for audited positive imm8 on a 386, the table's range by the
/// immediate's bits where it has one, else the flat estimate.
///
/// Intel 80386 Programmer's Reference Manual, IMUL: for positive m,
/// max(ceil(log2(m)), 3) + 6. Restricted to positive imm8 so the value is
/// identical in word and dword forms. See docs/measurement/timing-audit.md.
pub fn immediate_multiply<'a>(
    cpu: impl Into<ProfileOrName<'a>> + Copy,
    number: i64,
) -> Result<i64, String> {
    if targets::profile(cpu)?.name == "386" && (0..=127).contains(&number) {
        return Ok((if number != 0 { bit_length(number - 1) } else { 0 }).max(3) + 6);
    }
    // A CPU whose table gives the multiply a range of clocks prices it by the
    // immediate's bits (`timing::multiply_clocks`).
    if let Some(clocks) = timing::multiply_clocks_of(cpu, 4, Some(bit_length(number)), number < 0)? {
        return Ok(clocks);
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

/// `cheapest_chain` for a value narrower than the dword a `lea` makes: shifts,
/// adds and subtracts only.
pub fn cheapest_narrow_chain<'a>(
    number: i64,
    cpu: impl Into<ProfileOrName<'a>>,
) -> Result<Option<(Vec<(&'static str, i64)>, i64)>, String> {
    chains(number, cpu, false)
}

/// The chain of `number`: of its magnitude negated, or, as GCC's `synth_mult`
/// has it for a negative `t` (`a * -7` is `a
/// - a*8`), of `1 - number` shifted and taken from the source.
fn chains<'a>(
    number: i64,
    cpu: impl Into<ProfileOrName<'a>>,
    with_lea: bool,
) -> Result<Option<(Chain, i64)>, String> {
    // Asked again and again for one multiply (is it scalable, what does it
    // cost, what is it): the answer is a function of the number and of the
    // profile's prices, so the profile remembers it.
    let target = targets::profile(cpu)?;
    if let Some(known) = target.multiplies.get(number, with_lea) {
        return Ok(known);
    }
    let found = unremembered_chains(number, target, with_lea)?;
    target.multiplies.put(number, with_lea, found.clone());
    Ok(found)
}

fn unremembered_chains(
    number: i64,
    target: &targets::Profile,
    with_lea: bool,
) -> Result<Option<(Chain, i64)>, String> {
    if number >= -1 || number == i64::MIN {
        return positive_chains(number, target, with_lea);
    }
    let alu = cost(target, "alu_rr")?;
    let mut best: Option<(Chain, i64)> = None;
    let mut consider = |parts: Chain, price: i64| {
        if best.as_ref().is_none_or(|(_, kept)| price < *kept) {
            best = Some((parts, price));
        }
    };
    if let Some((mut parts, price)) = positive_chains(-number, target, with_lea)? {
        parts.push(("neg", 0));
        consider(parts, price + alu);
    }
    // `1 - number = q << m`: the product `source - (source * q << m)`.
    let up = 1 - number;
    let m = i64::from(up.trailing_zeros());
    let q = up >> m;
    let shifted =
        if q == 1 { Some((Vec::new(), cost(target, "mov_rr")?)) } else { positive_chains(q, target, with_lea)? };
    if let Some((mut parts, price)) = shifted {
        parts.extend([("shl", m), ("rsub", 0)]);
        consider(parts, price + shift(target, m)? + alu);
    }
    Ok(best)
}

fn positive_chains<'a>(
    number: i64,
    cpu: impl Into<ProfileOrName<'a>>,
    with_lea: bool,
) -> Result<Option<(Chain, i64)>, String> {
    let target = targets::profile(cpu)?;
    if number <= 1 {
        return Ok(None);
    }

    let chain = |signed: bool| -> Vec<(&'static str, i64)> {
        let mut digits = Vec::new();
        let mut remaining = number;
        while remaining > 1 {
            let digit = if remaining & 1 != 0 { if signed { 2 - remaining.rem_euclid(4) } else { 1 } } else { 0 };
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

    // `lea r,[a+cur*scale]` where the target has the form: the shift and add it
    // replaces in one non-destructive instruction. Only a form with no
    // address-size prefix: behind one a dword `lea` carries two prefixes the
    // shift and add do not.
    let lea = |scale: i64| {
        let native = target
            .address_forms
            .iter()
            .any(|form| form.index_width == 4 && form.scales.contains(&scale) && !form.secondary);
        (with_lea && native)
            .then(|| {
                llrm_mir::target::three_operand(
                    &target.operations,
                    &target.address_forms,
                    4,
                    i64::from(target.operand_bytes),
                    scale,
                    false,
                )
            })
            .flatten()
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
        // One copy seeds the accumulator without destroying the source, unless
        // a `lea` makes the first step's result.
        let mut total = if parts.first().is_some_and(|part| part.0 == "lea") { 0 } else { cost(target, "mov_rr")? };
        for (name, count) in parts {
            total += match *name {
                "shl" => shift(target, *count)?,
                "lea" => lea(*count).expect("a lea part has its form"),
                // `cur + cur*scale` is one `lea`; `cur<<count` +- `cur` is a
                // shifted copy and an add.
                "flea" => lea(*count).expect("a lea part has its form"),
                "fadd" | "fsub" => cost(target, "mov_rr")? + shift(target, *count)? + cost(target, "alu_rr")?,
                _ => cost(target, "alu_rr")?,
            };
        }
        Ok(total)
    };

    // `min(..., key=clocks)`: the first of equal keys wins.
    let mut best: Option<(Vec<(&'static str, i64)>, i64)> = None;
    // Horner's chains first: what they cost bounds the search.
    for parts in [chain(false), chain(true)].into_iter().flat_map(|parts| [parts.clone(), fused(parts)]) {
        let price = clocks(&parts)?;
        if best.as_ref().is_none_or(|(_, kept)| price < *kept) {
            best = Some((parts, price));
        }
    }
    let shifts = |count: i64| shift(target, count).unwrap_or(i64::MAX / 4);
    let leas = |scale: i64| lea(scale).filter(|_| scale <= 8);
    let ops = Ops { shift: &shifts, alu: cost(target, "alu_rr")?, mov: cost(target, "mov_rr")?, lea: &leas };
    let limit = best.as_ref().map_or(i64::MAX / 4, |(_, price)| *price);
    let synthesized = synth(number, limit, &mut BTreeMap::new(), &ops).map(|(parts, _)| parts);
    if let Some(parts) = synthesized {
        let price = clocks(&parts)?;
        if best.as_ref().is_none_or(|(_, kept)| price < *kept) {
            best = Some((parts, price));
        }
    }
    Ok(best)
}

type Chain = Vec<(&'static str, i64)>;

#[cfg(test)]
thread_local! {
    /// The targets `synth` was asked of on this thread: the work a search did.
    static SEARCHED: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// What a chain's operations cost on the target, in clocks.
struct Ops<'a> {
    shift: &'a dyn Fn(i64) -> i64,
    alu: i64,
    mov: i64,
    /// A `lea` of `base + index*scale`, where the target has the scale.
    lea: &'a dyn Fn(i64) -> Option<i64>,
}

/// GCC's `alg_hash` entry (expmed.cc): the chain found for `t` within a limit,
/// or that none was.
enum Known {
    Found(Chain, i64),
    Impossible(i64),
}

/// GCC's `synth_mult` (expmed.cc): the chain of `t` costing less than `limit`,
/// and its cost, or none. The search is bounded as GCC's is: `limit` is what
/// the best chain so far costs (`cost_limit`, passed down less each operation:
/// `new_limit`), a `t` is looked up in `memo` first (`alg_hash`: what was
/// found, or that nothing is under that limit), and the caller starts from the
/// cost of the `imul` and the Horner chain (`expand_mult`'s `max_cost`).
/// Even `t` is a shift of `t >> m`; odd `t` is `t - 1` or `t + 1` (the one a
/// run of ones points to) plus or minus the source, a shift and a source added
/// to `(t - 1) >> m` or `(t + 1) >> m`, or `q * (2^m +- 1)` for a factor of
/// that form: `q`'s chain, then `cur + cur<<m` or `cur<<m - cur`. The chain's
/// copy of the source is not counted.
fn synth(
    t: i64,
    limit: i64,
    memo: &mut BTreeMap<i64, Known>,
    ops: &Ops,
) -> Option<(Chain, i64)> {
    #[cfg(test)]
    SEARCHED.with(|count| count.set(count.get() + 1));
    if limit <= 0 || t < 1 {
        return None;
    }
    if t == 1 {
        return Some((Vec::new(), 0));
    }
    match memo.get(&t) {
        Some(Known::Found(chain, cost)) => return (*cost < limit).then(|| (chain.clone(), *cost)),
        Some(Known::Impossible(under)) if limit <= *under => return None,
        _ => {}
    }
    let alu = ops.alu;
    // The options: what `t` is reached from, and the operations after it.
    let mut options: Vec<(i64, Chain, i64)> = Vec::new();
    // `q << m` and then an add or subtract of the source: one `lea` for an add
    // of a scale.
    let shift_add = |m: i64, sub: bool| -> (Chain, i64) {
        match (sub, (ops.lea)(1 << m)) {
            (false, Some(lea)) if (1..=3).contains(&m) => (vec![("lea", 1 << m)], lea),
            _ => (vec![("shl", m), (if sub { "sub" } else { "add" }, 0)], (ops.shift)(m) + alu),
        }
    };
    if t & 1 == 0 {
        let m = i64::from(t.trailing_zeros());
        options.push((t >> m, vec![("shl", m)], (ops.shift)(m)));
    } else {
        // A run of ones at the bottom (but not 3) is `(t + 1) - 1`; otherwise
        // `(t - 1) + 1`.
        if (t + 1) & !t > 2 && t != 3 {
            options.push((t + 1, vec![("sub", 0)], alu));
        } else {
            options.push((t - 1, vec![("add", 0)], alu));
        }
        for m in (2..=(63 - i64::from((t - 1).leading_zeros()))).rev() {
            let up = (1i64 << m) + 1;
            let down = (1i64 << m) - 1;
            if t % up == 0 && t > up {
                let (op, cost) = match (m <= 3).then(|| (ops.lea)(1 << m)).flatten() {
                    Some(lea) => (("flea", 1 << m), lea),
                    None => (("fadd", m), ops.mov + (ops.shift)(m) + alu),
                };
                options.push((t / up, vec![op], cost));
                break;
            }
            if t % down == 0 && t > down {
                options.push((t / down, vec![("fsub", m)], ops.mov + (ops.shift)(m) + alu));
                break;
            }
        }
        for (q, sub) in [(t - 1, false), (t + 1, true)] {
            let m = i64::from(q.trailing_zeros());
            if m > 0 {
                let (tail, cost) = shift_add(m, sub);
                options.push((q >> m, tail, cost));
            }
        }
    }
    let mut bound = limit;
    let mut best: Option<(Chain, i64)> = None;
    for (q, tail, cost) in options {
        // No progress: a larger target whose odd part is no smaller.
        if q >= t && q >> q.trailing_zeros() >= t {
            continue;
        }
        let Some((mut parts, below)) = synth(q, bound - cost, memo, ops) else { continue };
        parts.extend(tail);
        bound = below + cost;
        best = Some((parts, bound));
    }
    memo.insert(
        t,
        match &best {
            Some((chain, cost)) => Known::Found(chain.clone(), *cost),
            None => Known::Impossible(limit),
        },
    );
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    /// x3, x5 and x9 were `shl`, `add` and a copy (or an `imul` at -Os) where
    /// the address unit makes them in one instruction: the chain of a
    /// factor `base + base*scale` is that one `lea`.
    #[test]
    fn test_a_factor_of_one_more_than_a_scale_is_one_lea() {
        let m32 = targets::tuned_for(&llrm_x86_m32::M32, "486", false).unwrap();
        for factor in [3, 5, 9] {
            let (chain, _) = cheapest_chain(factor, m32).unwrap().unwrap();
            assert_eq!(chain, [("lea", factor - 1)], "x{factor}");
        }
        assert!(
            cheapest_narrow_chain(3, m32).unwrap().unwrap().0.iter().all(|part| part.0 != "lea"),
            "a word has no lea"
        );
    }

    /// An `imul` by a 31-bit constant took 41 clocks on a 486 (Table 10.1 note
    /// 3: 10 + log2 of the multiplier) and was priced as the flat 26, so
    /// the 29-clock chain lost to it: x_switch's `r * 1103515245` cost 20
    /// clocks a trip over gcc's shifts and adds.
    #[test]
    fn test_a_multiply_by_a_wide_constant_is_priced_by_its_multiplier_on_a_486() {
        let m32 = targets::tuned_for(&llrm_x86_m32::M32, "486", false).unwrap();
        assert_eq!(immediate_multiply(m32, 1103515245).unwrap(), 41);
        assert_eq!(immediate_multiply(m32, 10).unwrap(), 14);
        assert_eq!(immediate_multiply(m32, 3).unwrap(), 13);
        assert_eq!(immediate_multiply(m32, -3).unwrap(), 15, "a negative multiplier has a floor of 5 bits");
        assert_eq!(immediate_multiply(m32, -1103515245).unwrap(), 41);
        assert!(scale(1103515245, m32).unwrap().is_some(), "the chain is below the imul");
    }

    /// GCC's `synth_mult` factors `q * (2^m +- 1)` and tries `t - 1` and `t +
    /// 1`; the chains here were Horner chains only, 17 operations and 29
    /// clocks for x_switch's `r * 1103515245` against gcc's 13. Every chain is
    /// the product, in 32 and in 16 bits.
    #[test]
    fn test_synthesized_chains_are_the_product_and_shorter_than_horner() {
        let m32 = targets::tuned_for(&llrm_x86_m32::M32, "486", false).unwrap();
        let eval = |chain: &[(&str, i64)], source: i128| {
            let mut value = source;
            for &(name, count) in chain {
                match name {
                    "lea" => value = source + value * i128::from(count),
                    "flea" => value += value * i128::from(count),
                    "neg" => value = -value,
                    "rsub" => value = source - value,
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
        let mut factors: Vec<i64> = (-2000i64..2000).filter(|n| n.abs() > 1).collect();
        factors.extend([
            1103515245,
            214013,
            69069,
            1664525,
            22695477,
            1000003,
            40503,
            2654435761,
            0x7fff_ffff,
            0xffff_fffe,
            -1103515245,
            -7,
            -15,
            -2147483647,
        ]);
        for factor in factors {
            for (name, chain) in
                [("dword", cheapest_chain(factor, m32).unwrap()), ("word", cheapest_narrow_chain(factor, m32).unwrap())]
            {
                let Some((chain, _)) = chain else { continue };
                for source in [0i128, 1, -1, 7, -32768, 32767, -2147483648, 2147483647] {
                    assert_eq!(eval(&chain, source), source * i128::from(factor), "{name} x{factor}: {chain:?}");
                }
            }
        }
        // `a * -15` is `a - a*16` (synth_mult's negative `t`), not the product
        // by 15 and a negation.
        assert_eq!(cheapest_narrow_chain(-15, m32).unwrap().unwrap().0, [("shl", 4), ("rsub", 0)]);
        let (chain, clocks) = cheapest_chain(1103515245, m32).unwrap().unwrap();
        assert!(clocks < 29, "{chain:?} {clocks}");
    }

    /// `a * 2654448107u` took 75 million instructions in the selector, four of
    /// them 337 million, at -O0 as well: the selector asks for a multiply's
    /// chain three times (is it scalable, what does it cost, what is it) and
    /// each time searched every way down from every target. GCC's
    /// `synth_mult` prunes by the cost so far and keeps what
    /// it found (`alg_hash`).
    #[test]
    fn test_the_search_for_a_wide_odd_constant_is_made_once_and_bounded() {
        let m32 = targets::tuned_for(&llrm_x86_m32::M32, "486", false).unwrap();
        for factor in [2654448107i64, 3141592653, 4294967291, 2147483647, 1103515245] {
            SEARCHED.with(|count| count.set(0));
            let first = cheapest_chain(factor, m32).unwrap().unwrap();
            let once = SEARCHED.with(std::cell::Cell::get);
            for _ in 0..3 {
                assert_eq!(cheapest_chain(factor, m32).unwrap().unwrap(), first);
            }
            assert_eq!(SEARCHED.with(std::cell::Cell::get), once, "x{factor} searched again");
            assert!(once < 4000, "x{factor}: {once} targets searched for {:?}", first.0);
        }
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
                            "neg" => value = -value,
                            "rsub" => value = source - value,
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

    /// The selector expanded x*85 using a 22-clock guess; its immediate
    /// multiply costs 13.
    #[test]
    fn test_386_small_immediate_multiply_is_not_costed_as_unknown_dword() {
        assert!(scale(85, "386").unwrap().is_none());
        assert!(scale(10, "386").unwrap().is_some());
    }

    #[test]
    fn test_386_positive_immediate_early_out() {
        for (number, clocks) in [(0, 9), (1, 9), (8, 9), (9, 10), (20, 11), (85, 13), (127, 13)] {
            assert_eq!(immediate_multiply("386", number), Ok(clocks), "{number}");
        }
    }
}
