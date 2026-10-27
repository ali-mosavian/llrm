//! Whether a value is defined: `isGuaranteedNotToBeUndefOrPoison` and what it
//! needs.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ValueTracking.cpp`
//! (`isGuaranteedNotToBeUndefOrPoison`, `canCreateUndefOrPoison`,
//! `impliesPoison`, `isGuaranteedToTransferExecutionToSuccessor`,
//! `propagatesPoison`, `programUndefinedIfPoison` and the helpers), each
//! function linking the line it translates; `Instruction::mayThrow`,
//! `willReturn` and `isVolatile` and `CallBase`'s attribute lookups, which
//! they call, are translated here and link `Instruction.cpp` and
//! `Instructions.cpp`. `Query::dt`, `Query::assumptions` and `Query::cxt` are
//! the `DT`, `AC` and `CtxI` arguments. An `Operator` is an instruction or a
//! constant expression. Not translated, as MIR has no such thing:
//! * `undef`: the `UndefValue` constant (8154-8156) and its vector element
//!   (8168); no value is undef, so `UndefOnly` is asked of the same code.
//! * Vectors: `insertelement`, `extractelement` and `shufflevector` (8027-8043,
//!   8224-8229) and a constant expression splat (8162-8167).
//! * `nofpclass` and `addrspacecast nonnull`
//!   (`hasPoisonGeneratingAnnotations`): MIR cannot state them.
//!   `nocreateundeforpoison` (8025) is read but no MIR declaration carries it,
//!   intrinsics included.
//! * Operand bundles: `getKnowledgeValidInContext` (8287) finds none.
//! * Atomics, `catchpad` and the other funclet instructions (8387-8398,
//!   8571-8579, `mayThrow`), `GlobalAlias` and `MetadataAsValue` (8140).
//! * `programUndefinedIfUndef` is not in this commit of LLVM.
//!
//! # Example
//! A `freeze` is never undef or poison.

use super::pattern::{self, IntrinsicId, arg_size, call_info, called_function};
use super::query::Query;
use super::range::shift_amount_known_in_range;
use crate::context::{ConstantExpr, ConstantId, ConstantKind};
use crate::facts::Facts;
use crate::hash::HashSet;
use crate::loopinfo::Loop;
use crate::module::{BlockId, InstId, Instruction, Operand, ValueDef};
use crate::opcode::{Attribute, BinaryOp, CastOp, Clause, Flags, Opcode};
use crate::types::Type;

/// `UndefPoisonKind`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UndefPoisonKind {
    PoisonOnly,
    UndefOnly,
    UndefOrPoison,
}

/// `includesPoison(Kind)`.
fn includes_poison(kind: UndefPoisonKind) -> bool {
    matches!(
        kind,
        UndefPoisonKind::PoisonOnly | UndefPoisonKind::UndefOrPoison
    )
}

/// `includesUndef(Kind)`.
fn includes_undef(kind: UndefPoisonKind) -> bool {
    matches!(
        kind,
        UndefPoisonKind::UndefOnly | UndefPoisonKind::UndefOrPoison
    )
}

/// LLVM's `Operator`: an instruction or a constant expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operator {
    Instruction(InstId),
    Expr(ConstantId),
}

impl Operator {
    /// `dyn_cast<Operator>(V)`.
    pub fn of(
        q: &Query,
        v: Operand,
    ) -> Option<Operator> {
        match v {
            Operand::Value(_) => q.instruction_of(v).map(|(inst, _)| Operator::Instruction(inst)),
            Operand::Constant(id) => {
                matches!(q.context().get(id).kind, ConstantKind::Expr(_)).then_some(Operator::Expr(id))
            }
            Operand::Block(_) => None,
        }
    }

    /// `Operator::operands()`.
    fn operands(
        self,
        q: &Query,
    ) -> Vec<Operand> {
        match self {
            Operator::Instruction(inst) => q.function.instruction(inst).operands.clone(),
            Operator::Expr(id) => match &q.context().get(id).kind {
                ConstantKind::Expr(ConstantExpr::GetElementPtr { operands, .. }) => {
                    operands.iter().map(|one| Operand::Constant(*one)).collect()
                }
                ConstantKind::Expr(ConstantExpr::Cast { value, .. }) => vec![Operand::Constant(*value)],
                _ => Vec::new(),
            },
        }
    }
}

fn key(attribute: &Attribute) -> &str {
    match attribute {
        Attribute::Flag(name) | Attribute::Int(name, _) | Attribute::Type(name, _) | Attribute::Str(name, _) => name,
        Attribute::Memory(_) => "memory",
        Attribute::Range { .. } => "range",
        Attribute::Initializes(_) => "initializes",
    }
}

fn has(
    attributes: &[Attribute],
    name: &str,
) -> bool {
    attributes.iter().any(|one| key(one) == name)
}

/// `CallBase::hasFnAttr(Kind)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instructions.cpp#L463>
fn has_fn_attr(
    q: &Query,
    inst: InstId,
    name: &str,
) -> bool {
    let Some(info) = call_info(q.function.instruction(inst)) else { return false };
    has(&info.attrs, name)
        || called_function(q, q.function.instruction(inst)).is_some_and(|callee| has(&callee.attrs, name))
}

/// `CallBase::hasRetAttr(Kind)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instructions.cpp#L2414>
fn has_ret_attr(
    q: &Query,
    inst: InstId,
    name: &str,
) -> bool {
    let Some(info) = call_info(q.function.instruction(inst)) else { return false };
    has(&info.return_attrs, name)
        || called_function(q, q.function.instruction(inst)).is_some_and(|callee| has(&callee.return_attrs, name))
}

