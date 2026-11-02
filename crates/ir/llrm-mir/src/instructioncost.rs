//! `TargetTransformInfo::getInstructionCost`: what one instruction costs under
//! a cost kind, asked of the target's [`Tti`](crate::tti::Tti).
//!
//! # How it works
//! The instruction's opcode picks the query, as the switch of
//! `TargetTransformInfoImplCRTPBase::getInstructionCost` does: arithmetic,
//! casts, compares, selects, loads, stores and control flow go to the
//! per-operation costs of the target; a GEP is free where its address is a
//! legal addressing mode; a call is a basic cost per argument, or an
//! intrinsic's own price. The operands' constness and powers of two are
//! read off the instruction (`TTI::getOperandInfo`).
//!
//! A type the cost model does not hold (a vector, an aggregate: T1 of the plan
//! page) makes the cost invalid, as the target's own answer for it is.
//!
//! # Used by
//! [`CodeMetrics`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/CodeMetrics.cpp#L117)
//! readers: loop rotation's header size, the unroll cost, `peelsize`.
//!
//! # See also
//! [`getInstructionCost`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/TargetTransformInfoImpl.h#L1464);
//! the plan page `tti-plan`.

use crate::context::{ConstantKind, Context};
use crate::datalayout::DataLayout;
use crate::module::{Function, GlobalValue, InstId, Linkage, Operand, ValueDef};
use crate::opcode::{BinaryOp, CastOp, Opcode};
use crate::tti::{ArithOp, CastKind, CmpSel, Cost, CostKind, MemoryOp, OperandInfo, Tti, Ty};
use crate::types::{Type, TypeId};

/// `TTI::getOperandInfo`: what is known of an operand's value.
fn operand_info(
    context: &Context,
    operand: Operand,
) -> OperandInfo {
    let Operand::Constant(id) = operand else { return OperandInfo::ANY };
    let constant = context.get(id);
    match (&constant.kind, context.types.get(constant.ty)) {
        (ConstantKind::Int(bits), Type::Int(width)) => OperandInfo::constant(*bits as i128, *width),
        (ConstantKind::Zero, Type::Int(width)) => OperandInfo::constant(0, *width),
        (ConstantKind::Float(_), _) => OperandInfo { constant: true, ..OperandInfo::ANY },
        _ => OperandInfo::ANY,
    }
}

