//! `visitFADD`, `visitFSUB`, `visitFMUL` and `visitFDIV`, with
//! `simplifyFPBinop`, `canIgnoreSignBitOfZero` and the `fneg` part of
//! `TargetLowering::getNegatedExpression`.
//!
//! DEVIATE FP2: no vector forms, no `foldBinOpIntoSelect`, no FMA fusion
//! (`visit*ForFMACombine`: the targets have no FMA), no
//! `combineFMulOrFDivWithIntPow2`, no `fmul` of a signed select, no
//! `combineRepeatedFPDivisors`, rsqrt or reciprocal estimates in `visitFDIV`
//! (x87 has none), and
//! `getNegatedExpression` knows `fneg`, constants, `fadd`, `fsub`, `fmul` and
//! `fdiv` only (not `fma`, `fp_extend`, `fp_round`, `select`).
//! `isKnownNeverLogicalZero` is "a non-zero constant". Exit: those. Owner
//! sdag-c.
//!
//! # See also
//! LLVM's `DAGCombiner::visitFADD`
//! ([L19421](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L19421)),
//! `visitFSUB`
//! ([L19636](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L19636)),
//! `visitFMUL`
//! ([L19825](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L19825)),
//! `SelectionDAG::simplifyFPBinop`
//! ([L11767](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAG.cpp#L11767)),
//! `TargetLowering::getNegatedExpression`
//! ([L7726](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L7726)).

use super::dag::{NodeData, SDNodeFlags, SDValue};
use super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::fp_value::{self, FpClass};
use super::isd::ISD;
use super::mvt::EVT;

/// `TargetLowering::NegatibleCost`, in the pin's order; `Expensive` is `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Cost {
    Cheaper,
    Neutral,
}

/// `MaxRecursionDepth`.
const MAX_DEPTH: u32 = 6;

