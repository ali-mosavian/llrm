//! MIR: a subset of LLVM IR (`docs/architecture/rich-mir.md`). Its text is
//! LLVM's assembly language and its in-memory form LLVM's object model,
//! held in arenas addressed by id.

pub mod alias;
pub mod apint;
pub mod blocklist;
pub mod build;
pub mod callgraph;
pub mod constantrange;
pub mod constfold;
pub mod context;
pub mod counts;
pub mod datalayout;
pub mod debuginfo;
pub mod dense;
pub mod depends;
pub mod domtree;
pub mod domtree_updater;
pub mod dumpdir;
pub mod edit;
pub mod facts;
#[cfg(test)]
mod facts_coverage;
mod facts_rewrite;
pub mod footprint;
pub mod forest;
pub mod hash;
pub mod inline;
pub mod interpret;
pub mod intrinsics;
pub mod knownbits;
pub mod lexer;
pub mod lint;
pub mod loopinfo;
pub mod memory;
pub mod module;
pub mod opcode;
pub mod parse;
pub mod passes;
pub mod print;
pub mod program;
pub mod scalarevolution;
pub mod simplify;
pub mod spaces;
pub mod splice;
pub mod target;
pub mod tbaa;
pub mod touched;
pub mod types;
pub mod valuetracking;
pub mod verify;

pub use context::{Constant, ConstantExpr, ConstantId, ConstantKind, Context, GlobalId};
pub use edit::Position;
pub use lexer::ParseError;
pub use module::{
    Block, BlockId, Change, DebugRecord, DebugWhat, Function, GlobalKind, GlobalValue, GlobalVariable, InstId,
    Instruction, Linkage, MetadataId, MetadataNode, MetadataOperand, Module, Operand, UnnamedAddr, Use, ValueData,
    ValueDef, ValueId,
};
pub use opcode::{Attribute, BinaryOp, CallInfo, CastOp, Clause, Flags, FloatPredicate, IntPredicate, Opcode, Tail};
pub use types::{FloatKind, Type, TypeId, Types};

#[cfg(test)]
mod build_tests;
#[cfg(test)]
mod constantrange_tests;
#[cfg(test)]
mod edit_tests;
#[cfg(test)]
mod interpret_tests;
#[cfg(test)]
mod knownbits_tests;
#[cfg(test)]
mod lint_tests;
#[cfg(test)]
mod loopinfo_tests;
#[cfg(test)]
mod pass_tests;
#[cfg(test)]
mod program_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod touched_tests;
#[cfg(test)]
mod valuetracking_tests;
