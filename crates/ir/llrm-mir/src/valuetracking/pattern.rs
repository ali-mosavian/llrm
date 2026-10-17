//! LLVM's `PatternMatch.h` on MIR: the `m_*` matchers ValueTracking uses.
//!
//! # How it works
//! Each matcher takes an operand and answers what the operand is: the operands
//! of the binary operation it is, the constant it is. One function per `m_*`,
//! named for it. Add the matcher a translation needs here, at the end, under a
//! comment naming the file that needs it; two files needing one matcher share
//! it.
//!
//! # Example
//! `binary(q, v, BinaryOp::And)` on `%y = and i32 %a, 7` is `Some((%a, 7))`.

use super::query::Query;
use crate::module::{InstId, Instruction, Operand};
use crate::opcode::{BinaryOp, CastOp, Opcode};

/// `m_BinOp`-family: `v` is `op a, b`: its instruction and both operands.
pub fn binary<'a>(
    q: &Query<'a>,
    v: Operand,
    op: BinaryOp,
) -> Option<(InstId, &'a Instruction, Operand, Operand)> {
    let (inst, instruction) = q.instruction_of(v)?;
    (instruction.opcode == Opcode::Binary(op))
        .then(|| (inst, instruction, instruction.operands[0], instruction.operands[1]))
}

// ---- known.rs

use std::cell::Cell;

use crate::constantrange::{ConstantRange, PreferredRangeType};
use crate::context::{ConstantKind, mask};
use crate::module::{BlockId, Function, ValueDef};
use crate::opcode::{Attribute, CallInfo, Flags, IntPredicate};

/// A matcher: LLVM's `m_*` object, called on one operand.
pub type Matcher<'a> = &'a dyn Fn(Operand) -> bool;

/// `m_Value()`.
pub fn any(_: Operand) -> bool {
    true
}

/// `m_Value(X)`: matches anything and remembers it.
pub fn bind(cell: &Cell<Option<Operand>>) -> impl Fn(Operand) -> bool + '_ {
    move |v| {
        cell.set(Some(v));
        true
    }
}

/// `m_Specific(X)` and `m_Deferred(X)`: the value `cell` holds when asked.
pub fn specific(cell: &Cell<Option<Operand>>) -> impl Fn(Operand) -> bool + '_ {
    move |v| cell.get() == Some(v)
}

/// `m_Specific(V)`.
pub fn exactly(value: Operand) -> impl Fn(Operand) -> bool {
    move |v| v == value
}

/// `m_APInt(C)` with the value kept: the constant integer and its width.
pub fn apint(
    q: &Query,
    v: Operand,
) -> Option<(u128, u32)> {
    q.int_constant(v)
}

/// `m_APInt(C)`, remembering the constant in `cell`.
pub fn bind_apint<'a>(
    q: &'a Query,
    cell: &'a Cell<Option<(u128, u32)>>,
) -> impl Fn(Operand) -> bool + 'a {
    move |v| {
        let found = q.int_constant(v);
        if found.is_some() {
            cell.set(found);
        }
        found.is_some()
    }
}

/// `m_ZeroInt()`.
pub fn zero_int(
    q: &Query,
    v: Operand,
) -> bool {
    q.int_constant(v).is_some_and(|(bits, _)| bits == 0)
}

/// `m_One()`.
pub fn one(
    q: &Query,
    v: Operand,
) -> bool {
    q.int_constant(v).is_some_and(|(bits, _)| bits == 1)
}

/// `m_AllOnes()`.
pub fn all_ones(
    q: &Query,
    v: Operand,
) -> bool {
    q.int_constant(v).is_some_and(|(bits, width)| bits == mask(width))
}

/// `m_SpecificInt(Value)`: a constant integer of that unsigned value.
pub fn specific_int(
    q: &Query,
    v: Operand,
    value: u128,
) -> bool {
    q.int_constant(v).is_some_and(|(bits, _)| bits == value)
}

/// `m_Zero()`: any null constant.
pub fn zero(
    q: &Query,
    v: Operand,
) -> bool {
    let Operand::Constant(id) = v else { return false };
    matches!(
        q.context().get(id).kind,
        ConstantKind::Int(0) | ConstantKind::Zero | ConstantKind::Null
    )
}

