//! `SimplifyDemandedUseBits`: an integer instruction is rewritten, or its
//! operands are, when the users read only some of its bits.
//!
//! Translation of
//! [`InstCombinerImpl::SimplifyDemandedUseBits`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSimplifyDemanded.cpp#L235),
//! `SimplifyDemandedBits` (:166), `SimplifyDemandedInstructionBits` (:132),
//! `SimplifyMultipleUseDemandedBits` (:1228), `simplifyShrShlDemandedBits`
//! (:1404), `simplifyShiftSelectingPackedElement` (:70) and
//! `ShrinkDemandedConstant` (:43) at d1106deb71cc.
//!
//! MIRROR: the order of the cases and of the checks in each, the demanded mask
//! each operand is asked with, the flags dropped after an operand changed, and
//! the known bits each case leaves for its caller.
//!
//! DEVIATE 1: `ctpop` and `ptrmask` are not rewritten. `(ctpop X) ^ (ctpop Y)`
//! and `ctpop (not X)` build an intrinsic call (`combine_build` DEVIATE 2), and
//! `ptrmask` reads a pointer's known bits; their calls get the known bits of
//! `computeKnownBits`, as LLVM's fall-through does.
//!
//! DEVIATE 2: `convertOrOfShiftsToFunnelShift` is asked only as "is this `or` a
//! rotate": the shift pair, not the `zext` concatenation (:3114). A
//! `shl`/`lshr` that feeds only a concatenation is simplified here where LLVM
//! leaves it.
//!
//! DEVIATE 3: `haveNoCommonBitsSet` of the cached known bits asks the
//! operands' known bits again as well; the answer is only ever "yes" more
//! often when the fresh bits know more than the cached.
//!
//! DEVIATE 4: `m_ImmConstant` is an integer constant; `undef` and `poison` do
//! not match. No vectors (MIR has none), no target-specific hook.
//!
//! # Used by
//! The `visit*` of the integer families (`visitAnd`, `visitAdd`, `visitTrunc`,
//! `visitSelect`...) through [`Combiner::simplify_demanded_instruction_bits`].
//!
//! # Example
//! ```text
//! %a = or i32 %x, 255
//! %b = and i32 %a, 65280      ; only bits 8..15 are read, so %a is %x
//! ```

use llrm_mir::apint::mask;
use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::knownbits::KnownBits;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::valuetracking::MAX_ANALYSIS_RECURSION_DEPTH;
use llrm_mir::valuetracking::context::compute_known_bits_from_context;
use llrm_mir::valuetracking::known::{
    self, adjust_known_bits_for_select_arm, analyze_known_bits_from_and_xor_or, compute_known_bits,
    compute_known_bits_into,
};
use llrm_mir::valuetracking::pattern::{self, IntrinsicId};
use llrm_mir::valuetracking::signbits::compute_num_sign_bits;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{query, replace_operand};

/// What `SimplifyDemandedUseBits` returns: null, the instruction itself (an
/// operand or a flag changed), or the value it equals where those bits are
/// read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Simplified {
    Nothing,
    Edited,
    Other(Operand),
}

/// The count of leading zeros of the `w`-bit `v`.
fn clz(
    v: u128,
    w: u32,
) -> u32 {
    (v & mask::<u128>(w)).leading_zeros() - (128 - w)
}

/// The count of trailing zeros of the `w`-bit `v`.
fn ctz(
    v: u128,
    w: u32,
) -> u32 {
    if v & mask::<u128>(w) == 0 { w } else { v.trailing_zeros() }
}

/// `APInt::getActiveBits`.
fn active_bits(
    v: u128,
    w: u32,
) -> u32 {
    w - clz(v, w)
}

/// `APInt::getHighBitsSet(w, n)`.
fn high_bits(
    w: u32,
    n: u32,
) -> u128 {
    mask::<u128>(w) & !mask::<u128>(w - n)
}

/// `a.isSubsetOf(b)`.
fn subset(
    a: u128,
    b: u128,
) -> bool {
    a & !b == 0
}

/// `v << n` in `w` bits, zero when `n` shifts everything out.
fn shl_w(
    v: u128,
    n: u32,
    w: u32,
) -> u128 {
    if n >= w { 0 } else { (v << n) & mask::<u128>(w) }
}

/// `v >> n`, zero when `n` shifts everything out.
fn lshr_w(
    v: u128,
    n: u32,
) -> u128 {
    if n >= 128 { 0 } else { v >> n }
}

/// `APInt::getLimitedValue(limit)`.
fn limited(
    v: u128,
    limit: u32,
) -> u32 {
    v.min(u128::from(limit)) as u32
}

/// What `SimplifyDemandedUseBits` knows of the instruction it is asked about,
/// handed to the case of its opcode.
#[derive(Clone, Copy)]
struct Demand {
    cxt: InstId,
    inst: InstId,
    demanded: u128,
    depth: u32,
    w: u32,
    ty: llrm_mir::types::TypeId,
    flags: Flags,
    self_value: Operand,
}

