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

/// The 16-bit x86 target as `llrm-driver` names it.
pub struct Code16;

impl llrm_target::Target for Code16 {
    fn name(&self) -> &'static str {
        "x86-code16"
    }

    fn machine(&self) -> machine::Machine {
        machine::BUILT_IN.clone()
    }

    fn cpus(&self) -> &'static [&'static str] {
        &machine::CPUS
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
        assert_eq!([1, 2, 4, 8].map(|width| Code16.results(width)), [vec![EAX], vec![EAX], vec![EAX, EDX], vec![EAX, EDX]]);
    }
}
