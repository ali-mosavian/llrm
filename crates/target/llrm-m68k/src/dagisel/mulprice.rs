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
//! build takes the chain where it is faster; -O1 keeps the pin's chain unless
//! the multiply is no worse in both (the function-level trial below). The 68000
//! and 68010 have no 32-bit multiply: it is a call (16 B at the site, and the
//! routine), so the chain always wins there, as the pin has it. Wider than 32
//! bits: the pin's `atLeast68020` answer decomposed `i64` into pairs; here an
//! `i64` below the 68020 is a chain of at most three pair shifts
//! ([`wide_chain`]), where the multiply is a call or partial products (`b * 3`
//! at 68000 -O2: 38 B against 72), and from the 68020 it stays the multiply (28
//! B against the chain's 38). Owner m68k. Exit 2026-12-20: the price from the
//! selected instructions (G2, `synth_mult` for any constant).
//!
//!
//! DEVIATE M68K-MUL-LATE: the per-site answer cannot see what the chain does to
//! the function (the scheduler hoists independent calls above the chain and
//! spills their results, 15 B a call in select_of_constants -Os; the shared
//! `lea __mulsi3, %a6` stops paying; `muls.w` of a halfword is 4 B against an
//! 8 B chain). A function that used the chain is also built with the multiply
//! kept (`Mode::Kept`) and the cheaper by the level's price is kept
//! (`crate::trial`): -Os by bytes, -O2/-O3 by `bytes + 2 * clocks`, a tie keeps
//! the multiply; -O1 (no worse in bytes and clocks) replaces the chain only
//! where the multiply is no worse in both, and keeps it where each build wins
//! one axis. The library routine's clocks come from [`crate::routine`]. Owner
//! m68k-b. Exit 2026-12-20, with the price above. Cells (68000, against
//! #8532 alone): select_of_constants -Os, int64_division -Os, matmul -Os,
//! nbody_fixed -O1 and div_constants_short -Os smaller; none larger, none
//! slower. The -O1 cells of select_of_constants, int64_division and matmul
//! keep the chain: bytes above the pin's `__mulsi3` build, clocks far below.
//!
//! Complexity: O(1) per multiply; a function that used the chain is compiled
//! twice (1150 of 12320 corpus functions).

use std::cell::Cell;
use std::rc::Rc;

use super::hooks::Cpu;
use crate::timing::Price;

/// How a build treats a multiply by a constant.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Mode {
    /// The decomposition stands, as the pin's M68k has it.
    Pin,
    /// The decomposition stands; a function that used it is also built with
    /// [`Mode::Kept`], and the cheaper kept.
    Trial,
    /// The multiply stays: `muls.l`, or the library call before the 68020.
    Kept,
    /// As [`Mode::Trial`] without the chains beyond the pin's shape (G2): the
    /// build the trial judges them against.
    TrialPinShapes,
}

/// Whether the combiner may decompose a multiply in this build of a function,
/// and whether it did: DEVIATE M68K-MUL-LATE builds a function that did also
/// without, and keeps the cheaper (`crate::trial`).
#[derive(Clone, Default)]
pub struct Chain {
    off: bool,
    pin: bool,
    shapes_only: bool,
    used: Rc<Cell<bool>>,
    synthesized: Rc<Cell<bool>>,
}

impl Chain {
    pub fn of(mode: Mode) -> Chain {
        Chain { off: mode == Mode::Kept, pin: mode == Mode::Pin, shapes_only: mode == Mode::TrialPinShapes, used: Rc::default(), synthesized: Rc::default() }
    }

    /// Whether this build decomposed at least one multiply.
    pub fn used(&self) -> bool {
        self.used.get()
    }

    /// Whether this build took a chain beyond the pin's shape (DEVIATE G2).
    pub fn synthesized(&self) -> bool {
        self.synthesized.get()
    }

    pub(super) fn note_synthesized(&self) {
        self.synthesized.set(true);
    }

    /// Whether a chain beyond the pin's shape may be taken: not in a build
    /// that follows the pin, nor one that keeps the multiply.
    pub(super) fn allows_beyond_pin(&self) -> bool {
        !self.pin && !self.off && !self.shapes_only
    }

    pub(super) fn allows(&self, yes: bool) -> bool {
        let yes = yes && !self.off;
        if yes {
            self.used.set(true);
        }
        yes
    }
}

/// `(bytes, clocks)` of `text` on `cpu`: `bytes` are the instruction's length.
pub(super) fn one(
    cpu: Cpu,
    text: &str,
    bytes: usize,
) -> (usize, u32) {
    let clocks = crate::timing::cost(cpu, text, bytes as u32, false).map_or(0, |cost| cost.clocks);
    (bytes, clocks)
}

pub(super) fn sum(parts: impl IntoIterator<Item = (usize, u32)>) -> (usize, u32) {
    parts.into_iter().fold((0, 0), |(b, c), (pb, pc)| (b + pb, c + pc))
}

/// A shift of a data register by `amount`: the count in the instruction up to
/// 8, else in a register set by `moveq`.
pub(super) fn shift(
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
    if bits > 64 {
        return false;
    }
    if bits == 64 {
        // from the 68020 the pair's multiply is inline and shorter than the
        // chain (`b * 3`: 28 B against 38)
        return !cpu.at_least(Cpu::M68020) && wide_chain(c as u64);
    }
    let c = (c & u128::from(u32::MAX)) as u32;
    let Some(chain) = chain(cpu, c) else { return false };
    if !cpu.at_least(Cpu::M68020) {
        return true;
    }
    price.beats(cpu, chain, multiply(cpu))
}

/// Whether `x * c` of 64 bits is `(2^n +- 1) << k` with `n + k` at most 3: each
/// shift of the pair by one is `add.l` and `addx.l` (4 B), so the chain is 12 B
/// plus 4 B a shift, against the call's 26 B (`b * 3` at -O2: 38 B against 72 B
/// of partial products). The function trial keeps the multiply where the chain
/// loses.
fn wide_chain(c: u64) -> bool {
    let magnitude = if c >> 63 == 1 { c.wrapping_neg() } else { c };
    if magnitude == 0 {
        return false;
    }
    let zeros = magnitude.trailing_zeros();
    let odd = magnitude >> zeros;
    let bits = if (odd - 1).is_power_of_two() {
        odd - 1
    } else if (odd + 1).is_power_of_two() {
        odd + 1
    } else {
        return false;
    };
    bits.trailing_zeros() + zeros <= 3
}
