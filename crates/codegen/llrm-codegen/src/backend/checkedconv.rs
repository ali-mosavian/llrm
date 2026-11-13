//! Expands `llvm.experimental.fp{to}{s,u}i.checked`, a float taken to an
//! integer together with whether it fits, before instruction selection.
//!
//! # How it works
//! Where the target converts into an integer wider than the result
//! (`Tti::wide_float_to_int`: the x87's `fistp` stores a dword or a qword, and
//! NaN and overflow come out as that width's most negative value), the float is
//! converted at that width and the check is made on the integer: the result
//! fits when sign-extending its low part gives the wide value back (unsigned:
//! when the wide value is below `2^bits`). No float bound is read, so the pool
//! holds none and no `fcom` runs. Elsewhere the check is two float compares
//! against the integer's range, as C's `-fsanitize=float-cast-overflow` and
//! the Nib frontend made them before this intrinsic.
//!
//! # Example
//! ```text
//! %p = call { i32, i1 } @llvm.experimental.fptosi.checked.i32.f64(double %x)
//! ==> %w = fptosi double %x to i64 ; %v = trunc %w to i32
//!     %hi = trunc (lshr %w, 32) to i32 ; %s = ashr %v, 31 ; %ok = icmp eq %hi, %s
//! ```
//!
//! # Runs
//! Once per module in `driver::optimized`, after the mid-end: the intrinsic is
//! opaque to it, and `constantfolding` folds a constant argument.
//!
//! DEVIATE FPTOINT-CHECKED (permanent): LLVM has no checked float-to-integer
//! (`llvm.fptosi.sat` saturates, and a float past the range is poison in
//! `fptosi`); the structure is `X86TargetLowering::LowerFP_TO_INT_SAT`, which
//! converts at a width the hardware has and decides on the integer
//! ([X86ISelLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/X86/X86ISelLowering.cpp)).
//! The pin clamps the float first, which needs a float min/max the x87 lacks,
//! so the check here is on the integer. The trap stays the caller's: the
//! intrinsic returns `{ value, fits }`. Reopen when a Nib SSE or m64 target,
//! or another frontend, emits the intrinsic: `Tti::wide_float_to_int` is
//! x87-only because no program there could price an SSE form.

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Function, InstId, Module, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, FloatPredicate, IntPredicate, Opcode};
use llrm_mir::types::{FloatKind, Type, TypeId};

use super::expansion;

/// Every call of the checked conversion in `module`, made the integer and the
/// flag; `wide` is `Tti::wide_float_to_int`.
pub fn expanded(
    module: &mut Module,
    wide: impl Fn(u32) -> Option<u32>,
) -> Result<(), String> {
    expansion::expand_calls(
        module,
        |intrinsic| match intrinsic {
            Intrinsic::FpToIntChecked { signed } => Some(signed),
            _ => None,
        },
        |context, function, inst, signed| expand(context, function, inst, signed, &wide),
    )
}

/// An instruction made before `at`.
fn make(
    function: &mut Function,
    at: InstId,
    opcode: Opcode,
    ty: TypeId,
    operands: Vec<Operand>,
) -> Result<Operand, String> {
    let made = function.create_instruction(opcode, ty, operands, Flags::default(), None);
    function.insert(made, Position::Before(at))?;
    Ok(Operand::Value(function.instruction(made).result.expect("a value")))
}

