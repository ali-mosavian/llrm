//! `reassociateForUses`: `(X op Y) op Z` with `X` or `Y` used elsewhere is
//! rebuilt so the one-use values share an instruction.
//!
//! Translation of
//! [`reassociateForUses`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L2195),
//! called by `visitAnd`, `visitOr` and `visitXor`.
//!
//! # Example
//! `down` is read again elsewhere, the two compares are not:
//!
//! ```text
//! %c = and i1 %down, %sign_ne
//! %r = and i1 %c, %mask_ne        ==>   %t = and i1 %sign_ne, %mask_ne ; %r = and i1 %t, %down
//! ```
//! The compares now meet in one `and`, which the backend can chain with `ccmp`.

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{has_one_use, query};

/// `inst` is `(X op Y) op Z`, the inner `op` and `Z` one-use and no operand a
/// constant, with `X` or `Y` used elsewhere: `(Y op Z) op X` or `(X op Z) op
/// Y`.
pub(crate) fn reassociate_for_uses(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let Opcode::Binary(op) = c.function.instruction(inst).opcode else { return None };
    let operands = c.function.instruction(inst).operands.clone();
    let [lhs, rhs] = operands[..] else { return None };
    // `m_c_BinOp(Opcode, m_OneUse(m_BinOp(Opcode, X, Y)), m_OneUse(Z))`: either
    // order.
    let inner_of = |c: &Combiner, value: Operand| -> Option<(Operand, Operand)> {
        if !has_one_use(c, value) {
            return None;
        }
        let q = query(c, Some(inst));
        let (_, instruction) = q.instruction_of(value)?;
        (instruction.opcode == Opcode::Binary(op)).then(|| (instruction.operands[0], instruction.operands[1]))
    };
    let (x, y, z) = if let Some((x, y)) = inner_of(c, lhs).filter(|_| has_one_use(c, rhs)) {
        (x, y, rhs)
    } else if let Some((x, y)) = inner_of(c, rhs).filter(|_| has_one_use(c, lhs)) {
        (x, y, lhs)
    } else {
        return None;
    };
    let constant = |v: Operand| !matches!(v, Operand::Value(_));
    if constant(x) || constant(y) || constant(z) {
        return None;
    }
    let default = Flags::default();
    // (X op Y) op Z --> (Y op Z) op X, else (X op Z) op Y
    let (pair, last) = if !has_one_use(c, x) {
        (c.create_binop(inst, op, y, z, default), x)
    } else if !has_one_use(c, y) {
        (c.create_binop(inst, op, x, z, default), y)
    } else {
        return None;
    };
    Some(c.replace_with_binop(inst, op, pair, last, default))
}

#[cfg(test)]
#[path = "combine_reassociate_for_uses_tests.rs"]
mod tests;
