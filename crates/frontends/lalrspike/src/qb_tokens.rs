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
    /// `FN` written apart from the name it prefixes: `DEF FN twice`, `FN twice(4)`.
    FnWord,
}

/// What the lexer can fail with, when the parser pulls tokens from it.
pub type Failure = qbfront::LexError;

pub type Spanned = Result<(usize, Tok, usize), Failure>;

/// The terminal of each reserved id.
fn reserved() -> &'static [Option<Tok>] {
    static TABLE: OnceLock<Vec<Option<Tok>>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let by_name: HashMap<&str, Kw> = NAMES.iter().copied().collect();
        TOKENS.iter().map(|(name, ..)| by_name.get(name).map(|&kw| Tok::Kw(kw))).collect()
    })
}

/// Whether a word is a reserved word's spelling, so that a longer span is its suffixed name.
fn alphabetic(id: u16) -> Option<usize> {
    static TABLE: OnceLock<Vec<Option<usize>>> = OnceLock::new();
    TABLE.get_or_init(|| {
        TOKENS.iter().map(|(_, spelling, ..)| spelling.starts_with(|c: char| c.is_ascii_alphabetic()).then_some(spelling.len())).collect()
    })[usize::from(id)]
}

/// `type$`, `key%`: the lexer takes a suffixed word for its bare keyword; the suffix makes it a name.
pub fn suffixed(id: u16, width: usize) -> bool {
    alphabetic(id).is_some_and(|spelling| width == spelling + 1)
}

pub fn terminal(kind: &TokenKind) -> Tok {
    match kind {
        TokenKind::Reserved(id) => {
            reserved()[usize::from(*id)].clone().expect("every reserved word has a terminal")
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
    let name = |at: usize| matches!(source.get(at).map(|token| &token.kind), Some(TokenKind::Identifier(_)));
    source
        .iter()
        .enumerate()
        .map(|(at, token)| {
            let apart = matches!(&token.kind, TokenKind::Identifier(word) if word == "FN") && name(at + 1);
            // `type$`, `key%`: the lexer takes a suffixed word for its bare keyword; the suffix makes it a name
            let suffixed = matches!(&token.kind, TokenKind::Reserved(id) if suffixed(*id, token.span.end - token.span.start));
            Ok((at, if apart { Tok::FnWord } else if suffixed { Tok::Ident } else { terminal(&token.kind) }, at + 1))
        })
        .collect()
}
