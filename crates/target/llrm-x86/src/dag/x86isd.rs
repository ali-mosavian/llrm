//! Target opcodes (`X86ISD::*`) as [`ISD::Target`] codes, in the pin's order
//! of first use here; the legalizer and lowering add theirs after these.

use super::isd::ISD;

pub const CALL: ISD = ISD::Target(1);
pub const RET_GLUE: ISD = ISD::Target(2);
pub const REP_STOS: ISD = ISD::Target(3);
pub const REP_MOVS: ISD = ISD::Target(4);
/// A float call's result, popped from `st(0)`: `chain, Register $fp0, glue`.
pub const POP_FROM_X87_REG: ISD = ISD::Target(5);
/// A `llrm.ia16.asm.*` block (`tools/dagisel/own_ids.txt`); see
/// `selectiondag::inline_asm` in llrm-core.
pub const LLRM_ASM: ISD = ISD::Target(24578);
/// The selector of the segment a global lives in, an `i16`: a far global is
/// `build_pair(GlobalAddress, SEGMENT_OF)`. Carries `NodeData::Global`.
pub const SEGMENT_OF: ISD = ISD::Target(24580);
/// `llrm.ia16.window`: a huge pointer as the far one into its current
/// segment window, `far -> far` (not a delta-0 `HUGE_ADD`, which does not
/// normalize; expanded by the type legalizer).
pub const LLRM_WINDOW: ISD = ISD::Target(24581);
/// A `llrm.ia16.code.*` block: `chain, TargetExternalSymbol(name),
/// TargetFrameIndex per place the name patches in, glue`; the result is read
/// from the convention's result registers after it.
pub const LLRM_CODE: ISD = ISD::Target(24582);
/// `llrm.ia16.ptrdiff`: the byte difference of two huge pointers into one
/// object, `(far, far) -> i32`, expanded by the type legalizer with the
/// machine's selector stride.
pub const LLRM_PTRDIFF: ISD = ISD::Target(24583);
/// A call through a far pointer: `X86ISD::CALL` with the callee's offset and
/// selector in a 4-byte stack cell, `chain, TargetFrameIndex cell, ...` as
/// `CALL`'s other operands; printed `call far [cell]`.
///
/// DEVIATE SD208: the pin has no far call. The old selector stored the offset
/// then the selector to a 4-byte temporary and did `call m16:16` through it.
/// Exit: none (m16 only). Owner sdag-b. Review 2026-12-10.
pub const LLRM_FARCALL: ISD = ISD::Target(24584);

/// `getTargetNodeName`.
pub fn name(code: u16) -> String {
    match code {
        1 => "X86ISD::CALL".into(),
        2 => "X86ISD::RET_GLUE".into(),
        3 => "X86ISD::REP_STOS".into(),
        4 => "X86ISD::REP_MOVS".into(),
        5 => "X86ISD::POP_FROM_X87_REG".into(),
        24578 => "X86ISD::LLRM_ASM".into(),
        24580 => "X86ISD::SEGMENT_OF".into(),
        24581 => "X86ISD::LLRM_WINDOW".into(),
        24582 => "X86ISD::LLRM_CODE".into(),
        24584 => "X86ISD::LLRM_FARCALL".into(),
        24583 => "X86ISD::LLRM_PTRDIFF".into(),
        other => format!("X86ISD#{other}"),
    }
}
