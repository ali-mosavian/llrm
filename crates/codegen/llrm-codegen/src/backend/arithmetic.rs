//! Ranks the ways to multiply by a constant on a target: shift and add
//! chains, `lea`, or `imul`.
//!
//! # How it works
//! A constant multiply is priced two ways. `immediate_multiply` is the
//! clocks of `imul r,r,imm` on the CPU profile: on a 386 the manual's
//! `max(ceil(log2 m), 3) + 6` for a positive imm8, elsewhere the timing
//! table's range by the multiplier's bits, else the flat `imul_r32` price.
//!
//! `cheapest_chain` is the cheapest sequence of `shl`, `add`, `sub`, `lea`
//! and copies. Horner chains over binary and signed digits give a first
//! bound. Then `synth` searches (Bernstein 1986, branch and bound): an even `t`
//! is a shift of `t >> m`; an odd `t` is `t-1` or `t+1` plus or minus the
//! source; a factor `2^m +- 1` is `cur + cur<<m` or `cur<<m - cur`, one
//! `lea` for a scale of 2, 4 or 8. The search stops at the best cost so far
//! and remembers what it found per `t`. A negative multiplier is the chain
//! of its magnitude and a `neg`, or of `1 - n` shifted and subtracted from
//! the source. `cheapest_narrow_chain` drops `lea` for values narrower than
//! a dword.
//!
//! ```text
//!  x * 5, no lea             x * 5, with lea
//!    mov  t,x                  lea  t,[x + x*4]
//!    shl  t,2
//!    add  t,x
//! ```
//!
//! `scale` returns the chain only where it is cheaper than `imul`.
//! Results are kept in the profile, as a multiply is asked about again and
//! again.
//!
//! DEVIATE M1 (owner anyisa/w10, exit 2026-12-01; AM-M1, priced): the pin's X86
//! does a scalar constant multiply by `combineMul`
//! ([X86ISelLowering.cpp#L51191](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L51191))
//! and `decomposeMulByConstant` (#L3636): a `lea` for 3, 5 and 9 (times a power
//! of two) and shifts and adds for 2^n +- 1, otherwise `imul`; ours searches
//! the CPU profile's clocks (above), so a product like `x*7 + y` in an unrolled
//! loop is `lea`s where the pin has `imul`. Price, 2026-10-23, one build with
//! the chains switched off by a probe, 71 vsgcc programs at m32: clocks with
//! the chains are -5.1% / -5.9% / -6.0% at -O1 / -O2 / -O3, code
//! +3072 / +720 / +688 B (the chains cost bytes), -Os identical (no chain is
//! chosen; the DAG path since 2026-10-27 takes them at m32 -Os as the pin does:
//! +3037 B over 58 objects of tests/run/c, histo and x_dct clocks equal the
//! pin's, and keeps `imul` at m16 -Os, where they grow 27 of 175 by 3550 B);
//! QCport m16 -38 B at -O1..-O3, 0 at -Os. Instructions against the pin on the
//! same IR: lea +471 at -O2 (`docs/working/instruction-gap-vs-pin.md`). Exit:
//! none while the clocks decide; reviewed with the tuning phase.
//!
//! # Example
//! What the ranking answers on a 486 (`targets::tuned_for(M32, "486")`):
//!
//! ```text
//! cheapest_chain(3, 5, 9)          one lea each: [x+x*2], [x+x*4], [x+x*8]
//! cheapest_narrow_chain(-15)       shl 4 ; rsub        (x - x*16, no lea in a word)
//! immediate_multiply, clocks       3 -> 13   10 -> 14   -3 -> 15   1103515245 -> 41
//! ```
//!
//! From unit tests `test_a_factor_of_one_more_than_a_scale_is_one_lea`,
//! `test_synthesized_chains_are_the_product_and_shorter_than_horner` and
//! `test_a_multiply_by_a_wide_constant_is_priced_by_its_multiplier_on_a_486` in
//! `arithmetic.rs`.
//!
//! # Used by
//! `isel/matcher.rs` (which multiply to select), `division.rs` (a divisor's
//! multiply chain), and the QB ABI (`abi/qb.rs`) for its multiply costs.
//! Design notes: `docs/machine/arithmetic-targets.md`.

