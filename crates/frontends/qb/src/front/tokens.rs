//! QB tokens for the LALR parser: the lexer's, as a terminal each.

use crate::front::kw::Kw;
use crate::front::token::{Token, TokenKind};
use crate::syntax::Binary;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
    Form,
    /// `FN` written apart from the name it prefixes: `DEF FN twice`, `FN
    /// twice(4)`.
    FnWord,
}

/// What the lexer can fail with, when the parser pulls tokens from it.
pub type Failure = crate::LexError;

pub type Spanned = Result<(usize, Tok, usize), Failure>;

/// `type$`, `key%`: the lexer takes a suffixed word for its bare keyword; the
/// suffix makes it a name.
pub fn suffixed(
    word: Kw,
    width: usize,
) -> bool {
    let spelling = word.spelling();
    spelling.starts_with(|c: char| c.is_ascii_alphabetic()) && width == spelling.len() + 1
}

pub fn terminal(kind: &TokenKind) -> Tok {
    match kind {
        TokenKind::Reserved(word) => Tok::Kw(*word),
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
        TokenKind::FormDeclaration(_) => Tok::Form,
    }
}

pub fn tokens(source: &[Token]) -> Vec<Spanned> {
    let name = |at: usize| matches!(
        source.get(at).map(|token| &token.kind),
        Some(TokenKind::Identifier(_))
    );
    source
        .iter()
        .enumerate()
        .map(|(at, token)| {
            let apart = matches!(&token.kind, TokenKind::Identifier(word) if word == "FN") && name(at + 1);
            // `type$`, `key%`: the lexer takes a suffixed word for its bare
            // keyword; the suffix makes it a name
            let suffixed = matches!(
                &token.kind,
                TokenKind::Reserved(word) if suffixed(*word, token.span.end - token.span.start)
            );
            Ok((
                at,
                if apart {
                    Tok::FnWord
                } else if suffixed {
                    Tok::Ident
                } else {
                    terminal(&token.kind)
                },
                at + 1,
            ))
        })
        .collect()
}
