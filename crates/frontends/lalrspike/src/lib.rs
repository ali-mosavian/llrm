//! Spike: LALR(1) parsers for Nib and the QB family.

pub mod ast;
pub mod nib_tokens;

#[allow(clippy::all, unused)]
pub mod nib {
    lalrpop_util::lalrpop_mod!(pub grammar, "/nib.rs");
}

use llrm_nib::lexer::{Token, lex};

/// A syntax error of the LALR parser, as the index of the token it stopped at.
pub fn parse_nib(tokens: &[Token]) -> Result<ast::Module, String> {
    nib::grammar::ModuleParser::new()
        .parse(nib_tokens::tokens(tokens))
        .map_err(|error| {
            let at = |location: usize| tokens.get(location).map(|t| (t.span.line, t.span.column, &t.kind));
            match error {
                lalrpop_util::ParseError::UnrecognizedToken { token: (start, ..), expected } => {
                    format!("unexpected {:?}, expected one of {}", at(start), expected.len())
                }
                lalrpop_util::ParseError::UnrecognizedEof { location, .. } => format!("eof at {:?}", at(location)),
                other => format!("{other:?}"),
            }
        })
}

pub fn parse_nib_source(source: &str) -> Result<ast::Module, String> {
    let tokens = lex(source).map_err(|d| format!("lex: {d:?}"))?;
    parse_nib(&tokens)
}

pub mod qb_kw;
pub mod qb_tokens;
pub mod qb_tree;

#[allow(clippy::all, unused)]
pub mod qb {
    lalrpop_util::lalrpop_mod!(pub grammar, "/qb.rs");
}

/// The QB token stream parsed to a generic tree, or where it stopped.
pub fn parse_qb(tokens: &[qbfront::generated_parser::Token]) -> Result<qb_tree::Tree, String> {
    qb::grammar::ProgramParser::new().parse(qb_tokens::tokens(tokens)).map_err(|error| match error {
        lalrpop_util::ParseError::UnrecognizedToken { token: (at, ..), expected } => {
            let token = &tokens[at];
            format!("{}:{} unexpected {:?}, expected {}", token.span.line, token.span.start, token.kind, expected.join(" "))
        }
        lalrpop_util::ParseError::UnrecognizedEof { expected, .. } => format!("eof, expected {}", expected.join(" ")),
        other => format!("{other:?}"),
    })
}
