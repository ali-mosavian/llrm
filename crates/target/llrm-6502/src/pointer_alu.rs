//! The byte ops that read their second operand through a pointer (`ADD8P`..`SUBOC8PZ`): one table for the allocator, the
//! text emitter, the frame code and the selector.

use crate::dagisel::generated::ids::opcode as op;

/// One folded op: the 6502 mnemonic, the carry set-up before it, and where its operands sit.
pub struct PointerAlu {
    pub mnemonic: &'static str,
    pub carry: Option<&'static str>,
    /// The operand holding the first byte.
    pub a: usize,
    /// The operand holding the pointer; the offset follows it in a `Y` form.
    pub ptr: usize,
    /// `ldy #off; op (p),y`; the other form is the 65C02's `op (p)`.
    pub y_form: bool,
}

pub fn pointer_alu(opcode: u16) -> Option<PointerAlu> {
    let (mnemonic, carry, a, ptr, y_form) = match opcode {
        op::ADD8P => ("ADC", Some("CLC"), 1, 2, true),
        op::ADD8PZ => ("ADC", Some("CLC"), 1, 2, false),
        op::SUB8P => ("SBC", Some("SEC"), 1, 2, true),
        op::SUB8PZ => ("SBC", Some("SEC"), 1, 2, false),
        op::AND8P => ("AND", None, 1, 2, true),
        op::AND8PZ => ("AND", None, 1, 2, false),
        op::OR8P => ("ORA", None, 1, 2, true),
        op::OR8PZ => ("ORA", None, 1, 2, false),
        op::XOR8P => ("EOR", None, 1, 2, true),
        op::XOR8PZ => ("EOR", None, 1, 2, false),
        // d, carry out, a, [carry in,] pointer
        op::ADDO8P => ("ADC", Some("CLC"), 2, 3, true),
        op::ADDO8PZ => ("ADC", Some("CLC"), 2, 3, false),
        op::SUBO8P => ("SBC", Some("SEC"), 2, 3, true),
        op::SUBO8PZ => ("SBC", Some("SEC"), 2, 3, false),
        op::ADDOC8P => ("ADC", None, 2, 4, true),
        op::ADDOC8PZ => ("ADC", None, 2, 4, false),
        op::SUBOC8P => ("SBC", None, 2, 4, true),
        op::SUBOC8PZ => ("SBC", None, 2, 4, false),
        _ => return None,
    };
    Some(PointerAlu { mnemonic, carry, a, ptr, y_form })
}