/// `BinaryOp_match<L, R, Opcode, Commutable>` on a binary operator with the
/// flags `needs`: left is tried first, on the first operand, then, if
/// `commutable`, on the second.
pub fn match_binary(
    q: &Query,
    v: Operand,
    op: BinaryOp,
    needs: Flags,
    commutable: bool,
    l: Matcher,
    r: Matcher,
) -> bool {
    let Some((_, instruction)) = q.instruction_of(v) else { return false };
    if instruction.opcode != Opcode::Binary(op) || !instruction.flags.contains(needs) {
        return false;
    }
    let (a, b) = (instruction.operands[0], instruction.operands[1]);
    (l(a) && r(b)) || (commutable && l(b) && r(a))
}

/// `AnyBinaryOp_match<L, R, Commutable>`: the operator's opcode as well.
pub fn match_any_binary(
    q: &Query,
    v: Operand,
    commutable: bool,
    l: Matcher,
    r: Matcher,
) -> Option<BinaryOp> {
    let (_, instruction) = q.instruction_of(v)?;
    let Opcode::Binary(op) = instruction.opcode else { return None };
    let (a, b) = (instruction.operands[0], instruction.operands[1]);
    ((l(a) && r(b)) || (commutable && l(b) && r(a))).then_some(op)
}

/// `m_Not(V)`: `xor V, -1` in either order.
pub fn not(
    q: &Query,
    v: Operand,
    inner: Matcher,
) -> bool {
    match_binary(q, v, BinaryOp::Xor, Flags::default(), true, &|x| all_ones(q, x), inner)
}

/// `m_Neg(V)`: `sub 0, V`.
pub fn neg(
    q: &Query,
    v: Operand,
    inner: Matcher,
) -> bool {
    match_binary(q, v, BinaryOp::Sub, Flags::default(), false, &|x| zero_int(q, x), inner)
}

/// `m_ZExt(V)`, `m_SExt(V)`, `m_Trunc(V)`: the cast and its operand matching.
pub fn cast(
    q: &Query,
    v: Operand,
    op: CastOp,
    inner: Matcher,
) -> bool {
    let Some((_, instruction)) = q.instruction_of(v) else { return false };
    instruction.opcode == Opcode::Cast(op) && inner(instruction.operands[0])
}

/// `m_ICmp(Pred, L, R)` with `m_c_ICmp`'s swap when `commutable`: the
/// predicate as matched.
pub fn match_icmp(
    q: &Query,
    v: Operand,
    commutable: bool,
    l: Matcher,
    r: Matcher,
) -> Option<IntPredicate> {
    let (_, instruction) = q.instruction_of(v)?;
    let Opcode::ICmp(pred) = instruction.opcode else { return None };
    let (a, b) = (instruction.operands[0], instruction.operands[1]);
    if l(a) && r(b) {
        return Some(pred);
    }
    (commutable && l(b) && r(a)).then(|| pred.swapped())
}

/// A call's arguments: its operands but the callee (and, of an invoke, its
/// two destinations).
pub fn call_arguments(instruction: &Instruction) -> &[Operand] {
    let tail = if matches!(instruction.opcode, Opcode::Invoke(_)) { 3 } else { 1 };
    &instruction.operands[..instruction.operands.len() - tail]
}

/// `CallBase::getCalledFunction()`.
pub fn called_function<'a>(
    q: &Query<'a>,
    inst: InstId,
) -> Option<&'a Function> {
    let instruction = q.function.instruction(inst);
    let (Opcode::Call(_) | Opcode::Invoke(_)) = instruction.opcode else { return None };
    let Operand::Constant(id) = *instruction.operands.last()? else { return None };
    let ConstantKind::Global(global) = q.context().get(id).kind else { return None };
    q.globals.get(global.0 as usize)?.function()
}

fn call_info(instruction: &Instruction) -> Option<&CallInfo> {
    match &instruction.opcode {
        Opcode::Call(info) | Opcode::Invoke(info) => Some(info),
        _ => None,
    }
}

fn range_attribute(
    q: &Query,
    attrs: &[Attribute],
) -> Option<ConstantRange> {
    attrs
        .iter()
        .find_map(
            |attr| match attr {
                Attribute::Range { ty, lower, upper } => {
                    Some(ConstantRange::new(q.context().types.int_bits(*ty)?, *lower, *upper))
                }
                _ => None,
            },
        )
}

/// `Argument::getRange()`: the `range` attribute of parameter `index`.
pub fn argument_range(
    q: &Query,
    index: u32,
) -> Option<ConstantRange> {
    range_attribute(q, q.function.parameter_attrs.get(index as usize)?)
}

