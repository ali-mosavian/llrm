//! InstSimplify: the value an instruction equals when its operands say so,
//! found without making an instruction.
//!
//! A translation of LLVM's
//! [`InstructionSimplify.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp)
//! at d1106deb71cc, integer scalars only. Each function names the C++ function
//! it translates and keeps its order of checks; `max_recurse` is the C++
//! `MaxRecurse` (`RecursionLimit` is 3). A result is an existing operand or a
//! constant; the function is never changed.
//!
//! MIRROR: `simplifyInstruction`'s dispatch; the floating functions of
//! `simplify_fp.rs`; `simplifyAddInst`,
//! `simplifySubInst`, `simplifyMulInst`, `simplifyDivRem`, `simplifyShift` and
//! the shift, and/or/xor functions, `simplifyAssociativeBinOp`,
//! `simplifyLogicOfAddSub`, `simplifySelectInst`, `simplifyCastInst`,
//! `simplifyPHINode`, the `undef` arms of each, and the compare-of-self, bool
//! and zero parts of `simplifyICmpInst`.
//!
//! `nsw`/`nuw`/`exact` are read from the instruction (`IIQ.UseInstrInfo`).
//!
//! DEVIATE, permanent (MIR lacks the construct):
//! 1. Vectors: `extractelement`, `insertelement`, `shufflevector`,
//!    `simplifyIdentityInterleave`, `simplifySVEIntReduction`. MIR types
//!    vectors but has no lane operation and nothing produces a vector value
//!    (#2201).
//!
//! OWED, each with an issue and its exit condition:
//! 2. `x86_fp80` constants: MIR has the type (`FloatKind::X86Fp80`) but
//!    `ConstantKind::Float` holds 64 bits, so an extended constant is held as
//!    the nearest double (`constfold.rs`): #2251. Whether MIR carries fp80 per
//!    target is a separate decision.
//! 3. Floating-point folds: `simplify_fp.rs` has `fneg` and the five binary
//!    operations; `simplifyFMAFMul`'s `computeKnownFPClass` of a value, `FMA`,
//!    `FCmp`, `AndOrOfFCmps`, `Ldexp`, constrained calls and `bitcast (uitofp
//!    X)` remain: #2246.
//! 4. Pointer compares, `addrspacecast`: #2247.
//! 4a. `simplifyLoadInst`
//!    ([:7702](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L7702))
//!    is `simplify_load`, whose fold is `ConstantFoldLoadFromConstPtr`
//!    ([L898](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L898)):
//!    a non-volatile load of a constant address. The half for an address that
//!    is an instruction over a constant global (`getUnderlyingObject`,
//!    `stripAndAccumulateConstantOffsets`, uniform initializers) is not
//!    translated. Reason: the constant-address half is what the client loop
//!    needed; the other needs a value-level underlying-object walk. Owner:
//!    w-3, with #2247. Exit: translated, 2026-11-06.
//! 5. `simplifyICmpWithIntrinsicOnLHS`, the call arm of
//!    `simplifyWithOpsReplaced`, the floating-point intrinsics: #2248, #2246.
//!    `simplifyCall`'s integer intrinsics are `simplify_calls.rs`; the callee
//!    is found by name, so `simplify_instruction_in`'s caller passes the
//!    globals.
//! 6. `simplifyICmpWithDominatingAssume`: #2249; `simplifySelectWithFCmp`:
//!    #2246. The select-on-compare forms are `simplify_select.rs`.
//! 7. `extractvalue`, `insertvalue`: #2250.
//! 8. GEP and cast constant expressions, wrap-flag poison folds in
//!    `constfold.rs`: #2252.
//!
//! # Used by
//! `earlycse`, the combine worklist, `gvn` and `fold` would call
//! `simplify_instruction_in` with their `FunctionAnalyses` (the dominator tree
//! and the assumptions). The other entry points (`simplify_binop_operands`...)
//! pass no module, tree, assumptions or context instruction:
//! `simplifyByDomEq`, `isImpliedByDomCondition` and `valueDominatesPHI` then
//! find only the facts that need none (a single-predecessor branch needs only
//! the context instruction, which `simplify_instruction_in` sets).

use crate::constantfolding;
use crate::constantrange::ConstantRange;
use crate::constantrange::PreferredRangeType;
use crate::constfold;
use crate::context::{Constant, ConstantId, ConstantKind, Context, mask, signed};
use crate::datalayout::DataLayout;
use crate::knownbits::KnownBits;
use crate::module::{Function, GlobalValue, InstId, Operand, ValueDef};
use crate::opcode::{BinaryOp, CastOp, Flags, FloatPredicate, IntPredicate, Opcode};
use crate::types::TypeId;
use crate::valuetracking::{
    FunctionAnalyses, Query as ValueQuery, context as conditions, guarantee, known, nonzero, pattern, power2, range,
    signbits,
};

/// `RecursionLimit` in InstructionSimplify.cpp.
const RECURSION_LIMIT: u32 = 3;

/// The `SimplifyQuery`: what an operand is, looked up in `function`.
struct Query<'a> {
    context: &'a Context,
    function: &'a Function,
    layout: &'a DataLayout,
    /// The module's globals, which name the callees; empty where the caller has
    /// no module.
    globals: &'a [GlobalValue],
    /// `SQ.DT` and `SQ.AC`, where the caller has them.
    analyses: Option<&'a FunctionAnalyses>,
    /// `SQ.CxtI`: the instruction the query is asked at.
    cxt: Option<InstId>,
    /// `SQ.CanUseUndef`: false in `getWithoutUndef()`.
    can_use_undef: bool,
}

impl Query<'_> {
    fn ty(
        &self,
        operand: Operand,
    ) -> TypeId {
        self.function.operand_type(self.context, operand).expect("a value or a constant")
    }

    /// The width of an integer operand of at most 128 bits.
    fn width(
        &self,
        operand: Operand,
    ) -> Option<u32> {
        self.function
            .operand_type(self.context, operand)
            .and_then(|ty| self.context.types.int_bits(ty))
            .filter(|&width| width <= 128)
    }

    fn constant(
        &self,
        operand: Operand,
    ) -> Option<ConstantId> {
        match operand {
            Operand::Constant(id) => Some(id),
            _ => None,
        }
    }

    /// `m_APInt`: the bits and width of a constant integer (a zero too).
    fn number(
        &self,
        operand: Operand,
    ) -> Option<(u128, u32)> {
        let constant = self.context.get(self.constant(operand)?);
        let width = self.context.types.int_bits(constant.ty).filter(|&width| width <= 128)?;
        match constant.kind {
            ConstantKind::Int(bits) => Some((bits, width)),
            ConstantKind::Null | ConstantKind::Zero => Some((0, width)),
            _ => None,
        }
    }

    fn is_poison(
        &self,
        operand: Operand,
    ) -> bool {
        self.constant(operand).is_some_and(|id| self.context.get(id).kind == ConstantKind::Poison)
    }

    /// `SimplifyQuery::isUndefValue`: `m_Undef`, which poison matches too.
    fn is_undef(
        &self,
        operand: Operand,
    ) -> bool {
        self.can_use_undef
            && self
                .constant(operand)
                .is_some_and(|id| matches!(
                    self.context.get(id).kind,
                    ConstantKind::Undef | ConstantKind::Poison
                ))
    }

    /// `UndefValue::get`.
    fn undef(
        &mut self,
        ty: TypeId,
    ) -> Operand {
        Operand::Constant(self.context.constant(Constant { ty, kind: ConstantKind::Undef }))
    }

    /// `m_Zero`.
    fn is_zero(
        &self,
        operand: Operand,
    ) -> bool {
        self.number(operand).is_some_and(|(bits, _)| bits == 0)
    }

    /// `m_One`.
    fn is_one(
        &self,
        operand: Operand,
    ) -> bool {
        self.number(operand).is_some_and(|(bits, _)| bits == 1)
    }

    /// `m_AllOnes`.
    fn is_all_ones(
        &self,
        operand: Operand,
    ) -> bool {
        self.number(operand).is_some_and(|(bits, width)| bits == mask(width))
    }

    /// `m_SignMask`.
    fn is_sign_mask(
        &self,
        operand: Operand,
    ) -> bool {
        self.number(operand).is_some_and(|(bits, width)| bits == 1u128 << (width - 1))
    }

    /// `ConstantInt::get(ty, bits)`.
    fn int(
        &mut self,
        ty: TypeId,
        bits: u128,
    ) -> Operand {
        Operand::Constant(self.context.int(ty, bits as i128))
    }

    /// `Constant::getNullValue`.
    fn null(
        &mut self,
        ty: TypeId,
    ) -> Operand {
        self.int(ty, 0)
    }

    /// `Constant::getAllOnesValue`.
    fn all_ones(
        &mut self,
        ty: TypeId,
    ) -> Operand {
        let width = self.context.types.int_bits(ty).expect("an integer");
        self.int(ty, mask(width))
    }

    /// `PoisonValue::get`.
    fn poison(
        &mut self,
        ty: TypeId,
    ) -> Operand {
        Operand::Constant(self.context.constant(Constant { ty, kind: ConstantKind::Poison }))
    }

    /// The instruction that defines `operand`, if one in the body does.
    fn definition(
        &self,
        operand: Operand,
    ) -> Option<InstId> {
        let Operand::Value(value) = operand else { return None };
        let ValueDef::Instruction(inst) = self.function.value(value).def else { return None };
        self.function.parent(inst).map(|_| inst)
    }

    /// `dyn_cast<BinaryOperator>`: the opcode, operands and flags.
    fn binary(
        &self,
        operand: Operand,
    ) -> Option<(BinaryOp, Operand, Operand, Flags)> {
        let instruction = self.function.instruction(self.definition(operand)?);
        let Opcode::Binary(op) = instruction.opcode else { return None };
        let [a, b] = instruction.operands[..] else { return None };
        Some((op, a, b, instruction.flags))
    }

    /// `m_BinOp` of one opcode: its two operands and flags.
    fn binary_of(
        &self,
        operand: Operand,
        wanted: BinaryOp,
    ) -> Option<(Operand, Operand, Flags)> {
        self.binary(operand).filter(|&(op, ..)| op == wanted).map(|(_, a, b, flags)| (a, b, flags))
    }

    /// `m_PtrToInt(m_Value(X))`: an instruction or a constant expression.
    fn ptr_to_int_source(
        &self,
        operand: Operand,
    ) -> Option<Operand> {
        if let Some((CastOp::PtrToInt, source, _)) = self.cast(operand) {
            return Some(source);
        }
        let Operand::Constant(id) = operand else { return None };
        match &self.context.get(id).kind {
            ConstantKind::Expr(crate::context::ConstantExpr::Cast { op: CastOp::PtrToInt, value }) => {
                Some(Operand::Constant(*value))
            }
            _ => None,
        }
    }

    /// `m_Cast`: the opcode and source.
    fn cast(
        &self,
        operand: Operand,
    ) -> Option<(CastOp, Operand, Flags)> {
        let instruction = self.function.instruction(self.definition(operand)?);
        let Opcode::Cast(op) = instruction.opcode else { return None };
        Some((op, instruction.operands[0], instruction.flags))
    }

    /// `m_Not(m_Value(X))`: `xor X, -1` (either side).
    fn not_of(
        &self,
        operand: Operand,
    ) -> Option<Operand> {
        let (a, b, _) = self.binary_of(operand, BinaryOp::Xor)?;
        if self.is_all_ones(b) {
            Some(a)
        } else if self.is_all_ones(a) {
            Some(b)
        } else {
            None
        }
    }

    /// `m_Neg(m_Value(X))`: `sub 0, X`.
    fn neg_of(
        &self,
        operand: Operand,
    ) -> Option<(Operand, Flags)> {
        let (a, b, flags) = self.binary_of(operand, BinaryOp::Sub)?;
        self.is_zero(a).then_some((b, flags))
    }

    /// `m_c_BinOp(m_Specific(x), m_Value(Y))`: the other operand of a
    /// commutative `op` of `x`.
    fn other_of(
        &self,
        operand: Operand,
        wanted: BinaryOp,
        x: Operand,
    ) -> Option<Operand> {
        let (a, b, _) = self.binary_of(operand, wanted)?;
        if a == x {
            Some(b)
        } else if b == x {
            Some(a)
        } else {
            None
        }
    }

    /// A ValueTracking query over this body (`SimplifyQuery`).
    fn tracking(&self) -> ValueQuery<'_> {
        ValueQuery {
            globals: self.globals,
            dt: self.analyses.map(FunctionAnalyses::dominator_tree),
            assumptions: self.analyses.map(FunctionAnalyses::assumption_cache),
            dom_conditions: self.analyses.and_then(FunctionAnalyses::dom_conditions),
            cxt: self.cxt,
            ..ValueQuery::bare(self.context, self.function, self.layout)
        }
    }

    /// `computeKnownBits(V, Q)`.
    fn known(
        &self,
        operand: Operand,
    ) -> KnownBits {
        known::compute_known_bits(&self.tracking(), operand, 0)
    }

    /// `computeConstantRange(V, ForSigned, ...)`.
    fn range(
        &self,
        operand: Operand,
        for_signed: bool,
    ) -> ConstantRange {
        range::compute_constant_range(&self.tracking(), operand, for_signed, 0)
    }

    /// `isKnownNonZero(V, Q)`.
    fn is_known_non_zero(
        &self,
        operand: Operand,
    ) -> bool {
        nonzero::is_known_non_zero(&self.tracking(), operand, 0)
    }

    /// `isImpliedCondition(LHS, RHS, DL, LHSIsTrue)`.
    fn implied(
        &self,
        lhs: Operand,
        rhs: Operand,
        lhs_is_true: bool,
    ) -> Option<bool> {
        conditions::is_implied_condition(&self.tracking(), lhs, rhs, lhs_is_true, 0)
    }

    /// `isImpliedCondition(LHS, RHS, DL).value_or(false)`.
    fn is_implied(
        &self,
        lhs: Operand,
        rhs: Operand,
    ) -> bool {
        self.implied(lhs, rhs, true).unwrap_or(false)
    }

    /// `MaskedValueIsZero(V, Mask, Q)`.
    fn masked_value_is_zero(
        &self,
        operand: Operand,
        mask: u128,
    ) -> bool {
        known::masked_value_is_zero(&self.tracking(), operand, mask, 0)
    }

    /// `ComputeNumSignBits(V, DL, ...)`.
    fn num_sign_bits(
        &self,
        operand: Operand,
    ) -> u32 {
        signbits::compute_num_sign_bits(&self.tracking(), operand, 0)
    }

    /// `m_c_Or(m_NUWShl(m_Value(X), m_APInt(Amount)), m_Value(Y))`: X, the
    /// shifted value, the amount and Y.
    fn or_of_nuw_shl(
        &self,
        operand: Operand,
    ) -> Option<(Operand, Operand, u128, Operand)> {
        let (a, b, _) = self.binary_of(operand, BinaryOp::Or)?;
        [(a, b), (b, a)]
            .into_iter()
            .find_map(
                |(shifted, y)| {
                    let (x, amount, flags) = self.binary_of(shifted, BinaryOp::Shl)?;
                    let (amount, _) = self.number(amount)?;
                    flags.contains(Flags::NUW).then_some((x, shifted, amount, y))
                },
            )
    }

    /// `dyn_cast<ICmpInst>`: the predicate and the operands.
    fn icmp(
        &self,
        operand: Operand,
    ) -> Option<(IntPredicate, Operand, Operand)> {
        let instruction = self.function.instruction(self.definition(operand)?);
        let (Opcode::ICmp(pred), [a, b]) = (&instruction.opcode, &instruction.operands[..]) else { return None };
        Some((*pred, *a, *b))
    }

    /// `dyn_cast<SelectInst>`: the condition and the two arms.
    fn select_arms(
        &self,
        operand: Operand,
    ) -> Option<(Operand, Operand, Operand)> {
        let instruction = self.function.instruction(self.definition(operand)?);
        let (Opcode::Select, [cond, if_true, if_false]) = (&instruction.opcode, &instruction.operands[..]) else {
            return None;
        };
        Some((*cond, *if_true, *if_false))
    }

    /// `isa<SelectInst>`.
    fn is_select(
        &self,
        operand: Operand,
    ) -> bool {
        self.select_arms(operand).is_some()
    }

    /// `isa<PHINode>`.
    fn is_phi(
        &self,
        operand: Operand,
    ) -> bool {
        self.definition(operand).is_some_and(|inst| self.function.instruction(inst).opcode == Opcode::Phi)
    }

    /// `valueDominatesPHI`
    /// ([:176](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L176)):
    /// whether `v` dominates the phi `phi`.
    fn value_dominates_phi(
        &self,
        v: Operand,
        phi: InstId,
    ) -> bool {
        // Arguments and constants dominate all instructions.
        let Some(def) = self.definition(v) else { return true };
        // If we have a DominatorTree then do a precise test.
        if let Some(analyses) = self.analyses {
            return analyses.dominator_tree().instruction_dominates(self.function, def, phi);
        }
        // Otherwise, if the instruction is in the entry block and is not an
        // invoke, then it obviously dominates all phi nodes.
        self.function.parent(def) == self.function.entry()
            && !matches!(self.function.instruction(def).opcode, Opcode::Invoke(_))
    }

    /// `isKnownToBeAPowerOfTwo(V, DL, OrZero, ...)`.
    fn is_power_of_two(
        &self,
        operand: Operand,
        or_zero: bool,
    ) -> bool {
        power2::is_known_to_be_a_power_of_two(&self.tracking(), operand, or_zero, 0)
    }

    /// `match(C, m_LowBitMask())`: `2^n - 1`, n > 0.
    fn is_low_bit_mask(
        &self,
        operand: Operand,
    ) -> bool {
        self.number(operand).is_some_and(|(bits, _)| bits != 0 && (bits & bits.wrapping_add(1)) == 0)
    }
}

/// `isKnownNegation` (ValueTracking.cpp): `x` is `-y` or `y` is `-x`, as `sub
/// 0, _` (with `nsw` where `need_nsw`).
fn is_known_negation(
    q: &Query,
    x: Operand,
    y: Operand,
    need_nsw: bool,
) -> bool {
    let negates = |value: Operand, of: Operand| {
        q.neg_of(value).is_some_and(|(inner, flags)| inner == of && (!need_nsw || flags.contains(Flags::NSW)))
    };
    negates(x, y) || negates(y, x)
}

/// `foldOrCommuteConstant`
/// ([:566](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L566)).
fn fold_or_commute_constant(
    q: &mut Query,
    op: BinaryOp,
    a: &mut Operand,
    b: &mut Operand,
) -> Option<Operand> {
    if let Some(left) = q.constant(*a) {
        if let Some(right) = q.constant(*b) {
            return constfold::binary(q.context, op, left, right).map(Operand::Constant);
        }
        // Canonicalize the constant to the RHS if this is a commutative
        // operation.
        if matches!(
            op,
            BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor
        ) {
            std::mem::swap(a, b);
        }
    }
    None
}

fn is_commutative(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor
    )
}

/// `expandBinOp`
/// ([:165](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L165)):
/// `V op OtherOp` where `V` is `B0 expand B1`, as `(B0 op OtherOp) expand (B1
/// op OtherOp)` if both parts simplify.
fn expand_bin_op(
    q: &mut Query,
    opcode: BinaryOp,
    v: Operand,
    other_op: Operand,
    opcode_to_expand: BinaryOp,
    max_recurse: u32,
) -> Option<Operand> {
    let (b0, b1, _) = q.binary_of(v, opcode_to_expand)?;
    // Evaluate the expression "B0 op OtherOp" and "B1 op OtherOp",
    // `Q.getWithoutUndef()`.
    let could_use_undef = std::mem::replace(&mut q.can_use_undef, false);
    let l = simplify_binop(q, opcode, b0, other_op, max_recurse);
    let r = l.and_then(|_| simplify_binop(q, opcode, b1, other_op, max_recurse));
    q.can_use_undef = could_use_undef;
    let (l, r) = (l?, r?);
    // Does "(B0 op OtherOp) expand (B1 op OtherOp)" equal B?
    if (l == b0 && r == b1) || (is_commutative(opcode_to_expand) && l == b1 && r == b0) {
        return Some(v);
    }
    simplify_binop(q, opcode_to_expand, l, r, max_recurse)
}

/// `expandCommutativeBinOp`
/// ([:232](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L232)).
fn expand_commutative_bin_op(
    q: &mut Query,
    opcode: BinaryOp,
    l: Operand,
    r: Operand,
    opcode_to_expand: BinaryOp,
    mut max_recurse: u32,
) -> Option<Operand> {
    if max_recurse == 0 {
        return None;
    }
    max_recurse -= 1;
    if let Some(v) = expand_bin_op(q, opcode, l, r, opcode_to_expand, max_recurse) {
        return Some(v);
    }
    expand_bin_op(q, opcode, r, l, opcode_to_expand, max_recurse)
}

