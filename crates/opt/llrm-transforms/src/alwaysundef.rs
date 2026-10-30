//! Whether passing a constant into a phi is undefined behavior at a use of
//! the phi: SimplifyCFG's `passingValueIsAlwaysUndefined`.
//!
//! # How it works
//! A null (or undef) input of `phi` makes the program undefined where the
//! first use of `phi` after it in its block, with nothing in between that may
//! not come back, dereferences it, stores through it, calls it, divides by
//! it, returns it from a `noundef` function or passes it to a `noundef`
//! parameter; a use through a `getelementptr` from the pointer is looked
//! through. `removeUndefIntroducingPredecessor` and the folds that would make
//! a select of the phi ask it, to make the undefined control flow go instead.
//!
//! # Example
//! A null the block loads through makes its way in undefined:
//!
//! ```text
//!   b1: br b3                       b1: unreachable
//!   b3: %p = phi ptr [ null, b1 ], [ %q, b2 ]   (b1's edge is gone)
//!       %v = load i16, ptr %p
//! ```
//!
//! # LLVM structure
//! | LLVM | here | |
//! |---|---|---|
//! | [`passingValueIsAlwaysUndefined`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9094) | `always_undefined` | MIRROR; D1, D2 |
//!
//! - D1. `NullPointerIsDefined` is the function's `null_pointer_is_valid`
//!   attribute for every address space.
//! - D2. `isPassingUndefUB` is the `noundef` attribute of the call's argument;
//!   the callee's own parameter attributes are not read.

use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::module::{Function, InstId, Operand};
use llrm_mir::opcode::{Attribute, BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::types::Type;

use crate::twoentryphi::Speculation;

fn has_flag(
    attrs: &[Attribute],
    name: &str,
) -> bool {
    attrs.iter().any(|one| matches!(one, Attribute::Flag(found) if found == name))
}

/// `NullPointerIsDefined`: the function says a null pointer is a valid one
/// (`null_pointer_is_valid`), whatever the address space.
fn null_pointer_is_defined(function: &Function) -> bool {
    has_flag(&function.attrs, "null_pointer_is_valid")
}

/// Whether the call `inst` is `llvm.assume`.
fn is_assume(
    context: &Context,
    spec: &Speculation,
    function: &Function,
    inst: InstId,
) -> bool {
    llrm_mir::memory::callee(context, function, inst)
        .and_then(|global| spec.outer.globals.get(global.0 as usize))
        .and_then(|global| global.name.as_deref())
        .is_some_and(|name| name == "llvm.assume")
}

/// `value`, an input of `inst` (a phi, or a `getelementptr` of the pointer
/// the phi stood for), makes the program undefined where `inst`'s first
/// witnessing use runs. `pointer_may_be_modified`: the value was offset by
/// a `getelementptr` that may have left null.
pub fn always_undefined(
    context: &Context,
    spec: &Speculation,
    function: &Function,
    value: Operand,
    inst: InstId,
    pointer_may_be_modified: bool,
) -> bool {
    let Operand::Constant(id) = value else { return false };
    let Some(result) = function.instruction(inst).result else { return false };
    if function.users(result).is_empty() {
        return false;
    }
    let kind = &context.get(id).kind;
    let null = matches!(
        kind,
        ConstantKind::Null | ConstantKind::Zero | ConstantKind::Int(0)
    );
    let undef = matches!(kind, ConstantKind::Undef | ConstantKind::Poison);
    if !(null || undef) {
        return false;
    }
    let block = function.parent(inst).expect("a placed instruction");
    // The first use that can witness it: in this block, after `inst`, of a kind
    // that is undefined on the value.
    let witness = function
        .users(result)
        .iter()
        .filter(|one| {
            one.user != inst && function.parent(one.user) == Some(block) && function.comes_before(inst, one.user)
        })
        .filter(|one| {
            matches!(
                function.instruction(one.user).opcode,
                Opcode::GetElementPtr { .. }
                    | Opcode::Ret
                    | Opcode::Cast(CastOp::BitCast)
                    | Opcode::Load { .. }
                    | Opcode::Store { .. }
                    | Opcode::Call(_)
                    | Opcode::Invoke(_)
                    | Opcode::Binary(BinaryOp::UDiv | BinaryOp::URem | BinaryOp::SDiv | BinaryOp::SRem)
            )
        })
        .min_by(|a, b| {
            if a.user == b.user {
                std::cmp::Ordering::Equal
            } else if function.comes_before(a.user, b.user) {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        })
        .copied();
    let Some(witness) = witness else { return false };
    let (user, index) = (witness.user, witness.index as usize);
    // Nothing between may fail to come back.
    let between = function.iter_after(inst).take_while(|&one| one != user);
    for one in between {
        if !crate::local::is_guaranteed_to_transfer_execution_to_successor(context, &spec.outer.globals, function, one)
        {
            return false;
        }
    }
    let operation = function.instruction(user);
    match &operation.opcode {
        // Look through a `getelementptr` from the pointer.
        Opcode::GetElementPtr { .. } if index == 0 => {
            if matches!(context.types.get(operation.ty), Type::Vector { .. }) {
                return false;
            }
            let all_zero = operation.operands[1..].iter().all(|one| {
                matches!(
                    one,
                    Operand::Constant(c) if matches!(context.get(*c).kind, ConstantKind::Int(0) | ConstantKind::Zero)
                )
            });
            let moved = pointer_may_be_modified
                || (!all_zero && (!operation.flags.contains(Flags::INBOUNDS) || null_pointer_is_defined(function)));
            always_undefined(context, spec, function, value, user, moved)
        }
        Opcode::Ret => {
            let no_undef = has_flag(&function.return_attrs, "noundef");
            if undef && no_undef {
                return true;
            }
            if null && no_undef && has_flag(&function.return_attrs, "nonnull") {
                return !pointer_may_be_modified;
            }
            false
        }
        Opcode::Load { volatile, .. } => !*volatile && !null_pointer_is_defined(function),
        Opcode::Store { volatile, .. } => !*volatile && !null_pointer_is_defined(function) && index == 1,
        Opcode::Call(info) | Opcode::Invoke(info) => {
            if null && null_pointer_is_defined(function) {
                return false;
            }
            if is_assume(context, spec, function, user) && index == 0 {
                return true;
            }
            // The callee is the last operand.
            if index == operation.operands.len() - 1 {
                return true;
            }
            let attrs = info.argument_attrs.get(index).map_or(&[][..], Vec::as_slice);
            if null
                && matches!(
                    context.types.get(function.operand_type(context, value).unwrap_or(operation.ty)),
                    Type::Pointer(_)
                )
                && has_flag(attrs, "nonnull")
                && has_flag(attrs, "noundef")
            {
                return !pointer_may_be_modified;
            }
            undef && has_flag(attrs, "noundef")
        }
        // Division or remainder by zero.
        Opcode::Binary(BinaryOp::UDiv | BinaryOp::URem | BinaryOp::SDiv | BinaryOp::SRem) => index == 1,
        _ => false,
    }
}
