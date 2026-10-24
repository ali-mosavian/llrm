//! The scanner for the typed parser, written to BC's lexical rules rather than the table-driven
//! scanner's: spaces inside a number are ignored (`1 2` is 12), a string left open ends at the
//! line's end, a literal with eight or more digits is a DOUBLE, an integer past the LONG range is
//! one too, `type$`-style words are names, and a DATA statement's text is kept as written.
//!
//! The tokens are the table-driven scanner's `Token`s, so the typed syntax's spans and the
//! parser's kinds are as before; a DATA keyword is followed by one `String` token holding the
//! statement's raw text.

use crate::generated_parser::{Token, TokenKind};
use crate::syntax::{Binary, Span};
use crate::{Dialect, LexError};

use crate::front::lexer::logical_lines;
use crate::front::words::Word;

fn error(line: usize, start: usize, end: usize, message: &str) -> LexError {
    LexError { span: Span { line, start, end }, message: message.into() }
}

fn ids() -> &'static Ids {
    static IDS: std::sync::OnceLock<Ids> = std::sync::OnceLock::new();
    IDS.get_or_init(|| {
        let id = |spelling: &str| {
            crate::generated_parser::tables::TOKENS
                .iter()
                .find_map(|(_, candidate, id, _)| candidate.eq_ignore_ascii_case(spelling).then_some(*id))
                .unwrap_or_else(|| panic!("{spelling} is a grammar token"))
        };
        let mut punct = [0; 128];
        for byte in "<>()?+-*/\\^=,#;:\n".bytes() {
            punct[usize::from(byte)] = id(&char::from(byte).to_string());
        }
        let words = crate::front::words::NAMES
            .iter()
            .map(|name| {
                let spelling = crate::generated_parser::tables::TOKENS
                    .iter()
                    .find(|(candidate, ..)| candidate == name)
                    .expect("word is a token")
                    .1;
                id(spelling)
            })
            .collect();
        Ids { punct, words, rem: id("REM"), data: id("DATA") }
    })
}

struct Ids {
    punct: [u16; 128],
    words: Vec<u16>,
    rem: u16,
    data: u16,
}

fn punct(byte: u8) -> TokenKind {
    TokenKind::Reserved(ids().punct[usize::from(byte)])
}

fn reserved(name: &str) -> Option<u16> {
    use logos::Logos;
    let mut lexer = Word::lexer(name);
    match lexer.next() {
        Some(Ok(word)) if lexer.span().end == name.len() => Some(ids().words[word as usize]),
        _ => None,
    }
}

fn is_suffix(byte: u8, dialect: Dialect) -> bool {
    matches!(byte, b'%' | b'&' | b'!' | b'#' | b'$') || (byte == b'@' && dialect.currency_suffix())
}

fn name_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
}

/// A byte that may continue a name: VBDOS BC also takes the underscore.
fn name_continues(byte: u8, dialect: Dialect) -> bool {
    byte.is_ascii_alphanumeric() || (byte == b'_' && dialect == Dialect::VbDos)
}

/// The index of the next byte that is not a space or tab.
fn skip_blanks(bytes: &[u8], mut at: usize) -> usize {
    while at < bytes.len() && matches!(bytes[at], b' ' | b'\t') {
        at += 1;
    }
    at
}

pub fn scan(source: &str, dialect: Dialect) -> Result<Vec<Token>, LexError> {
    let mut out = Vec::with_capacity(source.len() / 4);
    let newline = punct(b'\n');
    for (line_index, line) in logical_lines(source, dialect) {
        scan_line(&line, line_index, dialect, &mut out)?;
        out.push(Token { kind: newline.clone(), span: Span { line: line_index, start: line.len(), end: line.len() } });
    }
    if source.is_empty() {
        out.push(Token { kind: newline, span: Span { line: 1, start: 0, end: 0 } });
    }
    Ok(out)
}

