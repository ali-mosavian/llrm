//! Spike: an LALR(1) parser for Nib, and the oracle tools for the QB front.

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
        .map_err(
            |error| {
                let at = |location: usize| tokens.get(location).map(|t| (t.span.line, t.span.column, &t.kind));
                match error {
                    lalrpop_util::ParseError::UnrecognizedToken { token: (start, ..), expected } => {
                        format!("unexpected {:?}, expected one of {}", at(start), expected.len())
                    }
                    lalrpop_util::ParseError::UnrecognizedEof { location, .. } => format!("eof at {:?}", at(location)),
                    other => format!("{other:?}"),
                }
            },
        )
}

pub fn parse_nib_source(source: &str) -> Result<ast::Module, String> {
    let tokens = lex(source).map_err(|d| format!("lex: {d:?}"))?;
    parse_nib(&tokens)
}

pub use qbfront::front::ast as qb_ast;
pub use qbfront::front::check as qb_check;

/// A DOS source file as text: no byte order mark, and no Ctrl-Z, which ends a
/// DOS text file and which QB ignores.
pub fn dos_source(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let text: Vec<u8> = bytes.iter().copied().filter(|&byte| byte != 0x1A).collect();
    String::from_utf8_lossy(&text).into_owned()
}