mod chain_search;

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
/// identical in word and dword forms. See docs/optimizations/backend/timing.md.
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
    Ok((clocks < immediate_multiply(target, number)? + dword_prefix(target)).then_some(best))
}

/// Bytes of `chain`'s shifts, adds and subtracts on `width`-byte registers.
/// The copy that seeds it is not counted: the allocator drops it where the
/// source dies.
pub fn chain_bytes(
    chain: &[(&str, i64)],
    width: i64,
    arch: &dyn llrm_target::Target,
) -> i64 {
    let operand = arch.operand_bytes();
    // A `lea r,[a+cur*s]` is the opcode, the ModRM and the SIB.
    chain
        .iter()
        .map(|&(name, count)| match name {
            "shl" => arch.shift_op_bytes(count, width),
            "lea" | "flea" => 3 + i64::from(width != operand),
            "fadd" | "fsub" => arch.shift_op_bytes(count, width) + 2 * arch.register_op_bytes(width),
            _ => arch.register_op_bytes(width),
        })
        .sum()
}

/// The chain for a multiply by `number` where it takes fewer clocks than
/// `imul r,r,imm` on the CPU's table, whatever its bytes (the search
/// at speed).
pub fn chain_by_clocks<'a>(
    number: i64,
    width: i64,
    cpu: impl Into<ProfileOrName<'a>>,
) -> Result<Option<Vec<(&'static str, i64)>>, String> {
    let target = targets::profile(cpu)?;
    let found = if width == 4 { cheapest_chain(number, target)? } else { cheapest_narrow_chain(number, target)? };
    let Some((chain, clocks)) = found else { return Ok(None) };
    let prefix = if width == 4 { dword_prefix(target) } else { 0 };
    Ok((immediate_multiply(target, number)? + prefix - clocks > 0).then_some(chain))
}

/// The encoded size of a chain's operations, to break a tie between chains of
/// equal clocks: `arch` and the width of the value.
#[derive(Clone, Copy)]
struct Bytes<'a> {
    arch: &'a dyn llrm_target::Target,
    width: i64,
}

/// Clocks weigh more than any chain's bytes in a `Bytes` ranking.
const CLOCK_WEIGHT: i64 = 4096;

/// [`chain_by_clocks`] with ties in clocks broken by encoded bytes: among the
/// chains of the fewest clocks, the one of fewest bytes (a Pareto choice,
/// equal clocks and fewer bytes; DEVIATE M32-MUL-O1-CHAIN in
/// `llrm-x86-m32`). The search ranks `clocks * CLOCK_WEIGHT + bytes`, so a
/// chain with more clocks never wins on bytes.
pub fn chain_by_clocks_then_bytes<'a>(
    number: i64,
    width: i64,
    cpu: impl Into<ProfileOrName<'a>>,
    arch: &dyn llrm_target::Target,
) -> Result<Option<Vec<(&'static str, i64)>>, String> {
    let target = targets::profile(cpu)?;
    let bytes = Bytes { arch, width };
    let found = if width == 4 {
        unremembered_chains(number, target, true, Some(bytes))?
    } else {
        unremembered_chains(number, target, false, Some(bytes))?
    };
    let Some((chain, clocks)) = found else { return Ok(None) };
    let prefix = if width == 4 { dword_prefix(target) } else { 0 };
    Ok((immediate_multiply(target, number)? + prefix - clocks > 0).then_some(chain))
}