/// `CallBase::paramHasAttr(ArgNo, Kind)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instructions.cpp#L420>
fn param_has_attr(
    q: &Query,
    inst: InstId,
    arg_no: usize,
    name: &str,
) -> bool {
    let Some(info) = call_info(q.function.instruction(inst)) else { return false };
    info.argument_attrs.get(arg_no).is_some_and(|attrs| has(attrs, name))
        || called_function(q, q.function.instruction(inst))
            .is_some_and(|callee| callee.parameter_attrs.get(arg_no).is_some_and(|attrs| has(attrs, name)))
}

/// `CallBase::getReturnedArgOperand()`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instructions.cpp#L407>
fn returned_arg_operand(
    q: &Query,
    inst: InstId,
) -> Option<Operand> {
    let instruction = q.function.instruction(inst);
    let info = call_info(instruction)?;
    let somewhere = |lists: &[Vec<Attribute>]| lists.iter().position(|attrs| has(attrs, "returned"));
    let index = somewhere(&info.argument_attrs)
        .or_else(|| called_function(q, q.function.instruction(inst)).and_then(|f| somewhere(&f.parameter_attrs)))?;
    instruction.operands.get(index).copied().filter(|_| index < arg_size(instruction))
}

/// `CallBase::isIndirectCall()`: the callee is not a constant.
fn is_indirect_call(instruction: &Instruction) -> bool {
    matches!(instruction.operands.last(), Some(Operand::Value(_)))
}

/// `Instruction::isVolatile()`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instruction.cpp#L1216>
fn is_volatile(
    q: &Query,
    inst: InstId,
) -> bool {
    let instruction = q.function.instruction(inst);
    match &instruction.opcode {
        Opcode::Store { volatile, .. } | Opcode::Load { volatile, .. } => *volatile,
        Opcode::Call(_) | Opcode::Invoke(_) => {
            let one_at = |at: usize| {
                instruction.operands.get(at).and_then(|op| q.int_constant(*op)).is_some_and(|(bits, _)| bits != 0)
            };
            match (q.intrinsic(inst), pattern::intrinsic_id(q, inst)) {
                (Some(crate::intrinsics::Intrinsic::MemCpy | crate::intrinsics::Intrinsic::MemMove), _)
                | (Some(crate::intrinsics::Intrinsic::MemSet), _) => one_at(3),
                (_, Some(IntrinsicId::MatrixColumnMajorLoad)) => one_at(2),
                (_, Some(IntrinsicId::MatrixColumnMajorStore)) => one_at(3),
                _ => false,
            }
        }
        _ => false,
    }
}

/// `Instruction::willReturn()`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instruction.cpp#L1367>
pub fn will_return(
    q: &Query,
    inst: InstId,
) -> bool {
    if is_volatile(q, inst) {
        return false;
    }
    if call_info(q.function.instruction(inst)).is_some() {
        return has_fn_attr(q, inst, "willreturn");
    }
    true
}

/// `canUnwindPastLandingPad(LP, IncludePhaseOneUnwind)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instruction.cpp#L1309>
fn can_unwind_past_landing_pad(
    q: &Query,
    landing_pad: InstId,
    include_phase_one_unwind: bool,
) -> bool {
    let instruction = q.function.instruction(landing_pad);
    let Opcode::LandingPad { cleanup, clauses } = &instruction.opcode else { return true };
    if *cleanup {
        return include_phase_one_unwind;
    }
    for (clause, operand) in clauses.iter().zip(&instruction.operands) {
        let Operand::Constant(id) = *operand else { continue };
        let constant = q.context().get(id);
        match clause {
            Clause::Catch if is_null_pointer(q, *operand) => return false,
            Clause::Filter if matches!(
                q.context().types.get(constant.ty),
                Type::Array { count: 0, .. }
            ) => {
                return false;
            }
            _ => {}
        }
    }
    true
}

/// `Instruction::mayThrow()`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instruction.cpp#L1331>
pub fn may_throw(
    q: &Query,
    inst: InstId,
) -> bool {
    let instruction = q.function.instruction(inst);
    match &instruction.opcode {
        Opcode::Call(_) => !has_fn_attr(q, inst, "nounwind"),
        Opcode::Resume => true,
        Opcode::Invoke(_) => {
            let unwind = instruction.operands.len().checked_sub(2).map(|at| instruction.operands[at]);
            let Some(Operand::Block(destination)) = unwind else { return false };
            let pad = q.function.iter_block(destination).find(|one| q.function.instruction(*one).opcode != Opcode::Phi);
            match pad {
                Some(pad) if matches!(q.function.instruction(pad).opcode, Opcode::LandingPad { .. }) => {
                    can_unwind_past_landing_pad(q, pad, false)
                }
                _ => false,
            }
        }
        _ => false,
    }
}

/// `isa<ConstantPointerNull>(V)`.
fn is_null_pointer(
    q: &Query,
    v: Operand,
) -> bool {
    let Operand::Constant(id) = v else { return false };
    let constant = q.context().get(id);
    match constant.kind {
        ConstantKind::Null => true,
        ConstantKind::Zero => matches!(q.context().types.get(constant.ty), Type::Pointer(_)),
        _ => false,
    }
}