/// `CallBase::getRange()`: the call's `range` return attribute, intersected
/// with the callee's.
pub fn call_range(
    q: &Query,
    inst: InstId,
) -> Option<ConstantRange> {
    let call = call_info(q.function.instruction(inst))?;
    let at_call = range_attribute(q, &call.return_attrs);
    let at_function = called_function(q, inst).and_then(|function| range_attribute(q, &function.return_attrs));
    match (at_call, at_function) {
        (Some(one), Some(other)) => Some(one.intersect_with(&other, PreferredRangeType::Smallest)),
        (one, other) => one.or(other),
    }
}

/// `CallBase::getReturnedArgOperand()`.
pub fn returned_argument_operand(
    q: &Query,
    inst: InstId,
) -> Option<Operand> {
    let instruction = q.function.instruction(inst);
    let returned = |attrs: &Vec<Vec<Attribute>>| {
        attrs.iter().position(|one| one.iter().any(|attr| matches!(attr, Attribute::Flag(name) if name == "returned")))
    };
    let call = call_info(instruction)?;
    let index = returned(&call.argument_attrs)
        .or_else(|| called_function(q, inst).and_then(|function| returned(&function.parameter_attrs)))?;
    call_arguments(instruction).get(index).copied()
}

/// `CallBase::getRetAlign()`.
pub fn call_return_align(
    q: &Query,
    inst: InstId,
) -> Option<u64> {
    let align = |attrs: &[Attribute]| {
        attrs
            .iter()
            .find_map(
                |attr| match attr {
                    Attribute::Int(name, value) if name == "align" => Some(*value),
                    _ => None,
                },
            )
    };
    let call = call_info(q.function.instruction(inst))?;
    align(&call.return_attrs).or_else(|| called_function(q, inst).and_then(|function| align(&function.return_attrs)))
}

/// The incoming values and blocks of a phi.
pub fn phi_incoming(instruction: &Instruction) -> impl Iterator<Item = (Operand, BlockId)> + '_ {
    instruction
        .operands
        .chunks(2)
        .map(
            |pair| {
                let Operand::Block(from) = pair[1] else { unreachable!("a phi's block") };
                (pair[0], from)
            },
        )
}

/// The result of `inst` as an operand.
pub fn result_of(instruction: &Instruction) -> Operand {
    Operand::Value(instruction.result.expect("an instruction with a result"))
}

/// The instruction that defines `v` when it is an instruction of class phi.
pub fn phi_of<'a>(
    q: &Query<'a>,
    v: Operand,
) -> Option<(InstId, &'a Instruction)> {
    q.instruction_of(v).filter(|(_, instruction)| instruction.opcode == Opcode::Phi)
}

/// `m_c_BinOp(m_Phi(P), m_Value())` on `bo`: the phi that is one of its
/// operands.
pub fn phi_operand_of<'a>(
    q: &Query<'a>,
    bo: &Instruction,
) -> Option<(InstId, &'a Instruction)> {
    phi_of(q, bo.operands[0]).or_else(|| phi_of(q, bo.operands[1]))
}

/// What `Intrinsic::ID` names in ValueTracking: the intrinsics MIR does not
/// enumerate, matched by the callee's name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntrinsicId {
    Abs,
    Assume,
    Bitreverse,
    Bswap,
    Clmul,
    Ctlz,
    Ctpop,
    Cttz,
    Fshl,
    Fshr,
    Pdep,
    Pext,
    Ptrmask,
    Smax,
    Smin,
    Smulh,
    Umax,
    Umin,
    Umulh,
    UaddSat,
    UsubSat,
    SaddSat,
    SsubSat,
    UaddWithOverflow,
    SaddWithOverflow,
    UsubWithOverflow,
    SsubWithOverflow,
    UmulWithOverflow,
    SmulWithOverflow,
    Vscale,
    X86Sse42Crc32_64_64,
    AmdgcnMbcntHi,
    AmdgcnMbcntLo,
    RiscvVsetvli,
    RiscvVsetvlimax,
}