impl Combiner<'_> {
    fn operand_of(
        &self,
        inst: InstId,
        index: usize,
    ) -> Operand {
        self.function.instruction(inst).operands[index]
    }

    fn result_of(
        &self,
        inst: InstId,
    ) -> Operand {
        Operand::Value(self.function.instruction(inst).result.expect("a value"))
    }

    /// The instruction that defines `v`.
    fn def_of(
        &self,
        v: Operand,
    ) -> Option<InstId> {
        query(self, None).instruction_of(v).map(|(inst, _)| inst)
    }

    /// `m_APInt` of a scalar: the integer constant `v` is.
    fn apint_of(
        &self,
        v: Operand,
    ) -> Option<u128> {
        query(self, None).int_constant(v).map(|(bits, _)| bits)
    }

    /// `v` is `op a, b` with the flags it carries.
    fn binary_of(
        &self,
        v: Operand,
        op: BinaryOp,
    ) -> Option<(InstId, Flags, Operand, Operand)> {
        let q = query(self, None);
        pattern::binary(&q, v, op).map(|(inst, instruction, a, b)| (inst, instruction.flags, a, b))
    }

    /// `I->dropPoisonGeneratingFlags()`.
    fn drop_poison_generating_flags(
        &mut self,
        inst: InstId,
    ) {
        let flags = self.function.instruction(inst).flags;
        self.function.set_flags(inst, flags.without(Flags::POISON_GENERATING));
    }

    /// `I->dropPoisonGeneratingAnnotations()`.
    fn drop_poison_generating_annotations(
        &mut self,
        inst: InstId,
    ) {
        self.drop_poison_generating_flags(inst);
        self.function.unannotate(inst, "range");
    }

    /// `new BinaryOperator` (or a cast) inserted before `at` by
    /// `InsertNewInstWith`: its value.
    fn insert_value(
        &mut self,
        at: InstId,
        opcode: Opcode,
        ty: llrm_mir::types::TypeId,
        operands: Vec<Operand>,
        flags: Flags,
    ) -> Operand {
        let new = self.insert_before(at, opcode, ty, operands, flags);
        self.result_of(new)
    }

    /// `computeKnownBits(I, Known, Q, Depth)` with the context instruction
    /// `cxt`.
    fn known_of(
        &self,
        cxt: InstId,
        v: Operand,
        known: &mut KnownBits,
        depth: u32,
    ) {
        compute_known_bits_into(&query(self, Some(cxt)), v, known, depth);
    }

    /// `ShrinkDemandedConstant(I, OpNo, Demanded)`.
    fn shrink_demanded_constant(
        &mut self,
        inst: InstId,
        index: usize,
        demanded: u128,
    ) -> bool {
        let op = self.operand_of(inst, index);
        let Some(c) = self.apint_of(op) else { return false };
        if subset(c, demanded) {
            return false;
        }
        let ty = self.type_of(op);
        let shrunk = self.int(ty, c & demanded);
        self.function.set_operand(inst, index, shrunk);
        true
    }

    /// `SimplifyDemandedInstructionBits(Inst)`: all bits of `inst` are asked.
    pub fn simplify_demanded_instruction_bits(
        &mut self,
        inst: InstId,
    ) -> Visit {
        let ty = self.function.instruction(inst).ty;
        let Some(bits) = self.context.types.int_bits(ty) else { return Visit::None };
        let mut known = KnownBits::unknown(bits);
        match self.simplify_demanded_use_bits(inst, inst, mask(bits), &mut known, 0) {
            Simplified::Nothing => Visit::None,
            Simplified::Edited => Visit::Changed,
            Simplified::Other(v) => Visit::Replace(v),
        }
    }

    /// `SimplifyDemandedBits(I, OpNo, DemandedMask, Known, Q, Depth)`: operand
    /// `index` of `inst` is simplified in place. Whether anything changed.
    pub fn simplify_demanded_bits(
        &mut self,
        cxt: InstId,
        inst: InstId,
        index: usize,
        demanded: u128,
        known: &mut KnownBits,
        depth: u32,
    ) -> bool {
        let v = self.operand_of(inst, index);
        if matches!(v, Operand::Constant(_)) {
            self.known_of(cxt, v, known, depth);
            return false;
        }

        known.reset_all();
        if demanded == 0 {
            // Not demanding any bits from V.
            let ty = self.type_of(v);
            let undef = Operand::Constant(self.context.constant(Constant { ty, kind: ConstantKind::Undef }));
            replace_operand(self, inst, index, undef);
            return true;
        }

        let Some(def) = self.def_of(v) else {
            self.known_of(cxt, v, known, depth);
            return false;
        };

        if depth == MAX_ANALYSIS_RECURSION_DEPTH {
            return false;
        }

        let new = if self.has_one_use(v) {
            // If the instruction has one use, we can directly simplify it.
            self.simplify_demanded_use_bits(cxt, def, demanded, known, depth)
        } else {
            // If there are multiple uses of this instruction, then we can
            // simplify VInst to some other value, but not modify the
            // instruction.
            match self.simplify_multiple_use_demanded_bits(cxt, def, demanded, known, depth) {
                Some(other) => Simplified::Other(other),
                None => Simplified::Nothing,
            }
        };
        let new = match new {
            Simplified::Nothing => return false,
            Simplified::Edited => v,
            Simplified::Other(other) => other,
        };
        replace_operand(self, inst, index, new);
        true
    }

    /// `SimplifyDemandedUseBits(I, DemandedMask, Known, Q, Depth)`: `inst` is
    /// read only at the bits of `demanded`; `known` has its width.
    #[allow(clippy::too_many_lines)]
    pub fn simplify_demanded_use_bits(
        &mut self,
        cxt: InstId,
        inst: InstId,
        demanded: u128,
        known: &mut KnownBits,
        depth: u32,
    ) -> Simplified {
        let (opcode, flags, ty) = {
            let instruction = self.function.instruction(inst);
            (instruction.opcode.clone(), instruction.flags, instruction.ty)
        };
        let d = Demand { cxt, inst, demanded, depth, w: known.width, ty, flags, self_value: self.result_of(inst) };
        let found = match opcode {
            Opcode::Binary(BinaryOp::And) => self.demanded_and(&d, known),
            Opcode::Binary(BinaryOp::Or) => self.demanded_or(&d, known),
            Opcode::Binary(BinaryOp::Xor) => self.demanded_xor(&d, known),
            Opcode::Select => self.demanded_select(&d, known),
            Opcode::Cast(op @ (CastOp::Trunc | CastOp::ZExt)) => self.demanded_trunc_zext(&d, op, known),
            Opcode::Cast(CastOp::SExt) => self.demanded_sext(&d, known),
            Opcode::Binary(BinaryOp::Add) => self.demanded_add(&d, known),
            Opcode::Binary(BinaryOp::Sub) => self.demanded_sub(&d, known),
            Opcode::Binary(BinaryOp::Mul) => self.demanded_mul(&d, known),
            Opcode::Binary(BinaryOp::Shl) => self.demanded_shl(&d, known),
            Opcode::Binary(BinaryOp::LShr) => self.demanded_lshr(&d, known),
            Opcode::Binary(BinaryOp::AShr) => self.demanded_ashr(&d, known),
            Opcode::Binary(BinaryOp::UDiv) => self.demanded_udiv(&d, known),
            Opcode::Binary(BinaryOp::SRem) => self.demanded_srem(&d, known),
            Opcode::Call(_) => self.demanded_call(&d, known),
            _ => {
                self.known_of(cxt, d.self_value, known, depth);
                Simplified::Nothing
            }
        };
        if found != Simplified::Nothing {
            return found;
        }

        // If the client is only demanding bits that we know, return the known
        // constant.
        if subset(demanded, known.zero | known.one) {
            return Simplified::Other(self.int(ty, known.one));
        }

        Simplified::Nothing
    }

    /// Update flags after simplifying an operand based on the fact that some
    /// high order bits are not demanded: with the top bit unread, `nsw` and
    /// `nuw` can no longer be promised.
    fn disable_wrap_flags(
        &mut self,
        inst: InstId,
        nlz: u32,
    ) -> Simplified {
        if nlz > 0 {
            let flags = self.function.instruction(inst).flags;
            self.function.set_flags(inst, flags.without(Flags::NSW).without(Flags::NUW));
        }
        Simplified::Edited
    }

    fn demanded_and(
        &mut self,
        d: &Demand,
        known: &mut KnownBits,
    ) -> Simplified {
        let Demand { cxt, inst, demanded, depth, w, ty, .. } = *d;
        let d1 = depth + 1;
        let mut lhs = KnownBits::unknown(w);
        let mut rhs = KnownBits::unknown(w);
        // If either the LHS or the RHS are Zero, the result is
        // zero.
        if self.simplify_demanded_bits(cxt, inst, 1, demanded, &mut rhs, d1)
            || self.simplify_demanded_bits(cxt, inst, 0, demanded & !rhs.zero, &mut lhs, d1)
        {
            return Simplified::Edited;
        }

        *known = analyze_known_bits_from_and_xor_or(&query(self, Some(cxt)), inst, &lhs, &rhs, depth);

        // If the client is only demanding bits that we know,
        // return the known constant.
        if subset(demanded, known.zero | known.one) {
            return Simplified::Other(self.int(ty, known.one));
        }

        // If all of the demanded bits are known 1 on one side,
        // return the other.
        if subset(demanded, lhs.zero | rhs.one) {
            return Simplified::Other(self.operand_of(inst, 0));
        }
        if subset(demanded, rhs.zero | lhs.one) {
            return Simplified::Other(self.operand_of(inst, 1));
        }

        // If the RHS is a constant, see if we can simplify it.
        if self.shrink_demanded_constant(inst, 1, demanded & !lhs.zero) {
            return Simplified::Edited;
        }
        Simplified::Nothing
    }

    fn demanded_or(
        &mut self,
        d: &Demand,
        known: &mut KnownBits,
    ) -> Simplified {
        let Demand { cxt, inst, demanded, depth, w, ty, flags, .. } = *d;
        let d1 = depth + 1;
        let mut lhs = KnownBits::unknown(w);
        let mut rhs = KnownBits::unknown(w);
        // If either the LHS or the RHS are One, the result is One.
        if self.simplify_demanded_bits(cxt, inst, 1, demanded, &mut rhs, d1)
            || self.simplify_demanded_bits(cxt, inst, 0, demanded & !rhs.one, &mut lhs, d1)
        {
            // Disjoint flag may not longer hold.
            self.drop_poison_generating_flags(inst);
            return Simplified::Edited;
        }

        *known = analyze_known_bits_from_and_xor_or(&query(self, Some(cxt)), inst, &lhs, &rhs, depth);

        if subset(demanded, known.zero | known.one) {
            return Simplified::Other(self.int(ty, known.one));
        }

        // If all of the demanded bits are known zero on one side,
        // return the other.
        if subset(demanded, lhs.one | rhs.zero) {
            return Simplified::Other(self.operand_of(inst, 0));
        }
        if subset(demanded, rhs.one | lhs.zero) {
            return Simplified::Other(self.operand_of(inst, 1));
        }

        if self.shrink_demanded_constant(inst, 1, demanded) {
            return Simplified::Edited;
        }

        // Infer disjoint flag if no common bits are set.
        if !flags.contains(Flags::DISJOINT) {
            let (a, b) = (self.operand_of(inst, 0), self.operand_of(inst, 1));
            let disjoint = KnownBits::have_no_common_bits_set(&lhs, &rhs)
                || known::have_no_common_bits_set(&query(self, Some(cxt)), a, b);
            if disjoint {
                let mut flags = flags;
                flags.insert(Flags::DISJOINT);
                self.function.set_flags(inst, flags);
                return Simplified::Edited;
            }
        }
        Simplified::Nothing
    }

    fn demanded_xor(
        &mut self,
        d: &Demand,
        known: &mut KnownBits,
    ) -> Simplified {
        let Demand { cxt, inst, demanded, depth, w, ty, .. } = *d;
        let d1 = depth + 1;
        let mut lhs = KnownBits::unknown(w);
        let mut rhs = KnownBits::unknown(w);
        if self.simplify_demanded_bits(cxt, inst, 1, demanded, &mut rhs, d1)
            || self.simplify_demanded_bits(cxt, inst, 0, demanded, &mut lhs, d1)
        {
            return Simplified::Edited;
        }

        *known = analyze_known_bits_from_and_xor_or(&query(self, Some(cxt)), inst, &lhs, &rhs, depth);

        if subset(demanded, known.zero | known.one) {
            return Simplified::Other(self.int(ty, known.one));
        }

        // If all of the demanded bits are known zero on one side,
        // return the other.
        if subset(demanded, rhs.zero) {
            return Simplified::Other(self.operand_of(inst, 0));
        }
        if subset(demanded, lhs.zero) {
            return Simplified::Other(self.operand_of(inst, 1));
        }

        let (op0, op1) = (self.operand_of(inst, 0), self.operand_of(inst, 1));

        // If all of the demanded bits are known to be zero on one
        // side or the other, turn this into an *inclusive* or.
        if subset(demanded, rhs.zero | lhs.zero) {
            let mut or_flags = Flags::default();
            if demanded == mask(w) {
                or_flags.insert(Flags::DISJOINT);
            }
            return Simplified::Other(self.insert_value(
                inst,
                Opcode::Binary(BinaryOp::Or),
                ty,
                vec![op0, op1],
                or_flags,
            ));
        }

        // If all of the demanded bits on one side are known, and
        // all of the set bits on that side are also known to be
        // set on the other side, turn this into an AND.
        if subset(demanded, rhs.zero | rhs.one) && subset(rhs.one, lhs.one) {
            let and_c = self.int(ty, !rhs.one & demanded);
            return Simplified::Other(self.insert_value(
                inst,
                Opcode::Binary(BinaryOp::And),
                ty,
                vec![op0, and_c],
                Flags::default(),
            ));
        }

        // If the RHS is a constant, see if we can change it. Don't
        // alter a -1 constant because that's a canonical 'not' op.
        if let Some(c) = self.apint_of(op1)
            && c != mask(w)
        {
            if (c | !demanded) & mask::<u128>(w) == mask(w) {
                // Force bits to 1 to create a 'not' op.
                let ones = self.all_ones(ty);
                self.function.set_operand(inst, 1, ones);
                return Simplified::Edited;
            }
            // If we can't turn this into a 'not', try to shrink
            // the constant.
            if self.shrink_demanded_constant(inst, 1, demanded) {
                return Simplified::Edited;
            }
        }

        // If our LHS is an 'and' and if it has one use, and if any
        // of the bits we are flipping are known to be set, then
        // the xor is just resetting those bits to zero.
        if let Some((_, _, _, and_c)) = self.binary_of(op0, BinaryOp::And)
            && self.has_one_use(op0)
            && let Some(xor_rhs) = self.apint_of(op1)
            && let Some(and_rhs) = self.apint_of(and_c)
            && lhs.one & rhs.one & demanded != 0
        {
            let new_mask = !(lhs.one & rhs.one & demanded) & mask::<u128>(w);
            let and_c = self.int(ty, new_mask & and_rhs);
            let new_and =
                self.insert_value(inst, Opcode::Binary(BinaryOp::And), ty, vec![op0, and_c], Flags::default());
            let xor_c = self.int(ty, new_mask & xor_rhs);
            return Simplified::Other(self.insert_value(
                inst,
                Opcode::Binary(BinaryOp::Xor),
                ty,
                vec![new_and, xor_c],
                Flags::default(),
            ));
        }
        Simplified::Nothing
    }

    fn demanded_select(
        &mut self,
        d: &Demand,
        known: &mut KnownBits,
    ) -> Simplified {
        let Demand { cxt, inst, demanded, depth, w, .. } = *d;
        let d1 = depth + 1;
        let mut lhs = KnownBits::unknown(w);
        let mut rhs = KnownBits::unknown(w);
        if self.simplify_demanded_bits(cxt, inst, 2, demanded, &mut rhs, d1)
            || self.simplify_demanded_bits(cxt, inst, 1, demanded, &mut lhs, d1)
        {
            return Simplified::Edited;
        }

        // If the operands are constants, see if we can simplify
        // them.
        if self.canonicalize_select_constant(inst, 1, demanded) || self.canonicalize_select_constant(inst, 2, demanded)
        {
            return Simplified::Edited;
        }

        // Only known if known in both the LHS and RHS.
        let (cond, on_true, on_false) = (self.operand_of(inst, 0), self.operand_of(inst, 1), self.operand_of(inst, 2));
        let q = query(self, Some(cxt));
        adjust_known_bits_for_select_arm(&q, &mut lhs, cond, on_true, false, depth);
        adjust_known_bits_for_select_arm(&q, &mut rhs, cond, on_false, true, depth);
        *known = lhs.intersect_with(&rhs);
        Simplified::Nothing
    }

    fn demanded_trunc_zext(
        &mut self,
        d: &Demand,
        op: CastOp,
        known: &mut KnownBits,
    ) -> Simplified {
        let Demand { cxt, inst, demanded, depth, w, ty, flags, .. } = *d;
        let d1 = depth + 1;
        if op == CastOp::Trunc {
            // If we do not demand the high bits of a right-shifted
            // and truncated value, then we may be able to truncate
            // it before the shift.
            let op0 = self.operand_of(inst, 0);
            if let Some((_, _, x, amount)) = self.binary_of(op0, BinaryOp::LShr)
                && self.has_one_use(op0)
                && let Some(c) = self.apint_of(amount)
            {
                // The shift amount must be valid (not poison) in
                // the narrow type, and it must not be greater than
                // the high bits demanded of the result.
                if c < u128::from(w) && c <= u128::from(clz(demanded, w)) {
                    // trunc (lshr X, C) --> lshr (trunc X), C
                    let trunc = self.create_cast(inst, CastOp::Trunc, x, ty, Flags::default());
                    let shift = self.int(ty, c);
                    return Simplified::Other(self.create_binop(inst, BinaryOp::LShr, trunc, shift, Flags::default()));
                }
            }
        }

        let src = self.operand_of(inst, 0);
        let src_w = self.context.types.int_bits(self.type_of(src)).expect("an integer cast");
        let input_demanded = if src_w >= w { demanded } else { demanded & mask::<u128>(src_w) };
        let mut input_known = KnownBits::unknown(src_w);
        if self.simplify_demanded_bits(cxt, inst, 0, input_demanded, &mut input_known, d1) {
            // For zext nneg, we may have dropped the instruction
            // which made the input non-negative.
            self.drop_poison_generating_flags(inst);
            return Simplified::Edited;
        }
        if op == CastOp::ZExt && flags.contains(Flags::NNEG) && !input_known.is_negative() {
            input_known.make_non_negative();
        }
        *known = input_known.zext_or_trunc(w);
        Simplified::Nothing
    }

    fn demanded_sext(
        &mut self,
        d: &Demand,
        known: &mut KnownBits,
    ) -> Simplified {
        let Demand { cxt, inst, demanded, depth, w, ty, .. } = *d;
        let d1 = depth + 1;
        // Compute the bits in the result that are not present in
        // the input.
        let src = self.operand_of(inst, 0);
        let src_w = self.context.types.int_bits(self.type_of(src)).expect("an integer cast");
        let mut input_demanded = demanded & mask::<u128>(src_w);

        // If any of the sign extended bits are demanded, we know
        // that the sign bit is demanded.
        if active_bits(demanded, w) > src_w {
            input_demanded |= 1 << (src_w - 1);
        }

        let mut input_known = KnownBits::unknown(src_w);
        if self.simplify_demanded_bits(cxt, inst, 0, input_demanded, &mut input_known, d1) {
            return Simplified::Edited;
        }

        // If the input sign bit is known zero, or if the NewBits
        // are not demanded convert this into a zero extension.
        if input_known.is_non_negative() || active_bits(demanded, w) <= src_w {
            return Simplified::Other(self.insert_value(
                inst,
                Opcode::Cast(CastOp::ZExt),
                ty,
                vec![src],
                Flags::default(),
            ));
        }

        // If the sign bit of the input is known set or clear, then
        // we know the top bits of the result.
        *known = input_known.sext(w);
        Simplified::Nothing
    }

    fn demanded_add(
        &mut self,
        d: &Demand,
        known: &mut KnownBits,
    ) -> Simplified {
        let Demand { cxt, inst, demanded, depth, w, ty, flags, .. } = *d;
        let d1 = depth + 1;
        let mut lhs = KnownBits::unknown(w);
        let mut rhs = KnownBits::unknown(w);
        if demanded & 1 == 0 {
            // If we do not need the low bit, try to convert bool
            // math to logic.
            if let Some(found) = self.demanded_bool_add(inst, ty) {
                return Simplified::Other(found);
            }
        }

        // Right fill the mask of bits for the operands to demand
        // the most significant bit and all those below it.
        let nlz = clz(demanded, w);
        let from_ops = mask::<u128>(w - nlz);
        if self.shrink_demanded_constant(inst, 1, from_ops)
            || self.simplify_demanded_bits(cxt, inst, 1, from_ops, &mut rhs, d1)
        {
            return self.disable_wrap_flags(inst, nlz);
        }

        // If low order bits are not demanded and known to be zero
        // in one operand, then we don't need to demand them from
        // the other operand, since they can't cause overflow into
        // any bits that are demanded in the result.
        let ntz = ctz(!(!demanded & rhs.zero), w);
        let from_lhs = from_ops & !mask::<u128>(ntz);
        if self.shrink_demanded_constant(inst, 0, from_lhs)
            || self.simplify_demanded_bits(cxt, inst, 0, from_lhs, &mut lhs, d1)
        {
            return self.disable_wrap_flags(inst, nlz);
        }

        let ntz_lhs = ctz(!(!demanded & lhs.zero), w);
        let from_rhs = from_ops & !mask::<u128>(ntz_lhs);
        if self.shrink_demanded_constant(inst, 1, from_rhs) {
            return self.disable_wrap_flags(inst, nlz);
        }

        // If we are known to be adding zeros to every bit below
        // the highest demanded bit, we just return the other side.
        if subset(from_ops, rhs.zero) {
            return Simplified::Other(self.operand_of(inst, 0));
        }
        if subset(from_ops, lhs.zero) {
            return Simplified::Other(self.operand_of(inst, 1));
        }

        // (add X, C) --> (xor X, C) IFF C is equal to the top bit
        // of the DemandMask
        if let Some(c) = self.apint_of(self.operand_of(inst, 1))
            && active_bits(demanded, w) > 0
            && c == 1 << (active_bits(demanded, w) - 1)
        {
            let x = self.operand_of(inst, 0);
            let c = self.int(ty, c);
            return Simplified::Other(self.create_binop(inst, BinaryOp::Xor, x, c, Flags::default()));
        }

        // Otherwise just compute the known bits of the result.
        *known = KnownBits::add(&lhs, &rhs, flags.contains(Flags::NSW), flags.contains(Flags::NUW), false);
        Simplified::Nothing
    }

    fn demanded_sub(
        &mut self,
        d: &Demand,
        known: &mut KnownBits,
    ) -> Simplified {
        let Demand { cxt, inst, demanded, depth, w, flags, .. } = *d;
        let d1 = depth + 1;
        let mut lhs = KnownBits::unknown(w);
        let mut rhs = KnownBits::unknown(w);
        let nlz = clz(demanded, w);
        let from_ops = mask::<u128>(w - nlz);
        if self.shrink_demanded_constant(inst, 1, from_ops)
            || self.simplify_demanded_bits(cxt, inst, 1, from_ops, &mut rhs, d1)
        {
            return self.disable_wrap_flags(inst, nlz);
        }

        // If low order bits are not demanded and are known to be
        // zero in RHS, then we don't need to demand them from LHS,
        // since they can't cause a borrow from any bits that are
        // demanded in the result.
        let ntz = ctz(!(!demanded & rhs.zero), w);
        let from_lhs = from_ops & !mask::<u128>(ntz);
        if self.shrink_demanded_constant(inst, 0, from_lhs)
            || self.simplify_demanded_bits(cxt, inst, 0, from_lhs, &mut lhs, d1)
        {
            return self.disable_wrap_flags(inst, nlz);
        }

        // If we are known to be subtracting zeros from every bit
        // below the highest demanded bit, we just return the other
        // side.
        if subset(from_ops, rhs.zero) {
            return Simplified::Other(self.operand_of(inst, 0));
        }
        // We can't do this with the LHS for subtraction, unless we
        // are only demanding the LSB.
        if from_ops == 1 && subset(from_ops, lhs.zero) {
            return Simplified::Other(self.operand_of(inst, 1));
        }

        // Canonicalize sub mask, X -> ~X
        let (op0, op1) = (self.operand_of(inst, 0), self.operand_of(inst, 1));
        if let Some((mask_c, _)) = pattern::low_bit_mask(&query(self, None), op0)
            && subset(from_ops, mask_c)
        {
            return Simplified::Other(self.create_not(inst, op1));
        }

        *known = KnownBits::sub(&lhs, &rhs, flags.contains(Flags::NSW), flags.contains(Flags::NUW));
        Simplified::Nothing
    }

    fn demanded_mul(
        &mut self,
        d: &Demand,
        known: &mut KnownBits,
    ) -> Simplified {
        let Demand { cxt, inst, demanded, depth, w, ty, self_value, .. } = *d;
        let d1 = depth + 1;
        let mut lhs = KnownBits::unknown(w);
        let mut rhs = KnownBits::unknown(w);
        // If the high-bits of a MUL are not demanded, then we do
        // not care about the high bits of the operands.
        let nlz = clz(demanded, w);
        let from_ops = mask::<u128>(w - nlz);
        if self.shrink_demanded_constant(inst, 0, from_ops)
            || self.simplify_demanded_bits(cxt, inst, 0, from_ops, &mut lhs, d1)
            || self.shrink_demanded_constant(inst, 1, from_ops)
            || self.simplify_demanded_bits(cxt, inst, 1, from_ops, &mut rhs, d1)
        {
            self.disable_wrap_flags(inst, nlz);
            return Simplified::Edited;
        }

        let (op0, op1) = (self.operand_of(inst, 0), self.operand_of(inst, 1));
        if demanded.is_power_of_two() {
            // The LSB of X*Y is set only if (X & 1) == 1 and
            // (Y & 1) == 1. If we demand exactly one bit N and we
            // have "X * (C' << N)" where C' is odd, then the
            // left-shifted low bit of X is the answer.
            let n = demanded.trailing_zeros();
            if let Some(c) = self.apint_of(op1)
                && ctz(c, w) == n
            {
                let amount = self.int(ty, u128::from(n));
                return Simplified::Other(self.insert_value(
                    inst,
                    Opcode::Binary(BinaryOp::Shl),
                    ty,
                    vec![op0, amount],
                    Flags::default(),
                ));
            }
        }
        // For a squared value "X * X", the bottom 2 bits are 0 and
        // X[0] because X * X is odd iff X is odd.
        if op0 == op1 && demanded < 4 {
            let one = self.int(ty, 1);
            return Simplified::Other(self.insert_value(
                inst,
                Opcode::Binary(BinaryOp::And),
                ty,
                vec![op0, one],
                Flags::default(),
            ));
        }

        self.known_of(cxt, self_value, known, depth);
        Simplified::Nothing
    }

    fn demanded_shl(
        &mut self,
        d: &Demand,
        known: &mut KnownBits,
    ) -> Simplified {
        let Demand { cxt, inst, demanded, depth, w, ty, flags, self_value, .. } = *d;
        let d1 = depth + 1;
        let (op0, op1) = (self.operand_of(inst, 0), self.operand_of(inst, 1));
        if let Some(sa) = self.apint_of(op1) {
            for shr_op in [BinaryOp::LShr, BinaryOp::AShr] {
                if let Some((shr, _, _, amount)) = self.binary_of(op0, shr_op)
                    && let Some(shr_amt) = self.apint_of(amount)
                    && let Some(found) = self.simplify_shr_shl_demanded_bits(shr, shr_amt, inst, sa, demanded, known)
                {
                    return Simplified::Other(found);
                }
            }

            // Do not simplify if shl is part of funnel-shift
            // pattern
            if self.feeds_a_rotate(inst) {
                self.known_of(cxt, self_value, known, depth);
                return Simplified::Nothing;
            }

            // We only want bits that already match the signbit
            // then we don't need to shift.
            let shift_amt = limited(sa, w - 1);
            if ctz(demanded, w) >= shift_amt {
                if flags.contains(Flags::NSW) {
                    let hi_demanded = w - ctz(demanded, w);
                    let sign_bits = compute_num_sign_bits(&query(self, Some(cxt)), op0, d1);
                    if sign_bits > shift_amt && sign_bits - shift_amt >= hi_demanded {
                        return Simplified::Other(op0);
                    }
                }

                // If we can pre-shift a right-shifted constant to
                // the left without losing any high bits and we
                // don't demand the low bits, then eliminate the
                // left-shift:
                // (C >> X) << LeftShiftAmtC --> (C <<
                // LeftShiftAmtC) >> X
                if let Some((_, _, c, x)) = self.binary_of(op0, BinaryOp::LShr)
                    && let Some(c) = self.apint_of(c)
                {
                    let new_c = shl_w(c, shift_amt, w);
                    if lshr_w(new_c, shift_amt) == c {
                        let new_c = self.int(ty, new_c);
                        return Simplified::Other(self.insert_value(
                            inst,
                            Opcode::Binary(BinaryOp::LShr),
                            ty,
                            vec![new_c, x],
                            Flags::default(),
                        ));
                    }
                }
            }

            let mut demanded_in = lshr_w(demanded, shift_amt);

            // If the shift is NUW/NSW, then it does demand the high
            // bits.
            if flags.contains(Flags::NSW) {
                demanded_in |= high_bits(w, shift_amt + 1);
            } else if flags.contains(Flags::NUW) {
                demanded_in |= high_bits(w, shift_amt);
            }

            if self.simplify_demanded_bits(cxt, inst, 0, demanded_in, known, d1) {
                return Simplified::Edited;
            }

            *known = KnownBits::shl(
                known,
                &KnownBits::constant(w, u128::from(shift_amt)),
                flags.contains(Flags::NUW),
                flags.contains(Flags::NSW),
                false,
            );
        } else {
            // This is a variable shift, so we can't shift the
            // demand mask by a known amount. But if we are not
            // demanding high bits, then we are not demanding those
            // bits from the pre-shifted operand either.
            let leading = clz(demanded, w);
            if leading != 0 {
                let from_op = mask::<u128>(w - leading);
                if self.simplify_demanded_bits(cxt, inst, 0, from_op, known, d1) {
                    // We can't guarantee that nsw/nuw hold after
                    // simplifying the operand.
                    self.drop_poison_generating_flags(inst);
                    return Simplified::Edited;
                }
            }
            self.known_of(cxt, self_value, known, depth);
        }
        Simplified::Nothing
    }

    fn demanded_lshr(
        &mut self,
        d: &Demand,
        known: &mut KnownBits,
    ) -> Simplified {
        let Demand { cxt, inst, demanded, depth, w, ty, self_value, .. } = *d;
        let d1 = depth + 1;
        let (op0, op1) = (self.operand_of(inst, 0), self.operand_of(inst, 1));
        if let Some(sa) = self.apint_of(op1) {
            let shift_amt = limited(sa, w - 1);

            // Do not simplify if lshr is part of funnel-shift
            // pattern
            if self.feeds_a_rotate(inst) {
                self.known_of(cxt, self_value, known, depth);
                return Simplified::Nothing;
            }

            // If we are just demanding the shifted sign bit and
            // below, then this can be treated as an ASHR in
            // disguise.
            if clz(demanded, w) >= shift_amt {
                // If we only want bits that already match the
                // signbit then we don't need to shift.
                let hi_demanded = w - ctz(demanded, w);
                let sign_bits = compute_num_sign_bits(&query(self, Some(cxt)), op0, d1);
                if sign_bits >= hi_demanded {
                    return Simplified::Other(op0);
                }

                // If we can pre-shift a left-shifted constant to
                // the right without losing any low bits (we
                // already know we don't demand the high bits),
                // then eliminate the right-shift:
                // (C << X) >> RightShiftAmtC --> (C >>
                // RightShiftAmtC) << X
                if let Some((_, _, c, x)) = self.binary_of(op0, BinaryOp::Shl)
                    && let Some(c) = self.apint_of(c)
                {
                    let new_c = lshr_w(c, shift_amt);
                    if shl_w(new_c, shift_amt, w) == c {
                        let new_c = self.int(ty, new_c);
                        return Simplified::Other(self.insert_value(
                            inst,
                            Opcode::Binary(BinaryOp::Shl),
                            ty,
                            vec![new_c, x],
                            Flags::default(),
                        ));
                    }
                }

                if let Some((_, _, x, factor)) = self.binary_of(op0, BinaryOp::Mul)
                    && self.has_one_use(op0)
                    && let Some(factor) = self.apint_of(factor)
                    && ctz(factor, w) >= shift_amt
                {
                    let new_factor = self.int(ty, lshr_w(factor, shift_amt));
                    return Simplified::Other(self.insert_value(
                        inst,
                        Opcode::Binary(BinaryOp::Mul),
                        ty,
                        vec![x, new_factor],
                        Flags::default(),
                    ));
                }
            }

            // Unsigned shift right.
            let demanded_in = shl_w(demanded, shift_amt, w);
            if self.simplify_demanded_bits(cxt, inst, 0, demanded_in, known, d1) {
                // exact flag may not longer hold.
                self.drop_poison_generating_flags(inst);
                return Simplified::Edited;
            }
            *known = known.lshr_by(shift_amt);
            if shift_amt != 0 {
                // high bits known zero.
                known.zero |= high_bits(w, shift_amt);
            }
            return Simplified::Nothing;
        }
        if let Some(found) = self.shift_selecting_packed_element(inst, demanded, depth) {
            return Simplified::Other(found);
        }

        self.known_of(cxt, self_value, known, depth);
        Simplified::Nothing
    }

    fn demanded_ashr(
        &mut self,
        d: &Demand,
        known: &mut KnownBits,
    ) -> Simplified {
        let Demand { cxt, inst, demanded, depth, w, ty, flags, self_value, .. } = *d;
        let d1 = depth + 1;
        let (op0, op1) = (self.operand_of(inst, 0), self.operand_of(inst, 1));
        let sign_bits = compute_num_sign_bits(&query(self, Some(cxt)), op0, d1);

        // If we only want bits that already match the signbit then
        // we don't need to shift.
        let hi_demanded = w - ctz(demanded, w);
        if sign_bits >= hi_demanded {
            return Simplified::Other(op0);
        }

        // If this is an arithmetic shift right and only the
        // low-bit is set, we can always convert this into a
        // logical shr, even if the shift amount is variable.
        if demanded == 1 {
            return Simplified::Other(self.insert_value(
                inst,
                Opcode::Binary(BinaryOp::LShr),
                ty,
                vec![op0, op1],
                Flags::default(),
            ));
        }

        if let Some(sa) = self.apint_of(op1) {
            let shift_amt = limited(sa, w - 1);

            // Signed shift right.
            let mut demanded_in = shl_w(demanded, shift_amt, w);
            // If any of the bits being shifted in are demanded,
            // then we should set the sign bit as demanded.
            let shifted_in_demanded = clz(demanded, w) < shift_amt;
            if shifted_in_demanded {
                demanded_in |= 1 << (w - 1);
            }
            if self.simplify_demanded_bits(cxt, inst, 0, demanded_in, known, d1) {
                // exact flag may not longer hold.
                self.drop_poison_generating_flags(inst);
                return Simplified::Edited;
            }

            // If the input sign bit is known to be zero, or if none
            // of the shifted in bits are demanded, turn this into
            // an unsigned shift right.
            if known.zero >> (w - 1) & 1 != 0 || !shifted_in_demanded {
                let exact = flags.intersect(Flags::EXACT);
                return Simplified::Other(self.insert_value(
                    inst,
                    Opcode::Binary(BinaryOp::LShr),
                    ty,
                    vec![op0, op1],
                    exact,
                ));
            }

            *known = KnownBits::ashr(
                known,
                &KnownBits::constant(w, u128::from(shift_amt)),
                shift_amt != 0,
                flags.contains(Flags::EXACT),
            );
        } else {
            self.known_of(cxt, self_value, known, depth);
        }
        Simplified::Nothing
    }

    fn demanded_udiv(
        &mut self,
        d: &Demand,
        known: &mut KnownBits,
    ) -> Simplified {
        let Demand { cxt, inst, depth, w, flags, self_value, .. } = *d;
        let d1 = depth + 1;
        let mut lhs = KnownBits::unknown(w);
        // UDiv doesn't demand low bits that are zero in the
        // divisor.
        if let Some(sa) = self.apint_of(self.operand_of(inst, 1)) {
            let trailing = ctz(sa, w);
            let demanded_in = high_bits(w, w - trailing);
            if self.simplify_demanded_bits(cxt, inst, 0, demanded_in, &mut lhs, d1) {
                // We can't guarantee that "exact" is still true
                // after changing the dividend.
                self.drop_poison_generating_flags(inst);
                return Simplified::Edited;
            }

            *known = KnownBits::udiv(&lhs, &KnownBits::constant(w, sa), flags.contains(Flags::EXACT));
        } else {
            self.known_of(cxt, self_value, known, depth);
        }
        Simplified::Nothing
    }

    fn demanded_srem(
        &mut self,
        d: &Demand,
        known: &mut KnownBits,
    ) -> Simplified {
        let Demand { cxt, inst, demanded, depth, w, self_value, .. } = *d;
        let d1 = depth + 1;
        let mut lhs = KnownBits::unknown(w);
        if let Some(rem) = self.apint_of(self.operand_of(inst, 1))
            && rem.is_power_of_two()
        {
            if demanded < rem {
                // srem won't affect demanded bits
                return Simplified::Other(self.operand_of(inst, 0));
            }

            let mask2 = (rem - 1) | 1 << (w - 1);
            if self.simplify_demanded_bits(cxt, inst, 0, mask2, &mut lhs, d1) {
                return Simplified::Edited;
            }
            *known = KnownBits::srem(&lhs, &KnownBits::constant(w, rem));
            return Simplified::Nothing;
        }

        self.known_of(cxt, self_value, known, depth);
        Simplified::Nothing
    }

    fn demanded_call(
        &mut self,
        d: &Demand,
        known: &mut KnownBits,
    ) -> Simplified {
        let Demand { cxt, inst, demanded, depth, w, ty, self_value, .. } = *d;
        let d1 = depth + 1;
        let mut lhs = KnownBits::unknown(w);
        let mut rhs = KnownBits::unknown(w);
        let mut known_computed = false;
        let id = pattern::call_intrinsic(&query(self, None), inst);
        let arg = |c: &Self, index: usize| c.operand_of(inst, index);
        match id {
            Some(IntrinsicId::Abs) => {
                if demanded == 1 {
                    return Simplified::Other(arg(self, 0));
                }
            }
            Some(IntrinsicId::Bswap) => {
                // If the only bits demanded come from one byte of
                // the bswap result, just shift the input byte into
                // position to eliminate the bswap.
                let nlz = clz(demanded, w) & !7;
                let ntz = ctz(demanded, w) & !7;

                // If we need exactly one byte, we can do this
                // transformation.
                if w - nlz - ntz == 8 {
                    // Replace this with either a left or right
                    // shift to get the byte into the right place.
                    let (op, amount) = if nlz > ntz { (BinaryOp::LShr, nlz - ntz) } else { (BinaryOp::Shl, ntz - nlz) };
                    let amount = self.int(ty, u128::from(amount));
                    let x = arg(self, 0);
                    return Simplified::Other(self.insert_value(
                        inst,
                        Opcode::Binary(op),
                        ty,
                        vec![x, amount],
                        Flags::default(),
                    ));
                }
            }
            Some(id @ (IntrinsicId::Fshl | IntrinsicId::Fshr)) => {
                if let Some(sa) = self.apint_of(arg(self, 2)) {
                    // Normalize to funnel shift left.
                    let mut shift_amt = (sa % u128::from(w)) as u32;
                    if id == IntrinsicId::Fshr {
                        shift_amt = w - shift_amt;
                    }

                    let demanded_lhs = lshr_w(demanded, shift_amt);
                    let demanded_rhs = shl_w(demanded, w - shift_amt, w);
                    let (op0, op1) = (arg(self, 0), arg(self, 1));
                    if op0 != op1 {
                        if self.simplify_demanded_bits(cxt, inst, 0, demanded_lhs, &mut lhs, d1)
                            || self.simplify_demanded_bits(cxt, inst, 1, demanded_rhs, &mut rhs, d1)
                        {
                            // Range attribute or metadata may no
                            // longer hold.
                            self.drop_poison_generating_annotations(inst);
                            return Simplified::Edited;
                        }
                    } else {
                        // fshl is a rotate. Avoid converting
                        // rotate into funnel shift. Only simplify
                        // if one operand is constant.
                        lhs = compute_known_bits(&query(self, Some(inst)), op0, d1);
                        if subset(demanded_lhs, lhs.zero | lhs.one) && self.apint_of(op0) != Some(lhs.one) {
                            let constant = self.int(ty, lhs.one);
                            replace_operand(self, inst, 0, constant);
                            self.drop_poison_generating_annotations(inst);
                            return Simplified::Edited;
                        }

                        rhs = compute_known_bits(&query(self, Some(inst)), op1, d1);
                        if subset(demanded_rhs, rhs.zero | rhs.one) && self.apint_of(op1) != Some(rhs.one) {
                            let constant = self.int(ty, rhs.one);
                            replace_operand(self, inst, 1, constant);
                            self.drop_poison_generating_annotations(inst);
                            return Simplified::Edited;
                        }
                    }

                    lhs = lhs.shl_by(shift_amt);
                    rhs = rhs.lshr_by(w - shift_amt);
                    *known = lhs.union_with(&rhs);
                    known_computed = true;
                }
            }
            Some(IntrinsicId::Umax) => {
                // UMax(A, C) == A if the lowest non-zero bit of
                // DemandMask is higher than the highest non-zero
                // bit of C.
                if let Some(c) = self.apint_of(arg(self, 1))
                    && ctz(demanded, w) >= active_bits(c, w)
                {
                    return Simplified::Other(arg(self, 0));
                }
            }
            Some(IntrinsicId::Umin) => {
                // This comes from using DeMorgans on the above umax
                // example.
                if let Some(c) = self.apint_of(arg(self, 1))
                    && ctz(demanded, w) >= active_bits(!c, w)
                {
                    return Simplified::Other(arg(self, 0));
                }
            }
            _ => {}
        }

        if !known_computed {
            self.known_of(cxt, self_value, known, depth);
        }
        Simplified::Nothing
    }

    /// `CanonicalizeSelectConstant(I, OpNo, DemandedMask)` of the `select`
    /// case: keep the selected constants the same as the `icmp`'s constant,
    /// where the demanded bits allow.
    fn canonicalize_select_constant(
        &mut self,
        inst: InstId,
        index: usize,
        demanded: u128,
    ) -> bool {
        let Some((sel_c, sel_bits)) = query(self, None).int_constant(self.operand_of(inst, index)) else {
            return false;
        };

        // Get the constant out of the ICmp, if there is one. Only try this
        // when exactly 1 operand is a constant.
        let cond = self.operand_of(inst, 0);
        let cmp = {
            let q = query(self, None);
            q.instruction_of(cond).and_then(|(_, instruction)| match instruction.opcode {
                Opcode::ICmp(_) => {
                    let (x, c) = (instruction.operands[0], instruction.operands[1]);
                    // A constant of another width is not the arm's: setting it
                    // would not make them equal, and the arm is set again.
                    q.int_constant(c)
                        .filter(|&(_, bits)| !matches!(x, Operand::Constant(_)) && bits == sel_bits)
                        .map(|(bits, _)| bits)
                }
                _ => None,
            })
        };
        let Some(cmp_c) = cmp else { return self.shrink_demanded_constant(inst, index, demanded) };

        // If the constant is already the same as the ICmp, leave it as-is.
        if cmp_c == sel_c {
            return false;
        }
        // If the constants are not already the same, but can be with the
        // demand mask, use the constant value from the ICmp.
        if cmp_c & demanded == sel_c & demanded {
            let ty = self.function.instruction(inst).ty;
            let constant = self.int(ty, cmp_c);
            self.function.set_operand(inst, index, constant);
            return true;
        }
        self.shrink_demanded_constant(inst, index, demanded)
    }

    /// The `add` of a `zext i1` and a `sext i1`, or of two `sext i1`, when the
    /// low bit is not demanded: the logic that stands for it.
    fn demanded_bool_add(
        &mut self,
        inst: InstId,
        ty: llrm_mir::types::TypeId,
    ) -> Option<Operand> {
        let (op0, op1) = (self.operand_of(inst, 0), self.operand_of(inst, 1));
        let i1 = self.context.types.int(1);
        let cast_of = |c: &Self, v: Operand, op: CastOp| {
            let q = query(c, None);
            q.instruction_of(v).and_then(|(_, instruction)| {
                (instruction.opcode == Opcode::Cast(op)).then_some(instruction.operands[0])
            })
        };

        // add iN (zext i1 X), (sext i1 Y) --> sext (~X & Y) to iN
        for (zext, sext) in [(op0, op1), (op1, op0)] {
            if let (Some(x), Some(y)) = (cast_of(self, zext, CastOp::ZExt), cast_of(self, sext, CastOp::SExt))
                && self.has_one_use(zext)
                && self.has_one_use(sext)
                && self.type_of(x) == i1
                && self.type_of(y) == i1
            {
                let not_x = self.create_not(inst, x);
                let and_not = self.create_binop(inst, BinaryOp::And, not_x, y, Flags::default());
                return Some(self.create_cast(inst, CastOp::SExt, and_not, ty, Flags::default()));
            }
        }

        // add iN (sext i1 X), (sext i1 Y) --> sext (X | Y) to iN
        if let (Some(x), Some(y)) = (cast_of(self, op0, CastOp::SExt), cast_of(self, op1, CastOp::SExt))
            && self.type_of(x) == i1
            && self.type_of(y) == i1
            && (self.has_one_use(op0) || self.has_one_use(op1))
        {
            let or = self.create_binop(inst, BinaryOp::Or, x, y, Flags::default());
            return Some(self.create_cast(inst, CastOp::SExt, or, ty, Flags::default()));
        }
        None
    }

    /// `SimplifyMultipleUseDemandedBits(I, DemandedMask, Known, Q, Depth)`:
    /// the value `inst` equals for the one user that reads `demanded`, when it
    /// has more users and cannot change.
    pub fn simplify_multiple_use_demanded_bits(
        &mut self,
        cxt: InstId,
        inst: InstId,
        demanded: u128,
        known: &mut KnownBits,
        depth: u32,
    ) -> Option<Operand> {
        let w = known.width;
        let (opcode, flags, ty) = {
            let instruction = self.function.instruction(inst);
            (instruction.opcode.clone(), instruction.flags, instruction.ty)
        };
        let self_value = self.result_of(inst);
        // Only the binary arms read the operands; a load or a call has fewer
        // than two.
        let (op0, op1) = if matches!(opcode, Opcode::Binary(_)) {
            (self.operand_of(inst, 0), self.operand_of(inst, 1))
        } else {
            (self_value, self_value)
        };
        let d1 = depth + 1;
        let mut lhs = KnownBits::unknown(w);
        let mut rhs = KnownBits::unknown(w);

        // Despite the fact that we can't simplify this instruction in all
        // User's context, we can at least compute the known bits, and we can
        // do simplifications that apply to *just* the one user if we know that
        // this instruction has a simpler value in that context.
        match opcode {
            Opcode::Binary(op @ (BinaryOp::And | BinaryOp::Or | BinaryOp::Xor)) => {
                self.known_of(cxt, op1, &mut rhs, d1);
                self.known_of(cxt, op0, &mut lhs, d1);
                let q = query(self, Some(cxt));
                *known = analyze_known_bits_from_and_xor_or(&q, inst, &lhs, &rhs, depth);
                compute_known_bits_from_context(&q, self_value, known, depth);

                // If the client is only demanding bits that we know, return
                // the known constant.
                if subset(demanded, known.zero | known.one) {
                    return Some(self.int(ty, known.one));
                }

                match op {
                    // If all of the demanded bits are known 1 on one side,
                    // return the other. These bits cannot contribute to the
                    // result of the 'and' in this context.
                    BinaryOp::And => {
                        if subset(demanded, lhs.zero | rhs.one) {
                            return Some(op0);
                        }
                        if subset(demanded, rhs.zero | lhs.one) {
                            return Some(op1);
                        }
                    }
                    // We can simplify (X|Y) -> X or Y in the user's context if
                    // we know that only bits from X or Y are demanded.
                    BinaryOp::Or => {
                        if subset(demanded, lhs.one | rhs.zero) {
                            return Some(op0);
                        }
                        if subset(demanded, rhs.one | lhs.zero) {
                            return Some(op1);
                        }
                    }
                    _ => {
                        if subset(demanded, rhs.zero) {
                            return Some(op0);
                        }
                        if subset(demanded, lhs.zero) {
                            return Some(op1);
                        }
                    }
                }
            }
            Opcode::Binary(BinaryOp::Add) => {
                let nlz = clz(demanded, w);
                let from_ops = mask::<u128>(w - nlz);

                // If an operand adds zeros to every bit below the highest
                // demanded bit, that operand doesn't change the result. Return
                // the other side.
                self.known_of(cxt, op1, &mut rhs, d1);
                if subset(from_ops, rhs.zero) {
                    return Some(op0);
                }

                self.known_of(cxt, op0, &mut lhs, d1);
                if subset(from_ops, lhs.zero) {
                    return Some(op1);
                }

                *known = KnownBits::add(&lhs, &rhs, flags.contains(Flags::NSW), flags.contains(Flags::NUW), false);
                compute_known_bits_from_context(&query(self, Some(cxt)), self_value, known, depth);
            }
            Opcode::Binary(BinaryOp::Sub) => {
                let nlz = clz(demanded, w);
                let from_ops = mask::<u128>(w - nlz);

                // If an operand subtracts zeros from every bit below the
                // highest demanded bit, that operand doesn't change the
                // result. Return the other side.
                self.known_of(cxt, op1, &mut rhs, d1);
                if subset(from_ops, rhs.zero) {
                    return Some(op0);
                }

                self.known_of(cxt, op0, &mut lhs, d1);
                *known = KnownBits::sub(&lhs, &rhs, flags.contains(Flags::NSW), flags.contains(Flags::NUW));
                compute_known_bits_from_context(&query(self, Some(cxt)), self_value, known, depth);
            }
            Opcode::Binary(BinaryOp::AShr) => {
                // Compute the Known bits to simplify things downstream.
                self.known_of(cxt, self_value, known, depth);

                // If this user is only demanding bits that we know, return the
                // known constant.
                if subset(demanded, known.zero | known.one) {
                    return Some(self.int(ty, known.one));
                }

                // If the right shift operand 0 is a result of a left shift by
                // the same amount, this is probably a zero/sign extension,
                // which may be unnecessary, if we do not demand any of the new
                // sign bits. So, return the original operand instead.
                if let Some((_, _, x, left)) = self.binary_of(op0, BinaryOp::Shl)
                    && let (Some(left), Some(right)) = (self.apint_of(left), self.apint_of(op1))
                    && left == right
                    && left < u128::from(w)
                    && subset(demanded, mask::<u128>(w - right as u32))
                {
                    return Some(x);
                }
            }
            _ => {
                // Compute the Known bits to simplify things downstream.
                self.known_of(cxt, self_value, known, depth);

                // If this user is only demanding bits that we know, return the
                // known constant.
                if subset(demanded, known.zero | known.one) {
                    return Some(self.int(ty, known.one));
                }
            }
        }

        None
    }

    /// `simplifyShrShlDemandedBits(Shr, ShrOp1, Shl, ShlOp1, DemandedMask,
    /// Known)`: `(X lsr C1) << C2` as the one shift `X << (C2 - C1)` or
    /// `X >> (C1 - C2)`, where the bits the two differ in are not demanded.
    fn simplify_shr_shl_demanded_bits(
        &mut self,
        shr: InstId,
        shr_op1: u128,
        shl: InstId,
        shl_op1: u128,
        demanded: u128,
        known: &mut KnownBits,
    ) -> Option<Operand> {
        if shl_op1 == 0 || shr_op1 == 0 {
            return None; // No-op.
        }

        let var_x = self.operand_of(shr, 0);
        let ty = self.type_of(var_x);
        let w = self.context.types.int_bits(ty).expect("an integer shift");
        if shl_op1 >= u128::from(w) || shr_op1 >= u128::from(w) {
            return None; // Undef.
        }

        let shl_amt = shl_op1 as u32;
        let shr_amt = shr_op1 as u32;

        known.one = 0;
        known.zero |= mask::<u128>(shl_amt - 1);
        known.zero &= demanded;

        let is_lshr = self.function.instruction(shr).opcode == Opcode::Binary(BinaryOp::LShr);
        let all = mask::<u128>(w);

        // The arithmetic shift of all ones is all ones.
        let bit_mask1 = shl_w(if is_lshr { lshr_w(all, shr_amt) } else { all }, shl_amt, w);
        let bit_mask2 = if shr_amt <= shl_amt {
            shl_w(all, shl_amt - shr_amt, w)
        } else if is_lshr {
            lshr_w(all, shr_amt - shl_amt)
        } else {
            all
        };

        // Check if condition-2 is satisfied.
        if bit_mask1 & demanded != bit_mask2 & demanded {
            return None;
        }
        if shr_amt == shl_amt {
            return Some(var_x);
        }

        if !self.has_one_use(self.result_of(shr)) {
            return None;
        }

        let shl_flags = self.function.instruction(shl).flags;
        let shr_flags = self.function.instruction(shr).flags;
        Some(if shr_amt < shl_amt {
            let amount = self.int(ty, u128::from(shl_amt - shr_amt));
            let mut flags = shl_flags.intersect(Flags::NSW);
            flags.insert(shl_flags.intersect(Flags::NUW));
            self.insert_value(shl, Opcode::Binary(BinaryOp::Shl), ty, vec![var_x, amount], flags)
        } else {
            let amount = self.int(ty, u128::from(shr_amt - shl_amt));
            let op = if is_lshr { BinaryOp::LShr } else { BinaryOp::AShr };
            self.insert_value(shl, Opcode::Binary(op), ty, vec![var_x, amount], shr_flags.intersect(Flags::EXACT))
        })
    }

    /// `simplifyShiftSelectingPackedElement(I, DemandedMask, IC, Depth)`: the
    /// `lshr` of `(Upper << M) | Lower` by 0 or `M` picks one of the halves.
    fn shift_selecting_packed_element(
        &mut self,
        inst: InstId,
        demanded: u128,
        depth: u32,
    ) -> Option<Operand> {
        let w = self.context.types.int_bits(self.function.instruction(inst).ty)?;
        let (or_value, shr_amt) = (self.operand_of(inst, 0), self.operand_of(inst, 1));
        let (_, or_flags, a, b) = self.binary_of(or_value, BinaryOp::Or)?;
        if !self.has_one_use(or_value) || !or_flags.contains(Flags::DISJOINT) {
            return None;
        }
        // m_c_DisjointOr(m_OneUse(m_Shl(Upper, ConstantInt)), Lower)
        let (upper, shl_amt, lower) = [(a, b), (b, a)]
            .into_iter()
            .find_map(
                |(shl, lower)| {
                    let (_, _, upper, amount) = self.binary_of(shl, BinaryOp::Shl)?;
                    (self.has_one_use(shl)).then_some(())?;
                    Some((upper, self.apint_of(amount)?, lower))
                },
            )?;

        if !shl_amt.is_power_of_two() {
            return None;
        }

        let demanded_width = u128::from(active_bits(demanded, w));
        if demanded_width > shl_amt {
            return None;
        }

        // Check that upper demanded bits are not lost from lshift.
        let upper_w = self.context.types.int_bits(self.type_of(upper))?;
        if u128::from(upper_w) < shl_amt + demanded_width {
            return None;
        }

        let known_lower = compute_known_bits(&query(self, Some(inst)), lower, depth);
        if shl_amt < 128 && known_lower.max_value() >> shl_amt != 0 {
            return None;
        }

        let known_shr = compute_known_bits(&query(self, Some(inst)), shr_amt, depth);
        // Verify that ShrAmt is either exactly ShlAmt (which is a power of 2)
        // or zero.
        if !known_shr.zero & mask::<u128>(w) != shl_amt {
            return None;
        }

        let ty = self.type_of(shr_amt);
        let zero = self.int(ty, 0);
        let shr_amt_z = self.create_icmp(inst, IntPredicate::Eq, shr_amt, zero);
        Some(self.create_select(inst, shr_amt_z, lower, upper))
    }

    /// `Inst` is the `shl` or `lshr` of an `or` that
    /// `convertOrOfShiftsToFunnelShift` turns into a rotate: the shift pair,
    /// both with one use, of one value, whose amounts sum to the width or are
    /// the masked negations of each other.
    fn feeds_a_rotate(
        &self,
        shift: InstId,
    ) -> bool {
        let Some(user) = self.function.users(self.function.instruction(shift).result.expect("a value")).only() else {
            return false;
        };
        let or = user.user;
        if self.function.instruction(or).opcode != Opcode::Binary(BinaryOp::Or) {
            return false;
        }
        self.or_is_rotate(or)
    }

    /// `convertOrOfShiftsToFunnelShift(Or)` has equal value operands.
    fn or_is_rotate(
        &self,
        or: InstId,
    ) -> bool {
        let q = query(self, Some(or));
        let w = q.scalar_bits(self.result_of(or));
        let (o0, o1) = (self.operand_of(or, 0), self.operand_of(or, 1));
        let logical_shift = |v: Operand| {
            let (inst, instruction) = q.instruction_of(v)?;
            let Opcode::Binary(op @ (BinaryOp::Shl | BinaryOp::LShr)) = instruction.opcode else { return None };
            self.has_one_use(v).then_some((op, instruction.operands[0], instruction.operands[1], inst))
        };
        let (Some((op0, val0, amt0, _)), Some((op1, val1, amt1, _))) = (logical_shift(o0), logical_shift(o1)) else {
            return false;
        };
        if op0 == op1 || val0 != val1 {
            return false;
        }
        // Canonicalize to or(shl(ShVal, ShAmt0), lshr(ShVal, ShAmt1)).
        let (shl_amt, lshr_amt) = if op0 == BinaryOp::Shl { (amt0, amt1) } else { (amt1, amt0) };
        self.rotate_amount(&q, shl_amt, lshr_amt, w, or) || self.rotate_amount(&q, lshr_amt, shl_amt, w, or)
    }

    /// `matchShiftAmount(L, R, Width)` of a rotate (both shifts of one value).
    fn rotate_amount(
        &self,
        q: &llrm_mir::valuetracking::Query<'_>,
        l: Operand,
        r: Operand,
        w: u32,
        or: InstId,
    ) -> bool {
        // Check for constant shift amounts that sum to the bitwidth.
        if let (Some(li), Some(ri)) = (self.apint_of(l), self.apint_of(r))
            && li < u128::from(w)
            && ri < u128::from(w)
            && li + ri == u128::from(w)
        {
            return true;
        }

        // (shl ShVal, X) | (lshr ShVal, (Width - x)) iff X < Width.
        if let Some((_, _, c, sub_r)) = self.binary_of(r, BinaryOp::Sub)
            && self.has_one_use(r)
            && self.apint_of(c) == Some(u128::from(w))
            && sub_r == l
        {
            return compute_known_bits(&query(self, Some(or)), l, 0).max_value() < u128::from(w);
        }

        // For non-constant cases we don't support non-pow2 shift masks.
        if !w.is_power_of_two() {
            return false;
        }
        let m = u128::from(w - 1);
        let and_of = |v: Operand| {
            let (_, _, x, mk) = pattern::binary(q, v, BinaryOp::And).map(|(i, n, a, b)| (i, n, a, b))?;
            (self.apint_of(mk) == Some(m)).then_some(x)
        };
        let neg_of = |v: Operand| pattern::neg(q, v).map(|(_, x, _)| x);
        let zext_of = |v: Operand| pattern::cast(q, v, CastOp::ZExt).map(|(_, _, x)| x);

        // (shl ShVal, (X & (Width - 1))) | (lshr ShVal, ((-X) & (Width - 1)))
        if let (Some(x), Some(neg)) = (and_of(l), and_of(r))
            && neg_of(neg) == Some(x)
        {
            return true;
        }
        // (shl ShVal,(X+1) & (Width-1)) | (lshr ShVal,((X & (Width-1)) ^
        // (Width-1)))
        if let Some(plus_one) = and_of(l)
            && let Some((_, _, x, one)) = pattern::binary(q, plus_one, BinaryOp::Add)
            && pattern::one(q, one)
            && let Some((_, _, xand, xm)) = pattern::binary(q, r, BinaryOp::Xor)
            && self.apint_of(xm) == Some(m)
            && and_of(xand) == Some(x)
        {
            return true;
        }
        // (shl ShVal, X) | (lshr ShVal, ((-X) & (Width - 1)))
        if let Some(neg) = and_of(r)
            && neg_of(neg) == Some(l)
        {
            return true;
        }
        // The shift amount may be extended after masking.
        if let Some(inner) = zext_of(l)
            && let Some(x) = and_of(inner)
        {
            if let Some(neg) = and_of(r)
                && let Some(zx) = neg_of(neg)
                && let Some(inner2) = zext_of(zx)
                && and_of(inner2) == Some(x)
            {
                return true;
            }
            if let Some(zr) = zext_of(r)
                && let Some(neg) = and_of(zr)
                && neg_of(neg) == Some(x)
            {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
#[path = "combine_demanded_tests.rs"]
mod tests;