/// The chain for a multiply by `number` of `width` bytes in a block `freq`
/// times as hot as the entry, where the clocks it saves over `imul r,r,imm`
/// pay for the bytes it adds at `milliclocks_per_byte` (the target's rate at
/// speed): `saved * freq * 1000 >= extra_bytes * rate`. None where the rate
/// is 0: the target does not trade bytes for clocks.
pub fn chain_at_speed<'a>(
    number: i64,
    width: i64,
    cpu: impl Into<ProfileOrName<'a>>,
    arch: &dyn llrm_target::Target,
    freq: f64,
    milliclocks_per_byte: i64,
) -> Result<Option<Vec<(&'static str, i64)>>, String> {
    priced_chain_at_speed(number, width, cpu, arch, freq, milliclocks_per_byte, false)
}

/// `chain_at_speed`; with `ties_keep_imul` the chain must pay strictly
/// (`price_pin_multiplies`: the `imul` is one instruction, so a tie is its).
pub fn priced_chain_at_speed<'a>(
    number: i64,
    width: i64,
    cpu: impl Into<ProfileOrName<'a>>,
    arch: &dyn llrm_target::Target,
    freq: f64,
    milliclocks_per_byte: i64,
    ties_keep_imul: bool,
) -> Result<Option<Vec<(&'static str, i64)>>, String> {
    let target = targets::profile(cpu)?;
    if milliclocks_per_byte <= 0 {
        return Ok(None);
    }
    let found = if width == 4 { cheapest_chain(number, target)? } else { cheapest_narrow_chain(number, target)? };
    let Some((chain, clocks)) = found else { return Ok(None) };
    let prefix = if width == 4 { dword_prefix(target) } else { 0 };
    let saved = immediate_multiply(target, number)? + prefix - clocks;
    let extra = chain_bytes(&chain, width, arch) - arch.multiply_immediate_bytes(number, width);
    let paid = saved as f64 * freq * 1000.0;
    let price = (extra * milliclocks_per_byte) as f64;
    Ok((saved > 0 && if ties_keep_imul { paid > price } else { paid >= price }).then_some(chain))
}

/// What a dword instruction costs beyond a word one where the target's own
/// operand is a word (m16): the operand-size prefix (66h). Each step of a
/// dword chain pays it, and so does the `imul` it is priced against.
fn dword_prefix(target: &targets::Profile) -> i64 {
    if target.operand_bytes < 4 { target.prefix_cost } else { 0 }
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

/// The chain of `number`: of its magnitude negated, or, for a negative `t` (`a
/// * -7` is `a
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
    let found = unremembered_chains(number, target, with_lea, None)?;
    target.multiplies.put(number, with_lea, found.clone());
    Ok(found)
}

fn unremembered_chains(
    number: i64,
    target: &targets::Profile,
    with_lea: bool,
    bytes: Option<Bytes>,
) -> Result<Option<(Chain, i64)>, String> {
    if number >= -1 || number == i64::MIN {
        return positive_chains_ranked(number, target, with_lea, bytes);
    }
    let rank = |parts: &Chain, price: i64| match bytes {
        Some(Bytes { arch, width }) => price * CLOCK_WEIGHT + chain_bytes(parts, width, arch),
        None => price,
    };
    let alu = cost(target, "alu_rr")?;
    let mut best: Option<(Chain, i64)> = None;
    let mut consider = |parts: Chain, price: i64| {
        if best.as_ref().is_none_or(|(kept_parts, kept)| rank(&parts, price) < rank(kept_parts, *kept)) {
            best = Some((parts, price));
        }
    };
    if let Some((mut parts, price)) = positive_chains_ranked(-number, target, with_lea, bytes)? {
        parts.push(("neg", 0));
        consider(parts, price + alu);
    }
    // `1 - number = q << m`: the product `source - (source * q << m)`.
    let up = 1 - number;
    let m = i64::from(up.trailing_zeros());
    let q = up >> m;
    let shifted = if q == 1 {
        Some((Vec::new(), cost(target, "mov_rr")?))
    } else {
        positive_chains_ranked(q, target, with_lea, bytes)?
    };
    if let Some((mut parts, price)) = shifted {
        parts.extend([("shl", m), ("rsub", 0)]);
        consider(parts, price + shift(target, m)? + alu);
    }
    Ok(best)
}

