//! `getBaseObjectSize`: the size of the object a pointer is the start of.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `MemoryBuiltins.cpp`
//! [`getBaseObjectSize`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryBuiltins.cpp#L577):
//! `undef` is 0 bytes; `null` is unknown (or 0 bytes where `null_is_unknown`
//! is off and the address space is 0); a global variable is its type's
//! allocation size if it is sized, initialized and not interposable; a
//! `byval`, `byref`, `inalloca` or `preallocated` argument is its pointee's
//! allocation size; an `alloca` is its allocation size. With `round_to_align`
//! the size is rounded up to the stated alignment.
//!
//! DEVIATE 1: a call (`getAllocSize`: the allocation functions of the target
//! library and `allocsize`) has an unknown size; MIR has no table of them.
//! Exit: the table, with `TargetLibraryInfo`.
//!
//! DEVIATE 2: `isInterposable` is the linkage test alone (MIR has no
//! `dso_local` or semantic-interposition flag).
//!
//! # Used by
//! `constraintelim`, for the facts a load or store gives.
//!
//! # Example
//! `@g = global [10 x i32] zeroinitializer` is 40 bytes; `alloca i64, i32 3`
//! is 24.

use super::known::is_sized;
use super::query::Query;
use crate::Linkage;
use crate::context::ConstantKind;
use crate::module::{GlobalKind, GlobalValue, Operand, ValueDef};
use crate::opcode::{Attribute, Opcode};
use crate::types::Type;

/// `ObjectSizeOpts`, of the two options `getBaseObjectSize` reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct ObjectSizeOpts {
    pub round_to_align: bool,
    pub null_is_unknown_size: bool,
}

fn align_to(
    size: u64,
    align: Option<u64>,
    opts: ObjectSizeOpts,
) -> u64 {
    match align {
        Some(align) if opts.round_to_align && align > 0 => size.next_multiple_of(align),
        _ => size,
    }
}

/// `getBaseObjectSize`.
pub fn get_base_object_size(
    q: &Query,
    ptr: Operand,
    opts: ObjectSizeOpts,
) -> Option<u64> {
    let types = &q.context().types;
    match ptr {
        Operand::Constant(id) => match q.context().get(id).kind {
            ConstantKind::Undef => Some(0),
            ConstantKind::Null => {
                let space = match q.type_of(ptr).map(|ty| types.get(ty)) {
                    Some(Type::Pointer(space)) => *space,
                    _ => 0,
                };
                (!opts.null_is_unknown_size && space == 0).then_some(0)
            }
            ConstantKind::Global(global) => {
                let Some(GlobalValue { kind: GlobalKind::Variable(variable), linkage, .. }) =
                    q.globals.get(global.0 as usize)
                else {
                    return None;
                };
                let interposable =
                    matches!(
                        linkage,
                        Linkage::Weak | Linkage::LinkOnce | Linkage::Common | Linkage::ExternWeak
                    );
                if !is_sized(q, variable.ty) || variable.initializer.is_none() || interposable {
                    return None;
                }
                Some(align_to(q.layout.alloc_size(types, variable.ty), variable.align, opts))
            }
            _ => None,
        },
        Operand::Value(value) => match q.function.value(value).def {
            ValueDef::Argument(index) => {
                let attrs = q.function.parameter_attrs.get(index as usize).map_or(&[][..], Vec::as_slice);
                let pointee = attrs
                    .iter()
                    .find_map(
                        |attr| match attr {
                            Attribute::Type(name, ty)
                                if matches!(name.as_str(), "byval" | "byref" | "inalloca" | "preallocated") =>
                            {
                                Some(*ty)
                            }
                            _ => None,
                        },
                    )?;
                if !is_sized(q, pointee) {
                    return None;
                }
                let align = attrs
                    .iter()
                    .find_map(
                        |attr| match attr {
                            Attribute::Int(name, value) if name == "align" => Some(*value),
                            _ => None,
                        },
                    );
                Some(align_to(q.layout.alloc_size(types, pointee), align, opts))
            }
            ValueDef::Instruction(inst) => {
                let instruction = q.function.instruction(inst);
                let Opcode::Alloca { allocated, align, .. } = &instruction.opcode else { return None };
                // `getAllocationSize`: the size times a constant count.
                if !is_sized(q, *allocated) {
                    return None;
                }
                let count = match instruction.operands.first() {
                    None => 1,
                    Some(count) => u64::try_from(q.int_constant(*count)?.0).ok()?,
                };
                let size = q.layout.alloc_size(types, *allocated).checked_mul(count)?;
                // `getAlign` of an alloca states one: the type's preferred
                // when the IR does not.
                let align = align.unwrap_or_else(|| q.layout.pref_align(types, *allocated));
                Some(align_to(size, Some(align), opts))
            }
        },
        Operand::Block(_) => None,
    }
}
