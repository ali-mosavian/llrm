//! `getUnderlyingObject`.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ValueTracking.cpp`
//! (`getUnderlyingObject`, `getArgumentAliasingToReturnedPointer`): the
//! pointer a value is based on, found by peeling GEPs, pointer casts and
//! one-armed phis, and the argument a call returns. Where MIR lacks what the
//! C++ reads, the case is skipped:
//! - `GlobalAlias` (7302-7306): MIR has no aliases.
//! - `isIntrinsicReturningPointerAliasingArgumentWithoutCapturing` (7243): MIR
//!   has none of its intrinsics (`launder.invariant.group` and the target
//!   ones), so only a `returned` argument is followed.
//!
//! # Example
//! `getelementptr i8, ptr %p, i64 4` is based on `%p`.

use super::pattern;
use super::query::Query;
use crate::context::{ConstantExpr, ConstantKind};
use crate::module::Operand;
use crate::opcode::{CastOp, Opcode};
use crate::types::Type;

/// `MaxLookupSearchDepth`.
pub const MAX_LOOKUP_SEARCH_DEPTH: u32 = 10;

fn is_pointer(
    q: &Query,
    v: Operand,
) -> bool {
    q.type_of(v).is_some_and(|ty| matches!(q.context.types.get(ty), Type::Pointer(_)))
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7289>
///
/// `max_lookup` 0 is unbounded, as in C++.
pub fn get_underlying_object(
    q: &Query,
    mut v: Operand,
    max_lookup: u32,
) -> Operand {
    let mut count = 0;
    while max_lookup == 0 || count < max_lookup {
        count += 1;
        // A GEPOperator, instruction or constant expression, and the pointer
        // casts (BitCast and AddrSpaceCast operators).
        let peeled = match v {
            Operand::Constant(id) => match &q.context.get(id).kind {
                ConstantKind::Expr(ConstantExpr::GetElementPtr { operands, .. }) => {
                    operands.first().map(|&base| Operand::Constant(base))
                }
                ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::BitCast | CastOp::AddrSpaceCast, value }) => {
                    Some(Operand::Constant(*value))
                }
                _ => return v,
            },
            Operand::Value(_) => {
                let Some((_, instruction)) = q.instruction_of(v) else { return v };
                match instruction.opcode {
                    Opcode::GetElementPtr { .. } | Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) => {
                        instruction.operands.first().copied()
                    }
                    // Look through single-arg phi nodes created by LCSSA.
                    Opcode::Phi => {
                        if instruction.operands.len() == 2 {
                            v = instruction.operands[0];
                            continue;
                        }
                        return v;
                    }
                    Opcode::Call(_) | Opcode::Invoke(_) => match pattern::returned_argument_operand(q, instruction) {
                        Some(returned) => {
                            v = returned;
                            continue;
                        }
                        None => return v,
                    },
                    _ => return v,
                }
            }
            Operand::Block(_) => return v,
        };
        // Only handle scalar pointer base.
        match peeled {
            Some(base) if is_pointer(q, base) => v = base,
            _ => return v,
        }
    }
    v
}
