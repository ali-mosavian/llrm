//! `IVDescriptors`: the recurrences (reductions) and induction variables of a
//! loop's header phis, `IVDescriptors.cpp` at `d1106deb71cc`.
//!
//! # How it works
//! [`is_reduction_phi`] tries each recurrence kind in LLVM's order (`Add`,
//! `Sub`, `AddChainWithSubs`, `Mul`, `Or`, `And`, `Xor`, the min/max
//! intrinsics, `AnyOf`, `FindLast`, `FMul`, `FSub`, `FAdd`,
//! `FAddChainWithSubs`, `FMulAdd`) and answers the first
//! [`RecurrenceDescriptor`] that fits: the walk from the phi through its users
//! must come back to it, every value in the cycle used once by the cycle (apart
//! from phis and conditional patterns), at most one value used outside the
//! loop, and each instruction a recurrence instruction of the kind
//! (`is_recurrence_instr`). [`is_induction_phi`] asks SCEV for an affine
//! recurrence of the loop whose start is the preheader's value and gives its
//! step; a floating-point phi is an induction by its `fadd`/`fsub` of an
//! invariant.
//!
//! # Deviations from LLVM
//! - D1: `DemandedBits`, `AssumptionCache` and `DominatorTree` are not
//!   parameters: with none of them `computeRecurrenceType` is the type's width
//!   rounded up to a power of two, and that is all this port computes. The only
//!   caller (`LoopInterchange`) passes none of them.
//! - D2: `PredicatedScalarEvolution`: [`is_induction_phi`] takes the
//!   `ScalarEvolution` overload only; the phi's SCEV is not forced into an add
//!   recurrence under predicates.
//! - D4: an FP induction's step is its addend (`InductionStep::Fp`); SCEV has
//!   no floating-point type to hold `getUnknown(Addend)`.
//! - D3: `isFixedOrderRecurrence` and the conditional induction are not
//!   translated: nothing here asks for them.
//!
//! # Example
//! `%s = phi [0, %pre], [%s.next, %latch]` with `%s.next = add %s, %x` and one
//! use of `%s.next` after the loop is an `Add` reduction starting at 0;
//! `%i = phi [0, %pre], [%i.next, %latch]` with `%i.next = add nuw %i, 1` is an
//! integer induction of step 1.

use llrm_mir::loopinfo::Loop;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, Flags, FloatPredicate, IntPredicate, Opcode};
use llrm_mir::types::{Type as MirType, TypeId};
use llrm_mir::valuetracking::pattern::{IntrinsicId, call_intrinsic};

use crate::scev::expr::{Kind, Scev};
use crate::scev::{Cx, ScalarEvolution};

/// `RecurKind`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecurKind {
    None,
    Add,
    Sub,
    AddChainWithSubs,
    Mul,
    Or,
    And,
    Xor,
    SMin,
    SMax,
    UMin,
    UMax,
    FAdd,
    FAddChainWithSubs,
    FSub,
    FMul,
    FMin,
    FMax,
    FMinNum,
    FMaxNum,
    FMinimum,
    FMaximum,
    FMinimumNum,
    FMaximumNum,
    FMulAdd,
    AnyOf,
    FindIV,
    FindLast,
}

impl RecurKind {
    /// `isIntegerRecurrenceKind`.
    pub fn is_integer(self) -> bool {
        matches!(
            self,
            Self::AddChainWithSubs
                | Self::Sub
                | Self::Add
                | Self::Mul
                | Self::Or
                | Self::And
                | Self::Xor
                | Self::SMax
                | Self::SMin
                | Self::UMax
                | Self::UMin
                | Self::AnyOf
                | Self::FindIV
                | Self::FindLast
        )
    }

    /// `isFloatingPointRecurrenceKind`.
    pub fn is_floating_point(self) -> bool {
        self != Self::None && !self.is_integer()
    }

    /// `isSubRecurrenceKind`.
    pub fn is_sub(self) -> bool {
        matches!(self, Self::Sub | Self::FSub)
    }

    /// `isIntMinMaxRecurrenceKind`.
    pub fn is_int_min_max(self) -> bool {
        matches!(self, Self::UMin | Self::UMax | Self::SMin | Self::SMax)
    }

    /// `isFPMinMaxNumRecurrenceKind`.
    pub fn is_fp_min_max_num(self) -> bool {
        matches!(self, Self::FMinNum | Self::FMaxNum)
    }

    /// `isFPMinMaxRecurrenceKind`.
    pub fn is_fp_min_max(self) -> bool {
        matches!(
            self,
            Self::FMin
                | Self::FMax
                | Self::FMinimum
                | Self::FMaximum
                | Self::FMinimumNum
                | Self::FMaximumNum
                | Self::FMinNum
                | Self::FMaxNum
        )
    }

    /// `isMinMaxRecurrenceKind`.
    pub fn is_min_max(self) -> bool {
        self.is_int_min_max() || self.is_fp_min_max()
    }

    /// `isFindRecurrenceKind`.
    pub fn is_find(self) -> bool {
        matches!(self, Self::FindLast | Self::FindIV)
    }

    /// `RecurrenceDescriptor::getOpcode`: the opcode of the operation (a
    /// compare for min/max).
    pub fn opcode(self) -> Opcode {
        match self {
            Self::Sub => Opcode::Binary(BinaryOp::Sub),
            Self::AddChainWithSubs | Self::Add => Opcode::Binary(BinaryOp::Add),
            Self::Mul => Opcode::Binary(BinaryOp::Mul),
            Self::Or => Opcode::Binary(BinaryOp::Or),
            Self::And => Opcode::Binary(BinaryOp::And),
            Self::Xor => Opcode::Binary(BinaryOp::Xor),
            Self::FMul => Opcode::Binary(BinaryOp::FMul),
            Self::FMulAdd | Self::FAddChainWithSubs | Self::FAdd => Opcode::Binary(BinaryOp::FAdd),
            Self::FSub => Opcode::Binary(BinaryOp::FSub),
            Self::SMax | Self::SMin | Self::UMax | Self::UMin => Opcode::ICmp(IntPredicate::Eq),
            Self::FMax | Self::FMin | Self::FMaximum | Self::FMinimum | Self::FMaximumNum | Self::FMinimumNum => {
                Opcode::FCmp(FloatPredicate::Oeq)
            }
            _ => unreachable!("Unknown recurrence operation"),
        }
    }
}

/// What a reduction phi is: `RecurrenceDescriptor`.
#[derive(Clone, Debug)]
pub struct RecurrenceDescriptor {
    /// Reductions may store temporary or final result to an invariant address:
    /// the last such store met.
    pub intermediate_store: Option<InstId>,
    /// The starting value of the recurrence; it does not have to be zero.
    pub start_value: Operand,
    /// The instruction whose value is used outside the loop.
    pub loop_exit_instr: Option<InstId>,
    pub kind: RecurKind,
    /// The fast-math flags common to the reduction's operations.
    pub fmf: Flags,
    /// The first instruction in the use-def chain that requires exact FP math.
    pub exact_fp_math_inst: Option<InstId>,
    /// The type of the recurrence; narrower than the phi's if it was
    /// type-promoted.
    pub recurrence_type: TypeId,
    pub is_signed: bool,
    pub is_ordered: bool,
    pub phi_has_uses_outside_reduction_chain: bool,
    /// The casts of a type-promoted recurrence.
    pub cast_insts: Vec<InstId>,
    pub min_width_cast_to_recurrence_type: u32,
}

impl RecurrenceDescriptor {
    /// `hasUsesOutsideReductionChain`.
    pub fn has_uses_outside_reduction_chain(&self) -> bool {
        self.phi_has_uses_outside_reduction_chain
    }

