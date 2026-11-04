//! The `strcmp`, `strncmp`, `memcmp` and `bcmp` arms of `LibCallSimplifier`
//! that form a value without a new call: `optimizeStrCmp` (:607),
//! `optimizeStrNCmp` (:668), `optimizeMemCmpVarSize` (:1557),
//! `optimizeMemCmpConstantSize` (:1599) without its equality expansion, and
//! `optimizeMemCmpBCmpCommon` (:1646), at d1106deb71cc.

use std::cmp::Ordering;

use llrm_mir::context::ConstantKind;
use llrm_mir::edit::Position;
use llrm_mir::module::Operand;
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::Unit;
use llrm_mir::types::TypeId;

use super::{Rewrite, Site};

/// `getConstantStringInfo(V, Str, TrimAtNul)` of a pointer that is a constant
/// global.
fn string_info(
    unit: &Unit,
    site: &Site,
    operand: Operand,
    trim_at_nul: bool,
) -> Option<Vec<u8>> {
    let bytes = super::constant_array(unit, site.env, operand)?;
    Some(if trim_at_nul { bytes.into_iter().take_while(|&byte| byte != 0).collect() } else { bytes })
}

/// `std::clamp(A.compare(B), -1, 1)` as a constant of the call's type.
fn ordering(
    unit: &Unit,
    ty: TypeId,
    order: Ordering,
) -> Rewrite {
    Rewrite::Value(Operand::Constant(unit.context.int(ty, order as i128)))
}

fn zero(
    unit: &Unit,
    site: &Site,
) -> Rewrite {
    Rewrite::Value(Operand::Constant(unit.context.int(unit.function.instruction(site.call).ty, 0)))
}

/// The instruction `opcode` placed before the call, as an operand.
fn build(
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

/// `B.CreateZExt(B.CreateLoad(i8, pointer))`, negated for `negate`.
fn first_byte(
    unit: &mut Unit,
    site: &Site,
    pointer: Operand,
    negate: bool,
) -> Rewrite {
    let ty = unit.function.instruction(site.call).ty;
    let byte = unit.context.types.int(8);
    let load = build(unit, site, Opcode::Load { align: Some(1), volatile: false }, byte, vec![pointer]);
    let wide = build(unit, site, Opcode::Cast(CastOp::ZExt), ty, vec![load]);
    if !negate {
        return Rewrite::Value(wide);
    }
    let zero = Operand::Constant(unit.context.int(ty, 0));
    Rewrite::Value(build(unit, site, Opcode::Binary(BinaryOp::Sub), ty, vec![zero, wide]))
}

/// `optimizeStrCmp`; the `memcmp` rewrites are not translated yet.
pub(super) fn str_cmp(
    unit: &mut Unit,
    site: &Site,
) -> Option<Rewrite> {
    let operands = unit.function.instruction(site.call).operands.to_vec();
    let (first, second) = (operands[0], operands[1]);
    if first == second {
        return Some(zero(unit, site));
    }
    let left = string_info(unit, site, first, true);
    let right = string_info(unit, site, second, true);
    let ty = unit.function.instruction(site.call).ty;
    if let (Some(left), Some(right)) = (&left, &right) {
        return Some(ordering(unit, ty, left.cmp(right)));
    }
    if left.as_ref().is_some_and(|text| text.is_empty()) {
        return Some(first_byte(unit, site, second, true));
    }
    if right.as_ref().is_some_and(|text| text.is_empty()) {
        return Some(first_byte(unit, site, first, false));
    }
    None
}

/// The length operand as a constant, `dyn_cast<ConstantInt>(Size)`.
fn constant_length(
    unit: &Unit,
    operand: Operand,
) -> Option<u64> {
    let Operand::Constant(id) = operand else { return None };
    match unit.context.get(id).kind {
        ConstantKind::Int(bits) => Some(bits as u64),
        _ => None,
    }
}

/// `optimizeStrNCmp`; the `memcmp` rewrites are not translated yet.
pub(super) fn str_n_cmp(
    unit: &mut Unit,
    site: &Site,
) -> Option<Rewrite> {
    let operands = unit.function.instruction(site.call).operands.to_vec();
    let (first, second, size) = (operands[0], operands[1], operands[2]);
    if first == second {
        return Some(zero(unit, site));
    }
    let Some(length) = constant_length(unit, size) else {
        return mem_cmp_var_size(unit, site, first, second, size, true);
    };
    if length == 0 {
        return Some(zero(unit, site));
    }
    let left = string_info(unit, site, first, true);
    let right = string_info(unit, site, second, true);
    let ty = unit.function.instruction(site.call).ty;
    let cut = |text: &Vec<u8>| text[..text.len().min(length as usize)].to_vec();
    if let (Some(left), Some(right)) = (&left, &right) {
        return Some(ordering(unit, ty, cut(left).cmp(&cut(right))));
    }
    if left.as_ref().is_some_and(|text| text.is_empty()) {
        return Some(first_byte(unit, site, second, true));
    }
    if right.as_ref().is_some_and(|text| text.is_empty()) {
        return Some(first_byte(unit, site, first, false));
    }
    None
}

/// `optimizeMemCmpVarSize`: a `memcmp` (or `strncmp` for `str_n`) of two
/// constant arrays and a size not constant.
fn mem_cmp_var_size(
    unit: &mut Unit,
    site: &Site,
    left: Operand,
    right: Operand,
    size: Operand,
    str_n: bool,
) -> Option<Rewrite> {
    if left == right {
        return Some(zero(unit, site));
    }
    let left = string_info(unit, site, left, false)?;
    let right = string_info(unit, site, right, false)?;
    let mut at = 0;
    loop {
        if at == left.len().min(right.len()) || (str_n && left[at] == 0 && right[at] == 0) {
            return Some(zero(unit, site));
        }
        if left[at] != right[at] {
            break;
        }
        at += 1;
    }
    let ty = unit.function.instruction(site.call).ty;
    let size_ty = unit.function.operand_type(unit.context, size)?;
    let bound = Operand::Constant(unit.context.int(size_ty, at as i128));
    let flag = unit.context.types.int(1);
    let small = build(unit, site, Opcode::ICmp(IntPredicate::Ule), flag, vec![size, bound]);
    let result = Operand::Constant(unit.context.int(ty, if left[at] < right[at] { -1 } else { 1 }));
    let none = Operand::Constant(unit.context.int(ty, 0));
    Some(Rewrite::Value(build(unit, site, Opcode::Select, ty, vec![small, none, result])))
}

/// `optimizeMemCmpBCmpCommon`; the equality expansion is not translated yet.
pub(super) fn mem_cmp(
    unit: &mut Unit,
    site: &Site,
) -> Option<Rewrite> {
    let operands = unit.function.instruction(site.call).operands.to_vec();
    let (left, right, size) = (operands[0], operands[1], operands[2]);
    if let Some(rewrite) = mem_cmp_var_size(unit, site, left, right, size, false) {
        return Some(rewrite);
    }
    match constant_length(unit, size)? {
        0 => Some(zero(unit, site)),
        1 => {
            let ty = unit.function.instruction(site.call).ty;
            let byte = unit.context.types.int(8);
            let widen = |unit: &mut Unit, pointer| {
                let load = build(unit, site, Opcode::Load { align: Some(1), volatile: false }, byte, vec![pointer]);
                build(unit, site, Opcode::Cast(CastOp::ZExt), ty, vec![load])
            };
            let (left, right) = (widen(unit, left), widen(unit, right));
            Some(Rewrite::Value(build(unit, site, Opcode::Binary(BinaryOp::Sub), ty, vec![left, right])))
        }
        _ => None,
    }
}