impl<H: CombineHooks> DagCombiner<'_, H> {
    pub(super) fn legal_operations(&self) -> bool {
        self.level >= CombineLevel::AfterLegalizeVectorOps
    }

    /// `isOperationLegalOrCustom(op, vt)`; false without a table.
    pub(super) fn fp_op_legal_or_custom(
        &self,
        op: ISD,
        vt: EVT,
    ) -> bool {
        self.tli.is_some_and(|tli| tli.is_operation_legal_or_custom(op, vt))
    }

    pub(super) fn fp_op_legal(
        &self,
        op: ISD,
        vt: EVT,
    ) -> bool {
        self.tli.is_some_and(|tli| tli.is_operation_legal(op, vt))
    }

    /// The class of `value` when it is a `ConstantFP`.
    fn fp_const(
        &self,
        value: SDValue,
    ) -> Option<FpClass> {
        let NodeData::ConstFp { bits } = self.dag.node(value).data else { return None };
        fp_value::classify(self.dag.value_type(value), bits)
    }

    fn fp_bits(
        &self,
        value: SDValue,
    ) -> Option<u128> {
        let NodeData::ConstFp { bits } = self.dag.node(value).data else { return None };
        Some(bits)
    }

    /// `isExactlyValue(v)` for a small integer `v`.
    fn is_exactly(
        &self,
        value: SDValue,
        v: u64,
        negative: bool,
    ) -> bool {
        let vt = self.dag.value_type(value);
        let Some(bits) = self.fp_bits(value) else { return false };
        let want = fp_value::from_integer(vt, v).and_then(|b| if negative { fp_value::negate(vt, b) } else { Some(b) });
        want == Some(bits)
    }

    fn fp_constant(
        &mut self,
        v: u64,
        vt: EVT,
    ) -> Option<SDValue> {
        let bits = fp_value::from_integer(vt, v)?;
        Some(self.dag.get_constant_fp(bits, vt))
    }

    /// `simplifyFPBinop`.
    fn simplify_fp_binop(
        &mut self,
        opcode: ISD,
        x: SDValue,
        y: SDValue,
        flags: SDNodeFlags,
    ) -> Option<SDValue> {
        let (xc, yc) = (self.fp_const(x), self.fp_const(y));
        let has_nan = xc.is_some_and(|c| c.nan) || yc.is_some_and(|c| c.nan);
        let has_inf = xc.is_some_and(|c| c.infinite) || yc.is_some_and(|c| c.infinite);
        let undef = self.dag.is_undef(x) || self.dag.is_undef(y);
        let vt = self.dag.value_type(x);
        if flags.has(SDNodeFlags::NO_NANS) && (has_nan || undef) {
            return Some(self.dag.get_undef(vt));
        }
        if flags.has(SDNodeFlags::NO_INFS) && (has_inf || undef) {
            return Some(self.dag.get_undef(vt));
        }
        let yc = yc?;
        let zero = yc.zero;
        match opcode {
            // X + -0.0 --> X
            ISD::FAdd if zero && yc.negative => Some(x),
            // X - +0.0 --> X
            ISD::FSub if zero && !yc.negative => Some(x),
            // X * 1.0 --> X, X / 1.0 --> X
            ISD::FMul | ISD::FDiv if yc.one => Some(x),
            // X * 0.0 --> 0.0
            ISD::FMul if flags.has(SDNodeFlags::NO_NANS) && flags.has(SDNodeFlags::NO_SIGNED_ZEROS) && zero => {
                Some(self.dag.get_constant_fp(0, self.dag.value_type(y)))
            }
            _ => None,
        }
    }

    /// `isKnownNeverLogicalZero`, as far as a constant shows it.
    fn fp_known_never_zero(
        &self,
        value: SDValue,
    ) -> bool {
        self.fp_const(value).is_some_and(|c| !c.zero && !c.nan)
    }

    /// `SelectionDAG::canIgnoreSignBitOfZero`.
    fn can_ignore_sign_bit_of_zero(
        &self,
        node: SDValue,
    ) -> bool {
        if self.dag.node(node).flags.has(SDNodeFlags::NO_SIGNED_ZEROS) {
            return true;
        }
        if self.dag.use_count_up_to(node.node, 3) > 2 {
            return false;
        }
        self.dag
            .users(node)
            .all(
                |user| {
                    let user_node = self.dag.node(SDValue { node: user, res: 0 });
                    if user_node.flags.has(SDNodeFlags::NO_SIGNED_ZEROS) {
                        return true;
                    }
                    let at = user_node.ops.iter().position(|op| *op == node).unwrap_or(0);
                    match user_node.opcode {
                        ISD::SetCC | ISD::FAbs | ISD::FpToSInt | ISD::FpToUInt => true,
                        ISD::FCopySign => at == 0,
                        ISD::FAdd | ISD::FSub => self.fp_known_never_zero(user_node.ops[1 - at]),
                        _ => false,
                    }
                },
            )
    }

    /// `TargetLowering::getNegatedExpression`: the negation and what it
    /// costs, or `None` when it is not cheap enough.
    fn negated_expression(
        &mut self,
        op: SDValue,
        depth: u32,
    ) -> Option<(SDValue, Cost)> {
        if self.dag.opcode(op) == ISD::FNeg {
            return Some((self.dag.operand(op, 0), Cost::Cheaper));
        }
        if depth > MAX_DEPTH {
            return None;
        }
        let depth = depth + 1;
        let vt = self.dag.value_type(op);
        let opcode = self.dag.opcode(op);
        let flags = self.dag.node(op).flags;
        if !self.dag.has_one_use(op.node) && opcode != ISD::ConstantFP {
            return None;
        }
        match opcode {
            ISD::ConstantFP => {
                let negated = fp_value::negate(vt, self.fp_bits(op)?)?;
                let legal = self
                    .tli
                    .is_some_and(
                        |tli| tli.is_operation_legal(ISD::ConstantFP, vt)
                            || matches!(vt, EVT::Simple(m) if tli.is_fp_imm_legal(m, negated)),
                    );
                if self.legal_operations() && !legal {
                    return None;
                }
                let made = self.dag.get_constant_fp(negated, vt);
                // free to negate a multiply-used constant only if the
                // negation is already in use
                if !self.dag.has_one_use(op.node) && !self.dag.has_users(made) {
                    return None;
                }
                Some((made, Cost::Neutral))
            }
            ISD::FAdd => {
                if !flags.has(SDNodeFlags::NO_SIGNED_ZEROS)
                    || (self.legal_operations() && !self.fp_op_legal_or_custom(ISD::FSub, vt))
                {
                    return None;
                }
                let (x, y) = (self.dag.operand(op, 0), self.dag.operand(op, 1));
                let neg_x = self.negated_expression(x, depth);
                let neg_y = self.negated_expression(y, depth);
                if let Some((n, cost_x)) = neg_x.filter(|(_, cx)| neg_y.is_none_or(|(_, cy)| *cx <= cy)) {
                    return Some((self.dag.get_node_flags(ISD::FSub, vt, &[n, y], flags), cost_x));
                }
                let (n, cost_y) = neg_y?;
                Some((self.dag.get_node_flags(ISD::FSub, vt, &[n, x], flags), cost_y))
            }
            ISD::FSub => {
                if !flags.has(SDNodeFlags::NO_SIGNED_ZEROS) {
                    return None;
                }
                let (x, y) = (self.dag.operand(op, 0), self.dag.operand(op, 1));
                if self.fp_const(x).is_some_and(|c| c.zero) {
                    return Some((y, Cost::Cheaper));
                }
                Some((self.dag.get_node_flags(ISD::FSub, vt, &[y, x], flags), Cost::Neutral))
            }
            ISD::FMul | ISD::FDiv => {
                let (x, y) = (self.dag.operand(op, 0), self.dag.operand(op, 1));
                let neg_x = self.negated_expression(x, depth);
                let neg_y = self.negated_expression(y, depth);
                if let Some((n, cost_x)) = neg_x.filter(|(_, cx)| neg_y.is_none_or(|(_, cy)| *cx <= cy)) {
                    return Some((self.dag.get_node_flags(opcode, vt, &[n, y], flags), cost_x));
                }
                // "Ignore X * 2.0 because that is expected to be canonicalized
                // to X + X."
                if opcode == ISD::FMul && self.is_exactly(y, 2, false) {
                    return None;
                }
                let (n, cost_y) = neg_y?;
                Some((self.dag.get_node_flags(opcode, vt, &[x, n], flags), cost_y))
            }
            _ => None,
        }
    }

    /// `getCheaperNegatedExpression`.
    fn cheaper_negated(
        &mut self,
        op: SDValue,
    ) -> Option<SDValue> {
        let (negated, cost) = self.negated_expression(op, 0)?;
        (cost <= Cost::Cheaper).then_some(negated)
    }

    /// `visitFADD`.
    pub(super) fn visit_fadd(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        let flags = self.dag.node(node).flags;
        if !vt.is_floating_point() {
            return None;
        }
        let (n0_cfp, n1_cfp) = (self.fp_const(n0).is_some(), self.fp_const(n1).is_some());

        if let Some(r) = self.simplify_fp_binop(ISD::FAdd, n0, n1, flags) {
            return Some(r);
        }
        if let Some(c) = self.dag.fold_constant_fp(ISD::FAdd, vt, n0, n1) {
            return Some(c);
        }
        // canonicalize constant to RHS
        if n0_cfp && !n1_cfp {
            return Some(self.dag.get_node_flags(ISD::FAdd, vt, &[n1, n0], flags));
        }
        // N0 + -0.0 --> N0 (also allowed with +0.0 and fast-math)
        if let Some(c) = self.fp_const(n1).filter(|c| c.zero) {
            if c.negative || self.can_ignore_sign_bit_of_zero(node) {
                return Some(n0);
            }
        }
        let fsub_ok = !self.legal_operations() || self.fp_op_legal_or_custom(ISD::FSub, vt);
        // fold (fadd A, (fneg B)) -> (fsub A, B)
        if fsub_ok {
            if let Some(neg) = self.cheaper_negated(n1) {
                return Some(self.dag.get_node_flags(ISD::FSub, vt, &[n0, neg], flags));
            }
        }
        // fold (fadd (fneg A), B) -> (fsub B, A)
        if fsub_ok {
            if let Some(neg) = self.cheaper_negated(n0) {
                return Some(self.dag.get_node_flags(ISD::FSub, vt, &[n1, neg], flags));
            }
        }
        let is_fmul_neg_two = |this: &Self, value: SDValue| {
            this.dag.has_one_use(value.node)
                && this.dag.opcode(value) == ISD::FMul
                && this.is_exactly(this.dag.operand(value, 1), 2, true)
        };
        // fadd (fmul B, -2.0), A --> fsub A, (fadd B, B)
        for (mul, other) in [(n0, n1), (n1, n0)] {
            if is_fmul_neg_two(self, mul) {
                let b = self.dag.operand(mul, 0);
                let add = self.dag.get_node_flags(ISD::FAdd, vt, &[b, b], flags);
                return Some(self.dag.get_node_flags(ISD::FSub, vt, &[other, add], flags));
            }
        }
        // No FP constant should be created after legalization.
        let allow_new_const = self.level < CombineLevel::AfterLegalizeDag;
        if flags.has(SDNodeFlags::NO_NANS) && allow_new_const {
            // (fadd (fneg x), x) and (fadd x, (fneg x)) -> 0.0
            let cancels =
                |this: &Self, a: SDValue, b: SDValue| this.dag.opcode(a) == ISD::FNeg && this.dag.operand(a, 0) == b;
            if cancels(self, n0, n1) || cancels(self, n1, n0) {
                return Some(self.dag.get_constant_fp(0, vt));
            }
        }
        if flags.has(SDNodeFlags::ALLOW_REASSOC) && flags.has(SDNodeFlags::NO_SIGNED_ZEROS) && allow_new_const {
            return self.reassociated_fadd(node, n0, n1, vt, flags);
        }
        None
    }

    /// The `reassoc` and `nsz` block of `visitFADD`.
    fn reassociated_fadd(
        &mut self,
        _node: SDValue,
        n0: SDValue,
        n1: SDValue,
        vt: EVT,
        flags: SDNodeFlags,
    ) -> Option<SDValue> {
        let is_cfp = |this: &Self, v: SDValue| this.fp_const(v).is_some();
        let (n0_cfp, n1_cfp) = (is_cfp(self, n0), is_cfp(self, n1));
        // fadd (fadd x, c1), c2 -> fadd x, c1 + c2
        if n1_cfp && self.dag.opcode(n0) == ISD::FAdd && is_cfp(self, self.dag.operand(n0, 1)) {
            let new_c = self.dag.get_node_flags(ISD::FAdd, vt, &[self.dag.operand(n0, 1), n1], flags);
            return Some(self.dag.get_node_flags(ISD::FAdd, vt, &[self.dag.operand(n0, 0), new_c], flags));
        }
        if (self.legal_operations() && !self.fp_op_legal_or_custom(ISD::FMul, vt)) || n0_cfp || n1_cfp {
            return None;
        }
        let same_pair = |this: &Self, v: SDValue| {
            this.dag.opcode(v) == ISD::FAdd && this.dag.operand(v, 0) == this.dag.operand(v, 1)
        };
        // (fadd (fmul x, c), x) -> (fmul x, c+1); (fadd (fmul x, c), (fadd x,
        // x)) -> (fmul x, c+2)
        for (mul, other) in [(n0, n1), (n1, n0)] {
            if self.dag.opcode(mul) != ISD::FMul {
                continue;
            }
            let (m0, m1) = (self.dag.operand(mul, 0), self.dag.operand(mul, 1));
            if !is_cfp(self, m1) || is_cfp(self, m0) {
                continue;
            }
            if m0 == other {
                let one = self.fp_constant(1, vt)?;
                let c = self.dag.get_node_flags(ISD::FAdd, vt, &[m1, one], flags);
                return Some(self.dag.get_node_flags(ISD::FMul, vt, &[other, c], flags));
            }
            if same_pair(self, other) && self.dag.operand(other, 0) == m0 {
                let two = self.fp_constant(2, vt)?;
                let c = self.dag.get_node_flags(ISD::FAdd, vt, &[m1, two], flags);
                return Some(self.dag.get_node_flags(ISD::FMul, vt, &[m0, c], flags));
            }
        }
        // (fadd (fadd x, x), x) and (fadd x, (fadd x, x)) -> (fmul x, 3.0)
        for (pair, other) in [(n0, n1), (n1, n0)] {
            if same_pair(self, pair) && !is_cfp(self, self.dag.operand(pair, 0)) && self.dag.operand(pair, 0) == other {
                let three = self.fp_constant(3, vt)?;
                return Some(self.dag.get_node_flags(ISD::FMul, vt, &[other, three], flags));
            }
        }
        // (fadd (fadd x, x), (fadd x, x)) -> (fmul x, 4.0)
        if same_pair(self, n0) && same_pair(self, n1) && self.dag.operand(n0, 0) == self.dag.operand(n1, 0) {
            let four = self.fp_constant(4, vt)?;
            return Some(self.dag.get_node_flags(ISD::FMul, vt, &[self.dag.operand(n0, 0), four], flags));
        }
        None
    }

    /// `visitFSUB`.
    pub(super) fn visit_fsub(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        let flags = self.dag.node(node).flags;
        if !vt.is_floating_point() {
            return None;
        }
        let (n0_cfp, n1_cfp) = (self.fp_const(n0), self.fp_const(n1));
        if let Some(r) = self.simplify_fp_binop(ISD::FSub, n0, n1, flags) {
            return Some(r);
        }
        if let Some(c) = self.dag.fold_constant_fp(ISD::FSub, vt, n0, n1) {
            return Some(c);
        }
        // (fsub A, 0) -> A
        if let Some(c) = n1_cfp.filter(|c| c.zero) {
            if !c.negative || self.can_ignore_sign_bit_of_zero(node) {
                return Some(n0);
            }
        }
        // (fsub x, x) -> 0.0
        if n0 == n1 && flags.has(SDNodeFlags::NO_NANS) {
            return Some(self.dag.get_constant_fp(0, vt));
        }
        // (fsub -0.0, N1) -> -N1
        if let Some(c) = n0_cfp.filter(|c| c.zero) {
            if c.negative || self.can_ignore_sign_bit_of_zero(node) {
                if let Some((neg, _)) = self.negated_expression(n1, 0) {
                    return Some(neg);
                }
                if !self.legal_operations() || self.fp_op_legal(ISD::FNeg, vt) {
                    return Some(self.dag.get_node_flags(ISD::FNeg, vt, &[n1], flags));
                }
            }
        }
        if flags.has(SDNodeFlags::ALLOW_REASSOC)
            && flags.has(SDNodeFlags::NO_SIGNED_ZEROS)
            && self.dag.opcode(n1) == ISD::FAdd
        {
            // X - (X + Y) -> -Y, X - (Y + X) -> -Y
            for (same, other) in [(0, 1), (1, 0)] {
                if n0 == self.dag.operand(n1, same) {
                    let y = self.dag.operand(n1, other);
                    return Some(self.dag.get_node_flags(ISD::FNeg, vt, &[y], flags));
                }
            }
        }
        // fold (fsub A, (fneg B)) -> (fadd A, B)
        let (neg, _) = self.negated_expression(n1, 0)?;
        Some(self.dag.get_node_flags(ISD::FAdd, vt, &[n0, neg], flags))
    }

    /// `visitFNEG`: a constant, an expression that negates for free, and
    /// `-(x - y)` as `y - x` where signed zeros do not matter.
    pub(super) fn visit_fneg(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let n0 = self.dag.operand(node, 0);
        let vt = self.dag.value_type(node);
        let flags = self.dag.node(node).flags;
        if !vt.is_floating_point() {
            return None;
        }
        if let Some(bits) = self.fp_bits(n0) {
            return Some(self.dag.get_constant_fp(fp_value::negate(vt, bits)?, vt));
        }
        if let Some((negated, _)) = self.negated_expression(n0, 0) {
            return Some(negated);
        }
        (self.dag.opcode(n0) == ISD::FSub && flags.has(SDNodeFlags::NO_SIGNED_ZEROS) && self.dag.has_one_use(n0.node))
            .then(|| {
                let (x, y) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
                self.dag.get_node_flags(ISD::FSub, vt, &[y, x], flags)
            })
    }

    /// `visitFABS`: a constant.
    pub(super) fn visit_fabs(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let n0 = self.dag.operand(node, 0);
        let vt = self.dag.value_type(node);
        let bits = self.fp_bits(n0).filter(|_| vt.is_floating_point())?;
        Some(self.dag.get_constant_fp(fp_value::abs(vt, bits)?, vt))
    }

    /// `visitFDIV`: the reciprocal multiply, `x / sqrt(x)` and `-x / -y`.
    pub(super) fn visit_fdiv(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        let flags = self.dag.node(node).flags;
        if !vt.is_floating_point() {
            return None;
        }
        if let Some(r) = self.simplify_fp_binop(ISD::FDiv, n0, n1, flags) {
            return Some(r);
        }
        if let Some(c) = self.dag.fold_constant_fp(ISD::FDiv, vt, n0, n1) {
            return Some(c);
        }
        // fold (fdiv X, c2) -> (fmul X, 1/c2) if there is no loss in
        // precision, or the loss is acceptable with AllowReciprocal.
        if let Some((recip, exact)) = self.fp_bits(n1).and_then(|b| fp_value::reciprocal(vt, b)) {
            let legal_imm = !self.legal_operations()
                || self.fp_op_legal(ISD::ConstantFP, vt)
                || matches!(
                    vt,
                    EVT::Simple(m) if self.tli.is_some_and(|tli| tli.is_fp_imm_legal(m, recip))
                );
            if (exact || flags.has(SDNodeFlags::ALLOW_RECIPROCAL)) && legal_imm {
                let c = self.dag.get_constant_fp(recip, vt);
                return Some(self.dag.get_node_flags(ISD::FMul, vt, &[n0, c], flags));
            }
        }
        // fold X/Sqrt(X) -> Sqrt(X)
        if self.can_ignore_sign_bit_of_zero(node)
            && flags.has(SDNodeFlags::ALLOW_REASSOC)
            && self.dag.opcode(n1) == ISD::FSqrt
            && n0 == self.dag.operand(n1, 0)
        {
            return Some(n1);
        }
        // (fdiv (fneg X), (fneg Y)) -> (fdiv X, Y)
        let (neg0, cost0) = self.negated_expression(n0, 0)?;
        let (neg1, cost1) = self.negated_expression(n1, 0)?;
        (cost0 == Cost::Cheaper || cost1 == Cost::Cheaper)
            .then(|| self.dag.get_node_flags(ISD::FDiv, vt, &[neg0, neg1], flags))
    }

    fn int_const(
        &self,
        value: SDValue,
    ) -> Option<u128> {
        match self.dag.node(value).data {
            NodeData::Const { bits, .. } => Some(bits),
            _ => None,
        }
    }

    /// `visitFP_ROUND`: a constant, `round(extend x)` and `round(round x)`.
    pub(super) fn visit_fp_round(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        if let Some(bits) = self.fp_bits(n0)
            && let Some(c) = fp_value::convert(self.dag.value_type(n0), vt, bits)
        {
            return Some(self.dag.get_constant_fp(c, vt));
        }
        let inner = self.dag.operand(n0, 0);
        match self.dag.opcode(n0) {
            // fold (fp_round (fp_extend x)) -> x
            ISD::FpExtend if self.dag.value_type(inner) == vt => Some(inner),
            // fold (fp_round (fp_round x)) -> (fp_round x), when the first
            // truncates exactly, or both allow contraction
            ISD::FpRound => {
                let (trunc, inner_trunc) = (self.int_const(n1)? == 1, self.int_const(self.dag.operand(n0, 1))? == 1);
                let contract = self.dag.node(node).flags.has(SDNodeFlags::ALLOW_CONTRACT)
                    && self.dag.node(n0).flags.has(SDNodeFlags::ALLOW_CONTRACT);
                if (self.legal_operations() && !self.fp_op_legal_or_custom(ISD::FpRound, vt))
                    || !(contract || inner_trunc)
                {
                    return None;
                }
                let flag =
                    self.dag.get_constant_with(u128::from(trunc && inner_trunc), self.dag.value_type(n1), true, false);
                Some(self.dag.get_node(ISD::FpRound, vt, &[inner, flag]))
            }
            _ => None,
        }
    }

    /// `visitFP_EXTEND`: a constant and `extend(round(x, 1))`.
    pub(super) fn visit_fp_extend(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let n0 = self.dag.operand(node, 0);
        let vt = self.dag.value_type(node);
        // fp_round(fpextend) is left for the round to fold
        if self.dag.has_one_use(node.node)
            && self
                .dag
                .users(node)
                .next()
                .is_some_and(|user| self.dag.node(SDValue { node: user, res: 0 }).opcode == ISD::FpRound)
        {
            return None;
        }
        if let Some(bits) = self.fp_bits(n0)
            && let Some(c) = fp_value::convert(self.dag.value_type(n0), vt, bits)
        {
            return Some(self.dag.get_constant_fp(c, vt));
        }
        if self.dag.opcode(n0) == ISD::FpRound && self.int_const(self.dag.operand(n0, 1)) == Some(1) {
            let (inner, flag) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
            let in_vt = self.dag.value_type(inner);
            return Some(if in_vt == vt {
                inner
            } else if vt.size_in_bits() < in_vt.size_in_bits() {
                self.dag.get_node(ISD::FpRound, vt, &[inner, flag])
            } else {
                self.dag.get_node(ISD::FpExtend, vt, &[inner])
            });
        }
        None
    }

    /// `visitSINT_TO_FP` and `visitFP_TO_SINT` of a constant.
    pub(super) fn visit_fp_int_convert(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let n0 = self.dag.operand(node, 0);
        let vt = self.dag.value_type(node);
        if self.dag.opcode(node) == ISD::SIntToFp {
            let width = self.dag.value_type(n0).size_in_bits();
            let bits = self.int_const(n0).filter(|_| width <= 64)?;
            let value = ((bits << (128 - width)) as i128 >> (128 - width)) as i64;
            return Some(self.dag.get_constant_fp(fp_value::from_signed(vt, value)?, vt));
        }
        let value = fp_value::to_signed(self.dag.value_type(n0), self.fp_bits(n0)?, vt.size_in_bits())?;
        Some(self.dag.get_constant(value as u128 & super::fold::mask(vt.size_in_bits()), vt))
    }

    /// `visitFMUL`.
    pub(super) fn visit_fmul(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        let flags = self.dag.node(node).flags;
        if !vt.is_floating_point() {
            return None;
        }
        let n1_cfp = self.fp_const(n1);
        if let Some(r) = self.simplify_fp_binop(ISD::FMul, n0, n1, flags) {
            return Some(r);
        }
        if let Some(c) = self.dag.fold_constant_fp(ISD::FMul, vt, n0, n1) {
            return Some(c);
        }
        // canonicalize constant to RHS
        if self.fp_const(n0).is_some() && n1_cfp.is_none() {
            return Some(self.dag.get_node_flags(ISD::FMul, vt, &[n1, n0], flags));
        }
        if flags.has(SDNodeFlags::ALLOW_REASSOC) {
            // fmul (fmul X, C1), C2 -> fmul X, C1 * C2
            if n1_cfp.is_some() && self.dag.opcode(n0) == ISD::FMul {
                let (n00, n01) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
                // the inner multiply has not been constant folded yet
                if self.fp_const(n01).is_some() && self.fp_const(n00).is_none() {
                    let c = self.dag.get_node_flags(ISD::FMul, vt, &[n01, n1], flags);
                    return Some(self.dag.get_node_flags(ISD::FMul, vt, &[n00, c], flags));
                }
            }
            // fmul (fadd X, X), C -> fmul X, 2.0 * C
            if self.dag.opcode(n0) == ISD::FAdd
                && self.dag.has_one_use(n0.node)
                && self.dag.operand(n0, 0) == self.dag.operand(n0, 1)
            {
                let two = self.fp_constant(2, vt)?;
                let c = self.dag.get_node_flags(ISD::FMul, vt, &[two, n1], flags);
                return Some(self.dag.get_node_flags(ISD::FMul, vt, &[self.dag.operand(n0, 0), c], flags));
            }
        }
        // fold (fmul X, 2.0) -> (fadd X, X)
        if self.is_exactly(n1, 2, false) {
            return Some(self.dag.get_node_flags(ISD::FAdd, vt, &[n0, n0], flags));
        }
        // fold (fmul X, -1.0) -> (fsub -0.0, X)
        if self.is_exactly(n1, 1, true) && (!self.legal_operations() || self.fp_op_legal(ISD::FSub, vt)) {
            let minus_zero = fp_value::negative_zero(vt)?;
            let zero = self.dag.get_constant_fp(minus_zero, vt);
            return Some(self.dag.get_node_flags(ISD::FSub, vt, &[zero, n0], flags));
        }
        // -N0 * -N1 --> N0 * N1
        let (neg0, cost0) = self.negated_expression(n0, 0)?;
        let (neg1, cost1) = self.negated_expression(n1, 0)?;
        (cost0 == Cost::Cheaper || cost1 == Cost::Cheaper)
            .then(|| self.dag.get_node_flags(ISD::FMul, vt, &[neg0, neg1], flags))
    }
}
