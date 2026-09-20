//! Generated QBasic grammar frontend.
//!
//! The table bytes come from the vendored `qbasbnf.prs`; this module adapts
//! their actions to [`crate::frontend::qb::syntax`] and contains no p-code representation.

mod ast;
mod engine;
mod lexer;
mod state;
pub mod tables;

pub use ast::ParseOutput;
pub use ast::parse;
pub use ast::parse_vertical_slice;
pub use engine::ParseResult;
pub use lexer::LexError;
pub use lexer::Token;
pub use lexer::TokenKind;
pub use lexer::lex;
pub use tables::AstAction;
