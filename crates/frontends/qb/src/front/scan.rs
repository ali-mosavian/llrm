//! The scanner for the typed parser, written to BC's lexical rules rather than
//! the table-driven scanner's: spaces inside a number are ignored (`1 2` is
//! 12), a string left open ends at the line's end, a literal with eight or more
//! digits is a DOUBLE, an integer past the LONG range is one too, `type$`-style
//! words are names, and a DATA statement's text is kept as written.
//!
//! The tokens are the table-driven scanner's `Token`s, so the typed syntax's
//! spans and the parser's kinds are as before; a DATA keyword is followed by
//! one `String` token holding the statement's raw text.

use crate::front::kw::{self, Kw};
use crate::front::lines::logical_lines;
use crate::front::token::{Token, TokenKind};
use crate::front::words::Word;
use crate::syntax::{Binary, Span};
use crate::{Dialect, LexError};

fn error(
    line: usize,
    start: usize,
    end: usize,
    message: &str,
) -> LexError {
    LexError { span: Span { line, start, end }, message: message.into() }
}

fn punct(byte: u8) -> TokenKind {
    TokenKind::Reserved(kw::punctuation(byte))
}

fn reserved(name: &str) -> Option<Kw> {
    use logos::Logos;
    let mut lexer = Word::lexer(name);
    match lexer.next() {
        Some(Ok(word)) if lexer.span().end == name.len() => Some(crate::front::words::kw_of(word)),
        _ => None,
    }
}

fn is_suffix(
    byte: u8,
    dialect: Dialect,
) -> bool {
    matches!(byte, b'%' | b'&' | b'!' | b'#' | b'$') || (byte == b'@' && dialect.currency_suffix())
}

fn name_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
}

/// A byte that may continue a name: VBDOS BC also takes the underscore.
fn name_continues(
    byte: u8,
    dialect: Dialect,
) -> bool {
    byte.is_ascii_alphanumeric() || (byte == b'_' && dialect.underscore_in_names())
}

/// The index of the next byte that is not a space or tab.
fn skip_blanks(
    bytes: &[u8],
    mut at: usize,
) -> usize {
    while at < bytes.len() && matches!(bytes[at], b' ' | b'\t') {
        at += 1;
    }
    at
}

pub fn scan(
    source: &str,
    dialect: Dialect,
) -> Result<Vec<Token>, LexError> {
    let mut out = Vec::with_capacity(source.len() / 4);
    let newline = punct(b'\n');
    // PDS 7.1 reads REM at the start of a TYPE body line as a field name, never
    // a comment.
    let mut type_body = false;
    for (line_index, line) in logical_lines(source, dialect) {
        scan_line(&line, line_index, dialect, type_body && dialect == Dialect::Pds71, &mut out)?;
        let mut words = line.split(|c: char| !c.is_ascii_alphanumeric()).filter(|word| !word.is_empty());
        let first = words.next().map(str::to_ascii_uppercase);
        let first = if first.as_deref().is_some_and(|w| w.bytes().all(|b| b.is_ascii_digit())) {
            words.next().map(str::to_ascii_uppercase)
        } else {
            first
        };
        match (first.as_deref(), words.next().map(str::to_ascii_uppercase).as_deref()) {
            (Some("TYPE"), Some(_)) => type_body = true,
            (Some("END"), Some("TYPE")) => type_body = false,
            _ => {}
        }
        out.push(Token { kind: newline.clone(), span: Span { line: line_index, start: line.len(), end: line.len() } });
    }
    if source.is_empty() {
        out.push(Token { kind: newline, span: Span { line: 1, start: 0, end: 0 } });
    }
    Ok(out)
}

