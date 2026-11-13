//! The `memcpy`, `mempcpy`, `memmove`, `memset` and `memccpy` arms of
//! `LibCallSimplifier`: `optimizeMemCpy` (:1687), `optimizeMemCCpy` (:1700),
//! `optimizeMemPCpy` (:1740), `optimizeMemMove` (:1753) and `optimizeMemSet`
//! (:1766), at d1106deb71cc. A call of the library function is the
//! intrinsic with `align 1` on its pointers; `mempcpy` also yields `dst + n`.
//!
//! # Deviations
//! - DEVIATE M1 (owner w2, exit 2026-11-30): `mergeAttributesAndFlags` and
//!   `copyFlags` are not translated: the intrinsic gets `align 1` and nothing
//!   of the call's attributes, flags or metadata, and the arguments are not
//!   annotated `nonnull`/`dereferenceable`
//!   (`annotateNonNullAndDereferenceable`).
//! - DEVIATE M3 (owner w2, exit 2026-12-01): the pin marks `memccpy`
//!   unavailable on Windows outside MinGW (TargetLibraryInfo.cpp#L387) and on
//!   PlayStation, `mempcpy` on PlayStation only (#L801-L802, the `isPS()` block
//!   from #L692). The triple is not visible here (`libfunc.rs` DEVIATE 1): a
//!   target's `Library` states it, and those targets state none, so a user's
//!   own `memccpy` there is rewritten. Exit: the Windows `Library`.
//! - DEVIATE M2 (owner w2, exit 2026-11-30): a pointer in a non-default address
//!   space is left as the call (the pin's intrinsic is overloaded on it; the
//!   table here names the default space only).

use llrm_mir::context::ConstantKind;
use llrm_mir::edit::Position;
use llrm_mir::module::Operand;
use llrm_mir::opcode::{Attribute, CallInfo, CastOp, Flags, Opcode, Tail};
use llrm_mir::passes::Unit;
use llrm_mir::types::{Type, TypeId};

use super::{Rewrite, Site};
use crate::loopidioms::How;

/// The call's operands, without its callee.
fn arguments(
    unit: &Unit,
    site: &Site,
) -> Vec<Operand> {
    let mut operands = unit.function.instruction(site.call).operands.to_vec();
    operands.pop();
    operands
}

/// Whether `operand` is a pointer in the default address space.
fn near(
    unit: &Unit,
    operand: Operand,
) -> bool {
    unit.function
        .operand_type(unit.context, operand)
        .is_some_and(|ty| matches!(unit.context.types.get(ty), Type::Pointer(0)))
}

/// The width of the length `operand`, if it is an integer.
fn width(
    unit: &Unit,
    operand: Operand,
) -> Option<u32> {
    unit.function.operand_type(unit.context, operand).and_then(|ty| unit.context.types.int_bits(ty))
}

fn place(
    unit: &mut Unit,
    site: &Site,
    opcode: Opcode,
    ty: TypeId,
    operands: Vec<Operand>,
) -> Operand {
    let made = unit.function.create_instruction(opcode, ty, operands, Flags::default(), None);
    unit.function.insert(made, Position::Before(site.call)).expect("a placed instruction");
    Operand::Value(unit.function.instruction(made).result.expect("a result"))
}

/// `B.CreateMemCpy` / `CreateMemMove(dst, Align(1), src, Align(1), size)`
/// before the call.
fn copy_call(
    unit: &mut Unit,
    site: &Site,
    how: How,
    dst: Operand,
    src: Operand,
    size: Operand,
) -> Option<()> {
    let bits = width(unit, size)?;
    if !near(unit, dst) || !near(unit, src) {
        return None;
    }
    let (callee, function_type) = crate::loopidioms::_copy(unit.context, unit.declared, how, 0, 0, bits);
    emit_intrinsic(unit, site, callee, function_type, vec![dst, src, size], 2)
}

/// A call of the declared intrinsic before the call, with `align 1` on the
/// first `pointers` arguments and `isvolatile` false last.
fn emit_intrinsic(
    unit: &mut Unit,
    site: &Site,
    callee: llrm_mir::context::GlobalId,
    function_type: TypeId,
    mut operands: Vec<Operand>,
    pointers: usize,
) -> Option<()> {
    let i1 = unit.context.types.int(1);
    operands.push(Operand::Constant(unit.context.int(i1, 0)));
    let pointer = unit.context.types.ptr(0);
    let count = operands.len();
    operands.push(Operand::Constant(
        unit.context.constant(llrm_mir::context::Constant { ty: pointer, kind: ConstantKind::Global(callee) }),
    ));
    let aligned = vec![Attribute::Int("align".into(), 1)];
    let argument_attrs = (0..count).map(|at| if at < pointers { aligned.clone() } else { Vec::new() }).collect();
    let info = CallInfo {
        function_type,
        calling_convention: 0,
        return_attrs: Vec::new(),
        argument_attrs,
        attrs: Vec::new(),
        tail: Tail::None,
    };
    let void = unit.context.types.void();
    let made = unit.function.create_instruction(Opcode::Call(Box::new(info)), void, operands, Flags::default(), None);
    unit.function.insert(made, Position::Before(site.call)).expect("a placed call");
    Some(())
}