/// `threadBinOpOverSelect`
/// ([:354](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L354)):
/// `lhs op rhs` where one is a select, by evaluating it on both arms.
fn thread_bin_op_over_select(
    q: &mut Query,
    opcode: BinaryOp,
    lhs: Operand,
    rhs: Operand,
    mut max_recurse: u32,
) -> Option<Operand> {
    // Recursion is always used, so bail out at once if we already hit the
    // limit.
    if max_recurse == 0 {
        return None;
    }
    max_recurse -= 1;
    let si = if q.is_select(lhs) { lhs } else { rhs };
    let (_, true_val, false_val) = q.select_arms(si)?;
    // Evaluate the BinOp on the true and false branches of the select.
    let (tv, fv) = if si == lhs {
        (simplify_binop(q, opcode, true_val, rhs, max_recurse), simplify_binop(q, opcode, false_val, rhs, max_recurse))
    } else {
        (simplify_binop(q, opcode, lhs, true_val, max_recurse), simplify_binop(q, opcode, lhs, false_val, max_recurse))
    };
    // If they simplified to the same value, then return the common value. If
    // they both failed to simplify then return null.
    if tv == fv {
        return tv;
    }
    // If one branch simplified to undef, return the other one.
    if tv.is_some_and(|one| q.is_undef(one)) {
        return fv;
    }
    if fv.is_some_and(|one| q.is_undef(one)) {
        return tv;
    }
    // If applying the operation did not change the true and false select
    // values, then the result of the binop is the select itself.
    if tv == Some(true_val) && fv == Some(false_val) {
        return Some(si);
    }
    // If one branch simplified and the other did not, and the simplified value
    // is equal to the unsimplified one, return the simplified value. For
    // example, select (cond, X, X & Z) & Z -> X & Z.
    if tv.is_some() != fv.is_some() {
        let simplified = fv.or(tv)?;
        // The simplified value has the form "X op Y", `op` as the original
        // operation, and no poison-generating flag.
        let (op, a, b, flags) = q.binary(simplified)?;
        let poison_generating = [Flags::NUW, Flags::NSW, Flags::EXACT, Flags::DISJOINT];
        if op == opcode && !poison_generating.iter().any(|&flag| flags.contains(flag)) {
            // The value that didn't simplify is "UnsimplifiedLHS op
            // UnsimplifiedRHS".
            let unsimplified_branch = if fv.is_some() { true_val } else { false_val };
            let unsimplified_lhs = if si == lhs { unsimplified_branch } else { lhs };
            let unsimplified_rhs = if si == lhs { rhs } else { unsimplified_branch };
            if a == unsimplified_lhs && b == unsimplified_rhs {
                return Some(simplified);
            }
            if is_commutative(op) && b == unsimplified_lhs && a == unsimplified_rhs {
                return Some(simplified);
            }
        }
    }
    None
}

/// `isSameCompare`
/// ([:98](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L98)):
/// whether `v` is the compare `lhs pred rhs`.
fn is_same_compare(
    q: &Query,
    v: Operand,
    pred: IntPredicate,
    lhs: Operand,
    rhs: Operand,
) -> bool {
    let Some(inst) = q.definition(v) else { return false };
    let cmp = q.function.instruction(inst);
    let Opcode::ICmp(cmp_pred) = cmp.opcode else { return false };
    let [cmp_lhs, cmp_rhs] = cmp.operands[..] else { return false };
    (cmp_pred == pred && cmp_lhs == lhs && cmp_rhs == rhs)
        || (cmp_pred == swapped(pred) && cmp_lhs == rhs && cmp_rhs == lhs)
}

/// `simplifyCmpSelCase`
/// ([:111](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L111)),
/// with `simplifyCmpSelTrueCase` and `simplifyCmpSelFalseCase`
/// (`true_or_false` is their constant).
fn simplify_cmp_sel_case(
    q: &mut Query,
    pred: IntPredicate,
    lhs: Operand,
    rhs: Operand,
    cond: Operand,
    max_recurse: u32,
    true_or_false: bool,
) -> Option<Operand> {
    let simplified = simplify_icmp(q, pred, lhs, rhs, max_recurse);
    if simplified == Some(cond) {
        // %cmp simplified to the select condition (%cond).
        let ty = q.ty(cond);
        return Some(q.int(ty, u128::from(true_or_false)));
    }
    if simplified.is_none() && is_same_compare(q, cond, pred, lhs, rhs) {
        // It didn't simplify. However, if composed comparison is equivalent to
        // the select condition (%cond) then we can replace it.
        let ty = q.ty(cond);
        return Some(q.int(ty, u128::from(true_or_false)));
    }
    simplified
}

/// `handleOtherCmpSelSimplifications`
/// ([:150](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L150)).
fn handle_other_cmp_sel_simplifications(
    q: &mut Query,
    t_cmp: Operand,
    f_cmp: Operand,
    cond: Operand,
    max_recurse: u32,
) -> Option<Operand> {
    // If the false value simplified to false, then the result of the compare
    // is equal to "Cond && TCmp". Folding select to and/or isn't poison-safe in
    // general; impliesPoison checks it does not convert a well-defined value
    // into poison.
    if q.is_zero(f_cmp)
        && guarantee::implies_poison(&q.tracking(), t_cmp, cond)
        && let Some(v) = simplify_and(q, cond, t_cmp, max_recurse)
    {
        return Some(v);
    }
    // If the true value simplified to true, then the result of the compare is
    // equal to "Cond || FCmp".
    if q.is_one(t_cmp)
        && guarantee::implies_poison(&q.tracking(), f_cmp, cond)
        && let Some(v) = simplify_or(q, cond, f_cmp, max_recurse)
    {
        return Some(v);
    }
    // Finally, if the false value simplified to true and the true value to
    // false, then the result of the compare is equal to "!Cond".
    if q.is_one(f_cmp) && q.is_zero(t_cmp) {
        let ty = q.ty(cond);
        let all_ones = q.all_ones(ty);
        return simplify_xor(q, cond, all_ones, max_recurse);
    }
    None
}

/// `threadCmpOverSelect`
/// ([:433](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L433)).
fn thread_cmp_over_select(
    q: &mut Query,
    mut pred: IntPredicate,
    mut lhs: Operand,
    mut rhs: Operand,
    mut max_recurse: u32,
) -> Option<Operand> {
    // Recursion is always used, so bail out at once if we already hit the
    // limit.
    if max_recurse == 0 {
        return None;
    }
    max_recurse -= 1;
    // Make sure the select is on the LHS.
    if !q.is_select(lhs) {
        std::mem::swap(&mut lhs, &mut rhs);
        pred = swapped(pred);
    }
    let (cond, true_val, false_val) = q.select_arms(lhs)?;
    // Now that we have "cmp select(Cond, TV, FV), RHS", analyse it. Does "cmp
    // TV, RHS" simplify?
    let t_cmp = simplify_cmp_sel_case(q, pred, true_val, rhs, cond, max_recurse, true)?;
    // Does "cmp FV, RHS" simplify?
    let f_cmp = simplify_cmp_sel_case(q, pred, false_val, rhs, cond, max_recurse, false)?;
    // If both sides simplified to the same value, then use it as the result of
    // the original comparison.
    if t_cmp == f_cmp {
        return Some(t_cmp);
    }
    handle_other_cmp_sel_simplifications(q, t_cmp, f_cmp, cond, max_recurse)
}

/// The incoming `(value, block terminator)` pairs of the phi `phi`.
fn phi_incoming(
    q: &Query,
    phi: InstId,
) -> Vec<(Operand, Option<InstId>)> {
    q.function
        .instruction(phi)
        .operands
        .chunks(2)
        .map(|pair| {
            let terminator = match pair[1] {
                Operand::Block(block) => q.function.terminator(block),
                _ => None,
            };
            (pair[0], terminator)
        })
        .collect()
}

/// `threadBinOpOverPHI`
/// ([:478](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L478)).
fn thread_bin_op_over_phi(
    q: &mut Query,
    opcode: BinaryOp,
    lhs: Operand,
    rhs: Operand,
    mut max_recurse: u32,
) -> Option<Operand> {
    // Recursion is always used, so bail out at once if we already hit the
    // limit.
    if max_recurse == 0 {
        return None;
    }
    max_recurse -= 1;
    let pi = if q.is_phi(lhs) { lhs } else { rhs };
    let phi = q.definition(pi)?;
    // Bail out if the other operand and the phi may be mutually interdependent
    // due to a loop.
    if !q.value_dominates_phi(if pi == lhs { rhs } else { lhs }, phi) {
        return None;
    }
    // Evaluate the BinOp on the incoming phi values.
    let mut common_value = None;
    for (incoming, terminator) in phi_incoming(q, phi) {
        // If the incoming value is the phi node itself, it can safely be
        // skipped.
        if incoming == pi {
            continue;
        }
        let saved = std::mem::replace(&mut q.cxt, terminator);
        let v = if pi == lhs {
            simplify_binop(q, opcode, incoming, rhs, max_recurse)
        } else {
            simplify_binop(q, opcode, lhs, incoming, max_recurse)
        };
        q.cxt = saved;
        // If the operation failed to simplify, or simplified to a different
        // value to previously, then give up.
        let v = v?;
        if common_value.is_some_and(|common| common != v) {
            return None;
        }
        common_value = Some(v);
    }
    common_value
}

/// `threadCmpOverPHI`
/// ([:525](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L525)).
fn thread_cmp_over_phi(
    q: &mut Query,
    mut pred: IntPredicate,
    mut lhs: Operand,
    mut rhs: Operand,
    mut max_recurse: u32,
) -> Option<Operand> {
    // Recursion is always used, so bail out at once if we already hit the
    // limit.
    if max_recurse == 0 {
        return None;
    }
    max_recurse -= 1;
    // Make sure the phi is on the LHS.
    if !q.is_phi(lhs) {
        std::mem::swap(&mut lhs, &mut rhs);
        pred = swapped(pred);
    }
    let phi = q.definition(lhs)?;
    // Bail out if RHS and the phi may be mutually interdependent due to a loop.
    if !q.value_dominates_phi(rhs, phi) {
        return None;
    }
    // Evaluate the compare on the incoming phi values.
    let mut common_value = None;
    for (incoming, terminator) in phi_incoming(q, phi) {
        // If the incoming value is the phi node itself, it can safely be
        // skipped.
        if incoming == lhs {
            continue;
        }
        // Change the context instruction to the "edge" that flows into the phi:
        // that is where incoming is actually "evaluated".
        let saved = std::mem::replace(&mut q.cxt, terminator);
        let v = simplify_icmp(q, pred, incoming, rhs, max_recurse);
        q.cxt = saved;
        let v = v?;
        if common_value.is_some_and(|common| common != v) {
            return None;
        }
        common_value = Some(v);
    }
    common_value
}

/// `simplifyByDomEq`
/// ([:712](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L712)):
/// `op0 opcode op1` where a dominating condition says the operands are equal.
fn simplify_by_dom_eq(
    q: &mut Query,
    opcode: BinaryOp,
    op0: Operand,
    op1: Operand,
    max_recurse: u32,
) -> Option<Operand> {
    // Recursive run it can not get any benefit
    if max_recurse != RECURSION_LIMIT {
        return None;
    }
    let cxt = q.cxt?;
    if conditions::is_implied_by_dom_condition(&q.tracking(), IntPredicate::Eq, op0, op1, cxt) != Some(true) {
        return None;
    }
    let ty = q.ty(op0);
    match opcode {
        BinaryOp::Sub | BinaryOp::Xor | BinaryOp::URem | BinaryOp::SRem => Some(q.null(ty)),
        BinaryOp::SDiv | BinaryOp::UDiv => Some(q.int(ty, 1)),
        // Could be either one - choose Op1 since that's more likely a constant.
        BinaryOp::And | BinaryOp::Or => Some(op1),
        _ => None,
    }
}

/// `simplifyWithOpsReplaced`
/// ([:4429](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L4429))
/// for one replacement, `AllowRefinement` and no `DropFlags`: what `v` equals
/// with `rep_op` in place of `op`.
fn simplify_with_op_replaced(
    q: &mut Query,
    v: Operand,
    op: Operand,
    rep_op: Operand,
    max_recurse: u32,
) -> Option<Operand> {
    simplify_with_ops_replaced(q, v, &[(op, rep_op)], true, max_recurse)
}

/// `getBinOpIdentity(Opcode, Ty, AllowRHSConstant)`, as a number: the left
/// identity of a commutative operation, the right one of any.
fn bin_op_identity(
    opcode: BinaryOp,
    rhs: bool,
    width: u32,
) -> Option<u128> {
    match opcode {
        BinaryOp::Add | BinaryOp::Or | BinaryOp::Xor => Some(0),
        BinaryOp::Mul => Some(1),
        BinaryOp::And => Some(mask(width)),
        BinaryOp::Sub | BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr if rhs => Some(0),
        BinaryOp::UDiv | BinaryOp::SDiv if rhs => Some(1),
        _ => None,
    }
}

/// `getBinOpAbsorber(Opcode, Ty)`, as a number.
fn bin_op_absorber(
    opcode: BinaryOp,
    width: u32,
) -> Option<u128> {
    match opcode {
        BinaryOp::Mul | BinaryOp::And => Some(0),
        BinaryOp::Or => Some(mask(width)),
        _ => None,
    }
}

/// `simplifyWithOpsReplaced`
/// ([:4429](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L4429)):
/// what `v` equals with each `ops.0` in place of `ops.1`, a refinement of it
/// where `allow_refinement`. DEVIATE: `DropFlags` is always null; a flag that
/// would be dropped ends the fold. Vector operands are not modelled.
fn simplify_with_ops_replaced(
    q: &mut Query,
    v: Operand,
    ops: &[(Operand, Operand)],
    allow_refinement: bool,
    mut max_recurse: u32,
) -> Option<Operand> {
    for &(op, rep_op) in ops {
        // We cannot replace a constant, and shouldn't even try.
        q.constant(op).is_none().then_some(())?;
        // Trivial replacement.
        if v == op {
            return Some(rep_op);
        }
    }
    if max_recurse == 0 {
        return None;
    }
    max_recurse -= 1;
    let inst = q.definition(v)?;
    // The arguments of a phi node might refer to a value from a previous cycle
    // iteration. Don't simplify freeze.
    if matches!(
        q.function.instruction(inst).opcode,
        Opcode::Phi | Opcode::Freeze
    ) {
        return None;
    }
    let intrinsic = pattern::call_intrinsic(&q.tracking(), inst);
    if q.tracking().callee_name(inst).is_some_and(|name| name.starts_with("llvm.is.constant")) {
        return None;
    }
    // Replace the ops in the instruction's operands.
    let mut new_ops = Vec::new();
    let mut any_replaced = false;
    for inst_op in q.function.instruction(inst).operands.to_vec() {
        match simplify_with_ops_replaced(q, inst_op, ops, allow_refinement, max_recurse) {
            Some(new_inst_op) => {
                new_ops.push(new_inst_op);
                any_replaced = inst_op != new_inst_op;
            }
            None => new_ops.push(inst_op),
        }
        let last = *new_ops.last().expect("an operand");
        if !q.can_use_undef
            && q.constant(last)
                .is_some_and(|id| matches!(
                    q.context.get(id).kind,
                    ConstantKind::Undef | ConstantKind::Poison
                ))
        {
            return None;
        }
    }
    if !any_replaced {
        return None;
    }
    let instruction = q.function.instruction(inst);
    let (opcode, ty, flags) = (instruction.opcode.clone(), instruction.ty, instruction.flags);
    if !allow_refinement {
        // Without DropFlags, a fold that needs the flags dropped is no fold.
        if let Opcode::Binary(binop) = opcode
            && q.context.types.int_bits(ty).is_some()
        {
            let width = q.context.types.int_bits(ty)?;
            let number = |q: &Query, operand: Operand| q.number(operand).map(|(bits, _)| bits);
            if bin_op_identity(binop, false, width).is_some_and(|id| number(q, new_ops[0]) == Some(id)) {
                return Some(new_ops[1]);
            }
            if bin_op_identity(binop, true, width).is_some_and(|id| number(q, new_ops[1]) == Some(id)) {
                return Some(new_ops[0]);
            }
            if matches!(binop, BinaryOp::And | BinaryOp::Or) && new_ops[0] == new_ops[1] {
                if flags.contains(Flags::DISJOINT) {
                    return None;
                }
                return Some(new_ops[0]);
            }
            if matches!(binop, BinaryOp::Sub | BinaryOp::Xor)
                && new_ops[0] == new_ops[1]
                && ops.iter().any(|&(_, rep)| rep == new_ops[0])
            {
                return Some(q.null(ty));
            }
            if let Some(absorber) = bin_op_absorber(binop, width)
                && (number(q, new_ops[0]) == Some(absorber) || number(q, new_ops[1]) == Some(absorber))
                && let Some(result) = q.function.instruction(inst).result
                && ops.iter().any(|&(first, _)| guarantee::implies_poison(&q.tracking(), Operand::Value(result), first))
            {
                return Some(q.int(ty, absorber));
            }
        }
        if let Some(id) = intrinsic {
            let annotated = {
                let Opcode::Call(info) = &q.function.instruction(inst).opcode else { return None };
                info.return_attrs.iter().any(|one| matches!(one, crate::opcode::Attribute::Range { .. }))
                    || q.function.instruction(inst).metadata.iter().any(|(kind, _)| kind == "range")
            };
            if matches!(id, pattern::IntrinsicId::Scmp | pattern::IntrinsicId::Ucmp) && new_ops[0] == new_ops[1] {
                if annotated {
                    return None;
                }
                return Some(q.int(ty, 0));
            }
            if calls::min_max_identity(id, q.width(new_ops[0])).is_some() {
                let identity = calls::min_max_identity(id, q.width(new_ops[0]))?;
                let result = if q.number(new_ops[0]).is_some_and(|(bits, _)| bits == identity) {
                    Some(new_ops[1])
                } else if q.number(new_ops[1]).is_some_and(|(bits, _)| bits == identity) {
                    Some(new_ops[0])
                } else {
                    None
                };
                if let Some(result) = result {
                    if annotated {
                        return None;
                    }
                    return Some(result);
                }
            }
        }
        if matches!(opcode, Opcode::GetElementPtr { .. }) && new_ops.len() == 2 && q.is_zero(new_ops[1]) {
            return Some(new_ops[0]);
        }
    } else {
        // Prevent self-simplification.
        return simplify_instruction_with_operands(q, inst, &new_ops, max_recurse)
            .filter(|&simplified| simplified != v);
    }
    let constants = new_ops.iter().map(|&one| q.constant(one)).collect::<Option<Vec<_>>>()?;
    let abs = intrinsic == Some(pattern::IntrinsicId::Abs);
    if guarantee::can_create_poison(&q.tracking(), guarantee::Operator::Instruction(inst), true) {
        let not_min = q.number(new_ops[0]).is_some_and(|(bits, width)| bits != 1u128 << (width - 1));
        if !(abs && not_min) {
            return None;
        }
    }
    crate::constantfolding::constant_fold_inst_operands_at(
        &*q.context,
        crate::constantfolding::Env { globals: q.globals, layout: q.layout },
        q.function,
        inst,
        &constants,
    )
    .map(Operand::Constant)
}

/// `m_ICmpLike`: the predicate and operands of an `icmp`, or of a `trunc nuw`
/// to `i1` as `icmp ne x, 0`.
fn icmp_like(
    q: &mut Query,
    v: Operand,
) -> Option<(IntPredicate, Operand, Operand)> {
    let inst = q.definition(v)?;
    let instruction = q.function.instruction(inst);
    if let (Opcode::ICmp(pred), [a, b]) = (&instruction.opcode, &instruction.operands[..]) {
        return Some((*pred, *a, *b));
    }
    if q.width(v) == Some(1)
        && let Some((CastOp::Trunc, x, flags)) = q.cast(v)
        && flags.contains(Flags::NUW)
    {
        let ty = q.ty(x);
        let zero = q.null(ty);
        return Some((IntPredicate::Ne, x, zero));
    }
    None
}

/// `simplifyAndOrWithICmpEq`
/// ([:1945](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L1945)).
fn simplify_and_or_with_icmp_eq(
    q: &mut Query,
    opcode: BinaryOp,
    op0: Operand,
    op1: Operand,
    max_recurse: u32,
) -> Option<Operand> {
    let (pred, a, b) = icmp_like(q, op0)?;
    if !is_equality(pred) {
        return None;
    }
    let simplify = |q: &mut Query, res: Operand| -> Option<Operand> {
        let ty = q.ty(res);
        // The absorber of and is 0 and of or is -1; the identity the reverse.
        let (absorber, identity) =
            if opcode == BinaryOp::And { (q.null(ty), q.all_ones(ty)) } else { (q.all_ones(ty), q.null(ty)) };
        // and (icmp eq a, b), x implies (a==b) inside x; or (icmp ne a, b), x
        // likewise. If x simplifies to true/false, we can simplify the and/or.
        if pred == if opcode == BinaryOp::And { IntPredicate::Eq } else { IntPredicate::Ne } {
            if res == absorber {
                return Some(absorber);
            }
            if res == identity {
                return Some(op0);
            }
            return None;
        }
        // If we have and (icmp ne a, b), x and for a==b we can simplify x to
        // false, then we can drop the icmp, as x will already be false in the
        // case where the icmp is false. Similar for or and true.
        if res == absorber {
            return Some(op1);
        }
        None
    };
    if let Some(res) = simplify_with_op_replaced(q, op1, a, b, max_recurse) {
        return simplify(q, res);
    }
    if let Some(res) = simplify_with_op_replaced(q, op1, b, a, max_recurse) {
        return simplify(q, res);
    }
    None
}

/// `m_c_ICmp(Pred, m_Specific(a), m_Specific(b))`: the predicate of the compare
/// of `a` and `b`, in that order.
fn icmp_of_pair(
    q: &mut Query,
    v: Operand,
    a: Operand,
    b: Operand,
) -> Option<IntPredicate> {
    let (pred, l, r) = q.icmp(v)?;
    if (l, r) == (a, b) {
        Some(pred)
    } else if (l, r) == (b, a) {
        Some(swapped(pred))
    } else {
        None
    }
}

