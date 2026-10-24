//! Generated QBasic grammar frontend.
//!
//! The table bytes come from the vendored `qbasbnf.prs`; this module adapts
//! their actions to [`crate::syntax`] and contains no p-code representation.

pub(crate) mod ast;
mod engine;
mod lexer;
pub mod tables;

pub use ast::parse;
pub use ast::parse_vertical_slice;
pub use ast::ParseOutput;
pub use ast::FORMAT_FIELD;
pub use ast::{AUGMENTED, EACH, ON_SELECTOR, STEP_MARK, TUPLE};
pub use engine::ParseResult;
pub(crate) use lexer::format_string;
pub use lexer::lex;
pub use lexer::FormatSegment;
pub use lexer::LexError;
pub use lexer::Token;
pub use lexer::TokenKind;
pub use tables::AstAction;