fn operand_type(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> Option<TypeId> {
    match operand {
        Operand::Value(value) => Some(function.value(value).ty),
        Operand::Constant(id) => Some(context.get(id).ty),
        Operand::Block(_) => None,
    }
}

fn arith_of(op: BinaryOp) -> ArithOp {
    match op {
        BinaryOp::Add => ArithOp::Add,
        BinaryOp::Sub => ArithOp::Sub,
        BinaryOp::Mul => ArithOp::Mul,
        BinaryOp::UDiv => ArithOp::UDiv,
        BinaryOp::SDiv => ArithOp::SDiv,
        BinaryOp::URem => ArithOp::URem,
        BinaryOp::SRem => ArithOp::SRem,
        BinaryOp::Shl => ArithOp::Shl,
        BinaryOp::LShr => ArithOp::LShr,
        BinaryOp::AShr => ArithOp::AShr,
        BinaryOp::And => ArithOp::And,
        BinaryOp::Or => ArithOp::Or,
        BinaryOp::Xor => ArithOp::Xor,
        BinaryOp::FAdd => ArithOp::FAdd,
        BinaryOp::FSub => ArithOp::FSub,
        BinaryOp::FMul => ArithOp::FMul,
        BinaryOp::FDiv => ArithOp::FDiv,
        BinaryOp::FRem => ArithOp::FRem,
    }
}

fn cast_of(op: CastOp) -> Option<CastKind> {
    Some(match op {
        CastOp::Trunc => CastKind::Trunc,
        CastOp::ZExt => CastKind::ZExt,
        CastOp::SExt => CastKind::SExt,
        CastOp::FPTrunc => CastKind::FPTrunc,
        CastOp::FPExt => CastKind::FPExt,
        CastOp::FPToUI => CastKind::FPToUI,
        CastOp::FPToSI => CastKind::FPToSI,
        CastOp::UIToFP => CastKind::UIToFP,
        CastOp::SIToFP => CastKind::SIToFP,
        CastOp::PtrToInt => CastKind::PtrToInt,
        CastOp::IntToPtr => CastKind::IntToPtr,
        CastOp::BitCast => CastKind::BitCast,
        CastOp::FPRound | CastOp::AddrSpaceCast => return None,
    })
}

/// A `select` of booleans with a constant arm is a logic operation, not a
/// choice: `select x, y, false` is `x & y` and `select x, true, y` is `x | y`.
/// The operation and the arm it takes as its second operand.
pub fn select_logic(
    context: &Context,
    ty: TypeId,
    on_true: Operand,
    on_false: Operand,
) -> Option<(ArithOp, Operand)> {
    if !matches!(context.types.get(ty), Type::Int(1)) {
        return None;
    }
    if constant_int(context, on_false) == Some(0) {
        Some((ArithOp::And, on_true))
    } else if constant_int(context, on_true) == Some(1) {
        Some((ArithOp::Or, on_false))
    } else {
        None
    }
}

/// The cost of a `select` of type `ty` between `on_true` and `on_false`.
pub fn select_cost(
    context: &Context,
    tti: &dyn Tti,
    ty: TypeId,
    condition: Operand,
    on_true: Operand,
    on_false: Operand,
    kind: CostKind,
) -> Cost {
    let Some(result) = Ty::of(&context.types, ty) else { return Cost::INVALID };
    match select_logic(context, ty, on_true, on_false) {
        Some((op, second)) => {
            tti.arithmetic_cost(op, result, kind, operand_info(context, condition), operand_info(context, second))
        }
        None => tti.cmp_sel_cost(CmpSel::Select, result, kind),
    }
}

/// `isa<Constant>`-and-a-small-integer: the value of an integer constant.
fn constant_int(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(bits) => Some(bits),
        ConstantKind::Zero => Some(0),
        _ => None,
    }
}

/// `AllocaInst::isStaticAlloca`: a constant size, in the entry block.
fn is_static_alloca(
    function: &Function,
    inst: InstId,
) -> bool {
    let sized = function.instruction(inst).operands.first().is_none_or(|count| matches!(count, Operand::Constant(_)));
    sized && function.layout().first().is_some_and(|&entry| function.parent(inst) == Some(entry))
}

/// `getGEPCost` of `TargetTransformInfoImplCRTPBase`: free where the address
/// is a legal addressing mode, a basic cost where it is not.
fn gep_cost(
    tti: &dyn Tti,
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    operands: &[Operand],
    source: TypeId,
    access_bytes: Option<u64>,
) -> Cost {
    let base_global = matches!(
        operands[0],
        Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Global(_))
    );
    if operands.len() == 1 {
        return if base_global { Cost::BASIC } else { Cost::FREE };
    }
    let indices: Vec<Option<i128>> = operands[1..]
        .iter()
        .map(|&index| {
            constant_int(context, index).map(|bits| {
                let width = operand_type(context, function, index)
                    .and_then(|ty| match context.types.get(ty) {
                        Type::Int(width) => Some(*width),
                        _ => None,
                    })
                    .unwrap_or(64);
                // `sextOrTrunc` of the index to the pointer's width.
                let shift = 128 - width.min(128);
                ((bits << shift) as i128) >> shift
            })
        })
        .collect();
    let (offset, variable) = layout.collect_offset(&context.types, source, &indices);
    // No addressing mode takes two scale registers.
    if variable.len() > 1 {
        return Cost::BASIC;
    }
    let scale = variable.first().map_or(0, |&(_, scale)| scale as i64);
    // A scaled index that costs instructions of its own (real mode's prefixed
    // form) is not free in the address.
    let folds = scale <= 1 || tti.scaled_index_insns() == 0;
    let legal = match access_bytes {
        Some(bytes) => tti.is_legal_addressing_mode_for(bytes, base_global, offset as i64, !base_global, scale),
        None => tti.is_legal_addressing_mode(base_global, offset as i64, !base_global, scale),
    };
    if folds && legal { Cost::FREE } else { Cost::BASIC }
}