/// `Operator::hasPoisonGeneratingFlags()`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Operator.cpp#L23>
fn has_poison_generating_flags(
    q: &Query,
    op: Operator,
) -> bool {
    let inst = match op {
        Operator::Expr(id) => {
            return matches!(
                q.context().get(id).kind,
                ConstantKind::Expr(ConstantExpr::GetElementPtr { inbounds: true, .. })
            );
        }
        Operator::Instruction(inst) => inst,
    };
    let instruction = q.function.instruction(inst);
    let any = |flags: &[Flags]| flags.iter().any(|flag| instruction.flags.contains(*flag));
    match &instruction.opcode {
        Opcode::Binary(BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl) | Opcode::Cast(CastOp::Trunc) => {
            any(&[Flags::NUW, Flags::NSW])
        }
        Opcode::Binary(BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::AShr | BinaryOp::LShr) => any(&[Flags::EXACT]),
        Opcode::Binary(BinaryOp::Or) => any(&[Flags::DISJOINT]),
        Opcode::GetElementPtr { .. } => any(&[Flags::INBOUNDS, Flags::NUSW, Flags::NUW]),
        Opcode::Cast(CastOp::UIToFP | CastOp::ZExt) => any(&[Flags::NNEG]),
        Opcode::ICmp(_) => any(&[Flags::SAMESIGN]),
        Opcode::Cast(CastOp::AddrSpaceCast) => false,
        opcode => {
            if matches!(opcode, Opcode::Call(_))
                && matches!(
                    pattern::intrinsic_id(q, inst),
                    Some(IntrinsicId::Ctlz | IntrinsicId::Cttz | IntrinsicId::Abs)
                )
            {
                return instruction
                    .operands
                    .get(1)
                    .and_then(|op| q.int_constant(*op))
                    .is_some_and(|(bits, _)| bits == 1);
            }
            is_fp_math_operator(q, inst) && any(&[Flags::NNAN, Flags::NINF])
        }
    }
}

/// `FPMathOperator::classof`.
fn is_fp_math_operator(
    q: &Query,
    inst: InstId,
) -> bool {
    let instruction = q.function.instruction(inst);
    match &instruction.opcode {
        Opcode::FNeg
        | Opcode::FCmp(_)
        | Opcode::Binary(BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul | BinaryOp::FDiv | BinaryOp::FRem) => true,
        Opcode::Phi | Opcode::Select | Opcode::Call(_) => {
            let types = &q.context().types;
            let mut ty = instruction.ty;
            while let Type::Array { element, .. } = types.get(ty) {
                ty = *element;
            }
            let scalar = match types.get(ty) {
                Type::Vector { element, .. } => *element,
                _ => ty,
            };
            matches!(types.get(scalar), Type::Float(_))
        }
        _ => false,
    }
}

/// `Instruction::hasPoisonGeneratingAttributes()`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instruction.cpp#L525>
fn has_poison_generating_attributes(instruction: &Instruction) -> bool {
    let Some(info) = call_info(instruction) else { return false };
    let poison_generating =
        |attrs: &[Attribute]| attrs.iter().any(|one| matches!(key(one), "range" | "align" | "nonnull"));
    poison_generating(&info.return_attrs) || info.argument_attrs.iter().any(|attrs| poison_generating(attrs))
}

/// `Instruction::hasPoisonGeneratingMetadata()`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instruction.cpp#L492>
fn has_poison_generating_metadata(instruction: &Instruction) -> bool {
    instruction.metadata.iter().any(|(kind, _)| matches!(kind.as_str(), "range" | "nonnull" | "align" | "nofpclass"))
}

/// `Operator::hasPoisonGeneratingAnnotations()`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Operator.cpp#L78>
fn has_poison_generating_annotations(
    q: &Query,
    op: Operator,
) -> bool {
    if has_poison_generating_flags(q, op) {
        return true;
    }
    let Operator::Instruction(inst) = op else { return false };
    let instruction = q.function.instruction(inst);
    has_poison_generating_attributes(instruction) || has_poison_generating_metadata(instruction)
}

/// `canCreateUndefOrPoison(Op, Kind, ConsiderFlagsAndMetadata)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7983>
fn can_create_undef_or_poison_kind(
    q: &Query,
    op: Operator,
    kind: UndefPoisonKind,
    consider_flags_and_metadata: bool,
) -> bool {
    if consider_flags_and_metadata && includes_poison(kind) && has_poison_generating_annotations(q, op) {
        return true;
    }
    let inst = match op {
        Operator::Expr(id) => {
            return match &q.context().get(id).kind {
                ConstantKind::Expr(ConstantExpr::Cast {
                    op: CastOp::FPToUI | CastOp::FPToSI | CastOp::AddrSpaceCast,
                    ..
                }) => true,
                ConstantKind::Expr(ConstantExpr::Cast { .. } | ConstantExpr::GetElementPtr { .. }) => false,
                _ => true,
            };
        }
        Operator::Instruction(inst) => inst,
    };
    let instruction = q.function.instruction(inst);
    let call_base = |q: &Query| !has_ret_attr(q, inst, "noundef") && !has_fn_attr(q, inst, "nocreateundeforpoison");
    match &instruction.opcode {
        Opcode::Binary(BinaryOp::Shl | BinaryOp::AShr | BinaryOp::LShr) => {
            includes_poison(kind) && !shift_amount_known_in_range(q, instruction.operands[1])
        }
        Opcode::Cast(CastOp::FPToSI | CastOp::FPToUI) => true,
        Opcode::Call(_) => {
            match pattern::intrinsic_id(q, inst) {
                Some(IntrinsicId::Ctlz | IntrinsicId::Cttz | IntrinsicId::Abs) => return false,
                Some(IntrinsicId::SshlSat | IntrinsicId::UshlSat) => {
                    if !includes_poison(kind) || shift_amount_known_in_range(q, instruction.operands[1]) {
                        return false;
                    }
                }
                _ => {}
            }
            call_base(q)
        }
        Opcode::Invoke(_) => call_base(q),
        Opcode::FNeg
        | Opcode::Phi
        | Opcode::Select
        | Opcode::ExtractValue(_)
        | Opcode::InsertValue(_)
        | Opcode::Freeze
        | Opcode::ICmp(_)
        | Opcode::FCmp(_)
        | Opcode::GetElementPtr { .. } => false,
        Opcode::Cast(CastOp::AddrSpaceCast) => true,
        Opcode::Cast(_) | Opcode::Binary(_) => false,
        _ => true,
    }
}

