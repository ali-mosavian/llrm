use crate::error::Diagnostic;
use crate::syntax::Span;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TokenKind {
    Identifier(String),
    Integer(i64),
    Float(String),
    Character(u8),
    String(Vec<u8>),
    FString(Vec<u8>),
    Fn,
    Type,
    Fixed,
    Fraction,
    Struct,
    Let,
    Var,
    Mut,
    Return,
    If,
    Else,
    While,
    For,
    In,
    Break,
    Continue,
    True,
    False,
    Not,
    Is,
    Char,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    F32,
    F64,
    StringType,
    Bool,
    Void,
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    LeftBrace,
    RightBrace,
    Comma,
    Semicolon,
    Dot,
    Range,
    Colon,
    Arrow,
    Equal,
    EqualEqual,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Plus,
    Minus,
    Star,
    Ampersand,
    Slash,
    Percent,
    Newline,
    Indent,
    Dedent,
    Eof,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

fn token(kind: TokenKind, line: usize, start: usize, end: usize) -> Token {
    Token {
        kind,
        span: Span::new(line, start + 1, end + 1),
    }
}

fn keyword(word: &str) -> Option<TokenKind> {
    Some(match word {
        "fn" => TokenKind::Fn,
        "type" => TokenKind::Type,
        "fixed" => TokenKind::Fixed,
        "fraction" => TokenKind::Fraction,
        "struct" => TokenKind::Struct,
        "let" => TokenKind::Let,
        "var" => TokenKind::Var,
        "mut" => TokenKind::Mut,
        "return" => TokenKind::Return,
        "if" => TokenKind::If,
        "else" => TokenKind::Else,
        "while" => TokenKind::While,
        "for" => TokenKind::For,
        "in" => TokenKind::In,
        "break" => TokenKind::Break,
        "continue" => TokenKind::Continue,
        "true" => TokenKind::True,
        "false" => TokenKind::False,
        "not" => TokenKind::Not,
        "is" => TokenKind::Is,
        "char" => TokenKind::Char,
        "i8" => TokenKind::I8,
        "u8" => TokenKind::U8,
        "i16" => TokenKind::I16,
        "u16" => TokenKind::U16,
        "i32" => TokenKind::I32,
        "u32" => TokenKind::U32,
        "f32" => TokenKind::F32,
        "f64" => TokenKind::F64,
        "string" => TokenKind::StringType,
        "bool" => TokenKind::Bool,
        "void" => TokenKind::Void,
        _ => return None,
    })
}

/// Convert source text into tokens, making layout explicit as INDENT/DEDENT.
pub fn lex(source: &str) -> Result<Vec<Token>, Diagnostic> {
    let mut tokens = Vec::new();
    let mut indents = vec![0usize];
    let mut nesting = 0usize;
    let mut last_line = 1usize;

    for (zero_line, raw_line) in source.split_inclusive('\n').enumerate() {
        let line_number = zero_line + 1;
        last_line = line_number;
        let line = raw_line
            .strip_suffix('\n')
            .unwrap_or(raw_line)
            .strip_suffix('\r')
            .unwrap_or_else(|| raw_line.strip_suffix('\n').unwrap_or(raw_line));
        let bytes = line.as_bytes();
        let mut index = 0usize;

        if nesting == 0 {
            while index < bytes.len() && bytes[index] == b' ' {
                index += 1;
            }
            if index < bytes.len() && bytes[index] == b'\t' {
                return Err(Diagnostic::new(
                    Span::new(line_number, index + 1, index + 2),
                    "tabs are not allowed in indentation",
                ));
            }
            let blank = index == bytes.len() || bytes[index] == b'#';
            if blank {
                continue;
            }
            match index.cmp(indents.last().expect("indentation stack")) {
                std::cmp::Ordering::Greater => {
                    indents.push(index);
                    tokens.push(token(TokenKind::Indent, line_number, 0, index));
                }
                std::cmp::Ordering::Less => {
                    while index < *indents.last().expect("indentation stack") {
                        indents.pop();
                        tokens.push(token(TokenKind::Dedent, line_number, 0, index));
                    }
                    if index != *indents.last().expect("indentation stack") {
                        return Err(Diagnostic::new(
                            Span::new(line_number, 1, index + 1),
                            "indentation does not match an enclosing block",
                        ));
                    }
                }
                std::cmp::Ordering::Equal => {}
            }
        }

        while index < bytes.len() {
            let start = index;
            match bytes[index] {
                b' ' | b'\t' => {
                    index += 1;
                }
                b'#' => break,
                b'f' if index + 1 < bytes.len() && bytes[index + 1] == b'"' => {
                    index += 1;
                    let value = quoted(bytes, line_number, &mut index, start)?;
                    tokens.push(token(TokenKind::FString(value), line_number, start, index));
                }
                b'"' => {
                    let value = quoted(bytes, line_number, &mut index, start)?;
                    tokens.push(token(TokenKind::String(value), line_number, start, index));
                }
                b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                    index += 1;
                    while index < bytes.len()
                        && matches!(bytes[index], b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_')
                    {
                        index += 1;
                    }
                    let word = &line[start..index];
                    let kind = keyword(word).unwrap_or_else(|| TokenKind::Identifier(word.into()));
                    tokens.push(token(kind, line_number, start, index));
                }
                b'0'..=b'9' => {
                    index += 1;
                    while index < bytes.len() && bytes[index].is_ascii_digit() {
                        index += 1;
                    }
                    let mut floating = false;
                    if index < bytes.len()
                        && bytes[index] == b'.'
                        && bytes.get(index + 1) != Some(&b'.')
                    {
                        floating = true;
                        index += 1;
                        while index < bytes.len() && bytes[index].is_ascii_digit() {
                            index += 1;
                        }
                    }
                    if index < bytes.len() && matches!(bytes[index], b'e' | b'E') {
                        floating = true;
                        index += 1;
                        if index < bytes.len() && matches!(bytes[index], b'+' | b'-') {
                            index += 1;
                        }
                        let exponent = index;
                        while index < bytes.len() && bytes[index].is_ascii_digit() {
                            index += 1;
                        }
                        if exponent == index {
                            return Err(Diagnostic::new(
                                Span::new(line_number, start + 1, index + 1),
                                "floating literal exponent requires digits",
                            ));
                        }
                    }
                    let spelling = &line[start..index];
                    if floating {
                        tokens.push(token(
                            TokenKind::Float(spelling.into()),
                            line_number,
                            start,
                            index,
                        ));
                    } else {
                        let value = spelling.parse::<i64>().map_err(|_| {
                            Diagnostic::new(
                                Span::new(line_number, start + 1, index + 1),
                                "integer literal is too large",
                            )
                        })?;
                        tokens.push(token(TokenKind::Integer(value), line_number, start, index));
                    }
                }
                b'.' if index + 1 < bytes.len() && bytes[index + 1].is_ascii_digit() => {
                    index += 2;
                    while index < bytes.len() && bytes[index].is_ascii_digit() {
                        index += 1;
                    }
                    if index < bytes.len() && matches!(bytes[index], b'e' | b'E') {
                        index += 1;
                        if index < bytes.len() && matches!(bytes[index], b'+' | b'-') {
                            index += 1;
                        }
                        let exponent = index;
                        while index < bytes.len() && bytes[index].is_ascii_digit() {
                            index += 1;
                        }
                        if exponent == index {
                            return Err(Diagnostic::new(
                                Span::new(line_number, start + 1, index + 1),
                                "floating literal exponent requires digits",
                            ));
                        }
                    }
                    tokens.push(token(
                        TokenKind::Float(line[start..index].into()),
                        line_number,
                        start,
                        index,
                    ));
                }
                b'\'' => {
                    index += 1;
                    if index >= bytes.len() {
                        return Err(Diagnostic::new(
                            Span::new(line_number, start + 1, index + 1),
                            "unterminated character literal",
                        ));
                    }
                    let value = if bytes[index] == b'\\' {
                        index += 1;
                        if index >= bytes.len() {
                            return Err(Diagnostic::new(
                                Span::new(line_number, start + 1, index + 1),
                                "unterminated character escape",
                            ));
                        }
                        if bytes[index] == b'x' {
                            if index + 2 >= bytes.len() {
                                return Err(Diagnostic::new(
                                    Span::new(line_number, index + 1, bytes.len() + 1),
                                    "byte escape requires two hexadecimal digits",
                                ));
                            }
                            let high = hex_digit(bytes[index + 1]);
                            let low = hex_digit(bytes[index + 2]);
                            let (Some(high), Some(low)) = (high, low) else {
                                return Err(Diagnostic::new(
                                    Span::new(line_number, index + 1, index + 4),
                                    "byte escape requires two hexadecimal digits",
                                ));
                            };
                            index += 3;
                            high * 16 + low
                        } else {
                            let escaped = match bytes[index] {
                                b'0' => 0,
                                b'n' => b'\n',
                                b'r' => b'\r',
                                b't' => b'\t',
                                b'\\' => b'\\',
                                b'\'' => b'\'',
                                _ => {
                                    return Err(Diagnostic::new(
                                        Span::new(line_number, index + 1, index + 2),
                                        "unknown character escape",
                                    ))
                                }
                            };
                            index += 1;
                            escaped
                        }
                    } else if bytes[index].is_ascii() {
                        let value = bytes[index];
                        index += 1;
                        value
                    } else {
                        return Err(Diagnostic::new(
                            Span::new(line_number, index + 1, index + 2),
                            "character literals are single-byte code units",
                        ));
                    };
                    if index >= bytes.len() || bytes[index] != b'\'' {
                        return Err(Diagnostic::new(
                            Span::new(line_number, start + 1, index + 1),
                            "character literal must contain exactly one byte",
                        ));
                    }
                    index += 1;
                    tokens.push(token(
                        TokenKind::Character(value),
                        line_number,
                        start,
                        index,
                    ));
                }
                b'(' => {
                    nesting += 1;
                    index += 1;
                    tokens.push(token(TokenKind::LeftParen, line_number, start, index));
                }
                b')' => {
                    if nesting == 0 {
                        return Err(Diagnostic::new(
                            Span::new(line_number, start + 1, start + 2),
                            "unmatched ')'",
                        ));
                    }
                    nesting -= 1;
                    index += 1;
                    tokens.push(token(TokenKind::RightParen, line_number, start, index));
                }
                b'[' => {
                    nesting += 1;
                    index += 1;
                    tokens.push(token(TokenKind::LeftBracket, line_number, start, index));
                }
                b'{' => {
                    index += 1;
                    nesting += 1;
                    tokens.push(token(TokenKind::LeftBrace, line_number, start, index));
                }
                b'}' => {
                    index += 1;
                    nesting = nesting.checked_sub(1).ok_or_else(|| {
                        Diagnostic::new(
                            Span::new(line_number, start + 1, index + 1),
                            "unmatched '}'",
                        )
                    })?;
                    tokens.push(token(TokenKind::RightBrace, line_number, start, index));
                }
                b']' => {
                    if nesting == 0 {
                        return Err(Diagnostic::new(
                            Span::new(line_number, start + 1, start + 2),
                            "unmatched ']'",
                        ));
                    }
                    nesting -= 1;
                    index += 1;
                    tokens.push(token(TokenKind::RightBracket, line_number, start, index));
                }
                b',' => {
                    index += 1;
                    tokens.push(token(TokenKind::Comma, line_number, start, index));
                }
                b';' => {
                    index += 1;
                    tokens.push(token(TokenKind::Semicolon, line_number, start, index));
                }
                b'.' => {
                    index += 1;
                    let kind = if index < bytes.len() && bytes[index] == b'.' {
                        index += 1;
                        TokenKind::Range
                    } else {
                        TokenKind::Dot
                    };
                    tokens.push(token(kind, line_number, start, index));
                }
                b':' => {
                    index += 1;
                    tokens.push(token(TokenKind::Colon, line_number, start, index));
                }
                b'+' => {
                    index += 1;
                    tokens.push(token(TokenKind::Plus, line_number, start, index));
                }
                b'-' => {
                    index += 1;
                    let kind = if index < bytes.len() && bytes[index] == b'>' {
                        index += 1;
                        TokenKind::Arrow
                    } else {
                        TokenKind::Minus
                    };
                    tokens.push(token(kind, line_number, start, index));
                }
                b'*' => {
                    index += 1;
                    tokens.push(token(TokenKind::Star, line_number, start, index));
                }
                b'&' => {
                    index += 1;
                    tokens.push(token(TokenKind::Ampersand, line_number, start, index));
                }
                b'/' => {
                    index += 1;
                    tokens.push(token(TokenKind::Slash, line_number, start, index));
                }
                b'%' => {
                    index += 1;
                    tokens.push(token(TokenKind::Percent, line_number, start, index));
                }
                b'=' => {
                    index += 1;
                    let kind = if index < bytes.len() && bytes[index] == b'=' {
                        index += 1;
                        TokenKind::EqualEqual
                    } else {
                        TokenKind::Equal
                    };
                    tokens.push(token(kind, line_number, start, index));
                }
                b'!' if index + 1 < bytes.len() && bytes[index + 1] == b'=' => {
                    index += 2;
                    tokens.push(token(TokenKind::NotEqual, line_number, start, index));
                }
                b'<' => {
                    index += 1;
                    let kind = if index < bytes.len() && bytes[index] == b'=' {
                        index += 1;
                        TokenKind::LessEqual
                    } else {
                        TokenKind::Less
                    };
                    tokens.push(token(kind, line_number, start, index));
                }
                b'>' => {
                    index += 1;
                    let kind = if index < bytes.len() && bytes[index] == b'=' {
                        index += 1;
                        TokenKind::GreaterEqual
                    } else {
                        TokenKind::Greater
                    };
                    tokens.push(token(kind, line_number, start, index));
                }
                byte if byte >= 0x80 => {
                    return Err(Diagnostic::new(
                        Span::new(line_number, start + 1, start + 2),
                        "identifiers and operators must be ASCII",
                    ));
                }
                other => {
                    return Err(Diagnostic::new(
                        Span::new(line_number, start + 1, start + 2),
                        format!("unexpected character {:?}", other as char),
                    ));
                }
            }
        }

        if nesting == 0 {
            tokens.push(token(
                TokenKind::Newline,
                line_number,
                bytes.len(),
                bytes.len(),
            ));
        }
    }

    if nesting != 0 {
        return Err(Diagnostic::new(
            Span::new(last_line, 1, 1),
            "unclosed parenthesized expression",
        ));
    }
    while indents.len() > 1 {
        indents.pop();
        tokens.push(token(TokenKind::Dedent, last_line + 1, 0, 0));
    }
    tokens.push(token(TokenKind::Eof, last_line + 1, 0, 0));
    Ok(tokens)
}

fn quoted(
    bytes: &[u8],
    line: usize,
    index: &mut usize,
    start: usize,
) -> Result<Vec<u8>, Diagnostic> {
    debug_assert_eq!(bytes[*index], b'"');
    *index += 1;
    let mut value = Vec::new();
    while *index < bytes.len() && bytes[*index] != b'"' {
        let at = *index;
        if bytes[*index] == b'\\' {
            *index += 1;
            if *index >= bytes.len() {
                return Err(Diagnostic::new(
                    Span::new(line, start + 1, *index + 1),
                    "unterminated string escape",
                ));
            }
            if bytes[*index] == b'x' {
                if *index + 2 >= bytes.len() {
                    return Err(Diagnostic::new(
                        Span::new(line, *index + 1, bytes.len() + 1),
                        "byte escape requires two hexadecimal digits",
                    ));
                }
                let high = hex_digit(bytes[*index + 1]);
                let low = hex_digit(bytes[*index + 2]);
                let Some((high, low)) = high.zip(low) else {
                    return Err(Diagnostic::new(
                        Span::new(line, *index + 1, *index + 4),
                        "byte escape requires two hexadecimal digits",
                    ));
                };
                value.push((high << 4) | low);
                *index += 3;
                continue;
            }
            let escaped = match bytes[*index] {
                b'0' => 0,
                b'n' => b'\n',
                b'r' => b'\r',
                b't' => b'\t',
                b'\\' => b'\\',
                b'"' => b'"',
                b'{' => b'{',
                b'}' => b'}',
                _ => {
                    return Err(Diagnostic::new(
                        Span::new(line, *index + 1, *index + 2),
                        "unknown string escape",
                    ))
                }
            };
            value.push(escaped);
            *index += 1;
        } else if bytes[*index].is_ascii() {
            value.push(bytes[*index]);
            *index += 1;
        } else {
            return Err(Diagnostic::new(
                Span::new(line, at + 1, at + 2),
                "string literals contain target-code-page bytes; use a byte escape",
            ));
        }
    }
    if *index >= bytes.len() {
        return Err(Diagnostic::new(
            Span::new(line, start + 1, bytes.len() + 1),
            "unterminated string literal",
        ));
    }
    *index += 1;
    Ok(value)
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indentation_is_explicit_and_blank_lines_are_invisible() {
        let tokens = lex("if true:\n    work()\n\n    # note\nnext()\n").unwrap();
        let kinds: Vec<_> = tokens.into_iter().map(|one| one.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::If,
                TokenKind::True,
                TokenKind::Colon,
                TokenKind::Newline,
                TokenKind::Indent,
                TokenKind::Identifier("work".into()),
                TokenKind::LeftParen,
                TokenKind::RightParen,
                TokenKind::Newline,
                TokenKind::Dedent,
                TokenKind::Identifier("next".into()),
                TokenKind::LeftParen,
                TokenKind::RightParen,
                TokenKind::Newline,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn a_non_enclosing_dedent_is_rejected() {
        let error = lex("if true:\n    work()\n  next()\n").unwrap_err();
        assert_eq!(error.span.line, 3);
        assert!(error.message.contains("does not match"));
    }

    #[test]
    fn lexes_every_primitive_name_and_scalar_literal_form() {
        let tokens =
            lex("char i8 u8 i16 u16 i32 u32 f32 f64 bool void 1 1.5 .25 2e3 'A' '\\x80'\n")
                .unwrap();
        let kinds: Vec<_> = tokens.into_iter().map(|one| one.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::Char,
                TokenKind::I8,
                TokenKind::U8,
                TokenKind::I16,
                TokenKind::U16,
                TokenKind::I32,
                TokenKind::U32,
                TokenKind::F32,
                TokenKind::F64,
                TokenKind::Bool,
                TokenKind::Void,
                TokenKind::Integer(1),
                TokenKind::Float("1.5".into()),
                TokenKind::Float(".25".into()),
                TokenKind::Float("2e3".into()),
                TokenKind::Character(b'A'),
                TokenKind::Character(0x80),
                TokenKind::Newline,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lexes_byte_strings_f_strings_and_fixed_array_punctuation() {
        let tokens = lex("string [i32; 2] \"A\\x80\" f\"{value}\"\n").unwrap();
        let kinds: Vec<_> = tokens.into_iter().map(|one| one.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::StringType,
                TokenKind::LeftBracket,
                TokenKind::I32,
                TokenKind::Semicolon,
                TokenKind::Integer(2),
                TokenKind::RightBracket,
                TokenKind::String(vec![b'A', 0x80]),
                TokenKind::FString(b"{value}".to_vec()),
                TokenKind::Newline,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lexes_a_range_after_an_integer_without_making_a_float() {
        let tokens = lex("0..step_count - 1\n").unwrap();
        let kinds: Vec<_> = tokens.into_iter().map(|one| one.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::Integer(0),
                TokenKind::Range,
                TokenKind::Identifier("step_count".into()),
                TokenKind::Minus,
                TokenKind::Integer(1),
                TokenKind::Newline,
                TokenKind::Eof,
            ]
        );
    }
}
