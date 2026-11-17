//! `CombineExtLoad`: an extension of a vector load the type cannot hold is
//! loads of the pieces it can.
//!
//! `DAGCombiner::CombineExtLoad`
//! ([DAGCombiner.cpp#L15440](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L15440)),
//! called by `visitZERO_EXTEND` and `visitSIGN_EXTEND` after
//! `tryToFoldExtOfLoad` declines.
//!
//! # How it works
//! `(zext (load x))` of a vector whose extending load is neither legal nor
//! custom is halved until the extending load is, as many loads as pieces,
//! concatenated. The target says whether it wants this
//! (`isVectorLoadExtDesirable`; the pin's default is no). When the
//! type is already one the target loads, the result is one extending load,
//! which the target lowers by hand (arm64's `v4i8` to `v4i32`: `ldr s;
//! ushll; ushll`).
//!
//! ```text
//! v4i32 (zext (v4i8 (load p)))  ->  v4i32 (zextload<v4i8> p)
//! ```
//!
//! DEVIATE EXL1 (owner w11, exit 2026-11-27): a load other than the
//! extension's has no users to rewrite (only a one-use load is combined, as
//! the pin's, so `ExtendSetCCUses` has nothing to do); the pieces of a split
//! share the load's pointer info (`MemBase` has no offset).
//!
//! # Used by
//! `visit_extend`.

use super::actions::{Action, LoadExt};
use super::dag::{MemInfo, NodeData, SDNodeFlags, SDValue};
use super::dag_combiner::{CombineHooks, DagCombiner};
use super::isd::ISD;
use super::mvt::{EVT, MVT};

/// `GetSplitDestVTs`: the type of each half.
fn half(vt: EVT) -> Option<EVT> {
    let EVT::Simple(mvt) = vt else { return None };
    Some(EVT::Simple(MVT::vector(mvt.element()?, mvt.lanes()? / 2)?))
}

impl<H: CombineHooks> DagCombiner<'_, H> {
    pub(super) fn combine_ext_load(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (dst, load) = (self.dag.value_type(node), self.dag.operand(node, 0));
        let (ext, extend) = match self.dag.opcode(node) {
            ISD::ZeroExtend => (LoadExt::ZExtLoad, ISD::ZeroExtend),
            ISD::SignExtend => (LoadExt::SExtLoad, ISD::SignExtend),
            _ => return None,
        };
        let _ = extend;
        if self.dag.opcode(load) != ISD::Load {
            return None;
        }
        let NodeData::Mem(info) = self.dag.node(load).data else { return None };
        // "isNON_EXTLoad, isUNINDEXEDLoad, hasOneUse, isSimple".
        if info.ext.is_some()
            || info.volatile
            || info.ordering.is_some()
            || self.dag.users(load).count() != 1
            || !dst.is_vector()
            || !dst.lanes().is_power_of_two()
            || !self.hooks.vector_load_ext_desirable(self.dag, node)
        {
            return None;
        }
        let tli = self.tli?;
        // "Try to split the vector types to get down to legal types."
        let (mut split_src, mut split_dst) = (info.mem_vt, dst);
        let usable = |src: EVT, dst: EVT| match (dst, src) {
            (EVT::Simple(wide), EVT::Simple(mem)) => {
                matches!(
                    tli.load_ext_action(ext, wide, mem),
                    Action::Legal | Action::Custom
                )
            }
            _ => false,
        };
        while !usable(split_src, split_dst) && split_src.lanes() > 1 {
            split_dst = half(split_dst)?;
            split_src = half(split_src)?;
        }
        if !usable(split_src, split_dst) {
            return None;
        }
        let splits = dst.lanes() / split_dst.lanes();
        let stride = u128::from(split_src.size_in_bits() / 8);
        let (chain, mut ptr) = (self.dag.operand(load, 0), self.dag.operand(load, 1));
        let (mut loads, mut chains) = (Vec::new(), Vec::new());
        for _ in 0..splits {
            let piece =
                self.dag.get_ext_load(ext, split_dst, chain, ptr, MemInfo { mem_vt: split_src, ext: None, ..info });
            let step = self.dag.get_constant(stride, self.dag.value_type(ptr));
            ptr = self.dag.get_mem_base_plus_offset(ptr, step, SDNodeFlags::default());
            loads.push(piece);
            chains.push(SDValue { node: piece.node, res: 1 });
        }
        let value = if loads.len() == 1 { loads[0] } else { self.dag.get_node(ISD::ConcatVectors, dst, &loads) };
        let new_chain =
            if chains.len() == 1 { chains[0] } else { self.dag.get_node(ISD::TokenFactor, EVT::OTHER, &chains) };
        self.replace_value(SDValue { node: load.node, res: 1 }, new_chain);
        Some(value)
    }
}
