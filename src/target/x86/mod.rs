//! x86 legality, registers, ABI, selection, timing, and encoding hooks.

mod allocation;
mod instructions;
mod mc;
mod registers;

pub use allocation::{X86AllocationError, allocate_registers};
pub use instructions::{ComparisonKind, ConditionCode, OperandSize, X86Opcode};
pub use mc::{McLowerError, UnresolvedOperand, lower_instruction};
pub use registers::{X86Register, X86RegisterClass};