fn scan_line(line: &str, line_index: usize, dialect: Dialect, out: &mut Vec<Token>) -> Result<(), LexError> {
    let bytes = line.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        let byte = bytes[at];
        if matches!(byte, b' ' | b'\t' | 0x0c) {
            at += 1;
            continue;
        }
        let start = at;
        let span = |end| Span { line: line_index, start, end };
        let kind = match byte {
            b'\'' => {
                let directive = line[at + 1..].trim().to_ascii_uppercase();
                at = bytes.len();
                match directive.as_str() {
                    "$DYNAMIC" => TokenKind::ArrayDynamic,
                    "$STATIC" => TokenKind::ArrayStatic,
                    other if matches!(dialect, Dialect::VbDos | Dialect::Quickr) && other.starts_with("$FORM") && other.len() > 5 && other.as_bytes()[5].is_ascii_whitespace() => {
                        TokenKind::FormDeclaration(other[5..].trim().to_owned())
                    }
                    _ => continue,
                }
            }
            b'"' => {
                at += 1;
                let mut value = String::new();
                let mut content = at;
                loop {
                    while at < bytes.len() && bytes[at] != b'"' {
                        at += 1;
                    }
                    if at >= bytes.len() {
                        // BC closes a string left open at the end of the line
                        value.push_str(&line[content..]);
                        break;
                    }
                    if dialect.quote_escape() && bytes.get(at + 1) == Some(&b'"') {
                        value.push_str(&line[content..=at]);
                        at += 2;
                        content = at;
                        continue;
                    }
                    value.push_str(&line[content..at]);
                    at += 1;
                    break;
                }
                TokenKind::String(value)
            }
            b'0'..=b'9' => {
                if start > 0 && bytes[start - 1] == b'.' {
                    // `a.1x`: a digit after a period continues the name
                    while at < bytes.len() && name_char(bytes[at]) {
                        at += 1;
                    }
                    if at < bytes.len() && is_suffix(bytes[at], dialect) {
                        at += 1;
                    }
                    TokenKind::Identifier(line[start..at].to_ascii_uppercase())
                } else {
                    number(line, &mut at, line_index, dialect)?
                }
            }
            b'.' => {
                let after_name = at > 0 && (name_char(bytes[at - 1]) || bytes[at - 1] == b')');
                if bytes.get(at + 1).is_some_and(u8::is_ascii_digit) && !after_name {
                    number(line, &mut at, line_index, dialect)?
                } else if after_name || bytes.get(at + 1).is_some_and(|b| name_char(*b)) {
                    at += 1;
                    TokenKind::Period
                } else {
                    // a lone point is the number 0
                    at += 1;
                    TokenKind::Real("0".into(), None)
                }
            }
            b'&' => based(line, &mut at, line_index, dialect)?,
            b'A'..=b'Z' | b'a'..=b'z' => {
                while at < bytes.len() && name_continues(bytes[at], dialect) {
                    at += 1;
                }
                if at - start > 40 {
                    return Err(error(line_index, start, at, "Identifier too long"));
                }
                // letters, digits and the type suffix; a period starts a field or the rest of a dotted name
                if at < bytes.len() && is_suffix(bytes[at], dialect) {
                    at += 1;
                }
                let name = &line[start..at];
                let bare = name.trim_end_matches(['%', '&', '!', '#', '$', '@']);
                let reserved_id = reserved(name);
                if reserved_id == Some(ids().rem) || (reserved_id.is_none() && bare.eq_ignore_ascii_case("REM") && name.len() == bare.len()) {
                    let directive = line[at..].trim().to_ascii_uppercase();
                    at = bytes.len();
                    match directive.as_str() {
                        "$DYNAMIC" => TokenKind::ArrayDynamic,
                        "$STATIC" => TokenKind::ArrayStatic,
                        _ => TokenKind::Reserved(ids().rem),
                    }
                } else if let Some(id) = reserved_id {
                    if id == ids().data {
                        out.push(Token { kind: TokenKind::Reserved(id), span: span(at) });
                        let text_start = at;
                        let text_end = statement_end(line, at);
                        out.push(Token { kind: TokenKind::String(line[text_start..text_end].to_owned()), span: Span { line: line_index, start: text_start, end: text_end } });
                        at = text_end;
                        continue;
                    }
                    TokenKind::Reserved(id)
                } else if let Some(word) = extension_keyword(bare).filter(|word| name.len() == bare.len() && (*word != "EXPLICIT" || matches!(dialect, Dialect::VbDos | Dialect::Quickr))) {
                    TokenKind::ExtensionKeyword(word)
                } else {
                    TokenKind::Identifier(name.to_ascii_uppercase())
                }
            }
            b'_' => return Err(error(line_index, at, at + 1, "identifier cannot begin with underscore")),
            b'<' => {
                at += 1;
                match bytes.get(at) {
                    Some(b'=') => {
                        at += 1;
                        TokenKind::Comparison(Binary::LessEqual)
                    }
                    Some(b'>') => {
                        at += 1;
                        TokenKind::Comparison(Binary::NotEqual)
                    }
                    _ => punct(b'<'),
                }
            }
            b'>' => {
                at += 1;
                match bytes.get(at) {
                    Some(b'=') => {
                        at += 1;
                        TokenKind::Comparison(Binary::GreaterEqual)
                    }
                    Some(b'<') => {
                        at += 1;
                        TokenKind::Comparison(Binary::NotEqual)
                    }
                    _ => punct(b'>'),
                }
            }
            b'=' => {
                at += 1;
                match bytes.get(at) {
                    Some(b'<') => {
                        at += 1;
                        TokenKind::Comparison(Binary::LessEqual)
                    }
                    Some(b'>') => {
                        at += 1;
                        TokenKind::Comparison(Binary::GreaterEqual)
                    }
                    _ => punct(b'='),
                }
            }
            b'(' | b'[' => {
                at += 1;
                punct(b'(')
            }
            b')' | b']' => {
                at += 1;
                punct(b')')
            }
            b'+' | b'-' | b'*' | b'/' | b'\\' | b'^' | b',' | b'#' | b';' | b':' => {
                at += 1;
                punct(byte)
            }
            _ => {
                return Err(error(line_index, at, at + line[at..].chars().next().map_or(1, char::len_utf8), &format!("unknown character {:?}", line[at..].chars().next().unwrap_or('?'))));
            }
        };
        out.push(Token { kind, span: span(at) });
    }
    Ok(())
}

