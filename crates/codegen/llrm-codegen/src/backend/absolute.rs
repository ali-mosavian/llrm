//! Expands `llvm.abs` into a sign mask, an xor and a subtract in MIR before
//! instruction selection, for the targets whose description lists `abs` under
//! `expand`.
//!
//! # How it works
//! With `s = x >> (w - 1)` (arithmetic), `abs(x) = (x ^ s) - s`; `x` is
//! frozen first, as it is read twice. The minimum value maps to itself, which
//! is also what the poison flag allows.
//!
//! # Example
//! ```text
//! %r = call i16 @llvm.abs.i16(i16 %x, i1 false)
//! ==> %f = freeze %x ; %s = ashr %f, 15 ; %v = xor %f, %s ; %r = sub %v, %s
//! ```
//!
//! # Runs
//! Once per module in `driver::optimized`, for a target that lists `abs` under
//! `expand`.
//!
//! DEVIATE 1: only the shift form of `expandABS`: the `smax`/`umin` forms are
//! taken where those are legal, and here they are the select lowering's.
//!
//! # See also
//! LLVM's `TargetLowering::expandABS`
//! ([TargetLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L11308)).

use llrm_mir::context::Context;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Function, InstId, Module};
use llrm_mir::opcode::{BinaryOp, Opcode};

use super::expansion::{self, Emitter};

/// Every call of `llvm.abs` in `module`, if `expands("abs")`, made a sign mask,
/// an xor and a subtract.
pub fn expanded(
    module: &mut Module,
    expands: impl Fn(&str) -> bool,
) -> Result<(), String> {
    if !expands("abs") {
        return Ok(());
    }
    expansion::expand_calls(module, |intrinsic| matches!(intrinsic, Intrinsic::Abs).then_some(()), expand)
}

/// The call `inst` of `llvm.abs`.
fn expand(
    context: &mut Context,
    function: &mut Function,
    inst: InstId,
    (): (),
) -> Result<(), String> {
    let call = function.instruction(inst).clone();
    let result = call.result.ok_or("an abs value")?;
    let x = *call.operands.first().ok_or("an abs of one operand")?;
    let bits = context.types.int_bits(call.ty).ok_or("an abs of an integer")?;
    let mut emit = Emitter { context: &*context, function: &mut *function, at: inst, ty: call.ty, bits };
    let frozen = emit.emit(Opcode::Freeze, call.ty, vec![x])?;
    let sign = emit.shift_right_arithmetic(frozen, bits - 1)?;
    let flipped = emit.binary(BinaryOp::Xor, frozen, sign)?;
    let absolute = emit.binary(BinaryOp::Sub, flipped, sign)?;
    function.replace_all_uses_with(result, absolute);
    function.erase(inst)
}

#[cfg(test)]
#[path = "absolute_tests.rs"]
mod tests;
