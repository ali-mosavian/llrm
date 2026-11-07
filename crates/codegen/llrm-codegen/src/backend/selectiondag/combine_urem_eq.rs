//! `buildUREMEqFold`: `(seteq|setne (urem n, d), c)` for a constant `d`
//! is `(setule|setugt (rotr (mul (sub n, c), p), k), q)`, with `d = d0 * 2^k`,
//! `p` the inverse of `d0` modulo `2^w` and `q = floor((2^w - 1) / d)`
//! (Hacker's Delight 10-17). No divide, and no multiply-high.
//!
//! Scalars only: the vector arms (per-lane tautologies, `vselect` fix-up) are
//! not ported. Not for a power-of-two divisor (a bit test is better), for a
//! tautological compare (`d <= c`, or `d == 1`), or where the target's divide
//! is cheap.
//!
//! # See also
//! LLVM's `TargetLowering::prepareUREMEqFold`
//! ([L7247](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L7247)),
//! called from `SimplifySetCC`
//! ([L5817](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L5817)).

use super::dag::SDValue;
use super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::isd::{CondCode, ISD};
use super::mvt::EVT;

/// `p` with `p * d0 = 1 (mod 2^width)`, `d0` odd (Newton's iteration).
fn inverse(
    d0: u128,
    width: u32,
) -> u128 {
    let mask = if width >= 128 { u128::MAX } else { (1u128 << width) - 1 };
    let mut p = d0;
    for _ in 0..7 {
        p = p.wrapping_mul(2u128.wrapping_sub(d0.wrapping_mul(p)));
    }
    p & mask
}

impl<H: CombineHooks> DagCombiner<'_, H> {
    /// `SimplifySetCC`'s "fold remainder of division by a constant" for the
    /// unsigned remainder, scalar; `compare` is the constant it is compared to.
    pub(super) fn build_urem_eq_fold(
        &mut self,
        setcc_vt: EVT,
        rem: SDValue,
        compare: u128,
        cond: CondCode,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(rem);
        let width = vt.size_in_bits();
        if !vt.is_integer() || width > 64 || !self.dag.has_one_use(rem.node) {
            return None;
        }
        // `!isIntDivCheap(VT) && !MinSize` (DEVIATE SD206: never for `i8`).
        if self.dag.config().int_div_cheap && vt != EVT::I8 {
            return None;
        }
        let (n, d) = (self.dag.operand(rem, 0), self.dag.constant(self.dag.operand(rem, 1))?);
        let mask = (1u128 << width) - 1;
        let (d, cmp) = (d & mask, compare & mask);
        // Division by zero is undefined; a tautological lane folds elsewhere;
        // a power of two is a bit test.
        if d == 0 || d == 1 || d <= cmp {
            return None;
        }
        let k = d.trailing_zeros();
        let d0 = d >> k;
        if d0 == 1 {
            return None;
        }
        let before_legalize_ops = self.level < CombineLevel::AfterLegalizeVectorOps;
        let legal = |this: &Self, op: ISD| {
            before_legalize_ops || this.tli.is_some_and(|t| t.is_operation_legal_or_custom(op, vt))
        };
        if !legal(self, ISD::Mul) || (cmp != 0 && !legal(self, ISD::Sub)) || (k != 0 && !legal(self, ISD::Rotr)) {
            return None;
        }
        let p = inverse(d0, width);
        let (mut q, r) = (mask / d, mask % d);
        if cmp > r {
            q -= 1;
        }
        let mut n = n;
        if cmp != 0 {
            let c = self.dag.get_constant(cmp, vt);
            n = self.dag.get_node(ISD::Sub, vt, &[n, c]);
        }
        let p = self.dag.get_constant(p, vt);
        let mut value = self.dag.get_node(ISD::Mul, vt, &[n, p]);
        self.add_to_worklist(value.node);
        if k != 0 {
            let amount = self.shift_amount(vt, k);
            value = self.dag.get_node(ISD::Rotr, vt, &[value, amount]);
            self.add_to_worklist(value.node);
        }
        let q = self.dag.get_constant(q, vt);
        Some(self.dag.get_setcc(
            setcc_vt,
            value,
            q,
            if cond == CondCode::SetEq { CondCode::SetULe } else { CondCode::SetUGt },
        ))
    }
}
