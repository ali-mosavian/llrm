//! `AArch64TTIImpl::isProfitableToSinkOperands`, the `Mul` arm
//! ([AArch64TargetTransformInfo.cpp#L7912](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L7912)).
//!
//! # How it works
//! A vector multiply of two extensions (or of an extension and the splat of a
//! scalar one) is one `smull`/`umull` only where isel sees both extensions
//! beside the multiply; made in another block they cross as opaque `v2i64`
//! registers and the multiply is scalarized. The hook names the extensions,
//! the splat shuffles and the lane-0 inserts to remake in the multiply's
//! block, when two extensions of the same sign are found, or, for a `v8i16`
//! or `v4i32` product, when a splat can fold into the indexed form.
//!
//! ```text
//! b1: %e = sext <8 x i16> %a to <8 x i64>     b2: %e = sext ...
//! b2: %m = mul <8 x i64> %e, %s            =>     %m = mul <8 x i64> %e, %s
//! ```
//!
//! DEVIATE SK1 (owner llrm-arm64-vec, exit 2026-11-27): the other arms (the
//! `smull`/`umull`/`sqdmull`/`fma` intrinsics, `FMul`, `Add`, `Sub`, the
//! shifts, `Select`, `GetElementPtr`, the SVE and SME ones) are not
//! translated: nothing here selects what they sink for.

use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::module::{Function, InstId, Operand, Use, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Opcode};
use llrm_mir::types::Type;
use llrm_mir::valuetracking::masked_value_is_zero;

fn instruction_of(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    match function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

fn operand_of(
    function: &Function,
    at: Use,
) -> Operand {
    function.instruction(at.user).operands[at.index as usize]
}

/// `isSplatShuffle`: a shuffle whose mask is one lane over and over.
fn is_splat_shuffle(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let inst = instruction_of(function, operand)?;
    match &function.instruction(inst).opcode {
        Opcode::ShuffleVector(mask) if mask.windows(2).all(|pair| pair[0] == pair[1]) => Some(inst),
        _ => None,
    }
}

fn cast_kind(
    function: &Function,
    inst: InstId,
) -> Option<CastOp> {
    match function.instruction(inst).opcode {
        Opcode::Cast(kind @ (CastOp::SExt | CastOp::ZExt)) => Some(kind),
        _ => None,
    }
}

/// `isProfitableToSinkOperands` for a `Mul` of fixed vectors.
pub fn mul_operands_to_sink(
    context: &Context,
    function: &Function,
    inst: InstId,
    ops: &mut Vec<Use>,
) -> bool {
    let one = function.instruction(inst);
    let Type::Vector { element, .. } = context.types.get(one.ty) else { return false };
    let Some(bits) = context.types.int_bits(*element) else { return false };
    // "Indexed variants of Mul exist for i16 and i32 element types only."
    let indexed = bits == 16 || bits == 32;
    let (mut sexts, mut zexts) = (0, 0);
    for index in 0..one.operands.len() {
        let op = one.operands[index];
        let here = Use { user: inst, index: index as u32 };
        if ops.iter().any(|&at| operand_of(function, at) == op) {
            continue;
        }
        let Some(def) = instruction_of(function, op) else { continue };
        if let Some(kind) = cast_kind(function, def) {
            let source = function.instruction(def).operands[0];
            if is_splat_shuffle(function, source).is_some() && indexed {
                ops.push(Use { user: def, index: 0 });
            }
            ops.push(here);
            if kind == CastOp::SExt {
                sexts += 1;
            } else {
                zexts += 1;
                // "A zext(a) is also a sext(zext(a)), if we take more than 2
                // steps."
                let from = function
                    .operand_type(context, source)
                    .and_then(
                        |ty| match context.types.get(ty) {
                            Type::Vector { element, .. } => context.types.int_bits(*element),
                            _ => None,
                        },
                    );
                if from.is_some_and(|from| from * 2 < bits) {
                    sexts += 1;
                }
            }
            continue;
        }
        let Some(shuffle) = is_splat_shuffle(function, op) else { continue };
        let first = function.instruction(shuffle).operands[0];
        // "If the Shuffle is a splat and the operand is a zext/sext, sinking
        // the operand and the s/zext can help create indexed s/umull."
        if let Some(kind) = instruction_of(function, first).and_then(|at| cast_kind(function, at)) {
            ops.push(Use { user: shuffle, index: 0 });
            ops.push(here);
            if kind == CastOp::SExt {
                sexts += 1;
            } else {
                zexts += 1;
            }
            continue;
        }
        // A splat of lane 0 of an insert of a scalar.
        let Some(insert) = instruction_of(function, first) else { continue };
        if function.instruction(insert).opcode != Opcode::InsertElement {
            continue;
        }
        let Some(scalar) = instruction_of(function, function.instruction(insert).operands[1]) else { continue };
        let lane_zero = matches!(
            function.instruction(insert).operands[2],
            Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Int(0))
        );
        if !lane_zero {
            continue;
        }
        match function.instruction(scalar).opcode {
            Opcode::Cast(CastOp::SExt) => sexts += 1,
            Opcode::Cast(CastOp::ZExt) => zexts += 1,
            _ => {
                // "If we find that the top bits are known 0, then we can sink
                // and allow the backend to generate a umull."
                let upper = (u128::MAX >> (128 - bits / 2)) << (bits / 2);
                let value = function.instruction(insert).operands[1];
                if !masked_value_is_zero(context, function, value, upper & (u128::MAX >> (128 - bits)), 0, Some(inst)) {
                    continue;
                }
                zexts += 1;
            }
        }
        // "And(Load) is excluded to prevent CGP getting stuck in a loop of
        // sinking the And, just to hoist it again back to the load."
        let and_of_load = function.instruction(scalar).opcode == Opcode::Binary(BinaryOp::And)
            && instruction_of(function, function.instruction(scalar).operands[0])
                .is_some_and(|load| matches!(function.instruction(load).opcode, Opcode::Load { .. }));
        if !and_of_load {
            ops.push(Use { user: insert, index: 1 });
        }
        ops.push(Use { user: shuffle, index: 0 });
        ops.push(here);
    }
    // "It is profitable to sink if we found two of the same type of extends."
    if !ops.is_empty() && (sexts == 2 || zexts == 2) {
        return true;
    }
    // "Otherwise, see if we should sink splats for indexed variants."
    if !indexed {
        return false;
    }
    ops.clear();
    for index in 0..one.operands.len() {
        if is_splat_shuffle(function, one.operands[index]).is_some() {
            ops.push(Use { user: inst, index: index as u32 });
        }
    }
    !ops.is_empty()
}
