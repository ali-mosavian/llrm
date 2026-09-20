//! x86 legality, registers, ABI, selection, timing, and encoding hooks.

mod instructions;
mod registers;

pub use instructions::{ComparisonKind, ConditionCode, OperandSize, X86Opcode};
pub use registers::{X86Register, X86RegisterClass};
