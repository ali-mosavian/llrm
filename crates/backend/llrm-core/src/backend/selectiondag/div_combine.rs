//! `visitSDIV`, `visitUDIV` and `visitREM` with the division-by-constant
//! logic: `BuildSDIV` and `BuildUDIV` (multiply-high and shifts, magic
//! numbers from [`super::div_const`]) and the power-of-two shifts.
//!
//! DEVIATE SD206: integer types the target holds in a register only (no
//! promoted-type multiply, no `BuildExactSDIV`/`BuildExactUDIV` for `exact`
//! divides, no widened `MULHU` of an `i32`), no `DIVREM` merging, and
//! `BuildSDIVPow2` is the generic shift sequence: the pin's `cmov` form for a
//! CPU with `cmov` (P6 and later) is not ported. `isIntDivCheap` is the
//! configured `int_div_cheap` (the pin's `minsize`; the `-Os` rule of the
//! target is not wired). Exit: the promoted and exact forms, the `cmov`
//! sequence, the `-Os` rule.
//! Owner sdag-b. Review 2026-11-27.
//!
//! # See also
//! LLVM's `DAGCombiner::visitSDIV`
//! ([L5294](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L5294)),
//! `visitUDIV`
//! ([L5452](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L5452)),
//! `TargetLowering::BuildSDIV`
//! ([L6749](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L6749)),
//! `BuildUDIV`
//! ([L6938](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L6938)).

use super::dag::{SDNodeFlags, SDValue};
use super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::div_const::{signed_magic, unsigned_magic};
use super::isd::{CondCode, ISD};
use super::mvt::EVT;