    /// `getExactFPMathInst`.
    pub fn exact_fp_math_inst(&self) -> Option<InstId> {
        self.exact_fp_math_inst
    }

    /// `getRecurrenceKind`.
    pub fn recurrence_kind(&self) -> RecurKind {
        self.kind
    }
}

/// `InstDesc`: what an instruction is to a recurrence being built.
#[derive(Clone, Copy, Debug)]
struct InstDesc {
    is_recurrence: bool,
    pattern_last_inst: Option<InstId>,
    rec_kind: RecurKind,
    exact_fp_math_inst: Option<InstId>,
}

impl InstDesc {
    /// `InstDesc(bool IsRecur, Instruction *I, Instruction *ExactFP)`.
    fn plain(
        is_recurrence: bool,
        inst: Option<InstId>,
        exact: Option<InstId>,
    ) -> Self {
        Self { is_recurrence, pattern_last_inst: inst, rec_kind: RecurKind::None, exact_fp_math_inst: exact }
    }

    /// `InstDesc(Instruction *I, RecurKind K, Instruction *ExactFP)`.
    fn of_kind(
        inst: InstId,
        kind: RecurKind,
        exact: Option<InstId>,
    ) -> Self {
        Self { is_recurrence: true, pattern_last_inst: Some(inst), rec_kind: kind, exact_fp_math_inst: exact }
    }
}

/// The function and loop an analysis runs over, with `m_*` helpers.
struct Ctx<'a, 'b> {
    cx: &'a Cx<'b>,
    the_loop: Loop<'a>,
}

impl<'a, 'b> Ctx<'a, 'b> {
    fn function(&self) -> &'b Function {
        self.cx.function
    }

    fn instruction(
        &self,
        inst: InstId,
    ) -> &'b llrm_mir::module::Instruction {
        self.cx.function.instruction(inst)
    }

    /// `dyn_cast<Instruction>(V)`.
    fn inst_of(
        &self,
        operand: Operand,
    ) -> Option<InstId> {
        let Operand::Value(value) = operand else { return None };
        match self.function().value(value).def {
            ValueDef::Instruction(inst) => Some(inst),
            ValueDef::Argument(_) => None,
        }
    }

    fn is_phi(
        &self,
        inst: InstId,
    ) -> bool {
        self.instruction(inst).opcode == Opcode::Phi
    }

    fn is_select(
        &self,
        inst: InstId,
    ) -> bool {
        self.instruction(inst).opcode == Opcode::Select
    }

    fn is_cmp(
        &self,
        inst: InstId,
    ) -> bool {
        matches!(
            self.instruction(inst).opcode,
            Opcode::ICmp(_) | Opcode::FCmp(_)
        )
    }

    fn is_fcmp(
        &self,
        inst: InstId,
    ) -> bool {
        matches!(self.instruction(inst).opcode, Opcode::FCmp(_))
    }

    /// `Instruction::isCommutative`.
    fn is_commutative(
        &self,
        inst: InstId,
    ) -> bool {
        match self.instruction(inst).opcode {
            Opcode::Binary(op) => matches!(
                op,
                BinaryOp::Add
                    | BinaryOp::Mul
                    | BinaryOp::And
                    | BinaryOp::Or
                    | BinaryOp::Xor
                    | BinaryOp::FAdd
                    | BinaryOp::FMul
            ),
            Opcode::ICmp(pred) => matches!(pred, IntPredicate::Eq | IntPredicate::Ne),
            Opcode::FCmp(pred) => matches!(
                pred,
                FloatPredicate::Oeq
                    | FloatPredicate::One
                    | FloatPredicate::Ord
                    | FloatPredicate::Uno
                    | FloatPredicate::Ueq
                    | FloatPredicate::Une
            ),
            _ => false,
        }
    }

    /// The instructions using `inst`'s value.
    fn users(
        &self,
        inst: InstId,
    ) -> Vec<InstId> {
        let Some(result) = self.instruction(inst).result else { return Vec::new() };
        self.function().users(result).iter().map(|one| one.user).collect()
    }

    /// `hasNUses` and its kin count the uses (an instruction using the value
    /// twice is two).
    fn use_count(
        &self,
        inst: InstId,
    ) -> usize {
        let Some(result) = self.instruction(inst).result else { return 0 };
        self.function().users(result).iter().count()
    }

    fn parent(
        &self,
        inst: InstId,
    ) -> BlockId {
        self.function().parent(inst).expect("a placed instruction")
    }

    /// `PHINode::getIncomingValueForBlock`.
    fn incoming_for(
        &self,
        phi: InstId,
        block: BlockId,
    ) -> Option<Operand> {
        self.instruction(phi).operands.chunks(2).find(|pair| pair[1] == Operand::Block(block)).map(|pair| pair[0])
    }

    /// `FPMathOperator`: an instruction with fast-math flags.
    fn is_fp_math_operator(
        &self,
        inst: InstId,
    ) -> bool {
        let instruction = self.instruction(inst);
        let float = |ty: TypeId| matches!(self.cx.context.types.get(ty), MirType::Float(_));
        match instruction.opcode {
            Opcode::Binary(op) => {
                matches!(
                    op,
                    BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul | BinaryOp::FDiv | BinaryOp::FRem
                )
            }
            Opcode::FCmp(_) | Opcode::Select | Opcode::Phi | Opcode::Call(_) => float(instruction.ty),
            _ => false,
        }
    }

    /// `isFMulAddIntrinsic`.
    fn is_fmuladd(
        &self,
        inst: InstId,
    ) -> bool {
        call_intrinsic(&self.cx.query(None), inst) == Some(IntrinsicId::Fmuladd)
    }

    /// `m_Intrinsic<ID>(A, B)`: the two arguments.
    fn intrinsic2(
        &self,
        value: Operand,
        id: IntrinsicId,
    ) -> Option<(Operand, Operand)> {
        let inst = self.inst_of(value)?;
        if call_intrinsic(&self.cx.query(None), inst) != Some(id) {
            return None;
        }
        let operands = &self.instruction(inst).operands;
        (operands.len() == 3).then(|| (operands[0], operands[1]))
    }

    /// `m_OrdOrUnordFMin(A, B)` / `m_OrdOrUnordFMax(A, B)`: `select (fcmp A,
    /// B), A, B` (or the same with the select's arms and the predicate
    /// inverted) with an ordered or unordered less (or greater) predicate;
    /// `min` picks which.
    fn fp_select_min_max(
        &self,
        value: Operand,
        min: bool,
    ) -> Option<(Operand, Operand)> {
        let select = self.inst_of(value)?;
        let instruction = self.instruction(select);
        if instruction.opcode != Opcode::Select {
            return None;
        }
        let cmp = self.inst_of(instruction.operands[0])?;
        let Opcode::FCmp(pred) = self.instruction(cmp).opcode else { return None };
        let (true_val, false_val) = (instruction.operands[1], instruction.operands[2]);
        let (lhs, rhs) = (self.instruction(cmp).operands[0], self.instruction(cmp).operands[1]);
        if (true_val != lhs || false_val != rhs) && (true_val != rhs || false_val != lhs) {
            return None;
        }
        let pred = if lhs == true_val { pred } else { pred.inverse() };
        let wanted = if min {
            matches!(
                pred,
                FloatPredicate::Olt | FloatPredicate::Ole | FloatPredicate::Ult | FloatPredicate::Ule
            )
        } else {
            matches!(
                pred,
                FloatPredicate::Ogt | FloatPredicate::Oge | FloatPredicate::Ugt | FloatPredicate::Uge
            )
        };
        wanted.then_some((lhs, rhs))
    }

    /// `getMinMaxRK`'s `GetMinMaxRK(V, A, B)`.
    fn min_max_kind(
        &self,
        value: Operand,
    ) -> (RecurKind, Option<(Operand, Operand)>) {
        let table = [
            (IntrinsicId::Umin, RecurKind::UMin),
            (IntrinsicId::Umax, RecurKind::UMax),
            (IntrinsicId::Smax, RecurKind::SMax),
            (IntrinsicId::Smin, RecurKind::SMin),
        ];
        for (id, kind) in table {
            if let Some(args) = self.intrinsic2(value, id) {
                return (kind, Some(args));
            }
        }
        if let Some(args) = self.fp_select_min_max(value, true).or_else(|| self.intrinsic2(value, IntrinsicId::Minnum))
        {
            return (RecurKind::FMin, Some(args));
        }
        if let Some(args) = self.fp_select_min_max(value, false).or_else(|| self.intrinsic2(value, IntrinsicId::Maxnum))
        {
            return (RecurKind::FMax, Some(args));
        }
        let table = [
            (IntrinsicId::Minimum, RecurKind::FMinimum),
            (IntrinsicId::Maximum, RecurKind::FMaximum),
            (IntrinsicId::Minimumnum, RecurKind::FMinimumNum),
            (IntrinsicId::Maximumnum, RecurKind::FMaximumNum),
        ];
        for (id, kind) in table {
            if let Some(args) = self.intrinsic2(value, id) {
                return (kind, Some(args));
            }
        }
        (RecurKind::None, None)
    }

    fn scalar_is_float(
        &self,
        ty: TypeId,
    ) -> bool {
        matches!(self.cx.context.types.get(ty), MirType::Float(_))
    }

    fn scalar_is_int(
        &self,
        ty: TypeId,
    ) -> bool {
        matches!(self.cx.context.types.get(ty), MirType::Int(_))
    }

    fn scalar_is_pointer(
        &self,
        ty: TypeId,
    ) -> bool {
        matches!(self.cx.context.types.get(ty), MirType::Pointer(_))
    }

    fn contains(
        &self,
        inst: InstId,
    ) -> bool {
        self.the_loop.contains_instruction(self.function(), inst)
    }
}

