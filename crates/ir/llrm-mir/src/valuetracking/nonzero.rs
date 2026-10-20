//! `isKnownNonZero`, `isKnownNonEqual`.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ValueTracking.cpp`
//! (`isKnownNonZeroFromOperator` and the `isNonZero*` helpers,
//! `isKnownNonNullFromDominatingCondition`, `isKnownNonEqual`), each function
//! linking the line it translates. Scalars only: the `DemandedElts` argument
//! is gone, and so are the vector cases (`insertelement`, `extractelement`,
//! `shufflevector`: 3587-3640; `vector_reverse` and `vector_reduce_*`:
//! 3729-3741; constant vectors: 3855-3866).
//!
//! Where MIR lacks what the C++ reads, the case is skipped:
//! - `ptrtoaddr` (3394-3398) and `ConstantPtrAuth` (3869-3870): MIR has
//!   neither.
//! - `isAbsoluteSymbolRef` (3876-3877): MIR keeps no `!absolute_symbol`.
//! - `Q.DC`, the dominating-condition cache (4223-4244): the query has none, as
//!   the `SimplifyQuery` of the oracle.
//!
//! `Operator` is an instruction or a `ConstantExpr` MIR has (`getelementptr`
//! and casts).
//!
//! # Example
//! `or i32 %x, 1` is non-zero.

use std::borrow::Cow;

use super::pattern::{
    self, CmpPredicate, IntrinsicId, Side, arg_size, call_info, call_range, called_function, range_attribute,
    returned_argument_operand,
};
use super::query::Query;
use super::{MAX_ANALYSIS_RECURSION_DEPTH, context, guarantee, known, power2};
use crate::apint::{ashr, bit, lshr, popcount, sext, signed};
use crate::constantrange::ConstantRange;
use crate::context::{ConstantExpr, ConstantKind, mask};
use crate::domtree::DomTree;
use crate::knownbits::KnownBits;
use crate::module::{BlockId, Function, InstId, Instruction, Linkage, MetadataOperand, Operand, ValueDef};
use crate::opcode::{Attribute, BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use crate::types::{Type, TypeId};

/// `DomConditionsMaxUses`.
const DOM_CONDITIONS_MAX_USES: usize = 20;

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L781>
pub fn cmp_excludes_zero(
    q: &Query,
    pred: IntPredicate,
    rhs: Operand,
) -> bool {
    if pred == IntPredicate::Ugt {
        return true;
    }
    if pred == IntPredicate::Ne {
        return pattern::zero(q, rhs);
    }
    if let Some((c, width)) = pattern::apint(q, rhs) {
        return !ConstantRange::make_exact_icmp_region(pred, width, c).contains(0);
    }
    false
}

/// `dyn_cast<Operator>(V)`: an instruction, or a constant expression written
/// as the instruction it is.
struct Operator<'a> {
    instruction: Cow<'a, Instruction>,
    inst: Option<InstId>,
}

fn operator<'a>(
    q: &Query<'a>,
    v: Operand,
) -> Option<Operator<'a>> {
    match v {
        Operand::Value(_) => q
            .instruction_of(v)
            .map(|(inst, instruction)| Operator { instruction: Cow::Borrowed(instruction), inst: Some(inst) }),
        Operand::Constant(id) => {
            let constant = q.context.get(id);
            let (opcode, operands, flags) = match &constant.kind {
                ConstantKind::Expr(ConstantExpr::GetElementPtr { source, inbounds, operands, .. }) => (
                    Opcode::GetElementPtr { source: *source },
                    operands.iter().map(|one| Operand::Constant(*one)).collect(),
                    if *inbounds { Flags::INBOUNDS } else { Flags::default() },
                ),
                ConstantKind::Expr(ConstantExpr::Cast { op, value }) => {
                    (Opcode::Cast(*op), vec![Operand::Constant(*value)], Flags::default())
                }
                _ => return None,
            };
            let instruction = Instruction {
                opcode,
                ty: constant.ty,
                operands,
                flags,
                result: None,
                metadata: Vec::new(),
                link: (0, 0),
            };
            Some(Operator { instruction: Cow::Owned(instruction), inst: None })
        }
        Operand::Block(_) => None,
    }
}

/// `getBitWidth(Ty, DL)`: an integer's width or a pointer's, 0 for anything
/// else.
fn type_bits(
    q: &Query,
    ty: TypeId,
) -> u32 {
    match q.context.types.get(ty) {
        Type::Int(bits) => *bits,
        Type::Pointer(space) => q.layout.pointer(*space).bits,
        _ => 0,
    }
}

fn pointer_space(
    q: &Query,
    ty: TypeId,
) -> Option<u32> {
    match q.context.types.get(ty) {
        Type::Pointer(space) => Some(*space),
        _ => None,
    }
}

/// `NullPointerIsDefined(F, AS)`.
pub fn null_pointer_is_defined(
    function: Option<&Function>,
    space: u32,
) -> bool {
    function.is_some_and(|function| has_flag(&function.attrs, "null_pointer_is_valid")) || space != 0
}

fn has_flag(
    attrs: &[Attribute],
    name: &str,
) -> bool {
    attrs.iter().any(|one| matches!(one, Attribute::Flag(flag) if flag == name))
}

fn int_attr(
    attrs: &[Attribute],
    name: &str,
) -> Option<u64> {
    attrs
        .iter()
        .find_map(
            |one| match one {
                Attribute::Int(key, value) if key == name => Some(*value),
                _ => None,
            },
        )
}

fn has_type_attr(
    attrs: &[Attribute],
    name: &str,
) -> bool {
    attrs.iter().any(|one| matches!(one, Attribute::Type(key, _) if key == name))
}

/// `CallBase::hasRetAttr(Kind)`.
fn ret_has_attr(
    q: &Query,
    instruction: &Instruction,
    name: &str,
) -> bool {
    call_info(instruction).is_some_and(|info| has_flag(&info.return_attrs, name))
        || called_function(q, instruction).is_some_and(|function| has_flag(&function.return_attrs, name))
}

/// `CallBase::paramHasAttr(ArgNo, Kind)` for the attributes that are flags.
fn param_has_attr(
    q: &Query,
    instruction: &Instruction,
    arg: usize,
    name: &str,
) -> bool {
    call_info(instruction).is_some_and(|info| info.argument_attrs.get(arg).is_some_and(|attrs| has_flag(attrs, name)))
        || called_function(q, instruction)
            .is_some_and(|function| function.parameter_attrs.get(arg).is_some_and(|attrs| has_flag(attrs, name)))
}

/// `CallBase::paramHasAttr(ArgNo, Dereferenceable)`.
fn param_has_dereferenceable(
    q: &Query,
    instruction: &Instruction,
    arg: usize,
) -> bool {
    let has = |attrs: &Vec<Attribute>| int_attr(attrs, "dereferenceable").is_some();
    call_info(instruction).is_some_and(|info| info.argument_attrs.get(arg).is_some_and(has))
        || called_function(q, instruction).is_some_and(|function| function.parameter_attrs.get(arg).is_some_and(has))
}

/// `CallBase::paramHasNonNullAttr(ArgNo, AllowUndefOrPoison)`.
pub(crate) fn param_has_non_null_attr(
    q: &Query,
    instruction: &Instruction,
    arg: usize,
    allow_undef_or_poison: bool,
) -> bool {
    if param_has_attr(q, instruction, arg, "nonnull")
        && (allow_undef_or_poison || param_has_attr(q, instruction, arg, "noundef"))
    {
        return true;
    }
    let space = instruction.operands.get(arg).and_then(|one| q.type_of(*one)).and_then(|ty| pointer_space(q, ty));
    param_has_dereferenceable(q, instruction, arg) && !null_pointer_is_defined(Some(q.function), space.unwrap_or(0))
}

