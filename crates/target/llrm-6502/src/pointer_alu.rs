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
    /// The operand after the pointer is a byte register holding the index (`(p),y` with `y` the index), not an offset.
    pub indexed: bool,
}

pub fn pointer_alu(opcode: u16) -> Option<PointerAlu> {
    let (mnemonic, carry, a, ptr, word, indexed) = match opcode {
        op::ADD8P => ("ADC", Some("CLC"), 1, 2, false, false),
        op::SUB8P => ("SBC", Some("SEC"), 1, 2, false, false),
        op::AND8P => ("AND", None, 1, 2, false, false),
        op::OR8P => ("ORA", None, 1, 2, false, false),
        op::XOR8P => ("EOR", None, 1, 2, false, false),
        // d, carry out, a, [carry in,] pointer
        op::ADDO8P => ("ADC", Some("CLC"), 2, 3, false, false),
        op::SUBO8P => ("SBC", Some("SEC"), 2, 3, false, false),
        op::ADDOC8P => ("ADC", None, 2, 4, false, false),
        op::SUBOC8P => ("SBC", None, 2, 4, false, false),
        op::ADD16P => ("ADC", Some("CLC"), 1, 2, true, false),
        op::SUB16P => ("SBC", Some("SEC"), 1, 2, true, false),
        op::AND16P => ("AND", None, 1, 2, true, false),
        op::OR16P => ("ORA", None, 1, 2, true, false),
        op::XOR16P => ("EOR", None, 1, 2, true, false),
        op::ADDO16P => ("ADC", Some("CLC"), 2, 3, true, false),
        op::SUBO16P => ("SBC", Some("SEC"), 2, 3, true, false),
        op::ADDOC16P => ("ADC", None, 2, 4, true, false),
        op::SUBOC16P => ("SBC", None, 2, 4, true, false),
        op::ADD8PI => ("ADC", Some("CLC"), 1, 2, false, true),
        op::SUB8PI => ("SBC", Some("SEC"), 1, 2, false, true),
        op::AND8PI => ("AND", None, 1, 2, false, true),
        op::OR8PI => ("ORA", None, 1, 2, false, true),
        op::XOR8PI => ("EOR", None, 1, 2, false, true),
        op::ADDO8PI => ("ADC", Some("CLC"), 2, 3, false, true),
        op::SUBO8PI => ("SBC", Some("SEC"), 2, 3, false, true),
        op::ADDOC8PI => ("ADC", None, 2, 4, false, true),
        op::SUBOC8PI => ("SBC", None, 2, 4, false, true),
        _ => return None,
    };
    Some(PointerAlu { mnemonic, carry, a, ptr, word, indexed })
}
