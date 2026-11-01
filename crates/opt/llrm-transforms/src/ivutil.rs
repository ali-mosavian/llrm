//! What the counter-rewriting passes ([`crate::maskscale`], [`crate::ivscale`])
//! read of the IR: an integer constant, an operand's defining instruction, a
//! value wrapped to a width, and a constant of a type.
//!
//! # Example
//! `int(context, i16, 1 << 16, 16)` is the 16-bit constant 0.

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::module::{Function, InstId, Operand, ValueDef};

/// The integer `operand` is a constant of.
pub(crate) fn constant(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(value) => Some(value),
        _ => None,
    }
}

/// The instruction that defines `operand`.
pub(crate) fn def(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    match function.value(value).def {
        ValueDef::Instruction(def) => Some(def),
        _ => None,
    }
}

/// `value` modulo `2^bits`.
pub(crate) fn wrap(
    value: u128,
    bits: u32,
) -> u128 {
    if bits >= 128 { value } else { value & ((1u128 << bits) - 1) }
}

/// The constant `value` of type `like`, wrapped to `width` bits.
pub(crate) fn int(
    context: &Context,
    like: llrm_mir::types::TypeId,
    value: u128,
    width: u32,
) -> Operand {
    Operand::Constant(context.constant(Constant { ty: like, kind: ConstantKind::Int(wrap(value, width)) }))
}
