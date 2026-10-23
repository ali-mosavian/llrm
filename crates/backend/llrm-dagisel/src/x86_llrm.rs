//! The `llrm.ia16.*` intrinsic nodes (`X86ISD::LLRM_ASM`, `_WINDOW`, `_CODE`):
//! passed through selection unchanged as one pseudo machine node.
//!
//! # How it works
//! The builder makes one custom node per intrinsic (chain, the symbol holding
//! the verbatim text, fixup addresses, input registers, glue). No pattern
//! covers them; this case morphs the node into [`LLRM_ASM_OPCODE`] with the
//! same operands and result types, and the emitter prints the text.
//!
//! Numbered deviation: **SD305** these nodes and the pseudo are llrm's own, not
//! the pin's. Owner sdag-c. Exit: the frontend emits the pin's `INLINEASM`.
//! Review 2026-12-10.
//!
//! # Used by
//! [`crate::x86_select::select_node`].

use crate::dag::{Dag, SDValue};

/// `X86ISD::LLRM_ASM`, `LLRM_WINDOW`, `LLRM_CODE`
/// (`tools/dagisel/own_ids.txt`).
pub const LLRM_NODES: [u16; 3] = [24578, 24579, 24580];

/// The machine opcode the three nodes become: after the pin's own and m16's
/// `REP_*_16` numbers.
pub const LLRM_ASM_OPCODE: u16 = 30010;

/// Makes `node` the pseudo machine node with its own operands and result types.
pub fn select_llrm_node<D: Dag>(
    dag: &mut D,
    node: u32,
) {
    let vts: Vec<_> = (0..dag.num_values(node)).map(|i| dag.value_type(SDValue::new(node, i as u8))).collect();
    let ops: Vec<_> = (0..dag.num_operands(node)).map(|i| dag.operand(node, i)).collect();
    dag.morph_node_to(node, LLRM_ASM_OPCODE, &vts, &ops);
}
