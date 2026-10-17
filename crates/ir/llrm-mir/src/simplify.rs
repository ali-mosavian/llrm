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
//! MIRROR: `simplifyInstruction`'s dispatch; `simplifyAddInst`,
//! `simplifySubInst`, `simplifyMulInst`, `simplifyDivRem`, `simplifyShift` and
//! the shift, and/or/xor functions, `simplifyAssociativeBinOp`,
//! `simplifyLogicOfAddSub`, `simplifySelectInst`, `simplifyCastInst`,
//! `simplifyPHINode`, and the compare-of-self, bool and zero parts of
//! `simplifyICmpInst`.
//!
//! DEVIATE, with the reason:
//! - `undef` (MIR has none), vectors, floating point, pointers, intrinsics
//!   (`fshl`, `fshr`, `llvm.umin`...), `llvm.assume`: not in the integer
//!   subset.
//! - `threadBinOpOverSelect` / `threadBinOpOverPHI`, `expandBinOp` /
//!   `expandCommutativeBinOp`, `simplifyByDomEq`, `isImpliedCondition`,
//!   `isImpliedByDomCondition`: each needs a dominator tree or the condition
//!   machinery; MIR's `Dominators` is an analysis, not in `llrm-mir`'s query.
//!   Their cases are not found; a rule that needs one asks the analysis.
//! - `computeKnownBits` is [`crate::valuetracking::known_zero`], the zero half
//!   only: a check on `Known.One` is not made; `ComputeNumSignBits` is
//!   [`crate::valuetracking::sign_bits`]. `isKnownToBeAPowerOfTwo`,
//!   `isKnownNonZero` and `ConstantRange` are not translated.
//! - `nsw`/`nuw`/`exact` are read from the instruction (`IIQ.UseInstrInfo`).
//!
//! # Used by
//! `earlycse`, the combine worklist, `gvn`, `fold`.

use crate::constantrange::ConstantRange;
use crate::constfold;
use crate::context::{Constant, ConstantId, ConstantKind, Context, mask, signed};
use crate::datalayout::DataLayout;
use crate::knownbits::KnownBits;
use crate::module::{Function, InstId, Operand, ValueDef};
use crate::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use crate::types::TypeId;
use crate::valuetracking::{Query as ValueQuery, context as conditions, known, nonzero, power2, range};

/// `RecursionLimit` in InstructionSimplify.cpp.
const RECURSION_LIMIT: u32 = 3;

