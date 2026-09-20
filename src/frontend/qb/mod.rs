//! Source parser for the QB family.
//!
//! The lexer follows the recovered QBasic parser's source rules, but the
//! output is typed syntax.  There is deliberately no p-code buffer, opcode,
//! executor, or scanner patch phase in this crate.

pub mod dialect;
pub mod dialect_extensions;
pub mod error;
pub mod generated_parser;
mod hir_json;
pub mod intrinsics;
pub mod mc;
pub mod module_header;
pub mod object;
pub mod semantic;
pub mod source;
pub mod statement_mc;
pub mod statement_table;
pub mod syntax;

pub use dialect::Dialect;
pub use error::LexError;
pub use error::ParseError;
pub use generated_parser::parse;
pub use semantic::{SemanticError, compile, compile_hir};
pub use syntax::Module;
