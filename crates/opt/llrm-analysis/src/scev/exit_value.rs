//! Exit counts found by running the loop: `computeShiftCompareExitLimit` and
//! `computeExitCountExhaustively`.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ScalarEvolution.cpp`. A shift
//! recurrence (`%iv = phi [start, %pre], [%iv >> c, %loop]`) reaches 0 or -1
//! in at most `bitwidth` iterations; if the compare is false there, the
//! backedge is taken a bounded number of times.
//!
//! # Deviations
//! - D-T4 (temporary,
//!   [#2328](https://github.com/ali-mosavian/llrm/issues/2328)):
//!   `computeExitCountExhaustively`, `getConstantEvolvingPHI`,
//!   `EvaluateExpression` and `getConstantEvolutionLoopExitValue` run the loop
//!   on constants through `ConstantFoldInstOperands`, which MIR's folder does
//!   not offer for an instruction by operand list. The exhaustive count is
//!   CouldNotCompute, which is LLVM's answer when `getConstantEvolvingPHI`
//!   finds no phi.
//!
//! # Example
//! `while (x >>= 1)` over an `i32` is taken at most 32 times.

use llrm_mir::module::{BlockId, Operand};
use llrm_mir::opcode::{BinaryOp, IntPredicate, Opcode};
use llrm_mir::valuetracking::known::compute_known_bits_at;
use llrm_mir::valuetracking::pattern;

use super::ap::Ap;
use super::expr::Scev;
use super::trip::ExitLimit;
use super::{Cx, ScalarEvolution};

