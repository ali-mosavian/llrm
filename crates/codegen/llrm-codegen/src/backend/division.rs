//! Select a dword division by a constant as a multiply by its reciprocal,
//! where the CPU's prices make that cheaper.
//!
//! # How it works
//! MIR keeps `sdiv`/`udiv`/`srem`/`urem` by a constant as divisions; this
//! module is asked by isel (`divide` in `isel.rs`) and answers with the
//! instruction sequence, or none to keep `idiv`/`div`. Only dword operands
//! qualify, and never when the profile is tuned for size (the magic number
//! is a 5-byte immediate; `cdq; idiv r` is 3 bytes).
//!
//! [`magic`] (signed, after LLVM's SignedDivisionByConstantInfo) and
//! [`unsigned_magic`] (Hacker's Delight `magicu`) give the multiplier and
//! shift. The sequences, with `x` the dividend and `q`, `r` the results:
//!
//! ```text
//!   signed (divisor d, 1 < d < 2^31,       unsigned (3 <= d < 2^31, not 2^k)
//!   not 2^k: shifts, `power_of_two`)
//!     m = MAGIC                              m = MAGIC
//!     (lo,hi) = imul m, x                    (lo,hi) = mul m, x
//!     hi += x          if MAGIC < 0          t  = x - hi        } if the
//!     s  = hi >> 31 (logical)                t  = t >> 1        } multiplier
//!     hi = hi >> sh (arithmetic) if sh != 0  hi = t + hi        } is 33 bits
//!     q  = hi + s                            q  = hi >> sh (or sh-1)
//!     r  = x - q*d     (remainder wanted)    r  = x - q*d
//! ```
//!
//! `q*d` is built by [`arithmetic::scale`]'s shift/add/`lea` chain where one
//! exists, else by `imul q, d`. A negative divisor below -1 divides by its
//! magnitude and negates the quotient; the remainder is unchanged, since it
//! takes the dividend's sign. `-1` and `i32::MIN` stay divisions.
//!
//! The profitability test prices both sides in clocks from the CPU profile.
//! The estimate sums the register copies, the multiply (`timing`'s
//! audited bounds, interpolated by the dividend's significant bits where the
//! CPU ends a multiply early), shifts, adds and the remainder's `q*d`. On a
//! target whose code is not 32-bit, each dword instruction adds one prefix
//! clock. The reciprocal is chosen when its estimate is below the divide's
//! minimum. A CPU with no audited bound for the multiply or divide keeps the
//! division.
//!
//! The multiplier is placed in the accumulator and the dividend where the
//! 486's early-out reads the operand's length, so the multiply is priced by
//! the dividend's bits and not the magic's 31.
//!
//! # Example
//! `Tenth& = x \ 10` on a LONG: the divide is a multiply by `0x66666667`, a
//! shift by 2 and the sign. `x` is the loaded dword `v2`. `; inserted` notes
//! are dropped.
//!
//! ```lir
//! v2 := mov [None v1@unplaced]
//! v5 := mov 0x66666667
//! v6, v7 := imul v5, v2
//! v8 := shr v7, 0x1f
//! v9 := sar v7, 0x2
//! v4 := add v9, v8
//! ```
//!
//! From `ISEL_DUMP=1 llrm-qb -O2 -march=i386` of function `TENTH&`, stage
//! `FarIndirectCalls`.
//!
//! # Used by
//! `isel.rs` (`divide`), for each division by a constant.
//!
//! # Needs
//! [`crate::backend::arithmetic`] for the multiply chains and
//! [`crate::backend::timing`] for the clocks.
//!
//! # See also
//! GCC `expand_divmod`; `docs/machine/arithmetic-targets.md`

use crate::backend::arithmetic;
use crate::backend::cpu::{self as targets, ProfileOrName};
use crate::backend::timing;
use crate::model::ir;

