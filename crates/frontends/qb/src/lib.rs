//! Source parser for the QB family.
//!
//! The lexer follows the recovered QBasic parser's source rules, but the
//! output is typed syntax.  There is deliberately no p-code buffer, opcode,
//! executor, or scanner patch phase in this crate.

pub mod dialect;
pub mod error;
pub mod front;
pub mod hir_driver;
pub mod intrinsics;
pub mod semantic;
pub mod source;
pub mod syntax;

pub use dialect::Dialect;
pub use error::LexError;
pub use error::ParseError;
pub use front::parse;
pub use llrm_codegen::abi::qb as abi;
pub use semantic::SemanticError;
pub use semantic::compile;
pub use syntax::Module;
pub mod arm64;
pub mod cli;
pub mod compile;
pub mod driver;
pub mod qbstages;
mod zero_fill;

#[cfg(test)]
mod test_hir;

#[cfg(test)]
mod test_hir_part1;

#[cfg(test)]
mod test_hir_part2;

#[cfg(test)]
mod test_hir_part3;
#[cfg(test)]
mod test_resume;

#[cfg(test)]
mod test_quickr;

#[cfg(test)]
mod test_frames;

#[cfg(test)]
mod test_stack_check;

#[cfg(test)]
mod test_intrinsics;

#[cfg(test)]
mod test_inline_runtime;

#[cfg(test)]
mod test_debug;

#[cfg(test)]
mod test_runtime_model;

#[cfg(test)]
mod callconv_tests;
#[cfg(test)]
mod test_lines;
#[cfg(test)]
mod test_pipeline;