/// `simplifyUnsignedRangeCheck`
/// ([:1512](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L1512)).
/// Commuted variants are assumed to be handled by calling this function again
/// with the parameters swapped.
fn simplify_unsigned_range_check(
    q: &mut Query,
    zero_icmp: Operand,
    unsigned_icmp: Operand,
    is_and: bool,
) -> Option<Operand> {
    let i1 = q.context.types.int(1);
    let (eq_pred, y, zero) = q.icmp(zero_icmp)?;
    if !q.is_zero(zero) || !is_equality(eq_pred) {
        return None;
    }
    let mut unsigned_pred;
    // Y = (A - B);
    if let Some((a, b, _)) = q.binary_of(y, BinaryOp::Sub) {
        if let Some(pred) = icmp_of_pair(q, unsigned_icmp, a, b)
            && is_unsigned(pred)
        {
            unsigned_pred = pred;
            // A >=/<= B || (A - B) != 0  <-->  true
            if matches!(unsigned_pred, IntPredicate::Uge | IntPredicate::Ule) && eq_pred == IntPredicate::Ne && !is_and
            {
                return Some(q.int(i1, 1));
            }
            // A </> B && (A - B) == 0  <-->  false
            if matches!(unsigned_pred, IntPredicate::Ult | IntPredicate::Ugt) && eq_pred == IntPredicate::Eq && is_and {
                return Some(q.null(i1));
            }
            // A </> B && (A - B) != 0  <-->  A </> B
            // A </> B || (A - B) != 0  <-->  (A - B) != 0
            if eq_pred == IntPredicate::Ne && matches!(unsigned_pred, IntPredicate::Ult | IntPredicate::Ugt) {
                return Some(if is_and { unsigned_icmp } else { zero_icmp });
            }
            // A <=/>= B && (A - B) == 0  <-->  (A - B) == 0
            // A <=/>= B || (A - B) == 0  <-->  A <=/>= B
            if eq_pred == IntPredicate::Eq && matches!(unsigned_pred, IntPredicate::Ule | IntPredicate::Uge) {
                return Some(if is_and { zero_icmp } else { unsigned_icmp });
            }
        }
        // Given Y = (A - B)
        //   Y >= A && Y != 0  --> Y >= A  iff B != 0
        //   Y <  A || Y == 0  --> Y <  A  iff B != 0
        if let Some(pred) = icmp_of_pair(q, unsigned_icmp, y, a) {
            if pred == IntPredicate::Uge && is_and && eq_pred == IntPredicate::Ne && q.is_known_non_zero(b) {
                return Some(unsigned_icmp);
            }
            if pred == IntPredicate::Ult && !is_and && eq_pred == IntPredicate::Eq && q.is_known_non_zero(b) {
                return Some(unsigned_icmp);
            }
        }
    }
    let x;
    match q.icmp(unsigned_icmp) {
        Some((pred, l, r)) if r == y && is_unsigned(pred) => {
            x = l;
            unsigned_pred = pred;
        }
        Some((pred, l, r)) if l == y && is_unsigned(swapped(pred)) => {
            x = r;
            unsigned_pred = swapped(pred);
        }
        _ => return None,
    }
    // X > Y && Y == 0  -->  Y == 0  iff X != 0
    // X > Y || Y == 0  -->  X > Y   iff X != 0
    if unsigned_pred == IntPredicate::Ugt && eq_pred == IntPredicate::Eq && q.is_known_non_zero(x) {
        return Some(if is_and { zero_icmp } else { unsigned_icmp });
    }
    // X <= Y && Y != 0  -->  X <= Y  iff X != 0
    // X <= Y || Y != 0  -->  Y != 0  iff X != 0
    if unsigned_pred == IntPredicate::Ule && eq_pred == IntPredicate::Ne && q.is_known_non_zero(x) {
        return Some(if is_and { unsigned_icmp } else { zero_icmp });
    }
    // X < Y && Y != 0  -->  X < Y
    // X < Y || Y != 0  -->  Y != 0
    if unsigned_pred == IntPredicate::Ult && eq_pred == IntPredicate::Ne {
        return Some(if is_and { unsigned_icmp } else { zero_icmp });
    }
    // X >= Y && Y == 0  -->  Y == 0
    // X >= Y || Y == 0  -->  X >= Y
    if unsigned_pred == IntPredicate::Uge && eq_pred == IntPredicate::Eq {
        return Some(if is_and { zero_icmp } else { unsigned_icmp });
    }
    // X < Y && Y == 0  -->  false
    if unsigned_pred == IntPredicate::Ult && eq_pred == IntPredicate::Eq && is_and {
        return Some(q.null(i1));
    }
    // X >= Y || Y != 0  -->  true
    if unsigned_pred == IntPredicate::Uge && eq_pred == IntPredicate::Ne && !is_and {
        return Some(q.int(i1, 1));
    }
    None
}

/// `simplifyAndOrOfICmpsWithConstants`
/// ([:1621](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L1621)):
/// a pair of compares with a shared operand and 2 constants has an empty set
/// intersection, full set union, or one compare is a superset of the other.
fn simplify_and_or_of_icmps_with_constants(
    q: &mut Query,
    cmp0: Operand,
    cmp1: Operand,
    is_and: bool,
) -> Option<Operand> {
    let i1 = q.context.types.int(1);
    let (pred0, x0, c0) = q.icmp(cmp0)?;
    let (pred1, x1, c1) = q.icmp(cmp1)?;
    // Look for this pattern: {and/or} (icmp X, C0), (icmp X, C1)).
    if x0 != x1 {
        return None;
    }
    let ((c0, width), (c1, _)) = (q.number(c0)?, q.number(c1)?);
    let range0 = ConstantRange::make_exact_icmp_region(pred0, width, c0);
    let range1 = ConstantRange::make_exact_icmp_region(pred1, width, c1);
    // For and-of-compares, check if the intersection is empty.
    if is_and && range0.intersect_with(&range1, PreferredRangeType::Smallest).is_empty_set() {
        return Some(q.null(i1));
    }
    // For or-of-compares, check if the union is full.
    if !is_and && range0.union_with(&range1, PreferredRangeType::Smallest).is_full_set() {
        return Some(q.int(i1, 1));
    }
    // Is one range a superset of the other? If this is and-of-compares, take
    // the smaller set; or-of-compares, the larger.
    if range0.contains_range(&range1) {
        return Some(if is_and { cmp1 } else { cmp0 });
    }
    if range1.contains_range(&range0) {
        return Some(if is_and { cmp0 } else { cmp1 });
    }
    None
}

/// `simplifyAndOfICmpsWithAdd` (:1658) and `simplifyOrOfICmpsWithAdd`
/// (:1757), which differ only in the predicates and the answer.
fn simplify_and_or_of_icmps_with_add(
    q: &mut Query,
    op0: Operand,
    op1: Operand,
    is_and: bool,
) -> Option<Operand> {
    let i1 = q.context.types.int(1);
    // (icmp (add V, C0), C1) & (icmp V, C0)
    let (pred0, add, c1) = q.icmp(op0)?;
    let (v, c0, add_flags) = q.binary_of(add, BinaryOp::Add)?;
    let ((c0_bits, width), (c1_bits, _)) = (q.number(c0)?, q.number(c1)?);
    let (pred1, v1, rhs1) = q.icmp(op1)?;
    if v1 != v || c0 != rhs1 {
        return None;
    }
    let (is_nsw, is_nuw) = (add_flags.contains(Flags::NSW), add_flags.contains(Flags::NUW));
    let delta = c1_bits.wrapping_sub(c0_bits) & mask(width);
    // The answer is false for and, true for or.
    use IntPredicate::*;
    let (p2_unsigned, p2_signed, p1_unsigned, p1_signed, signed_b, unsigned_b) =
        if is_and { (Ult, Slt, Ule, Sle, Sgt, Ugt) } else { (Uge, Sge, Ugt, Sgt, Sle, Ule) };
    let answer = |q: &mut Query| Some(q.int(i1, u128::from(!is_and)));
    if signed(c0_bits, width) > 0 {
        if delta == 2 {
            if pred0 == p2_unsigned && pred1 == signed_b {
                return answer(q);
            }
            if pred0 == p2_signed && pred1 == signed_b && is_nsw {
                return answer(q);
            }
        }
        if delta == 1 {
            if pred0 == p1_unsigned && pred1 == signed_b {
                return answer(q);
            }
            if pred0 == p1_signed && pred1 == signed_b && is_nsw {
                return answer(q);
            }
        }
    }
    if c0_bits != 0 && is_nuw {
        if delta == 2 && pred0 == p2_unsigned && pred1 == unsigned_b {
            return answer(q);
        }
        if delta == 1 && pred0 == p1_unsigned && pred1 == unsigned_b {
            return answer(q);
        }
    }
    None
}

/// `simplifyAndOrOfICmpsWithCtpop`
/// ([:1706](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L1706)).
fn simplify_and_or_of_icmps_with_ctpop(
    q: &mut Query,
    cmp0: Operand,
    cmp1: Operand,
    is_and: bool,
) -> Option<Operand> {
    let (pred0, ctpop, c) = q.icmp(cmp0)?;
    let tracking = q.tracking();
    let (_, args) = pattern::intrinsic_call(&tracking, ctpop, "llvm.ctpop")?;
    let [x] = *args else { return None };
    let (pred1, x1, zero) = q.icmp(cmp1)?;
    if x1 != x || !q.is_zero(zero) || q.number(c).is_none_or(|(bits, _)| bits == 0) {
        return None;
    }
    // (ctpop(X) == C) || (X != 0) --> X != 0 where C > 0
    if !is_and && pred0 == IntPredicate::Eq && pred1 == IntPredicate::Ne {
        return Some(cmp1);
    }
    // (ctpop(X) != C) && (X == 0) --> X == 0 where C > 0
    if is_and && pred0 == IntPredicate::Ne && pred1 == IntPredicate::Eq {
        return Some(cmp1);
    }
    None
}

/// `simplifyAndOfICmps` (:1725) and `simplifyOrOfICmps` (:1799).
fn simplify_and_or_of_icmps(
    q: &mut Query,
    op0: Operand,
    op1: Operand,
    is_and: bool,
) -> Option<Operand> {
    if let Some(x) = simplify_unsigned_range_check(q, op0, op1, is_and) {
        return Some(x);
    }
    if let Some(x) = simplify_unsigned_range_check(q, op1, op0, is_and) {
        return Some(x);
    }
    if let Some(x) = simplify_and_or_of_icmps_with_constants(q, op0, op1, is_and) {
        return Some(x);
    }
    if let Some(x) = simplify_and_or_of_icmps_with_ctpop(q, op0, op1, is_and) {
        return Some(x);
    }
    if let Some(x) = simplify_and_or_of_icmps_with_ctpop(q, op1, op0, is_and) {
        return Some(x);
    }
    if let Some(x) = simplify_and_or_of_icmps_with_add(q, op0, op1, is_and) {
        return Some(x);
    }
    simplify_and_or_of_icmps_with_add(q, op1, op0, is_and)
}

/// `simplifyAndOrOfCmps`
/// ([:1902](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L1902)).
fn simplify_and_or_of_cmps(
    q: &mut Query,
    mut op0: Operand,
    mut op1: Operand,
    is_and: bool,
) -> Option<Operand> {
    // Look through casts of the 'and' operands to find compares.
    let cast0 = q.cast(op0);
    let cast_ty = cast0.map(|_| q.ty(op0));
    if let (Some((opc0, src0, _)), Some((opc1, src1, _))) = (cast0, q.cast(op1))
        && opc0 == opc1
        && q.ty(src0) == q.ty(src1)
    {
        op0 = src0;
        op1 = src1;
    }
    let v = if q.icmp(op0).is_some() && q.icmp(op1).is_some() {
        simplify_and_or_of_icmps(q, op0, op1, is_and)
    } else {
        None
    }?;
    let Some((cast_op, _, _)) = cast0 else { return Some(v) };
    // If we looked through casts, we can only handle a constant simplification
    // because we are not allowed to create a cast instruction here.
    let c = q.constant(v)?;
    constfold::cast(q.context, cast_op, c, cast_ty?).map(Operand::Constant)
}

/// `isCheckForZeroAndMulWithOverflow`
/// ([OverflowInstAnalysis.cpp:21](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/OverflowInstAnalysis.cpp#L21)):
/// `op0` is `X != 0` (`X == 0`) and `op1` the overflow bit of `X * ?` (its
/// negation).
fn is_check_for_zero_and_mul_with_overflow(
    q: &Query,
    op0: Operand,
    op1: Operand,
    is_and: bool,
) -> bool {
    let Some((pred, x, zero)) = q.icmp(op0) else { return false };
    if !q.is_zero(zero) {
        return false;
    }
    // %Agg = call { i4, i1 } @llvm.[us]mul.with.overflow.i4(i4 %X, i4 %???)
    // %V = extractvalue { i4, i1 } %Agg, 1
    let match_mul_overflow_check = |v: Operand| {
        let Some(extract) = q.definition(v).map(|inst| q.function.instruction(inst)) else { return false };
        // We should only be extracting the overflow bit.
        if !matches!(
            &extract.opcode,
            Opcode::ExtractValue(indices) if indices[..] == [1]
        ) {
            return false;
        }
        let tracking = q.tracking();
        let Some(args) = ["llvm.umul.with.overflow", "llvm.smul.with.overflow"]
            .into_iter()
            .find_map(|base| pattern::intrinsic_call(&tracking, extract.operands[0], base))
            .map(|(_, args)| args)
        else {
            return false;
        };
        matches!(args, [a, b] if *a == x || *b == x)
    };
    (is_and && pred == IntPredicate::Ne && match_mul_overflow_check(op1))
        || (!is_and && pred == IntPredicate::Eq && q.not_of(op1).is_some_and(match_mul_overflow_check))
}

/// `simplifyAssociativeBinOp`
/// ([:250](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L250)).
fn simplify_associative(
    q: &mut Query,
    opcode: BinaryOp,
    lhs: Operand,
    rhs: Operand,
    mut max_recurse: u32,
) -> Option<Operand> {
    // Recursion is always used, so bail out at once if we already hit the
    // limit.
    if max_recurse == 0 {
        return None;
    }
    max_recurse -= 1;
    let op0 = q.binary_of(lhs, opcode).map(|(a, b, _)| (a, b));
    let op1 = q.binary_of(rhs, opcode).map(|(a, b, _)| (a, b));
    // Transform: "(A op B) op C" ==> "A op (B op C)" if it simplifies
    // completely.
    if let Some((a, b)) = op0 {
        let c = rhs;
        if let Some(v) = simplify_binop(q, opcode, b, c, max_recurse) {
            if v == b {
                return Some(lhs);
            }
            if let Some(w) = simplify_binop(q, opcode, a, v, max_recurse) {
                return Some(w);
            }
        }
    }
    // Transform: "A op (B op C)" ==> "(A op B) op C" if it simplifies
    // completely.
    if let Some((b, c)) = op1 {
        let a = lhs;
        if let Some(v) = simplify_binop(q, opcode, a, b, max_recurse) {
            if v == b {
                return Some(rhs);
            }
            if let Some(w) = simplify_binop(q, opcode, v, c, max_recurse) {
                return Some(w);
            }
        }
    }
    // The remaining transforms require commutativity as well as associativity.
    if !is_commutative(opcode) {
        return None;
    }
    // Transform: "(A op B) op C" ==> "(C op A) op B" if it simplifies
    // completely.
    if let Some((a, b)) = op0 {
        let c = rhs;
        if let Some(v) = simplify_binop(q, opcode, c, a, max_recurse) {
            if v == a {
                return Some(lhs);
            }
            if let Some(w) = simplify_binop(q, opcode, v, b, max_recurse) {
                return Some(w);
            }
        }
    }
    // Transform: "A op (B op C)" ==> "B op (C op A)" if it simplifies
    // completely.
    if let Some((b, c)) = op1 {
        let a = lhs;
        if let Some(v) = simplify_binop(q, opcode, c, a, max_recurse) {
            if v == c {
                return Some(rhs);
            }
            if let Some(w) = simplify_binop(q, opcode, b, v, max_recurse) {
                return Some(w);
            }
        }
    }
    None
}

/// `simplifyAddInst`
/// ([:594](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L594)).
fn simplify_add(
    q: &mut Query,
    mut op0: Operand,
    mut op1: Operand,
    nsw: bool,
    nuw: bool,
    max_recurse: u32,
) -> Option<Operand> {
    if let Some(c) = fold_or_commute_constant(q, BinaryOp::Add, &mut op0, &mut op1) {
        return Some(c);
    }
    // X + poison -> poison
    if q.is_poison(op1) {
        return Some(op1);
    }
    // X + undef -> undef
    if q.is_undef(op1) {
        return Some(op1);
    }
    // X + 0 -> X
    if q.is_zero(op1) {
        return Some(op0);
    }
    // If two operands are negative, return 0.
    if is_known_negation(q, op0, op1, false) {
        let ty = q.ty(op0);
        return Some(q.null(ty));
    }
    // X + (Y - X) -> Y; (Y - X) + X -> Y
    if let Some((y, x, _)) = q.binary_of(op1, BinaryOp::Sub)
        && x == op0
    {
        return Some(y);
    }
    if let Some((y, x, _)) = q.binary_of(op0, BinaryOp::Sub)
        && x == op1
    {
        return Some(y);
    }
    // X + ~X -> -1   since   ~X = -X-1
    if q.not_of(op0) == Some(op1) || q.not_of(op1) == Some(op0) {
        let ty = q.ty(op0);
        return Some(q.all_ones(ty));
    }
    // add nsw/nuw (xor Y, signmask), signmask --> Y
    if (nsw || nuw)
        && q.is_sign_mask(op1)
        && let Some((y, mask_operand, _)) = q.binary_of(op0, BinaryOp::Xor)
        && q.is_sign_mask(mask_operand)
    {
        return Some(y);
    }
    // add nuw %x, -1  ->  -1, because %x can only be 0.
    if nuw && q.is_all_ones(op1) {
        return Some(op1);
    }
    // i1 add -> xor.
    if max_recurse > 0
        && q.width(op0) == Some(1)
        && let Some(v) = simplify_xor(q, op0, op1, max_recurse - 1)
    {
        return Some(v);
    }
    // Try some generic simplifications for associative operations.
    simplify_associative(q, BinaryOp::Add, op0, op1, max_recurse)
}

/// `simplifySubInst`
/// ([:746](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L746)).
fn simplify_sub(
    q: &mut Query,
    mut op0: Operand,
    mut op1: Operand,
    nsw: bool,
    nuw: bool,
    max_recurse: u32,
) -> Option<Operand> {
    if let Some(c) = fold_or_commute_constant(q, BinaryOp::Sub, &mut op0, &mut op1) {
        return Some(c);
    }
    // X - poison -> poison; poison - X -> poison
    if q.is_poison(op0) || q.is_poison(op1) {
        let ty = q.ty(op0);
        return Some(q.poison(ty));
    }
    // X - undef -> undef; undef - X -> undef
    if q.is_undef(op0) || q.is_undef(op1) {
        let ty = q.ty(op0);
        return Some(q.undef(ty));
    }
    // X - 0 -> X
    if q.is_zero(op1) {
        return Some(op0);
    }
    // X - X -> 0
    if op0 == op1 {
        let ty = q.ty(op0);
        return Some(q.null(ty));
    }
    // Is this a negation?
    if q.is_zero(op0) {
        // 0 - X -> 0 if the sub is NUW.
        if nuw {
            let ty = q.ty(op0);
            return Some(q.null(ty));
        }
        if let Some(width) = q.width(op1)
            && q.known(op1).zero == mask(width) >> 1
        {
            // Op1 is either 0 or the minimum signed value. If the sub is NSW,
            // then Op1 must be 0 because negating the minimum
            // signed value is undefined.
            if nsw {
                let ty = q.ty(op0);
                return Some(q.null(ty));
            }
            // 0 - X -> X if X is 0 or the minimum signed value.
            return Some(op1);
        }
    }
    // (X + Y) - Z -> X + (Y - Z) or Y + (X - Z) if everything simplifies.
    let z = op1;
    if max_recurse > 0
        && let Some((x, y, _)) = q.binary_of(op0, BinaryOp::Add)
    {
        if let Some(v) = simplify_binop(q, BinaryOp::Sub, y, z, max_recurse - 1)
            && let Some(w) = simplify_binop(q, BinaryOp::Add, x, v, max_recurse - 1)
        {
            return Some(w);
        }
        if let Some(v) = simplify_binop(q, BinaryOp::Sub, x, z, max_recurse - 1)
            && let Some(w) = simplify_binop(q, BinaryOp::Add, y, v, max_recurse - 1)
        {
            return Some(w);
        }
    }
    // X - (Y + Z) -> (X - Y) - Z or (X - Z) - Y if everything simplifies.
    let x = op0;
    if max_recurse > 0
        && let Some((y, z, _)) = q.binary_of(op1, BinaryOp::Add)
    {
        if let Some(v) = simplify_binop(q, BinaryOp::Sub, x, y, max_recurse - 1)
            && let Some(w) = simplify_binop(q, BinaryOp::Sub, v, z, max_recurse - 1)
        {
            return Some(w);
        }
        if let Some(v) = simplify_binop(q, BinaryOp::Sub, x, z, max_recurse - 1)
            && let Some(w) = simplify_binop(q, BinaryOp::Sub, v, y, max_recurse - 1)
        {
            return Some(w);
        }
    }
    // Z - (X - Y) -> (Z - X) + Y if everything simplifies.
    let z = op0;
    if max_recurse > 0
        && let Some((x, y, _)) = q.binary_of(op1, BinaryOp::Sub)
        && let Some(v) = simplify_binop(q, BinaryOp::Sub, z, x, max_recurse - 1)
        && let Some(w) = simplify_binop(q, BinaryOp::Add, v, y, max_recurse - 1)
    {
        return Some(w);
    }
    // ptrtoint(X) - ptrtoint(Y) -> the difference of their constant offsets,
    // from one base.
    if let Some(x) = q.ptr_to_int_source(op0)
        && let Some(y) = q.ptr_to_int_source(op1)
        && let Some(difference) = compute_pointer_difference(q, x, y)
    {
        let ty = q.ty(op0);
        return Some(q.int(ty, difference));
    }
    // trunc(X) - trunc(Y) -> trunc(X - Y) if everything simplifies.
    if max_recurse > 0
        && let Some((CastOp::Trunc, x, _)) = q.cast(op0)
        && let Some((CastOp::Trunc, y, _)) = q.cast(op1)
        && q.ty(x) == q.ty(y)
        && let Some(v) = simplify_binop(q, BinaryOp::Sub, x, y, max_recurse - 1)
    {
        let to = q.ty(op0);
        if let Some(w) = simplify_cast(q, CastOp::Trunc, v, to, max_recurse - 1) {
            return Some(w);
        }
    }
    // i1 sub -> xor.
    if max_recurse > 0
        && q.width(op0) == Some(1)
        && let Some(v) = simplify_xor(q, op0, op1, max_recurse - 1)
    {
        return Some(v);
    }
    if let Some(v) = simplify_by_dom_eq(q, BinaryOp::Sub, op0, op1, max_recurse) {
        return Some(v);
    }
    // (sub nuw C_Mask, (xor X, C_Mask)) -> X
    if nuw
        && let Some(x) = q.other_of(op1, BinaryOp::Xor, op0)
        && q.is_low_bit_mask(op0)
    {
        return Some(x);
    }
    None
}

