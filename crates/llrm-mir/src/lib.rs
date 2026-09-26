//! MIR: a subset of LLVM IR (`docs/architecture/rich-mir.md`). Its text is
//! LLVM's assembly language and its in-memory form LLVM's object model,
//! held in arenas addressed by id.

pub mod context;
pub mod lexer;
pub mod module;
pub mod opcode;
pub mod parse;
pub mod print;
pub mod types;

pub use context::{Constant, ConstantExpr, ConstantId, ConstantKind, Context, GlobalId};
pub use lexer::ParseError;
pub use module::{
    Block, BlockId, Function, GlobalKind, GlobalValue, GlobalVariable, InstId, Instruction, Linkage, MetadataId, MetadataNode, MetadataOperand,
    Module, Operand, UnnamedAddr, ValueData, ValueDef, ValueId,
};
pub use opcode::{Attribute, BinaryOp, CallInfo, CastOp, Clause, FloatPredicate, Flags, IntPredicate, Opcode, Tail};
pub use types::{FloatKind, Type, TypeId, Types};

#[cfg(test)]
mod tests;