impl<H: CombineHooks> DagCombiner<'_, H> {
    /// `isOperationLegalOrCustom(op, vt, IsAfterLegalization)`, and the type
    /// legal; false without a table.
    fn can_do(
        &self,
        op: ISD,
        vt: EVT,
    ) -> bool {
        let Some(tli) = self.tli else { return false };
        if self.level >= CombineLevel::AfterLegalizeVectorOps {
            tli.is_operation_legal(op, vt)
        } else {
            tli.is_operation_legal_or_custom(op, vt)
        }
    }

    fn shift_amount(
        &mut self,
        count: u32,
    ) -> SDValue {
        self.dag.get_constant(u128::from(count), EVT::I8)
    }

    /// The high half of `x * y` (`GetMULHS`/`GetMULHU`): the node, or the
    /// second result of the double-width multiply.
    fn multiply_high(
        &mut self,
        signed: bool,
        vt: EVT,
        x: SDValue,
        y: SDValue,
    ) -> Option<SDValue> {
        let (high, lohi) = if signed { (ISD::MulHS, ISD::SMulLoHi) } else { (ISD::MulHU, ISD::UMulLoHi) };
        if self.can_do(high, vt) {
            return Some(self.dag.get_node(high, vt, &[x, y]));
        }
        if self.can_do(lohi, vt) {
            let both = self.dag.get_node_multi(lohi, &[vt, vt], &[x, y]);
            return Some(SDValue { node: both.node, res: 1 });
        }
        // Neither: "if the type twice as wide is legal, widen and use a mul
        // plus a shift" (`MulVT = WideVT`, [L6798][pin]).
        //
        // [pin]:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L6798
        let bits = vt.size_in_bits();
        let wide = EVT::integer(bits * 2);
        if bits <= 16 && self.can_do(ISD::Mul, wide) {
            let extend = if signed { ISD::SignExtend } else { ISD::ZeroExtend };
            let (x, y) = (self.dag.get_node(extend, wide, &[x]), self.dag.get_node(extend, wide, &[y]));
            let product = self.dag.get_node(ISD::Mul, wide, &[x, y]);
            let amount = self.shift_amount(bits);
            let high = self.dag.get_node(ISD::Srl, wide, &[product, amount]);
            return Some(self.dag.get_node(ISD::Truncate, vt, &[high]));
        }
        None
    }

    fn is_signed_min(
        value: u128,
        bits: u32,
    ) -> bool {
        value == 1u128 << (bits - 1)
    }

    /// `visitSDIV`.
    pub(super) fn visit_sdiv(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        let bits = vt.size_in_bits();
        if self.dag.constant(n0).is_some() && self.dag.constant(n1).is_some() {
            return Some(self.dag.get_node(ISD::SDiv, vt, &[n0, n1]));
        }
        let c1 = self.dag.constant(n1);
        if c1 == Some(super::fold::mask(bits)) {
            return Some(self.dag.get_negative(n0, vt));
        }
        if c1.is_some_and(|c| Self::is_signed_min(c, bits)) {
            let test = self.dag.get_setcc(EVT::I8, n0, n1, CondCode::SetEq);
            let (one, zero) = (self.dag.get_constant(1, vt), self.dag.get_constant(0, vt));
            return Some(self.dag.get_select(vt, test, one, zero));
        }
        if self.sign_bit_is_zero(n1) && self.sign_bit_is_zero(n0) {
            return Some(self.dag.get_node(ISD::UDiv, vt, &[n0, n1]));
        }
        self.visit_sdiv_like(n0, n1, node)
    }

    fn sign_bit_is_zero(
        &self,
        value: SDValue,
    ) -> bool {
        let bits = self.dag.value_type(value).size_in_bits();
        super::known_bits::known_bits(self.dag, value).zero >> (bits - 1) & 1 == 1
    }

    /// `isIntDivCheap(VT)`. DEVIATE SD206: never for `i8`, whose `DIV8r` the
    /// selector does not translate (SD302): the divide would be unselectable.
    fn int_div_cheap(
        &self,
        vt: EVT,
    ) -> bool {
        self.dag.config().int_div_cheap && vt != EVT::I8
    }

    /// `visitSDIVLike`.
    pub(super) fn visit_sdiv_like(
        &mut self,
        n0: SDValue,
        n1: SDValue,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        let bits = vt.size_in_bits();
        let exact = self.dag.node(node).flags.0 & SDNodeFlags::EXACT.0 != 0;
        let divisor = self.dag.constant(n1).filter(|_| !self.dag.is_opaque(n1));
        if let Some(c) = divisor.filter(|&c| c != 0 && !exact) {
            let negated = c.wrapping_neg() & super::fold::mask(bits);
            if c.is_power_of_two() || negated.is_power_of_two() {
                // `Sign = x >> (bits-1)`, add `Sign >>> (bits - log2 |d|)`,
                // shift right by `log2 |d|`; a negative divisor
                // negates.
                let log = c.trailing_zeros();
                let (top, inexact, by) =
                    (self.shift_amount(bits - 1), self.shift_amount(bits - log), self.shift_amount(log));
                let sign = self.dag.get_node(ISD::Sra, vt, &[n0, top]);
                let srl = self.dag.get_node(ISD::Srl, vt, &[sign, inexact]);
                let sum = self.dag.get_node(ISD::Add, vt, &[n0, srl]);
                let quotient = if c == 1 || negated == 1 { n0 } else { self.dag.get_node(ISD::Sra, vt, &[sum, by]) };
                let negative = c >> (bits - 1) & 1 == 1;
                return Some(if negative { self.dag.get_negative(quotient, vt) } else { quotient });
            }
        }
        if self.dag.constant(n1).is_some() && !self.int_div_cheap(vt) {
            return self.build_sdiv(n0, n1, node);
        }
        None
    }

    /// `TargetLowering::BuildSDIV`, for a type the target holds.
    fn build_sdiv(
        &mut self,
        n0: SDValue,
        n1: SDValue,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        let bits = vt.size_in_bits();
        if self.dag.node(node).flags.0 & SDNodeFlags::EXACT.0 != 0 || !self.tli?.types.is_type_legal(vt) {
            return None;
        }
        let mask = super::fold::mask(bits);
        let d = self.dag.constant(n1)? & mask;
        if d == 0 {
            return None;
        }
        let (magic, factor, shift, sign_mask) = if d == 1 || d == mask {
            (0, d, 0, 0)
        } else {
            let found = signed_magic(d, bits);
            let top = 1u128 << (bits - 1);
            let factor = match (d & top != 0, found.magic & top != 0) {
                (false, true) => 1,
                (true, false) => mask,
                _ => 0,
            };
            (found.magic, factor, found.shift, mask)
        };
        let magic = self.dag.get_constant(magic, vt);
        let high = self.multiply_high(true, vt, n0, magic)?;
        let factor = self.dag.get_constant(factor, vt);
        let scaled = self.dag.get_node(ISD::Mul, vt, &[n0, factor]);
        let sum = self.dag.get_node(ISD::Add, vt, &[high, scaled]);
        let by = self.shift_amount(shift);
        let quotient = self.dag.get_node(ISD::Sra, vt, &[sum, by]);
        let top = self.shift_amount(bits - 1);
        let sign = self.dag.get_node(ISD::Srl, vt, &[quotient, top]);
        let sign_mask = self.dag.get_constant(sign_mask, vt);
        let sign = self.dag.get_node(ISD::And, vt, &[sign, sign_mask]);
        Some(self.dag.get_node(ISD::Add, vt, &[quotient, sign]))
    }

    /// `visitUDIV`.
    pub(super) fn visit_udiv(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        if self.dag.constant(n0).is_some() && self.dag.constant(n1).is_some() {
            return Some(self.dag.get_node(ISD::UDiv, vt, &[n0, n1]));
        }
        if self.dag.constant(n1) == Some(super::fold::mask(vt.size_in_bits())) {
            let test = self.dag.get_setcc(EVT::I8, n0, n1, CondCode::SetEq);
            let (one, zero) = (self.dag.get_constant(1, vt), self.dag.get_constant(0, vt));
            return Some(self.dag.get_select(vt, test, one, zero));
        }
        self.visit_udiv_like(n0, n1, node)
    }

    /// `visitUDIVLike`.
    pub(super) fn visit_udiv_like(
        &mut self,
        n0: SDValue,
        n1: SDValue,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        if let Some(c) = self.dag.constant(n1).filter(|_| !self.dag.is_opaque(n1)) {
            if c.is_power_of_two() {
                let by = self.shift_amount(c.trailing_zeros());
                return Some(self.dag.get_node(ISD::Srl, vt, &[n0, by]));
            }
        }
        // `udiv x, (shl c, y)` with a power of two `c`: `x >>> (log2 c + y)`.
        if self.dag.opcode(n1) == ISD::Shl {
            let (c, y) = (self.dag.operand(n1, 0), self.dag.operand(n1, 1));
            if let Some(c) = self.dag.constant(c).filter(|&c| c.is_power_of_two() && !self.dag.is_opaque(n1)) {
                let amount_vt = self.dag.value_type(y);
                let log = self.dag.get_constant(u128::from(c.trailing_zeros()), amount_vt);
                let total = self.dag.get_node(ISD::Add, amount_vt, &[y, log]);
                return Some(self.dag.get_node(ISD::Srl, vt, &[n0, total]));
            }
        }
        if self.dag.constant(n1).is_some() && !self.int_div_cheap(vt) {
            return self.build_udiv(n0, n1, node);
        }
        None
    }

    /// `TargetLowering::BuildUDIV`, for a type the target holds. The pin's
    /// widened `i32` multiply needs a legal `i64`, which this target has not.
    fn build_udiv(
        &mut self,
        n0: SDValue,
        n1: SDValue,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        let bits = vt.size_in_bits();
        if self.dag.node(node).flags.0 & SDNodeFlags::EXACT.0 != 0 || !self.tli?.types.is_type_legal(vt) {
            return None;
        }
        let d = self.dag.constant(n1)? & super::fold::mask(bits);
        if d <= 1 {
            return None;
        }
        let known = super::known_bits::known_bits(self.dag, n0);
        let known_zeros = (0..bits).rev().take_while(|&bit| known.zero >> bit & 1 == 1).count() as u32;
        let leading = known_zeros.min(bits - (128 - d.leading_zeros()));
        let found = unsigned_magic(d, bits, leading, true, false);
        let mut quotient = n0;
        if found.pre_shift != 0 {
            let by = self.shift_amount(found.pre_shift);
            quotient = self.dag.get_node(ISD::Srl, vt, &[quotient, by]);
        }
        let magic = self.dag.get_constant(found.magic, vt);
        quotient = self.multiply_high(false, vt, quotient, magic)?;
        if found.is_add {
            let difference = self.dag.get_node(ISD::Sub, vt, &[n0, quotient]);
            let one = self.shift_amount(1);
            let half = self.dag.get_node(ISD::Srl, vt, &[difference, one]);
            quotient = self.dag.get_node(ISD::Add, vt, &[half, quotient]);
        }
        if found.post_shift != 0 {
            let by = self.shift_amount(found.post_shift);
            quotient = self.dag.get_node(ISD::Srl, vt, &[quotient, by]);
        }
        Some(quotient)
    }

    /// `visitREM`: `x % c` is `x - x / c * c` when the quotient has a cheap
    /// form.
    pub(super) fn visit_rem(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let op = self.dag.opcode(node);
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        let signed = op == ISD::SRem;
        if self.dag.constant(n0).is_some() && self.dag.constant(n1).is_some() {
            return Some(self.dag.get_node(op, vt, &[n0, n1]));
        }
        if signed {
            if self.sign_bit_is_zero(n1) && self.sign_bit_is_zero(n0) {
                return Some(self.dag.get_node(ISD::URem, vt, &[n0, n1]));
            }
        } else if self.dag.constant(n1).is_some_and(|c| c.is_power_of_two()) {
            let ones = self.dag.get_all_ones_constant(vt);
            let below = self.dag.get_node(ISD::Add, vt, &[n1, ones]);
            return Some(self.dag.get_node(ISD::And, vt, &[n0, below]));
        }
        if self.dag.constant(n1).is_none_or(|c| c == 0) {
            return None;
        }
        // "Checking cheapness here makes sense since the simplification
        // results in fatter code."
        if self.int_div_cheap(vt) {
            return None;
        }
        let quotient = if signed { self.visit_sdiv_like(n0, n1, node) } else { self.visit_udiv_like(n0, n1, node) }?;
        let product = self.dag.get_node(ISD::Mul, vt, &[quotient, n1]);
        Some(self.dag.get_node(ISD::Sub, vt, &[n0, product]))
    }
}