const INTRINSICS: [(&str, IntrinsicId); 35] = [
    ("llvm.abs", IntrinsicId::Abs),
    ("llvm.assume", IntrinsicId::Assume),
    ("llvm.bitreverse", IntrinsicId::Bitreverse),
    ("llvm.bswap", IntrinsicId::Bswap),
    ("llvm.clmul", IntrinsicId::Clmul),
    ("llvm.ctlz", IntrinsicId::Ctlz),
    ("llvm.ctpop", IntrinsicId::Ctpop),
    ("llvm.cttz", IntrinsicId::Cttz),
    ("llvm.fshl", IntrinsicId::Fshl),
    ("llvm.fshr", IntrinsicId::Fshr),
    ("llvm.pdep", IntrinsicId::Pdep),
    ("llvm.pext", IntrinsicId::Pext),
    ("llvm.ptrmask", IntrinsicId::Ptrmask),
    ("llvm.smax", IntrinsicId::Smax),
    ("llvm.smin", IntrinsicId::Smin),
    ("llvm.smulh", IntrinsicId::Smulh),
    ("llvm.umax", IntrinsicId::Umax),
    ("llvm.umin", IntrinsicId::Umin),
    ("llvm.umulh", IntrinsicId::Umulh),
    ("llvm.uadd.sat", IntrinsicId::UaddSat),
    ("llvm.usub.sat", IntrinsicId::UsubSat),
    ("llvm.sadd.sat", IntrinsicId::SaddSat),
    ("llvm.ssub.sat", IntrinsicId::SsubSat),
    ("llvm.uadd.with.overflow", IntrinsicId::UaddWithOverflow),
    ("llvm.sadd.with.overflow", IntrinsicId::SaddWithOverflow),
    ("llvm.usub.with.overflow", IntrinsicId::UsubWithOverflow),
    ("llvm.ssub.with.overflow", IntrinsicId::SsubWithOverflow),
    ("llvm.umul.with.overflow", IntrinsicId::UmulWithOverflow),
    ("llvm.smul.with.overflow", IntrinsicId::SmulWithOverflow),
    ("llvm.vscale", IntrinsicId::Vscale),
    ("llvm.x86.sse42.crc32.64.64", IntrinsicId::X86Sse42Crc32_64_64),
    ("llvm.amdgcn.mbcnt.hi", IntrinsicId::AmdgcnMbcntHi),
    ("llvm.amdgcn.mbcnt.lo", IntrinsicId::AmdgcnMbcntLo),
    ("llvm.riscv.vsetvli", IntrinsicId::RiscvVsetvli),
    ("llvm.riscv.vsetvlimax", IntrinsicId::RiscvVsetvlimax),
];

/// `IntrinsicInst::getIntrinsicID()` for the intrinsics above: the call's
/// callee name, less its `.iN` type suffix.
pub fn intrinsic_id(
    q: &Query,
    inst: InstId,
) -> Option<IntrinsicId> {
    if !matches!(q.function.instruction(inst).opcode, Opcode::Call(_)) {
        return None;
    }
    let name = q.callee_name(inst)?;
    INTRINSICS
        .iter()
        .filter(|(base, _)| name.strip_prefix(base).is_some_and(|rest| rest.is_empty() || rest.starts_with('.')))
        .max_by_key(|(base, _)| base.len())
        .map(|(_, id)| *id)
}

/// `isa<IntrinsicInst>(I)`: a call to a function named `llvm.*`.
pub fn is_intrinsic(
    q: &Query,
    inst: InstId,
) -> bool {
    matches!(q.function.instruction(inst).opcode, Opcode::Call(_))
        && q.callee_name(inst).is_some_and(|name| name.starts_with("llvm."))
}

/// `dyn_cast<IntrinsicInst>(V)`.
pub fn intrinsic_call<'a>(
    q: &Query<'a>,
    v: Operand,
) -> Option<(InstId, &'a Instruction)> {
    let (inst, instruction) = q.instruction_of(v)?;
    is_intrinsic(q, inst).then_some((inst, instruction))
}

/// `matchTwoInputRecurrence<InstTy>(PN, Inst, Init, OtherOp)`: `accept` says
/// whether an incoming value is an `InstTy`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9695>
pub fn match_two_input_recurrence<'a>(
    q: &Query<'a>,
    phi: InstId,
    accept: &dyn Fn(InstId, &Instruction) -> bool,
) -> Option<(InstId, Operand, Operand)> {
    let instruction = q.function.instruction(phi);
    if instruction.operands.len() != 4 {
        return None;
    }
    let this = result_of(instruction);
    let incoming: Vec<Operand> = phi_incoming(instruction).map(|(value, _)| value).collect();
    for i in 0..2 {
        let Some((inst, operation)) = q.instruction_of(incoming[i]) else { continue };
        if !accept(inst, operation) || operation.operands.len() < 2 {
            continue;
        }
        let (lhs, rhs) = (operation.operands[0], operation.operands[1]);
        if lhs != this && rhs != this {
            continue;
        }
        return Some((inst, incoming[1 - i], if lhs == this { rhs } else { lhs }));
    }
    None
}

