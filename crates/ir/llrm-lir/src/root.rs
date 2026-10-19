//! `root`: the register a general register is a view of, named by its dword.

use crate::registers::RegId;

/// The root of a byte, word, dword or (long mode's) quadword general register:
/// the dword register of its family, which iced states from the register itself
/// (`AL`, `AX`, `EAX` and `RAX` are views of `EAX`; `R8B`..`R8` of `R8D`). The
/// name is the token a target's `registers.regs` roots its family at; any other
/// register (segment, x87, SSE) is its own. A 32-bit target never meets the
/// 64-bit views, so its roots are what they were.
pub fn root(register: RegId) -> RegId {
    register.root()
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
        // Long mode's registers root at their dword: the quadword and the new
        // bytes are views of it.
        for (root_of, views) in [
            (RegId::ESI, [RegId::SIL, RegId::RSI]),
            (RegId::EAX, [RegId::AL, RegId::RAX]),
            (RegId::R8D, [RegId::R8L, RegId::R8]),
            (RegId::R15D, [RegId::R15W, RegId::R15]),
        ] {
            assert_eq!(root(root_of), root_of);
            for view in views {
                assert_eq!(root(view), root_of, "{view:?}");
            }
        }
    }
}
