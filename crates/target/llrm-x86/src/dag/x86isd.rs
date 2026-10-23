//! Target opcodes (`X86ISD::*`) as [`ISD::Target`] codes, in the pin's order
//! of first use here; the legalizer and lowering add theirs after these.

use super::isd::ISD;

pub const CALL: ISD = ISD::Target(1);
pub const RET_GLUE: ISD = ISD::Target(2);
pub const REP_STOS: ISD = ISD::Target(3);
pub const REP_MOVS: ISD = ISD::Target(4);
/// A `llrm.ia16.asm.*` block (`tools/dagisel/own_ids.txt`); see
/// [`super::inline_asm`].
pub const LLRM_ASM: ISD = ISD::Target(24578);

/// `getTargetNodeName`.
pub fn name(code: u16) -> String {
    match code {
        1 => "X86ISD::CALL".into(),
        2 => "X86ISD::RET_GLUE".into(),
        3 => "X86ISD::REP_STOS".into(),
        4 => "X86ISD::REP_MOVS".into(),
        24578 => "X86ISD::LLRM_ASM".into(),
        other => format!("X86ISD#{other}"),
    }
}
