//! Whether two accesses may touch the same bytes, as LLVM's BasicAA
//! answers from their underlying objects: distinct identified objects do
//! not overlap, one object's constant ranges overlap as they overlap, and
//! a stack slot whose address never escapes is reached only through
//! itself.

use crate::context::{ConstantKind, Context};
use crate::datalayout::DataLayout;
use crate::memory::{self, Callees};
use crate::module::{Function, Operand, ValueDef, ValueId};
use crate::opcode::{Attribute, Opcode};
use crate::valuetracking::underlying;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Alias {
    No,
    May,
    Must,
}

/// `bytes` bytes at `pointer`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Location {
    pub pointer: Operand,
    pub bytes: u64,
}

pub fn alias(context: &Context, layout: &DataLayout, callees: &Callees, function: &Function, a: Location, b: Location) -> Alias {
    let (base_a, offset_a) = underlying(context, layout, function, a.pointer);
    let (base_b, offset_b) = underlying(context, layout, function, b.pointer);
    if base_a == base_b {
        let (Some(x), Some(y)) = (offset_a, offset_b) else { return Alias::May };
        return if x == y && a.bytes == b.bytes {
            Alias::Must
        } else if x + a.bytes as i64 <= y || y + b.bytes as i64 <= x {
            Alias::No
        } else {
            Alias::May
        };
    }
    let (one, other) = (object(context, function, base_a), object(context, function, base_b));
    match (one, other) {
        (Some(_), Some(_)) => Alias::No,
        // An unescaped slot is named by nothing but its own address.
        (Some(Object::Slot(slot)), None) | (None, Some(Object::Slot(slot))) if !captured(context, callees, function, slot) => Alias::No,
        _ => Alias::May,
    }
}

/// An object no other object overlaps: a stack slot, a global, or what a
/// `noalias` parameter points to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Object {
    Slot(ValueId),
    Global,
    Unaliased,
}

pub fn object(context: &Context, function: &Function, base: Operand) -> Option<Object> {
    match base {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(inst) if matches!(function.instruction(inst).opcode, Opcode::Alloca { .. }) => Some(Object::Slot(value)),
            ValueDef::Argument(at) => function.parameter_attrs[at as usize]
                .iter()
                .any(|attr| matches!(attr, Attribute::Flag(flag) if flag == "noalias"))
                .then_some(Object::Unaliased),
            ValueDef::Instruction(_) => None,
        },
        Operand::Constant(id) => matches!(context.get(id).kind, ConstantKind::Global(_)).then_some(Object::Global),
        Operand::Block(_) => None,
    }
}

/// Whether the address of `slot`, or one derived from it, may be kept
/// beyond the instruction using it: stored, passed to a call that does
/// not promise otherwise, returned, or made an integer.
pub fn captured(context: &Context, callees: &Callees, function: &Function, slot: ValueId) -> bool {
    let mut work = vec![slot];
    let mut seen = vec![slot];
    while let Some(value) = work.pop() {
        for one_use in function.users(value) {
            let instruction = function.instruction(one_use.user);
            let derived = match &instruction.opcode {
                Opcode::Load { volatile: false, .. } | Opcode::ICmp(_) => None,
                Opcode::Store { volatile: false, .. } if one_use.index == 1 => None,
                Opcode::GetElementPtr { .. } | Opcode::Cast(crate::opcode::CastOp::AddrSpaceCast) | Opcode::Phi | Opcode::Select => instruction.result,
                Opcode::Call(_) if memory::nocapture(context, callees, function, one_use.user, one_use.index as usize) => None,
                _ => return true,
            };
            if let Some(derived) = derived.filter(|one| !seen.contains(one)) {
                seen.push(derived);
                work.push(derived);
            }
        }
    }
    false
}

/// Whether `inner`'s bytes all lie within `outer`'s.
pub fn contains(context: &Context, layout: &DataLayout, function: &Function, outer: Location, inner: Location) -> bool {
    let (base_outer, offset_outer) = underlying(context, layout, function, outer.pointer);
    let (base_inner, offset_inner) = underlying(context, layout, function, inner.pointer);
    match (offset_outer, offset_inner) {
        (Some(x), Some(y)) => base_outer == base_inner && x <= y && y + inner.bytes as i64 <= x + outer.bytes as i64,
        _ => false,
    }
}
