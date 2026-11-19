//! The pin's register numbers and x86's registers, each from the other: what the
//! matcher interpreter and the DAG builder name a physical register by.

use llrm_lir::registers::RegId;

/// The id the flags register has: iced numbers no EFLAGS, so it takes the last
/// raw id.
pub const EFLAGS_ID: RegId = RegId::from_raw(255);

/// The register of the pin's register number `register` that the builder names
/// as a physical register (`NodeData::PhysReg`); `None` for `$noreg` (0) and
/// the ones with no id.
pub fn phys_reg(register: u16) -> Option<RegId> {
    use llrm_dagisel::generated::ids::x86;
    // The pin's register numbers, in its X86 enum order (alphabetical: AH 1,
    // AL 2, AX 3, ... EDX 27, ES 31); the ids module names the ones
    // tools/dagisel/extra_symbols.txt asks for.
    const AL: u16 = 2;
    Some(match register {
        AL => RegId::AL,
        x86::AH => RegId::AH,
        x86::AX => RegId::AX,
        x86::CL => RegId::CL,
        x86::CX => RegId::CX,
        x86::DX => RegId::DX,
        x86::EAX => RegId::EAX,
        x86::EDX => RegId::EDX,
        x86::RAX => RegId::RAX,
        x86::RIP => RegId::RIP,
        x86::RDX => RegId::RDX,
        x86::CS => RegId::CS,
        x86::DS => RegId::DS,
        x86::ES => RegId::ES,
        x86::FS => RegId::FS,
        x86::GS => RegId::GS,
        x86::SS => RegId::SS,
        x86::BH => RegId::BH,
        x86::BL => RegId::BL,
        x86::BP => RegId::BP,
        x86::BX => RegId::BX,
        x86::CH => RegId::CH,
        x86::DH => RegId::DH,
        x86::DI => RegId::DI,
        x86::DL => RegId::DL,
        x86::EBP => RegId::EBP,
        x86::EBX => RegId::EBX,
        x86::ECX => RegId::ECX,
        x86::EDI => RegId::EDI,
        x86::ESI => RegId::ESI,
        x86::ESP => RegId::ESP,
        x86::RBP => RegId::RBP,
        x86::RBX => RegId::RBX,
        x86::RCX => RegId::RCX,
        x86::RDI => RegId::RDI,
        x86::RSI => RegId::RSI,
        x86::RSP => RegId::RSP,
        x86::SI => RegId::SI,
        x86::SP => RegId::SP,
        x86::R8 => RegId::R8,
        x86::R9 => RegId::R9,
        x86::R10 => RegId::R10,
        x86::R11 => RegId::R11,
        x86::R12 => RegId::R12,
        x86::R13 => RegId::R13,
        x86::R14 => RegId::R14,
        x86::R15 => RegId::R15,
        x86::R8D => RegId::R8D,
        x86::R9D => RegId::R9D,
        x86::R10D => RegId::R10D,
        x86::R11D => RegId::R11D,
        x86::R12D => RegId::R12D,
        x86::R13D => RegId::R13D,
        x86::R14D => RegId::R14D,
        x86::R15D => RegId::R15D,
        x86::EFLAGS => EFLAGS_ID,
        _ => return None,
    })
}

/// The pin's register number of the register the builder names `id`
/// (`phys_reg` inverted); `None` where it has none.
pub fn pin_register(id: RegId) -> Option<u16> {
    // The inverse of `phys_reg`, made once: a scan of 200 numbers per query cost 81 K Ir
    // a call lowering (memory_expansion, 953 calls) and the same in selection.
    static INVERSE: std::sync::OnceLock<Vec<Option<u16>>> = std::sync::OnceLock::new();
    let table = INVERSE.get_or_init(|| {
        let mut table = vec![None; RegId::values().len()];
        for number in (0..=u16::MAX.min(200)).rev() {
            if let Some(found) = phys_reg(number) {
                table[found.index()] = Some(number);
            }
        }
        table
    });
    table.get(id.index()).copied().flatten()
}


#[cfg(test)]
mod tests {
    use super::*;

    /// A register the builder names maps to the pin's number and back, and the
    /// flags have one: a gap here sends a call's clobber mask or a copy to the
    /// wrong register.
    #[test]
    fn test_a_pin_number_and_its_register_map_each_other() {
        for number in 1..=200u16 {
            if let Some(register) = phys_reg(number) {
                assert_eq!(pin_register(register), Some(number), "pin register {number}");
            }
        }
        assert!(pin_register(EFLAGS_ID).is_some());
        assert_eq!(pin_register(RegId::EAX).and_then(phys_reg), Some(RegId::EAX));
    }
}
