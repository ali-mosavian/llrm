//! CodeGenPrepare's extension promotion: `optimizeExt`, `tryToPromoteExts`,
//! `TypePromotionHelper` and `canFormExtLd`
//! ([CodeGenPrepare.cpp#L7250](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L7250)).
//!
//! # How it works
//! An extension `ext (op a, b)` is moved up through its operand when the
//! operand can be promoted: `op (ext a), (ext b)` computes the same value in
//! the wide type. Done speculatively in a [`Transaction`] and followed up the
//! chain until an extension reaches a load, where the target folds the pair
//! into one extending load; if none reaches one, or the instructions made cost
//! more than the extension saved, the transaction is rolled back.
//!
//! ```text
//!  b1: %a = load i8, ptr %p              b0: %a = load i8, ptr %p
//!      %b = add nsw i8 %a, 1                 %e = sext i8 %a to i32  ; an extending load
//!  b2: %e = sext i8 %b to i32                %w = add nsw i32 %e, 1
//! ```
//!
//! MIRROR: `canGetThrough`, `getAction`, the two promote functions, the
//! cost rule (`TotalCreatedInstsCost > 1`, legality of the promoted
//! instruction, one new extension at most where the extension was not free),
//! `canFormExtLd` and `isExtLoad`, `hasSameExtUse`.
//!
//! DEVIATE EXT1: `isExtLoad` asks `isLoadExtLegal` alone; the pin's
//! `isLoadLegal` also asks whether the access is aligned for the target, which
//! every x86 target allows. Exit: the alignment hook with the DAG switch,
//! 2027-02-04.
//!
//! DEVIATE EXT2: `performAddressTypePromotion` is not translated: only AArch64
//! and RISC-V answer `shouldConsiderAddressTypePromotion` true, x86 does not.
//!
//! # Example
//! `test_a_sign_extended_sum_of_a_load_is_promoted_to_an_extending_load` in
//! `extpromotion_tests.rs` is the sketch above.
//!
//! # Runs
//! In `cgpaddr`'s run, before `optimizeExtUses`, where the target enables the
//! promotion (`enableExtLdPromotion`).

use llrm_mir::context::{Constant, ConstantKind, Context, signed};
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::edit::Position;
use llrm_mir::lowering::{Ext, ExtUser, Isd, LoadExt, Lowering};
use llrm_mir::module::{Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::types::TypeId;

use crate::typepromotion::Transaction;

/// What a promoted instruction's original type was and by which extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Seen {
    Sign,
    Zero,
    Both,
}

/// `PromotedInsts` and `InsertedInsts`.
#[derive(Default)]
pub struct State {
    promoted: IdMap<InstId, (TypeId, Seen)>,
    /// The instructions the other CodeGenPrepare optimizations inserted: not
    /// promoted again, or the two would undo each other.
    pub inserted: IdSet<InstId>,
}

enum Action {
    TruncAndAny,
    Other { sign: bool },
}

struct Env<'a> {
    context: &'a Context,
    layout: &'a llrm_mir::datalayout::DataLayout,
    lowering: &'a dyn Lowering,
}

