//! `ROOT` and `root` from `qbopt/model/ir.py`.

use iced_x86::Register;

/// `ROOT`: the 32-bit root of every general-purpose register this pass ever
/// reasons about, in Python's insertion order.
pub const ROOT: [(Register, Register); 24] = [
    (Register::AL, Register::EAX),
    (Register::AH, Register::EAX),
    (Register::AX, Register::EAX),
    (Register::EAX, Register::EAX),
    (Register::BL, Register::EBX),
    (Register::BH, Register::EBX),
    (Register::BX, Register::EBX),
    (Register::EBX, Register::EBX),
    (Register::CL, Register::ECX),
    (Register::CH, Register::ECX),
    (Register::CX, Register::ECX),
    (Register::ECX, Register::ECX),
    (Register::DL, Register::EDX),
    (Register::DH, Register::EDX),
    (Register::DX, Register::EDX),
    (Register::EDX, Register::EDX),
    (Register::SI, Register::ESI),
    (Register::ESI, Register::ESI),
    (Register::DI, Register::EDI),
    (Register::EDI, Register::EDI),
    (Register::BP, Register::EBP),
    (Register::EBP, Register::EBP),
    (Register::SP, Register::ESP),
    (Register::ESP, Register::ESP),
];

/// `ROOT.get(register, register)`.
pub fn root(register: Register) -> Register {
    ROOT.iter().find(|(key, _)| *key == register).map_or(register, |(_, rooted)| *rooted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roots_every_family_and_passes_others_through() {
        for (register, rooted) in ROOT {
            assert_eq!(root(register), rooted);
        }
        assert_eq!(root(Register::ES), Register::ES);
        assert_eq!(root(Register::ST0), Register::ST0);
    }
}
