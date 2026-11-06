//! Expands `llvm.fmuladd` into an `fmul` and an `fadd` in MIR before the old
//! instruction selector, which has no fused multiply-add to select.
//!
//! # How it works
//! `fmuladd(a, b, c)` is `fadd (fmul a, b), c`: the unfused form
//! `SelectionDAGBuilder` takes when the target has no faster FMA.
//!
//! # Example
//! ```text
//! %r = call float @llvm.fmuladd.f32(float %a, float %b, float %c)
//! ==> %p = fmul %a, %b ; %r = fadd %p, %c
//! ```
//!
//! # Runs
//! Once per module in `driver::optimized`, unless the selector is the DAG,
//! which lowers `fmuladd` itself.
//!
//! DEVIATE 1: the new instructions carry no fast-math flags; the call's
//! `contract`/`fast` bits are dropped. Exit: none (the old selector is the
//! only user).
//!
//! # See also
//! LLVM's `SelectionDAGBuilder::visitIntrinsicCall`, `Intrinsic::fmuladd`
//! ([SelectionDAGBuilder.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp)).

use llrm_mir::context::Context;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Function, InstId, Module};
use llrm_mir::opcode::BinaryOp;

use super::expansion::{self, Emitter};

/// Every call of `llvm.fmuladd` in `module` made an `fmul` and an `fadd`.
pub fn expanded(module: &mut Module) -> Result<(), String> {
    expansion::expand_calls(module, |intrinsic| matches!(intrinsic, Intrinsic::FMulAdd).then_some(()), expand)
}

/// The call `inst` of `llvm.fmuladd`.
fn expand(
    context: &mut Context,
    function: &mut Function,
    inst: InstId,
    (): (),
) -> Result<(), String> {
    let call = function.instruction(inst).clone();
    let result = call.result.ok_or("an fmuladd value")?;
    let [a, b, c, ..] = call.operands[..] else { return Err("an fmuladd of three operands".into()) };
    let mut emit = Emitter { context: &*context, function: &mut *function, at: inst, ty: call.ty, bits: 0 };
    let product = emit.binary(BinaryOp::FMul, a, b)?;
    let sum = emit.binary(BinaryOp::FAdd, product, c)?;
    function.replace_all_uses_with(result, sum);
    function.erase(inst)
}

#[cfg(test)]
#[path = "fmuladd_tests.rs"]
mod tests;
