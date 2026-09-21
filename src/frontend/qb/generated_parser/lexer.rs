//! Source scanner for the generated QBasic table parser.
//!
//! This is the only source-token path. It keeps the logical-line span
//! convention and universal VBDOS source-syntax superset while looking up
//! reserved words in the generated catalogue.

use crate::frontend::qb::dialect::Dialect;
pub use crate::frontend::qb::error::LexError;
use crate::frontend::qb::syntax::{Binary, Span};

use super::tables;

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Reserved(u16),
    ExtensionKeyword(&'static str),
    Identifier(String),
    Integer(i64, Option<char>),
    Real(String, Option<char>),
    String(String),
    Comparison(Binary),
    Period,
    ArrayDynamic,
    ArrayStatic,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

pub fn lex(source: &str, dialect: Dialect) -> Result<Vec<Token>, LexError> {
    let mut out = Vec::new();
    for (line_index, line) in logical_lines(source) {
        let bytes = line.as_bytes();
        let mut at = 0;
        while at < bytes.len() {
            if matches!(bytes[at], b' ' | b'\t') {
                at += 1;
                continue;
            }
            let start = at;
            let span = |end| Span {
                line: line_index,
                start,
                end,
            };
            let kind = match bytes[at] {
                b'\'' => {
                    let directive = line[at + 1..].trim().to_ascii_uppercase();
                    at = bytes.len();
                    match directive.as_str() {
                        "$DYNAMIC" => TokenKind::ArrayDynamic,
                        "$STATIC" => TokenKind::ArrayStatic,
                        _ => continue,
                    }
                }
                b'"' => {
                    at += 1;
                    let content = at;
                    while at < bytes.len() && bytes[at] != b'"' {
                        at += 1;
                    }
                    if at == bytes.len() {
                        return Err(error(span(at), "unterminated string"));
                    }
                    let value = line[content..at].to_string();
                    at += 1;
                    TokenKind::String(value)
                }
                b'&' => based_integer(&line, &mut at, span)?,
                b'0'..=b'9' if start > 0 && bytes[start - 1] == b'.' => {
                    at += 1;
                    while at < bytes.len() && identifier_char(bytes[at], dialect) {
                        at += 1;
                    }
                    if at < bytes.len() && is_type_suffix(bytes[at]) {
                        at += 1;
                    }
                    TokenKind::Identifier(line[start..at].to_ascii_uppercase())
                }
                b'0'..=b'9' | b'.'
                    if bytes[at] != b'.'
                        || (bytes.get(at + 1).is_some_and(u8::is_ascii_digit)
                            && (at == 0 || !identifier_char(bytes[at - 1], dialect))) =>
                {
                    numeric(&line, &mut at, span)?
                }
                b'A'..=b'Z' | b'a'..=b'z' => identifier_or_reserved(&line, &mut at, dialect),
                b'_' => {
                    return Err(error(
                        span(at + 1),
                        "identifier cannot begin with underscore",
                    ));
                }
                b'<' => {
                    at += 1;
                    if bytes.get(at) == Some(&b'=') {
                        at += 1;
                        TokenKind::Comparison(Binary::LessEqual)
                    } else if bytes.get(at) == Some(&b'>') {
                        at += 1;
                        TokenKind::Comparison(Binary::NotEqual)
                    } else {
                        reserved_token("<")
                    }
                }
                b'>' => {
                    at += 1;
                    if bytes.get(at) == Some(&b'=') {
                        at += 1;
                        TokenKind::Comparison(Binary::GreaterEqual)
                    } else {
                        reserved_token(">")
                    }
                }
                b'(' | b'[' => {
                    at += 1;
                    reserved_token("(")
                }
                b')' | b']' => {
                    at += 1;
                    reserved_token(")")
                }
                b'.' => {
                    at += 1;
                    TokenKind::Period
                }
                b'?' => {
                    at += 1;
                    // `?` is the recovered `PRINT` shorthand, not an unknown character.
                    reserved_token("?")
                }
                byte @ (b'+' | b'-' | b'*' | b'/' | b'\\' | b'^' | b'=' | b',' | b'#' | b';'
                | b':') => {
                    at += 1;
                    reserved_token(&char::from(byte).to_string())
                }
                other => {
                    return Err(error(
                        span(at + 1),
                        format!("unknown character {:?}", char::from(other)),
                    ));
                }
            };
            out.push(Token {
                kind,
                span: span(at),
            });
        }
        out.push(Token {
            kind: reserved_token("\n"),
            span: Span {
                line: line_index,
                start: bytes.len(),
                end: bytes.len(),
            },
        });
    }
    if source.is_empty() {
        out.push(Token {
            kind: reserved_token("\n"),
            span: Span {
                line: 1,
                start: 0,
                end: 0,
            },
        });
    }
    Ok(out)
}

fn error(span: Span, message: impl Into<String>) -> LexError {
    LexError {
        span,
        message: message.into(),
    }
}

fn identifier_char(byte: u8, _dialect: Dialect) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn is_type_suffix(byte: u8) -> bool {
    matches!(byte, b'%' | b'&' | b'!' | b'#' | b'$')
}

fn reserved(spelling: &str) -> Option<u16> {
    tables::TOKENS
        .iter()
        .find_map(|(_name, candidate, id, _flags)| {
            candidate.eq_ignore_ascii_case(spelling).then_some(*id)
        })
}

fn reserved_token(spelling: &str) -> TokenKind {
    TokenKind::Reserved(reserved(spelling).expect("local spelling exists in qbasbnf.prs"))
}

fn identifier_or_reserved(line: &str, at: &mut usize, dialect: Dialect) -> TokenKind {
    let bytes = line.as_bytes();
    let start = *at;
    *at += 1;
    while *at < bytes.len() && identifier_char(bytes[*at], dialect) {
        *at += 1;
    }
    if *at < bytes.len() && is_type_suffix(bytes[*at]) {
        *at += 1;
    }
    let name = line[start..*at].to_ascii_uppercase();
    let bare = name.trim_end_matches(['%', '&', '!', '#', '$']);
    if bare == "REM" {
        let directive = line[*at..].trim().to_ascii_uppercase();
        *at = bytes.len();
        return match directive.as_str() {
            "$DYNAMIC" => TokenKind::ArrayDynamic,
            "$STATIC" => TokenKind::ArrayStatic,
            _ => reserved_token("REM"),
        };
    }
    reserved(&name)
        .or_else(|| reserved(bare))
        .map(TokenKind::Reserved)
        .or_else(|| tables::extension_keyword(bare).map(TokenKind::ExtensionKeyword))
        .unwrap_or(TokenKind::Identifier(name))
}

fn numeric(
    line: &str,
    at: &mut usize,
    span: impl Fn(usize) -> Span,
) -> Result<TokenKind, LexError> {
    let bytes = line.as_bytes();
    let start = *at;
    *at += 1;
    while *at < bytes.len() && bytes[*at].is_ascii_digit() {
        *at += 1;
    }
    let mut real = bytes[start] == b'.';
    if *at < bytes.len() && bytes[*at] == b'.' {
        real = true;
        *at += 1;
        while *at < bytes.len() && bytes[*at].is_ascii_digit() {
            *at += 1;
        }
    }
    let exponent_marker = bytes
        .get(*at)
        .copied()
        .filter(|byte| matches!(byte, b'E' | b'e' | b'D' | b'd'));
    let mantissa_end = *at;
    if exponent_marker.is_some() {
        real = true;
        *at += 1;
        if *at < bytes.len() && matches!(bytes[*at], b'+' | b'-') {
            *at += 1;
        }
        let exponent = *at;
        while *at < bytes.len() && bytes[*at].is_ascii_digit() {
            *at += 1;
        }
        if exponent == *at {
            return Err(error(span(*at), "missing exponent digits"));
        }
    }
    let explicit_suffix = if *at < bytes.len() && matches!(bytes[*at], b'%' | b'&' | b'!' | b'#') {
        let suffix = char::from(bytes[*at]);
        real |= matches!(bytes[*at], b'!' | b'#');
        *at += 1;
        Some(suffix)
    } else {
        None
    };
    let text = &line[start..*at];
    if real {
        let suffix = explicit_suffix.or_else(|| {
            if exponent_marker.is_some_and(|byte| matches!(byte, b'D' | b'd'))
                || (exponent_marker.is_none()
                    && bytes[start..mantissa_end]
                        .iter()
                        .filter(|byte| byte.is_ascii_digit())
                        .count()
                        > 15)
            {
                Some('#')
            } else {
                None
            }
        });
        Ok(TokenKind::Real(
            text.trim_end_matches(['!', '#']).replace(['D', 'd'], "E"),
            suffix,
        ))
    } else {
        let number = text
            .trim_end_matches(['%', '&'])
            .parse()
            .map_err(|_| error(span(*at), "integer literal is out of range"))?;
        if number > i64::from(i32::MAX)
            || (explicit_suffix == Some('%') && number > i64::from(i16::MAX))
        {
            return Err(error(span(*at), "integer literal is out of range"));
        }
        let suffix = explicit_suffix.or_else(|| (number > i64::from(i16::MAX)).then_some('&'));
        Ok(TokenKind::Integer(number, suffix))
    }
}

fn based_integer(
    line: &str,
    at: &mut usize,
    span: impl Fn(usize) -> Span,
) -> Result<TokenKind, LexError> {
    let bytes = line.as_bytes();
    *at += 1;
    let radix = match bytes.get(*at).copied() {
        Some(b'H' | b'h') => {
            *at += 1;
            16
        }
        Some(b'O' | b'o') => {
            *at += 1;
            8
        }
        // `&377` is the scanner's default-octal spelling.
        _ => 8,
    };
    let digits = *at;
    while *at < bytes.len()
        && if radix == 16 {
            bytes[*at].is_ascii_hexdigit()
        } else {
            matches!(bytes[*at], b'0'..=b'7')
        }
    {
        *at += 1;
    }
    if digits == *at {
        return Err(error(span(*at), "missing based-integer digits"));
    }
    let end_digits = *at;
    let suffix = if bytes.get(*at).is_some_and(|one| matches!(one, b'%' | b'&')) {
        let suffix = char::from(bytes[*at]);
        *at += 1;
        Some(suffix)
    } else {
        None
    };
    let unsigned = u64::from_str_radix(&line[digits..end_digits], radix)
        .map_err(|_| error(span(*at), "integer literal is out of range"))?;
    let bits = if suffix == Some('&') || (suffix.is_none() && unsigned > 0xffff) {
        32
    } else {
        16
    };
    if unsigned >= (1_u64 << bits) {
        return Err(error(span(*at), "based integer does not fit its type"));
    }
    let value = if unsigned & (1_u64 << (bits - 1)) != 0 {
        (unsigned as i64) - (1_i64 << bits)
    } else {
        unsigned as i64
    };
    Ok(TokenKind::Integer(
        value,
        if bits == 32 { Some('&') } else { suffix },
    ))
}

fn logical_lines(source: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut begins = 1;
    for (index, physical) in source.lines().enumerate() {
        if current.is_empty() {
            begins = index + 1;
        }
        let trimmed = physical.trim_end();
        let continued =
            trimmed.ends_with('_') && trimmed[..trimmed.len() - 1].ends_with(char::is_whitespace);
        if continued {
            current.push_str(trimmed[..trimmed.len() - 1].trim_end());
            current.push(' ');
        } else {
            current.push_str(physical);
            out.push((begins, std::mem::take(&mut current)));
        }
    }
    if !current.is_empty() {
        out.push((begins, current));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_catalog_handles_extended_words_and_recovered_shorthand() {
        let tokens = lex("? sin(&377) + right$(\"ab\", 1)", Dialect::QuickBasic45).unwrap();
        assert!(tokens.iter().any(|token| token.kind == reserved_token("?")));
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == reserved_token("SIN"))
        );
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == reserved_token("RIGHT$"))
        );
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::Integer(255, None))
        );
    }
}
