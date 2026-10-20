//! `foldSelectExtConst`: `select c, (ext x), C` as `ext (select c, x, C')`.
//!
//! Translation of
//! [`InstCombinerImpl::foldSelectExtConst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L2711)
//! at d1106deb71cc, with `getLosslessInvCast` (ConstantFolding.cpp:5017) for
//! the `zext` and `sext` cases.
//!
//! MIRROR: narrowed only when the extension has one use, the constant is the
//! same after truncation and extension back, and the extended type is `i1` or
//! the condition is an `icmp` or `trunc` of that type.
//!
//! DEVIATE 1: no vectors; only an integer constant narrows (a `poison` arm
//! never reaches here: `simplifySelectInst` takes it first).
//!
//! # Example
//! ```text
//! %e = zext i1 %x to i16                    %n = select i1 %c, i1 %x, i1 true
//! %r = select i1 %c, i16 %e, i16 1    ==>   %r = zext i1 %n to i16
//! ```

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{CastOp, Flags, Opcode};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{has_one_use, query};
use crate::combine_visit_select::Sel;

fn mask(bits: u32) -> u128 {
    if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 }
}

/// `getLosslessInvCast(C, small, ext)`: `C` in `small` bits, if `ext` of it is
/// `C` again.
fn lossless_inverse(
    value: u128,
    bits: u32,
    small: u32,
    ext: CastOp,
) -> Option<u128> {
    let narrow = value & mask(small);
    let back = match ext {
        CastOp::ZExt => narrow,
        _ if narrow >> (small - 1) & 1 == 1 => narrow | (mask(bits) & !mask(small)),
        _ => narrow,
    };
    (back == value).then_some(narrow)
}

pub(crate) fn select_ext_const(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    let q = query(c, None);
    // The constant arm and the instruction arm: either way round.
    let constant = |v: Operand| matches!(v, Operand::Constant(_));
    let (ext_value, constant_value) = if constant(sel.if_true) {
        (sel.if_false, sel.if_true)
    } else if constant(sel.if_false) {
        (sel.if_true, sel.if_false)
    } else {
        return None;
    };
    let ext = c.definition_of(ext_value)?;
    let extension = c.function.instruction(ext).clone();
    let op = match extension.opcode {
        Opcode::Cast(op @ (CastOp::ZExt | CastOp::SExt)) => op,
        _ => return None,
    };
    let x = extension.operands[0];
    let small_ty = c.type_of(x);
    let small = c.context.types.int_bits(small_ty)?;
    // A boolean, or a select that can be as wide as its condition's operands.
    if small != 1 {
        let cond = c.definition_of(sel.cond)?;
        let cond = c.function.instruction(cond);
        if !matches!(cond.opcode, Opcode::ICmp(_) | Opcode::Cast(CastOp::Trunc))
            || c.type_of(cond.operands[0]) != small_ty
        {
            return None;
        }
    }
    let (value, bits) = q.int_constant(constant_value)?;
    let narrow = lossless_inverse(value, bits, small, op)?;
    if !has_one_use(c, ext_value) {
        return None;
    }
    let narrow = c.int(small_ty, narrow);
    let arms = if ext_value == sel.if_true { [x, narrow] } else { [narrow, x] };
    let select = c.select_like(inst, inst, [sel.cond, arms[0], arms[1]], Flags::default());
    Some(c.replace_with_new(inst, Opcode::Cast(op), sel.ty, vec![select], Flags::default()))
}

#[cfg(test)]
#[path = "combine_select_ext_const_tests.rs"]
mod tests;
