//! Frontend diagnostics shared by the scanner and parser implementations.

use crate::syntax::Span;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LexError {
    pub span: Span,
    pub message: String,
}

/// Why a source is refused.
///
/// `code` is the name BC gives the error ("Duplicate definition"), which the
/// oracle tools compare; `message` says in plain words what is wrong, `help`
/// what to do about it, and `expected` what the parser wanted where it stopped.
/// A stage that only knows the code leaves `message` equal to it, and
/// `front::explain` words it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseError {
    pub span: Span,
    pub code: String,
    pub message: String,
    pub help: Option<String>,
    pub expected: Option<String>,
    /// The name the error is about, when it is one word of a longer span.
    pub subject: Option<String>,
    /// What was found and what was wanted, when the code alone would not say.
    pub detail: Option<String>,
    /// A sentence in place of the usual one, where the checker knows more.
    pub headline: Option<String>,
    /// Another place in the source the message talks about.
    pub related: Option<Span>,
}

impl ParseError {
    /// An error known by its BC name only.
    pub fn coded(
        span: Span,
        code: impl Into<String>,
    ) -> Self {
        let code = code.into();
        Self {
            span,
            message: code.clone(),
            code,
            help: None,
            expected: None,
            subject: None,
            detail: None,
            headline: None,
            related: None,
        }
    }
}

impl From<LexError> for ParseError {
    fn from(error: LexError) -> Self {
        Self::coded(error.span, error.message)
    }
}