fn expand(
    context: &mut Context,
    function: &mut Function,
    inst: InstId,
    signed: bool,
    wide: &impl Fn(u32) -> Option<u32>,
) -> Result<(), String> {
    let call = function.instruction(inst).clone();
    let pair = call.result.ok_or("a checked conversion with no result")?;
    let x = *call.operands.first().ok_or("a checked conversion of one operand")?;
    let from = function.operand_type(context, x).ok_or("a checked conversion of a typed operand")?;
    let fields = context.types.fields(call.ty).ok_or("a checked conversion returns a pair")?.to_vec();
    let [value_ty, flag_ty] = fields[..] else { return Err("a checked conversion returns a pair".into()) };
    let bits = context.types.int_bits(value_ty).ok_or("a checked conversion to an integer")?;
    let (value, fits) = match wide(bits).filter(|wide| *wide > bits) {
        Some(wide) => {
            let wide_ty = context.types.int(wide);
            let whole = make(function, inst, Opcode::Cast(CastOp::FPToSI), wide_ty, vec![x])?;
            let value = make(function, inst, Opcode::Cast(CastOp::Trunc), value_ty, vec![whole])?;
            let fits = if signed && wide == 2 * bits {
                // the high half is the low half's sign
                let shift = Operand::Constant(context.int(wide_ty, i128::from(bits)));
                let high = make(function, inst, Opcode::Binary(BinaryOp::LShr), wide_ty, vec![whole, shift])?;
                let high = make(function, inst, Opcode::Cast(CastOp::Trunc), value_ty, vec![high])?;
                let sign = Operand::Constant(context.int(value_ty, i128::from(bits - 1)));
                let sign = make(function, inst, Opcode::Binary(BinaryOp::AShr), value_ty, vec![value, sign])?;
                make(function, inst, Opcode::ICmp(IntPredicate::Eq), flag_ty, vec![high, sign])?
            } else if signed {
                let again = make(function, inst, Opcode::Cast(CastOp::SExt), wide_ty, vec![value])?;
                make(function, inst, Opcode::ICmp(IntPredicate::Eq), flag_ty, vec![again, whole])?
            } else {
                let limit = Operand::Constant(context.int(wide_ty, 1i128 << bits));
                make(function, inst, Opcode::ICmp(IntPredicate::Ult), flag_ty, vec![whole, limit])?
            };
            (value, fits)
        }
        None => {
            let cast = if signed { CastOp::FPToSI } else { CastOp::FPToUI };
            let value = make(function, inst, Opcode::Cast(cast), value_ty, vec![x])?;
            let (minimum, maximum) =
                if signed { (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1) } else { (0, (1i128 << bits) - 1) };
            // An f32 holds no value strictly between -2^31 - 1 and -2^31.
            let (lower, strict) =
                if matches!(context.types.get(from), Type::Float(FloatKind::Float)) && bits == 32 && signed {
                    (minimum, false)
                } else {
                    (minimum - 1, true)
                };
            let [lower, upper] = [lower, maximum + 1].map(|bound| float_constant(context, from, bound));
            let [lower, upper] = [lower?, upper?];
            let above = make(
                function,
                inst,
                Opcode::FCmp(if strict { FloatPredicate::Ogt } else { FloatPredicate::Oge }),
                flag_ty,
                vec![x, lower],
            )?;
            let below = make(function, inst, Opcode::FCmp(FloatPredicate::Olt), flag_ty, vec![x, upper])?;
            let fits = make(function, inst, Opcode::Binary(BinaryOp::And), flag_ty, vec![above, below])?;
            (value, fits)
        }
    };
    let users: Vec<InstId> = function.users(pair).into_iter().map(|one| one.user).collect();
    for user in users {
        let Opcode::ExtractValue(path) = function.instruction(user).opcode.clone() else {
            return Err("a checked conversion read other than by extractvalue".into());
        };
        let result = function.instruction(user).result.ok_or("an extractvalue with no result")?;
        function.replace_all_uses_with(result, if path == [0] { value } else { fits });
        function.erase(user)?;
    }
    function.erase(inst)
}

/// The float of type `ty` that is `value`: a power of two or one less, exact
/// at the width of the integers it bounds.
fn float_constant(
    context: &Context,
    ty: TypeId,
    value: i128,
) -> Result<Operand, String> {
    let kind = match context.types.get(ty) {
        Type::Float(FloatKind::Float) => ConstantKind::Float(u64::from((value as f32).to_bits())),
        Type::Float(FloatKind::Double) => ConstantKind::Float((value as f64).to_bits()),
        other => return Err(format!("a checked conversion of {other:?}")),
    };
    Ok(Operand::Constant(context.constant(Constant { ty, kind })))
}

#[cfg(test)]
#[path = "checkedconv_tests.rs"]
mod tests;
