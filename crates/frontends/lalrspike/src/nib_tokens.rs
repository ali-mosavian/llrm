//! Nib tokens for the LALR parser: the lexer's, with the one `?` that opens a
//! conditional marked, since no finite lookahead tells it from `x?`.

use llrm_nib::lexer::{Token, TokenKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tok {
    Plain(TokenKind),
    /// The `?` of `cond ? then : otherwise`.
    Ternary,
    /// The `not` of `a is not b`; elsewhere `not` is a name.
    Not,
}

#[derive(Debug)]
pub enum Never {}

pub type Spanned = Result<(usize, Tok, usize), Never>;

pub fn tokens(source: &[Token]) -> Vec<Spanned> {
    source
        .iter()
        .enumerate()
        .map(|(at, token)| {
            let after_is = at > 0 && source[at - 1].kind == TokenKind::Is;
            let tok = match &token.kind {
                TokenKind::Question if opens_conditional(&source[at + 1..]) => Tok::Ternary,
                TokenKind::Identifier(word)
                    if after_is
                        && word == "not"
                        && source.get(at + 1).is_some_and(|next| starts_operand(&next.kind)) =>
                {
                    Tok::Not
                }
                _ => Tok::Plain(token.kind.clone()),
            };
            Ok((at, tok, at + 1))
        })
        .collect()
}

/// A `:` follows at the same depth on the line, and not at its end, where it
/// opens a suite (`with x = f()?:`).
fn opens_conditional(rest: &[Token]) -> bool {
    let mut depth = 0_i32;
    for (index, token) in rest.iter().enumerate() {
        match token.kind {
            TokenKind::LeftParen | TokenKind::LeftBracket | TokenKind::LeftBrace => depth += 1,
            TokenKind::RightParen | TokenKind::RightBracket | TokenKind::RightBrace => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            TokenKind::Colon if depth == 0 => {
                return !matches!(
                    rest.get(index + 1).map(|next| &next.kind),
                    Some(TokenKind::Newline | TokenKind::Eof)
                );
            }
            TokenKind::Comma if depth == 0 => return false,
            TokenKind::Newline | TokenKind::Eof => return false,
            _ => {}
        }
    }
    false
}

fn starts_operand(kind: &TokenKind) -> bool {
    !matches!(
        kind,
        TokenKind::Newline
            | TokenKind::Eof
            | TokenKind::RightParen
            | TokenKind::RightBracket
            | TokenKind::RightBrace
            | TokenKind::Comma
            | TokenKind::Colon
    )
}
