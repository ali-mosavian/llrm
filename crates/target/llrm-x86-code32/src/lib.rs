//! The flat 32-bit x86 target: CS = DS = SS, base 0, 4 GB, one 32-bit pointer.
//! Its descriptions are the files beside this one; this answers what they
//! cannot say.

use iced_x86::Register::{self, EAX, EBP, EBX, EDI, EDX, ESI, ESP};
use std::sync::LazyLock;

use llrm_mir::target::{AddressForm, OperationCosts};
use llrm_target::machine::Machine;
use llrm_target::CostModel;

/// Flat DOS under an extender, and the PC ports it shares.
pub const DOS32: &str = concat!(include_str!("machines/dos32.toml"), include_str!("../../llrm-target/src/machines/pc-ports.toml"));

/// The processors flat code is priced for: those `timings.times` has.
pub const CPUS: [&str; 2] = ["486", "P5"];

/// The data layout and address spaces: `machines/datalayout.toml`.
pub const DATALAYOUT_TOML: &str = include_str!("machines/datalayout.toml");

/// The flat 32-bit x86 target as `llrm-driver` names it.
pub struct Code32;

/// The registers and the address forms the descriptions state.
static REGISTERS: LazyLock<Vec<llrm_target::registers::Register>> = LazyLock::new(|| llrm_target::registers::parse(include_str!("registers.regs")).expect("registers.regs parses"));

/// How flat operations are priced from the instruction forms: `opcosts.txt`.
static OPCOSTS: LazyLock<llrm_target::opcosts::Description> = LazyLock::new(|| llrm_target::opcosts::Description::parse(include_str!("opcosts.txt")).expect("opcosts.txt parses"));

/// The CPUs' timings: `timings.times`.
static TIMINGS: LazyLock<llrm_target::timings::Timings> = LazyLock::new(|| llrm_target::timings::Timings::parse(include_str!("timings.times")).expect("timings.times parses"));

static ADDRESS_FORMS: LazyLock<Vec<AddressForm>> = LazyLock::new(|| llrm_target::addressing::forms(include_str!("machines/datalayout.toml")).expect("datalayout.toml parses"));

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

    fn layout(&self) -> llrm_target::layout::Layout {
        llrm_target::layout::Layout::parse(DATALAYOUT_TOML).expect("flat datalayout.toml parses")
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

    /// Flat: no segments, one model.
    fn object(&self) -> llrm_target::object::ObjectFormat {
        llrm_target::object::ObjectFormat::parse(include_str!("machines/object.toml")).expect("flat object.toml parses")
    }

    fn stack_pointer(&self) -> Register {
        ESP
    }

    /// cdecl32 keeps EBX, ESI and EDI whole.
    fn callee_saved(&self) -> Vec<(Register, Register)> {
        vec![(EBX, EBX), (ESI, ESI), (EDI, EDI)]
    }

    fn registers_text(&self) -> String {
        include_str!("registers.regs").to_owned()
    }

    fn forms_text(&self) -> String {
        llrm_x86::instructions::joined(include_str!("instructions/x86.instr"))
    }

    fn default_cpu(&self) -> &'static str {
        TIMINGS.default_cpu().expect("timings.times states a default CPU")
    }

    fn cpu_table(&self, name: &str) -> Option<llrm_target::timings::CpuTable> {
        TIMINGS.cpu(name).cloned()
    }

    fn operation_costs(&self, price: &dyn Fn(&str) -> i64, prefix: i64) -> OperationCosts {
        OPCOSTS.operations(price, prefix)
    }

    fn register_capacity(&self) -> i64 {
        llrm_target::registers::allocatable(&REGISTERS) as i64
    }

    fn address_forms(&self, _: &OperationCosts, _: i64) -> Vec<AddressForm> {
        ADDRESS_FORMS.clone()
    }

    fn cost_model(&self) -> CostModel {
        llrm_target::described
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
        assert!(!forms[0].secondary && forms[0].index_width == 4 && forms[0].scales == std::collections::BTreeSet::from([1, 2, 4, 8]));
        let model = (Code32.cost_model())(&llrm_target::CpuPrices { costs: Vec::new(), prefix: 1, address_stall: 0, registers: Code32.register_capacity(), call_registers: Code32.callee_saved().len() as i64, address_forms: forms.clone(), operations: OperationCosts::default() });
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

    /// One address space of 32-bit pointers: `near` and `far` are space 0, an unmarked
    /// dword pointer is near, and the pair kinds are none.
    #[test]
    fn test_code32_layout_is_one_32_bit_space() {
        let layout = Code32.layout();
        assert!(layout.datalayout.starts_with("e-p:32:32"));
        let spaces = layout.spaces;
        assert_eq!((spaces.near, spaces.far, spaces.segment, spaces.huge, spaces.fixed, spaces.unmarked(4)), (0, 0, None, None, None, Ok(0)));
    }

    /// cdecl32 (calling.toml): EBP and ESP frame, EBX/ESI/EDI kept whole, first argument at [ebp+8].
    #[test]
    fn test_code32_answers_cdecl32() {
        let frame = Code32.frame_registers();
        assert_eq!((Code32.object().bitness, Code32.object().header), (32, vec![".386".to_owned(), ".model flat".to_owned()]));
        assert_eq!((frame.pointer, frame.stack), (EBP, ESP));
        assert_eq!(frame.saved, [(EBX, EBX), (ESI, ESI), (EDI, EDI)]);
        assert_eq!((Code32.stack_slot_bytes(), Code32.first_argument_offset(false)), (4, 8));
        assert_eq!([4, 8].map(|width| Code32.results(width)), [vec![EAX], vec![EAX, EDX]]);
    }
}
