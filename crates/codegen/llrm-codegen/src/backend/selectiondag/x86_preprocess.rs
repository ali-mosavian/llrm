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
//! With SSE2 `f32` and `f64` are in xmm registers, and a conversion between
//! one and an x87 type (`fpext float to x86_fp80`) goes through the slot
//! too: the store in the narrower type, then an extending load.
//!
//! # Runs
//! Before selection, from the `--isel=dag` driver, which owns the frame
//! objects ([`FrameObjects`]).
//!
//! # See also
//! LLVM's `X86DAGToDAGISel::PreprocessISelDAG`
//! ([L988](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L988)),
//! its `FP_ROUND`/`FP_EXTEND` case at L1454.

use super::dag::{MemBase, MemInfo, NodeData, SDValue, SelectionDAG};
use super::isd::ISD;
use super::mvt::{EVT, MVT};

/// `CreateStackTemporary`: where the stack slots come from.
pub trait FrameObjects {
    /// How many frame objects there are: the index the next one gets.
    fn objects(&self) -> i32;

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

/// `isScalarFPTypeInSSEReg`: with SSE2, `f32` and `f64` live in an xmm
/// register.
fn in_sse(
    vt: EVT,
    sse2: bool,
) -> bool {
    sse2 && matches!(vt, EVT::Simple(MVT::F32 | MVT::F64))
}

/// DEVIATE SD226: a store of an x87 `fp_round` that nothing else reads is one
/// truncating store (`fst`) of the rounded value. The pin's `visitSTORE` makes
/// the same fold but only where the truncating store is legal, and X86 marks
/// `f64 -> f32` Expand, so it keeps the round trip through a stack temporary
/// (`fstps (%esp); flds (%esp); fstps (%eax)`); the old selector stores once.
/// Returns the `fp_round`s it folded.
fn truncating_stores(
    dag: &mut SelectionDAG,
    sse2: bool,
) -> Vec<u32> {
    let mut folded = Vec::new();
    for node in dag.node_ids().collect::<Vec<_>>() {
        let value = SDValue { node, res: 0 };
        if dag.opcode(value) != ISD::FpRound || dag.node(value).ops.len() != 2 || !dag.has_one_use(node) {
            continue;
        }
        let source = dag.operand(value, 0);
        let (from, to) = (dag.value_type(source), dag.value_type(value));
        if !is_x87(from) || !is_x87(to) || in_sse(from, sse2) || in_sse(to, sse2) {
            continue;
        }
        let Some(user) = dag.users(value).next() else { continue };
        let store = SDValue { node: user, res: 0 };
        if dag.opcode(store) != ISD::Store || dag.operand(store, 1) != value {
            continue;
        }
        let NodeData::Mem(info) = dag.node(store).data else { continue };
        if info.volatile || info.mem_vt != to {
            continue;
        }
        let (chain, ptr) = (dag.operand(store, 0), dag.operand(store, 2));
        let truncating = dag.get_store(chain, source, ptr, info);
        dag.replace_all_uses_with(store, truncating);
        folded.push(node);
    }
    folded
}

/// Rewrite the `fp_round`s and `fp_extend`s that cross between the x87 stack
/// and the SSE registers, or round on the stack, through memory; returns how
/// many. The x87 has an extending load and a truncating store; SSE folds a
/// direct load into other operations.
pub fn preprocess(
    dag: &mut SelectionDAG,
    frame: &mut dyn FrameObjects,
    sse2: bool,
) -> usize {
    let folded = truncating_stores(dag, sse2);
    let mut rewritten = folded.len();
    // The pin walks the nodes from the last made; a node made on the way
    // (the store and load below) is neither.
    for node in dag.node_ids().rev() {
        let value = SDValue { node, res: 0 };
        if folded.contains(&node) {
            continue;
        }
        let round = match dag.opcode(value) {
            ISD::FpRound if dag.node(value).ops.len() == 2 => true,
            ISD::FpExtend => false,
            _ => continue,
        };
        let source = dag.operand(value, 0);
        let (from, to) = (dag.value_type(source), dag.value_type(value));
        if !is_x87(from) && !in_sse(from, sse2) || !is_x87(to) && !in_sse(to, sse2) {
            continue;
        }
        let (from_sse, to_sse) = (in_sse(from, sse2), in_sse(to, sse2));
        // Both in xmm registers: a legal conversion. Neither: an extension is a
        // no-op, and so is `fp_round x, 1` (the value is exact in the narrower
        // type).
        if from_sse && to_sse || !from_sse && !to_sse && (!round || dag.constant(dag.operand(value, 1)) != Some(0)) {
            continue;
        }
        let mem = if round { to } else { from };
        let size = mem.size_in_bits().div_ceil(8);
        let index = frame.create_stack_temporary(size, size);
        let slot = dag.get_frame_index(dag.config().pointer, index);
        let info = MemInfo {
            mem_vt: mem,
            align: size,
            volatile: false,
            base: MemBase::Stack(index),
            ext: None,
            space: 0,
            ordering: None,
        };
        let chain = dag.entry_node();
        let store = dag.get_store(chain, source, slot, info);
        let reloaded = if mem == to {
            dag.get_load(to, store, slot, info)
        } else {
            dag.get_ext_load(super::actions::LoadExt::ExtLoad, to, store, slot, info)
        };
        dag.replace_all_uses_with(value, reloaded);
        rewritten += 1;
    }
    rewritten
}
