//! `WidenVSELECTMask`: a `VSELECT` whose mask is a vector of `i1` (no
//! register holds one) takes the mask at the compare's own lane width, so the
//! compare is not made at `v4i32` and truncated to `v4i16` to be extended
//! again.
//!
//! Translated from `WidenVSELECTMask`, `convertMaskTree`,
//! `convertMaskTreeImpl`, `convertMask`, `adjustMaskToType` and
//! `unifyMaskTypes` of
//! [LegalizeVectorTypes.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeVectorTypes.cpp#L7408).
//!
//! DEVIATE VM1 (owner llrm-arm64-vec, exit 2026-11-27): a scalable vector, a
//! widened (`TypeWidenVector`) select and strict compares do not exist here.

use llrm_sdag::lowering::TypeAction;

use super::{SDValue, TypeLegalizer};
use crate::backend::selectiondag::isd::ISD;
use crate::backend::selectiondag::mvt::{EVT, MVT};

/// `SelectionDAG::MaxRecursionDepth`.
const MAX_RECURSION_DEPTH: u32 = 6;

/// `EVT::getVectorVT`: the vector of `lanes` lanes of `element`, if it is a
/// type.
fn vector_of(
    element: EVT,
    lanes: u32,
) -> Option<EVT> {
    let EVT::Simple(lane) = element else { return None };
    MVT::vector(lane, lanes).map(EVT::Simple)
}

fn is_logical_mask_op(op: ISD) -> bool {
    matches!(op, ISD::And | ISD::Or | ISD::Xor)
}

