//! The pin's AArch64 register numbers as arm64 register ids, by name.

use llrm_codegen::backend::selectiondag::dagisel_adapter::PinRegisters;
use llrm_dagisel::generated::aarch64::ids::reg;
use llrm_lir::registers::RegId;

use crate::REGISTER_INFO;

/// The pin's names that `registers.regs` spells otherwise.
const ALIASES: &[(&str, &str)] = &[("FP", "x29"), ("LR", "x30")];

/// The flags register has no line in `registers.regs`; the shared stages
/// know the one id that stands for it (x86's `EFLAGS`, here `NZCV`).
const FLAGS: RegId = llrm_codegen::backend::selectiondag::dagisel_adapter::EFLAGS_ID;

pub struct Aarch64Ids {
    registers: Vec<(u16, RegId)>,
}

impl Aarch64Ids {
    /// Every register of `reg::REGISTERS` that the arm64 file has.
    pub fn new() -> Aarch64Ids {
        let named = |name: &str| (0..=u8::MAX).map(RegId::from_raw).find(|&id| REGISTER_INFO.name(id) == Some(name));
        let registers = reg::REGISTERS
            .iter()
            .filter_map(|&(pin, name)| {
                let ours = ALIASES.iter().find(|(from, _)| *from == name).map_or(name, |(_, to)| to);
                named(&ours.to_ascii_lowercase()).map(|id| (pin, id))
            })
            .chain([(reg::NZCV, FLAGS)])
            .collect();
        Aarch64Ids { registers }
    }
}

impl Default for Aarch64Ids {
    fn default() -> Self {
        Aarch64Ids::new()
    }
}

impl PinRegisters for Aarch64Ids {
    fn phys_reg(
        &self,
        pin: u16,
    ) -> Option<RegId> {
        self.registers.iter().find(|(number, _)| *number == pin).map(|&(_, id)| id)
    }

    fn pin_register(
        &self,
        id: RegId,
    ) -> Option<u16> {
        self.registers.iter().find(|(_, ours)| *ours == id).map(|&(pin, _)| pin)
    }

    /// Every number but `$noreg` and the registers of the file.
    fn unmapped(
        &self,
        pin: u16,
    ) -> bool {
        pin != 0 && self.phys_reg(pin).is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reg as ours;

    fn pin(name: &str) -> u16 {
        reg::REGISTERS.iter().find(|(_, n)| *n == name).unwrap().0
    }

    /// The pin's X0 is 239 and its W0 208; neither is the arm64 file's number,
    /// and an unmapped one used to read as virtual register N.
    #[test]
    fn pin_registers_map_by_name_and_a_stranger_is_unknown() {
        let ids = Aarch64Ids::new();
        assert_eq!(ids.phys_reg(pin("W0")), Some(ours::W0));
        assert_eq!(ids.phys_reg(pin("X1")), Some(ours::X1));
        assert_eq!(ids.phys_reg(pin("FP")), Some(ours::X29));
        assert_eq!(ids.phys_reg(pin("SP")), Some(ours::SP));
        assert!(!ids.unmapped(0));
        assert!(ids.unmapped(60000));
        assert_eq!(ids.pin_register(ours::W1), Some(pin("W1")));
        assert_eq!(ids.pin_register(ours::V3), None);
    }
}
