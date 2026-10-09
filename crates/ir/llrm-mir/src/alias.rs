//! Whether two accesses may touch the same bytes, as LLVM's BasicAA
//! answers from their underlying objects: distinct identified objects do
//! not overlap, one object's constant ranges overlap as they overlap, and
//! a stack slot whose address never escapes is reached only through
//! itself. Where those leave it open, the accesses' `!tbaa` tags decide, as
//! LLVM's TypeBasedAA does.

use crate::context::{ConstantKind, Context};
use crate::datalayout::DataLayout;
use crate::facts::Facts;
use crate::memory::{self, Callees};
use crate::module::{Function, InstId, MetadataId, MetadataNode, MetadataOperand, Operand, ValueDef, ValueId};
use crate::opcode::Opcode;
use crate::valuetracking::underlying;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Alias {
    No,
    May,
    Must,
}

/// `bytes` bytes at `pointer`, accessed as the `!tbaa` tag `tbaa` says.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Location {
    pub pointer: Operand,
    pub bytes: u64,
    pub tbaa: Option<MetadataId>,
}

/// The `!tbaa` tag of the access `inst`.
pub fn tag(
    function: &Function,
    inst: InstId,
) -> Option<MetadataId> {
    function.instruction(inst).metadata.iter().find(|(kind, _)| kind == "tbaa").map(|&(_, node)| node)
}

pub fn alias(
    context: &Context,
    layout: &DataLayout,
    callees: &Callees,
    metadata: &[MetadataNode],
    function: &Function,
    a: Location,
    b: Location,
) -> Alias {
    match based(context, layout, callees, function, a, b) {
        Alias::May if typed_apart(context, metadata, a.tbaa, b.tbaa) => Alias::No,
        answer => answer,
    }
}

/// What the pointers alone say.
fn based(
    context: &Context,
    layout: &DataLayout,
    callees: &Callees,
    function: &Function,
    a: Location,
    b: Location,
) -> Alias {
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
        (Some(Object::Slot(slot)), None) | (None, Some(Object::Slot(slot)))
            if !captured(context, callees, function, slot) =>
        {
            Alias::No
        }
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

pub fn object(
    context: &Context,
    function: &Function,
    base: Operand,
) -> Option<Object> {
    match base {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(inst) if matches!(function.instruction(inst).opcode, Opcode::Alloca { .. }) => {
                Some(Object::Slot(value))
            }
            ValueDef::Argument(at) => Facts::param(function, at as usize).no_alias().then_some(Object::Unaliased),
            ValueDef::Instruction(_) => None,
        },
        Operand::Constant(id) => matches!(context.get(id).kind, ConstantKind::Global(_)).then_some(Object::Global),
        Operand::Block(_) => None,
    }
}

/// Whether the address of `slot`, or one derived from it, may be kept
/// beyond the instruction using it: stored, passed to a call that does
/// not promise otherwise, returned, or made an integer.
pub fn captured(
    context: &Context,
    callees: &Callees,
    function: &Function,
    slot: ValueId,
) -> bool {
    let mut work = vec![slot];
    let mut seen = vec![slot];
    while let Some(value) = work.pop() {
        for one_use in function.users(value) {
            let instruction = function.instruction(one_use.user);
            let derived = match &instruction.opcode {
                Opcode::Load { volatile: false, .. } | Opcode::ICmp(_) => None,
                Opcode::Store { volatile: false, .. } if one_use.index == 1 => None,
                Opcode::GetElementPtr { .. }
                | Opcode::Cast(crate::opcode::CastOp::AddrSpaceCast)
                | Opcode::Phi
                | Opcode::Select => instruction.result,
                Opcode::Call(_)
                    if memory::nocapture(context, callees, function, one_use.user, one_use.index as usize) =>
                {
                    None
                }
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
pub fn contains(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    outer: Location,
    inner: Location,
) -> bool {
    let (base_outer, offset_outer) = underlying(context, layout, function, outer.pointer);
    let (base_inner, offset_inner) = underlying(context, layout, function, inner.pointer);
    match (offset_outer, offset_inner) {
        (Some(x), Some(y)) => base_outer == base_inner && x <= y && y + inner.bytes as i64 <= x + outer.bytes as i64,
        _ => false,
    }
}

/// Whether two tagged accesses touch different memory: under one root,
/// neither's type is the other's or an ancestor of it. Only scalar tags,
/// `!{!type, !type, i64 0}`, are read; any other tag may alias anything.
fn typed_apart(
    context: &Context,
    metadata: &[MetadataNode],
    a: Option<MetadataId>,
    b: Option<MetadataId>,
) -> bool {
    let (Some(a), Some(b)) = (a, b) else { return false };
    let (Some(a), Some(b)) = (scalar_type(context, metadata, a), scalar_type(context, metadata, b)) else {
        return false;
    };
    let (a, b) = (ancestors(metadata, a), ancestors(metadata, b));
    a.last() == b.last() && !a.contains(&b[0]) && !b.contains(&a[0])
}

/// The type an access tag names, if it is a scalar tag.
fn scalar_type(
    context: &Context,
    metadata: &[MetadataNode],
    tag: MetadataId,
) -> Option<MetadataId> {
    match metadata.get(tag.0 as usize)?.operands[..] {
        [MetadataOperand::Node(base), MetadataOperand::Node(access), MetadataOperand::Constant(offset), ..]
            if base == access && context.get(offset).kind == ConstantKind::Int(0) =>
        {
            Some(access)
        }
        _ => None,
    }
}

/// A type node and its parents, `!{!"name", !parent, i64 0}`, up to the root.
fn ancestors(
    metadata: &[MetadataNode],
    node: MetadataId,
) -> Vec<MetadataId> {
    let mut chain = vec![node];
    while let Some(MetadataOperand::Node(parent)) =
        metadata.get(chain[chain.len() - 1].0 as usize).and_then(|one| one.operands.get(1))
    {
        if chain.contains(parent) {
            break;
        }
        chain.push(*parent);
    }
    chain
}