fn extension_keyword(bare: &str) -> Option<&'static str> {
    crate::generated_parser::tables::extension_keyword(&bare.to_ascii_uppercase())
}

/// Where a DATA statement's text ends: a colon outside quotes, or the end of the line.
fn statement_end(line: &str, from: usize) -> usize {
    let bytes = line.as_bytes();
    let mut quoted = false;
    for (offset, &byte) in bytes[from..].iter().enumerate() {
        match byte {
            b'"' => quoted = !quoted,
            b':' if !quoted => return from + offset,
            _ => {}
        }
    }
    bytes.len()
}

/// A numeric literal from its first byte. Spaces and tabs between its parts are ignored.
fn number(line: &str, at: &mut usize, line_index: usize, dialect: Dialect) -> Result<TokenKind, LexError> {
    let bytes = line.as_bytes();
    let start = *at;
    let mut digits = String::new();
    let mut i = start;
    // the next byte of the literal after blanks, if blanks are followed by one of `wanted`
    let digits_run = |i: &mut usize, digits: &mut String| {
        loop {
            while *i < bytes.len() && bytes[*i].is_ascii_digit() {
                digits.push(char::from(bytes[*i]));
                *i += 1;
            }
            let next = skip_blanks(bytes, *i);
            if next > *i && next < bytes.len() && bytes[next].is_ascii_digit() {
                *i = next;
            } else {
                return;
            }
        }
    };
    let mut real = false;
    let mut mantissa_digits = 0;
    digits_run(&mut i, &mut digits);
    if i > start || digits.is_empty() {
        // blanks may precede the point: `1 .5`
    }
    let after_blanks = skip_blanks(bytes, i);
    let dot = if bytes.get(i) == Some(&b'.') {
        Some(i)
    } else if bytes.get(after_blanks) == Some(&b'.') && !digits.is_empty() {
        Some(after_blanks)
    } else {
        None
    };
    if let Some(dot) = dot {
        real = true;
        digits.push('.');
        i = dot + 1;
        digits_run(&mut i, &mut digits);
    }
    mantissa_digits += digits.bytes().filter(u8::is_ascii_digit).count();
    // an exponent: E or D, an optional sign, digits; blanks anywhere between them
    let mut exponent_marker = None;
    let marker_at = skip_blanks(bytes, i);
    if let Some(&marker) = bytes.get(marker_at) {
        if matches!(marker, b'E' | b'e' | b'D' | b'd') && !digits.is_empty() {
            let mut j = skip_blanks(bytes, marker_at + 1);
            let sign = matches!(bytes.get(j), Some(b'+' | b'-')).then(|| bytes[j]);
            if sign.is_some() {
                j = skip_blanks(bytes, j + 1);
            }
            if bytes.get(j).is_some_and(u8::is_ascii_digit) {
                exponent_marker = Some(marker.to_ascii_uppercase());
                real = true;
                digits.push('E');
                if let Some(sign) = sign {
                    digits.push(char::from(sign));
                }
                i = j;
                digits_run(&mut i, &mut digits);
            }
        }
    }
    // a type suffix, blanks before it allowed
    let suffix_at = skip_blanks(bytes, i);
    let suffix = match bytes.get(suffix_at) {
        Some(&s @ (b'%' | b'&' | b'!' | b'#')) if !bytes.get(suffix_at + 1).is_some_and(|b| name_char(*b)) => Some(s),
        Some(b'@') if dialect.currency_suffix() => Some(b'@'),
        _ => None,
    };
    if let Some(s) = suffix {
        i = suffix_at + 1;
        if exponent_marker.is_some() {
            // BC: `1.5E2!` is an invalid character after an exponent
            return Err(error(line_index, suffix_at, suffix_at + 1, "Invalid character"));
        }
        if real && matches!(s, b'%' | b'&') {
            return Err(error(line_index, start, suffix_at + 1, "Illegal type character in numeric constant"));
        }
    }
    *at = i;
    let text = digits;
    let suffix_char = suffix.map(char::from);
    let span_end = i;
    if suffix == Some(b'@') {
        return Ok(TokenKind::Real(text, Some('@')));
    }
    if real || matches!(suffix, Some(b'!' | b'#')) {
        let double = suffix == Some(b'#') || exponent_marker.is_some_and(|m| m == b'D') || mantissa_digits >= 8;
        let single = suffix == Some(b'!');
        let kind_suffix = if double && !single { Some('#') } else if single { Some('!') } else { None };
        let text = text.replace('D', "E");
        let text = if text.starts_with('.') { text } else { text };
        return Ok(TokenKind::Real(text, kind_suffix));
    }
    // an integer
    let Ok(value) = text.parse::<i64>() else {
        // more digits than an i64 holds: a DOUBLE
        return Ok(TokenKind::Real(text, Some('#')));
    };
    match suffix {
        Some(b'%') => {
            if value > i64::from(i16::MAX) {
                return Err(error(line_index, start, span_end, "Illegal type character in numeric constant"));
            }
            Ok(TokenKind::Integer(value, Some('%')))
        }
        Some(b'&') => {
            if value > i64::from(i32::MAX) {
                return Err(error(line_index, start, span_end, "Illegal type character in numeric constant"));
            }
            Ok(TokenKind::Integer(value, Some('&')))
        }
        _ => {
            let _ = suffix_char;
            if value > i64::from(i32::MAX) {
                // past LONG: a DOUBLE, which prints in full
                Ok(TokenKind::Real(text, Some('#')))
            } else if value > i64::from(i16::MAX) {
                Ok(TokenKind::Integer(value, Some('&')))
            } else {
                Ok(TokenKind::Integer(value, None))
            }
        }
    }
}

