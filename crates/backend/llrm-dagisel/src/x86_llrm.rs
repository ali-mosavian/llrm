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
//! Numbered deviations: **SD214** (the transcendental opcodes below, sdag-b,
//! exit: libm calls on a target with a libm, review 2026-12-10); **SD305**
//! these nodes and the pseudo are llrm's own, not the pin's. Owner sdag-c.
//! Exit: the frontend emits the pin's `INLINEASM`. Review 2026-12-10.
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

/// `X86ISD::LLRM_PORT` (sdag-a): an `in` or an `out`.
pub const LLRM_PORT: u16 = 24586;

/// The machine opcode `LLRM_PORT` becomes.
pub const PORT_OPCODE: u16 = 30014;

/// `X86ISD::LLRM_TC_RETURN` (sdag-a): `TC_RETURN` at m16, where the pin has no
/// pattern for it (a far callee is jumped to, `jmp far`).
pub const LLRM_TC_RETURN: u16 = 24587;

/// The machine opcode `LLRM_TC_RETURN` becomes.
pub const TC_RETURN_OPCODE: u16 = 30026;

/// The machine opcodes the transcendental `FLOG`, `FEXP`, `FLOG2`, `FEXP2`,
/// `FSIN`, `FCOS`, `FATAN` and `FRINT` become (SD214): the x87 operations or
/// sequences `fln`, `fexp`, `flog2`, `fexp2`, `fsin`, `fcos` and `fatan`,
/// which the pin has no pattern for (it calls libm unless unsafe math is on;
/// llrm's targets have none).
pub const FLN_OPCODE: u16 = 30015;
pub const FEXP_OPCODE: u16 = 30016;
pub const FLOG2_OPCODE: u16 = 30017;
pub const FEXP2_OPCODE: u16 = 30018;
pub const FSIN_OPCODE: u16 = 30019;
pub const FCOS_OPCODE: u16 = 30020;
pub const FATAN_OPCODE: u16 = 30021;
pub const FRINT_OPCODE: u16 = 30025;

/// The machine opcodes `X86ISD::FIST` of an `i16` becomes (SD215): the pin has
/// `IST_Fp16m32/64/80` and no pattern for them, since its `lrint` of a
/// `short` is promoted; QB's `CINT` stores a word.
pub const IST16_OPCODES: [u16; 3] = [30022, 30023, 30024];

/// The pseudo opcode an llrm node of this id becomes.
pub fn llrm_opcode(node: u16) -> Option<u16> {
    match node {
        SEGMENT_OF => Some(SEGMENT_OF_OPCODE),
        LLRM_FARCALL => Some(FARCALL_OPCODE),
        LLRM_CODE => Some(CODE_OPCODE),
        LLRM_PORT => Some(PORT_OPCODE),
        LLRM_TC_RETURN => Some(TC_RETURN_OPCODE),
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
