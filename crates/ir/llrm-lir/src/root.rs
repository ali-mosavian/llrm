//! `root` from `qbopt/model/ir.py`: the 32-bit register a general register is a view of.

use iced_x86::Register;

/// The 32-bit root of a byte, word or dword general register of the eight legacy families, which iced states
/// from the register itself; any other register (segment, x87, the extended ones) is its own.
pub fn root(register: Register) -> Register {
    let full = register.full_register32();
    let extended_byte = matches!(register, Register::SIL | Register::DIL | Register::BPL | Register::SPL);
    let legacy = (register.is_gpr8() && !extended_byte) || register.is_gpr16() || register.is_gpr32();
    if legacy && matches!(full, Register::EAX | Register::EBX | Register::ECX | Register::EDX | Register::ESI | Register::EDI | Register::EBP | Register::ESP) {
        full
    } else {
        register
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every view of each legacy family roots to its dword; anything else passes through. The 24 views the
    /// pass ever reasons about were a table here, which iced's own answer replaces.
    #[test]
    fn roots_every_family_and_passes_others_through() {
        let families = [
            (Register::EAX, [Register::AL, Register::AH, Register::AX]),
            (Register::EBX, [Register::BL, Register::BH, Register::BX]),
            (Register::ECX, [Register::CL, Register::CH, Register::CX]),
            (Register::EDX, [Register::DL, Register::DH, Register::DX]),
            (Register::ESI, [Register::SI, Register::SI, Register::SI]),
            (Register::EDI, [Register::DI, Register::DI, Register::DI]),
            (Register::EBP, [Register::BP, Register::BP, Register::BP]),
            (Register::ESP, [Register::SP, Register::SP, Register::SP]),
        ];
        for (rooted, views) in families {
            assert_eq!(root(rooted), rooted);
            for view in views {
                assert_eq!(root(view), rooted, "{view:?}");
            }
        }
        assert_eq!(root(Register::ES), Register::ES);
        assert_eq!(root(Register::ST0), Register::ST0);
        assert_eq!(root(Register::SIL), Register::SIL);
        assert_eq!(root(Register::RAX), Register::RAX);
        assert_eq!(root(Register::R8D), Register::R8D);
    }
}