/// All the fast-math flags LLVM intersects over a reduction.
fn all_fmf() -> Flags {
    [Flags::REASSOC, Flags::NNAN, Flags::NINF, Flags::NSZ, Flags::ARCP, Flags::CONTRACT]
        .into_iter()
        .fold(Flags::default(), Flags::union)
}

/// `collectMinMaxFMF`: the flags of `inst`, and its fcmp's if it is a select.
fn collect_min_max_fmf(
    c: &Ctx,
    inst: InstId,
) -> Flags {
    let instruction = c.instruction(inst);
    let mut flags = fast_math_of(instruction.flags);
    if instruction.opcode == Opcode::Select
        && let Some(cmp) = c.inst_of(instruction.operands[0])
        && c.is_fcmp(cmp)
    {
        flags = union(flags, fast_math_of(c.instruction(cmp).flags));
    }
    flags
}

/// The fast-math part of `flags`.
fn fast_math_of(flags: Flags) -> Flags {
    let mut out = Flags::default();
    for one in [Flags::REASSOC, Flags::NNAN, Flags::NINF, Flags::NSZ, Flags::ARCP, Flags::CONTRACT] {
        if flags.contains(one) {
            out = union(out, one);
        }
    }
    out
}

fn union(
    a: Flags,
    b: Flags,
) -> Flags {
    a.union(b)
}

fn intersection(
    a: Flags,
    b: Flags,
) -> Flags {
    a.intersect(b)
}

/// `hasRequiredFastMathFlags`: the flags of `inst`, if it has `nnan` and `nsz`
/// or is of a kind that needs neither; the kind is updated for `minnum` and
/// `maxnum`.
fn has_required_fast_math_flags(
    c: &Ctx,
    inst: InstId,
    kind: &mut RecurKind,
) -> Option<Flags> {
    let flags = c.instruction(inst).flags;
    let required = c.is_fp_math_operator(inst) && flags.contains(Flags::NNAN) && flags.contains(Flags::NSZ);
    if required {
        return Some(collect_min_max_fmf(c, inst));
    }
    match *kind {
        RecurKind::FMinimum | RecurKind::FMaximum | RecurKind::FMinimumNum | RecurKind::FMaximumNum => {}
        RecurKind::FMax => {
            c.intrinsic2(Operand::Value(c.instruction(inst).result?), IntrinsicId::Maxnum)?;
            *kind = RecurKind::FMaxNum;
        }
        RecurKind::FMin => {
            c.intrinsic2(Operand::Value(c.instruction(inst).result?), IntrinsicId::Minnum)?;
            *kind = RecurKind::FMinNum;
        }
        _ => return None,
    }
    Some(collect_min_max_fmf(c, inst))
}

/// `lookThroughAnd`: if `phi` has one user, an `and` with a type mask, the
/// narrower type, the `and` as the starting instruction; else `phi` itself.
fn look_through_and(
    c: &Ctx,
    phi: InstId,
    recurrence_type: &mut TypeId,
    visited: &mut Vec<InstId>,
    cast_insts: &mut Vec<InstId>,
) -> InstId {
    if c.use_count(phi) != 1 {
        return phi;
    }
    let j = c.users(phi)[0];
    let jn = c.instruction(j);
    if jn.opcode != Opcode::Binary(BinaryOp::And) {
        return phi;
    }
    // m_And(m_Instruction(I), m_APInt(M)), commutative.
    let query = c.cx.query(None);
    let (mask, other) = match (query.int_constant(jn.operands[0]), query.int_constant(jn.operands[1])) {
        (_, Some(mask)) => (mask, jn.operands[0]),
        (Some(mask), None) => (mask, jn.operands[1]),
        _ => return phi,
    };
    if c.inst_of(other).is_none() {
        return phi;
    }
    let (value, width) = mask;
    // (M + 1).exactLogBase2() > 0
    let next = if width >= 128 { 0 } else { (value.wrapping_add(1)) & ((1u128 << width) - 1) };
    if next != 0 && next.is_power_of_two() {
        let bits = next.trailing_zeros();
        if bits > 0 {
            *recurrence_type = c.cx.context.types.int(bits);
            visited.push(phi);
            cast_insts.push(j);
            return j;
        }
    }
    phi
}

/// `areAllUsesIn`: every operand of `inst` is an instruction in `set`.
fn are_all_uses_in(
    c: &Ctx,
    inst: InstId,
    set: &[InstId],
) -> bool {
    c.instruction(inst).operands.iter().all(|one| c.inst_of(*one).is_some_and(|def| set.contains(&def)))
}

/// `hasMultipleUsesOf`: whether more than `max` operands of `inst` are in
/// `insts`.
fn has_multiple_uses_of(
    c: &Ctx,
    inst: InstId,
    insts: &[InstId],
    max: usize,
) -> bool {
    let mut uses = 0;
    for operand in &c.instruction(inst).operands {
        if c.inst_of(*operand).is_some_and(|def| insts.contains(&def)) {
            uses += 1;
        }
        if uses > max {
            return true;
        }
    }
    false
}

/// `checkOrderedReduction`.
fn check_ordered_reduction(
    c: &Ctx,
    kind: RecurKind,
    exact_fp_math_inst: Option<InstId>,
    exit: InstId,
    phi: InstId,
) -> bool {
    // Currently only FAdd and FMulAdd are supported.
    if kind != RecurKind::FAdd && kind != RecurKind::FMulAdd {
        return false;
    }
    if kind == RecurKind::FAdd && c.instruction(exit).opcode != Opcode::Binary(BinaryOp::FAdd) {
        return false;
    }
    if kind == RecurKind::FMulAdd && !c.is_fmuladd(exit) {
        return false;
    }
    // Ensure the exit instruction has only one user other than the reduction
    // PHI.
    if Some(exit) != exact_fp_math_inst || c.use_count(exit) >= 3 {
        return false;
    }
    // The only pattern accepted is the one in which the reduction PHI is used
    // as one of the operands of the exit instruction.
    let operands = &c.instruction(exit).operands;
    let phi_value = Operand::Value(c.instruction(phi).result.expect("a phi"));
    if kind == RecurKind::FAdd && operands[0] != phi_value && operands[1] != phi_value {
        return false;
    }
    if kind == RecurKind::FMulAdd && operands[2] != phi_value {
        return false;
    }
    true
}

