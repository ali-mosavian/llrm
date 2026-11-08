//! The average of two values, `avgfloor[su]` and `avgceil[su]`, formed from a
//! shift of a sum by one.
//!
//! - [`DagCombiner::fold_shift_to_avg`]: `foldShiftToAvg`, `srl (add nuw x, y),
//!   1` is `avgflooru x, y` (`sra` and `nsw`: `avgfloors`) where the target has
//!   the node at the shift's own type.
//! - [`DagCombiner::combine_shift_to_avg`]: `combineShiftToAVG`, run by
//!   `SimplifyDemandedBits`: `srl (add (zext a), (zext b)), 1` is `zext
//!   (avgflooru a, b)` at the narrowest type the known bits allow, and the sum
//!   plus one is the `avgceil` forms.
//!
//! DEVIATE SD600: before types are legalized the pin forms the node at the
//! narrow type whether or not the target has it (the legalizer expands it with
//! `expandAVG`, which is not translated); here the node is formed only where
//! the target has it legal or custom, so a target without it keeps its
//! shift and add. Exit: `expandAVG` and the type-legalizer cases translated.
//! Owner llrm-6502-b, review 2026-12-10.
//!
//! DEVIATE SD601: `willNotOverflowAdd` (the wider-type fallback after types are
//! legal) reads the add's `nuw`/`nsw` flag, not the known-bits range proof.
//! Exit: with `willNotOverflowAdd`. Owner llrm-6502-b, review 2026-12-10.
//!
//! # See also
//! LLVM's `DAGCombiner::foldShiftToAvg`
//! ([pin](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L12872))
//! and `combineShiftToAVG`
//! ([pin](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L1054)).

use super::dag::{SDNodeFlags, SDValue};
use super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::fold::mask;
use super::isd::ISD;
use super::known_bits::{KnownBits, known_bits, num_sign_bits};
use super::mvt::EVT;

/// `KnownBits::countMinLeadingZeros`.
fn leading_zeros(known: &KnownBits) -> u32 {
    (!known.zero & mask(known.width)).leading_zeros() - (128 - known.width)
}

