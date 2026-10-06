//! The 16-bit x86 target: what its instructions cost, as `Dos` prices them
//! for the passes, and the machine description a program is built for.

pub mod cycles;
pub mod instructions;
pub mod machine;
pub mod target;
pub mod timings;

pub use cycles::report;
pub use target::{Dos, ENCODABLE_BASES, FRAME, GENERAL, PRESERVED, WORD_INDEXES, word_bases};
pub use timings::ARCHS;

/// Real mode's data layout and address spaces: `machines/datalayout.toml`.
pub const DATALAYOUT_TOML: &str = include_str!("machines/datalayout.toml");

/// `DATALAYOUT_TOML`, parsed.
pub fn layout() -> llrm_target::layout::Layout {
    llrm_target::layout::Layout::parse(DATALAYOUT_TOML).expect("real mode's datalayout.toml parses")
}

/// The 16-bit x86 target as `llrm-driver` names it.
pub struct Code16;

fn cost_model(prices: &llrm_target::CpuPrices) -> std::rc::Rc<dyn llrm_mir::target::Machine> {
    std::rc::Rc::new(Dos::priced(&prices.costs, prices.prefix, prices.address_stall, prices.registers, prices.call_registers))
}

impl llrm_target::Target for Code16 {
    fn register_capacity(&self) -> i64 {
        GENERAL.len() as i64
    }

    fn address_forms(&self, costs: &llrm_mir::target::OperationCosts, address_stall: i64) -> Vec<llrm_mir::target::AddressForm> {
        target::address_forms(costs, address_stall)
    }

    fn cost_model(&self) -> llrm_target::CostModel {
        cost_model
    }

    fn name(&self) -> &'static str {
        "x86-code16"
    }

    fn machine(&self) -> machine::Machine {
        machine::BUILT_IN.clone()
    }

    fn cpus(&self) -> &'static [&'static str] {
        &machine::CPUS
    }

    fn layout(&self) -> llrm_target::layout::Layout {
        layout()
    }

    /// A byte is pushed as a word.
    fn stack_slot_bytes(&self) -> i64 {
        2
    }

    fn frame_register(&self) -> iced_x86::Register {
        FRAME
    }

    /// Past BP and a 2-byte return address, or a 4-byte far one.
    fn first_argument_offset(&self, far: bool) -> i64 {
        if far { 6 } else { 4 }
    }

    fn listing_header(&self) -> Vec<String> {
        vec![".model medium".to_owned(), ".386".to_owned()]
    }

    fn stack_pointer(&self) -> iced_x86::Register {
        iced_x86::Register::SP
    }

    /// A Borland caller keeps SI and DI, not their upper halves.
    fn callee_saved(&self) -> Vec<(iced_x86::Register, iced_x86::Register)> {
        PRESERVED.to_vec()
    }

    /// A dword leaves in DX:AX and an i64 in EDX:EAX, so both name EAX and EDX.
    fn results(&self, width: u32) -> Vec<iced_x86::Register> {
        use iced_x86::Register::{EAX, EDX};
        if matches!(width, 4 | 8) { vec![EAX, EDX] } else { vec![EAX] }
    }
}

#[cfg(test)]
mod tests {
    use iced_x86::Register::{EAX, EDX};
    use llrm_target::Target;

    use super::*;

    /// isel read these as literals: a 2-byte slot, BP, the first argument at
    /// [bp+4] (near) or [bp+6] (far), a dword result in DX:AX and an i64 in EDX:EAX.
    #[test]
    fn test_code16_answers_the_literals_isel_had() {
        assert_eq!((Code16.stack_slot_bytes(), Code16.frame_register()), (2, iced_x86::Register::BP));
        assert_eq!((Code16.first_argument_offset(false), Code16.first_argument_offset(true)), (4, 6));
        assert_eq!(Code16.listing_header(), [".model medium", ".386"]);
        assert_eq!(Code16.stack_pointer(), iced_x86::Register::SP);
        assert_eq!(Code16.callee_saved(), PRESERVED.to_vec());
        assert_eq!([1, 2, 4, 8].map(|width| Code16.results(width)), [vec![EAX], vec![EAX], vec![EAX, EDX], vec![EAX, EDX]]);
    }
}
