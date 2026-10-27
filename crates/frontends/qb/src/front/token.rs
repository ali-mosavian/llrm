//! The tokens the scanner makes and the parser reads.

use crate::front::kw::Kw;
use crate::syntax::{Binary, Span};

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Reserved(Kw),
    ExtensionKeyword(&'static str),
    Identifier(String),
    Integer(i64, Option<char>),
    Real(String, Option<char>),
    String(String),
    /// QuickrBASIC's `f"…"`.
    FormatString(Vec<FormatSegment>),
    Comparison(Binary),
    Period,
    ArrayDynamic,
    ArrayStatic,
    /// VBDOS's `'$FORM name` metacommand.
    FormDeclaration(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum FormatSegment {
    Text(String),
    /// `{expression}` or `{expression:spec}`, the expression already lexed.
    Field {
        tokens: Vec<Token>,
        spec: Option<String>,
        span: Span,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}