/// `canCreateUndefOrPoison(Op, ConsiderFlagsAndMetadata)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8068>
pub fn can_create_undef_or_poison(
    q: &Query,
    op: Operator,
    consider_flags_and_metadata: bool,
) -> bool {
    can_create_undef_or_poison_kind(q, op, UndefPoisonKind::UndefOrPoison, consider_flags_and_metadata)
}

/// `canCreatePoison(Op, ConsiderFlagsAndMetadata)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8074>
pub fn can_create_poison(
    q: &Query,
    op: Operator,
    consider_flags_and_metadata: bool,
) -> bool {
    can_create_undef_or_poison_kind(q, op, UndefPoisonKind::PoisonOnly, consider_flags_and_metadata)
}

/// `directlyImpliesPoison(ValAssumedPoison, V, Depth)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8079>
fn directly_implies_poison(
    q: &Query,
    val_assumed_poison: Operand,
    v: Operand,
    depth: u32,
) -> bool {
    if val_assumed_poison == v {
        return true;
    }
    const MAX_DEPTH: u32 = 2;
    if depth >= MAX_DEPTH {
        return false;
    }
    if let Some((inst, instruction)) = q.instruction_of(v) {
        if instruction.operands.iter().enumerate().any(|(at, op)| {
            propagates_poison(q, Operator::Instruction(inst), at)
                && directly_implies_poison(q, val_assumed_poison, *op, depth + 1)
        }) {
            return true;
        }
        if let Some((_, _, aggregate)) = pattern::extract_value(q, v)
            && let Some((_, call_instruction)) = pattern::with_overflow_inst(q, aggregate)
            && (pattern::extract_value(q, val_assumed_poison).is_some_and(|(_, _, of)| of == aggregate)
                || call_instruction.operands[..arg_size(call_instruction)].contains(&val_assumed_poison))
        {
            return true;
        }
    }
    false
}

/// `impliesPoison(ValAssumedPoison, V, Depth)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8107>
fn implies_poison_at(
    q: &Query,
    val_assumed_poison: Operand,
    v: Operand,
    depth: u32,
) -> bool {
    let bare = Query { dt: None, assumptions: None, cxt: None, ..*q };
    if is_guaranteed_not_to_be_poison(&bare, val_assumed_poison, 0) {
        return true;
    }
    if directly_implies_poison(q, val_assumed_poison, v, 0) {
        return true;
    }
    const MAX_DEPTH: u32 = 2;
    if depth >= MAX_DEPTH {
        return false;
    }
    if let Some((inst, instruction)) = q.instruction_of(val_assumed_poison)
        && !can_create_poison(q, Operator::Instruction(inst), true)
    {
        return instruction.operands.iter().all(|op| implies_poison_at(q, *op, v, depth + 1));
    }
    false
}

/// `impliesPoison(ValAssumedPoison, V)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8128>
pub fn implies_poison(
    q: &Query,
    val_assumed_poison: Operand,
    v: Operand,
) -> bool {
    implies_poison_at(q, val_assumed_poison, v, 0)
}

/// The pin's `SmallPtrSet<T, N>`: up to `N` members in place, a hash set past
/// that. The walks that use it ask about a handful of values, and a hash set
/// allocated on the first insert of each call was 6x the pin's cost per call
/// (`isSCEVExprNeverPoison`: 3.3 k Ir against 0.57 k, d_faces -O2).
pub struct SmallSet<T, const N: usize> {
    few: [Option<T>; N],
    len: usize,
    many: HashSet<T>,
}

impl<T, const N: usize> Default for SmallSet<T, N> {
    fn default() -> Self {
        Self { few: [const { None }; N], len: 0, many: HashSet::default() }
    }
}

impl<T: Copy + Eq + std::hash::Hash, const N: usize> SmallSet<T, N> {
    /// Whether the set outgrew its inline slots.
    #[cfg(test)]
    pub fn spilled(&self) -> bool {
        self.len > N
    }

    /// Whether `v` is in the set.
    pub fn contains(
        &self,
        v: &T,
    ) -> bool {
        if self.len <= N {
            self.few[..self.len].iter().any(|one| one.as_ref() == Some(v))
        } else {
            self.many.contains(v)
        }
    }

    /// Whether `v` was not in the set.
    pub fn insert(
        &mut self,
        v: T,
    ) -> bool {
        if self.len <= N {
            if self.contains(&v) {
                return false;
            }
            if self.len < N {
                self.few[self.len] = Some(v);
                self.len += 1;
                return true;
            }
            self.many.extend(self.few.iter().flatten().copied());
            self.len = N + 1;
        }
        self.many.insert(v)
    }
}

/// A cast chain is rarely longer than four.
pub(crate) type Seen = SmallSet<Operand, 4>;

/// `SmallPtrSet<const Value *, 16>` of the values that yield poison.
pub type PoisonSet = SmallSet<Operand, 16>;