/// `matchThreeInputRecurrence<InstTy>(PN, Inst, Init, OtherOp0, OtherOp1)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9722>
pub fn match_three_input_recurrence<'a>(
    q: &Query<'a>,
    phi: InstId,
    accept: &dyn Fn(InstId, &Instruction) -> bool,
) -> Option<(InstId, Operand, Operand, Operand)> {
    let instruction = q.function.instruction(phi);
    if instruction.operands.len() != 4 {
        return None;
    }
    let this = result_of(instruction);
    let incoming: Vec<Operand> = phi_incoming(instruction).map(|(value, _)| value).collect();
    for i in 0..2 {
        let Some((inst, operation)) = q.instruction_of(incoming[i]) else { continue };
        if !accept(inst, operation) || operation.operands.len() < 3 {
            continue;
        }
        let (op0, op1, op2) = (operation.operands[0], operation.operands[1], operation.operands[2]);
        if op0 != this && op1 != this && op2 != this {
            continue;
        }
        let (other0, other1) = if op0 == this {
            (op1, op2)
        } else if op1 == this {
            (op0, op2)
        } else {
            (op0, op1)
        };
        return Some((inst, incoming[1 - i], other0, other1));
    }
    None
}

/// `matchSimpleRecurrence(P, BO, Start, Step)`: `%iv = phi [Start], [%iv.next]`
/// with `%iv.next = binop %iv, Step` or `binop Step, %iv`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9755>
pub fn match_simple_recurrence<'a>(
    q: &Query<'a>,
    phi: InstId,
) -> Option<(InstId, Operand, Operand)> {
    match_two_input_recurrence(q, phi, &|_, operation| matches!(operation.opcode, Opcode::Binary(_)))
}

/// `matchSimpleRecurrence(I, P, Start, Step)`: the phi `bo` is the
/// recurrence of, and its start and step.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9766>
pub fn match_simple_recurrence_of<'a>(
    q: &Query<'a>,
    bo: InstId,
) -> Option<(InstId, Operand, Operand)> {
    let instruction = q.function.instruction(bo);
    if !matches!(instruction.opcode, Opcode::Binary(_)) {
        return None;
    }
    let (phi, _) = phi_operand_of(q, instruction)?;
    let (found, start, step) = match_simple_recurrence(q, phi)?;
    (found == bo).then_some((phi, start, step))
}

/// `matchSimpleBinaryIntrinsicRecurrence(I, P, Init, OtherOp)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9773>
pub fn match_simple_binary_intrinsic_recurrence<'a>(
    q: &Query<'a>,
    inst: InstId,
) -> Option<(InstId, Operand, Operand)> {
    let instruction = q.function.instruction(inst);
    let arguments = call_arguments(instruction);
    let ty = instruction.ty;
    if arguments.len() != 2 || q.type_of(arguments[0]) != Some(ty) || q.type_of(arguments[1]) != Some(ty) {
        return None;
    }
    let phi = phi_of(q, arguments[0]).or_else(|| phi_of(q, arguments[1]))?.0;
    let (found, init, other) = match_two_input_recurrence(q, phi, &|inst, _| is_intrinsic(q, inst))?;
    (found == inst).then_some((phi, init, other))
}

/// `matchSimpleTernaryIntrinsicRecurrence(I, P, Init, OtherOp0, OtherOp1)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L9789>
pub fn match_simple_ternary_intrinsic_recurrence<'a>(
    q: &Query<'a>,
    inst: InstId,
) -> Option<(InstId, Operand, Operand, Operand)> {
    let instruction = q.function.instruction(inst);
    let arguments = call_arguments(instruction);
    let ty = instruction.ty;
    if arguments.len() != 3 || arguments.iter().any(|argument| q.type_of(*argument) != Some(ty)) {
        return None;
    }
    let phi = phi_of(q, arguments[0]).or_else(|| phi_of(q, arguments[1])).or_else(|| phi_of(q, arguments[2]))?.0;
    let (found, init, other0, other1) = match_three_input_recurrence(q, phi, &|inst, _| is_intrinsic(q, inst))?;
    (found == inst).then_some((phi, init, other0, other1))
}

/// Whether `v` is an argument (`isa<Argument>`).
pub fn is_argument(
    q: &Query,
    v: Operand,
) -> bool {
    matches!(
        v,
        Operand::Value(id) if matches!(q.function.value(id).def, ValueDef::Argument(_))
    )
}