impl<H: CombineHooks> DagCombiner<'_, H> {
    /// `foldShiftToAvg`.
    pub(super) fn fold_shift_to_avg(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let op = self.dag.opcode(node);
        if !matches!(op, ISD::Sra | ISD::Srl) || self.dag.constant(self.dag.operand(node, 1)) != Some(1) {
            return None;
        }
        let sum = self.dag.operand(node, 0);
        let unsigned = op == ISD::Srl;
        let wrap = if unsigned { SDNodeFlags::NO_UNSIGNED_WRAP } else { SDNodeFlags::NO_SIGNED_WRAP };
        if self.dag.opcode(sum) != ISD::Add || !self.dag.node(sum).flags.has(wrap) {
            return None;
        }
        let vt = self.dag.value_type(node);
        let floor = if unsigned { ISD::AvgFloorU } else { ISD::AvgFloorS };
        if !self.has_avg_operation(floor, vt) {
            return None;
        }
        let (a, b) = (self.dag.operand(sum, 0), self.dag.operand(sum, 1));
        Some(self.dag.get_node(floor, vt, &[a, b]))
    }

    /// `hasOperation`: once operations are legalized only a legal one counts.
    fn has_avg_operation(
        &self,
        op: ISD,
        vt: EVT,
    ) -> bool {
        if self.legal_operations() { self.fp_op_legal(op, vt) } else { self.fp_op_legal_or_custom(op, vt) }
    }

    /// `MatchOperands`: `add_node` is an `add (x, y)` and `other` the sum's
    /// other operand; one of the two being 1 makes it the `avgceil` form
    /// `(x, y)` (the sum plus one) and `other`.
    fn match_avg_ceil(
        &self,
        add_node: SDValue,
        other: SDValue,
    ) -> Option<(SDValue, SDValue, SDValue)> {
        if self.dag.opcode(add_node) != ISD::Add {
            return None;
        }
        let (x, y) = (self.dag.operand(add_node, 0), self.dag.operand(add_node, 1));
        if self.dag.constant(y) == Some(1) {
            return Some((x, other, add_node));
        }
        if self.dag.constant(other) == Some(1) {
            return Some((x, y, add_node));
        }
        None
    }

    /// `combineShiftToAVG` for the right shift `op` with `demanded` bits read.
    pub(super) fn combine_shift_to_avg(
        &mut self,
        op: SDValue,
        demanded: u128,
    ) -> Option<SDValue> {
        let shift = self.dag.opcode(op);
        if self.dag.constant(self.dag.operand(op, 1)) != Some(1) {
            return None;
        }
        let sum = self.dag.operand(op, 0);
        if self.dag.opcode(sum) != ISD::Add {
            return None;
        }
        let (mut a, mut b) = (self.dag.operand(sum, 0), self.dag.operand(sum, 1));
        let mut add2 = None;
        let mut ceil = false;
        if let Some((x, y, inner)) = self.match_avg_ceil(a, b).or_else(|| self.match_avg_ceil(b, a)) {
            (a, b, add2, ceil) = (x, y, Some(inner), true);
        }
        let vt = self.dag.value_type(op);
        let width = vt.size_in_bits();
        let signs = num_sign_bits(self.dag, a).min(num_sign_bits(self.dag, b)) - 1;
        let zeros = leading_zeros(&known_bits(self.dag, a)).min(leading_zeros(&known_bits(self.dag, b)));
        let (signed, known) = match shift {
            ISD::Sra if zeros >= 2 && signs < zeros => (false, zeros),
            ISD::Sra if signs >= 1 => (true, signs),
            ISD::Srl if zeros >= 1 && signs < zeros => (false, zeros),
            ISD::Srl if signs >= 1 && demanded & (1u128 << (width - 1)) == 0 => (true, signs),
            _ => return None,
        };
        let avg = match (ceil, signed) {
            (true, true) => ISD::AvgCeilS,
            (true, false) => ISD::AvgCeilU,
            (false, true) => ISD::AvgFloorS,
            (false, false) => ISD::AvgFloorU,
        };
        // The narrowest power-of-two type, no narrower than a byte.
        let mut narrow = EVT::integer((width - known).max(8).next_power_of_two());
        if narrow.size_in_bits() > width {
            return None;
        }
        let legal_types = self.level >= CombineLevel::AfterLegalizeTypes;
        let tli = self.tli?;
        if legal_types && !tli.is_operation_legal(avg, narrow) {
            // "If we could not transform, and (both) adds are nuw/nsw, we can
            // use the larger type size."
            let wrap = if signed { SDNodeFlags::NO_SIGNED_WRAP } else { SDNodeFlags::NO_UNSIGNED_WRAP };
            let wraps = |dag: &super::dag::SelectionDAG, add: SDValue| dag.node(add).flags.has(wrap);
            if self.legal_operations() && !tli.is_operation_legal(avg, vt) {
                return None;
            }
            if wraps(self.dag, sum) && add2.is_none_or(|add| wraps(self.dag, add)) {
                narrow = vt;
            } else {
                return None;
            }
        }
        // DEVIATE SD600: only a node the target has
        if !self.has_avg_operation(avg, narrow) {
            return None;
        }
        // "Don't create a AVGFLOOR node with a scalar constant unless its legal
        // as this is likely to stop other folds."
        if !ceil
            && !tli.is_operation_legal(avg, narrow)
            && (self.dag.constant(a).is_some() || self.dag.constant(b).is_some())
        {
            return None;
        }
        let (x, y) = if signed {
            (self.dag.get_sext_or_trunc(a, narrow), self.dag.get_sext_or_trunc(b, narrow))
        } else {
            (self.dag.get_zext_or_trunc(a, narrow), self.dag.get_zext_or_trunc(b, narrow))
        };
        let result = self.dag.get_node(avg, narrow, &[x, y]);
        Some(if signed { self.dag.get_sext_or_trunc(result, vt) } else { self.dag.get_zext_or_trunc(result, vt) })
    }
}