/// `collectCastInstrs`: the casts from the recurrence type among `exit`'s
/// loop-varying operands, and the narrowest cast to it.
fn collect_cast_instrs(
    c: &Ctx,
    exit: InstId,
    recurrence_type: TypeId,
    casts: &mut Vec<InstId>,
) -> u32 {
    let mut worklist = vec![exit];
    let mut visited: Vec<InstId> = Vec::new();
    let mut min_width = u32::MAX;
    while let Some(value) = worklist.pop() {
        visited.push(value);
        if let Opcode::Cast(_) = c.instruction(value).opcode {
            let source = c.cx.function.operand_type(c.cx.context, c.instruction(value).operands[0]);
            if source == Some(recurrence_type) {
                // If the source type of a cast instruction is equal to the
                // recurrence type, it will be eliminated.
                if !casts.contains(&value) {
                    casts.push(value);
                }
                continue;
            }
            if c.instruction(value).ty == recurrence_type {
                if let Some(source) = source {
                    min_width = min_width.min(c.cx.layout.size_bits(&c.cx.context.types, source) as u32);
                }
                continue;
            }
        }
        // Add all operands to the work list if they are loop-varying values
        // that we haven't yet visited.
        for operand in &c.instruction(value).operands {
            if let Some(def) = c.inst_of(*operand)
                && c.contains(def)
                && !visited.contains(&def)
            {
                worklist.push(def);
            }
        }
    }
    min_width
}

/// `isFindLastLikePhi`: a phi that selects between the original value
/// (`header_phi`) and an arbitrary non-reduction value.
fn is_find_last_like_phi(
    c: &Ctx,
    phi: InstId,
    header_phi: InstId,
    reduction_instrs: &[InstId],
) -> bool {
    let mut non_redux_inputs = 0;
    let header_value = Operand::Value(c.instruction(header_phi).result.expect("a phi"));
    for operand in &c.instruction(phi).operands {
        if matches!(operand, Operand::Block(_)) {
            continue;
        }
        let in_reduction = c.inst_of(*operand).is_some_and(|def| reduction_instrs.contains(&def));
        if !in_reduction {
            non_redux_inputs += 1;
            if non_redux_inputs > 1 {
                return false;
            }
        } else if *operand != header_value {
            // TODO: Remove this restriction once chained phis are supported.
            return false;
        }
    }
    non_redux_inputs == 1
}

/// `isAnyOfPattern`.
fn is_any_of_pattern(
    c: &Ctx,
    orig_phi: InstId,
    inst: InstId,
    prev: &InstDesc,
) -> InstDesc {
    // We must handle the select(cmp(),x,y) as a single instruction. Advance to
    // the select.
    if c.is_cmp(inst) && c.use_count(inst) == 1 {
        let user = c.users(inst)[0];
        if c.is_select(user) {
            return InstDesc::of_kind(user, prev.rec_kind, None);
        }
    }
    let instruction = c.instruction(inst);
    if instruction.opcode != Opcode::Select || !c.inst_of(instruction.operands[0]).is_some_and(|def| c.is_cmp(def)) {
        return InstDesc::plain(false, Some(inst), None);
    }
    let phi_value = Operand::Value(c.instruction(orig_phi).result.expect("a phi"));
    let (true_val, false_val) = (instruction.operands[1], instruction.operands[2]);
    let non_phi = if true_val == phi_value {
        false_val
    } else if false_val == phi_value {
        true_val
    } else {
        return InstDesc::plain(false, Some(inst), None);
    };
    // We are looking for selects of the form:
    //   select(cmp(), phi, loop_invariant) or
    //   select(cmp(), loop_invariant, phi)
    if !c.the_loop.is_loop_invariant(c.function(), non_phi) {
        return InstDesc::plain(false, Some(inst), None);
    }
    InstDesc::of_kind(inst, RecurKind::AnyOf, None)
}

/// `isFindPattern`: `select(cmp(), phi, value)` or `select(cmp(), value, phi)`
/// when the phi has one use.
fn is_find_pattern(
    c: &Ctx,
    orig_phi: InstId,
    inst: InstId,
) -> InstDesc {
    if c.use_count(orig_phi) != 1 {
        return InstDesc::plain(false, Some(inst), None);
    }
    let instruction = c.instruction(inst);
    if instruction.opcode != Opcode::Select || !c.inst_of(instruction.operands[0]).is_some_and(|def| c.is_cmp(def)) {
        return InstDesc::plain(false, Some(inst), None);
    }
    let phi_value = Operand::Value(c.instruction(orig_phi).result.expect("a phi"));
    if instruction.operands[2] != phi_value && instruction.operands[1] != phi_value {
        return InstDesc::plain(false, Some(inst), None);
    }
    InstDesc::of_kind(inst, RecurKind::FindLast, None)
}

/// `isConditionalRdxPattern`: a select of a binary operation on the phi and
/// the phi, under a compare used once:
///
/// ```text
/// %cmp = fcmp pred %0, %CFP
/// %add = fadd %0, %sum.1
/// %sum.2 = select %cmp, %add, %sum.1
/// ```
fn is_conditional_rdx_pattern(
    c: &Ctx,
    inst: InstId,
) -> InstDesc {
    let no = InstDesc::plain(false, Some(inst), None);
    let instruction = c.instruction(inst);
    if instruction.opcode != Opcode::Select {
        return no;
    }
    // Only handle single use cases for now.
    let Some(cmp) = c.inst_of(instruction.operands[0]) else { return no };
    if !c.is_cmp(cmp) || c.use_count(cmp) != 1 {
        return no;
    }
    let (true_val, false_val) = (instruction.operands[1], instruction.operands[2]);
    let true_phi = c.inst_of(true_val).is_some_and(|def| c.is_phi(def));
    let false_phi = c.inst_of(false_val).is_some_and(|def| c.is_phi(def));
    // Handle only when either of operands of select instruction is a PHI node.
    if (true_phi && false_phi) || (!true_phi && !false_phi) {
        return no;
    }
    let other = if true_phi { false_val } else { true_val };
    let Some(i1) = c.inst_of(other) else { return no };
    let Opcode::Binary(op) = c.instruction(i1).opcode else { return no };
    let fast = c.instruction(i1).flags.contains(Flags::FAST);
    let accepted = match op {
        BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul => fast,
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul => true,
        _ => false,
    };
    if !accepted {
        return no;
    }
    let (op1, op2) = (c.instruction(i1).operands[0], c.instruction(i1).operands[1]);
    let phi_op = if c.inst_of(op1).is_some_and(|def| c.is_phi(def)) { op1 } else { op2 };
    let Some(i_phi) = c.inst_of(phi_op) else { return no };
    if Operand::Value(c.instruction(i_phi).result.expect("a phi")) != false_val {
        return no;
    }
    InstDesc::plain(true, Some(inst), None)
}

