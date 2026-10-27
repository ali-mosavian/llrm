//! `getUnderlyingObject` and `getUnderlyingObjects`.
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

/// Whether `from` is a pointer in a space the layout marks `ni:` (the target's
/// selector alone) over an object: cast back to a far pointer it is
/// `segment:0`, not a pointer into that object.
fn selector_source(
    q: &Query,
    from: Operand,
) -> bool {
    q.type_of(from).is_some_and(|ty| {
        matches!(
            q.context.types.get(ty),
            Type::Pointer(space) if q.layout.has_unstable_representation(*space)
        )
    }) && crate::valuetracking::object_below(q.context, q.function, from)
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
                    // A selector alone names no object (DEVIATE C1).
                    if selector_source(q, Operand::Constant(*value)) {
                        return v;
                    }
                    Some(Operand::Constant(*value))
                }
                _ => return v,
            },
            Operand::Value(_) => {
                let Some((_, instruction)) = q.instruction_of(v) else { return v };
                match instruction.opcode {
                    Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast)
                        if instruction.operands.first().is_some_and(|&from| selector_source(q, from)) =>
                    {
                        return v;
                    }
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

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7264>
///
/// `pn`, a phi in a loop header, defines a loop-variant pointer: whether the
/// previous iteration referred to the same object.
fn is_same_underlying_object_in_loop(
    q: &Query,
    pn: &crate::module::Instruction,
    pn_block: crate::module::BlockId,
    loops: &crate::loopinfo::LoopInfo,
) -> bool {
    // Find the loop-defined value.
    let Some(lp) = loops.get_loop_for(pn_block) else { return true };
    if pn.operands.len() != 4 {
        return true;
    }

    // Find the value from the previous iteration.
    let in_loop = |value: Operand| {
        let (inst, instruction) = q.instruction_of(value)?;
        let block = q.function.parent(inst)?;
        (loops.get_loop_for(block).is_some_and(|found| found.id() == lp.id())).then_some(instruction)
    };
    let Some(prev) = in_loop(pn.operands[0]).or_else(|| in_loop(pn.operands[2])) else { return true };

    // If a new pointer is loaded in the loop, the pointer references a
    // different object in every iteration.
    if let Opcode::Load { .. } = prev.opcode
        && !lp.is_loop_invariant(q.function, prev.operands[0])
    {
        return false;
    }
    true
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7338>
///
/// Every underlying object `v` may be based on, through selects and phis.
/// `max_lookup` 0 is unbounded, as in C++.
pub fn get_underlying_objects(
    q: &Query,
    v: Operand,
    loops: Option<&crate::loopinfo::LoopInfo>,
    max_lookup: u32,
) -> Vec<Operand> {
    let mut visited = crate::hash::HashSet::default();
    let mut objects = Vec::new();
    let mut worklist = vec![v];
    while let Some(p) = worklist.pop() {
        let p = get_underlying_object(q, p, max_lookup);

        if !visited.insert(p) {
            continue;
        }

        if let Some((_, instruction)) = q.instruction_of(p) {
            match instruction.opcode {
                Opcode::Select => {
                    worklist.push(instruction.operands[1]);
                    worklist.push(instruction.operands[2]);
                    continue;
                }
                // If this phi changes the underlying object in every
                // iteration of the loop, don't look through it: `Prev` tracks
                // `Curr` one iteration behind, so they refer to different
                // underlying objects.
                Opcode::Phi => {
                    let (inst, _) = q.instruction_of(p).expect("an instruction");
                    let block = q.function.parent(inst).expect("a block");
                    let incoming = || instruction.operands.chunks(2).map(|pair| pair[0]);
                    match loops {
                        Some(loops)
                            if loops.is_loop_header(block)
                                && !is_same_underlying_object_in_loop(q, instruction, block, loops) =>
                        {
                            objects.push(p);
                        }
                        _ => worklist.extend(incoming()),
                    }
                    continue;
                }
                _ => {}
            }
        }

        objects.push(p);
    }
    objects
}