/// `stripPointerCastsSameRepresentation()`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Value.cpp#L642>
pub fn strip_pointer_casts_same_representation(
    q: &Query,
    v: Operand,
) -> Operand {
    if !q.is_pointer(v) {
        return v;
    }
    let mut visited = Seen::default();
    let mut v = v;
    visited.insert(v);
    loop {
        let all_zero =
            |indices: &[Operand]| indices.iter().all(|one| q.int_constant(*one).is_some_and(|(bits, _)| bits == 0));
        let next = match Operator::of(q, v) {
            Some(Operator::Instruction(inst)) => {
                let instruction = q.function.instruction(inst);
                match &instruction.opcode {
                    Opcode::GetElementPtr { .. } => {
                        if !all_zero(&instruction.operands[1..]) {
                            return v;
                        }
                        instruction.operands[0]
                    }
                    Opcode::Cast(CastOp::BitCast) => {
                        if !q.is_pointer(instruction.operands[0]) {
                            return v;
                        }
                        instruction.operands[0]
                    }
                    Opcode::Call(_) | Opcode::Invoke(_) => match returned_arg_operand(q, inst) {
                        Some(returned) => returned,
                        None => return v,
                    },
                    _ => return v,
                }
            }
            Some(Operator::Expr(id)) => match &q.context().get(id).kind {
                ConstantKind::Expr(ConstantExpr::GetElementPtr { operands, .. }) => {
                    let operands: Vec<Operand> = operands.iter().map(|one| Operand::Constant(*one)).collect();
                    if !all_zero(&operands[1..]) {
                        return v;
                    }
                    operands[0]
                }
                ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::BitCast, value }) => {
                    if !q.is_pointer(Operand::Constant(*value)) {
                        return v;
                    }
                    Operand::Constant(*value)
                }
                _ => return v,
            },
            None => return v,
        };
        v = next;
        if !visited.insert(v) {
            return v;
        }
    }
}

/// Whether `v` is a constant of a vector type.
fn is_vector_constant(
    q: &Query,
    v: Operand,
) -> bool {
    matches!(
        q.type_of(v).map(|ty| q.context().types.get(ty)),
        Some(Type::Vector { .. })
    )
}

/// `isGuaranteedNotToBeUndefOrPoison(V, AC, CtxI, DT, Depth, Kind)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8134>
fn is_guaranteed_not_to_be_undef_or_poison_kind(
    q: &Query,
    v: Operand,
    depth: u32,
    kind: UndefPoisonKind,
) -> bool {
    if depth >= super::MAX_ANALYSIS_RECURSION_DEPTH {
        return false;
    }

    if let Operand::Value(value) = v
        && let ValueDef::Argument(index) = q.function.value(value).def
        && let Some(attrs) = q.function.parameter_attrs.get(index as usize)
        && (Facts::of(attrs).no_undef() || has(attrs, "dereferenceable") || has(attrs, "dereferenceable_or_null"))
    {
        return true;
    }

    if let Operand::Constant(id) = v {
        let constant = q.context().get(id);
        match &constant.kind {
            ConstantKind::Poison => return !includes_poison(kind),
            ConstantKind::Undef => return !includes_undef(kind),
            ConstantKind::Int(_) | ConstantKind::Float(_) | ConstantKind::Null | ConstantKind::Global(_) => {
                return true;
            }
            ConstantKind::Zero
                if !matches!(
                    q.context().types.get(constant.ty),
                    Type::Array { .. } | Type::Struct { .. } | Type::Named(_)
                ) =>
            {
                return true;
            }
            _ => {}
        }
        if is_vector_constant(q, v) && !matches!(constant.kind, ConstantKind::Expr(_)) {
            if let ConstantKind::Aggregate(members) = &constant.kind {
                let members: Vec<_> = members.iter().map(|one| q.context().get(*one)).collect();
                if includes_undef(kind) && members.iter().any(|one| matches!(one.kind, ConstantKind::Undef)) {
                    return false;
                }
                if includes_poison(kind) && members.iter().any(|one| matches!(one.kind, ConstantKind::Poison)) {
                    return false;
                }
                return !members.iter().any(|one| matches!(one.kind, ConstantKind::Expr(_)));
            }
            return true;
        }
    }

    let stripped = strip_pointer_casts_same_representation(q, v);
    if q.instruction_of(stripped).is_some_and(|(_, one)| matches!(one.opcode, Opcode::Alloca { .. }))
        || matches!(
            stripped,
            Operand::Constant(id) if matches!(q.context().get(id).kind, ConstantKind::Global(_))
        )
        || is_null_pointer(q, stripped)
    {
        return true;
    }

    let op_check = |one: Operand| is_guaranteed_not_to_be_undef_or_poison_kind(q, one, depth + 1, kind);

    if let Some(operator) = Operator::of(q, v) {
        if let Operator::Instruction(inst) = operator {
            let instruction = q.function.instruction(inst);
            if instruction.opcode == Opcode::Freeze {
                return true;
            }
            if call_info(instruction).is_some()
                && (has_ret_attr(q, inst, "noundef")
                    || has_ret_attr(q, inst, "dereferenceable")
                    || has_ret_attr(q, inst, "dereferenceable_or_null"))
            {
                return true;
            }
        }

        if !can_create_undef_or_poison_kind(q, operator, kind, true) {
            let phi = match operator {
                Operator::Instruction(inst) if q.function.instruction(inst).opcode == Opcode::Phi => Some(inst),
                _ => None,
            };
            if let Some(phi) = phi {
                let instruction = q.function.instruction(phi);
                let mut well_defined = true;
                for pair in instruction.operands.chunks(2) {
                    if Some(pair[0]) == instruction.result.map(Operand::Value) {
                        continue;
                    }
                    let Operand::Block(block) = pair[1] else { continue };
                    let terminator = q.function.terminator(block);
                    if !is_guaranteed_not_to_be_undef_or_poison_kind(&q.with_cxt(terminator), pair[0], depth + 1, kind)
                    {
                        well_defined = false;
                        break;
                    }
                }
                if well_defined {
                    return true;
                }
            } else if operator.operands(q).into_iter().all(op_check) {
                return true;
            }
        }
    }

    if let Some((_, instruction)) = q.instruction_of(v)
        && matches!(instruction.opcode, Opcode::Load { .. })
        && instruction
            .metadata
            .iter()
            .any(|(name, _)| matches!(
                name.as_str(),
                "noundef" | "dereferenceable" | "dereferenceable_or_null"
            ))
    {
        return true;
    }

    if program_undefined_if_undef_or_poison_of(q, v, !includes_undef(kind)) {
        return true;
    }

    let Some(cxt) = q.cxt else { return false };
    let Some(block) = q.block_of(cxt) else { return false };
    let Some(dt) = q.dt else { return false };

    if !dt.is_reachable(block) {
        return false;
    }

    let mut dominator = dt.immediate_dominator(block);
    let is_integer = matches!(
        q.type_of(v).map(|ty| q.context().types.get(ty)),
        Some(Type::Int(_))
    );
    if !includes_undef(kind) || is_integer {
        while let Some(at) = dominator {
            let terminator = q.function.terminator(at).map(|one| q.function.instruction(one));
            let condition = terminator.and_then(|one| match one.opcode {
                Opcode::Br if one.operands.len() == 3 => Some(one.operands[0]),
                Opcode::Switch => one.operands.first().copied(),
                _ => None,
            });

            if let Some(condition) = condition {
                if condition == v {
                    return true;
                } else if !includes_undef(kind)
                    && let Some(operator) = Operator::of(q, condition)
                    && operator
                        .operands(q)
                        .iter()
                        .enumerate()
                        .any(|(at, one)| *one == v && propagates_poison(q, operator, at))
                {
                    return true;
                }
            }

            dominator = dt.immediate_dominator(at);
        }
    }

    false
}

