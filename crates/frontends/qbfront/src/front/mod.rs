//! The QB family's front: the scanner, the typed parser and the checker that holds it to what the
//! real BC (QB 4.5, PDS 7.1, VBDOS) accepts. Calibrated against BC itself; see crates/frontends/lalrspike.

pub mod ast;
pub mod check;
pub mod classes;
pub mod kw;
pub mod lexer;
pub mod reserved;
pub mod scan;
pub mod sigs;
pub mod tokens;
pub mod words;

use crate::{Dialect, Module, ParseError};

/// A source parsed and held to what the real BC accepts: the first thing BC refuses is the error.
/// QuickrBASIC's extensions are not in this front yet, so that dialect still goes to the grammar-table
/// parser, which has no checker.
pub fn parse(source: &str, dialect: Dialect) -> Result<Module, ParseError> {
    if dialect.format_strings() {
        return crate::generated_parser::parse(source, dialect);
    }
    let module = ast::parse(source, dialect)?;
    match check::check(&module, dialect).into_iter().next() {
        Some(diagnostic) => Err(ParseError { span: diagnostic.span, message: diagnostic.message }),
        None => Ok(module),
    }
}
