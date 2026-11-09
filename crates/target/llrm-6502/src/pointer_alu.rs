//! The ops that read their second operand through a pointer (`ADD8P`..`SUBOC16P`): one table for the allocator, the frame
//! code and the selector.

use crate::dagisel::generated::ids::opcode as op;

/// One folded op: the 6502 mnemonic, the carry set-up before it, and where its operands sit.
pub struct PointerAlu {
    pub mnemonic: &'static str,
    pub carry: Option<&'static str>,
    /// The operand holding the first value.
    pub a: usize,
    /// The operand holding the pointer; the offset follows it.
    pub ptr: usize,
    /// The operands and the result are words, the second at `off` and `off + 1`.
    pub word: bool,
}

pub fn pointer_alu(opcode: u16) -> Option<PointerAlu> {
    let (mnemonic, carry, a, ptr, word) = match opcode {
        op::ADD8P => ("ADC", Some("CLC"), 1, 2, false),
        op::SUB8P => ("SBC", Some("SEC"), 1, 2, false),
        op::AND8P => ("AND", None, 1, 2, false),
        op::OR8P => ("ORA", None, 1, 2, false),
        op::XOR8P => ("EOR", None, 1, 2, false),
        // d, carry out, a, [carry in,] pointer
        op::ADDO8P => ("ADC", Some("CLC"), 2, 3, false),
        op::SUBO8P => ("SBC", Some("SEC"), 2, 3, false),
        op::ADDOC8P => ("ADC", None, 2, 4, false),
        op::SUBOC8P => ("SBC", None, 2, 4, false),
        op::ADD16P => ("ADC", Some("CLC"), 1, 2, true),
        op::SUB16P => ("SBC", Some("SEC"), 1, 2, true),
        op::AND16P => ("AND", None, 1, 2, true),
        op::OR16P => ("ORA", None, 1, 2, true),
        op::XOR16P => ("EOR", None, 1, 2, true),
        op::ADDO16P => ("ADC", Some("CLC"), 2, 3, true),
        op::SUBO16P => ("SBC", Some("SEC"), 2, 3, true),
        op::ADDOC16P => ("ADC", None, 2, 4, true),
        op::SUBOC16P => ("SBC", None, 2, 4, true),
        _ => return None,
    };
    Some(PointerAlu { mnemonic, carry, a, ptr, word })
}