/// `isRecurrenceInstr`.
fn is_recurrence_instr(
    c: &Ctx,
    orig_phi: InstId,
    inst: InstId,
    kind: RecurKind,
    prev: &InstDesc,
    has_scev: bool,
) -> InstDesc {
    debug_assert!(prev.rec_kind == RecurKind::None || prev.rec_kind == kind);
    let instruction = c.instruction(inst);
    let reassoc_or_exact = |c: &Ctx| if c.instruction(inst).flags.contains(Flags::REASSOC) { None } else { Some(inst) };
    match instruction.opcode {
        Opcode::Phi => InstDesc {
            is_recurrence: true,
            pattern_last_inst: Some(inst),
            rec_kind: prev.rec_kind,
            exact_fp_math_inst: prev.exact_fp_math_inst,
        },
        Opcode::Binary(BinaryOp::Sub) => {
            InstDesc::plain(kind == RecurKind::Sub || kind == RecurKind::AddChainWithSubs, Some(inst), None)
        }
        Opcode::Binary(BinaryOp::Add) => {
            InstDesc::plain(kind == RecurKind::Add || kind == RecurKind::AddChainWithSubs, Some(inst), None)
        }
        Opcode::Binary(BinaryOp::Mul) => InstDesc::plain(kind == RecurKind::Mul, Some(inst), None),
        Opcode::Binary(BinaryOp::And) => InstDesc::plain(kind == RecurKind::And, Some(inst), None),
        Opcode::Binary(BinaryOp::Or) => InstDesc::plain(kind == RecurKind::Or, Some(inst), None),
        Opcode::Binary(BinaryOp::Xor) => InstDesc::plain(kind == RecurKind::Xor, Some(inst), None),
        Opcode::Binary(BinaryOp::FDiv | BinaryOp::FMul) => {
            InstDesc::plain(kind == RecurKind::FMul, Some(inst), reassoc_or_exact(c))
        }
        Opcode::Binary(BinaryOp::FSub) => InstDesc::plain(
            kind == RecurKind::FSub || kind == RecurKind::FAddChainWithSubs,
            Some(inst),
            reassoc_or_exact(c),
        ),
        Opcode::Binary(BinaryOp::FAdd) => InstDesc::plain(
            kind == RecurKind::FAdd || kind == RecurKind::FAddChainWithSubs,
            Some(inst),
            reassoc_or_exact(c),
        ),
        Opcode::Select | Opcode::FCmp(_) | Opcode::ICmp(_) | Opcode::Call(_) => {
            if instruction.opcode == Opcode::Select {
                if kind.is_sub()
                    || matches!(
                        kind,
                        RecurKind::FAdd
                            | RecurKind::FMul
                            | RecurKind::Add
                            | RecurKind::Mul
                            | RecurKind::AddChainWithSubs
                            | RecurKind::FAddChainWithSubs
                    )
                {
                    return is_conditional_rdx_pattern(c, inst);
                }
                if kind.is_find() && has_scev {
                    return is_find_pattern(c, orig_phi, inst);
                }
            }
            if kind == RecurKind::AnyOf {
                return is_any_of_pattern(c, orig_phi, inst, prev);
            }
            if c.is_fmuladd(inst) {
                return InstDesc::plain(kind == RecurKind::FMulAdd, Some(inst), reassoc_or_exact(c));
            }
            InstDesc::plain(false, Some(inst), None)
        }
        _ => InstDesc::plain(false, Some(inst), None),
    }
}

