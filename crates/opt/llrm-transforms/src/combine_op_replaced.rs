//! `simplifyAndOrWithOpReplaced`: an `and`/`or` of `Op` and an expression of
//! `Op` that is simpler when `Op` is the identity of the operation.
//!
//! Translation of
//! [`simplifyAndOrWithOpReplaced`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L2371)
//! and its two callers in `visitAnd` (:2960) and `visitOr` (:4776) at
//! d1106deb71cc.
//!
//! MIRROR: in `or X, E(X)` every `X` inside the tree of `and`/`or`/`xor` that
//! makes `E` (three levels, one use each to be rewritten) reads `0`: the `or`
//! sets those bits already. For `and` it reads all ones. A subtree that
//! another user reads is only simplified, never rebuilt.
//!
//! # Example
//! ```text
//! %a = xor i8 %x, %y                      %o = or i8 %x, %y
//! %o = or i8 %x, %a            ==>
//! ```
//!
//! # Used by
//! `visit_or` and `visit_and`.

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};

use crate::combine::{Combiner, Visit, defining};
use crate::combine_shared::{parts, simplify_binop};

/// The deepest tree of logic operations looked through.
const MAX_DEPTH: u32 = 3;

/// The two calls of `visitAnd` (`op` is `And`) and `visitOr` (`Or`): `inst` is
/// `op op0, op1`.
pub(crate) fn fold_and_or_with_op_replaced(
    c: &mut Combiner,
    inst: InstId,
    op: BinaryOp,
) -> Option<Visit> {
    let (op0, op1, _) = parts(c, inst);
    let ty = c.type_of(op0);
    let identity = if op == BinaryOp::And { c.all_ones(ty) } else { c.int(ty, 0) };
    if let Some(v) = replaced(c, inst, op0, op1, identity, false, 0) {
        return Some(c.replace_with_binop(inst, op, v, op1, Flags::default()));
    }
    let v = replaced(c, inst, op1, op0, identity, false, 0)?;
    Some(c.replace_with_binop(inst, op, op0, v, Flags::default()))
}

/// `simplifyAndOrWithOpReplaced(V, Op, RepOp, SimplifyOnly, IC, Depth)`: `v`
/// with `op` replaced by `rep`, none if that changes nothing or is not
/// simpler.
fn replaced(
    c: &mut Combiner,
    at: InstId,
    v: Operand,
    op: Operand,
    rep: Operand,
    mut simplify_only: bool,
    depth: u32,
) -> Option<Operand> {
    if op == rep {
        return None;
    }
    if v == op {
        return Some(rep);
    }
    if depth >= MAX_DEPTH {
        return None;
    }
    let def = defining(c.function, v)?;
    let Opcode::Binary(logic @ (BinaryOp::And | BinaryOp::Or | BinaryOp::Xor)) = c.function.instruction(def).opcode
    else {
        return None;
    };
    if !c.has_one_use(v) {
        simplify_only = true;
    }
    let (a, b, _) = parts(c, def);
    let new_a = replaced(c, at, a, op, rep, simplify_only, depth + 1);
    let new_b = replaced(c, at, b, op, rep, simplify_only, depth + 1);
    if new_a.is_none() && new_b.is_none() {
        return None;
    }
    let (a, b) = (new_a.unwrap_or(a), new_b.unwrap_or(b));
    if let Some(found) = simplify_binop(c, def, logic, a, b) {
        return Some(found);
    }
    if simplify_only {
        return None;
    }
    Some(c.create_binop(at, logic, a, b, Flags::default()))
}
