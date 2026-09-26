//! MIR: a subset of LLVM IR (`docs/architecture/rich-mir.md`). Its text is
//! LLVM's assembly language and its in-memory form LLVM's object model,
//! held in arenas addressed by id.

pub mod build;
pub mod context;
pub mod datalayout;
pub mod dominators;
pub mod edit;
pub mod interpret;
pub mod intrinsics;
pub mod lexer;
pub mod lint;
pub mod loops;
pub mod memory;
pub mod module;
pub mod opcode;
pub mod parse;
pub mod passes;
pub mod print;
pub mod scalarevolution;
pub mod transforms;
pub mod types;
pub mod valuetracking;
pub mod verify;

pub use edit::Position;
pub use context::{Constant, ConstantExpr, ConstantId, ConstantKind, Context, GlobalId};
pub use lexer::ParseError;
pub use module::{
    Block, BlockId, Change, Function, GlobalKind, GlobalValue, GlobalVariable, InstId, Instruction, Linkage, MetadataId, MetadataNode, MetadataOperand,
    Module, Operand, UnnamedAddr, Use, ValueData, ValueDef, ValueId,
};
pub use opcode::{Attribute, BinaryOp, CallInfo, CastOp, Clause, FloatPredicate, Flags, IntPredicate, Opcode, Tail};
pub use types::{FloatKind, Type, TypeId, Types};

#[cfg(test)]
mod build_tests;
#[cfg(test)]
mod edit_tests;
#[cfg(test)]
mod interpret_tests;
#[cfg(test)]
mod lint_tests;
#[cfg(test)]
mod pass_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod transforms_tests;
#[cfg(test)]
mod valuetracking_tests;