/// `AddReductionVar`.
fn add_reduction_var(
    c: &Ctx,
    se: &mut Option<&mut ScalarEvolution>,
    phi: InstId,
    mut kind: RecurKind,
) -> Option<RecurrenceDescriptor> {
    let phi_inst = c.instruction(phi);
    if phi_inst.operands.len() != 4 {
        return None;
    }
    // Reduction variables are only found in the loop header block.
    if c.parent(phi) != c.the_loop.get_header() {
        return None;
    }
    // Obtain the reduction start value from the value that comes from the loop
    // preheader.
    let preheader = c.the_loop.get_loop_preheader(c.function())?;
    let rdx_start = c.incoming_for(phi, preheader)?;
    // ExitInstruction is the single value which is used outside the loop.
    let mut exit_instruction: Option<InstId> = None;
    // The last store of the reduction to an invariant address.
    let mut intermediate_store: Option<InstId> = None;
    // Indicates that we found a reduction operation in our scan.
    let mut found_redux_op = false;
    // The cycle must include the original PHI.
    let mut found_start_phi = false;
    let mut num_cmp_select_pattern_inst = 0;
    let mut redux_desc = InstDesc::plain(false, None, None);
    let mut found_find_last_like_phi = false;

    // Data used for determining if the recurrence has been type-promoted.
    let mut recurrence_type = phi_inst.ty;
    let mut cast_insts: Vec<InstId> = Vec::new();
    let mut start = phi;
    let is_signed = false;

    let mut visited: Vec<InstId> = Vec::new();
    let mut worklist: Vec<InstId> = Vec::new();

    // Return early if the recurrence kind does not match the type of Phi. If
    // the recurrence kind is arithmetic, we attempt to look through AND
    // operations resulting from the type promotion performed by InstCombine.
    let scalar = phi_inst.ty;
    if kind == RecurKind::FindLast {
        if !c.scalar_is_float(scalar) && !c.scalar_is_int(scalar) && !c.scalar_is_pointer(scalar) {
            return None;
        }
    } else if c.scalar_is_float(scalar) {
        if !kind.is_floating_point() {
            return None;
        }
    } else if c.scalar_is_int(scalar) {
        if !kind.is_integer() {
            return None;
        }
        start = look_through_and(c, phi, &mut recurrence_type, &mut visited, &mut cast_insts);
    } else {
        // Pointer min/max may exist, but it is not supported as a reduction
        // op.
        return None;
    }

    worklist.push(start);
    visited.push(start);

    // Start with all flags set because we will intersect this with the
    // reduction flags from all the reduction operations.
    let mut fmf = all_fmf();
    // The first instruction in the use-def chain of the Phi node that
    // requires exact floating point operations.
    let mut exact_fp_math_inst: Option<InstId> = None;

    while let Some(cur) = worklist.pop() {
        // Store instructions are allowed iff it is the store of the reduction
        // value to the same loop invariant memory location.
        if matches!(c.instruction(cur).opcode, Opcode::Store { .. }) {
            let Some(se) = se.as_deref_mut() else { return None };
            let ptr = se.get_scev(c.cx, c.instruction(cur).operands[1]);
            // Check it is the same address as previous stores.
            if let Some(previous) = intermediate_store {
                let other = se.get_scev(c.cx, c.instruction(previous).operands[1]);
                if other != ptr {
                    return None;
                }
            }
            // Check the pointer is loop invariant.
            if !se.is_loop_invariant(c.cx, ptr, Some(c.the_loop.get_header())) {
                return None;
            }
            // IntermediateStore is always the last store in the loop.
            intermediate_store = Some(cur);
            continue;
        }

        // No Users: a broken chain can't be a reduction variable.
        if c.use_count(cur) == 0 {
            return None;
        }

        let is_a_phi = c.is_phi(cur);
        // A header PHI use other than the original PHI.
        if cur != phi && is_a_phi && c.parent(cur) == c.parent(phi) {
            return None;
        }

        // Reductions of instructions such as Div, and Sub is only possible if
        // the LHS is the reduction variable.
        if !c.is_commutative(cur)
            && !is_a_phi
            && !c.is_select(cur)
            && !c.is_cmp(cur)
            && !c.inst_of(c.instruction(cur).operands[0]).is_some_and(|def| visited.contains(&def))
        {
            return None;
        }

        // Any reduction instruction must be of one of the allowed kinds. We
        // ignore the starting value.
        if cur != start {
            redux_desc = is_recurrence_instr(c, phi, cur, kind, &redux_desc, se.is_some());
            if exact_fp_math_inst.is_none() {
                exact_fp_math_inst = redux_desc.exact_fp_math_inst;
            }
            if !redux_desc.is_recurrence {
                return None;
            }
            if let Some(pattern) = redux_desc.pattern_last_inst
                && c.is_fp_math_operator(pattern)
                && !is_a_phi
            {
                fmf = intersection(fmf, collect_min_max_fmf(c, pattern));
            }
            // Update this reduction kind if we matched a new instruction.
            if redux_desc.rec_kind != RecurKind::None {
                kind = redux_desc.rec_kind;
            }
        }

        let is_a_select = c.is_select(cur);

        // A conditional reduction operation must only have 2 or less uses in
        // VisitedInsts.
        if is_a_select
            && (kind == RecurKind::FAdd || kind == RecurKind::FMul)
            && has_multiple_uses_of(c, cur, &visited, 2)
        {
            return None;
        }

        // A reduction operation must only have one use of the reduction value.
        if !is_a_phi && !is_a_select && kind != RecurKind::AnyOf && has_multiple_uses_of(c, cur, &visited, 1) {
            return None;
        }

        // All inputs to a PHI node must be a reduction value, unless the phi
        // is a "FindLast-like" phi.
        if is_a_phi && cur != phi && !are_all_uses_in(c, cur, &visited) {
            found_find_last_like_phi = kind == RecurKind::FindLast
                && !found_find_last_like_phi
                && is_find_last_like_phi(c, cur, phi, &visited);
            if !found_find_last_like_phi {
                return None;
            }
        }

        if kind == RecurKind::AnyOf && is_a_select {
            num_cmp_select_pattern_inst += 1;
        }

        // Check whether we found a reduction operator.
        found_redux_op |= (!is_a_phi || found_find_last_like_phi) && cur != start;

        // Process users of current instruction. Push non-PHI nodes after PHI
        // nodes onto the stack. This way we are going to have seen all inputs
        // to PHI nodes once we get to them.
        let mut non_phis: Vec<InstId> = Vec::new();
        let mut phis: Vec<InstId> = Vec::new();
        for ui in c.users(cur) {
            // If the user is a call to llvm.fmuladd then the instruction can
            // only be the final operand.
            if c.is_fmuladd(ui) {
                let cur_value = Operand::Value(c.instruction(cur).result.expect("a value"));
                let operands = &c.instruction(ui).operands;
                if cur_value == operands[0] || cur_value == operands[1] {
                    return None;
                }
            }

            // Check if we found the exit user.
            let parent = c.parent(ui);
            if !c.the_loop.contains(parent) {
                // If we already know this instruction is used externally,
                // move on to the next user.
                if exit_instruction == Some(cur) {
                    continue;
                }
                // Exit if you find multiple values used outside or if the
                // header phi node is being used.
                if exit_instruction.is_some() || cur == phi {
                    return None;
                }
                // The instruction used by an outside user must be the last
                // instruction before we feed back to the reduction phi.
                let cur_value = Operand::Value(c.instruction(cur).result.expect("a value"));
                if !c.instruction(phi).operands.contains(&cur_value) {
                    return None;
                }
                exit_instruction = Some(cur);
                continue;
            }

            // Process instructions only once (termination). Each reduction
            // cycle value must only be used once, except by phi nodes and
            // conditional reductions which are represented as a cmp followed
            // by a select.
            if !visited.contains(&ui) {
                visited.push(ui);
                if c.is_phi(ui) {
                    phis.push(ui);
                } else {
                    if matches!(c.instruction(ui).opcode, Opcode::Store { .. }) {
                        let cur_value = Operand::Value(c.instruction(cur).result.expect("a value"));
                        if c.instruction(ui).operands[1] == cur_value {
                            // Reduction variable chain can only be stored
                            // somewhere but it can't be used as an address.
                            return None;
                        }
                    }
                    non_phis.push(ui);
                }
            } else if !c.is_phi(ui) {
                let ignored = InstDesc::plain(false, None, None);
                if !is_conditional_rdx_pattern(c, ui).is_recurrence
                    && !is_any_of_pattern(c, phi, ui, &ignored).is_recurrence
                {
                    return None;
                }
            }

            // Remember that we completed the cycle.
            if ui == phi {
                found_start_phi = true;
            }
        }
        worklist.extend(phis);
        worklist.extend(non_phis);
    }

    if kind == RecurKind::AnyOf && num_cmp_select_pattern_inst != 1 {
        return None;
    }

    if let Some(store) = intermediate_store {
        // Check that stored value goes to the phi node again. This way we
        // make sure that the value stored in IntermediateStore is indeed the
        // final reduction value.
        let stored = c.instruction(store).operands[0];
        if !c.instruction(phi).operands.contains(&stored) {
            return None;
        }
        // If there is an exit instruction it's value should be stored in
        // IntermediateStore.
        if let Some(exit) = exit_instruction
            && stored != Operand::Value(c.instruction(exit).result.expect("a value"))
        {
            return None;
        }
        // If all uses are inside the loop (intermediate stores), then the
        // reduction value after the loop will be the one used in the last
        // store.
        if exit_instruction.is_none() {
            exit_instruction = Some(c.inst_of(stored).expect("a stored instruction"));
        }
    }

    if !found_start_phi || !found_redux_op {
        return None;
    }
    let exit_instruction = exit_instruction?;

    let is_ordered = check_ordered_reduction(c, kind, exact_fp_math_inst, exit_instruction, phi);

    if start != phi {
        // D1: with no demanded bits, assumption cache or dominator tree the
        // computed type is the exit instruction's width rounded up to a power
        // of two; the starting `and` is only a type-promotion if it names that.
        let width = c.cx.layout.size_bits(&c.cx.context.types, c.instruction(exit_instruction).ty) as u32;
        let computed = Some(c.cx.context.types.int(width.next_power_of_two()));
        if computed != Some(recurrence_type) {
            return None;
        }
    }

    // Collect cast instructions and the minimum width used by the recurrence.
    let min_width = collect_cast_instrs(c, exit_instruction, recurrence_type, &mut cast_insts);

    // Save the description of this reduction variable.
    Some(RecurrenceDescriptor {
        intermediate_store,
        start_value: rdx_start,
        loop_exit_instr: Some(exit_instruction),
        kind,
        fmf,
        exact_fp_math_inst,
        recurrence_type,
        is_signed,
        is_ordered,
        phi_has_uses_outside_reduction_chain: false,
        cast_insts,
        min_width_cast_to_recurrence_type: min_width,
    })
}