/// `computePointerDifference`
/// ([InstructionSimplify.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp)):
/// `LHS - RHS` in the pointers' index width, when both are constant offsets
/// (`gep`s with constant indices, bitcasts) from one base; the difference
/// sign-extended, which the caller's `int` cuts to the integer
/// (`getIntegerCast`). DEVIATE: a pointer whose offset carries into its
/// selector (a huge one) has no such difference.
fn compute_pointer_difference(
    q: &Query,
    lhs: Operand,
    rhs: Operand,
) -> Option<u128> {
    let space = |operand: Operand| match q.context.types.get(q.ty(operand)) {
        crate::types::Type::Pointer(space) => Some(*space),
        _ => None,
    };
    let (lhs_space, rhs_space) = (space(lhs)?, space(rhs)?);
    if lhs_space != rhs_space || q.layout.has_unstable_representation(lhs_space) {
        return None;
    }
    let width = q.layout.pointer(lhs_space).index_bits;
    let tracking = q.tracking();
    let (lhs_base, lhs_offset) = nonzero::strip_and_accumulate_constant_offsets(&tracking, lhs, width, true);
    let (rhs_base, rhs_offset) = nonzero::strip_and_accumulate_constant_offsets(&tracking, rhs, width, true);
    (lhs_base == rhs_base).then(|| signed(lhs_offset.wrapping_sub(rhs_offset) & mask(width), width) as u128)
}

/// `simplifyMulInst`
/// ([:899](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L899)).
fn simplify_mul(
    q: &mut Query,
    mut op0: Operand,
    mut op1: Operand,
    nsw: bool,
    max_recurse: u32,
) -> Option<Operand> {
    if let Some(c) = fold_or_commute_constant(q, BinaryOp::Mul, &mut op0, &mut op1) {
        return Some(c);
    }
    // X * poison -> poison
    if q.is_poison(op1) {
        return Some(op1);
    }
    // X * undef -> 0; X * 0 -> 0
    if q.is_undef(op1) || q.is_zero(op1) {
        let ty = q.ty(op0);
        return Some(q.null(ty));
    }
    // X * 1 -> X
    if q.is_one(op1) {
        return Some(op0);
    }
    // (X / Y) * Y -> X if the division is exact; Y * (X / Y) likewise.
    for (divided, divisor) in [(op0, op1), (op1, op0)] {
        if let Some((op, x, y, flags)) = q.binary(divided)
            && matches!(op, BinaryOp::UDiv | BinaryOp::SDiv)
            && flags.contains(Flags::EXACT)
            && y == divisor
        {
            return Some(x);
        }
    }
    if q.width(op0) == Some(1) {
        // mul i1 nsw is a special-case because -1 * -1 is poison.
        if nsw {
            let ty = q.ty(op0);
            return Some(q.null(ty));
        }
        // Treat "mul i1" as "and i1".
        if max_recurse > 0
            && let Some(v) = simplify_and(q, op0, op1, max_recurse - 1)
        {
            return Some(v);
        }
    }
    // Try some generic simplifications for associative operations.
    if let Some(v) = simplify_associative(q, BinaryOp::Mul, op0, op1, max_recurse) {
        return Some(v);
    }
    // Mul distributes over Add. Try some generic simplifications based on this.
    if let Some(v) = expand_commutative_bin_op(q, BinaryOp::Mul, op0, op1, BinaryOp::Add, max_recurse) {
        return Some(v);
    }
    if (q.is_select(op0) || q.is_select(op1))
        && let Some(v) = thread_bin_op_over_select(q, BinaryOp::Mul, op0, op1, max_recurse)
    {
        return Some(v);
    }
    if (q.is_phi(op0) || q.is_phi(op1))
        && let Some(v) = thread_bin_op_over_phi(q, BinaryOp::Mul, op0, op1, max_recurse)
    {
        return Some(v);
    }
    None
}

/// `simplifyDivRem`
/// ([:1046](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L1046)).
fn simplify_div_rem(
    q: &mut Query,
    opcode: BinaryOp,
    op0: Operand,
    op1: Operand,
    max_recurse: u32,
) -> Option<Operand> {
    let is_div = matches!(opcode, BinaryOp::SDiv | BinaryOp::UDiv);
    let is_signed = matches!(opcode, BinaryOp::SDiv | BinaryOp::SRem);
    let ty = q.ty(op0);
    // X / undef -> poison; X % undef -> poison
    if q.is_undef(op1) || q.is_poison(op1) {
        return Some(q.poison(ty));
    }
    // X / 0 -> poison
    if q.is_zero(op1) {
        return Some(q.poison(ty));
    }
    // poison / X -> poison
    if q.is_poison(op0) {
        return Some(op0);
    }
    // undef / X -> 0; undef % X -> 0
    if q.is_undef(op0) {
        return Some(q.null(ty));
    }
    // 0 / X -> 0
    if q.is_zero(op0) {
        return Some(q.null(ty));
    }
    // X / X -> 1; X % X -> 0
    if op0 == op1 {
        return Some(if is_div { q.int(ty, 1) } else { q.null(ty) });
    }
    if let Some(width) = q.width(op1) {
        let known = q.known(op1);
        // If the divisor is known to be zero, just return poison.
        if known.is_zero() {
            return Some(q.poison(ty));
        }
        // If the divisor can only be zero or one, X / 1 -> X, X % 1 -> 0.
        if known.count_min_leading_zeros() == width - 1 {
            return Some(if is_div { op0 } else { q.null(ty) });
        }
    }
    // (X * Y) / Y -> X if the multiplication does not overflow.
    if let Some((BinaryOp::Mul, a, b, flags)) = q.binary(op0) {
        let x = if b == op1 {
            Some(a)
        } else if a == op1 {
            Some(b)
        } else {
            None
        };
        if let Some(x) = x {
            let wrapless = if is_signed { flags.contains(Flags::NSW) } else { flags.contains(Flags::NUW) };
            let divided = q
                .binary_of(x, if is_signed { BinaryOp::SDiv } else { BinaryOp::UDiv })
                .is_some_and(|(_, d, _)| d == op1);
            if wrapless || divided {
                return Some(if is_div { x } else { q.null(ty) });
            }
        }
    }
    // If the dividend is known smaller than the divisor, X / Y is 0 and X % Y
    // is X.
    if is_div_zero(q, op0, op1, max_recurse, is_signed) {
        return Some(if is_div { q.null(ty) } else { op0 });
    }
    if let Some(v) = simplify_by_dom_eq(q, opcode, op0, op1, max_recurse) {
        return Some(v);
    }
    if (q.is_select(op0) || q.is_select(op1))
        && let Some(v) = thread_bin_op_over_select(q, opcode, op0, op1, max_recurse)
    {
        return Some(v);
    }
    if (q.is_phi(op0) || q.is_phi(op1))
        && let Some(v) = thread_bin_op_over_phi(q, opcode, op0, op1, max_recurse)
    {
        return Some(v);
    }
    None
}

/// `isICmpTrue`: `icmp pred lhs, rhs` simplifies to true.
fn is_icmp_true(
    q: &mut Query,
    pred: IntPredicate,
    lhs: Operand,
    rhs: Operand,
    max_recurse: u32,
) -> bool {
    simplify_icmp(q, pred, lhs, rhs, max_recurse).is_some_and(|v| q.is_one(v))
}

/// `isDivZero`
/// ([:981](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L981)):
/// `X / Y` is 0 (and `X % Y` is X).
fn is_div_zero(
    q: &mut Query,
    x: Operand,
    y: Operand,
    mut max_recurse: u32,
    is_signed: bool,
) -> bool {
    // Recursion is always used, so bail out at once if we already hit the
    // limit.
    if max_recurse == 0 {
        return false;
    }
    max_recurse -= 1;
    let ty = q.ty(x);
    if is_signed {
        // (X srem Y) sdiv Y --> 0
        if matches!(
            q.binary(x),
            Some((BinaryOp::SRem, _, divisor, _)) if divisor == y
        ) {
            return true;
        }
        let Some(width) = q.width(x) else { return false };
        let minimum = 1u128 << (width - 1);
        // |X| / |Y| --> 0 where one is a constant: |Y| > |C| is Y < -abs(C) or Y > abs(C).
        if let Some((c, _)) = q.number(x)
            && c != minimum
        {
            let magnitude = signed(c, width).unsigned_abs();
            let (positive, negative) = (q.int(ty, magnitude), q.int(ty, magnitude.wrapping_neg() & mask(width)));
            if is_icmp_true(q, IntPredicate::Slt, y, negative, max_recurse)
                || is_icmp_true(q, IntPredicate::Sgt, y, positive, max_recurse)
            {
                return true;
            }
        }
        if let Some((c, _)) = q.number(y) {
            // The smallest signed number has no magnitude: prove the dividend
            // is not it.
            if c == minimum {
                return is_icmp_true(q, IntPredicate::Ne, x, y, max_recurse);
            }
            // |X| < |C| --> X > -abs(C) and X < abs(C)
            let magnitude = signed(c, width).unsigned_abs();
            let (positive, negative) = (q.int(ty, magnitude), q.int(ty, magnitude.wrapping_neg() & mask(width)));
            if is_icmp_true(q, IntPredicate::Sgt, x, negative, max_recurse)
                && is_icmp_true(q, IntPredicate::Slt, x, positive, max_recurse)
            {
                return true;
            }
        }
        return false;
    }
    // Is the unsigned dividend known to be less than a constant divisor?
    if let Some((c, _)) = q.number(y)
        && q.known(x).max_value() < c
    {
        return true;
    }
    // Try again for any divisor: is the dividend unsigned less than the
    // divisor?
    is_icmp_true(q, IntPredicate::Ult, x, y, max_recurse)
}

/// `simplifyDiv`
/// ([:1140](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L1140)).
fn simplify_div(
    q: &mut Query,
    opcode: BinaryOp,
    mut op0: Operand,
    mut op1: Operand,
    is_exact: bool,
    max_recurse: u32,
) -> Option<Operand> {
    if let Some(c) = fold_or_commute_constant(q, opcode, &mut op0, &mut op1) {
        return Some(c);
    }
    if let Some(v) = simplify_div_rem(q, opcode, op0, op1, max_recurse) {
        return Some(v);
    }
    if is_exact && let Some((divisor, width)) = q.number(op1) {
        // The dividend must have at least as many trailing zeros as the
        // divisor to divide evenly; with fewer the result is poison.
        let divisor_zeros = if divisor == 0 { width } else { divisor.trailing_zeros() };
        if divisor_zeros != 0 && q.known(op0).count_max_trailing_zeros() < divisor_zeros {
            let ty = q.ty(op0);
            return Some(q.poison(ty));
        }
        // udiv exact (mul nsw X, C), C --> X; sdiv exact (mul nuw X, C), C -->
        // X; where C is not a power of 2.
        if !divisor.is_power_of_two()
            && let Some((BinaryOp::Mul, x, multiplier, flags)) = q.binary(op0)
            && multiplier == op1
            && flags.contains(if opcode == BinaryOp::UDiv { Flags::NSW } else { Flags::NUW })
        {
            return Some(x);
        }
    }
    None
}

/// `simplifyRem`
/// ([:1175](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L1175)).
fn simplify_rem(
    q: &mut Query,
    opcode: BinaryOp,
    mut op0: Operand,
    mut op1: Operand,
    max_recurse: u32,
) -> Option<Operand> {
    if let Some(c) = fold_or_commute_constant(q, opcode, &mut op0, &mut op1) {
        return Some(c);
    }
    if let Some(v) = simplify_div_rem(q, opcode, op0, op1, max_recurse) {
        return Some(v);
    }
    let ty = q.ty(op0);
    // srem (nsw shl X, _) X... ; urem (nuw shl X, _), X -> 0
    if let Some((BinaryOp::Shl, a, _, flags)) = q.binary(op0)
        && a == op1
        && flags.contains(if opcode == BinaryOp::SRem { Flags::NSW } else { Flags::NUW })
    {
        return Some(q.null(ty));
    }
    if let Some((c0, width)) = q.number(op1)
        && let Some((BinaryOp::Mul, a, b, flags)) = q.binary(op0)
        && flags.contains(if opcode == BinaryOp::SRem { Flags::NSW } else { Flags::NUW })
        && c0 != 0
        && let Some(factor) = [a, b].into_iter().find_map(|one| q.number(one))
    {
        let zero = if opcode == BinaryOp::SRem {
            signed(factor.0, width).checked_rem(signed(c0, width)) == Some(0)
        } else {
            factor.0 % c0 == 0
        };
        if zero {
            return Some(q.null(ty));
        }
    }
    None
}

/// `simplifyShift`
/// ([:1302](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L1302)).
fn simplify_shift(
    q: &mut Query,
    opcode: BinaryOp,
    mut op0: Operand,
    mut op1: Operand,
    is_nsw: bool,
    max_recurse: u32,
) -> Option<Operand> {
    if let Some(c) = fold_or_commute_constant(q, opcode, &mut op0, &mut op1) {
        return Some(c);
    }
    // poison shift X -> poison
    if q.is_poison(op0) {
        return Some(op0);
    }
    // 0 shift X -> 0
    if q.is_zero(op0) {
        let ty = q.ty(op0);
        return Some(q.null(ty));
    }
    // X shift 0 -> X; X shift (sext i1) -> X
    if q.is_zero(op1)
        || matches!(
            q.cast(op1),
            Some((CastOp::SExt, x, _)) if q.width(x) == Some(1)
        )
    {
        return Some(op0);
    }
    // `isPoisonShift`: a shift by undef may shift by the width; so may one by
    // the width or more.
    if q.is_undef(op1) || q.number(op1).is_some_and(|(amount, width)| amount >= u128::from(width)) {
        let ty = q.ty(op0);
        return Some(q.poison(ty));
    }
    // If the operation is with the result of a select or phi instruction,
    // check whether operating on its branches always yields the same value.
    if q.is_select(op0) || q.is_select(op1) {
        if let Some(v) = thread_bin_op_over_select(q, opcode, op0, op1, max_recurse) {
            return Some(v);
        }
    }
    if q.is_phi(op0) || q.is_phi(op1) {
        if let Some(v) = thread_bin_op_over_phi(q, opcode, op0, op1, max_recurse) {
            return Some(v);
        }
    }
    let width = q.width(op1)?;
    let known_amount = q.known(op1);
    // A shift amount known to be the width or more is poison.
    if known_amount.min_value() >= u128::from(width) {
        let ty = q.ty(op0);
        return Some(q.poison(ty));
    }
    // An amount whose low bits are known zero is 0 or poison: op0.
    let valid_shift_bits = u32::BITS - (width - 1).leading_zeros();
    if known_amount.count_min_trailing_zeros() >= valid_shift_bits {
        return Some(op0);
    }
    if is_nsw {
        let known_value = q.known(op0);
        let mut known_shl = KnownBits::shl(&known_value, &known_amount, false, false, false);
        if known_value.zero >> (width - 1) & 1 == 1 {
            known_shl.zero |= 1u128 << (width - 1);
        }
        if known_value.one >> (width - 1) & 1 == 1 {
            known_shl.one |= 1u128 << (width - 1);
        }
        if known_shl.has_conflict() {
            let ty = q.ty(op0);
            return Some(q.poison(ty));
        }
    }
    None
}

/// `simplifyRightShift`
/// ([:1355](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp)).
fn simplify_right_shift(
    q: &mut Query,
    opcode: BinaryOp,
    op0: Operand,
    op1: Operand,
    is_exact: bool,
    max_recurse: u32,
) -> Option<Operand> {
    if let Some(v) = simplify_shift(q, opcode, op0, op1, false, max_recurse) {
        return Some(v);
    }
    // X >> X -> 0
    if op0 == op1 {
        let ty = q.ty(op0);
        return Some(q.null(ty));
    }
    // undef >> X -> 0; undef >> X -> undef (if it's exact)
    if q.is_undef(op0) {
        let ty = q.ty(op0);
        return Some(if is_exact { op0 } else { q.null(ty) });
    }
    // The low bit cannot be shifted out of an exact shift if it is set.
    if is_exact && q.known(op0).one & 1 == 1 {
        return Some(op0);
    }
    None
}

/// `simplifyShlInst`
/// ([:1401](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L1401)).
fn simplify_shl(
    q: &mut Query,
    op0: Operand,
    op1: Operand,
    is_nsw: bool,
    is_nuw: bool,
    max_recurse: u32,
) -> Option<Operand> {
    if let Some(v) = simplify_shift(q, BinaryOp::Shl, op0, op1, is_nsw, max_recurse) {
        return Some(v);
    }
    let ty = q.ty(op0);
    // undef << X -> 0; undef << X -> undef if it's NSW/NUW
    if q.is_undef(op0) {
        return Some(if is_nsw || is_nuw { op0 } else { q.null(ty) });
    }
    // (X >>exact Y) << Y -> X
    if let Some((shr, x, amount, flags)) = q.binary(op0)
        && matches!(shr, BinaryOp::LShr | BinaryOp::AShr)
        && flags.contains(Flags::EXACT)
        && amount == op1
    {
        return Some(x);
    }
    // nuw (negative << _) -> the negative
    if is_nuw
        && let Some((bits, width)) = q.number(op0)
        && bits >> (width - 1) & 1 == 1
    {
        return Some(op0);
    }
    // nsw nuw shl by width-1 is 0
    if is_nsw
        && is_nuw
        && let Some((amount, width)) = q.number(op1)
        && amount == u128::from(width - 1)
    {
        return Some(q.null(ty));
    }
    None
}

/// `simplifyLShrInst`
/// ([:1442](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L1442)).
fn simplify_lshr(
    q: &mut Query,
    op0: Operand,
    op1: Operand,
    is_exact: bool,
    max_recurse: u32,
) -> Option<Operand> {
    if let Some(v) = simplify_right_shift(q, BinaryOp::LShr, op0, op1, is_exact, max_recurse) {
        return Some(v);
    }
    // (X <<nuw Y) >> Y -> X
    if let Some((BinaryOp::Shl, x, amount, flags)) = q.binary(op0)
        && flags.contains(Flags::NUW)
        && amount == op1
    {
        return Some(x);
    }
    // ((X << A) | Y) >> A -> X  if effective width of Y is not larger than A.
    // We can return X as we do in the above case since OR alters no bits in X.
    if let Some((shift, _)) = q.number(op1)
        && let Some((x, _, amount, y)) = q.or_of_nuw_shl(op0)
        && shift == amount
        && shift >= u128::from(q.known(y).count_max_active_bits())
    {
        return Some(x);
    }
    None
}

/// `simplifyAShrInst`
/// ([:1479](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L1479)).
fn simplify_ashr(
    q: &mut Query,
    op0: Operand,
    op1: Operand,
    is_exact: bool,
    max_recurse: u32,
) -> Option<Operand> {
    if let Some(v) = simplify_right_shift(q, BinaryOp::AShr, op0, op1, is_exact, max_recurse) {
        return Some(v);
    }
    // all ones >> X -> all ones; (all ones << X) >> X -> all ones
    if q.is_all_ones(op0)
        || matches!(
            q.binary(op0),
            Some((BinaryOp::Shl, a, amount, _)) if q.is_all_ones(a) && amount == op1
        )
    {
        let ty = q.ty(op0);
        return Some(q.all_ones(ty));
    }
    // (X <<nsw Y) >> Y -> X
    if let Some((BinaryOp::Shl, x, amount, flags)) = q.binary(op0)
        && flags.contains(Flags::NSW)
        && amount == op1
    {
        return Some(x);
    }
    // Arithmetic shifting an all-sign-bits value does nothing.
    if q.width(op0).is_some_and(|width| q.num_sign_bits(op0) == width) {
        return Some(op0);
    }
    None
}

/// `simplifyLogicOfAddSub`
/// ([:1996](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L1996)):
/// `(X + C) & (C' - X)` where `C' = ~C`.
fn simplify_logic_of_add_sub(
    q: &mut Query,
    op0: Operand,
    op1: Operand,
    opcode: BinaryOp,
) -> Option<Operand> {
    for (add, sub) in [(op0, op1), (op1, op0)] {
        if let Some((x, c1, _)) = q.binary_of(add, BinaryOp::Add)
            && let Some((c2, x2, _)) = q.binary_of(sub, BinaryOp::Sub)
            && x2 == x
            && let (Some((bits1, width)), Some((bits2, _))) = (q.number(c1), q.number(c2))
            && !bits1 & mask(width) == bits2
        {
            let ty = q.ty(op0);
            return Some(if opcode == BinaryOp::And { q.null(ty) } else { q.all_ones(ty) });
        }
    }
    None
}