/// The bytes the load or store that is the only user of the GEP `inst`
/// accesses, as the `AccessType` `getInstructionCost` gives `getGEPCost` ("only
/// in the simple case where the GEP has one user"); none for any other user.
fn sole_access_bytes(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    inst: InstId,
) -> Option<u64> {
    let value = function.instruction(inst).result?;
    let user = function.users(value).only()?.user;
    let reader = function.instruction(user);
    let accessed = match reader.opcode {
        Opcode::Load { .. } if reader.operands[0] == Operand::Value(value) => reader.ty,
        Opcode::Store { .. } if reader.operands[1] == Operand::Value(value) => {
            operand_type(context, function, reader.operands[0])?
        }
        _ => return None,
    };
    Some(layout.store_size(&context.types, accessed))
}

/// Whether `pointer` is a GEP with an index that is not a constant.
fn stores_through_index(
    function: &Function,
    pointer: Operand,
) -> bool {
    let Operand::Value(value) = pointer else { return false };
    let ValueDef::Instruction(def) = function.value(value).def else { return false };
    let gep = function.instruction(def);
    matches!(gep.opcode, Opcode::GetElementPtr { .. })
        && !gep.operands[1..].iter().all(|operand| matches!(operand, Operand::Constant(_)))
}

/// `getInstructionCost(inst, kind)`.
#[allow(clippy::too_many_arguments)]
pub fn instruction_cost(
    tti: &dyn Tti,
    context: &Context,
    layout: &DataLayout,
    globals: &[GlobalValue],
    function: &Function,
    inst: InstId,
    kind: CostKind,
) -> Cost {
    instruction_cost_with(tti, context, layout, globals, function, inst, &function.instruction(inst).operands, kind)
}

