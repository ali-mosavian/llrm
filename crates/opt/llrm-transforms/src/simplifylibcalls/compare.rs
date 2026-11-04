//! The `strcmp`, `strncmp`, `memcmp` and `bcmp` arms of `LibCallSimplifier`:
//! `optimizeStrCmp` (:607), `optimizeStrNCmp` (:668),
//! `optimizeMemCmpVarSize` (:1557), `optimizeMemCmpConstantSize` (:1599),
//! `optimizeMemCmpBCmpCommon` (:1646), `optimizeMemCmp` (:1664) and
//! `canTransformToMemCmp` (:296), at d1106deb71cc.

use std::cmp::Ordering;

use llrm_mir::constantfolding::constant_fold_load_from_const_ptr;
use llrm_mir::context::ConstantKind;
use llrm_mir::edit::Position;
use llrm_mir::module::Operand;
use llrm_mir::opcode::{Attribute, BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Outer, Unit};
use llrm_mir::types::TypeId;
use llrm_mir::valuetracking::{self, Query, Sizes};

use super::{NewCall, Rewrite, Site};

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

/// `GetStringLength`: the length with the NUL of a constant string, 0 where
/// the array has no NUL.
fn string_length(
    unit: &Unit,
    site: &Site,
    operand: Operand,
) -> u64 {
    match super::constant_array(unit, site.env, operand) {
        Some(bytes) => bytes.iter().position(|&byte| byte == 0).map_or(0, |end| end as u64 + 1),
        None => 0,
    }
}

/// `isOnlyUsedInComparisonWithZero`: every user is an `icmp` against zero.
fn only_compared_with_zero(
    unit: &Unit,
    site: &Site,
) -> bool {
    let Some(result) = unit.function.instruction(site.call).result else { return true };
    unit.function.users(result).iter().all(|used| {
        let user = unit.function.instruction(used.user);
        let zero = |operand: Operand| {
            matches!(
                operand,
                Operand::Constant(id) if matches!(unit.context.get(id).kind, ConstantKind::Int(0) | ConstantKind::Null)
            )
        };
        matches!(user.opcode, Opcode::ICmp(_)) && zero(user.operands[1])
    })
}

/// `canTransformToMemCmp`: `len` bytes at `string` can be read whether or not
/// the program would.
fn can_transform_to_mem_cmp(
    unit: &Unit,
    site: &Site,
    outer: &Outer,
    string: Operand,
    len: u64,
) -> bool {
    if !only_compared_with_zero(unit, site) {
        return false;
    }
    // The module's globals are measured once, not once for each function.
    let sizes = outer.memo::<Sizes>(0, || valuetracking::sizes_of(unit.context, site.env.globals, unit.layout));
    if !valuetracking::dereferenceable(unit.context, unit.layout, &sizes, unit.function, string, len) {
        return false;
    }
    !unit.function.attrs.iter().any(|attr| matches!(attr, Attribute::Flag(name) if name == "sanitize_memory"))
}

/// `emitMemCmp` (`emitBCmp` for `bcmp`) of the call's two pointers and `len`.
fn mem_cmp_call(
    unit: &Unit,
    site: &Site,
    standard: &'static str,
    pointers: [Operand; 2],
    len: Operand,
) -> Rewrite {
    let pointer = unit.context.types.ptr(0);
    Rewrite::Call(NewCall {
        standard,
        args: vec![pointers[0], pointers[1], len],
        returns: site.int,
        params: vec![pointer, pointer, site.size],
    })
}

fn size_constant(
    unit: &Unit,
    site: &Site,
    len: u64,
) -> Operand {
    Operand::Constant(unit.context.int(site.size, i128::from(len)))
}