fn scan_line(
    line: &str,
    line_index: usize,
    dialect: Dialect,
    type_body: bool,
    out: &mut Vec<Token>,
) -> Result<(), LexError> {
    let bytes = line.as_bytes();
    let mut at = 0;
    // where this line's tokens begin in `out`
    let line_start = out.len();
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
                // a metacommand is the first thing on its line; after a
                // statement it is a comment
                let directive =
                    if out.len() == line_start { line[at + 1..].trim().to_ascii_uppercase() } else { String::new() };
                if let Some(problem) = metacommand_problem(&directive, dialect) {
                    return Err(error(line_index, start, bytes.len(), problem));
                }
                at = bytes.len();
                match directive.as_str() {
                    "$DYNAMIC" => TokenKind::ArrayDynamic,
                    "$STATIC" => TokenKind::ArrayStatic,
                    other
                        if matches!(dialect, Dialect::VbDos | Dialect::Quickr)
                            && other.starts_with("$FORM")
                            && other.len() > 5
                            && other.as_bytes()[5].is_ascii_whitespace() =>
                    {
                        TokenKind::FormDeclaration(other[5..].trim().to_owned())
                    }
                    _ => continue,
                }
            }
            // QuickrBASIC's `f"..."`
            b'f' | b'F' if dialect.format_strings() && bytes.get(at + 1) == Some(&b'"') => {
                crate::front::quickr::format_string(line, &mut at, line_index, dialect)?
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
                    number(line, &mut at, line_index, dialect, out.len() != line_start)?
                }
            }
            b'.' => {
                // `1.2.3` is the numbers `1.2` and `.3`: a point after a number
                // starts another
                let after_number = out
                    .last()
                    .is_some_and(
                        |last| matches!(last.kind, TokenKind::Integer(..) | TokenKind::Real(..)) && last.span.end == at,
                    );
                let after_name = !after_number && at > 0 && (name_char(bytes[at - 1]) || bytes[at - 1] == b')');
                if bytes.get(at + 1).is_some_and(u8::is_ascii_digit) && !after_name {
                    number(line, &mut at, line_index, dialect, out.len() != line_start)?
                } else if after_name || bytes.get(at + 1).is_some_and(|b| name_char(*b)) {
                    at += 1;
                    TokenKind::Period
                } else {
                    // a lone point is the number 0
                    at += 1;
                    TokenKind::Real("0".into(), None)
                }
            }
            b'&' => {
                // a based number cannot be a line number: BC does not know it
                // for a statement
                if out.len() == line_start && bytes.get(at + 1).is_some_and(|b| b.is_ascii_alphabetic()) {
                    return Err(error(line_index, at, bytes.len(), "Statement unrecognizable"));
                }
                based(line, &mut at, line_index, dialect)?
            }
            b'A'..=b'Z' | b'a'..=b'z' => {
                while at < bytes.len() && name_continues(bytes[at], dialect) {
                    at += 1;
                }
                if at - start > 40 {
                    return Err(error(line_index, start, at, "Identifier too long"));
                }
                // letters, digits and the type suffix; a period starts a field
                // or the rest of a dotted name
                if at < bytes.len() && is_suffix(bytes[at], dialect) {
                    at += 1;
                }
                let name = &line[start..at];
                let bare = name.trim_end_matches(['%', '&', '!', '#', '$', '@']);
                let reserved_id = reserved(name);
                // `GO TO` and `GO SUB` are GOTO and GOSUB; `ENDIF` is END IF
                if bare.eq_ignore_ascii_case("GO") && name.len() == bare.len() {
                    let rest = &line[at..];
                    let after_blanks = rest.trim_start();
                    let word: String = after_blanks.chars().take_while(char::is_ascii_alphabetic).collect();
                    let ends = after_blanks[word.len()..].bytes().next().is_none_or(|b| !name_continues(b, dialect));
                    let joined = if ends { reserved(&format!("GO{}", word.to_ascii_uppercase())) } else { None };
                    if let (Some(id), true) = (joined, matches!(word.to_ascii_uppercase().as_str(), "TO" | "SUB")) {
                        at += rest.len() - after_blanks.len() + word.len();
                        out.push(Token { kind: TokenKind::Reserved(id), span: span(at) });
                        continue;
                    }
                }
                if bare.eq_ignore_ascii_case("ENDIF") && name.len() == bare.len() {
                    if let (Some(end), Some(then)) = (reserved("END"), reserved("IF")) {
                        out.push(Token { kind: TokenKind::Reserved(end), span: span(at) });
                        out.push(Token { kind: TokenKind::Reserved(then), span: span(at) });
                        continue;
                    }
                }
                let type_field = type_body && out.len() == line_start;
                if !type_field
                    && (reserved_id == Some(Kw::REM)
                        || (reserved_id.is_none() && bare.eq_ignore_ascii_case("REM") && name.len() == bare.len()))
                {
                    let first = out.len() == line_start;
                    let directive = if first { line[at..].trim().to_ascii_uppercase() } else { String::new() };
                    // after a statement (and a line number) REM ends it, as `:
                    // REM` would
                    let after_label =
                        out.len() == line_start + 1 && matches!(out[line_start].kind, TokenKind::Integer(..));
                    let after_colon = out.last().is_some_and(|last| last.kind == punct(b':'));
                    if !first && !after_label && !after_colon {
                        out.push(Token { kind: punct(b':'), span: span(start) });
                    }
                    at = bytes.len();
                    match directive.as_str() {
                        "$DYNAMIC" => TokenKind::ArrayDynamic,
                        "$STATIC" => TokenKind::ArrayStatic,
                        _ => TokenKind::Reserved(Kw::REM),
                    }
                } else if let Some(id) = reserved_id {
                    if id == Kw::DATA {
                        out.push(Token { kind: TokenKind::Reserved(id), span: span(at) });
                        let text_start = at;
                        let text_end = statement_end(line, at);
                        out.push(Token {
                            kind: TokenKind::String(line[text_start..text_end].to_owned()),
                            span: Span { line: line_index, start: text_start, end: text_end },
                        });
                        at = text_end;
                        continue;
                    }
                    TokenKind::Reserved(id)
                } else if let Some(word) = extension_keyword(bare).filter(|word| {
                    name.len() == bare.len()
                        && (*word != "EXPLICIT" || matches!(dialect, Dialect::VbDos | Dialect::Quickr))
                }) {
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
                return Err(error(
                    line_index,
                    at,
                    at + line[at..].chars().next().map_or(1, char::len_utf8),
                    "Invalid character",
                ));
            }
        };
        out.push(Token { kind, span: span(at) });
    }
    Ok(())
}