/// `&H…`, `&O…` and `&…` (octal).
fn based(line: &str, at: &mut usize, line_index: usize, dialect: Dialect) -> Result<TokenKind, LexError> {
    let bytes = line.as_bytes();
    let start = *at;
    let mut i = start + 1;
    let radix = match bytes.get(i) {
        Some(b'H' | b'h') => {
            i += 1;
            16
        }
        Some(b'O' | b'o') => {
            i += 1;
            8
        }
        _ => 8,
    };
    let digits_from = i;
    while i < bytes.len() && if radix == 16 { bytes[i].is_ascii_hexdigit() } else { matches!(bytes[i], b'0'..=b'7') } {
        i += 1;
    }
    if i == digits_from {
        return Err(error(line_index, start, i, "missing based-integer digits"));
    }
    let digits_end = i;
    let suffix = match bytes.get(i) {
        Some(&s @ (b'%' | b'&')) => {
            i += 1;
            Some(char::from(s))
        }
        _ => None,
    };
    let _ = dialect;
    *at = i;
    let unsigned = u64::from_str_radix(&line[digits_from..digits_end], radix)
        .map_err(|_| error(line_index, start, i, "Overflow in numeric constant"))?;
    let bits = if suffix == Some('&') || (suffix.is_none() && unsigned > 0xffff) { 32 } else { 16 };
    if unsigned >= (1_u64 << bits) {
        return Err(error(line_index, start, i, "Overflow in numeric constant"));
    }
    let value = if unsigned & (1_u64 << (bits - 1)) != 0 { (unsigned as i64) - (1_i64 << bits) } else { unsigned as i64 };
    Ok(TokenKind::Integer(value, if bits == 32 { Some('&') } else { suffix }))
}
