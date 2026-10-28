//! Expands `llvm.fshl` and `llvm.fshr` into shifts and an or in MIR before
//! instruction selection, for the targets whose description lists `fshl` or
//! `fshr` under `expand`.
//!
//! # How it works
//! A constant amount is taken modulo the width: zero keeps the half that is not
//! shifted, any other `c` is `x << c | y >> (w - c)` (`fshl`) or
//! `x << (w - c) | y >> c` (`fshr`). A variable amount cannot shift by the
//! width, so the second operand is shifted by one first and then by the
//! complement: `x << s | (y >> 1) >> (~z & (w - 1))` with `s = z & (w - 1)`
//! (`urem` and `sub` where the width is not a power of two).
//!
//! # Example
//! `fshl` of an `i16` by a variable amount:
//!
//! ```text
//! %r = call i16 @llvm.fshl.i16(i16 %x, i16 %y, i16 %z)
//! ==> %s = and %z, 15 ; %n = xor %z, -1 ; %i = and %n, 15
//!     %a = shl %x, %s ; %b = lshr %y, 1 ; %c = lshr %b, %i ; %r = or %a, %c
//! ```
//!
//! # Runs
//! Once per module in `driver::optimized`, for a target that lists `fshl` or
//! `fshr` under `expand`; a call on one that does not reaches selection, which
//! refuses it by name.
//!
//! DEVIATE 1: the `isNonZeroModBitWidthOrUndef` two-shift form for a variable
//! amount known to be non-zero modulo the width is not made: the general form
//! is always right.
//!
//! # See also
//! LLVM's `TargetLowering::expandFunnelShift`
//! ([TargetLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L8697)).

use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Function, InstId, Module, Operand};
use llrm_mir::opcode::BinaryOp;

use super::expansion::{self, Emitter};

/// Every call of `llvm.fshl` and `llvm.fshr` in `module`, if the target lists
/// it, made shifts and an or.
pub fn expanded(
    module: &mut Module,
    expands: impl Fn(&str) -> bool,
    kept: impl Fn(u32) -> bool,
) -> Result<(), String> {
    let (left, right) = (expands("fshl"), expands("fshr"));
    expansion::expand_calls(
        module,
        |intrinsic| match intrinsic {
            Intrinsic::FunnelShift { left: true } if left => Some(true),
            Intrinsic::FunnelShift { left: false } if right => Some(false),
            _ => None,
        },
        |context, function, inst, left| {
            let bits = context.types.int_bits(function.instruction(inst).ty);
            if bits.is_some_and(&kept) {
                return Ok(());
            }
            expand(context, function, inst, left)
        },
    )
}

/// Whether the selector takes a funnel shift of `bits` as it is: the DAG
/// lowers a funnel shift of a byte, word or dword itself (`LowerFunnelShift`,
/// `SHLD`/`SHRD`), as the pin does.
pub fn kept_for_isel(
    isel: crate::backend::shadow::Selector,
    ty: &str,
) -> bool {
    isel == crate::backend::shadow::Selector::Dag && matches!(ty, "i8" | "i16" | "i32")
}

/// The integer constant `operand` is.
fn constant(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(bits) => Some(bits),
        ConstantKind::Zero | ConstantKind::Null => Some(0),
        _ => None,
    }
}

/// The call `inst` of `llvm.fshl` (`left`) or `llvm.fshr`, made shifts and an
/// or.
fn expand(
    context: &mut Context,
    function: &mut Function,
    inst: InstId,
    left: bool,
) -> Result<(), String> {
    let call = function.instruction(inst).clone();
    let result = call.result.ok_or("a funnel shift's value")?;
    let [x, y, z, _] = call.operands[..] else { return Err("a funnel shift of three operands".into()) };
    let bits = context.types.int_bits(call.ty).ok_or("a funnel shift of an integer")?;
    let amount = constant(context, z);
    let mut emit = Emitter { context: &*context, function: &mut *function, at: inst, ty: call.ty, bits };
    let found = match amount {
        Some(by) => match (by % u128::from(bits)) as u32 {
            0 => {
                if left {
                    x
                } else {
                    y
                }
            }
            by => {
                let (lower, upper) = (by, bits - by);
                let high = emit.shift_left(x, if left { lower } else { upper })?;
                let low = emit.shift_right(y, if left { upper } else { lower })?;
                emit.binary(BinaryOp::Or, high, low)?
            }
        },
        None => {
            let (shift, inverse) = if bits.is_power_of_two() {
                let mask = emit.constant(u128::from(bits - 1));
                let ones = emit.constant(u128::MAX);
                let inverted = emit.binary(BinaryOp::Xor, z, ones)?;
                (emit.binary(BinaryOp::And, z, mask)?, emit.binary(BinaryOp::And, inverted, mask)?)
            } else {
                let width = emit.constant(u128::from(bits));
                let last = emit.constant(u128::from(bits - 1));
                let shift = emit.binary(BinaryOp::URem, z, width)?;
                (shift, emit.binary(BinaryOp::Sub, last, shift)?)
            };
            let one = emit.constant(1);
            let (high, low) = if left {
                let half = emit.binary(BinaryOp::LShr, y, one)?;
                (emit.binary(BinaryOp::Shl, x, shift)?, emit.binary(BinaryOp::LShr, half, inverse)?)
            } else {
                let half = emit.binary(BinaryOp::Shl, x, one)?;
                (emit.binary(BinaryOp::Shl, half, inverse)?, emit.binary(BinaryOp::LShr, y, shift)?)
            };
            emit.binary(BinaryOp::Or, high, low)?
        }
    };
    function.replace_all_uses_with(result, found);
    function.erase(inst)
}

#[cfg(test)]
#[path = "funnel_tests.rs"]
mod tests;