/// `isGuaranteedNotToBeUndefOrPoison(V, AC, CtxI, DT, Depth)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8293>
pub fn is_guaranteed_not_to_be_undef_or_poison(
    q: &Query,
    v: Operand,
    depth: u32,
) -> bool {
    is_guaranteed_not_to_be_undef_or_poison_kind(q, v, depth, UndefPoisonKind::UndefOrPoison)
}

/// `isGuaranteedNotToBePoison(V, AC, CtxI, DT, Depth)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8301>
pub fn is_guaranteed_not_to_be_poison(
    q: &Query,
    v: Operand,
    depth: u32,
) -> bool {
    is_guaranteed_not_to_be_undef_or_poison_kind(q, v, depth, UndefPoisonKind::PoisonOnly)
}

/// `isGuaranteedNotToBeUndef(V, AC, CtxI, DT, Depth)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8308>
pub fn is_guaranteed_not_to_be_undef(
    q: &Query,
    v: Operand,
    depth: u32,
) -> bool {
    is_guaranteed_not_to_be_undef_or_poison_kind(q, v, depth, UndefPoisonKind::UndefOnly)
}

/// `mustExecuteUBIfPoisonOnPathTo(Root, OnPathTo, DT)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8322>
pub fn must_execute_ub_if_poison_on_path_to(
    q: &Query,
    root: InstId,
    on_path_to: InstId,
    dt: &crate::domtree::DomTree,
) -> bool {
    let mut known_poison = PoisonSet::default();
    let mut worklist = vec![root];
    while let Some(inst) = worklist.pop() {
        if must_trigger_ub(q, inst, &known_poison) && dt.instruction_dominates(q.function, inst, on_path_to) {
            return true;
        }

        let instruction = q.function.instruction(inst);
        if inst != root
            && !instruction
                .operands
                .iter()
                .enumerate()
                .any(|(at, op)| known_poison.contains(op) && propagates_poison(q, Operator::Instruction(inst), at))
        {
            continue;
        }

        if let Some(result) = instruction.result
            && known_poison.insert(Operand::Value(result))
        {
            worklist.extend(q.function.users(result).iter().map(|one| one.user));
        }
    }
    false
}

/// `isGuaranteedToTransferExecutionToSuccessor(I)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8372>
pub fn is_guaranteed_to_transfer_execution_to_successor(
    q: &Query,
    inst: InstId,
) -> bool {
    if matches!(
        q.function.instruction(inst).opcode,
        Opcode::Ret | Opcode::Unreachable
    ) {
        return false;
    }
    !may_throw(q, inst) && will_return(q, inst)
}

/// `isGuaranteedToTransferExecutionToSuccessor(BB)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8404>
pub fn is_guaranteed_to_transfer_execution_to_successor_block(
    q: &Query,
    block: BlockId,
) -> bool {
    q.function.iter_block(block).all(|one| is_guaranteed_to_transfer_execution_to_successor(q, one))
}

/// `isGuaranteedToTransferExecutionToSuccessor(Range, ScanLimit)`, and with
/// it the `(Begin, End, ScanLimit)` form: the range is an iterator, as
/// `Function::iter_range` and `iter_from` make one.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8420>
pub fn is_guaranteed_to_transfer_execution_to_successor_range(
    q: &Query,
    range: impl Iterator<Item = InstId>,
    scan_limit: u32,
) -> bool {
    let mut scan_limit = scan_limit;
    for inst in range {
        scan_limit -= 1;
        if scan_limit == 0 {
            return false;
        }
        if !is_guaranteed_to_transfer_execution_to_successor(q, inst) {
            return false;
        }
    }
    true
}

/// `isGuaranteedToExecuteForEveryIteration(I, L)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8432>
pub fn is_guaranteed_to_execute_for_every_iteration(
    q: &Query,
    inst: InstId,
    l: &Loop,
) -> bool {
    if q.block_of(inst) != Some(l.get_header()) {
        return false;
    }
    for one in q.function.iter_block(l.get_header()) {
        if one == inst {
            return true;
        }
        if !is_guaranteed_to_transfer_execution_to_successor(q, one) {
            return false;
        }
    }
    unreachable!("Instruction not contained in its own parent basic block.")
}

