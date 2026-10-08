//! A near address a register holds, given its object back. The core raise
//! spells `[bx]` as `inttoptr` of BX: an offset into DGROUP with no
//! object. Where that offset was formed from an object's address -- `mov
//! si,offset X` then `[si+10]`, `lea ax,[bp-8]` then `[bx]` -- it is a GEP
//! off that object, which alias analysis can see. A constant offset lands
//! in the carved object holding its byte; one with a variable part stays in
//! the object it was formed from, as `[bx+X]` does in the core raise.
//!
//! A far address through DGROUP's own selector (`push ds / pop es`) is the
//! near address `addrspacecast` makes far.

use llrm_mir::context::{ConstantExpr, ConstantKind, Context, signed};
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::types::Type;

use crate::objects::Objects;

/// What an operand is made of: an instruction's or a constant
/// expression's opcode and operands.
pub(crate) fn made(function: &Function, context: &Context, operand: Operand) -> Option<(Opcode, Vec<Operand>)> {
    match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(inst) => {
                let one = function.instruction(inst);
                Some((one.opcode.clone(), one.operands.clone()))
            }
            ValueDef::Argument(_) => None,
        },
        Operand::Constant(id) => match &context.get(id).kind {
            ConstantKind::Expr(ConstantExpr::Cast { op, value }) => Some((Opcode::Cast(*op), vec![Operand::Constant(*value)])),
            ConstantKind::Expr(ConstantExpr::GetElementPtr { source, operands, .. }) => {
                Some((Opcode::GetElementPtr { source: *source }, operands.iter().map(|&one| Operand::Constant(one)).collect()))
            }
            _ => None,
        },
        Operand::Block(_) => None,
    }
}

pub(crate) fn constant(context: &Context, operand: Operand) -> Option<i64> {
    let Operand::Constant(id) = operand else { return None };
    let one = context.get(id);
    match one.kind {
        ConstantKind::Int(bits) => Some(signed(bits, context.types.int_bits(one.ty)?) as i64),
        _ => None,
    }
}

fn space(function: &Function, context: &Context, operand: Operand) -> Option<u32> {
    match context.types.get(function.operand_type(context, operand)?) {
        Type::Pointer(space) => Some(*space),
        _ => None,
    }
}

/// A pointer as a root, a constant byte offset and variable byte terms.
#[derive(Clone, Debug)]
pub(crate) struct Parts {
    pub root: Operand,
    pub constant: i64,
    pub terms: Vec<Operand>,
}

/// A pointer through its byte GEPs.
pub(crate) fn pointer_parts(function: &Function, context: &Context, pointer: Operand) -> Parts {
    let mut parts = Parts { root: pointer, constant: 0, terms: Vec::new() };
    while let Some((Opcode::GetElementPtr { source }, operands)) = made(function, context, parts.root) {
        if context.types.get(source) != &Type::Int(8) || operands.len() != 2 {
            break;
        }
        match constant(context, operands[1]) {
            Some(offset) => parts.constant += offset,
            None => parts.terms.push(operands[1]),
        }
        parts.root = operands[0];
    }
    parts
}

/// The pointer a 16-bit offset was formed from, and what was added to it.
pub(crate) fn offset_parts(function: &Function, context: &Context, offset: Operand, depth: u32) -> Option<Parts> {
    if depth > 16 {
        return None;
    }
    let (opcode, operands) = made(function, context, offset)?;
    match opcode {
        Opcode::Cast(CastOp::PtrToInt) if space(function, context, operands[0]) == Some(0) => Some(pointer_parts(function, context, operands[0])),
        Opcode::Binary(op @ (BinaryOp::Add | BinaryOp::Sub)) => {
            let (left, right) = (offset_parts(function, context, operands[0], depth + 1), offset_parts(function, context, operands[1], depth + 1));
            let (mut parts, other) = match (left, right, op) {
                (Some(parts), None, _) => (parts, operands[1]),
                (None, Some(parts), BinaryOp::Add) => (parts, operands[0]),
                _ => return None,
            };
            match (constant(context, other), op) {
                (Some(value), BinaryOp::Add) => parts.constant += value,
                (Some(value), _) => parts.constant -= value,
                (None, BinaryOp::Add) => parts.terms.push(other),
                (None, _) => return None,
            }
            Some(parts)
        }
        _ => None,
    }
}

