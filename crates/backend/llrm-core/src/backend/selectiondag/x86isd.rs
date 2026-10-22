//! Target opcodes (`X86ISD::*`) as [`ISD::Target`] codes, in the pin's order
//! of first use here; the legalizer and lowering add theirs after these.

use super::isd::ISD;

pub const CALL: ISD = ISD::Target(1);
pub const RET_GLUE: ISD = ISD::Target(2);

/// `getTargetNodeName`.
pub fn name(code: u16) -> String {
    match code {
        1 => "X86ISD::CALL".into(),
        2 => "X86ISD::RET_GLUE".into(),
        other => format!("X86ISD#{other}"),
    }
}