/// `intrinsicPropagatesPoison(IID)`, for the intrinsic `inst` calls.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8447>
pub fn intrinsic_propagates_poison(
    q: &Query,
    inst: InstId,
) -> bool {
    use crate::intrinsics::Intrinsic;
    if matches!(q.intrinsic(inst), Some(Intrinsic::WithOverflow { .. })) {
        return true;
    }
    matches!(
        pattern::intrinsic_id(q, inst),
        Some(
            IntrinsicId::SaddWithOverflow
                | IntrinsicId::SsubWithOverflow
                | IntrinsicId::SmulWithOverflow
                | IntrinsicId::UaddWithOverflow
                | IntrinsicId::UsubWithOverflow
                | IntrinsicId::UmulWithOverflow
                | IntrinsicId::Ctpop
                | IntrinsicId::Ctlz
                | IntrinsicId::Cttz
                | IntrinsicId::Abs
                | IntrinsicId::Smax
                | IntrinsicId::Smin
                | IntrinsicId::Umax
                | IntrinsicId::Umin
                | IntrinsicId::Scmp
                | IntrinsicId::IsFpclass
                | IntrinsicId::Ptrmask
                | IntrinsicId::Ucmp
                | IntrinsicId::Bitreverse
                | IntrinsicId::Bswap
                | IntrinsicId::SaddSat
                | IntrinsicId::SsubSat
                | IntrinsicId::SshlSat
                | IntrinsicId::UaddSat
                | IntrinsicId::UsubSat
                | IntrinsicId::UshlSat
                | IntrinsicId::SmulFix
                | IntrinsicId::SmulFixSat
                | IntrinsicId::UmulFix
                | IntrinsicId::UmulFixSat
                | IntrinsicId::Pow
                | IntrinsicId::Powi
                | IntrinsicId::Sin
                | IntrinsicId::Sinh
                | IntrinsicId::Cos
                | IntrinsicId::Cosh
                | IntrinsicId::Sincos
                | IntrinsicId::Sincospi
                | IntrinsicId::Tan
                | IntrinsicId::Tanh
                | IntrinsicId::Asin
                | IntrinsicId::Acos
                | IntrinsicId::Atan
                | IntrinsicId::Atan2
                | IntrinsicId::Canonicalize
                | IntrinsicId::Sqrt
                | IntrinsicId::Exp
                | IntrinsicId::Exp2
                | IntrinsicId::Exp10
                | IntrinsicId::Log
                | IntrinsicId::Log2
                | IntrinsicId::Log10
                | IntrinsicId::Modf
                | IntrinsicId::Floor
                | IntrinsicId::Ceil
                | IntrinsicId::Trunc
                | IntrinsicId::Rint
                | IntrinsicId::Nearbyint
                | IntrinsicId::Round
                | IntrinsicId::Roundeven
                | IntrinsicId::Lrint
                | IntrinsicId::Llrint
                | IntrinsicId::Fshl
                | IntrinsicId::Fshr
                | IntrinsicId::Frexp
                | IntrinsicId::GetActiveLaneMask
        )
    )
}

/// `propagatesPoison(PoisonOp)`: `operand_no` of `user` is the use.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8526>
pub fn propagates_poison(
    q: &Query,
    user: Operator,
    operand_no: usize,
) -> bool {
    let inst = match user {
        Operator::Expr(_) => return true,
        Operator::Instruction(inst) => inst,
    };
    match &q.function.instruction(inst).opcode {
        Opcode::Freeze | Opcode::Phi | Opcode::Invoke(_) => false,
        Opcode::Select => operand_no == 0,
        Opcode::Call(_) => intrinsic_propagates_poison(q, inst),
        Opcode::ICmp(_) | Opcode::FCmp(_) | Opcode::GetElementPtr { .. } => true,
        Opcode::Binary(_) | Opcode::FNeg | Opcode::Cast(_) => true,
        _ => false,
    }
}

/// `handleGuaranteedWellDefinedOps(I, Handle)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8556>
fn handle_guaranteed_well_defined_ops(
    q: &Query,
    inst: InstId,
    handle: &mut dyn FnMut(Operand) -> bool,
) -> bool {
    let instruction = q.function.instruction(inst);
    match &instruction.opcode {
        Opcode::Store { .. } => {
            if handle(instruction.operands[1]) {
                return true;
            }
        }
        Opcode::Load { .. } => {
            if handle(instruction.operands[0]) {
                return true;
            }
        }
        Opcode::Call(_) | Opcode::Invoke(_) => {
            if is_indirect_call(instruction) && handle(*instruction.operands.last().expect("a callee")) {
                return true;
            }
            for at in 0..arg_size(instruction) {
                if (param_has_attr(q, inst, at, "noundef")
                    || param_has_attr(q, inst, at, "dereferenceable")
                    || param_has_attr(q, inst, at, "dereferenceable_or_null"))
                    && handle(instruction.operands[at])
                {
                    return true;
                }
            }
        }
        Opcode::Ret => {
            if Facts::of(&q.function.return_attrs).no_undef()
                && let Some(value) = instruction.operands.first()
                && handle(*value)
            {
                return true;
            }
        }
        Opcode::Switch => {
            if handle(instruction.operands[0]) {
                return true;
            }
        }
        Opcode::Br if instruction.operands.len() == 3 => {
            if handle(instruction.operands[0]) {
                return true;
            }
        }
        _ => {}
    }
    false
}

/// `handleGuaranteedNonPoisonOps(I, Handle)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8616>
fn handle_guaranteed_non_poison_ops(
    q: &Query,
    inst: InstId,
    handle: &mut dyn FnMut(Operand) -> bool,
) -> bool {
    if handle_guaranteed_well_defined_ops(q, inst, handle) {
        return true;
    }
    let instruction = q.function.instruction(inst);
    match instruction.opcode {
        Opcode::Binary(BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem) => {
            handle(instruction.operands[1])
        }
        _ => false,
    }
}