/// `ConstantFoldCompareInstOperands` of two integers.
fn compare(
    pred: IntPredicate,
    x: Ap,
    y: Ap,
) -> bool {
    match pred {
        IntPredicate::Eq => x == y,
        IntPredicate::Ne => x != y,
        IntPredicate::Ugt => y.ult(x),
        IntPredicate::Uge => y.ule(x),
        IntPredicate::Ult => x.ult(y),
        IntPredicate::Ule => x.ule(y),
        IntPredicate::Sgt => y.slt(x),
        IntPredicate::Sge => y.sle(x),
        IntPredicate::Slt => x.slt(y),
        IntPredicate::Sle => x.sle(y),
    }
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L9594
    /// `computeShiftCompareExitLimit`.
    pub(super) fn compute_shift_compare_exit_limit(
        &mut self,
        cx: &Cx,
        header: BlockId,
        lhs: Operand,
        rhs_v: Operand,
        pred: IntPredicate,
    ) -> ExitLimit {
        let query = cx.query(None);
        let loop_ = cx.loops.get_loop_for(header).expect("the header of a loop");
        let Some((rhs_bits, width)) = pattern::apint(&query, rhs_v) else {
            return ExitLimit::could_not_compute(self);
        };
        let rhs = Ap::new(width, rhs_bits);

        let Some(latch) = loop_.get_loop_latch(cx.function) else { return ExitLimit::could_not_compute(self) };
        let Some(predecessor) = loop_.get_loop_predecessor(cx.function) else {
            return ExitLimit::could_not_compute(self);
        };

        // Return true if V is of the form "LHS `shift_op` <positive
        // constant>". Return LHS in OutLHS, shift_op in OutOpCode, and the
        // shift amount in OutShiftAmt.
        let match_positive_shift = |v: Operand| -> Option<(Operand, BinaryOp, u32)> {
            let (_, instruction) = query.instruction_of(v)?;
            let Opcode::Binary(op @ (BinaryOp::LShr | BinaryOp::AShr | BinaryOp::Shl)) = instruction.opcode else {
                return None;
            };
            let (amount, _) = pattern::apint(&query, instruction.operands[1])?;
            let limited = u32::try_from(amount).unwrap_or(u32::MAX);
            let out_lhs = instruction.operands[0];
            let scalar_bits = query.scalar_bits(out_lhs);
            if limited == 0 || limited >= scalar_bits {
                return None;
            }
            Some((out_lhs, op, limited))
        };

        // Recognize a "shift recurrence" either of the form %iv or of
        // %iv.shifted in
        //
        // loop:
        //   %iv = phi i32 [ %iv.shifted, %loop ], [ %val, %preheader ]
        //   %iv.shifted = lshr i32 %iv, <positive constant>
        //
        // Return true on a successful match. Return the corresponding PHI node
        // (%iv above) in PNOut, the opcode of the shift operation in
        // OpCodeOut, and the shift amount in ShiftAmtOut.
        let mut lhs = lhs;
        let mut post_shift_op: Option<BinaryOp> = None;
        // If we encounter a shift instruction, "peel off" the shift operation,
        // and remember that we did so. Later when we inspect %iv's backedge
        // value, we will make sure that the backedge value uses the same
        // operation.
        if let Some((inner, op, _)) = match_positive_shift(lhs) {
            post_shift_op = Some(op);
            lhs = inner;
        }

        let Some((phi_inst, phi)) = query.instruction_of(lhs) else { return ExitLimit::could_not_compute(self) };
        if phi.opcode != Opcode::Phi || cx.function.parent(phi_inst) != Some(header) {
            return ExitLimit::could_not_compute(self);
        }
        let pn = lhs;

        let Some(be_value) = pattern::phi_incoming(phi).find(|&(_, from)| from == latch).map(|(value, _)| value) else {
            return ExitLimit::could_not_compute(self);
        };
        let Some((op_lhs, op_code, shift_amt)) = match_positive_shift(be_value) else {
            // The backedge value for the PHI node must be a shift by a
            // positive amount.
            return ExitLimit::could_not_compute(self);
        };
        // of the PHI node itself, and the kind of shift should be match the
        // kind of shift we peeled off, if any.
        if op_lhs != pn || post_shift_op.is_some_and(|op| op != op_code) {
            return ExitLimit::could_not_compute(self);
        }

        // The key rationale for this optimization is that for some kinds of
        // shift recurrences, the value of the recurrence "stabilizes" to
        // either 0 or -1 within a finite number of iterations. If the
        // condition guarding the backedge (in the sense that the backedge is
        // taken if the condition is true) is false for the value the shift
        // recurrence stabilizes to, then we know that the backedge is taken
        // only a finite number of times.
        let stable_value = match op_code {
            BinaryOp::AShr => {
                // {K,ashr,<positive-constant>} stabilizes to signum(K) in at
                // most bitwidth(K) iterations.
                let Some(first_value) =
                    pattern::phi_incoming(phi).find(|&(_, from)| from == predecessor).map(|(value, _)| value)
                else {
                    return ExitLimit::could_not_compute(self);
                };
                let known = compute_known_bits_at(&query, first_value, cx.function.terminator(predecessor), 0);
                if known.is_non_negative() {
                    Ap::zero(width)
                } else if known.is_negative() {
                    Ap::all_ones(width)
                } else {
                    return ExitLimit::could_not_compute(self);
                }
            }
            // Both {K,lshr,<positive-constant>} and {K,shl,<positive-constant>}
            // stabilize to 0 in at most bitwidth(K) iterations.
            _ => Ap::zero(width),
        };

        let result = compare(pred, stable_value, rhs);

        if !result {
            let bit_width = self.get_type_size_in_bits(cx, self.ty_of_operand(cx, rhs_v));
            let mut max_btc = bit_width;

            // For right-shift recurrences (lshr/ashr with non-negative start),
            // we can compute a tighter max backedge-taken count from the range
            // of the start value. After k shifts of ShiftAmt, value =
            // start >> (k * ShiftAmt). The value reaches 0 (the stable value)
            // when k * ShiftAmt >= activeBits(start), so max BTC =
            // ceil(activeBits(maxStart) / ShiftAmt).
            if matches!(op_code, BinaryOp::LShr | BinaryOp::AShr)
                && let Some(start_value) =
                    pattern::phi_incoming(phi).find(|&(_, from)| from == predecessor).map(|(value, _)| value)
            {
                let start_scev = self.get_scev(cx, start_value);
                let max_start = self.get_unsigned_range_max(cx, start_scev);
                if !max_start.is_zero() {
                    let active_bits = max_start.active_bits();
                    let range_btc = active_bits.div_ceil(shift_amt);
                    max_btc = max_btc.min(range_btc);
                }
            }

            let ty = self.ty_of_operand(cx, rhs_v);
            let ty = self.get_effective_scev_type(cx, ty);
            let upper_bound = self.get_constant_of_type(cx, ty, i128::from(max_btc));
            let unknown = self.get_could_not_compute();
            return ExitLimit::new(self, unknown, upper_bound, upper_bound, false);
        }

        ExitLimit::could_not_compute(self)
    }

    /// The SCEV type of a value's IR type.
    fn ty_of_operand(
        &self,
        cx: &Cx,
        v: Operand,
    ) -> super::expr::Ty {
        let ty = cx.function.operand_type(cx.context, v).expect("a typed value");
        self.ty_of(cx, ty).expect("a SCEVable value")
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L10040
    /// `computeExitCountExhaustively` (D-T4): CouldNotCompute.
    pub(super) fn compute_exit_count_exhaustively(
        &mut self,
        _cx: &Cx,
        _header: BlockId,
        _cond: Operand,
        _exit_when: bool,
    ) -> Scev {
        self.get_could_not_compute()
    }
}