/// `CallBase::getRetDereferenceableBytes()`.
fn ret_dereferenceable_bytes(
    q: &Query,
    instruction: &Instruction,
) -> u64 {
    let site = call_info(instruction).and_then(|info| int_attr(&info.return_attrs, "dereferenceable")).unwrap_or(0);
    let callee = called_function(q, instruction)
        .and_then(|function| int_attr(&function.return_attrs, "dereferenceable"))
        .unwrap_or(0);
    site.max(callee)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instructions.cpp#L396>
fn is_return_non_null(
    q: &Query,
    instruction: &Instruction,
) -> bool {
    if ret_has_attr(q, instruction, "nonnull") {
        return true;
    }
    ret_dereferenceable_bytes(q, instruction) > 0
        && !null_pointer_is_defined(Some(q.function), pointer_space(q, instruction.ty).unwrap_or(0))
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7219>
fn get_argument_aliasing_to_returned_pointer(
    q: &Query,
    instruction: &Instruction,
    inst: InstId,
    must_preserve_offset: bool,
) -> Option<Operand> {
    if let Some(returned) = returned_argument_operand(q, instruction) {
        return Some(returned);
    }
    let aliasing = match pattern::call_intrinsic(q, inst)? {
        IntrinsicId::LaunderInvariantGroup
        | IntrinsicId::Aarch64Irg
        | IntrinsicId::Aarch64Tagp
        | IntrinsicId::AmdgcnMakeBufferRsrc => true,
        IntrinsicId::Ptrmask => !must_preserve_offset,
        IntrinsicId::ThreadlocalAddress => !has_flag(&q.function.attrs, "presplitcoroutine"),
        _ => false,
    };
    aliasing.then(|| instruction.operands[0])
}

/// `getLoadStorePointerOperand(UI)`.
fn load_store_pointer_operand(instruction: &Instruction) -> Option<Operand> {
    match instruction.opcode {
        Opcode::Load { .. } => instruction.operands.first().copied(),
        Opcode::Store { .. } => instruction.operands.get(1).copied(),
        _ => None,
    }
}

/// `DominatorTree::dominates(Def, User)`.
fn instruction_dominates(
    q: &Query,
    dt: &DomTree,
    def: InstId,
    user: InstId,
) -> bool {
    let (Some(def_block), Some(use_block)) = (q.block_of(def), q.block_of(user)) else { return false };
    if !dt.is_reachable(use_block) {
        return true;
    }
    if !dt.is_reachable(def_block) {
        return false;
    }
    if def == user {
        return false;
    }
    if matches!(q.function.instruction(def).opcode, Opcode::Invoke(_))
        || q.function.instruction(user).opcode == Opcode::Phi
    {
        return block_dominates_instructions(q, dt, def, use_block);
    }
    if def_block != use_block {
        return dt.dominates(def_block, use_block);
    }
    q.comes_before(def, user)
}

/// `DominatorTree::dominates(Def, UseBB)`.
fn block_dominates_instructions(
    q: &Query,
    dt: &DomTree,
    def: InstId,
    use_block: BlockId,
) -> bool {
    let Some(def_block) = q.block_of(def) else { return false };
    if !dt.is_reachable(use_block) {
        return true;
    }
    if !dt.is_reachable(def_block) {
        return false;
    }
    if def_block == use_block {
        return false;
    }
    let instruction = q.function.instruction(def);
    if matches!(instruction.opcode, Opcode::Invoke(_)) {
        let Some(Operand::Block(normal)) = instruction.operands.get(instruction.operands.len().saturating_sub(3))
        else {
            return false;
        };
        return dominates_edge(q, dt, def_block, *normal, use_block);
    }
    dt.dominates(def_block, use_block)
}

/// `DominatorTree::dominates(BasicBlockEdge(Start, End), UseBB)`.
fn dominates_edge(
    q: &Query,
    dt: &DomTree,
    start: BlockId,
    end: BlockId,
    use_block: BlockId,
) -> bool {
    if !dt.dominates(end, use_block) {
        return false;
    }
    let predecessors: Vec<BlockId> = q
        .function
        .block_users(end)
        .iter()
        .filter(|one| q.function.instruction(one.user).opcode.is_terminator())
        .filter_map(|one| q.block_of(one.user))
        .collect();
    if predecessors.len() == 1 {
        return true;
    }
    let mut duplicate = false;
    for block in predecessors {
        if block == start {
            if duplicate {
                return false;
            }
            duplicate = true;
            continue;
        }
        if !dt.dominates(end, block) {
            return false;
        }
    }
    true
}

/// A `GEPOperator`: an instruction or a constant expression.
struct Gep {
    source: TypeId,
    inbounds: bool,
    nuw: bool,
    pointer: Operand,
    indices: Vec<Operand>,
}

fn gep_of(operator: &Operator) -> Option<Gep> {
    let Opcode::GetElementPtr { source } = operator.instruction.opcode else { return None };
    let operands = &operator.instruction.operands;
    Some(Gep {
        source,
        inbounds: operator.instruction.flags.contains(Flags::INBOUNDS),
        nuw: operator.instruction.flags.contains(Flags::NUW),
        pointer: *operands.first()?,
        indices: operands[1..].to_vec(),
    })
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L2984>
fn is_gep_known_non_null(
    q: &Query,
    gep: &Gep,
    in_function: bool,
    mut depth: u32,
) -> bool {
    let function = in_function.then_some(q.function);
    let space = q.type_of(gep.pointer).and_then(|ty| pointer_space(q, ty)).unwrap_or(0);
    if !gep.nuw && !(gep.inbounds && !null_pointer_is_defined(function, space)) {
        return false;
    }

    if is_known_non_zero(q, gep.pointer, depth) {
        return true;
    }

    let types = &q.context.types;
    let mut current = gep.source;
    for (at, index) in gep.indices.iter().enumerate() {
        let stride = if at == 0 {
            q.layout.alloc_size(types, current)
        } else {
            match types.get(current) {
                Type::Struct { .. } | Type::Named(_) => {
                    let (element, _) = pattern::apint(q, *index).unwrap_or((0, 0));
                    let (_, offsets) = q.layout.struct_layout(types, current);
                    if offsets.get(element as usize).copied().unwrap_or(0) > 0 {
                        return true;
                    }
                    current = types.member(current, element as u64).unwrap_or(current);
                    continue;
                }
                Type::Array { element, .. } | Type::Vector { element, .. } => {
                    current = *element;
                    q.layout.alloc_size(types, current)
                }
                _ => return false,
            }
        };

        if stride == 0 {
            continue;
        }

        if let Some((value, _)) = pattern::apint(q, *index) {
            if value != 0 {
                return true;
            }
            continue;
        }

        let before = depth;
        depth += 1;
        if before >= MAX_ANALYSIS_RECURSION_DEPTH {
            continue;
        }

        if is_known_non_zero(q, *index, depth) {
            return true;
        }
    }

    false
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L3048>
fn is_known_non_null_from_dominating_condition(
    q: &Query,
    v: Operand,
    cxt: Option<InstId>,
    dt: Option<&DomTree>,
) -> bool {
    let (Some(cxt), Some(dt)) = (cxt, dt) else { return false };
    let Operand::Value(value) = v else { return false };
    let Some(cxt_block) = q.block_of(cxt) else { return false };

    let mut explored = 0;
    // LLVM walks the use list newest first, as `users` does.
    for use_ in q.function.users(value) {
        if explored >= DOM_CONDITIONS_MAX_USES {
            break;
        }
        explored += 1;

        let user = use_.user;
        let instruction = q.function.instruction(user);
        if q.is_pointer(v)
            && call_info(instruction).is_some()
            && (use_.index as usize) < arg_size(instruction)
            && param_has_non_null_attr(q, instruction, use_.index as usize, false)
            && instruction_dominates(q, dt, user, cxt)
        {
            return true;
        }

        if load_store_pointer_operand(instruction) == Some(v) {
            let space = q.type_of(v).and_then(|ty| pointer_space(q, ty)).unwrap_or(0);
            if !null_pointer_is_defined(Some(q.function), space) && instruction_dominates(q, dt, user, cxt) {
                return true;
            }
        }

        if matches!(
            instruction.opcode,
            Opcode::Binary(BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem)
        ) && instruction.operands[1] == v
            && context::is_valid_assume_for_context(q, user, cxt, false)
        {
            return true;
        }

        let Some(result) = instruction.result else { continue };
        let Some((pred, rhs)) = pattern::c_icmp_specific(q, Operand::Value(result), v) else { continue };

        let non_null_if_true = if cmp_excludes_zero(q, pred, rhs) {
            true
        } else if cmp_excludes_zero(q, pred.inverse(), rhs) {
            false
        } else {
            continue;
        };

        let mut work: Vec<InstId> = Vec::new();
        let mut visited: Vec<InstId> = Vec::new();
        for cmp_use in q.function.users(result) {
            let cmp_user = cmp_use.user;
            if !visited.contains(&cmp_user) {
                visited.push(cmp_user);
                work.push(cmp_user);
            }

            while let Some(current) = work.pop() {
                let current_instruction = q.function.instruction(current);
                if non_null_if_true
                    && let Some(current_result) = current_instruction.result
                    && pattern::logical_and(q, Operand::Value(current_result)).is_some()
                {
                    for next in q.function.users(current_result) {
                        if !visited.contains(&next.user) {
                            visited.push(next.user);
                            work.push(next.user);
                        }
                    }
                    continue;
                }

                if current_instruction.opcode == Opcode::Br && current_instruction.operands.len() == 3 {
                    let Operand::Block(successor) = current_instruction.operands[if non_null_if_true { 1 } else { 2 }]
                    else {
                        continue;
                    };
                    let Some(branch_block) = q.block_of(current) else { continue };
                    if dominates_edge(q, dt, branch_block, successor, cxt_block) {
                        return true;
                    }
                } else if non_null_if_true
                    && pattern::call_intrinsic(q, current) == Some(IntrinsicId::ExperimentalGuard)
                    && instruction_dominates(q, dt, current, cxt)
                {
                    return true;
                }
            }
        }
    }

    false
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L3145>
fn range_metadata_excludes_value(
    q: &Query,
    ranges: &[MetadataOperand],
    value: u128,
) -> bool {
    let bound = |at: usize| match ranges.get(at) {
        Some(MetadataOperand::Constant(id)) => {
            let constant = q.context.get(*id);
            match constant.kind {
                ConstantKind::Int(bits) => Some((bits, q.context.types.int_bits(constant.ty)?)),
                _ => None,
            }
        }
        _ => None,
    };
    for i in 0..ranges.len() / 2 {
        let (Some((lower, width)), Some((upper, _))) = (bound(2 * i), bound(2 * i + 1)) else { continue };
        if ConstantRange::new(width, lower, upper).contains(value) {
            return false;
        }
    }
    true
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L3162>
fn is_non_zero_recurrence(
    q: &Query,
    phi: InstId,
) -> bool {
    let Some((bo, start, step)) = pattern::match_simple_recurrence(q, phi) else { return false };
    let bo = q.function.instruction(bo);
    let Some((start_c, width)) = pattern::apint(q, start) else { return false };
    if start_c == 0 {
        return false;
    }
    let Opcode::Binary(op) = bo.opcode else { return false };
    let nuw = bo.flags.contains(Flags::NUW);
    let nsw = bo.flags.contains(Flags::NSW);
    match op {
        BinaryOp::Add => {
            nuw || (nsw
                && pattern::apint(q, step).is_some_and(|(step_c, _)| bit(start_c, width - 1) == bit(step_c, width - 1)))
        }
        BinaryOp::Mul => (nuw || nsw) && pattern::apint(q, step).is_some_and(|(step_c, _)| step_c != 0),
        BinaryOp::Shl => nuw || nsw,
        BinaryOp::AShr | BinaryOp::LShr => bo.flags.contains(Flags::EXACT),
        BinaryOp::Or => true,
        _ => false,
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L3192>
fn match_op_with_op_eq_zero(
    q: &Query,
    op0: Operand,
    op1: Operand,
) -> bool {
    let of = |v: Operand, other: Operand| {
        let extended = pattern::cast(q, v, CastOp::ZExt).or_else(|| pattern::cast(q, v, CastOp::SExt));
        let Some((_, _, source)) = extended else { return false };
        matches!(
            pattern::icmp(q, source),
            Some((_, _, pattern::CmpPredicate { pred: IntPredicate::Eq, .. }, lhs, rhs)) if lhs == other && pattern::zero(q, rhs)
        )
    };
    of(op0, op1) || of(op1, op0)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L3199>
fn is_non_zero_add(
    q: &Query,
    bit_width: u32,
    x: Operand,
    y: Operand,
    nsw: bool,
    nuw: bool,
    depth: u32,
) -> bool {
    if match_op_with_op_eq_zero(q, x, y) {
        return true;
    }

    if nuw {
        return is_known_non_zero(q, y, depth) || is_known_non_zero(q, x, depth);
    }

    let x_known = known::compute_known_bits(q, x, depth);
    let y_known = known::compute_known_bits(q, y, depth);

    if x_known.is_non_negative()
        && y_known.is_non_negative()
        && (is_known_non_zero(q, y, depth) || is_known_non_zero(q, x, depth))
    {
        return true;
    }

    if x_known.is_negative() && y_known.is_negative() {
        let signed_max = mask(bit_width) >> 1;
        if x_known.one & signed_max != 0 {
            return true;
        }
        if y_known.one & signed_max != 0 {
            return true;
        }
    }

    if x_known.is_non_negative() && power2::is_known_to_be_a_power_of_two(q, y, false, depth) {
        return true;
    }
    if y_known.is_non_negative() && power2::is_known_to_be_a_power_of_two(q, x, false, depth) {
        return true;
    }

    KnownBits::add(&x_known, &y_known, nsw, nuw, false).is_non_zero()
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L3245>
fn is_non_zero_sub(
    q: &Query,
    x: Operand,
    y: Operand,
    depth: u32,
) -> bool {
    if match_op_with_op_eq_zero(q, x, y) {
        return true;
    }

    if pattern::zero(q, x) && is_known_non_zero(q, y, depth) {
        return true;
    }

    is_known_non_equal_demanded(q, x, y, depth)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L3261>
fn is_non_zero_mul(
    q: &Query,
    bit_width: u32,
    x: Operand,
    y: Operand,
    nsw: bool,
    nuw: bool,
    depth: u32,
) -> bool {
    if nsw || nuw {
        return is_known_non_zero(q, x, depth) && is_known_non_zero(q, y, depth);
    }

    let x_known = known::compute_known_bits(q, x, depth);
    if x_known.one & 1 != 0 {
        return is_known_non_zero(q, y, depth);
    }

    let y_known = known::compute_known_bits(q, y, depth);
    if y_known.one & 1 != 0 {
        return x_known.is_non_zero() || is_known_non_zero(q, x, depth);
    }

    x_known.count_max_trailing_zeros() + y_known.count_max_trailing_zeros() < bit_width
}

/// `APInt::shl`, `lshr` and `ashr` by an `APInt`: an amount of the width or
/// more shifts everything out.
fn shift(
    op: BinaryOp,
    lhs: u128,
    rhs: u128,
    width: u32,
) -> u128 {
    let amount = rhs.min(u128::from(width)) as u32;
    match op {
        BinaryOp::Shl if amount >= width => 0,
        BinaryOp::Shl => (lhs << amount) & mask(width),
        BinaryOp::LShr => lshr(lhs, amount),
        _ => ashr(lhs, width, amount),
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L3289>
fn is_non_zero_shift(
    q: &Query,
    i: &Instruction,
    known_val: &KnownBits,
    depth: u32,
) -> bool {
    let Opcode::Binary(op) = i.opcode else { return false };
    let inv_shift_op = |lhs: u128, rhs: u128, width: u32| match op {
        BinaryOp::Shl => shift(BinaryOp::LShr, lhs, rhs, width),
        _ => shift(BinaryOp::Shl, lhs, rhs, width),
    };

    if known_val.is_unknown() {
        return false;
    }

    let known_cnt = known::compute_known_bits(q, i.operands[1], depth);
    let max_shift = known_cnt.max_value();
    let num_bits = known_val.width;
    if max_shift >= u128::from(num_bits) {
        return false;
    }

    if shift(op, known_val.one, max_shift, num_bits) != 0 {
        return true;
    }

    let inverse = u128::from(num_bits) - max_shift;
    inv_shift_op(known_val.zero, inverse, num_bits) == inv_shift_op(mask(num_bits), inverse, num_bits)
        && is_known_non_zero(q, i.operands[0], depth)
}

/// `m_NUWAdd(m_Value(A), m_Value(B))`.
fn nuw_add(
    q: &Query,
    v: Operand,
) -> Option<(Operand, Operand)> {
    let (_, instruction, a, b) = pattern::binary(q, v, BinaryOp::Add)?;
    instruction.flags.contains(Flags::NUW).then_some((a, b))
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L3340>
fn is_known_non_zero_from_operator(
    q: &Query,
    v: Operand,
    operator: &Operator,
    depth: u32,
) -> bool {
    let i = &*operator.instruction;
    let bit_width = type_bits(q, i.ty);
    match &i.opcode {
        Opcode::Alloca { .. } => return pointer_space(q, i.ty) == Some(0),
        Opcode::GetElementPtr { .. } => {
            if pointer_space(q, i.ty).is_some()
                && let Some(gep) = gep_of(operator)
            {
                return is_gep_known_non_null(q, &gep, operator.inst.is_some(), depth);
            }
        }
        Opcode::Cast(CastOp::BitCast) => {
            if let Some(from) = q.type_of(i.operands[0])
                && matches!(q.context.types.get(from), Type::Int(_) | Type::Pointer(_))
                && bit_width % type_bits(q, from) == 0
            {
                return is_known_non_zero(q, i.operands[0], depth);
            }
        }
        Opcode::Cast(CastOp::IntToPtr) => {
            if let Some(from) = q.type_of(i.operands[0])
                && q.layout.size_bits(&q.context.types, from) <= q.layout.size_bits(&q.context.types, i.ty)
            {
                return is_known_non_zero(q, i.operands[0], depth);
            }
        }
        Opcode::Cast(CastOp::PtrToInt) => {
            if let Some(space) = q.type_of(i.operands[0]).and_then(|ty| pointer_space(q, ty))
                && q.layout.pointer(space).index_bits <= bit_width
            {
                return is_known_non_zero(q, i.operands[0], depth);
            }
        }
        Opcode::Cast(CastOp::Trunc) => {
            if i.flags.contains(Flags::NSW) || i.flags.contains(Flags::NUW) {
                return is_known_non_zero(q, i.operands[0], depth);
            }
        }
        Opcode::Binary(BinaryOp::Xor | BinaryOp::Sub) => {
            return is_non_zero_sub(q, i.operands[0], i.operands[1], depth);
        }
        Opcode::Binary(BinaryOp::Or) => {
            if match_op_with_op_eq_zero(q, i.operands[0], i.operands[1]) {
                return true;
            }
            if is_known_non_equal_demanded(q, i.operands[0], i.operands[1], depth) {
                return true;
            }
            return is_known_non_zero(q, i.operands[1], depth) || is_known_non_zero(q, i.operands[0], depth);
        }
        Opcode::Cast(CastOp::SExt | CastOp::ZExt) => return is_known_non_zero(q, i.operands[0], depth),
        Opcode::Binary(BinaryOp::Shl) => {
            if i.flags.contains(Flags::NUW) || i.flags.contains(Flags::NSW) {
                return is_known_non_zero(q, i.operands[0], depth);
            }

            let known = known::compute_known_bits(q, i.operands[0], depth);
            if known.one & 1 != 0 {
                return true;
            }

            return is_non_zero_shift(q, i, &known, depth);
        }
        Opcode::Binary(BinaryOp::LShr | BinaryOp::AShr) => {
            if i.flags.contains(Flags::EXACT) {
                return is_known_non_zero(q, i.operands[0], depth);
            }

            let known = known::compute_known_bits(q, i.operands[0], depth);
            if known.is_negative() {
                return true;
            }

            if depth + 1 < MAX_ANALYSIS_RECURSION_DEPTH
                && let Some((a, b)) = nuw_add(q, i.operands[0])
                && let Some((c, _)) = pattern::apint(q, i.operands[1])
                && c < u128::from(bit_width)
            {
                let known_a = known::compute_known_bits(q, a, depth + 1);
                if lshr(known_a.one, c as u32) != 0 {
                    return true;
                }
                let known_b = known::compute_known_bits(q, b, depth + 1);
                if lshr(known_b.one, c as u32) != 0 {
                    return true;
                }
            }

            return is_non_zero_shift(q, i, &known, depth);
        }
        Opcode::Binary(op @ (BinaryOp::UDiv | BinaryOp::SDiv)) => {
            if i.flags.contains(Flags::EXACT) {
                return is_known_non_zero(q, i.operands[0], depth);
            }

            let mut x_known = known::compute_known_bits(q, i.operands[0], depth);
            if x_known.is_unknown() {
                return false;
            }

            let mut y_known = known::compute_known_bits(q, i.operands[1], depth);
            if *op == BinaryOp::SDiv {
                x_known = x_known.abs(false);
                y_known = y_known.abs(false);
            }
            return KnownBits::uge(&x_known, &y_known) == Some(true);
        }
        Opcode::Binary(BinaryOp::Add) => {
            return is_non_zero_add(
                q,
                bit_width,
                i.operands[0],
                i.operands[1],
                i.flags.contains(Flags::NSW),
                i.flags.contains(Flags::NUW),
                depth,
            );
        }
        Opcode::Binary(BinaryOp::Mul) => {
            return is_non_zero_mul(
                q,
                bit_width,
                i.operands[0],
                i.operands[1],
                i.flags.contains(Flags::NSW),
                i.flags.contains(Flags::NUW),
                depth,
            );
        }
        Opcode::Select => {
            let select_arm_is_non_zero = |is_true_arm: bool| {
                let op = if is_true_arm { i.operands[1] } else { i.operands[2] };
                if is_known_non_zero(q, op, depth) {
                    return true;
                }

                let Some((mut pred, x)) = pattern::c_icmp_specific(q, i.operands[0], op) else { return false };

                if !is_true_arm {
                    pred = pred.inverse();
                }

                cmp_excludes_zero(q, pred, x)
            };

            if select_arm_is_non_zero(true) && select_arm_is_non_zero(false) {
                return true;
            }
        }
        Opcode::Phi => {
            let Some(phi) = operator.inst else { return false };
            if is_non_zero_recurrence(q, phi) {
                return true;
            }

            let result = Operand::Value(i.result.expect("a phi has a result"));
            let new_depth = depth.max(MAX_ANALYSIS_RECURSION_DEPTH - 1);
            let parent = q.block_of(phi);
            return i
                .operands
                .chunks(2)
                .all(
                    |incoming| {
                        let value = incoming[0];
                        if value == result {
                            return true;
                        }
                        let Operand::Block(block) = incoming[1] else { return false };
                        let Some(terminator) = q.function.terminator(block) else { return false };
                        let rec_q = q.without_cond_context().with_cxt(Some(terminator));
                        let branch = q.function.instruction(terminator);
                        if branch.opcode == Opcode::Br
                            && branch.operands.len() == 3
                            && let Some((mut pred, x)) = pattern::c_icmp_specific(q, branch.operands[0], value)
                            && let (Operand::Block(true_succ), Operand::Block(false_succ)) =
                                (branch.operands[1], branch.operands[2])
                            && (Some(true_succ) == parent) != (Some(false_succ) == parent)
                        {
                            if Some(false_succ) == parent {
                                pred = pred.inverse();
                            }
                            if cmp_excludes_zero(q, pred, x) {
                                return true;
                            }
                        }
                        is_known_non_zero(&rec_q, value, new_depth)
                    },
                );
        }
        Opcode::Freeze => {
            return is_known_non_zero(q, i.operands[0], depth)
                && guarantee::is_guaranteed_not_to_be_poison(q, i.operands[0], depth);
        }
        Opcode::Load { .. } => {
            let Some(inst) = operator.inst else { return false };
            if let Some(space) = pointer_space(q, i.ty) {
                if q.metadata_node(inst, "nonnull").is_some()
                    || (q.metadata_node(inst, "dereferenceable").is_some()
                        && !null_pointer_is_defined(Some(q.function), space))
                {
                    return true;
                }
            } else if let Some(ranges) = q.metadata_node(inst, "range") {
                return range_metadata_excludes_value(q, &ranges.operands, 0);
            }
            return false;
        }
        Opcode::ExtractValue(indices) => {
            if indices.as_slice() == [0]
                && let Some((call, instruction)) = q.instruction_of(i.operands[0])
                && let Some(id) = pattern::call_intrinsic(q, call)
            {
                let (x, y) = (instruction.operands[0], instruction.operands[1]);
                match id {
                    IntrinsicId::UaddWithOverflow | IntrinsicId::SaddWithOverflow => {
                        return is_non_zero_add(q, bit_width, x, y, false, false, depth);
                    }
                    IntrinsicId::UsubWithOverflow | IntrinsicId::SsubWithOverflow => {
                        return is_non_zero_sub(q, x, y, depth);
                    }
                    IntrinsicId::UmulWithOverflow | IntrinsicId::SmulWithOverflow => {
                        return is_non_zero_mul(q, bit_width, x, y, false, false, depth);
                    }
                    _ => {}
                }
            }
        }
        Opcode::Call(_) | Opcode::Invoke(_) => {
            let Some(inst) = operator.inst else { return false };
            if pointer_space(q, i.ty).is_some() {
                if is_return_non_null(q, i) {
                    return true;
                }
                if let Some(rp) = get_argument_aliasing_to_returned_pointer(q, i, inst, true) {
                    return is_known_non_zero(q, rp, depth);
                }
            } else {
                if let Some(ranges) = q.metadata_node(inst, "range") {
                    return range_metadata_excludes_value(q, &ranges.operands, 0);
                }
                if let Some(range) = call_range(q, i)
                    && !range.contains(0)
                {
                    return true;
                }
                if let Some(rv) = returned_argument_operand(q, i)
                    && q.type_of(rv) == Some(i.ty)
                    && is_known_non_zero(q, rv, depth)
                {
                    return true;
                }
            }

            let is_intrinsic = matches!(i.opcode, Opcode::Call(_))
                && q.callee_name(inst).is_some_and(|name| name.starts_with("llvm."));
            if !is_intrinsic {
                return false;
            }
            let arg = |at: usize| i.operands[at];
            match pattern::call_intrinsic(q, inst) {
                Some(
                    IntrinsicId::SshlSat
                    | IntrinsicId::UshlSat
                    | IntrinsicId::Abs
                    | IntrinsicId::Bitreverse
                    | IntrinsicId::Bswap
                    | IntrinsicId::Ctpop,
                ) => return is_known_non_zero(q, arg(0), depth),
                Some(IntrinsicId::SsubSat) => {
                    if bit_width == 1 {
                        return false;
                    }
                    return is_non_zero_sub(q, arg(0), arg(1), depth);
                }
                Some(IntrinsicId::SaddSat) => return is_non_zero_add(q, bit_width, arg(0), arg(1), true, false, depth),
                Some(IntrinsicId::Umax | IntrinsicId::UaddSat) => {
                    if match_op_with_op_eq_zero(q, arg(0), arg(1)) {
                        return true;
                    }

                    return is_known_non_zero(q, arg(1), depth) || is_known_non_zero(q, arg(0), depth);
                }
                Some(IntrinsicId::Smax) => {
                    let is_non_zero = |op: Operand, cache: &mut Option<bool>, op_known: &KnownBits| {
                        *cache.get_or_insert_with(|| op_known.is_non_zero() || is_known_non_zero(q, op, depth))
                    };
                    let mut op0_non_zero: Option<bool> = None;
                    let mut op1_non_zero: Option<bool> = None;
                    let op1_known = known::compute_known_bits(q, arg(1), depth);
                    if op1_known.is_non_negative() && is_non_zero(arg(1), &mut op1_non_zero, &op1_known) {
                        return true;
                    }
                    let op0_known = known::compute_known_bits(q, arg(0), depth);
                    if op0_known.is_non_negative() && is_non_zero(arg(0), &mut op0_non_zero, &op0_known) {
                        return true;
                    }
                    return is_non_zero(arg(1), &mut op1_non_zero, &op1_known)
                        && is_non_zero(arg(0), &mut op0_non_zero, &op0_known);
                }
                Some(id @ (IntrinsicId::Smin | IntrinsicId::Umin)) => {
                    if id == IntrinsicId::Smin {
                        let op1_known = known::compute_known_bits(q, arg(1), depth);
                        if op1_known.is_negative() {
                            return true;
                        }
                        let op0_known = known::compute_known_bits(q, arg(0), depth);
                        if op0_known.is_negative() {
                            return true;
                        }

                        if op1_known.is_non_zero() && op0_known.is_non_zero() {
                            return true;
                        }
                    }
                    return is_known_non_zero(q, arg(0), depth) && is_known_non_zero(q, arg(1), depth);
                }
                Some(IntrinsicId::Cttz) => return known::compute_known_bits(q, arg(0), depth).zero & 1 != 0,
                Some(IntrinsicId::Ctlz) => return known::compute_known_bits(q, arg(0), depth).is_non_negative(),
                Some(IntrinsicId::Fshr | IntrinsicId::Fshl) => {
                    if arg(0) == arg(1) {
                        return is_known_non_zero(q, arg(0), depth);
                    }
                }
                Some(IntrinsicId::Vscale) => return true,
                Some(IntrinsicId::ExperimentalGetVectorLength) => return is_known_non_zero(q, arg(0), depth),
                _ => {}
            }
        }
        _ => {}
    }

    known::compute_known_bits(q, v, depth).one != 0
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L11029>
fn strip_null_test(
    q: &Query,
    v: Operand,
) -> Option<Operand> {
    let (_, _, a, b) = pattern::binary(q, v, BinaryOp::Add).or_else(|| pattern::binary(q, v, BinaryOp::Or))?;
    let attempt = |shifted: Operand, extended: Operand| {
        let (_, _, x, c1) = pattern::binary(q, shifted, BinaryOp::LShr)?;
        let (c1, _) = pattern::apint(q, c1)?;
        let (_, _, compared) = pattern::cast(q, extended, CastOp::ZExt)?;
        let (_, _, pred, and, zero) = pattern::icmp(q, compared)?;
        let pred = pred.pred;
        if pred != IntPredicate::Ne || !pattern::zero(q, zero) {
            return None;
        }
        let (_, _, and_x, mask_c) = pattern::binary(q, and, BinaryOp::And)?;
        let (c2, _) = pattern::low_bit_mask(q, mask_c)?;
        (and_x == x && u128::from(popcount(c2)) == c1).then_some(x)
    };
    attempt(a, b).or_else(|| attempt(b, a))
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L3829>
fn is_known_non_zero_demanded(
    q: &Query,
    v: Operand,
    mut depth: u32,
) -> bool {
    let is_constant = matches!(v, Operand::Constant(_));
    if let Operand::Constant(id) = v {
        let constant = q.context.get(id);
        match &constant.kind {
            ConstantKind::Zero | ConstantKind::Null | ConstantKind::Int(0) => return false,
            ConstantKind::Int(_) => return true,
            ConstantKind::Global(global) => {
                let not_weak =
                    q.globals.get(global.0 as usize).is_some_and(|global| global.linkage != Linkage::ExternWeak);
                return not_weak && pointer_space(q, constant.ty) == Some(0);
            }
            ConstantKind::Expr(_) => {}
            _ => return false,
        }
    }

    let argument = match v {
        Operand::Value(value) => match q.function.value(value).def {
            ValueDef::Argument(at) => q.function.parameter_attrs.get(at as usize),
            ValueDef::Instruction(_) => None,
        },
        _ => None,
    };
    if let Some(attrs) = argument
        && let Some(range) = range_attribute(q, attrs)
        && !range.contains(0)
    {
        return true;
    }

    if !is_constant && context::is_known_non_zero_from_assume(q, v) {
        return true;
    }

    let before = depth;
    depth += 1;
    if before >= MAX_ANALYSIS_RECURSION_DEPTH {
        return false;
    }

    if let Some(space) = q.type_of(v).and_then(|ty| pointer_space(q, ty))
        && let Some(attrs) = argument
    {
        let by_value_copy = has_type_attr(attrs, "byval") || has_type_attr(attrs, "inalloca");
        let non_null = has_flag(attrs, "nonnull")
            || (int_attr(attrs, "dereferenceable").unwrap_or(0) > 0
                && !null_pointer_is_defined(Some(q.function), space));
        if (by_value_copy && !null_pointer_is_defined(Some(q.function), space)) || non_null {
            return true;
        }
    }

    if let Some(operator) = operator(q, v)
        && is_known_non_zero_from_operator(q, v, &operator, depth)
    {
        return true;
    }

    if !is_constant && is_known_non_null_from_dominating_condition(q, v, q.cxt, q.dt) {
        return true;
    }

    if let Some(stripped) = strip_null_test(q, v) {
        return is_known_non_zero_demanded(q, stripped, depth);
    }

    false
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L3927>
pub fn is_known_non_zero(
    q: &Query,
    v: Operand,
    depth: u32,
) -> bool {
    is_known_non_zero_demanded(q, v, depth)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L3941>
fn get_invertible_operands(
    q: &Query,
    op1: &Operator,
    op2: &Operator,
) -> Option<(Operand, Operand)> {
    let (i1, i2) = (&*op1.instruction, &*op2.instruction);
    if i1.opcode != i2.opcode {
        return None;
    }

    let get_operands = |at: usize| Some((i1.operands[at], i2.operands[at]));
    let wraps = |flag: Flags| i1.flags.contains(flag) && i2.flags.contains(flag);
    match i1.opcode {
        Opcode::Binary(op @ (BinaryOp::Or | BinaryOp::Xor | BinaryOp::Add)) => {
            if op == BinaryOp::Or && !wraps(Flags::DISJOINT) {
                return None;
            }
            let other_of = |x: Operand| {
                if i2.operands[0] == x {
                    Some(i2.operands[1])
                } else if i2.operands[1] == x {
                    Some(i2.operands[0])
                } else {
                    None
                }
            };
            if let Some(other) = other_of(i1.operands[0]) {
                return Some((i1.operands[1], other));
            }
            if let Some(other) = other_of(i1.operands[1]) {
                return Some((i1.operands[0], other));
            }
            None
        }
        Opcode::Binary(BinaryOp::Sub) => {
            if i1.operands[0] == i2.operands[0] {
                return get_operands(1);
            }
            if i1.operands[1] == i2.operands[1] {
                return get_operands(0);
            }
            None
        }
        Opcode::Binary(BinaryOp::Mul) => {
            if !wraps(Flags::NUW) && !wraps(Flags::NSW) {
                return None;
            }

            if i1.operands[1] == i2.operands[1] && matches!(pattern::apint(q, i1.operands[1]), Some((c, _)) if c != 0) {
                return get_operands(0);
            }
            None
        }
        Opcode::Binary(BinaryOp::Shl) => {
            if !wraps(Flags::NUW) && !wraps(Flags::NSW) {
                return None;
            }

            if i1.operands[1] == i2.operands[1] {
                return get_operands(0);
            }
            None
        }
        Opcode::Binary(BinaryOp::AShr | BinaryOp::LShr) => {
            if !wraps(Flags::EXACT) {
                return None;
            }

            if i1.operands[1] == i2.operands[1] {
                return get_operands(0);
            }
            None
        }
        Opcode::Cast(CastOp::SExt | CastOp::ZExt) => {
            if q.type_of(i1.operands[0]) == q.type_of(i2.operands[0]) {
                return get_operands(0);
            }
            None
        }
        Opcode::Phi => {
            let (pn1, pn2) = (op1.inst?, op2.inst?);
            if q.block_of(pn1) != q.block_of(pn2) {
                return None;
            }
            let (bo1, start1, _) = pattern::match_simple_recurrence(q, pn1)?;
            let bo1_instruction = q.function.instruction(bo1);
            let (bo2, start2, _) = pattern::match_simple_recurrence(q, pn2)?;
            let bo2_instruction = q.function.instruction(bo2);

            let values = get_invertible_operands(
                q,
                &Operator { instruction: Cow::Borrowed(bo1_instruction), inst: Some(bo1) },
                &Operator { instruction: Cow::Borrowed(bo2_instruction), inst: Some(bo2) },
            )?;

            if Some(values.0) != i1.result.map(Operand::Value) || Some(values.1) != i2.result.map(Operand::Value) {
                return None;
            }

            Some((start1, start2))
        }
        _ => None,
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L4058>
fn is_modifying_binop_of_non_zero(
    q: &Query,
    v1: Operand,
    v2: Operand,
    depth: u32,
) -> bool {
    let Some((_, bo)) = q.instruction_of(v1) else { return false };
    let Opcode::Binary(op) = bo.opcode else { return false };
    match op {
        BinaryOp::Or if !bo.flags.contains(Flags::DISJOINT) => false,
        BinaryOp::Or | BinaryOp::Xor | BinaryOp::Add => {
            let op = if v2 == bo.operands[0] {
                bo.operands[1]
            } else if v2 == bo.operands[1] {
                bo.operands[0]
            } else {
                return false;
            };
            is_known_non_zero(q, op, depth + 1)
        }
        _ => false,
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L4087>
fn is_non_equal_mul(
    q: &Query,
    v1: Operand,
    v2: Operand,
    depth: u32,
) -> bool {
    let Some((_, obo, lhs, rhs)) = pattern::binary(q, v2, BinaryOp::Mul) else { return false };
    matches!(
        pattern::apint(q, rhs),
        Some((c, _)) if lhs == v1 && c != 0 && c != 1
    ) && (obo.flags.contains(Flags::NUW) || obo.flags.contains(Flags::NSW))
        && is_known_non_zero(q, v1, depth + 1)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L4102>
fn is_non_equal_shl(
    q: &Query,
    v1: Operand,
    v2: Operand,
    depth: u32,
) -> bool {
    let Some((_, obo, lhs, rhs)) = pattern::binary(q, v2, BinaryOp::Shl) else { return false };
    matches!(pattern::apint(q, rhs), Some((c, _)) if lhs == v1 && c != 0)
        && (obo.flags.contains(Flags::NUW) || obo.flags.contains(Flags::NSW))
        && is_known_non_zero(q, v1, depth + 1)
}

/// `PHINode::getIncomingValueForBlock(BB)`.
fn incoming_value_for_block(
    instruction: &Instruction,
    block: BlockId,
) -> Option<Operand> {
    instruction.operands.chunks(2).find(|incoming| incoming[1] == Operand::Block(block)).map(|incoming| incoming[0])
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L4114>
fn is_non_equal_phis(
    q: &Query,
    pn1: InstId,
    pn2: InstId,
    depth: u32,
) -> bool {
    if q.block_of(pn1) != q.block_of(pn2) {
        return false;
    }

    let (phi1, phi2) = (q.function.instruction(pn1), q.function.instruction(pn2));
    let mut visited: Vec<BlockId> = Vec::new();
    let mut used_full_recursion = false;
    for incoming in phi1.operands.chunks(2) {
        let Operand::Block(incom_bb) = incoming[1] else { return false };
        if visited.contains(&incom_bb) {
            continue;
        }
        visited.push(incom_bb);
        let iv1 = incoming[0];
        let Some(iv2) = incoming_value_for_block(phi2, incom_bb) else { return false };
        if let (Some((c1, _)), Some((c2, _))) = (pattern::apint(q, iv1), pattern::apint(q, iv2))
            && c1 != c2
        {
            continue;
        }

        if used_full_recursion {
            return false;
        }

        let rec_q = q.without_cond_context().with_cxt(q.function.terminator(incom_bb));
        if !is_known_non_equal_demanded(&rec_q, iv1, iv2, depth + 1) {
            return false;
        }
        used_full_recursion = true;
    }
    true
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L4145>
fn is_non_equal_select(
    q: &Query,
    v1: Operand,
    v2: Operand,
    depth: u32,
) -> bool {
    let Some((_, si1)) = q.instruction_of(v1) else { return false };
    if si1.opcode != Opcode::Select {
        return false;
    }

    if let Some((_, si2)) = q.instruction_of(v2)
        && si2.opcode == Opcode::Select
    {
        let (cond1, cond2) = (si1.operands[0], si2.operands[0]);
        if cond1 == cond2 {
            return is_known_non_equal_demanded(q, si1.operands[1], si2.operands[1], depth + 1)
                && is_known_non_equal_demanded(q, si1.operands[2], si2.operands[2], depth + 1);
        }
    }
    is_known_non_equal_demanded(q, si1.operands[1], v2, depth + 1)
        && is_known_non_equal_demanded(q, si1.operands[2], v2, depth + 1)
}

/// `sextOrTrunc` of `value` from `from` bits to `to`.
fn sext_or_trunc(
    value: u128,
    from: u32,
    to: u32,
) -> u128 {
    if to > from { sext(value, from, to) } else { value & mask(to) }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Operator.cpp#L141>
fn accumulate_constant_offset(
    q: &Query,
    gep: &Gep,
    width: u32,
) -> Option<u128> {
    let types = &q.context.types;
    if types.int_bits(gep.source) == Some(8) && !gep.indices.is_empty() {
        let (value, from) = pattern::apint(q, gep.indices[0])?;
        return Some(sext_or_trunc(value, from, width));
    }

    let mut offset = 0_u128;
    let mut current = gep.source;
    for (at, index) in gep.indices.iter().enumerate() {
        let (stride, field) = if at == 0 {
            (q.layout.alloc_size(types, current), None)
        } else {
            match types.get(current) {
                Type::Struct { .. } | Type::Named(_) => (1, Some(current)),
                Type::Array { element, .. } | Type::Vector { element, .. } => {
                    current = *element;
                    (q.layout.alloc_size(types, current), None)
                }
                _ => return None,
            }
        };
        let (value, from) = pattern::apint(q, *index)?;
        if value == 0 {
            if let Some(structure) = field {
                current = types.member(structure, 0).unwrap_or(structure);
            }
            continue;
        }
        let amount = if let Some(structure) = field {
            let (_, offsets) = q.layout.struct_layout(types, structure);
            let element = value as usize;
            current = types.member(structure, element as u64)?;
            u128::from(*offsets.get(element)?) & mask(width)
        } else {
            sext_or_trunc(value, from, width)
        };
        offset = offset.wrapping_add(amount.wrapping_mul(u128::from(stride) & mask(width))) & mask(width);
    }
    Some(offset)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Value.cpp#L732>
///
/// `Value::stripAndAccumulateConstantOffsets` with the default arguments
/// but `AllowNonInbounds`: no invariant group, no external analysis, no look
/// through `inttoptr`.
pub fn strip_and_accumulate_constant_offsets(
    q: &Query,
    v: Operand,
    width: u32,
    allow_non_inbounds: bool,
) -> (Operand, u128) {
    if !q.is_pointer(v) {
        return (v, 0);
    }

    let mut offset = 0_u128;
    let mut visited = vec![v];
    let mut current = v;
    loop {
        let Some(operator) = operator(q, current) else { return (current, offset) };
        if let Some(gep) = gep_of(&operator) {
            if !allow_non_inbounds && !gep.inbounds {
                return (current, offset);
            }
            let gep_width = q
                .type_of(current)
                .and_then(|ty| pointer_space(q, ty))
                .map_or(width, |space| q.layout.pointer(space).index_bits);
            let Some(gep_offset) = accumulate_constant_offset(q, &gep, gep_width) else { return (current, offset) };
            if crate::apint::significant_bits(gep_offset, gep_width) > width {
                return (current, offset);
            }
            offset = offset.wrapping_add(sext_or_trunc(gep_offset, gep_width, width)) & mask(width);
            current = gep.pointer;
        } else if matches!(operator.instruction.opcode, Opcode::Cast(CastOp::BitCast)) {
            let source = operator.instruction.operands[0];
            if !q.is_pointer(source) {
                return (current, offset);
            }
            current = source;
        } else if matches!(
            operator.instruction.opcode,
            Opcode::Cast(CastOp::AddrSpaceCast)
        ) {
            current = operator.instruction.operands[0];
        } else if call_info(&operator.instruction).is_some() {
            if let Some(returned) = returned_argument_operand(q, &operator.instruction) {
                current = returned;
            }
        } else {
            return (current, offset);
        }
        if visited.contains(&current) {
            return (current, offset);
        }
        visited.push(current);
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L4170>
fn is_non_equal_pointers_with_recursive_gep(
    q: &Query,
    a: Operand,
    b: Operand,
) -> bool {
    if !q.is_pointer(a) || !q.is_pointer(b) {
        return false;
    }

    let Some(gep_a) = operator(q, a).and_then(|operator| gep_of(&operator)) else { return false };
    if gep_a.indices.len() != 1 || !matches!(gep_a.indices[0], Operand::Constant(_)) {
        return false;
    }

    let Some((_, pn)) = q.instruction_of(gep_a.pointer) else { return false };
    if pn.opcode != Opcode::Phi || pn.operands.len() != 4 {
        return false;
    }

    let step = a;
    let start = if pn.operands[0] == step {
        pn.operands[2]
    } else if pn.operands[2] == step {
        pn.operands[0]
    } else {
        return false;
    };

    let Some(space) = q.type_of(start).and_then(|ty| pointer_space(q, ty)) else { return false };
    let index_width = q.layout.pointer(space).index_bits;
    let (start, start_offset) = strip_and_accumulate_constant_offsets(q, start, index_width, false);
    let (step, step_offset) = strip_and_accumulate_constant_offsets(q, step, index_width, false);

    if step != gep_a.pointer {
        return false;
    }
    let (b, offset_b) = strip_and_accumulate_constant_offsets(q, b, index_width, false);
    let (start_signed, step_signed, b_signed) =
        (signed(start_offset, index_width), signed(step_offset, index_width), signed(offset_b, index_width));
    start == b && ((start_signed >= b_signed && step_signed > 0) || (start_signed <= b_signed && step_signed < 0))
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L4217>
fn is_known_non_equal_from_context(
    q: &Query,
    v1: Operand,
    v2: Operand,
    depth: u32,
) -> bool {
    if q.cxt.is_none() {
        return false;
    }

    if q.assumptions.is_none() {
        return false;
    }

    for &assume in q.assumptions.map_or(&[][..], |cache| cache.assumptions_for(v1)) {
        let condition = q.function.instruction(assume).operands[0];
        if context::is_implied_condition_cmp(
            q,
            condition,
            CmpPredicate::new(IntPredicate::Ne),
            Side::of(q, v1),
            Side::of(q, v2),
            true,
            depth,
        )
        .unwrap_or(false)
            && q.cxt.is_some_and(|cxt| context::is_valid_assume_for_context(q, assume, cxt, false))
        {
            return true;
        }
    }

    false
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L4274>
fn is_non_equal_urem(
    q: &Query,
    x: Operand,
    rem: Operand,
) -> bool {
    let Some((_, _, lhs, y)) = pattern::binary(q, rem, BinaryOp::URem) else { return false };
    if lhs != x {
        return false;
    }

    if context::is_true_predicate(q, IntPredicate::Ule, Side::Value(y), Side::Value(x)) {
        return true;
    }

    q.cxt.and_then(|cxt| context::is_implied_by_dom_condition(q, IntPredicate::Uge, x, y, cxt)) == Some(true)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L4291>
fn is_known_non_equal_demanded(
    q: &Query,
    v1: Operand,
    v2: Operand,
    depth: u32,
) -> bool {
    if v1 == v2 {
        return false;
    }
    if q.type_of(v1) != q.type_of(v2) {
        return false;
    }

    if depth >= MAX_ANALYSIS_RECURSION_DEPTH {
        return false;
    }

    let (o1, o2) = (operator(q, v1), operator(q, v2));
    if let (Some(o1), Some(o2)) = (&o1, &o2)
        && o1.instruction.opcode == o2.instruction.opcode
    {
        if let Some((first, second)) = get_invertible_operands(q, o1, o2) {
            return is_known_non_equal_demanded(q, first, second, depth + 1);
        }

        if o1.instruction.opcode == Opcode::Phi
            && let (Some(pn1), Some(pn2)) = (o1.inst, o2.inst)
            && is_non_equal_phis(q, pn1, pn2, depth)
        {
            return true;
        }
    }

    if is_modifying_binop_of_non_zero(q, v1, v2, depth) || is_modifying_binop_of_non_zero(q, v2, v1, depth) {
        return true;
    }

    if is_non_equal_mul(q, v1, v2, depth) || is_non_equal_mul(q, v2, v1, depth) {
        return true;
    }

    if is_non_equal_shl(q, v1, v2, depth) || is_non_equal_shl(q, v2, v1, depth) {
        return true;
    }

    if q.type_of(v1).is_some_and(|ty| matches!(q.context.types.get(ty), Type::Int(_))) {
        let known1 = known::compute_known_bits(q, v1, depth);
        if !known1.is_unknown() {
            let known2 = known::compute_known_bits(q, v2, depth);
            if known1.zero & known2.one != 0 || known2.zero & known1.one != 0 {
                return true;
            }
        }
    }

    if is_non_equal_select(q, v1, v2, depth) || is_non_equal_select(q, v2, v1, depth) {
        return true;
    }

    if is_non_equal_pointers_with_recursive_gep(q, v1, v2) || is_non_equal_pointers_with_recursive_gep(q, v2, v1) {
        return true;
    }

    let ptr_to_int_same_size = |v: Operand| {
        let operator = operator(q, v)?;
        let source = operator.instruction.operands.first().copied()?;
        let same = matches!(operator.instruction.opcode, Opcode::Cast(CastOp::PtrToInt))
            && q.layout.size_bits(&q.context.types, operator.instruction.ty)
                == q.layout.size_bits(&q.context.types, q.type_of(source)?);
        same.then_some(source)
    };
    if let (Some(a), Some(b)) = (ptr_to_int_same_size(v1), ptr_to_int_same_size(v2)) {
        return is_known_non_equal_demanded(q, a, b, depth + 1);
    }

    if is_non_equal_urem(q, v1, v2) || is_non_equal_urem(q, v2, v1) {
        return true;
    }

    if is_known_non_equal_from_context(q, v1, v2, depth) {
        return true;
    }

    false
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L334>
pub fn is_known_non_equal(
    q: &Query,
    v1: Operand,
    v2: Operand,
    depth: u32,
) -> bool {
    if v1 == v2 || q.type_of(v1) != q.type_of(v2) {
        return false;
    }
    is_known_non_equal_demanded(q, v1, v2, depth)
}

/// The `nz` column of `examples/vtdump.rs`.
pub fn column(
    q: &Query,
    v: Operand,
) -> String {
    format!(" nz={}", u8::from(is_known_non_zero(q, v, 0)))
}

/// `getOffsetFromIndex`
/// ([Value.cpp:1065](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Value.cpp#L1065)):
/// the offset the indices of `gep` from operand `idx` on add, all constant.
fn get_offset_from_index(
    q: &Query,
    gep: &Gep,
    idx: usize,
) -> Option<i64> {
    let types = &q.context.types;
    // The type each index steps over: the source type's elements for the
    // first, then the indexed type of the one before.
    let mut current = gep.source;
    let mut offset = 0_i64;
    for (at, index) in gep.indices.iter().enumerate() {
        // `at + 1` is the operand number.
        let (stride, field) = if at == 0 {
            (q.layout.alloc_size(types, current), None)
        } else {
            match types.get(current) {
                Type::Struct { .. } | Type::Named(_) => (1, Some(current)),
                Type::Array { element, .. } | Type::Vector { element, .. } => {
                    current = *element;
                    (q.layout.alloc_size(types, current), None)
                }
                _ => return None,
            }
        };
        let (value, from) = pattern::apint(q, *index).map_or((None, 0), |(v, w)| (Some(v), w));
        if at + 1 < idx {
            // Skip over the first indices: only the type walk matters.
            if let Some(structure) = field {
                let element = value? as u64;
                current = types.member(structure, element)?;
            }
            continue;
        }
        let value = value?;
        if value == 0 {
            if let Some(structure) = field {
                current = types.member(structure, 0).unwrap_or(structure);
            }
            continue;
        }
        if let Some(structure) = field {
            // Struct indices add their field offset to the pointer.
            let (_, offsets) = q.layout.struct_layout(types, structure);
            let element = value as usize;
            current = types.member(structure, element as u64)?;
            offset += *offsets.get(element)? as i64;
            continue;
        }
        // Otherwise a sequential type: the index times the element size.
        offset += stride as i64 * signed(value, from) as i64;
    }
    Some(offset)
}

/// `Value::getPointerOffsetFrom`
/// ([Value.cpp:1098](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Value.cpp#L1098)):
/// the constant number of bytes `this` is past `other`, if it is one.
pub fn get_pointer_offset_from(
    q: &Query,
    this: Operand,
    other: Operand,
) -> Option<i64> {
    let index_bits = |v: Operand| {
        q.type_of(v).and_then(|ty| pointer_space(q, ty)).map_or(64, |space| q.layout.pointer(space).index_bits)
    };
    let (width1, width2) = (index_bits(other), index_bits(this));
    let (ptr1, offset1) = strip_and_accumulate_constant_offsets(q, other, width1, true);
    let (ptr2, offset2) = strip_and_accumulate_constant_offsets(q, this, width2, true);
    let (offset1, offset2) = (signed(offset1, width1) as i64, signed(offset2, width2) as i64);

    // Handle the trivial case first.
    if ptr1 == ptr2 {
        return Some(offset2 - offset1);
    }
    let gep1 = operator(q, ptr1).and_then(|operator| gep_of(&operator))?;
    let gep2 = operator(q, ptr2).and_then(|operator| gep_of(&operator))?;
    // Right now we handle the case when Ptr1/Ptr2 are both GEPs with an
    // identical base. After that base, they may have some number of common (and
    // potentially variable) indices. After that they handle some constant
    // offset, which determines their offset from each other.
    if gep1.pointer != gep2.pointer || gep1.source != gep2.source {
        return None;
    }
    // Skip any common indices and track the GEP types.
    let mut idx = 1;
    while idx != gep1.indices.len() + 1
        && idx != gep2.indices.len() + 1
        && gep1.indices[idx - 1] == gep2.indices[idx - 1]
    {
        idx += 1;
    }
    let (index1, index2) = (get_offset_from_index(q, &gep1, idx)?, get_offset_from_index(q, &gep2, idx)?);
    Some(index2 - index1 + offset2 - offset1)
}