/// `simplifyAndCommutative`
/// ([:2019](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L2019)).
fn simplify_and_commutative(
    q: &mut Query,
    op0: Operand,
    op1: Operand,
    max_recurse: u32,
) -> Option<Operand> {
    // ~A & A == 0
    if q.not_of(op0) == Some(op1) {
        let ty = q.ty(op0);
        return Some(q.null(ty));
    }
    // (A | ?) & A --> A
    if q.other_of(op0, BinaryOp::Or, op1).is_some() {
        return Some(op1);
    }
    // (X | ~Y) & (X | Y) --> X
    if let Some((x, notted, _)) = q.binary_of(op0, BinaryOp::Or).map(|(a, b, f)| (a, b, f)).and_then(|(a, b, f)| {
        if q.not_of(b).is_some() {
            Some((a, b, f))
        } else if q.not_of(a).is_some() {
            Some((b, a, f))
        } else {
            None
        }
    }) && let Some(y) = q.not_of(notted)
        && let Some((p, r, _)) = q.binary_of(op1, BinaryOp::Or)
        && ((p == x && r == y) || (p == y && r == x))
    {
        return Some(x);
    }
    // If we have a multiplication overflow check that is being 'and'ed with a
    // check that one of the multipliers is not zero, we can omit the 'and', and
    // only keep the overflow check.
    if is_check_for_zero_and_mul_with_overflow(q, op0, op1, true) {
        return Some(op1);
    }
    // -A & A = A if A is a power of two or zero.
    if q.neg_of(op0).is_some_and(|(inner, _)| inner == op1) && q.is_power_of_two(op1, true) {
        return Some(op1);
    }
    // (A - 1) & A --> 0 (if A is a power-of-2 or 0)
    if let Some((a, c, _)) = q.binary_of(op0, BinaryOp::Add)
        && a == op1
        && q.is_all_ones(c)
        && q.is_power_of_two(op1, true)
    {
        let ty = q.ty(op1);
        return Some(q.null(ty));
    }
    // (x << N) & ((x << M) - 1) --> 0, where x is known to be a power of 2 and
    // M <= N.
    if let Some((BinaryOp::Shl, x, n, _)) = q.binary(op0)
        && let Some((n, _)) = q.number(n)
        && let Some((sum, c, _)) = q.binary_of(op1, BinaryOp::Add)
        && q.is_all_ones(c)
        && let Some((BinaryOp::Shl, x2, m, _)) = q.binary(sum)
        && x2 == x
        && let Some((m, _)) = q.number(m)
        && q.is_power_of_two(x, true)
        && n >= m
    {
        let ty = q.ty(op0);
        return Some(q.null(ty));
    }
    simplify_and_or_with_icmp_eq(q, BinaryOp::And, op0, op1, max_recurse)
}

/// `simplifyAndInst`
/// ([:2071](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L2071)).
fn simplify_and(
    q: &mut Query,
    mut op0: Operand,
    mut op1: Operand,
    max_recurse: u32,
) -> Option<Operand> {
    if let Some(c) = fold_or_commute_constant(q, BinaryOp::And, &mut op0, &mut op1) {
        return Some(c);
    }
    // X & poison -> poison
    if q.is_poison(op1) {
        return Some(op1);
    }
    // X & undef -> 0
    if q.is_undef(op1) {
        let ty = q.ty(op0);
        return Some(q.null(ty));
    }
    // X & X = X
    if op0 == op1 {
        return Some(op0);
    }
    // X & 0 = 0
    if q.is_zero(op1) {
        let ty = q.ty(op0);
        return Some(q.null(ty));
    }
    // X & -1 = X
    if q.is_all_ones(op1) {
        return Some(op0);
    }
    if let Some(v) = simplify_and_commutative(q, op0, op1, max_recurse) {
        return Some(v);
    }
    if let Some(v) = simplify_and_commutative(q, op1, op0, max_recurse) {
        return Some(v);
    }
    if let Some(v) = simplify_logic_of_add_sub(q, op0, op1, BinaryOp::And) {
        return Some(v);
    }
    // A mask that only clears bits that the shift already cleared is a no-op.
    if let Some((mask_bits, width)) = q.number(op1) {
        if let Some((BinaryOp::Shl, _, amount, _)) = q.binary(op0)
            && let Some((shift, _)) = q.number(amount)
            && shift < u128::from(width)
            && ((!mask_bits & mask(width)) >> shift) == 0
        {
            return Some(op0);
        }
        if let Some((BinaryOp::LShr, _, amount, _)) = q.binary(op0)
            && let Some((shift, _)) = q.number(amount)
            && shift < u128::from(width)
            && (((!mask_bits & mask(width)) << shift) & mask(width)) == 0
        {
            return Some(op0);
        }
    }
    // and 2^x-1, 2^C --> 0 where x <= C.
    if let Some((power, _)) = q.number(op1)
        && power.is_power_of_two()
        && let Some((shift, c, _)) = q.binary_of(op0, BinaryOp::Add)
        && q.is_all_ones(c)
        && q.is_power_of_two(shift, false)
    {
        // Use the active bits to make use of the power of two knowledge.
        let active_bits = |value: u128| 128 - value.leading_zeros();
        if active_bits(power) >= active_bits(q.known(shift).max_value()) {
            let ty = q.ty(op1);
            return Some(q.null(ty));
        }
    }
    if let Some(v) = simplify_and_or_of_cmps(q, op0, op1, true) {
        return Some(v);
    }
    // (zext X) & (sext X) = zext X; the other order likewise.
    if let (Some((CastOp::ZExt, x, _)), Some((CastOp::SExt, y, _))) = (q.cast(op0), q.cast(op1))
        && x == y
    {
        return Some(op0);
    }
    if let (Some((CastOp::ZExt, x, _)), Some((CastOp::SExt, y, _))) = (q.cast(op1), q.cast(op0))
        && x == y
    {
        return Some(op1);
    }
    // Try some generic simplifications for associative operations.
    if let Some(v) = simplify_associative(q, BinaryOp::And, op0, op1, max_recurse) {
        return Some(v);
    }
    // And distributes over Or and over Xor.
    if let Some(v) = expand_commutative_bin_op(q, BinaryOp::And, op0, op1, BinaryOp::Or, max_recurse) {
        return Some(v);
    }
    if let Some(v) = expand_commutative_bin_op(q, BinaryOp::And, op0, op1, BinaryOp::Xor, max_recurse) {
        return Some(v);
    }
    if q.is_select(op0) || q.is_select(op1) {
        if q.width(op0) == Some(1) {
            // A & (A && B) -> A && B
            if q.select_arms(op1).is_some_and(|(c, _, f)| c == op0 && q.is_zero(f)) {
                return Some(op1);
            }
            if q.select_arms(op0).is_some_and(|(c, _, f)| c == op1 && q.is_zero(f)) {
                return Some(op0);
            }
        }
        if let Some(v) = thread_bin_op_over_select(q, BinaryOp::And, op0, op1, max_recurse) {
            return Some(v);
        }
    }
    if (q.is_phi(op0) || q.is_phi(op1))
        && let Some(v) = thread_bin_op_over_phi(q, BinaryOp::And, op0, op1, max_recurse)
    {
        return Some(v);
    }
    // ((X << A) | Y) & Mask -> Y, if Mask = ((1 << effective_width_of(Y)) - 1);
    // ((X << A) | Y) & Mask -> X << A, if Mask = ((1 << effective_width_of(X))
    // - 1) << A.
    if let Some((mask_bits, width)) = q.number(op1)
        && let Some((x, shifted, amount, y)) = q.or_of_nuw_shl(op0)
    {
        let shift_count = amount.min(u128::from(width)) as u32;
        let width_y = q.known(y).count_max_active_bits();
        if width_y <= shift_count {
            let width_x = q.known(x).count_max_active_bits();
            let low_bits = |count: u32| if count == 0 { 0 } else { mask(count) };
            let bits_y = low_bits(width_y);
            let bits_x = if shift_count >= width { 0 } else { (low_bits(width_x) << shift_count) & mask(width) };
            if bits_y & !mask_bits == 0 && bits_x & mask_bits == 0 {
                return Some(y);
            }
            if bits_x & !mask_bits == 0 && bits_y & mask_bits == 0 {
                return Some(shifted);
            }
        }
    }
    // ((X | Y) ^ X ) & ((X | Y) ^ Y) --> 0
    // ((X | Y) ^ Y ) & ((X | Y) ^ X) --> 0
    if let Some((a, b, _)) = q.binary_of(op0, BinaryOp::Xor) {
        for (x, or) in [(a, b), (b, a)] {
            if let Some(y) = q.other_of(or, BinaryOp::Or, x)
                && let Some((p, r, _)) = q.binary_of(op1, BinaryOp::Xor)
                && ((p == or && r == y) || (p == y && r == or))
            {
                let ty = q.ty(op0);
                return Some(q.null(ty));
            }
        }
    }
    // (A ^ C1) & (A ^ ~C1) -> 0
    if let Some((a, c1, _)) = q.binary_of(op0, BinaryOp::Xor)
        && let Some((bits, width)) = q.number(c1)
        && let Some((a2, c2, _)) = q.binary_of(op1, BinaryOp::Xor)
        && a2 == a
        && q.number(c2).is_some_and(|(other, _)| other == !bits & mask(width))
    {
        let ty = q.ty(op0);
        return Some(q.null(ty));
    }
    if q.width(op0) == Some(1) {
        if let Some(implied) = q.implied(op0, op1, true) {
            // If Op0 is true implies Op1 is true, then Op0 is a subset of Op1.
            // If it implies Op1 is false, then they are not true together.
            return Some(if implied { op0 } else { q.null(q.ty(op0)) });
        }
        if let Some(implied) = q.implied(op1, op0, true) {
            return Some(if implied { op1 } else { q.null(q.ty(op1)) });
        }
    }
    if let Some(v) = simplify_by_dom_eq(q, BinaryOp::And, op0, op1, max_recurse) {
        return Some(v);
    }
    None
}

/// `simplifyOrLogic`
/// ([:2265](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L2265)),
/// the cases over `not`, `and`, `xor`.
fn simplify_or_logic(
    q: &mut Query,
    x: Operand,
    y: Operand,
) -> Option<Operand> {
    let ty = q.ty(x);
    // (X | ~X) --> -1
    if q.not_of(y) == Some(x) {
        return Some(q.all_ones(ty));
    }
    // X | ~(X & ?) --> -1
    if let Some(inner) = q.not_of(y)
        && q.other_of(inner, BinaryOp::And, x).is_some()
    {
        return Some(q.all_ones(ty));
    }
    // X | (X & ?) --> X
    if q.other_of(y, BinaryOp::And, x).is_some() {
        return Some(x);
    }
    // (A ^ B) | (A | B) --> A | B
    if let Some((a, b, _)) = q.binary_of(x, BinaryOp::Xor)
        && let Some((p, r, _)) = q.binary_of(y, BinaryOp::Or)
        && ((p == a && r == b) || (p == b && r == a))
    {
        return Some(y);
    }
    // ~(A ^ B) | (A | B) --> -1
    if let Some(inner) = q.not_of(x)
        && let Some((a, b, _)) = q.binary_of(inner, BinaryOp::Xor)
        && let Some((p, r, _)) = q.binary_of(y, BinaryOp::Or)
        && ((p == a && r == b) || (p == b && r == a))
    {
        return Some(q.all_ones(ty));
    }
    None
}

/// `simplifyOrInst`
/// ([:2356](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L2356)).
fn simplify_or(
    q: &mut Query,
    mut op0: Operand,
    mut op1: Operand,
    max_recurse: u32,
) -> Option<Operand> {
    if let Some(c) = fold_or_commute_constant(q, BinaryOp::Or, &mut op0, &mut op1) {
        return Some(c);
    }
    // X | poison -> poison
    if q.is_poison(op1) {
        return Some(op1);
    }
    // X | undef -> -1; X | -1 = -1
    if q.is_undef(op1) || q.is_all_ones(op1) {
        let ty = q.ty(op0);
        return Some(q.all_ones(ty));
    }
    // X | X = X; X | 0 = X
    if op0 == op1 || q.is_zero(op1) {
        return Some(op0);
    }
    if let Some(r) = simplify_or_logic(q, op0, op1) {
        return Some(r);
    }
    if let Some(r) = simplify_or_logic(q, op1, op0) {
        return Some(r);
    }
    if let Some(v) = simplify_logic_of_add_sub(q, op0, op1, BinaryOp::Or) {
        return Some(v);
    }
    // (zext X) | (sext X) = sext X; the other order likewise.
    if let (Some((CastOp::ZExt, x, _)), Some((CastOp::SExt, y, _))) = (q.cast(op0), q.cast(op1))
        && x == y
    {
        return Some(op1);
    }
    if let (Some((CastOp::ZExt, x, _)), Some((CastOp::SExt, y, _))) = (q.cast(op1), q.cast(op0))
        && x == y
    {
        return Some(op0);
    }
    if let Some(v) = simplify_and_or_with_icmp_eq(q, BinaryOp::Or, op0, op1, max_recurse) {
        return Some(v);
    }
    if let Some(v) = simplify_and_or_with_icmp_eq(q, BinaryOp::Or, op1, op0, max_recurse) {
        return Some(v);
    }
    if let Some(v) = simplify_and_or_of_cmps(q, op0, op1, false) {
        return Some(v);
    }
    // If we have a multiplication overflow check that is being 'or'ed with a
    // check that one of the multipliers is not zero, we can omit the 'or', and
    // only keep the overflow check.
    if is_check_for_zero_and_mul_with_overflow(q, op0, op1, false) {
        return Some(op1);
    }
    if is_check_for_zero_and_mul_with_overflow(q, op1, op0, false) {
        return Some(op0);
    }
    // Try some generic simplifications for associative operations.
    if let Some(v) = simplify_associative(q, BinaryOp::Or, op0, op1, max_recurse) {
        return Some(v);
    }
    // Or distributes over And.
    if let Some(v) = expand_commutative_bin_op(q, BinaryOp::Or, op0, op1, BinaryOp::And, max_recurse) {
        return Some(v);
    }
    if q.is_select(op0) || q.is_select(op1) {
        if q.width(op0) == Some(1) {
            // A | (A || B) -> A || B
            if q.select_arms(op1).is_some_and(|(c, t, _)| c == op0 && q.is_one(t)) {
                return Some(op1);
            }
            if q.select_arms(op0).is_some_and(|(c, t, _)| c == op1 && q.is_one(t)) {
                return Some(op0);
            }
        }
        if let Some(v) = thread_bin_op_over_select(q, BinaryOp::Or, op0, op1, max_recurse) {
            return Some(v);
        }
    }
    // (A & C1)|(B & C2)
    if let Some((a, c1, _)) = q.binary_of(op0, BinaryOp::And)
        && let Some((b, c2, _)) = q.binary_of(op1, BinaryOp::And)
        && let (Some((c1, width)), Some((c2, _))) = (q.number(c1), q.number(c2))
        && c1 == !c2 & mask(width)
    {
        // If we have: ((V + N) & C1) | (V & C2) .. and C2 = ~C1 and C2 is 0+1+
        // and (N & C2) == 0, replace with V+N. Add commutes, try both ways.
        let is_mask = |bits: u128| bits != 0 && bits & bits.wrapping_add(1) == 0;
        if is_mask(c2)
            && let Some(n) = q.other_of(a, BinaryOp::Add, b)
            && q.masked_value_is_zero(n, c2)
        {
            return Some(a);
        }
        if is_mask(c1)
            && let Some(n) = q.other_of(b, BinaryOp::Add, a)
            && q.masked_value_is_zero(n, c1)
        {
            return Some(b);
        }
    }
    if (q.is_phi(op0) || q.is_phi(op1))
        && let Some(v) = thread_bin_op_over_phi(q, BinaryOp::Or, op0, op1, max_recurse)
    {
        return Some(v);
    }
    // (A ^ C1) | (A ^ ~C1) -> -1
    if let Some((a, c1, _)) = q.binary_of(op0, BinaryOp::Xor)
        && let Some((bits, width)) = q.number(c1)
        && let Some((a2, c2, _)) = q.binary_of(op1, BinaryOp::Xor)
        && a2 == a
        && q.number(c2).is_some_and(|(other, _)| other == !bits & mask(width))
    {
        let ty = q.ty(op0);
        return Some(q.all_ones(ty));
    }
    if q.width(op0) == Some(1) {
        if let Some(implied) = q.implied(op0, op1, false) {
            // If Op0 is false implies Op1 is false, then Op1 is a subset of
            // Op0. If it implies Op1 is true, at least one is always true.
            return Some(if implied { q.all_ones(q.ty(op0)) } else { op0 });
        }
        if let Some(implied) = q.implied(op1, op0, false) {
            return Some(if implied { q.all_ones(q.ty(op1)) } else { op1 });
        }
    }
    if let Some(v) = simplify_by_dom_eq(q, BinaryOp::Or, op0, op1, max_recurse) {
        return Some(v);
    }
    None
}

/// `simplifyXorInst`
/// ([:2550](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L2550)).
fn simplify_xor(
    q: &mut Query,
    mut op0: Operand,
    mut op1: Operand,
    max_recurse: u32,
) -> Option<Operand> {
    if let Some(c) = fold_or_commute_constant(q, BinaryOp::Xor, &mut op0, &mut op1) {
        return Some(c);
    }
    // A ^ poison -> poison
    if q.is_poison(op1) {
        return Some(op1);
    }
    // A ^ undef -> undef
    if q.is_undef(op1) {
        return Some(op1);
    }
    // A ^ 0 = A
    if q.is_zero(op1) {
        return Some(op0);
    }
    // A ^ A = 0
    if op0 == op1 {
        let ty = q.ty(op0);
        return Some(q.null(ty));
    }
    // A ^ ~A = ~A ^ A = -1
    if q.not_of(op0) == Some(op1) || q.not_of(op1) == Some(op0) {
        let ty = q.ty(op0);
        return Some(q.all_ones(ty));
    }
    // (~A & B) ^ (A | B) --> A; (~A | B) ^ (A & B) --> ~A
    for (x, y) in [(op0, op1), (op1, op0)] {
        if let Some((p, r, _)) = q.binary_of(x, BinaryOp::And) {
            for (not_a, b) in [(p, r), (r, p)] {
                if let Some(a) = q.not_of(not_a)
                    && let Some((s, t, _)) = q.binary_of(y, BinaryOp::Or)
                    && ((s == a && t == b) || (s == b && t == a))
                {
                    return Some(a);
                }
            }
        }
        if let Some((p, r, _)) = q.binary_of(x, BinaryOp::Or) {
            for (not_a, b) in [(p, r), (r, p)] {
                if let Some(a) = q.not_of(not_a)
                    && let Some((s, t, _)) = q.binary_of(y, BinaryOp::And)
                    && ((s == a && t == b) || (s == b && t == a))
                {
                    return Some(not_a);
                }
            }
        }
    }
    if let Some(v) = simplify_logic_of_add_sub(q, op0, op1, BinaryOp::Xor) {
        return Some(v);
    }
    // Try some generic simplifications for associative operations.
    if let Some(v) = simplify_associative(q, BinaryOp::Xor, op0, op1, max_recurse) {
        return Some(v);
    }
    if let Some(v) = simplify_by_dom_eq(q, BinaryOp::Xor, op0, op1, max_recurse) {
        return Some(v);
    }
    // (xor (sub nuw C_Mask, X), C_Mask) -> X
    if let Some((c, x, flags)) = q.binary_of(op0, BinaryOp::Sub)
        && flags.contains(Flags::NUW)
        && c == op1
        && q.is_low_bit_mask(op1)
    {
        return Some(x);
    }
    None
}

/// `simplifyBinOp`
/// ([:6390](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L6390)),
/// the flagless form the recursion calls.
fn simplify_binop(
    q: &mut Query,
    opcode: BinaryOp,
    lhs: Operand,
    rhs: Operand,
    max_recurse: u32,
) -> Option<Operand> {
    simplify_binop_flags(q, opcode, lhs, rhs, Flags::default(), max_recurse)
}

/// The per-opcode functions of `simplifyBinOp` with the instruction's flags
/// (`simplifyInstructionWithOperands` passes them).
fn simplify_binop_flags(
    q: &mut Query,
    opcode: BinaryOp,
    lhs: Operand,
    rhs: Operand,
    flags: Flags,
    max_recurse: u32,
) -> Option<Operand> {
    let (nsw, nuw, exact) = (flags.contains(Flags::NSW), flags.contains(Flags::NUW), flags.contains(Flags::EXACT));
    match opcode {
        BinaryOp::Add => simplify_add(q, lhs, rhs, nsw, nuw, max_recurse),
        BinaryOp::Sub => simplify_sub(q, lhs, rhs, nsw, nuw, max_recurse),
        BinaryOp::Mul => simplify_mul(q, lhs, rhs, nsw, max_recurse),
        BinaryOp::SDiv => {
            // isKnownNegation(Op0, Op1, /*NeedNSW=*/true) -> -1
            if is_known_negation(q, lhs, rhs, true) {
                let ty = q.ty(lhs);
                return Some(q.all_ones(ty));
            }
            simplify_div(q, BinaryOp::SDiv, lhs, rhs, exact, max_recurse)
        }
        BinaryOp::UDiv => simplify_div(q, BinaryOp::UDiv, lhs, rhs, exact, max_recurse),
        BinaryOp::SRem => {
            // srem X, (sext i1) -> 0
            if matches!(
                q.cast(rhs),
                Some((CastOp::SExt, x, _)) if q.width(x) == Some(1)
            ) {
                let ty = q.ty(lhs);
                return Some(q.null(ty));
            }
            if is_known_negation(q, lhs, rhs, false) {
                let ty = q.ty(lhs);
                return Some(q.null(ty));
            }
            simplify_rem(q, BinaryOp::SRem, lhs, rhs, max_recurse)
        }
        BinaryOp::URem => simplify_rem(q, BinaryOp::URem, lhs, rhs, max_recurse),
        BinaryOp::Shl => simplify_shl(q, lhs, rhs, nsw, nuw, max_recurse),
        BinaryOp::LShr => simplify_lshr(q, lhs, rhs, exact, max_recurse),
        BinaryOp::AShr => simplify_ashr(q, lhs, rhs, exact, max_recurse),
        BinaryOp::And => simplify_and(q, lhs, rhs, max_recurse),
        BinaryOp::Or => simplify_or(q, lhs, rhs, max_recurse),
        BinaryOp::Xor => simplify_xor(q, lhs, rhs, max_recurse),
        BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul | BinaryOp::FDiv | BinaryOp::FRem => {
            fp::simplify_fp_binop(q, opcode, lhs, rhs, flags)
        }
    }
}

