//! The rewrites `X86DAGToDAGISel` makes of the DAG before selection (the pin's
//! `PreprocessISelDAG`), for the ones that need no matcher.
//!
//! # How it works
//! Without SSE a `float` or `double` is held in an x87 register at the
//! register's own width, so an `fp_round` (the `FPTrunc` of the IR) that
//! drops precision is done by storing the value in the narrower type to a
//! stack slot and loading it back: the store rounds. [`preprocess`] rewrites
//! every such node, `fp_round x, 0` between x87 types, into that store and
//! load. An `fp_extend` between x87 types and an `fp_round` with the
//! "value is unchanged" flag set stay: the register holds either exactly.
//!
//! # Runs
//! Before selection, from the `--isel=dag` driver, which owns the frame
//! objects ([`FrameObjects`]).
//!
//! # See also
//! LLVM's `X86DAGToDAGISel::PreprocessISelDAG`
//! ([L988](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L988)),
//! its `FP_ROUND`/`FP_EXTEND` case at L1454.

use super::dag::{MemBase, MemInfo, SDValue, SelectionDAG};
use super::isd::ISD;
use super::mvt::{EVT, MVT};

/// `CreateStackTemporary`: where the stack slots come from.
pub trait FrameObjects {
    /// A new frame object of `size` bytes, aligned to `align`; its index.
    fn create_stack_temporary(
        &mut self,
        size: u32,
        align: u32,
    ) -> i32;
}

fn is_x87(vt: EVT) -> bool {
    matches!(vt, EVT::Simple(MVT::F32 | MVT::F64 | MVT::F80))
}

/// Rewrite the x87 `fp_round`s of `dag` through memory; returns how many.
pub fn preprocess(
    dag: &mut SelectionDAG,
    frame: &mut dyn FrameObjects,
) -> usize {
    let mut rewritten = 0;
    // The pin walks the nodes from the last made; a node made on the way
    // (the store and load below) is not an `fp_round`.
    for node in dag.node_ids().rev() {
        let value = SDValue { node, res: 0 };
        if dag.opcode(value) != ISD::FpRound || dag.node(value).ops.len() != 2 {
            continue;
        }
        let source = dag.operand(value, 0);
        let (from, to) = (dag.value_type(source), dag.value_type(value));
        // `fp_round x, 1` says the value is exact in the narrower type.
        if !is_x87(from) || !is_x87(to) || dag.constant(dag.operand(value, 1)) != Some(0) {
            continue;
        }
        let size = to.size_in_bits().div_ceil(8);
        let index = frame.create_stack_temporary(size, size);
        let slot = dag.get_frame_index(dag.config().pointer, index);
        let info = MemInfo { mem_vt: to, align: size, volatile: false, base: MemBase::Stack(index), ext: None, space: 0 };
        let chain = dag.entry_node();
        let store = dag.get_store(chain, source, slot, info);
        let reloaded = dag.get_load(to, store, slot, info);
        dag.replace_all_uses_with(value, reloaded);
        rewritten += 1;
    }
    rewritten
}
