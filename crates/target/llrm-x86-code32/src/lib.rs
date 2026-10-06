//! The flat 32-bit x86 target: CS = DS = SS, base 0, 4 GB, one 32-bit pointer.
//! Its descriptions are the files beside this one; this answers what they
//! cannot say.

use iced_x86::Register::{self, EAX, EBP, EBX, EDI, EDX, ESI, ESP};
use std::collections::BTreeSet;
use std::rc::Rc;

use llrm_mir::target::{AddressForm, Machine as CostMachine, OperationCosts};
use llrm_target::machine::Machine;
use llrm_target::{CostModel, CpuPrices};

/// Flat DOS under an extender, and the PC ports it shares.
pub const DOS32: &str = concat!(include_str!("machines/dos32.toml"), include_str!("../../llrm-target/src/machines/pc-ports.toml"));

/// The processors flat code is priced for: those the descriptions know.
pub const CPUS: [&str; 8] = ["386", "486", "P5", "P6", "K5", "K6", "K7", "Core"];

/// The flat 32-bit x86 target as `llrm-driver` names it.
pub struct Code32;

/// Every GPR but the frame and stack registers holds a value (registers.regs).
const ALLOCATABLE: i64 = 6;

/// Any register is a base, any but ESP an index, scaled by 1, 2, 4 or 8, for nothing:
/// the only address form flat code has.
fn address_form() -> AddressForm {
    AddressForm::new(4, BTreeSet::from([1, 2, 4, 8]), 0, 0, 0, false, None).expect("no fallback to disagree")
}

/// What the passes ask of flat code on one CPU. The prices are unit ones until
/// the flat timings table (`timings.times`) is wired; its registers and its
/// address form are the target's.
struct Flat32 {
    registers: i64,
    call_registers: i64,
}

impl CostMachine for Flat32 {
    /// Flat memory is linear: no selector and offset reach foreign memory.
    fn foreign_span(&self, _: (i64, i64), _: (i64, i64), _: i64) -> Option<(i64, i64)> {
        None
    }

    fn costs(&self) -> OperationCosts {
        OperationCosts::default()
    }

    fn registers(&self) -> i64 {
        self.registers
    }

    fn call_registers(&self) -> i64 {
        self.call_registers
    }

    fn two_address(&self) -> bool {
        true
    }

    fn address_forms(&self) -> Vec<AddressForm> {
        vec![address_form()]
    }
}

fn cost_model(prices: &CpuPrices) -> Rc<dyn CostMachine> {
    Rc::new(Flat32 { registers: prices.registers, call_registers: prices.call_registers })
}

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

    fn register_capacity(&self) -> i64 {
        ALLOCATABLE
    }

    fn address_forms(&self, _: &OperationCosts, _: i64) -> Vec<AddressForm> {
        vec![address_form()]
    }

    fn cost_model(&self) -> CostModel {
        cost_model
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

    /// Flat code indexes by dwords natively: one form, no prefix, any scale.
    #[test]
    fn test_code32_has_one_native_dword_address_form() {
        let forms = Code32.address_forms(&OperationCosts::default(), 0);
        assert_eq!(forms.len(), 1);
        assert!(!forms[0].secondary && forms[0].index_width == 4 && forms[0].scales == BTreeSet::from([1, 2, 4, 8]));
        let model = (Code32.cost_model())(&CpuPrices { costs: Vec::new(), prefix: 1, address_stall: 0, registers: Code32.register_capacity(), call_registers: Code32.callee_saved().len() as i64 });
        assert_eq!((model.registers(), model.call_registers()), (6, 3));
        assert_eq!(model.address_forms(), forms);
    }

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
