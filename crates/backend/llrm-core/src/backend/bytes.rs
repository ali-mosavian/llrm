//! Expands `llvm.bswap` into shifts, masks and ors in MIR before instruction
//! selection, for the targets whose description lists `bswap` under `expand`.
//!
//! # How it works
//! Byte `i` of an `n`-byte integer moves to byte `n - 1 - i`: a left shift by
//! `(n - 1 - 2i)` bytes when that is positive, a right shift by its negation
//! otherwise, then a mask of the byte's new place. The bytes are or-ed. Two
//! shifts and masks, an or, per pair of bytes: linear in the width, and no
//! multiply.
//!
//! ```text
//!  %r = call i16 @llvm.bswap.i16(i16 %x)    ; (x << 8) | (x >> 8)
//! ```
//!
//! # Example
//! `bswap` of an `i16`:
//!
//! ```mir
//! define i16 @f(i16 %x) {
//!   %r = call i16 @llvm.bswap.i16(i16 %x)
//!   ret i16 %r
//! }
//! ```
//!
//! After, in the order the instructions are made:
//!
//! ```mir
//! define i16 @f(i16 %x) {
//!   %1 = shl i16 %x, 8
//!   %2 = and i16 %1, -256
//!   %3 = lshr i16 %x, 8
//!   %4 = and i16 %3, 255
//!   %5 = or i16 %2, %4
//!   ret i16 %5
//! }
//! ```
//!
//! From unit test `a_byte_swap_is_shifts_masks_and_an_or` in `bytes.rs`.
//!
//! # Runs
//! Once per module in `driver::optimized`, with the bit counts, for a target
//! that lists `bswap` under `expand` (m16 and m32 x86: no `bswap`/`xchg` forms
//! are selected for it yet). A call on a target that does not list it reaches
//! selection, which refuses it by name.
//!
//! # See also
//! LLVM's `TargetLowering::expandBSWAP`
//! ([TargetLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp)),
//! which does the same with fixed forms for `i16`, `i32` and `i64`.

use llrm_mir::context::Context;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Function, InstId, Module, Operand};
use llrm_mir::opcode::BinaryOp;

use super::expansion::{self, Emitter};

/// Every call of `llvm.bswap` in `module`, if `expands("bswap")`, made shifts,
/// masks and ors.
pub fn expanded(
    module: &mut Module,
    expands: impl Fn(&str) -> bool,
) -> Result<(), String> {
    if !expands("bswap") {
        return Ok(());
    }
    expansion::expand_calls(module, |intrinsic| matches!(intrinsic, Intrinsic::ByteSwap).then_some(()), expand)
}

/// Every call of `llvm.bitreverse` in `module`, if `expands("bitreverse")`:
/// the bytes swapped, then the nibbles, the bit pairs and the bits of each
/// byte (`TargetLowering::expandBITREVERSE`). The old selector's input; the
/// DAG route legalizes the node.
pub fn bits_reversed(
    module: &mut Module,
    expands: impl Fn(&str) -> bool,
) -> Result<(), String> {
    if !expands("bitreverse") {
        return Ok(());
    }
    expansion::expand_calls(module, |intrinsic| matches!(intrinsic, Intrinsic::BitReverse).then_some(()), reverse)
}

/// The call `inst` of `llvm.bitreverse`.
fn reverse(
    context: &mut Context,
    function: &mut Function,
    inst: InstId,
    (): (),
) -> Result<(), String> {
    let call = function.instruction(inst).clone();
    let result = call.result.ok_or("a bit reversal's value")?;
    let x = *call.operands.first().ok_or("a bit reversal of one operand")?;
    let bits = context.types.int_bits(call.ty).ok_or("a bit reversal of an integer")?;
    if bits < 8 || !bits.is_power_of_two() {
        return Err(format!("llvm.bitreverse.i{bits}: only a power-of-two width of 8 bits or more is expanded"));
    }
    let mut emit = Emitter { context: &*context, function: &mut *function, at: inst, ty: call.ty, bits };
    let mut value = if bits > 8 { byte_swap(&mut emit, x, bits / 8)? } else { x };
    for (by, byte) in [(4, 0x0F), (2, 0x33), (1, 0x55)] {
        let splat = (0..bits / 8).fold(0u128, |all, i| all | byte << (8 * i));
        let high = emit.shift_right(value, by)?;
        let high = emit.mask(high, splat)?;
        let low = emit.mask(value, splat)?;
        let low = emit.shift_left(low, by)?;
        value = emit.binary(BinaryOp::Or, high, low)?;
    }
    function.replace_all_uses_with(result, value);
    function.erase(inst)
}

/// The call `inst` of `llvm.bswap`, made shifts, masks and ors.
fn expand(
    context: &mut Context,
    function: &mut Function,
    inst: InstId,
    (): (),
) -> Result<(), String> {
    let call = function.instruction(inst).clone();
    let result = call.result.ok_or("a byte swap's value")?;
    let x = *call.operands.first().ok_or("a byte swap of one operand")?;
    let bits = context.types.int_bits(call.ty).ok_or("a byte swap of an integer")?;
    let bytes = bits / 8;
    let mut emit = Emitter { context: &*context, function: &mut *function, at: inst, ty: call.ty, bits };
    let swapped = byte_swap(&mut emit, x, bytes)?;
    function.replace_all_uses_with(result, swapped);
    function.erase(inst)
}

/// `x` with its `bytes` bytes in the opposite order.
fn byte_swap(
    emit: &mut Emitter,
    x: Operand,
    bytes: u32,
) -> Result<Operand, String> {
    let mut swapped = None;
    for byte in 0..bytes {
        // Byte `byte` goes to `bytes - 1 - byte`.
        let moved = i64::from(bytes) - 1 - 2 * i64::from(byte);
        let shifted = match moved {
            0 => x,
            moved if moved > 0 => emit.shift_left(x, 8 * moved as u32)?,
            moved => emit.shift_right(x, 8 * (-moved) as u32)?,
        };
        let place = llrm_mir::context::mask(8) << (8 * (bytes - 1 - byte));
        let placed = emit.mask(shifted, place)?;
        swapped = Some(match swapped {
            None => placed,
            Some(so_far) => emit.binary(BinaryOp::Or, so_far, placed)?,
        });
    }
    swapped.ok_or_else(|| "a byte swap of no bytes".to_owned())
}

#[cfg(test)]
#[path = "bytes_tests.rs"]
mod tests;