fn extension_keyword(bare: &str) -> Option<&'static str> {
    kw::extension_keyword(&bare.to_ascii_uppercase())
}

/// Where a DATA statement's text ends: a colon outside quotes, or the end of
/// the line.
fn statement_end(
    line: &str,
    from: usize,
) -> usize {
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

/// A numeric literal from its first byte. Spaces and tabs between its parts are
/// ignored.
fn number(
    line: &str,
    at: &mut usize,
    line_index: usize,
    dialect: Dialect,
    joins_blanks: bool,
) -> Result<TokenKind, LexError> {
    let bytes = line.as_bytes();
    let start = *at;
    let mut digits = String::new();
    let mut i = start;
    // the next byte of the literal after blanks, if blanks are followed by one
    // of `wanted`
    let digits_run = |i: &mut usize, digits: &mut String| loop {
        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            digits.push(char::from(bytes[*i]));
            *i += 1;
        }
        let next = skip_blanks(bytes, *i);
        // blanks between digits are skipped, except in a line number (`10 20
        // PRINT` is two numbers)
        if joins_blanks && next > *i && next < bytes.len() && bytes[next].is_ascii_digit() {
            *i = next;
        } else {
            return;
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
    // an exponent: E or D, an optional sign, digits; blanks anywhere between
    // them
    let mut exponent_marker = None;
    let marker_at = skip_blanks(bytes, i);
    if let Some(&marker) = bytes.get(marker_at) {
        if matches!(marker, b'E' | b'e' | b'D' | b'd') && !digits.is_empty() {
            let mut j = skip_blanks(bytes, marker_at + 1);
            let sign = matches!(bytes.get(j), Some(b'+' | b'-')).then(|| bytes[j]);
            if sign.is_some() {
                j = skip_blanks(bytes, j + 1);
            }
            // `1E+` is `1E+0`: a sign without digits still completes the
            // exponent
            // A bare `1E` or `1D` is `1E0` (BC, all three compilers) when a
            // blank follows the marker: `IF 1E THEN`, `x = 1E : y =
            // 1`. At the end of the line, before `)` or
            // before a letter (`1EA`) it is a syntax error.
            let bare = sign.is_none()
                && !bytes.get(j).is_some_and(u8::is_ascii_digit)
                && matches!(bytes.get(marker_at + 1), Some(b' ' | b'\t'));
            if bytes.get(j).is_some_and(u8::is_ascii_digit) || sign.is_some() || bare {
                exponent_marker = Some(marker.to_ascii_uppercase());
                real = true;
                digits.push('E');
                if let Some(sign) = sign {
                    digits.push(char::from(sign));
                }
                i = j;
                digits_run(&mut i, &mut digits);
                if !digits.ends_with(|c: char| c.is_ascii_digit()) {
                    digits.push('0');
                }
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
        let kind_suffix = if double && !single {
            Some('#')
        } else if single {
            Some('!')
        } else {
            None
        };
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
fn based(
    line: &str,
    at: &mut usize,
    line_index: usize,
    dialect: Dialect,
) -> Result<TokenKind, LexError> {
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
    let value =
        if unsigned & (1_u64 << (bits - 1)) != 0 { (unsigned as i64) - (1_i64 << bits) } else { unsigned as i64 };
    Ok(TokenKind::Integer(value, if bits == 32 { Some('&') } else { suffix }))
}

/// BC's complaint about a metacommand that is written wrong, if it is: an
/// `$INCLUDE` with no quoted file name, a `$LINESIZE` with no size, a `$FORM`
/// with no name. (An `$INCLUDE` that is well formed has been expanded before
/// the scanner sees the text.)
fn metacommand_problem(
    directive: &str,
    dialect: Dialect,
) -> Option<&'static str> {
    let argument = |keyword: &str| directive.strip_prefix(keyword).map(|rest| rest.trim_start_matches(':').trim());
    if let Some(file) = argument("$INCLUDE") {
        let quoted = file.len() > 2 && file.starts_with('\'') && file.ends_with('\'');
        if !directive["$INCLUDE".len()..].starts_with(':') || !quoted {
            return Some("$Metacommand Error");
        }
    }
    if argument("$LINESIZE").is_some_and(str::is_empty) {
        return Some("$Metacommand Error");
    }
    if matches!(dialect, Dialect::VbDos | Dialect::Quickr) && directive == "$FORM" {
        return Some("Form variable already in use or illegal form name");
    }
    None
}