fn bits_of(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> Option<u32> {
    function.operand_type(context, operand).and_then(|ty| context.types.int_bits(ty))
}

fn instruction_of(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    match function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

fn is_sext(
    function: &Function,
    inst: InstId,
) -> Option<bool> {
    match function.instruction(inst).opcode {
        Opcode::Cast(CastOp::SExt) => Some(true),
        Opcode::Cast(CastOp::ZExt) => Some(false),
        _ => None,
    }
}

fn has_one_use(
    function: &Function,
    inst: InstId,
) -> bool {
    function.instruction(inst).result.is_some_and(|result| function.users(result).has_n_uses(1))
}

fn constant_bits(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(bits) => Some(bits),
        _ => None,
    }
}

/// `addPromotedInst`.
fn add_promoted(
    state: &mut State,
    function: &Function,
    inst: InstId,
    sign: bool,
) {
    let kind = if sign { Seen::Sign } else { Seen::Zero };
    let ty = function.instruction(inst).ty;
    match state.promoted.get_mut(&inst) {
        None => {
            state.promoted.insert(inst, (ty, kind));
        }
        Some(found) if found.1 == kind => {}
        Some(found) => *found = (ty, Seen::Both),
    }
}

/// `getOrigType`.
fn orig_type(
    state: &State,
    inst: InstId,
    sign: bool,
) -> Option<TypeId> {
    let kind = if sign { Seen::Sign } else { Seen::Zero };
    state.promoted.get(&inst).filter(|found| found.1 == kind).map(|found| found.0)
}

/// `TypePromotionHelper::canGetThrough`.
fn can_get_through(
    env: &Env,
    function: &Function,
    inst: InstId,
    considered_bits: u32,
    state: &State,
    sign: bool,
) -> bool {
    let one = function.instruction(inst);
    if env.context.types.int_bits(one.ty).is_none() {
        return false;
    }
    match one.opcode {
        Opcode::Cast(CastOp::ZExt) => return true,
        Opcode::Cast(CastOp::SExt) if sign => return true,
        Opcode::Binary(BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl)
            if (!sign && one.flags.contains(Flags::NUW)) || (sign && one.flags.contains(Flags::NSW)) =>
        {
            return true;
        }
        Opcode::Binary(BinaryOp::And | BinaryOp::Or) => return true,
        Opcode::Binary(BinaryOp::Xor) => {
            if let Some(bits) = constant_bits(env.context, one.operands[1]) {
                let width = env.context.types.int_bits(one.ty).unwrap_or(0);
                let ones = if width >= 128 { u128::MAX } else { (1u128 << width) - 1 };
                if bits & ones != ones {
                    return true;
                }
            }
        }
        Opcode::Binary(BinaryOp::LShr) if !sign => return true,
        _ => {}
    }
    // `(shl x, c)` read by one extension that one `and` with a mask in the
    // narrow type reads.
    if matches!(one.opcode, Opcode::Binary(BinaryOp::Shl)) && has_one_use(function, inst) {
        let result = one.result.expect("a shift has a value");
        let user = function.users(result).only().expect("one use").user;
        if has_one_use(function, user) {
            let user_result = function.instruction(user).result.expect("an extension has a value");
            let and = function.users(user_result).only().expect("one use").user;
            let and = function.instruction(and);
            if matches!(and.opcode, Opcode::Binary(BinaryOp::And))
                && let Some(bits) = constant_bits(env.context, and.operands[1])
            {
                let width = env.context.types.int_bits(one.ty).unwrap_or(0);
                if width >= 128 || bits >> width == 0 {
                    return true;
                }
            }
        }
    }
    if !matches!(one.opcode, Opcode::Cast(CastOp::Trunc)) {
        return false;
    }
    let operand = one.operands[0];
    let Some(operand_bits) = bits_of(env.context, function, operand) else { return false };
    if operand_bits > considered_bits {
        return false;
    }
    let Some(source) = instruction_of(function, operand) else { return false };
    let original = if let Some(ty) = orig_type(state, source, sign) {
        ty
    } else if is_sext(function, source) == Some(sign) {
        function.operand_type(env.context, function.instruction(source).operands[0]).expect("a typed operand")
    } else {
        return false;
    };
    env.context.types.int_bits(one.ty).unwrap_or(0) >= env.context.types.int_bits(original).unwrap_or(u32::MAX)
}

/// `TypePromotionHelper::getAction`.
fn get_action(
    env: &Env,
    function: &Function,
    ext: InstId,
    state: &State,
) -> Option<Action> {
    let sign = is_sext(function, ext)?;
    let ext_ty = function.instruction(ext).ty;
    let ext_bits = env.context.types.int_bits(ext_ty)?;
    let opnd = instruction_of(function, function.instruction(ext).operands[0])?;
    if !can_get_through(env, function, opnd, ext_bits, state, sign) {
        return None;
    }
    if matches!(function.instruction(opnd).opcode, Opcode::Cast(CastOp::Trunc)) && state.inserted.contains(&opnd) {
        return None;
    }
    if matches!(function.instruction(opnd).opcode, Opcode::Cast(_)) {
        return Some(Action::TruncAndAny);
    }
    let narrow = bits_of(env.context, function, Operand::Value(function.instruction(opnd).result?))?;
    if !has_one_use(function, opnd) && !env.lowering.is_truncate_free(ext_bits, narrow) {
        return None;
    }
    Some(Action::Other { sign })
}

fn is_ext_free(
    env: &Env,
    function: &Function,
    inst: InstId,
) -> bool {
    let one = function.instruction(inst);
    let (Opcode::Cast(kind), Some(from), Some(to)) =
        (&one.opcode, bits_of(env.context, function, one.operands[0]), env.context.types.int_bits(one.ty))
    else {
        return false;
    };
    let free = match kind {
        CastOp::ZExt => env.lowering.is_ext_free(Ext::ZExt, from, to),
        CastOp::SExt => env.lowering.is_ext_free(Ext::SExt, from, to),
        _ => return false,
    };
    free || env.lowering.is_ext_free_impl(&ext_users(env, function, inst))
}

/// How each user of the extension `inst` reads it (`isExtFreeImpl`'s loop).
fn ext_users(
    env: &Env,
    function: &Function,
    inst: InstId,
) -> Vec<ExtUser> {
    let one = function.instruction(inst);
    let Some(result) = one.result else { return Vec::new() };
    function
        .users(result)
        .iter()
        .map(|used| {
            let user = function.instruction(used.user);
            match &user.opcode {
                Opcode::Binary(BinaryOp::Shl)
                    if used.index == 0 && matches!(user.operands[1], Operand::Constant(_)) =>
                {
                    ExtUser::ConstShl
                }
                Opcode::Cast(CastOp::Trunc) if function.operand_type(env.context, one.operands[0]) == Some(user.ty) => {
                    ExtUser::SameTypeTrunc
                }
                Opcode::GetElementPtr { source } if used.index >= 1 => {
                    let mut current = *source;
                    for &index in user.operands[2..=used.index as usize].iter() {
                        let types = &env.context.types;
                        current = match types.get(current) {
                            llrm_mir::types::Type::Array { element, .. }
                            | llrm_mir::types::Type::Vector { element, .. } => *element,
                            _ => match index {
                                Operand::Constant(c) => match env.context.get(c).kind {
                                    ConstantKind::Int(at) => types.member(current, at as u64).unwrap_or(current),
                                    _ => return ExtUser::Other,
                                },
                                _ => return ExtUser::Other,
                            },
                        };
                    }
                    ExtUser::Gep { store_bits: env.layout.store_size(&env.context.types, current) * 8 }
                }
                _ => ExtUser::Other,
            }
        })
        .collect()
}

/// `TypePromotionHelper::promoteOperandForTruncAndAnyExt`: the value that
/// replaces `ext`, and the cost of what was made.
fn promote_for_trunc_and_any_ext(
    env: &Env,
    function: &mut Function,
    tpt: &mut Transaction,
    ext: InstId,
    exts: &mut Vec<InstId>,
    cost: &mut u32,
) -> Operand {
    let opnd = instruction_of(function, function.instruction(ext).operands[0]).expect("an instruction operand");
    let ext_ty = function.instruction(ext).ty;
    let mut ext_val = Operand::Value(function.instruction(ext).result.expect("an extension has a value"));
    let mut merged_non_free = false;
    if matches!(function.instruction(opnd).opcode, Opcode::Cast(CastOp::ZExt)) {
        merged_non_free = !is_ext_free(env, function, opnd);
        let source = function.instruction(opnd).operands[0];
        let zext = tpt.create_cast(function, CastOp::ZExt, source, ext_ty, ext);
        tpt.replace_all_uses_with(function, ext, zext);
        tpt.erase_instruction(function, env.context, ext, None);
        ext_val = zext;
    } else {
        let source = function.instruction(opnd).operands[0];
        tpt.set_operand(function, ext, 0, source);
    }
    *cost = 0;
    if function.instruction(opnd).result.is_some_and(|result| function.users(result).is_empty()) {
        tpt.erase_instruction(function, env.context, opnd, None);
    }
    let Some(ext_inst) = instruction_of(function, ext_val) else { return ext_val };
    let one = function.instruction(ext_inst);
    if function.operand_type(env.context, one.operands[0]) != Some(one.ty) {
        exts.push(ext_inst);
        *cost = u32::from(!is_ext_free(env, function, ext_inst) && !merged_non_free);
        return ext_val;
    }
    let next = one.operands[0];
    tpt.erase_instruction(function, env.context, ext_inst, Some(next));
    next
}

/// `TypePromotionHelper::promoteOperandForOther`.
fn promote_for_other(
    env: &Env,
    function: &mut Function,
    tpt: &mut Transaction,
    state: &mut State,
    ext: InstId,
    sign: bool,
    exts: &mut Vec<InstId>,
    cost: &mut u32,
) -> Operand {
    let opnd = instruction_of(function, function.instruction(ext).operands[0]).expect("an instruction operand");
    let ext_ty = function.instruction(ext).ty;
    let ext_bits = env.context.types.int_bits(ext_ty).expect("an integer extension");
    *cost = 0;
    if !has_one_use(function, opnd) {
        let narrow = function.instruction(opnd).ty;
        let trunc = tpt.create_cast(
            function,
            CastOp::Trunc,
            Operand::Value(function.instruction(ext).result.expect("a value")),
            narrow,
            ext,
        );
        if let Some(trunc_inst) = instruction_of(function, trunc) {
            function.move_to(trunc_inst, Position::After(opnd)).expect("a placed instruction moves");
        }
        tpt.replace_all_uses_with(function, opnd, trunc);
        tpt.set_operand(function, ext, 0, Operand::Value(function.instruction(opnd).result.expect("a value")));
    }
    add_promoted(state, function, opnd, sign);
    tpt.mutate_type(function, opnd, ext_ty);
    let ext_value = Operand::Value(function.instruction(ext).result.expect("a value"));
    let opnd_value = Operand::Value(function.instruction(opnd).result.expect("a value"));
    let _ = ext_value;
    tpt.replace_all_uses_with(function, ext, opnd_value);
    let operands = function.instruction(opnd).operands.clone();
    let is_select = matches!(function.instruction(opnd).opcode, Opcode::Select);
    for (index, operand) in operands.into_iter().enumerate() {
        if function.operand_type(env.context, operand) == Some(ext_ty) || (is_select && index == 0) {
            continue;
        }
        if let Some(bits) = constant_bits(env.context, operand) {
            let width = bits_of(env.context, function, operand).unwrap_or(ext_bits);
            let wide = if sign { signed(bits, width) as u128 } else { bits };
            let value = Operand::Constant(
                env.context.constant(Constant { ty: ext_ty, kind: ConstantKind::Int(mask(wide, ext_bits)) }),
            );
            tpt.set_operand(function, opnd, index, value);
            continue;
        }
        if let Operand::Constant(id) = operand
            && matches!(env.context.get(id).kind, ConstantKind::Undef)
        {
            let value = Operand::Constant(env.context.constant(Constant { ty: ext_ty, kind: ConstantKind::Undef }));
            tpt.set_operand(function, opnd, index, value);
            continue;
        }
        let kind = if sign { CastOp::SExt } else { CastOp::ZExt };
        let made = tpt.create_cast(function, kind, operand, ext_ty, opnd);
        tpt.set_operand(function, opnd, index, made);
        if let Some(inst) = instruction_of(function, made) {
            exts.push(inst);
            *cost += u32::from(!is_ext_free(env, function, inst));
        }
    }
    tpt.erase_instruction(function, env.context, ext, None);
    opnd_value
}

fn mask(
    value: u128,
    bits: u32,
) -> u128 {
    if bits >= 128 { value } else { value & ((1u128 << bits) - 1) }
}

/// `isPromotedInstructionLegal`.
fn promoted_is_legal(
    env: &Env,
    function: &Function,
    value: Operand,
) -> bool {
    let Some(inst) = instruction_of(function, value) else { return false };
    let one = function.instruction(inst);
    let op = match one.opcode {
        Opcode::Binary(BinaryOp::Add) => Isd::Add,
        Opcode::Binary(BinaryOp::Sub) => Isd::Sub,
        Opcode::Binary(BinaryOp::Mul) => Isd::Mul,
        Opcode::Binary(BinaryOp::UDiv) => Isd::UDiv,
        Opcode::Binary(BinaryOp::SDiv) => Isd::SDiv,
        Opcode::Binary(BinaryOp::URem) => Isd::URem,
        Opcode::Binary(BinaryOp::SRem) => Isd::SRem,
        Opcode::Binary(BinaryOp::Shl) => Isd::Shl,
        Opcode::Binary(BinaryOp::LShr) => Isd::Srl,
        Opcode::Binary(BinaryOp::AShr) => Isd::Sra,
        Opcode::Binary(BinaryOp::And) => Isd::And,
        Opcode::Binary(BinaryOp::Or) => Isd::Or,
        Opcode::Binary(BinaryOp::Xor) => Isd::Xor,
        Opcode::Select => Isd::Select,
        Opcode::ICmp(_) => Isd::SetCc,
        Opcode::Cast(CastOp::Trunc) => Isd::Truncate,
        Opcode::Cast(CastOp::ZExt) => Isd::ZeroExtend,
        Opcode::Cast(CastOp::SExt) => Isd::SignExtend,
        // No ISD opcode: legal as it was.
        _ => return true,
    };
    env.context.types.int_bits(one.ty).is_some_and(|bits| env.lowering.is_operation_legal_or_custom(op, bits))
}

/// `hasSameExtUse`.
fn has_same_ext_use(
    env: &Env,
    function: &Function,
    inst: InstId,
) -> bool {
    let Some(result) = function.instruction(inst).result else { return false };
    let users: Vec<InstId> = function.users(result).iter().map(|one| one.user).collect();
    let Some(&first) = users.first() else { return false };
    let Some(sign) = is_sext(function, first) else { return false };
    let first_ty = function.instruction(first).ty;
    for user in users {
        if is_sext(function, user) != Some(sign) {
            return false;
        }
        let ty = function.instruction(user).ty;
        if ty == first_ty {
            continue;
        }
        if sign {
            return false;
        }
        let (a, b) = (env.context.types.int_bits(first_ty).unwrap_or(0), env.context.types.int_bits(ty).unwrap_or(0));
        let (narrow, large) = if a > b { (b, a) } else { (a, b) };
        if !env.lowering.is_ext_free(Ext::ZExt, narrow, large) {
            return false;
        }
    }
    true
}

/// `tryToPromoteExts`: whether any was promoted; the extensions that stay in
/// `moved`.
fn try_to_promote_exts(
    env: &Env,
    function: &mut Function,
    tpt: &mut Transaction,
    state: &mut State,
    exts: &[InstId],
    moved: &mut Vec<InstId>,
    created_cost: u32,
) -> bool {
    let mut promoted = false;
    for &ext in exts {
        let source = function.instruction(ext).operands[0];
        if instruction_of(function, source)
            .is_some_and(|one| matches!(function.instruction(one).opcode, Opcode::Load { .. }))
        {
            moved.push(ext);
            continue;
        }
        if !env.lowering.enable_ext_ld_promotion() {
            return false;
        }
        let Some(action) = get_action(env, function, ext, state) else {
            moved.push(ext);
            continue;
        };
        let good = tpt.restoration_point();
        let mut new_exts = Vec::new();
        let mut new_cost = 0;
        let ext_cost = u32::from(!is_ext_free(env, function, ext));
        let value = match action {
            Action::TruncAndAny => promote_for_trunc_and_any_ext(env, function, tpt, ext, &mut new_exts, &mut new_cost),
            Action::Other { sign } => {
                promote_for_other(env, function, tpt, state, ext, sign, &mut new_exts, &mut new_cost)
            }
        };
        let total = (i64::from(created_cost) + i64::from(new_cost) - i64::from(ext_cost)).max(0);
        if total > 1 || !promoted_is_legal(env, function, value) || (ext_cost == 0 && new_exts.len() > 1) {
            tpt.rollback(function, good);
            moved.push(ext);
            continue;
        }
        let mut newly_moved = Vec::new();
        try_to_promote_exts(env, function, tpt, state, &new_exts, &mut newly_moved, total as u32);
        let mut new_promoted = false;
        for moved_ext in newly_moved {
            let operand = function.instruction(moved_ext).operands[0];
            if let Some(load) = instruction_of(function, operand)
                && matches!(function.instruction(load).opcode, Opcode::Load { .. })
                && !(new_cost <= ext_cost || has_one_use(function, load) || has_same_ext_use(env, function, load))
            {
                continue;
            }
            moved.push(moved_ext);
            new_promoted = true;
        }
        if !new_promoted {
            tpt.rollback(function, good);
            moved.push(ext);
            continue;
        }
        promoted = true;
    }
    promoted
}

/// `isExtLoad`: whether the target folds `ext` into `load`.
fn is_ext_load(
    env: &Env,
    function: &Function,
    load: InstId,
    ext: InstId,
) -> bool {
    let Some(sign) = is_sext(function, ext) else { return false };
    let (Some(ext_bits), Some(load_bits)) = (
        env.context.types.int_bits(function.instruction(ext).ty),
        env.context.types.int_bits(function.instruction(load).ty),
    ) else {
        return false;
    };
    if !has_one_use(function, load)
        && (env.lowering.is_type_legal(load_bits) || !env.lowering.is_type_legal(ext_bits))
        && !env.lowering.is_truncate_free(ext_bits, load_bits)
    {
        return false;
    }
    env.lowering.is_load_ext_legal(if sign { LoadExt::Sign } else { LoadExt::Zero }, ext_bits, load_bits)
}

/// `optimizeExt`: whether the extension (or what it was promoted to) now
/// stands after the load that feeds it.
pub fn optimize_ext(
    context: &Context,
    layout: &llrm_mir::datalayout::DataLayout,
    lowering: &dyn Lowering,
    function: &mut Function,
    state: &mut State,
    ext: InstId,
) -> bool {
    let env = Env { context, layout, lowering };
    let mut tpt = Transaction::new();
    let good = tpt.restoration_point();
    let mut moved = Vec::new();
    let promoted = try_to_promote_exts(&env, function, &mut tpt, state, &[ext], &mut moved, 0);
    let fed = moved
        .iter()
        .copied()
        .find(
            |&one| instruction_of(function, function.instruction(one).operands[0])
                .is_some_and(|load| matches!(function.instruction(load).opcode, Opcode::Load { .. })),
        );
    if let Some(fed) = fed {
        let load = instruction_of(function, function.instruction(fed).operands[0]).expect("a load");
        let same_block = function.parent(load) == function.parent(fed);
        if (promoted || !same_block) && is_ext_load(&env, function, load, fed) {
            tpt.commit(function);
            let _ = function.move_to(fed, Position::After(load));
            return true;
        }
    }
    tpt.rollback(function, good);
    false
}

#[cfg(test)]
#[path = "extpromotion_tests.rs"]
mod tests;