/// `simplifyPHINode`
/// ([:5620](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L5620)).
fn simplify_phi(
    q: &mut Query,
    phi: InstId,
) -> Option<Operand> {
    let result = q.function.instruction(phi).result.map(Operand::Value);
    let ty = q.function.instruction(phi).ty;
    let mut common: Option<Operand> = None;
    let mut has_poison = false;
    let mut has_undef = false;
    for incoming in q.function.instruction(phi).operands.iter().step_by(2).copied().collect::<Vec<_>>() {
        if Some(incoming) == result {
            continue;
        }
        if q.is_poison(incoming) {
            has_poison = true;
            continue;
        }
        if q.is_undef(incoming) {
            // Remember that we saw an undef value, but otherwise ignore them.
            has_undef = true;
            continue;
        }
        if common.is_some_and(|one| one != incoming) {
            return None;
        }
        common = Some(incoming);
    }
    // Every incoming value was undef, poison or the phi itself.
    let Some(common) = common else { return Some(if has_undef { q.undef(ty) } else { q.poison(ty) }) };
    // If we have a PHI node like phi(X, poison, X), where X is defined by some
    // instruction, we cannot return X as the result of the PHI node unless it
    // dominates the PHI block.
    if (has_poison || has_undef) && !q.value_dominates_phi(common, phi) {
        return None;
    }
    // Make sure we do not replace an undef value with poison.
    if has_undef && !guarantee::is_guaranteed_not_to_be_poison(&q.tracking(), common, 0) {
        return None;
    }
    Some(common)
}

/// `simplifyFreezeInst`
/// ([:7690](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/InstructionSimplify.cpp#L7690)).
fn simplify_freeze(
    q: &mut Query,
    op0: Operand,
) -> Option<Operand> {
    // Use a utility function defined in ValueTracking.
    if guarantee::is_guaranteed_not_to_be_undef_or_poison(&q.tracking(), op0, 0) {
        return Some(op0);
    }
    None
}

/// `simplifyCastInst`
/// ([:5672](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L5672)).
fn simplify_cast(
    q: &mut Query,
    cast_opc: CastOp,
    op: Operand,
    ty: TypeId,
    _max_recurse: u32,
) -> Option<Operand> {
    if let Some(c) = q.constant(op) {
        return constantfolding::constant_fold_cast_operand(q.context, q.layout, cast_opc, c, ty)
            .map(Operand::Constant);
    }
    // A cast of a cast back to the source type that is eliminable
    // (`isEliminableCastPair == BitCast`): trunc (zext X) and trunc (sext X) to
    // X's type.
    if cast_opc == CastOp::Trunc
        && let Some((CastOp::ZExt | CastOp::SExt, src, _)) = q.cast(op)
        && q.ty(src) == ty
    {
        return Some(src);
    }
    // zext/sext of a trunc that does not wrap back to its source.
    if matches!(cast_opc, CastOp::ZExt | CastOp::SExt)
        && let Some((CastOp::Trunc, src, flags)) = q.cast(op)
        && q.ty(src) == ty
        && flags.contains(if cast_opc == CastOp::ZExt { Flags::NUW } else { Flags::NSW })
    {
        return Some(src);
    }
    None
}

/// `simplifySelectInst`
/// ([:5109](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L5109)),
/// the constant condition, the i1 forms and the equal arms.
fn simplify_select(
    q: &mut Query,
    cond: Operand,
    true_val: Operand,
    false_val: Operand,
    max_recurse: u32,
) -> Option<Operand> {
    if let Some(cond_c) = q.constant(cond) {
        if let (Some(t), Some(f)) = (q.constant(true_val), q.constant(false_val))
            && let Some(c) = constfold::select(q.context, cond_c, t, f)
        {
            return Some(Operand::Constant(c));
        }
        if q.is_poison(cond) {
            let ty = q.ty(true_val);
            return Some(q.poison(ty));
        }
        // select undef, X, Y -> X or Y
        if q.is_undef(cond) {
            return Some(if q.constant(false_val).is_some() { false_val } else { true_val });
        }
        if q.is_one(cond) {
            return Some(true_val);
        }
        if q.is_zero(cond) {
            return Some(false_val);
        }
    }
    if q.ty(cond) == q.ty(true_val) {
        // select C, 1, 0 -> C
        if q.is_one(true_val) && q.is_zero(false_val) {
            return Some(cond);
        }
        // select (X && Y), Y... : the logical and/or of the arms.
        if let Some((a, b, _)) = q.binary_of(cond, BinaryOp::And)
            && ((a == true_val && b == false_val) || (a == false_val && b == true_val))
        {
            return Some(false_val);
        }
        if let Some((a, b, _)) = q.binary_of(cond, BinaryOp::Or)
            && ((a == true_val && b == false_val) || (a == false_val && b == true_val))
        {
            return Some(true_val);
        }
    }
    // select ?, X, X -> X
    if true_val == false_val {
        return Some(true_val);
    }
    if cond == true_val {
        // select C, C, 0 -> C; select C, C, 1 -> true
        if q.is_zero(false_val) {
            return Some(cond);
        }
        if q.is_one(false_val) {
            let ty = q.ty(cond);
            return Some(q.int(ty, 1));
        }
    }
    if cond == false_val {
        // select C, 1, C -> C; select C, 0, C -> false
        if q.is_one(true_val) {
            return Some(cond);
        }
        if q.is_zero(true_val) {
            let ty = q.ty(cond);
            return Some(q.null(ty));
        }
    }
    // select ?, poison, X -> X; select ?, undef, X -> X if X is no poison
    // unless the condition is; and the mirror.
    if q.is_poison(true_val) || (q.is_undef(true_val) && guarantee::implies_poison(&q.tracking(), false_val, cond)) {
        return Some(false_val);
    }
    if q.is_poison(false_val) || (q.is_undef(false_val) && guarantee::implies_poison(&q.tracking(), true_val, cond)) {
        return Some(true_val);
    }
    if let Some(v) = select::simplify_select_with_icmp_cond(q, cond, true_val, false_val, max_recurse) {
        return Some(v);
    }
    if let Some(v) = select::simplify_select_with_bit_test(q, cond, true_val, false_val) {
        return Some(v);
    }
    if let Some(cxt) = q.cxt
        && let Some(implied) = conditions::is_implied_by_dom_condition_cond(&q.tracking(), cond, cxt)
    {
        return Some(if implied { true_val } else { false_val });
    }
    if q.is_phi(true_val) && q.is_phi(false_val) {
        if select::is_select_with_identical_phi(q, true_val, false_val) {
            return Some(false_val);
        }
        if select::is_select_with_identical_phi(q, false_val, true_val) {
            return Some(true_val);
        }
    }
    None
}

/// `simplifyICmpInst`
/// ([:3883](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L3883)):
/// the constant fold, poison, the compare of a value with itself, the bool and
/// zero forms, and the compares of an extension against an extension or a
/// constant, the binary-operator and min/max forms and the monotonic values.
/// DEVIATE: assume is not translated.
fn simplify_icmp(
    q: &mut Query,
    mut pred: IntPredicate,
    mut lhs: Operand,
    mut rhs: Operand,
    max_recurse: u32,
) -> Option<Operand> {
    if let Some(left) = q.constant(lhs) {
        if let Some(right) = q.constant(rhs) {
            return constfold::compare_in(q.context, q.globals, pred, left, right).map(Operand::Constant);
        }
        std::mem::swap(&mut lhs, &mut rhs);
        pred = swapped(pred);
    }
    let i1 = q.context.types.int(1);
    // icmp X, poison -> poison
    if q.is_poison(rhs) {
        return Some(q.poison(i1));
    }
    // icmp eq/ne X, undef -> undef: the undef can be chosen to pass or fail.
    if q.is_undef(rhs) && is_equality(pred) {
        return Some(q.undef(i1));
    }
    // icmp X, X -> isTrueWhenEqual(Pred); icmp X, undef likewise, undef could
    // be X.
    if lhs == rhs || q.is_undef(rhs) {
        let holds = matches!(
            pred,
            IntPredicate::Eq | IntPredicate::Uge | IntPredicate::Ule | IntPredicate::Sge | IntPredicate::Sle
        );
        return Some(q.int(i1, u128::from(holds)));
    }
    if let Some(v) = simplify_icmp_of_bools(q, pred, lhs, rhs) {
        return Some(v);
    }
    if let Some(v) = simplify_icmp_with_zero(q, pred, lhs, rhs) {
        return Some(v);
    }
    if let Some(v) = simplify_icmp_with_constant(q, pred, lhs, rhs) {
        return Some(v);
    }
    if let Some(v) = simplify_icmp_of_extensions(q, pred, lhs, rhs, max_recurse) {
        return Some(v);
    }
    // icmp eq/ne X, Y where X and Y are known to differ.
    if matches!(pred, IntPredicate::Eq | IntPredicate::Ne)
        && !q.is_zero(rhs)
        && nonzero::is_known_non_equal(&q.tracking(), lhs, rhs, 0)
    {
        return Some(q.int(i1, u128::from(pred == IntPredicate::Ne)));
    }
    if let Some(v) = simplify_icmp_with_bin_op(q, pred, lhs, rhs, max_recurse) {
        return Some(v);
    }
    if let Some(v) = simplify_icmp_with_min_max(q, pred, lhs, rhs, max_recurse) {
        return Some(v);
    }
    if let Some(v) = simplify_icmp_with_intrinsic_on_lhs(q, pred, lhs, rhs) {
        return Some(v);
    }
    if let Some(v) = simplify_icmp_with_intrinsic_on_lhs(q, swapped(pred), rhs, lhs) {
        return Some(v);
    }
    if let Some(v) = simplify_icmp_using_monotonic_values(q, pred, lhs, rhs) {
        return Some(v);
    }
    if let Some(v) = simplify_icmp_using_monotonic_values(q, swapped(pred), rhs, lhs) {
        return Some(v);
    }
    if let Some(v) = simplify_icmp_with_dominating_assume(q, pred, lhs, rhs) {
        return Some(v);
    }
    if let Some(cxt) = q.cxt
        && let Some(implied) = conditions::is_implied_by_dom_condition(&q.tracking(), pred, lhs, rhs, cxt)
    {
        return Some(q.int(i1, u128::from(implied)));
    }
    // If the comparison is with the result of a select or a phi instruction,
    // check whether comparing with either branch always yields the same value.
    if (q.is_select(lhs) || q.is_select(rhs))
        && let Some(v) = thread_cmp_over_select(q, pred, lhs, rhs, max_recurse)
    {
        return Some(v);
    }
    if q.is_phi(lhs) || q.is_phi(rhs) {
        return thread_cmp_over_phi(q, pred, lhs, rhs, max_recurse);
    }
    None
}

/// `simplifyICmpWithDominatingAssume`
/// ([:3811](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L3811)):
/// an `llvm.assume` valid at the context instruction implies the compare.
fn simplify_icmp_with_dominating_assume(
    q: &mut Query,
    pred: IntPredicate,
    lhs: Operand,
    rhs: Operand,
) -> Option<Operand> {
    // Gracefully handle instructions that have not been inserted yet.
    let (Some(analyses), Some(cxt)) = (q.analyses, q.cxt) else { return None };
    let tracking = q.tracking();
    let found = [lhs, rhs]
        .into_iter()
        .find_map(
            |base| analyses
                .assumption_cache()
                .assumptions_for(base)
                .iter()
                .find_map(
                    |&assume| {
                        let condition = q.function.instruction(assume).operands[0];
                        let implied = conditions::is_implied_condition_cmp(
                            &tracking,
                            condition,
                            pattern::CmpPredicate::new(pred),
                            pattern::Side::of(&tracking, lhs),
                            pattern::Side::of(&tracking, rhs),
                            true,
                            0,
                        )?;
                        conditions::is_valid_assume_for_context(&tracking, assume, cxt, false).then_some(implied)
                    },
                ),
        )?;
    let i1 = q.context.types.int(1);
    Some(q.int(i1, u128::from(found)))
}

/// `simplifyICmpWithIntrinsicOnLHS`
/// ([:3835](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L3835)).
fn simplify_icmp_with_intrinsic_on_lhs(
    q: &mut Query,
    pred: IntPredicate,
    lhs: Operand,
    rhs: Operand,
) -> Option<Operand> {
    let tracking = q.tracking();
    let (inst, _) = tracking.instruction_of(lhs)?;
    let id = pattern::call_intrinsic(&tracking, inst)?;
    let [x, y, _] = q.function.instruction(inst).operands[..] else { return None };
    let i1 = q.context.types.int(1);
    match id {
        // uadd.sat(X, Y) uge X + Y
        pattern::IntrinsicId::UaddSat => {
            let sum = q.binary_of(rhs, BinaryOp::Add).is_some_and(|(a, b, _)| (a == x && b == y) || (a == y && b == x));
            if sum && pred == IntPredicate::Uge {
                return Some(q.int(i1, 1));
            }
            if sum && pred == IntPredicate::Ult {
                return Some(q.int(i1, 0));
            }
            None
        }
        // usub.sat(X, Y) ule X - Y
        pattern::IntrinsicId::UsubSat => {
            let difference = q.binary_of(rhs, BinaryOp::Sub).is_some_and(|(a, b, _)| a == x && b == y);
            if difference && pred == IntPredicate::Ule {
                return Some(q.int(i1, 1));
            }
            if difference && pred == IntPredicate::Ugt {
                return Some(q.int(i1, 0));
            }
            None
        }
        _ => None,
    }
}

/// `CmpInst::getInversePredicate`.
fn inverse(p: IntPredicate) -> IntPredicate {
    match p {
        IntPredicate::Eq => IntPredicate::Ne,
        IntPredicate::Ne => IntPredicate::Eq,
        IntPredicate::Ugt => IntPredicate::Ule,
        IntPredicate::Uge => IntPredicate::Ult,
        IntPredicate::Ult => IntPredicate::Uge,
        IntPredicate::Ule => IntPredicate::Ugt,
        IntPredicate::Sgt => IntPredicate::Sle,
        IntPredicate::Sge => IntPredicate::Slt,
        IntPredicate::Slt => IntPredicate::Sge,
        IntPredicate::Sle => IntPredicate::Sgt,
    }
}

/// `m_SMax`, `m_SMin`, `m_UMax` and `m_UMin` (PatternMatch.h): a call of
/// `llvm.smax` and the like, with its two arguments. Known only where the
/// query has the module's globals.
fn min_max_call(
    q: &Query,
    v: Operand,
    signed_kind: bool,
    max: bool,
) -> Option<(Operand, Operand)> {
    let base = match (signed_kind, max) {
        (true, true) => "llvm.smax",
        (true, false) => "llvm.smin",
        (false, true) => "llvm.umax",
        (false, false) => "llvm.umin",
    };
    #[cfg(test)]
    MIN_MAX_ASKS.with(|asks| asks.set(asks.get() + 1));
    let tracking = q.tracking();
    let (_, args) = pattern::intrinsic_call(&tracking, v, base)?;
    let [a, b] = *args else { return None };
    Some((a, b))
}

