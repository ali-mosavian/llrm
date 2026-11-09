//! Whether a multiply by a constant is shifts and an add, priced.
//!
//! The pin's `M68kTargetLowering::decomposeMulByConstant` answers
//! `VT.bitsLE(i32) || atLeast68020` for every constant
//! ([L1547](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/M68k/M68kISelLowering.cpp#L1547)).
//! DEVIATE M68K-MUL-PRICE: from the 68020 the multiply is one `muls.l` (4 B;
//! the constant's register is shared), and the pin's chain of `(2^n +- 1) << k`
//! is 6 to 10 B: 45 small programs grew 2 to 28 B at -Os. The answer here is
//! the level's [`Price`] of the chain against the multiply, on the CPU's
//! tables, so a size build keeps `muls.l` where the chain is longer and a speed
//! build takes the chain where it is faster. The 68000 and 68010 have no 32-bit
//! multiply: it is a call (16 B at the site, and the routine), so the chain
//! always wins there, as the pin has it. Wider than 32 bits stays the multiply
//! (the pin's `atLeast68020` answer decomposed `i64` into pairs; unpriced).
//! Owner m68k. Exit 2026-12-20: the price from the selected instructions (G2,
//! `synth_mult` for any constant).
//!
//! Complexity: O(1) per multiply.

use super::hooks::Cpu;
use crate::timing::Price;

/// `(bytes, clocks)` of `text` on `cpu`: `bytes` are the instruction's length.
fn one(
    cpu: Cpu,
    text: &str,
    bytes: usize,
) -> (usize, u32) {
    let clocks = crate::timing::cost(cpu, text, bytes as u32, false).map_or(0, |cost| cost.clocks);
    (bytes, clocks)
}

fn sum(parts: impl IntoIterator<Item = (usize, u32)>) -> (usize, u32) {
    parts.into_iter().fold((0, 0), |(b, c), (pb, pc)| (b + pb, c + pc))
}

/// A shift of a data register by `amount`: the count in the instruction up to
/// 8, else in a register set by `moveq`.
fn shift(
    cpu: Cpu,
    amount: u32,
) -> (usize, u32) {
    if amount <= 8 {
        return one(cpu, &format!("lsl.l #{amount}, %d1"), 2);
    }
    sum([one(cpu, &format!("moveq #{amount}, %d2"), 2), one(cpu, "lsl.l %d2, %d1", 2)])
}

/// The chain the combiner builds for `c = +-(2^n +- 1) << k`, as
/// instructions: the pin's `(x << n + k) +- (x << k)` (then a negate), or
/// `None` for any other constant.
pub fn chain(
    cpu: Cpu,
    c: u32,
) -> Option<(usize, u32)> {
    let negative = c >> 31 == 1;
    let magnitude = if negative { c.wrapping_neg() } else { c };
    let zeros = if magnitude == 2 { 0 } else { magnitude.trailing_zeros() };
    let odd = magnitude >> zeros;
    let bits = if (odd - 1).is_power_of_two() {
        odd - 1
    } else if (odd + 1).is_power_of_two() {
        odd + 1
    } else {
        return None;
    };
    let n = bits.trailing_zeros();
    let mut parts = vec![one(cpu, "move.l %d0, %d1", 2), shift(cpu, n + zeros)];
    if zeros > 0 {
        parts.push(shift(cpu, zeros));
    }
    parts.push(one(cpu, "add.l %d1, %d0", 2));
    if negative {
        parts.push(one(cpu, "neg.l %d0", 2));
    }
    Some(sum(parts))
}

/// The multiply: `muls.l` alone. The constant's register is set once for
/// every multiply by it in the function and loop-invariant (`moveq` or a long
/// immediate, hoisted and shared), where a chain repeats its instructions at
/// each site; counting the load per site made a tie of 6 B and grew
/// `rotate_small` (four `x * 31`) by 12 B.
pub fn multiply(cpu: Cpu) -> (usize, u32) {
    one(cpu, "muls.l %d1, %d0", 4)
}

/// Whether `x * c` of `bits` bits is shifts and an add under `price`.
pub fn decomposes(
    cpu: Cpu,
    price: Price,
    bits: u32,
    c: u128,
) -> bool {
    if bits > 32 {
        return false;
    }
    let c = (c & u128::from(u32::MAX)) as u32;
    let Some(chain) = chain(cpu, c) else { return false };
    if !cpu.at_least(Cpu::M68020) {
        return true;
    }
    price.beats(cpu, chain, multiply(cpu))
}
