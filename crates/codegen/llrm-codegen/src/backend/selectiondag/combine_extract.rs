//! `visitEXTRACT_VECTOR_ELT`
//! ([DAGCombiner.cpp#L25912](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L25912)),
//! and `TargetLowering::scalarizeExtractedVectorLoad`
//! ([TargetLowering.cpp#L14264](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L14264)):
//! the lane of a vector the DAG already knows is that value, and a lane of a
//! loaded vector is a scalar load of the lane's bytes.
//!
//! # How it works
//! An extract of undef is undef, one of an out-of-range lane is poison, of a
//! `SCALAR_TO_VECTOR` its scalar (poison for a lane other than 0), of an
//! `INSERT_VECTOR_ELT` at the same lane the inserted value, of a
//! `BUILD_VECTOR` the operand. After the vector operations are legal, a
//! constant lane of a normal load becomes a load at the lane's address, the
//! old load's chain joined to the new one's.
//!
//! DEVIATE EV1 (owner llrm-arm64-vec, exit 2026-11-27): the pin's `BITCAST`,
//! `VECTOR_SHUFFLE`, `CONCAT_VECTORS` and `FREEZE` sources, the variable
//! lane, the demanded-elements simplification, `scalarizeExtractedBinOp`, the
//! known-constant `fp` lane and the pairwise forms are not made.
//!
//! DEVIATE EV2 (owner llrm-arm64-vec, exit 2026-11-27): the pin's source asks
//! that the extract be the load's only use (`hasNUsesOfValue(1, 0)`), yet its
//! binary scalarizes a load read by two or three lane extracts (probed with
//! `llc` on `<2 x i64>` and `<4 x i32>`), and keeps the vector load when one
//! other node reads it. This port scalarizes when every reader is a
//! constant-lane extract. The cause of the difference is not located.
//!
//! # Used by
//! `DagCombiner::visit`.

use super::actions::LoadExt;
use super::dag::{MemBase, MemInfo, NodeData, SDValue};
use super::dag_combiner::{CombineHooks, DagCombiner};
use super::isd::ISD;
use super::mvt::EVT;