/// `getMinMaxRecurrence`.
fn get_min_max_recurrence(
    c: &Ctx,
    se: &mut Option<&mut ScalarEvolution>,
    phi: InstId,
) -> Option<RecurrenceDescriptor> {
    let phi_inst = c.instruction(phi);
    let ty = phi_inst.ty;
    let latch = c.the_loop.get_loop_latch(c.function());
    if phi_inst.operands.len() != 4
        || c.parent(phi) != c.the_loop.get_header()
        || (!c.scalar_is_int(ty) && !c.scalar_is_float(ty))
        || latch.is_none()
    {
        return None;
    }
    let latch = latch?;
    let phi_value = Operand::Value(phi_inst.result.expect("a phi"));

    let mut fmf = all_fmf();
    let backedge_value = c.incoming_for(phi, latch)?;
    let mut rk = RecurKind::None;
    // Walk def-use chains upwards from BackedgeValue to identify min/max
    // recurrences.
    let mut worklist: Vec<Operand> = vec![backedge_value];
    let mut chain: Vec<Operand> = vec![phi_value];
    while let Some(cur) = worklist.pop() {
        if chain.contains(&cur) {
            continue;
        }
        chain.push(cur);
        let Some(i) = c.inst_of(cur) else { return None };
        if !c.contains(i) {
            return None;
        }
        if c.is_phi(i) {
            worklist.extend(c.instruction(i).operands.iter().filter(|one| !matches!(one, Operand::Block(_))).copied());
            continue;
        }
        let (cur_rk, args) = c.min_max_kind(cur);
        if cur_rk == RecurKind::None || (rk != RecurKind::None && cur_rk != rk) {
            return None;
        }
        let (a, b) = args.expect("operands of a min/max");
        rk = cur_rk;
        // Check required fast-math flags for FP recurrences.
        if cur_rk.is_fp_min_max() {
            let cur_fmf = has_required_fast_math_flags(c, i, &mut rk)?;
            fmf = intersection(fmf, cur_fmf);
        }

        if c.is_select(i) {
            chain.push(c.instruction(i).operands[0]);
        }

        if a == phi_value || b == phi_value {
            continue;
        }

        // Add operand to worklist if it matches the pattern (exactly one must
        // match).
        let matches =
            |operand: Operand| c.inst_of(operand).is_some_and(|def| c.contains(def)) && c.min_max_kind(operand).0 == rk;
        let (a_matches, b_matches) = (matches(a), matches(b));
        if a_matches == b_matches {
            // Both or neither match.
            return None;
        }
        worklist.push(if a_matches { a } else { b });
    }

    // Handle argmin/argmax pattern: PHI has uses outside the reduction chain
    // that are not intermediate min/max operations.
    let phi_has_invalid_uses = c
        .users(phi)
        .into_iter()
        .any(
            |user| {
                let user_value = c.instruction(user).result.map(Operand::Value);
                !user_value.is_some_and(|value| chain.contains(&value))
                    && c.contains(user)
                    && user_value.is_none_or(|value| c.min_max_kind(value).0 == RecurKind::None)
            },
        );
    if phi_has_invalid_uses {
        let backedge_uses = c.inst_of(backedge_value).map_or(0, |def| c.use_count(def));
        if !rk.is_min_max() || backedge_uses != 1 {
            return None;
        }
        let preheader = c.the_loop.get_loop_preheader(c.function())?;
        return Some(RecurrenceDescriptor {
            intermediate_store: None,
            start_value: c.incoming_for(phi, preheader)?,
            loop_exit_instr: None,
            kind: rk,
            fmf: Flags::default(),
            exact_fp_math_inst: None,
            recurrence_type: ty,
            is_signed: false,
            is_ordered: false,
            phi_has_uses_outside_reduction_chain: true,
            cast_insts: Vec::new(),
            min_width_cast_to_recurrence_type: u32::MAX,
        });
    }

    // Validate chain entries and collect stores from chain entries and
    // intermediate ops.
    let mut stores: Vec<InstId> = Vec::new();
    for value in chain.clone() {
        let users: Vec<InstId> = match value {
            Operand::Value(v) => c.function().users(v).iter().map(|one| one.user).collect(),
            _ => continue,
        };
        for user in users {
            let user_value = c.instruction(user).result.map(Operand::Value);
            if user_value.is_some_and(|one| chain.contains(&one)) {
                continue;
            }
            if !c.contains(user) && value != backedge_value {
                return None;
            }
            if !c.contains(user) {
                continue;
            }
            if matches!(c.instruction(user).opcode, Opcode::Store { .. }) {
                stores.push(user);
                continue;
            }
            // Must be intermediate min/max of the same kind.
            let Some(user_value) = user_value else { return None };
            if c.min_max_kind(user_value).0 != rk {
                return None;
            }
            for inner in c.users(user) {
                if matches!(c.instruction(inner).opcode, Opcode::Store { .. }) {
                    stores.push(inner);
                } else if !c.instruction(inner).result.map(Operand::Value).is_some_and(|one| chain.contains(&one)) {
                    return None;
                }
            }
        }
    }

    // Validate all stores go to same invariant address and are in the same
    // block.
    let mut intermediate_store: Option<InstId> = None;
    let mut store_ptr_scev: Option<Scev> = None;
    for store in stores {
        let Some(se) = se.as_deref_mut() else { return None };
        let ptr = se.get_scev(c.cx, c.instruction(store).operands[1]);
        if !se.is_loop_invariant(c.cx, ptr, Some(c.the_loop.get_header()))
            || store_ptr_scev.is_some_and(|one| one != ptr)
        {
            return None;
        }
        store_ptr_scev = Some(ptr);
        match intermediate_store {
            None => intermediate_store = Some(store),
            Some(current) if c.parent(current) != c.parent(store) => return None,
            Some(current) => {
                // `comesBefore`: later in the block.
                let order: Vec<InstId> = c.function().iter_block(c.parent(store)).collect();
                let at = |one: InstId| order.iter().position(|&x| x == one);
                if at(current) < at(store) {
                    intermediate_store = Some(store);
                }
            }
        }
    }

    let preheader = c.the_loop.get_loop_preheader(c.function())?;
    Some(RecurrenceDescriptor {
        intermediate_store,
        start_value: c.incoming_for(phi, preheader)?,
        loop_exit_instr: c.inst_of(backedge_value),
        kind: rk,
        fmf,
        exact_fp_math_inst: None,
        recurrence_type: ty,
        is_signed: false,
        is_ordered: false,
        phi_has_uses_outside_reduction_chain: false,
        cast_insts: Vec::new(),
        min_width_cast_to_recurrence_type: u32::MAX,
    })
}

/// `RecurrenceDescriptor::isReductionPHI`: the description of `phi` as a
/// reduction of `the_loop`, if it is one. `se` is the optional
/// `ScalarEvolution` (without it a store of the reduction and the find
/// patterns are not recognised).
pub fn is_reduction_phi(
    cx: &Cx,
    se: Option<&mut ScalarEvolution>,
    phi: InstId,
    the_loop: Loop,
) -> Option<RecurrenceDescriptor> {
    let c = Ctx { cx, the_loop };
    let mut se = se;
    let order = [
        RecurKind::Add,
        RecurKind::Sub,
        RecurKind::AddChainWithSubs,
        RecurKind::Mul,
        RecurKind::Or,
        RecurKind::And,
        RecurKind::Xor,
    ];
    for kind in order {
        if let Some(found) = add_reduction_var(&c, &mut se, phi, kind) {
            return Some(found);
        }
    }
    if let Some(found) = get_min_max_recurrence(&c, &mut se, phi) {
        debug_assert!(found.kind.is_min_max(), "Expected a min/max recurrence kind");
        return Some(found);
    }
    let order = [
        RecurKind::AnyOf,
        RecurKind::FindLast,
        RecurKind::FMul,
        RecurKind::FSub,
        RecurKind::FAdd,
        RecurKind::FAddChainWithSubs,
        RecurKind::FMulAdd,
    ];
    for kind in order {
        if let Some(found) = add_reduction_var(&c, &mut se, phi, kind) {
            return Some(found);
        }
    }
    // Not a reduction of known type.
    None
}