// The intrinsic-name lookups `min_max_call` made, for the test that an
// icmp of two non-calls makes none.
#[cfg(test)]
thread_local! {
    pub(crate) static MIN_MAX_ASKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// `extractEquivalentCondition`
/// ([:2640](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L2640)):
/// the compare `v` selects on if it is `LHS pred RHS`.
fn extract_equivalent_condition(
    q: &Query,
    v: Operand,
    pred: IntPredicate,
    lhs: Operand,
    rhs: Operand,
) -> Option<Operand> {
    let select = q.function.instruction(q.definition(v)?);
    if select.opcode != Opcode::Select {
        return None;
    }
    let condition = select.operands[0];
    let cmp = q.function.instruction(q.definition(condition)?);
    let Opcode::ICmp(cmp_pred) = cmp.opcode else { return None };
    let [cmp_lhs, cmp_rhs] = cmp.operands[..] else { return None };
    if (pred == cmp_pred && lhs == cmp_lhs && rhs == cmp_rhs)
        || (pred == swapped(cmp_pred) && lhs == cmp_rhs && rhs == cmp_lhs)
    {
        return Some(condition);
    }
    None
}

/// `simplifyICmpWithMinMax`
/// ([:3626](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L3626)).
/// The C++ writes the signed and the unsigned variants out; they differ only
/// in the matchers, `EqP` and the predicates of the switch, so one function
/// takes the kind.
fn simplify_icmp_with_min_max(
    q: &mut Query,
    mut pred: IntPredicate,
    mut lhs: Operand,
    mut rhs: Operand,
    max_recurse: u32,
) -> Option<Operand> {
    // Every matcher below is a call of an intrinsic: with neither side a call
    // none matches, nothing swaps, and the answer is none.
    let is_call = |v: Operand| {
        q.definition(v).is_some_and(|inst| matches!(q.function.instruction(inst).opcode, Opcode::Call { .. }))
    };
    if !is_call(lhs) && !is_call(rhs) {
        return None;
    }
    let i1 = q.context.types.int(1);
    for signed_kind in [true, false] {
        // "A == max/min(A,B)" iff "A eq_p B"; `max_pred` is the predicate of
        // "max(A, B) p A" read as "A pred max/min".
        let (max_eq, min_eq, ge, gt, le, lt) = if signed_kind {
            (
                IntPredicate::Sge,
                IntPredicate::Sle,
                IntPredicate::Sge,
                IntPredicate::Sgt,
                IntPredicate::Sle,
                IntPredicate::Slt,
            )
        } else {
            (
                IntPredicate::Uge,
                IntPredicate::Ule,
                IntPredicate::Uge,
                IntPredicate::Ugt,
                IntPredicate::Ule,
                IntPredicate::Ult,
            )
        };
        let mut found = None;
        if let Some((a, b)) = min_max_call(q, lhs, signed_kind, true)
            && (a == rhs || b == rhs)
        {
            // max(A, B) pred A.
            found = Some((if a != rhs { b } else { a }, if a != rhs { a } else { b }, max_eq, pred));
        } else if let Some((a, b)) = min_max_call(q, rhs, signed_kind, true)
            && (a == lhs || b == lhs)
        {
            // A pred max(A, B), as max(A, B) swapped-pred A.
            found = Some((if a != lhs { b } else { a }, if a != lhs { a } else { b }, max_eq, swapped(pred)));
        } else if let Some((a, b)) = min_max_call(q, lhs, signed_kind, false)
            && (a == rhs || b == rhs)
        {
            // min(A, B) pred A, as max(-A, -B) swapped-pred -A.
            found = Some((if a != rhs { b } else { a }, if a != rhs { a } else { b }, min_eq, swapped(pred)));
        } else if let Some((a, b)) = min_max_call(q, rhs, signed_kind, false)
            && (a == lhs || b == lhs)
        {
            // A pred min(A, B), as max(-A, -B) pred -A.
            found = Some((if a != lhs { b } else { a }, if a != lhs { a } else { b }, min_eq, pred));
        }
        if let Some((a, b, eq_p, p)) = found {
            // Cases correspond to "max(A, B) p A".
            if p == IntPredicate::Eq || p == le {
                // Equivalent to "A EqP B"; this may be the condition tested in
                // the max/min, or simplify.
                if let Some(v) = extract_equivalent_condition(q, lhs, eq_p, a, b) {
                    return Some(v);
                }
                if let Some(v) = extract_equivalent_condition(q, rhs, eq_p, a, b) {
                    return Some(v);
                }
                if max_recurse > 0
                    && let Some(v) = simplify_icmp(q, eq_p, a, b, max_recurse - 1)
                {
                    return Some(v);
                }
            } else if p == IntPredicate::Ne || p == gt {
                let inv_eq_p = inverse(eq_p);
                if let Some(v) = extract_equivalent_condition(q, lhs, inv_eq_p, a, b) {
                    return Some(v);
                }
                if let Some(v) = extract_equivalent_condition(q, rhs, inv_eq_p, a, b) {
                    return Some(v);
                }
                if max_recurse > 0
                    && let Some(v) = simplify_icmp(q, inv_eq_p, a, b, max_recurse - 1)
                {
                    return Some(v);
                }
            } else if p == ge {
                return Some(q.int(i1, 1));
            } else if p == lt {
                return Some(q.null(i1));
            }
        }
    }
    // Comparing 1 each of min/max with a common operand? Canonicalize min
    // operand to RHS.
    if min_max_call(q, lhs, false, false).is_some() || min_max_call(q, lhs, true, false).is_some() {
        std::mem::swap(&mut lhs, &mut rhs);
        pred = swapped(pred);
    }
    let shares = |a: Operand, b: Operand, c: Operand, d: Operand| a == c || a == d || b == c || b == d;
    if let (Some((a, b)), Some((c, d))) = (min_max_call(q, lhs, true, true), min_max_call(q, rhs, true, false))
        && shares(a, b, c, d)
    {
        // smax(A, B) >=s smin(A, D) --> true; <s --> false
        if pred == IntPredicate::Sge {
            return Some(q.int(i1, 1));
        }
        if pred == IntPredicate::Slt {
            return Some(q.null(i1));
        }
    } else if let (Some((a, b)), Some((c, d))) = (min_max_call(q, lhs, false, true), min_max_call(q, rhs, false, false))
        && shares(a, b, c, d)
    {
        if pred == IntPredicate::Uge {
            return Some(q.int(i1, 1));
        }
        if pred == IntPredicate::Ult {
            return Some(q.null(i1));
        }
    }
    None
}

/// `CmpInst::isEquality`.
fn is_equality(p: IntPredicate) -> bool {
    matches!(p, IntPredicate::Eq | IntPredicate::Ne)
}

/// `CmpInst::isUnsigned`.
fn is_unsigned(p: IntPredicate) -> bool {
    matches!(
        p,
        IntPredicate::Ult | IntPredicate::Ule | IntPredicate::Ugt | IntPredicate::Uge
    )
}

/// `APInt::operator<<(const APInt &)` on `bits` of `width`: an amount of the
/// width or more shifts everything out.
fn shl_apint(
    bits: u128,
    amount: u128,
    width: u32,
) -> u128 {
    if amount >= u128::from(width) { 0 } else { (bits << amount) & mask(width) }
}

/// `simplifyICmpWithBinOpOnLHS`
/// ([:3247](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L3247)):
/// `icmp pred LBO, RHS`, where `lbo` is the binary operator on the left.
fn simplify_icmp_with_bin_op_on_lhs(
    q: &mut Query,
    pred: IntPredicate,
    lbo: Operand,
    rhs: Operand,
) -> Option<Operand> {
    let i1 = q.context.types.int(1);
    let (op, l0, l1, _) = q.binary(lbo)?;
    // icmp pred (or X, Y), X
    if op == BinaryOp::Or
        && let Some(y) = q.other_of(lbo, BinaryOp::Or, rhs)
        && matches!(pred, IntPredicate::Slt | IntPredicate::Sge)
    {
        let rhs_known = q.known(rhs);
        let y_known = q.known(y);
        if rhs_known.is_non_negative() && y_known.is_negative() {
            return Some(q.int(i1, u128::from(pred == IntPredicate::Slt)));
        }
        if rhs_known.is_negative() || y_known.is_non_negative() {
            return Some(q.int(i1, u128::from(pred != IntPredicate::Slt)));
        }
    }
    // icmp pred (urem X, Y), Y
    if op == BinaryOp::URem && l1 == rhs {
        match pred {
            IntPredicate::Sgt | IntPredicate::Sge if q.known(rhs).is_non_negative() => {
                return Some(q.null(i1));
            }
            IntPredicate::Eq | IntPredicate::Ugt | IntPredicate::Uge => return Some(q.null(i1)),
            IntPredicate::Slt | IntPredicate::Sle if q.known(rhs).is_non_negative() => {
                return Some(q.int(i1, 1));
            }
            IntPredicate::Ne | IntPredicate::Ult | IntPredicate::Ule => return Some(q.int(i1, 1)),
            _ => {}
        }
    }
    // If x is nonzero: x >>u C <u x --> true for C != 0, and the like for
    // x udiv C with C != 1.
    if l0 == rhs
        && let Some((c, _)) = q.number(l1)
        && ((op == BinaryOp::LShr && c != 0) || (op == BinaryOp::UDiv && c != 1))
        && q.is_known_non_zero(rhs)
    {
        match pred {
            IntPredicate::Eq | IntPredicate::Uge | IntPredicate::Ugt => return Some(q.null(i1)),
            IntPredicate::Ne | IntPredicate::Ult | IntPredicate::Ule => return Some(q.int(i1, 1)),
            _ => {}
        }
    }
    // (x*C1)/C2 <= x for C1 <= C2, and the shift forms.
    if matches!(op, BinaryOp::UDiv | BinaryOp::LShr)
        && let Some((c2, width)) = q.number(l1)
        && let Some((inner, i0, i1_, _)) = q.binary(l0)
        && i0 == rhs
        && let Some((c1, _)) = q.number(i1_)
        && match (op, inner) {
            (BinaryOp::UDiv, BinaryOp::Mul) => c1 <= c2,
            (BinaryOp::LShr, BinaryOp::Mul) => c1 <= shl_apint(1, c2, width),
            (BinaryOp::UDiv, BinaryOp::Shl) => shl_apint(1, c1, width) <= c2,
            _ => false,
        }
    {
        if pred == IntPredicate::Ugt {
            return Some(q.null(i1));
        }
        if pred == IntPredicate::Ule {
            return Some(q.int(i1, 1));
        }
    }
    // (sub C, X) == X, C is odd  --> false
    if op == BinaryOp::Sub && l1 == rhs && q.number(l0).is_some_and(|(c, _)| c & 1 == 1) && is_equality(pred) {
        return Some(q.int(i1, u128::from(pred != IntPredicate::Eq)));
    }
    None
}

/// `trySimplifyICmpWithAdds`
/// ([:3321](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L3321)).
fn try_simplify_icmp_with_adds(
    q: &Query,
    pred: IntPredicate,
    mut lhs: Operand,
    mut rhs: Operand,
) -> bool {
    if !is_signed(pred) {
        return false;
    }
    let nsw_add = |q: &Query, v: Operand| q.binary_of(v, BinaryOp::Add).is_some_and(|(_, _, f)| f.contains(Flags::NSW));
    if !nsw_add(q, rhs) {
        std::mem::swap(&mut lhs, &mut rhs);
    }
    if !nsw_add(q, rhs) {
        return false;
    }
    let (Some((x, c1, _)), Some((x2, c2, _))) = (q.binary_of(lhs, BinaryOp::Add), q.binary_of(rhs, BinaryOp::Add))
    else {
        return false;
    };
    let (Some((c1, width)), Some((c2, _))) = (q.number(c1), q.number(c2)) else { return false };
    if x2 != x {
        return false;
    }
    let (s1, s2) = (signed(c1, width), signed(c2, width));
    (s1 <= s2 && s1 >= 0) || (s2 <= s1 && s1 <= 0)
}

/// `simplifyICmpWithBinOp`
/// ([:3392](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L3392)).
fn simplify_icmp_with_bin_op(
    q: &mut Query,
    mut pred: IntPredicate,
    lhs: Operand,
    rhs: Operand,
    max_recurse: u32,
) -> Option<Operand> {
    let i1 = q.context.types.int(1);
    let lbo = q.binary(lhs);
    let rbo = q.binary(rhs);
    if max_recurse > 0 && (lbo.is_some() || rbo.is_some()) {
        // LHS = A + B (or A and B are null); RHS = C + D (or C and D are null).
        let no_wrap_problem = |bo: Option<(BinaryOp, Operand, Operand, Flags)>| match bo {
            Some((BinaryOp::Add, a, b, flags)) => (
                Some((a, b)),
                is_equality(pred)
                    || (is_unsigned(pred) && flags.contains(Flags::NUW))
                    || (is_signed(pred) && flags.contains(Flags::NSW)),
            ),
            _ => (None, false),
        };
        let (ab, no_lhs_wrap_problem) = no_wrap_problem(lbo);
        let (cd, no_rhs_wrap_problem) = no_wrap_problem(rbo);
        // icmp (X+Y), X -> icmp Y, 0 for equalities or if there is no overflow.
        if let Some((a, b)) = ab
            && (a == rhs || b == rhs)
            && no_lhs_wrap_problem
        {
            let zero = q.null(q.ty(rhs));
            if let Some(v) = simplify_icmp(q, pred, if a == rhs { b } else { a }, zero, max_recurse - 1) {
                return Some(v);
            }
        }
        // icmp X, (X+Y) -> icmp 0, Y for equalities or if there is no overflow.
        if let Some((c, d)) = cd
            && (c == lhs || d == lhs)
            && no_rhs_wrap_problem
        {
            let zero = q.null(q.ty(lhs));
            if let Some(v) = simplify_icmp(q, pred, zero, if c == lhs { d } else { c }, max_recurse - 1) {
                return Some(v);
            }
        }
        // icmp (X+Y), (X+Z) -> icmp Y,Z for equalities or if there is no
        // overflow.
        let can_simplify =
            (no_lhs_wrap_problem && no_rhs_wrap_problem) || try_simplify_icmp_with_adds(q, pred, lhs, rhs);
        if let (Some((a, b)), Some((c, d))) = (ab, cd)
            && (a == c || a == d || b == c || b == d)
            && can_simplify
        {
            let (y, z) = if a == c {
                (b, d)
            } else if a == d {
                (b, c)
            } else if b == c {
                (a, d)
            } else {
                (a, c)
            };
            if let Some(v) = simplify_icmp(q, pred, y, z, max_recurse - 1) {
                return Some(v);
            }
        }
    }
    if lbo.is_some()
        && let Some(v) = simplify_icmp_with_bin_op_on_lhs(q, pred, lhs, rhs)
    {
        return Some(v);
    }
    if rbo.is_some()
        && let Some(v) = simplify_icmp_with_bin_op_on_lhs(q, swapped(pred), rhs, lhs)
    {
        return Some(v);
    }
    // 0 - (zext X) pred C
    if !is_unsigned(pred)
        && let Some((neg, _)) = q.neg_of(lhs)
        && matches!(q.cast(neg), Some((CastOp::ZExt, ..)))
        && let Some((c, width)) = q.number(rhs)
    {
        let c = signed(c, width);
        if c > 0 {
            if matches!(pred, IntPredicate::Slt | IntPredicate::Ne) {
                return Some(q.int(i1, 1));
            }
            if matches!(pred, IntPredicate::Sge | IntPredicate::Eq) {
                return Some(q.null(i1));
            }
        }
        if c >= 0 {
            if pred == IntPredicate::Sle {
                return Some(q.int(i1, 1));
            }
            if pred == IntPredicate::Sgt {
                return Some(q.null(i1));
            }
        }
    }
    // If C2 is a power-of-2 and C is not: (C2 << X) == C --> false, != C -->
    // true.
    if let Some((BinaryOp::Shl, c2, _, flags)) = lbo
        && q.number(c2).is_some_and(|(c2, _)| c2.is_power_of_two())
    {
        if let Some((c, _)) = q.number(rhs)
            && !c.is_power_of_two()
            // C2 << X can equal zero in some circumstances; it is safe if the
            // shift is nsw or nuw, C2 is one, or C isn't zero.
            && (flags.contains(Flags::NSW) || flags.contains(Flags::NUW) || q.is_one(c2) || c != 0)
        {
            if pred == IntPredicate::Eq {
                return Some(q.null(i1));
            }
            if pred == IntPredicate::Ne {
                return Some(q.int(i1, 1));
            }
        }
        // If C is a power-of-2: (C << X) >u 0x8000 --> false, <=u --> true.
        if q.is_sign_mask(rhs) {
            if pred == IntPredicate::Ugt {
                return Some(q.null(i1));
            }
            if pred == IntPredicate::Ule {
                return Some(q.int(i1, 1));
            }
        }
    }
    let (Some((lop, l0, l1, lflags)), Some((rop, r0, r1, rflags))) = (lbo, rbo) else { return None };
    if max_recurse == 0 || lop != rop {
        return None;
    }
    let both = |flag: Flags| lflags.contains(flag) && rflags.contains(flag);
    if l0 == r0 {
        match lop {
            BinaryOp::Shl => {
                let (nuw, nsw) = (both(Flags::NUW), both(Flags::NSW));
                if nuw && !(is_signed(pred) && !nsw) && q.is_known_non_zero(l0) {
                    if let Some(v) = simplify_icmp(q, pred, l1, r1, max_recurse - 1) {
                        return Some(v);
                    }
                }
            }
            // If C1 & C2 == C1, A = X and/or C1, B = X and/or C2:
            // icmp ule A, B -> true; icmp ugt A, B -> false; and the signed
            // ones when C1 and C2 are the same sign.
            BinaryOp::And | BinaryOp::Or => {
                if !is_equality(pred)
                    && let (Some((c1, width)), Some((c2, _))) = (q.number(l1), q.number(r1))
                {
                    let (mut c1, mut c2) = (c1, c2);
                    if c1 & !c2 != 0 {
                        std::mem::swap(&mut c1, &mut c2);
                        pred = swapped(pred);
                    }
                    if c1 & !c2 == 0 {
                        if pred == IntPredicate::Ule {
                            return Some(q.int(i1, 1));
                        }
                        if pred == IntPredicate::Ugt {
                            return Some(q.null(i1));
                        }
                        let non_negative = |c: u128| c >> (width - 1) & 1 == 0;
                        if non_negative(c1) == non_negative(c2) {
                            if pred == IntPredicate::Sle {
                                return Some(q.int(i1, 1));
                            }
                            if pred == IntPredicate::Sgt {
                                return Some(q.null(i1));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if l1 == r1 {
        let exact = both(Flags::EXACT);
        let compare_first = match lop {
            BinaryOp::UDiv | BinaryOp::LShr => !is_signed(pred) && exact,
            BinaryOp::SDiv => is_equality(pred) && exact,
            BinaryOp::AShr => exact,
            BinaryOp::Shl => {
                let (nuw, nsw) = (both(Flags::NUW), both(Flags::NSW));
                (nuw || nsw) && (nsw || !is_signed(pred))
            }
            _ => false,
        };
        if compare_first && let Some(v) = simplify_icmp(q, pred, l0, r0, max_recurse - 1) {
            return Some(v);
        }
    }
    None
}

/// The zext/sext cases of `simplifyICmpInst` (:3883): the compare of an
/// extension against an extension or a constant.
fn simplify_icmp_of_extensions(
    q: &mut Query,
    pred: IntPredicate,
    lhs: Operand,
    rhs: Operand,
    max_recurse: u32,
) -> Option<Operand> {
    let i1 = q.context.types.int(1);
    let Some((first, src_op, _)) = q.cast(lhs).filter(|&(op, ..)| matches!(op, CastOp::ZExt | CastOp::SExt)) else {
        return None;
    };
    let src_ty = q.ty(src_op);
    let zero_extended = first == CastOp::ZExt;
    if let Some((second, other_src, _)) = q.cast(rhs).filter(|&(op, ..)| matches!(op, CastOp::ZExt | CastOp::SExt)) {
        if second == first {
            // zext X vs zext Y (sext vs sext) of one type: compare the sources.
            if max_recurse > 0 && q.ty(other_src) == src_ty {
                let p = if zero_extended { unsigned_predicate(pred) } else { pred };
                return simplify_icmp(q, p, src_op, other_src, max_recurse - 1);
            }
        } else if src_op == other_src {
            // zext X vs sext X: ULE, SGE true and UGT, SLT false; sext X vs
            // zext X: the mirror.
            let (always, never) = if zero_extended {
                ([IntPredicate::Ule, IntPredicate::Sge], [IntPredicate::Ugt, IntPredicate::Slt])
            } else {
                ([IntPredicate::Uge, IntPredicate::Sle], [IntPredicate::Ult, IntPredicate::Sgt])
            };
            if always.contains(&pred) {
                return Some(q.int(i1, 1));
            }
            if never.contains(&pred) {
                return Some(q.null(i1));
            }
        }
        return None;
    }
    // An extension against a constant: the constant truncated and extended
    // again.
    let (c, dst_width) = q.number(rhs)?;
    let src_width = q.width(src_op)?;
    let truncated = c & mask(src_width);
    let extended = if zero_extended { truncated } else { (signed(truncated, src_width) as u128) & mask(dst_width) };
    let any_eq = extended == c;
    if any_eq {
        if max_recurse > 0 {
            let p = if zero_extended { unsigned_predicate(pred) } else { pred };
            let narrow = q.int(src_ty, truncated);
            return simplify_icmp(q, p, src_op, narrow, max_recurse - 1);
        }
        return None;
    }
    let negative = signed(c, dst_width) < 0;
    let answer = |q: &mut Query, value: bool| Some(q.int(i1, u128::from(value)));
    if zero_extended {
        match pred {
            IntPredicate::Eq | IntPredicate::Ugt | IntPredicate::Uge => answer(q, false),
            IntPredicate::Ne | IntPredicate::Ult | IntPredicate::Ule => answer(q, true),
            IntPredicate::Sgt | IntPredicate::Sge => answer(q, negative),
            IntPredicate::Slt | IntPredicate::Sle => answer(q, !negative),
        }
    } else {
        match pred {
            IntPredicate::Eq => answer(q, false),
            IntPredicate::Ne => answer(q, true),
            IntPredicate::Sgt | IntPredicate::Sge => answer(q, negative),
            IntPredicate::Slt | IntPredicate::Sle => answer(q, !negative),
            // An unsigned compare of a sign extension against a constant it
            // cannot equal is a sign test of the source.
            IntPredicate::Ugt | IntPredicate::Uge if max_recurse > 0 => {
                let zero = q.null(src_ty);
                simplify_icmp(q, IntPredicate::Slt, src_op, zero, max_recurse - 1)
            }
            IntPredicate::Ult | IntPredicate::Ule if max_recurse > 0 => {
                let zero = q.null(src_ty);
                simplify_icmp(q, IntPredicate::Sge, src_op, zero, max_recurse - 1)
            }
            _ => None,
        }
    }
}

/// `getUnsignedMonotonicValues`
/// ([:3170](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L3170)).
fn unsigned_monotonic_values(
    q: &Query,
    result: &mut Vec<Operand>,
    v: Operand,
    greater_eq: bool,
    depth: u32,
) {
    if result.contains(&v) {
        return;
    }
    result.push(v);
    if depth + 1 > 1 {
        return;
    }
    let depth = depth + 1;
    // m_Intrinsic<uadd_sat>(X, Y) like the or; m_Intrinsic<usub_sat>(X) as the
    // and's first operand.
    let saturating = {
        let tracking = q.tracking();
        let call = tracking
            .instruction_of(v)
            .and_then(|(inst, _)| pattern::call_intrinsic(&tracking, inst).map(|id| (inst, id)));
        call.map(|(inst, id)| (id, q.function.instruction(inst).operands.to_vec()))
    };
    match saturating {
        Some((pattern::IntrinsicId::UaddSat, args)) if greater_eq => {
            unsigned_monotonic_values(q, result, args[0], greater_eq, depth);
            unsigned_monotonic_values(q, result, args[1], greater_eq, depth);
        }
        Some((pattern::IntrinsicId::UsubSat, args)) if !greater_eq => {
            unsigned_monotonic_values(q, result, args[0], greater_eq, depth);
        }
        _ => {}
    }
    let Some((op, x, y, flags)) = q.binary(v) else { return };
    if greater_eq {
        if op == BinaryOp::Or {
            unsigned_monotonic_values(q, result, x, greater_eq, depth);
            unsigned_monotonic_values(q, result, y, greater_eq, depth);
        }
        if op == BinaryOp::Mul && flags.contains(Flags::NUW) {
            if q.is_known_non_zero(x) {
                unsigned_monotonic_values(q, result, y, greater_eq, depth);
            }
            if q.is_known_non_zero(y) {
                unsigned_monotonic_values(q, result, x, greater_eq, depth);
            }
        }
    } else {
        match op {
            BinaryOp::And => {
                unsigned_monotonic_values(q, result, x, greater_eq, depth);
                unsigned_monotonic_values(q, result, y, greater_eq, depth);
            }
            BinaryOp::URem | BinaryOp::UDiv | BinaryOp::LShr => {
                unsigned_monotonic_values(q, result, x, greater_eq, depth);
            }
            _ => {}
        }
    }
}

/// `simplifyICmpUsingMonotonicValues`
/// ([:3228](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L3228)).
fn simplify_icmp_using_monotonic_values(
    q: &mut Query,
    pred: IntPredicate,
    lhs: Operand,
    rhs: Operand,
) -> Option<Operand> {
    if !matches!(pred, IntPredicate::Uge | IntPredicate::Ult) {
        return None;
    }
    // We have LHS uge GreaterValues and LowerValues uge RHS. If any of the
    // GreaterValues and LowerValues are the same, it follows that LHS uge RHS.
    let (mut greater, mut lower) = (Vec::new(), Vec::new());
    unsigned_monotonic_values(q, &mut greater, lhs, true, 0);
    unsigned_monotonic_values(q, &mut lower, rhs, false, 0);
    if greater.iter().any(|value| lower.contains(value)) {
        let i1 = q.context.types.int(1);
        return Some(q.int(i1, u128::from(pred == IntPredicate::Uge)));
    }
    None
}

/// `CmpInst::getSwappedPredicate`.
fn swapped(p: IntPredicate) -> IntPredicate {
    match p {
        IntPredicate::Eq => IntPredicate::Eq,
        IntPredicate::Ne => IntPredicate::Ne,
        IntPredicate::Ugt => IntPredicate::Ult,
        IntPredicate::Uge => IntPredicate::Ule,
        IntPredicate::Ult => IntPredicate::Ugt,
        IntPredicate::Ule => IntPredicate::Uge,
        IntPredicate::Sgt => IntPredicate::Slt,
        IntPredicate::Sge => IntPredicate::Sle,
        IntPredicate::Slt => IntPredicate::Sgt,
        IntPredicate::Sle => IntPredicate::Sge,
    }
}

/// `ICmpInst::getUnsignedPredicate`.
fn unsigned_predicate(p: IntPredicate) -> IntPredicate {
    match p {
        IntPredicate::Sgt => IntPredicate::Ugt,
        IntPredicate::Sge => IntPredicate::Uge,
        IntPredicate::Slt => IntPredicate::Ult,
        IntPredicate::Sle => IntPredicate::Ule,
        other => other,
    }
}

/// `CmpInst::isSigned`.
fn is_signed(p: IntPredicate) -> bool {
    matches!(
        p,
        IntPredicate::Slt | IntPredicate::Sle | IntPredicate::Sgt | IntPredicate::Sge
    )
}

/// `simplifyICmpWithConstant`
/// ([:3124](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L3124)).
/// DEVIATE: the `bitcast (uitofp X)` sign-bit check is floating point.
fn simplify_icmp_with_constant(
    q: &mut Query,
    pred: IntPredicate,
    lhs: Operand,
    rhs: Operand,
) -> Option<Operand> {
    let (c, width) = q.number(rhs)?;
    let i1 = q.context.types.int(1);
    let rhs_range = ConstantRange::make_exact_icmp_region(pred, width, c);
    if rhs_range.is_empty_set() {
        return Some(q.null(i1));
    }
    if rhs_range.is_full_set() {
        return Some(q.int(i1, 1));
    }
    let lhs_range = q.range(lhs, is_signed(pred));
    if !lhs_range.is_full_set() {
        if rhs_range.contains_range(&lhs_range) {
            return Some(q.int(i1, 1));
        }
        if rhs_range.inverse().contains_range(&lhs_range) {
            return Some(q.null(i1));
        }
    }
    // icmp eq/ne (mul nuw X, M), C where M does not divide C.
    if matches!(pred, IntPredicate::Eq | IntPredicate::Ne)
        && let Some((BinaryOp::Mul, a, b, flags)) = q.binary(lhs)
        && let Some((m, _)) = q.number(b).or_else(|| q.number(a))
        && m != 0
        && ((flags.contains(Flags::NUW) && c % m != 0)
            || (flags.contains(Flags::NSW)
                && signed(c, width).checked_rem(signed(m, width)).is_some_and(|rest| rest != 0)))
    {
        return Some(q.int(i1, u128::from(pred == IntPredicate::Ne)));
    }
    if pred == IntPredicate::Uge && c == 1 && q.is_known_non_zero(lhs) {
        return Some(q.int(i1, 1));
    }
    None
}

/// `simplifyICmpOfBools`
/// ([:2905](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L2905)).
fn simplify_icmp_of_bools(
    q: &mut Query,
    pred: IntPredicate,
    lhs: Operand,
    rhs: Operand,
) -> Option<Operand> {
    if q.width(lhs) != Some(1) {
        return None;
    }
    let i1 = q.context.types.int(1);
    if q.is_zero(rhs) {
        match pred {
            // X != 0 -> X; X >u 0 -> X; X <s 0 -> X
            IntPredicate::Ne | IntPredicate::Ugt | IntPredicate::Slt => return Some(lhs),
            // not(X) == 0 -> X; not(X) <=u 0 -> X; not(X) >=s 0 -> X
            IntPredicate::Eq | IntPredicate::Ule | IntPredicate::Sge => {
                if let Some(x) = q.not_of(lhs) {
                    return Some(x);
                }
            }
            // X <u 0 -> false; X >s 0 -> false
            IntPredicate::Ult | IntPredicate::Sgt => return Some(q.null(i1)),
            // X >=u 0 -> true; X <=s 0 -> true
            IntPredicate::Uge | IntPredicate::Sle => return Some(q.int(i1, 1)),
        }
    } else if q.is_one(rhs) {
        match pred {
            // X == 1 -> X; X >=u 1 -> X; X <=s -1 -> X
            IntPredicate::Eq | IntPredicate::Uge | IntPredicate::Sle => return Some(lhs),
            // not(X) != 1 -> X; not(X) <u 1 -> X; not(X) >s -1... -> X
            IntPredicate::Ne | IntPredicate::Ult | IntPredicate::Sgt => {
                if let Some(x) = q.not_of(lhs) {
                    return Some(x);
                }
            }
            // X >u 1 -> false; X <s -1 -> false
            IntPredicate::Ugt | IntPredicate::Slt => return Some(q.null(i1)),
            // X <=u 1 -> true; X >=s -1 -> true
            IntPredicate::Ule | IntPredicate::Sge => return Some(q.int(i1, 1)),
        }
    }
    // For a signed comparison the i1 values are 0 and -1, so SGE is "RHS
    // implies LHS" and SLE the reverse.
    let (from, to) = match pred {
        IntPredicate::Uge | IntPredicate::Sle => (rhs, lhs),
        IntPredicate::Sge | IntPredicate::Ule => (lhs, rhs),
        _ => return None,
    };
    if q.is_implied(from, to) {
        return Some(q.int(i1, 1));
    }
    None
}

/// `matchEquivZeroRHS`: whether the compare against `rhs` is one against zero,
/// and the predicate it is then.
fn match_equivalent_zero_rhs(
    q: &Query,
    pred: IntPredicate,
    rhs: Operand,
) -> Option<IntPredicate> {
    if q.is_zero(rhs) {
        return Some(pred);
    }
    if q.is_all_ones(rhs) {
        return match pred {
            IntPredicate::Sgt => Some(IntPredicate::Sge),
            IntPredicate::Sle => Some(IntPredicate::Slt),
            _ => None,
        };
    }
    if q.is_one(rhs) {
        return match pred {
            IntPredicate::Sge => Some(IntPredicate::Sgt),
            IntPredicate::Uge => Some(IntPredicate::Ugt),
            IntPredicate::Slt => Some(IntPredicate::Sle),
            IntPredicate::Ult => Some(IntPredicate::Ule),
            _ => None,
        };
    }
    None
}

/// `simplifyICmpWithZero`
/// ([:3063](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L3063)).
fn simplify_icmp_with_zero(
    q: &mut Query,
    pred: IntPredicate,
    lhs: Operand,
    rhs: Operand,
) -> Option<Operand> {
    let pred = match_equivalent_zero_rhs(q, pred, rhs)?;
    let i1 = q.context.types.int(1);
    let (yes, no) = (true, false);
    let answer = |q: &mut Query, value: bool| Some(q.int(i1, u128::from(value)));
    match pred {
        IntPredicate::Ult => answer(q, no),
        IntPredicate::Uge => answer(q, yes),
        IntPredicate::Eq | IntPredicate::Ule => {
            if q.is_known_non_zero(lhs) {
                return answer(q, no);
            }
            None
        }
        IntPredicate::Ne | IntPredicate::Ugt => {
            if q.is_known_non_zero(lhs) {
                return answer(q, yes);
            }
            None
        }
        IntPredicate::Slt => {
            let known = q.known(lhs);
            if known.is_negative() {
                return answer(q, yes);
            }
            if known.is_non_negative() {
                return answer(q, no);
            }
            None
        }
        IntPredicate::Sle => {
            let known = q.known(lhs);
            if known.is_negative() {
                return answer(q, yes);
            }
            if known.is_non_negative() && q.is_known_non_zero(lhs) {
                return answer(q, no);
            }
            None
        }
        IntPredicate::Sge => {
            let known = q.known(lhs);
            if known.is_negative() {
                return answer(q, no);
            }
            if known.is_non_negative() {
                return answer(q, yes);
            }
            None
        }
        IntPredicate::Sgt => {
            let known = q.known(lhs);
            if known.is_negative() {
                return answer(q, no);
            }
            if known.is_non_negative() && q.is_known_non_zero(lhs) {
                return answer(q, yes);
            }
            None
        }
    }
}

/// `simplifyGEPInst`
/// ([:5302](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L5302)):
/// no index, all indices zero, a poison pointer or index. DEVIATE: the
/// zero-size element (`getTypeAllocSize`), the `ptrtoint` difference forms and
/// the constant `getelementptr` expression need the data layout or constant
/// expressions.
fn simplify_gep(
    q: &mut Query,
    source: TypeId,
    ptr: Operand,
    indices: &[Operand],
    flags: Flags,
) -> Option<Operand> {
    if indices.is_empty() {
        return Some(ptr);
    }
    // gep P, 0, 0... -> P
    if indices.iter().all(|&index| q.is_zero(index)) {
        return Some(ptr);
    }
    // gep poison, ... / gep P, poison -> poison
    if q.is_poison(ptr) || indices.iter().any(|&index| q.is_poison(index)) {
        let ty = q.ty(ptr);
        return Some(q.poison(ty));
    }
    // getelementptr undef, idx -> undef
    if q.is_undef(ptr) {
        let ty = q.ty(ptr);
        return Some(q.undef(ty));
    }
    // Check to see if this is constant foldable.
    let ptr_c = q.constant(ptr)?;
    let indices_c = indices.iter().map(|&index| q.constant(index)).collect::<Option<Vec<_>>>()?;
    if !constantfolding::integer_indices(q.context, &indices_c) {
        return None;
    }
    let nw = constantfolding::nw_of_flags(flags);
    let env = constantfolding::Env { globals: q.globals, layout: q.layout };
    let folded =
        match constantfolding::constant_expr_get_gep_with_layout(q.context, q.layout, source, ptr_c, &indices_c, nw) {
            Some(expr) => constantfolding::constant_fold_constant(q.context, env, expr),
            None => constantfolding::constant_fold_get_element_ptr(q.context, source, ptr_c, &indices_c, nw),
        };
    Some(Operand::Constant(folded))
}

/// `simplifyInstruction`
/// ([:7741](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L7741),
/// `simplifyLoadInst`
/// ([:7702](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L7702)):
/// the value a non-volatile load of a constant address reads from a constant
/// global's definitive initializer (`ConstantFoldLoadFromConstPtr`), and one
/// of an instruction over a constant global (uniform initializer, or a constant
/// offset).
fn simplify_load(
    q: &Query,
    volatile: bool,
    ptr: Operand,
    ty: TypeId,
) -> Option<Operand> {
    if volatile {
        return None;
    }
    let ptr = match ptr {
        Operand::Constant(ptr) => ptr,
        Operand::Value(_) => {
            let (object, _) = crate::valuetracking::underlying(q.context, q.layout, q.function, ptr);
            let Operand::Constant(object) = object else { return None };
            // The offset counts only where stripping constant steps reaches the
            // object.
            let (base, offset) =
                crate::valuetracking::pointer_base_with_constant_offset(q.context, q.layout, q.function, ptr);
            let offset = (base == Operand::Constant(object)).then_some(offset);
            return constantfolding::constant_fold_load_from_global_object(
                q.context, q.layout, q.globals, object, offset, ty,
            )
            .map(Operand::Constant);
        }
        Operand::Block(_) => return None,
    };
    constantfolding::constant_fold_load_from_const_ptr(q.context, q.layout, q.globals, ptr, ty).map(Operand::Constant)
}

/// `simplifyInstructionWithOperands`): the value `inst` equals, or None.
pub fn simplify_instruction(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    inst: InstId,
) -> Option<Operand> {
    simplify_instruction_with(
        Query { context, function, layout, globals: &[], analyses: None, cxt: None, can_use_undef: true },
        inst,
    )
}

/// `simplifyInstruction` for a module's function: `globals` name the callees,
/// so the intrinsics (`llvm.smax`...) are known, and `analyses` give the
/// dominator tree and the assumptions.
pub fn simplify_instruction_in(
    context: &Context,
    globals: &[GlobalValue],
    layout: &DataLayout,
    analyses: Option<&FunctionAnalyses>,
    function: &Function,
    inst: InstId,
) -> Option<Operand> {
    simplify_instruction_with(
        Query { context, function, layout, globals, analyses, cxt: None, can_use_undef: true },
        inst,
    )
}

fn simplify_instruction_with(
    mut q: Query,
    inst: InstId,
) -> Option<Operand> {
    q.cxt = Some(inst);
    let operands = q.function.instruction(inst).operands.to_vec();
    simplify_instruction_with_operands(&mut q, inst, &operands, RECURSION_LIMIT)
}

/// `simplifyInstructionWithOperands`
/// ([:7741](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L7741)):
/// what `inst` equals with `operands` in place of its own. DEVIATE: calls and
/// the other opcodes are not simplified (the C++ constant-folds them).
fn simplify_instruction_with_operands(
    q: &mut Query,
    inst: InstId,
    operands: &[Operand],
    max_recurse: u32,
) -> Option<Operand> {
    let function = q.function;
    let instruction = function.instruction(inst);
    let flags = instruction.flags;
    match &instruction.opcode {
        Opcode::Binary(op) => {
            let [lhs, rhs] = operands[..] else { return None };
            simplify_binop_flags(q, *op, lhs, rhs, flags, max_recurse)
        }
        Opcode::FNeg => fp::simplify_fneg(q, operands[0]),
        Opcode::ICmp(pred) => {
            let [lhs, rhs] = operands[..] else { return None };
            simplify_icmp(q, *pred, lhs, rhs, max_recurse)
        }
        Opcode::FCmp(pred) => {
            let [lhs, rhs] = operands[..] else { return None };
            fp::simplify_fcmp(q, *pred, lhs, rhs, flags)
        }
        Opcode::Select => {
            let [cond, a, b] = operands[..] else { return None };
            simplify_select(q, cond, a, b, max_recurse)
        }
        Opcode::Cast(op) => simplify_cast(q, *op, operands[0], instruction.ty, max_recurse),
        Opcode::Phi => simplify_phi(q, inst),
        Opcode::Load { volatile, .. } => simplify_load(q, *volatile, operands[0], instruction.ty),
        Opcode::Freeze => simplify_freeze(q, operands[0]),
        Opcode::ExtractValue(indices) => simplify_extract_value(q, operands[0], indices),
        Opcode::InsertValue(indices) => simplify_insert_value(q, operands[0], operands[1], indices),
        Opcode::Call(_) => calls::simplify_call(q, inst, operands),
        Opcode::GetElementPtr { source } => simplify_gep(q, *source, operands[0], &operands[1..], flags),
        _ => None,
    }
}

/// `simplifyExtractValueInst`
/// ([:5526](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/InstructionSimplify.cpp#L5526)):
/// a constant aggregate's member, or the value an `insertvalue` of the same path put in.
/// DEVIATE: the `umul.with.overflow` by one case is `lazyvalueinfo`'s alone.
fn simplify_extract_value(
    q: &Query,
    agg: Operand,
    indices: &[u32],
) -> Option<Operand> {
    if let Operand::Constant(c) = agg {
        return constantfolding::constant_fold_extract_value_instruction(q.context, c, indices).map(Operand::Constant);
    }
    // extractvalue x, (insertvalue y, elt, n), n -> elt
    let mut seen = Vec::new();
    let mut at = agg;
    while let Some(def) = q.definition(at) {
        let insert = q.function.instruction(def);
        let Opcode::InsertValue(put) = &insert.opcode else { break };
        // An `insertvalue` cycle is unreachable code's.
        if seen.contains(&def) {
            break;
        }
        seen.push(def);
        let common = put.len().min(indices.len());
        if put[..common] == indices[..common] {
            if put.len() == indices.len() {
                return Some(insert.operands[1]);
            }
            break;
        }
        at = insert.operands[0];
    }
    None
}

/// `simplifyInsertValueInst`
/// ([:5588](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/InstructionSimplify.cpp#L5588)):
/// two constants are one, and putting back what was just read is no change.
fn simplify_insert_value(
    q: &Query,
    agg: Operand,
    value: Operand,
    indices: &[u32],
) -> Option<Operand> {
    if let (Operand::Constant(c_agg), Operand::Constant(c_val)) = (agg, value) {
        return constantfolding::constant_fold_insert_value_instruction(q.context, c_agg, c_val, indices)
            .map(Operand::Constant);
    }
    // insertvalue x, (extractvalue x, n), n -> x
    let def = q.definition(value)?;
    let read = q.function.instruction(def);
    (matches!(&read.opcode, Opcode::ExtractValue(path) if path[..] == *indices) && read.operands[0] == agg)
        .then_some(agg)
}

/// `simplifyBinOp` for a caller that has operands and no instruction (a
/// builder).
pub fn simplify_binop_operands(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    opcode: BinaryOp,
    flags: Flags,
    lhs: Operand,
    rhs: Operand,
) -> Option<Operand> {
    let mut q = Query { context, function, layout, globals: &[], analyses: None, cxt: None, can_use_undef: true };
    simplify_binop_flags(&mut q, opcode, lhs, rhs, flags, RECURSION_LIMIT)
}

/// `simplifyBinOp(Opcode, LHS, RHS, SQ.getWithInstruction(&cxt))` for a caller
/// that has a module (its callees, dominator tree and assumptions) and the
/// instruction it asks at.
pub fn simplify_binop_at(
    context: &Context,
    globals: &[GlobalValue],
    layout: &DataLayout,
    analyses: Option<&FunctionAnalyses>,
    function: &Function,
    cxt: InstId,
    opcode: BinaryOp,
    lhs: Operand,
    rhs: Operand,
) -> Option<Operand> {
    let mut q = Query { context, function, layout, globals, analyses, cxt: Some(cxt), can_use_undef: true };
    simplify_binop_flags(&mut q, opcode, lhs, rhs, Flags::default(), RECURSION_LIMIT)
}

/// `simplifyBinOp(Opcode, LHS, RHS,
/// SQ.getWithInstruction(&cxt).getWithoutUndef())`: `simplify_binop_at` where
/// an `undef` operand is not read as a value of the caller's choosing.
pub fn simplify_binop_at_without_undef(
    context: &Context,
    globals: &[GlobalValue],
    layout: &DataLayout,
    analyses: Option<&FunctionAnalyses>,
    function: &Function,
    cxt: InstId,
    opcode: BinaryOp,
    lhs: Operand,
    rhs: Operand,
) -> Option<Operand> {
    let mut q = Query { context, function, layout, globals, analyses, cxt: Some(cxt), can_use_undef: false };
    simplify_binop_flags(&mut q, opcode, lhs, rhs, Flags::default(), RECURSION_LIMIT)
}

/// `simplifyBinOp(Opcode, LHS, RHS, FMF, SQ.getWithInstruction(&cxt))`:
/// `simplify_binop_at` with the fast-math flags of the operation.
pub fn simplify_binop_with_flags_at(
    context: &Context,
    globals: &[GlobalValue],
    layout: &DataLayout,
    analyses: Option<&FunctionAnalyses>,
    function: &Function,
    cxt: InstId,
    opcode: BinaryOp,
    flags: Flags,
    lhs: Operand,
    rhs: Operand,
) -> Option<Operand> {
    let mut q = Query { context, function, layout, globals, analyses, cxt: Some(cxt), can_use_undef: true };
    simplify_binop_flags(&mut q, opcode, lhs, rhs, flags, RECURSION_LIMIT)
}

/// `simplifyWithOpReplaced(V, Op, RepOp, SQ.getWithInstruction(&cxt),
/// /*AllowRefinement=*/true)`: what `v` equals with `rep_op` in place of `op`.
pub fn simplify_with_op_replaced_at(
    context: &Context,
    globals: &[GlobalValue],
    layout: &DataLayout,
    analyses: Option<&FunctionAnalyses>,
    function: &Function,
    cxt: InstId,
    v: Operand,
    op: Operand,
    rep_op: Operand,
) -> Option<Operand> {
    let mut q = Query { context, function, layout, globals, analyses, cxt: Some(cxt), can_use_undef: true };
    simplify_with_op_replaced(&mut q, v, op, rep_op, RECURSION_LIMIT)
}

/// `simplifyInstructionWithOperands(&I, Ops, SQ.getWithInstruction(&cxt))`:
/// what `inst` equals with `operands` in place of its own, asked at `cxt`
/// (none: `SQ` of the data layout alone).
pub fn simplify_instruction_operands_at(
    context: &mut Context,
    globals: &[GlobalValue],
    layout: &DataLayout,
    analyses: Option<&FunctionAnalyses>,
    function: &Function,
    cxt: Option<InstId>,
    inst: InstId,
    operands: &[Operand],
) -> Option<Operand> {
    let mut q = Query { context, function, layout, globals, analyses, cxt, can_use_undef: true };
    simplify_instruction_with_operands(&mut q, inst, operands, RECURSION_LIMIT)
}

/// `simplifyICmpInst` for operands.
pub fn simplify_icmp_operands(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    predicate: IntPredicate,
    lhs: Operand,
    rhs: Operand,
) -> Option<Operand> {
    let mut q = Query { context, function, layout, globals: &[], analyses: None, cxt: None, can_use_undef: true };
    simplify_icmp(&mut q, predicate, lhs, rhs, RECURSION_LIMIT)
}

/// `simplifyCastInst` for operands.
pub fn simplify_cast_operand(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    op: CastOp,
    value: Operand,
    to: TypeId,
) -> Option<Operand> {
    let mut q = Query { context, function, layout, globals: &[], analyses: None, cxt: None, can_use_undef: true };
    simplify_cast(&mut q, op, value, to, RECURSION_LIMIT)
}

/// `simplifyGEPInst` for operands.
pub fn simplify_gep_operands(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    source: TypeId,
    ptr: Operand,
    indices: &[Operand],
    flags: Flags,
) -> Option<Operand> {
    let mut q = Query { context, function, layout, globals: &[], analyses: None, cxt: None, can_use_undef: true };
    simplify_gep(&mut q, source, ptr, indices, flags)
}

/// `simplifySelectInst` for operands.
pub fn simplify_select_operands(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    condition: Operand,
    if_true: Operand,
    if_false: Operand,
) -> Option<Operand> {
    let mut q = Query { context, function, layout, globals: &[], analyses: None, cxt: None, can_use_undef: true };
    simplify_select(&mut q, condition, if_true, if_false, RECURSION_LIMIT)
}

#[path = "simplify_calls.rs"]
mod calls;

#[path = "simplify_select.rs"]
mod select;

#[path = "simplify_fp.rs"]
mod fp;

#[cfg(test)]
#[path = "simplify_tests.rs"]
mod tests;
