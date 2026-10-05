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
