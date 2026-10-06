//! The flat 32-bit x86 target: CS = DS = SS, base 0, 4 GB, one 32-bit pointer.
//! Its descriptions are the files beside this one; this answers what they
//! cannot say.

use iced_x86::Register::{self, EAX, EBP, EBX, EDI, EDX, ESI, ESP};
use llrm_target::machine::Machine;

/// Flat DOS under an extender, and the PC ports it shares.
pub const DOS32: &str = concat!(include_str!("machines/dos32.toml"), include_str!("../../llrm-target/src/machines/pc-ports.toml"));

/// The processors flat code is priced for: those the descriptions know.
pub const CPUS: [&str; 8] = ["386", "486", "P5", "P6", "K5", "K6", "K7", "Core"];

/// The flat 32-bit x86 target as `llrm-driver` names it.
pub struct Code32;

impl llrm_target::Target for Code32 {
    fn name(&self) -> &'static str {
        "x86-code32"
    }

    fn machine(&self) -> Machine {
        Machine::parse(DOS32, &CPUS).expect("the flat DOS description parses")
    }

    fn cpus(&self) -> &'static [&'static str] {
        &CPUS
    }

    /// An argument takes a dword at least.
    fn stack_slot_bytes(&self) -> i64 {
        4
    }

    fn frame_register(&self) -> Register {
        EBP
    }

    /// Past EBP and the 4-byte return address: [ebp+8]; there is no far call.
    fn first_argument_offset(&self, _far: bool) -> i64 {
        8
    }

    fn stack_pointer(&self) -> Register {
        ESP
    }

    /// cdecl32 keeps EBX, ESI and EDI whole.
    fn callee_saved(&self) -> Vec<(Register, Register)> {
        vec![(EBX, EBX), (ESI, ESI), (EDI, EDI)]
    }

    /// A dword leaves in EAX and an i64 in EDX:EAX.
    fn results(&self, width: u32) -> Vec<Register> {
        if width == 8 { vec![EAX, EDX] } else { vec![EAX] }
    }
}

#[cfg(test)]
mod tests {
    use llrm_target::Target;

    use super::*;

    #[test]
    fn test_dos32_is_a_flat_machine_with_the_pc_ports() {
        let machine = Code32.machine();
        assert_eq!(machine.addressing, llrm_target::machine::Addressing::Flat);
        assert!(machine.segments.is_none());
        assert!(!machine.ports.is_empty());
    }

    /// cdecl32 (calling.toml): EBP and ESP frame, EBX/ESI/EDI kept whole, first argument at [ebp+8].
    #[test]
    fn test_code32_answers_cdecl32() {
        let frame = Code32.frame_registers();
        assert_eq!((frame.pointer, frame.stack), (EBP, ESP));
        assert_eq!(frame.saved, [(EBX, EBX), (ESI, ESI), (EDI, EDI)]);
        assert_eq!((Code32.stack_slot_bytes(), Code32.first_argument_offset(false)), (4, 8));
        assert_eq!([4, 8].map(|width| Code32.results(width)), [vec![EAX], vec![EAX, EDX]]);
    }
}