/// `mustTriggerUB(I, KnownPoison)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8632>
pub fn must_trigger_ub(
    q: &Query,
    inst: InstId,
    known_poison: &PoisonSet,
) -> bool {
    handle_guaranteed_non_poison_ops(q, inst, &mut |v| known_poison.contains(&v))
}

/// `programUndefinedIfUndefOrPoison(V, PoisonOnly)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8638>
fn program_undefined_if_undef_or_poison_of(
    q: &Query,
    v: Operand,
    poison_only: bool,
) -> bool {
    // Where the walk of a block starts: the instruction after `v`'s, or none.
    let (mut block, mut begin) = if let Some((inst, _)) = q.instruction_of(v) {
        let Some(block) = q.block_of(inst) else { return false };
        (block, q.function.next_in_block(inst))
    } else if let Operand::Value(value) = v
        && let ValueDef::Argument(_) = q.function.value(value).def
    {
        if q.function.is_declaration() {
            return false;
        }
        let Some(entry) = q.function.entry() else { return false };
        (entry, q.function.block(entry).first())
    } else {
        return false;
    };

    let mut scan_limit = 32_u32;

    if !poison_only {
        for inst in begin.into_iter().flat_map(|first| q.function.iter_from(first)) {
            scan_limit -= 1;
            if scan_limit == 0 {
                break;
            }
            if handle_guaranteed_well_defined_ops(q, inst, &mut |well_defined| well_defined == v) {
                return true;
            }
            if !is_guaranteed_to_transfer_execution_to_successor(q, inst) {
                break;
            }
        }
        return false;
    }

    let mut yields_poison = PoisonSet::default();
    let mut visited: SmallSet<BlockId, 4> = SmallSet::default();
    yields_poison.insert(v);
    visited.insert(block);

    loop {
        for inst in begin.into_iter().flat_map(|first| q.function.iter_from(first)) {
            scan_limit -= 1;
            if scan_limit == 0 {
                return false;
            }
            if must_trigger_ub(q, inst, &yields_poison) {
                return true;
            }
            if !is_guaranteed_to_transfer_execution_to_successor(q, inst) {
                return false;
            }

            let instruction = q.function.instruction(inst);
            for (at, op) in instruction.operands.iter().enumerate() {
                if yields_poison.contains(op) && propagates_poison(q, Operator::Instruction(inst), at) {
                    if let Some(result) = instruction.result {
                        yields_poison.insert(Operand::Value(result));
                    }
                    break;
                }
            }

            if instruction.opcode == Opcode::Select
                && yields_poison.contains(&instruction.operands[1])
                && yields_poison.contains(&instruction.operands[2])
                && let Some(result) = instruction.result
            {
                yields_poison.insert(Operand::Value(result));
            }
        }

        let Some(terminator) = q.function.terminator(block) else { break };
        let mut edges = q
            .function
            .instruction(terminator)
            .operands
            .iter()
            .filter_map(
                |op| match op {
                    Operand::Block(target) => Some(*target),
                    _ => None,
                },
            );
        let (Some(next), None) = (edges.next(), edges.next()) else { break };
        if !visited.insert(next) {
            break;
        }
        block = next;
        begin = q.function.iter_block(block).find(|&one| q.function.instruction(one).opcode != Opcode::Phi);
    }
    false
}

/// `programUndefinedIfUndefOrPoison(Inst)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8732>
pub fn program_undefined_if_undef_or_poison(
    q: &Query,
    inst: Operand,
) -> bool {
    program_undefined_if_undef_or_poison_of(q, inst, false)
}

/// `programUndefinedIfPoison(Inst)`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8736>
pub fn program_undefined_if_poison(
    q: &Query,
    inst: Operand,
) -> bool {
    program_undefined_if_undef_or_poison_of(q, inst, true)
}

/// The `gp` column of `examples/vtdump.rs`: `isGuaranteedNotToBePoison` then
/// `isGuaranteedNotToBeUndef`, each 0 or 1.
pub fn column(
    q: &Query,
    v: Operand,
) -> String {
    format!(
        " gp={}{}",
        u8::from(is_guaranteed_not_to_be_poison(q, v, 0)),
        u8::from(is_guaranteed_not_to_be_undef(q, v, 0))
    )
}

/// `Instruction::mayReadFromMemory()`. MIR has no atomics, fences or `va_arg`:
/// a load reads, and a call reads unless it or its callee only writes memory
/// (`CallBase::onlyWritesMemory`).
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instruction.cpp#L1137>
pub fn may_read_from_memory(
    q: &Query,
    inst: InstId,
) -> bool {
    let instruction = q.function.instruction(inst);
    match &instruction.opcode {
        Opcode::Load { .. } => true,
        Opcode::Call(info) | Opcode::Invoke(info) => {
            let only_writes = |attrs: &[Attribute]| !crate::memory::stated(attrs).reads;
            !(only_writes(&info.attrs)
                || called_function(q, instruction).is_some_and(|callee| only_writes(&callee.attrs)))
        }
        _ => false,
    }
}

/// `Instruction::mayWriteToMemory()`. MIR has no atomics, fences or `va_arg`:
/// a load writes nothing (it is always unordered), and a call writes unless it
/// or its callee only reads memory (`CallBase::onlyReadsMemory`).
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instruction.cpp#L1157>
pub fn may_write_to_memory(
    q: &Query,
    inst: InstId,
) -> bool {
    let instruction = q.function.instruction(inst);
    match &instruction.opcode {
        Opcode::Store { .. } => true,
        Opcode::Call(info) | Opcode::Invoke(info) => {
            let only_reads = |attrs: &[Attribute]| !crate::memory::stated(attrs).writes;
            !(only_reads(&info.attrs)
                || called_function(q, instruction).is_some_and(|callee| only_reads(&callee.attrs)))
        }
        _ => false,
    }
}
