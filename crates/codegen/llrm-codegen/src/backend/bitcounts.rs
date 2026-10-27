//! Expands `llvm.ctpop`, `llvm.ctlz` and `llvm.cttz` into shifts, masks and
//! adds in MIR before instruction selection, for the targets whose description
//! lists them under `expand`.
//!
//! # How it works
//! A count is made at a power-of-two width `W` of at least 8; a narrower
//! integer is zero-extended to it first and the answer truncated back.
//!
//! - `ctpop` is the parallel count: `x - ((x >> 1) & 0x55..)`, then pairs of
//!   two-bit sums by `0x33..`, nibble sums by `0x0f..`, then the byte sums
//!   folded by shifts (`x + (x >> 8)`, `x + (x >> 16)`, ...) and the low
//!   `log2(W) + 1` bits kept. No multiply.
//! - `ctlz` smears the highest set bit down (`x | x >> 1`, `| x >> 2`, `| x >>
//!   4`, ...) and counts the zeros of the result as `ctpop (~smeared)`; a zero
//!   gives `W`. A narrower type subtracts the bits it was extended by.
//! - `cttz` is `ctpop (~x & (x - 1))`; a zero gives `W`, and a narrower type
//!   sets the bit just above its width first so that a zero gives its width.
//!
//! The second argument of `ctlz` and `cttz` (a zero is poison) is not read: a
//! defined answer for a zero refines poison.
//!
//! ```text
//!  %n = call i16 @llvm.ctpop.i16(i16 %x)      ; shifts, ands and subs and adds
//!  %z = call i16 @llvm.ctlz.i16(i16 %x, i1 0) ; smear, then ctpop of the not
//!  %t = call i16 @llvm.cttz.i16(i16 %x, i1 0) ; ctpop of (~x & (x - 1))
//! ```
//!
//! # Example
//! `ctpop` of an `i8`:
//!
//! ```mir
//! define i8 @f(i8 %x) {
//!   %n = call i8 @llvm.ctpop.i8(i8 %x)
//!   ret i8 %n
//! }
//! ```
//!
//! After, in the order the instructions are made:
//!
//! ```mir
//! define i8 @f(i8 %x) {
//!   %1 = lshr i8 %x, 1
//!   %2 = and i8 %1, 85
//!   %3 = sub i8 %x, %2
//!   %4 = and i8 %3, 51
//!   %5 = lshr i8 %3, 2
//!   %6 = and i8 %5, 51
//!   %7 = add i8 %4, %6
//!   %8 = lshr i8 %7, 4
//!   %9 = add i8 %7, %8
//!   %10 = and i8 %9, 15
//!   %11 = and i8 %10, 15
//!   ret i8 %11
//! }
//! ```
//!
//! From unit test `a_population_count_is_shifts_and_masks` in `bitcounts.rs`.
//!
//! # Runs
//! Once per module in `driver::optimized`, before the odd widths are promoted,
//! for each of `ctpop`, `ctlz` and `cttz` the target description lists under
//! `expand` (m16 and m32 x86: no `popcnt`, and `bsr` and `bsf` are not selected
//! yet). A call of one the target does not list reaches selection, which
//! refuses it by name.
//!
//! # See also
//! LLVM's `TargetLowering::expandCTPOP`, `expandCTLZ` and `expandCTTZ`
//! ([TargetLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp)),
//! which this follows with the multiply-free `ctpop`.

use llrm_mir::context::{Context, mask};
use llrm_mir::intrinsics::{BitCount, Intrinsic};
use llrm_mir::module::{Function, InstId, Module, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Opcode};

use super::expansion::{self, Emitter};

/// The operation name a target lists under `expand` for `count`.
fn operation(count: BitCount) -> &'static str {
    match count {
        BitCount::LeadingZeros => "ctlz",
        BitCount::TrailingZeros => "cttz",
        BitCount::Population => "ctpop",
    }
}

/// Every call of an intrinsic `expands` names, in `module`, made shifts, masks
/// and adds.
pub fn expanded(
    module: &mut Module,
    expands: impl Fn(&str) -> bool,
) -> Result<(), String> {
    expansion::expand_calls(
        module,
        |intrinsic| match intrinsic {
            Intrinsic::BitCount(count) if expands(operation(count)) => Some(count),
            _ => None,
        },
        expand,
    )
}

/// What the bit counts add to an [`Emitter`]: the parallel count and its kin.
trait Counts {
    /// `x` with every byte of the computing width `byte`.
    fn splat(
        &self,
        byte: u128,
    ) -> u128;
    /// `TargetLowering::expandCTPOP`, without the multiply.
    fn population(
        &mut self,
        x: Operand,
    ) -> Result<Operand, String>;
    /// `TargetLowering::expandCTLZ`.
    fn leading_zeros(
        &mut self,
        x: Operand,
    ) -> Result<Operand, String>;
    /// `TargetLowering::expandCTTZ`.
    fn trailing_zeros(
        &mut self,
        x: Operand,
    ) -> Result<Operand, String>;
}