/// Whether `segment`, an `addrspace(2)` value, is DGROUP's selector.
fn dgroup(function: &Function, context: &Context, objects: &Objects, segment: Operand, depth: u32) -> bool {
    if depth > 8 {
        return false;
    }
    match made(function, context, segment) {
        Some((Opcode::Cast(CastOp::IntToPtr), operands)) => match made(function, context, operands[0]) {
            Some((Opcode::Cast(CastOp::PtrToInt), inner)) => dgroup(function, context, objects, inner[0], depth + 1),
            _ => false,
        },
        Some((Opcode::Cast(CastOp::AddrSpaceCast), operands)) => match made(function, context, operands[0]) {
            Some((Opcode::Cast(CastOp::AddrSpaceCast), inner)) if space(function, context, inner[0]) == Some(0) => {
                let root = pointer_parts(function, context, inner[0]).root;
                matches!(root, Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Global(global) if objects.placed(global).is_some()))
            }
            _ => false,
        },
        _ => false,
    }
}

fn before(function: &mut Function, at: InstId, opcode: Opcode, ty: llrm_mir::types::TypeId, operands: Vec<Operand>) -> Operand {
    let made = function.create_instruction(opcode, ty, operands, Flags::default(), None);
    function.insert(made, Position::Before(at)).expect("placed");
    Operand::Value(function.instruction(made).result.expect("a value"))
}

/// Erases what `root` leaves computing nothing anything reads.
fn sweep(function: &mut Function, root: Operand) {
    let mut pending = vec![root];
    while let Some(Operand::Value(value)) = pending.pop() {
        let ValueDef::Instruction(inst) = function.value(value).def else { continue };
        if function.is_erased(inst) || !function.users(value).is_empty() {
            continue;
        }
        let pure = matches!(function.instruction(inst).opcode, Opcode::Cast(_) | Opcode::GetElementPtr { .. } | Opcode::Binary(BinaryOp::Add | BinaryOp::Sub));
        if !pure {
            continue;
        }
        let operands = function.instruction(inst).operands.clone();
        function.set_operands(inst, Vec::new());
        function.erase(inst).expect("placed");
        pending.extend(operands);
    }
}

/// Gives every near address in `function` formed from an object's
/// address that object.
pub fn attribute(function: &mut Function, context: &mut Context, objects: &Objects, spaces: &llrm_mir::spaces::Spaces) {
    let (near, far, segment, word, byte) = (context.types.ptr(0), context.types.ptr(spaces.far), context.types.ptr(crate::segment(spaces)), context.types.int(16), context.types.int(8));
    // A far address through DGROUP's selector is a near one.
    let fars: Vec<InstId> = function.walk().map(|(_, one)| one).filter(|&one| matches!(function.instruction(one).opcode, Opcode::GetElementPtr { source } if source == byte)).collect();
    for inst in fars {
        let operands = function.instruction(inst).operands.clone();
        if operands.len() != 2 || function.instruction(inst).ty != far {
            continue;
        }
        let Some((Opcode::Cast(CastOp::AddrSpaceCast), base)) = made(function, context, operands[0]) else { continue };
        if function.operand_type(context, base[0]) != Some(segment) || !dgroup(function, context, objects, base[0], 0) {
            continue;
        }
        let pointer = before(function, inst, Opcode::Cast(CastOp::IntToPtr), near, vec![operands[1]]);
        let made = before(function, inst, Opcode::Cast(CastOp::AddrSpaceCast), far, vec![pointer]);
        let result = function.instruction(inst).result.expect("a pointer");
        function.replace_all_uses_with(result, made);
        function.set_operands(inst, Vec::new());
        function.erase(inst).expect("placed");
        sweep(function, operands[0]);
    }
    let casts: Vec<InstId> = function.walk().map(|(_, one)| one).filter(|&one| function.instruction(one).opcode == Opcode::Cast(CastOp::IntToPtr) && function.instruction(one).ty == near).collect();
    for inst in casts {
        let offset = function.instruction(inst).operands[0];
        let Some(parts) = offset_parts(function, context, offset, 0) else { continue };
        let (mut root, mut constant) = (parts.root, parts.constant);
        if let Operand::Constant(id) = root
            && let ConstantKind::Global(global) = context.get(id).kind
            && let Some((index, object)) = objects.placed(global)
            && parts.terms.is_empty()
            && let Some(target) = objects.at(index, object.start + constant)
        {
            constant += object.start - target.start;
            root = Operand::Constant(target.reference);
        }
        let mut index: Option<Operand> = (constant != 0).then(|| Operand::Constant(context.int(word, i128::from(constant))));
        for &term in &parts.terms {
            index = Some(match index {
                None => term,
                Some(sum) => before(function, inst, Opcode::Binary(BinaryOp::Add), word, vec![sum, term]),
            });
        }
        let pointer = match index {
            None => root,
            Some(index) => before(function, inst, Opcode::GetElementPtr { source: byte }, near, vec![root, index]),
        };
        let result = function.instruction(inst).result.expect("a pointer");
        function.replace_all_uses_with(result, pointer);
        function.set_operands(inst, Vec::new());
        function.erase(inst).expect("placed");
        sweep(function, offset);
    }
}
