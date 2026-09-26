//! Facts about a value's bits, as LLVM's ValueTracking proves them for
//! every pass and selector to ask.

use crate::context::{signed, ConstantKind, Context};
use crate::datalayout::DataLayout;
use crate::module::{Function, Operand, ValueDef};
use crate::opcode::{Attribute, BinaryOp, CastOp, Opcode};

/// How deep a question recurses, as LLVM's `MaxAnalysisRecursionDepth`.
const DEPTH: u32 = 6;

/// How many of an integer's top bits are copies of its sign bit, at least
/// one: LLVM's `ComputeNumSignBits`.
pub fn sign_bits(context: &Context, function: &Function, operand: Operand) -> u32 {
    _sign_bits(context, function, operand, 0)
}

fn _sign_bits(context: &Context, function: &Function, operand: Operand, depth: u32) -> u32 {
    let Some(width) = function.operand_type(context, operand).and_then(|ty| context.types.int_bits(ty)) else { return 1 };
    let constant = |operand: Operand| match operand {
        Operand::Constant(id) => match context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits),
            _ => None,
        },
        _ => None,
    };
    if let Some(bits) = constant(operand) {
        let value = signed(bits, width);
        let magnitude = if value < 0 { !value } else { value };
        return (magnitude.leading_zeros() - (128 - width)).max(1);
    }
    let Operand::Value(value) = operand else { return 1 };
    let ValueDef::Instruction(inst) = function.value(value).def else { return 1 };
    if depth == DEPTH {
        return 1;
    }
    let instruction = function.instruction(inst);
    let operands = &instruction.operands;
    let of = |operand: Operand| _sign_bits(context, function, operand, depth + 1);
    let from = |operand: Operand| function.operand_type(context, operand).and_then(|ty| context.types.int_bits(ty)).unwrap_or(width);
    let amount = |operand: Operand| constant(operand).map(|bits| bits as u32).filter(|&count| count < width);
    match instruction.opcode {
        Opcode::Cast(CastOp::SExt) => of(operands[0]) + (width - from(operands[0])),
        Opcode::Cast(CastOp::ZExt) if width > from(operands[0]) => width - from(operands[0]),
        Opcode::Cast(CastOp::Trunc) => of(operands[0]).saturating_sub(from(operands[0]) - width).max(1),
        Opcode::Binary(BinaryOp::AShr) => match amount(operands[1]) {
            Some(count) => (of(operands[0]) + count).min(width),
            None => of(operands[0]),
        },
        Opcode::Binary(BinaryOp::Shl) => match amount(operands[1]) {
            Some(count) => of(operands[0]).saturating_sub(count).max(1),
            None => 1,
        },
        // The product has no more significant bits than its factors together.
        Opcode::Binary(BinaryOp::Mul) => {
            let significant = (width - of(operands[0]) + 1) + (width - of(operands[1]) + 1);
            if significant > width { 1 } else { width - significant + 1 }
        }
        _ => 1,
    }
}

/// The object `pointer` points into, through every GEP and address space
/// cast, and how far into it when every step is constant: LLVM's
/// `getUnderlyingObject` and `GetPointerBaseWithConstantOffset` in one.
pub fn underlying(context: &Context, layout: &DataLayout, function: &Function, pointer: Operand) -> (Operand, Option<i64>) {
    let mut at = pointer;
    let mut offset = Some(0_i64);
    for _ in 0..DEPTH {
        let Operand::Value(value) = at else { break };
        let ValueDef::Instruction(inst) = function.value(value).def else { break };
        let instruction = function.instruction(inst);
        match instruction.opcode {
            Opcode::GetElementPtr { source } => {
                let indices: Vec<Option<i128>> = instruction.operands[1..]
                    .iter()
                    .map(|&one| match one {
                        Operand::Constant(id) => match context.get(id).kind {
                            ConstantKind::Int(bits) => Some(signed(bits, context.types.int_bits(context.get(id).ty).unwrap_or(64))),
                            _ => None,
                        },
                        _ => None,
                    })
                    .collect();
                let (constant, variable) = layout.collect_offset(&context.types, source, &indices);
                offset = offset.filter(|_| variable.is_empty()).map(|one| one + constant as i64);
            }
            Opcode::Cast(CastOp::AddrSpaceCast) => {}
            _ => break,
        }
        at = instruction.operands[0];
    }
    (at, offset)
}

/// Whether `bytes` bytes at `pointer` can be read whether or not the
/// program would: LLVM's `isDereferenceablePointer`.
pub fn dereferenceable(context: &Context, layout: &DataLayout, function: &Function, pointer: Operand, bytes: u64) -> bool {
    let (base, Some(offset)) = underlying(context, layout, function, pointer) else { return false };
    let Operand::Value(value) = base else { return false };
    let size = match function.value(value).def {
        ValueDef::Argument(at) => function.parameter_attrs[at as usize].iter().find_map(|attr| match attr {
            Attribute::Int(name, bytes) if name == "dereferenceable" => Some(*bytes),
            _ => None,
        }),
        ValueDef::Instruction(inst) => match function.instruction(inst).opcode {
            Opcode::Alloca { allocated, .. } if function.instruction(inst).operands.is_empty() => Some(layout.alloc_size(&context.types, allocated)),
            _ => None,
        },
    };
    size.is_some_and(|size| offset >= 0 && offset as u64 + bytes <= size)
}