/// Positive-divisor form of LLVM's SignedDivisionByConstantInfo algorithm.
pub fn magic(
    divisor: i64,
    bits: i64,
) -> Result<(i64, i64), String> {
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

/// `quotient` times the divisor, by the shifts, adds and `lea`s of `chain`
/// (`arithmetic::scale`): the product of the step before it is shifted, or
/// added to the quotient, or the quotient plus it scaled, as the address unit
/// makes it.
fn chain_product(
    parts: &mut Vec<ir::Semantics>,
    chain: &[(&'static str, i64)],
    quotient: ir::Held,
    fresh: &mut dyn FnMut() -> u32,
) -> ir::Held {
    let width = quotient.width;
    let mut product = quotient;
    for &(name, amount) in chain {
        let into = ir::Held { value: fresh(), width };
        if name == "flea" {
            let cell =
                ir::Mem { base: Some(product), index: Some(product), scale: amount, ..ir::Mem::new(None, width) };
            parts.push(ir::Semantics {
                name: Some("lea".into()),
                dests: vec![ir::Loc::Held(into)],
                sources: vec![ir::Loc::Mem(cell)],
                ..ir::Semantics::new(ir::Operation::Address)
            });
            product = into;
            continue;
        }
        if name == "fadd" || name == "fsub" {
            let shifted = ir::Held { value: fresh(), width };
            parts.push(ir::Semantics {
                name: Some("shl".into()),
                dests: vec![ir::Loc::Held(shifted)],
                sources: vec![ir::Loc::Held(product), ir::Loc::Imm(ir::Imm { value: amount, width: 1, address: None })],
                ..ir::Semantics::new(ir::Operation::Binary)
            });
            let op = if name == "fadd" { "add" } else { "sub" };
            parts.push(ir::Semantics {
                name: Some(op.into()),
                dests: vec![ir::Loc::Held(into)],
                sources: vec![ir::Loc::Held(shifted), ir::Loc::Held(product)],
                ..ir::Semantics::new(ir::Operation::Binary)
            });
            product = into;
            continue;
        }
        parts.push(if name == "lea" {
            let cell =
                ir::Mem { base: Some(quotient), index: Some(product), scale: amount, ..ir::Mem::new(None, width) };
            ir::Semantics {
                name: Some("lea".into()),
                dests: vec![ir::Loc::Held(into)],
                sources: vec![ir::Loc::Mem(cell)],
                ..ir::Semantics::new(ir::Operation::Address)
            }
        } else {
            let other = if name == "shl" {
                ir::Loc::Imm(ir::Imm { value: amount, width: 1, address: None })
            } else {
                ir::Loc::Held(quotient)
            };
            ir::Semantics {
                name: Some(name.into()),
                dests: vec![ir::Loc::Held(into)],
                sources: vec![ir::Loc::Held(product), other],
                ..ir::Semantics::new(ir::Operation::Binary)
            }
        });
        product = into;
    }
    product
}

/// The significant bits of a positive `value`.
fn bit_length(value: i64) -> i64 {
    i64::from(64 - value.leading_zeros())
}

/// A reciprocal is not taken for size: its magic number is a 32-bit immediate
/// (5 bytes, a `mov`) before the multiply and the shifts and corrections after
/// it, where `cdq; idiv r` is 3 bytes. Tuned for size the division stays, as
/// GCC's `-Os` leaves it (LLVM's, which multiplies, differs: it does not weigh
/// the bytes here).
pub fn reciprocal<'a>(
    dividend: ir::Held,
    divisor: i64,
    results: &[ir::Held],
    fresh: &mut dyn FnMut() -> u32,
    cpu: impl Into<ProfileOrName<'a>>,
    remainder: bool,
    bits: Option<i64>,
) -> Result<Option<Vec<ir::Semantics>>, String> {
    // By a negative divisor the quotient is the negation of the one by its
    // magnitude (truncation is symmetric) and the remainder is the same (it
    // takes the dividend's sign): LLVM's BuildSDIV negates the quotient the
    // same way. The divisor INT_MIN, whose magnitude is not an `i32`, and
    // -1 stay divisions.
    if !(i64::from(i32::MIN) < divisor && divisor < -1) {
        return positive_reciprocal(dividend, divisor, results, fresh, cpu, remainder, bits);
    }
    let width = dividend.width;
    let magnitude = ir::Held { value: fresh(), width };
    let Some(mut parts) =
        positive_reciprocal(dividend, -divisor, &[magnitude, results[1]], fresh, cpu, remainder, bits)?
    else {
        return Ok(None);
    };
    parts.push(ir::Semantics {
        name: Some("neg".into()),
        dests: vec![ir::Loc::Held(results[0])],
        sources: vec![ir::Loc::Held(magnitude)],
        ..ir::Semantics::new(ir::Operation::Unary)
    });
    Ok(Some(parts))
}

/// `x / 2^k` by shifts, as the pin's DAGCombiner `BuildSDIVPow2` makes it: a
/// negative dividend is biased by `2^k - 1` (its sign, shifted down to the
/// bias) before the arithmetic shift rounds toward zero:
///
/// ```text
///   s = sar x, 31;  b = shr s, 32 - k;  q = sar (add x, b), k
///   r = sub x, (shl q, k)                        (remainder wanted)
/// ```
///
/// A multiply by the magic number was the choice for a power of two (the
/// pass `algebraic` had expanded it before isel saw it): `imul` and three
/// fixups for what is four shifts and an add, 31% of `nbody_fixed`'s clocks.
fn power_of_two(
    dividend: ir::Held,
    divisor: i64,
    results: &[ir::Held],
    fresh: &mut dyn FnMut() -> u32,
    remainder: bool,
) -> Vec<ir::Semantics> {
    let width = dividend.width;
    let k = i64::from(divisor.trailing_zeros());
    let imm = |value: i64| ir::Loc::Imm(ir::Imm { value, width: 1, address: None });
    let mut parts = Vec::new();
    let mut emit = |name: &str, sources: Vec<ir::Loc>, into: Option<ir::Held>| -> ir::Held {
        let into = into.unwrap_or_else(|| ir::Held { value: fresh(), width });
        parts.push(ir::Semantics {
            name: Some(name.into()),
            dests: vec![ir::Loc::Held(into)],
            sources,
            ..ir::Semantics::new(ir::Operation::Binary)
        });
        into
    };
    let sign = emit("sar", vec![ir::Loc::Held(dividend), imm(i64::from(width) * 8 - 1)], None);
    let bias = emit("shr", vec![ir::Loc::Held(sign), imm(i64::from(width) * 8 - k)], None);
    let biased = emit("add", vec![ir::Loc::Held(dividend), ir::Loc::Held(bias)], None);
    let quotient = emit("sar", vec![ir::Loc::Held(biased), imm(k)], Some(results[0]));
    if remainder {
        let product = emit("shl", vec![ir::Loc::Held(quotient), imm(k)], None);
        emit("sub", vec![ir::Loc::Held(dividend), ir::Loc::Held(product)], Some(results[1]));
    }
    parts
}

fn positive_reciprocal<'a>(
    dividend: ir::Held,
    divisor: i64,
    results: &[ir::Held],
    fresh: &mut dyn FnMut() -> u32,
    cpu: impl Into<ProfileOrName<'a>>,
    remainder: bool,
    bits: Option<i64>,
) -> Result<Option<Vec<ir::Semantics>>, String> {
    let cpu = targets::profile(cpu)?;
    let width = dividend.width;
    if !matches!(width, 2 | 4) || !(1 < divisor && divisor < 1 << (8 * width - 1)) {
        return Ok(None);
    }
    // The pin shifts a power of two at every size (`visitSDIVLike` has no
    // size test; `isIntDivCheap` guards only the reciprocal below).
    if divisor & (divisor - 1) == 0 {
        return Ok(Some(power_of_two(dividend, divisor, results, fresh, remainder)));
    }
    if llrm_target::keeps_division(i64::from(width), cpu.size) {
        return Ok(None);
    }
    // The magic is in the accumulator and the dividend where the early-out
    // reads: its bits set the price.
    let multiply_cost = timing::multiply_clocks(cpu, i64::from(width), Some(bits.unwrap_or(0)))?;
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
                total += match *name {
                    "shl" => arithmetic::shift(cpu, *count)?,
                    "fadd" | "fsub" => arithmetic::shift(cpu, *count)? + cost("alu_rr")?,
                    "flea" => cost("lea")?,
                    _ => cost("alu_rr")?,
                };
            }
            total
        }
        None => {
            // `imul q, divisor`: the immediate is the multiplier.
            timing::multiply_clocks(cpu, i64::from(width), Some(bit_length(divisor)))?
                .ok_or_else(|| "no multiply bound".to_owned())?
        }
    };
    if !remainder {
        reconstruction = 0;
    }
    // Materialize magic, seed multiply, preserve dividend and correction,
    // and seed reconstruction. Allocation may eliminate some of these moves.
    let copies = if remainder { 5 } else { 4 };
    let mut estimate = copies * cost("mov_rr")?
        + multiply_cost
        + reconstruction
        + (1 + i64::from(shift != 0)) * cost("shift_ri")?
        + (1 + i64::from(remainder) + i64::from(multiplier < 0)) * cost("alu_rr")?;
    // One clock per operand-size prefix (Intel 241430-004 section 24.3) on each
    // dword instruction where the code's own size is not a dword: every one
    // in real mode, none when flat. Charge the reserved copies too.
    if cpu.operand_bytes != 4 {
        let extra = copies + 4 + i64::from(remainder) + i64::from(multiplier < 0) + i64::from(shift != 0);
        estimate += cpu.operations.prefix * extra;
        if remainder {
            estimate += cpu.operations.prefix
                * match chained {
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
        let into = into.unwrap_or_else(|| ir::Held { value: fresh(), width });
        parts.push(ir::Semantics {
            name: Some(name.into()),
            dests: vec![ir::Loc::Held(into)],
            sources,
            ..ir::Semantics::new(operation)
        });
        into
    };
    let imm = |value: i64, width: u32| ir::Loc::Imm(ir::Imm { value, width, address: None });

    let constant = emit(&mut parts, fresh, ir::Operation::Move, "mov", vec![imm(multiplier, width)], None);
    let low = ir::Held { value: fresh(), width };
    let mut high = ir::Held { value: fresh(), width };
    parts.push(ir::Semantics {
        name: Some("imul".into()),
        dests: vec![ir::Loc::Held(low), ir::Loc::Held(high)],
        // The accumulator is the first source and the r/m operand the last: the
        // dividend, the one whose length the early-out reads, goes where it is
        // read (GCC's order; the magic there takes 31 bits whatever the
        // dividend).
        sources: vec![ir::Loc::Held(constant), ir::Loc::Held(dividend)],
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
    let sign = emit(&mut parts, fresh, ir::Operation::Binary, "shr", vec![ir::Loc::Held(high), imm(31, 1)], None);
    if shift != 0 {
        high = emit(&mut parts, fresh, ir::Operation::Binary, "sar", vec![ir::Loc::Held(high), imm(shift, 1)], None);
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
    let product = if let Some(chain) = chained {
        chain_product(&mut parts, chain, quotient, fresh)
    } else {
        emit(
            &mut parts,
            fresh,
            ir::Operation::Multiply,
            "imul",
            vec![ir::Loc::Held(quotient), imm(divisor, width)],
            None,
        )
    };
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

/// Hacker's Delight `magicu` for 32 bits: the multiplier, whether the product
/// needs the dividend added back (a 33-bit multiplier), and the shift after it.
/// `divisor` is 3 or more and not a power of two.
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

/// An unsigned division of a dword by a constant as a multiply by its
/// reciprocal, where the target's own prices (the audited multiply bounds, its
/// divide, the prefix its code size carries) make that cheaper: `mul`'s high
/// half shifted, with the dividend added back where the multiplier is 33 bits.
/// None keeps `div`.
pub fn unsigned_reciprocal<'a>(
    dividend: ir::Held,
    divisor: i64,
    results: &[ir::Held],
    fresh: &mut dyn FnMut() -> u32,
    cpu: impl Into<ProfileOrName<'a>>,
    remainder: bool,
    bits: Option<i64>,
) -> Result<Option<Vec<ir::Semantics>>, String> {
    let cpu = targets::profile(cpu)?;
    let width = dividend.width;
    if !(2 < divisor && divisor < 1 << 31)
        || (divisor & (divisor - 1)) == 0
        || llrm_target::keeps_division(i64::from(width), cpu.size)
    {
        return Ok(None);
    }
    let (Some(multiply), Some(divide)) =
        (timing::multiply_clocks(cpu, 4, Some(bits.unwrap_or(0)))?, timing::unsigned_divide(cpu, 4)?)
    else {
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
                    total += match *name {
                        "shl" => arithmetic::shift(cpu, *count)?,
                        "fadd" | "fsub" => arithmetic::shift(cpu, *count)? + cost("alu_rr")?,
                        "flea" => cost("lea")?,
                        _ => cost("alu_rr")?,
                    };
                }
                total
            }
            None => timing::multiply_clocks(cpu, 4, Some(bit_length(divisor)))?
                .ok_or_else(|| "no multiply bound".to_owned())?,
        }
    };
    // The multiplier, the multiply's seed, the dividend kept for the correction
    // and for the remainder.
    let copies = if remainder { 4 } else { 3 };
    let shifts = if add { 2 } else { 1 };
    let alu = if add { 3 } else { 0 } + i64::from(remainder);
    let mut estimate =
        copies * cost("mov_rr")? + multiply + reconstruction + shifts * cost("shift_ri")? + alu * cost("alu_rr")?;
    if cpu.operand_bytes != 4 {
        estimate += cpu.operations.prefix
            * (copies + 1 + shifts + alu + i64::from(remainder) * chained.map_or(1, |chain| chain.len() as i64));
    }
    if estimate >= divide.minimum {
        return Ok(None);
    }
    let mut parts = Vec::new();
    let imm = |value: i64, width: u32| ir::Loc::Imm(ir::Imm { value, width, address: None });
    let emit = |parts: &mut Vec<ir::Semantics>,
                operation: ir::Operation,
                name: &str,
                sources: Vec<ir::Loc>,
                into: Option<ir::Held>,
                fresh: &mut dyn FnMut() -> u32|
     -> ir::Held {
        let into = into.unwrap_or_else(|| ir::Held { value: fresh(), width });
        parts.push(ir::Semantics {
            name: Some(name.into()),
            dests: vec![ir::Loc::Held(into)],
            sources,
            ..ir::Semantics::new(operation)
        });
        into
    };
    let constant = emit(&mut parts, ir::Operation::Move, "mov", vec![imm(multiplier as i64, width)], None, fresh);
    let low = ir::Held { value: fresh(), width };
    let high = ir::Held { value: fresh(), width };
    parts.push(ir::Semantics {
        name: Some("mul".into()),
        dests: vec![ir::Loc::Held(low), ir::Loc::Held(high)],
        sources: vec![ir::Loc::Held(constant), ir::Loc::Held(dividend)],
        ..ir::Semantics::new(ir::Operation::Multiply)
    });
    let quotient_into = results[0];
    let quotient = if add {
        // ((x - t) >> 1) + t, then shifted the rest of the way.
        let difference = emit(
            &mut parts,
            ir::Operation::Binary,
            "sub",
            vec![ir::Loc::Held(dividend), ir::Loc::Held(high)],
            None,
            fresh,
        );
        let half =
            emit(&mut parts, ir::Operation::Binary, "shr", vec![ir::Loc::Held(difference), imm(1, 1)], None, fresh);
        let sum =
            emit(&mut parts, ir::Operation::Binary, "add", vec![ir::Loc::Held(half), ir::Loc::Held(high)], None, fresh);
        if shift > 1 {
            emit(
                &mut parts,
                ir::Operation::Binary,
                "shr",
                vec![ir::Loc::Held(sum), imm(shift - 1, 1)],
                Some(quotient_into),
                fresh,
            )
        } else {
            emit(&mut parts, ir::Operation::Move, "mov", vec![ir::Loc::Held(sum)], Some(quotient_into), fresh)
        }
    } else if shift > 0 {
        emit(
            &mut parts,
            ir::Operation::Binary,
            "shr",
            vec![ir::Loc::Held(high), imm(shift, 1)],
            Some(quotient_into),
            fresh,
        )
    } else {
        emit(&mut parts, ir::Operation::Move, "mov", vec![ir::Loc::Held(high)], Some(quotient_into), fresh)
    };
    if !remainder {
        return Ok(Some(parts));
    }
    let product = if let Some(chain) = chained {
        chain_product(&mut parts, chain, quotient, fresh)
    } else {
        emit(
            &mut parts,
            ir::Operation::Multiply,
            "imul",
            vec![ir::Loc::Held(quotient), imm(divisor, width)],
            None,
            fresh,
        )
    };
    emit(
        &mut parts,
        ir::Operation::Binary,
        "sub",
        vec![ir::Loc::Held(dividend), ir::Loc::Held(product)],
        Some(results[1]),
        fresh,
    );
    Ok(Some(parts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::hash::HashMap;

    fn signed(value: i64) -> i64 {
        ((value & 0xffffffff) ^ 0x80000000) - 0x80000000
    }

    fn held(one: &ir::Loc) -> u32 {
        match one {
            ir::Loc::Held(held) => held.value,
            other => panic!("{other:?}"),
        }
    }

    /// #789 gave the multiply chains a `lea` step; the remainder's
    /// quotient-times-divisor took it for a binary operation and named it `lea
    /// r, r2`, which no instruction is: a flat target's `x % 10` failed to
    /// assemble ("Semantics(op=BINARY, name='lea' ...)") at
    /// -O2 on a Pentium.
    #[test]
    fn test_a_remainders_product_makes_its_lea_step_as_an_address() {
        let m32 = crate::backend::cpu::tuned_for(&llrm_x86_m32::M32, "P5", false).unwrap();
        let results = [ir::Held { value: 2, width: 4 }, ir::Held { value: 3, width: 4 }];
        let mut count = 4..;
        let mut fresh = || count.next().unwrap();
        for signed in [true, false] {
            let parts = if signed {
                reciprocal(ir::Held { value: 1, width: 4 }, 10, &results, &mut fresh, m32, true, None)
            } else {
                unsigned_reciprocal(ir::Held { value: 1, width: 4 }, 10, &results, &mut fresh, m32, true, None)
            }
            .unwrap()
            .expect("a reciprocal");
            assert!(
                !parts.iter().any(|one| one.name.as_deref() == Some("lea") && one.op == ir::Operation::Binary),
                "signed {signed}: {parts:?}"
            );
            assert!(
                parts.iter().any(|one| one.name.as_deref() == Some("lea") && one.op == ir::Operation::Address),
                "signed {signed}: {parts:?}"
            );
        }
    }

    /// The 486's multiply ends early on its r/m operand, the last source of the
    /// one-operand form: the magic number (31 bits whatever the divisor) is
    /// in the accumulator and the dividend where its length is read. The other
    /// way round every division took 42 clocks, as long as `idiv`'s 43.
    #[test]
    fn test_the_dividend_is_the_operand_the_early_out_reads() {
        let m32 = crate::backend::cpu::tuned_for(&llrm_x86_m32::M32, "486", false).unwrap();
        let results = [ir::Held { value: 2, width: 4 }, ir::Held { value: 3, width: 4 }];
        let dividend = ir::Held { value: 1, width: 4 };
        for signed in [true, false] {
            let mut count = 4..;
            let mut fresh = || count.next().unwrap();
            let parts = if signed {
                reciprocal(dividend, 7, &results, &mut fresh, m32, false, None)
            } else {
                unsigned_reciprocal(dividend, 7, &results, &mut fresh, m32, false, None)
            }
            .unwrap()
            .expect("a reciprocal");
            let high = parts
                .iter()
                .find(|one| one.op == ir::Operation::Multiply && one.dests.len() == 2)
                .expect("the high multiply");
            assert_eq!(high.sources.last(), Some(&ir::Loc::Held(dividend)), "signed {signed}: {high:?}");
        }
    }

    /// A signed dividend known to need all 32 bits keeps the division on the
    /// 486 (42 + the fixups against 43); one of unknown length is priced at
    /// the middle of 13 to 42; a short one is cheaper still. The middle is an
    /// assumption.
    #[test]
    fn test_a_signed_reciprocal_is_priced_by_the_dividends_length() {
        let m32 = crate::backend::cpu::tuned_for(&llrm_x86_m32::M32, "486", false).unwrap();
        let results = [ir::Held { value: 2, width: 4 }, ir::Held { value: 3, width: 4 }];
        let attempt = |divisor: i64, bits: Option<i64>| {
            let mut count = 4..;
            let mut fresh = || count.next().unwrap();
            reciprocal(ir::Held { value: 1, width: 4 }, divisor, &results, &mut fresh, m32, true, bits)
                .unwrap()
                .is_some()
        };
        for divisor in [3, 7, 10, 641] {
            assert_eq!(
                (attempt(divisor, Some(32)), attempt(divisor, None), attempt(divisor, Some(12))),
                (false, true, true),
                "divisor {divisor}"
            );
        }
    }

    /// recsum -Os grew from 64 to 78 bytes and recmany -Os from 167 to 200 when
    /// every `x % n` took the reciprocal for its clocks: the magic number
    /// alone is 5 bytes against `cdq; idiv`'s 3. Tuned for size the division
    /// stays.
    #[test]
    fn test_tuned_for_size_a_division_by_a_constant_stays_a_division() {
        let m32 = crate::backend::cpu::tuned_for(&llrm_x86_m32::M32, "486", true).unwrap();
        let results = [ir::Held { value: 2, width: 4 }, ir::Held { value: 3, width: 4 }];
        let mut count = 4..;
        let mut fresh = || count.next().unwrap();
        assert_eq!(reciprocal(ir::Held { value: 1, width: 4 }, 7, &results, &mut fresh, m32, true, None), Ok(None));
        assert_eq!(
            unsigned_reciprocal(ir::Held { value: 1, width: 4 }, 7, &results, &mut fresh, m32, true, None),
            Ok(None)
        );
    }

    /// Tuned for size a power of two is still shifts (the pin's `visitSDIVLike`
    /// has no size test for it): recchop -Os read `(lo + hi) / 2` as `cwd;
    /// idiv` and nbody_fixed -Os ran 48% slower without the shifts.
    #[test]
    fn test_tuned_for_size_a_division_by_a_power_of_two_is_still_shifts() {
        let m16 = crate::backend::cpu::tuned_for(&llrm_x86_m16::M16, "486", true).unwrap();
        let results = [ir::Held { value: 2, width: 2 }, ir::Held { value: 3, width: 2 }];
        let mut count = 4..;
        let mut fresh = || count.next().unwrap();
        let parts = reciprocal(ir::Held { value: 1, width: 2 }, 2, &results, &mut fresh, m16, false, None)
            .unwrap()
            .expect("shifts");
        assert!(parts.iter().all(|one| one.op == ir::Operation::Binary), "{parts:?}");
        // The reciprocal still waits for the clocks: not at -Os.
        let results = [ir::Held { value: 2, width: 4 }, ir::Held { value: 3, width: 4 }];
        assert_eq!(reciprocal(ir::Held { value: 1, width: 4 }, 7, &results, &mut fresh, m16, false, None), Ok(None));
    }

    /// LNGMXX's q+r needs both answers, including negative truncation and
    /// INT_MIN; here also by negative divisors, whose quotient is the one
    /// by the magnitude, negated, and whose remainder is the same.
    #[test]
    fn test_reciprocal_preserves_signed_quotient_and_remainder() {
        for divisor in [3, 7, 10, 31, 1000, 2147483647, -3, -7, -10, -31, -1000, -2147483647] {
            let source = ir::Held { value: 1, width: 4 };
            let results = [ir::Held { value: 2, width: 4 }, ir::Held { value: 3, width: 4 }];
            let mut count = 4..;
            let mut fresh = || count.next().unwrap();
            let parts =
                reciprocal(source, divisor, &results, &mut fresh, "P5", true, None).unwrap().expect("a reciprocal");
            for number in [-2147483648, -divisor, -divisor + 1, -1, 0, 1, divisor - 1, divisor, 2147483647] {
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
                        "neg" => -args[0],
                        other => panic!("{other}"),
                    };
                    values.insert(held(&part.dests[0]), signed(answer));
                }
                let quotient = number / divisor;
                assert_eq!(values[&2], quotient, "{number} / {divisor}");
                assert_eq!(values[&3], number - quotient * divisor, "{number} % {divisor}");
            }
        }
    }

    /// A signed division by a power of two is shifts, the pin's
    /// `BuildSDIVPow2`, never the magic multiply: `nbody_fixed`'s `/ 512`
    /// took `imul` and three fixups once `algebraic` no longer expanded it,
    /// and ran 31% slower.
    #[test]
    fn test_a_signed_division_by_a_power_of_two_is_shifts() {
        // Words too: `(lo + hi) / 2` at m16 was `cwd; idiv` (recchop -Os).
        for (width, divisor) in [2, 4, 16, 512, 1 << 30, -2, -16, -512]
            .into_iter()
            .map(|divisor| (4, divisor))
            .chain([2, 4, 16, 512, 1 << 14, -2, -16, -512].into_iter().map(|divisor| (2, divisor)))
        {
            let source = ir::Held { value: 1, width };
            let results = [ir::Held { value: 2, width }, ir::Held { value: 3, width }];
            let bits = 8 * i64::from(width);
            let wrap = |value: i64| (value << (64 - bits)) >> (64 - bits);
            let mut count = 4..;
            let mut fresh = || count.next().unwrap();
            let parts =
                reciprocal(source, divisor, &results, &mut fresh, "486", true, None).unwrap().expect("a sequence");
            assert!(!parts.iter().any(|one| one.op == ir::Operation::Multiply), "{divisor}: a multiply in {parts:?}");
            for number in [-2147483648, -513, -512, -17, -1, 0, 1, 16, 511, 512, 513, 2147483647]
                .into_iter()
                .map(wrap)
                .filter(|&number| width == 4 || (-32768..=32767).contains(&number))
            {
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
                        "add" => args[0] + args[1],
                        "sub" => args[0] - args[1],
                        "shl" => args[0] << args[1],
                        "shr" => (args[0] & ((1i64 << bits) - 1)) >> args[1],
                        "sar" => args[0] >> args[1],
                        "neg" => -args[0],
                        other => panic!("{other}"),
                    };
                    values.insert(held(&part.dests[0]), wrap(answer));
                }
                let quotient = number / divisor;
                assert_eq!(values[&2], quotient, "{number} / {divisor}");
                assert_eq!(values[&3], number - quotient * divisor, "{number} % {divisor}");
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

    /// Run `parts` on `number` as the machine would, unsigned, and give each
    /// value by its number.
    fn run_unsigned(
        parts: &[ir::Semantics],
        number: u64,
    ) -> HashMap<u32, u64> {
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

    /// A multiply by the reciprocal gives the quotient and the remainder of
    /// every dividend, the multiplier of 33 bits (7, 19) with its add-back
    /// and the others without.
    #[test]
    fn test_unsigned_reciprocal_preserves_quotient_and_remainder() {
        for divisor in [3_i64, 5, 6, 7, 10, 19, 100, 641, 1000, 65537, 2147483647] {
            let results = [ir::Held { value: 2, width: 4 }, ir::Held { value: 3, width: 4 }];
            let mut count = 4..;
            let mut fresh = || count.next().unwrap();
            let parts =
                unsigned_reciprocal(ir::Held { value: 1, width: 4 }, divisor, &results, &mut fresh, "P5", true, None)
                    .unwrap()
                    .expect("a reciprocal on a Pentium");
            for number in [
                0_u64,
                1,
                2,
                divisor as u64 - 1,
                divisor as u64,
                divisor as u64 + 1,
                12345,
                0x7fff_ffff,
                0x8000_0000,
                0xffff_fffe,
                0xffff_ffff,
            ] {
                let values = run_unsigned(&parts, number);
                assert_eq!(values[&2], number / divisor as u64, "{number} / {divisor}");
                assert_eq!(values[&3], number % divisor as u64, "{number} % {divisor}");
            }
        }
    }

    /// The 486's multiply ends early on a short r/m operand: 10 + max(bits, 3),
    /// 13 to 42 clocks, against `div`'s 40. The dividend is that operand,
    /// so a reciprocal is priced by its bits: a dividend known to need all 32
    /// keeps the division, one of unknown length is priced at the early-out
    /// minimum (gcc's assumption: it multiplies by the reciprocal whatever the
    /// dividend), a short one is priced exactly. A Pentium's multiply is 10
    /// whatever the operand.
    #[test]
    fn test_unsigned_reciprocal_follows_the_cpu_and_the_dividends_length() {
        let results = [ir::Held { value: 2, width: 4 }, ir::Held { value: 3, width: 4 }];
        let attempt = |cpu: &str, bits: Option<i64>| {
            let mut count = 4..;
            let mut fresh = || count.next().unwrap();
            unsigned_reciprocal(ir::Held { value: 1, width: 4 }, 10, &results, &mut fresh, cpu, false, bits)
                .unwrap()
                .is_some()
        };
        assert_eq!((attempt("386", Some(32)), attempt("486", Some(32)), attempt("P5", Some(32))), (false, false, true));
        assert_eq!((attempt("386", None), attempt("486", None), attempt("P5", None)), (true, true, true));
        // 31 bits: the multiply takes 41 of the 486's 42.
        assert!(!attempt("486", Some(31)) && attempt("486", Some(16)));
        assert_eq!((attempt("386", Some(12)), attempt("486", Some(12)), attempt("P5", Some(12))), (true, true, true));
    }

    #[test]
    fn test_slow_multiply_keeps_division() {
        let mut count = 4..;
        let mut fresh = || count.next().unwrap();
        let results = [ir::Held { value: 2, width: 4 }, ir::Held { value: 3, width: 4 }];
        assert_eq!(
            reciprocal(ir::Held { value: 1, width: 4 }, 7, &results, &mut fresh, "386", true, Some(32)),
            Ok(None)
        );
    }
}
