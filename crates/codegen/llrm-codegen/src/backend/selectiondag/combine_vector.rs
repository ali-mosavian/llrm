//! `visitINSERT_VECTOR_ELT`
//! ([DAGCombiner.cpp#L25349](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L25349)):
//! an insert past the end is poison, an insert of poison is the vector, and a
//! chain of inserts into undef, a `BUILD_VECTOR` or a `SCALAR_TO_VECTOR` is a
//! `BUILD_VECTOR`.
//!
//! # How it works
//! The chain is walked up from the insert while each link has one user and a
//! constant index; the elements found fill a list of lanes, the first one
//! seen for a lane winning (it is the latest insert). The walk ends at undef
//! (the lanes left over are undefined), a `BUILD_VECTOR` or `SCALAR_TO_VECTOR`
//! (they supply the rest), and every lane is then widened to the widest
//! element (`getAnyExtOrTrunc`).
//!
//! DEVIATE VEC2: `mergeInsertEltWithShuffle`, `combineInsertEltToShuffle`,
//! `combineInsertEltToLoad`, the `VECTOR_SHUFFLE` and `FREEZE(UNDEF)` links of
//! the chain, the and/or-mask form, the variable-index forms and the
//! one-element vector are not made. Exit: the vector shuffle combines.
//!
//! `visitVECTOR_SHUFFLE`
//! ([DAGCombiner.cpp#L29412](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L29412)):
//! the canonical forms (a shuffle of undefs, of one vector twice, of an undef
//! first operand, with references to an undef second operand) and a splat of
//! a `SCALAR_TO_VECTOR` or of an `INSERT_VECTOR_ELT` into the lane is a
//! `BUILD_VECTOR` of the scalar. DEVIATE VEC7 (owner llrm-arm64-vec, exit
//! 2026-11-27): the commuted undef first operand, `replaceShuffleOfInsert`,
//! `combineShuffleOfSplatVal`, `formSplatFromShuffles`, the splat of a binary
//! operation, of a bitcast, and the rest of the function are not translated.
//!
//! # Used by
//! `DagCombiner::visit`.

use super::dag::SDValue;
use super::dag_combiner::{CombineHooks, DagCombiner};
use super::isd::ISD;

impl<H: CombineHooks> DagCombiner<'_, H> {
    pub(super) fn visit_insert_vector_elt(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (in_vec, in_val, elt_no) =
            (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
        let vt = self.dag.value_type(node);
        let lanes = vt.lanes() as usize;
        let index = self.dag.constant(elt_no);
        // "Insert into out-of-bounds element is poison."
        if index.is_some_and(|at| at >= lanes as u128) {
            return Some(self.dag.get_poison(vt));
        }
        // "Remove insert of UNDEF/POISON elements."
        if self.dag.is_undef(in_val) {
            return (self.dag.opcode(in_val) == ISD::Poison || self.dag.opcode(in_vec) == ISD::Undef).then_some(in_vec);
        }
        let at = index? as usize;
        // "Attempt to convert an insert_vector_elt chain into a legal
        // build_vector."
        let build_vector_legal = self.tli.is_some_and(|tli| tli.is_operation_legal(ISD::BuildVector, vt));
        if self.legal_operations() && !build_vector_legal {
            return None;
        }
        let mut ops: Vec<Option<SDValue>> = vec![None; lanes];
        ops[at] = Some(in_val);
        let mut current = in_vec;
        let finish = |this: &mut Self, ops: Vec<Option<SDValue>>| -> SDValue {
            // `CanonicalizeBuildVector`: every lane at the widest element.
            let widest = ops
                .iter()
                .flatten()
                .map(|&op| this.dag.value_type(op))
                .fold(this.dag.value_type(in_val), |a, b| if a.bits_lt(b) { b } else { a });
            let undef = this.dag.get_undef(widest);
            let lanes: Vec<SDValue> = ops
                .into_iter()
                .map(|op| match op {
                    Some(op) => this.dag.get_any_ext_or_trunc(op, widest),
                    None => undef,
                })
                .collect();
            this.dag.get_build_vector(vt, &lanes)
        };
        loop {
            // "UNDEF - build new BUILD_VECTOR from already inserted operands."
            if self.dag.is_undef(current) {
                return Some(finish(self, ops));
            }
            // "BUILD_VECTOR - insert unused operands and build new
            // BUILD_VECTOR."
            if self.dag.opcode(current) == ISD::BuildVector && self.dag.has_one_use(current.node) {
                for (lane, slot) in ops.iter_mut().enumerate() {
                    slot.get_or_insert_with(|| self.dag.operand(current, lane));
                }
                return Some(finish(self, ops));
            }
            // "SCALAR_TO_VECTOR - insert unused scalar and build new
            // BUILD_VECTOR."
            if self.dag.opcode(current) == ISD::ScalarToVector && self.dag.has_one_use(current.node) {
                ops[0].get_or_insert_with(|| self.dag.operand(current, 0));
                return Some(finish(self, ops));
            }
            // "INSERT_VECTOR_ELT - insert operand and continue up the chain."
            if self.dag.opcode(current) == ISD::InsertVectorElt && self.dag.has_one_use(current.node) {
                if let Some(lane) = self.dag.constant(self.dag.operand(current, 2)).filter(|&lane| lane < lanes as u128)
                {
                    ops[lane as usize].get_or_insert_with(|| self.dag.operand(current, 1));
                    // "Found entire BUILD_VECTOR."
                    if ops.iter().all(Option::is_some) {
                        return Some(finish(self, ops));
                    }
                    current = self.dag.operand(current, 0);
                    continue;
                }
            }
            return None;
        }
    }
}