/// The `SimplifyQuery`: what an operand is, looked up in `function`.
struct Query<'a> {
    context: &'a mut Context,
    function: &'a Function,
    layout: &'a DataLayout,
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
        ValueQuery::bare(self.context, self.function, self.layout)
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

    /// `isImpliedCondition(LHS, RHS, DL).value_or(false)`.
    fn is_implied(
        &self,
        lhs: Operand,
        rhs: Operand,
    ) -> bool {
        conditions::is_implied_condition(&self.tracking(), lhs, rhs, true, 0).unwrap_or(false)
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
    // Evaluate the expression "B0 op OtherOp" and "B1 op OtherOp".
    let l = simplify_binop(q, opcode, b0, other_op, max_recurse)?;
    let r = simplify_binop(q, opcode, b1, other_op, max_recurse)?;
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
    // (sub nuw C_Mask, (xor X, C_Mask)) -> X
    if nuw
        && let Some(x) = q.other_of(op1, BinaryOp::Xor, op0)
        && q.is_low_bit_mask(op0)
    {
        return Some(x);
    }
    None
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
    // X * 0 -> 0
    if q.is_zero(op1) {
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
    expand_commutative_bin_op(q, BinaryOp::Mul, op0, op1, BinaryOp::Add, max_recurse)
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
    if q.is_poison(op1) {
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
    // simplifyByDomEq, and the select and phi threading: DEVIATE (see the
    // module header).
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
    // Shift amount of the width or more is poison.
    if let Some((amount, width)) = q.number(op1)
        && amount >= u128::from(width)
    {
        let ty = q.ty(op0);
        return Some(q.poison(ty));
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
        let mut known_shl = KnownBits::shl(&known_value, &known_amount, false, true, false);
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
) -> Option<Operand> {
    if let Some(v) = simplify_shift(q, opcode, op0, op1, false) {
        return Some(v);
    }
    // X >> X -> 0
    if op0 == op1 {
        let ty = q.ty(op0);
        return Some(q.null(ty));
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
) -> Option<Operand> {
    if let Some(v) = simplify_shift(q, BinaryOp::Shl, op0, op1, is_nsw) {
        return Some(v);
    }
    let ty = q.ty(op0);
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
) -> Option<Operand> {
    if let Some(v) = simplify_right_shift(q, BinaryOp::LShr, op0, op1, is_exact) {
        return Some(v);
    }
    // (X <<nuw Y) >> Y -> X
    if let Some((BinaryOp::Shl, x, amount, flags)) = q.binary(op0)
        && flags.contains(Flags::NUW)
        && amount == op1
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
) -> Option<Operand> {
    if let Some(v) = simplify_right_shift(q, BinaryOp::AShr, op0, op1, is_exact) {
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
    if q.width(op0).is_some_and(|width| crate::valuetracking::sign_bits(q.context, q.function, op0) == width) {
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
    None
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
    if let Some(v) = simplify_and_commutative(q, op0, op1) {
        return Some(v);
    }
    if let Some(v) = simplify_and_commutative(q, op1, op0) {
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
    // X | -1 = -1
    if q.is_all_ones(op1) {
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
    // Try some generic simplifications for associative operations.
    if let Some(v) = simplify_associative(q, BinaryOp::Or, op0, op1, max_recurse) {
        return Some(v);
    }
    // Or distributes over And.
    if let Some(v) = expand_commutative_bin_op(q, BinaryOp::Or, op0, op1, BinaryOp::And, max_recurse) {
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
        BinaryOp::Shl => simplify_shl(q, lhs, rhs, nsw, nuw),
        BinaryOp::LShr => simplify_lshr(q, lhs, rhs, exact),
        BinaryOp::AShr => simplify_ashr(q, lhs, rhs, exact),
        BinaryOp::And => simplify_and(q, lhs, rhs, max_recurse),
        BinaryOp::Or => simplify_or(q, lhs, rhs, max_recurse),
        BinaryOp::Xor => simplify_xor(q, lhs, rhs, max_recurse),
        _ => None,
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
    for incoming in q.function.instruction(phi).operands.iter().step_by(2).copied().collect::<Vec<_>>() {
        if Some(incoming) == result {
            continue;
        }
        if q.is_poison(incoming) {
            has_poison = true;
            continue;
        }
        if common.is_some_and(|one| one != incoming) {
            return None;
        }
        common = Some(incoming);
    }
    let Some(common) = common else { return Some(q.poison(ty)) };
    // A value defined after the phi in the same loop does not dominate it:
    // valueDominatesPHI (DEVIATE: dominators are an analysis). A constant or an
    // argument always does; an instruction only where the phi is not its own
    // user through a cycle, which `Some(incoming) == result` skipped above.
    if has_poison && q.definition(common).is_some() {
        return None;
    }
    Some(common)
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
        return constfold::cast(q.context, cast_opc, c, ty).map(Operand::Constant);
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
    // select ?, poison, X -> X; select ?, X, poison -> X
    if q.is_poison(true_val) {
        return Some(false_val);
    }
    if q.is_poison(false_val) {
        return Some(true_val);
    }
    None
}

/// `simplifyICmpInst`
/// ([:3883](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L3883)):
/// the constant fold, poison, the compare of a value with itself, the bool and
/// zero forms, and the compares of an extension against an extension or a
/// constant. DEVIATE: `simplifyICmpWithConstant`, `ConstantRange` (`getRange`),
/// `simplifyICmpWithBinOp`, min/max, assume, monotonic-value and the `trunc`
/// forms are not translated (they need `ConstantRange` and `isKnownNonZero`).
fn simplify_icmp(
    q: &mut Query,
    mut pred: IntPredicate,
    mut lhs: Operand,
    mut rhs: Operand,
    max_recurse: u32,
) -> Option<Operand> {
    if let Some(left) = q.constant(lhs) {
        if let Some(right) = q.constant(rhs) {
            return constfold::compare(q.context, pred, left, right).map(Operand::Constant);
        }
        std::mem::swap(&mut lhs, &mut rhs);
        pred = swapped(pred);
    }
    let i1 = q.context.types.int(1);
    // icmp X, poison -> poison
    if q.is_poison(rhs) {
        return Some(q.poison(i1));
    }
    // icmp X, X -> isTrueWhenEqual(Pred)
    if lhs == rhs {
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
    if let Some(v) = simplify_icmp_using_monotonic_values(q, pred, lhs, rhs) {
        return Some(v);
    }
    simplify_icmp_using_monotonic_values(q, swapped(pred), rhs, lhs)
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
/// DEVIATE: `uadd.sat` / `usub.sat` are intrinsics.
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
    ptr: Operand,
    indices: &[Operand],
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
    None
}

/// `simplifyInstruction`
/// ([:7741](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L7741),
/// `simplifyInstructionWithOperands`): the value `inst` equals, or None.
pub fn simplify_instruction(
    context: &mut Context,
    layout: &DataLayout,
    function: &Function,
    inst: InstId,
) -> Option<Operand> {
    let instruction = function.instruction(inst);
    let operands = instruction.operands.to_vec();
    let flags = instruction.flags;
    let mut q = Query { context, function, layout };
    match &instruction.opcode {
        Opcode::Binary(op) => {
            let [lhs, rhs] = operands[..] else { return None };
            simplify_binop_flags(&mut q, *op, lhs, rhs, flags, RECURSION_LIMIT)
        }
        Opcode::ICmp(pred) => {
            let [lhs, rhs] = operands[..] else { return None };
            simplify_icmp(&mut q, *pred, lhs, rhs, RECURSION_LIMIT)
        }
        Opcode::Select => {
            let [cond, a, b] = operands[..] else { return None };
            simplify_select(&mut q, cond, a, b)
        }
        Opcode::Cast(op) => simplify_cast(&mut q, *op, operands[0], instruction.ty, RECURSION_LIMIT),
        Opcode::Phi => simplify_phi(&mut q, inst),
        Opcode::GetElementPtr { .. } => simplify_gep(&mut q, operands[0], &operands[1..]),
        _ => None,
    }
}

/// `simplifyBinOp` for a caller that has operands and no instruction (a
/// builder).
pub fn simplify_binop_operands(
    context: &mut Context,
    layout: &DataLayout,
    function: &Function,
    opcode: BinaryOp,
    flags: Flags,
    lhs: Operand,
    rhs: Operand,
) -> Option<Operand> {
    let mut q = Query { context, function, layout };
    simplify_binop_flags(&mut q, opcode, lhs, rhs, flags, RECURSION_LIMIT)
}

/// `simplifyICmpInst` for operands.
pub fn simplify_icmp_operands(
    context: &mut Context,
    layout: &DataLayout,
    function: &Function,
    predicate: IntPredicate,
    lhs: Operand,
    rhs: Operand,
) -> Option<Operand> {
    let mut q = Query { context, function, layout };
    simplify_icmp(&mut q, predicate, lhs, rhs, RECURSION_LIMIT)
}

/// `simplifyCastInst` for operands.
pub fn simplify_cast_operand(
    context: &mut Context,
    layout: &DataLayout,
    function: &Function,
    op: CastOp,
    value: Operand,
    to: TypeId,
) -> Option<Operand> {
    let mut q = Query { context, function, layout };
    simplify_cast(&mut q, op, value, to, RECURSION_LIMIT)
}

/// `simplifySelectInst` for operands.
pub fn simplify_select_operands(
    context: &mut Context,
    layout: &DataLayout,
    function: &Function,
    condition: Operand,
    if_true: Operand,
    if_false: Operand,
) -> Option<Operand> {
    let mut q = Query { context, function, layout };
    simplify_select(&mut q, condition, if_true, if_false)
}

#[cfg(test)]
#[path = "simplify_tests.rs"]
mod tests;
