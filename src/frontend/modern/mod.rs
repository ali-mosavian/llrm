//! Frontend for the provisional modern systems language.
//!
//! Syntax is private to this crate. Successful compilation crosses the
//! process boundary as typed, name-resolved common HIR.

pub mod compile;
pub mod conversions;
pub mod driver;
pub mod error;
pub mod hir;
pub mod lexer;
pub mod main;
pub mod modernstages;
pub mod parser;
pub mod semantic;
pub mod syntax;

#[cfg(test)]
pub(crate) mod test_modern_frontend;

pub use error::Diagnostic;
pub use lexer::lex;
pub use parser::parse;

pub fn compile(source: &str, module_name: &str) -> Result<String, Diagnostic> {
    let tokens = lex(source)?;
    let module = parse(tokens)?;
    semantic::compile(&module, module_name)
}