impl<H: CombineHooks> DagCombiner<'_, H> {
    pub(super) fn visit_vector_shuffle(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        let lanes = vt.lanes() as i32;
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let mask = self.dag.shuffle_mask(node).to_vec();
        // "Canonicalize shuffle undef, undef -> undef"
        if self.dag.is_undef(n0) && self.dag.is_undef(n1) {
            return Some(self.dag.get_undef(vt));
        }
        // "Canonicalize shuffle v, v -> v, poison"
        if n0 == n1 {
            let unary: Vec<i32> = mask.iter().map(|&at| if at >= lanes { at - lanes } else { at }).collect();
            let poison = self.dag.get_poison(vt);
            return Some(self.dag.get_vector_shuffle(vt, n0, poison, &unary));
        }
        // "Remove references to rhs if it is undef"
        if self.dag.is_undef(n1) && mask.iter().any(|&at| at >= lanes) {
            let kept: Vec<i32> = mask.iter().map(|&at| if at >= lanes { -1 } else { at }).collect();
            return Some(self.dag.get_vector_shuffle(vt, n0, n1, &kept));
        }
        // "If it is a splat, check if the argument vector is another splat or
        // a build_vector."
        let lane = mask.iter().copied().find(|&at| at >= 0)?;
        if lane >= lanes || !mask.iter().all(|&at| at < 0 || at == lane) {
            return None;
        }
        let build_vector_legal = self.tli.is_some_and(|tli| tli.is_operation_legal(ISD::BuildVector, vt));
        if (!self.legal_operations() || build_vector_legal) && self.dag.has_one_use(n0.node) {
            // "splat(scalar_to_vector(x), 0) -> build_vector(x,...,x)"
            if self.dag.opcode(n0) == ISD::ScalarToVector && lane == 0 {
                let scalar = self.dag.operand(n0, 0);
                return Some(self.dag.get_splat_build_vector(vt, scalar));
            }
            // "splat(insert_vector_elt(v, x, c), c) -> build_vector(x,...,x)"
            if self.dag.opcode(n0) == ISD::InsertVectorElt
                && self.dag.constant(self.dag.operand(n0, 2)) == Some(lane as u128)
            {
                let scalar = self.dag.operand(n0, 1);
                return Some(self.dag.get_splat_build_vector(vt, scalar));
            }
        }
        None
    }
}
