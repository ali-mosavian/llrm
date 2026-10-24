//! The `llrm.ia16.*` intrinsic nodes (`X86ISD::LLRM_ASM`, `_WINDOW`) and
//! `SEGMENT_OF`: passed through selection unchanged as one pseudo machine node
//! each.
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

/// `X86ISD::LLRM_ASM` and `LLRM_WINDOW` (`tools/dagisel/own_ids.txt`).
pub const LLRM_NODES: [u16; 2] = [24578, 24581];

/// `X86ISD::SEGMENT_OF`: the segment of its global operand, or `DGROUP` with
/// none.
pub const SEGMENT_OF: u16 = 24580;

/// The machine opcode the intrinsic nodes become: after the pin's own and m16's
/// `REP_*_16` numbers.
pub const LLRM_ASM_OPCODE: u16 = 30010;

/// The machine opcode `SEGMENT_OF` becomes; the emitter prints `seg g` or
/// `DGROUP`.
pub const SEGMENT_OF_OPCODE: u16 = 30011;

/// `X86ISD::LLRM_FARCALL` (sdag-b, SD208): a far call, chain and glue as the
/// near `CALL`.
pub const LLRM_FARCALL: u16 = 24584;

/// The machine opcode `LLRM_FARCALL` becomes; the emitter prints `call far`.
pub const FARCALL_OPCODE: u16 = 30012;

/// `X86ISD::LLRM_CODE` (sdag-a): an `llrm.ia16.code` call, with its bytes in
/// the frame.
pub const LLRM_CODE: u16 = 24582;

/// The machine opcode `LLRM_CODE` becomes.
pub const CODE_OPCODE: u16 = 30013;

/// The pseudo opcode an llrm node of this id becomes.
pub fn llrm_opcode(node: u16) -> Option<u16> {
    match node {
        SEGMENT_OF => Some(SEGMENT_OF_OPCODE),
        LLRM_FARCALL => Some(FARCALL_OPCODE),
        LLRM_CODE => Some(CODE_OPCODE),
        n if LLRM_NODES.contains(&n) => Some(LLRM_ASM_OPCODE),
        _ => None,
    }
}

/// Makes `node` the pseudo machine node with its own operands and result types.
pub fn select_llrm_node<D: Dag>(
    dag: &mut D,
    node: u32,
    opcode: u16,
) {
    let vts: Vec<_> = (0..dag.num_values(node)).map(|i| dag.value_type(SDValue::new(node, i as u8))).collect();
    let ops: Vec<_> = (0..dag.num_operands(node)).map(|i| dag.operand(node, i)).collect();
    dag.morph_node_to(node, opcode, &vts, &ops);
}
