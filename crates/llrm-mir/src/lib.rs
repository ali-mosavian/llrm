//! Rich portable MIR (`docs/architecture/rich-mir.md`), step 1: the typed shell.
//!
//! Interned types, typed SSA values and constants, explicit parameters and
//! returns, CFG edges owned by terminators, and the integer family -- with the
//! verifier, the text form and an interpreter for that slice. Nothing produces
//! it yet; the old MIR in `llrm-core` stays the compiler's until raising does.

pub mod function;
pub mod interpret;
pub mod opcode;
pub mod parse;
pub mod print;
pub mod types;
pub mod verify;

pub use function::{Block, BlockId, Constant, Edge, EdgeId, Function, Instruction, InstructionId, Module, Operand, ValueId, ValueInfo};
pub use opcode::{Opcode, Overflow, Predicate};
pub use print::View;
pub use types::{FloatFormat, MirContext, Type, TypeId};

#[cfg(test)]
mod tests;