impl RecurrenceDescriptor {
    /// `getReductionOpChain`: the operations from `phi` to the loop exit
    /// instruction, empty where it is not a simple chain.
    pub fn reduction_op_chain(
        &self,
        cx: &Cx,
        phi: InstId,
        the_loop: Loop,
    ) -> Vec<InstId> {
        let c = Ctx { cx, the_loop };
        let Some(loop_exit) = self.loop_exit_instr else { return Vec::new() };
        let is_min_max = self.kind.is_min_max();
        // Search down from the Phi to the LoopExitInstr, looking for
        // instructions with a single user of the correct type for the
        // reduction.
        let expected_uses = if is_min_max { 2 } else { 1 };

        let next_instruction = |cur: InstId| -> Option<InstId> {
            for user in c.users(cur) {
                if c.is_phi(user) {
                    continue;
                }
                if is_min_max {
                    // We are expecting a icmp/select pair, which we go to the
                    // next select instruction if we can.
                    if c.is_select(user) {
                        return Some(user);
                    }
                    continue;
                }
                return Some(user);
            }
            None
        };
        let correct_opcode = |cur: InstId| -> bool {
            let opcode = &c.instruction(cur).opcode;
            if is_min_max {
                // matchSelectPattern(Cur).Flavor is min or max.
                let query = c.cx.query(None);
                let flavor = llrm_mir::valuetracking::range::match_select_pattern(
                    &query,
                    Operand::Value(c.instruction(cur).result.expect("a value")),
                )
                .flavor;
                return flavor.is_min_or_max();
            }
            // Recognize a call to the llvm.fmuladd intrinsic.
            if c.is_fmuladd(cur) {
                return true;
            }
            if *opcode == Opcode::Binary(BinaryOp::Sub) && self.kind == RecurKind::AddChainWithSubs {
                return true;
            }
            if *opcode == Opcode::Binary(BinaryOp::FSub) && self.kind == RecurKind::FAddChainWithSubs {
                return true;
            }
            *opcode == self.kind.opcode()
        };

        // Attempt to look through Phis which are part of the reduction chain.
        let mut extra_phi_uses = 0;
        let mut rdx_instr = loop_exit;
        if c.is_phi(loop_exit) {
            let operands = &c.instruction(loop_exit).operands;
            if operands.len() != 4 {
                return Vec::new();
            }
            let inc0 = c.inst_of(operands[0]);
            let inc1 = c.inst_of(operands[2]);
            let chain = if inc0 == Some(phi) {
                inc1
            } else if inc1 == Some(phi) {
                inc0
            } else {
                return Vec::new();
            };
            let Some(chain) = chain else { return Vec::new() };
            rdx_instr = chain;
            extra_phi_uses = 1;
        }

        // The loop exit instruction we check first (as a quick test) but add
        // last. They will have one use from the phi and one from a LCSSA value,
        // no matter the type.
        if !correct_opcode(rdx_instr) || c.use_count(loop_exit) != 2 {
            return Vec::new();
        }

        // Check that the Phi has one (or two for min/max) uses, plus an extra
        // use for conditional reductions.
        if c.use_count(phi) != expected_uses + extra_phi_uses {
            return Vec::new();
        }

        let mut operations = Vec::new();
        let mut cur = next_instruction(phi);
        // Each other instruction in the chain should have the expected number
        // of uses and be the correct opcode.
        while cur != Some(rdx_instr) {
            let Some(one) = cur else { return Vec::new() };
            if !correct_opcode(one) || c.use_count(one) != expected_uses {
                return Vec::new();
            }
            operations.push(one);
            cur = next_instruction(one);
        }
        operations.push(rdx_instr);
        operations
    }
}

/// `InductionDescriptor::InductionKind`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InductionKind {
    /// An integer induction.
    Int,
    /// A pointer induction.
    Ptr,
    /// A floating-point induction.
    Fp,
}

/// The step of an induction. D4: SCEV has no floating-point type, so where
/// LLVM keeps `getUnknown(Addend)` for an FP induction this keeps the addend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InductionStep {
    Scev(Scev),
    Fp(Operand),
}

/// `InductionDescriptor`.
#[derive(Clone, Debug)]
pub struct InductionDescriptor {
    pub start_value: Operand,
    pub kind: InductionKind,
    /// The step.
    pub step: InductionStep,
    /// The binary operation updating the induction, if any.
    pub induction_bin_op: Option<InstId>,
}

/// `InductionDescriptor::isFPInductionPHI`: the floating-point branch of the
/// `PredicatedScalarEvolution` overload of `isInductionPHI` (D2); the
/// `ScalarEvolution` overload ([`is_induction_phi`]) does not take it.
pub fn is_fp_induction_phi(
    cx: &Cx,
    phi: InstId,
    the_loop: Loop,
) -> Option<InductionDescriptor> {
    let c = &Ctx { cx, the_loop };
    if c.the_loop.get_header() != c.parent(phi) {
        return None;
    }
    // The loop may have multiple entrances or multiple exits; we can analyze
    // this phi if it has a unique entry value and a unique backedge value.
    let operands = &c.instruction(phi).operands;
    if operands.len() != 4 {
        return None;
    }
    let (Operand::Block(block0), Operand::Block(block1)) = (operands[1], operands[3]) else { return None };
    let (be_value, start_value) = if c.the_loop.contains(block0) {
        (operands[0], operands[2])
    } else {
        debug_assert!(c.the_loop.contains(block1), "Unexpected Phi node in the loop");
        (operands[2], operands[0])
    };

    let bop = c.inst_of(be_value)?;
    let Opcode::Binary(op) = c.instruction(bop).opcode else { return None };
    let phi_value = Operand::Value(c.instruction(phi).result.expect("a phi"));
    let bop_operands = &c.instruction(bop).operands;
    let addend = match op {
        BinaryOp::FAdd => {
            if bop_operands[0] == phi_value {
                Some(bop_operands[1])
            } else if bop_operands[1] == phi_value {
                Some(bop_operands[0])
            } else {
                None
            }
        }
        BinaryOp::FSub => (bop_operands[0] == phi_value).then_some(bop_operands[1]),
        _ => None,
    }?;

    // The addend should be loop invariant.
    if let Some(def) = c.inst_of(addend)
        && c.contains(def)
    {
        return None;
    }

    Some(InductionDescriptor {
        start_value,
        kind: InductionKind::Fp,
        step: InductionStep::Fp(addend),
        induction_bin_op: Some(bop),
    })
}

/// `InductionDescriptor::isInductionPHI(Phi, TheLoop, SE, D)`: the description
/// of `phi` as an induction variable of `the_loop`, if it is one (D2).
pub fn is_induction_phi(
    cx: &Cx,
    se: &mut ScalarEvolution,
    phi: InstId,
    the_loop: Loop,
) -> Option<InductionDescriptor> {
    let c = Ctx { cx, the_loop };
    let ty = c.instruction(phi).ty;
    // isSCEVable returns true for integer and pointer types.
    if !c.scalar_is_int(ty) && !c.scalar_is_pointer(ty) {
        return None;
    }
    let phi_scev = se.get_scev(cx, Operand::Value(c.instruction(phi).result.expect("a phi")));
    // Check that the PHI is consecutive: an affine add recurrence of the loop.
    let Kind::AddRec(operands, header) = se.node(phi_scev).kind.clone() else { return None };
    if operands.len() != 2 || header != the_loop.get_header() {
        return None;
    }
    let step = InductionStep::Scev(operands[1].scev);

    debug_assert!(c.parent(phi) == the_loop.get_header(), "Invalid Phi node, not present in loop header");

    let preheader = the_loop.get_loop_preheader(cx.function)?;
    let start_value = c.incoming_for(phi, preheader)?;
    let latch = the_loop.get_loop_latch(cx.function)?;

    if c.scalar_is_int(ty) {
        let bop = c
            .incoming_for(phi, latch)
            .and_then(|value| c.inst_of(value))
            .filter(|&def| matches!(c.instruction(def).opcode, Opcode::Binary(_)));
        return Some(InductionDescriptor { start_value, kind: InductionKind::Int, step, induction_bin_op: bop });
    }

    // This allows induction variables w/non-constant steps.
    Some(InductionDescriptor { start_value, kind: InductionKind::Ptr, step, induction_bin_op: None })
}

/// Whether `phi` is in the header of `the_loop` (the callers' precondition).
pub fn is_header_phi(
    function: &Function,
    the_loop: Loop,
    phi: InstId,
) -> bool {
    function.instruction(phi).opcode == Opcode::Phi && function.parent(phi) == Some(the_loop.get_header())
}

#[cfg(test)]
#[path = "ivdescriptors_tests.rs"]
mod tests;