/// `optimizeStrCmp`
pub(super) fn str_cmp(
    unit: &mut Unit,
    site: &Site,
    outer: &Outer,
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
    // strcmp(P, "x") -> memcmp(P, "x", 2); the arm for two lengths known
    // without two strings needs a PHI or select, which `constant_array` does
    // not look through (D3).
    let (len1, len2) = (string_length(unit, site, first), string_length(unit, site, second));
    let (pointer, len) = match (&left, &right) {
        (None, Some(_)) => (first, len2),
        (Some(_), None) => (second, len1),
        _ => return None,
    };
    if !can_transform_to_mem_cmp(unit, site, outer, pointer, len) {
        return None;
    }
    let len = size_constant(unit, site, len);
    Some(mem_cmp_call(unit, site, "memcmp", [first, second], len))
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
    outer: &Outer,
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
    if length == 1 {
        return Some(mem_cmp_call(unit, site, "memcmp", [first, second], size));
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
    // strncmp to memcmp
    let (pointer, len) = match (&left, &right) {
        (None, Some(_)) => (first, string_length(unit, site, second).min(length)),
        (Some(_), None) => (second, string_length(unit, site, first).min(length)),
        _ => return None,
    };
    if !can_transform_to_mem_cmp(unit, site, outer, pointer, len) {
        return None;
    }
    let len = size_constant(unit, site, len);
    Some(mem_cmp_call(unit, site, "memcmp", [first, second], len))
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
    let result = if left[at] < right[at] { -1 } else { 1 };
    // IRBuilder folds the compare and the select of a constant size.
    if let Some(known) = constant_length(unit, size) {
        let value = if known <= at as u64 { 0 } else { result };
        return Some(Rewrite::Value(Operand::Constant(unit.context.int(ty, value))));
    }
    let size_ty = unit.function.operand_type(unit.context, size)?;
    let bound = Operand::Constant(unit.context.int(size_ty, at as i128));
    let flag = unit.context.types.int(1);
    let small = build(unit, site, Opcode::ICmp(IntPredicate::Ule), flag, vec![size, bound]);
    let result = Operand::Constant(unit.context.int(ty, result));
    let none = Operand::Constant(unit.context.int(ty, 0));
    Some(Rewrite::Value(build(unit, site, Opcode::Select, ty, vec![small, none, result])))
}

/// `optimizeMemCmpConstantSize`'s `memcmp(S1, S2, N/8) == 0` arm: one integer
/// compare where the target has that integer and both pointers are aligned.
fn equality_expansion(
    unit: &mut Unit,
    site: &Site,
    left: Operand,
    right: Operand,
    len: u64,
) -> Option<Rewrite> {
    let bits = u32::try_from(len.checked_mul(8)?).ok()?;
    if !unit.layout.legal_integer(bits) || !equality_only(unit, site) {
        return None;
    }
    let ty = unit.context.types.int(bits);
    let preferred = unit.layout.pref_align(&unit.context.types, ty);
    // First, see if either argument folds to a constant; a constant has no
    // alignment to ask.
    let folded = |operand: Operand| match operand {
        Operand::Constant(id) => constant_fold_load_from_const_ptr(unit.context, unit.layout, site.env.globals, id, ty)
            .map(Operand::Constant),
        _ => None,
    };
    let (left_value, right_value) = (folded(left), folded(right));
    let aligned = |operand: Operand, value: Option<Operand>| {
        value.is_some()
            || valuetracking::alignment(unit.context, unit.layout, site.env.globals, unit.function, operand)
                >= preferred
    };
    if !aligned(left, left_value) || !aligned(right, right_value) {
        return None;
    }
    let load = |unit: &mut Unit, pointer: Operand, value: Option<Operand>| {
        value.unwrap_or_else(|| {
            build(unit, site, Opcode::Load { align: Some(preferred), volatile: false }, ty, vec![pointer])
        })
    };
    let left = load(unit, left, left_value);
    let right = load(unit, right, right_value);
    let flag = unit.context.types.int(1);
    let differ = build(unit, site, Opcode::ICmp(IntPredicate::Ne), flag, vec![left, right]);
    let result = unit.function.instruction(site.call).ty;
    Some(Rewrite::Value(build(unit, site, Opcode::Cast(CastOp::ZExt), result, vec![differ])))
}

/// `isOnlyUsedInZeroEqualityComparison(CI)`.
fn equality_only(
    unit: &Unit,
    site: &Site,
) -> bool {
    valuetracking::known::is_only_used_in_zero_equality_comparison(
        &Query::bare(unit.context, unit.function, unit.layout),
        site.call,
    )
}

/// `optimizeMemCmp` (`optimizeBCmp` for `bcmp`, the same without the last
/// step): `optimizeMemCmpBCmpCommon`, then `memcmp(x, y, n) == 0` as
/// `bcmp`.
pub(super) fn mem_cmp(
    unit: &mut Unit,
    site: &Site,
    memcmp: bool,
) -> Option<Rewrite> {
    let operands = unit.function.instruction(site.call).operands.to_vec();
    let (left, right, size) = (operands[0], operands[1], operands[2]);
    if let Some(rewrite) = mem_cmp_var_size(unit, site, left, right, size, false) {
        return Some(rewrite);
    }
    if let Some(len) = constant_length(unit, size) {
        match len {
            0 => return Some(zero(unit, site)),
            1 => {
                let ty = unit.function.instruction(site.call).ty;
                let byte = unit.context.types.int(8);
                let widen = |unit: &mut Unit, pointer| {
                    let load = build(unit, site, Opcode::Load { align: Some(1), volatile: false }, byte, vec![pointer]);
                    build(unit, site, Opcode::Cast(CastOp::ZExt), ty, vec![load])
                };
                let (left, right) = (widen(unit, left), widen(unit, right));
                return Some(Rewrite::Value(build(unit, site, Opcode::Binary(BinaryOp::Sub), ty, vec![left, right])));
            }
            _ => {
                if let Some(rewrite) = equality_expansion(unit, site, left, right, len) {
                    return Some(rewrite);
                }
            }
        }
    }
    // memcmp(x, y, Len) == 0 -> bcmp(x, y, Len) == 0
    if memcmp && equality_only(unit, site) {
        return Some(mem_cmp_call(unit, site, "bcmp", [left, right], size));
    }
    None
}