/// `getInstructionCost(inst, Operands, kind)`: the cost of `inst` as if its
/// operands were `operands`, which a caller that has simplified some of them
/// to constants asks (`LoopUnrollPass`'s `analyzeLoopUnrollCost`).
#[allow(clippy::too_many_arguments)]
pub fn instruction_cost_with(
    tti: &dyn Tti,
    context: &Context,
    layout: &DataLayout,
    globals: &[GlobalValue],
    function: &Function,
    inst: InstId,
    operands: &[Operand],
    kind: CostKind,
) -> Cost {
    let operation = function.instruction(inst);
    let modelled =
        |operand: Operand| operand_type(context, function, operand).and_then(|ty| Ty::of(&context.types, ty));
    let result = Ty::of(&context.types, operation.ty);
    let info = |at: usize| operand_info(context, operands[at]);
    match &operation.opcode {
        Opcode::Call(call) | Opcode::Invoke(call) => {
            let arguments = operands.len().saturating_sub(1) as i64;
            let callee = crate::memory::callee(context, function, inst).and_then(|id| globals.get(id.0 as usize));
            let Some(callee) = callee else { return Cost::BASIC * (arguments + 1) };
            let name = callee.name.as_deref();
            if name.is_some_and(|name| name.starts_with("llvm.")) {
                let arguments: Option<Vec<Ty>> =
                    operands[..operands.len() - 1].iter().map(|&one| modelled(one)).collect();
                let ret = result;
                return match arguments {
                    Some(arguments) => tti.intrinsic_cost(name.unwrap_or(""), ret, &arguments, kind),
                    None => Cost::INVALID,
                };
            }
            let local = matches!(callee.linkage, Linkage::Internal | Linkage::Private);
            if !tti.is_lowered_to_call(name, local) {
                return Cost::BASIC;
            }
            let parameters = match context.types.get(call.function_type) {
                Type::Function { parameters, .. } => parameters.len() as i64,
                _ => arguments,
            };
            Cost::BASIC * (parameters + 1)
        }
        Opcode::Br | Opcode::Ret | Opcode::Switch => tti.cf_instr_cost(false, kind),
        Opcode::Phi => tti.cf_instr_cost(true, kind),
        Opcode::Freeze => Cost::FREE,
        Opcode::ExtractValue(_) | Opcode::InsertValue(_) => {
            if matches!(operation.opcode, Opcode::InsertValue(_)) && kind != CostKind::RecipThroughput {
                Cost::BASIC
            } else {
                Cost::FREE
            }
        }
        Opcode::Alloca { .. } => {
            if is_static_alloca(function, inst) {
                Cost::FREE
            } else {
                Cost::BASIC
            }
        }
        Opcode::GetElementPtr { source } => {
            let access = sole_access_bytes(context, layout, function, inst);
            gep_cost(tti, context, layout, function, operands, *source, access)
        }
        Opcode::Binary(op) => match result {
            Some(ty) => tti.arithmetic_cost(arith_of(*op), ty, kind, info(0), info(1)),
            None => Cost::INVALID,
        },
        Opcode::FNeg => match result {
            Some(ty) => tti.arithmetic_cost(ArithOp::FNeg, ty, kind, info(0), OperandInfo::ANY),
            None => Cost::INVALID,
        },
        // An address space cast between spaces below 256 makes no code on x86.
        Opcode::Cast(CastOp::AddrSpaceCast) => Cost::FREE,
        Opcode::Cast(op) => match (cast_of(*op), result, modelled(operands[0])) {
            (Some(op), Some(dst), Some(src)) => {
                let of_load = matches!(
                    operands[0],
                    Operand::Value(value) if matches!(function.value(value).def, ValueDef::Instruction(def) if matches!(function.instruction(def).opcode, Opcode::Load { .. }))
                );
                if of_load && let Some(cost) = tti.cast_of_load_cost(op, dst, src, kind) {
                    return cost;
                }
                tti.cast_cost(op, dst, src, kind)
            }
            (None, ..) => Cost::BASIC,
            _ => Cost::INVALID,
        },
        Opcode::Store { .. }
            if stores_through_index(function, operands[1]) && tti.indexed_store_cost(kind).is_some() =>
        {
            tti.indexed_store_cost(kind).expect("a price")
        }
        // BasicTTIImpl prices the other instructions at one unit.
        Opcode::Atomic(_) => Cost::BASIC,
        Opcode::Store { .. } => match modelled(operands[0]) {
            Some(ty) => tti.memory_op_cost(MemoryOp::Store, ty, kind, info(0).constant),
            None => Cost::INVALID,
        },
        Opcode::Load { .. } => {
            let mut loaded = operation.ty;
            // A load whose only use is a trunc is priced as the narrower load,
            // for code size.
            if kind == CostKind::CodeSize
                && let Some(value) = operation.result
                && let Some(only) = function.users(value).only()
                && let Opcode::Cast(CastOp::Trunc) = function.instruction(only.user).opcode
            {
                loaded = function.instruction(only.user).ty;
            }
            match Ty::of(&context.types, loaded) {
                Some(ty) => tti.memory_op_cost(MemoryOp::Load, ty, kind, false),
                None => Cost::INVALID,
            }
        }
        Opcode::Select => select_cost(context, tti, operation.ty, operands[0], operands[1], operands[2], kind),
        Opcode::ICmp(_) | Opcode::FCmp(_) => {
            let op = if matches!(operation.opcode, Opcode::ICmp(_)) { CmpSel::ICmp } else { CmpSel::FCmp };
            match modelled(operands[0]) {
                Some(ty) => tti.cmp_sel_cost(op, ty, kind),
                None => Cost::INVALID,
            }
        }
        Opcode::Unreachable | Opcode::Resume | Opcode::LandingPad { .. } => Cost::BASIC,
    }
}
