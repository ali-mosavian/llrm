//! `root` from `qbopt/model/ir.py`: the 32-bit register a general register is a
//! view of.

use crate::registers::RegId;

/// The 32-bit root of a byte, word or dword general register of the eight
/// legacy families, which iced states from the register itself; any other
/// register (segment, x87, the extended ones) is its own.
pub fn root(register: RegId) -> RegId {
    let full = register.full_register32();
    let extended_byte = matches!(register, RegId::SIL | RegId::DIL | RegId::BPL | RegId::SPL);
    let legacy = (register.is_gpr8() && !extended_byte) || register.is_gpr16() || register.is_gpr32();
    if legacy
        && matches!(
            full,
            RegId::EAX | RegId::EBX | RegId::ECX | RegId::EDX | RegId::ESI | RegId::EDI | RegId::EBP | RegId::ESP
        )
    {
        full
    } else {
        register
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every view of each legacy family roots to its dword; anything else
    /// passes through. The 24 views the pass ever reasons about were a
    /// table here, which iced's own answer replaces.
    #[test]
    fn roots_every_family_and_passes_others_through() {
        let families = [
            (RegId::EAX, [RegId::AL, RegId::AH, RegId::AX]),
            (RegId::EBX, [RegId::BL, RegId::BH, RegId::BX]),
            (RegId::ECX, [RegId::CL, RegId::CH, RegId::CX]),
            (RegId::EDX, [RegId::DL, RegId::DH, RegId::DX]),
            (RegId::ESI, [RegId::SI, RegId::SI, RegId::SI]),
            (RegId::EDI, [RegId::DI, RegId::DI, RegId::DI]),
            (RegId::EBP, [RegId::BP, RegId::BP, RegId::BP]),
            (RegId::ESP, [RegId::SP, RegId::SP, RegId::SP]),
        ];
        for (rooted, views) in families {
            assert_eq!(root(rooted), rooted);
            for view in views {
                assert_eq!(root(view), rooted, "{view:?}");
            }
        }
        assert_eq!(root(RegId::ES), RegId::ES);
        assert_eq!(root(RegId::ST0), RegId::ST0);
        assert_eq!(root(RegId::SIL), RegId::SIL);
        assert_eq!(root(RegId::RAX), RegId::RAX);
        assert_eq!(root(RegId::R8D), RegId::R8D);
    }
}