impl Counts for Emitter<'_> {
    /// `x` with every byte of the computing width `byte`.
    fn splat(
        &self,
        byte: u128,
    ) -> u128 {
        (0..self.bits / 8).fold(0, |all, at| all | byte << (8 * at))
    }

    /// `TargetLowering::expandCTPOP`, without the multiply.
    fn population(
        &mut self,
        x: Operand,
    ) -> Result<Operand, String> {
        // x - ((x >> 1) & 0x55..): each pair of bits holds its count.
        let half = self.shift_right(x, 1)?;
        let half = self.mask(half, self.splat(0x55))?;
        let x = self.binary(BinaryOp::Sub, x, half)?;
        // Pairs added into nibbles.
        let low = self.mask(x, self.splat(0x33))?;
        let high = self.shift_right(x, 2)?;
        let high = self.mask(high, self.splat(0x33))?;
        let x = self.binary(BinaryOp::Add, low, high)?;
        // Nibbles added into bytes.
        let nibbles = self.shift_right(x, 4)?;
        let x = self.binary(BinaryOp::Add, x, nibbles)?;
        let mut x = self.mask(x, self.splat(0x0f))?;
        // Bytes folded into the low byte, and the count's bits kept.
        let mut at = 8;
        while at < self.bits {
            let folded = self.shift_right(x, at)?;
            x = self.binary(BinaryOp::Add, x, folded)?;
            at *= 2;
        }
        self.mask(x, mask(self.bits.ilog2() + 1))
    }

    /// `TargetLowering::expandCTLZ`: the zeros above the highest set bit are
    /// the zeros of what smearing that bit down leaves.
    fn leading_zeros(
        &mut self,
        x: Operand,
    ) -> Result<Operand, String> {
        let mut smeared = x;
        let mut at = 1;
        while at < self.bits {
            let down = self.shift_right(smeared, at)?;
            smeared = self.binary(BinaryOp::Or, smeared, down)?;
            at *= 2;
        }
        let zeros = self.binary(BinaryOp::Xor, smeared, self.constant(u128::MAX))?;
        self.population(zeros)
    }

    /// `TargetLowering::expandCTTZ`: the ones of `~x & (x - 1)` are the zeros
    /// below the lowest set bit.
    fn trailing_zeros(
        &mut self,
        x: Operand,
    ) -> Result<Operand, String> {
        let not = self.binary(BinaryOp::Xor, x, self.constant(u128::MAX))?;
        let below = self.binary(BinaryOp::Sub, x, self.constant(1))?;
        let ones = self.binary(BinaryOp::And, not, below)?;
        self.population(ones)
    }
}

/// The call `inst` of `count`'s intrinsic, made shifts, masks and adds.
fn expand(
    context: &mut Context,
    function: &mut Function,
    inst: InstId,
    count: BitCount,
) -> Result<(), String> {
    let call = function.instruction(inst).clone();
    let result = call.result.ok_or("a bit count's value")?;
    let x = *call.operands.first().ok_or("a bit count of one operand")?;
    let bits = context.types.int_bits(call.ty).ok_or("a bit count of an integer")?;
    // A power of two of at least 8 bits: the narrower value is extended to it.
    let wide = bits.next_power_of_two().max(8);
    let ty = context.types.int(wide);
    let mut emit = Emitter { context: &*context, function: &mut *function, at: inst, ty, bits: wide };
    let x = if wide == bits { x } else { emit.emit(Opcode::Cast(CastOp::ZExt), ty, vec![x])? };
    let counted = match count {
        BitCount::Population => emit.population(x)?,
        BitCount::LeadingZeros => {
            let zeros = emit.leading_zeros(x)?;
            // The extension's zeros are not the value's.
            emit.binary(BinaryOp::Sub, zeros, emit.constant(u128::from(wide - bits)))?
        }
        BitCount::TrailingZeros => {
            // A zero must give `bits`: the bit above the value stops the count
            // there.
            let x = if wide == bits { x } else { emit.binary(BinaryOp::Or, x, emit.constant(1 << bits))? };
            emit.trailing_zeros(x)?
        }
    };
    let answer = if wide == bits { counted } else { emit.emit(Opcode::Cast(CastOp::Trunc), call.ty, vec![counted])? };
    function.replace_all_uses_with(result, answer);
    function.erase(inst)
}

#[cfg(test)]
#[path = "bitcounts_tests.rs"]
mod tests;
