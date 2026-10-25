//! Expands `llvm.scmp` and `llvm.ucmp` into two compares and a subtract in MIR
//! before instruction selection, for the targets whose description lists them
//! under `expand`.
//!
//! # How it works
//! The three-way compare is `(a > b) - (a < b)`, each side a compare widened
//! to the result's integer type: -1, 0 or 1 at that width.
//!
//! ```text
//!  %r = call i8 @llvm.scmp.i8.i16(i16 %a, i16 %b)
//!        ; zext (a > b) - zext (a < b), in i8
//! ```
//!
//! # Example
//! `ucmp` of two `i16` into an `i8`:
//!
//! ```mir
//! define i8 @f(i16 %a, i16 %b) {
//!   %r = call i8 @llvm.ucmp.i8.i16(i16 %a, i16 %b)
//!   ret i8 %r
//! }
//! ```
//!
//! After:
//!
//! ```mir
//! define i8 @f(i16 %a, i16 %b) {
//!   %1 = icmp ugt i16 %a, %b
//!   %2 = zext i1 %1 to i8
//!   %3 = icmp ult i16 %a, %b
//!   %4 = zext i1 %3 to i8
//!   %5 = sub i8 %2, %4
//!   ret i8 %5
//! }
//! ```
//!
//! From unit test `a_three_way_compare_is_two_compares_and_a_subtract` in
//! `compares.rs`.
//!
//! # Runs
//! Once per module in `driver::optimized`, with the bit counts, for a target
//! that lists `scmp` or `ucmp` under `expand`. A call on a target that does not
//! list it reaches selection, which refuses it by name.
//!
//! # See also
//! LLVM's `TargetLowering::expandCMP`
//! ([TargetLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp)).

use llrm_mir::context::Context;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Function, InstId, Module};
use llrm_mir::opcode::{BinaryOp, CastOp, IntPredicate, Opcode};

use super::expansion::{self, Emitter};

/// The operation name a target lists under `expand` for a compare.
fn operation(signed: bool) -> &'static str {
    if signed { "scmp" } else { "ucmp" }
}

/// Every call of `llvm.scmp` or `llvm.ucmp` in `module` that `expands` names,
/// made two compares and a subtract.
pub fn expanded(
    module: &mut Module,
    expands: impl Fn(&str) -> bool,
) -> Result<(), String> {
    expansion::expand_calls(
        module,
        |intrinsic| match intrinsic {
            Intrinsic::Compare { signed } if expands(operation(signed)) => Some(signed),
            _ => None,
        },
        expand,
    )
}

/// The call `inst` of a three-way compare, made `(a > b) - (a < b)`.
fn expand(
    context: &mut Context,
    function: &mut Function,
    inst: InstId,
    signed: bool,
) -> Result<(), String> {
    let call = function.instruction(inst).clone();
    let result = call.result.ok_or("a compare's value")?;
    let [a, b] = call.operands[..2] else { return Err("a compare of two operands".to_owned()) };
    let bits = context.types.int_bits(call.ty).ok_or("a compare into an integer")?;
    let flag = context.types.int(1);
    let (greater, less) =
        if signed { (IntPredicate::Sgt, IntPredicate::Slt) } else { (IntPredicate::Ugt, IntPredicate::Ult) };
    let mut emit = Emitter { context: &*context, function: &mut *function, at: inst, ty: call.ty, bits };
    let above = emit.emit(Opcode::ICmp(greater), flag, vec![a, b])?;
    let above = emit.emit(Opcode::Cast(CastOp::ZExt), call.ty, vec![above])?;
    let below = emit.emit(Opcode::ICmp(less), flag, vec![a, b])?;
    let below = emit.emit(Opcode::Cast(CastOp::ZExt), call.ty, vec![below])?;
    let answer = emit.binary(BinaryOp::Sub, above, below)?;
    function.replace_all_uses_with(result, answer);
    function.erase(inst)
}

#[cfg(test)]
#[path = "compares_tests.rs"]
mod tests;
