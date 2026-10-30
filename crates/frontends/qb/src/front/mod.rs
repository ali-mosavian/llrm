//! The QB family's front: the scanner, the typed parser and the checker that
//! holds it to what the real BC (QB 4.5, PDS 7.1, VBDOS) accepts. Calibrated
//! against BC itself; see crates/frontends/qb/oracle.

pub mod ast;
pub mod check;
pub mod classes;
pub mod explain;
pub mod kw;
mod kw_data;
pub mod lines;
pub mod quickr;
pub mod reserved;
pub mod scan;
pub mod sigs;
pub mod token;
pub mod tokens;
pub mod words;

use crate::{Dialect, Module, ParseError};

/// A source parsed and held to what the real BC (QB 4.5, PDS 7.1, VBDOS)
/// accepts: the first thing BC refuses is the error, worded for the person who
/// wrote it.
pub fn parse(
    source: &str,
    dialect: Dialect,
) -> Result<Module, ParseError> {
    // QuickrBASIC is held to no BC: its programs are parsed, and its own rules
    // are the semantic pass's
    if dialect.format_strings() {
        return ast::parse(source, dialect).map_err(|error| explain::explain(error, source));
    }
    parse_coded(source, dialect).map_err(|error| refused(error, source, dialect))
}

/// `parse` with the error left as BC names it: for the tools that hold the
/// front to BC's verdicts.
pub fn parse_coded(
    source: &str,
    dialect: Dialect,
) -> Result<Module, ParseError> {
    let module = ast::parse(source, dialect)?;
    match check::check(&module, dialect).into_iter().next() {
        Some(diagnostic) => Err(ParseError {
            subject: diagnostic.subject,
            detail: diagnostic.detail,
            headline: diagnostic.headline,
            related: diagnostic.related,
            ..ParseError::coded(diagnostic.span, diagnostic.message)
        }),
        None => Ok(module),
    }
}

/// The error worded, and where a program is valid for another dialect, which
/// one.
fn refused(
    error: ParseError,
    source: &str,
    dialect: Dialect,
) -> ParseError {
    let mut error = explain::explain(error, source);
    let others: Vec<Dialect> = [Dialect::QuickBasic45, Dialect::Pds71, Dialect::VbDos]
        .into_iter()
        .filter(|other| *other != dialect && parse_coded(source, *other).is_ok())
        .collect();
    if !others.is_empty() {
        let names: Vec<String> = others
            .iter()
            .map(|other| format!("{} (--dialect {})", explain::dialect_name(*other), explain::dialect_flag(*other)))
            .collect();
        explain::add_help(&mut error, format!("this source is valid for {}", names.join(" and ")));
    } else if matches!(
        error.code.as_str(),
        "Syntax error" | "User-defined type not defined" | "Formal parameter specification illegal"
    ) && ast::parse(source, Dialect::Quickr).is_ok()
    {
        explain::add_help(&mut error, "the quickr profile (--dialect quickr) takes this source".to_owned());
    }
    error
}