/// `optimizeMemCpy` and `optimizeMemMove`: the intrinsic, the result `dst`.
pub(super) fn mem_copy(
    unit: &mut Unit,
    site: &Site,
    how: How,
) -> Option<Rewrite> {
    let args = arguments(unit, site);
    let [dst, src, size] = args[..] else { return None };
    copy_call(unit, site, how, dst, src, size)?;
    Some(Rewrite::Value(dst))
}

/// `optimizeMemPCpy`: `llvm.memcpy` and the result `dst + n`.
pub(super) fn mem_p_cpy(
    unit: &mut Unit,
    site: &Site,
) -> Option<Rewrite> {
    let args = arguments(unit, site);
    let [dst, src, size] = args[..] else { return None };
    copy_call(unit, site, How::Apart, dst, src, size)?;
    Some(Rewrite::Value(end_of(unit, site, dst, size)))
}

/// `B.CreateInBoundsGEP(i8, dst, n)`.
fn end_of(
    unit: &mut Unit,
    site: &Site,
    dst: Operand,
    count: Operand,
) -> Operand {
    let i8 = unit.context.types.int(8);
    let ty = unit.function.operand_type(unit.context, dst).expect("a typed pointer");
    let made = unit
        .function
        .create_instruction(Opcode::GetElementPtr { source: i8 }, ty, vec![dst, count], Flags::INBOUNDS, None);
    unit.function.insert(made, Position::Before(site.call)).expect("a placed gep");
    Operand::Value(unit.function.instruction(made).result.expect("a pointer"))
}

/// `optimizeMemSet`: `llvm.memset(align 1 p, (i8)v, n)`, the result `p`.
pub(super) fn mem_set(
    unit: &mut Unit,
    site: &Site,
) -> Option<Rewrite> {
    let args = arguments(unit, site);
    let [pointer, value, size] = args[..] else { return None };
    let bits = width(unit, size)?;
    if !near(unit, pointer) {
        return None;
    }
    let i8 = unit.context.types.int(8);
    let from = width(unit, value)?;
    let byte = if from == 8 {
        value
    } else if from > 8 {
        place(unit, site, Opcode::Cast(CastOp::Trunc), i8, vec![value])
    } else {
        place(unit, site, Opcode::Cast(CastOp::ZExt), i8, vec![value])
    };
    let (callee, function_type) = crate::loopidioms::_memset(unit.context, unit.declared, 0, bits);
    emit_intrinsic(unit, site, callee, function_type, vec![pointer, byte, size], 1)?;
    Some(Rewrite::Value(pointer))
}

/// The value of a constant integer operand.
fn constant_int(
    unit: &Unit,
    operand: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match unit.context.get(id).kind {
        ConstantKind::Int(value) => Some(value),
        _ => None,
    }
}

/// `optimizeMemCCpy`: with a constant count and a constant source, the copy
/// up to and including the stop byte.
pub(super) fn mem_c_cpy(
    unit: &mut Unit,
    site: &Site,
) -> Option<Rewrite> {
    let args = arguments(unit, site);
    let [dst, src, stop, count] = args[..] else { return None };
    let ty = unit.function.instruction(site.call).ty;
    let unused =
        unit.function.instruction(site.call).result.is_none_or(|result| unit.function.users(result).is_empty());
    // memccpy(d, d, c, n) with the result unused is nothing.
    if unused && dst == src {
        return Some(Rewrite::Value(dst));
    }
    let n = constant_int(unit, count)?;
    let null = Operand::Constant(llrm_mir::constfold::null_value(unit.context, ty));
    if n == 0 {
        return Some(Rewrite::Value(null));
    }
    let text = super::constant_array(unit, site.env, src)?;
    let stop = constant_int(unit, stop)?;
    let wanted = (stop & 0xFF) as u8;
    let n = n as usize;
    match text.iter().position(|&byte| byte == wanted) {
        None if n <= text.len() => {
            copy_call(unit, site, How::Apart, dst, src, count)?;
            Some(Rewrite::Value(null))
        }
        None => None,
        Some(at) => {
            let size = (at + 1).min(n);
            let count_ty = unit.function.operand_type(unit.context, count)?;
            let new_n = Operand::Constant(unit.context.int(count_ty, size as i128));
            copy_call(unit, site, How::Apart, dst, src, new_n)?;
            Some(Rewrite::Value(if at + 1 <= n { end_of(unit, site, dst, new_n) } else { null }))
        }
    }
}
