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
//!   %0 = shl i16 %x, 8
//!   %1 = and i16 %0, -256
//!   %2 = lshr i16 %x, 8
//!   %3 = and i16 %2, 255
//!   %4 = or i16 %1, %3
//!   ret i16 %4
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
use llrm_mir::module::{Function, InstId, Module};
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
    function.replace_all_uses_with(result, swapped.ok_or("a byte swap of no bytes")?);
    function.erase(inst)
}

#[cfg(test)]
#[path = "bytes_tests.rs"]
mod tests;