impl TypeLegalizer<'_> {
    /// The type `vt` becomes until it is legal (`getTypeToTransformTo` in a
    /// loop).
    fn legal_form(
        &self,
        mut vt: EVT,
    ) -> EVT {
        while self.action(vt) != TypeAction::Legal {
            vt = self.tli.types.type_conversion(vt).1;
        }
        vt
    }

    /// `WidenVSELECTMask`: the mask of a `VSELECT` as an integer vector of the
    /// select's own lane width, or `None` where the generic promotion is to
    /// be used.
    pub(super) fn widen_vselect_mask(
        &mut self,
        select: SDValue,
    ) -> Option<SDValue> {
        if self.dag.opcode(select) != ISD::VSelect {
            return None;
        }
        let cond = self.dag.operand(select, 0);
        let cond_vt = self.dag.value_type(cond);
        // A split `VSELECT` handled before has the lane width already.
        if cond_vt.element().size_in_bits() != 1 {
            return None;
        }
        let select_vt = self.dag.value_type(select);
        if !select_vt.size_in_bits().is_power_of_two() {
            return None;
        }
        // Not where the select will be scalarized.
        let mut final_vt = select_vt;
        while self.action(final_vt) == TypeAction::SplitVector {
            final_vt = vector_of(final_vt.element(), final_vt.lanes() / 2)?;
        }
        // Where an `i1` vector mask is a register type, leave it.
        if self.dag.opcode(cond) == ISD::SetCC {
            let operand_vt = self.legal_form(self.dag.value_type(self.dag.operand(cond, 0)));
            if self.tli.types.set_cc_result_type(operand_vt).element().size_in_bits() == 1 {
                return None;
            }
        } else if self.legal_form(cond_vt).element().size_in_bits() == 1 {
            return None;
        }
        let mut to_mask = select_vt;
        if !to_mask.element().is_integer() {
            to_mask = to_mask.change_vector_element_type_to_integer();
        }
        self.convert_mask_tree(cond, to_mask)
    }

    /// `convertMaskTree`: the tree at the lane width, then the lane count.
    fn convert_mask_tree(
        &mut self,
        value: SDValue,
        to_vt: EVT,
    ) -> Option<SDValue> {
        let tree_vt = vector_of(to_vt.element(), self.dag.value_type(value).lanes())?;
        let (result, _) = self.convert_mask_tree_impl(value, tree_vt, 0)?;
        Some(self.adjust_mask_to_type(result, to_vt))
    }

    /// `convertMask`: the compare made again with the result type `mask_vt`.
    fn convert_mask(
        &mut self,
        mask: SDValue,
        mask_vt: EVT,
        to_vt: EVT,
    ) -> SDValue {
        let ops = self.dag.node(mask).ops.clone();
        let flags = self.dag.node(mask).flags;
        let made = self.dag.get_node_flags(self.dag.opcode(mask), mask_vt, &ops, flags);
        self.adjust_mask_to_type(made, to_vt)
    }

    /// `adjustMaskToType`: the lane width by sign extension or truncation,
    /// the lane count by `EXTRACT_SUBVECTOR` or by `CONCAT_VECTORS` with
    /// poison.
    fn adjust_mask_to_type(
        &mut self,
        mut mask: SDValue,
        to_vt: EVT,
    ) -> SDValue {
        let mask_vt = self.dag.value_type(mask);
        let (bits, to_bits) = (mask_vt.element().size_in_bits(), to_vt.element().size_in_bits());
        if bits != to_bits {
            let resized = vector_of(to_vt.element(), mask_vt.lanes()).expect("a vector of the lane width");
            let opcode = if bits < to_bits { ISD::SignExtend } else { ISD::Truncate };
            mask = self.dag.get_node(opcode, resized, &[mask]);
        }
        let lanes = self.dag.value_type(mask).lanes();
        if lanes > to_vt.lanes() {
            let zero = self.dag.get_constant(0, EVT::I64);
            mask = self.dag.get_node(ISD::ExtractSubvector, to_vt, &[mask, zero]);
        } else if lanes < to_vt.lanes() {
            let sub_vt = self.dag.value_type(mask);
            let poison = self.dag.get_poison(sub_vt);
            let mut parts = vec![poison; (to_vt.lanes() / lanes) as usize];
            parts[0] = mask;
            mask = self.dag.get_node(ISD::ConcatVectors, to_vt, &parts);
        }
        mask
    }

    /// `unifyMaskTypes`: both operands at a common lane width, the one that
    /// costs the fewest extensions and truncations.
    fn unify_mask_types(
        &mut self,
        op0: &mut SDValue,
        lenient0: bool,
        op1: &mut SDValue,
        lenient1: bool,
        to_vt: EVT,
    ) -> EVT {
        if lenient0 != lenient1 {
            // The lenient operand follows the other's type; the cast folds
            // away.
            let (lenient, fixed) = if lenient0 { (op0, op1) } else { (op1, op0) };
            let vt = self.dag.value_type(*fixed);
            *lenient = self.adjust_mask_to_type(*lenient, vt);
            return vt;
        }
        let bits0 = self.dag.value_type(*op0).element().size_in_bits();
        let bits1 = self.dag.value_type(*op1).element().size_in_bits();
        let (narrow, wide) = (bits0.min(bits1), bits0.max(bits1));
        let to_bits = to_vt.element().size_in_bits();
        let int_bits = if narrow == wide {
            narrow
        } else if to_bits >= wide {
            wide
        } else if to_bits <= narrow {
            narrow
        } else {
            to_bits
        };
        let lanes = self.dag.value_type(*op0).lanes();
        let vt = vector_of(EVT::integer(int_bits), lanes).expect("a mask lane width has a vector type");
        *op0 = self.adjust_mask_to_type(*op0, vt);
        *op1 = self.adjust_mask_to_type(*op1, vt);
        vt
    }

    /// `convertMaskTreeImpl`: the value and whether its type is lenient (an
    /// all-zeros or all-ones constant, the same at every width).
    fn convert_mask_tree_impl(
        &mut self,
        value: SDValue,
        to_vt: EVT,
        depth: u32,
    ) -> Option<(SDValue, bool)> {
        if depth >= MAX_RECURSION_DEPTH || self.dag.value_type(value).lanes() != to_vt.lanes() {
            return None;
        }
        let op = self.dag.opcode(value);
        // A compare makes the mask at its natural type.
        if op == ISD::SetCC {
            let mask_vt = self.tli.types.set_cc_result_type(self.dag.value_type(self.dag.operand(value, 0)));
            return Some((self.convert_mask(value, mask_vt, mask_vt), false));
        }
        if self.dag.is_build_vector_all(value, false) {
            return Some((self.dag.get_constant(0, to_vt), true));
        }
        if self.dag.is_build_vector_all(value, true) {
            return Some((self.dag.get_all_ones_constant(to_vt), true));
        }
        if is_logical_mask_op(op) {
            let (mut lhs, lenient0) = self.convert_mask_tree_impl(self.dag.operand(value, 0), to_vt, depth + 1)?;
            let (mut rhs, lenient1) = self.convert_mask_tree_impl(self.dag.operand(value, 1), to_vt, depth + 1)?;
            let vt = self.unify_mask_types(&mut lhs, lenient0, &mut rhs, lenient1, to_vt);
            return Some((self.dag.get_node(op, vt, &[lhs, rhs]), lenient0 && lenient1));
        }
        if op == ISD::Freeze {
            let (inner, lenient) = self.convert_mask_tree_impl(self.dag.operand(value, 0), to_vt, depth + 1)?;
            let vt = self.dag.value_type(inner);
            return Some((self.dag.get_node(ISD::Freeze, vt, &[inner]), lenient));
        }
        if op == ISD::VectorShuffle {
            let mask = self.dag.shuffle_mask(value).to_vec();
            let (mut first, lenient0) = self.convert_mask_tree_impl(self.dag.operand(value, 0), to_vt, depth + 1)?;
            let second = self.dag.operand(value, 1);
            if self.dag.is_undef(second) {
                let vt = self.dag.value_type(first);
                let undef = self.dag.get_undef(vt);
                return Some((self.dag.get_vector_shuffle(vt, first, undef, &mask), lenient0));
            }
            let (mut second, lenient1) = self.convert_mask_tree_impl(second, to_vt, depth + 1)?;
            let vt = self.unify_mask_types(&mut first, lenient0, &mut second, lenient1, to_vt);
            return Some((self.dag.get_vector_shuffle(vt, first, second, &mask), lenient0 && lenient1));
        }
        if matches!(op, ISD::Select | ISD::VSelect) {
            let (mut lhs, lenient0) = self.convert_mask_tree_impl(self.dag.operand(value, 1), to_vt, depth + 1)?;
            let (mut rhs, lenient1) = self.convert_mask_tree_impl(self.dag.operand(value, 2), to_vt, depth + 1)?;
            let vt = self.unify_mask_types(&mut lhs, lenient0, &mut rhs, lenient1, to_vt);
            // The condition is widened by its own traversal, if at all.
            let cond = self.dag.operand(value, 0);
            return Some((self.dag.get_node(op, vt, &[cond, lhs, rhs]), lenient0 && lenient1));
        }
        None
    }
}
