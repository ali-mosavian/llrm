//! QB tokens for the LALR parser: the lexer's, as a terminal each.

use std::collections::HashMap;
use std::sync::OnceLock;

use qbfront::generated_parser::tables::TOKENS;
use qbfront::generated_parser::{Token, TokenKind};
use qbfront::syntax::Binary;

use crate::qb_kw::{Kw, NAMES};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tok {
    Kw(Kw),
    Ident,
    Int,
    Real,
    Str,
    Format,
    Period,
    Le,
    Ne,
    Ge,
    Ext,
    Dynamic,
    Static,
}

#[derive(Debug)]
pub enum Never {}

pub type Spanned = Result<(usize, Tok, usize), Never>;

fn by_name() -> &'static HashMap<&'static str, Kw> {
    static TABLE: OnceLock<HashMap<&'static str, Kw>> = OnceLock::new();
    TABLE.get_or_init(|| NAMES.iter().copied().collect())
}

pub fn terminal(kind: &TokenKind) -> Tok {
    match kind {
        TokenKind::Reserved(id) => {
            let (name, ..) = TOKENS[usize::from(*id)];
            Tok::Kw(by_name()[name])
        }
        TokenKind::ExtensionKeyword(_) => Tok::Ext,
        TokenKind::Identifier(_) => Tok::Ident,
        TokenKind::Integer(..) => Tok::Int,
        TokenKind::Real(..) => Tok::Real,
        TokenKind::String(_) => Tok::Str,
        TokenKind::FormatString(_) => Tok::Format,
        TokenKind::Comparison(Binary::LessEqual) => Tok::Le,
        TokenKind::Comparison(Binary::NotEqual) => Tok::Ne,
        TokenKind::Comparison(Binary::GreaterEqual) => Tok::Ge,
        TokenKind::Comparison(other) => unreachable!("{other:?} is no comparison token"),
        TokenKind::Period => Tok::Period,
        TokenKind::ArrayDynamic => Tok::Dynamic,
        TokenKind::ArrayStatic => Tok::Static,
    }
}

pub fn tokens(source: &[Token]) -> Vec<Spanned> {
    source.iter().enumerate().map(|(at, token)| Ok((at, terminal(&token.kind), at + 1))).collect()
}