impl<H: CombineHooks> DagCombiner<'_, H> {
    pub(super) fn visit_extract_vector_elt(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (vec, index) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vec_vt = self.dag.value_type(vec);
        // The same node is the pair extract (`EXTRACT_ELEMENT`) of an integer.
        if !vec_vt.is_vector() {
            return None;
        }
        let scalar_vt = self.dag.value_type(node);
        if self.dag.is_undef(vec) {
            return Some(self.dag.get_undef(scalar_vt));
        }
        let lane = self.dag.constant(index);
        let opcode = self.dag.opcode(vec);
        // "extract_vector_elt (insert_vector_elt vec, val, idx), idx) -> val"
        if opcode == ISD::InsertVectorElt && index == self.dag.operand(vec, 2) {
            let value = self.dag.operand(vec, 1);
            return Some(if vec_vt.element().is_integer() {
                self.dag.get_any_ext_or_trunc(value, scalar_vt)
            } else {
                value
            });
        }
        // "(vextract (scalar_to_vector val, 0) -> val"
        if opcode == ISD::ScalarToVector {
            if lane.is_some_and(|at| at != 0) {
                return Some(self.dag.get_poison(scalar_vt));
            }
            let value = self.dag.operand(vec, 0);
            let from = self.dag.value_type(value);
            if from == scalar_vt {
                return Some(value);
            }
            if from.is_integer() && scalar_vt.is_integer() {
                return Some(self.dag.get_any_ext_or_trunc(value, scalar_vt));
            }
            return None;
        }
        let lane = lane?;
        // "extract_vector_elt of out-of-bounds element -> POISON"
        if lane >= u128::from(vec_vt.lanes()) {
            return Some(self.dag.get_poison(scalar_vt));
        }
        // "extract_vector_elt (build_vector x, y), 1 -> y"
        if opcode == ISD::BuildVector && self.tli.is_some_and(|tli| tli.types.is_type_legal(vec_vt)) {
            let element = self.dag.operand(vec, lane as usize);
            if (self.dag.has_one_use(vec.node) || self.dag.is_null_constant(element))
                && self.dag.value_type(element) == scalar_vt
            {
                return Some(element);
            }
        }
        // "Perform only after legalization to ensure build_vector /
        // vector_shuffle optimizations have already been done."
        if !self.legal_operations() {
            return None;
        }
        self.scalarize_extracted_vector_load(node, vec, lane as u32)
    }

    /// `(vextract (v4f32 load $addr), c) -> (f32 load $addr+c*size)`.
    fn scalarize_extracted_vector_load(
        &mut self,
        node: SDValue,
        vec: SDValue,
        lane: u32,
    ) -> Option<SDValue> {
        let scalar_vt = self.dag.value_type(node);
        let vec_vt = self.dag.value_type(vec);
        let element_vt = vec_vt.element();
        // "If the result of load has to be truncated, then it's not
        // necessarily profitable."
        if scalar_vt.bits_lt(element_vt) && !self.hooks.is_truncate_free(element_vt, scalar_vt) {
            return None;
        }
        if self.dag.opcode(vec) != ISD::Load || vec.res != 0 {
            return None;
        }
        let NodeData::Mem(info) = self.dag.node(vec).data else { return None };
        // "isSimple" and "isNormalLoad".
        if info.volatile || info.ordering.is_some() || info.ext.is_some() {
            return None;
        }
        // DEVIATE EV2: every reader is a constant-lane extract of the load.
        let readers_are_lanes = self
            .dag
            .users(vec)
            .all(
                |user| self.dag.opcode(SDValue { node: user, res: 0 }) == ISD::ExtractElement
                    && self.dag.operand(SDValue { node: user, res: 0 }, 0) == vec
                    && self.dag.constant(self.dag.operand(SDValue { node: user, res: 0 }, 1)).is_some(),
            );
        if !readers_are_lanes {
            return None;
        }
        // "If the vector element type is not a multiple of a byte then we are
        // unable to correctly compute an address".
        let element_bits = element_vt.size_in_bits();
        if element_bits % 8 != 0 {
            return None;
        }
        let EVT::Simple(_) = element_vt else { return None };
        if !self.tli.is_some_and(|tli| tli.is_operation_legal_or_custom(ISD::Load, element_vt)) {
            return None;
        }
        let bytes = element_bits / 8;
        let offset = bytes * lane;
        // `commonAlignment(Alignment, ByteOffset)`
        let align = info.align.min(1 << offset.trailing_zeros().min(31));
        let (chain, ptr) = (self.dag.operand(vec, 0), self.dag.operand(vec, 1));
        let address = self.dag.get_object_ptr_offset(ptr, i64::from(offset));
        let base = match info.base {
            MemBase::Stack(at) => MemBase::Stack(at + offset as i32),
            MemBase::CallArgument(at) => MemBase::CallArgument(at + offset as i32),
            // the recorded location is of the whole vector
            MemBase::Value(_) => MemBase::Unknown,
            other => other,
        };
        let access = MemInfo { mem_vt: element_vt, align, base, ..info };
        let load = if scalar_vt.bits_gt(element_vt) {
            // "If the result type of vextract is wider than the load, then
            // issue an extending load instead."
            let zero_extends = matches!(
                (scalar_vt, element_vt),
                (EVT::Simple(wide), EVT::Simple(narrow))
                    if self.tli.is_some_and(|tli| tli.load_ext_action(LoadExt::ZExtLoad, wide, narrow) == super::actions::Action::Legal)
            );
            let ext = if zero_extends { LoadExt::ZExtLoad } else { LoadExt::ExtLoad };
            self.dag.get_ext_load(ext, scalar_vt, chain, address, access)
        } else {
            self.dag.get_load(element_vt, chain, address, access)
        };
        self.make_equivalent_memory_ordering(SDValue { node: vec.node, res: 1 }, SDValue { node: load.node, res: 1 });
        Some(if scalar_vt.bits_lt(element_vt) {
            self.dag.get_node(ISD::Truncate, scalar_vt, &[load])
        } else if scalar_vt == element_vt {
            load
        } else {
            load
        })
    }
}
