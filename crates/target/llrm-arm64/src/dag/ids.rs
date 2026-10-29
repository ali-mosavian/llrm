//! The pin's AArch64 register numbers as arm64 register ids, by name.

use llrm_codegen::backend::selectiondag::dagisel_adapter::{PinRegister, TargetIds};
use llrm_dagisel::generated::aarch64::ids::reg;
use llrm_lir::registers::RegId;

use crate::REGISTER_INFO;

/// The pin's names that `registers.regs` spells otherwise.
const ALIASES: &[(&str, &str)] = &[("FP", "x29"), ("LR", "x30")];

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
            .collect();
        Aarch64Ids { registers }
    }
}

impl Default for Aarch64Ids {
    fn default() -> Self {
        Aarch64Ids::new()
    }
}

impl TargetIds for Aarch64Ids {
    fn register(
        &self,
        pin: u16,
    ) -> PinRegister {
        if pin == 0 {
            return PinRegister::Virtual;
        }
        self.registers.iter().find(|(number, _)| *number == pin).map_or(PinRegister::Unknown, |&(_, id)| PinRegister::Physical(id))
    }

    fn pin_register(
        &self,
        id: RegId,
    ) -> Option<u16> {
        self.registers.iter().find(|(_, ours)| *ours == id).map(|&(pin, _)| pin)
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
        assert_eq!(ids.register(pin("W0")), PinRegister::Physical(ours::W0));
        assert_eq!(ids.register(pin("X1")), PinRegister::Physical(ours::X1));
        assert_eq!(ids.register(pin("FP")), PinRegister::Physical(ours::X29));
        assert_eq!(ids.register(pin("SP")), PinRegister::Physical(ours::SP));
        assert_eq!(ids.register(0), PinRegister::Virtual);
        assert_eq!(ids.register(60000), PinRegister::Unknown);
        assert_eq!(ids.pin_register(ours::W1), Some(pin("W1")));
        assert_eq!(ids.pin_register(ours::V3), None);
    }
}
