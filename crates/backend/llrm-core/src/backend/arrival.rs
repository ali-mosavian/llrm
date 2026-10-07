//! Where a parameter is while the function has not yet stored it into its home: in the register the convention
//! passed it in, from the entry until the instruction that stores it. Read back from the code, like the frame rows.

use iced_x86::{Decoder, DecoderOptions, FlowControl, Mnemonic, OpKind, Register};

fn full(register: Register) -> Register {
    if register.is_gpr() { register.full_register32() } else { register }
}

/// The offset after the first instruction that stores `register` into the frame cell `disp` bytes past `frame`,
/// found along the straight line from the entry; None where a branch, a return or a write to `register` comes first
/// (the value is then not the argument any more), or none stores it.
pub fn stored(code: &[u8], frame: Register, disp: i64, register: Register) -> Option<usize> {
    let mut decoder = Decoder::with_ip(32, code, 0, DecoderOptions::NONE);
    while decoder.can_decode() {
        let one = decoder.decode();
        if one.is_invalid() {
            return None;
        }
        let to_cell = one.mnemonic() == Mnemonic::Mov
            && one.op0_kind() == OpKind::Memory
            && full(one.memory_base()) == full(frame)
            && one.memory_index() == Register::None
            && i64::from(one.memory_displacement32() as i32) == disp
            && one.op1_kind() == OpKind::Register
            && full(one.op1_register()) == full(register);
        if to_cell {
            return Some(one.ip() as usize + one.len());
        }
        if one.op0_kind() == OpKind::Register && full(one.op0_register()) == full(register) && one.mnemonic() != Mnemonic::Push {
            return None;
        }
        if !matches!(one.flow_control(), FlowControl::Next | FlowControl::Call) {
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use iced_x86::Register::{EAX, EBP, ECX, EDX};

    use super::stored;

    /// `push ebp; mov ebp, esp; sub esp, 8; mov [ebp-4], eax; mov [ebp-8], edx; ret`: eax is stored at 9, edx at 12,
    /// each after the one before; a register nothing stores, and one stored to another cell, are none.
    #[test]
    fn the_first_store_of_the_arrival_register_into_the_home_ends_its_stay_in_the_register() {
        let code = [0x55, 0x89, 0xE5, 0x83, 0xEC, 0x08, 0x89, 0x45, 0xFC, 0x89, 0x55, 0xF8, 0xC3];
        assert_eq!(stored(&code, EBP, -4, EAX), Some(9));
        assert_eq!(stored(&code, EBP, -8, EDX), Some(12));
        assert_eq!(stored(&code, EBP, -8, EAX), None);
        assert_eq!(stored(&code, EBP, -4, ECX), None);
    }

    /// A register written before its store holds something else by then: the store is no arrival's.
    #[test]
    fn a_register_overwritten_before_its_store_is_not_the_argument() {
        // mov eax, 1; mov [ebp-4], eax
        let code = [0xB8, 1, 0, 0, 0, 0x89, 0x45, 0xFC];
        assert_eq!(stored(&code, EBP, -4, EAX), None);
    }
}