fn positive_chains_ranked<'a>(
    number: i64,
    cpu: impl Into<ProfileOrName<'a>>,
    with_lea: bool,
    bytes: Option<Bytes>,
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
                llrm_mir::target::three_operand_of(
                    &target.operations,
                    &target.address_forms,
                    4,
                    i64::from(target.operand_bytes),
                    scale,
                    false,
                    2,
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
            // 386: two core clocks. P6: the P6 cost table ranks indexed
            // LEA at one unit, like a shift, versus four for multiply.
            return Ok((if target.name == "386" { 2 } else { 1 }) + cost(target, "shift_ri")?);
        }
        // One copy seeds the accumulator without destroying the source, unless
        // a `lea` makes the first step's result.
        let prefix = if with_lea { dword_prefix(target) } else { 0 };
        let mut total =
            if parts.first().is_some_and(|part| part.0 == "lea") { 0 } else { cost(target, "mov_rr")? + prefix };
        for (name, count) in parts {
            total += match *name {
                "shl" => shift(target, *count)? + prefix,
                "lea" => lea(*count).expect("a lea part has its form"),
                // `cur + cur*scale` is one `lea`; `cur<<count` +- `cur` is a
                // shifted copy and an add.
                "flea" => lea(*count).expect("a lea part has its form"),
                "fadd" | "fsub" => {
                    cost(target, "mov_rr")? + shift(target, *count)? + cost(target, "alu_rr")? + 3 * prefix
                }
                _ => cost(target, "alu_rr")? + prefix,
            };
        }
        Ok(total)
    };

    // `min(..., key=clocks)`: the first of equal keys wins; with `bytes`, key
    // (clocks, bytes).
    let weigh = |parts: &Chain, price: i64| match bytes {
        Some(Bytes { arch, width }) => price * CLOCK_WEIGHT + chain_bytes(parts, width, arch),
        None => price,
    };
    let mut best: Option<(Vec<(&'static str, i64)>, i64)> = None;
    // Horner's chains first: what they cost bounds the search.
    for parts in [chain(false), chain(true)].into_iter().flat_map(|parts| [parts.clone(), fused(parts)]) {
        let price = clocks(&parts)?;
        if best.as_ref().is_none_or(|(kept_parts, kept)| weigh(&parts, price) < weigh(kept_parts, *kept)) {
            best = Some((parts, price));
        }
    }
    // The search ranks what `weigh` ranks: each operation's clocks, scaled,
    // and its bytes.
    let scaled = |clocks: i64, op_bytes: i64| match bytes {
        Some(_) => clocks * CLOCK_WEIGHT + op_bytes,
        None => clocks,
    };
    let (register, immediate_lea) = bytes.map_or((0, 0), |Bytes { arch, width }| {
        (arch.register_op_bytes(width), 3 + i64::from(width != arch.operand_bytes()))
    });
    let shifts = |count: i64| {
        let clocks = shift(target, count).unwrap_or(i64::MAX / 4 / CLOCK_WEIGHT);
        scaled(clocks, bytes.map_or(0, |Bytes { arch, width }| arch.shift_op_bytes(count, width)))
    };
    let leas = |scale: i64| lea(scale).filter(|_| scale <= 8).map(|clocks| scaled(clocks, immediate_lea));
    let ops = Ops {
        shift: &shifts,
        alu: scaled(cost(target, "alu_rr")?, register),
        mov: scaled(cost(target, "mov_rr")?, register),
        lea: &leas,
        seeded: bytes.is_some(),
    };
    let limit = best.as_ref().map_or(i64::MAX / 4, |(parts, price)| weigh(parts, *price));
    let synthesized =
        chain_search::synth(number, limit, &mut chain_search::Memo::default(), &ops).map(|(parts, _)| parts);
    if let Some(parts) = synthesized {
        let price = clocks(&parts)?;
        if best.as_ref().is_none_or(|(kept_parts, kept)| weigh(&parts, price) < weigh(kept_parts, *kept)) {
            best = Some((parts, price));
        }
    }
    Ok(best)
}

/// One step of a multiply chain: `shl m`, `add` / `sub` of the source,
/// `fadd m` (`cur + (cur << m)`), `fsub m` (`(cur << m) - cur`).
pub type ChainStep = (&'static str, i64);

/// The cheapest shift, add and subtract chain for `x * number` with a target's
/// own scalar prices: `shift(m)` for a shift by `m`, `alu` for an add or
/// subtract of the source, `mov` for a copy (a factor step copies the running
/// value; a chain that adds the source seeds with one). `None` when no chain
/// costs less than `limit`.
pub fn synthesize_chain(
    number: i64,
    limit: i64,
    shift: &dyn Fn(i64) -> i64,
    alu: i64,
    mov: i64,
) -> Option<(Vec<ChainStep>, i64)> {
    let ops = Ops { shift, alu, mov, lea: &|_| None, seeded: true };
    chain_search::synth(number, limit, &mut chain_search::Memo::default(), &ops)
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
    /// Whether the copy that seeds a chain not starting with a `lea` is
    /// counted, as `chains` counts it in a chain's clocks. A clocks-only
    /// ranking leaves it out; a ranking by (clocks, bytes) must count it,
    /// or a chain with a seed looks as cheap as one without and the search
    /// keeps the shorter, slower one.
    seeded: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The search was a translation of gcc's (GPL-3.0-or-later) in the default
    /// build, with its permalink and names. It is written from Bernstein's
    /// 1986 paper now; the chains it finds are the same.
    #[test]
    fn test_the_search_names_no_gcc_source() {
        for (file, text) in [
            ("arithmetic.rs", include_str!("arithmetic.rs")),
            ("chain_search.rs", include_str!("arithmetic/chain_search.rs")),
        ] {
            let production = text.split("#[cfg(test)]\nmod tests {").next().unwrap();
            for name in ["gcc-mirror", "expmed", "synth_mult", "alg_hash", "gcc.gnu"] {
                assert!(!production.contains(name), "{file} names {name}");
            }
        }
    }

    /// x_switch's `r * 1103515245`: the chain of 15 clocks on a 486 at m32,
    /// the same one the search always found.
    #[test]
    fn test_the_wide_lcg_multiplier_is_a_15_clock_chain_on_a_486() {
        let m32 = targets::tuned_for(&llrm_x86_m32::M32, "486", false).unwrap();
        assert_eq!(cheapest_chain(1103515245, m32).unwrap().unwrap().1, 15);
    }

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

    /// -O1 chose the first chain of the fewest clocks, whatever its bytes:
    /// `x * 1664525` was 29 B where another chain of the same clocks is
    /// shorter (mul_constants -O1: +547 B over the pin). Among the chains of
    /// the fewest clocks the one of fewest bytes is chosen, never more clocks.
    #[test]
    fn test_a_chain_of_equal_clocks_is_the_shorter_one() {
        let arch = &llrm_x86_m32::M32;
        let m32 = targets::tuned_for(arch, "486", false).unwrap();
        let profile = targets::profile(m32).unwrap();
        let (mut shorter, mut longer) = (0, 0);
        for number in (2..5000).chain((2..400).map(|n| -n)) {
            let Some((old, old_clocks)) = unremembered_chains(number, profile, true, None).unwrap() else { continue };
            let bytes = Some(Bytes { arch, width: 4 });
            let (new, new_clocks) = unremembered_chains(number, profile, true, bytes).unwrap().expect("one was found");
            assert!(
                new_clocks <= old_clocks,
                "x{number}: {new:?} ({new_clocks}) is slower than {old:?} ({old_clocks})"
            );
            let (old_bytes, new_bytes) = (chain_bytes(&old, 4, arch), chain_bytes(&new, 4, arch));
            if new_bytes < old_bytes {
                shorter += 1;
            }
            if new_clocks == old_clocks && new_bytes > old_bytes {
                longer += 1;
            }
        }
        assert_eq!(longer, 0, "at equal clocks a Pareto choice is never longer");
        assert!(shorter > 0, "no constant had a shorter chain of equal clocks");
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

    /// In 16-bit code every dword step of a chain carries the 66h prefix (a
    /// clock on the 486), as does the `imul`. Unpriced, the 11-step chain for
    /// 30011 (16 clocks against `imul`'s 25) ran slower than the `imul`:
    /// recpow c -O2 m16, +13501 clocks, +50002 instructions.
    #[test]
    fn test_a_dword_chain_in_16_bit_code_pays_the_prefix_per_step() {
        let m16 = targets::tuned_for(&llrm_x86_m16::M16, "486", false).unwrap();
        let m32 = targets::tuned_for(&llrm_x86_m32::M32, "486", false).unwrap();
        assert_eq!(immediate_multiply(m16, 30011).unwrap(), 25, "premise: the sheet's imul");
        assert!(scale(30011, m16).unwrap().is_none(), "the chain's steps cost a prefix each");
        assert!(scale(30011, m32).unwrap().is_some(), "no prefix at m32");
    }

    /// The chain is taken where the clocks it saves, times the block's
    /// frequency, pay for the bytes it adds at the target's rate (m16: one
    /// clock is two bytes): x*43 run once is not worth its bytes, in a loop
    /// (8 trips) it is, and with no rate (m32, m64) never.
    #[test]
    fn test_a_chain_is_taken_where_saved_clocks_times_frequency_pay_the_bytes() {
        let m16 = targets::tuned_for(&llrm_x86_m16::M16, "486", false).unwrap();
        let at = |width, freq, rate| chain_at_speed(43, width, m16, &llrm_x86_m16::M16, freq, rate).unwrap();
        assert!(at(4, 1.0, 500).is_none(), "once: 43 clocks' worth of bytes is not saved");
        assert!(at(4, 8.0, 500).is_some(), "in a loop");
        assert!(at(4, 1000.0, 0).is_none(), "no rate: bytes and clocks are not traded");
        assert!(at(2, 1.0, 500).is_none() || at(2, 8.0, 500).is_some(), "hotter never loses a chain");
        let (chain, clocks) = cheapest_chain(43, m16).unwrap().unwrap();
        let extra = chain_bytes(&chain, 4, &llrm_x86_m16::M16)
            - llrm_target::Target::multiply_immediate_bytes(&llrm_x86_m16::M16, 43, 4);
        let saved = immediate_multiply(m16, 43).unwrap() + dword_prefix(m16) - clocks;
        assert!(saved > 0 && extra > 2 * saved && extra <= 2 * saved * 8, "premise: saved {saved}, extra {extra}");
    }

    /// The search factors `q * (2^m +- 1)` and tries `t - 1` and `t +
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
        // `a * -15` is `a - a*16` (a negative `t`), not the product
        // by 15 and a negation.
        assert_eq!(cheapest_narrow_chain(-15, m32).unwrap().unwrap().0, [("shl", 4), ("rsub", 0)]);
        let (chain, clocks) = cheapest_chain(1103515245, m32).unwrap().unwrap();
        assert!(clocks < 29, "{chain:?} {clocks}");
    }

    /// `a * 2654448107u` took 75 million instructions in the selector, four of
    /// them 337 million, at -O0 as well: the selector asks for a multiply's
    /// chain three times (is it scalable, what does it cost, what is it) and
    /// each time searched every way down from every target. The search now
    /// prunes by the cost so far and keeps what it found.
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
