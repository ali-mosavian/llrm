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
}
